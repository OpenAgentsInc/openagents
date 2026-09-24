//! The delegate door: a turn answered by Claude Code, Codex, or Microluna
//! from a Jev briefing, with the Open Responses door as the fallback.
//!
//! Terminal-Bench measured a Jev probe battery and a strong executor
//! beating a model exploring on its own, so Coder Terminal answers the
//! same way when this machine has an executor to run. One turn is four
//! steps, all of them Coder One's library rather than a second copy here:
//!
//! 1. The host runs the read-only probe battery in the working directory.
//! 2. Jev judges the probes and candidate files.
//! 3. Code packs a briefing from the request and the conversation.
//! 4. The executor runs it, inside a `coder-boundary` boundary.
//!
//! See [`coder_one::terminal`] for the four steps and the policy they run.
//!
//! # Which door answers
//!
//! [`choose`] decides, from [`MODE_VAR`], [`AGENT_VAR`], the doors the
//! environment names explicitly, and the targets this machine has:
//!
//! - `CODER_DELEGATE=auto`, the default, delegates to Microluna when the
//!   Codex login (`~/.codex/auth.json`) has more than ten minutes left on
//!   its access token; Microluna runs in this process, so nothing needs
//!   installing. Without a usable login it delegates to an installed and
//!   authenticated `claude`, then `codex`, and falls back to the door
//!   [`Door::from_env`] builds when none is available, saying why.
//! - `CODER_DELEGATE=always` delegates or refuses to start.
//! - `CODER_DELEGATE=off` never delegates.
//!
//! A relay worker or a local executor named with `CODER_WORKER` or
//! `CODER_EXECUTOR` is an explicit request for that door and outranks
//! delegation. An Open Responses key is not: it names the fallback.
//!
//! # What stays the host's
//!
//! Delegation is a host decision. The model never sees a delegate tool;
//! the door is chosen before a turn generates a word. The turn's
//! [`crate::permit::Permit`] decides what the executor may do: a turn that
//! runs no commands, such as a clarifying one, runs the executor in a
//! read-only boundary, and a turn permitted to run commands may write
//! inside the workspace and nowhere else.
//!
//! `coder-worker` builds its door with [`Door::from_env`] and never
//! reaches this module.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use coder_one::delegate::{Agent as Cli, Credential, Status as DelegateStatus};
use coder_one::stream::{Event as StreamEvent, Kind};
use coder_one::terminal::{self, Progress};
use serde_json::Value;

use crate::generate::{Door, Generate, GenerateError, Message, Meta, Role, Usage};
use crate::shell::{Outcome, Proposal, Status};

/// The variable that turns delegation on, off, or on unconditionally:
/// `auto`, `always`, or `off`.
///
/// Before this door existed the same variable named the capability a
/// program's `delegate` step hands work to, and it still does when its
/// value is none of those three words. See [`crate::agent::DELEGATE_VAR`].
pub const MODE_VAR: &str = "CODER_DELEGATE";

/// The variable that names the target: `claude-code`, `codex`, or
/// `microluna`.
pub const AGENT_VAR: &str = "CODER_DELEGATE_AGENT";

/// The variable that names the target's model.
pub const MODEL_VAR: &str = "CODER_DELEGATE_MODEL";

/// What this door's name is in a trace's session header.
pub const NAME: &str = "delegate";

/// Whether a turn may delegate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Delegate when a target is available, else fall back.
    Auto,
    /// Delegate, or refuse to start.
    Always,
    /// Never delegate.
    Off,
}

impl Mode {
    /// The mode `text` names, or `None` when it names none of the three.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Mode::Auto),
            "always" | "on" => Some(Mode::Always),
            "off" | "no" | "false" | "none" | "0" => Some(Mode::Off),
            _ => None,
        }
    }

    /// The mode a setting asks for: [`Mode::Auto`] when it is unset,
    /// empty, or a capability slug for a program's `delegate` step.
    #[must_use]
    pub fn read(setting: Option<&str>) -> Self {
        setting.and_then(Self::parse).unwrap_or(Mode::Auto)
    }

    /// The mode's word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Mode::Auto => "auto",
            Mode::Always => "always",
            Mode::Off => "off",
        }
    }
}

/// One executor this machine might delegate to, and what it has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// Which executor.
    pub agent: Cli,
    /// Where its binary is, when it is installed. Microluna runs in this
    /// process and has none.
    pub binary: Option<PathBuf>,
    /// Where its credential comes from, by name.
    pub credential: Credential,
    /// Why a credential that was found can't carry a turn, such as a Codex
    /// access token about to expire.
    pub problem: Option<String>,
}

impl Target {
    /// What `env` says about `agent`: its binary and its credential.
    pub fn find(agent: Cli, env: impl Fn(&str) -> Option<String>) -> Self {
        let (binary, credential) = coder_one::delegate::resolve(agent, &env);
        let problem = match (agent, credential) {
            (Cli::Microluna, Credential::CodexAuthFile) => codex_login(&env).err(),
            _ => None,
        };
        Target {
            agent,
            binary,
            credential,
            problem,
        }
    }

    /// Whether a turn could run on it: installed, or in this process, and
    /// authenticated with a credential that can be used.
    #[must_use]
    pub fn available(&self) -> bool {
        (self.binary.is_some() || !self.agent.is_cli())
            && self.credential != Credential::Missing
            && self.problem.is_none()
    }

    /// One sentence on where it stands.
    #[must_use]
    pub fn describe(&self) -> String {
        if !self.agent.is_cli() {
            return match (self.credential, &self.problem) {
                (Credential::Missing, _) => {
                    format!("{} has no Codex login", self.agent.word())
                }
                (_, Some(problem)) => {
                    format!("{} can't use the Codex login: {problem}", self.agent.word())
                }
                (credential, None) => format!(
                    "{} runs in this process on the Codex login ({})",
                    self.agent.word(),
                    credential.word()
                ),
            };
        }
        match (&self.binary, self.credential) {
            (None, _) => format!("{} is not installed", self.agent.word()),
            (Some(path), Credential::Missing) => format!(
                "{} is installed at {} and has no credential",
                self.agent.word(),
                path.display()
            ),
            (Some(path), credential) => format!(
                "{} is installed at {} and authenticated ({})",
                self.agent.word(),
                path.display(),
                credential.word()
            ),
        }
    }
}

/// The targets this machine has, in the order `auto` prefers them:
/// Microluna on the Codex login, then Claude Code, then Codex CLI.
pub fn targets(env: impl Fn(&str) -> Option<String>) -> Vec<Target> {
    [Cli::Microluna, Cli::ClaudeCode, Cli::Codex]
        .into_iter()
        .map(|agent| Target::find(agent, &env))
        .collect()
}

/// What the Codex login says about itself, without its secrets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexLogin {
    /// Where the login file is.
    pub path: PathBuf,
    /// When the access token expires, in seconds since the epoch, when the
    /// token says.
    pub expires_at: Option<u64>,
}

impl CodexLogin {
    /// Hours of validity left at `now`, in seconds since the epoch.
    #[must_use]
    pub fn hours_left(&self, now: u64) -> Option<f64> {
        self.expires_at
            .map(|at| at.saturating_sub(now) as f64 / 3_600.0)
    }
}

/// Reads the Codex login Microluna runs on, only to report on it: where
/// it is and when its access token expires. It never refreshes the login
/// and never returns a token.
///
/// # Errors
///
/// A sentence when the login is missing, unreadable, not a ChatGPT
/// sign-in, or within ten minutes of expiring.
pub fn codex_login(env: &impl Fn(&str) -> Option<String>) -> Result<CodexLogin, String> {
    let path = coder_one::delegate::codex_auth_file(env)
        .ok_or("no CODEX_HOME or HOME to find the Codex login in")?;
    let login = microluna::codex::Login::load(&path).map_err(|error| error.to_string())?;
    Ok(CodexLogin {
        path,
        expires_at: login.expires_at(),
    })
}

/// What answers a session's turns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Chosen {
    /// This target, from a Jev briefing.
    Delegate(Target),
    /// The door [`Door::from_env`] builds.
    Fallback,
}

/// A door choice and the reason for it, which the session header, the
/// trace, and `coder doctor` all say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    /// What answers.
    pub chosen: Chosen,
    /// Why, in one sentence.
    pub reason: String,
}

/// Chooses the door from the mode, the preferred target, an explicit door
/// request, and the targets found. Split from the environment so it is
/// decided without reading one.
///
/// `explicit` is a sentence naming a door the environment asks for by
/// name, such as `CODER_WORKER asks for the relay`.
///
/// # Errors
///
/// Returns a sentence when `always` cannot be honored: no target is
/// available, or the environment also names another door.
pub fn choose(
    mode: Mode,
    preferred: Option<Cli>,
    explicit: Option<&str>,
    targets: &[Target],
) -> Result<Choice, String> {
    if mode == Mode::Off {
        return Ok(Choice {
            chosen: Chosen::Fallback,
            reason: format!("{MODE_VAR}=off turns delegation off"),
        });
    }
    if let Some(explicit) = explicit {
        return match mode {
            Mode::Always => Err(format!(
                "{MODE_VAR}=always and {explicit}. Unset one of them."
            )),
            _ => Ok(Choice {
                chosen: Chosen::Fallback,
                reason: format!("{explicit}, which outranks delegation"),
            }),
        };
    }
    let considered: Vec<&Target> = targets
        .iter()
        .filter(|target| preferred.is_none_or(|agent| target.agent == agent))
        .collect();
    if let Some(target) = considered.iter().find(|target| target.available()) {
        let named = match preferred {
            Some(_) => format!(" and {AGENT_VAR} names it"),
            None => String::new(),
        };
        return Ok(Choice {
            chosen: Chosen::Delegate((*target).clone()),
            reason: format!("{}{named}", target.describe()),
        });
    }
    let found = if considered.is_empty() {
        "no target was considered".to_string()
    } else {
        considered
            .iter()
            .map(|target| target.describe())
            .collect::<Vec<_>>()
            .join("; ")
    };
    match mode {
        Mode::Always => Err(format!(
            "{MODE_VAR}=always and no delegation target is available: {found}"
        )),
        _ => Ok(Choice {
            chosen: Chosen::Fallback,
            reason: format!("fallback: no delegation target is available ({found})"),
        }),
    }
}

/// What the environment says about delegation, read once.
#[derive(Clone, Debug)]
pub struct Settings {
    /// [`MODE_VAR`], read.
    pub mode: Mode,
    /// [`AGENT_VAR`], when it names a target.
    pub preferred: Option<Cli>,
    /// [`MODEL_VAR`], when it names a model.
    pub model: Option<String>,
    /// The door the environment names explicitly, in a sentence.
    pub explicit: Option<String>,
    /// The targets this machine has.
    pub targets: Vec<Target>,
}

impl Settings {
    /// Reads the process environment.
    ///
    /// # Errors
    ///
    /// Returns a sentence when [`AGENT_VAR`] names neither target.
    pub fn read() -> Result<Self, String> {
        Ok(Settings {
            mode: Mode::read(env_value(MODE_VAR).as_deref()),
            preferred: env_value(AGENT_VAR)
                .map(|text| Cli::parse(&text).map_err(|why| format!("{AGENT_VAR}: {why}")))
                .transpose()?,
            model: env_value(MODEL_VAR),
            explicit: explicit_door(&env_value),
            targets: targets(env_value),
        })
    }

    /// The door these settings choose, and why.
    ///
    /// # Errors
    ///
    /// See [`choose`].
    pub fn choose(&self) -> Result<Choice, String> {
        choose(
            self.mode,
            self.preferred,
            self.explicit.as_deref(),
            &self.targets,
        )
    }
}

/// A variable's value, trimmed, when it is set and not blank.
#[must_use]
pub fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// The door a terminal or `coder -p` session answers through, and why.
///
/// # Errors
///
/// Returns a sentence when the environment asks for something that cannot
/// be built: two explicit doors, `always` with no target, or an unknown
/// [`AGENT_VAR`].
pub fn open() -> Result<(Door, String), String> {
    let settings = Settings::read()?;
    let choice = settings.choose()?;
    match choice.chosen {
        Chosen::Fallback => Ok((Door::from_env()?, choice.reason)),
        Chosen::Delegate(target) => {
            let workdir = std::env::current_dir()
                .map_err(|error| format!("the working directory: {error}"))?;
            let (jev, jev_source) = jev_from(&env_value);
            let door = DelegateDoor::new(target, settings.model, workdir, jev, jev_source);
            Ok((Door::Delegate(std::sync::Arc::new(door)), choice.reason))
        }
    }
}

/// The door the environment names explicitly, in a sentence, when it
/// names the relay or a local executor.
fn explicit_door(env: &impl Fn(&str) -> Option<String>) -> Option<String> {
    if env("CODER_WORKER").is_some() {
        Some("CODER_WORKER asks for the relay".to_string())
    } else if env(crate::executor_door::EXECUTOR_VAR).is_some() {
        Some(format!(
            "{} asks for a local executor",
            crate::executor_door::EXECUTOR_VAR
        ))
    } else {
        None
    }
}

/// The Jev client Coder One's judge asks through, and where its key came
/// from. `None` when this machine has no TypeSafe key.
pub fn jev_from(env: &impl Fn(&str) -> Option<String>) -> (Option<jev::Client>, String) {
    let Some(dir) = coder_one::credentials::openagents_dir() else {
        return (None, "no home directory to read jev.json from".to_string());
    };
    match coder_one::credentials::jev_key(env, &dir) {
        Ok(found) => match coder_one::credentials::jev_client(&found.secret) {
            Ok(client) => (
                Some(client),
                format!(
                    "{} from {}",
                    coder_one::credentials::JEV_MODEL,
                    found.source
                ),
            ),
            Err(why) => (None, why),
        },
        Err(why) => (None, why),
    }
}

/// A turn delegated to one target.
pub struct DelegateDoor {
    target: Target,
    model: String,
    /// The model the operator named, when one was named.
    named_model: Option<String>,
    /// What the session header records: `<agent>/<model>`.
    label: String,
    workdir: PathBuf,
    jev: Option<jev::Client>,
    jev_source: String,
    /// The executor's session, which the next turn resumes. Microluna
    /// never has one: each turn rebuilds its context.
    session: Mutex<Option<String>>,
    /// Scripted Microluna replies, for a test.
    script: Option<Vec<microluna::Reply>>,
    /// Turns this door has answered, which numbers their artifacts.
    turns: AtomicUsize,
    /// When the door opened, which names the artifacts directory.
    opened: u64,
    /// Where turns' artifacts go, when not under `~/.openagents`.
    artifacts: Option<PathBuf>,
}

impl std::fmt::Debug for DelegateDoor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DelegateDoor")
            .field("target", &self.target)
            .field("model", &self.model)
            .field("workdir", &self.workdir)
            .finish_non_exhaustive()
    }
}

/// What a delegated turn reports as it runs, in the turn's own terms.
#[derive(Clone, Debug)]
pub enum Update {
    /// A progress line from the probes, the judge, or the host.
    Line(String),
    /// Text the executor said.
    Text(String),
    /// A command the executor started.
    Proposed(Proposal),
    /// A command the executor finished.
    Ran(Outcome),
}

/// One delegated turn, answered or not.
#[derive(Debug)]
pub struct Delegated {
    /// The executor's final answer, or what it said before it failed.
    pub text: String,
    /// Why the turn did not answer, when it did not.
    pub failure: Option<GenerateError>,
    /// Tokens the executor reported, cache reads and writes counted as
    /// input.
    pub usage: Option<Usage>,
    /// The turn's cost in dollars, Jev and the executor together, when
    /// every part of it is known.
    pub cost_usd: Option<f64>,
    /// Every step the turn recorded, for the session's trace.
    pub steps: Vec<atif::Step>,
    /// The turn's summary record: agent, model, status, session, boundary,
    /// briefing, and usage.
    pub summary: Value,
    /// The model that answered.
    pub model: String,
    /// How many commands the executor ran.
    pub commands: usize,
}

impl DelegateDoor {
    /// A door onto `target`, running `model` when one is named.
    #[must_use]
    pub fn new(
        target: Target,
        model: Option<String>,
        workdir: PathBuf,
        jev: Option<jev::Client>,
        jev_source: String,
    ) -> Self {
        let resolved = model.clone().unwrap_or_else(|| match target.agent {
            Cli::ClaudeCode => terminal::policy().policy.executor.model,
            Cli::Codex | Cli::Microluna => target.agent.default_model().to_string(),
        });
        DelegateDoor {
            label: format!("{}/{resolved}", target.agent.word()),
            target,
            model: resolved,
            named_model: model,
            workdir,
            jev,
            jev_source,
            session: Mutex::new(None),
            script: None,
            turns: AtomicUsize::new(0),
            opened: atif::now_ms(),
            artifacts: None,
        }
    }

    /// The same door, writing each turn's briefing and stream under `dir`.
    #[must_use]
    pub fn writing_under(mut self, dir: PathBuf) -> Self {
        self.artifacts = Some(dir);
        self
    }

    /// The same door, with Microluna answering from `replies` in place of
    /// the Codex login. For tests.
    #[must_use]
    pub fn scripting(mut self, replies: Vec<microluna::Reply>) -> Self {
        self.script = Some(replies);
        self
    }

    /// The target this door runs.
    #[must_use]
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// `<agent>/<model>`, what the session header records as the model.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Where the Jev key came from, or why there is none.
    #[must_use]
    pub fn jev_source(&self) -> &str {
        &self.jev_source
    }

    /// The executor session the next turn resumes, when there is one.
    #[must_use]
    pub fn session(&self) -> Option<String> {
        self.session.lock().ok().and_then(|session| session.clone())
    }

    /// Where one turn's briefing and stream are written.
    fn artifacts(&self, turn: usize) -> PathBuf {
        let base = self.artifacts.clone().unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".openagents/coder/delegate"))
                .unwrap_or_else(std::env::temp_dir)
        });
        base.join(format!("{}-{turn}", self.opened))
    }

    /// Answers one turn. `request` is what the user asked; `earlier` is
    /// the conversation before it, which a resumed session already holds
    /// and so is not sent again. `read_only` puts the executor in a
    /// read-only boundary.
    ///
    /// # Errors
    ///
    /// Returns a `Stream` error when the turn's worker thread could not
    /// run. Every other failure is [`Delegated::failure`], so the caller
    /// can still record what the turn spent.
    pub async fn answer(
        &self,
        request: &str,
        earlier: &str,
        read_only: bool,
        clarify: bool,
        on: &mut (dyn FnMut(Update) + Send),
    ) -> Result<Delegated, GenerateError> {
        let turn = self.turns.fetch_add(1, Ordering::SeqCst) + 1;
        // A CLI resumes its session. Microluna rebuilds the context from
        // the conversation instead, so it always reads `earlier`.
        let resume = self.session().filter(|_| self.target.agent.is_cli());
        let request = terminal::Request {
            workdir: self.workdir.clone(),
            request: request.to_string(),
            earlier: if resume.is_some() {
                String::new()
            } else {
                earlier.to_string()
            },
            resume,
            read_only,
            clarify,
            agent: self.target.agent,
            model: self.named_model.clone(),
            binary: self.target.binary.clone(),
            credential: self.target.credential,
            jev: self.jev.clone(),
            artifacts: self.artifacts(turn),
            script: self.script.clone(),
            // The issue flow works in a clone of its own and opens a draft
            // pull request, so the operator's permit governs it, not this
            // turn's route.
            issues: crate::permit::Permit::operator().executes(),
            issue: false,
            review: false,
        };
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<FromThread>();
        // Coder One's judge and recorder are not `Send`, so the turn runs
        // on a thread of its own with a current-thread runtime, and its
        // progress comes back over the channel.
        let spawned = std::thread::Builder::new()
            .name("coder-delegate".to_string())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = tx.send(FromThread::Failed(error.to_string()));
                        return;
                    }
                };
                let progress = tx.clone();
                let answer = runtime.block_on(terminal::answer(
                    &request,
                    Rc::new(move |item| {
                        let _ = progress.send(FromThread::Progress(item));
                    }),
                ));
                let _ = tx.send(FromThread::Done(Box::new(answer)));
            });
        if let Err(error) = spawned {
            return Err(GenerateError::Stream(format!(
                "the delegate thread would not start: {error}"
            )));
        }
        let mut mapper = Mapper::default();
        let mut commands = 0usize;
        while let Some(message) = rx.recv().await {
            match message {
                FromThread::Progress(Progress::Line(line)) => on(Update::Line(line)),
                FromThread::Progress(Progress::Event(event)) => {
                    if let Some(update) = mapper.map(&event) {
                        if matches!(update, Update::Ran(_)) {
                            commands += 1;
                        }
                        on(update);
                    }
                }
                FromThread::Failed(why) => {
                    return Err(GenerateError::Stream(format!(
                        "the delegate thread could not run: {why}"
                    )));
                }
                FromThread::Done(answer) => {
                    if let Ok(mut session) = self.session.lock()
                        && answer.session_id.is_some()
                        && answer.agent.is_cli()
                    {
                        session.clone_from(&answer.session_id);
                    }
                    return Ok(delegated(&answer, commands));
                }
            }
        }
        Err(GenerateError::Stream(
            "the delegate thread ended without an answer".to_string(),
        ))
    }
}

/// What the worker thread sends back.
enum FromThread {
    Progress(Progress),
    Failed(String),
    Done(Box<terminal::Answer>),
}

/// A finished turn in this crate's terms.
fn delegated(answer: &terminal::Answer, commands: usize) -> Delegated {
    let report = &answer.report;
    let summary = &report.summary;
    let usage = summary.usage.as_ref().map(|_| {
        let input = [
            "input_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
        ]
        .iter()
        .filter_map(|key| summary.tokens(key))
        .sum();
        Usage {
            input_tokens: input,
            output_tokens: summary.tokens("output_tokens").unwrap_or(0),
        }
    });
    let agent = answer.agent.word();
    let failure = match &report.status {
        DelegateStatus::Answered => None,
        DelegateStatus::Refused(code) => Some(GenerateError::Refused {
            code: code.clone(),
            message: report.output(),
        }),
        DelegateStatus::TimedOut => Some(GenerateError::Quiet {
            heard: summary.result.is_some(),
            reason: format!("{agent} ran past its deadline"),
        }),
        DelegateStatus::Failed(code) => Some(GenerateError::Stream(format!(
            "{agent} exited {code}: {}",
            report.output()
        ))),
        DelegateStatus::Harness(why) => Some(GenerateError::Stream(format!(
            "{agent} could not run: {why}"
        ))),
        DelegateStatus::Transport { detail, .. } => Some(GenerateError::Stream(format!(
            "{agent} could not reach its provider: {detail}"
        ))),
    };
    Delegated {
        text: summary
            .result
            .clone()
            .unwrap_or_default()
            .trim()
            .to_string(),
        failure,
        usage,
        cost_usd: answer.cost_usd(),
        steps: answer.steps.clone(),
        summary: terminal::summary(answer),
        model: summary
            .model
            .clone()
            .unwrap_or_else(|| answer.model.clone()),
        commands,
    }
}

/// Turns normalized executor events into the turn's own updates.
///
/// Claude Code reports a finished tool by its tool-use ID rather than its
/// command, and reports every tool's result while only `Bash` starts a
/// command, so a finished command is paired with the oldest one started
/// and still open; a result with no open command is some other tool's
/// and is not a command outcome.
#[derive(Default)]
pub struct Mapper {
    open: Vec<(String, Instant)>,
}

impl Mapper {
    /// The update `event` is, when it is one the turn shows.
    pub fn map(&mut self, event: &StreamEvent) -> Option<Update> {
        match &event.kind {
            Kind::SessionStarted {
                session_id: Some(id),
            } => Some(Update::Line(format!("session ▸ {id}"))),
            Kind::SessionStarted { session_id: None } => None,
            Kind::AssistantClaim { text } if !text.trim().is_empty() => {
                Some(Update::Text(text.clone()))
            }
            Kind::AssistantClaim { .. } => None,
            Kind::CommandStarted { command } => {
                let shown = coder_one::stream::unwrap_shell(command);
                self.open.push((shown.clone(), Instant::now()));
                Some(Update::Proposed(Proposal {
                    command: shown,
                    why: "the executor ran it".to_string(),
                }))
            }
            Kind::CommandCompleted {
                command,
                exit_code,
                output,
            } => {
                let shown = coder_one::stream::unwrap_shell(command);
                let index = self
                    .open
                    .iter()
                    .position(|(open, _)| *open == shown)
                    .or_else(|| (!self.open.is_empty()).then_some(0))?;
                let (command, started) = self.open.remove(index);
                Some(Update::Ran(Outcome {
                    proposal: Proposal {
                        command,
                        why: "the executor ran it".to_string(),
                    },
                    status: match exit_code {
                        Some(code) => Status::Exit(i32::try_from(*code).unwrap_or(i32::MAX)),
                        None => Status::Failed("the executor reported no exit code".to_string()),
                    },
                    bytes: output.len() as u64,
                    output: output.clone(),
                    elapsed: started.elapsed(),
                }))
            }
            Kind::ArtifactChanged { path, change } => {
                Some(Update::Line(format!("{change} ▸ {path}")))
            }
            Kind::UsageUpdate { .. } | Kind::SessionEnded { .. } => None,
        }
    }
}

/// The conversation before its last message, rendered for a briefing.
#[must_use]
pub fn earlier(transcript: &[Message]) -> String {
    let before = match transcript.split_last() {
        Some((_, before)) => before,
        None => transcript,
    };
    before
        .iter()
        .map(|message| {
            let who = match message.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
            };
            format!("{who}: {}", message.text.trim())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl Generate for DelegateDoor {
    /// One generation through the executor: the last message is the
    /// request, the rest is the conversation, and the turn only reads.
    /// A caller that holds a permit runs [`DelegateDoor::answer`] instead.
    async fn generate<'a>(
        &'a self,
        _instructions: &'a str,
        input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        let request = input
            .last()
            .map(|message| message.text.clone())
            .unwrap_or_default();
        let done = self
            .answer(&request, &earlier(input), true, false, &mut |update| {
                if let Update::Line(line) = update {
                    meta(Meta::Judgment(line));
                }
            })
            .await?;
        if let Some(failure) = done.failure {
            return Err(failure);
        }
        sink(&done.text);
        meta(Meta::Model(done.model.clone()));
        Ok((done.text, done.usage))
    }
}

/// How long a turn may take before the executor's own deadline stops it:
/// the reference policy's, for `coder doctor` to state.
#[must_use]
pub fn deadline() -> Duration {
    Duration::from_secs(terminal::policy().policy.executor.deadline_sec)
}

/// Whether this host can enforce the boundary a delegated turn runs in.
///
/// # Errors
///
/// Returns the sentence the boundary refuses with.
pub fn boundary_available(workdir: &Path) -> Result<(), String> {
    let scratch = std::env::temp_dir().join(format!("coder-boundary-check-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).map_err(|error| format!("{}: {error}", scratch.display()))?;
    let built = terminal::boundary(true, workdir, &scratch).map(|_| ());
    let _ = std::fs::remove_dir(&scratch);
    built
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(agent: Cli, installed: bool, credential: Credential) -> Target {
        Target {
            agent,
            binary: installed.then(|| PathBuf::from(format!("/bin/{}", agent.program()))),
            credential,
            problem: None,
        }
    }

    /// Microluna on a Codex login, with `problem` when it can't be used,
    /// ahead of both CLIs as [`targets`] orders them.
    fn all(login: Option<Option<&str>>, claude: bool, codex: bool) -> Vec<Target> {
        let mut targets = vec![Target {
            agent: Cli::Microluna,
            binary: None,
            credential: if login.is_some() {
                Credential::CodexAuthFile
            } else {
                Credential::Missing
            },
            problem: login.flatten().map(str::to_string),
        }];
        targets.extend(both(claude, codex));
        targets
    }

    #[test]
    fn auto_prefers_microluna_on_a_usable_codex_login() {
        let chosen = |targets: &[Target]| choose(Mode::Auto, None, None, targets).unwrap();
        let usable = all(Some(None), true, true);
        let choice = chosen(&usable);
        assert_eq!(choice.chosen, Chosen::Delegate(usable[0].clone()));
        assert!(
            choice
                .reason
                .contains("runs in this process on the Codex login"),
            "{}",
            choice.reason
        );
        // A token about to expire, or no login at all, falls through to
        // Claude Code, then Codex, then the Open Responses door.
        let expiring = all(
            Some(Some("the Codex access token expires in 60 s")),
            true,
            true,
        );
        assert_eq!(
            chosen(&expiring).chosen,
            Chosen::Delegate(expiring[1].clone())
        );
        let none = all(None, false, true);
        assert_eq!(chosen(&none).chosen, Chosen::Delegate(none[2].clone()));
        let fallback = chosen(&all(Some(Some("expired")), false, false));
        assert_eq!(fallback.chosen, Chosen::Fallback);
        assert!(
            fallback
                .reason
                .contains("microluna can't use the Codex login: expired"),
            "{}",
            fallback.reason
        );
        // The operator's named agent still outranks the default.
        let named = choose(Mode::Auto, Some(Cli::ClaudeCode), None, &usable).unwrap();
        assert_eq!(named.chosen, Chosen::Delegate(usable[1].clone()));
    }

    #[test]
    fn a_microluna_target_needs_no_binary_but_a_usable_login() {
        let [microluna, ..] = &all(Some(None), false, false)[..] else {
            unreachable!()
        };
        assert!(microluna.available());
        let [expiring, ..] = &all(Some(Some("expiring")), false, false)[..] else {
            unreachable!()
        };
        assert!(!expiring.available());
        let [missing, ..] = &all(None, false, false)[..] else {
            unreachable!()
        };
        assert!(!missing.available());
        assert_eq!(missing.describe(), "microluna has no Codex login");
    }

    #[test]
    fn the_codex_login_reports_its_expiry_and_never_its_token() {
        let dir = tempfile::tempdir().unwrap();
        let env = |home: &Path| {
            let home = home.to_string_lossy().into_owned();
            move |name: &str| (name == "CODEX_HOME").then(|| home.clone())
        };
        let missing = codex_login(&env(dir.path())).unwrap_err();
        assert!(missing.contains("no Codex login"), "{missing}");
        // A JWT whose payload is {"exp": 4102444800}, 2100-01-01.
        let payload = "eyJleHAiOjQxMDI0NDQ4MDB9";
        let token = format!("h.{payload}.s");
        std::fs::write(
            dir.path().join("auth.json"),
            serde_json::json!({
                "auth_mode": "chatgpt",
                "tokens": { "access_token": token, "account_id": "acct", "refresh_token": "r" }
            })
            .to_string(),
        )
        .unwrap();
        let login = codex_login(&env(dir.path())).unwrap();
        assert_eq!(login.expires_at, Some(4_102_444_800));
        assert_eq!(login.hours_left(4_102_444_800 - 7_200), Some(2.0));
        assert!(!format!("{login:?}").contains(&token));
    }

    fn both(claude: bool, codex: bool) -> Vec<Target> {
        vec![
            target(
                Cli::ClaudeCode,
                claude,
                if claude {
                    Credential::CliLogin
                } else {
                    Credential::Missing
                },
            ),
            target(
                Cli::Codex,
                codex,
                if codex {
                    Credential::CodexAuthFile
                } else {
                    Credential::Missing
                },
            ),
        ]
    }

    #[test]
    fn auto_prefers_claude_then_codex_then_the_fallback() {
        let chosen = |targets: &[Target]| choose(Mode::Auto, None, None, targets).unwrap();
        assert_eq!(
            chosen(&both(true, true)).chosen,
            Chosen::Delegate(both(true, true)[0].clone())
        );
        assert_eq!(
            chosen(&both(false, true)).chosen,
            Chosen::Delegate(both(false, true)[1].clone())
        );
        let fallback = chosen(&both(false, false));
        assert_eq!(fallback.chosen, Chosen::Fallback);
        assert!(
            fallback.reason.starts_with("fallback: "),
            "{}",
            fallback.reason
        );
        assert!(fallback.reason.contains("claude-code is not installed"));
    }

    #[test]
    fn an_installed_target_without_a_credential_is_not_available() {
        let targets = vec![target(Cli::ClaudeCode, true, Credential::Missing)];
        let choice = choose(Mode::Auto, None, None, &targets).unwrap();
        assert_eq!(choice.chosen, Chosen::Fallback);
        assert!(
            choice.reason.contains("has no credential"),
            "{}",
            choice.reason
        );
    }

    #[test]
    fn the_named_agent_is_the_only_one_considered() {
        let choice = choose(Mode::Auto, Some(Cli::Codex), None, &both(true, true)).unwrap();
        assert_eq!(choice.chosen, Chosen::Delegate(both(true, true)[1].clone()));
        assert!(choice.reason.contains(AGENT_VAR));
        let missing = choose(Mode::Auto, Some(Cli::Codex), None, &both(true, false)).unwrap();
        assert_eq!(missing.chosen, Chosen::Fallback);
    }

    #[test]
    fn off_never_delegates_and_always_never_falls_back() {
        let off = choose(Mode::Off, None, None, &both(true, true)).unwrap();
        assert_eq!(off.chosen, Chosen::Fallback);
        assert!(off.reason.contains("=off"));
        let refused = choose(Mode::Always, None, None, &both(false, false)).unwrap_err();
        assert!(refused.contains("no delegation target"), "{refused}");
    }

    #[test]
    fn an_explicit_door_outranks_auto_and_contradicts_always() {
        let relay = "CODER_WORKER asks for the relay";
        let auto = choose(Mode::Auto, None, Some(relay), &both(true, true)).unwrap();
        assert_eq!(auto.chosen, Chosen::Fallback);
        assert!(auto.reason.contains("outranks delegation"));
        assert!(choose(Mode::Always, None, Some(relay), &both(true, true)).is_err());
    }

    #[test]
    fn the_mode_reads_its_three_words_and_treats_anything_else_as_auto() {
        assert_eq!(Mode::read(None), Mode::Auto);
        assert_eq!(Mode::read(Some("ALWAYS")), Mode::Always);
        assert_eq!(Mode::read(Some("off")), Mode::Off);
        assert_eq!(Mode::read(Some("devin-local")), Mode::Auto);
        assert_eq!(Mode::parse("devin-local"), None);
    }

    fn event(kind: Kind) -> StreamEvent {
        StreamEvent {
            seq: 1,
            line: 1,
            offset: None,
            kind,
        }
    }

    #[test]
    fn executor_events_become_the_turns_own() {
        let mut mapper = Mapper::default();
        assert!(matches!(
            mapper.map(&event(Kind::AssistantClaim { text: "Reading.".to_string() })),
            Some(Update::Text(text)) if text == "Reading."
        ));
        assert!(matches!(
            mapper.map(&event(Kind::CommandStarted { command: "/bin/bash -lc 'ls crates'".to_string() })),
            Some(Update::Proposed(proposal)) if proposal.command == "ls crates"
        ));
        // Claude Code names the finished tool by its ID.
        let Some(Update::Ran(outcome)) = mapper.map(&event(Kind::CommandCompleted {
            command: "toolu_01".to_string(),
            exit_code: Some(0),
            output: "gym\nkev\n".to_string(),
        })) else {
            panic!("a finished command is an outcome");
        };
        assert_eq!(outcome.proposal.command, "ls crates");
        assert!(matches!(outcome.status, Status::Exit(0)));
        assert_eq!(outcome.bytes, 8);
        // A result with no open command is a read, not a command.
        assert!(
            mapper
                .map(&event(Kind::CommandCompleted {
                    command: "toolu_02".to_string(),
                    exit_code: Some(0),
                    output: "file contents".to_string(),
                }))
                .is_none()
        );
        assert!(matches!(
            mapper.map(&event(Kind::ArtifactChanged { path: "a.rs".to_string(), change: "update".to_string() })),
            Some(Update::Line(line)) if line == "update ▸ a.rs"
        ));
        assert!(
            mapper
                .map(&event(Kind::SessionEnded {
                    error: false,
                    result: None
                }))
                .is_none()
        );
    }

    /// A stand-in Claude Code that tries to write `marker` in its working
    /// directory and answers with what happened and the session it was
    /// started or resumed under, `s-1` when it was named none.
    const WRITER: &str = r#"#!/bin/sh
id=s-1
while [ $# -gt 0 ]; do
  case "$1" in --session-id|--resume) id=$2; shift ;; esac
  shift
done
cat > /dev/null
printf x > marker 2>/dev/null && wrote=wrote || wrote=refused
echo "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"$id\",\"model\":\"stand-in\"}"
echo "{\"type\":\"assistant\",\"message\":{\"id\":\"m1\",\"content\":[{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"Bash\",\"input\":{\"command\":\"printf x > marker\"}}],\"usage\":{\"input_tokens\":5}}}"
echo "{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"toolu_1\",\"content\":\"$wrote\",\"is_error\":false}]}}"
echo "{\"type\":\"assistant\",\"message\":{\"id\":\"m2\",\"content\":[{\"type\":\"text\",\"text\":\"the write was $wrote in $id\"}],\"usage\":{\"input_tokens\":5}}}"
echo "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":1,\"result\":\"the write was $wrote in $id\",\"session_id\":\"$id\",\"total_cost_usd\":0.01,\"usage\":{\"input_tokens\":10,\"output_tokens\":3}}"
"#;

    /// An agent whose delegate door runs [`WRITER`] in a fresh workspace,
    /// or `None` on a host that cannot enforce a boundary.
    fn writer(dir: &Path) -> Option<(crate::agent::Agent, PathBuf)> {
        let workdir = dir.join("work");
        std::fs::create_dir_all(&workdir).unwrap();
        if let Err(why) = boundary_available(&workdir) {
            eprintln!("skipped: {why}");
            return None;
        }
        let binary = coder_one::adapter::standin::install(&dir.join("bin"), "claude", WRITER);
        let target = Target {
            agent: Cli::ClaudeCode,
            binary: Some(binary),
            credential: Credential::CliLogin,
            problem: None,
        };
        let door = DelegateDoor::new(target, None, workdir.clone(), None, String::new())
            .writing_under(dir.join("artifacts"));
        let agent = crate::agent::Agent::new(None, Door::Delegate(std::sync::Arc::new(door)));
        Some((agent, workdir))
    }

    async fn turn(
        agent: &mut crate::agent::Agent,
        permit: crate::permit::Permit,
        draft: &str,
    ) -> (crate::agent::Turned, Vec<crate::shell::ShellEvent>, String) {
        agent.push_user(draft);
        let mut shell = Vec::new();
        let mut streamed = String::new();
        let turned = agent
            .turn(
                false,
                permit,
                &mut |delta| streamed.push_str(delta),
                &mut |_| {},
                &mut |event| shell.push(event),
            )
            .await
            .unwrap();
        (turned, shell, streamed)
    }

    #[tokio::test]
    async fn the_permit_decides_whether_the_executor_may_write_and_a_follow_up_resumes() {
        let dir = tempfile::tempdir().unwrap();
        let Some((mut agent, workdir)) = writer(dir.path()) else {
            return;
        };
        let (read, shell, streamed) = turn(
            &mut agent,
            crate::permit::Permit::answering(),
            "what is here?",
        )
        .await;
        let (said, session) = read.text.split_once(" in ").unwrap();
        assert_eq!(said, "the write was refused");
        assert_ne!(session, "s-1", "a new session is started under a named ID");
        assert_eq!(streamed, read.text);
        assert!(!workdir.join("marker").exists(), "a read-only turn wrote");
        assert_eq!(read.cost_usd, Some(0.01));
        assert_eq!(read.commands, 1);
        assert!(matches!(
            &shell[..],
            [crate::shell::ShellEvent::Proposed(proposal), crate::shell::ShellEvent::Ran(_)]
                if proposal.command == "printf x > marker"
        ));

        let (wrote, _, _) = turn(
            &mut agent,
            crate::permit::Permit::executing(),
            "now write the marker",
        )
        .await;
        assert_eq!(
            wrote.text,
            format!("the write was wrote in {session}"),
            "a follow-up resumes the first turn's session"
        );
        assert!(
            workdir.join("marker").exists(),
            "a permitted turn could not write"
        );
        assert_eq!(agent.transcript().len(), 4);
    }

    #[tokio::test]
    async fn a_microluna_turn_streams_as_shell_events_and_a_follow_up_rebuilds_its_context() {
        use microluna::fake::call;
        let dir = tempfile::tempdir().unwrap();
        let workdir = dir.path().join("work");
        std::fs::create_dir_all(&workdir).unwrap();
        if let Err(why) = boundary_available(&workdir) {
            eprintln!("skipped: {why}");
            return;
        }
        let usage = microluna::TokenUsage {
            input: 1_000,
            cached: 0,
            output: 20,
            reasoning: 0,
        };
        let target = Target {
            agent: Cli::Microluna,
            binary: None,
            credential: Credential::CodexAuthFile,
            problem: None,
        };
        let door = DelegateDoor::new(target, None, workdir.clone(), None, String::new())
            .writing_under(dir.path().join("artifacts"))
            .scripting(vec![
                call(
                    "c1",
                    "run_command",
                    &serde_json::json!({ "command": "printf x > marker", "timeout_seconds": 10 }),
                    usage,
                ),
                call(
                    "c2",
                    "finish",
                    &serde_json::json!({ "status": "done", "summary": "Tried the marker.", "answer": "Here is the answer." }),
                    usage,
                ),
            ]);
        assert_eq!(door.label(), "microluna/gpt-6-luna");
        let door = std::sync::Arc::new(door);
        let mut agent =
            crate::agent::Agent::new(None, Door::Delegate(std::sync::Arc::clone(&door)));

        let (read, shell, streamed) = turn(
            &mut agent,
            crate::permit::Permit::answering(),
            "what is here?",
        )
        .await;
        assert_eq!(read.text, "Here is the answer.");
        assert_eq!(streamed, read.text);
        assert!(!workdir.join("marker").exists(), "a read-only turn wrote");
        assert_eq!(read.commands, 1);
        assert!(read.cost_usd.is_some_and(|usd| usd > 0.0));
        assert!(
            matches!(
                &shell[..],
                [crate::shell::ShellEvent::Proposed(proposal), crate::shell::ShellEvent::Ran(outcome)]
                    if proposal.command == "printf x > marker" && !matches!(outcome.status, Status::Exit(0))
            ),
            "{shell:?}"
        );
        assert_eq!(door.session(), None, "Microluna keeps no session to resume");

        let (wrote, shell, _) = turn(
            &mut agent,
            crate::permit::Permit::executing(),
            "now write the marker",
        )
        .await;
        assert_eq!(wrote.text, "Here is the answer.");
        assert!(
            workdir.join("marker").exists(),
            "a permitted turn could not write"
        );
        assert!(
            matches!(
                &shell[..],
                [crate::shell::ShellEvent::Proposed(_), crate::shell::ShellEvent::Ran(outcome)]
                    if matches!(outcome.status, Status::Exit(0))
            ),
            "{shell:?}"
        );
    }

    #[test]
    fn the_conversation_before_the_request_is_rendered_without_it() {
        let transcript = [
            Message {
                role: Role::User,
                text: "what is gym?".to_string(),
            },
            Message {
                role: Role::Assistant,
                text: "The measurement plane.".to_string(),
            },
            Message {
                role: Role::User,
                text: "and kev?".to_string(),
            },
        ];
        assert_eq!(
            earlier(&transcript),
            "User: what is gym?\nAssistant: The measurement plane."
        );
        assert_eq!(earlier(&transcript[..1]), "");
    }
}
