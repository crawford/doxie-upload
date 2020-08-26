use anyhow::Result;
use log::debug;
use tokio::signal;

pub fn cleanup() -> Result<()> {
    Ok(())
}

pub async fn wait_for_shutdown() {
    signal::ctrl_c().await.expect("CTRL-C handler");
    debug!("CTRL-C received");
}
