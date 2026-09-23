//! Synthetic candidates with known answers, one known-good and at least
//! one known-bad per failure family. A scenario earns its place in the
//! catalog only if it fails the bad candidate it targets and passes the
//! good one.

use std::collections::BTreeMap;

use super::{Budget, Candidate, Input, Observed, TaskText};
use crate::minitask::{LOG_FILES, find, sources};

/// One synthetic case: a candidate and the scenarios it must fail and
/// pass.
pub struct Case {
    pub name: &'static str,
    pub family: &'static str,
    /// Whether this is the family's known-good candidate.
    pub good: bool,
    pub input: Input,
    pub fails: Vec<&'static str>,
    pub passes: Vec<&'static str>,
}

fn candidate(label: &str, files: &[(&str, &str)], provided: &[(&str, &str)]) -> Candidate {
    Candidate {
        label: label.to_string(),
        origin: "synthetic".to_string(),
        files: files
            .iter()
            .map(|(p, t)| ((*p).to_string(), (*t).to_string()))
            .collect(),
        programs: Vec::new(),
        provided: provided
            .iter()
            .map(|(p, t)| ((*p).to_string(), (*t).to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn input(task: &str, candidate: Candidate, observed: Observed) -> Input {
    let task = find(task).expect("a mini-task");
    Input {
        task: TaskText {
            title: format!("mini-task {}", task.id),
            instruction: task.instruction.to_string(),
        },
        requirements: None,
        candidate,
        observed,
        budget: Budget::default(),
    }
}

fn log_samples() -> Observed {
    Observed {
        samples: LOG_FILES
            .iter()
            .flat_map(|(_, _, lines)| lines.iter().map(|l| (*l).to_string()))
            .collect(),
        source: "the log-severity mini-task's files".to_string(),
    }
}

/// Every synthetic case.
#[must_use]
pub fn cases() -> Vec<Case> {
    let data = ["data.message-severity", "data.date-boundaries"];
    let terminal = ["interactive.program", "interactive.interrupt"];
    let cancel = [
        "cancel.signal.below",
        "cancel.signal.at",
        "cancel.signal.above",
        "cancel.internal.above",
    ];
    let provided = [("base_terminal.py", sources::BASE_TERMINAL)];
    vec![
        Case {
            name: "log-field-parser",
            family: "field meaning in data",
            good: true,
            input: input(
                "log-severity",
                candidate(
                    "field parser",
                    &[("summarize.py", sources::SUMMARIZE_FIELD)],
                    &[],
                ),
                log_samples(),
            ),
            fails: vec![],
            passes: data.to_vec(),
        },
        Case {
            name: "log-whole-line-search",
            family: "field meaning in data",
            good: false,
            input: input(
                "log-severity",
                candidate(
                    "whole-line search",
                    &[("summarize.py", sources::SUMMARIZE_WHOLE_LINE)],
                    &[],
                ),
                log_samples(),
            ),
            fails: vec!["data.message-severity"],
            passes: vec!["data.date-boundaries"],
        },
        Case {
            name: "log-off-by-one-window",
            family: "field meaning in data",
            good: false,
            input: input(
                "log-severity",
                candidate(
                    "off-by-one window",
                    &[("summarize.py", sources::SUMMARIZE_OFF_BY_ONE)],
                    &[],
                ),
                log_samples(),
            ),
            fails: vec!["data.date-boundaries"],
            passes: vec!["data.message-severity"],
        },
        Case {
            name: "terminal-pty",
            family: "interactive behavior",
            good: true,
            input: input(
                "interactive-terminal",
                candidate(
                    "pseudo-terminal",
                    &[("headless_terminal.py", sources::TERMINAL_PTY)],
                    &provided,
                ),
                Observed::default(),
            ),
            fails: vec![],
            passes: terminal.to_vec(),
        },
        Case {
            name: "terminal-builtins-only",
            family: "interactive behavior",
            good: false,
            input: input(
                "interactive-terminal",
                candidate(
                    "builtins only",
                    &[("headless_terminal.py", sources::TERMINAL_BUILTINS)],
                    &provided,
                ),
                Observed::default(),
            ),
            fails: terminal.to_vec(),
            passes: vec![],
        },
        Case {
            name: "cancel-awaited-cleanup",
            family: "cancellation lifecycle",
            good: true,
            input: input(
                "cancel-cleanup",
                candidate("awaited cleanup", &[("run.py", sources::RUN_AWAITED)], &[]),
                Observed::default(),
            ),
            fails: vec![],
            passes: cancel.to_vec(),
        },
        Case {
            name: "cancel-early-return",
            family: "cancellation lifecycle",
            good: false,
            input: input(
                "cancel-cleanup",
                candidate(
                    "early return",
                    &[("run.py", sources::RUN_EARLY_RETURN)],
                    &[],
                ),
                Observed::default(),
            ),
            fails: cancel.to_vec(),
            passes: vec![],
        },
    ]
}

/// The case named `name`.
#[must_use]
pub fn case(name: &str) -> Option<Case> {
    cases().into_iter().find(|c| c.name == name)
}
