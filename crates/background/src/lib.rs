//! Background processes: durable rules the host runs without a
//! conversation (docs/background/2026-10-02-background-processes.md).
//!
//! Phase 1 is the disk cleanup monitor, the built-in rule `disk`: when a
//! watched volume's free space falls below its start level, it deletes
//! disposable build caches and finished worktrees, in class order and least
//! recently used first, until the stop level, and never anything in use,
//! unsaved, denied, or reached through a link. No model is called.
//!
//! The crate has no host dependency: the host gives it the task store
//! ([`Facts`]) and starts [`runner::start`]; `openagents background` calls
//! [`run::run`] directly.

#![cfg(unix)]

pub mod git;
pub mod inuse;
pub mod paths;
pub mod plan;
pub mod plugins;
pub mod rule;
pub mod run;
pub mod runner;
pub mod store;
pub mod view;
pub mod volume;

pub use paths::{Layout, SLOTS};
pub use plan::{Env, Facts, Plan, TaskFact};
pub use rule::{Class, Rule};
pub use run::{Cause, Record, Report};

#[cfg(test)]
mod tests;
