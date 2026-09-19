use std::{fmt, io};

use crate::store::StoreError;

#[derive(Debug)]
pub enum GatewayError {
    Config(String),
    Io(io::Error),
    Store(StoreError),
    WebSocket(tokio_tungstenite::tungstenite::Error),
    Internal(String),
}

impl fmt::Display for GatewayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(reason) => write!(f, "configuration error: {reason}"),
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Store(error) => write!(f, "store error: {error}"),
            Self::WebSocket(error) => write!(f, "WebSocket error: {error}"),
            Self::Internal(reason) => write!(f, "gateway invariant failed: {reason}"),
        }
    }
}

impl std::error::Error for GatewayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::WebSocket(error) => Some(error),
            Self::Config(_) | Self::Internal(_) => None,
        }
    }
}

impl From<io::Error> for GatewayError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<StoreError> for GatewayError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for GatewayError {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::WebSocket(error)
    }
}
