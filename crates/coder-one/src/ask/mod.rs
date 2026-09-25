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
pub mod drafts;
pub mod executor;
pub mod gather;
pub mod study;
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
usage: coder-one ask \"QUESTION\" [--scope gym|repo|highlights] [--claim KEY]...
                     [--executor luna|opus] [--budget USD]
                     [--json | --events] [--run RUN] [--context TEXT]... [--rank]
                     [--model MODEL] [--timeout SECONDS] [--gym PATH] [--repo DIR]
                     [--out DIR] [--no-jev] [--jev-recorded FILE] [--jev-record FILE]
                     [--answer-file FILE] [--proposals DIR | --no-proposals]

Answers a question about Terminal-Bench runs by reading the Gym, with every
claim's citations checked by code. It only reads: each command runs inside a
filesystem boundary that leaves only the ask's scratch directory and the
executor CLI's own state writable.

--scope gym (the default) probes the Gym's runs, reason groups, marks, and
outcome matrix; --scope repo searches the repository's Markdown instead.
--scope highlights drafts short text from the claims `gym runs highlights`
computes: the ones each --claim KEY names, or the 3 strongest. Code refuses a
draft with a number its claim doesn't give, a run its claim doesn't cite, or
an n=1 claim it doesn't say rests on one run. Nothing posts anywhere, and the
question may be empty. --executor
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
executor's answer instead of running one.

The answer may carry typed proposals: a policy or check merge patch on a
checked-in manifest, a question-set change, a new mini-task, or a code change.
Code validates each and writes it under ~/.openagents/coder-one/proposals
unless --proposals names another directory (--no-proposals writes none).
Nothing runs until a person approves one with `gym coder proposals approve
ID`; then `coder-one proposal run ID` measures it.";

/// Where the question is answered from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Gym,
    Repo,
    /// Drafts from the Gym's highlights.
    Highlights,
}

impl Scope {
    fn parse(text: &str) -> Result<Self, String> {
        match text.trim() {
            "gym" => Ok(Scope::Gym),
            "repo" => Ok(Scope::Repo),
            "highlights" => Ok(Scope::Highlights),
            other => Err(format!(
                "--scope takes gym, repo, or highlights, not {other}"
            )),
        }
    }

    fn word(self) -> &'static str {
        match self {
            Scope::Gym => "gym",
            Scope::Repo => "repo",
            Scope::Highlights => "highlights",
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
    /// For a highlights ask, the highlights to draft, by key.
    pub claims: Vec<String>,
    /// Where the answer's proposals are written; `None` is
    /// `~/.openagents/coder-one/proposals`.
    pub proposals: Option<PathBuf>,
    /// Whether the answer's proposals are validated and written at all.
    pub record_proposals: bool,
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
            claims: Vec::new(),
            proposals: None,
            record_proposals: true,
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
                "--claim" => options.claims.push(value("--claim")?),
                "--gym" => options.gym = Some(PathBuf::from(value("--gym")?)),
                "--repo" => options.repo = Some(PathBuf::from(value("--repo")?)),
                "--out" => options.out = Some(PathBuf::from(value("--out")?)),
                "--proposals" => options.proposals = Some(PathBuf::from(value("--proposals")?)),
                "--no-proposals" => options.record_proposals = false,
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
        if options.question.is_empty() && options.scope == Scope::Highlights {
            options.question = "Draft short posts from these highlights.".to_string();
        }
        if !options.claims.is_empty() && options.scope != Scope::Highlights {
            return Err("--claim names a highlight; it needs --scope highlights".to_string());
        }
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
        // An empty float sum is -0.0, which prints as `$-0.0000`.
        .sum::<f64>()
        + 0.0
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
            "record ▸ can't write to {} ({error}), so this ask is recorded under {}",
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
    if options.scope != Scope::Repo && gym.is_none() {
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
        Scope::Highlights => gather_highlights(&options, &reader, &recorder, progress).await?,
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
        "brief ▸ the briefing is {} characters, from {} of {} candidate runs",
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
        progress.line("executor ▸ skipped: Jev used up the budget");
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
    if options.scope == Scope::Highlights {
        drafts::check(&mut claims, &gathered.inputs.highlights);
    }
    let totals = cite::totals(&claims);
    recorder.end(
        &invocation,
        Finish::new(Outcome::Completed)
            .output(json!({ "totals": totals, "claims": claims.iter().map(cite::Claim::to_json).collect::<Vec<_>>() }))
            .cost(Cost::none()),
    );
    if answer.is_some() {
        progress.line(&if options.scope == Scope::Highlights {
            format!(
                "drafts ▸ {} of {} drafts pass the number and citation checks, and {} are refused",
                totals["verified"], totals["claims"], totals["unverified"]
            )
        } else {
            format!(
                "cite ▸ {} of {} citations check; {} of {} claims verified",
                totals["valid_citations"],
                totals["citations"],
                totals["verified"],
                totals["claims"]
            )
        });
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

    let proposals = if options.record_proposals && options.scope != Scope::Highlights {
        match &answer {
            Some(answer) if !crate::proposal::drafts(answer).is_empty() => record_proposals(
                answer,
                &options,
                (&id, &dir, &repo),
                &facts,
                &gathered.shown,
                progress,
            ),
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };

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
        "proposals": proposals.iter().map(|p| json!({
            "id": p["id"],
            "kind": p["kind"],
            "title": p["title"],
            "valid": p["valid"],
            "problems": p["problems"],
            "needs_code": p["needs_code"],
            "digest": p["digest"],
            "dir": p["dir"],
        })).collect::<Vec<_>>(),
        "highlights": gathered.inputs.highlights.iter().map(|h| h["key"].clone()).collect::<Vec<_>>(),
        "drafts": if options.scope == Scope::Highlights {
            json!(claims.iter().map(|c| json!({
                "highlight": c.highlight,
                "draft": c.text,
                "runs": c.runs,
                "status": if c.verified() { "passed" } else { "refused" },
                "problems": c.problems,
            })).collect::<Vec<_>>())
        } else {
            Value::Null
        },
        "citations": totals,
        "cited_runs": facts.iter().map(|(cited, fact)| json!({
            "cited": cited,
            "run": fact.as_ref().map(|f| f.id.clone()),
            "task": fact.as_ref().map(|f| f.task.clone()),
        })).collect::<Vec<_>>(),
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
            "marks": gathered.inputs.marks.as_ref().and_then(|m| m["marks"].as_array()).map_or(0, Vec::len),
            "about_marks": gathered.inputs.about_marks,
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
        strings(&["runs", "marks", "--json"]),
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
    let marks = probes[3]
        .json()
        .filter(|marks| marks["marks"].as_array().is_some_and(|m| !m.is_empty()));
    gathered.inputs.totals = learning.is_object().then(|| {
        json!({ "total": learning["total"], "ranked": learning["ranked"], "running": learning["running"] })
    });
    let selected = options
        .run
        .as_ref()
        .and_then(|_| probes.get(4))
        .and_then(Probe::json);

    // Which reasons and tasks the question names.
    let groups = reasons
        .as_ref()
        .map(gather::reason_groups)
        .unwrap_or_default();
    let mut asked_reasons = Vec::new();
    let mut about_marks = false;
    if !groups.is_empty() || marks.is_some() {
        let mut state = json!({
            "question": options.question,
            "context": options.context,
            "reasons": groups.iter().map(|(id, tag, count)| json!({"id": id, "tag": tag, "runs": count})).collect::<Vec<_>>(),
        });
        let mut questions = gather::reason_questions(groups.len());
        if let Some(marks) = &marks {
            let all = marks["marks"].as_array().cloned().unwrap_or_default();
            let bad = all.iter().filter(|m| m["verdict"] == "bad").count();
            let mut tags: Vec<&str> = all
                .iter()
                .flat_map(|m| {
                    m["tags"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                })
                .collect();
            tags.sort_unstable();
            tags.dedup();
            state["marks"] = json!({
                "marked_bad": bad,
                "cleared": all.len() - bad,
                "tags": tags,
            });
            questions = questions.with("marks", gather::marks_noul());
        }
        let asked = gather::judge(jev, recorder, "ask_reasons", state, questions).await;
        if marks.is_some() {
            about_marks = match asked.noul("marks") {
                Some(p) => crate::decision::ASK_GATHER_YES.yes(p),
                None => gather::names_marks(&options.question),
            };
            progress.line(&format!(
                "jev ▸ the question {} about marked runs{}",
                if about_marks { "asks" } else { "doesn't ask" },
                asked.noul("marks").map_or(
                    " (by its words; Jev didn't answer)".to_string(),
                    |p| format!(" ({p:.2})")
                )
            ));
        }
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
    // The marked runs come first when the question asks about them.
    if about_marks {
        extra.insert(0, strings(&["runs", "--marked", "--json", "--limit", "30"]));
    }
    let extra_probes: Vec<Probe> =
        join_all(extra.iter().map(|args| reader.gym(args.clone()))).await;
    // Strategy fingerprints for the named tasks, and the candidate moves
    // when the question asks about strategies. Neither asks Jev anything.
    let mut strategy: Vec<Vec<String>> = named
        .iter()
        .map(|task| strings(&["runs", "fingerprints", "--task", task, "--json", "--no-jev"]))
        .collect();
    if gather::asks_strategy(&options.question) {
        strategy.push(strings(&["runs", "moves", "--cached", "--json"]));
    }
    let strategy_probes: Vec<Probe> =
        join_all(strategy.iter().map(|args| reader.gym(args.clone()))).await;
    if !strategy_probes.is_empty() {
        gather::record_probes(
            recorder,
            "strategy probes",
            &strategy_probes.iter().collect::<Vec<_>>(),
            progress,
        );
    }
    gathered.inputs.strategy = strategy_probes.iter().filter_map(Probe::json).collect();
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
        // A question about marked runs opens them, bad before cleared.
        if about_marks && let Some(marks) = &marks {
            let marked: Vec<usize> = gather::marked_runs(marks)
                .iter()
                .filter_map(|run| candidates.iter().position(|c| c["run"] == run.as_str()))
                .take(gather::MAX_OPENED)
                .collect();
            for (at, index) in marked.into_iter().enumerate() {
                picked.retain(|(i, _)| *i != index);
                picked.insert(at, (index, relevance.get(&index).copied()));
            }
            picked.truncate(gather::MAX_OPENED);
        }
        if selected.is_some() && !picked.iter().any(|(i, _)| *i == 0) {
            picked.insert(0, (0, relevance.get(&0).copied()));
            picked.truncate(gather::MAX_OPENED);
        }
        progress.line(&format!(
            "jev ▸ {} of {} candidate runs are relevant to the question{}",
            relevance
                .values()
                .filter(|p| crate::decision::ASK_GATHER_YES.yes(**p))
                .count(),
            candidates.len(),
            if asked.answered() {
                String::new()
            } else {
                format!(
                    "; Jev didn't answer ({}), so a fixed order picks the runs",
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
                    mine.retain(|(_, p)| p.is_some_and(|p| crate::decision::ASK_GATHER_YES.yes(p)));
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
                "jev ▸ {} of {} transcript steps are relevant to the question",
                probabilities
                    .values()
                    .filter(|p| crate::decision::ASK_GATHER_YES.yes(**p))
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
    gathered.inputs.marks = marks;
    gathered.inputs.about_marks = about_marks;
    gathered.inputs.reasons = reasons;
    gathered.inputs.asked_reasons = asked_reasons;
    gathered.inputs.tasks = named;
    gathered
}

/// A highlights ask: read `gym runs highlights --json` and choose the
/// highlights to draft. It asks Jev nothing: code chose the claims.
async fn gather_highlights(
    options: &Options,
    reader: &Reader<'_>,
    recorder: &Recorder,
    progress: &Progress,
) -> Result<Gathered, String> {
    let mut gathered = Gathered::default();
    let probe = reader
        .gym(strings(&["runs", "highlights", "--json", "--limit", "60"]))
        .await;
    gather::record_probes(recorder, "highlights", &[&probe], progress);
    let highlights = probe.json().ok_or_else(|| {
        format!(
            "`gym runs highlights --json` failed: {}",
            clip(probe.stderr.trim(), 300)
        )
    })?;
    let (chosen, missing) = drafts::choose(&highlights, &options.claims);
    if !missing.is_empty() {
        progress.line(&format!(
            "highlights ▸ no highlight has the key {}; `gym runs highlights` lists them",
            missing.join(", ")
        ));
    }
    if chosen.is_empty() {
        return Err(if missing.is_empty() {
            "`gym runs highlights` found no claim to draft".to_string()
        } else {
            format!(
                "no highlight has the key {}; `gym runs highlights` lists them",
                missing.join(", ")
            )
        });
    }
    progress.line(&format!(
        "highlights ▸ drafting {} of {}: {}",
        chosen.len(),
        highlights["total"],
        chosen
            .iter()
            .filter_map(|h| h["key"].as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    gathered.candidates = chosen.len();
    gathered.inputs.highlights = chosen;
    Ok(gathered)
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
        progress.line("probe ▸ rg isn't installed, so the briefing has no files");
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
        "jev ▸ {} of {} files are relevant to the question",
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

/// Validates the answer's proposals against the runs the ask read and
/// writes each one; a store that can't be written is reported, not fatal.
fn record_proposals(
    answer: &Value,
    options: &Options,
    (id, dir, repo): (&str, &Path, &Path),
    facts: &BTreeMap<String, Option<cite::RunFacts>>,
    shown: &BTreeMap<String, Value>,
    progress: &Progress,
) -> Vec<Value> {
    let mut run_tasks = BTreeMap::new();
    for (cited, fact) in facts {
        if let Some(fact) = fact {
            run_tasks.insert(cited.clone(), fact.task.clone());
            run_tasks.insert(fact.id.clone(), fact.task.clone());
        }
    }
    for (run, value) in shown {
        if let Some(fact) = cite::RunFacts::from_show(value) {
            run_tasks
                .entry(run.clone())
                .or_insert_with(|| fact.task.clone());
            run_tasks.entry(fact.id.clone()).or_insert(fact.task);
        }
    }
    let context = crate::proposal::Context {
        ask_id: id.to_string(),
        question: options.question.clone(),
        ask_dir: dir.display().to_string(),
        policies: crate::proposal::policies_dir(repo),
        run_tasks,
    };
    let Some(root) = options
        .proposals
        .clone()
        .or_else(crate::proposal::default_dir)
    else {
        progress.line("proposals ▸ HOME isn't set, so the proposals aren't recorded");
        return Vec::new();
    };
    match crate::proposal::record_all(answer, &context, &root) {
        Ok(records) => {
            progress.line(&format!(
                "proposals ▸ {} of {} are valid; each waits under {} for a person to approve it",
                records.iter().filter(|r| r["valid"] == true).count(),
                records.len(),
                root.display()
            ));
            records
        }
        Err(why) => {
            progress.line(&format!("proposals ▸ not recorded: {why}"));
            Vec::new()
        }
    }
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
    if record["scope"] == "highlights" {
        out.clear();
        out.push_str(&drafts_text(record));
    } else if !claims.is_empty() {
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
    let proposals = record["proposals"].as_array().cloned().unwrap_or_default();
    if !proposals.is_empty() {
        out.push_str("Proposals (✓ validated, ✗ refused), each waiting for a person's approval:\n");
        for proposal in &proposals {
            let valid = proposal["valid"] == true;
            out.push_str(&format!(
                "{} {} [{}] {}{}\n",
                if valid { '✓' } else { '✗' },
                proposal["id"].as_str().unwrap_or("?"),
                proposal["kind"].as_str().unwrap_or("?"),
                proposal["title"].as_str().unwrap_or(""),
                if proposal["needs_code"] == true {
                    " (needs code: a drafted issue)"
                } else {
                    ""
                }
            ));
            for problem in proposal["problems"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                out.push_str(&format!("     refused: {problem}\n"));
            }
        }
        out.push_str(
            "Approve one with `gym coder proposals approve ID`, then `coder-one proposal run ID`.\n\n",
        );
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

/// The drafts of a highlights ask as text: each draft with its highlight,
/// its runs, and, for a refused one, why.
fn drafts_text(record: &Value) -> String {
    let mut out = String::from(
        "Drafts from highlights (✓ passed the number and citation checks, ✗ refused). \
         Nothing posts; a person picks, edits, and posts.\n\n",
    );
    let drafts = record["drafts"].as_array().cloned().unwrap_or_default();
    if drafts.is_empty() {
        out.push_str(&format!(
            "No drafts: the executor {}.\n\n",
            record["status"].as_str().unwrap_or("didn't answer")
        ));
    }
    for (index, draft) in drafts.iter().enumerate() {
        let passed = draft["status"] == "passed";
        out.push_str(&format!(
            "{} {}. [{}] {}\n",
            if passed { '✓' } else { '✗' },
            index + 1,
            draft["highlight"].as_str().unwrap_or("no highlight"),
            draft["draft"].as_str().unwrap_or("")
        ));
        let runs: Vec<&str> = draft["runs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        if !runs.is_empty() {
            out.push_str(&format!("     cites: {}\n", runs.join("; ")));
        }
        for problem in draft["problems"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            out.push_str(&format!("     refused: {problem}\n"));
        }
    }
    out.push('\n');
    out
}

/// `coder-one ask …`: the exit code, 0 when it answered.
///
/// # Errors
///
/// Returns a message when the arguments don't parse or the ask can't
/// start.
pub async fn command(args: &[String]) -> Result<i32, String> {
    if args.first().map(String::as_str) == Some("study") {
        return study::command(&args[1..]).await;
    }
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
        let mark = json!({
            "run": "tb4--coder-one-tunable-v6--demo/demo__1", "step": null, "verdict": "bad",
            "tags": ["unearned_success"], "note": "said all tests pass; three failed",
            "author": "chris", "task": "demo",
        });
        let run = json!({
            "job": "tb4--coder-one-tunable-v6--demo", "trial": "demo__1", "task": "demo",
            "agent": "Coder One", "variant": "tunable-v6", "outcome": "failed",
            "tests": {"passed": 1, "failed": 3, "total": 4}, "cost_usd": 0.4,
            "learning": {"learning": 0.8, "reasons": [{"id": "unearned_success", "probability": 0.96}]},
            "marks": [mark],
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
                    "marks": [mark],
                }),
            ),
            (
                "marks.json",
                json!({"schema": "openagents.gym.runs-marks.v1", "marks": [mark]}),
            ),
            (
                "marked.json",
                json!({"total": 2, "ranked": 2, "running": 0, "runs": [run]}),
            ),
            (
                "highlights.json",
                json!({"schema": "openagents.gym.runs-highlights.v1", "total": 2, "highlights": [
                    {
                        "key": "cost-aaaa", "rule": "cost", "task": "demo",
                        "claim": "Coder One · tunable-v6 passed demo for 20% of what Claude Code · Opus 5.5 spent: $0.40 against $2.00 a passing run on average, 5.0 times as much for the second, over 2 and 2 runs.",
                        "runs": ["tb4--coder-one-tunable-v6--demo/demo__1"],
                        "numbers": [{"label": "share_percent", "value": 20.0, "text": "20%"}],
                        "sample": 2, "n1": false,
                        "caveats": ["Claude Code · Opus 5.5's cost is the Claude Code CLI's own list-price figure; these runs used a subscription, so it isn't a bill."],
                    },
                    {
                        "key": "surprise-bbbb", "rule": "surprise", "task": "demo",
                        "claim": "Coder One · tunable-v6 failed demo, an outcome Jev judged surprising (0.78).",
                        "runs": ["tb4--coder-one-tunable-v6--demo/demo__1"],
                        "numbers": [], "sample": 1, "n1": true, "caveats": ["One run: an anecdote, not a benchmark result."],
                    },
                ]}),
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
             'runs marks --json') cat \"$D/marks.json\" ;;\n\
             'runs --marked --json --limit 30') cat \"$D/marked.json\" ;;\n\
             'runs highlights --json --limit 60') cat \"$D/highlights.json\" ;;\n\
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

    fn replay(
        fixtures: &Path,
        out: &Path,
        args: &[&str],
        answer: &Value,
    ) -> Result<(Value, i32), String> {
        let gym = fake_gym(fixtures);
        let file = fixtures.join("answer.json");
        std::fs::write(&file, answer.to_string()).unwrap();
        let mut options = Options::parse(&strings(args)).unwrap();
        options.gym = Some(gym);
        options.out = Some(out.to_path_buf());
        options.repo = Some(fixtures.to_path_buf());
        options.answer_file = Some(file);
        let progress = Progress {
            events: false,
            quiet: true,
        };
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(run(options, &progress))
    }

    #[test]
    fn an_answer_can_carry_proposals_that_code_validates_and_writes() {
        if coder_boundary::Boundary::readonly().build().is_err() {
            return;
        }
        let fixtures = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let proposals = out.path().join("proposals");
        let demo = "tb4--coder-one-tunable-v6--demo/demo__1";
        let proposal = |runs: &[&str], patch: &str| {
            json!({
                "kind": "check", "title": "Run the behavior scenarios",
                "rationale": "The checks passed a false claim.",
                "source_runs": runs, "expected_tasks": ["demo"],
                "base": "tunable-luna-v2", "patch": patch,
                "question_set": "", "minitask": "", "issue": "",
            })
        };
        let answer = json!({
            "answer": "The demo run claimed success it didn't earn.",
            "claims": [{"claim": "It said all tests pass.", "runs": [demo], "steps": [], "judgments": [], "files": [], "marks": [], "highlight": ""}],
            "proposed_change": "",
            "proposals": [
                proposal(&[demo], r#"{"policy":{"verify":{"behavior":true}}}"#),
                proposal(&["tb4--nowhere/x__1"], r#"{"policy":{"verify":{"behavior":true}}}"#),
            ],
        });
        let (record, code) = replay(
            fixtures.path(),
            out.path(),
            &[
                "why did demo pass?",
                "--no-jev",
                "--proposals",
                &proposals.display().to_string(),
            ],
            &answer,
        )
        .unwrap();
        assert_eq!(code, 0, "{record}");
        let listed = record["proposals"].as_array().unwrap();
        assert_eq!(listed.len(), 2, "{record}");
        assert_eq!(listed[0]["valid"], true, "{record}");
        assert_eq!(listed[1]["valid"], false, "{record}");
        let id = listed[0]["id"].as_str().unwrap();
        assert_eq!(
            id,
            format!(
                "prop-{}-1",
                record["id"].as_str().unwrap().trim_start_matches("ask-")
            )
        );
        let (dir, written) = crate::proposal::load(&proposals, id).unwrap();
        assert_eq!(written["ask"]["id"], record["id"]);
        assert!(dir.join(crate::proposal::POLICY_FILE).is_file());
        let text = text(&record);
        assert!(text.contains(&format!("✓ {id} [check]")), "{text}");
        assert!(text.contains("isn't a run the ask read"), "{text}");
    }

    #[test]
    fn a_question_about_marks_opens_the_marked_runs_and_an_answer_can_cite_a_mark() {
        if coder_boundary::Boundary::readonly().build().is_err() {
            return;
        }
        let fixtures = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let demo = "tb4--coder-one-tunable-v6--demo/demo__1";
        let answer = json!({
            "answer": "A person marked the demo run bad for claiming success.",
            "claims": [
                {"claim": "A person marked it bad: it said all tests pass.", "runs": [demo], "steps": [], "judgments": [], "files": [], "marks": [demo], "highlight": ""},
                {"claim": "Step 2 was marked too.", "runs": [demo], "steps": [], "judgments": [], "files": [], "marks": [format!("{demo}/2")], "highlight": ""},
            ],
            "proposed_change": "",
        });
        let (record, code) = replay(
            fixtures.path(),
            out.path(),
            &[
                "which", "runs", "did", "a", "person", "mark", "bad?", "--no-jev",
            ],
            &answer,
        )
        .unwrap();
        assert_eq!(code, 0, "{record}");
        assert_eq!(record["evidence"]["marks"], 1, "{record}");
        assert_eq!(record["evidence"]["about_marks"], true, "{record}");
        assert_eq!(record["evidence"]["opened"][0]["run"], demo, "{record}");
        let verified: Vec<bool> = record["claims"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["verified"] == true)
            .collect();
        assert_eq!(verified, vec![true, false], "{record}");
        assert_eq!(
            record["claims"][1]["problems"][0],
            format!("no person marked step 2 of {demo}")
        );
        let dir = PathBuf::from(record["dir"].as_str().unwrap());
        let briefing = std::fs::read_to_string(dir.join("briefing.md")).unwrap();
        for needle in [
            "# A person's marks",
            "marked bad, tagged unearned_success — \"said all tests pass; three failed\" (chris)",
            "so the marked runs come first",
            "A person's mark: `tb4--coder-one-tunable-v6--demo/demo__1` demo: marked bad",
        ] {
            assert!(briefing.contains(needle), "{needle}\n{briefing}");
        }
    }

    #[test]
    fn a_highlights_ask_refuses_a_draft_whose_numbers_or_citations_do_not_check() {
        if coder_boundary::Boundary::readonly().build().is_err() {
            return;
        }
        let fixtures = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let demo = "tb4--coder-one-tunable-v6--demo/demo__1";
        let draft = |text: &str, key: &str, runs: &[&str]| json!({"claim": text, "runs": runs, "steps": [], "judgments": [], "files": [], "marks": [], "highlight": key});
        let answer = json!({
            "answer": "Two drafts.",
            "claims": [
                draft("On demo, Coder One passed for $0.40 a run; Claude Code on Opus 5.5 spent $2.00, so Coder One cost 20% as much, over 2 runs each.", "cost-aaaa", &[demo]),
                draft("Coder One passed demo for 7 times less than Claude Code.", "cost-aaaa", &[demo]),
                draft("Coder One failed demo, a surprise.", "surprise-bbbb", &[demo]),
                draft("Coder One cost 20% as much.", "cost-aaaa", &["tb4--nowhere/x__1"]),
            ],
            "proposed_change": "",
        });
        let (record, code) = replay(
            fixtures.path(),
            out.path(),
            &["--scope", "highlights", "--claim", "cost-aaaa"],
            &answer,
        )
        .unwrap();
        assert_eq!(code, 0, "{record}");
        assert_eq!(record["highlights"], json!(["cost-aaaa"]));
        let status: Vec<&str> = record["drafts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["status"].as_str().unwrap())
            .collect();
        assert_eq!(
            status,
            vec!["passed", "refused", "refused", "refused"],
            "{record}"
        );
        assert_eq!(
            record["drafts"][1]["problems"],
            json!(["7 isn't a number cost-aaaa gives"])
        );
        assert_eq!(
            record["drafts"][2]["problems"],
            json!(["surprise-bbbb isn't one of the chosen highlights"])
        );
        assert_eq!(record["cost"]["jev_requests"], 0);
        let text = text(&record);
        assert!(text.starts_with("Drafts from highlights"), "{text}");
        assert!(text.contains("✓ 1. [cost-aaaa] On demo"), "{text}");
        assert!(
            text.contains("refused: 7 isn't a number cost-aaaa gives"),
            "{text}"
        );
        assert!(
            text.contains("refused: the Gym has no run tb4--nowhere/x__1"),
            "{text}"
        );
        let dir = PathBuf::from(record["dir"].as_str().unwrap());
        let briefing = std::fs::read_to_string(dir.join("briefing.md")).unwrap();
        assert!(briefing.contains("# Claims to draft"), "{briefing}");
        assert!(briefing.contains("## `cost-aaaa` (cost)"), "{briefing}");
        assert!(!briefing.contains("surprise-bbbb"), "{briefing}");

        // With no --claim, the strongest are chosen; an unknown key is refused.
        let (record, _) = replay(
            fixtures.path(),
            out.path(),
            &["--scope", "highlights"],
            &answer,
        )
        .unwrap();
        assert_eq!(record["highlights"], json!(["cost-aaaa", "surprise-bbbb"]));
        assert_eq!(
            record["drafts"][2]["status"], "refused",
            "an n=1 draft must say so"
        );
        assert!(
            replay(
                fixtures.path(),
                out.path(),
                &["--scope", "highlights", "--claim", "nope"],
                &answer
            )
            .unwrap_err()
            .contains("no highlight has the key nope")
        );
        assert!(Options::parse(&strings(&["q", "--claim", "x"])).is_err());
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
