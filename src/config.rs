use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_address: SocketAddr,
}
