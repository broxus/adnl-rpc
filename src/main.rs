use adnl_rpc::*;
use adnl_rpc::config::Config;
use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
use log::LevelFilter;

#[tokio::main]
async fn main() {
    env_logger::Builder::new().filter_module("warp_json_rpc::filters",LevelFilter::Trace).filter_module("adnl_rpc",LevelFilter::Trace).init();
let socket=    SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);

    adnl_rpc::api::serve(Config {
        listen_address: socket.into()
    }).await
}
