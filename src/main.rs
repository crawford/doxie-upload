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

use anyhow::{Context, Error, Result};
use futures::future::FutureExt;
use hyper::server::conn::AddrStream;
use hyper::{service, Body, Request, Response, Server, StatusCode};
use log::{debug, error, info, trace, LevelFilter};
use multipart_async::{server::Multipart, BodyChunk};
use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use structopt::StructOpt;
use tokio::stream::StreamExt;
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

#[tokio::main]
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

    Server::bind(&(opts.address, opts.port).into())
        .serve(service::make_service_fn(|socket: &AddrStream| {
            info!("Request from {}", socket.remote_addr());

            let opts = opts.clone();
            async move {
                Ok::<_, Error>(service::service_fn(move |req| {
                    handle_request(opts.clone(), req).inspect(|resp| debug!("Response {:?}", resp))
                }))
            }
        }))
        .with_graceful_shutdown(sys::wait_for_shutdown())
        .await?;

    sys::cleanup()
}

async fn handle_request(opts: Arc<Options>, req: Request<Body>) -> Result<Response<Body>> {
    match Multipart::try_from_request(req) {
        Ok(multipart) => match handle_multipart(&opts, multipart)
            .await
            .context("handling multipart form")
        {
            Ok(Some(path)) => Ok(Response::builder()
                .status(StatusCode::OK)
                .body(Body::from(format!("Uploaded {}", path.display())))
                .context("creating response")?),
            Ok(None) => Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("No file in request"))
                .context("creating response")?),
            Err(err) => {
                error!("{:#}", err);
                Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .context("creating response")?)
            }
        },
        Err(_) => Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Expecting multipart/form-data"))
            .context("creating response")?),
    }
}

async fn handle_multipart(
    opts: &Options,
    mut multipart: Multipart<Body>,
) -> Result<Option<PathBuf>> {
    while let Some(mut field) = multipart.next_field().await.context("next form field")? {
        if field.headers.name != "file" {
            debug!(r#"Ignoring unexpected field "{}""#, field.headers.name);
            continue;
        }

        let extension = field
            .headers
            .filename
            .map(PathBuf::from)
            .and_then(|f| f.extension().map(|e| e.to_os_string()))
            .unwrap_or_else(|| OsString::from("pdf"));
        let filename =
            PathBuf::from(Uuid::new_v4().to_simple().to_string()).with_extension(extension);
        let path = opts.root.join(&filename);

        let mut upload =
            File::create(&path).with_context(|| format!("creating file ({})", path.display()))?;

        while let Some(chunk) = field.data.try_next().await.context("next field chunk")? {
            trace!("Got field chunk, len: {:?}", chunk.len());
            upload
                .write_all(chunk.as_slice())
                .with_context(|| format!("writing file ({})", path.display()))?
        }

        info!("Created {}", filename.display());

        return Ok(Some(filename));
    }

    Ok(None)
}
