//! The delegate door: a turn answered by Microcoder in this process, or by
//! Claude Code or Codex from a Jev briefing, with the Open Responses door
//! as the fallback.
//!
//! Microcoder ([`microcoder`]) is the simple loop: Jev judges the state,
//! one structured model call returns the next commands, and they run in
//! the working directory inside a `coder-boundary` boundary. It generates
//! through the first connected provider with capacity (the Codex login,
//! then Claude Code's login), reading the capacity book the auto-start
//! policy reads, and fails over to the next one when a provider refuses
//! for a usage or rate limit during the turn.
//!
//! A turn delegated to Claude Code or Codex CLI is four steps, all of them
//! Coder One's library rather than a second copy here:
//!
//! 1. The host runs the read-only probe battery in the working directory.
//! 2. Jev judges the probes and candidate files.
//! 3. Code packs a briefing from the request and the conversation.
//! 4. The executor runs it, inside a `coder-boundary` boundary.
//!
//! See [`coder_delegate::terminal`] for the four steps and the policy they run.
//!
//! # Which door answers
//!
//! [`choose`] decides, from [`MODE_VAR`], [`AGENT_VAR`], the doors the
//! environment names explicitly, and the targets this machine has:
//!
//! - `CODER_DELEGATE=auto`, the default, delegates to Microcoder when a
//!   provider it can use is connected and has capacity; Microcoder runs in
//!   this process, so nothing needs installing. Otherwise it delegates to
//!   an installed and authenticated `claude`, then `codex`, when its login
//!   has capacity, and falls back to the door [`Door::from_env`] builds
//!   when none is available, saying why. When that fallback would be the
//!   stub and a target was skipped for capacity, every turn ends with one
//!   sentence naming each provider and when it resets.
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

pub mod microcoder;

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use coder_delegate::delegate::{Agent as Cli, Credential, Status as DelegateStatus};
use coder_delegate::stream::{Event as StreamEvent, Kind};
use coder_delegate::terminal::{self, Progress};
use serde_json::Value;

use crate::generate::{Door, Generate, GenerateError, Message, Meta, Role, StubGenerate, Usage};
use crate::shell::{Outcome, Proposal, Status};
use crate::task::capacity::{self, Connection, Provider, Refusal};
use microcoder::ProviderState;

/// The variable that turns delegation on, off, or on unconditionally:
/// `auto`, `always`, or `off`.
///
/// Before this door existed the same variable named the capability a
/// program's `delegate` step hands work to, and it still does when its
/// value is none of those three words. See [`crate::agent::DELEGATE_VAR`].
pub const MODE_VAR: &str = "CODER_DELEGATE";

/// The variable that names the target: `microcoder`, `claude-code`, or
/// `codex`. `microluna` still names Microcoder, which replaced it.
pub const AGENT_VAR: &str = "CODER_DELEGATE_AGENT";

/// The variable that names the target's model. For Microcoder it names
/// the Codex model.
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

/// An executor a turn may be delegated to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    /// The Microcoder loop, in this process.
    Microcoder,
    /// An installed CLI, from a Coder One briefing.
    Cli(Cli),
}

impl Agent {
    /// The executor `text` names.
    ///
    /// # Errors
    ///
    /// A sentence when it names none.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim().to_ascii_lowercase().as_str() {
            // Microcoder replaced Microluna on 2026-09-28.
            "microcoder" | "microluna" => Ok(Agent::Microcoder),
            other => match Cli::parse(other)? {
                Cli::Microluna => Ok(Agent::Microcoder),
                cli => Ok(Agent::Cli(cli)),
            },
        }
    }

    /// The executor's word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Agent::Microcoder => microcoder::WORD,
            Agent::Cli(cli) => cli.word(),
        }
    }

    /// Whether it is an installed CLI rather than this process.
    #[must_use]
    pub fn is_cli(self) -> bool {
        matches!(self, Agent::Cli(_))
    }

    /// The provider whose capacity a CLI spends: Claude Code spends the
    /// Claude login, Codex CLI the Codex login.
    #[must_use]
    pub fn provider(self) -> Option<Provider> {
        match self {
            Agent::Cli(Cli::ClaudeCode) => Some(Provider::Claude),
            Agent::Cli(Cli::Codex) => Some(Provider::Codex),
            _ => None,
        }
    }
}

/// One executor this machine might delegate to, and what it has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// Which executor.
    pub agent: Agent,
    /// Where its binary is, when it is installed. Microcoder runs in this
    /// process and has none.
    pub binary: Option<PathBuf>,
    /// Where its credential comes from, by name. Microcoder's providers
    /// say where theirs stand instead.
    pub credential: Credential,
    /// Why a credential that was found can't carry a turn, such as a
    /// login out of its usage limit.
    pub problem: Option<String>,
    /// The refusal in the capacity book that keeps a CLI's login from
    /// work, when one holds.
    pub refusal: Option<Refusal>,
    /// Microcoder's providers in preference order, and where each stands.
    /// Empty for a CLI.
    pub providers: Vec<ProviderState>,
}

impl Target {
    /// What `env` says about the CLI `agent`: its binary, its credential,
    /// and whether its login has capacity in `book` at `now`.
    pub fn find(
        agent: Cli,
        env: impl Fn(&str) -> Option<String>,
        book: &capacity::Book,
        now: u64,
    ) -> Self {
        let (binary, credential) = coder_delegate::delegate::resolve(agent, &env);
        let agent = Agent::Cli(agent);
        let refusal = agent
            .provider()
            .and_then(|provider| book.blocking(provider, now))
            .cloned();
        Target {
            agent,
            binary,
            credential,
            problem: refusal
                .as_ref()
                .map(|refusal| format!("its login {}", microcoder::blocked(refusal))),
            refusal,
            providers: Vec::new(),
        }
    }

    /// Microcoder, over `providers`.
    #[must_use]
    pub fn microcoder(providers: Vec<ProviderState>) -> Self {
        Target {
            agent: Agent::Microcoder,
            binary: None,
            credential: Credential::Missing,
            problem: None,
            refusal: None,
            providers,
        }
    }

    /// The provider Microcoder starts on: the first connected one with
    /// capacity.
    #[must_use]
    pub fn provider(&self) -> Option<&ProviderState> {
        self.providers.iter().find(|state| state.usable())
    }

    /// Whether a turn could run on it: Microcoder with a provider it can
    /// start on, or a CLI installed and authenticated with a login that
    /// has capacity.
    #[must_use]
    pub fn available(&self) -> bool {
        match self.agent {
            Agent::Microcoder => self.provider().is_some(),
            Agent::Cli(_) => {
                self.binary.is_some()
                    && self.credential != Credential::Missing
                    && self.problem.is_none()
            }
        }
    }

    /// Whether it was passed over only for capacity: a connected
    /// provider, or an installed and authenticated CLI, whose login is out
    /// of its limit.
    #[must_use]
    pub fn out_of_capacity(&self) -> bool {
        match self.agent {
            Agent::Microcoder => {
                self.provider().is_none()
                    && self
                        .providers
                        .iter()
                        .any(|state| state.connection.is_connected() && state.refusal.is_some())
            }
            Agent::Cli(_) => {
                self.binary.is_some()
                    && self.credential != Credential::Missing
                    && self.refusal.is_some()
            }
        }
    }

    /// One sentence on where it stands.
    #[must_use]
    pub fn describe(&self) -> String {
        if self.agent == Agent::Microcoder {
            let every = || {
                self.providers
                    .iter()
                    .map(ProviderState::describe)
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            return match self.provider() {
                Some(state) => {
                    let skipped: Vec<String> = self
                        .providers
                        .iter()
                        .take_while(|other| other.provider != state.provider)
                        .map(|other| format!("{} because {}", other.provider, other.describe()))
                        .collect();
                    format!(
                        "microcoder runs in this process on {} ({}){}",
                        state.provider,
                        state.model,
                        if skipped.is_empty() {
                            String::new()
                        } else {
                            format!("; it skips {}", skipped.join(" and "))
                        }
                    )
                }
                None => format!("microcoder has no provider to use: {}", every()),
            };
        }
        match (&self.binary, self.credential, &self.problem) {
            (None, _, _) => format!("{} is not installed", self.agent.word()),
            (Some(path), Credential::Missing, _) => format!(
                "{} is installed at {} and has no credential",
                self.agent.word(),
                path.display()
            ),
            (Some(_), _, Some(problem)) => format!("{}: {problem}", self.agent.word()),
            (Some(path), credential, None) => format!(
                "{} is installed at {} and authenticated ({})",
                self.agent.word(),
                path.display(),
                credential.word()
            ),
        }
    }
}

/// The directory of the capacity book: the task store,
/// `~/.openagents/tasks`, where the auto-start policy and repository runs
/// keep it.
#[must_use]
pub fn capacity_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents/tasks"))
}

/// The targets this machine has, in the order `auto` prefers them:
/// Microcoder, then Claude Code, then Codex CLI. `model` names
/// Microcoder's Codex model; `book` is the capacity book's directory,
/// and `probe` says whether a provider has a usable login.
///
/// The OpenAgents cloud is the fallback when nothing else is set up, so an
/// own door key (`CODER_DOOR_KEY` or `CODER_AI_GATEWAY_KEY`) takes it off
/// Microcoder's providers: the operator's own door answers instead.
pub fn targets(
    env: impl Fn(&str) -> Option<String>,
    model: Option<&str>,
    book: &Path,
    probe: &dyn Fn(Provider) -> Connection,
    now: u64,
) -> Vec<Target> {
    let recorded = capacity::Book::load(book);
    let own_door = OWN_DOOR_VARS.into_iter().find(|name| env(name).is_some());
    let probe = |provider: Provider| match (provider, own_door) {
        (Provider::Vertex, Some(name)) => Connection::Missing(format!(
            "{name} names this host's own door, which answers in the cloud's place"
        )),
        _ => probe(provider),
    };
    let mut found = vec![Target::microcoder(microcoder::providers(
        &microcoder::lineup(model),
        book,
        &probe,
        now,
    ))];
    found.extend(
        [Cli::ClaudeCode, Cli::Codex]
            .into_iter()
            .map(|agent| Target::find(agent, &env, &recorded, now)),
    );
    found
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

/// Reads the Codex login Microcoder generates through, only to report on
/// it: where it is and when its access token expires. It never refreshes
/// the login and never returns a token.
///
/// # Errors
///
/// A sentence when the login is missing, unreadable, not a ChatGPT
/// sign-in, or within ten minutes of expiring.
pub fn codex_login(env: &impl Fn(&str) -> Option<String>) -> Result<CodexLogin, String> {
    let path = coder_delegate::delegate::codex_auth_file(env)
        .ok_or("no CODEX_HOME or HOME to find the Codex login in")?;
    let login = codex_transport::codex::Login::load(&path).map_err(|error| error.to_string())?;
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
    preferred: Option<Agent>,
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
                reason: format!("{explicit}, which takes precedence over delegation"),
            }),
        };
    }
    let considered: Vec<&Target> = targets
        .iter()
        .filter(|target| preferred.is_none_or(|agent| target.agent == agent))
        .collect();
    // The cloud is the last resort: a CLI that can answer is preferred to
    // Microcoder when the cloud is all Microcoder has.
    let on_the_cloud = |target: &Target| {
        target.agent == Agent::Microcoder
            && target
                .provider()
                .is_some_and(|state| state.provider == Provider::Vertex)
    };
    let found = considered
        .iter()
        .find(|target| target.available() && !on_the_cloud(target))
        .or_else(|| considered.iter().find(|target| target.available()));
    if let Some(target) = found {
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
        "no target matched the requested agent".to_string()
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
            reason: format!("no delegation target is available ({found})"),
        }),
    }
}

/// The sentence a session with no model to answer ends every turn with:
/// each target and where it stands, reset times included.
#[must_use]
pub fn no_capacity(targets: &[Target]) -> String {
    let parts: Vec<String> = targets.iter().map(Target::describe).collect();
    format!(
        "No model can answer now: {}. Set CODER_DOOR_KEY to answer through the Open Responses door instead.",
        parts.join("; ")
    )
}

/// What the environment says about delegation, read once.
#[derive(Clone, Debug)]
pub struct Settings {
    /// [`MODE_VAR`], read.
    pub mode: Mode,
    /// [`AGENT_VAR`], when it names a target.
    pub preferred: Option<Agent>,
    /// [`MODEL_VAR`], when it names a model.
    pub model: Option<String>,
    /// The door the environment names explicitly, in a sentence.
    pub explicit: Option<String>,
    /// The capacity book's directory.
    pub book: PathBuf,
    /// The targets this machine has.
    pub targets: Vec<Target>,
}

impl Settings {
    /// Reads the process environment, the logins this host has, and the
    /// capacity book.
    ///
    /// # Errors
    ///
    /// Returns a sentence when [`AGENT_VAR`] names no target.
    pub fn read() -> Result<Self, String> {
        let model = env_value(MODEL_VAR);
        let book = capacity_dir().unwrap_or_else(std::env::temp_dir);
        Ok(Settings {
            mode: Mode::read(env_value(MODE_VAR).as_deref()),
            preferred: env_value(AGENT_VAR)
                .map(|text| Agent::parse(&text).map_err(|why| format!("{AGENT_VAR}: {why}")))
                .transpose()?,
            explicit: explicit_door(&env_value),
            targets: targets(
                env_value,
                model.as_deref(),
                &book,
                &capacity::probe,
                microcoder::now(),
            ),
            model,
            book,
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
        Chosen::Fallback => {
            let door = Door::from_env()?;
            if matches!(door, Door::Stub(_))
                && settings.mode != Mode::Off
                && settings.targets.iter().any(Target::out_of_capacity)
            {
                let sentence = no_capacity(&settings.targets);
                return Ok((Door::Stub(StubGenerate::refusing(sentence)), choice.reason));
            }
            Ok((door, choice.reason))
        }
        Chosen::Delegate(target) => {
            let workdir = std::env::current_dir()
                .map_err(|error| format!("the working directory: {error}"))?;
            let (jev, jev_source) = jev_from(&env_value);
            let door = DelegateDoor::new(target, settings.model, workdir, jev, jev_source)
                .reading_capacity_in(settings.book);
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

/// The variables that name this host's own Open Responses door key.
const OWN_DOOR_VARS: [&str; 2] = ["CODER_DOOR_KEY", "CODER_AI_GATEWAY_KEY"];

/// The Jev client Coder One's judge asks through, and how it reaches Jev:
/// this computer's TypeSafe key, or the OpenAgents hosted decision service
/// when there is none (`coder_delegate::credentials::jev`). `None` only
/// when neither is available, with the reason.
pub fn jev_from(env: &impl Fn(&str) -> Option<String>) -> (Option<jev::Client>, String) {
    let Some(dir) = coder_delegate::credentials::openagents_dir() else {
        return (None, "no home directory to read jev.json from".to_string());
    };
    match coder_delegate::credentials::jev(env, &dir) {
        Ok(resolved) => (
            Some(resolved.client),
            format!(
                "{} through {}",
                coder_delegate::credentials::JEV_MODEL,
                resolved.via
            ),
        ),
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
    /// The executor's session, which the next turn resumes. Microcoder
    /// never has one: each turn rebuilds its context.
    session: Mutex<Option<String>>,
    /// Scripted Microcoder replies per provider, for a test.
    script: Option<Vec<(Provider, microcoder::Script)>>,
    /// The door Microcoder's cloud lane talks to in place of the
    /// OpenAgents relay, for a test.
    cloud: Option<std::sync::Arc<Door>>,
    /// Whether a turn that asks to work a GitHub issue runs the issue flow:
    /// the operator's permit runs commands.
    issues: bool,
    /// The capacity book's directory, which Microcoder reads before each
    /// turn and writes when a provider refuses.
    book: PathBuf,
    /// The clock Microcoder's failover reads, in Unix seconds.
    now: fn() -> u64,
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
        let resolved = match target.agent {
            Agent::Microcoder => target.provider().or(target.providers.first()).map_or_else(
                || microcoder::CODEX_MODEL.to_string(),
                |state| state.model.clone(),
            ),
            Agent::Cli(Cli::ClaudeCode) => model
                .clone()
                .unwrap_or_else(|| terminal::policy().executor.model),
            Agent::Cli(cli) => model
                .clone()
                .unwrap_or_else(|| cli.default_model().to_string()),
        };
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
            cloud: None,
            issues: crate::permit::Permit::operator().executes(),
            book: capacity_dir().unwrap_or_else(std::env::temp_dir),
            now: microcoder::now,
            turns: AtomicUsize::new(0),
            opened: atif::now_ms(),
            artifacts: None,
        }
    }

    /// The same door, reading and writing the capacity book in `dir`.
    #[must_use]
    pub fn reading_capacity_in(mut self, dir: PathBuf) -> Self {
        self.book = dir;
        self
    }

    /// The same door, starting the issue flow for a turn that asks to work
    /// an issue only when `on`.
    #[must_use]
    pub fn issues(mut self, on: bool) -> Self {
        self.issues = on;
        self
    }

    /// The same door, with Microcoder's clock at `now`. For tests.
    #[must_use]
    pub fn clocked(mut self, now: fn() -> u64) -> Self {
        self.now = now;
        self
    }

    /// The same door, writing each turn's briefing and stream under `dir`.
    #[must_use]
    pub fn writing_under(mut self, dir: PathBuf) -> Self {
        self.artifacts = Some(dir);
        self
    }

    /// The same door, with each Microcoder provider answering from its
    /// scripted replies in place of a model call. For tests.
    #[must_use]
    pub fn scripting(mut self, replies: Vec<(Provider, Vec<microcoder::Scripted>)>) -> Self {
        self.script = Some(
            replies
                .into_iter()
                .map(|(provider, replies)| {
                    (
                        provider,
                        std::sync::Arc::new(std::sync::Mutex::new(replies.into())),
                    )
                })
                .collect(),
        );
        self
    }

    /// The same door, with Microcoder's cloud lane answering through
    /// `door` in place of the OpenAgents relay. For tests.
    #[must_use]
    pub fn cloud_through(mut self, door: Door) -> Self {
        self.cloud = Some(std::sync::Arc::new(door));
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
        let cli = match self.target.agent {
            Agent::Microcoder => {
                return self.microcoder(request, earlier, read_only, on).await;
            }
            Agent::Cli(cli) => cli,
        };
        // A CLI resumes its session.
        let resume = self.session();
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
            agent: cli,
            model: self.named_model.clone(),
            binary: self.target.binary.clone(),
            credential: self.target.credential,
            jev: self.jev.clone(),
            artifacts: self.artifacts(turn),
            // The CLI turn never starts the issue flow; Microcoder's turn
            // does ([`microcoder`]).
            issues: false,
            issue: false,
            review: false,
            extra: (),
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
                "could not start the delegation thread: {error}"
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
                        "the delegation thread could not run: {why}"
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
            "the delegation thread ended without an answer".to_string(),
        ))
    }
}

impl DelegateDoor {
    /// One turn through the Microcoder loop, on a thread of its own: the
    /// loop's generators and failover are not `Send`, so it runs on a
    /// current-thread runtime and its updates come back over a channel.
    async fn microcoder(
        &self,
        request: &str,
        earlier: &str,
        read_only: bool,
        on: &mut (dyn FnMut(Update) + Send),
    ) -> Result<Delegated, GenerateError> {
        // The book is read again at each turn, so a refusal an earlier
        // turn or another process recorded is honored.
        let lineup: Vec<(Provider, String)> = self
            .target
            .providers
            .iter()
            .map(|state| (state.provider, state.model.clone()))
            .collect();
        let providers = microcoder::providers(
            &lineup,
            &self.book,
            &|provider| {
                self.target
                    .providers
                    .iter()
                    .find(|state| state.provider == provider)
                    .map_or_else(
                        || Connection::Missing("not probed".to_string()),
                        |state| state.connection.clone(),
                    )
            },
            (self.now)(),
        );
        let turn = microcoder::Turn {
            request: request.to_string(),
            earlier: earlier.to_string(),
            read_only,
            workdir: self.workdir.clone(),
            jev: self.jev.clone(),
            jev_missing: format!("no Jev: {}", self.jev_source),
            book: self.book.clone(),
            providers,
            script: self.script.clone(),
            cloud: self.cloud.clone(),
            max_usd: microcoder::MAX_USD,
            ask: true,
            // The issue flow works in a checkout of its own and opens a
            // draft pull request, so the operator's permit governs it, not
            // this turn's route.
            issues: self.issues,
            now: self.now,
        };
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<FromLoop>();
        let spawned = std::thread::Builder::new()
            .name("coder-microcoder".to_string())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = tx.send(FromLoop::Failed(error.to_string()));
                        return;
                    }
                };
                let progress = tx.clone();
                let done = runtime.block_on(microcoder::respond(
                    turn,
                    Rc::new(move |update| {
                        let _ = progress.send(FromLoop::Update(update));
                    }),
                ));
                let _ = tx.send(FromLoop::Done(Box::new(done)));
            });
        if let Err(error) = spawned {
            return Err(GenerateError::Stream(format!(
                "could not start the Microcoder thread: {error}"
            )));
        }
        while let Some(message) = rx.recv().await {
            match message {
                FromLoop::Update(update) => on(update),
                FromLoop::Failed(why) => {
                    return Err(GenerateError::Stream(format!(
                        "the Microcoder thread could not run: {why}"
                    )));
                }
                FromLoop::Done(done) => {
                    if done.failure.is_none() && !done.text.is_empty() {
                        on(Update::Text(done.text.clone()));
                    }
                    return Ok(*done);
                }
            }
        }
        Err(GenerateError::Stream(
            "the Microcoder thread ended without an answer".to_string(),
        ))
    }
}

/// What the Microcoder thread sends back.
enum FromLoop {
    Update(Update),
    Failed(String),
    Done(Box<Delegated>),
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
            reason: format!("{agent} did not finish before its time limit"),
        }),
        DelegateStatus::Failed(code) => Some(GenerateError::Stream(format!(
            "{agent} exited with code {code}: {}",
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
            } => Some(Update::Line(format!(
                "session ▸ the agent started session {id}"
            ))),
            Kind::SessionStarted { session_id: None } => None,
            Kind::AssistantClaim { text } if !text.trim().is_empty() => {
                Some(Update::Text(text.clone()))
            }
            Kind::AssistantClaim { .. } => None,
            Kind::CommandStarted { command } => {
                let shown = coder_delegate::stream::unwrap_shell(command);
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
                let shown = coder_delegate::stream::unwrap_shell(command);
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
    Duration::from_secs(terminal::policy().executor.deadline_sec)
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
    use microcoder_loop::models::{Ask, NextAction};

    fn target(agent: Cli, installed: bool, credential: Credential) -> Target {
        Target {
            agent: Agent::Cli(agent),
            binary: installed.then(|| PathBuf::from(format!("/bin/{}", agent.program()))),
            credential,
            problem: None,
            refusal: None,
            providers: Vec::new(),
        }
    }

    /// A Codex usage-limit refusal observed at 1000 that resets at 5000.
    fn codex_refusal() -> Refusal {
        Refusal::new(
            Provider::Codex,
            capacity::Kind::UsageLimit,
            1_000,
            Some(5_000),
        )
    }

    /// Microcoder's providers: Codex, `refused` when its book entry holds,
    /// and Claude, connected or not.
    fn providers(codex: bool, refused: bool, claude: bool) -> Vec<ProviderState> {
        let connection = |on: bool| {
            if on {
                Connection::Connected
            } else {
                Connection::Missing("no login".to_string())
            }
        };
        vec![
            ProviderState {
                provider: Provider::Codex,
                model: microcoder::CODEX_MODEL.to_string(),
                connection: connection(codex),
                refusal: refused.then(codex_refusal),
            },
            ProviderState {
                provider: Provider::Claude,
                model: microcoder::CLAUDE_MODEL.to_string(),
                connection: connection(claude),
                refusal: None,
            },
        ]
    }

    /// Microcoder over `providers`, ahead of both CLIs as [`targets`]
    /// orders them.
    fn all(providers: Vec<ProviderState>, claude: bool, codex: bool) -> Vec<Target> {
        let mut targets = vec![Target::microcoder(providers)];
        targets.extend(both(claude, codex));
        targets
    }

    #[test]
    fn auto_prefers_microcoder_on_a_provider_with_capacity() {
        let chosen = |targets: &[Target]| choose(Mode::Auto, None, None, targets).unwrap();
        let usable = all(providers(true, false, true), true, true);
        let choice = chosen(&usable);
        assert_eq!(choice.chosen, Chosen::Delegate(usable[0].clone()));
        assert_eq!(
            choice.reason,
            "microcoder runs in this process on codex (gpt-6.1-sol)"
        );
        // Codex out of its limit: Microcoder still answers, on Claude, and
        // says why it skipped Codex.
        let refused = all(providers(true, true, true), true, true);
        let choice = chosen(&refused);
        assert_eq!(choice.chosen, Chosen::Delegate(refused[0].clone()));
        assert!(
            choice.reason.contains("on claude (opus); it skips codex because the Codex login is out of its usage limit until 1970-01-01 01:23 UTC"),
            "{}",
            choice.reason
        );
        // No provider it can use falls through to Claude Code, then Codex,
        // then the Open Responses door.
        let none = all(providers(false, false, false), true, true);
        assert_eq!(chosen(&none).chosen, Chosen::Delegate(none[1].clone()));
        let fallback = chosen(&all(providers(true, true, false), false, false));
        assert_eq!(fallback.chosen, Chosen::Fallback);
        assert!(
            fallback.reason.contains(
                "microcoder has no provider to use: the Codex login is out of its usage limit"
            ),
            "{}",
            fallback.reason
        );
        // The operator's named agent still outranks the default.
        let named = choose(Mode::Auto, Some(Agent::Cli(Cli::ClaudeCode)), None, &usable).unwrap();
        assert_eq!(named.chosen, Chosen::Delegate(usable[1].clone()));
    }

    #[test]
    fn the_capacity_book_skips_a_cli_whose_login_is_out_of_its_limit() {
        let dir = tempfile::tempdir().unwrap();
        capacity::record_with(dir.path(), codex_refusal(), |_| None).unwrap();
        let book = capacity::Book::load_with(dir.path(), |_| None);
        let env = |_: &str| None;
        let codex = Target::find(Cli::Codex, env, &book, 2_000);
        assert!(
            codex
                .problem
                .as_deref()
                .is_some_and(|why| why.contains("usage limit"))
        );
        assert!(!codex.available());
        let claude = Target::find(Cli::ClaudeCode, env, &book, 2_000);
        assert_eq!(claude.refusal, None);
        // After the reset, the book no longer holds it.
        assert_eq!(Target::find(Cli::Codex, env, &book, 5_000).refusal, None);
        // Microcoder reads the same book for its providers.
        let connected = |_: Provider| Connection::Connected;
        let found = targets(env, None, dir.path(), &connected, 2_000);
        assert_eq!(found[0].agent, Agent::Microcoder);
        assert_eq!(found[0].providers[0].refusal, Some(codex_refusal()));
        assert_eq!(
            found[0].provider().map(|state| state.provider),
            Some(Provider::Claude)
        );
    }

    #[test]
    fn with_nothing_left_the_sentence_names_each_target_and_its_reset() {
        let mut claude = target(Cli::ClaudeCode, false, Credential::Missing);
        claude.problem = None;
        let mut codex = target(Cli::Codex, true, Credential::CodexAuthFile);
        codex.refusal = Some(codex_refusal());
        codex.problem = Some(format!(
            "its login {}",
            microcoder::blocked(&codex_refusal())
        ));
        let targets = vec![
            Target::microcoder(providers(true, true, false)),
            claude,
            codex,
        ];
        assert!(targets[0].out_of_capacity());
        assert!(targets[2].out_of_capacity());
        let sentence = no_capacity(&targets);
        assert!(
            sentence.starts_with("No model can answer now: "),
            "{sentence}"
        );
        assert!(
            sentence
                .contains("the Codex login is out of its usage limit until 1970-01-01 01:23 UTC"),
            "{sentence}"
        );
        assert!(
            sentence.contains("claude-code is not installed"),
            "{sentence}"
        );
        assert!(
            sentence
                .contains("codex: its login is out of its usage limit until 1970-01-01 01:23 UTC"),
            "{sentence}"
        );
    }

    #[test]
    fn microcoder_and_microluna_both_name_microcoder() {
        assert_eq!(Agent::parse("microcoder"), Ok(Agent::Microcoder));
        assert_eq!(Agent::parse("microluna"), Ok(Agent::Microcoder));
        assert_eq!(Agent::parse("claude-code"), Ok(Agent::Cli(Cli::ClaudeCode)));
        assert!(Agent::parse("devin").is_err());
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
            fallback
                .reason
                .starts_with("no delegation target is available"),
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
        let choice = choose(
            Mode::Auto,
            Some(Agent::Cli(Cli::Codex)),
            None,
            &both(true, true),
        )
        .unwrap();
        assert_eq!(choice.chosen, Chosen::Delegate(both(true, true)[1].clone()));
        assert!(choice.reason.contains(AGENT_VAR));
        let missing = choose(
            Mode::Auto,
            Some(Agent::Cli(Cli::Codex)),
            None,
            &both(true, false),
        )
        .unwrap();
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
        assert!(auto.reason.contains("takes precedence over delegation"));
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
        let binary = coder_delegate::adapter::standin::install(&dir.join("bin"), "claude", WRITER);
        let target = Target {
            binary: Some(binary),
            ..target(Cli::ClaudeCode, true, Credential::CliLogin)
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

    fn finish(reply: &str) -> NextAction {
        NextAction {
            rationale: "answer".to_string(),
            commands: Vec::new(),
            view: Vec::new(),
            freeze_tests: false,
            expand: Vec::new(),
            finished: true,
            reply: reply.to_string(),
            ask: Ask::None,
        }
    }

    fn running(command: &str) -> NextAction {
        NextAction {
            finished: false,
            commands: vec![command.to_string()],
            ..finish("")
        }
    }

    /// The fixed clock the Microcoder tests run at.
    fn at_two_thousand() -> u64 {
        2_000
    }

    /// A Microcoder door over `providers` in a fresh workspace, with the
    /// capacity book in `dir`, or `None` on a host that cannot enforce a
    /// boundary.
    fn microcoder_door(
        dir: &Path,
        providers: Vec<ProviderState>,
        script: Vec<(Provider, Vec<microcoder::Scripted>)>,
    ) -> Option<(std::sync::Arc<DelegateDoor>, PathBuf)> {
        let workdir = dir.join("work");
        std::fs::create_dir_all(&workdir).unwrap();
        if let Err(why) = boundary_available(&workdir) {
            eprintln!("skipped: {why}");
            return None;
        }
        let door = DelegateDoor::new(
            Target::microcoder(providers),
            None,
            workdir.clone(),
            None,
            String::new(),
        )
        .writing_under(dir.join("artifacts"))
        .reading_capacity_in(dir.join("tasks"))
        .clocked(at_two_thousand)
        .scripting(script);
        Some((std::sync::Arc::new(door), workdir))
    }

    #[tokio::test]
    async fn a_microcoder_turn_streams_as_shell_events_and_the_permit_bounds_its_writes() {
        let dir = tempfile::tempdir().unwrap();
        let script = vec![(
            Provider::Codex,
            vec![
                Ok(running("printf x > marker")),
                Ok(finish("Here is the answer.")),
                Ok(running("printf x > marker")),
                Ok(finish("Wrote it.")),
            ],
        )];
        let Some((door, workdir)) =
            microcoder_door(dir.path(), providers(true, false, false), script)
        else {
            return;
        };
        assert_eq!(door.label(), "microcoder/gpt-6.1-sol");
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
        assert!(
            matches!(
                &shell[..],
                [crate::shell::ShellEvent::Proposed(proposal), crate::shell::ShellEvent::Ran(outcome)]
                    if proposal.command == "printf x > marker" && !matches!(outcome.status, Status::Exit(0))
            ),
            "{shell:?}"
        );
        assert_eq!(
            door.session(),
            None,
            "Microcoder keeps no session to resume"
        );

        let (wrote, _, _) = turn(
            &mut agent,
            crate::permit::Permit::executing(),
            "now write the marker",
        )
        .await;
        assert_eq!(wrote.text, "Wrote it.");
        assert!(
            workdir.join("marker").exists(),
            "a permitted turn could not write"
        );
    }

    #[tokio::test]
    async fn a_usage_limit_mid_turn_is_recorded_and_the_turn_finishes_on_the_next_provider() {
        let dir = tempfile::tempdir().unwrap();
        let script = vec![
            (Provider::Codex, vec![Err(codex_refusal())]),
            (Provider::Claude, vec![Ok(finish("Hello from Claude."))]),
        ];
        let Some((door, _)) = microcoder_door(dir.path(), providers(true, false, true), script)
        else {
            return;
        };
        let done = door
            .answer("hello", "", true, false, &mut |_| {})
            .await
            .unwrap();
        assert!(done.failure.is_none(), "{:?}", done.failure);
        assert_eq!(done.text, "Hello from Claude.");
        assert_eq!(done.model, microcoder::CLAUDE_MODEL);
        // The refusal is in the book with its reset.
        let book = capacity::Book::load_with(&dir.path().join("tasks"), |_| None);
        // The refusal keeps the login's fingerprint, when it has one (#10105).
        let held = book
            .blocking(Provider::Codex, 2_000)
            .cloned()
            .map(|mut held| {
                held.account = None;
                held
            });
        assert_eq!(held, Some(codex_refusal()));
        // The trace holds the switch.
        let switched = done.steps.iter().any(|step| {
            serde_json::to_value(step)
                .unwrap()
                .to_string()
                .contains("route_switch")
        });
        assert!(switched, "no route_switch step");
        // The next turn starts on Claude without asking Codex again.
        let script = vec![(Provider::Claude, vec![Ok(finish("Again."))])];
        let door = DelegateDoor::new(
            Target::microcoder(providers(true, false, true)),
            None,
            dir.path().join("work"),
            None,
            String::new(),
        )
        .reading_capacity_in(dir.path().join("tasks"))
        .clocked(at_two_thousand)
        .scripting(script);
        let done = door
            .answer("hello", "", true, false, &mut |_| {})
            .await
            .unwrap();
        assert_eq!(done.text, "Again.");
    }

    #[tokio::test]
    async fn with_every_provider_out_the_turn_ends_with_a_plain_sentence() {
        let dir = tempfile::tempdir().unwrap();
        let script = vec![(Provider::Codex, vec![Err(codex_refusal())])];
        let Some((door, _)) = microcoder_door(dir.path(), providers(true, false, false), script)
        else {
            return;
        };
        let done = door
            .answer("hello", "", true, false, &mut |_| {})
            .await
            .unwrap();
        let Some(GenerateError::NoCapacity(sentence)) = &done.failure else {
            panic!("{:?}", done.failure);
        };
        assert!(
            sentence
                .contains("the Codex login is out of its usage limit until 1970-01-01 01:23 UTC"),
            "{sentence}"
        );
        assert!(
            sentence.contains("the Claude Code login can't be used (no login)"),
            "{sentence}"
        );
        assert_eq!(done.failure.as_ref().unwrap().to_string(), *sentence);
        // A later turn refuses before it asks Jev or a model anything.
        let done = door
            .answer("hello", "", true, false, &mut |_| {})
            .await
            .unwrap();
        assert!(matches!(done.failure, Some(GenerateError::NoCapacity(_))));
        assert_eq!(done.cost_usd, Some(0.0));
    }

    /// The OpenAgents cloud's quota refusal, observed at 2000, with the
    /// worker's wait of 41 seconds.
    fn cloud_refusal() -> Refusal {
        Refusal::cloud("quota_exhausted", Some(41_000), 2_000).unwrap()
    }

    /// The cloud first, then Claude, both connected.
    fn cloud_then_claude() -> Vec<ProviderState> {
        vec![
            ProviderState {
                provider: Provider::Vertex,
                model: microcoder::CLOUD_MODEL.to_string(),
                connection: Connection::Connected,
                refusal: None,
            },
            ProviderState {
                provider: Provider::Claude,
                model: microcoder::CLAUDE_MODEL.to_string(),
                connection: Connection::Connected,
                refusal: None,
            },
        ]
    }

    fn after_the_cloud_reset() -> u64 {
        2_100
    }

    #[tokio::test]
    async fn a_cloud_refusal_mid_turn_is_recorded_and_the_next_provider_finishes() {
        let dir = tempfile::tempdir().unwrap();
        let script = vec![
            (Provider::Vertex, vec![Err(cloud_refusal())]),
            (Provider::Claude, vec![Ok(finish("Hello from Claude."))]),
        ];
        let Some((door, _)) = microcoder_door(dir.path(), cloud_then_claude(), script) else {
            return;
        };
        let done = door
            .answer("hello", "", true, false, &mut |_| {})
            .await
            .unwrap();
        assert!(done.failure.is_none(), "{:?}", done.failure);
        assert_eq!(done.text, "Hello from Claude.");
        // The book holds the cloud's quota refusal until its wait ends.
        let book = capacity::Book::load_with(&dir.path().join("tasks"), |_| None);
        let held = book.blocking(Provider::Vertex, 2_000).unwrap();
        assert_eq!(held.kind, capacity::Kind::UsageLimit);
        assert_eq!(held.until, 2_041);
        // The next turn skips the cloud without asking it, and says why.
        let providers = microcoder::providers(
            &microcoder::lineup(None),
            &dir.path().join("tasks"),
            &|_| Connection::Connected,
            2_000,
        );
        let cloud = providers
            .iter()
            .find(|state| state.provider == Provider::Vertex)
            .unwrap();
        assert!(!cloud.usable());
        assert!(
            cloud.describe().starts_with(
                "the OpenAgents cloud is out of its usage limit until 1970-01-01 00:34 UTC"
            ),
            "{}",
            cloud.describe()
        );
        let script = vec![(Provider::Claude, vec![Ok(finish("Again."))])];
        let door = DelegateDoor::new(
            Target::microcoder(cloud_then_claude()),
            None,
            dir.path().join("work"),
            None,
            String::new(),
        )
        .reading_capacity_in(dir.path().join("tasks"))
        .clocked(at_two_thousand)
        .scripting(script);
        let done = door
            .answer("hello", "", true, false, &mut |_| {})
            .await
            .unwrap();
        assert_eq!(done.text, "Again.");
        // After the reset, the cloud answers first again.
        let script = vec![(Provider::Vertex, vec![Ok(finish("From the cloud."))])];
        let door = DelegateDoor::new(
            Target::microcoder(cloud_then_claude()),
            None,
            dir.path().join("work"),
            None,
            String::new(),
        )
        .reading_capacity_in(dir.path().join("tasks"))
        .clocked(after_the_cloud_reset)
        .scripting(script);
        let done = door
            .answer("hello", "", true, false, &mut |_| {})
            .await
            .unwrap();
        assert_eq!(done.text, "From the cloud.");
    }

    #[test]
    fn the_cloud_is_always_the_last_provider_and_needs_no_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let lineup = microcoder::lineup(None);
        assert_eq!(
            lineup.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
            [Provider::Codex, Provider::Claude, Provider::Vertex]
        );
        assert_eq!(
            lineup.last().unwrap(),
            &(Provider::Vertex, microcoder::CLOUD_MODEL.to_string())
        );
        // On a fresh host, with no login and nothing configured, the
        // cloud is the provider a turn uses, so Microcoder is available.
        let targets = targets(|_| None, None, dir.path(), &fresh_host, 2_000);
        let microcoder = &targets[0];
        assert!(microcoder.available());
        assert_eq!(microcoder.provider().unwrap().provider, Provider::Vertex);
        // Turned off, it says why it can't be used.
        let state = ProviderState {
            provider: Provider::Vertex,
            model: microcoder::CLOUD_MODEL.to_string(),
            connection: capacity::cloud_connection(Some("off")),
            refusal: None,
        };
        assert_eq!(
            state.describe(),
            "the OpenAgents cloud can't be used (CODER_CLOUD=off turns the cloud fallback off)"
        );
    }

    #[test]
    fn the_cloud_answers_only_when_nothing_on_the_host_can() {
        let dir = tempfile::tempdir().unwrap();
        let chosen = |targets: &[Target]| choose(Mode::Auto, None, None, targets).unwrap();
        // Microcoder has only the cloud: an authenticated Claude Code CLI
        // answers instead.
        let mut on_cloud = providers(false, false, false);
        on_cloud.push(ProviderState {
            provider: Provider::Vertex,
            model: microcoder::CLOUD_MODEL.to_string(),
            connection: Connection::Connected,
            refusal: None,
        });
        let cli = all(on_cloud.clone(), true, false);
        assert_eq!(chosen(&cli).chosen, Chosen::Delegate(cli[1].clone()));
        // With no CLI either, the cloud answers.
        let bare = all(on_cloud, false, false);
        assert_eq!(chosen(&bare).chosen, Chosen::Delegate(bare[0].clone()));
        // An own door key takes the cloud off Microcoder's providers, and
        // the operator's door answers.
        let keyed = targets(
            |name| (name == "CODER_DOOR_KEY").then(|| "k".to_string()),
            None,
            dir.path(),
            &fresh_host,
            2_000,
        );
        assert!(!keyed[0].available());
        assert!(
            keyed[0]
                .describe()
                .contains("CODER_DOOR_KEY names this host's own door"),
            "{}",
            keyed[0].describe()
        );
        assert_eq!(chosen(&keyed).chosen, Chosen::Fallback);
    }

    /// A host with nothing configured: no Codex login, no Claude Code, and
    /// the cloud needing nothing.
    fn fresh_host(provider: Provider) -> Connection {
        match provider {
            Provider::Vertex => capacity::cloud_connection(None),
            _ => Connection::Missing("not signed in".into()),
        }
    }

    #[tokio::test]
    async fn a_fresh_host_completes_a_turn_through_the_cloud_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let workdir = dir.path().join("work");
        std::fs::create_dir_all(&workdir).unwrap();
        if let Err(why) = boundary_available(&workdir) {
            eprintln!("skipped: {why}");
            return;
        }
        let providers = microcoder::providers(
            &microcoder::lineup(None),
            &dir.path().join("tasks"),
            &fresh_host,
            2_000,
        );
        let reply = |action: &NextAction| serde_json::to_string(action).unwrap();
        // The cloud's worker answers each step's job with the action as
        // text: one command, then the answer.
        let cloud = Door::Stub(StubGenerate::scripted(
            vec![
                reply(&running("printf hi")),
                format!(
                    "```json\n{}\n```",
                    reply(&finish("Done through the cloud."))
                ),
            ],
            "",
        ));
        let door = DelegateDoor::new(
            Target::microcoder(providers),
            None,
            workdir,
            None,
            String::new(),
        )
        .reading_capacity_in(dir.path().join("tasks"))
        .clocked(at_two_thousand)
        .cloud_through(cloud);
        let done = door
            .answer("say hi", "", false, false, &mut |_| {})
            .await
            .unwrap();
        assert!(done.failure.is_none(), "{:?}", done.failure);
        assert_eq!(done.text, "Done through the cloud.");
        assert_eq!(done.commands, 1);
        // The host paid nothing for it.
        assert_eq!(done.cost_usd, Some(0.0));
    }

    /// A Microcoder turn for the issue flow's tests, over `providers`, with
    /// the capacity book in `dir`.
    fn issue_turn(
        dir: &Path,
        providers: Vec<ProviderState>,
        script: Vec<(Provider, Vec<microcoder::Scripted>)>,
    ) -> microcoder::Turn {
        microcoder::Turn {
            request: "work on #1".to_string(),
            earlier: String::new(),
            read_only: false,
            workdir: dir.join("work"),
            jev: None,
            jev_missing: "no Jev key in a test".to_string(),
            book: dir.join("tasks"),
            providers,
            script: Some(
                script
                    .into_iter()
                    .map(|(provider, replies)| {
                        (
                            provider,
                            std::sync::Arc::new(std::sync::Mutex::new(replies.into())),
                        )
                    })
                    .collect(),
            ),
            max_usd: microcoder::ISSUE_MAX_USD,
            ask: false,
            issues: true,
            cloud: None,
            now: at_two_thousand,
        }
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn the_issue_flow_works_on_microcoder_and_fails_over_mid_session() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        if let Err(why) = boundary_available(&repo) {
            eprintln!("skipped: {why}");
            return;
        }
        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.email", "test@example.com"]);
        git(&repo, &["config", "user.name", "test"]);
        git(&repo, &["commit", "--allow-empty", "-qm", "base"]);
        let script = vec![
            (Provider::Codex, vec![Err(codex_refusal())]),
            (
                Provider::Claude,
                vec![
                    Ok(running("printf done > note.txt")),
                    Ok(finish("Wrote note.txt.")),
                ],
            ),
        ];
        let turn = issue_turn(dir.path(), providers(true, false, true), script);
        let lines = Rc::new(std::cell::RefCell::new(Vec::new()));
        let heard = lines.clone();
        let worker = microcoder::IssueWorker::new(
            turn,
            Rc::new(move |update| {
                if let Update::Line(line) = update {
                    heard.borrow_mut().push(line);
                }
            }),
        );
        let reference = coder_delegate::issue::Reference {
            repository: Some("example/example".to_string()),
            number: 1,
        };
        let prepared = coder_delegate::issue::Prepared {
            inner: terminal::Request {
                workdir: repo.clone(),
                request: "# Issue #1: Add a note\n\nWrite note.txt saying done.".to_string(),
                earlier: String::new(),
                resume: None,
                read_only: false,
                clarify: false,
                agent: Cli::Codex,
                model: None,
                binary: None,
                credential: Credential::Missing,
                jev: None,
                artifacts: dir.path().join("artifacts"),
                issues: false,
                issue: true,
                review: false,
                extra: (),
            },
            workdir: repo.clone(),
            source: None,
            branch: "coder/issue-1-test".to_string(),
            issue: coder_delegate::issue::Fetched {
                url: "https://github.com/example/example/issues/1".to_string(),
                title: "Add a note".to_string(),
                body: "Write note.txt saying done.".to_string(),
            },
        };
        let (answer, _) = coder_delegate::issue::work(
            &worker,
            prepared,
            reference,
            Rc::new(|_| {}),
            &coder_delegate::record::Recorder::default(),
            false,
        )
        .await;
        // The session finished on Claude after Codex refused mid-session.
        assert_eq!(
            std::fs::read_to_string(repo.join("note.txt")).unwrap(),
            "done"
        );
        let result = answer.report.summary.result.clone().unwrap_or_default();
        assert!(result.contains("Wrote note.txt."), "{result}");
        assert!(result.contains("Published nothing"), "{result}");
        let book = capacity::Book::load_with(&dir.path().join("tasks"), |_| None);
        // The refusal keeps the login's fingerprint, when it has one (#10105).
        let held = book
            .blocking(Provider::Codex, 2_000)
            .cloned()
            .map(|mut held| {
                held.account = None;
                held
            });
        assert_eq!(held, Some(codex_refusal()));
        let switched = answer.steps.iter().any(|step| {
            serde_json::to_value(step)
                .unwrap()
                .to_string()
                .contains("route_switch")
        });
        assert!(switched, "no route_switch step");
        let done = worker.delegated(&answer, 1);
        assert!(done.failure.is_none(), "{:?}", done.failure);
        assert_eq!(done.commands, 1);
        assert_eq!(done.model, microcoder::CLAUDE_MODEL);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn with_no_provider_the_issue_flow_ends_before_it_checks_anything_out() {
        let dir = tempfile::tempdir().unwrap();
        let turn = issue_turn(dir.path(), providers(true, true, false), Vec::new());
        let request = terminal::Request {
            workdir: dir.path().to_path_buf(),
            request: "work on #1".to_string(),
            earlier: String::new(),
            resume: None,
            read_only: false,
            clarify: false,
            agent: Cli::Codex,
            model: None,
            binary: None,
            credential: Credential::Missing,
            jev: None,
            artifacts: dir.path().join("artifacts"),
            issues: true,
            issue: false,
            review: false,
            extra: (),
        };
        let reference = coder_delegate::issue::Reference {
            repository: Some("example/example".to_string()),
            number: 1,
        };
        let done = microcoder::issue(
            turn,
            &request,
            reference,
            Rc::new(|_| {}),
            coder_delegate::record::Recorder::default(),
        )
        .await;
        let Some(GenerateError::NoCapacity(sentence)) = &done.failure else {
            panic!("{:?}", done.failure);
        };
        assert!(
            sentence.contains("the Codex login is out of its usage limit until"),
            "{sentence}"
        );
        assert!(!dir.path().join("coder").exists());
    }

    #[tokio::test]
    async fn a_session_with_no_model_left_ends_every_turn_with_the_sentence() {
        let stub = Door::Stub(StubGenerate::refusing("No model can answer now: x."));
        let mut agent = crate::agent::Agent::new(None, stub);
        agent.push_user("hello");
        let error = agent
            .turn(
                false,
                crate::permit::Permit::answering(),
                &mut |_| {},
                &mut |_| {},
                &mut |_| {},
            )
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "No model can answer now: x.");
        assert_eq!(error.cause(), "no_capacity");
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
