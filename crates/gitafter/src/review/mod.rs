//! PR review automation for GitAfter

pub mod auto_checks;
pub mod checklist;

pub use auto_checks::{AutoCheckRunner, CheckResult, CheckStatus};
pub use checklist::{ChecklistGenerator, ChecklistItem, ReviewTemplate};
