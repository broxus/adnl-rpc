use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub use ton_config::AdnlConfig;

mod ton_config;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub listen_address: SocketAddr,

    #[serde(default = "default_logger_settings")]
    pub logger_settings: serde_yaml::Value,

    pub adnl_config: AdnlConfig,

    pub max_unreliability: usize,

    pub max_time_diff: u32,

    pub max_connection_count: u32,

    pub min_idle_connection_count: Option<u32>,

    #[serde(with = "serde_time")]
    pub last_block_cache_duration: Duration,

    #[serde(with = "serde_time")]
    pub indexer_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_address: "127.0.0.1:9000".parse().unwrap(),
            logger_settings: default_logger_settings(),
            adnl_config: AdnlConfig::default_mainnet_config(),
            max_unreliability: 30,
            max_time_diff: 120,
            max_connection_count: 100,
            min_idle_connection_count: Some(5),
            last_block_cache_duration: Duration::from_secs(1),
            indexer_interval: Duration::from_secs(10),
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

pub mod serde_time {
    use super::*;

    use serde::de::Error;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DurationValue {
        Number(u64),
        String(String),
    }

    pub fn serialize<S>(data: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(data.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value: DurationValue = serde::Deserialize::deserialize(deserializer)?;
        match value {
            DurationValue::Number(seconds) => Ok(Duration::from_secs(seconds)),
            DurationValue::String(string) => {
                let string = string.trim();

                let seconds = if string.chars().all(|c| c.is_digit(10)) {
                    u64::from_str(string).map_err(D::Error::custom)?
                } else {
                    humantime::Duration::from_str(string)
                        .map_err(D::Error::custom)?
                        .as_secs()
                };

                Ok(Duration::from_secs(seconds))
            }
        }
    }
}
