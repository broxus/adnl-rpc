use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use adnl::client;
use adnl::client::AdnlClientConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TonConfig {
    pub server_address: SocketAddr,
    pub server_key: String,
}

impl TonConfig {
    pub fn tonlib_config(&self) -> Result<client::AdnlClientConfig> {
        let json = serde_json::json!({
            "client_key": serde_json::Value::Null,
            "server_address": self.server_address.to_string(),
            "client_key": serde_json::Value::Null,
            "server_key": {
                "type_id": adnl::common::KeyOption::KEY_ED25519,
                "pub_key": self.server_key.clone(),
                "pvt_key": serde_json::Value::Null,
            },
            "timeouts": adnl::common::Timeouts::default()
        });
        AdnlClientConfig::from_json(&json.to_string()).map_err(|e| anyhow::Error::msg(e.to_string()))
    }
}


impl Default for TonConfig {
    fn default() -> Self {
        Self::default_testnet_config()
    }
}

impl TonConfig{
    pub fn default_mainnet_config() -> TonConfig {
        TonConfig {
            server_address: SocketAddrV4::new(Ipv4Addr::new(54, 158, 97, 195), 3031).into(),
            server_key: "uNRRL+6enQjuiZ/s6Z+vO7yxUUR7uxdfzIy+RxkECrc=".to_owned(),
        }
    }

    pub fn default_testnet_config() -> TonConfig {
        TonConfig {
            server_address: SocketAddrV4::new(Ipv4Addr::new(54, 158, 97, 195), 3032).into(),
            server_key: "uNRRL+6enQjuiZ/s6Z+vO7yxUUR7uxdfzIy+RxkECrc=".to_owned(),
        }
    }
}

