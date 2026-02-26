use thiserror::Error;

use crate::wallet::Network;

#[derive(Debug, Error)]
pub enum SparkError {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),

    #[error("key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("wallet initialization failed: {0}")]
    InitializationFailed(String),

    #[error("wallet operation failed: {0}")]
    Wallet(String),

    #[error("invalid payment request: {0}")]
    InvalidPaymentRequest(String),

    #[error("unsupported Spark network: {0:?}")]
    UnsupportedNetwork(Network),
}
