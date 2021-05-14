use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

pub use ton_config::AdnlConfig;

mod ton_config;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub listen_address: SocketAddr,

    #[serde(default = "default_logger_settings")]
    pub logger_settings: serde_yaml::Value,

    pub adnl_config: AdnlConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_address: "127.0.0.1:9000".parse().unwrap(),
            logger_settings: default_logger_settings(),
            adnl_config: AdnlConfig::default_mainnet_config(),
        }
    }
}

fn default_logger_settings() -> serde_yaml::Value {
    const DEFAULT_LOG4RS_SETTINGS: &str = r##"
    appenders:
      stdout:
        kind: console
        encoder:
          pattern: "{d(%Y-%m-%d %H:%M:%S %Z)(utc)} - {h({l})} {M} {f}:{L} = {m} {n}"
    root:
      level: error
      appenders:
        - stdout
    loggers:
      adnl_rpc:
        level: debug
        appenders:
          - stdout
        additive: false
    "##;
    serde_yaml::from_str(DEFAULT_LOG4RS_SETTINGS).unwrap()
}
