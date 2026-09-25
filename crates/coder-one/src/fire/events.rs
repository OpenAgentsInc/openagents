//! Reads a trial's logs as events: while the trial runs, from the host's
//! live copy, and after it ends, from the episode bundle.
//!
//! A Coder One episode writes `episode.atif.jsonl` and one log per
//! Microluna session under `artifacts/`. Each line is one ATIF record.
//! [`parse`] turns a line into an [`Event`], and [`Follower`] reads the new
//! complete lines of every log in a directory each time it's polled.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::Value;

/// One thing that happened in a trial.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// Milliseconds since the Unix epoch, as the log recorded it.
    pub at: u64,
    /// The log the event came from: `episode` or a session's name.
    pub log: String,
    pub kind: Kind,
}

/// What an event is.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A log's first record.
    Session {
        id: String,
        model: String,
        directive: String,
    },
    /// A component started: its name, its implementation, and the digest
    /// of its parameters.
    Start {
        id: String,
        parent: Option<String>,
        component: String,
        name: String,
        implementation: String,
        digest: String,
    },
    /// A component ended, with what it said about its output.
    Finish {
        id: String,
        component: String,
        outcome: String,
        summary: Value,
    },
    /// One Jev request: the state, the questions, and the answers.
    Jev {
        name: String,
        state: Value,
        questions: Value,
        answers: Value,
        milliseconds: Option<u64>,
        /// Jev's input tokens, which it bills, when the log reports them.
        input_tokens: Option<u64>,
    },
    /// One model turn: its reasoning headlines, tokens, time, and cost.
    Think {
        headline: String,
        input: u64,
        output: u64,
        milliseconds: u64,
        usd: f64,
    },
    /// One tool call and its output.
    Tool {
        name: String,
        arguments: Value,
        output: String,
    },
    /// A message: the task, the host's notes, or the model's text.
    Say { source: String, text: String },
    /// A log's last record.
    End { state: String },
}

impl Event {
    /// Whether this is an action the run's model took in the workspace.
    #[must_use]
    pub fn is_action(&self) -> bool {
        matches!(&self.kind, Kind::Tool { name, .. } if name != "delegate")
    }
}

/// Parses one log line. `None` for a line that isn't a record this reads.
#[must_use]
pub fn parse(line: &str, log: &str) -> Option<Event> {
    let record: Value = serde_json::from_str(line).ok()?;
    let event = |at: u64, kind: Kind| {
        Some(Event {
            at,
            log: log.to_string(),
            kind,
        })
    };
    let text = |value: &Value| value.as_str().unwrap_or_default().to_string();
    match record["record"].as_str()? {
        "session" => {
            let session = &record["session"];
            event(
                record["at"].as_u64().unwrap_or_default(),
                Kind::Session {
                    id: text(&session["id"]),
                    model: text(&session["model"]),
                    directive: text(&session["directive"]),
                },
            )
        }
        "end" => event(
            record["at"].as_u64().unwrap_or_default(),
            Kind::End {
                state: text(&record["state"]),
            },
        ),
        "step" => step(&record["step"], log),
        _ => None,
    }
}

fn step(step: &Value, log: &str) -> Option<Event> {
    let at = step["at"].as_u64().unwrap_or_default();
    let text = |value: &Value| value.as_str().unwrap_or_default().to_string();
    let kind = if let Some(invocation) = step["extensions"].get("invocation") {
        match invocation["event"].as_str() {
            Some("start") => Kind::Start {
                id: text(&invocation["id"]),
                parent: invocation["parent"].as_str().map(str::to_string),
                component: text(&invocation["component"]),
                name: text(&invocation["name"]),
                implementation: text(&invocation["implementation"]["name"]),
                digest: text(&invocation["implementation"]["digest"]),
            },
            _ => Kind::Finish {
                id: text(&invocation["id"]),
                component: text(&invocation["component"]),
                outcome: text(&invocation["outcome"]),
                summary: invocation["output"]["summary"].clone(),
            },
        }
    } else if let Some(call) = step.get("call").filter(|call| call.is_object()) {
        let name = text(&call["name"]);
        let output = match &call["output"] {
            Value::String(output) => output.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        };
        if name.starts_with("jev_") {
            Kind::Jev {
                name,
                state: call["arguments"]["state"].clone(),
                questions: call["arguments"]["questions"].clone(),
                answers: serde_json::from_str(&output).unwrap_or(Value::String(output)),
                milliseconds: step["milliseconds"].as_u64(),
                input_tokens: step["extensions"]["jev_usage"]["input_tokens"].as_u64(),
            }
        } else {
            Kind::Tool {
                name,
                arguments: call["arguments"].clone(),
                output,
            }
        }
    } else if step.get("tokens").is_some() || step.get("reasoning").is_some() {
        let usage = &step["extensions"]["microluna.usage.v1"];
        let message = text(&step["message"]);
        if !message.is_empty() && step.get("reasoning").is_none() {
            Kind::Say {
                source: text(&step["source"]),
                text: message,
            }
        } else {
            Kind::Think {
                headline: text(&step["reasoning"]),
                input: step["tokens"][0].as_u64().unwrap_or_default(),
                output: step["tokens"][1].as_u64().unwrap_or_default(),
                milliseconds: step["milliseconds"].as_u64().unwrap_or_default(),
                usd: usage["cost_usd"].as_f64().unwrap_or_default(),
            }
        }
    } else {
        Kind::Say {
            source: text(&step["source"]),
            text: text(&step["message"]),
        }
    };
    Some(Event {
        at,
        log: log.to_string(),
        kind,
    })
}

/// The logs in a trial's log directory: the episode's, then each
/// session's, in name order.
#[must_use]
pub fn logs(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    let episode = dir.join("episode.atif.jsonl");
    if episode.is_file() {
        found.push(("episode".to_string(), episode));
    }
    if let Ok(entries) = std::fs::read_dir(dir.join("artifacts")) {
        let mut sessions: Vec<(String, PathBuf)> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter_map(|path| {
                let name = path.file_name()?.to_str()?;
                let log = name.strip_suffix(".atif.jsonl")?.to_string();
                Some((log, path))
            })
            .collect();
        sessions.sort();
        found.extend(sessions);
    }
    found
}

/// Every event in a finished trial's log directory, in time order.
#[must_use]
pub fn load(dir: &Path) -> Vec<Event> {
    let mut events = Vec::new();
    for (log, path) in logs(dir) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        events.extend(text.lines().filter_map(|line| parse(line, &log)));
    }
    events.sort_by_key(|event| event.at);
    events
}

/// Reads the new complete lines of every log in a directory.
#[derive(Default)]
pub struct Follower {
    offsets: BTreeMap<PathBuf, u64>,
}

impl Follower {
    /// The events written since the last poll, in time order. A line
    /// without its newline yet is left for the next poll.
    pub fn poll(&mut self, dir: &Path) -> Vec<Event> {
        let mut events = Vec::new();
        for (log, path) in logs(dir) {
            let offset = self.offsets.get(&path).copied().unwrap_or_default();
            let Ok(mut file) = std::fs::File::open(&path) else {
                continue;
            };
            if file.seek(SeekFrom::Start(offset)).is_err() {
                continue;
            }
            let mut bytes = Vec::new();
            if file.read_to_end(&mut bytes).is_err() {
                continue;
            }
            let Some(last) = bytes.iter().rposition(|byte| *byte == b'\n') else {
                continue;
            };
            let complete = &bytes[..=last];
            self.offsets
                .insert(path.clone(), offset + complete.len() as u64);
            events.extend(
                String::from_utf8_lossy(complete)
                    .lines()
                    .filter_map(|line| parse(line, &log)),
            );
        }
        events.sort_by_key(|event| event.at);
        events
    }
}
