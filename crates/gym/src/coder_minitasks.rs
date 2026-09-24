//! Coder One's mini-task runs: small local episodes with their own
//! graders, labeled as mini-tasks rather than Terminal-Bench attempts.
//!
//! `coder-one minitask run` records each run as a directory under
//! `~/.openagents/coder-one/minitasks/`: a `manifest.json` in the
//! `openagents.coder-one.minitask-run.v1` shape, the durable episode log
//! `episode.atif.jsonl`, the grader's `verification/grade.json`, and, when
//! `verify.checks` ran, its coverage in `verification/checks.json`. This
//! module reads them into runs with their timelines, outcomes, and grades.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::timeline::{self, Timeline};

/// The manifest schema a run carries.
pub const RUN_SCHEMA: &str = "openagents.coder-one.minitask-run.v1";

/// The schema of this module's JSON.
pub const SCHEMA: &str = "openagents.gym.coder-minitasks.v1";

/// One mini-task run.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub dir: PathBuf,
    pub id: String,
    pub task: String,
    pub family: String,
    /// `scripted` or `cli`.
    pub executor_kind: String,
    pub executor: String,
    /// How the episode ended: `delegated`, `delegate_failed`, and so on.
    pub outcome: String,
    /// The grader's verdict: `passed`, `failed`, or `unavailable`.
    pub verdict: String,
    pub detail: String,
    pub reward: Option<f64>,
    pub started_at: Option<String>,
    pub milliseconds: Option<u64>,
    /// The session's host-control record, for the scripted executor.
    pub session: Value,
    /// The requirement coverage `verify.checks` wrote, when it ran.
    pub coverage: Option<Value>,
    /// The `verify.repair` record, when a repair was asked for.
    pub repair: Option<Value>,
    /// The run's timeline, or why it doesn't read.
    pub timeline: Result<Timeline, String>,
}

/// Where mini-task runs are recorded by default.
#[must_use]
pub fn default_runs_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/minitasks"))
}

fn text(value: &Value, pointer: &str) -> String {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// Reads one run directory.
///
/// # Errors
///
/// Returns a message when the manifest is missing or isn't a mini-task
/// run.
pub fn read_run(dir: &Path) -> Result<Run, String> {
    let path = dir.join("manifest.json");
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("{} is not JSON: {error}", path.display()))?;
    if manifest.get("schema").and_then(Value::as_str) != Some(RUN_SCHEMA) {
        return Err(format!("{} is not a {RUN_SCHEMA} manifest", path.display()));
    }
    let log = dir.join(
        manifest
            .pointer("/files/invocation_log")
            .and_then(Value::as_str)
            .unwrap_or("episode.atif.jsonl"),
    );
    let read = |pointer: &str| -> Option<Value> {
        manifest
            .pointer(pointer)
            .and_then(Value::as_str)
            .and_then(|relative| std::fs::read_to_string(dir.join(relative)).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
    };
    // `verify.support`'s requirement states show beside the scenarios.
    let coverage = read("/files/checks").map(|mut report: Value| {
        if let Some(support) = read("/files/support") {
            report["support"] = support;
        }
        report
    });
    Ok(Run {
        dir: dir.to_path_buf(),
        id: text(&manifest, "/id"),
        task: text(&manifest, "/task/id"),
        family: text(&manifest, "/task/family"),
        executor_kind: text(&manifest, "/executor/kind"),
        executor: text(&manifest, "/executor/label"),
        outcome: text(&manifest, "/outcome"),
        verdict: text(&manifest, "/grade/verdict"),
        detail: text(&manifest, "/grade/detail"),
        reward: manifest.get("reward").and_then(Value::as_f64),
        started_at: manifest
            .get("started_at")
            .and_then(Value::as_str)
            .map(str::to_owned),
        milliseconds: manifest.get("milliseconds").and_then(Value::as_u64),
        session: manifest.get("session").cloned().unwrap_or(Value::Null),
        coverage,
        repair: read("/files/repair"),
        timeline: timeline::read_log(&log),
    })
}

/// Every run under `dir`, newest first, and the directories that didn't
/// read.
#[must_use]
pub fn load(dir: &Path) -> (Vec<Run>, Vec<String>) {
    let mut runs = Vec::new();
    let mut errors = Vec::new();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("manifest.json").is_file())
        .collect();
    dirs.sort();
    for path in dirs {
        match read_run(&path) {
            Ok(run) => runs.push(run),
            Err(error) => errors.push(error),
        }
    }
    runs.sort_by(|a, b| b.started_at.cmp(&a.started_at).then(b.id.cmp(&a.id)));
    (runs, errors)
}

impl Run {
    /// The run as versioned JSON; `detail` adds its timeline.
    #[must_use]
    pub fn to_json(&self, detail: bool) -> Value {
        let mut value = json!({
            "kind": "mini-task",
            "id": self.id,
            "dir": self.dir.display().to_string(),
            "task": self.task,
            "family": self.family,
            "executor": { "kind": self.executor_kind, "label": self.executor },
            "outcome": self.outcome,
            "grade": { "verdict": self.verdict, "detail": self.detail },
            "reward": self.reward,
            "started_at": self.started_at,
            "milliseconds": self.milliseconds,
            "invocations": self.timeline.as_ref().map(|t| t.entries.len()).ok(),
            "coverage": self.coverage.as_ref().map(crate::coder_coverage::summary),
            "repair": self.repair.as_ref().map(|r| json!({
                "triggered": r["triggered"],
                "changed": r["changed"],
                "skipped": r["skipped"],
                "brief": r["brief"],
                "session": r["session"],
            })),
        });
        if detail {
            value["session"] = self.session.clone();
            value["timeline"] = match &self.timeline {
                Ok(timeline) => timeline.to_json(),
                Err(error) => json!({ "error": error }),
            };
            value["coverage_report"] = self.coverage.clone().unwrap_or(Value::Null);
            value["repair_record"] = self.repair.clone().unwrap_or(Value::Null);
        }
        value
    }

    /// The run's one-line row.
    #[must_use]
    pub fn row(&self) -> String {
        format!(
            "{:<17} {:<22} {:<24} {:<16} {:<11} {:>7}",
            self.started_at
                .as_deref()
                .map_or("—".to_owned(), |at| at.chars().take(16).collect()),
            clip(&self.task, 22),
            clip(&self.executor, 24),
            clip(&self.outcome, 16),
            self.verdict,
            self.milliseconds
                .map_or("—".to_owned(), |ms| format!("{:.1}s", ms as f64 / 1000.0)),
        )
    }

    /// The run's detail: grade, session control, coverage, and timeline.
    #[must_use]
    pub fn detail_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!(
                "Mini-task {} ({}) · {} · {}",
                self.task, self.family, self.executor, self.id
            ),
            format!("Grade: {} · {}", self.verdict, self.detail),
            format!("Recorded in {}", self.dir.display()),
        ];
        if let Some(actions) = self.session.get("actions").and_then(Value::as_array) {
            lines.push(format!(
                "Session control: {}",
                actions
                    .iter()
                    .map(|action| format!(
                        "{} {}",
                        action["capability"].as_str().unwrap_or_default(),
                        action["outcome"].as_str().unwrap_or_default()
                    ))
                    .collect::<Vec<_>>()
                    .join(" · ")
            ));
        }
        if let Some(events) = self.session.get("events").and_then(Value::as_object) {
            lines.push(format!(
                "Executor events: {}",
                events
                    .iter()
                    .map(|(kind, count)| format!("{kind} {count}"))
                    .collect::<Vec<_>>()
                    .join(" · ")
            ));
        }
        if let Some(coverage) = &self.coverage {
            lines.extend(crate::coder_coverage::lines(coverage));
        }
        if let Some(repair) = &self.repair {
            lines.extend(repair_lines(repair));
        }
        match &self.timeline {
            Ok(timeline) => lines.extend(timeline.lines()),
            Err(error) => lines.push(format!("Could not read the episode timeline: {error}")),
        }
        lines
    }
}

fn clip(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

/// A `verify.repair` record as text: why it ran or didn't, the brief,
/// the fresh session, and the recheck.
#[must_use]
pub fn repair_lines(repair: &Value) -> Vec<String> {
    if let Some(why) = repair["skipped"].as_str() {
        return vec![format!("Repair: did not run · {why}")];
    }
    let brief = &repair["brief"];
    let session = &repair["session"];
    let mut lines = vec![
        format!(
            "Repair: {} brief · {} characters · requirements {} · scenarios {} · {}",
            brief["kind"].as_str().unwrap_or("?"),
            brief["chars"],
            brief["requirements"],
            brief["scenarios"],
            brief["path"].as_str().unwrap_or("no file")
        ),
        format!(
            "  session: {} · {} · {} ({}) · {} · cost {}",
            if session["fresh"] == json!(true) {
                "new session"
            } else {
                "resumed session"
            },
            session["session_id"].as_str().unwrap_or("?"),
            session["agent"].as_str().unwrap_or("?"),
            session["model"].as_str().unwrap_or("?"),
            session["status"].as_str().unwrap_or("?"),
            session["cost_usd"]
                .as_f64()
                .map_or("unknown".to_owned(), |usd| format!("${usd:.4}")),
        ),
    ];
    let invalidated = repair["invalidated"].as_array().map_or(0, Vec::len);
    lines.push(match repair["recheck"].get("summary") {
        Some(summary) => format!(
            "  the repair changed the candidate: {} requirement results are out of date · rechecked {} scenarios and {} diagnostic packets",
            invalidated, summary["scenarios"], summary["packets"]
        ),
        None => format!(
            "  the repair left the candidate unchanged: {}",
            repair["recheck"]["skipped"].as_str().unwrap_or("no recheck")
        ),
    });
    lines
}

/// The header line above the rows.
pub const HEADER: &str = "started           task                   executor                 outcome          grade          time";

/// The list of runs, then the selected run's detail.
#[must_use]
pub fn lines(runs: &[Run], errors: &[String], selected: Option<usize>) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Coder One mini-task runs · {} runs · local episodes with their own graders, not Terminal-Bench attempts",
            runs.len()
        ),
        "Fast local screen: graders check invariants after each episode.".to_owned(),
        "Checks: severity-field counts, typed terminal input, completed cleanup,".to_owned(),
        "and a recovered clean commit. Scripted run: about 1 second, $0 model cost.".to_owned(),
        "Not a Terminal-Bench 4.0 result. Real-executor figures and details:".to_owned(),
        "docs/coder/guides/coder-one-minitasks.md.".to_owned(),
        HEADER.to_owned(),
    ];
    if runs.is_empty() {
        lines.push("  No runs. Record one with `coder-one minitask run ID`.".to_owned());
    }
    lines.extend(runs.iter().map(Run::row));
    lines.extend(
        errors
            .iter()
            .map(|error| format!("Could not read: {error}")),
    );
    if let Some(run) = selected.and_then(|index| runs.get(index)) {
        lines.push(String::new());
        lines.extend(run.detail_lines());
    }
    lines
}

const HELP: &str = "\
gym coder minitasks [--runs-dir PATH] [--run ID|latest] [--task ID] [--json]

Lists Coder One's mini-task runs, newest first: task, executor, how the
episode ended, and the grader's verdict. --run shows one run's detail:
how the session was started and stopped, which requirements its checks
covered, and every component call in order.
Runs are read from ~/.openagents/coder-one/minitasks unless --runs-dir
names another directory. Record one with `coder-one minitask run ID`.";

/// `gym coder minitasks …`.
///
/// # Errors
///
/// Returns a message for an unknown option or a run that isn't there.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let mut dir = default_runs_dir();
    let mut run = None;
    let mut task = None;
    let mut json_output = false;
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        match argument {
            "help" | "--help" | "-h" => {
                writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
                return Ok(0);
            }
            "--json" => json_output = true,
            "--runs-dir" | "--run" | "--task" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{argument} needs a value"))?
                    .clone();
                match argument {
                    "--runs-dir" => dir = Some(value.into()),
                    "--run" => run = Some(value),
                    _ => task = Some(value),
                }
                index += 1;
            }
            other => return Err(format!("unknown option {other}\n\n{HELP}")),
        }
        index += 1;
    }
    let dir = dir.ok_or("no --runs-dir and no HOME")?;
    let (mut runs, errors) = load(&dir);
    if let Some(task) = &task {
        runs.retain(|r| &r.task == task);
    }
    let selected = match run.as_deref() {
        None => None,
        Some("latest") => (!runs.is_empty()).then_some(0),
        Some(id) => Some(
            runs.iter()
                .position(|r| {
                    r.id == id
                        || r.dir
                            .file_name()
                            .is_some_and(|name| name.to_string_lossy() == id)
                })
                .ok_or_else(|| format!("no mini-task run {id} under {}", dir.display()))?,
        ),
    };
    if json_output {
        let value = match selected {
            Some(index) => json!({ "schema": SCHEMA, "run": runs[index].to_json(true) }),
            None => json!({
                "schema": SCHEMA,
                "runs_dir": dir.display().to_string(),
                "runs": runs.iter().map(|r| r.to_json(false)).collect::<Vec<_>>(),
                "read_errors": errors,
            }),
        };
        serde_json::to_writer_pretty(&mut *out, &value).map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())?;
    } else {
        let shown = match selected {
            Some(index) => runs[index].detail_lines(),
            None => lines(&runs, &errors, None),
        };
        for line in shown {
            writeln!(out, "{line}").map_err(|error| error.to_string())?;
        }
    }
    Ok(0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A run directory shaped like the one `coder-one minitask run` writes.
    pub(crate) fn fixture_run(root: &Path, name: &str, verdict: &str) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(dir.join("verification")).unwrap();
        let session = atif::Session::opening(name, "none", "mini-task", "/w", "v");
        let path = dir.join("episode.atif.jsonl");
        let mut log = atif::Log::create_at(&path, &session).unwrap();
        let event = |kind: &str, id: &str, component: &str, at: u64, extra: Value| {
            let mut record = json!({
                "schema": timeline::INVOCATION_SCHEMA, "event": kind, "id": id, "parent": null,
                "component": component, "name": "mini-task", "at": at,
                "implementation": { "name": "mini-task", "digest": "0123456789abcdef" },
            });
            if let (Some(record), Some(extra)) = (record.as_object_mut(), extra.as_object()) {
                record.extend(extra.clone());
            }
            let step: atif::document::Step = serde_json::from_value(json!({
                "at": at, "source": "System", "message": "", "extensions": { "invocation": record }
            }))
            .unwrap();
            step
        };
        log.append(&event("start", "inv-1", "episode", 1000, json!({})))
            .unwrap();
        log.append(&event(
            "end",
            "inv-1",
            "episode",
            1100,
            json!({ "outcome": "completed", "milliseconds": 100, "output": { "summary": { "outcome": "delegated" } } }),
        ))
        .unwrap();
        log.finish(atif::log::ENDED).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            json!({
                "schema": RUN_SCHEMA, "kind": "mini-task", "id": name,
                "task": { "id": "log-severity", "family": "field meaning in data" },
                "executor": { "kind": "scripted", "label": "scripted-bad" },
                "session": { "actions": [{ "capability": "start", "outcome": "done" }], "events": { "assistant_claim": 2 } },
                "outcome": "delegated",
                "grade": { "verdict": verdict, "detail": "summary.csv differs" },
                "reward": if verdict == "passed" { json!(1.0) } else { json!(0.0) },
                "started_at": "2026-09-22T10:00:00.000Z",
                "milliseconds": 120,
                "files": { "invocation_log": "episode.atif.jsonl" },
            })
            .to_string(),
        )
        .unwrap();
        dir
    }

    #[test]
    fn a_run_reads_with_its_timeline_grade_and_coverage() {
        let root = std::env::temp_dir().join(format!("gym-minitasks-{}", std::process::id()));
        fixture_run(&root, "minitask-log-severity-scripted-bad-1", "failed");
        let (runs, errors) = load(&root);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        assert_eq!(run.verdict, "failed");
        assert!(run.timeline.as_ref().unwrap().complete);
        let text = lines(&runs, &errors, Some(0)).join("\n");
        assert!(text.contains("not Terminal-Bench attempts"), "{text}");
        assert!(text.contains("graders check invariants after each episode"), "{text}");
        assert!(text.contains("severity-field counts, typed terminal input, completed cleanup,"), "{text}");
        assert!(text.contains("and a recovered clean commit. Scripted run: about 1 second, $0 model cost"), "{text}");
        assert!(text.contains("Not a Terminal-Bench 4.0 result"), "{text}");
        assert!(text.contains("Grade: failed"), "{text}");
        assert!(text.contains("Session control: start done"), "{text}");
        assert!(text.contains("Episode timeline"), "{text}");
        let mut out = Vec::new();
        command(
            &[
                "--runs-dir".to_owned(),
                root.display().to_string(),
                "--run".to_owned(),
                "latest".to_owned(),
                "--json".to_owned(),
            ],
            &mut out,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(value["run"]["kind"], json!("mini-task"));
        let _ = std::fs::remove_dir_all(root);
    }
}
