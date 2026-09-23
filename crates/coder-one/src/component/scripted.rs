//! `exec.scripted` and `task.mini`: the scripted executor's session
//! handling and whole mini-task episodes, as components the runner can
//! suite in seconds.

use std::path::PathBuf;
use std::time::Duration;

use futures_util::future::LocalBoxFuture;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use super::{Component, Fixture, Ran, input};
use crate::component::jev::JevMode;
use crate::delegate::{Briefing, BriefingInputs};
use crate::record::{Implementation, Recorder};
use crate::scripted::{Script, Scripted};
use crate::session::{self, Controls};

fn scratch(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "coder-one-component-{label}-{}-{}",
        std::process::id(),
        atif::now_ms()
    ))
}

/// A replay of a retained native stream.
#[derive(Deserialize)]
struct Replay {
    name: String,
    stream: String,
    #[serde(default)]
    rebind: Vec<String>,
    #[serde(default = "gap")]
    gap_ms: u64,
}

fn gap() -> u64 {
    5
}

#[derive(Deserialize, Default)]
struct Expect {
    status: Option<String>,
    #[serde(default)]
    files: Vec<String>,
    /// `capability:outcome` words, in order.
    #[serde(default)]
    actions: Vec<String>,
}

#[derive(Deserialize)]
struct ScriptedInput {
    #[serde(default)]
    script: Option<Script>,
    #[serde(default)]
    replay: Option<Replay>,
    #[serde(default)]
    controls: Controls,
    #[serde(default)]
    expect: Expect,
}

/// `exec.scripted`: a scripted session under host control.
pub struct ScriptedAdapter;

impl Component for ScriptedAdapter {
    fn id(&self) -> &'static str {
        "exec.scripted"
    }
    fn implementation(&self) -> Implementation {
        Implementation::new(
            "exec.scripted",
            "scripted executor under host session control",
            &json!({ "version": 1, "stream": "codex|claude", "capabilities": ["start", "observe", "stop", "resume", "steer"] }),
        )
    }
    fn about(&self) -> &'static str {
        "The host starts, observes, steers, stops, and resumes a scripted session."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        _jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: ScriptedInput = input(fixture)?;
            let script = match (input.script, input.replay) {
                (Some(script), _) => script,
                (None, Some(replay)) => {
                    let rebind: Vec<&str> = replay.rebind.iter().map(String::as_str).collect();
                    Script::from_stream(&replay.name, &replay.stream, replay.gap_ms, &rebind)
                }
                (None, None) => {
                    return Err("the input has neither a script nor a replay".to_string());
                }
            };
            let dir = scratch("scripted");
            std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
            let mut scripted = Scripted::new(script, dir.clone());
            scripted.recorder = recorder.clone();
            let briefing = Briefing::build(
                &BriefingInputs {
                    instruction: "Run the script.".to_string(),
                    requirements: Vec::new(),
                    files: Vec::new(),
                    spans: Vec::new(),
                    commands: Vec::new(),
                    last_output: None,
                    conclusion: String::new(),
                    directions: String::new(),
                },
                1_000,
            );
            let driven = session::drive(
                &mut scripted,
                &briefing,
                &input.controls,
                recorder,
                &mut session::virtual_time(),
            )
            .await;
            let mut files = Map::new();
            let mut stack = vec![dir.clone()];
            while let Some(at) = stack.pop() {
                for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else if let Ok(bytes) = std::fs::read(&path) {
                        let relative = path
                            .strip_prefix(&dir)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .into_owned();
                        files.insert(relative, json!(format!("{:x}", Sha256::digest(&bytes))));
                    }
                }
            }
            let _ = std::fs::remove_dir_all(&dir);
            let actions: Vec<String> = driven
                .actions
                .iter()
                .map(|a| format!("{}:{}", a.capability.word(), a.outcome))
                .collect();
            let status = driven.report.status.word();
            let mut matches = true;
            if let Some(want) = &input.expect.status {
                matches &= want == status;
            }
            matches &= input.expect.files.iter().all(|f| files.contains_key(f));
            if !input.expect.actions.is_empty() {
                matches &= input.expect.actions == actions;
            }
            let mut metrics = Map::new();
            metrics.insert("answered".to_string(), json!(status == "answered"));
            metrics.insert("events".to_string(), json!(driven.events.len()));
            metrics.insert(
                "refused".to_string(),
                json!(
                    driven
                        .actions
                        .iter()
                        .filter(|a| a.outcome == "refused")
                        .count()
                ),
            );
            metrics.insert("files".to_string(), json!(files.len()));
            metrics.insert("script_errors".to_string(), json!(scripted.errors().len()));
            metrics.insert("matches_expected".to_string(), json!(matches));
            Ok(Ran {
                output: json!({
                    "status": status,
                    "session_id": driven.session_id,
                    "events": crate::stream::tally(&driven.events),
                    "actions": actions,
                    "stops": driven.stops.iter().map(|s| json!({ "at_ms": s.at_ms, "pending": s.pending })).collect::<Vec<_>>(),
                    "files": files,
                    "stream_sha256": format!("{:x}", Sha256::digest(scripted.stream().as_bytes())),
                }),
                metrics,
            })
        })
    }
}

#[derive(Deserialize)]
struct MiniInput {
    task: String,
    variant: String,
    /// The grade the variant should get.
    expect: String,
    /// Whether verify.checks runs before the grader.
    #[serde(default)]
    checks: bool,
}

/// `task.mini`: a whole mini-task episode with the scripted executor.
pub struct MiniTaskRun;

impl Component for MiniTaskRun {
    fn id(&self) -> &'static str {
        "task.mini"
    }
    fn implementation(&self) -> Implementation {
        Implementation::new(
            "task.mini",
            "mini-task episode, scripted executor",
            &json!({ "version": 1, "explore_steps": 0, "mode": "always" }),
        )
    }
    fn about(&self) -> &'static str {
        "A whole mini-task episode with the scripted executor, graded."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        _jev: &'a JevMode,
        _recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: MiniInput = input(fixture)?;
            let task = crate::minitask::find(&input.task)?;
            let out = scratch("minitask");
            let ran = crate::minitask::run::run(crate::minitask::run::Options {
                task,
                executor: crate::minitask::run::ExecutorChoice::Scripted {
                    variant: input.variant.clone(),
                    script: None,
                },
                out: out.clone(),
                jev: None,
                speed: 0.0,
                deadline: Duration::from_secs(60),
                controls: Controls::default(),
                checks: input.checks,
                brief: None,
            })
            .await;
            let _ = std::fs::remove_dir_all(&out);
            let ran = ran?;
            let mut metrics = Map::new();
            metrics.insert(
                "passed".to_string(),
                if ran.grade.verdict == "unavailable" {
                    Value::Null
                } else {
                    json!(ran.grade.verdict == "passed")
                },
            );
            metrics.insert(
                "matches_expected".to_string(),
                if ran.grade.verdict == "unavailable" {
                    Value::Null
                } else {
                    json!(ran.grade.verdict == input.expect)
                },
            );
            if input.checks {
                metrics.insert(
                    "checks_detected".to_string(),
                    ran.manifest["checks"]["verdicts"]
                        .get("failed")
                        .map_or(json!(false), |n| json!(n.as_u64().unwrap_or(0) > 0)),
                );
            }
            metrics.insert(
                "under_ten_seconds".to_string(),
                json!(ran.milliseconds < 10_000),
            );
            Ok(Ran {
                output: json!({
                    "task": input.task,
                    "variant": input.variant,
                    "outcome": ran.manifest["outcome"],
                    "grade": ran.grade,
                }),
                metrics,
            })
        })
    }
}
