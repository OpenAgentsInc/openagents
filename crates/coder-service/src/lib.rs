//! The Coder host as a background service.
//!
//! This crate owns three things that let one Coder host keep running on a
//! computer and update without losing state:
//!
//! - [`service`] renders, installs, inspects, restarts, and removes the
//!   host's background service: a systemd user unit on Linux, with linger
//!   checks, and a launchd agent on macOS.
//! - [`launcher`] runs the host under that service. It runs a new version
//!   as a *trial* against a snapshot of the host's state, commits the trial
//!   when the host reports ready before a deadline, and otherwise restores
//!   the snapshot and returns to the previous version. Every step is a
//!   durable record, so a launcher that stops in any state recovers to a
//!   committed version on its next start.
//! - [`descriptor`] is what a client reads before it assumes anything
//!   about a host: its key, protocol version, generation, capability
//!   flags, and the state of the latest update.
//!
//! [`snapshot`] copies and restores the host's state directories for the
//! launcher. [`bundle`] reads the immutable, digest-named bundles that
//! `scripts/coder-host.py` stages; this crate never stages a binary.
//!
//! Unix only: the launcher owns its host through a process group, as the
//! `supervise` crate does. Read `docs/coder/runtime/host-service.md` before
//! you change a state transition, a rendered unit, or the descriptor.

pub mod bundle;
pub mod descriptor;
pub mod fsx;
pub mod launcher;
pub mod service;
pub mod snapshot;

use std::fmt;

/// Why an operation refused or failed.
#[derive(Debug)]
pub enum Error {
    /// The input, the configuration, or the retained state is not one this
    /// crate accepts. Nothing changed because of it.
    Refused(String),
    /// The operating system reported an error.
    Io(std::io::Error),
    /// A test stopped the launcher at a named point to stand in for a
    /// crash. Production code never produces this.
    Crash(&'static str),
}

impl Error {
    pub(crate) fn refused(message: impl Into<String>) -> Self {
        Error::Refused(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Refused(message) => write!(f, "{message}"),
            Error::Io(error) => write!(f, "{error}"),
            Error::Crash(point) => write!(f, "stopped at {point}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Refused(format!("malformed record: {error}"))
    }
}

/// The result type of this crate.
pub type Result<T> = std::result::Result<T, Error>;
