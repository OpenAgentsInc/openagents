//! Vibe error types

use thiserror::Error;

/// Errors that can occur in Vibe operations
#[derive(Error, Debug)]
pub enum VibeError {
    /// OANIX error
    #[error("oanix error: {0}")]
    Oanix(#[from] oanix::OanixError),

    /// IDE error
    #[error("ide error: {0}")]
    Ide(String),

    /// Dev runtime error
    #[error("dev runtime error: {0}")]
    DevRuntime(String),

    /// Backend build error
    #[error("backend build error: {0}")]
    BackendBuild(String),

    /// Configuration error
    #[error("config error: {0}")]
    Config(String),

    /// Agent job error
    #[error("agent error: {0}")]
    Agent(String),
}
