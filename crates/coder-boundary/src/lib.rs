//! A filesystem write boundary for delegated commands, and the snapshots
//! that check what a run actually left behind.
//!
//! Two halves, independent of Coder and CoderBench so a host anywhere can
//! run a program it does not trust with the whole disk:
//!
//! - [`boundary`] wraps one command in an enforced write policy. On macOS
//!   that is a `sandbox-exec` profile that denies `file-write*` everywhere
//!   and then permits exactly the checkout, scratch, and adapter-state
//!   paths the caller named; on Linux it is a `bwrap` mount namespace
//!   with a read-only root and those same paths bound writable; on
//!   Windows it is an AppContainer of the boundary's own, started by the
//!   `coder-boundary` launcher, whose only file access is the entries the
//!   boundary adds for it to those paths.
//!   Everywhere else the boundary refuses to exist, because a boundary
//!   that silently stopped bounding is worse than none.
//! - [`snapshot`] observes a directory tree before and after a run and
//!   answers what changed — creation, removal, renames, and content
//!   changes to files that were already dirty — without asking the
//!   version-control tool the observation is meant to check.
//!
//! - [`toolchains`] finds this computer's developer toolchains, from the
//!   person's `PATH` and the known toolchain roots, as a read allow list
//!   and a search path for a read-confined boundary whose command should
//!   be able to use what is installed here.
//!
//! - [`privacy`] names the places macOS guards with a privacy prompt and
//!   keeps every command out of them, so nothing OpenAgents runs makes
//!   macOS ask the person for their music, photos, or documents.
//!
//! This is filesystem write enforcement, plus network denial when the
//! caller asks for it with [`Spec::offline`], and read confinement when
//! the caller asks for it with [`Spec::confining_reads`] or
//! [`Spec::readable`]. It bounds neither time nor output nor memory;
//! those belong to the supervisor the caller already runs,
//! `crates/supervise`.

pub mod boundary;
pub mod cmdline;
pub mod privacy;
pub mod snapshot;
pub mod toolchains;
#[cfg(windows)]
pub mod windows;

pub use boundary::{
    BACKEND, BUBBLEWRAP, BUBBLEWRAP_NIXOS, BUBBLEWRAP_PATHS, Boundary, Error, Held, SANDBOX_EXEC,
    SYSTEM_READS, Spec, backend_path,
};
pub use cmdline::plain_path;
pub use snapshot::{Change, Fault, Limits, Snapshot, Verdict, compare};
pub use toolchains::Toolchains;
