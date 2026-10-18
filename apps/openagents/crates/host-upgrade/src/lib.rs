//! Upgrade the desktop app on this computer from `main` with no owner steps
//! (OpenAgents #12056).
//!
//! The owner's everyday app is the development bundle built from a clean
//! worktree of `main` (`apps/openagents/OPENAGENTS.md`, "The owner's everyday
//! app"). An upgrade runs in two halves:
//!
//! 1. **The upgrader** ([`upgrade::upgrade`], the `openagents host-upgrade`
//!    command, which the `upgrade_host` app tool starts) fetches `main`,
//!    builds a signed bundle beside the running one in the shared target
//!    folder, and checks it: the signature, the data folder written into it,
//!    and a headless engine from it answering on a scratch port. Any failure
//!    puts the checkout back and leaves the running app alone, and the log
//!    says why. Otherwise it hands the bundle to the app ([`record::Pending`]).
//! 2. **The app** ([`app::Watcher`]) waits for a quiet moment ([`quiet`]): the
//!    owner isn't typing and no voice call is live. It shows a notice, swaps
//!    the bundle in, and relaunches onto the chat that was open. Agents under
//!    keepers keep running through the restart, and the new engine picks
//!    them up (#11930, #11946, #11951). The reopened app says so and logs the
//!    upgrade.
//!
//! Every upgrade is a line in `<data>/host-upgrade/upgrades.jsonl`: from and
//! to commit, the Chicago time, and the result ([`record::LogEntry`]).

pub mod app;
pub mod grant;
pub mod layout;
pub mod quiet;
pub mod record;
pub mod swap;
#[cfg(unix)]
pub mod system;
pub mod upgrade;

pub use layout::Layout;
pub use record::{LogEntry, Outcome, Pending, Phase};
pub use upgrade::{Host, Request, upgrade};

/// The scope an agent needs to start an upgrade (#12033's machine scopes).
pub const SCOPE: &str = "host.upgrade";

/// Milliseconds since the Unix epoch.
#[must_use]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// A commit as people read it: its first ten characters.
#[must_use]
pub fn short(commit: &str) -> &str {
    commit.get(..10).unwrap_or(commit)
}
