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
