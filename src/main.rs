use adnl_rpc::config::Config;
use log::LevelFilter;
use std::net::{Ipv4Addr, SocketAddrV4};

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_module("warp_json_rpc::filters", LevelFilter::Trace)
        .filter_module("adnl_rpc", LevelFilter::Trace)
        .init();
    let socket = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);

    //TODO: CHANGE!!!!!!!!
    let andl_config = serde_json::json!({
        "server_address": "127.0.0.1:1234",
        "server_key" : {
            "type_id" : "1"
        }
    })
    .to_string();

    adnl_rpc::api::serve(Config {
        listen_address: socket.into(),
        adnl_config: serde_json::from_str(&andl_config).unwrap(),
    })
    .await
}
