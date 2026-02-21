use std::{
    collections::HashSet,
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
    pub sync_token_signing_key: String,
    pub sync_token_issuer: String,
    pub sync_token_audience: String,
    pub sync_revoked_jtis: HashSet<String>,
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
        let sync_token_signing_key = env::var("RUNTIME_SYNC_TOKEN_SIGNING_KEY")
            .unwrap_or_else(|_| "dev-sync-key".to_string());
        let sync_token_issuer = env::var("RUNTIME_SYNC_TOKEN_ISSUER")
            .unwrap_or_else(|_| "https://openagents.com".to_string());
        let sync_token_audience = env::var("RUNTIME_SYNC_TOKEN_AUDIENCE")
            .unwrap_or_else(|_| "openagents-sync".to_string());
        let sync_revoked_jtis = env::var("RUNTIME_SYNC_REVOKED_JTIS")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToString::to_string)
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        Ok(Self {
            service_name,
            bind_addr,
            build_sha,
            authority_write_mode,
            fanout_driver,
            fanout_queue_capacity,
            sync_token_signing_key,
            sync_token_issuer,
            sync_token_audience,
            sync_revoked_jtis,
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
