//! `coder link`: make a computer a serving Coder host and link it to the
//! owner's other devices.
//!
//! One command, run on the computer or through `ssh`, takes a machine from
//! nothing to a host that devices reach over Tailscale with relay fallback:
//!
//! - [`owner`]: the owner key file. Only its public half goes to a host.
//! - [`tailscale`] and [`plan`]: a WebSocket listener on the tailnet
//!   address, with TLS from `tailscale cert` when the tailnet issues
//!   certificates, advertised as a `tailnet` hint and recorded with
//!   `coder host init`'s settings so the host service serves it.
//! - [`service`]: install the host service, move it to the staged bundle, or
//!   restart it so new settings apply.
//! - [`directory`]: list the host in the NIP-REACH owner directory.
//! - [`devices`]: this machine as a device of other hosts, sharing the
//!   Computers screens' store, and a check of every route.
//! - [`ssh`]: the same commands on another machine.
//!
//! Tailscale and SSH only introduce machines. The host alone grants rights,
//! through one-use NIP-HOST invitations whose rights are always chosen
//! explicitly, and it rechecks the grant on every channel message.
//! Invitations are shown only on the terminal that minted them or carried on
//! an SSH channel's standard streams; they never appear in an argument or a
//! log line. Read `docs/coder/guides/link-devices.md`.

use std::fmt;
use std::path::PathBuf;

pub mod cli;
pub mod devices;
pub mod directory;
pub mod owner;
pub mod plan;
pub mod service;
pub mod ssh;
pub mod tailscale;

/// Why a `coder link` step failed. Messages never carry a secret key or an
/// invitation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(String);

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<coder_access::Error> for Error {
    fn from(error: coder_access::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<coder_host::Error> for Error {
    fn from(error: coder_host::Error) -> Self {
        Self(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// `$HOME/relative`.
///
/// # Errors
/// Reports an unset `HOME`.
pub fn home(relative: &str) -> Result<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(relative))
        .ok_or_else(|| Error::new("HOME is not set"))
}
