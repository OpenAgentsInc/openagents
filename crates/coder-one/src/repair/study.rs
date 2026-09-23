//! The conditional recovery study: preserved candidates, and each repair
//! arm on an isolated copy of the same state.
//!
//! Each mini-task's good and bad scripts run once, with `verify.checks`,
//! and their workspaces are preserved. Then each arm starts from its own
//! copy of a preserved workspace, `.git` included:
//!
//! | Arm | Brief | Profile |
//! | --- | --- | --- |
//! | `none` | No repair | None |
//! | `fresh` | Plain: the task, no packet | The same profile |
//! | `packet-same` | The delta brief from the packets | The same profile |
//! | `packet-other` | The delta brief | Another profile |
//!
//! The trigger decides which candidates an arm repairs: `detected`
//! repairs one only when a check contradicted a requirement, as the
//! episode does; `always` repairs every candidate, so an unneeded repair
//! of a passing candidate counts as damage when it breaks it. The grader
//! runs after each arm and never feeds back.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{BriefKind, Place, Policy, Profile, Trigger};
use crate::checks::{self, Report};
use crate::deadline::Deadline;
use crate::minitask::{self, MiniTask};
use crate::record::{Finish, Implementation, Outcome, Recorder, Start};

/// The schema of a study's result.
pub const SCHEMA: &str = "openagents.coder-one.repair-study.v1";

/// How a study runs.
#[derive(Clone, Debug)]
pub struct Options {
    pub tasks: Vec<MiniTask>,
    /// The profile that stands in for the executor that produced the
    /// candidate.
    pub same: Profile,
    /// Another profile.
    pub other: Profile,
    pub trigger: Trigger,
    /// The time each repair may ask of its episode deadline.
    pub allowance: Duration,
    pub out: PathBuf,
}

/// One arm.
#[derive(Clone, Debug)]
pub struct Arm {
    pub name: &'static str,
    /// The brief and profile, or `None` for no repair.
    pub repair: Option<(BriefKind, Profile)>,
}

/// The four arms for `options`.
#[must_use]
pub fn arms(options: &Options) -> Vec<Arm> {
    vec![
        Arm {
            name: "none",
            repair: None,
        },
        Arm {
            name: "fresh",
            repair: Some((BriefKind::Plain, options.same.clone())),
        },
        Arm {
            name: "packet-same",
            repair: Some((BriefKind::Packet, options.same.clone())),
        },
        Arm {
            name: "packet-other",
            repair: Some((BriefKind::Packet, options.other.clone())),
        },
    ]
}

/// A preserved candidate: a mini-task run's workspace and its check.
#[derive(Clone, Debug)]
pub struct Preserved {
    pub task: MiniTask,
    pub variant: String,
    pub dir: PathBuf,
    pub report: Report,
    pub passed: Option<bool>,
    /// The session that produced the candidate.
    pub session: Option<String>,
}

impl Preserved {
    fn label(&self) -> String {
        format!("{}-{}", self.task.id, self.variant)
    }
}

/// Runs each task's good and bad scripts once, with checks, and keeps
/// their workspaces.
///
/// # Errors
///
/// Returns a message when a run can't be made or its check doesn't read.
pub async fn preserve(tasks: &[MiniTask], out: &Path) -> Result<Vec<Preserved>, String> {
    let mut preserved = Vec::new();
    for task in tasks {
        for variant in ["bad", "good"] {
            preserved.push(preserve_one(task, variant, out).await?);
        }
    }
    Ok(preserved)
}

/// Runs one of `task`'s scripts with checks and keeps its workspace.
///
/// # Errors
///
/// Returns a message when the run can't be made or its check doesn't read.
pub async fn preserve_one(task: &MiniTask, variant: &str, out: &Path) -> Result<Preserved, String> {
    let ran = minitask::run::run(minitask::run::Options {
        task: *task,
        executor: minitask::run::ExecutorChoice::Scripted {
            variant: variant.to_string(),
            script: None,
        },
        out: out.to_path_buf(),
        jev: None,
        speed: 0.0,
        deadline: Duration::from_secs(60),
        controls: crate::session::Controls::default(),
        checks: true,
        brief: None,
        repair: None,
        monitor: None,
    })
    .await?;
    let report: Report = serde_json::from_str(
        &std::fs::read_to_string(ran.dir.join(checks::COVERAGE_FILE)).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(Preserved {
        task: *task,
        variant: variant.to_string(),
        passed: ran.grade.reward().map(|r| r >= 1.0),
        session: ran.manifest["session"]["session_id"]
            .as_str()
            .map(str::to_string),
        dir: ran.dir,
        report,
    })
}

/// Runs one arm on an isolated copy of `preserved` under `dir` and
/// returns its row.
///
/// # Errors
///
/// Returns a message when the copy, the log, or the repair can't be made.
pub async fn cell(
    preserved: &Preserved,
    arm: &Arm,
    trigger: Trigger,
    allowance: Duration,
    dir: &Path,
) -> Result<Value, String> {
    let started = Instant::now();
    let work = dir.join("work");
    let artifacts = dir.join("artifacts");
    super::copy_tree(&preserved.dir.join("work"), &work)?;
    std::fs::create_dir_all(dir.join("verification")).map_err(|e| e.to_string())?;
    let session = atif::Session::opening(
        &format!("repair-{}-{}", preserved.label(), arm.name),
        "none",
        "repair-study",
        &work.to_string_lossy(),
        &crate::episode::version(),
    );
    let log = atif::Log::create_at(&dir.join(crate::episode::INVOCATION_LOG), &session)
        .map_err(|e| format!("cannot create the arm's log: {e}"))?;
    let recorder = Recorder::durable(log);
    let root = recorder.enter(
        Start::new(
            "episode",
            Implementation::new(
                "episode",
                &format!("repair study arm {}", arm.name),
                &json!({ "arm": arm.name, "profile": arm.repair.as_ref().map(|(k, p)| (k.word(), p.word())), "trigger": trigger }),
            ),
        )
        .named(&format!("{} · {}", preserved.label(), arm.name))
        .with_effects(),
    );
    let input = checks::workspace_input(&preserved.task, &work);
    let deadline = Deadline::new(Some(allowance), Duration::ZERO);
    let repaired = match &arm.repair {
        None => None,
        Some((kind, profile)) => {
            let subject = crate::checks::Subject::mini(&preserved.task);
            let place = Place {
                task: Some(&preserved.task),
                subject: &subject,
                work: &work,
                dir,
                artifacts: &artifacts,
                recorder: &recorder,
                deadline: &deadline,
                jev: None,
                previous_session: preserved.session.clone(),
            };
            Some(
                super::with_profile(
                    &place,
                    (&input, &preserved.report),
                    None,
                    Policy {
                        kind: *kind,
                        trigger,
                        allowance,
                    },
                    profile,
                )
                .await?,
            )
        }
    };
    let grade = minitask::grade(&preserved.task, &work, &dir.join("grader")).await;
    let after = grade.reward().map(|r| r >= 1.0);
    recorder.end(
        &root,
        Finish::new(if after == Some(true) {
            Outcome::Completed
        } else {
            Outcome::Failed
        })
        .summary(json!({ "grade": grade.verdict })),
    );
    recorder.finish(atif::log::ENDED);
    let before = preserved.passed;
    Ok(json!({
        "candidate": preserved.label(),
        "task": preserved.task.id,
        "variant": preserved.variant,
        "arm": arm.name,
        "brief": arm.repair.as_ref().map(|(k, _)| k.word()),
        "profile": arm.repair.as_ref().map(|(_, p)| p.word()),
        "detected": preserved.report.detected(),
        "before": before,
        "after": after,
        "grade": grade,
        "triggered": repaired.as_ref().is_some_and(|r| r.ran),
        "changed": repaired.as_ref().is_some_and(|r| r.changed),
        "recovered": before == Some(false) && after == Some(true),
        "damaged": before == Some(true) && after == Some(false),
        "cost_usd": repaired.as_ref().map_or(Some(0.0), |r| r.cost_usd),
        "repair": repaired.as_ref().map(|r| r.record.clone()),
        "milliseconds": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "dir": dir.display().to_string(),
    }))
}

/// Recovery and damage per arm, with every dispatch's cost.
#[must_use]
pub fn totals(rows: &[Value], arms: &[Arm]) -> Value {
    json!(
        arms.iter()
            .map(|arm| {
                let mine: Vec<&Value> = rows.iter().filter(|r| r["arm"] == arm.name).collect();
                let failed = mine.iter().filter(|r| r["before"] == json!(false)).count();
                let passing = mine.iter().filter(|r| r["before"] == json!(true)).count();
                let recovered = mine.iter().filter(|r| r["recovered"] == json!(true)).count();
                let damaged = mine.iter().filter(|r| r["damaged"] == json!(true)).count();
                let triggered = mine.iter().filter(|r| r["triggered"] == json!(true)).count();
                let cost: Option<f64> = mine
                    .iter()
                    .map(|r| r["cost_usd"].as_f64())
                    .sum::<Option<f64>>()
                    .map(|usd| usd + 0.0);
                let ms: u64 = mine.iter().filter_map(|r| r["milliseconds"].as_u64()).sum();
                let rate = |n: usize, of: usize| {
                    if of == 0 {
                        Value::Null
                    } else {
                        json!((n as f64 / of as f64 * 1000.0).round() / 1000.0)
                    }
                };
                json!({
                    "arm": arm.name,
                    "brief": arm.repair.as_ref().map(|(k, _)| k.word()),
                    "profile": arm.repair.as_ref().map(|(_, p)| p.word()),
                    "failed_candidates": failed,
                    "recovered": recovered,
                    "recovery_rate": rate(recovered, failed),
                    "passing_candidates": passing,
                    "damaged": damaged,
                    "damage_rate": rate(damaged, passing),
                    "repairs_run": triggered,
                    "cost_usd": cost,
                    "cost_per_repair_usd": cost.and_then(|c| (triggered > 0).then(|| c / triggered as f64)),
                    "milliseconds": ms,
                })
            })
            .collect::<Vec<_>>()
    )
}

/// Runs the whole study and writes `<out>/study.json`.
///
/// # Errors
///
/// Returns a message when a candidate or an arm can't run.
pub async fn run(options: &Options) -> Result<Value, String> {
    let started = Instant::now();
    let at = atif::now_ms();
    let preserved = preserve(&options.tasks, &options.out.join("preserved")).await?;
    let arms = arms(options);
    let mut rows = Vec::new();
    for candidate in &preserved {
        for arm in &arms {
            let dir = options
                .out
                .join("arms")
                .join(candidate.label())
                .join(arm.name);
            rows.push(cell(candidate, arm, options.trigger, options.allowance, &dir).await?);
        }
    }
    let study = json!({
        "schema": SCHEMA,
        "kind": "conditional recovery",
        "executor": "scripted",
        "trigger": options.trigger,
        "same": options.same.word(),
        "other": options.other.word(),
        "allowance_sec": options.allowance.as_secs(),
        "implementation": super::implementation(BriefKind::Packet, options.trigger),
        "candidates": preserved.iter().map(|p| json!({
            "candidate": p.label(),
            "passed": p.passed,
            "detected": p.report.detected(),
            "packets": p.report.packets.len(),
            "dir": p.dir.display().to_string(),
        })).collect::<Vec<_>>(),
        "arms": totals(&rows, &arms),
        "rows": rows,
        "started_at": atif::document::iso(at),
        "milliseconds": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    });
    crate::record::write_atomic(
        &options.out.join("study.json"),
        serde_json::to_string_pretty(&study)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )?;
    Ok(study)
}

/// Where studies are written: `~/.openagents/coder-one/repair`.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/repair"))
}
