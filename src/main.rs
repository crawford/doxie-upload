use anyhow::{Error, Result};
use hyper::server::conn::AddrStream;
use hyper::{service, Body, Request, Response, Server, StatusCode};
use log::{debug, info, trace, LevelFilter};
use multipart_async::{server::Multipart, BodyChunk};
use std::fs::File;
use std::io::Write;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use structopt::StructOpt;
use tokio::stream::StreamExt;
use uuid::Uuid;

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
                    handle_request(opts.clone(), req)
                }))
            }
        }))
        .await?;

    Ok(())
}

async fn handle_request(opts: Arc<Options>, req: Request<Body>) -> Result<Response<Body>> {
    Ok(match Multipart::try_from_request(req) {
        Ok(multipart) => match handle_multipart(&opts, multipart).await {
            Ok(Some(path)) => Response::builder()
                .status(StatusCode::CREATED)
                .body(Body::from(format!("Uploaded {}", path.display())))?,
            Ok(None) => Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("No file in request"))?,
            Err(err) => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(err.to_string()))?,
        },
        Err(_) => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Expecting multipart/form-data"))?,
    })
}

async fn handle_multipart(
    opts: &Options,
    mut multipart: Multipart<Body>,
) -> Result<Option<PathBuf>> {
    while let Some(mut field) = multipart.next_field().await? {
        if field.headers.name != "file" {
            debug!(r#"Ignoring unexpected field "{}""#, field.headers.name);
            continue;
        }

        let path = field
            .headers
            .filename
            .and_then(|path| PathBuf::from(path).file_name().map(PathBuf::from))
            .unwrap_or_else(|| Uuid::new_v4().to_simple().to_string().into());

        let mut upload = File::create(opts.root.join(&path))?;
        while let Some(chunk) = field.data.try_next().await? {
            trace!("Got field chunk, len: {:?}", chunk.len());
            upload.write_all(chunk.as_slice())?
        }

        info!("Created {}", path.display());

        return Ok(Some(path));
    }

    Ok(None)
}
