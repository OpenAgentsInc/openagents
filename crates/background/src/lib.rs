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

#[cfg(unix)]
pub mod builtins;
#[cfg(unix)]
pub mod compile;
#[cfg(unix)]
pub mod engine;
#[cfg(unix)]
pub mod escalate;
#[cfg(unix)]
pub mod git;
#[cfg(unix)]
pub mod inuse;
#[cfg(unix)]
pub mod judged;
#[cfg(unix)]
pub mod kache;
#[cfg(unix)]
pub mod paths;
#[cfg(unix)]
pub mod plan;
#[cfg(unix)]
pub mod plugins;
pub mod pool;
pub mod records;
pub mod task_paths;

#[cfg(not(unix))]
#[path = "git_unavailable.rs"]
pub mod git;
#[cfg(all(unix, test))]
#[path = "git_unavailable.rs"]
mod git_unavailable_tests;

#[cfg(not(unix))]
pub mod services {
    pub use crate::records::{Claim, CoderRun, Failure, Usage};
}

#[cfg(not(unix))]
pub mod paths {
    pub use crate::task_paths::{SLOTS, STORE_VAR, task_store, task_targets, task_worktrees};
}
#[cfg(unix)]
pub mod presence;
#[cfg(unix)]
pub mod rule;
#[cfg(unix)]
pub mod run;
#[cfg(unix)]
pub mod runner;
#[cfg(unix)]
pub mod services;
#[cfg(unix)]
pub mod store;
#[cfg(unix)]
pub mod view;
#[cfg(unix)]
pub mod volume;

#[cfg(unix)]
pub use paths::Layout;
#[cfg(unix)]
pub use plan::{Env, Facts, Plan};
pub use records::TaskFact;
#[cfg(unix)]
pub use rule::{Class, Rule};
#[cfg(unix)]
pub use run::{Cause, Record, Report};
pub use task_paths::{SLOTS, task_store, task_targets, task_worktrees};

#[cfg(all(test, unix))]
mod judged_suite;
#[cfg(all(test, unix))]
mod phase2_tests;
#[cfg(all(test, unix))]
mod phase3_tests;
#[cfg(all(test, unix))]
mod tests;
