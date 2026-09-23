//! One episode on a mini-task, recorded as a mini-task run.
//!
//! The run lays out a directory the Gym reads:
//!
//! ```text
//! <runs>/minitask-<task>-<executor>-<ms>/
//!   manifest.json               kind "mini-task": task, executor, outcome, grade
//!   episode.atif.jsonl          every step and invocation, synced as it happens
//!   work/                       the scratch directory the task runs in
//!   artifacts/                  the briefing and the executor's native stream
//!   verification/grade.json     the mini-task grader's verdict
//!   verification/checks.json    verify.checks' coverage, when checks ran
//!   verification/support.json   verify.support's requirement states, with Jev
//! ```
//!
//! The episode runs the same explore-then-delegate path a Terminal-Bench
//! episode runs, with no explore steps, so the briefing packer, the
//! executor session, and the closing check compose as they do there. The
//! grader runs last and is never shown to the episode.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use atif::document::{Source, Step};
use serde_json::{Value, json};

use crate::delegate::{self, Agent, Cli, Executor, Mode, Plan, Policy};
use crate::judge::JevJudge;
use crate::record::{Finish, Implementation, Outcome, Recorder, Start};
use crate::scripted::{Script, Scripted};
use crate::session::{self, Controls};
use crate::shell::Checkout;
use crate::state::{Environment, Issue, State};
use crate::{Ended, Generate};

use super::{Grade, MiniTask, RUN_SCHEMA, grade, scripts, setup};

/// Who runs the mini-task.
#[derive(Clone, Debug)]
pub enum ExecutorChoice {
    /// The scripted executor with the task's `good` or `bad` script, or a
    /// script of the caller's.
    Scripted {
        variant: String,
        script: Option<Script>,
    },
    /// Claude Code or Codex, inside a `coder-boundary` filesystem boundary
    /// that lets it write only the run's directories and its own state.
    Cli { agent: Agent, model: String },
}

impl ExecutorChoice {
    /// The choice's label in a run's directory name and manifest.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            ExecutorChoice::Scripted { variant, .. } => format!("scripted-{variant}"),
            ExecutorChoice::Cli { agent, model } => format!("{}-{model}", agent.word()),
        }
    }
}

/// How one mini-task run is set up.
pub struct Options {
    pub task: MiniTask,
    pub executor: ExecutorChoice,
    /// The directory runs are recorded under.
    pub out: PathBuf,
    /// The Jev client for the closing check; `None` skips it.
    pub jev: Option<jev::Client>,
    /// Real milliseconds per scripted millisecond; 0 runs on virtual time.
    pub speed: f64,
    /// The executor's deadline.
    pub deadline: Duration,
    /// Session control during the scripted executor's run.
    pub controls: Controls,
    /// Whether `verify.checks` observes the workspace before the grader.
    pub checks: bool,
    /// The briefing policy under test; `None` runs the built-in one. A
    /// study screens a candidate's briefing policy this way.
    pub brief: Option<crate::policy::BriefPolicy>,
    /// A `control.monitor` in shadow mode over the executor's session:
    /// the rules, and Jev when `jev` is given.
    pub monitor: Option<crate::monitor::Params>,
}

/// What a run left.
#[derive(Clone, Debug)]
pub struct Ran {
    pub dir: PathBuf,
    pub manifest: Value,
    pub grade: Grade,
    pub milliseconds: u64,
}

/// A generator for a run that never generates: the explore phase has no
/// steps, so the loop never asks.
struct NoGenerator;

impl Generate for NoGenerator {
    async fn generate(&mut self, _prompt: &str) -> Result<String, String> {
        Err("a mini-task episode runs no explore steps".to_string())
    }
}

/// The CLI executor inside a filesystem boundary.
struct Bounded {
    cli: Cli,
    boundary: coder_boundary::Boundary,
}

impl Executor for Bounded {
    fn agent(&self) -> &str {
        self.cli.agent()
    }
    fn cost_provenance(&self) -> &'static str {
        self.cli.cost_provenance()
    }
    fn model(&self) -> &str {
        self.cli.model()
    }
    fn deadline(&self) -> Duration {
        self.cli.deadline()
    }
    fn describe(&self) -> serde_json::Map<String, Value> {
        let mut extra = self.cli.describe();
        extra.insert(
            "boundary".to_string(),
            json!({
                "backend": self.boundary.backend().display().to_string(),
                "writable": self.boundary.writable().iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                "checkout": self.boundary.checkout().map(|p| p.display().to_string()),
            }),
        );
        let (capabilities, note) = session::cli_capabilities(self.cli.agent);
        extra.insert(
            "capabilities".to_string(),
            capabilities.record(self.cli.agent.word(), note),
        );
        extra
    }
    fn system_options(&self) -> Vec<String> {
        self.cli.system_options()
    }
    fn select_system(&mut self, answers: Vec<(String, Option<f64>)>) {
        self.cli.select_system(answers);
    }
    async fn execute(&mut self, briefing: &delegate::Briefing) -> delegate::Report {
        let boundary = &self.boundary;
        let wrap = |command: std::process::Command| wrap(boundary, &command);
        self.cli.execute_wrapped(briefing, &wrap).await
    }
}

/// `command` rebuilt inside `boundary`: the same program, arguments,
/// working directory, and environment changes.
fn wrap(
    boundary: &coder_boundary::Boundary,
    command: &std::process::Command,
) -> Result<std::process::Command, String> {
    let program = Path::new(command.get_program());
    let program = if program.is_absolute() {
        program.to_path_buf()
    } else {
        super::process::which(&program.to_string_lossy())
            .ok_or_else(|| format!("{} is not on PATH", program.display()))?
    };
    let mut wrapped = boundary
        .command(&program, command.get_args())
        .map_err(|error| error.to_string())?;
    if let Some(dir) = command.get_current_dir() {
        wrapped.current_dir(dir);
    }
    for (name, value) in command.get_envs() {
        match value {
            Some(value) => wrapped.env(name, value),
            None => wrapped.env_remove(name),
        };
    }
    Ok(wrapped)
}

fn millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Runs one mini-task episode and records it under `options.out`.
///
/// # Errors
///
/// Returns a message when the run's directory, its setup, or the chosen
/// executor can't be made. A failed episode is a run with a failing
/// grade, not an error.
pub async fn run(options: Options) -> Result<Ran, String> {
    let started = Instant::now();
    let task = options.task;
    let at = atif::now_ms();
    let label = options.executor.label();
    let dir = options
        .out
        .join(format!("minitask-{}-{label}-{at}", task.id));
    let work = dir.join("work");
    let artifacts = dir.join("artifacts");
    for sub in [&work, &artifacts, &dir.join("verification")] {
        std::fs::create_dir_all(sub)
            .map_err(|error| format!("cannot create {}: {error}", sub.display()))?;
    }
    let id = format!("minitask-{}-{at}", task.id);
    let mut session = atif::Session::opening(
        &id,
        "none",
        "mini-task",
        &work.to_string_lossy(),
        &crate::episode::version(),
    );
    session.directive = "Complete this task.".to_string();
    let log_path = dir.join(crate::episode::INVOCATION_LOG);
    let log = atif::Log::create_at(&log_path, &session)
        .map_err(|error| format!("cannot create {}: {error}", log_path.display()))?;
    let recorder = Recorder::durable(log);

    let (script, script_record) = match &options.executor {
        ExecutorChoice::Scripted { variant, script } => {
            let script = match script {
                Some(script) => script.clone(),
                None => scripts(&task)
                    .into_iter()
                    .find(|(name, _)| name == variant)
                    .map(|(_, script)| script)
                    .ok_or_else(|| format!("{} has no {variant} script", task.id))?,
            };
            let record = json!({ "name": script.name, "digest": script.digest() });
            (Some(script), record)
        }
        ExecutorChoice::Cli { .. } => (None, Value::Null),
    };
    let executor_record = json!({
        "kind": if script.is_some() { "scripted" } else { "cli" },
        "label": label,
        "script": script_record,
    });
    let episode = recorder.enter(
        Start::new(
            "episode",
            Implementation::new(
                "episode",
                &format!("mini-task {}", task.id),
                &json!({ "task": task.id, "instruction": task.instruction, "executor": executor_record }),
            ),
        )
        .named("mini-task")
        .reading(&json!({ "instruction": task.instruction }))
        .with_effects(),
    );
    recorder.push(Step::said(
        Source::System,
        &format!("Mini-task {} ({}).", task.id, task.family),
    ));
    recorder.push(Step::said(Source::User, task.instruction));

    let prepare = recorder.enter(
        Start::new(
            "task.setup",
            Implementation::new("task.setup", "mini-task setup", &json!({ "task": task.id })),
        )
        .named(task.id)
        .with_effects(),
    );
    let prepared = setup(&task, &work);
    recorder.end(
        &prepare,
        Finish::new(if prepared.is_ok() {
            Outcome::Completed
        } else {
            Outcome::Failed
        })
        .summary(json!({ "error": prepared.as_ref().err() })),
    );
    prepared?;
    let base = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&work)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string());

    let mut state = State::new(
        Environment {
            repository: String::new(),
            workdir: work.to_string_lossy().into_owned(),
            os: std::env::consts::OS.to_string(),
        },
        Issue {
            url: String::new(),
            title: format!("mini-task {}", task.id),
            body: task.instruction.to_string(),
            labels: Vec::new(),
        },
    );
    let mut judge = JevJudge::new(
        options.jev.clone(),
        work.clone(),
        &state.issue,
        recorder.clone(),
    );
    let mut generator = NoGenerator;
    let mut shell = Checkout {
        workdir: work.clone(),
        deadline: Duration::from_secs(60),
        recorder: recorder.clone(),
        commands: 0,
        episode: crate::deadline::Deadline::unbounded(),
    };
    let brief = options
        .brief
        .clone()
        .unwrap_or_else(|| crate::policy::Manifest::builtin().policy.brief);
    let directions = brief.directions.text();
    let plan = Plan {
        mode: Mode::Always,
        policy: Policy {
            explore_steps: 0,
            ..Policy::default()
        },
        max_steps: 0,
        prompt: "Complete this task.",
        instruction: task.instruction,
        directions,
        cap: brief.cap,
        packer: brief.packer,
        pack: brief.pack_params(),
        isolation: if script.is_some() {
            "a scratch directory"
        } else {
            "coder-boundary: writes confined to the run's directories"
        },
        base: base.as_deref(),
    };
    let mut checkpoint = |_: &State| {};
    let monitor = options.monitor.clone().map(|params| crate::monitor::Setup {
        params,
        jev: options.jev.clone().map_or(
            crate::component::jev::JevMode::Off,
            crate::component::jev::JevMode::Live,
        ),
        task: task.instruction.to_string(),
    });
    let (ended, delegated, session_record) = match (&options.executor, script) {
        (ExecutorChoice::Scripted { .. }, Some(script)) => {
            let mut executor = Scripted::new(script, work.clone());
            executor.artifacts = Some(artifacts.clone());
            executor.deadline = options.deadline;
            executor.speed = options.speed;
            executor.controls = options.controls.clone();
            executor.monitor = monitor.clone();
            executor.recorder = recorder.clone();
            let (ended, delegated) = delegate::explore_then_delegate(
                &mut state,
                &plan,
                &mut judge,
                &mut generator,
                &mut shell,
                &mut executor,
                &recorder,
                &mut checkpoint,
            )
            .await;
            (
                ended,
                delegated,
                executor.last.clone().unwrap_or(Value::Null),
            )
        }
        (ExecutorChoice::Cli { agent, model }, _) => {
            let env = |name: &str| {
                std::env::var(name)
                    .ok()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
            };
            let (binary, credential) = delegate::resolve(*agent, env);
            let home = std::env::var_os("HOME").map(PathBuf::from);
            let mut spec = coder_boundary::Boundary::writing(&work).writable(&artifacts);
            // The CLI keeps its own session state and temporary files.
            for state_dir in [
                home.as_ref().map(|home| home.join(".claude")),
                home.as_ref().map(|home| home.join(".codex")),
                home.as_ref().map(|home| home.join(".cache")),
                Some(std::env::temp_dir()),
            ]
            .into_iter()
            .flatten()
            .filter(|path| path.is_dir())
            {
                spec = spec.writable(state_dir);
            }
            let boundary = spec
                .build()
                .map_err(|error| format!("cannot bound the executor: {error}"))?;
            let mut executor = Bounded {
                cli: Cli {
                    agent: *agent,
                    binary,
                    model: model.clone(),
                    deadline: options.deadline,
                    workdir: work.clone(),
                    artifacts: artifacts.clone(),
                    artifacts_label: "artifacts".to_string(),
                    env: Vec::new(),
                    credential,
                    effort: None,
                    tools: None,
                    prompt_cache_ttl: None,
                    system: None,
                    runs: 0,
                    episode: crate::deadline::Deadline::unbounded(),
                    gate: None,
                    granted: None,
                    control: delegate::Control {
                        recorder: Some(recorder.clone()),
                        controls: Some(options.controls.clone()),
                        last: None,
                        monitor: monitor.clone(),
                    },
                },
                boundary,
            };
            let (ended, delegated) = delegate::explore_then_delegate(
                &mut state,
                &plan,
                &mut judge,
                &mut generator,
                &mut shell,
                &mut executor,
                &recorder,
                &mut checkpoint,
            )
            .await;
            let record = executor.cli.control.last.clone().unwrap_or(Value::Null);
            (ended, delegated, record)
        }
        (ExecutorChoice::Scripted { .. }, None) => unreachable!("a scripted choice has a script"),
    };

    let checked = if options.checks {
        Some(
            crate::checks::check_workspace_as(
                &task,
                &work,
                &dir,
                &recorder,
                crate::checks::COVERAGE_FILE,
            )
            .await,
        )
    } else {
        None
    };
    // `verify.support` needs Jev: without a client it would leave every
    // requirement unresolved, so it doesn't run.
    let support = match (&checked, &options.jev) {
        (Some((input, report)), Some(client)) => {
            let judged = crate::support::judge(
                input,
                report,
                &crate::component::jev::JevMode::Live(client.clone()),
                &recorder,
                crate::support::Params::default(),
                None,
            )
            .await;
            crate::support::save(&judged, &dir)?;
            Some(judged)
        }
        _ => None,
    };
    let checks = checked.as_ref().map(|(_, report)| report);

    let grading = recorder.enter(
        Start::new(
            "task.grade",
            Implementation::new(
                "task.grade",
                "mini-task grader",
                &json!({ "task": task.id }),
            ),
        )
        .named(task.id)
        .with_effects(),
    );
    let grade = grade(&task, &work, &dir.join("grader")).await;
    recorder.end(
        &grading,
        Finish::new(match grade.verdict.as_str() {
            "passed" => Outcome::Completed,
            "failed" => Outcome::Failed,
            _ => Outcome::Skipped,
        })
        .output(json!({ "verdict": grade.verdict, "detail": grade.detail })),
    );
    let outcome = match &ended {
        Ended::Delegated { answered: true, .. } => "delegated",
        Ended::Delegated { .. } => "delegate_failed",
        Ended::Finished { .. } => "finished",
        Ended::StepLimit { .. } | Ended::Stopped { .. } => "step_limit",
        Ended::GenerationFailed { .. } => "generation_failed",
    };
    recorder.end(
        &episode,
        Finish::new(if grade.verdict == "passed" {
            Outcome::Completed
        } else {
            Outcome::Failed
        })
        .summary(json!({ "outcome": outcome, "grade": grade.verdict })),
    );
    recorder.finish(atif::log::ENDED);

    let milliseconds = millis(started);
    let grade_path = dir.join("verification/grade.json");
    crate::record::write_atomic(
        &grade_path,
        serde_json::to_string_pretty(
            &json!({ "task": task.id, "grade": grade, "reward": grade.reward() }),
        )
        .map_err(|error| error.to_string())?
        .as_bytes(),
    )?;
    let manifest = json!({
        "schema": RUN_SCHEMA,
        "kind": "mini-task",
        "id": id,
        "task": {
            "id": task.id,
            "family": task.family,
            "instruction_digest": atif::digest(&json!(task.instruction)),
        },
        "executor": executor_record,
        "brief": brief,
        "session": session_record,
        "delegation": delegated.as_ref().map(delegate::Delegated::record),
        "outcome": outcome,
        "grade": grade,
        "reward": grade.reward(),
        "checks": checks.map(crate::checks::Report::summary),
        "support": support.as_ref().map(crate::support::Report::summary),
        "started_at": atif::document::iso(at),
        "milliseconds": milliseconds,
        "version": crate::episode::version(),
        "files": {
            "invocation_log": crate::episode::INVOCATION_LOG,
            "grade": "verification/grade.json",
            "checks": checks.map(|_| crate::checks::COVERAGE_FILE),
            "support": support.as_ref().map(|_| crate::support::FILE),
            "workdir": "work",
            "artifacts": "artifacts",
        },
    });
    crate::record::write_atomic(
        &dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)
            .map_err(|error| error.to_string())?
            .as_bytes(),
    )?;
    Ok(Ran {
        dir,
        manifest,
        grade,
        milliseconds,
    })
}

/// Where mini-task runs are recorded: `~/.openagents/coder-one/minitasks`.
#[must_use]
pub fn default_runs_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/minitasks"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minitask::{CATALOG, find};

    fn out(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "coder-one-minitask-run-{label}-{}-{}",
            std::process::id(),
            atif::now_ms()
        ))
    }

    fn options(task: &str, variant: &str, out: &Path) -> Options {
        Options {
            task: find(task).unwrap(),
            executor: ExecutorChoice::Scripted {
                variant: variant.to_string(),
                script: None,
            },
            out: out.to_path_buf(),
            jev: None,
            speed: 0.0,
            deadline: Duration::from_secs(60),
            controls: Controls::default(),
            checks: false,
            brief: None,
            monitor: None,
        }
    }

    #[tokio::test]
    async fn a_running_episode_streams_its_executor_events_to_the_log_before_it_ends() {
        let out = out("live");
        let script = Script::parse(
            &json!({
                "schema": crate::scripted::SCRIPT_SCHEMA,
                "name": "slow",
                "events": [
                    { "at_ms": 0, "do": "claim", "text": "Reading the logs." },
                    { "at_ms": 150, "do": "command", "command": "ls logs", "output": "a.log" },
                    { "at_ms": 300, "do": "claim", "text": "Writing the summary." },
                    { "at_ms": 900, "do": "end" }
                ]
            })
            .to_string(),
        )
        .unwrap();
        let mut options = options("log-severity", "slow", &out);
        options.executor = ExecutorChoice::Scripted {
            variant: "slow".to_string(),
            script: Some(script),
        };
        options.speed = 1.0;
        // A reader on the host follows the log while the run goes on, as
        // `gym coder live` does: the log grows, and the manifest is absent.
        let watch = async {
            let mut seen_while_running = 0usize;
            for _ in 0..200 {
                tokio::time::sleep(Duration::from_millis(25)).await;
                let Some(dir) = std::fs::read_dir(&out)
                    .ok()
                    .and_then(|mut entries| entries.next())
                    .and_then(Result::ok)
                    .map(|entry| entry.path())
                else {
                    continue;
                };
                if dir.join("manifest.json").exists() {
                    break;
                }
                let log = std::fs::read_to_string(dir.join(crate::episode::INVOCATION_LOG))
                    .unwrap_or_default();
                seen_while_running =
                    seen_while_running.max(log.matches("\"executor_event\"").count());
            }
            seen_while_running
        };
        let (ran, seen) = tokio::join!(run(options), watch);
        let ran = ran.unwrap();
        assert!(
            seen >= 2,
            "a reader saw {seen} executor events before the run ended"
        );
        let log = atif::log::read_whole(&ran.dir.join(crate::episode::INVOCATION_LOG)).unwrap();
        let total = log
            .steps
            .iter()
            .filter(|step| step.extensions.contains_key(crate::session::EVENT_KEY))
            .count();
        assert!(total > seen, "{total} events in all, {seen} while running");
        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn a_scripted_episode_on_every_mini_task_grades_good_and_bad_apart_in_seconds() {
        let out = out("all");
        let python = crate::minitask::process::python().is_some();
        for task in CATALOG {
            for variant in ["good", "bad"] {
                let ran = run(options(task.id, variant, &out)).await.unwrap();
                assert!(
                    ran.milliseconds < 10_000,
                    "{} {variant} took {} ms",
                    task.id,
                    ran.milliseconds
                );
                let needs_python = matches!(task.id, "interactive-terminal" | "cancel-cleanup");
                if needs_python && !python {
                    assert_eq!(ran.grade.verdict, "unavailable");
                    continue;
                }
                let want = if variant == "good" {
                    "passed"
                } else {
                    "failed"
                };
                assert_eq!(
                    ran.grade.verdict, want,
                    "{} {variant}: {}",
                    task.id, ran.grade.detail
                );
                assert_eq!(ran.manifest["kind"], json!("mini-task"));
                assert_eq!(ran.manifest["outcome"], json!("delegated"));
                let log =
                    atif::log::read_whole(&ran.dir.join(crate::episode::INVOCATION_LOG)).unwrap();
                let components: Vec<String> = crate::record::invocations(&log.steps)
                    .into_iter()
                    .map(|i| i.component)
                    .collect();
                for component in [
                    "episode",
                    "task.setup",
                    "evidence.pack",
                    "exec.session",
                    "exec.control",
                    "verify.close",
                    "task.grade",
                ] {
                    assert!(
                        components.iter().any(|c| c == component),
                        "{} lacks {component}: {components:?}",
                        task.id
                    );
                }
            }
        }
        let _ = std::fs::remove_dir_all(out);
    }

    #[tokio::test]
    async fn checks_run_before_the_grader_and_leave_coverage_the_gym_reads() {
        if crate::minitask::process::python().is_none() {
            return;
        }
        let out = out("checks");
        let mut options = options("log-severity", "bad", &out);
        options.checks = true;
        let ran = run(options).await.unwrap();
        assert_eq!(ran.grade.verdict, "failed");
        assert_eq!(
            ran.manifest["files"]["checks"],
            json!(crate::checks::COVERAGE_FILE)
        );
        assert_eq!(ran.manifest["checks"]["verdicts"]["failed"], json!(1));
        let report: crate::checks::Report = serde_json::from_str(
            &std::fs::read_to_string(ran.dir.join(crate::checks::COVERAGE_FILE)).unwrap(),
        )
        .unwrap();
        assert!(
            report
                .packets
                .iter()
                .any(|p| p.scenario == "data.message-severity")
        );
        let log = atif::log::read_whole(&ran.dir.join(crate::episode::INVOCATION_LOG)).unwrap();
        let order: Vec<String> = crate::record::invocations(&log.steps)
            .into_iter()
            .map(|i| i.component)
            .filter(|c| c == "verify.checks" || c == "task.grade")
            .collect();
        assert_eq!(order, ["verify.checks", "task.grade"]);
        let _ = std::fs::remove_dir_all(out);
    }
}
