//! `accept.grade` as a component: split a frozen score script into check
//! lines and grade each by whether its expectation is supported
//! ([`crate::grade`], issue #9635).
//!
//! The fixture's input names the task and the script, inline (`script`)
//! or as a path (`script_path`, where a leading `~/` is the home
//! directory, and a relative path is from the repository root), and the
//! task's baseline behavior when there is one (`baseline`), and each check
//! line's result on the untouched workspace when it was run there
//! (`start`), which sets the line's authority class. The output is
//! the `check-grades.json` record the lean loop writes at the freeze, with
//! the instrumented copy's size so a run shows whether line results would
//! be recorded.

use std::path::PathBuf;

use futures_util::future::LocalBoxFuture;
use serde::Deserialize;
use serde_json::{Map, json};

use super::jev::JevMode;
use super::{Component, Fixture, Ran, input};
use crate::grade::{self, Freeze, Grade, Split};
use crate::record::{Implementation, Recorder};

/// The `accept.grade` component.
pub struct GradeComponent;

#[derive(Deserialize)]
struct GradeInput {
    task: String,
    #[serde(default)]
    script: Option<String>,
    #[serde(default)]
    script_path: Option<String>,
    #[serde(default)]
    baseline: Option<String>,
    /// Each check line's result on the untouched workspace, by line ID.
    #[serde(default)]
    start: Option<std::collections::BTreeMap<String, bool>>,
    #[serde(default = "default_check")]
    check: String,
}

fn default_check() -> String {
    "lean-1/evaluator/score.sh".to_string()
}

fn resolve(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ if path.starts_with('/') => PathBuf::from(path),
        _ => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path),
    }
}

impl Component for GradeComponent {
    fn id(&self) -> &'static str {
        grade::COMPONENT
    }
    fn implementation(&self) -> Implementation {
        grade::implementation()
    }
    fn about(&self) -> &'static str {
        "Code splits the frozen score script into check lines; Jev grades whether each line's \
         expectation follows from the task, the baseline, or a standard definition."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: GradeInput = input(fixture)?;
            let script = match (&input.script, &input.script_path) {
                (Some(text), None) => text.clone(),
                (None, Some(path)) => std::fs::read_to_string(resolve(path))
                    .map_err(|e| format!("cannot read {path}: {e}"))?,
                _ => {
                    return Err(
                        "the accept.grade input needs a script or a script_path, not both"
                            .to_string(),
                    );
                }
            };
            let (grades, parsed, calls) = grade::grade(
                jev,
                recorder,
                &grade::support::Context {
                    component: self.id(),
                    name: grade::DECISION,
                    id: "grade".to_string(),
                    deadline: None,
                },
                &Freeze {
                    check: input.check.clone(),
                    script: &script,
                    frozen_after_session: 1,
                    task: &input.task,
                    baseline: input.baseline.as_deref(),
                    start: input.start.as_ref(),
                },
            )
            .await;
            let instrumented = grade::instrument(&script, &parsed);
            let count = |g: Grade| grades.lines.iter().filter(|l| l.grade == g).count();
            let mut metrics = Map::new();
            metrics.insert("lines".to_string(), json!(grades.lines.len()));
            metrics.insert(
                "one_unit".to_string(),
                json!(grades.split == Split::OneUnit),
            );
            metrics.insert("follows".to_string(), json!(count(Grade::Follows)));
            metrics.insert("advisory".to_string(), json!(count(Grade::Advisory)));
            metrics.insert("unknown".to_string(), json!(count(Grade::Unknown)));
            metrics.insert("instrumented".to_string(), json!(instrumented.is_some()));
            metrics.insert("jev_usd".to_string(), json!(grades.jev_usd));
            Ok(Ran {
                output: json!({
                    "grades": grades,
                    "calls": calls,
                    "instrumented_chars": instrumented.map(|t| t.len()),
                }),
                metrics,
            })
        })
    }
}
