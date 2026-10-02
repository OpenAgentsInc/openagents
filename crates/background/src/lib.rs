//! Background processes: durable rules the host runs without a
//! conversation (docs/background/2026-10-02-background-processes.md).
//!
//! Phase 1 is the disk cleanup monitor, the built-in rule `disk`: when a
//! watched volume's free space falls below its start level, it deletes
//! disposable build caches and finished worktrees, in class order and least
//! recently used first, until the stop level, and never anything in use,
//! unsaved, denied, or reached through a link. No model is called.
//!
//! Phase 2 makes rules from conversation ([`compile`]: Jev chooses over
//! typed catalogs, code fills bounded fields, and a draft is saved only
//! once confirmed) and runs any rule the built-in actions support
//! ([`engine`]: daily and file triggers, typed conditions including a
//! bounded Jev judgment, notifications, and fast-forwarding a clean
//! checkout), with the same safety checks, log, and undo.
//!
//! Phase 3 judges unknown folders ([`judged`]: Jev proposes, the person
//! confirms, later runs need no model), escalates a rule that falls short
//! to a Coder run that proposes changes ([`escalate`]), adds the other
//! built-in processes ([`builtins`]), and lets plugins run their own
//! action and propose folders. What only the host can do (start a Coder
//! run, release a claim, restart) it does through [`services::Services`].
//!
//! The crate has no host dependency: the host gives it the task store
//! ([`Facts`]) and starts [`runner::start`]; `openagents background` calls
//! [`run::run`] directly.

#![cfg(unix)]

pub mod builtins;
pub mod compile;
pub mod engine;
pub mod escalate;
pub mod git;
pub mod inuse;
pub mod judged;
pub mod paths;
pub mod plan;
pub mod plugins;
pub mod rule;
pub mod run;
pub mod runner;
pub mod services;
pub mod store;
pub mod view;
pub mod volume;

pub use paths::{Layout, SLOTS};
pub use plan::{Env, Facts, Plan, TaskFact};
pub use rule::{Class, Rule};
pub use run::{Cause, Record, Report};

#[cfg(test)]
mod judged_suite;
#[cfg(test)]
mod phase2_tests;
#[cfg(test)]
mod phase3_tests;
#[cfg(test)]
mod tests;
