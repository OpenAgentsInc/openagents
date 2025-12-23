//! Error types for Spark integration

use thiserror::Error;

/// Errors that can occur during Spark operations
#[derive(Debug, Error)]
pub enum SparkError {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),

    #[error("key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("initialization failed: {0}")]
    InitializationFailed(String),

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

    #[error("balance query failed: {0}")]
    BalanceQueryFailed(String),

    #[error("failed to get address: {0}")]
    GetAddressFailed(String),
}
