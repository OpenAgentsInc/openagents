//! `coder-one ask`: answer a question about runs by reading the Gym.
//!
//! ```text
//! coder-one ask "QUESTION" [--scope gym|repo] [--executor luna|opus] [--budget USD] [--json]
//! ```
//!
//! An ask is an episode that ends in an answer with citations rather than
//! a change. It only reads:
//!
//! ```text
//! probe    the host runs a fixed battery of Gym reads, chosen by scope
//! judge    Jev says which reasons and tasks the question asks about, which
//!          runs bear on it, and which of their transcript steps do
//! brief    code assembles the evidence into a capped briefing
//! answer   Codex on Luna or Claude Code on Opus reads it, may run more
//!          allowlisted reads, and calls `answer` with its claims
//! cite     code checks every citation against the Gym; a claim whose
//!          citation doesn't check is marked unverified, never dropped
//! ```
//!
//! Every command, the host's and the executor's, runs inside one
//! `coder-boundary` filesystem boundary whose only writable paths are the
//! ask's own scratch directory and the executor CLI's state and temporary
//! directories: the Gym's stores and the repository stay read-only. The
//! executor's only tools are [`tools`]' `read`, which checks each command
//! against [`allow`], and `answer`.
//!
//! The episode records itself like every other Coder One episode: an ATIF
//! invocation log with a component for each probe battery, each Jev
//! request, the briefing, the executor, and the citation check, beside a
//! manifest, the briefing, the executor's stream, and the answer, under
//! `~/.openagents/coder-one/asks/ask-<ms>/`. `gym coder asks` reads them.

pub mod allow;
pub mod brief;
pub mod cite;
pub mod executor;
pub mod gather;
pub mod tools;

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use atif::document::{Call, Outcome as CallOutcome, Source, Step};
use futures_util::future::join_all;
use serde_json::{Value, json};

use crate::component::jev::{Asked, JevMode, Recorded, USD_PER_MILLION_INPUT};
use crate::record::{Cost, Finish, Implementation, Outcome, Recorder, Start};

use executor::Which;
use gather::{Probe, Reader};

/// The manifest's schema.
pub const SCHEMA: &str = "openagents.coder-one.ask.v1";

/// What an ask may spend unless the operator says otherwise, in dollars.
pub const DEFAULT_BUDGET_USD: f64 = 1.0;

/// How long the executor may run unless the operator says otherwise.
pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(240);

/// The command line's usage.
pub const USAGE: &str = "\
usage: coder-one ask \"QUESTION\" [--scope gym|repo] [--executor luna|opus] [--budget USD]
                     [--json | --events] [--run RUN] [--context TEXT]... [--rank]
                     [--model MODEL] [--timeout SECONDS] [--gym PATH] [--repo DIR]
                     [--out DIR] [--no-jev] [--jev-recorded FILE] [--jev-record FILE]
                     [--answer-file FILE]

Answers a question about Terminal-Bench runs by reading the Gym, with every
claim's citations checked by code. It only reads: each command runs inside a
filesystem boundary that leaves only the ask's scratch directory and the
executor CLI's own state writable.

--scope gym (the default) probes the Gym's runs, reason groups, and outcome
matrix; --scope repo searches the repository's Markdown instead. --executor
luna (the default) runs Codex on GPT-6 Luna, and opus runs Claude Code on Opus
5.5. --budget caps the whole ask, Jev included, at USD dollars (default 1.00);
Claude Code enforces its share, and a Codex run over it is reported. --run
names the run the operator has selected, --context adds a line the executor
reads, and --rank lets the executor run `gym runs rank`, which spends Jev
requests and writes the learning store.

--json prints the record as JSON; --events prints one JSON event per line as
the ask runs, then the record. The ask is recorded under
~/.openagents/coder-one/asks unless --out names another directory.
--gym PATH names the gym binary (default: $CODER_ONE_GYM_BIN, the gym beside
this binary, or gym on PATH), and --repo DIR the repository commands run in
(default: the current directory). --no-jev replaces Jev's relevance
judgments with code's order, --jev-recorded FILE replays recorded answers,
and --jev-record FILE writes the answers used. --answer-file FILE replays an
executor's answer instead of running one.";

/// Where the question is answered from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Gym,
    Repo,
}

impl Scope {
    fn parse(text: &str) -> Result<Self, String> {
        match text.trim() {
            "gym" => Ok(Scope::Gym),
            "repo" => Ok(Scope::Repo),
            other => Err(format!("--scope takes gym or repo, not {other}")),
        }
    }

    fn word(self) -> &'static str {
        match self {
            Scope::Gym => "gym",
            Scope::Repo => "repo",
        }
    }
}

/// Where Jev's answers come from.
#[derive(Clone, Debug, PartialEq)]
pub enum JevChoice {
    Live,
    Off,
    Recorded(PathBuf),
}

/// An ask's options.
#[derive(Clone, Debug)]
pub struct Options {
    pub question: String,
    pub scope: Scope,
    pub executor: Which,
    pub model: Option<String>,
    pub budget_usd: f64,
    pub deadline: Duration,
    pub json: bool,
    pub events: bool,
    pub rank: bool,
    pub run: Option<String>,
    pub context: Vec<String>,
    pub gym: Option<PathBuf>,
    pub repo: Option<PathBuf>,
    pub out: Option<PathBuf>,
    pub jev: JevChoice,
    pub jev_record: Option<PathBuf>,
    pub answer_file: Option<PathBuf>,
}

impl Options {
    /// Parses the arguments after `ask`.
    ///
    /// # Errors
    ///
    /// Returns a message with the usage when they don't parse.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Options {
            question: String::new(),
            scope: Scope::Gym,
            executor: Which::Luna,
            model: None,
            budget_usd: DEFAULT_BUDGET_USD,
            deadline: DEFAULT_DEADLINE,
            json: false,
            events: false,
            rank: false,
            run: None,
            context: Vec::new(),
            gym: None,
            repo: None,
            out: None,
            jev: JevChoice::Live,
            jev_record: None,
            answer_file: None,
        };
        let mut words: Vec<String> = Vec::new();
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| {
                args.next()
                    .cloned()
                    .ok_or_else(|| format!("{name} needs a value\n\n{USAGE}"))
            };
            match arg.as_str() {
                "--scope" => options.scope = Scope::parse(&value("--scope")?)?,
                "--executor" => options.executor = Which::parse(&value("--executor")?)?,
                "--model" => options.model = Some(value("--model")?),
                "--budget" => {
                    options.budget_usd = value("--budget")?
                        .trim_start_matches('$')
                        .parse()
                        .map_err(|_| "--budget takes dollars, such as 0.50".to_string())?;
                }
                "--timeout" => {
                    options.deadline = Duration::from_secs(
                        value("--timeout")?
                            .parse()
                            .map_err(|_| "--timeout takes seconds".to_string())?,
                    );
                }
                "--json" => options.json = true,
                "--events" => options.events = true,
                "--rank" => options.rank = true,
                "--run" => options.run = Some(value("--run")?),
                "--context" => options.context.push(value("--context")?),
                "--gym" => options.gym = Some(PathBuf::from(value("--gym")?)),
                "--repo" => options.repo = Some(PathBuf::from(value("--repo")?)),
                "--out" => options.out = Some(PathBuf::from(value("--out")?)),
                "--no-jev" => options.jev = JevChoice::Off,
                "--jev-recorded" => {
                    options.jev = JevChoice::Recorded(PathBuf::from(value("--jev-recorded")?));
                }
                "--jev-record" => options.jev_record = Some(PathBuf::from(value("--jev-record")?)),
                "--answer-file" => {
                    options.answer_file = Some(PathBuf::from(value("--answer-file")?));
                }
                "--help" | "-h" => return Err(USAGE.to_string()),
                // Everything after `--` is the question, whatever it looks
                // like: an executor capability appends the task that way.
                "--" => {
                    words.extend(args.by_ref().cloned());
                }
                flag if flag.starts_with("--") => {
                    return Err(format!("unknown option {flag}\n\n{USAGE}"));
                }
                word => words.push(word.to_string()),
            }
        }
        options.question = words.join(" ").trim().to_string();
        if options.question.is_empty() {
            return Err(format!("ask needs a question\n\n{USAGE}"));
        }
        if options.budget_usd.is_nan() || options.budget_usd <= 0.0 {
            return Err("--budget must be more than zero".to_string());
        }
        Ok(options)
    }
}

/// Where an ask's progress goes: lines on standard error, or one JSON event
/// per line on standard output.
#[derive(Clone, Copy, Debug, Default)]
pub struct Progress {
    pub events: bool,
    pub quiet: bool,
}

impl Progress {
    /// Reports one line of progress.
    pub fn line(&self, text: &str) {
        if self.quiet {
            return;
        }
        if self.events {
            println!("{}", json!({ "event": "progress", "text": text }));
        } else {
            eprintln!("  {text}");
        }
    }
}

/// `text` cut to `max` characters, with a marker when it was cut.
#[must_use]
pub fn clip(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_string(),
    }
}

/// `text` cut to about `max` characters by dropping its middle, with a
/// marker that says how much was dropped.
#[must_use]
pub fn clip_middle(text: &str, max: usize) -> String {
    let total = text.chars().count();
    if total <= max {
        return text.to_string();
    }
    let head = max * 3 / 4;
    let tail = max - head;
    let start: String = text.chars().take(head).collect();
    let end: String = text.chars().skip(total - tail).collect();
    format!(
        "{start}\n…[{} characters dropped from the middle]…\n{end}",
        total - head - tail
    )
}

/// The `gym` binary: `--gym`, `$CODER_ONE_GYM_BIN`, the one beside this
/// binary, or `gym` on `PATH`.
#[must_use]
pub fn find_gym(named: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = named {
        return Some(path.to_path_buf());
    }
    if let Some(path) = std::env::var_os("CODER_ONE_GYM_BIN").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("gym")))
        .filter(|path| path.is_file())
        .or_else(|| allow::which("gym"))
}

/// Where asks are recorded: `~/.openagents/coder-one/asks`.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    crate::credentials::openagents_dir().map(|dir| dir.join("coder-one/asks"))
}

fn millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn jev_usd(asked: &[&Asked]) -> f64 {
    asked
        .iter()
        .filter(|a| a.how == "live")
        .filter_map(|a| a.input_tokens)
        .map(|tokens| tokens as f64 * USD_PER_MILLION_INPUT / 1_000_000.0)
        .sum()
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).to_string()).collect()
}

/// What the gathering found, for the briefing and the citation check.
#[derive(Default)]
struct Gathered {
    inputs: brief::Inputs,
    /// `gym runs show RUN --json` per run, by the name it was read under.
    shown: BTreeMap<String, Value>,
    asked: Vec<Asked>,
    candidates: usize,
}

/// Runs one ask. Returns the record and the exit code: 0 when the
/// executor answered, 1 when it didn't.
///
/// # Errors
///
/// Returns a message when the ask can't start: no boundary on this host,
/// no `gym` binary for a Gym question, or a directory that can't be made.
pub async fn run(options: Options, progress: &Progress) -> Result<(Value, i32), String> {
    let started = Instant::now();
    let at = atif::now_ms();
    let id = format!("ask-{at}");
    let root = options
        .out
        .clone()
        .or_else(default_dir)
        .ok_or("HOME is not set, so there's nowhere to record the ask; pass --out")?;
    let mut dir = root.join(&id);
    if let Err(error) = std::fs::create_dir_all(&dir) {
        // Run as a delegated executor, the ask sits inside the caller's own
        // boundary, which keeps `~/.openagents` sealed. Its record then
        // goes to the temporary directory that boundary grants, and the
        // record says where.
        if options.out.is_some() {
            return Err(format!("cannot create {}: {error}", dir.display()));
        }
        let fallback = std::env::temp_dir().join("coder-one-asks").join(&id);
        std::fs::create_dir_all(&fallback).map_err(|second| {
            format!(
                "cannot create {} ({error}) or {} ({second})",
                dir.display(),
                fallback.display()
            )
        })?;
        progress.line(&format!(
            "record ▸ {} isn't writable here ({error}), so the ask records under {}",
            root.display(),
            fallback.display()
        ));
        dir = fallback;
    }
    let repo = options
        .repo
        .clone()
        .map_or_else(std::env::current_dir, Ok)
        .map_err(|error| format!("cannot read the current directory: {error}"))?;
    let repo = repo.canonicalize().unwrap_or(repo);
    let gym = find_gym(options.gym.as_deref());
    if options.scope == Scope::Gym && gym.is_none() {
        return Err(
            "no gym binary: build it with `cargo build -p gym`, then pass --gym PATH or set \
             CODER_ONE_GYM_BIN"
                .to_string(),
        );
    }

    // One boundary for every command the ask runs.
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut spec = coder_boundary::Boundary::readonly().owned_scratch_under(&dir);
    for state in [
        home.as_ref().map(|home| home.join(".claude")),
        home.as_ref().map(|home| home.join(".codex")),
        home.as_ref().map(|home| home.join(".cache")),
        Some(std::env::temp_dir()),
    ]
    .into_iter()
    .flatten()
    .filter(|path| path.is_dir())
    {
        spec = spec.writable(state);
    }
    if options.rank
        && let Some(learning) = home
            .as_ref()
            .map(|home| home.join(".openagents/gym/learning"))
    {
        std::fs::create_dir_all(&learning)
            .map_err(|error| format!("cannot create {}: {error}", learning.display()))?;
        spec = spec.writable(learning);
    }
    let boundary = spec
        .build()
        .map_err(|error| format!("cannot bound the ask, so it doesn't run: {error}"))?;
    let scratch = boundary
        .scratch()
        .map(Path::to_path_buf)
        .ok_or("the boundary made no scratch directory")?;

    let mut session = atif::Session::opening(
        &id,
        "none",
        "ask",
        &repo.to_string_lossy(),
        &crate::episode::version(),
    );
    session.directive = options.question.clone();
    let log_path = dir.join(crate::episode::INVOCATION_LOG);
    let log = atif::Log::create_at(&log_path, &session)
        .map_err(|error| format!("cannot create {}: {error}", log_path.display()))?;
    let recorder = Recorder::durable(log);
    let episode = recorder.enter(
        Start::new(
            "episode",
            Implementation::new(
                "episode",
                "ask",
                &json!({
                    "scope": options.scope.word(),
                    "executor": options.executor.word(),
                    "battery": "gym-v1",
                    "questions": "ask-relevance-v1",
                }),
            ),
        )
        .named("ask")
        .reading(&json!({ "question": options.question, "context": options.context })),
    );
    recorder.push(Step::said(Source::User, &options.question));
    progress.line(&format!("ask ▸ {id}: {}", clip(&options.question, 120)));

    let jev = match &options.jev {
        JevChoice::Off => JevMode::Off,
        JevChoice::Recorded(path) => JevMode::Recorded(Recorded::load(path)?),
        JevChoice::Live => {
            let dir = crate::credentials::openagents_dir().ok_or("HOME is not set")?;
            match crate::credentials::jev_key(|name| std::env::var(name).ok(), &dir) {
                Ok(key) => JevMode::Live(crate::credentials::jev_client(&key.secret)?),
                Err(why) => {
                    progress.line(&format!("jev ▸ off: {why}"));
                    JevMode::Off
                }
            }
        }
    };
    let reader = Reader {
        gym: gym.clone(),
        cwd: repo.clone(),
        boundary: Some(&boundary),
    };

    let mut gathered = match options.scope {
        Scope::Gym => {
            // The outcome matrix is the battery's slowest read and only the
            // briefing needs it, so it runs beside everything else.
            let (mut gathered, matrix) = tokio::join!(
                gather_gym(&options, &reader, &jev, &recorder, progress),
                reader.gym(strings(&["coder", "matrix", "--json"]))
            );
            gather::record_probes(&recorder, "outcome matrix", &[&matrix], progress);
            gathered.inputs.matrix = matrix.json();
            gathered
        }
        Scope::Repo => gather_repo(&options, &reader, &jev, &recorder, progress).await,
    };
    gathered.inputs.question.clone_from(&options.question);
    gathered.inputs.context = options.context.clone();
    if let Some(run) = &options.run {
        gathered
            .inputs
            .context
            .insert(0, format!("selected run: {run}"));
    }

    let briefing = brief::build(&gathered.inputs);
    let briefing_path = dir.join("briefing.md");
    std::fs::write(&briefing_path, &briefing)
        .map_err(|error| format!("cannot write {}: {error}", briefing_path.display()))?;
    let invocation = recorder.enter(
        Start::new(
            "ask.briefing",
            Implementation::new(
                "ask.briefing",
                "ask briefing v1",
                &json!({ "cap": brief::CAP }),
            ),
        )
        .named("briefing"),
    );
    recorder.end(
        &invocation,
        Finish::new(Outcome::Completed)
            .output(json!({ "chars": briefing.chars().count(), "sha256": atif::digest(&json!(briefing)), "opened": gathered.inputs.opened.len(), "candidates": gathered.candidates }))
            .cost(Cost::none()),
    );
    progress.line(&format!(
        "brief ▸ {} characters, {} runs opened of {} candidates",
        briefing.chars().count(),
        gathered.inputs.opened.len(),
        gathered.candidates
    ));

    let asked: Vec<&Asked> = gathered.asked.iter().collect();
    let jev_cost = jev_usd(&asked);
    let remaining = options.budget_usd - jev_cost;

    // The executor, or a replayed answer.
    let model = options
        .model
        .clone()
        .unwrap_or_else(|| options.executor.agent().default_model().to_string());
    let invocation = recorder.enter(
        Start::new(
            "ask.executor",
            Implementation::new(
                "ask.executor",
                &format!("{} {model}", options.executor.agent().word()),
                &json!({ "tools": ["read", "answer"], "allowlist": "ask-allowlist-v1", "deadline_sec": options.deadline.as_secs() }),
            ),
        )
        .named(options.executor.word())
        .with_effects(),
    );
    let (answer, executor_record, executor_cost, executor_status) = if let Some(file) =
        &options.answer_file
    {
        let answer: Value = serde_json::from_slice(
            &std::fs::read(file)
                .map_err(|error| format!("cannot read {}: {error}", file.display()))?,
        )
        .map_err(|error| format!("{} is not JSON: {error}", file.display()))?;
        progress.line(&format!("executor ▸ replayed from {}", file.display()));
        (
            Some(answer),
            json!({ "replayed_from": file.to_string_lossy() }),
            Cost {
                usd: Some(0.0),
                provenance: "recorded_replay".to_string(),
            },
            "replayed".to_string(),
        )
    } else if remaining <= 0.0 {
        progress.line("executor ▸ skipped: Jev spent the budget");
        (
            None,
            json!({ "skipped": "Jev spent the budget" }),
            Cost::none(),
            "skipped".to_string(),
        )
    } else {
        let env = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let (binary, credential) = crate::delegate::resolve(options.executor.agent(), env);
        let setup = executor::Setup {
            which: options.executor,
            model: model.clone(),
            binary,
            credential,
            coder_one: std::env::current_exe()
                .map_err(|error| format!("cannot find this binary: {error}"))?,
            gym: gym.clone().unwrap_or_else(|| PathBuf::from("gym")),
            cwd: repo.clone(),
            scratch: scratch.clone(),
            briefing: briefing_path.clone(),
            budget_usd: remaining,
            deadline: options.deadline,
            rank: options.rank,
            boundary: &boundary,
        };
        let ran = setup.run(progress).await;
        for (index, call) in ran.calls.iter().enumerate() {
            let mut extra = serde_json::Map::new();
            for key in ["exit", "bytes", "returned_chars", "refused", "reason"] {
                if let Some(value) = call.get(key) {
                    extra.insert(key.to_string(), value.clone());
                }
            }
            recorder.push(Step::called(Call {
                id: format!("{invocation}-call-{index}"),
                name: call["tool"].as_str().unwrap_or("?").to_string(),
                arguments: json!({ "command": call["command"] }),
                output: call["ending"].as_str().unwrap_or_default().to_string(),
                outcome: if call.get("refused").is_some()
                    || call["exit"].as_i64().is_some_and(|c| c != 0)
                {
                    CallOutcome::Failed
                } else {
                    CallOutcome::Completed
                },
                milliseconds: call["milliseconds"].as_u64().unwrap_or(0),
                purpose: None,
                extra,
            }));
        }
        let _ = std::fs::write(dir.join("stream.jsonl"), &ran.stream);
        let _ = std::fs::write(
            dir.join(tools::CALLS_FILE),
            ran.calls
                .iter()
                .map(|c| format!("{c}\n"))
                .collect::<String>(),
        );
        let usd = ran.summary.total_cost_usd;
        let provenance = ran.summary.cost_provenance.unwrap_or(if usd.is_some() {
            "cli_reported"
        } else {
            "unknown"
        });
        let record = json!({
            "agent": options.executor.agent().word(),
            "model": model,
            "reported_model": ran.summary.model,
            "credential": credential.word(),
            "status": ran.status.word(),
            "status_detail": ran.status.to_string(),
            "milliseconds": ran.milliseconds,
            "turns": ran.summary.num_turns,
            "usage": ran.summary.usage,
            "reads": ran.calls.iter().filter(|c| c["tool"] == "read").count(),
            "refused_reads": ran.calls.iter().filter(|c| c.get("refused").is_some()).count(),
            "stderr": clip(ran.stderr.trim(), 2_000),
            "args": ran.args,
            "boundary": {
                "backend": boundary.backend().display().to_string(),
                "writable": boundary.writable().iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            },
        });
        progress.line(&format!(
            "executor ▸ {} in {:.1}s, {}",
            ran.status,
            ran.milliseconds as f64 / 1000.0,
            usd.map_or("cost unknown".to_string(), |usd| format!(
                "${usd:.4} ({provenance})"
            ))
        ));
        let status = if ran.answer.is_some() {
            "answered".to_string()
        } else {
            ran.status.word().to_string()
        };
        (
            ran.answer,
            record,
            Cost {
                usd,
                provenance: provenance.to_string(),
            },
            status,
        )
    };
    if let Some(answer) = &answer {
        let _ = std::fs::write(
            dir.join(tools::ANSWER_FILE),
            serde_json::to_vec_pretty(answer).unwrap_or_default(),
        );
        recorder.push(Step::said(
            Source::Agent,
            answer["answer"].as_str().unwrap_or_default(),
        ));
    }
    recorder.end(
        &invocation,
        Finish::new(if answer.is_some() { Outcome::Completed } else { Outcome::Failed })
            .summary(json!({ "status": executor_status, "claims": answer.as_ref().and_then(|a| a["claims"].as_array()).map(Vec::len) }))
            .cost(executor_cost.clone()),
    );

    // The citation check.
    let invocation = recorder.enter(
        Start::new(
            "ask.citations",
            Implementation::new(
                "ask.citations",
                "citation check v1",
                &json!({ "reason_at": cite::REASON_AT }),
            ),
        )
        .named("citations")
        .effect("observe"),
    );
    let mut claims = answer.as_ref().map(cite::claims).unwrap_or_default();
    let cited = cite::cited_runs(&claims);
    let unknown: Vec<String> = cited
        .iter()
        .filter(|run| !gathered.shown.contains_key(*run))
        .cloned()
        .collect();
    let probes = join_all(
        unknown
            .iter()
            .map(|run| reader.gym(strings(&["runs", "show", run, "--json"]))),
    )
    .await;
    for (run, probe) in unknown.iter().zip(&probes) {
        if let Some(value) = probe.json() {
            gathered.shown.insert(run.clone(), value);
        }
    }
    if !probes.is_empty() {
        gather::record_probes(
            &recorder,
            "citation reads",
            &probes.iter().collect::<Vec<_>>(),
            progress,
        );
    }
    let facts: BTreeMap<String, Option<cite::RunFacts>> = cited
        .iter()
        .map(|run| {
            (
                run.clone(),
                gathered.shown.get(run).and_then(cite::RunFacts::from_show),
            )
        })
        .collect();
    cite::check(&mut claims, &facts, &repo);
    let totals = cite::totals(&claims);
    recorder.end(
        &invocation,
        Finish::new(Outcome::Completed)
            .output(json!({ "totals": totals, "claims": claims.iter().map(cite::Claim::to_json).collect::<Vec<_>>() }))
            .cost(Cost::none()),
    );
    if answer.is_some() {
        progress.line(&format!(
            "cite ▸ {} of {} citations check; {} of {} claims verified",
            totals["valid_citations"], totals["citations"], totals["verified"], totals["claims"]
        ));
    }

    let total_usd = executor_cost.usd.map(|usd| usd + jev_cost);
    let milliseconds = millis(started);
    recorder.end(
        &episode,
        Finish::new(if answer.is_some() {
            Outcome::Completed
        } else {
            Outcome::Failed
        })
        .summary(json!({ "status": executor_status, "citations": totals })),
    );
    recorder.finish(atif::log::ENDED);
    if let Some(path) = &options.jev_record {
        let mut recorded = Recorded::load(path)?;
        crate::component::jev::record_answers(&recorder.steps(), &id, &mut recorded);
        recorded.save(path)?;
    }

    let record = json!({
        "schema": SCHEMA,
        "id": id,
        "question": options.question,
        "scope": options.scope.word(),
        "context": gathered.inputs.context,
        "executor": options.executor.word(),
        "status": executor_status,
        "answer": answer.as_ref().map(|a| a["answer"].clone()),
        "proposed_change": answer.as_ref().and_then(|a| a["proposed_change"].as_str()).filter(|s| !s.trim().is_empty()),
        "claims": claims.iter().map(cite::Claim::to_json).collect::<Vec<_>>(),
        "citations": totals,
        "cost": {
            "usd": total_usd,
            "jev_usd": jev_cost,
            "jev_requests": gathered.asked.iter().filter(|a| a.how == "live").count(),
            "jev_mode": jev.word(),
            "executor_usd": executor_cost.usd,
            "executor_provenance": executor_cost.provenance,
            "budget_usd": options.budget_usd,
            "over_budget": total_usd.is_some_and(|usd| usd > options.budget_usd),
        },
        "milliseconds": milliseconds,
        "evidence": {
            "candidates": gathered.candidates,
            "opened": gathered.inputs.opened.iter().map(|o| json!({"run": o.id, "relevance": o.relevance, "steps": o.steps.iter().map(|s| s.0).collect::<Vec<_>>()})).collect::<Vec<_>>(),
            "reasons": gathered.inputs.asked_reasons.iter().map(|(id, p)| json!({"id": id, "probability": p})).collect::<Vec<_>>(),
            "tasks": gathered.inputs.tasks,
            "files": gathered.inputs.files.iter().map(|(path, p, _)| json!({"path": path, "relevance": p})).collect::<Vec<_>>(),
        },
        "executor_record": executor_record,
        "started_at": atif::document::iso(at),
        "version": crate::episode::version(),
        "dir": dir.to_string_lossy(),
        "files": {
            "invocation_log": crate::episode::INVOCATION_LOG,
            "briefing": "briefing.md",
            "stream": "stream.jsonl",
            "calls": tools::CALLS_FILE,
            "answer": tools::ANSWER_FILE,
        },
    });
    crate::record::write_atomic(
        &dir.join("manifest.json"),
        serde_json::to_string_pretty(&record)
            .map_err(|error| error.to_string())?
            .as_bytes(),
    )?;
    drop(boundary);
    Ok((record, i32::from(answer.is_none())))
}

/// The Gym battery, Jev's relevance judgments, and the opened runs.
async fn gather_gym(
    options: &Options,
    reader: &Reader<'_>,
    jev: &JevMode,
    recorder: &Recorder,
    progress: &Progress,
) -> Gathered {
    let mut gathered = Gathered::default();
    let mut battery = vec![
        strings(&["runs", "--order", "learning", "--json", "--limit", "40"]),
        strings(&["runs", "group", "--by", "reason", "--json"]),
        strings(&["runs", "group", "--by", "task", "--json"]),
    ];
    if let Some(run) = &options.run {
        battery.push(strings(&["runs", "show", run, "--json"]));
    }
    let probes: Vec<Probe> = join_all(battery.iter().map(|args| reader.gym(args.clone()))).await;
    gather::record_probes(
        recorder,
        "gym battery",
        &probes.iter().collect::<Vec<_>>(),
        progress,
    );
    let learning = probes[0].json().unwrap_or(Value::Null);
    let reasons = probes[1].json();
    let tasks = probes[2].json().unwrap_or(Value::Null);
    gathered.inputs.totals = learning.is_object().then(|| {
        json!({ "total": learning["total"], "ranked": learning["ranked"], "running": learning["running"] })
    });
    let selected = options
        .run
        .as_ref()
        .and_then(|_| probes.get(3))
        .and_then(Probe::json);

    // Which reasons and tasks the question names.
    let groups = reasons
        .as_ref()
        .map(gather::reason_groups)
        .unwrap_or_default();
    let mut asked_reasons = Vec::new();
    if !groups.is_empty() {
        let state = json!({
            "question": options.question,
            "context": options.context,
            "reasons": groups.iter().map(|(id, tag, count)| json!({"id": id, "tag": tag, "runs": count})).collect::<Vec<_>>(),
        });
        let asked = gather::judge(
            jev,
            recorder,
            "ask_reasons",
            state,
            gather::reason_questions(groups.len()),
        )
        .await;
        for (index, p) in gather::chosen(&asked, "reason", groups.len(), gather::MAX_REASONS, 0) {
            asked_reasons.push((groups[index].0.clone(), p));
        }
        progress.line(&format!(
            "jev ▸ reasons the question asks about: {}",
            if asked_reasons.is_empty() {
                "none".to_string()
            } else {
                asked_reasons
                    .iter()
                    .map(|(id, p)| {
                        format!("{id} {}", p.map_or("?".to_string(), |p| format!("{p:.2}")))
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ));
        gathered.asked.push(asked);
    }
    let named = gather::named_tasks(&options.question, &tasks);
    let mut extra: Vec<Vec<String>> = Vec::new();
    for task in &named {
        extra.push(strings(&[
            "runs", "--order", "learning", "--search", task, "--json", "--limit", "12",
        ]));
    }
    for (reason, _) in &asked_reasons {
        extra.push(strings(&[
            "runs", "--order", "learning", "--reason", reason, "--json", "--limit", "15",
        ]));
    }
    let extra_probes: Vec<Probe> =
        join_all(extra.iter().map(|args| reader.gym(args.clone()))).await;
    if !extra_probes.is_empty() {
        gather::record_probes(
            recorder,
            "question probes",
            &extra_probes.iter().collect::<Vec<_>>(),
            progress,
        );
    }

    // The candidates: the selected run, the named tasks' runs, the
    // reasons' runs, then the learning order.
    let mut seen = HashSet::new();
    let mut candidates: Vec<Value> = Vec::new();
    if let Some(shown) = &selected {
        let mut entry = shown["run"].clone();
        entry["learning"] = shown["learning"].clone();
        let summary = gather::run_summary(&entry);
        seen.insert(summary["run"].as_str().unwrap_or_default().to_string());
        candidates.push(summary);
    }
    for list in extra_probes
        .iter()
        .filter_map(Probe::json)
        .chain(std::iter::once(learning.clone()))
    {
        for run in list["runs"].as_array().into_iter().flatten() {
            let summary = gather::run_summary(run);
            let id = summary["run"].as_str().unwrap_or_default().to_string();
            if seen.insert(id) {
                candidates.push(summary);
            }
        }
    }
    candidates.truncate(gather::MAX_CANDIDATES);
    gathered.candidates = candidates.len();

    let mut picked: Vec<(usize, Option<f64>)> = Vec::new();
    let mut relevance = BTreeMap::new();
    if !candidates.is_empty() {
        let state =
            json!({ "question": options.question, "context": options.context, "runs": candidates });
        let asked = gather::judge(
            jev,
            recorder,
            "ask_runs",
            state,
            gather::run_questions(candidates.len()),
        )
        .await;
        picked = gather::chosen(&asked, "run", candidates.len(), gather::MAX_OPENED, 4);
        relevance = gather::probabilities(&asked, "run", candidates.len());
        if picked.len() < gather::MIN_OPENED && asked.answered() {
            let mut rest: Vec<(usize, f64)> = relevance
                .iter()
                .filter(|(i, _)| !picked.iter().any(|(j, _)| j == *i))
                .map(|(i, p)| (*i, *p))
                .collect();
            rest.sort_by(|a, b| b.1.total_cmp(&a.1));
            for (i, p) in rest.into_iter().take(gather::MIN_OPENED - picked.len()) {
                picked.push((i, Some(p)));
            }
        }
        if selected.is_some() && !picked.iter().any(|(i, _)| *i == 0) {
            picked.insert(0, (0, relevance.get(&0).copied()));
            picked.truncate(gather::MAX_OPENED);
        }
        progress.line(&format!(
            "jev ▸ {} of {} candidate runs bear on the question{}",
            relevance.values().filter(|p| **p >= gather::YES).count(),
            candidates.len(),
            if asked.answered() {
                String::new()
            } else {
                format!(
                    "; Jev didn't answer ({}), so code's order picks",
                    asked.error.as_deref().unwrap_or("off")
                )
            }
        ));
        gathered.asked.push(asked);
    }

    // Open the picked runs.
    let ids: Vec<String> = picked
        .iter()
        .map(|(i, _)| {
            candidates[*i]["run"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    let mut reads = Vec::new();
    for id in &ids {
        reads.push(strings(&["runs", "show", id, "--json"]));
        reads.push(strings(&["runs", "show", id, "--evidence"]));
    }
    let opened_probes: Vec<Probe> =
        join_all(reads.iter().map(|args| reader.gym(args.clone()))).await;
    if !opened_probes.is_empty() {
        gather::record_probes(
            recorder,
            "opened runs",
            &opened_probes.iter().collect::<Vec<_>>(),
            progress,
        );
    }
    let mut opened = Vec::new();
    for (k, (index, p)) in picked.iter().enumerate() {
        let Some(shown) = opened_probes[2 * k].json() else {
            continue;
        };
        gathered.shown.insert(ids[k].clone(), shown.clone());
        opened.push(brief::Opened {
            id: ids[k].clone(),
            relevance: p.or_else(|| relevance.get(index).copied()),
            shown,
            evidence: opened_probes[2 * k + 1].json(),
            steps: Vec::new(),
        });
    }

    // Which transcript steps bear on the question.
    if !opened.is_empty() {
        let keep = (gather::MAX_STEPS / opened.len()).max(3);
        let steps: Vec<gather::StepText> = opened
            .iter()
            .flat_map(|o| gather::steps_of(&o.id, &o.shown, keep))
            .collect();
        if !steps.is_empty() {
            let state = json!({
                "question": options.question,
                "steps": steps.iter().map(|s| json!({
                    "run": s.run,
                    "step": s.step,
                    "headline": clip(&s.headline, 200),
                    "text": clip(&s.body, 500),
                })).collect::<Vec<_>>(),
            });
            let asked = gather::judge(
                jev,
                recorder,
                "ask_steps",
                state,
                gather::step_questions(steps.len()),
            )
            .await;
            let probabilities = gather::probabilities(&asked, "step", steps.len());
            for o in &mut opened {
                let mut mine: Vec<(usize, Option<f64>)> = steps
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.run == o.id)
                    .map(|(i, _)| (i, probabilities.get(&i).copied()))
                    .collect();
                if asked.answered() {
                    mine.retain(|(_, p)| p.is_some_and(|p| p >= gather::YES));
                    mine.sort_by(|a, b| b.1.unwrap_or(0.0).total_cmp(&a.1.unwrap_or(0.0)));
                    mine.truncate(gather::STEPS_PER_RUN);
                } else {
                    let skip = mine.len().saturating_sub(3);
                    mine = mine.into_iter().skip(skip).collect();
                }
                mine.sort_by_key(|(i, _)| steps[*i].step);
                o.steps = mine
                    .into_iter()
                    .map(|(i, p)| {
                        (
                            steps[i].step,
                            steps[i].headline.clone(),
                            steps[i].body.clone(),
                            p,
                        )
                    })
                    .collect();
            }
            progress.line(&format!(
                "jev ▸ {} of {} transcript steps bear on the question",
                probabilities
                    .values()
                    .filter(|p| **p >= gather::YES)
                    .count(),
                steps.len()
            ));
            gathered.asked.push(asked);
        }
    }

    gathered.inputs.others = candidates
        .iter()
        .enumerate()
        .filter(|(i, _)| !picked.iter().any(|(j, _)| j == i))
        .map(|(i, c)| (c.clone(), relevance.get(&i).copied()))
        .collect();
    gathered.inputs.opened = opened;
    gathered.inputs.reasons = reasons;
    gathered.inputs.asked_reasons = asked_reasons;
    gathered.inputs.tasks = named;
    gathered
}

/// A repository question: search the Markdown for the question's words,
/// let Jev pick the files, and quote them.
async fn gather_repo(
    options: &Options,
    reader: &Reader<'_>,
    jev: &JevMode,
    recorder: &Recorder,
    progress: &Progress,
) -> Gathered {
    let mut gathered = Gathered::default();
    let words = gather::keywords(&options.question);
    let Some(rg) = allow::which("rg") else {
        progress.line("probe ▸ rg is not on PATH; the briefing has no files");
        return gathered;
    };
    if words.is_empty() {
        return gathered;
    }
    let mut args = strings(&["-c", "-i", "-F", "-g", "*.md"]);
    for word in &words {
        args.push("-e".to_string());
        args.push(word.clone());
    }
    args.push(".".to_string());
    let search = reader.run(&rg, "rg", args).await;
    gather::record_probes(recorder, "repository search", &[&search], progress);
    let mut files: Vec<(String, u64)> = search
        .stdout
        .lines()
        .filter_map(|line| {
            let (path, count) = line.rsplit_once(':')?;
            Some((
                path.trim_start_matches("./").to_string(),
                count.parse().ok()?,
            ))
        })
        .filter(|(path, _)| !path.starts_with("docs/transcripts/"))
        .collect();
    files.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    files.truncate(gather::MAX_FILES);
    gathered.candidates = files.len();
    let excerpt = |path: &str, context: &str, most: &str| {
        let mut args = strings(&["-n", "-i", "-F", "-m", most, "-C", context]);
        for word in &words {
            args.push("-e".to_string());
            args.push(word.clone());
        }
        args.push(path.to_string());
        args
    };
    let heads: Vec<Probe> = join_all(
        files
            .iter()
            .map(|(path, _)| reader.run(&rg, "rg", excerpt(path, "0", "3"))),
    )
    .await;
    let state = json!({
        "question": options.question,
        "files": files.iter().zip(&heads).map(|((path, count), head)| json!({"path": path, "matching_lines": count, "first_matches": clip(&head.stdout, 400)})).collect::<Vec<_>>(),
    });
    let asked = gather::judge(
        jev,
        recorder,
        "ask_files",
        state,
        gather::file_questions(files.len()),
    )
    .await;
    let picked = gather::chosen(&asked, "file", files.len(), gather::OPENED_FILES, 3);
    let quotes: Vec<Probe> = join_all(
        picked
            .iter()
            .map(|(i, _)| reader.run(&rg, "rg", excerpt(&files[*i].0, "2", "12"))),
    )
    .await;
    gather::record_probes(
        recorder,
        "repository excerpts",
        &quotes.iter().collect::<Vec<_>>(),
        progress,
    );
    progress.line(&format!(
        "jev ▸ {} of {} files bear on the question",
        picked.len(),
        files.len()
    ));
    gathered.inputs.files = picked
        .iter()
        .zip(&quotes)
        .map(|((i, p), quote)| (files[*i].0.clone(), *p, quote.stdout.clone()))
        .collect();
    gathered.asked.push(asked);
    gathered
}

/// The record as text: the answer, each claim with its citations and a
/// mark, the check's totals, the cost, and where it's recorded.
#[must_use]
pub fn text(record: &Value) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Question: {}\n\n",
        record["question"].as_str().unwrap_or("")
    ));
    match record["answer"].as_str() {
        Some(answer) => {
            out.push_str(answer.trim());
            out.push_str("\n\n");
        }
        None => out.push_str(&format!(
            "No answer: the executor {}. {}\n\n",
            record["status"].as_str().unwrap_or("didn't answer"),
            record["executor_record"]["status_detail"]
                .as_str()
                .unwrap_or("")
        )),
    }
    let claims = record["claims"].as_array().cloned().unwrap_or_default();
    if !claims.is_empty() {
        out.push_str("Claims (✓ citations checked, ? unverified):\n");
        for (index, claim) in claims.iter().enumerate() {
            let mark = if claim["verified"] == true {
                '✓'
            } else {
                '?'
            };
            out.push_str(&format!(
                "{mark} {}. {}\n",
                index + 1,
                claim["claim"].as_str().unwrap_or("")
            ));
            let mut cites: Vec<String> = claim["runs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            cites.extend(
                claim["steps"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|s| format!("{} step {}", s["run"].as_str().unwrap_or("?"), s["step"])),
            );
            cites.extend(
                claim["judgments"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string),
            );
            cites.extend(
                claim["files"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string),
            );
            if !cites.is_empty() {
                out.push_str(&format!("     cites: {}\n", cites.join("; ")));
            }
            for problem in claim["problems"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                out.push_str(&format!("     unverified: {problem}\n"));
            }
        }
        out.push('\n');
    }
    if let Some(change) = record["proposed_change"].as_str() {
        out.push_str(&format!(
            "Proposed change, for a person to decide on: {change}\n\n"
        ));
    }
    let citations = &record["citations"];
    let cost = &record["cost"];
    out.push_str(&format!(
        "{} of {} citations check; {} of {} claims verified.\n",
        citations["valid_citations"],
        citations["citations"],
        citations["verified"],
        citations["claims"]
    ));
    out.push_str(&format!(
        "Cost {}: executor {} ({}), Jev ${:.4} over {} requests. Time {:.1}s.\n",
        cost["usd"]
            .as_f64()
            .map_or("unknown".to_string(), |usd| format!("${usd:.4}")),
        cost["executor_usd"]
            .as_f64()
            .map_or("unknown".to_string(), |usd| format!("${usd:.4}")),
        cost["executor_provenance"].as_str().unwrap_or("?"),
        cost["jev_usd"].as_f64().unwrap_or(0.0),
        cost["jev_requests"],
        record["milliseconds"].as_f64().unwrap_or(0.0) / 1000.0
    ));
    if cost["over_budget"] == true {
        out.push_str(&format!("Over the ${} budget.\n", cost["budget_usd"]));
    }
    out.push_str(&format!(
        "Recorded: {}\n",
        record["dir"].as_str().unwrap_or("")
    ));
    out
}

/// `coder-one ask …`: the exit code, 0 when it answered.
///
/// # Errors
///
/// Returns a message when the arguments don't parse or the ask can't
/// start.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let options = Options::parse(args)?;
    let progress = Progress {
        events: options.events,
        quiet: options.json,
    };
    let (json_out, events) = (options.json, options.events);
    let (record, code) = run(options, &progress).await?;
    if events {
        println!("{}", json!({ "event": "answer", "record": record }));
    } else if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&record).map_err(|error| error.to_string())?
        );
    } else {
        print!("{}", text(&record));
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_options_parse_and_refuse_what_they_should() {
        let parse = |args: &[&str]| Options::parse(&strings(args));
        let options = parse(&[
            "why",
            "did",
            "it",
            "fail?",
            "--executor",
            "opus",
            "--budget",
            "$0.25",
            "--context",
            "filter: failed",
        ])
        .unwrap();
        assert_eq!(options.question, "why did it fail?");
        assert_eq!(options.executor, Which::Opus);
        assert!((options.budget_usd - 0.25).abs() < 1e-9);
        assert_eq!(options.context, vec!["filter: failed"]);
        assert_eq!(options.scope, Scope::Gym);
        assert!(parse(&[]).is_err());
        assert!(parse(&["q", "--executor", "gpt"]).is_err());
        assert!(parse(&["q", "--budget", "0"]).is_err());
        assert!(parse(&["q", "--scope", "web"]).is_err());
        assert!(parse(&["q", "--frobnicate"]).is_err());
        // An executor capability appends the task after `--`, and
        // everything there is the question.
        let options = parse(&["--scope", "gym", "--", "why --json?", "--no-jev"]).unwrap();
        assert_eq!(options.question, "why --json? --no-jev");
        assert_eq!(options.jev, JevChoice::Live);
    }

    /// A stand-in `gym` that prints fixed JSON for the reads an ask makes.
    fn fake_gym(dir: &Path) -> PathBuf {
        let run = json!({
            "job": "tb4--coder-one-tunable-v6--demo", "trial": "demo__1", "task": "demo",
            "agent": "Coder One", "variant": "tunable-v6", "outcome": "failed",
            "tests": {"passed": 1, "failed": 3, "total": 4}, "cost_usd": 0.4,
            "learning": {"learning": 0.8, "reasons": [{"id": "unearned_success", "probability": 0.96}]},
        });
        let other = json!({
            "job": "tb4--claude-code-opus--demo", "trial": "demo__2", "task": "demo",
            "agent": "Claude Code", "model": "Opus 5.5", "outcome": "passed",
            "learning": {"learning": 0.2, "reasons": []},
        });
        let files = [
            (
                "learning.json",
                json!({"total": 2, "ranked": 2, "running": 0, "runs": [run, other]}),
            ),
            (
                "reasons.json",
                json!({"groups": [{"key": "unearned_success", "tag": "claimed unearned success", "count": 1, "mean_probability": [{"id": "unearned_success", "mean": 0.96}], "members": []}]}),
            ),
            (
                "tasks.json",
                json!({"groups": [{"key": "demo", "count": 2}]}),
            ),
            (
                "show.json",
                json!({
                    "run": run,
                    "summary": [{"heading": "What happened", "text": "It said all tests pass."}],
                    "transcript": [{"step": 1, "headline": "Read the task", "body": "…"}, {"step": 2, "headline": "Report", "body": "All tests pass."}],
                    "learning": {"learning": 0.8, "value": 2.0, "judgments": {"unearned_success": 0.96, "near_miss": 0.1}, "every_judgment": [{"id": "unearned_success", "probability": 0.96, "reason": true}]},
                }),
            ),
            (
                "evidence.json",
                json!({"key": "k", "state": {"run": {"outcome": "failed"}}}),
            ),
            (
                "matrix.json",
                json!({"tasks": ["demo"], "cells": [], "complete_policies": []}),
            ),
        ];
        for (name, value) in files {
            std::fs::write(dir.join(name), value.to_string()).unwrap();
        }
        let show = "runs show tb4--coder-one-tunable-v6--demo/demo__1";
        let script = format!(
            "#!/bin/sh\nD='{}'\ncase \"$*\" in\n\
             'runs --order learning --json --limit 40') cat \"$D/learning.json\" ;;\n\
             'runs group --by reason --json') cat \"$D/reasons.json\" ;;\n\
             'runs group --by task --json') cat \"$D/tasks.json\" ;;\n\
             'coder matrix --json') cat \"$D/matrix.json\" ;;\n\
             '{show} --json') cat \"$D/show.json\" ;;\n\
             '{show} --evidence') cat \"$D/evidence.json\" ;;\n\
             'runs show '*) echo 'no run matches' >&2; exit 1 ;;\n\
             *) cat \"$D/learning.json\" ;;\n\
             esac\n",
            dir.display()
        );
        let path = dir.join("gym");
        std::fs::write(&path, script).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    #[test]
    fn an_ask_probes_briefs_replays_an_answer_and_checks_its_citations() {
        if coder_boundary::Boundary::readonly().build().is_err() {
            // No enforced boundary on this host: an ask refuses to run.
            return;
        }
        let fixtures = tempfile::tempdir().unwrap();
        let gym = fake_gym(fixtures.path());
        let out = tempfile::tempdir().unwrap();
        let demo = "tb4--coder-one-tunable-v6--demo/demo__1";
        let answer = fixtures.path().join("answer.json");
        let claims = json!([
            {"claim": "It reported that all tests pass at step 2.", "runs": [demo], "steps": [{"run": demo, "step": 2}], "judgments": ["unearned_success"], "files": []},
            {"claim": "A run that doesn't exist agrees.", "runs": ["tb4--nowhere/x__1"], "steps": [], "judgments": [], "files": []},
            {"claim": "It was a near miss.", "runs": [demo], "steps": [], "judgments": ["near_miss"], "files": []},
        ]);
        let replayed = json!({
            "answer": "The demo run claimed success it didn't earn.",
            "claims": claims,
            "proposed_change": "",
        });
        std::fs::write(&answer, replayed.to_string()).unwrap();
        let mut options = Options::parse(&strings(&[
            "why", "did", "demo", "claim", "unearned", "success?", "--no-jev", "--run", demo,
        ]))
        .unwrap();
        options.gym = Some(gym);
        options.out = Some(out.path().to_path_buf());
        options.repo = Some(fixtures.path().to_path_buf());
        options.answer_file = Some(answer);
        let progress = Progress {
            events: false,
            quiet: true,
        };
        let (record, code) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(run(options, &progress))
            .unwrap();
        assert_eq!(code, 0, "{record}");
        assert_eq!(record["status"], "replayed");
        assert_eq!(record["evidence"]["tasks"], json!(["demo"]));
        assert_eq!(record["evidence"]["opened"][0]["run"], demo);
        let verified: Vec<bool> = record["claims"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["verified"] == true)
            .collect();
        // Unverified claims are kept, not dropped.
        assert_eq!(verified, vec![true, false, false], "{record}");
        assert_eq!(record["citations"]["citations"], 6);
        assert_eq!(record["citations"]["valid_citations"], 4);
        assert_eq!(record["cost"]["jev_mode"], "off");

        let dir = PathBuf::from(record["dir"].as_str().unwrap());
        let briefing = std::fs::read_to_string(dir.join("briefing.md")).unwrap();
        assert!(
            briefing.contains(&format!("selected run: {demo}")),
            "{briefing}"
        );
        assert!(briefing.contains("step 2"), "{briefing}");
        assert!(dir.join("manifest.json").is_file());
        let log = std::fs::read_to_string(dir.join(crate::episode::INVOCATION_LOG)).unwrap();
        for component in [
            "\"ask.probes\"",
            "\"ask.relevance\"",
            "\"ask.briefing\"",
            "\"ask.executor\"",
            "\"ask.citations\"",
        ] {
            assert!(log.contains(component), "{component}");
        }
        let text = text(&record);
        assert!(text.contains("✓ 1."), "{text}");
        assert!(
            text.contains("unverified: the Gym has no run tb4--nowhere/x__1"),
            "{text}"
        );
        assert!(text.contains("4 of 6 citations check"), "{text}");
    }

    #[test]
    fn clipping_keeps_both_ends() {
        let text: String = (0..100)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let clipped = clip_middle(&text, 40);
        assert!(clipped.starts_with(&text[..30]));
        assert!(clipped.ends_with(&text[90..]));
        assert!(clipped.contains("60 characters dropped"));
        assert_eq!(clip("abcdef", 3), "abc…");
    }
}
