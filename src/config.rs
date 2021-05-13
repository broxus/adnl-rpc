use adnl::client::AdnlClientConfigJson;
use std::net::SocketAddr;

pub struct Config {
    pub listen_address: SocketAddr,
    pub adnl_config: AdnlClientConfigJson,
}
