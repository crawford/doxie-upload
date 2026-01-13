// Copyright (C) 2022  Alex Crawford
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use anyhow::{Context, Result};
use futures::StreamExt;
use http_body_util::{BodyStream, Full};
use hyper::body::{self, Bytes};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{header, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use log::{debug, error, info, trace, LevelFilter};
use multer::Multipart;
use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use structopt::StructOpt;
use tokio::net::TcpListener;
use uuid::Uuid;

#[cfg_attr(feature = "container", path = "container.rs")]
#[cfg_attr(not(feature = "container"), path = "host.rs")]
mod sys;

#[derive(Debug, StructOpt)]
#[structopt(about = "Simple HTTP server that accepts file uploads and writes them to disk")]
struct Options {
    #[structopt(short, long, default_value = "127.0.0.1")]
    address: IpAddr,

    #[structopt(short, long, default_value = "8080")]
    port: u16,

    #[structopt(short, long, default_value = ".")]
    root: PathBuf,

    #[structopt(short, long, parse(from_occurrences))]
    verbosity: u8,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let opts = Arc::new(Options::from_args());

    env_logger::Builder::from_default_env()
        .filter_level(match opts.verbosity {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        })
        .format_timestamp(None)
        .init();

    let listener = TcpListener::bind((opts.address, opts.port))
        .await
        .context("binding listening socket")?;

    loop {
        tokio::select! {
            _ = sys::wait_for_shutdown() => {
                break;
            }

            res = listener.accept() => {
                let (stream, addr) = res.context("incoming connection")?;
                info!("Request from {addr}");

                let io = TokioIo::new(stream);
                let opts = opts.clone();

                tokio::task::spawn(async move {
                    if let Err(err) = http1::Builder::new()
                        .serve_connection(
                            io,
                            service_fn(move |req| handle_request(opts.clone(), req)),
                        )
                        .await
                    {
                        error!("Failed to serve connection: {err:?}")
                    }
                });
            }
        }
    }

    sys::cleanup()
}

async fn handle_request(
    opts: Arc<Options>,
    req: Request<body::Incoming>,
) -> Result<Response<Full<Bytes>>> {
    let Ok(boundary) = req
        .headers()
        .get(header::CONTENT_TYPE)
        .context("reading Content-Type")
        .and_then(|ct| ct.to_str().context("parsing as string"))
        .and_then(|ct| multer::parse_boundary(ct).context("parsing multipart boundary"))
    else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::from("invalid multipart boundary"))
            .context("building response");
    };

    match handle_multipart(&opts, req.into_body(), &boundary)
        .await
        .context("handling multipart form")
    {
        Ok(Some(path)) => Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Full::from(format!("Uploaded {}", path.display())))
            .context("creating response")?),
        Ok(None) => Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::from("No file in request"))
            .context("creating response")?),
        Err(err) => {
            error!("{:#}", err);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::new()))
                .context("creating response")?)
        }
    }
}

async fn handle_multipart(
    opts: &Options,
    body: body::Incoming,
    boundary: &str,
) -> Result<Option<PathBuf>> {
    let body_stream = BodyStream::new(body)
        .filter_map(|result| async move { result.map(|frame| frame.into_data().ok()).transpose() });
    let mut multipart = Multipart::new(body_stream, boundary);

    while let Some(mut field) = multipart.next_field().await.context("next form field")? {
        let filename = match field.name() {
            Some("file") => field.file_name(),
            Some(name) => {
                debug!("Ignoring unexpected field '{name}'");
                continue;
            }
            None => {
                debug!("Ignoring unnamed field");
                continue;
            }
        };

        let extension = filename
            .map(PathBuf::from)
            .and_then(|f| f.extension().map(|e| e.to_os_string().to_ascii_lowercase()))
            .unwrap_or_else(|| OsString::from("pdf"));
        let path = opts
            .root
            .join(PathBuf::from(Uuid::new_v4().to_simple().to_string()).with_extension(extension));

        let mut upload =
            File::create(&path).with_context(|| format!("creating file ({})", path.display()))?;

        while let Some(chunk) = field.chunk().await.context("next field chunk")? {
            trace!("Got field chunk, len: {:?}", chunk.len());
            upload
                .write_all(&chunk)
                .with_context(|| format!("writing file ({})", path.display()))?
        }

        info!("Created {}", path.display());

        return Ok(Some(path));
    }

    Ok(None)
}
