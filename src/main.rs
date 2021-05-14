use anyhow::Result;
use log::LevelFilter;
use std::net::{Ipv4Addr, SocketAddrV4};

use adnl_rpc::config::{Config, TonConfig};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_module("warp_json_rpc::filters", LevelFilter::Trace)
        .filter_module("adnl_rpc", LevelFilter::Trace)
        .init();
    let socket = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);

    adnl_rpc::api::serve(Config {
        listen_address: socket.into(),
        adnl_config: TonConfig::default(),
    })
    .await
}
