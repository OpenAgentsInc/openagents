use thiserror::Error;

/// Neobank error types
#[derive(Error, Debug)]
pub enum Error {
    /// CDK wallet error
    #[error("CDK error: {0}")]
    Cdk(#[from] cdk::error::Error),

    /// Database error
    #[error("Database error: {0}")]
    Database(String),

    /// Mint is unreachable
    #[error("Mint unreachable: {0}")]
    MintUnreachable(String),

    /// Insufficient balance for operation
    #[error("Insufficient balance: have {have}, need {need}")]
    InsufficientBalance { have: u64, need: u64 },

    /// Quote has expired
    #[error("Quote expired: {0}")]
    QuoteExpired(String),

    /// Invalid Lightning invoice
    #[error("Invalid invoice: {0}")]
    InvalidInvoice(String),

    /// Currency mismatch
    #[error("Currency mismatch: expected {expected:?}, got {got:?}")]
    CurrencyMismatch {
        expected: crate::types::Currency,
        got: crate::types::Currency,
    },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// URL parse error
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),
}

/// Result type for neobank operations
pub type Result<T> = std::result::Result<T, Error>;
