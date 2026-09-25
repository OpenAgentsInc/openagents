//! Finding a checker or a reference program the task provides.
//!
//! Code reads only what the instruction names: its code spans, the files
//! they point to in the untouched workspace, and the commands in the
//! documents it names ([`extract::gather`]). A **checker** is a command
//! the task states that runs a file whose name says it checks, or a
//! runnable file with such a name; it becomes an oracle with one case,
//! passed on exit 0. A **reference program** is a named file whose name
//! says it's a reference; it can't check anything on its own, so the
//! oracle writer is told where it is.
//!
//! A file the task asks to write, fix, or implement is the code under
//! test, never a checker.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::super::contract::extract::{self, Pristine};
use super::super::contract::host::Stat;

/// Name parts that say a file checks something.
pub const CHECKER_WORDS: &[&str] = &[
    "check",
    "checker",
    "checks",
    "verify",
    "verifier",
    "validate",
    "validator",
    "grade",
    "grader",
    "evaluate",
    "evaluator",
    "judge",
    "harness",
    "test",
    "tests",
    "selftest",
];

/// Name parts that say a file is a reference to compare with.
pub const REFERENCE_WORDS: &[&str] = &["reference", "ref", "oracle", "golden"];

/// Words in a sentence that make the paths it names the code under test.
const WORK_WORDS: &[&str] = &[
    "fix",
    "repair",
    "modify",
    "edit",
    "implement",
    "complete",
    "update",
    "rewrite",
    "debug",
    "refactor",
    "change",
    "write",
    "create",
];

/// Extensions of files that run as programs.
const RUNNABLE: &[(&str, &str)] = &[
    ("py", "python3"),
    ("sh", "sh"),
    ("bash", "bash"),
    ("pl", "perl"),
    ("rb", "ruby"),
    ("js", "node"),
    ("mjs", "node"),
];

/// What finding produced.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Found {
    /// The checker's command, run from the working directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The checker's file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Why this checker: the sentence or document line it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Reference programs, by path.
    #[serde(default)]
    pub references: Vec<String>,
    /// Every candidate code looked at and why it was or wasn't taken.
    #[serde(default)]
    pub considered: Vec<String>,
}

fn name_parts(path: &str) -> BTreeSet<String> {
    let name = path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path);
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    stem.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn has_any(parts: &BTreeSet<String>, words: &[&str]) -> bool {
    words.iter().any(|w| parts.contains(*w))
}

fn words_of(sentence: &str) -> BTreeSet<String> {
    sentence
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Whether the sentence makes what it names the code under test.
fn names_work(sentence: &str) -> bool {
    let words = words_of(sentence);
    WORK_WORDS.iter().any(|w| words.contains(*w))
}

/// The command that runs `path` from the working directory, by its
/// extension; a file with none runs as itself.
#[must_use]
pub fn runner_for(path: &str) -> Option<String> {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((_, ext)) => RUNNABLE
            .iter()
            .find(|(e, _)| *e == ext.to_lowercase())
            .map(|(_, program)| format!("{program} {path}")),
        None => Some(path.to_string()),
    }
}

/// Finds a checker and reference programs among what `instruction` names,
/// reading the untouched workspace's `pristine` entries.
#[must_use]
pub fn find(instruction: &str, workdir: &str, pristine: &Pristine) -> Found {
    let mut found = Found::default();
    let exists = |path: &str| {
        pristine
            .entries
            .get(path.trim_end_matches('/'))
            .is_some_and(|e| e.stat != Stat::Missing)
    };
    let (units, blocks) = extract::segment(instruction);
    // 1. Commands the instruction and its documents state.
    let mut commands: Vec<(String, String)> = Vec::new();
    for unit in &units {
        for (_, span) in extract::spans(&unit.text) {
            if extract::is_command(&span) {
                commands.push((span, unit.text.clone()));
            }
        }
    }
    for block in &blocks {
        for (command, line) in extract::doc_commands(&format!("```sh\n{}```\n", block.body)) {
            commands.push((command, line));
        }
    }
    for (path, entry) in &pristine.entries {
        if let Some(text) = &entry.text
            && (path.ends_with(".md") || path.to_lowercase().contains("readme"))
        {
            commands.extend(extract::doc_commands(text));
        }
    }
    for (command, from) in &commands {
        let checks = command.split_whitespace().any(|word| {
            let parts = name_parts(word);
            has_any(&parts, CHECKER_WORDS)
                && extract::as_path(word, workdir).is_some_and(|p| exists(&p))
        }) || command.split_whitespace().next() == Some("pytest");
        if !checks {
            continue;
        }
        if names_work(from) && !from.to_lowercase().contains("run") {
            found.considered.push(format!(
                "{command}: the sentence asks to change what it names"
            ));
            continue;
        }
        found
            .considered
            .push(format!("{command}: a stated command that runs a checker"));
        if found.command.is_none() {
            found.command = Some(command.clone());
            found.origin = command
                .split_whitespace()
                .find_map(|w| extract::as_path(w, workdir).filter(|p| exists(p)));
            found.from = Some(super::super::contract::clip(from, 300));
        }
    }
    // 2. Named files, by their names.
    for unit in &units {
        for (_, span) in extract::spans(&unit.text) {
            let Some(path) = extract::as_path(&span, workdir) else {
                continue;
            };
            let Some(entry) = pristine.entries.get(path.trim_end_matches('/')) else {
                continue;
            };
            if !matches!(entry.stat, Stat::File(_)) {
                continue;
            }
            let parts = name_parts(&path);
            if has_any(&parts, REFERENCE_WORDS) {
                if !found.references.contains(&path) {
                    found.references.push(path.clone());
                    found
                        .considered
                        .push(format!("{path}: a named reference program"));
                }
                continue;
            }
            if !has_any(&parts, CHECKER_WORDS) || found.command.is_some() {
                continue;
            }
            if names_work(&unit.text) {
                found
                    .considered
                    .push(format!("{path}: the sentence asks to change it"));
                continue;
            }
            match runner_for(&path) {
                Some(command) => {
                    found
                        .considered
                        .push(format!("{path}: a named runnable checker"));
                    found.command = Some(command);
                    found.origin = Some(path.clone());
                    found.from = Some(super::super::contract::clip(&unit.text, 300));
                }
                None => found
                    .considered
                    .push(format!("{path}: named like a checker, but not a program")),
            }
        }
    }
    found
}
