//! Terminal-Bench runs in plain words: what each run was, whether it
//! worked, and what it cost.
//!
//! The other Terminal-Bench views are written for the people who build
//! the agents: components, invocations, digests, pins. This module is the
//! reader for everyone else. It finds every trial Harbor has started under
//! the jobs directory — finished, still running, or stopped before a
//! grade — and every trial retained in the checkout, and reads each into a
//! [`Run`]: the task, the agent, the outcome in words, the tests the
//! verifier passed, the cost, and the time.
//!
//! [`crate::runs_story`] turns one run's records into a short narrative,
//! and [`crate::runs_transcript`] into the transcript the terminal pane
//! draws. `gym runs` and `gym runs show <job>` print both as text.
//!
//! The reader only reads. It never writes into a job directory, and it
//! reads a finished run once: [`Catalog::refresh`] reads again only the
//! runs that were still going and the ones that are new.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::runs_transcript::model_name;
use crate::terminal_bench::timestamp_ms;

/// A run with no result and no activity for this long has stopped: the
/// harness that ran it is gone.
pub const ABANDONED_AFTER_MS: i64 = 6 * 60 * 60 * 1000;

/// Where runs are read from.
#[derive(Clone, Debug, Default)]
pub struct Sources {
    /// Harbor's jobs directory, `~/.openagents/terminal-bench/jobs`.
    pub jobs: Option<PathBuf>,
    /// The trials retained in the checkout, `bench/terminal-bench/traces`.
    pub traces: Option<PathBuf>,
    /// Where the Terminal-Bench task definitions are, to read a task's
    /// instruction when a run's own record does not name its path.
    pub tasks: Vec<PathBuf>,
    /// The startup index's directory (issue #9595): finished runs whose
    /// files are unchanged are read from it instead of parsed. `None`, as
    /// in [`Sources::standard`], parses every run; `gym-terminal` sets it
    /// to [`crate::index::default_dir`].
    pub index: Option<PathBuf>,
}

impl Sources {
    /// The operator's jobs directory and this checkout's retained traces.
    #[must_use]
    pub fn standard() -> Self {
        let home = std::env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .map(PathBuf::from);
        let upstream = home
            .as_ref()
            .map(|home| home.join(".openagents/terminal-bench/upstream"));
        let tasks = upstream
            .map(|root| {
                [
                    "terminal-bench-v4.0.0/tasks",
                    "terminal-bench/tasks",
                    "terminal-bench/archive",
                ]
                .iter()
                .map(|tail| root.join(tail))
                .filter(|path| path.is_dir())
                .collect()
            })
            .unwrap_or_default();
        Sources {
            jobs: home.map(|home| home.join(".openagents/terminal-bench/jobs")),
            traces: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces"),
            ),
            tasks,
            index: None,
        }
    }
}

/// Which agent ran.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Agent {
    CoderOne,
    ClaudeCode,
    Codex,
    /// The older Coder, v0.5.
    CoderV05,
    /// Harbor's oracle: the task's reference solution, a control.
    Reference,
    /// Harbor's `nop` agent, which does nothing: a control.
    Control,
    /// Anything else.
    Other,
}

impl Agent {
    /// Every agent, in the order the filter cycles through them.
    pub const ALL: [Agent; 7] = [
        Agent::CoderOne,
        Agent::ClaudeCode,
        Agent::Codex,
        Agent::CoderV05,
        Agent::Reference,
        Agent::Control,
        Agent::Other,
    ];

    /// The agent's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Agent::CoderOne => "Coder One",
            Agent::ClaudeCode => "Claude Code",
            Agent::Codex => "Codex",
            Agent::CoderV05 => "Coder v0.5",
            Agent::Reference => "Reference solution",
            Agent::Control => "No agent (control)",
            Agent::Other => "Other",
        }
    }

    fn from_names(names: &[&str]) -> Self {
        let has = |needle: &str| names.iter().any(|name| name.contains(needle));
        if has("MatchedPlain") {
            // The matched experiment's plain arm runs Claude Code directly.
            Agent::ClaudeCode
        } else if has("coder_one") || has("coder-one") {
            Agent::CoderOne
        } else if has("coder_v05") || has("coder-v05") {
            Agent::CoderV05
        } else if has("claude-code") || has("claude_code") {
            Agent::ClaudeCode
        } else if has("codex") {
            Agent::Codex
        } else if has("oracle") {
            Agent::Reference
        } else if names.contains(&"nop") {
            Agent::Control
        } else {
            Agent::Other
        }
    }
}

/// How a run came out.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Outcome {
    /// The verifier gave full marks.
    Passed,
    /// The verifier graded it and it fell short.
    Failed,
    /// It has not finished.
    Running,
    /// It ended without a grade that says anything about the agent. The
    /// text says why, in plain words.
    NotGraded(String),
}

impl Outcome {
    /// The outcome's word.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Outcome::Passed => "passed",
            Outcome::Failed => "failed",
            Outcome::Running => "running",
            Outcome::NotGraded(_) => "not graded",
        }
    }

    /// The mark the list draws beside the word.
    #[must_use]
    pub fn mark(&self) -> char {
        match self {
            Outcome::Passed => '✓',
            Outcome::Failed => '✗',
            Outcome::Running => '●',
            Outcome::NotGraded(_) => '○',
        }
    }

    /// The four outcomes, in the order the filter cycles through them.
    #[must_use]
    pub fn kinds() -> [&'static str; 4] {
        ["passed", "failed", "running", "not graded"]
    }
}

/// The verifier's test counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tests {
    pub passed: u64,
    pub failed: u64,
    pub total: u64,
}

/// Where a run's records are. Every path is optional: an older run, a
/// retained run, and a running one each have some of them.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Files {
    /// The trial directory, or the retained `<trial>.episode` directory.
    pub dir: PathBuf,
    pub result: Option<PathBuf>,
    pub config: Option<PathBuf>,
    /// Coder One's episode bundle.
    pub episode: Option<PathBuf>,
    /// The live copy of Coder One's episode log while the trial runs.
    pub live: Option<PathBuf>,
    /// Harbor's ATIF trajectory, or Coder One's older export.
    pub trajectory: Option<PathBuf>,
    /// Harbor's native Claude Code or Codex output.
    pub native: Option<PathBuf>,
    pub verifier: Option<PathBuf>,
    /// The harness's attempt record.
    pub attempt: Option<PathBuf>,
}

/// One trial, read.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Run {
    pub job: String,
    pub trial: String,
    /// The batch the job belongs to: `tb4`, `panel`, `extended`, and so on.
    pub batch: String,
    /// Whether the run comes from the checkout's retained traces.
    pub retained: bool,
    pub files: Files,
    /// The task's short name, such as `coq-block-bound`.
    pub task: String,
    pub task_path: Option<PathBuf>,
    /// What the task asks for, in a sentence or two.
    pub ask: Option<String>,
    /// The task's category and subcategory, when the task says.
    pub category: Option<String>,
    /// How long the task's author thinks an expert needs, in hours.
    pub expert_hours: Option<f64>,
    /// The agent's time limit, in seconds.
    pub time_limit_sec: Option<f64>,
    pub agent: Agent,
    /// The agent's variant, such as `tunable-v6`, for Coder One.
    pub variant: Option<String>,
    /// The model the agent ran, for Claude Code and Codex.
    pub model: Option<String>,
    pub started_ms: Option<i64>,
    pub ended_ms: Option<i64>,
    /// The agent's own working time.
    pub agent_ms: Option<u64>,
    /// When a running run last wrote anything.
    pub active_ms: Option<i64>,
    pub outcome: Outcome,
    pub reward: Option<f64>,
    pub tests: Option<Tests>,
    pub cost_usd: Option<f64>,
    /// Whether the cost is a list-price estimate rather than a reported
    /// charge.
    pub cost_estimated: bool,
    /// Short facts about how it ended: a timeout, a crash, a limit.
    pub notes: Vec<String>,
}

impl Run {
    /// `job/trial`, the run's unique name.
    #[must_use]
    pub fn id(&self) -> String {
        format!("{}/{}", self.job, self.trial)
    }

    /// `Coder One · tunable-v6`, `Claude Code · Opus 5.5`.
    #[must_use]
    pub fn agent_label(&self) -> String {
        match (&self.variant, &self.model) {
            (Some(variant), _) => format!("{} · {variant}", self.agent.name()),
            (None, Some(model)) => format!("{} · {}", self.agent.name(), model_name(model)),
            (None, None) => self.agent.name().to_owned(),
        }
    }

    /// The run's whole length: finished runs from start to end, running
    /// ones until `now`.
    #[must_use]
    pub fn elapsed_ms(&self, now: i64) -> Option<u64> {
        let end = match self.outcome {
            Outcome::Running => Some(now),
            _ => self.ended_ms,
        }?;
        u64::try_from(end - self.started_ms?).ok()
    }

    /// Whether `query` names this run: its task, what the task asks, its
    /// agent, its batch, or its job.
    #[must_use]
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }
        let haystack = format!(
            "{} {} {} {} {} {}",
            self.task,
            self.ask.as_deref().unwrap_or_default(),
            self.agent_label(),
            self.batch,
            self.job,
            self.outcome.word()
        )
        .to_lowercase();
        query.split_whitespace().all(|word| haystack.contains(word))
    }
}

/// What the task's own files say about it.
#[derive(Clone, Debug, Default)]
struct TaskInfo {
    ask: Option<String>,
    category: Option<String>,
    expert_hours: Option<f64>,
    time_limit_sec: Option<f64>,
}

/// Every run, newest first, kept between reads.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub sources: Sources,
    pub runs: Vec<Run>,
    pub errors: Vec<String>,
    tasks: HashMap<PathBuf, TaskInfo>,
    /// Finished runs as last parsed, kept between loads.
    index: Option<crate::index::Index<Run>>,
}

impl Catalog {
    /// Reads every run under `sources`.
    #[must_use]
    pub fn load(sources: Sources) -> Self {
        let scope = format!(
            "{:?} {:?} {:?}",
            sources.jobs, sources.traces, sources.tasks
        );
        let index = crate::index::Index::open(sources.index.as_deref(), "runs", &scope);
        let mut catalog = Catalog {
            sources,
            index: Some(index),
            ..Catalog::default()
        };
        catalog.refresh(now_ms());
        catalog
    }

    /// Reads again what may have changed: runs that were still going, and
    /// runs that are new. A finished run keeps what was read.
    pub fn refresh(&mut self, now: i64) {
        let mut kept: HashMap<String, Run> = std::mem::take(&mut self.runs)
            .into_iter()
            .filter(|run| run.outcome != Outcome::Running)
            .map(|run| (run.id(), run))
            .collect();
        let mut index = self.index.take().unwrap_or_default();
        // A run still going is parsed at every read and never kept.
        let finished = |run: &Run| run.outcome != Outcome::Running;
        let mut runs = Vec::new();
        let mut seen = std::collections::HashSet::new();
        if let Some(jobs) = self.sources.jobs.clone() {
            for job in subdirs(&jobs) {
                let job_name = file_name(&job);
                for trial in subdirs(&job) {
                    if !trial.join("config.json").is_file() {
                        continue;
                    }
                    let id = format!("{job_name}/{}", file_name(&trial));
                    seen.insert(id.clone());
                    let key = format!("local {id}");
                    match kept.remove(&id) {
                        Some(run) => {
                            index.keep(&key);
                            runs.push(run);
                        }
                        None => runs.push(index.get_or_read(
                            &key,
                            || self.read_local(&job, &trial, now),
                            finished,
                        )),
                    }
                }
            }
        }
        if let Some(traces) = self.sources.traces.clone() {
            for job in subdirs(&traces) {
                let job_name = file_name(&job);
                let mut trials: Vec<String> = std::fs::read_dir(&job)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|entry| {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        name.strip_suffix(".episode")
                            .or_else(|| name.strip_suffix(".json"))
                            .map(str::to_owned)
                    })
                    .collect();
                trials.sort();
                trials.dedup();
                for trial in trials {
                    let id = format!("{job_name}/{trial}");
                    if !seen.insert(id.clone()) {
                        continue;
                    }
                    let key = format!("retained {id}");
                    match kept.remove(&id) {
                        Some(run) => {
                            index.keep(&key);
                            runs.push(run);
                        }
                        None => runs.push(index.get_or_read(
                            &key,
                            || self.read_retained(&job, &trial),
                            finished,
                        )),
                    }
                }
            }
        }
        runs.sort_by(|a, b| {
            (b.outcome == Outcome::Running)
                .cmp(&(a.outcome == Outcome::Running))
                .then(b.started_ms.cmp(&a.started_ms))
                .then(a.id().cmp(&b.id()))
        });
        self.runs = runs;
        if let Err(error) = index.save()
            && !self.errors.contains(&error)
        {
            self.errors
                .push(format!("the startup index wasn't saved: {error}"));
        }
        self.index = Some(index);
    }

    /// How many runs the startup index answered, and how many were
    /// parsed, since the catalog was loaded.
    #[must_use]
    pub fn index_stats(&self) -> crate::index::Stats {
        self.index
            .as_ref()
            .map(|index| index.stats)
            .unwrap_or_default()
    }

    /// Whether any run is still going.
    #[must_use]
    pub fn running(&self) -> usize {
        self.runs
            .iter()
            .filter(|run| run.outcome == Outcome::Running)
            .count()
    }

    /// Finds a run by `job`, `job/trial`, trial name, or a piece of the
    /// job's name that only one job has; the newest run of that job wins.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Run> {
        let name = name.trim().trim_end_matches('/');
        let exact = self.runs.iter().find(|run| {
            run.id() == name || run.job == name || run.trial == name || run.task == name
        });
        if exact.is_some() {
            return exact;
        }
        let mut jobs: Vec<&str> = self
            .runs
            .iter()
            .filter(|run| run.job.contains(name) || run.trial.contains(name))
            .map(|run| run.job.as_str())
            .collect();
        jobs.dedup();
        match jobs.as_slice() {
            [job] => self.runs.iter().find(|run| run.job == *job),
            _ => None,
        }
    }

    fn task_info(&mut self, path: Option<&Path>, slug: &str) -> (Option<PathBuf>, TaskInfo) {
        let path = path
            .filter(|path| path.is_dir())
            .map(Path::to_path_buf)
            .or_else(|| {
                self.sources
                    .tasks
                    .iter()
                    .map(|root| root.join(slug))
                    .find(|path| path.is_dir())
            });
        let Some(path) = path else {
            return (None, TaskInfo::default());
        };
        crate::index::touch(&path.join("instruction.md"));
        crate::index::touch(&path.join("task.toml"));
        let info = self
            .tasks
            .entry(path.clone())
            .or_insert_with(|| read_task(&path))
            .clone();
        (Some(path), info)
    }

    fn read_local(&mut self, job: &Path, trial: &Path, now: i64) -> Run {
        let job_name = file_name(job);
        let trial_name = file_name(trial);
        let agent_dir = trial.join("agent");
        let native = existing(agent_dir.join("claude-code.txt"))
            .or_else(|| existing(agent_dir.join("codex.txt")));
        let files = Files {
            dir: trial.to_path_buf(),
            result: existing(trial.join("result.json")),
            config: existing(trial.join("config.json")),
            episode: existing(agent_dir.join("episode")),
            live: existing(agent_dir.join("live/episode.atif.jsonl")),
            trajectory: existing(agent_dir.join("trajectory.json")),
            native,
            verifier: existing(trial.join("verifier")),
            attempt: existing(
                job.join("tbench/attempts")
                    .join(format!("{trial_name}.json")),
            ),
        };
        let mut run = self.read(job_name, trial_name, files, false);
        if network_policy(trial).is_some_and(|network| network.public) {
            run.notes.push(
                "its agent phase ran with public network, not the model and Jev allowlist"
                    .to_owned(),
            );
        }
        // The contamination guard's check of the run's own briefings
        // (issue #9590).
        if let Some(report) = read_json(&agent_dir.join("contamination-run.json"))
            && report.get("clean") == Some(&Value::Bool(false))
        {
            let found = report
                .get("findings")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            run.notes.push(format!(
                "its briefings held {found} benchmark facts (contamination-run.json)"
            ));
        }
        if run.outcome == Outcome::Running {
            let active = [
                run.files.live.clone(),
                run.files.native.clone(),
                run.files.trajectory.clone(),
                Some(trial.join("trial.log")),
            ]
            .into_iter()
            .flatten()
            .filter_map(|path| modified_ms(&path))
            .max();
            run.active_ms = active;
            if run.started_ms.is_none() {
                run.started_ms = run.files.config.as_deref().and_then(modified_ms);
            }
            let last = active.or(run.started_ms).unwrap_or(now);
            if now - last > ABANDONED_AFTER_MS {
                run.outcome = Outcome::NotGraded(
                    "it stopped without a result; the harness running it is gone".to_owned(),
                );
                run.ended_ms = Some(last);
            }
        }
        run
    }

    fn read_retained(&mut self, job: &Path, trial: &str) -> Run {
        let dir = job.join(format!("{trial}.episode"));
        let files = Files {
            result: existing(dir.join("harbor-result.json")),
            config: None,
            episode: existing(dir.join("manifest.json")).map(|_| dir.clone()),
            live: None,
            trajectory: existing(job.join(format!("{trial}.json"))),
            native: existing(dir.join("native/claude-code.txt"))
                .or_else(|| existing(dir.join("native/codex.txt"))),
            verifier: existing(dir.join("verifier")),
            attempt: existing(dir.join("tbench-attempt.json")),
            dir,
        };
        let mut run = self.read(file_name(job), trial.to_owned(), files, true);
        // A retained result is sanitized and names no model; the
        // trajectory's head does.
        if run.model.is_none()
            && matches!(run.agent, Agent::ClaudeCode | Agent::Codex)
            && let Some(trajectory) = &run.files.trajectory
        {
            run.model = peek_model(trajectory);
        }
        if run.outcome == Outcome::Running {
            run.outcome = Outcome::NotGraded("no result was retained with it".to_owned());
        }
        run
    }

    fn read(&mut self, job: String, trial: String, files: Files, retained: bool) -> Run {
        let result = files.result.as_deref().and_then(read_json);
        let config = files.config.as_deref().and_then(read_json);
        let attempt = files.attempt.as_deref().and_then(read_json);
        let config_agent = config
            .as_ref()
            .and_then(|c| c.get("agent"))
            .or_else(|| result.as_ref().and_then(|r| r.pointer("/config/agent")))
            .cloned()
            .unwrap_or(Value::Null);
        let arm = job.split("--").nth(1).unwrap_or_default().to_owned();
        let agent_name = text(&config_agent, "/name").unwrap_or_default();
        let import = text(&config_agent, "/import_path").unwrap_or_default();
        let observed = result
            .as_ref()
            .and_then(|r| text(r, "/agent_info/name"))
            .unwrap_or_default();
        let agent = Agent::from_names(&[&agent_name, &import, &observed, &arm]);
        let model = text(&config_agent, "/model_name")
            .filter(|model| !model.is_empty() && model != "free")
            .filter(|_| matches!(agent, Agent::ClaudeCode | Agent::Codex));
        let variant = (agent == Agent::CoderOne)
            .then(|| {
                arm.strip_prefix("coder-one-")
                    .filter(|rest| !rest.is_empty())
                    .map(str::to_owned)
            })
            .flatten();
        let task_path = config
            .as_ref()
            .and_then(|c| text(c, "/task/path"))
            .or_else(|| result.as_ref().and_then(|r| text(r, "/task_id/path")))
            .or_else(|| result.as_ref().and_then(|r| text(r, "/config/task/path")))
            .map(PathBuf::from);
        let task = result
            .as_ref()
            .and_then(|r| text(r, "/task_name"))
            .or_else(|| {
                task_path
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| trial.split("__").next().unwrap_or(&trial).to_owned());
        let task = task
            .strip_prefix("terminal-bench/")
            .unwrap_or(&task)
            .to_owned();
        let (task_path, info) = self.task_info(task_path.as_deref(), &task);
        let manifest = files
            .episode
            .as_ref()
            .and_then(|episode| read_json(&episode.join("manifest.json")));
        let ask = info.ask.clone().or_else(|| {
            manifest
                .as_ref()
                .and_then(|m| text(m, "/result/title"))
                .map(|title| title.trim_end_matches('…').trim().to_owned())
        });

        let mut run = Run {
            batch: job.split("--").next().unwrap_or_default().to_owned(),
            job,
            trial,
            retained,
            task,
            task_path,
            ask,
            category: info.category,
            expert_hours: info.expert_hours,
            time_limit_sec: info.time_limit_sec,
            agent,
            variant,
            model,
            started_ms: None,
            ended_ms: None,
            agent_ms: None,
            active_ms: None,
            outcome: Outcome::Running,
            reward: None,
            tests: files.verifier.as_deref().and_then(read_tests),
            cost_usd: None,
            cost_estimated: false,
            notes: Vec::new(),
            files,
        };

        if let Some(result) = &result {
            run.started_ms = text(result, "/started_at").and_then(|t| timestamp_ms(&t));
            run.ended_ms = text(result, "/finished_at").and_then(|t| timestamp_ms(&t));
            run.agent_ms = span(result, "/agent_execution");
            run.reward = result
                .pointer("/verifier_result/rewards/reward")
                .and_then(Value::as_f64);
            run.outcome = outcome(result, run.reward, &mut run.notes);
            run.cost_usd = result
                .pointer("/agent_result/cost_usd")
                .and_then(Value::as_f64);
            if run.cost_usd.is_none() {
                let tokens = |key: &str| {
                    result
                        .pointer(&format!("/agent_result/{key}"))
                        .and_then(Value::as_u64)
                };
                let model = run.model.clone().or_else(|| {
                    run.job
                        .split("--")
                        .nth(1)
                        .and_then(|arm| arm.strip_prefix("codex-"))
                        .map(str::to_owned)
                });
                if let (Some(model), Some(input), Some(cache), Some(output)) = (
                    model,
                    tokens("n_input_tokens"),
                    tokens("n_cache_tokens"),
                    tokens("n_output_tokens"),
                ) {
                    run.cost_usd = crate::terminal_bench::list_price(&model, input, cache, output);
                    run.cost_estimated = run.cost_usd.is_some();
                }
            }
        } else if let Some(attempt) = &attempt {
            // An attempt record without Harbor's result: the harness wrote
            // what it knew.
            run.reward = attempt.pointer("/outcome/reward").and_then(Value::as_f64);
            let status = text(attempt, "/outcome/terminal_status").unwrap_or_default();
            run.outcome = match (status.as_str(), run.reward) {
                (_, Some(reward)) if reward >= 1.0 => Outcome::Passed,
                (_, Some(_)) => Outcome::Failed,
                ("cancelled", None) => {
                    Outcome::NotGraded("it was cancelled before it finished".to_owned())
                }
                (status, None) => Outcome::NotGraded(format!(
                    "it ended as {} with no grade",
                    status.replace('_', " ")
                )),
            };
            run.started_ms = text(attempt, "/timing/started_at").and_then(|t| timestamp_ms(&t));
        }
        if let Some(attempt) = &attempt
            && run.cost_usd.is_none()
        {
            run.cost_usd = attempt.pointer("/cost/amount_usd").and_then(Value::as_f64);
        }
        // Coder One's own ledger is the most complete cost it has.
        if let Some(episode) = &run.files.episode {
            let usage = read_json(&episode.join("evaluation/usage.json"))
                .or_else(|| read_json(&episode.join("usage.json")));
            if let Some(amount) = usage
                .as_ref()
                .and_then(|usage| usage.pointer("/cost/amount_usd"))
                .and_then(Value::as_f64)
            {
                run.cost_usd = Some(amount);
                run.cost_estimated = false;
            }
            if let Some(manifest) = &manifest {
                if manifest.get("usage_limit").is_some_and(Value::is_object)
                    || text(manifest, "/outcome").as_deref() == Some("usage_limited")
                {
                    run.outcome = Outcome::NotGraded(
                        "the model provider's usage limit stopped the session".to_owned(),
                    );
                }
                if let Some(name) = text(manifest, "/policy/name")
                    && let Some(variant) = name.strip_prefix("coder-one-")
                {
                    run.variant = Some(variant.to_owned());
                }
            }
        }
        run
    }
}

/// The first `model_name` in a trajectory's head, read without parsing
/// the whole document.
fn peek_model(path: &Path) -> Option<String> {
    use std::io::Read;
    crate::index::touch(path);
    let mut head = vec![0_u8; 4096];
    let read = std::fs::File::open(path).ok()?.read(&mut head).ok()?;
    let head = String::from_utf8_lossy(&head[..read]);
    let rest = head.split_once("\"model_name\"")?.1;
    let value = rest
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .strip_prefix('"')?;
    let model = value.split('"').next()?;
    (!model.is_empty()).then(|| model.to_owned())
}

/// How a finished result came out, and notes on how it ended.
fn outcome(result: &Value, reward: Option<f64>, notes: &mut Vec<String>) -> Outcome {
    let exception = result.get("exception_info").filter(|e| !e.is_null());
    if text(result, "/finished_at").is_none() && exception.is_none() && reward.is_none() {
        return Outcome::Running;
    }
    if let Some(exception) = exception {
        let kind = text(exception, "/exception_type").unwrap_or_default();
        let message = text(exception, "/exception_message").unwrap_or_default();
        let lower = message.to_lowercase();
        let why = match kind.as_str() {
            "CancelledError" => Some("it was cancelled before it finished".to_owned()),
            "AgentTimeoutError" => {
                notes.push(format!("The agent ran out of time: {message}"));
                None
            }
            "UsageLimitError" | "ApiRateLimitError" | "ApiUsageLimitError" => {
                Some("the model provider's usage limit stopped the session".to_owned())
            }
            "OSError" if lower.contains("no space left") => {
                Some("the machine ran out of disk space".to_owned())
            }
            "RuntimeError" if lower.contains("docker compose") => Some(
                if lower.contains("verifier") {
                    "the verifier's container failed to start"
                } else {
                    "the task's container failed to start"
                }
                .to_owned(),
            ),
            "NonZeroAgentExitCodeError" => {
                let detail = message
                    .split_once(": ")
                    .map_or(message.as_str(), |(_, rest)| rest)
                    .trim();
                let detail: String = detail.chars().take(120).collect();
                if lower.contains("permission denied") {
                    Some(format!(
                        "the agent could not start: {}",
                        detail.trim_end_matches(['.', ' '])
                    ))
                } else {
                    notes.push(format!("The agent exited with an error: {detail}"));
                    None
                }
            }
            "EpisodeTimeoutError" => Some(format!("the harness timed out: {message}")),
            "EpisodeContractError" => Some("the agent's binary could not run".to_owned()),
            _ => Some(format!(
                "the harness hit an error ({kind}{})",
                if message.is_empty() {
                    String::new()
                } else {
                    format!(": {}", message.chars().take(80).collect::<String>())
                }
            )),
        };
        if let Some(why) = why {
            return Outcome::NotGraded(why);
        }
    }
    match reward {
        Some(reward) if reward >= 1.0 => Outcome::Passed,
        Some(_) => Outcome::Failed,
        None => Outcome::NotGraded("the verifier gave no score".to_owned()),
    }
}

/// The milliseconds between a phase's `started_at` and `finished_at`.
fn span(value: &Value, pointer: &str) -> Option<u64> {
    let phase = value.pointer(pointer)?;
    let start = timestamp_ms(phase.get("started_at")?.as_str()?)?;
    let end = timestamp_ms(phase.get("finished_at")?.as_str()?)?;
    u64::try_from(end - start).ok()
}

/// The verifier's test counts, from its CTRF report or pytest's summary
/// line.
fn read_tests(verifier: &Path) -> Option<Tests> {
    if let Some(ctrf) = read_json(&verifier.join("ctrf.json"))
        && let Some(summary) = ctrf.pointer("/results/summary")
    {
        let n = |key: &str| summary.get(key).and_then(Value::as_u64).unwrap_or(0);
        if n("tests") > 0 {
            return Some(Tests {
                passed: n("passed"),
                failed: n("failed"),
                total: n("tests"),
            });
        }
    }
    let stdout = verifier.join("test-stdout.txt");
    crate::index::touch(&stdout);
    let stdout = std::fs::read_to_string(stdout).ok()?;
    pytest_summary(&stdout)
}

/// Parses pytest's last `=== 3 failed, 5 passed in 1.2s ===` line.
#[must_use]
pub fn pytest_summary(stdout: &str) -> Option<Tests> {
    let line = stdout.lines().rev().find(|line| {
        let line = line.trim();
        line.starts_with('=') && (line.contains(" passed") || line.contains(" failed"))
    })?;
    let mut tests = Tests::default();
    let words: Vec<&str> = line
        .split(|c: char| c == ',' || c.is_whitespace() || c == '=')
        .filter(|word| !word.is_empty())
        .collect();
    for pair in words.windows(2) {
        if let Ok(count) = pair[0].parse::<u64>() {
            match pair[1] {
                "passed" => tests.passed += count,
                "failed" | "error" | "errors" => tests.failed += count,
                "skipped" => {}
                _ => continue,
            }
            tests.total += count;
        }
    }
    (tests.total > 0).then_some(tests)
}

/// Reads a task's instruction and metadata.
fn read_task(path: &Path) -> TaskInfo {
    let mut info = TaskInfo {
        ask: std::fs::read_to_string(path.join("instruction.md"))
            .ok()
            .and_then(|text| ask_of(&text)),
        ..TaskInfo::default()
    };
    let Ok(toml) = std::fs::read_to_string(path.join("task.toml")) else {
        return info;
    };
    let mut section = String::new();
    let (mut category, mut subcategory) = (None, None);
    for line in toml.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            section = line.trim_matches(['[', ']']).to_owned();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim().trim_matches('"'));
        match (section.as_str(), key) {
            ("metadata", "category") => category = Some(value.to_owned()),
            ("metadata", "subcategory") => subcategory = Some(value.to_owned()),
            ("metadata", "expert_time_estimate_hours") => info.expert_hours = value.parse().ok(),
            ("agent", "timeout_sec") => info.time_limit_sec = value.parse().ok(),
            _ => {}
        }
    }
    info.category = match (category, subcategory) {
        (Some(c), Some(s)) if !s.is_empty() => Some(format!("{c}, {s}")),
        (Some(c), _) if !c.is_empty() => Some(c),
        _ => None,
    };
    info
}

/// Verbs a task's request tends to open with.
const ASKING: [&str; 61] = [
    "add",
    "audit",
    "benchmark",
    "clean",
    "compile",
    "complete",
    "decrypt",
    "document",
    "estimate",
    "evaluate",
    "fill",
    "improve",
    "increase",
    "investigate",
    "reduce",
    "restore",
    "rewrite",
    "upgrade",
    "analyze",
    "build",
    "calculate",
    "change",
    "compute",
    "configure",
    "convert",
    "create",
    "debug",
    "deploy",
    "design",
    "determine",
    "develop",
    "diagnose",
    "extract",
    "find",
    "fix",
    "generate",
    "get",
    "help",
    "identify",
    "implement",
    "install",
    "make",
    "merge",
    "migrate",
    "optimize",
    "parse",
    "port",
    "produce",
    "prove",
    "recover",
    "refactor",
    "repair",
    "reproduce",
    "resolve",
    "run",
    "set",
    "solve",
    "speed",
    "train",
    "update",
    "write",
];

/// What an instruction asks for, in a sentence or two: the first sentence
/// that asks for something, with the sentence before it when that one
/// sets the scene and both are short.
#[must_use]
pub fn ask_of(instruction: &str) -> Option<String> {
    let sentences = sentences(instruction);
    if sentences.is_empty() {
        return None;
    }
    let asks = |sentence: &str| {
        let lower = sentence.to_lowercase();
        let lower = lower
            .trim_start_matches(|c: char| !c.is_alphanumeric())
            .trim_start_matches("please ")
            .trim_start_matches("your task is to ")
            .trim_start_matches("your job is to ")
            .trim_start_matches("your goal is to ")
            .trim_start_matches("you need to ")
            .trim_start_matches("you must ");
        let first = lower.split_whitespace().next().unwrap_or_default();
        ASKING.contains(&first)
            || lower.contains("i need ")
            || lower.contains("i want ")
            || lower.contains("help me")
            || lower.contains("your task")
            || lower.contains("your job")
            || lower.contains("your goal")
    };
    let index = sentences.iter().take(5).position(|s| asks(s)).unwrap_or(0);
    let mut ask = sentences[index].clone();
    if index == 1 && sentences[0].len() + ask.len() < 170 {
        ask = format!("{} {ask}", sentences[0]);
    }
    Some(clip_words(&ask, 220))
}

/// The instruction's sentences, markdown headings and list markers
/// removed.
fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let text = strip_comments(text);
    for paragraph in text.split("\n\n") {
        let lines: Vec<&str> = paragraph
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("```"))
            .collect();
        if lines.is_empty() || lines[0].starts_with('#') && lines.len() == 1 {
            continue;
        }
        let joined = lines
            .iter()
            .map(|line| {
                line.trim_start_matches('#')
                    .trim_start_matches("- ")
                    .trim_start_matches("* ")
                    .trim()
            })
            .collect::<Vec<_>>()
            .join(" ");
        let mut current = String::new();
        let chars: Vec<char> = joined.chars().collect();
        for (i, c) in chars.iter().enumerate() {
            current.push(*c);
            let next = chars.get(i + 1).copied();
            if matches!(c, '.' | '?' | '!') && next.is_none_or(char::is_whitespace) {
                let sentence = current.trim().to_owned();
                if sentence.len() > 3 {
                    out.push(sentence);
                }
                current.clear();
            }
        }
        let rest = current.trim();
        if rest.len() > 3 {
            out.push(rest.to_owned());
        }
    }
    out
}

/// `text` without its HTML comments, such as a task's canary line.
fn strip_comments(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `text`, cut at a word boundary to at most `limit` characters with `…`.
#[must_use]
pub fn clip_words(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut out = String::new();
    for word in text.split_whitespace() {
        if out.chars().count() + word.chars().count() + 2 > limit {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out.push('…');
    out
}

/// `1h 04m`, `12m 30s`, `45s`.
#[must_use]
pub fn duration(ms: u64) -> String {
    let seconds = ms / 1000;
    if seconds >= 3600 {
        format!("{}h {:02}m", seconds / 3600, (seconds % 3600) / 60)
    } else if seconds >= 60 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

/// `$2.92`, or `$0.004` for very small amounts.
#[must_use]
pub fn money(usd: f64) -> String {
    if usd > 0.0 && usd < 0.001 {
        "under $0.001".to_owned()
    } else if usd > 0.0 && usd < 0.01 {
        format!("${usd:.3}")
    } else {
        format!("${usd:.2}")
    }
}

/// `just now`, `12m ago`, `3h ago`, and a date past a day.
#[must_use]
pub fn when(ms: i64, now: i64) -> String {
    let ago = now - ms;
    if ago < 0 {
        return date(ms);
    }
    let minutes = ago / 60_000;
    if minutes < 1 {
        "just now".to_owned()
    } else if minutes < 60 {
        format!("{minutes}m ago")
    } else if minutes < 24 * 60 {
        format!("{}h ago", minutes / 60)
    } else {
        date(ms)
    }
}

/// `Sep 22 14:05` in UTC.
#[must_use]
pub fn date(ms: i64) -> String {
    let seconds = ms.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    // Days since 1970-01-01 to a civil date.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{} {day} {:02}:{:02}",
        MONTHS[usize::try_from(month - 1).unwrap_or(0)],
        of_day / 3600,
        (of_day % 3600) / 60
    )
}

/// The time now, in milliseconds since the epoch.
#[must_use]
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

/// `path`, when it exists, noted as an input of the parse in progress.
fn existing(path: PathBuf) -> Option<PathBuf> {
    crate::index::touch(&path);
    path.exists().then_some(path)
}

fn modified_ms(path: &Path) -> Option<i64> {
    crate::index::touch(path);
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let elapsed = modified.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(elapsed.as_millis()).ok()
}

/// The network policy a trial's agent phase ran under, from the
/// `network-policy.json` the harness writes into the trial directory
/// (issue #9589).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    /// `public`, `no-network`, or `allowlist: host, host`.
    pub agent_phase: String,
    /// Whether the agent phase had public network.
    pub public: bool,
}

/// Reads a trial's `network-policy.json`, when the harness wrote one.
#[must_use]
pub fn network_policy(trial: &Path) -> Option<Network> {
    let record = read_json(&trial.join("network-policy.json"))?;
    let phase = record.get("agent_phase")?;
    let mode = text(phase, "/network_mode")?;
    let hosts: Vec<&str> = phase
        .get("allowed_hosts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let agent_phase = if hosts.is_empty() {
        mode.clone()
    } else {
        format!("{mode}: {}", hosts.join(", "))
    };
    Some(Network {
        public: mode == "public",
        agent_phase,
    })
}

pub(crate) fn read_json(path: &Path) -> Option<Value> {
    crate::index::touch(path);
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn text(value: &Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Which runs a list shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filter {
    pub agent: Option<Agent>,
    /// An outcome's word: `passed`, `failed`, `running`, or `not graded`.
    pub outcome: Option<&'static str>,
    pub search: String,
    /// Judgments a run must meet, each at or above its threshold, in the
    /// answers `gym runs rank` keeps. [`Filter::admits`] doesn't read them;
    /// [`Filter::judged`] does.
    pub reasons: Vec<crate::runs_group::Reason>,
    /// Only these runs, as `job/trial`, when any are named: the runs a
    /// highlight cites.
    pub runs: Vec<String>,
    /// The highlight that named [`Filter::runs`].
    pub cited_by: Option<String>,
}

impl Filter {
    /// Whether `run` passes.
    #[must_use]
    pub fn admits(&self, run: &Run) -> bool {
        self.agent.is_none_or(|agent| run.agent == agent)
            && self.outcome.is_none_or(|word| run.outcome.word() == word)
            && run.matches(&self.search)
            && (self.runs.is_empty() || self.runs.contains(&run.id()))
    }

    /// Whether Jev's `answer` for a run meets every `--reason`. With no
    /// reasons, every run does, judged or not.
    #[must_use]
    pub fn judged(&self, answer: Option<&crate::runs_learning::Answer>) -> bool {
        self.reasons.iter().all(|reason| reason.holds(answer))
    }

    /// What the filter is doing, in words, or `None` when it shows all.
    #[must_use]
    pub fn describe(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(agent) = self.agent {
            parts.push(agent.name().to_owned());
        }
        if let Some(outcome) = self.outcome {
            parts.push(outcome.to_owned());
        }
        if !self.search.trim().is_empty() {
            parts.push(format!("\"{}\"", self.search.trim()));
        }
        for reason in &self.reasons {
            parts.push(reason.describe());
        }
        if !self.runs.is_empty() {
            parts.push(format!(
                "the {} runs {} cites",
                self.runs.len(),
                self.cited_by.as_deref().unwrap_or("a highlight")
            ));
        }
        (!parts.is_empty()).then(|| parts.join(" · "))
    }
}

/// The columns one run takes in a list: when, outcome, task, agent,
/// tests, cost, and time.
#[must_use]
pub fn columns(run: &Run, now: i64) -> [String; 7] {
    let when = run
        .started_ms
        .map_or_else(|| "—".to_owned(), |ms| when(ms, now));
    let outcome = match &run.outcome {
        Outcome::Running => match run.active_ms {
            Some(active) if now - active > 10 * 60_000 => {
                format!(
                    "● quiet {}",
                    duration(u64::try_from(now - active).unwrap_or(0))
                )
            }
            _ => "● running".to_owned(),
        },
        other => format!("{} {}", other.mark(), other.word()),
    };
    let task = match &run.ask {
        Some(ask) => format!("{} — {ask}", run.task),
        None => run.task.clone(),
    };
    let tests = run
        .tests
        .map_or_else(String::new, |t| format!("{}/{}", t.passed, t.total));
    let cost = run.cost_usd.map_or_else(String::new, money);
    let time = run.elapsed_ms(now).map_or_else(String::new, duration);
    [when, outcome, task, run.agent_label(), tests, cost, time]
}

/// A run as JSON for `gym runs --json`.
#[must_use]
pub fn run_json(run: &Run, now: i64) -> Value {
    json!({
        "job": run.job,
        "trial": run.trial,
        "batch": run.batch,
        "retained": run.retained,
        "task": run.task,
        "asks": run.ask,
        "category": run.category,
        "agent": run.agent.name(),
        "variant": run.variant,
        "model": run.model.as_deref().map(model_name),
        "outcome": run.outcome.word(),
        "why_not_graded": match &run.outcome {
            Outcome::NotGraded(why) => Some(why.clone()),
            _ => None,
        },
        "reward": run.reward,
        "tests": run.tests.map(|t| json!({"passed": t.passed, "failed": t.failed, "total": t.total})),
        "cost_usd": run.cost_usd,
        "cost_estimated": run.cost_estimated,
        "started_ms": run.started_ms,
        "elapsed_ms": run.elapsed_ms(now),
        "agent_ms": run.agent_ms,
        "dir": run.files.dir,
    })
}

const USAGE: &str = "\
gym runs: Terminal-Bench runs in plain words.

Usage:
  gym runs [--order newest|learning] [--agent NAME] [--outcome WORD] [--search TEXT]
           [--reason ID[=P]]... [--limit N] [--json]
  gym runs group --by reason|task|agent|policy|outcome [filters] [--members N] [--json]
  gym runs show RUN [--transcript] [--expand] [--json | --evidence]
  gym runs rank [--limit N] [--recorded FILE] [--record FILE] [--no-jev] [--json]
  gym runs mark RUN[/STEP] [--tag ID]... [--note TEXT] [--clear]
  gym runs unmark RUN[/STEP]
  gym runs marks [--json]
  gym runs agreement [--json]
  gym runs highlights [--rule RULE]... [--limit N] [--json]
  gym runs analyze RUN [--json] [--write] [--no-jev]
  gym runs characterize RUN [--json] [--out DIR] | --all | diff A B

RUN is a job name, job/trial, a trial name, or a piece of a job name that
only one job has. --agent takes coder-one, claude-code, codex, or reference;
--outcome takes passed, failed, running, or not-graded. --jobs-dir PATH and
--traces-dir PATH read other directories; --no-jobs and --no-traces skip one,
and --no-tasks skips reading the task definitions.

--reason ID[=P] keeps the runs whose Jev judgment ID is at or above P, 0.5
when P is left out; repeat it to require several. The IDs are the question
set's: near_miss, output_slip, unearned_success, looped, harness_fault, and
the rest `gym runs show RUN --json` lists. A run Jev hasn't judged never
meets a reason. `group --by` counts the runs the filters keep per reason,
task, agent, policy (the agent with its variant or model), or outcome, lists
each group's members, and gives each judgment's mean probability over them.
A run is in every reason group whose judgment it meets, at 0.5 or at the
threshold a --reason names. Code computes the groups; nothing asks Jev.

--order learning lists the runs Jev judged most worth learning from first,
with the reasons, from the answers `gym runs rank` keeps. `gym runs rank`
asks Jev about each finished run whose evidence has no answer yet, and
reports how many it asked and what that cost; `show RUN --evidence` prints
the exact state Jev reads for a run. It reads the TypeSafe key from
TYPESAFE_API_KEY or `api_key` in ~/.openagents/jev.json. --recorded FILE
replays answers instead, --record FILE writes the answers used, and
--learning-dir PATH keeps the answers somewhere other than
~/.openagents/gym/learning. --no-reference leaves the leaderboard out.

`mark` records a person's word that a run, or one step of its transcript,
is bad, with judgment IDs as tags and a note; --clear records that the run
is fine. --marked keeps only marked runs, and the list, `show`, and the
transcript show each mark. `agreement` compares Jev's judgments with the
marks. `gym runs mark --help` says more. --marks-dir PATH keeps the marks
somewhere other than ~/.openagents/gym/marks.

`highlights` computes candidate claims worth sharing with fixed rules, each
with its runs, numbers, sample size, and caveats; `gym runs highlights
--help` says more. Nothing posts anywhere.

`analyze` computes one finished run's analysis: the verifier's result, the
true cost compared with Harbor's figure, the timeline and the slowest chain
of steps, how the acceptance tests compare with the verifier's tests,
sessions that undid earlier work, anomalies, and Fable 5.1's cheapest pass.
`gym runs analyze --help` says more.";

/// `gym runs`: the list, or one run's summary and transcript.
///
/// # Errors
///
/// Returns the usage text when the arguments do not parse, and a message
/// when the named run is not found.
pub fn command(args: &[String], out: &mut impl Write) -> Result<i32, String> {
    if args
        .first()
        .is_some_and(|word| crate::runs_marks::handles(word))
    {
        return crate::runs_marks::command(args, out);
    }
    if args
        .first()
        .is_some_and(|word| crate::runs_card_render::handles(word))
    {
        return crate::runs_card_render::command(args, out);
    }
    if args.first().map(String::as_str) == Some("analyze") {
        return crate::runs_analysis::command(&args[1..], out);
    }
    if args.first().map(String::as_str) == Some("highlights") {
        return crate::runs_highlights::command(args, out);
    }
    if args
        .first()
        .is_some_and(|word| crate::runs_fingerprint::handles(word))
    {
        return crate::runs_fingerprint::command(args, out);
    }
    let mut sources = Sources::standard();
    let mut marks_dir = crate::runs_marks::default_dir();
    let mut marked_only = false;
    let mut filter = Filter::default();
    let mut limit = 40usize;
    let (mut json_out, mut transcript, mut expand) = (false, false, false);
    let mut show: Option<String> = None;
    let mut learning_order = false;
    let mut rank = false;
    let mut rank_limit: Option<usize> = None;
    let mut recorded: Option<PathBuf> = None;
    let mut record: Option<PathBuf> = None;
    let mut no_jev = false;
    let mut evidence = false;
    let mut learning_dir = crate::runs_learning::default_dir();
    let mut reference = true;
    let mut group_by: Option<crate::runs_group::By> = None;
    let mut grouping = false;
    let mut members = 8usize;
    let mut index = 0;
    let value = |index: usize| args.get(index + 1).cloned().ok_or_else(|| USAGE.to_owned());
    while index < args.len() {
        match args[index].as_str() {
            "show" if show.is_none() => {
                show = Some(value(index)?);
                index += 1;
            }
            "rank" if index == 0 => rank = true,
            "group" if index == 0 => grouping = true,
            "--by" => {
                group_by = Some(crate::runs_group::By::parse(&value(index)?)?);
                index += 1;
            }
            "--members" => {
                members = value(index)?
                    .parse()
                    .map_err(|_| format!("--members needs a number\n\n{USAGE}"))?;
                index += 1;
            }
            "--reason" => {
                filter
                    .reasons
                    .push(crate::runs_group::Reason::parse(&value(index)?)?);
                index += 1;
            }
            "--json" => json_out = true,
            "--order" => {
                learning_order = match value(index)?.as_str() {
                    "learning" => true,
                    "newest" => false,
                    other => return Err(format!("unknown order {other}\n\n{USAGE}")),
                };
                index += 1;
            }
            "--recorded" => {
                recorded = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--record" => {
                record = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--learning-dir" => {
                learning_dir = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--no-jev" => no_jev = true,
            "--marked" => marked_only = true,
            "--marks-dir" => {
                marks_dir = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--evidence" => evidence = true,
            "--no-reference" => reference = false,
            "--transcript" => transcript = true,
            "--expand" => {
                transcript = true;
                expand = true;
            }
            "--agent" => {
                let name = value(index)?.to_lowercase().replace(['-', '_'], " ");
                filter.agent = Some(
                    Agent::ALL
                        .into_iter()
                        .find(|agent| {
                            let agent_name = agent.name().to_lowercase();
                            agent_name == name || agent_name.starts_with(&name)
                        })
                        .ok_or_else(|| format!("unknown agent {name}\n\n{USAGE}"))?,
                );
                index += 1;
            }
            "--outcome" => {
                let word = value(index)?.to_lowercase().replace('-', " ");
                filter.outcome = Some(
                    Outcome::kinds()
                        .into_iter()
                        .find(|kind| *kind == word)
                        .ok_or_else(|| format!("unknown outcome {word}\n\n{USAGE}"))?,
                );
                index += 1;
            }
            "--search" => {
                filter.search = value(index)?;
                index += 1;
            }
            "--limit" => {
                limit = value(index)?
                    .parse()
                    .map_err(|_| format!("--limit needs a number\n\n{USAGE}"))?;
                rank_limit = Some(limit);
                index += 1;
            }
            "--jobs-dir" => {
                sources.jobs = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--traces-dir" => {
                sources.traces = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--no-jobs" => sources.jobs = None,
            "--no-traces" => sources.traces = None,
            "--no-tasks" => sources.tasks.clear(),
            "--help" | "-h" => {
                writeln!(out, "{USAGE}").map_err(|e| e.to_string())?;
                return Ok(0);
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}")),
        }
        index += 1;
    }
    if grouping && group_by.is_none() {
        return Err(format!("group needs --by\n\n{USAGE}"));
    }
    let catalog = Catalog::load(sources);
    let now = now_ms();
    let write =
        |out: &mut dyn Write, text: &str| writeln!(out, "{text}").map_err(|e| e.to_string());
    let context = crate::runs_learning::Context::new(
        &catalog,
        reference
            .then(crate::terminal_bench_reference::Reference::checked)
            .flatten(),
    );
    let mut store = crate::runs_learning::Store::open(learning_dir);
    if rank {
        let judge = match (&recorded, no_jev) {
            (_, true) => crate::runs_learning::Judge::Off("--no-jev turns Jev off".to_owned()),
            (Some(path), false) => {
                crate::runs_learning::Judge::Recorded(crate::runs_learning::Recorded::load(path)?)
            }
            (None, false) => crate::runs_learning::Judge::from_environment(),
        };
        let mut recording = record
            .as_ref()
            .map(|_| crate::runs_learning::Recorded::empty());
        let report = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("cannot start a runtime: {error}"))?
            .block_on(crate::runs_learning::rank(
                &catalog.runs,
                &context,
                &mut store,
                &judge,
                rank_limit,
                recording.as_mut(),
            ));
        if let (Some(path), Some(recording)) = (&record, &recording) {
            recording.save(path)?;
        }
        if json_out {
            let value = json!({
                "schema": "openagents.gym.runs-rank.v1",
                "jev": judge.word(),
                "questions": crate::runs_learning::QUESTION_SET,
                "questions_digest": crate::runs_learning::questions_digest(),
                "usd_per_million_input": crate::runs_learning::USD_PER_MILLION_INPUT,
                "report": report,
            });
            write(
                out,
                &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
            )?;
            return Ok(0);
        }
        for line in crate::runs_learning::report_lines(&report, &judge) {
            write(out, &line)?;
        }
        write(out, "")?;
        write(out, "Most worth learning from:")?;
        let answers = crate::runs_learning::answers(&catalog, &store, &context);
        let rarity = crate::runs_learning::Rarity::of(answers.values().copied());
        let ordered = crate::runs_learning::order(catalog.runs.iter().collect(), &answers, &rarity);
        for run in ordered
            .into_iter()
            .filter(|run| answers.contains_key(&run.id()))
            .take(10)
        {
            let answer = answers[&run.id()];
            write(
                out,
                &format!(
                    "  {:.2}  {:<32} {:<30} {}",
                    answer.learning(&rarity),
                    clip_words(&run.task, 32),
                    run.agent_label(),
                    run.outcome.word()
                ),
            )?;
            let tags = answer.tags(&rarity, 3);
            if !tags.is_empty() {
                write(out, &format!("{:8}{}", "", tags.join(" · ")))?;
            }
        }
        return Ok(i32::from(report.asked > 0 && report.answered == 0));
    }
    let answers = crate::runs_learning::answers(&catalog, &store, &context);
    let rarity = crate::runs_learning::Rarity::of(answers.values().copied());
    let marks = crate::runs_marks::Marks::open(marks_dir);
    let marks_json = |run: &str| {
        Value::Array(
            marks
                .of_run(run)
                .into_iter()
                .map(crate::runs_marks::Mark::to_json)
                .collect(),
        )
    };
    if let Some(name) = show {
        let run = catalog
            .find(&name)
            .ok_or_else(|| format!("no run matches {name}"))?;
        let detail = crate::runs_story::Detail::load(run);
        let answer = answers.get(&run.id()).copied();
        if evidence {
            let state = crate::runs_learning::evidence(&detail, &context);
            write(
                out,
                &serde_json::to_string_pretty(&json!({
                    "key": crate::runs_learning::key(&state),
                    "state": state,
                }))
                .map_err(|e| e.to_string())?,
            )?;
            return Ok(0);
        }
        if json_out {
            let mut value = crate::runs_story::detail_json(&detail, now);
            value["learning"] = answer.map_or(Value::Null, |answer| answer.to_json(&rarity));
            value["marks"] = marks_json(&run.id());
            if run.outcome != Outcome::Running {
                value["card"] = crate::runs_card_render::card_json(
                    &crate::runs_card::characterize(run, &crate::runs_card::Options::for_show()),
                );
            }
            write(
                out,
                &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
            )?;
            return Ok(0);
        }
        let notes = marks
            .steps(&run.id())
            .into_iter()
            .map(|(step, mark)| (step, format!("{} {}", mark.verdict.flag(), mark.describe())))
            .collect();
        let lines =
            crate::runs_story::text_with_notes(&detail, now, 100, transcript, expand, &notes);
        let details = lines
            .iter()
            .position(|line| line == "Details")
            .unwrap_or(lines.len());
        for line in &lines[..details] {
            write(out, line)?;
        }
        write(out, "Worth learning from")?;
        match answer {
            Some(answer) => {
                for line in crate::runs_learning::summary_lines(answer, &rarity) {
                    write(out, &format!("  {line}"))?;
                }
            }
            None => write(
                out,
                "  Jev hasn't judged this run yet; `gym runs rank` asks.",
            )?,
        }
        write(out, "")?;
        if run.outcome != Outcome::Running {
            let card = crate::runs_card::characterize(run, &crate::runs_card::Options::for_show());
            write(out, "Run card")?;
            for line in crate::runs_card_render::summary_lines(&card) {
                write(out, &format!("  {line}"))?;
            }
            write(
                out,
                &format!("  The whole card: gym runs characterize {}", run.id()),
            )?;
            write(out, "")?;
        }
        let marked = crate::runs_marks::story_lines(&marks, &run.id());
        if !marked.is_empty() {
            write(out, "Marks")?;
            for line in marked {
                write(out, &format!("  {line}"))?;
            }
            write(out, "")?;
        }
        for line in &lines[details..] {
            write(out, line)?;
        }
        return Ok(0);
    }
    let admitted: Vec<&Run> = catalog
        .runs
        .iter()
        .filter(|run| filter.admits(run) && filter.judged(answers.get(&run.id()).copied()))
        .filter(|run| !marked_only || marks.is_marked(&run.id()))
        .collect();
    if let Some(by) = group_by {
        let groups = crate::runs_group::group(&admitted, &answers, &rarity, by, &filter.reasons);
        if json_out {
            let mut value =
                crate::runs_group::groups_json(&groups, by, &answers, &rarity, admitted.len());
            value["filter"] = json!(filter.describe());
            write(
                out,
                &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
            )?;
            return Ok(0);
        }
        for line in crate::runs_group::groups_text(&groups, by, &answers, &rarity, members) {
            write(out, &line)?;
        }
        if let Some(filter) = filter.describe() {
            write(out, "")?;
            write(out, &format!("Showing: {filter}"))?;
        }
        return Ok(0);
    }
    let ordered = if learning_order {
        crate::runs_learning::order(admitted, &answers, &rarity)
    } else {
        admitted
    };
    let runs: Vec<&Run> = ordered.into_iter().take(limit).collect();
    let unranked = catalog
        .runs
        .iter()
        .filter(|run| crate::runs_learning::rankable(run) && !answers.contains_key(&run.id()))
        .count();
    if json_out {
        let value = json!({
            "schema": "openagents.gym.runs.v1",
            "order": if learning_order { "learning" } else { "newest" },
            "running": catalog.running(),
            "total": catalog.runs.len(),
            "ranked": answers.len(),
            "unranked": unranked,
            "shown": runs.len(),
            "filter": filter.describe(),
            "marked_only": marked_only,
            "marked": catalog.runs.iter().filter(|run| marks.is_marked(&run.id())).count(),
            "runs": runs.iter().map(|run| {
                let mut value = run_json(run, now);
                value["learning"] = answers
                    .get(&run.id())
                    .map_or(Value::Null, |answer| answer.to_json(&rarity));
                value["marks"] = marks_json(&run.id());
                value
            }).collect::<Vec<_>>(),
        });
        write(
            out,
            &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
        )?;
        return Ok(0);
    }
    write(
        out,
        &format!(
            "Terminal-Bench runs, {}: {} shown of {}{}",
            if learning_order {
                "most worth learning from first"
            } else {
                "newest first"
            },
            runs.len(),
            catalog.runs.len(),
            match catalog.running() {
                0 => String::new(),
                n => format!(", {n} running"),
            }
        ),
    )?;
    if learning_order {
        write(
            out,
            &format!(
                "Jev ranked {} runs{}.",
                answers.len(),
                match unranked {
                    0 => String::new(),
                    n => format!(
                        "; {n} finished runs have no judgment yet, and `gym runs rank` asks about them"
                    ),
                }
            ),
        )?;
    }
    if let Some(filter) = filter.describe() {
        write(out, &format!("Showing: {filter}"))?;
    }
    if marked_only {
        write(out, "Showing: marked runs only")?;
    }
    write(out, "")?;
    for run in runs {
        let [when, outcome, task, agent, tests, cost, time] = columns(run, now);
        write(
            out,
            &format!(
                "{when:<13} {outcome:<14} {agent:<26} {tests:>6} {cost:>7} {time:>8}  {}",
                clip_words(&task, 90)
            ),
        )?;
        if let Outcome::NotGraded(why) = &run.outcome {
            write(out, &format!("{:<15}not graded: {why}", ""))?;
        }
        for mark in marks.of_run(&run.id()) {
            write(
                out,
                &format!("{:<15}{} {}", "", mark.verdict.flag(), mark.describe()),
            )?;
        }
        if learning_order && let Some(answer) = answers.get(&run.id()) {
            let tags = answer.tags(&rarity, 3);
            write(
                out,
                &format!(
                    "{:<15}worth learning from {:.2}{}",
                    "",
                    answer.learning(&rarity),
                    if tags.is_empty() {
                        String::new()
                    } else {
                        format!(": {}", tags.join(" · "))
                    }
                ),
            )?;
        }
    }
    write(out, "")?;
    write(
        out,
        "See one run: gym runs show <job>   (add --transcript for the transcript)",
    )?;
    Ok(0)
}

/// A copy of the retained fixture trials in a fresh directory, so every
/// file's time is now, and the sources that read it.
#[cfg(test)]
pub(crate) fn fixture_sources() -> (tempfile::TempDir, Sources) {
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("a fixture directory");
        for entry in std::fs::read_dir(from).expect("the fixtures").flatten() {
            let path = entry.path();
            let target = to.join(entry.file_name());
            if path.is_dir() {
                copy(&path, &target);
            } else {
                std::fs::copy(&path, &target).expect("a fixture file");
                // macOS copy can preserve the old modification time. The
                // running fixture must be fresh on every platform.
                std::fs::File::open(&target)
                    .expect("a copied fixture")
                    .set_modified(SystemTime::now())
                    .expect("a fresh fixture timestamp");
            }
        }
    }
    let dir = tempfile::tempdir().expect("a temporary directory");
    copy(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/runs"),
        dir.path(),
    );
    let sources = Sources {
        jobs: Some(dir.path().join("jobs")),
        traces: Some(dir.path().join("traces")),
        tasks: Vec::new(),
        index: None,
    };
    (dir, sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_task<'a>(catalog: &'a Catalog, task: &str) -> &'a Run {
        catalog
            .runs
            .iter()
            .find(|run| run.task == task)
            .unwrap_or_else(|| panic!("no {task} run"))
    }

    #[test]
    fn a_trial_names_its_agent_network_and_a_public_one_is_flagged() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(network_policy(dir.path()), None);
        let write = |phase: Value| {
            std::fs::write(
                dir.path().join("network-policy.json"),
                json!({"agent_phase": phase}).to_string(),
            )
            .unwrap();
        };
        write(
            json!({"network_mode": "allowlist", "allowed_hosts": ["chatgpt.com", "openagents.com"]}),
        );
        assert_eq!(
            network_policy(dir.path()),
            Some(Network {
                agent_phase: "allowlist: chatgpt.com, openagents.com".to_owned(),
                public: false,
            })
        );
        write(json!({"network_mode": "public", "allowed_hosts": []}));
        assert!(network_policy(dir.path()).is_some_and(|network| network.public));
    }

    #[test]
    fn the_catalog_reads_every_kind_of_retained_run() {
        let (_dir, sources) = fixture_sources();
        let catalog = Catalog::load(sources);
        assert_eq!(catalog.runs.len(), 5, "{:#?}", catalog.runs);
        // A run still going sorts first.
        assert_eq!(catalog.runs[0].task, "fin-saccr-rwa");
        assert_eq!(catalog.runs[0].outcome, Outcome::Running);
        assert_eq!(catalog.running(), 1);

        let coq = by_task(&catalog, "coq-block-bound");
        assert_eq!(coq.outcome, Outcome::Passed);
        assert_eq!(coq.agent, Agent::CoderOne);
        assert_eq!(coq.agent_label(), "Coder One · tunable-v6");
        assert_eq!(
            coq.tests,
            Some(Tests {
                passed: 4,
                failed: 0,
                total: 4
            })
        );
        assert_eq!(coq.cost_usd.map(money).as_deref(), Some("$2.92"));
        assert_eq!(coq.elapsed_ms(0).map(duration).as_deref(), Some("16m 34s"));
        assert!(
            coq.ask
                .as_deref()
                .is_some_and(|ask| ask.contains("target_theorem")),
            "{:?}",
            coq.ask
        );

        let wal = by_task(&catalog, "wal-recovery-ordering");
        assert_eq!(wal.outcome, Outcome::Failed);
        assert_eq!(wal.agent_label(), "Claude Code · Opus 5.5");
        assert_eq!(wal.tests.map(|t| (t.passed, t.total)), Some((95, 97)));

        let uefi = by_task(&catalog, "uefi-bootkit");
        assert_eq!(
            uefi.outcome,
            Outcome::NotGraded("the machine ran out of disk space".to_owned())
        );

        let codex = by_task(&catalog, "cancel-async-tasks");
        assert!(codex.retained);
        assert_eq!(codex.agent_label(), "Codex · GPT-6 Luna");
        assert_eq!(codex.outcome, Outcome::Failed);
    }

    #[test]
    fn a_run_nobody_has_touched_for_hours_is_not_left_running() {
        let (_dir, sources) = fixture_sources();
        let mut catalog = Catalog::load(sources);
        catalog.refresh(now_ms() + ABANDONED_AFTER_MS + 60_000);
        let fin = by_task(&catalog, "fin-saccr-rwa");
        assert!(
            matches!(&fin.outcome, Outcome::NotGraded(why) if why.contains("harness")),
            "{:?}",
            fin.outcome
        );
    }

    #[test]
    fn a_run_is_found_by_job_trial_or_task() {
        let (_dir, sources) = fixture_sources();
        let catalog = Catalog::load(sources);
        for name in [
            "tb4--coder-one-tunable-v6--coq-block-bound",
            "tb4--coder-one-tunable-v6--coq-block-bound/coq-block-bound__Mu8ygpJ",
            "coq-block-bound__Mu8ygpJ",
            "coq-block-bound",
            "v6--coq",
        ] {
            assert_eq!(
                catalog.find(name).map(|run| run.task.as_str()),
                Some("coq-block-bound"),
                "{name}"
            );
        }
        assert!(
            catalog.find("tunable").is_none(),
            "an ambiguous piece names nothing"
        );
    }

    #[test]
    fn gym_runs_prints_the_list_and_one_run() {
        let (dir, _) = fixture_sources();
        let jobs = dir.path().join("jobs").display().to_string();
        let traces = dir.path().join("traces").display().to_string();
        let mut out = Vec::new();
        let code = command(
            &[
                "--jobs-dir".to_owned(),
                jobs.clone(),
                "--traces-dir".to_owned(),
                traces.clone(),
            ],
            &mut out,
        )
        .unwrap();
        assert_eq!(code, 0);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("5 shown of 5, 1 running"), "{text}");
        assert!(
            text.contains("not graded: the machine ran out of disk space"),
            "{text}"
        );

        let mut out = Vec::new();
        command(
            &[
                "show".to_owned(),
                "coq-block-bound".to_owned(),
                "--transcript".to_owned(),
                "--jobs-dir".to_owned(),
                jobs.clone(),
                "--no-traces".to_owned(),
            ],
            &mut out,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        for heading in [
            "What the task asked",
            "What happened",
            "What the verifier found",
            "Why it passed",
            "Cost and time",
            "Transcript",
            "Claude Code on Opus 5.5 takes over",
        ] {
            assert!(text.contains(heading), "{heading}: {text}");
        }

        let mut out = Vec::new();
        command(
            &[
                "--json".to_owned(),
                "--outcome".to_owned(),
                "failed".to_owned(),
                "--jobs-dir".to_owned(),
                jobs,
                "--traces-dir".to_owned(),
                traces,
            ],
            &mut out,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(value["shown"], 2, "{value}");
        assert!(
            value["runs"]
                .as_array()
                .unwrap()
                .iter()
                .all(|run| run["outcome"] == "failed")
        );
    }

    #[test]
    fn gym_runs_rank_fills_the_cache_once_and_the_learning_order_reads_it() {
        let (dir, _) = fixture_sources();
        let store = tempfile::tempdir().unwrap();
        let recorded = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/runs-learning/recorded.json");
        let base = [
            "--jobs-dir".to_owned(),
            dir.path().join("jobs").display().to_string(),
            "--traces-dir".to_owned(),
            dir.path().join("traces").display().to_string(),
            "--no-tasks".to_owned(),
            "--no-reference".to_owned(),
            "--learning-dir".to_owned(),
            store.path().display().to_string(),
        ];
        let run = |extra: &[&str]| {
            let mut args: Vec<String> = extra.iter().map(|s| (*s).to_owned()).collect();
            args.extend(base.iter().cloned());
            let mut out = Vec::new();
            let code = command(&args, &mut out).unwrap();
            (code, String::from_utf8(out).unwrap())
        };
        let recorded = recorded.display().to_string();
        let (code, text) = run(&["rank", "--recorded", &recorded]);
        assert_eq!(code, 0, "{text}");
        assert!(text.contains("about 4, 4 answered, 0 failed"), "{text}");
        assert!(
            text.contains("Runs still running, to rank after they finish: 1."),
            "{text}"
        );
        assert!(text.contains("Most worth learning from:"), "{text}");

        // Unchanged evidence: a second pass makes no request.
        let (_, text) = run(&["rank", "--recorded", &recorded, "--json"]);
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["report"]["asked"], 0, "{value}");
        assert_eq!(value["report"]["cached"], 4, "{value}");

        // The learning order reads the cache with no Jev at all.
        let (_, text) = run(&["--order", "learning"]);
        assert!(text.contains("most worth learning from first"), "{text}");
        assert!(text.contains("Jev ranked 4 runs."), "{text}");
        assert!(text.contains("worth learning from 0."), "{text}");
        let (_, text) = run(&["--order", "learning", "--json"]);
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["order"], "learning");
        let runs = value["runs"].as_array().unwrap();
        let learning: Vec<f64> = runs
            .iter()
            .filter_map(|run| run["learning"]["learning"].as_f64())
            .collect();
        assert_eq!(learning.len(), 4);
        assert!(learning.windows(2).all(|w| w[0] >= w[1]), "{learning:?}");
        assert_eq!(runs.last().unwrap()["outcome"], "running");

        let (_, text) = run(&["show", "wal-recovery-ordering"]);
        assert!(
            text.contains("Worth learning from\n  Learning value"),
            "{text}"
        );
    }

    #[test]
    fn reasons_filter_and_groups_read_the_recorded_answers() {
        let (dir, _) = fixture_sources();
        let store = tempfile::tempdir().unwrap();
        let recorded = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/runs-learning/recorded.json")
            .display()
            .to_string();
        let base = [
            "--jobs-dir".to_owned(),
            dir.path().join("jobs").display().to_string(),
            "--traces-dir".to_owned(),
            dir.path().join("traces").display().to_string(),
            "--no-tasks".to_owned(),
            "--no-reference".to_owned(),
            "--learning-dir".to_owned(),
            store.path().display().to_string(),
        ];
        let run = |extra: &[&str]| {
            let mut args: Vec<String> = extra.iter().map(|s| (*s).to_owned()).collect();
            args.extend(base.iter().cloned());
            let mut out = Vec::new();
            let code = command(&args, &mut out).unwrap();
            (code, String::from_utf8(out).unwrap())
        };
        let json = |extra: &[&str]| -> Value { serde_json::from_str(&run(extra).1).unwrap() };
        let ids = |value: &Value| -> Vec<String> {
            let mut ids: Vec<String> = value["runs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|run| run["task"].as_str().unwrap().to_owned())
                .collect();
            ids.sort();
            ids
        };
        assert_eq!(run(&["rank", "--recorded", &recorded]).0, 0);

        // Two runs claimed a success Jev judges unearned.
        let value = json(&["--reason", "unearned_success", "--json"]);
        assert_eq!(
            ids(&value),
            vec!["cancel-async-tasks", "wal-recovery-ordering"],
            "{value}"
        );
        assert_eq!(value["filter"], "unearned_success at 0.50 or above");
        // A higher threshold keeps one, and reasons combine.
        let value = json(&["--reason", "unearned_success=0.97", "--json"]);
        assert_eq!(ids(&value), vec!["wal-recovery-ordering"], "{value}");
        let value = json(&[
            "--reason",
            "unearned_success",
            "--reason",
            "near_miss",
            "--json",
        ]);
        assert_eq!(ids(&value), vec!["wal-recovery-ordering"], "{value}");
        let value = json(&["--reason", "looped", "--outcome", "failed", "--json"]);
        assert!(ids(&value).is_empty(), "{value}");

        // Groups by reason: a run is in every reason it meets.
        let value = json(&["group", "--by", "reason", "--json"]);
        assert_eq!(value["by"], "reason");
        let groups = value["groups"].as_array().unwrap();
        let unearned = groups
            .iter()
            .find(|g| g["key"] == "unearned_success")
            .expect("an unearned-success group");
        assert_eq!(unearned["count"], 2, "{unearned}");
        assert_eq!(unearned["tag"], "claimed unearned success");
        assert_eq!(unearned["mean_probability"][0]["id"], "unearned_success");
        assert!(
            unearned["members"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m["probability"].as_f64().unwrap() >= 0.5)
        );
        let counts: Vec<u64> = groups
            .iter()
            .map(|g| g["count"].as_u64().unwrap())
            .collect();
        assert!(counts.windows(2).all(|w| w[0] >= w[1]), "{counts:?}");
        // The same answers group the same way every time.
        assert_eq!(value, json(&["group", "--by", "reason", "--json"]));

        // By outcome, every run is in exactly one group.
        let value = json(&["group", "--by", "outcome", "--json"]);
        let total: u64 = value["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["count"].as_u64().unwrap())
            .sum();
        assert_eq!(total, 5, "{value}");
        let (_, text) = run(&["group", "--by", "policy", "--reason", "unearned_success"]);
        assert!(text.contains("grouped by policy"), "{text}");
        assert!(
            text.contains("Showing: unearned_success at 0.50 or above"),
            "{text}"
        );

        // One run's JSON has every judgment's probability, not only the
        // reasons, and numbered transcript steps.
        let value = json(&["show", "wal-recovery-ordering", "--json"]);
        let every = value["learning"]["every_judgment"].as_array().unwrap();
        assert_eq!(every.len(), crate::runs_learning::JUDGMENTS.len());
        assert!(every.iter().any(|j| j["reason"] == false), "{value}");
        assert_eq!(value["transcript"][0]["step"], 1);

        assert!(command(&["group".to_owned()], &mut Vec::new()).is_err());
        assert!(command(&["--reason".to_owned(), "nope".to_owned()], &mut Vec::new()).is_err());
    }

    #[test]
    fn the_ask_is_the_sentence_that_asks() {
        let coq = "A Coq project is at `/app/`. The file `/app/Main.v` declares an abstract type.\n\nProve `target_theorem` in `/app/Main.v`.\n\nYou may add lemmas.";
        assert_eq!(
            ask_of(coq).as_deref(),
            Some("Prove `target_theorem` in `/app/Main.v`.")
        );
        let git = "I just made some changes to my personal site and checked out master, but now I can't find those changes. Please help me find them and merge them into master.\n";
        assert_eq!(ask_of(git).as_deref(), Some(git.trim()));
        assert_eq!(ask_of("   "), None);
    }

    #[test]
    fn pytest_counts_read_from_its_summary_line() {
        let out = "collected 16 items\n\n==== 8 failed, 8 passed in 0.40s ====\n";
        assert_eq!(
            pytest_summary(out),
            Some(Tests {
                passed: 8,
                failed: 8,
                total: 16
            })
        );
        assert_eq!(pytest_summary("no tests ran"), None);
    }

    #[test]
    fn times_and_money_read_plainly() {
        assert_eq!(duration(45_000), "45s");
        assert_eq!(duration(754_000), "12m 34s");
        assert_eq!(duration(3_840_000), "1h 04m");
        assert_eq!(money(2.9205), "$2.92");
        assert_eq!(money(0.004), "$0.004");
        let now = 1_790_173_207_539;
        assert_eq!(when(now - 30_000, now), "just now");
        assert_eq!(when(now - 12 * 60_000, now), "12m ago");
        assert_eq!(when(now - 3 * 3_600_000, now), "3h ago");
        assert_eq!(date(1_790_173_207_539), "Sep 23 14:20");
    }

    #[test]
    fn an_exception_says_why_a_run_was_not_graded() {
        let mut notes = Vec::new();
        let result = json!({
            "finished_at": "2026-09-23T14:35:36Z",
            "exception_info": {"exception_type": "OSError", "exception_message": "[Errno 28] No space left on device"},
            "verifier_result": null
        });
        assert_eq!(
            outcome(&result, None, &mut notes),
            Outcome::NotGraded("the machine ran out of disk space".to_owned())
        );
        let timeout = json!({
            "finished_at": "2026-09-23T14:35:36Z",
            "exception_info": {"exception_type": "AgentTimeoutError", "exception_message": "Agent execution timed out after 900.0 seconds"},
        });
        assert_eq!(outcome(&timeout, Some(0.0), &mut notes), Outcome::Failed);
        assert!(notes[0].contains("ran out of time"), "{notes:?}");
        let pass = json!({"finished_at": "2026-09-23T14:35:36Z", "exception_info": null});
        assert_eq!(outcome(&pass, Some(1.0), &mut notes), Outcome::Passed);
    }

    #[test]
    fn a_filter_narrows_by_agent_outcome_and_words() {
        let run = crate::runs_story::tests::fixture_run();
        let mut filter = Filter::default();
        assert!(filter.admits(&run));
        filter.agent = Some(Agent::ClaudeCode);
        assert!(!filter.admits(&run));
        filter.agent = Some(Agent::CoderOne);
        filter.outcome = Some("passed");
        filter.search = "coq prove".to_owned();
        assert!(filter.admits(&run), "{run:?}");
        filter.search = "sqlite".to_owned();
        assert!(!filter.admits(&run));
        assert_eq!(
            filter.describe().as_deref(),
            Some("Coder One · passed · \"sqlite\"")
        );
    }

    #[test]
    fn the_startup_index_keeps_finished_runs_and_follows_new_changed_and_deleted_ones() {
        let (dir, mut sources) = fixture_sources();
        let index = tempfile::tempdir().unwrap();
        sources.index = Some(index.path().to_path_buf());
        let first = Catalog::load(sources.clone());
        let plain = Catalog::load(Sources {
            index: None,
            ..sources.clone()
        });
        assert_eq!(first.runs, plain.runs);
        let finished = first
            .runs
            .iter()
            .filter(|run| run.outcome != Outcome::Running)
            .count();
        assert!(finished >= 3 && finished < first.runs.len());
        assert_eq!(first.index_stats().hits, 0);

        // Unchanged: every finished run comes from the index, and a
        // running one is parsed again.
        let second = Catalog::load(sources.clone());
        assert_eq!(second.runs, first.runs);
        assert_eq!(
            second.index_stats(),
            crate::index::Stats {
                hits: finished,
                parsed: first.runs.len() - finished,
            }
        );

        // A changed run: the verifier's grade is rewritten.
        let jobs = dir.path().join("jobs");
        let trial = jobs
            .join("tb4--claude-code-opus--wal-recovery-ordering/wal-recovery-ordering__9xaN7wM");
        let mut result = read_json(&trial.join("result.json")).unwrap();
        result["verifier_result"]["rewards"]["reward"] = json!(1.0);
        std::fs::write(
            trial.join("result.json"),
            serde_json::to_vec_pretty(&result).unwrap(),
        )
        .unwrap();
        // A new run: the same trial under another job.
        let new =
            jobs.join("tb4--claude-code-opus--wal-recovery-ordering-2/wal-recovery-ordering__new");
        std::fs::create_dir_all(new.join("agent")).unwrap();
        for file in ["config.json", "result.json", "agent/trajectory.json"] {
            std::fs::copy(trial.join(file), new.join(file)).unwrap();
        }
        // A deleted run.
        std::fs::remove_dir_all(jobs.join("tb4--coder-one-tunable-v6--fin-saccr-rwa")).unwrap();

        let third = Catalog::load(sources.clone());
        let wal = third
            .runs
            .iter()
            .find(|run| {
                run.id()
                    == "tb4--claude-code-opus--wal-recovery-ordering/wal-recovery-ordering__9xaN7wM"
            })
            .unwrap();
        assert_eq!(wal.outcome, Outcome::Passed);
        assert!(
            third
                .runs
                .iter()
                .any(|run| run.trial == "wal-recovery-ordering__new")
        );
        assert!(third.runs.iter().all(|run| run.task != "fin-saccr-rwa"));
        assert_eq!(third.runs.len(), first.runs.len());
        let plain = Catalog::load(Sources {
            index: None,
            ..sources.clone()
        });
        assert_eq!(third.runs, plain.runs);

        // The deleted run's entry is gone: every finished run the next
        // load finds comes from the index.
        let fourth = Catalog::load(sources);
        let finished = fourth
            .runs
            .iter()
            .filter(|run| run.outcome != Outcome::Running)
            .count();
        assert_eq!(fourth.index_stats().hits, finished);
        let kept: Value = serde_json::from_slice(
            &std::fs::read(
                std::fs::read_dir(index.path())
                    .unwrap()
                    .flatten()
                    .next()
                    .unwrap()
                    .path(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(kept["entries"].as_object().unwrap().len(), finished);
        assert!(
            kept["entries"]
                .as_object()
                .unwrap()
                .keys()
                .all(|key| !key.contains("fin-saccr-rwa"))
        );
    }
}
