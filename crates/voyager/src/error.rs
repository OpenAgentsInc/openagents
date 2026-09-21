//! The failures an episode can report.
//!
//! Every variant names a condition a caller can act on; none is a bare
//! number. A [`crate::bridge::Bridge`] fault and a [`crate::server::Server`]
//! fault are distinct because the answer to each is different.

use std::io;

/// One kind of failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A filesystem operation failed.
    #[error("{0}")]
    Io(#[from] io::Error),
    /// JSON did not encode or decode.
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    /// A world manifest is missing, malformed, or mismatched.
    #[error("world: {0}")]
    World(String),
    /// The Minecraft server failed: it would not start, never became
    /// ready, or exited early.
    #[error("server: {0}")]
    Server(String),
    /// The `mc-bridge` helper faulted: it would not start, closed a pipe,
    /// answered out of order, or missed a deadline.
    #[error("bridge: {0}")]
    Bridge(String),
    /// The helper understood the request and refused it — a typed
    /// failure in the world, like `no_blocks` or `timeout`.
    #[error("{code}: {message}")]
    Refused {
        /// The helper's machine-readable refusal.
        code: String,
        /// What it said went wrong.
        message: String,
    },
    /// An episode bound was reached: too many actions, too long, or a
    /// task the agent could not finish.
    #[error("episode: {0}")]
    Episode(String),
    /// The Nostr relay faulted: it would not start, the socket closed,
    /// or a call never answered.
    #[error("relay: {0}")]
    Relay(String),
    /// The decision door faulted or refused a question.
    #[error("decision: {0}")]
    Decision(String),
    /// No Java runtime was found where the search said one would be.
    #[error("java: {0}")]
    Java(String),
}

/// What a voyager call returns.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Builds a world error.
    #[must_use]
    pub fn world(message: impl Into<String>) -> Self {
        Error::World(message.into())
    }

    /// Builds a server error.
    #[must_use]
    pub fn server(message: impl Into<String>) -> Self {
        Error::Server(message.into())
    }

    /// Builds a bridge error.
    #[must_use]
    pub fn bridge(message: impl Into<String>) -> Self {
        Error::Bridge(message.into())
    }

    /// Builds an episode error.
    #[must_use]
    pub fn episode(message: impl Into<String>) -> Self {
        Error::Episode(message.into())
    }

    /// Builds a relay error.
    #[must_use]
    pub fn relay(message: impl Into<String>) -> Self {
        Error::Relay(message.into())
    }

    /// Builds a decision error.
    #[must_use]
    pub fn decision(message: impl Into<String>) -> Self {
        Error::Decision(message.into())
    }
}
