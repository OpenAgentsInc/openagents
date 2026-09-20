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
//!   with a read-only root and those same paths bound writable.
//!   Everywhere else the boundary refuses to exist, because a boundary
//!   that silently stopped bounding is worse than none.
//! - [`snapshot`] observes a directory tree before and after a run and
//!   answers what changed — creation, removal, renames, and content
//!   changes to files that were already dirty — without asking the
//!   version-control tool the observation is meant to check.
//!
//! This is filesystem write enforcement only. It confines neither reads
//! nor network, and it bounds neither time nor output nor memory; those
//! belong to the supervisor the caller already runs, `crates/supervise`.

pub mod boundary;
pub mod snapshot;

pub use boundary::{BACKEND, BUBBLEWRAP, Boundary, Error, Held, SANDBOX_EXEC, Spec};
pub use snapshot::{Change, Fault, Limits, Snapshot, Verdict, compare};
