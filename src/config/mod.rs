use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

pub use ton_config::TonConfig;

mod ton_config;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub listen_address: SocketAddr,
    pub adnl_config: TonConfig,
}
