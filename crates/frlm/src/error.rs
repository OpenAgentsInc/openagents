//! FRLM error types.

use thiserror::Error;

/// FRLM result type.
pub type Result<T> = std::result::Result<T, FrlmError>;

/// Errors that can occur during FRLM execution.
#[derive(Debug, Error)]
pub enum FrlmError {
    /// Budget exceeded
    #[error("budget exceeded: {spent} sats spent, {limit} sats limit")]
    BudgetExceeded { spent: u64, limit: u64 },

    /// Timeout waiting for sub-query results
    #[error("timeout waiting for sub-query results: {received}/{expected} received")]
    Timeout { received: usize, expected: usize },

    /// Quorum not met
    #[error("quorum not met: {received}/{required} results")]
    QuorumNotMet { received: usize, required: usize },

    /// Verification failed
    #[error("verification failed: {reason}")]
    VerificationFailed { reason: String },

    /// Sub-query execution failed
    #[error("sub-query {query_id} failed: {error}")]
    SubQueryFailed { query_id: String, error: String },

    /// No providers available
    #[error("no providers available for sub-query")]
    NoProviders,

    /// Local RLM error
    #[error("local RLM error: {0}")]
    RlmError(#[from] rlm::RlmError),

    /// Nostr error
    #[error("Nostr error: {0}")]
    NostrError(String),

    /// Invalid program
    #[error("invalid FRLM program: {0}")]
    InvalidProgram(String),

    /// Internal error
    #[error("internal error: {0}")]
    Internal(String),
}
