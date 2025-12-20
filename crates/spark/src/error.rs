//! Error types for Spark integration

use thiserror::Error;

/// Errors that can occur during Spark operations
#[derive(Debug, Error)]
pub enum SparkError {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),

    #[error("key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("wallet error: {0}")]
    Wallet(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("invalid address: {0}")]
    InvalidAddress(String),

    #[error("insufficient funds: {0}")]
    InsufficientFunds(String),

    #[error("payment failed: {0}")]
    PaymentFailed(String),
}
