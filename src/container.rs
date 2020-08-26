use anyhow::Result;
use log::{debug, trace};
use nix::{errno::Errno, sys::wait};
use tokio::signal::unix::{self, SignalKind};

pub fn cleanup() -> Result<()> {
    trace!("Reaping orphaned children");
    loop {
        match wait::wait() {
            Ok(status) => trace!("Child status: {:#?}", status),
            Err(nix::Error::Sys(Errno::ECHILD)) => break Ok(()),
            Err(err) => break Err(err.into()),
        }
    }
}

pub async fn wait_for_shutdown() {
    unix::signal(SignalKind::terminate())
        .expect("SIGTERM handler")
        .recv()
        .await;
    debug!("SIGTERM received");
}
