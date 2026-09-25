//! `evidence.baseline` as a component: find a workspace's entry points and
//! run each once in a bounded scratch copy ([`crate::baseline`], issue
//! #9633).
//!
//! The fixture's input holds the instruction (`task`), and either a
//! workspace directory (`workspace`, where a leading `~/` is the home
//! directory) or the workspace's files inline (`files`, path to text).
//! `workdir` is the directory the instruction calls the workspace, `/app`
//! by default for inline files and the directory itself otherwise.
//! `wall_sec` shortens the 60-second bound, for a fixture that tests it.
//!
//! The fixture's `retained` states what must happen: `runs`, each with its
//! `kind`, and optionally `exit`, `timed_out`, `stdout` and `stderr`
//! substrings, and `command`; `refused`, each with its `kind` and a `why`
//! substring; and `none` for a task with no entry point. The run reports
//! `matches_retained` when this host can enforce the boundary.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use futures_util::future::LocalBoxFuture;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::jev::JevMode;
use super::{Component, Fixture, Ran, input};
use crate::baseline::{self, Baseline, Setup};
use crate::record::{Implementation, Recorder};

/// The `evidence.baseline` component.
pub struct BaselineComponent;

#[derive(Deserialize)]
struct BaselineInput {
    task: String,
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    files: BTreeMap<String, String>,
    #[serde(default)]
    workdir: Option<String>,
    #[serde(default)]
    wall_sec: Option<u64>,
}

/// A directory that removes itself.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn expand(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

/// Whether this host can build the boundary the runs need.
fn enforced() -> bool {
    coder_boundary::Boundary::writing(std::env::temp_dir())
        .offline()
        .build()
        .is_ok()
}

fn contains(value: &Value, field: &str, have: &str) -> bool {
    value[field].as_str().is_none_or(|want| have.contains(want))
}

/// Whether `baseline` does what `retained` states.
fn matches(baseline: &Baseline, retained: &Value) -> bool {
    if let Some(none) = retained["none"].as_bool()
        && none != baseline.runs.is_empty()
    {
        return false;
    }
    let runs = retained["runs"].as_array().cloned().unwrap_or_default();
    if runs.len() != baseline.runs.len() {
        return false;
    }
    for (want, run) in runs.iter().zip(&baseline.runs) {
        let ok = want["kind"].as_str() == Some(run.kind.word())
            && want["exit"]
                .as_i64()
                .is_none_or(|e| run.exit == i32::try_from(e).ok())
            && want["timed_out"]
                .as_bool()
                .is_none_or(|t| t == run.timed_out)
            && contains(want, "command", &run.command)
            && contains(want, "stdout", &run.stdout_head)
            && contains(want, "stderr", &run.stderr_head);
        if !ok {
            return false;
        }
    }
    let refused = retained["refused"].as_array().cloned().unwrap_or_default();
    refused.len() == baseline.refused.len()
        && refused.iter().zip(&baseline.refused).all(|(want, entry)| {
            want["kind"].as_str() == Some(entry.kind.word())
                && contains(want, "why", entry.refused.as_deref().unwrap_or_default())
        })
}

impl Component for BaselineComponent {
    fn id(&self) -> &'static str {
        baseline::COMPONENT
    }
    fn implementation(&self) -> Implementation {
        baseline::implementation()
    }
    fn about(&self) -> &'static str {
        "Code finds the task's own programs and runs each once in a bounded scratch copy before \
         the first session."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        _jev: &'a JevMode,
        _recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: BaselineInput = input(fixture)?;
            let mut _scratch = None;
            let (root, alias) = match (&input.workspace, input.files.is_empty()) {
                (Some(dir), true) => {
                    let root = expand(dir);
                    let alias = input
                        .workdir
                        .clone()
                        .unwrap_or_else(|| root.display().to_string());
                    (root, alias)
                }
                (None, false) => {
                    let dir = std::env::temp_dir().join(format!(
                        "baseline-fixture-{}-{}",
                        std::process::id(),
                        atif::digest(&json!(input.files))
                            .get(..12)
                            .unwrap_or_default()
                    ));
                    let _ = std::fs::remove_dir_all(&dir);
                    for (path, text) in &input.files {
                        let at = dir.join(path);
                        if let Some(parent) = at.parent() {
                            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                        }
                        std::fs::write(&at, text).map_err(|e| e.to_string())?;
                    }
                    _scratch = Some(Scratch(dir.clone()));
                    let alias = input.workdir.clone().unwrap_or_else(|| "/app".to_string());
                    (dir, alias)
                }
                _ => {
                    return Err(
                        "the evidence.baseline input needs a workspace or files, not both"
                            .to_string(),
                    );
                }
            };
            if !root.is_dir() {
                return Err(format!("no workspace at {}", root.display()));
            }
            let setup = Setup {
                root,
                alias,
                wall: input
                    .wall_sec
                    .map_or(baseline::WALL, Duration::from_secs)
                    .min(baseline::WALL),
                container: false,
            };
            let found = baseline::run(&input.task, &setup).await;
            let mut metrics = Map::new();
            metrics.insert("runs".to_string(), json!(found.runs.len()));
            metrics.insert("refused".to_string(), json!(found.refused.len()));
            metrics.insert("finished".to_string(), json!(found.commands().len()));
            metrics.insert(
                "timed_out".to_string(),
                json!(found.runs.iter().filter(|r| r.timed_out).count()),
            );
            metrics.insert("entry_point".to_string(), json!(!found.runs.is_empty()));
            if let Some(untouched) = found.untouched {
                metrics.insert("untouched".to_string(), json!(untouched));
            }
            for kind in ["named", "module", "make", "script"] {
                metrics.insert(
                    format!("runs_{kind}"),
                    json!(found.runs.iter().filter(|r| r.kind.word() == kind).count()),
                );
            }
            if !fixture.retained.is_null() && enforced() {
                metrics.insert(
                    "matches_retained".to_string(),
                    json!(matches(&found, &fixture.retained)),
                );
            }
            Ok(Ran {
                output: json!({
                    "baseline": found,
                    "commands": found.commands(),
                    "evidence": found.evidence().map(|e| json!({"label": e.label, "text": e.text})),
                    "records": found.records(),
                }),
                metrics,
            })
        })
    }
}
