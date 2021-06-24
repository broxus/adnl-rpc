use std::convert::TryFrom;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tiny_adnl::AdnlTcpClientConfig;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdnlConfig {
    pub server_address: SocketAddrV4,
    pub server_key: String,
    pub socket_timeout_ms: u64,
}

impl TryFrom<AdnlConfig> for AdnlTcpClientConfig {
    type Error = anyhow::Error;

    fn try_from(c: AdnlConfig) -> Result<Self, Self::Error> {
        let server_key = base64::decode(&c.server_key)?;

        Ok(AdnlTcpClientConfig {
            server_address: c.server_address,
            server_key: ed25519_dalek::PublicKey::from_bytes(&server_key)?,
            socket_read_timeout: Duration::from_millis(c.socket_timeout_ms),
            socket_send_timeout: Duration::from_millis(c.socket_timeout_ms),
        })
    }
}

impl AdnlConfig {
    pub fn default_mainnet_config() -> AdnlConfig {
        AdnlConfig {
            server_address: SocketAddrV4::new(Ipv4Addr::new(54, 158, 97, 195), 3031),
            server_key: "uNRRL+6enQjuiZ/s6Z+vO7yxUUR7uxdfzIy+RxkECrc=".to_owned(),
            socket_timeout_ms: 20000,
        }
    }

    pub fn default_testnet_config() -> AdnlConfig {
        AdnlConfig {
            server_address: SocketAddrV4::new(Ipv4Addr::new(54, 158, 97, 195), 3032),
            server_key: "uNRRL+6enQjuiZ/s6Z+vO7yxUUR7uxdfzIy+RxkECrc=".to_owned(),
            socket_timeout_ms: 20000,
        }
    }
}
