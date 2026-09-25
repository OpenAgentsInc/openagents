//! Failure localization (issue #9658): three code components whose output
//! goes to the next Luna session as evidence.
//!
//! Six of eleven mapped Fable 5.1 winning runs localize a failure before
//! they fix it (`docs/terminal-bench/2026-09-25-fable-pattern-map.md`):
//! they print the source an error names, trace the first mismatched value
//! through intermediate stages, and time phases to find where the wait
//! goes. Each is a code operation here, and none asks a model:
//!
//! - **`evidence.error_context`** ([`context`]): file and line references
//!   in compiler, test-runner, and traceback output, read by one table of
//!   rules ([`parse::RULES`]), resolved to workspace files, and printed
//!   with a bounded window, deduplicated, most recent first.
//! - **`evidence.mismatch_trace`** ([`mismatch`]): the first failing case
//!   of an acceptance check, its input, observed against expected, and a
//!   diff; with the stages compared in order when the check exposes them.
//! - **`evidence.phase_timing`** ([`timing`]): a command that timed out or
//!   used most of its bound, run once more under a profiler on a scratch
//!   copy of the workspace, with where the time went.
//!
//! The lean loop's switch is `executor.microluna.lean.localize`, absent
//! from every manifest. [`offline`] is the measurement on retained
//! sessions, `docs/terminal-bench/2026-09-25-failure-localization.md`.

pub mod context;
pub mod mismatch;
pub mod offline;
pub mod parse;
#[cfg(test)]
mod tests;
pub mod timing;
pub mod trace;

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub use context::Failure;

/// The component IDs.
pub const ERROR_CONTEXT: &str = "evidence.error_context";
pub const MISMATCH_TRACE: &str = "evidence.mismatch_trace";
pub const PHASE_TIMING: &str = "evidence.phase_timing";

/// `executor.microluna.lean.localize`: which of the three components run,
/// and their bounds. Absent, as in every manifest before it, none runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Localize {
    /// `evidence.error_context` after every work session.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub error_context: bool,
    /// Also tell a running session the regions a failing command's output
    /// names, after the turn that ran it, through the host's watch.
    /// Needs `error_context`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub in_session: bool,
    /// `evidence.mismatch_trace` after every work session.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mismatch_trace: bool,
    /// `evidence.phase_timing` after a work session in which a command
    /// timed out or used at least half its bound.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub phase_timing: bool,
    /// Lines shown on each side of a named line.
    #[serde(default = "window")]
    pub window: usize,
    /// The profiled run's wall-time bound, in seconds.
    #[serde(default = "timing_sec")]
    pub timing_sec: u64,
}

fn window() -> usize {
    context::WINDOW
}

fn timing_sec() -> u64 {
    120
}

/// Times one session's in-session notes may interrupt it.
pub const IN_SESSION_TELLS: usize = 3;

impl Localize {
    /// What's wrong with the switch's settings.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !self.error_context && !self.mismatch_trace && !self.phase_timing {
            problems.push(
                "executor.microluna.lean.localize turns on none of error_context, \
                 mismatch_trace, and phase_timing"
                    .to_string(),
            );
        }
        if self.in_session && !self.error_context {
            problems.push(
                "executor.microluna.lean.localize.in_session requires error_context".to_string(),
            );
        }
        if !(1..=40).contains(&self.window) {
            problems
                .push("executor.microluna.lean.localize.window must be from 1 to 40".to_string());
        }
        if !(5..=600).contains(&self.timing_sec) {
            problems.push(
                "executor.microluna.lean.localize.timing_sec must be from 5 to 600".to_string(),
            );
        }
        problems
    }

    fn bounds(&self) -> context::Bounds {
        context::Bounds {
            window: self.window,
            ..context::Bounds::default()
        }
    }
}

/// The workspace's files, relative to `workdir`.
#[must_use]
pub fn workspace_files(workdir: &Path) -> BTreeSet<String> {
    crate::micro::parallel::workspace_files(workdir)
        .into_iter()
        .collect()
}

/// `evidence.error_context` over `failures`, oldest first, in the
/// workspace at `workdir`: the evidence text and its record, or `None`
/// with the record when no reference resolved.
#[must_use]
pub fn error_context(
    policy: &Localize,
    workdir: &Path,
    failures: &[Failure],
    skip: &BTreeSet<(String, u64)>,
) -> (Option<String>, Value) {
    let files = workspace_files(workdir);
    let root = workdir.display().to_string();
    let located: Vec<_> = context::located(failures, &files, &root)
        .into_iter()
        .filter(|(file, location, _)| !skip.contains(&(file.clone(), location.line)))
        .collect();
    let read = |file: &str| std::fs::read_to_string(workdir.join(file)).ok();
    let regions = context::regions(&located, policy.bounds(), read);
    let text = context::render(&regions, policy.bounds(), read);
    let parsed: usize = failures.iter().map(|f| parse::parse(&f.output).len()).sum();
    let record = json!({
        "component": ERROR_CONTEXT,
        "failures": failures.len(),
        "references": parsed,
        "resolved": located.len(),
        "regions": regions,
        "chars": text.as_ref().map_or(0, |t| t.chars().count()),
    });
    (text, record)
}

/// `evidence.mismatch_trace` over `checks`, `(check, output)` pairs most
/// recent first: the first that shows a failing case.
#[must_use]
pub fn mismatch_trace(checks: &[(String, String)]) -> (Option<String>, Value) {
    for (check, output) in checks {
        if let Some(case) = mismatch::first_case(output) {
            let text = mismatch::render(&case, check);
            let record = json!({
                "component": MISMATCH_TRACE,
                "check": check,
                "case": case,
                "first_differing_stage": mismatch::first_differing_stage(&case).map(|(i, s)| json!({"index": i, "name": s.name})),
            });
            return (Some(text), record);
        }
    }
    (
        None,
        json!({"component": MISMATCH_TRACE, "checks": checks.len(), "case": null}),
    )
}

/// A command worth timing: it timed out, or used at least half its bound.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slow {
    pub command: String,
    pub milliseconds: u64,
    pub bound_ms: u64,
    pub timed_out: bool,
}

impl Slow {
    /// Whether it triggers a profile.
    #[must_use]
    pub fn triggers(&self) -> bool {
        self.timed_out
            || (self.bound_ms > 0 && self.milliseconds.saturating_mul(2) >= self.bound_ms)
    }
}

/// The most recent triggering command of `slow`, oldest first, that isn't
/// a read.
#[must_use]
pub fn timing_target(slow: &[Slow]) -> Option<&Slow> {
    slow.iter().rev().find(|s| {
        s.triggers()
            && !microluna::tools::reads_only(
                "run_command",
                &json!({"command": s.command}).to_string(),
            )
    })
}

/// `evidence.phase_timing` for `target`: the profiled run on a scratch
/// copy of the workspace, bounded by `policy.timing_sec`.
pub async fn phase_timing(
    policy: &Localize,
    target: &Slow,
    place: &crate::checks::contract::executed::Place,
) -> (Option<String>, Value) {
    let bound = policy.timing_sec;
    let (profiler, command) = timing::plan(&target.command, bound.saturating_sub(3).max(1));
    let planned = crate::checks::contract::executed::Planned {
        kind: "timing".to_string(),
        command,
        requirements: Vec::new(),
    };
    let place = crate::checks::contract::executed::Place {
        workdir: place.workdir.clone(),
        contained: place.contained,
        wall: std::time::Duration::from_secs(bound),
        budget: std::time::Duration::from_secs(bound + 5),
    };
    match crate::checks::contract::executed::in_copy(&place.workdir, &place, &[planned]).await {
        Ok(runs) if !runs.is_empty() => {
            let ran = &runs[0].ran;
            let profiled = timing::Profiled {
                stdout: ran.stdout.clone(),
                stderr: ran.stderr.clone(),
                exit: ran.exit,
                timed_out: ran.timed_out,
                milliseconds: ran.milliseconds,
            };
            let text = timing::report(profiler, &target.command, &profiled, bound);
            let record = json!({
                "component": PHASE_TIMING,
                "target": target,
                "profiler": profiler,
                "exit": ran.exit,
                "timed_out": ran.timed_out,
                "milliseconds": ran.milliseconds,
                "failed": ran.failed,
            });
            (Some(text), record)
        }
        Ok(_) => (
            None,
            json!({"component": PHASE_TIMING, "target": target, "error": "nothing ran"}),
        ),
        Err(error) => (
            None,
            json!({"component": PHASE_TIMING, "target": target, "error": error}),
        ),
    }
}

/// The note one session's evidence becomes in the next brief.
#[must_use]
pub fn note(session: u32, component: &str, text: &str) -> String {
    format!("After session {session}, the host's {component}:\n{text}")
}
