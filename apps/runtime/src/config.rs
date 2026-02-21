use std::{
    env,
    net::{AddrParseError, SocketAddr},
};

use thiserror::Error;

#[derive(Clone, Debug)]
pub struct Config {
    pub service_name: String,
    pub bind_addr: SocketAddr,
    pub build_sha: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid RUNTIME_BIND_ADDR: {0}")]
    BindAddrParse(#[from] AddrParseError),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_addr = env::var("RUNTIME_BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:4100".to_string())
            .parse()?;
        let service_name =
            env::var("RUNTIME_SERVICE_NAME").unwrap_or_else(|_| "runtime".to_string());
        let build_sha = env::var("RUNTIME_BUILD_SHA").unwrap_or_else(|_| "dev".to_string());
        Ok(Self {
            service_name,
            bind_addr,
            build_sha,
        })
    }
}
