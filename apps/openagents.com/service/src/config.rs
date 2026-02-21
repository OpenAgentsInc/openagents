use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use thiserror::Error;

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8787";
const DEFAULT_LOG_FILTER: &str = "info";
const DEFAULT_STATIC_DIR: &str = "../web-shell/dist";

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub log_filter: String,
    pub static_dir: PathBuf,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid OA_CONTROL_BIND_ADDR value '{value}': {source}")]
    InvalidBindAddr {
        value: String,
        source: std::net::AddrParseError,
    },
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_addr_raw = env::var("OA_CONTROL_BIND_ADDR")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BIND_ADDR.to_string());

        let bind_addr = bind_addr_raw
            .parse()
            .map_err(|source| ConfigError::InvalidBindAddr {
                value: bind_addr_raw,
                source,
            })?;

        let log_filter = env::var("OA_CONTROL_LOG_FILTER")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_LOG_FILTER.to_string());

        let static_dir = env::var("OA_CONTROL_STATIC_DIR")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_STATIC_DIR));

        Ok(Self {
            bind_addr,
            log_filter,
            static_dir,
        })
    }
}
