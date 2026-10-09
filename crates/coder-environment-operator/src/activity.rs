//! What a person sees of an environment's setup: the conversation and the
//! work behind it, as an append-only log the web view renders and streams.
//!
//! Each environment keeps `activity.jsonl` under its studio directory. One
//! line is one [`Record`]; the log's revision is its line count, so a
//! viewer that has seen `n` records asks for the rest. Text here is what a
//! user reads (agent prose, command output excerpts already redacted by the
//! setup owner's evidence recorder); the complete evidence stays with the
//! owners.

use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The longest output excerpt one record keeps.
pub const MAX_EXCERPT: usize = 8 * 1024;

/// One check the fresh machine runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckLine {
    pub name: String,
    pub command: String,
}

/// One thing that happened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    /// The person's words: the objective, or steering.
    User { text: String },
    /// The setup agent's words.
    Agent { text: String },
    /// The setup computer is being started.
    Starting,
    /// The pinned commit was put on the setup computer.
    Source {
        ok: bool,
        revision: String,
        output: String,
    },
    /// A command the agent ran to learn about the repository.
    Explored {
        command: String,
        exit: Option<i64>,
        output: String,
    },
    /// The agent wrote a new install recipe revision.
    Recipe {
        revision: u64,
        script: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        previous: Option<String>,
    },
    /// The install recipe ran on the setup computer.
    Install {
        revision: u64,
        exit: Option<i64>,
        output: String,
    },
    /// The checks a fresh machine must pass.
    Checks { checks: Vec<CheckLine> },
    /// The agent stopped to ask the person something.
    Question { text: String },
    /// The clean build started or moved on.
    Build { stage: Stage, detail: String },
    /// The fresh-machine check started or moved on.
    Verify { stage: Stage, detail: String },
    /// A candidate is ready to review and save.
    Ready { summary: String },
    /// The person saved it.
    Saved { number: u64 },
    /// Something stopped the setup; the person can retry.
    Failed { reason: String },
    /// The person asked to try again.
    Retried,
}

/// How far one build or verification step got.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Started,
    Passed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub at_ms: u64,
    #[serde(flatten)]
    pub entry: Entry,
}

/// The activity logs under one directory, one per environment.
#[derive(Debug)]
pub struct Logs {
    root: PathBuf,
    write: Mutex<()>,
    changed: tokio::sync::watch::Sender<u64>,
}

impl Logs {
    pub fn under(root: impl Into<PathBuf>) -> Self {
        let (changed, _) = tokio::sync::watch::channel(0);
        Self {
            root: root.into(),
            write: Mutex::new(()),
            changed,
        }
    }

    fn path(&self, environment: &str) -> PathBuf {
        self.root.join(environment).join("activity.jsonl")
    }

    /// Append one record and wake every watcher.
    pub fn push(&self, environment: &str, entry: Entry, now_ms: u64) -> Result<(), String> {
        let _guard = self.write.lock().map_err(|_| "The activity log is busy.")?;
        let path = self.path(environment);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("Cannot create the activity log: {e}"))?;
        }
        let mut line = serde_json::to_vec(&Record {
            at_ms: now_ms,
            entry: bounded(entry),
        })
        .map_err(|_| "Cannot encode an activity record.")?;
        line.push(b'\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("Cannot open the activity log: {e}"))?;
        file.write_all(&line)
            .and_then(|_| file.sync_data())
            .map_err(|e| format!("Cannot write the activity log: {e}"))?;
        self.changed.send_modify(|n| *n += 1);
        Ok(())
    }

    /// Every record of `environment`, oldest first.
    pub fn read(&self, environment: &str) -> Vec<Record> {
        read_path(&self.path(environment))
    }

    /// A receiver that changes whenever any log grows.
    pub fn watch(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changed.subscribe()
    }
}

fn read_path(path: &Path) -> Vec<Record> {
    let Ok(file) = File::open(path) else {
        return vec![];
    };
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect()
}

/// A system message made fit to show a person: sentences that narrate
/// internals ([`oa_copy::violations`]) are left out, and the whole message
/// goes to the server log so nothing is lost. `fallback` stands in when no
/// sentence is left.
pub fn plain(text: &str, fallback: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return fallback.to_owned();
    }
    let kept: Vec<&str> = text
        .split_inclusive(". ")
        .map(str::trim)
        .filter(|s| !s.is_empty() && oa_copy::violations(s, &[]).is_empty())
        .collect();
    if kept.len() != text.split_inclusive(". ").count() {
        eprintln!("environments: {text}");
    }
    if kept.is_empty() {
        fallback.to_owned()
    } else {
        tail_of(&kept.join(" "), 600)
    }
}

/// Keep the end of `text`, at most [`MAX_EXCERPT`] bytes, on a character
/// boundary.
pub fn tail(text: &str) -> String {
    tail_of(text, MAX_EXCERPT)
}

pub fn tail_of(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    format!("…{}", &text[start..])
}

fn bounded(entry: Entry) -> Entry {
    match entry {
        Entry::Explored {
            command,
            exit,
            output,
        } => Entry::Explored {
            command,
            exit,
            output: tail(&output),
        },
        Entry::Install {
            revision,
            exit,
            output,
        } => Entry::Install {
            revision,
            exit,
            output: tail(&output),
        },
        Entry::Source {
            ok,
            revision,
            output,
        } => Entry::Source {
            ok,
            revision,
            output: tail(&output),
        },
        // System messages a person reads; the model keeps the whole text.
        Entry::Failed { reason } => Entry::Failed {
            reason: plain(&reason, "The setup stopped."),
        },
        Entry::Build { stage, detail } => Entry::Build {
            stage,
            detail: plain(&detail, "The build stopped."),
        },
        Entry::Verify { stage, detail } => Entry::Verify {
            stage,
            detail: plain(&detail, "The check stopped."),
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_append_in_order_and_survive_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let logs = Logs::under(dir.path());
        let mut watch = logs.watch();
        logs.push(
            "env-1",
            Entry::User {
                text: "Set it up".into(),
            },
            1,
        )
        .unwrap();
        logs.push(
            "env-1",
            Entry::Install {
                revision: 2,
                exit: Some(1),
                output: "x".repeat(MAX_EXCERPT * 2),
            },
            2,
        )
        .unwrap();
        assert!(watch.has_changed().unwrap());
        watch.borrow_and_update();
        let again = Logs::under(dir.path());
        let records = again.read("env-1");
        assert_eq!(records.len(), 2);
        assert_eq!(
            records[0].entry,
            Entry::User {
                text: "Set it up".into()
            }
        );
        match &records[1].entry {
            Entry::Install { output, .. } => assert!(output.len() <= MAX_EXCERPT + 4),
            other => panic!("{other:?}"),
        }
        assert!(again.read("env-2").is_empty());
    }

    #[test]
    fn plain_messages_leave_out_machine_talk() {
        assert_eq!(
            plain(
                "The clean build failed. The admitted digest drifted. Try again.",
                "x"
            ),
            "The clean build failed. Try again."
        );
        assert_eq!(plain("The cursor moved.", "It stopped."), "It stopped.");
        assert_eq!(plain("", "It stopped."), "It stopped.");
    }

    #[test]
    fn tails_keep_whole_characters() {
        let text = "é".repeat(10);
        let t = tail_of(&text, 5);
        assert!(t.starts_with('…'));
        assert!(t.chars().skip(1).all(|c| c == 'é'));
    }
}
