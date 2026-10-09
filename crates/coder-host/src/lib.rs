//! The resident Coder host and the client that connects to it.
//!
//! One host process composes the remote-access profiles:
//!
//! - NIP-HOST enrollment and grants from `coder-access`. The host's access
//!   store holds the one host key; every record the host signs names it.
//! - NIP-REACH presence, reachability hints, and the direct channel from
//!   `coder-reach`, with the real grant store behind its grant check.
//! - NIP-TERM terminals from `coder-pty`, with NIP-HOST rights behind its
//!   rights check and the direct channel or relay artifacts behind its
//!   frame sink.
//! - Task creation, steering, and cancellation through a [`tasks::Tasks`]
//!   owner. The `coder` binary supplies its durable task inbox.
//! - NIP-WS activity summaries for task changes.
//!
//! [`serve::start`] runs the host. [`client`] reaches it over the best route
//! a device can prove, with relay fallback, and [`client::Connector`] plugs
//! that into a `coder-link` registry. The default `host` feature builds the
//! host; a client build disables it and keeps [`client`]. Read `crates/coder-host/README.md` and
//! `docs/coder/runtime/host-serve.md` before changing a binding.

use std::fmt;

#[cfg(feature = "host")]
pub mod authority;
#[cfg(all(feature = "host", unix))]
pub mod background;
#[cfg(feature = "host")]
pub mod cli;
pub mod client;
pub mod cloud;
#[cfg(feature = "host")]
pub mod compute;
#[cfg(feature = "host")]
pub mod computer;
#[cfg(feature = "host")]
pub mod config;
#[cfg(feature = "host")]
pub mod control;
#[cfg(feature = "host")]
pub mod enroll;
#[cfg(feature = "host")]
pub mod generation;
pub mod mailbox;
pub mod message;
pub mod nudge;
pub mod projects;
#[cfg(feature = "host")]
mod publish;
#[cfg(feature = "host")]
pub mod serve;
#[cfg(feature = "host")]
pub mod sessions;
#[cfg(feature = "host")]
pub mod settings;
pub mod share;
#[cfg(feature = "host")]
pub mod spend;
pub mod tailnet;
pub mod tasks;
#[cfg(feature = "host")]
pub mod telemetry;
#[cfg(feature = "host")]
pub mod terminal_sessions;
#[cfg(feature = "host")]
mod tls;
#[cfg(feature = "verse-assets")]
pub mod verse_private;
#[cfg(feature = "host")]
pub mod wallet_link;

/// The composed profiles, re-exported so a client names one dependency.
pub use {coder_access as access, coder_link as link, coder_pty as pty, coder_reach as reach};

pub use coder_access::Code;
pub use coder_access::protocol::{
    CommandAction, QueueEdit, QueueItem, QueueLease, TaskCommand, TaskCreate, TaskQueue,
};
#[cfg(feature = "host")]
pub use config::Config;
#[cfg(feature = "host")]
pub use serve::{Running, start};
pub use tasks::{
    AgentReport, GoalDecision, NoTasks, Note, Passed, Principal, Reviewed, Standing, StartCause,
    TaskRef, Tasks,
};

/// The host protocol version the ready record and presence report.
pub const PROTOCOL_VERSION: u32 = coder_reach::PROTOCOL_VERSION;

/// Capability flags this host advertises in presence and its ready record.
/// `task-engine` says its `task.create` accepts the engine the person asked
/// for (#10081). `term-effects` says terminals answer their programs'
/// queries on the host and report bells, titles, and clipboard writes as
/// effect frames (NIP-TERM's effects feature); `term-snapshot` says a device
/// can join a terminal by snapshot and read older history; `term-blocks`
/// says it can page through a terminal's block journal; `term-typist` says
/// one attachment types at a time, and take and release move the role;
/// `term-shares` says a device with `terminal` can share one terminal with
/// another device key to watch or drive; `term-sessions` says the host keeps
/// session records with their members and default layout across restarts.
pub const CAPABILITIES: [&str; 13] = [
    "activity-summary",
    "direct-tcp",
    "relay-control",
    "task-control",
    "task-create",
    coder_access::protocol::TASK_ENGINE,
    "terminal",
    coder_pty::ext::CAPABILITY_EFFECTS,
    coder_pty::ext::CAPABILITY_SNAPSHOT,
    coder_pty::ext::CAPABILITY_BLOCKS,
    coder_pty::ext::CAPABILITY_TYPIST,
    coder_pty::share::CAPABILITY_SHARES,
    coder_pty::ext::CAPABILITY_SESSIONS,
];

/// Why a host or client operation failed. Messages carry no key, grant,
/// invitation, prompt, or terminal content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The configuration or a local precondition is wrong.
    Config(String),
    /// A NIP-HOST refusal, or a local access-store failure.
    Access(coder_access::Error),
    /// A NIP-REACH refusal.
    Reach(coder_reach::Error),
    /// A NIP-TERM refusal.
    Terminal(coder_pty::Refusal),
    /// A relay or socket failure.
    Transport(String),
    /// The direct channel closed. The code is the one the host sent, such as
    /// `revoked`, when it sent one after proving its key.
    Closed(Option<String>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(message) => write!(f, "configuration: {message}"),
            Self::Access(error) => write!(f, "host access: {error}"),
            Self::Reach(error) => write!(f, "host reach: {error}"),
            Self::Terminal(refusal) => write!(f, "terminal: {refusal}"),
            Self::Transport(message) => write!(f, "transport: {message}"),
            Self::Closed(Some(code)) => write!(f, "the host closed the channel: {code}"),
            Self::Closed(None) => f.write_str("the direct channel closed"),
        }
    }
}

impl std::error::Error for Error {}

impl From<coder_access::Error> for Error {
    fn from(error: coder_access::Error) -> Self {
        Self::Access(error)
    }
}

impl From<coder_reach::Error> for Error {
    fn from(error: coder_reach::Error) -> Self {
        Self::Reach(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Unix seconds from the system clock.
pub fn unix_time() -> Result<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| Error::Config("the system clock is before 1970".into()))
}
