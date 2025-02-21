use thiserror::Error;

#[derive(Error, Debug)]
pub enum GroqError {
    #[error("API request failed: {0}")]
    RequestFailed(String),

    #[error("Failed to parse response: {0}")]
    ParseError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Invalid API key")]
    InvalidApiKey,
}
