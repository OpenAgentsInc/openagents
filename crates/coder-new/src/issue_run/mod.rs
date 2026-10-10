//! `coder issue-run N` (#11214): one issue played from issue to pull
//! request inside the Coder terminal, as a test run.
//!
//! The run has three parts, each shown in the conversation as it happens:
//!
//! - **Decision steps** ([`decide`]): the context finder's deterministic
//!   stages (#11210, `scripts/filefind/filefind.py`), its scorer, the
//!   System One questions asked of Jev or Clef through `crates/jev`, the
//!   ranked files, and the briefing (#11211's generator, through
//!   `scripts/coder/issue_run_briefing.py`). Each shows as a decision card,
//!   a tool-call style of its own ([`cards`]).
//! - **The briefed agent** ([`agent`]): one Claude Agent SDK session
//!   (`crates/claude_agent_sdk`) on the owner's Claude Code login, with
//!   Read, Edit, Write, Grep and Glob held to the run's working copy, and
//!   an in-process `run_check` tool that runs only the briefing's checks.
//!   Its tool calls become the same transcript items Coder's own tools
//!   make, so they draw with Coder's Read, Edit (with diffs), Grep, Glob
//!   and Run widgets.
//! - **The summary card**: time, tokens, dollars, the files the agent
//!   opened that the briefing did not list, the check results, and the
//!   diff; for a closed issue, the comparison with the real fix.
//!
//! Test mode is the default: the working copy is the run's own fresh
//! worktree at the issue's base (the fix's parent for a closed issue,
//! `origin/main` for an open one), and nothing is pushed. `--open-pr`
//! commits to a branch, pushes it, and opens a pull request, for an open
//! issue only, and only when the run passed ([`verdict`]): every required
//! final check ran and passed and the agent ended without an error.
//!
//! Each run has a unique id and its own folder,
//! `STATE/<run-id>/` (`run.json`, `events.jsonl`, `summary.json`,
//! `change.patch`, and `worktree/`). Every run, including one that stopped
//! in setup, in the decision steps, or was cancelled, ends with a
//! `summary.json` naming its outcome and attempt, and is captured as a
//! trace (`scripts/bench/traces`), so failed attempts and retries survive
//! export. Every transcript change is also written to `events.jsonl`, so
//! `coder issue-run --replay FILE` plays a recorded run again.

pub mod agent;
pub mod cards;
pub mod decide;
pub mod setup;
pub mod verdict;

use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use crate::live::Entry;
use verdict::{Outcome, Status};

/// The tool name of a decision card.
pub const DECISION: &str = "Decision";
/// The tool name of the summary card.
pub const SUMMARY: &str = "Summary";

/// Which System One door answers the decision questions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decider {
    /// Jev, TypeSafe's API, with this computer's key; without one, our
    /// decision API (`openagents.com/api/v1/systemone`, Jev under the house
    /// key first, then connected Pylons; #11225), keyless.
    Auto,
    /// Jev, TypeSafe's hosted API, for every question.
    Jev,
    /// Clef-Flash on this computer's Ollama for every question.
    Clef,
    /// No model questions; the scorer's ranking stands.
    Off,
}

/// What `coder issue-run` was asked to do.
#[derive(Clone, Debug)]
pub struct Options {
    pub issue: u64,
    /// The repository checkout whose history the run reads.
    pub repo: PathBuf,
    /// The GitHub repository the issue lives in.
    pub github: String,
    /// Commit, push, and open a pull request at the end (open issues only).
    pub open_pr: bool,
    /// Print the transcript as text instead of opening the terminal UI.
    pub plain: bool,
    /// Write an SVG picture of the whole transcript here at the end.
    pub snapshot: Option<PathBuf>,
    /// Play a recorded `events.jsonl` instead of running.
    pub replay: Option<PathBuf>,
    /// How much faster than recorded a replay plays.
    pub speed: f64,
    pub decider: Decider,
    /// The fix commit of a closed issue, when the history search would pick
    /// the wrong one.
    pub fix: Option<String>,
    /// The briefed agent's model.
    pub model: String,
    /// How long the briefed agent may work.
    pub timeout: Duration,
    /// How many files the briefing lists.
    pub files: usize,
    /// Where the run's folders and its working copy live.
    pub state: PathBuf,
}

pub const USAGE: &str = "Usage: coder issue-run N [OPTIONS]

Plays issue N from issue to pull request in the Coder terminal, as a test
run: the decision steps that find the files and brief the agent, the
briefed Claude agent's work, and a summary. It works in a fresh worktree
at the issue's base and never pushes unless you pass --open-pr.

Options:
  --repo PATH        The repository checkout to read (default: this directory).
  --github OWNER/R   The GitHub repository of the issue (default: OpenAgentsInc/openagents).
  --fix SHA          A closed issue's fix commit (default: the newest commit naming #N).
  --decider WHO      auto, jev, clef, or off (default: auto: Jev with your TypeSafe key,
                     else our decision API, keyless).
  --files K          Files in the briefing (default 6).
  --model NAME       The agent's model (default claude-opus-5-5).
  --timeout SECS     How long the agent may work (default 1800).
  --open-pr          Commit, push a branch, and open a pull request (open issues only).
  --plain            Print the run as text instead of opening the terminal screen.
  --snapshot FILE    Also save a picture of the whole run as an SVG file.
  --replay FILE      Play a recorded run (its events.jsonl) again.
  --speed X          Play a recording X times faster (default 1).";

impl Options {
    /// Reads `coder issue-run`'s arguments.
    ///
    /// # Errors
    /// An option is unknown, lacks its value, or the value is not valid.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut issue = None;
        let mut options = Self {
            issue: 0,
            repo: std::env::current_dir().map_err(|_| "Cannot read this directory.".to_owned())?,
            github: "OpenAgentsInc/openagents".into(),
            open_pr: false,
            plain: false,
            snapshot: None,
            replay: None,
            speed: 1.0,
            decider: Decider::Auto,
            fix: None,
            model: "claude-opus-5-5".into(),
            timeout: Duration::from_secs(1800),
            files: 6,
            state: default_state(),
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| {
                args.next()
                    .cloned()
                    .ok_or(format!("{name} needs a value. Run coder issue-run --help."))
            };
            match arg.as_str() {
                "--repo" => options.repo = PathBuf::from(value("--repo")?),
                "--github" => options.github = value("--github")?,
                "--fix" => options.fix = Some(value("--fix")?),
                "--decider" => {
                    options.decider = match value("--decider")?.as_str() {
                        "auto" => Decider::Auto,
                        "jev" => Decider::Jev,
                        "clef" => Decider::Clef,
                        "off" => Decider::Off,
                        other => {
                            return Err(format!(
                                "--decider takes auto, jev, clef or off, not {other}."
                            ));
                        }
                    }
                }
                "--files" => {
                    options.files = value("--files")?
                        .parse::<usize>()
                        .map_err(|_| "--files takes a number.".to_owned())?
                        .clamp(1, 20);
                }
                "--model" => options.model = value("--model")?,
                "--timeout" => {
                    options.timeout = Duration::from_secs(
                        value("--timeout")?
                            .parse()
                            .map_err(|_| "--timeout takes seconds.".to_owned())?,
                    );
                }
                "--open-pr" => options.open_pr = true,
                "--plain" => options.plain = true,
                "--snapshot" => options.snapshot = Some(PathBuf::from(value("--snapshot")?)),
                "--replay" => options.replay = Some(PathBuf::from(value("--replay")?)),
                "--speed" => {
                    options.speed = value("--speed")?
                        .parse::<f64>()
                        .ok()
                        .filter(|speed| *speed > 0.0)
                        .ok_or("--speed takes a number above zero.")?;
                }
                "--state" => options.state = PathBuf::from(value("--state")?),
                other if other.starts_with('#') || other.parse::<u64>().is_ok() => {
                    issue = Some(
                        other
                            .trim_start_matches('#')
                            .parse::<u64>()
                            .map_err(|_| format!("{other} is not an issue number."))?,
                    );
                }
                other => {
                    return Err(format!(
                        "Unknown option {other}. Run coder issue-run --help."
                    ));
                }
            }
        }
        match (issue, &options.replay) {
            (Some(issue), _) => options.issue = issue,
            (None, Some(_)) => {}
            (None, None) => return Err("Name an issue number: coder issue-run N.".into()),
        }
        Ok(options)
    }
}

/// `~/.openagents/coder-new/issue-runs`.
fn default_state() -> PathBuf {
    model_access::store::openagents_dir()
        .map(|root| root.join("coder-new").join("issue-runs"))
        .unwrap_or_else(|| std::env::temp_dir().join("coder-issue-runs"))
}

/// One run's identity: its unique id, its folder `STATE/<id>/`, and where
/// it sits among the attempts at the same issue.
#[derive(Clone, Debug)]
pub struct Run {
    pub id: String,
    pub folder: PathBuf,
    /// `{"index": K, "prior_runs": [ids]}`: this is the K-th attempt at the
    /// issue under this state root, after the runs named (finished, failed,
    /// cancelled or killed alike).
    pub attempt: Value,
}

/// A run id: the issue, the start second, and 64 random bits, so two runs
/// started together never share a folder.
#[must_use]
pub fn run_id(issue: u64, seconds: u64) -> String {
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        bytes = (u64::from(nanos) << 32 | u64::from(std::process::id())).to_be_bytes();
    }
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{issue}-{seconds}-{hex}")
}

/// Makes a new run's folder under `state`. The folder is created
/// exclusively, so it is never another run's; `run.json` records the run's
/// identity and attempt before any work starts.
///
/// # Errors
/// The state root or the folder cannot be created.
pub fn new_run(options: &Options) -> Result<Run, String> {
    let state = &options.state;
    std::fs::create_dir_all(state)
        .map_err(|error| format!("Cannot create {}: {error}", state.display()))?;
    let prefix = format!("{}-", options.issue);
    let mut prior: Vec<String> = std::fs::read_dir(state)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().is_dir())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| name.starts_with(&prefix))
                .collect()
        })
        .unwrap_or_default();
    prior.sort();
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut made = None;
    for _ in 0..8 {
        let id = run_id(options.issue, seconds);
        let folder = state.join(&id);
        match std::fs::create_dir(&folder) {
            Ok(()) => {
                made = Some((id, folder));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("Cannot create {}: {error}", folder.display())),
        }
    }
    let (id, folder) = made.ok_or("Cannot find a free run id.")?;
    let attempt = json!({"index": prior.len() + 1, "prior_runs": prior});
    let record = json!({
        "v": "openagents.coder-issue-run.v1",
        "run_id": id,
        "issue": options.issue,
        "github": options.github,
        "repo": options.repo.display().to_string(),
        "model": options.model,
        "open_pr": options.open_pr,
        "pid": std::process::id(),
        "started_at": seconds,
        "attempt": attempt,
    });
    setup::write_atomic(
        &folder.join("run.json"),
        &serde_json::to_vec_pretty(&record).unwrap_or_default(),
    )?;
    Ok(Run {
        id,
        folder,
        attempt,
    })
}

/// One change to the run's transcript.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Entry `index` is now `entry` (a new one when `index` is the length).
    Set { index: usize, entry: Entry },
    /// Whether the run is still working.
    Busy(bool),
    /// A line under the transcript, such as where the run's files are.
    Notice(String),
}

/// A run's transcript changes, for the terminal to apply.
pub struct Feed {
    events: mpsc::Receiver<Event>,
    cancel: Arc<AtomicBool>,
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Feed {
    /// The changes that arrived since the last call.
    pub fn drain(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }

    /// Waits for the next change; `None` once the run has ended.
    pub fn next(&self) -> Option<Event> {
        self.events.recv().ok()
    }
}

/// The transcript the run writes: it sends each change to the terminal and
/// records it in `events.jsonl`.
pub struct Transcript {
    entries: Vec<Entry>,
    sender: mpsc::Sender<Event>,
    log: Option<std::fs::File>,
    started: Instant,
    pub(crate) cancel: Arc<AtomicBool>,
}

impl Transcript {
    fn send(&mut self, event: Event) {
        if let Some(log) = &mut self.log {
            let mut line = event_to_json(&event);
            line["t_ms"] = json!(self.started.elapsed().as_millis() as u64);
            let _ = writeln!(log, "{line}");
        }
        let _ = self.sender.send(event);
    }

    /// Adds an entry and returns its index.
    pub fn push(&mut self, entry: Entry) -> usize {
        let index = self.entries.len();
        self.entries.push(entry.clone());
        self.send(Event::Set { index, entry });
        index
    }

    /// Replaces entry `index`.
    pub fn set(&mut self, index: usize, entry: Entry) {
        if let Some(slot) = self.entries.get_mut(index) {
            if *slot == entry {
                return;
            }
            *slot = entry.clone();
            self.send(Event::Set { index, entry });
        }
    }

    pub fn notice(&mut self, text: impl Into<String>) {
        self.send(Event::Notice(text.into()));
    }

    pub fn busy(&mut self, busy: bool) {
        self.send(Event::Busy(busy));
    }

    /// Whether the person stopped the run.
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Starts a decision card and returns its index.
    pub fn card(&mut self, card: Value) -> usize {
        self.push(Entry::Tool {
            name: DECISION.into(),
            input: card,
            output: Value::Null,
            running: true,
        })
    }

    /// Finishes decision card `index` with its result.
    pub fn finish(&mut self, index: usize, card: Value, output: Value) {
        self.set(
            index,
            Entry::Tool {
                name: DECISION.into(),
                input: card,
                output,
                running: false,
            },
        );
    }
}

/// Starts the run on its own thread. The run's folder is
/// `STATE/<run-id>/`; `events.jsonl` there records every change.
pub fn start(options: Options) -> Feed {
    let (sender, events) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let feed = Feed {
        events,
        cancel: cancel.clone(),
    };
    std::thread::spawn(move || {
        if let Some(replay) = options.replay.clone() {
            play(&replay, options.speed, &sender, &cancel);
            return;
        }
        let made = new_run(&options);
        let log = made
            .as_ref()
            .ok()
            .and_then(|run| std::fs::File::create(run.folder.join("events.jsonl")).ok());
        let mut transcript = Transcript {
            entries: Vec::new(),
            sender,
            log,
            started: Instant::now(),
            cancel,
        };
        let run = match made {
            Ok(run) => run,
            Err(why) => {
                transcript.notice(format!("The run could not start: {why}"));
                return;
            }
        };
        transcript.busy(true);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build();
        match runtime {
            Ok(runtime) => {
                runtime.block_on(drive(&options, &run, &mut transcript));
            }
            Err(_) => transcript.notice("The run could not start its worker threads."),
        }
        transcript.busy(false);
    });
    feed
}

/// The whole run, step by step. Every way out, a stop in setup or the
/// decision steps, a cancel, or the end, writes the run's summary.
async fn drive(options: &Options, run: &Run, transcript: &mut Transcript) {
    transcript.push(Entry::User(format!(
        "Play issue #{} from issue to pull request, as a test run.",
        options.issue
    )));
    let started = Instant::now();
    let folder = run.folder.as_path();
    let mut ending = End {
        options,
        run,
        base: None,
        briefing: None,
        work: None,
        checks: Vec::new(),
        started,
    };
    let Some(base) = setup::prepare(options, folder, transcript).await else {
        let outcome = if transcript.cancelled() {
            Outcome::stopped(Status::Cancelled, "The run was stopped during setup.")
        } else {
            Outcome::stopped(
                Status::SetupFailed,
                "The issue, its base commit, or the worktree could not be prepared.",
            )
        };
        return ending.conclude(outcome, transcript).await;
    };
    ending.base = Some(base.clone());
    if transcript.cancelled() {
        let outcome = Outcome::stopped(Status::Cancelled, "The run was stopped after setup.");
        return ending.conclude(outcome, transcript).await;
    }
    let Some(briefing) = decide::decide(options, &base, folder, transcript).await else {
        let outcome = if transcript.cancelled() {
            Outcome::stopped(
                Status::Cancelled,
                "The run was stopped in the decision steps.",
            )
        } else {
            Outcome::stopped(
                Status::DecisionFailed,
                "The decision steps stopped before a briefing existed.",
            )
        };
        return ending.conclude(outcome, transcript).await;
    };
    ending.briefing = Some(briefing.clone());
    if transcript.cancelled() {
        let outcome = Outcome::stopped(Status::Cancelled, "The run was stopped before the agent.");
        return ending.conclude(outcome, transcript).await;
    }
    let work = agent::work(options, &base, &briefing, folder, transcript).await;
    ending.work = Some(work.clone());
    if !transcript.cancelled() {
        ending.checks = agent::final_checks(&base, &briefing, transcript).await;
    }
    let outcome = verdict::judge(
        transcript.cancelled(),
        work.error.as_deref(),
        &setup::required_checks(&briefing),
        &ending.checks,
    );
    ending.conclude(outcome, transcript).await;
}

/// What a run has when it ends.
struct End<'a> {
    options: &'a Options,
    run: &'a Run,
    base: Option<setup::Base>,
    briefing: Option<decide::Briefing>,
    work: Option<agent::Work>,
    checks: Vec<(String, bool)>,
    started: Instant,
}

impl End<'_> {
    /// Writes the summary, delivers only a passed run when `--open-pr` asks,
    /// and captures the run as a trace.
    async fn conclude(self, outcome: Outcome, transcript: &mut Transcript) {
        let folder = self.run.folder.as_path();
        let summary = setup::summary(&setup::Report {
            options: self.options,
            run: self.run,
            base: self.base.as_ref(),
            briefing: self.briefing.as_ref(),
            work: self.work.as_ref(),
            checks: &self.checks,
            outcome: &outcome,
            started: self.started,
            folder,
        });
        if self.base.is_some() && self.briefing.is_some() {
            let diff = self
                .base
                .as_ref()
                .map(|base| setup::working_diff(&base.worktree, &base.base))
                .unwrap_or_default();
            transcript.push(Entry::Tool {
                name: SUMMARY.into(),
                input: summary.clone(),
                output: json!({"diff": diff}),
                running: false,
            });
        }
        if let Err(why) = setup::write_atomic(
            &folder.join("summary.json"),
            &serde_json::to_vec_pretty(&summary).unwrap_or_default(),
        ) {
            transcript.notice(format!("{why}. The worktree is kept."));
        }
        if self.options.open_pr {
            match &self.base {
                Some(base) => setup::open_pr(self.options, base, &outcome, transcript).await,
                None => transcript.notice("No pull request: the run has no working copy."),
            }
        }
        capture(self.options, folder, transcript).await;
        let worktree = self
            .base
            .as_ref()
            .map(|base| format!(" Working copy: {}.", base.worktree.display()))
            .unwrap_or_default();
        transcript.notice(format!(
            "Test run ended: {}{}.{worktree} Recording: {}.",
            outcome.status.as_str(),
            outcome
                .reason
                .as_deref()
                .map(|why| format!(" ({why})"))
                .unwrap_or_default(),
            folder.join("events.jsonl").display()
        ));
    }
}

/// Stores the run as a trace (`scripts/bench/traces/traces.py capture`),
/// which marks the folder `trace-captured.json`; only then may a later run
/// clean its worktree. `CODER_ISSUE_RUN_CAPTURE=0` skips it.
async fn capture(options: &Options, folder: &Path, transcript: &mut Transcript) {
    if std::env::var("CODER_ISSUE_RUN_CAPTURE").as_deref() == Ok("0") {
        return;
    }
    let script = tools_root(&options.repo).join("scripts/bench/traces/traces.py");
    if !script.exists() {
        transcript.notice("The trace was not captured (no traces.py); the worktree is kept.");
        return;
    }
    let repo = options
        .repo
        .canonicalize()
        .unwrap_or_else(|_| options.repo.clone());
    let (script, repo, folder_text) = (
        script.to_string_lossy().into_owned(),
        repo.to_string_lossy().into_owned(),
        folder.to_string_lossy().into_owned(),
    );
    let args = [
        script.as_str(),
        "--repo",
        repo.as_str(),
        "capture",
        "--issue-run-folders",
        folder_text.as_str(),
    ];
    if let Err(why) = setup::output("python3", &args, folder).await {
        transcript.notice(format!(
            "The trace was not captured ({why}); the worktree is kept."
        ));
    }
}

/// Plays a recorded run with its original pacing, `speed` times faster.
fn play(path: &Path, speed: f64, sender: &mpsc::Sender<Event>, cancel: &AtomicBool) {
    let Ok(text) = std::fs::read_to_string(path) else {
        let _ = sender.send(Event::Notice(format!(
            "Cannot read the recording {}.",
            path.display()
        )));
        return;
    };
    let started = Instant::now();
    for line in text.lines() {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let at = value["t_ms"].as_u64().unwrap_or(0) as f64 / speed;
        let wait = Duration::from_millis(at as u64).saturating_sub(started.elapsed());
        // Long waits (an agent thinking, a build) are shortened to keep a
        // replay watchable.
        std::thread::sleep(wait.min(Duration::from_secs(3)));
        if let Some(event) = event_from_json(&value)
            && sender.send(event).is_err()
        {
            return;
        }
    }
}

/// An event as one JSON line of `events.jsonl`.
#[must_use]
pub fn event_to_json(event: &Event) -> Value {
    match event {
        Event::Set { index, entry } => json!({"set": index, "entry": entry_to_json(entry)}),
        Event::Busy(busy) => json!({"busy": busy}),
        Event::Notice(text) => json!({"notice": text}),
    }
}

/// The event one JSON line of `events.jsonl` holds.
#[must_use]
pub fn event_from_json(value: &Value) -> Option<Event> {
    if let Some(index) = value["set"].as_u64() {
        return Some(Event::Set {
            index: usize::try_from(index).ok()?,
            entry: entry_from_json(&value["entry"])?,
        });
    }
    if let Some(busy) = value["busy"].as_bool() {
        return Some(Event::Busy(busy));
    }
    value["notice"]
        .as_str()
        .map(|text| Event::Notice(text.to_owned()))
}

fn entry_to_json(entry: &Entry) -> Value {
    match entry {
        Entry::User(text) => json!({"user": text}),
        Entry::Assistant {
            text,
            model,
            elapsed_ms,
        } => json!({"assistant": text, "model": model, "elapsed_ms": elapsed_ms}),
        Entry::Tool {
            name,
            input,
            output,
            running,
        } => json!({"tool": name, "input": input, "output": output, "running": running}),
        Entry::Delegation { name, task, .. } => json!({"user": format!("{name}: {task}")}),
    }
}

fn entry_from_json(value: &Value) -> Option<Entry> {
    if let Some(text) = value["user"].as_str() {
        return Some(Entry::User(text.to_owned()));
    }
    if let Some(text) = value["assistant"].as_str() {
        return Some(Entry::Assistant {
            text: text.to_owned(),
            model: value["model"].as_str().map(str::to_owned),
            elapsed_ms: value["elapsed_ms"].as_u64(),
        });
    }
    Some(Entry::Tool {
        name: value["tool"].as_str()?.to_owned(),
        input: value["input"].clone(),
        output: value["output"].clone(),
        running: value["running"].as_bool().unwrap_or(false),
    })
}

/// Where the decision steps' scripts come from: `CODER_ISSUE_RUN_TOOLS`,
/// else the checkout this Coder was built from, else the run's repository.
pub(crate) fn tools_root(repo: &Path) -> PathBuf {
    if let Some(root) = std::env::var_os("CODER_ISSUE_RUN_TOOLS") {
        return PathBuf::from(root);
    }
    let built = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    if built.join("scripts/filefind/filefind.py").exists() {
        return built.canonicalize().unwrap_or(built);
    }
    repo.to_owned()
}

/// A secret from the environment, else from `~/work/.secrets/FILE`
/// (`NAME=value` lines). The value is never printed.
pub(crate) fn secret(name: &str, file: &str) -> Option<String> {
    if let Ok(value) = std::env::var(name)
        && !value.trim().is_empty()
    {
        return Some(value.trim().to_owned());
    }
    let home = std::env::var("HOME").ok()?;
    let text = std::fs::read_to_string(Path::new(&home).join("work/.secrets").join(file)).ok()?;
    text.lines().find_map(|line| {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line);
        let value = line.strip_prefix(name)?.strip_prefix('=')?;
        let value = value.trim().trim_matches('"').trim_matches('\'');
        (!value.is_empty()).then(|| value.to_owned())
    })
}

#[cfg(test)]
mod tests;
