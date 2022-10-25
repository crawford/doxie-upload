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
use log::debug;
use tokio::signal;

pub fn cleanup() -> Result<()> {
    Ok(())
}

pub async fn wait_for_shutdown() {
    signal::ctrl_c().await.expect("CTRL-C handler");
    debug!("CTRL-C received");
}
