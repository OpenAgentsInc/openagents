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
    pub authority_write_mode: AuthorityWriteMode,
    pub fanout_driver: String,
    pub fanout_queue_capacity: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorityWriteMode {
    RustActive,
    ShadowOnly,
    ReadOnly,
}

impl AuthorityWriteMode {
    #[must_use]
    pub fn writes_enabled(&self) -> bool {
        matches!(self, Self::RustActive)
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RustActive => "rust_active",
            Self::ShadowOnly => "shadow_only",
            Self::ReadOnly => "read_only",
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid RUNTIME_BIND_ADDR: {0}")]
    BindAddrParse(#[from] AddrParseError),
    #[error("invalid RUNTIME_AUTHORITY_WRITE_MODE: {0}")]
    InvalidAuthorityWriteMode(String),
    #[error("invalid RUNTIME_FANOUT_QUEUE_CAPACITY: {0}")]
    InvalidFanoutQueueCapacity(String),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_addr = env::var("RUNTIME_BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:4100".to_string())
            .parse()?;
        let service_name =
            env::var("RUNTIME_SERVICE_NAME").unwrap_or_else(|_| "runtime".to_string());
        let build_sha = env::var("RUNTIME_BUILD_SHA").unwrap_or_else(|_| "dev".to_string());
        let authority_write_mode = parse_authority_write_mode(
            env::var("RUNTIME_AUTHORITY_WRITE_MODE")
                .unwrap_or_else(|_| "rust_active".to_string())
                .as_str(),
        )?;
        let fanout_driver =
            env::var("RUNTIME_FANOUT_DRIVER").unwrap_or_else(|_| "memory".to_string());
        let fanout_queue_capacity = env::var("RUNTIME_FANOUT_QUEUE_CAPACITY")
            .unwrap_or_else(|_| "1024".to_string())
            .parse::<usize>()
            .map_err(|error| ConfigError::InvalidFanoutQueueCapacity(error.to_string()))?;
        Ok(Self {
            service_name,
            bind_addr,
            build_sha,
            authority_write_mode,
            fanout_driver,
            fanout_queue_capacity,
        })
    }
}

fn parse_authority_write_mode(raw: &str) -> Result<AuthorityWriteMode, ConfigError> {
    match raw.trim().to_lowercase().as_str() {
        "rust_active" => Ok(AuthorityWriteMode::RustActive),
        "shadow_only" => Ok(AuthorityWriteMode::ShadowOnly),
        "read_only" => Ok(AuthorityWriteMode::ReadOnly),
        other => Err(ConfigError::InvalidAuthorityWriteMode(other.to_string())),
    }
}
