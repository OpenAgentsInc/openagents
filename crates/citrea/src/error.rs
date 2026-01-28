use thiserror::Error;

#[derive(Debug, Error)]
pub enum CitreaError {
    #[error("invalid hex: {0}")]
    InvalidHex(String),
    #[error("invalid length: expected {expected} bytes, got {actual} bytes")]
    InvalidLength { expected: usize, actual: usize },
    #[error("invalid chain id: {0}")]
    InvalidChainId(String),
    #[error("key derivation error: {0}")]
    KeyDerivation(String),
    #[error("rpc error {code}: {message}")]
    Rpc {
        code: i64,
        message: String,
        data: Option<serde_json::Value>,
    },
    #[error("rpc response missing result")]
    MissingResult,
    #[error("http error: {0}")]
    Http(String),
    #[error("serialization error: {0}")]
    Serde(String),
    #[error("secp256k1 error: {0}")]
    Secp(String),
}

impl From<hex::FromHexError> for CitreaError {
    fn from(err: hex::FromHexError) -> Self {
        Self::InvalidHex(err.to_string())
    }
}

impl From<reqwest::Error> for CitreaError {
    fn from(err: reqwest::Error) -> Self {
        Self::Http(err.to_string())
    }
}

impl From<serde_json::Error> for CitreaError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serde(err.to_string())
    }
}

impl From<bitcoin::secp256k1::Error> for CitreaError {
    fn from(err: bitcoin::secp256k1::Error) -> Self {
        Self::Secp(err.to_string())
    }
}
