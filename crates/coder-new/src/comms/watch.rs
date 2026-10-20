//! A solver run seen from the channel: which hand is working (its stage)
//! and what it means to change (its intent).
//!
//! A solver's hands can't run `coder comms` themselves, so the channel
//! posts for them from the run's own transcript: after the Briefer, the
//! files under its "Files to change" heading; on each hand-back, the files
//! the Coder actually edited. Both come from the run's events, so a
//! replayed recording posts the same intents a live run did.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::issue_run::{self, Event};
use crate::live::Entry;

/// What changed in the run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// A new stage, in plain words.
    Stage(&'static str),
    /// The files it means to change now, and why it says so.
    Intent {
        files: Vec<String>,
        note: &'static str,
    },
}

/// Follows one solver run's events.
#[derive(Debug, Default)]
pub struct SolverWatch {
    stage: &'static str,
    /// The newest Briefer analysis text.
    analysis: String,
    /// Files the Coder edited, this run so far.
    edited: BTreeSet<String>,
    /// The files last posted as the intent.
    posted: Vec<String>,
}

impl SolverWatch {
    /// The current stage, in plain words.
    #[must_use]
    pub fn stage(&self) -> &'static str {
        if self.stage.is_empty() {
            "Starting"
        } else {
            self.stage
        }
    }

    /// Reads one event and returns what it changed.
    pub fn see(&mut self, event: &Event) -> Vec<Change> {
        let Event::Set { entry, .. } = event else {
            return Vec::new();
        };
        let mut changes = Vec::new();
        match entry {
            Entry::Tool { name, input, .. } if name == issue_run::DECISION => {
                let stage = match (input["kind"].as_str(), input["title"].as_str()) {
                    (Some("agent"), Some("Intaker")) => "Reading the issue",
                    (Some("agent"), Some("Briefer")) => "Planning",
                    (Some("agent"), Some("Coder")) => "Coding",
                    (Some("agent"), Some("Tester")) => "Testing",
                    (Some("stage" | "model" | "ranked" | "briefing"), _) => "Preparing",
                    _ => return changes,
                };
                self.enter(stage, &mut changes);
            }
            Entry::Tool {
                name,
                input,
                running: false,
                ..
            } if name == issue_run::SUMMARY => {
                // The last edits, if the run ended before a later stage.
                self.post_edits("These are the files it changed.", &mut changes);
                let stage =
                    if input.pointer("/outcome/status").and_then(Value::as_str) == Some("passed") {
                        "Done"
                    } else {
                        "Ended"
                    };
                self.stage = stage;
                changes.push(Change::Stage(stage));
            }
            Entry::Tool {
                name,
                input,
                running,
                ..
            } if matches!(self.stage, "Coding" | "Checking")
                && !*running
                && matches!(name.as_str(), "Edit" | "Write") =>
            {
                if let Some(path) = input["path"].as_str().and_then(super::overlap::normal) {
                    self.edited.insert(path);
                }
            }
            Entry::Tool { name, input, .. }
                if name == "Run" && input.get("check").is_some() && self.stage != "Testing" =>
            {
                self.enter("Checking", &mut changes);
            }
            Entry::Assistant { text, .. } if self.stage == "Planning" => {
                self.analysis.clone_from(text);
            }
            _ => {}
        }
        changes
    }

    fn enter(&mut self, stage: &'static str, changes: &mut Vec<Change>) {
        if stage == self.stage {
            return;
        }
        let leaving = self.stage;
        self.stage = stage;
        if leaving == "Planning" {
            let files = files_to_change(&self.analysis);
            if !files.is_empty() && files != self.posted {
                self.posted.clone_from(&files);
                changes.push(Change::Intent {
                    files,
                    note: "From its plan.",
                });
            }
        }
        if leaving == "Coding" || leaving == "Checking" {
            self.post_edits("These are the files it changed so far.", changes);
        }
        changes.push(Change::Stage(stage));
    }

    fn post_edits(&mut self, note: &'static str, changes: &mut Vec<Change>) {
        let files: Vec<String> = self.edited.iter().cloned().collect();
        if !files.is_empty() && files != self.posted {
            self.posted.clone_from(&files);
            changes.push(Change::Intent { files, note });
        }
    }
}

/// The paths a Briefer's analysis lists under its "Files to change"
/// heading: each bullet's first backticked path.
#[must_use]
pub fn files_to_change(analysis: &str) -> Vec<String> {
    let mut files = Vec::new();
    let mut inside = false;
    for line in analysis.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            inside = trimmed.to_ascii_lowercase().contains("files to change");
            continue;
        }
        if !inside || !(trimmed.starts_with('-') || trimmed.starts_with('*')) {
            continue;
        }
        let Some(start) = trimmed.find('`') else {
            continue;
        };
        let rest = &trimmed[start + 1..];
        let Some(end) = rest.find('`') else { continue };
        let path = &rest[..end];
        if !path.is_empty()
            && !path.contains(char::is_whitespace)
            && (path.contains('/') || path.contains('.'))
        {
            if let Some(path) = super::overlap::normal(path) {
                if !files.contains(&path) {
                    files.push(path);
                }
            }
        }
    }
    files
}
