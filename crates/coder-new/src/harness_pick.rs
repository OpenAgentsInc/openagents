//! Shift+Tab's harness pick (#11524): which harness answers this chat.
//!
//! The input's top-right rail starts on `Auto`, where the router (or the
//! fixed order, Claude Code, then OpenCode, then the Coder loop) chooses
//! each turn's harness. Shift+Tab steps through the harnesses that can run
//! here, in a fixed order: Auto, Claude Code, Codex, OpenCode, then the
//! ACP agents found on this computer (Devin, Cursor, Grok Build, Amp...).
//! One that can't run here is never offered. A manual pick pins the chat
//! to it until Shift+Tab moves on or back to Auto. It is a pick within the
//! one conversation, not a mode (owner, 2026-10-13).
//!
//! Each pick's facts (its sign-in, its model, its plan windows) come from
//! a small cache on disk with the time each was learned, so the rail shows
//! them at once. A pick highlighted for [`SETTLE`] is refreshed in the
//! background the first time this run, and again whenever a fact is stale
//! ([`USAGE_FRESH`], [`MODEL_FRESH`], [`SIGN_IN_FRESH`]). Nothing here
//! blocks typing, and a fact that isn't known is left out.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::bundled_runtime::{AcpAgent, AgentTransport};
use crate::harness_plugin::Harness;

/// What Shift+Tab says once the chat has a turn: only a new chat switches
/// agents (#11556).
pub const LOCKED_HINT: &str = "Start a new chat (Ctrl+N) to switch agents";

/// The harnesses Shift+Tab offers after Auto, in order. The Coder loop is
/// what Auto falls back to, so it has no stop of its own.
pub const ORDER: [Harness; 3] = [Harness::ClaudeCode, Harness::Codex, Harness::OpenCode];

/// How long a pick stays highlighted before its facts are refreshed, so
/// cycling quickly past a harness starts no work for it.
pub const SETTLE: Duration = Duration::from_millis(300);
/// Plan windows older than this are read again, in seconds.
pub const USAGE_FRESH: u64 = 5 * 60;
/// A model older than this is read again, in seconds.
pub const MODEL_FRESH: u64 = 24 * 60 * 60;
/// A sign-in answer older than this is checked again, in seconds.
pub const SIGN_IN_FRESH: u64 = 5 * 60;

/// What answers the chat's next turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    /// The router, or the fixed order.
    Auto,
    /// One harness, pinned.
    Harness(Harness),
    /// One ACP agent on this computer, pinned.
    Agent { id: String, name: String },
}

impl Pick {
    /// The cache's key for the pick.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Pick::Auto => "auto".into(),
            Pick::Harness(harness) => serde_json::to_value(harness)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default(),
            Pick::Agent { id, .. } => format!("agent:{id}"),
        }
    }

    /// The pick a chat holds: its pinned agent or harness, else Auto.
    #[must_use]
    pub fn of_chat(chat: &crate::live::Chat, agents: &[(String, String)]) -> Pick {
        if let Some(id) = &chat.agent {
            let name = agents
                .iter()
                .find(|(known, _)| known == id)
                .map_or_else(|| id.clone(), |(_, name)| name.clone());
            return Pick::Agent {
                id: id.clone(),
                name,
            };
        }
        match chat.runs_on {
            Some(harness) if chat.pinned => Pick::Harness(harness),
            _ => Pick::Auto,
        }
    }
}

/// What this computer offers, from cheap local checks only: programs on
/// disk, plugin switches, sign-ins already known. Nothing is started.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Local {
    pub claude: bool,
    pub codex: bool,
    pub opencode: bool,
    /// The ACP agents that can take a turn, `(id, name)` in catalog order.
    pub agents: Vec<(String, String)>,
}

/// The ACP agents that are harnesses of their own, so Shift+Tab never
/// offers them twice.
fn is_harness(id: &str) -> bool {
    crate::fleet::runs_claude(id) || matches!(id, "codex" | "opencode")
}

/// The ACP agents a chat turn can run on: installed, turned on in
/// /plugins, and speaking ACP.
#[must_use]
pub fn turn_agents(agents: &[AcpAgent]) -> Vec<(String, String)> {
    agents
        .iter()
        .filter(|agent| {
            agent.enabled && agent.transport == AgentTransport::Acp && !is_harness(&agent.id)
        })
        .map(|agent| (agent.id.clone(), agent.name.clone()))
        .collect()
}

/// The picks Shift+Tab steps through: Auto, then each available harness
/// in [`ORDER`], then each agent.
#[must_use]
pub fn options(local: &Local) -> Vec<Pick> {
    let mut picks = vec![Pick::Auto];
    for harness in ORDER {
        let available = match harness {
            Harness::ClaudeCode => local.claude,
            Harness::Codex => local.codex,
            Harness::OpenCode => local.opencode,
            Harness::CoderLoop => false,
        };
        if available {
            picks.push(Pick::Harness(harness));
        }
    }
    picks.extend(local.agents.iter().map(|(id, name)| Pick::Agent {
        id: id.clone(),
        name: name.clone(),
    }));
    picks
}

/// The pick after `current`: the next one offered, wrapping to Auto. A
/// pick no longer offered moves to Auto.
#[must_use]
pub fn next(options: &[Pick], current: &Pick) -> Pick {
    options
        .iter()
        .position(|pick| pick.key() == current.key())
        .and_then(|index| options.get((index + 1) % options.len()))
        .cloned()
        .unwrap_or(Pick::Auto)
}

/// What the cache knows about one pick, with when it learned each fact
/// (Unix seconds; 0 for never).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_in: Option<bool>,
    #[serde(default)]
    pub signed_in_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub model_at: u64,
    /// The plan windows as the rail shows them: "5h 12% · week 15%".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,
    #[serde(default)]
    pub usage_at: u64,
}

/// Which of an entry's facts are due to be read again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stale {
    pub sign_in: bool,
    pub model: bool,
    pub usage: bool,
}

impl Stale {
    #[must_use]
    pub fn any(self) -> bool {
        self.sign_in || self.model || self.usage
    }
}

impl Entry {
    /// The facts older than their freshness at `now`, or never learned.
    #[must_use]
    pub fn stale(&self, now: u64) -> Stale {
        let old = |at: u64, fresh: u64| at == 0 || now >= at.saturating_add(fresh);
        Stale {
            sign_in: old(self.signed_in_at, SIGN_IN_FRESH),
            model: old(self.model_at, MODEL_FRESH),
            usage: old(self.usage_at, USAGE_FRESH),
        }
    }

    /// The plan windows, while they are fresh. Windows older than
    /// [`USAGE_FRESH`] are kept for the next refresh but not shown as now.
    #[must_use]
    pub fn usage_now(&self, now: u64) -> Option<&str> {
        self.usage
            .as_deref()
            .filter(|_| now < self.usage_at.saturating_add(USAGE_SHOWN))
    }
}

/// How long a cached reading of the plan windows still shows, in seconds:
/// the windows move slowly, so a reading from the last hour is shown while
/// a fresher one loads.
pub const USAGE_SHOWN: u64 = 60 * 60;

/// The cache: one [`Entry`] per pick.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cache {
    #[serde(default)]
    pub picks: BTreeMap<String, Entry>,
}

impl Cache {
    /// Reads the cache at `path`; an absent or unreadable one is empty.
    #[must_use]
    pub fn load(path: &Path) -> Cache {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Writes the cache to `path`, through a file beside it.
    ///
    /// # Errors
    /// The file could not be written.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(partial, path)
    }

    #[must_use]
    pub fn entry(&self, key: &str) -> Option<&Entry> {
        self.picks.get(key)
    }
}

/// When to refresh a highlighted pick: once it has stayed highlighted for
/// [`SETTLE`], the first time this run and then only when a fact is stale.
#[derive(Debug, Default)]
pub struct Refresher {
    highlighted: Option<(String, Instant)>,
    refreshed: BTreeSet<String>,
}

impl Refresher {
    /// `key` is highlighted from `at`; a different pick before it settled
    /// replaces it, so it is never refreshed.
    pub fn highlight(&mut self, key: String, at: Instant) {
        if self
            .highlighted
            .as_ref()
            .is_none_or(|(held, _)| *held != key)
        {
            self.highlighted = Some((key, at));
        }
    }

    /// The pick to refresh at `at`, once: it settled, and this run hasn't
    /// refreshed it yet or `stale` says a fact is due.
    pub fn due(&mut self, at: Instant, stale: impl Fn(&str) -> bool) -> Option<String> {
        let (key, since) = self.highlighted.as_ref()?;
        if at.duration_since(*since) < SETTLE {
            return None;
        }
        let key = key.clone();
        self.highlighted = None;
        if key == Pick::Auto.key() {
            return None;
        }
        let first = self.refreshed.insert(key.clone());
        (first || stale(&key)).then_some(key)
    }
}

/// The cache this run holds, and how many refreshes have finished.
static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
static REFRESHED: AtomicU64 = AtomicU64::new(0);

/// Where the cache lives: `harness-facts.json` in Coder's data folder
/// (`--state DIR`, else `~/.openagents/coder`), once [`start`] named it.
/// A unit test never touches it.
static PATH: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

fn path() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    PATH.get().cloned()
}

/// Loads the cache under `dir` on a thread of its own at startup, so the
/// first frame never waits on it.
pub fn start(dir: Option<PathBuf>) {
    if let Some(dir) = dir {
        let _ = PATH.set(dir.join("harness-facts.json"));
    }
    std::thread::spawn(|| {
        let cache = path().map(|path| Cache::load(&path)).unwrap_or_default();
        let mut held = CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if held.is_none() {
            *held = Some(cache);
        }
    });
}

/// What the cache knows about `key` now, without waiting: `None` while it
/// is loading or knows nothing.
#[must_use]
pub fn cached(key: &str) -> Option<Entry> {
    CACHE
        .try_lock()
        .ok()?
        .as_ref()
        .and_then(|cache| cache.entry(key).cloned())
}

/// How many refreshes have finished this run.
#[must_use]
pub fn refreshes() -> u64 {
    REFRESHED.load(Ordering::Relaxed)
}

/// Writes what was just learned about `key` into the cache and onto disk.
pub fn remember(key: &str, update: impl FnOnce(&mut Entry)) {
    let mut held = CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let cache =
        held.get_or_insert_with(|| path().map(|path| Cache::load(&path)).unwrap_or_default());
    let before = cache.picks.get(key).cloned();
    let entry = cache.picks.entry(key.to_owned()).or_default();
    update(entry);
    if before.as_ref() != Some(entry)
        && let Some(path) = path()
    {
        let _ = cache.save(&path);
    }
}

/// What a refresh reads for one pick.
#[derive(Clone, Debug)]
pub struct Job {
    pub key: String,
    pub pick: Pick,
    /// The agent's program, for an agent pick.
    pub program: Option<PathBuf>,
}

/// Refreshes `job`'s stale facts on a thread of its own: a sign-in check
/// that may start the harness's own status command, the model it runs,
/// and its plan windows from its provider. The rail picks the answer up on
/// a later frame.
pub fn refresh(job: Job) {
    std::thread::spawn(move || {
        let now = atif::now_ms() / 1000;
        let stale = cached(&job.key).unwrap_or_default().stale(now);
        let learned = learn(&job, stale, now);
        remember(&job.key, |entry| merge(entry, learned, now));
        REFRESHED.fetch_add(1, Ordering::Relaxed);
    });
}

/// What one refresh learned; `None` for a fact it didn't read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Learned {
    pub signed_in: Option<bool>,
    pub model: Option<Option<String>>,
    pub usage: Option<Option<String>>,
}

/// `learned` written into `entry` at `now`.
pub fn merge(entry: &mut Entry, learned: Learned, now: u64) {
    if let Some(signed_in) = learned.signed_in {
        entry.signed_in = Some(signed_in);
        entry.signed_in_at = now;
    }
    if let Some(model) = learned.model {
        entry.model = model;
        entry.model_at = now;
    }
    if let Some(usage) = learned.usage {
        // A refresh that couldn't read the windows keeps the last reading.
        if usage.is_some() {
            entry.usage = usage;
        }
        entry.usage_at = now;
    }
}

#[cfg(test)]
fn learn(_job: &Job, _stale: Stale, _now: u64) -> Learned {
    Learned::default()
}

#[cfg(not(test))]
fn learn(job: &Job, stale: Stale, now: u64) -> Learned {
    use coder::task::usage::{fetch, refresh};
    use microcoder_loop::capacity::Provider;
    let mut learned = Learned::default();
    let store = coder::task::local::default_store();
    match &job.pick {
        Pick::Auto => {}
        Pick::Harness(Harness::ClaudeCode) => {
            if stale.sign_in {
                crate::claude::recheck();
                learned.signed_in = Some(crate::claude::available());
            }
            learned.model = Some(Some(crate::claude::choice().model));
            if stale.usage && learned.signed_in != Some(false) {
                let _ = refresh(&store, &[Provider::Claude], now, fetch);
                learned.usage = Some(crate::claude_plan::rail_text(now));
            }
        }
        Pick::Harness(Harness::Codex) => {
            if stale.sign_in {
                crate::codex::recheck();
                learned.signed_in = Some(crate::codex::available());
            }
            learned.model = Some(Some(crate::codex::choice().model));
            if stale.usage && learned.signed_in != Some(false) {
                let book = refresh(&store, &[Provider::Codex], now, fetch);
                learned.usage = Some(
                    book.reading(Provider::Codex, now)
                        .map(|reading| windows_text(&reading.windows, now))
                        .filter(|text| !text.is_empty()),
                );
            }
        }
        Pick::Harness(Harness::OpenCode) => {
            if stale.sign_in {
                learned.signed_in = Some(crate::opencode_chat::installed());
            }
            learned.model = Some(Some(crate::opencode_chat::model(None)));
        }
        Pick::Harness(Harness::CoderLoop) => {}
        Pick::Agent { .. } => {
            if stale.sign_in {
                learned.signed_in = Some(
                    job.program
                        .as_deref()
                        .and_then(|program| {
                            acp_client::process::first_executable([program.to_owned()])
                        })
                        .is_some(),
                );
            }
        }
    }
    learned
}

/// A provider's windows as the rail shows them: "5h 12% · week 15%",
/// leaving out any that has reset by `now`.
#[must_use]
pub fn windows_text(windows: &[microcoder_loop::usage::Window], now: u64) -> String {
    use microcoder_loop::usage::WindowName;
    windows
        .iter()
        .filter(|window| window.resets_at.is_none_or(|at| at > now))
        .map(|window| {
            let seconds = window.length_seconds.unwrap_or(match window.window {
                WindowName::FiveHour => 5 * 3600,
                _ => 7 * 86_400,
            });
            let span = match seconds / 60 {
                300 => "5h".to_owned(),
                10_080 => "week".to_owned(),
                m if m % 1_440 == 0 && m > 0 => format!("{}d", m / 1_440),
                m if m % 60 == 0 && m > 0 => format!("{}h", m / 60),
                m => format!("{m}m"),
            };
            format!("{span} {}%", (window.used_fraction * 100.0).round() as u64)
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// What Shift+Tab reads about this computer for `app`, from cheap local
/// checks: the plugin switches, whether each program is on disk, and a
/// sign-in already known (this run's, else the cache's). A harness the
/// person said never to use is not offered.
#[must_use]
pub fn local(app: &crate::App) -> Local {
    use crate::routing_preferences::Stance;
    // A forced no-harness run offers only Auto (#11548).
    if crate::no_harness::forced() {
        return Local::default();
    }
    let routing = app.plugins.execution_settings(app.context_cwd()).routing;
    let leaning =
        crate::routing_preferences::Leaning::read(routing.as_deref(), |harness| match harness {
            Harness::ClaudeCode => Some(crate::claude::choice().model),
            Harness::OpenCode => Some(crate::opencode_chat::model(None)),
            Harness::CoderLoop => Some(app.plugins.model.clone()),
            Harness::Codex => Some(crate::codex::choice().model),
        });
    let allowed = |harness: Harness| {
        leaning.stances.get(harness) != Stance::Never && !leaning.never_with_model.get(harness)
    };
    let signed_out = |harness: Harness| {
        cached(&Pick::Harness(harness).key()).and_then(|entry| entry.signed_in) == Some(false)
    };
    let bundled = &app.plugins.bundled;
    Local {
        claude: bundled.claude
            && allowed(Harness::ClaudeCode)
            && crate::harness_plugin::claude_present()
            && !signed_out(Harness::ClaudeCode),
        codex: bundled.codex && allowed(Harness::Codex) && crate::codex::available(),
        opencode: bundled.opencode
            && bundled.acp
            && allowed(Harness::OpenCode)
            && crate::opencode_chat::installed(),
        agents: if bundled.acp {
            turn_agents(&bundled.acp_registered())
                .into_iter()
                .filter(|(id, _)| {
                    cached(&format!("agent:{id}")).and_then(|entry| entry.signed_in) != Some(false)
                })
                .collect()
        } else {
            Vec::new()
        },
    }
}

/// The rail's short word that `name` runs steps without asking first, so
/// Coder can't hold a deploy, a merge, a secret or a payment (#11556). The
/// full account is in /plugins.
#[must_use]
pub fn unasked_notice(name: &str) -> String {
    format!(
        "{name} runs steps without asking first; deploys, merges, secrets and payments aren't held."
    )
}

/// The name a pick goes by in a notice.
#[must_use]
pub fn name(pick: &Pick) -> String {
    match pick {
        Pick::Auto => "Auto".into(),
        Pick::Harness(Harness::ClaudeCode) => "Claude Code".into(),
        Pick::Harness(Harness::Codex) => "Codex".into(),
        Pick::Harness(Harness::OpenCode) => "OpenCode".into(),
        Pick::Harness(Harness::CoderLoop) => "Coder".into(),
        Pick::Agent { name, .. } => name.clone(),
    }
}

/// Whose models /models lists: the harness or agent on the rail (#11556).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelSource {
    /// One harness's own models: Claude Code's, Codex's, OpenCode's, or the
    /// Coder loop's gateway (and BYOK) models.
    Harness(Harness),
    /// One ACP agent's own models, when it lists them.
    Agent { id: String, name: String },
    /// Auto before it routed anything: it picks the agent per message, and
    /// the Coder loop's models are the ones it runs on its own loop.
    Auto,
}

impl crate::App {
    /// The one answer to "whose models does /models list" (#11556): the
    /// pick the rail shows ([`Self::current_pick`]), never the harness a
    /// chat ran on before a Shift+Tab switch took hold. On Auto, the
    /// harness Auto routed the chat to, once it has.
    #[must_use]
    pub fn model_source(&self) -> ModelSource {
        if self.mode != crate::Mode::Live {
            return ModelSource::Harness(Harness::CoderLoop);
        }
        match self.current_pick() {
            Pick::Harness(harness) => ModelSource::Harness(harness),
            Pick::Agent { id, name } => ModelSource::Agent { id, name },
            Pick::Auto => self
                .live
                .runs_on
                .filter(|_| !self.live.pinned && self.live.agent.is_none())
                .map_or(ModelSource::Auto, ModelSource::Harness),
        }
    }

    /// The ACP agents a turn can run on here, `(id, name)`.
    pub(crate) fn pick_agents(&self) -> Vec<(String, String)> {
        if !self.plugins.bundled.acp {
            return Vec::new();
        }
        turn_agents(&self.plugins.bundled.acp_registered())
    }

    /// The pick the rail shows and the chat's next turn takes: the one
    /// made with Shift+Tab since the last turn, else the chat's own pin.
    #[must_use]
    pub fn current_pick(&self) -> Pick {
        self.next_pick
            .clone()
            .unwrap_or_else(|| Pick::of_chat(&self.live, &self.pick_agents()))
    }

    /// Shift+Tab: the next available pick, highlighted. It takes hold at
    /// the next message, so a turn running now is left alone.
    pub fn cycle_pick(&mut self) {
        let options = options(&local(self));
        let pick = next(&options, &self.current_pick());
        self.pick_refresher.highlight(pick.key(), Instant::now());
        // An agent that runs steps without asking first says so once per
        // chat, on the rail; never in the transcript (#11556).
        self.note_unasked(&pick);
        // /models for the pick opens at once: its list is read ahead.
        match &pick {
            Pick::Harness(Harness::OpenCode) => crate::opencode_models::warm(),
            Pick::Agent { id, .. } if crate::agent_models::lists_models(id) => {
                if let Some(agent) = self
                    .plugins
                    .bundled
                    .acp_registered()
                    .into_iter()
                    .find(|agent| agent.id == *id)
                {
                    crate::agent_models::warm(id, &agent.program);
                }
            }
            _ => {}
        }
        self.next_pick = Some(pick);
    }

    /// Says once per chat, on the rail, that the agent `pick` names runs
    /// steps without asking first (#11556).
    pub(crate) fn note_unasked(&mut self, pick: &Pick) {
        if let Pick::Agent { id, name } = pick
            && !coder_sdk::acp::gated(id)
            && !self.unasked_noted.contains(id)
        {
            self.unasked_noted.push(id.clone());
            self.rail_notice(unasked_notice(name), crate::RAIL_NOTICE_TTL);
        }
    }

    /// Puts the Shift+Tab pick on the chat as its next message goes out: a
    /// harness or agent is pinned (recorded as a pin), Auto lifts the pin
    /// and goes back to the harness Auto had chosen.
    pub(crate) fn apply_pick(&mut self) {
        use crate::routing_record::Rule;
        // A chat pinned earlier stays pinned; its follow-ups continue.
        let Some(pick) = self.next_pick.take() else {
            return;
        };
        let was_auto = !self.live.pinned && self.live.agent.is_none();
        match pick {
            Pick::Auto => {
                if !was_auto {
                    self.live.pinned = false;
                    self.live.agent = None;
                    self.live.pinned_model = None;
                    self.live.runs_on = self.live.before_pin.take();
                    if self.routing_rule == Some(Rule::Pin) {
                        self.routing_rule = None;
                    }
                }
            }
            Pick::Harness(harness) => {
                // The person's own pick gets a harness that failed
                // earlier another try (#11645).
                self.start_failed.retain(|failed| *failed != harness);
                if was_auto {
                    self.live.before_pin = self.live.runs_on;
                }
                if self.live.runs_on != Some(harness) {
                    self.live.pinned_model = None;
                }
                self.live.agent = None;
                self.live.runs_on = Some(harness);
                self.live.pinned = true;
                self.routing_rule = Some(Rule::Pin);
            }
            Pick::Agent { id, .. } => {
                if was_auto {
                    self.live.before_pin = self.live.runs_on;
                }
                self.live.agent = Some(id);
                self.live.pinned = true;
            }
        }
    }

    /// The ACP agent this chat is pinned to, when it can still run here.
    /// One that can't any more drops the chat back to Auto, with a notice.
    pub(crate) fn picked_agent(&mut self) -> Option<AcpAgent> {
        let id = self.live.agent.clone()?;
        let found = self
            .plugins
            .bundled
            .acp
            .then(|| {
                self.plugins
                    .bundled
                    .acp_registered()
                    .into_iter()
                    .find(|agent| agent.id == id && agent.transport == AgentTransport::Acp)
            })
            .flatten();
        if found.is_none() {
            self.live.notice = Some(
                "The agent this chat was on isn't available here any more, so it's back on Auto."
                    .into(),
            );
            self.next_pick = Some(Pick::Auto);
            self.apply_pick();
        }
        found
    }

    /// The draft as a turn on `agent` (#11524): the conversation so far
    /// goes to one session of it, every step through Coder's checks. A
    /// message sent while a turn runs waits for it.
    pub(crate) fn submit_agent(&mut self, agent: AcpAgent) {
        if self.draft.text.trim().is_empty() {
            return;
        }
        if self.live.busy || self.checking_key || self.checking_jev {
            self.queue_prompt();
            return;
        }
        if !self.composer.images.is_empty() {
            self.live.notice = Some(crate::attachments::TEXT_ONLY_NOTICE.into());
        }
        self.record_prompt();
        self.cancel_request();
        let cwd = self
            .cwd
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let text = std::mem::take(&mut self.draft.text);
        let chars = text.chars().count();
        self.draft.cursor = 0;
        self.composer = Default::default();
        self.screen = crate::Screen::Conversation;
        self.live.finish_partial();
        self.live.entries.push(crate::live::Entry::User(text));
        let messages = self.live.messages();
        let briefing = self
            .live
            .briefing
            .as_ref()
            .map(|briefing| briefing.render(false));
        self.history.dirty = true;
        self.scroll = u16::MAX;
        self.begin_claude_turn();
        // The files the turn changes, for /rewind (#11515).
        self.begin_turn_record();
        // Its routing record names the agent (#11535).
        self.record_agent_choice(&agent.id, chars);
        self.request = Some(crate::live::Request {
            id: self.request_id,
            key: model_access::ApiKey::new(""),
            kind: crate::live::Work::Acp {
                agent,
                messages,
                cwd,
                briefing,
            },
        });
    }

    /// Each frame: refreshes a pick that settled, and once a refresh
    /// finds the picked harness gone, drops the chat back to Auto with a
    /// one-line notice.
    pub(crate) fn poll_pick(&mut self) {
        let current = self.current_pick();
        let now = atif::now_ms() / 1000;
        if let Some(key) = self.pick_refresher.due(Instant::now(), |key| {
            cached(key).unwrap_or_default().stale(now).any()
        }) && key == current.key()
        {
            let program = match &current {
                Pick::Agent { id, .. } => self
                    .plugins
                    .bundled
                    .acp_registered()
                    .into_iter()
                    .find(|agent| agent.id == *id)
                    .map(|agent| agent.program),
                _ => None,
            };
            refresh(Job {
                key,
                pick: current.clone(),
                program,
            });
        }
        let finished = refreshes();
        if finished == self.pick_refreshes_seen || current == Pick::Auto {
            self.pick_refreshes_seen = finished;
            return;
        }
        self.pick_refreshes_seen = finished;
        if !options(&local(self)).contains(&current) {
            // A chat that answered stays on its agent: the rail says it
            // can't go on where it ran; its next message goes to Auto
            // (#11563, #11645).
            if self.live.answered() {
                if let Some(why) = self.chat_cant_go_on() {
                    self.rail_notice(why, crate::RAIL_NOTICE_TTL);
                }
                return;
            }
            self.rail_notice(
                format!(
                    "{} isn't available here any more, so this chat is back on Auto.",
                    name(&current)
                ),
                crate::RAIL_NOTICE_TTL,
            );
            self.next_pick = Some(Pick::Auto);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Local {
        Local {
            claude: true,
            codex: true,
            opencode: true,
            agents: vec![("devin-cli".into(), "Devin CLI".into())],
        }
    }

    fn devin() -> Pick {
        Pick::Agent {
            id: "devin-cli".into(),
            name: "Devin CLI".into(),
        }
    }

    /// Auto, Claude Code, Codex, OpenCode, then the agents, and round to
    /// Auto again (#11524).
    #[test]
    fn shift_tab_cycles_in_the_fixed_order() {
        let options = options(&all());
        assert_eq!(
            options,
            [
                Pick::Auto,
                Pick::Harness(Harness::ClaudeCode),
                Pick::Harness(Harness::Codex),
                Pick::Harness(Harness::OpenCode),
                devin(),
            ]
        );
        let mut pick = Pick::Auto;
        let mut seen = Vec::new();
        for _ in 0..options.len() {
            pick = next(&options, &pick);
            seen.push(pick.clone());
        }
        assert_eq!(seen.last(), Some(&Pick::Auto));
        assert_eq!(seen[..4], options[1..]);
    }

    /// A harness that can't run here is never offered, and a pick that
    /// stopped being offered moves to Auto.
    #[test]
    fn only_available_harnesses_are_offered() {
        let mut some = all();
        some.claude = false;
        some.opencode = false;
        some.agents.clear();
        let options = options(&some);
        assert_eq!(options, [Pick::Auto, Pick::Harness(Harness::Codex)]);
        assert_eq!(next(&options, &Pick::Auto), Pick::Harness(Harness::Codex));
        assert_eq!(next(&options, &Pick::Harness(Harness::Codex)), Pick::Auto);
        assert_eq!(next(&options, &devin()), Pick::Auto);
        // Nothing but Auto: Shift+Tab stays there.
        assert_eq!(
            next(&self::options(&Local::default()), &Pick::Auto),
            Pick::Auto
        );
    }

    /// The harness agents of the ACP catalog are offered as harnesses,
    /// never twice; an agent turned off is not offered.
    #[test]
    fn agents_that_are_harnesses_are_not_offered_twice() {
        let agent = |id: &str, name: &str, enabled: bool, transport: AgentTransport| AcpAgent {
            id: id.into(),
            name: name.into(),
            program: PathBuf::from("/bin/true"),
            transport,
            arguments: Vec::new(),
            mode: None,
            enabled,
        };
        let found = turn_agents(&[
            agent(
                "claude-code",
                "Claude Code",
                true,
                AgentTransport::ClaudeCli,
            ),
            agent("grok-build", "Grok Build", true, AgentTransport::Acp),
            agent("devin-cli", "Devin CLI", true, AgentTransport::Acp),
            agent("opencode", "OpenCode", true, AgentTransport::Acp),
            agent("cursor", "Cursor", false, AgentTransport::Acp),
            agent("codex", "Codex", true, AgentTransport::CodexCli),
        ]);
        assert_eq!(
            found,
            [
                ("grok-build".to_owned(), "Grok Build".to_owned()),
                ("devin-cli".to_owned(), "Devin CLI".to_owned())
            ]
        );
    }

    /// A chat's pick is its pin: the pinned agent, else the pinned
    /// harness; a routed chat is on Auto.
    #[test]
    fn a_chats_pick_is_its_pin() {
        let mut chat = crate::live::Chat::default();
        assert_eq!(Pick::of_chat(&chat, &[]), Pick::Auto);
        chat.runs_on = Some(Harness::ClaudeCode);
        assert_eq!(Pick::of_chat(&chat, &[]), Pick::Auto);
        chat.pinned = true;
        assert_eq!(
            Pick::of_chat(&chat, &[]),
            Pick::Harness(Harness::ClaudeCode)
        );
        chat.agent = Some("devin-cli".into());
        assert_eq!(
            Pick::of_chat(&chat, &[("devin-cli".into(), "Devin CLI".into())]),
            devin()
        );
        assert_eq!(Pick::Harness(Harness::OpenCode).key(), "opencode");
        assert_eq!(Pick::Harness(Harness::ClaudeCode).key(), "claude-code");
        assert_eq!(devin().key(), "agent:devin-cli");
    }

    /// Usage goes stale after five minutes, models after a day, sign-ins
    /// after five minutes; a fact never learned is stale.
    #[test]
    fn facts_go_stale_on_their_own_clocks() {
        let now = 1_000_000;
        assert_eq!(
            Entry::default().stale(now),
            Stale {
                sign_in: true,
                model: true,
                usage: true
            }
        );
        let entry = Entry {
            signed_in: Some(true),
            signed_in_at: now - 60,
            model: Some("claude-opus-5-5".into()),
            model_at: now - 3600,
            usage: Some("5h 12%".into()),
            usage_at: now - 60,
        };
        assert!(!entry.stale(now).any());
        let later = now + USAGE_FRESH;
        let stale = entry.stale(later);
        assert!(stale.usage && stale.sign_in && !stale.model);
        assert!(entry.stale(now - 3600 + MODEL_FRESH).model);
        // A stale reading still shows for a while, then is left out.
        assert_eq!(entry.usage_now(later), Some("5h 12%"));
        assert_eq!(entry.usage_now(now - 60 + USAGE_SHOWN), None);
    }

    /// A refresh writes what it read with the time; windows it couldn't
    /// read keep the last reading but count as read.
    #[test]
    fn a_refresh_merges_what_it_learned() {
        let mut entry = Entry {
            usage: Some("5h 12%".into()),
            usage_at: 10,
            ..Entry::default()
        };
        merge(
            &mut entry,
            Learned {
                signed_in: Some(false),
                model: None,
                usage: Some(None),
            },
            100,
        );
        assert_eq!(entry.signed_in, Some(false));
        assert_eq!(entry.signed_in_at, 100);
        assert_eq!(entry.usage.as_deref(), Some("5h 12%"));
        assert_eq!(entry.usage_at, 100);
        assert_eq!(entry.model_at, 0);
    }

    /// The cache survives a save and a load.
    #[test]
    fn the_cache_round_trips_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("harness-facts.json");
        assert_eq!(Cache::load(&path), Cache::default());
        let mut cache = Cache::default();
        cache.picks.insert(
            "codex".into(),
            Entry {
                model: Some("gpt-6.1-sol".into()),
                model_at: 5,
                ..Entry::default()
            },
        );
        cache.save(&path).unwrap();
        assert_eq!(Cache::load(&path), cache);
    }

    /// Cycling quickly past a pick starts no work for it; one held for
    /// [`SETTLE`] refreshes once, then again only when stale.
    #[test]
    fn refreshes_wait_for_the_pick_to_settle() {
        let start = Instant::now();
        let mut refresher = Refresher::default();
        let fresh = |_: &str| false;
        refresher.highlight("claude-code".into(), start);
        refresher.highlight("codex".into(), start + Duration::from_millis(100));
        refresher.highlight("opencode".into(), start + Duration::from_millis(200));
        assert_eq!(
            refresher.due(start + Duration::from_millis(400), fresh),
            None
        );
        assert_eq!(
            refresher.due(start + Duration::from_millis(500), fresh),
            Some("opencode".into())
        );
        // Asked once.
        assert_eq!(
            refresher.due(start + Duration::from_millis(900), fresh),
            None
        );
        // Highlighted again: fresh facts start nothing, stale ones do.
        let again = start + Duration::from_secs(1);
        refresher.highlight("opencode".into(), again);
        assert_eq!(refresher.due(again + SETTLE, fresh), None);
        refresher.highlight("opencode".into(), again);
        assert_eq!(
            refresher.due(again + SETTLE, |_| true),
            Some("opencode".into())
        );
        // Auto has nothing of its own to load.
        refresher.highlight("auto".into(), again);
        assert_eq!(refresher.due(again + SETTLE, |_| true), None);
        // The same pick highlighted again keeps its first time.
        let mut held = Refresher::default();
        held.highlight("codex".into(), start);
        held.highlight("codex".into(), start + Duration::from_millis(250));
        assert_eq!(held.due(start + SETTLE, fresh), Some("codex".into()));
    }

    #[test]
    fn windows_read_as_the_rail_writes_them() {
        use microcoder_loop::usage::{Window, WindowName};
        let windows = [
            Window {
                window: WindowName::Primary,
                used_fraction: 0.12,
                resets_at: Some(200),
                length_seconds: Some(5 * 3600),
            },
            Window {
                window: WindowName::Secondary,
                used_fraction: 0.151,
                resets_at: Some(200),
                length_seconds: Some(7 * 86_400),
            },
        ];
        assert_eq!(windows_text(&windows, 100), "5h 12% · week 15%");
        assert_eq!(windows_text(&windows, 300), "");
    }
    fn shift_tab(app: &mut crate::App) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
        app.handle(Event::Key(KeyEvent::new(
            KeyCode::BackTab,
            KeyModifiers::SHIFT,
        )));
    }

    fn rail(app: &mut crate::App) -> String {
        use ratatui::{Terminal, backend::TestBackend};
        let mut terminal = Terminal::new(TestBackend::new(90, 20)).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..20)
            .map(|y| (0..90).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Shift+Tab steps the available harnesses with text in the input and
    /// never sends it; the pick takes hold at the next message as a pin,
    /// and Auto lifts it again (#11524).
    #[test]
    fn shift_tab_pins_the_chat_and_auto_lifts_the_pin() {
        use crate::routing_record::Rule;
        crate::harness_plugin::sign_in_claude(true);
        crate::opencode_chat::install_for_test(true);
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.draft = crate::Draft {
            text: "half typed".into(),
            cursor: 10,
            pastes: Vec::new(),
        };
        assert_eq!(app.current_pick(), Pick::Auto);
        assert!(rail(&mut app).contains("Auto"));
        // Codex isn't installed in a test: Claude Code, OpenCode, Auto.
        let mut seen = Vec::new();
        for _ in 0..3 {
            shift_tab(&mut app);
            seen.push(app.current_pick());
        }
        assert_eq!(
            seen,
            [
                Pick::Harness(Harness::ClaudeCode),
                Pick::Harness(Harness::OpenCode),
                Pick::Auto
            ]
        );
        assert_eq!(app.draft.text, "half typed");
        assert!(app.live.entries.is_empty() && app.request.is_none());
        shift_tab(&mut app);
        let shown = rail(&mut app);
        assert!(shown.contains("Claude ("), "{shown}");
        assert!(!shown.contains("Auto"), "{shown}");
        // The chat routed by Auto earlier keeps that harness to go back to.
        app.live.runs_on = Some(Harness::CoderLoop);
        app.apply_pick();
        assert_eq!(app.live.runs_on, Some(Harness::ClaudeCode));
        assert!(app.live.pinned);
        assert_eq!(app.routing_rule, Some(Rule::Pin));
        assert_eq!(app.current_pick(), Pick::Harness(Harness::ClaudeCode));
        // A follow-up with no new pick stays pinned.
        app.routing_rule = None;
        app.apply_pick();
        assert!(app.live.pinned);
        shift_tab(&mut app);
        shift_tab(&mut app);
        assert_eq!(app.current_pick(), Pick::Auto);
        app.apply_pick();
        assert!(!app.live.pinned);
        assert_eq!(app.live.runs_on, Some(Harness::CoderLoop));
        // What Auto routed shows after its name.
        let shown = rail(&mut app);
        assert!(
            shown.contains("Auto → Coder") && !shown.contains("(auto)"),
            "{shown}"
        );
        crate::harness_plugin::sign_in_claude(false);
        crate::opencode_chat::install_for_test(false);
    }

    /// Ctrl+N keeps the pick for the new chat (#11480).
    #[test]
    fn a_new_chat_keeps_the_pick() {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
        crate::opencode_chat::install_for_test(true);
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        shift_tab(&mut app);
        assert_eq!(app.current_pick(), Pick::Harness(Harness::OpenCode));
        app.handle(Event::Key(KeyEvent::new(
            KeyCode::Char('n'),
            KeyModifiers::CONTROL,
        )));
        assert_eq!(app.current_pick(), Pick::Harness(Harness::OpenCode));
        // Also once the pick had pinned the last chat.
        app.apply_pick();
        assert!(app.live.pinned);
        app.handle(Event::Key(KeyEvent::new(
            KeyCode::Char('n'),
            KeyModifiers::CONTROL,
        )));
        assert!(!app.live.pinned);
        assert_eq!(app.current_pick(), Pick::Harness(Harness::OpenCode));
        crate::opencode_chat::install_for_test(false);
    }

    /// An agent pick pins the chat to the agent, and the pin is saved
    /// with the chat like a harness pin.
    #[test]
    fn an_agent_pick_is_saved_with_the_chat() {
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.next_pick = Some(devin());
        app.apply_pick();
        assert_eq!(app.live.agent.as_deref(), Some("devin-cli"));
        assert!(app.live.pinned);
        app.live.entries.push(crate::live::Entry::User("hi".into()));
        let saved =
            crate::trajectory::document(&app.live, "chat", "auto", std::path::Path::new("."));
        assert_eq!(saved["extra"]["agent"], "devin-cli");
        let reopened = crate::trajectory::from_document(&saved).unwrap();
        assert_eq!(reopened.agent.as_deref(), Some("devin-cli"));
        assert_eq!(
            Pick::of_chat(&reopened, &[("devin-cli".into(), "Devin CLI".into())]),
            devin()
        );
        // An agent that can't run here drops the chat back to Auto.
        assert!(app.picked_agent().is_none());
        assert_eq!(app.live.agent, None);
        assert!(!app.live.pinned);
        assert!(
            app.live
                .notice
                .as_deref()
                .is_some_and(|notice| notice.contains("back on Auto"))
        );
    }

    /// A turn on the picked ACP agent writes its routing record naming the
    /// agent and the version it reported, its own login paying (#11535).
    #[test]
    fn an_agent_turn_writes_a_routing_record_naming_the_agent() {
        use crate::routing_record::{Kind, Line, Rule};
        let dir = tempfile::tempdir().unwrap();
        let health = crate::harness_plugin::Health {
            signed_in: false,
            at_limit: false,
        };
        crate::routing_record::record_to(Some((
            dir.path().to_owned(),
            crate::harness_plugin::Facts {
                local: true,
                claude_plugin_on: false,
                claude: health,
                codex_on: false,
                codex: health,
                opencode_on: false,
                opencode: health,
                openrouter_key: false,
                leaning: crate::routing_preferences::Leaning::default(),
                failed: Default::default(),
            },
        )));
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.draft.text = "fix the test".into();
        let agent: AcpAgent =
            serde_json::from_value(serde_json::json!({"id":"amp","name":"Amp","program":"grok"}))
                .unwrap();
        app.submit_agent(agent);
        // The agent reports its version when its session starts.
        crate::routing_record::note_agent_version("amp", Some("0.9.4"));
        let id = app.request_id;
        app.apply_update(crate::live::Update::Finished {
            id,
            result: Ok(openrouter::Streamed {
                text: "done".into(),
                model: "grok-4.6".into(),
                ..Default::default()
            }),
        });
        crate::routing_record::record_to(None);
        let lines = crate::routing_record::read(dir.path());
        let Some(Line::Chose(chose)) = lines.first() else {
            panic!("{lines:?}");
        };
        let agent = chose.agent.as_ref().expect("the agent is named");
        assert_eq!(agent.id, "amp");
        assert_eq!(chose.kind, Kind::Primary);
        assert_eq!(chose.rule, Rule::Pin);
        assert_eq!(chose.payer, crate::harness_plugin::Payer::AgentLogin);
        let Some(Line::Ended(ended)) = lines.get(1) else {
            panic!("{lines:?}");
        };
        let agent = ended.agent.as_ref().expect("the agent is named");
        assert_eq!(
            (agent.id.as_str(), agent.version.as_deref()),
            ("amp", Some("0.9.4"))
        );
        // Its turns are counted apart from the Coder loop's.
        let summary = crate::routing_record::summarize(&lines);
        let amp = summary
            .iter()
            .find(|summary| summary.agent.as_deref() == Some("amp"))
            .expect("the agent's own summary");
        assert_eq!((amp.harness, amp.chosen), (None, 1));
        assert!(
            summary
                .iter()
                .all(|summary| summary.harness != Some(Harness::CoderLoop) || summary.chosen == 0)
        );
    }

    fn key(app: &mut crate::App, code: crossterm::event::KeyCode) {
        use crossterm::event::{Event, KeyEvent, KeyModifiers};
        app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
    }

    fn ctrl_n(app: &mut crate::App) {
        use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
        app.handle(Event::Key(KeyEvent::new(
            KeyCode::Char('n'),
            KeyModifiers::CONTROL,
        )));
    }

    fn answered(app: &mut crate::App) {
        app.live.entries.push(crate::live::Entry::User("hi".into()));
        app.live.entries.push(crate::live::Entry::Assistant {
            text: "Hello.".into(),
            model: None,
            elapsed_ms: None,
        });
    }

    fn picker_plugins(app: &crate::App) -> Vec<String> {
        app.model_picker
            .as_ref()
            .expect("/models opened")
            .models
            .iter()
            .map(|model| model.plugin.clone())
            .collect()
    }

    /// Shift+Tab switches agents only in a new chat (#11556): once the
    /// chat has a turn it leaves the agent alone, and the rail says how
    /// to switch, never the transcript.
    #[test]
    fn shift_tab_switches_only_before_the_first_message() {
        crate::opencode_chat::install_for_test(true);
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        shift_tab(&mut app);
        assert_eq!(app.current_pick(), Pick::Harness(Harness::OpenCode));
        app.apply_pick();
        answered(&mut app);
        let entries = app.live.entries.len();
        shift_tab(&mut app);
        assert_eq!(app.current_pick(), Pick::Harness(Harness::OpenCode));
        assert_eq!(app.rail_notice_text(), Some(LOCKED_HINT));
        assert_eq!(app.notice, None);
        assert_eq!(app.live.entries.len(), entries);
        let shown = rail(&mut app);
        assert!(
            shown.contains("Start a new chat (Ctrl+N) to switch agents"),
            "{shown}"
        );
        // A new chat switches again.
        ctrl_n(&mut app);
        assert!(app.live.entries.is_empty());
        shift_tab(&mut app);
        assert_eq!(app.current_pick(), Pick::Auto);
        crate::opencode_chat::install_for_test(false);
    }

    /// /models lists the models of the harness on the rail from the first
    /// frame of a new chat, before any turn (#11556).
    #[test]
    fn models_lists_the_rails_harness_before_any_turn() {
        crate::opencode_models::keep(crate::opencode_models::parse_verbose(
            &crate::opencode_models::tests::fixture(),
        ));
        let open = |pick: Pick| {
            let mut app = crate::App::default();
            app.set_mode(crate::Mode::Live);
            app.next_pick = Some(pick);
            app.open_models();
            app
        };
        // Codex: Codex's models only, the current one marked.
        let app = open(Pick::Harness(Harness::Codex));
        assert!(
            picker_plugins(&app)
                .iter()
                .all(|plugin| plugin == crate::codex::PICKER)
        );
        let picker = app.model_picker.as_ref().unwrap();
        assert!(
            picker
                .models
                .iter()
                .any(|model| model.name == "GPT-6.1-Sol")
        );
        assert_eq!(picker.active_model, crate::codex::choice().model);
        // Claude Code: Claude's.
        let app = open(Pick::Harness(Harness::ClaudeCode));
        assert!(
            picker_plugins(&app)
                .iter()
                .all(|plugin| plugin == crate::claude::PICKER)
        );
        // OpenCode: OpenCode's own picker, Free first, no gateway models.
        let app = open(Pick::Harness(Harness::OpenCode));
        let picker = app.model_picker.as_ref().unwrap();
        assert!(
            picker_plugins(&app)
                .iter()
                .all(|plugin| plugin == crate::opencode_chat::PLUGIN)
        );
        assert!(
            !picker
                .models
                .iter()
                .any(|model| model.provider == "OpenAgents"
                    || model.plugin == crate::gateway_models::PICKER)
        );
        assert_eq!(picker.models[0].id, "opencode/cat-jam-free");
        assert_eq!(picker.models[0].group.as_deref(), Some("Free"));
        assert_eq!(picker.models[1].id, "opencode/big-pickle");
        assert_eq!(picker.title.as_deref(), Some("Select model"));
        // Its search matches words fuzzily, as OpenCode's does.
        let mut app = open(Pick::Harness(Harness::OpenCode));
        for c in "cmp 2.5".chars() {
            key(&mut app, crossterm::event::KeyCode::Char(c));
        }
        let found: Vec<_> = app
            .model_picker
            .as_ref()
            .unwrap()
            .matching()
            .iter()
            .map(|model| model.id.clone())
            .collect();
        assert_eq!(found, ["cursor/composer-2.5"]);
        // Devin CLI: Devin's own list, SWE-2 first.
        crate::agent_models::keep(
            crate::agent_models::DEVIN,
            crate::agent_models::parse_devin(
                "SWE-1.6 Slow (swe-1.6-slow)\n  swe-1-6-slow   SWE-1.6 Slow\n\nSWE-2 (swe-2)\n  swe-2-medium   SWE-2 Medium\n",
            ),
        );
        let app = open(devin());
        let picker = app.model_picker.as_ref().unwrap();
        assert!(
            picker_plugins(&app)
                .iter()
                .all(|plugin| plugin == "agent:devin-cli")
        );
        assert_eq!(picker.models[0].group.as_deref(), Some("SWE-2"));
    }

    /// Switching the harness in a new chat, then /models, lists the new
    /// harness's models.
    #[test]
    fn models_follows_a_shift_tab_switch() {
        crate::harness_plugin::sign_in_claude(true);
        crate::opencode_chat::install_for_test(true);
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.plugins.enabled = true;
        // A chat Auto routed to the Coder loop earlier, not answered yet.
        app.live.runs_on = Some(Harness::CoderLoop);
        shift_tab(&mut app);
        app.open_models();
        assert!(
            picker_plugins(&app)
                .iter()
                .all(|plugin| plugin == crate::claude::PICKER)
        );
        app.model_picker = None;
        shift_tab(&mut app);
        assert_eq!(app.current_pick(), Pick::Harness(Harness::OpenCode));
        crate::opencode_models::keep(crate::opencode_models::parse_verbose(
            &crate::opencode_models::tests::fixture(),
        ));
        app.open_models();
        assert!(
            picker_plugins(&app)
                .iter()
                .all(|plugin| plugin == crate::opencode_chat::PLUGIN)
        );
        crate::harness_plugin::sign_in_claude(false);
        crate::opencode_chat::install_for_test(false);
    }

    /// After the first turn a chat keeps its model: another pick in
    /// /models says how to use it and changes nothing (#11556).
    #[test]
    fn a_model_pick_after_a_turn_changes_nothing() {
        use crossterm::event::KeyCode;
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.plugins.enabled = true;
        app.live.runs_on = Some(Harness::CoderLoop);
        answered(&mut app);
        let before = app.plugins.model.clone();
        app.open_models();
        let picker = app.model_picker.as_ref().unwrap();
        let other = picker
            .models
            .iter()
            .position(|model| model.id != picker.active_model)
            .unwrap();
        app.model_picker.as_mut().unwrap().selected = other;
        for _ in 0..3 {
            if app.model_picker.is_some() {
                key(&mut app, KeyCode::Enter);
            }
        }
        assert!(app.model_picker.is_none());
        assert_eq!(app.plugins.model, before);
        assert_eq!(app.rail_notice_text(), Some(crate::MODEL_LOCKED_HINT));
        assert_eq!(app.notice, None);
    }

    /// Auto never hops (#11556): once a turn answered, a later message
    /// routed elsewhere stays on the chat's harness and path.
    #[test]
    fn auto_keeps_one_harness_for_the_whole_chat() {
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.live.runs_on = Some(Harness::Codex);
        app.live.cheap = true;
        answered(&mut app);
        app.routed = Some(crate::router::Routed {
            harness: Harness::ClaudeCode,
            model: None,
            rule: crate::routing_record::Rule::Router,
            probabilities: Vec::new(),
            trace: crate::router::Trace {
                question_set: String::new(),
                policy: String::new(),
                step: Some("new-work".into()),
                follow_up: false,
                ms: 0,
                missing: None,
            },
        });
        assert_eq!(app.route_turn(), Harness::Codex);
        assert_eq!(app.live.runs_on, Some(Harness::Codex));
        assert!(app.live.cheap, "the same path, so the same payer");
    }

    /// A model picked in one chat never carries into the next (#11556).
    #[test]
    fn a_new_chat_starts_on_the_default_model() {
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.plugins.model = crate::models::OPENROUTER.into();
        ctrl_n(&mut app);
        assert_eq!(app.plugins.model, crate::models::AUTO);
        assert_eq!(app.gateway_model, None);
    }

    /// An agent that runs steps without asking first says so on the rail,
    /// once per chat, and the rail names its model in its own words.
    #[test]
    fn an_unasking_agent_is_noted_once_on_the_rail() {
        let cursor = Pick::Agent {
            id: "cursor".into(),
            name: "Cursor".into(),
        };
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.note_unasked(&cursor);
        let said = app.rail_notice_text().unwrap().to_owned();
        assert!(
            said.starts_with("Cursor runs steps without asking first"),
            "{said}"
        );
        assert!(app.live.entries.is_empty() && app.notice.is_none());
        app.rail_status = None;
        app.note_unasked(&cursor);
        assert_eq!(app.rail_notice_text(), None, "once per chat");
        ctrl_n(&mut app);
        app.rail_status = None;
        app.note_unasked(&cursor);
        assert!(app.rail_notice_text().is_some(), "and again in a new chat");
        // The rail names Cursor's model as words.
        remember("agent:cursor", |entry| {
            entry.model = Some("composer-2.5[fast=true]".into());
        });
        app.next_pick = Some(cursor);
        let shown = rail(&mut app);
        assert!(shown.contains("Cursor (Composer 2.5 Fast)"), "{shown}");
    }

    /// Devin CLI's rail names SWE-2 in Devin's words before any turn.
    #[test]
    fn devin_clis_rail_names_swe_2() {
        crate::agent_models::forget(crate::agent_models::DEVIN);
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.next_pick = Some(Pick::Agent {
            id: "devin-cli".into(),
            name: "Devin CLI".into(),
        });
        let shown = rail(&mut app);
        assert!(shown.contains("Devin CLI (SWE-2 Medium)"), "{shown}");
    }
}
