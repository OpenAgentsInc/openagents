//! Candidates recovered from retained Terminal-Bench trials.
//!
//! Harbor retained each trial's native executor stream, its trajectory,
//! and the verifier's output, but not the files the agent left in `/app`.
//! A candidate is recoverable when the stream holds it: a file written in
//! full by a here-document or a `Write` call, or a program fed to an
//! interpreter inline. A file the stream only names, such as one changed
//! by a patch or written by a program, can't be rebuilt, and the trial is
//! reported as unavailable rather than guessed at.
//!
//! The observed input samples come from the probe outputs the trajectory
//! recorded; a task-provided file, such as `base_terminal.py`, comes from a
//! `cat` of it in this trial's stream or a sibling trial's. The verifier's
//! test names are read only to prove that no scenario carries one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};

use super::{Budget, Candidate, InlineProgram, Input, Observed, TaskText};
use crate::stream::{self, Format};

/// The tasks whose failure families the scenarios cover.
pub const TASKS: &[&str] = &[
    "log-summary-date-ranges",
    "headless-terminal",
    "cancel-async-tasks",
];

/// One retained trial and what could be recovered from it.
#[derive(Clone, Debug, Serialize)]
pub struct Recovered {
    pub job: String,
    pub trial: String,
    pub arm: String,
    pub task: String,
    /// The verifier's reward.
    pub reward: Option<f64>,
    /// The check's input, when the candidate could be rebuilt.
    pub input: Option<Input>,
    /// Why it couldn't, when it couldn't.
    pub unavailable: Option<String>,
    /// The verifier's test names, kept out of every scenario.
    #[serde(skip)]
    pub protected_names: Vec<String>,
    /// Where each part came from.
    pub sources: Value,
}

fn job_parts(job: &str) -> Option<(String, String)> {
    let mut parts = job.split("--");
    let _profile = parts.next()?;
    let arm = parts.next()?.to_string();
    let rest = parts.next()?;
    let task = TASKS.iter().find(|t| rest.starts_with(**t))?;
    Some((arm, (*task).to_string()))
}

fn episode_dir(job_dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(job_dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir() && p.extension().is_some_and(|x| x == "episode"))
}

fn streams(episode: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(episode.join("artifacts"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(".stream.jsonl"))
        })
        .collect();
    found.sort();
    found
}

/// The task instruction: the trajectory's first user message.
fn instruction(job_dir: &Path) -> Option<String> {
    let trajectory = std::fs::read_dir(job_dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "json"))?;
    let value: Value = serde_json::from_str(&std::fs::read_to_string(trajectory).ok()?).ok()?;
    value["steps"]
        .as_array()?
        .iter()
        .find(|step| step["source"] == "user")
        .and_then(|step| step["message"].as_str())
        .map(str::to_string)
}

/// Record-shaped lines anywhere in the trajectory: a date, a time, and a
/// bracketed upper-case field.
fn samples(job_dir: &Path) -> Vec<String> {
    let Some(path) = std::fs::read_dir(job_dir).ok().and_then(|d| {
        d.flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|x| x == "json"))
    }) else {
        return Vec::new();
    };
    let Ok(value) =
        serde_json::from_str::<Value>(&std::fs::read_to_string(path).unwrap_or_default())
    else {
        return Vec::new();
    };
    let mut strings = Vec::new();
    let mut stack = vec![&value];
    while let Some(v) = stack.pop() {
        match v {
            Value::String(s) => strings.push(s.as_str()),
            Value::Array(a) => stack.extend(a.iter()),
            Value::Object(o) => stack.extend(o.values()),
            _ => {}
        }
    }
    let mut out: Vec<String> = Vec::new();
    for text in strings {
        for line in text.lines() {
            // A probe may prefix the record, as `grep -n` does with `11: `;
            // the record starts at its date.
            let words: Vec<&str> = line.split_whitespace().collect();
            let Some(at) = (0..words.len().saturating_sub(3)).find(|&i| {
                super::parse_day(words[i]).is_some()
                    && words[i + 1].len() == 8
                    && words[i + 1].chars().filter(|c| *c == ':').count() == 2
                    && words[i + 2].len() > 2
                    && words[i + 2].starts_with('[')
                    && words[i + 2].ends_with(']')
                    && words[i + 2][1..words[i + 2].len() - 1]
                        .chars()
                        .all(|c| c.is_ascii_uppercase())
            }) else {
                continue;
            };
            let record = words[at..].join(" ");
            if !out.contains(&record) && out.len() < 40 {
                out.push(record);
            }
        }
    }
    out.sort();
    out
}

/// The trajectory's tool calls as a Claude Code stream, for an agent that
/// kept no native stream: `Write`, `Edit`, `MultiEdit`, and `Bash` calls
/// in order.
fn trajectory_calls(job_dir: &Path) -> Option<(PathBuf, String)> {
    let path = std::fs::read_dir(job_dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "json"))?;
    let value: Value = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
    let mut lines = Vec::new();
    for step in value["steps"].as_array()? {
        for call in step["tool_calls"].as_array().into_iter().flatten() {
            let name = call["function_name"].as_str().unwrap_or_default();
            if matches!(name, "Write" | "Edit" | "MultiEdit" | "Bash") {
                lines.push(
                    json!({ "type": "assistant", "message": { "content": [{ "type": "tool_use", "name": name, "input": call["arguments"] }] } })
                        .to_string(),
                );
            }
        }
    }
    (!lines.is_empty()).then(|| (path, lines.join("\n")))
}

/// The content of `name` as a `cat` in a stream printed it.
fn catted(stream_text: &str, name: &str) -> Option<String> {
    let format = Format::detect(stream_text)?;
    for line in stream_text.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if format == Format::Codex
            && event["type"] == "item.completed"
            && event["item"]["type"] == "command_execution"
            && event["item"]["exit_code"] == 0
        {
            let script =
                stream::unwrap_shell(event["item"]["command"].as_str().unwrap_or_default());
            let words = stream::shell_words(&script);
            if words.len() == 2 && words[0] == "cat" && super::base_name(&words[1]) == name {
                return event["item"]["aggregated_output"]
                    .as_str()
                    .map(str::to_string);
            }
        }
    }
    None
}

fn read_reward(episode: &Path) -> Option<f64> {
    std::fs::read_to_string(episode.join("verifier/reward.txt"))
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn protected_names(episode: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(episode.join("verifier/ctrf.json")) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    value
        .pointer("/results/tests")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|t| t["name"].as_str())
        .map(|name| name.rsplit("::").next().unwrap_or(name).to_string())
        .filter(|name| name.len() > 6)
        .collect()
}

/// Recovers every trial of the covered tasks under `traces` whose arm is
/// `arm`, or every arm when `arm` is `all`.
///
/// # Errors
///
/// Returns a message when `traces` doesn't read.
pub fn recover_tree(traces: &Path, arm: &str) -> Result<Vec<Recovered>, String> {
    let mut jobs: Vec<PathBuf> = std::fs::read_dir(traces)
        .map_err(|error| format!("cannot read {}: {error}", traces.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    jobs.sort();
    // Task-provided files, by task, from any trial that printed them.
    let mut provided: BTreeMap<(String, String), (String, String)> = BTreeMap::new();
    for job in &jobs {
        let name = job
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let Some((_, task)) = job_parts(&name) else {
            continue;
        };
        let Some(episode) = episode_dir(job) else {
            continue;
        };
        for path in streams(&episode) {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            for file in ["base_terminal.py"] {
                if let Some(content) = catted(&text, file) {
                    provided
                        .entry((task.clone(), file.to_string()))
                        .or_insert((content, path.display().to_string()));
                }
            }
        }
    }
    let mut out = Vec::new();
    for job in &jobs {
        let name = job
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let Some((job_arm, task)) = job_parts(&name) else {
            continue;
        };
        if arm != "all" && job_arm != arm {
            continue;
        }
        let Some(episode) = episode_dir(job) else {
            continue;
        };
        let trial = episode
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let mut recovered = Recovered {
            job: name.clone(),
            trial,
            arm: job_arm,
            task: task.clone(),
            reward: read_reward(&episode),
            input: None,
            unavailable: None,
            protected_names: protected_names(&episode),
            sources: Value::Null,
        };
        let Some(instruction) = instruction(job) else {
            recovered.unavailable = Some("the trajectory holds no task instruction".to_string());
            out.push(recovered);
            continue;
        };
        let mut paths = streams(&episode);
        let mut texts: Vec<String> = paths
            .iter()
            .map(|path| std::fs::read_to_string(path).unwrap_or_default())
            .collect();
        if paths.is_empty() {
            // A direct agent keeps no delegate stream, but its trajectory
            // holds its tool calls.
            match trajectory_calls(job) {
                Some((path, text)) => {
                    paths.push(path);
                    texts.push(text);
                }
                None => {
                    recovered.unavailable = Some(
                        "no native executor stream was retained, and the trajectory holds no tool calls".to_string(),
                    );
                    out.push(recovered);
                    continue;
                }
            }
        }
        let mut writes = Vec::new();
        let mut programs = Vec::new();
        for text in &texts {
            let text = text.clone();
            let format = Format::detect(&text).unwrap_or(Format::Codex);
            writes.extend(stream::writes(format, &text));
            programs.extend(stream::programs(&text).into_iter().map(|p| InlineProgram {
                interpreter: p.interpreter,
                source: p.source,
            }));
        }
        let finals = stream::final_files(&writes);
        let unknown: Vec<String> = finals
            .iter()
            .filter(|w| w.how == stream::UNKNOWN)
            .map(|w| w.path.clone())
            .collect();
        if !unknown.is_empty() {
            recovered.unavailable = Some(format!(
                "the stream edits {} in a way it doesn't hold the content to replay",
                unknown.join(", ")
            ));
            out.push(recovered);
            continue;
        }
        let files: BTreeMap<String, String> =
            finals.into_iter().map(|w| (w.path, w.content)).collect();
        if files.is_empty() && programs.is_empty() {
            recovered.unavailable = Some(
                "the stream names the agent's files but holds none of their content".to_string(),
            );
            out.push(recovered);
            continue;
        }
        let mut given = BTreeMap::new();
        let mut given_from = Vec::new();
        for ((t, file), (content, from)) in &provided {
            if t == &task && !files.keys().any(|p| super::base_name(p) == file) {
                given.insert(file.clone(), content.clone());
                given_from.push(json!({ "file": file, "from": from }));
            }
        }
        let observed = Observed {
            samples: samples(job),
            source: "record-shaped lines in the retained trajectory's probe outputs".to_string(),
        };
        recovered.sources = json!({
            "streams": paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "files": files.keys().collect::<Vec<_>>(),
            "programs": programs.len(),
            "provided": given_from,
            "samples": observed.samples.len(),
        });
        recovered.input = Some(Input {
            task: TaskText {
                title: task.clone(),
                instruction,
            },
            requirements: None,
            candidate: Candidate {
                label: format!("{} / {}", recovered.job, recovered.trial),
                origin: "stream".to_string(),
                files,
                programs,
                provided: given,
            },
            observed,
            budget: Budget::default(),
            workspace: None,
        });
        out.push(recovered);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_v3_luna_failures_have_recoverable_candidates() {
        let traces =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces");
        let Ok(all) = recover_tree(&traces, "coder-one-jevprobe3-luna") else {
            return;
        };
        if all.is_empty() {
            return;
        }
        assert_eq!(all.len(), 9, "three trials of three tasks");
        let failed: Vec<&Recovered> = all.iter().filter(|r| r.reward == Some(0.0)).collect();
        assert_eq!(failed.len(), 5);
        for one in &failed {
            let input = one
                .input
                .as_ref()
                .unwrap_or_else(|| panic!("{}: {:?}", one.trial, one.unavailable));
            assert!(!input.candidate.files.is_empty() || !input.candidate.programs.is_empty());
            if one.task == "log-summary-date-ranges" {
                assert!(
                    !input.observed.samples.is_empty(),
                    "{} has no samples",
                    one.trial
                );
            }
            if one.task == "headless-terminal" {
                assert!(input.candidate.provided.contains_key("base_terminal.py"));
            }
        }
    }
}
