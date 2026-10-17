//! Run keepers: agent processes that outlive the engine that started them
//! (#11930, `docs/desktop/move-running-agents.md`).
//!
//! A keeper is the app binary run as `openagents keeper` for one run folder. It
//! starts in a session of its own, starts the agent in a process group of its
//! own, and holds the agent's stdin, stdout, and stderr. It journals every
//! line with a sequence number and serves the run over a Unix socket that
//! only this user can open, authenticated by a token in a file that only this
//! user can read.
//!
//! An engine talks to the agent only through the keeper ([`client::Link`]).
//! When the engine exits, the keeper keeps the agent running and keeps
//! journaling. The next engine finds the run folder ([`client::discover`]),
//! connects, and receives every frame after the last one it acknowledged,
//! plus any frame a driver is holding, such as a question the agent still
//! waits on. A keeper with no engine for [`ORPHAN_AFTER`] interrupts its
//! agent and exits, keeping the journal.
//!
//! Drivers opt in. A driver that can start under a keeper reads the
//! engine's [`KeeperScope`] while it launches, and a driver that can pick a
//! run back up implements [`crate::Harness::attach`]. [`lines::LineHarness`]
//! does both for agents that write OpenAgents events as JSON lines.

pub mod client;
pub mod lines;
pub mod protocol;
mod server;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub use client::{Found, Link, LinkSender, Spawn};
pub use protocol::{Frame, FrameBody, KeeperSignal};
pub use server::main;

/// How long a keeper keeps its agent running with no engine connected.
pub const ORPHAN_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

/// The first argument that turns the app binary into a keeper.
pub const SUBCOMMAND: &str = "keeper";

/// What an engine lends a driver while the driver starts a run: where run
/// folders live, the binary that runs keepers, and the engine's record of the
/// run, which the next engine reads back to adopt it.
#[derive(Clone, Debug)]
pub struct KeeperScope {
    pub root: PathBuf,
    pub keeper_exe: PathBuf,
    /// Arguments before `keeper` (a test binary's own; the app passes none).
    pub keeper_args: Vec<std::ffi::OsString>,
    pub meta: serde_json::Value,
    pub orphan_after: Duration,
    /// Set when the engine is going away on purpose. Drivers then leave the
    /// agent running instead of interrupting it.
    pub detach: Detach,
}

/// A flag an engine raises before it exits so its keepers' agents keep running.
#[derive(Clone, Debug, Default)]
pub struct Detach(Arc<AtomicBool>);

impl Detach {
    pub fn raise(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn raised(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

tokio::task_local! {
    static SCOPE: KeeperScope;
}

/// Run `future` with `scope` visible to the drivers it starts.
pub async fn scope<F: std::future::Future>(scope: KeeperScope, future: F) -> F::Output {
    SCOPE.scope(scope, future).await
}

/// The engine's scope for the run being started, if it lent one.
pub fn current() -> Option<KeeperScope> {
    SCOPE.try_with(Clone::clone).ok()
}
