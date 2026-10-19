//! Agent wakes and grants (#11965, #12044). For every saved agent whose
//! definition declares wakes, the host that runs its chat wakes it on its
//! account schedule and when something it manages changes, enforces the
//! grants the account owner gave it, and keeps its decision log. Each agent
//! has its own engine, keyed by its definition ID; nothing here knows an
//! agent by name.
//!
//! This module is the host-independent part: the saved state, the
//! append-only log, the coalescing of events into one check-in, the
//! check-in prompt, and reading GitHub's answers. `doc_host/agent_wakes.rs`
//! drives it from the existing host loop.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use openagents_proto::agent_wakes::{
    DecisionEntry, Grant, JudgmentReceipt, MAX_EVERY_MINUTES, MAX_STALL_MINUTES, MAX_STUCK_MINUTES,
    MIN_EVERY_MINUTES, MIN_STALL_MINUTES, MIN_STUCK_MINUTES, STALL_RUNG_MINUTES, Scope,
    Wake as WakeOn, WakeReason, WakeSettings,
};
use openagents_proto::saved_agent::Declared;
use openagents_proto::{Session, SessionStatus, ToolCall};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::EngineError;

/// Events wait this long for quiet before they start a check-in, unless the
/// agent's definition sets its own coalescing window…
pub(crate) const QUIET_MS: i64 = 60_000;
/// …but never longer than this after the first one.
pub(crate) const MAX_WAIT_MS: i64 = 5 * 60_000;
/// The least time between two check-ins started by events.
pub(crate) const MIN_GAP_MS: i64 = 3 * 60_000;
/// A claim with no newer comment for this long is stale.
pub(crate) const STALE_CLAIM_MS: i64 = 6 * 3600 * 1000;
/// How many recent entries each check-in prompt carries.
const PROMPT_ENTRIES: usize = 12;
/// The most entries the settings card shows.
pub(crate) const STATUS_ENTRIES: usize = 20;
/// A field an agent records is cut at this many bytes.
const MAX_FIELD: usize = 2000;
/// The most judgment receipts one decision keeps.
const MAX_JUDGMENTS: usize = 8;
const MAX_EVENTS: usize = 200;
const MAX_WAKES: usize = 64;
const MAX_ISSUES: usize = 1000;
const MAX_MANAGED: usize = 512;
const CLAIM_MARK: &str = "<!-- openagents-coder-claim";
const RELEASE_MARK: &str = "<!-- openagents-coder-release";

/// Something that happened that an agent should hear about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Event {
    ChatFinished {
        chat: String,
        title: String,
    },
    ChatErrored {
        chat: String,
        title: String,
    },
    ChatAwaitingInput {
        chat: String,
        title: String,
    },
    IssueOpened {
        number: u64,
        title: String,
    },
    IssueClosed {
        number: u64,
        title: String,
    },
    StaleClaim {
        number: u64,
        title: String,
        session: String,
        since: i64,
    },
    /// A managed chat has been working with no output for the stall window
    /// (#12032).
    ChatStalled {
        chat: String,
        title: String,
        since: i64,
    },
    /// A claimed issue has had no comment, no commit naming it, no push to
    /// its linked branch, and no local progress (its chat's output, terminal
    /// run activity, or a build in its worktree) for the stall window.
    ClaimNoProgress {
        number: u64,
        title: String,
        session: String,
        since: i64,
    },
}

impl Event {
    /// What an agent declares to wake on this.
    pub fn kind(&self) -> WakeOn {
        match self {
            Self::ChatFinished { .. } => WakeOn::ChatFinished,
            Self::ChatErrored { .. } => WakeOn::ChatErrored,
            Self::ChatAwaitingInput { .. } => WakeOn::ChatNeedsAnswer,
            Self::IssueOpened { .. } => WakeOn::IssueOpened,
            Self::IssueClosed { .. } => WakeOn::IssueClosed,
            Self::StaleClaim { .. } => WakeOn::ClaimStale,
            Self::ChatStalled { .. } => WakeOn::ChatStalled,
            Self::ClaimNoProgress { .. } => WakeOn::ClaimNoProgress,
        }
    }

    /// One plain line for the prompt and the log.
    pub fn line(&self) -> String {
        match self {
            Self::ChatFinished { chat, title } => {
                format!("Chat \"{title}\" ({}) finished its turn.", short(chat))
            }
            Self::ChatErrored { chat, title } => {
                format!("Chat \"{title}\" ({}) stopped with an error.", short(chat))
            }
            Self::ChatAwaitingInput { chat, title } => {
                format!(
                    "Chat \"{title}\" ({}) is waiting for an answer.",
                    short(chat)
                )
            }
            Self::IssueOpened { number, title } => format!("Issue #{number} opened: {title}"),
            Self::IssueClosed { number, title } => format!("Issue #{number} closed: {title}"),
            Self::StaleClaim {
                number,
                title,
                session,
                since,
            } => format!(
                "The claim on #{number} ({title}) by {session} has had no activity since {}.",
                clock(*since)
            ),
            Self::ChatStalled { chat, title, since } => format!(
                "Chat \"{title}\" ({}) is working but has produced no output since {}.",
                short(chat),
                clock(*since)
            ),
            Self::ClaimNoProgress {
                number,
                title,
                session,
                since,
            } => format!(
                "The claim on #{number} ({title}) by {session} has shown no progress (no commit, push, comment, chat output, terminal activity, or build) since {}.",
                clock(*since)
            ),
        }
    }
}

/// Work with no progress for the stall window, kept until it moves again,
/// so every check-in can say which rung of the ladder is due (#12032).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Stall {
    /// What stalled, in plain words.
    pub subject: String,
    /// Its last progress, Unix ms.
    pub since: i64,
    /// The last rung a check-in announced; reaching the next one starts a
    /// check-in of its own, so each rung is heard when it is due.
    #[serde(default)]
    pub rung: u32,
    /// The owner was told on the last rung.
    #[serde(default)]
    pub told: bool,
}

/// A stall on its last rung, for the owner's phone (#12040).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnerNotice {
    /// Stable per stall (its key and start), so a retry never notifies twice.
    pub request_id: String,
    pub title: String,
    pub body: String,
}

impl Stall {
    /// The rung due now: 1 at the stall window, then one more every
    /// [`STALL_RUNG_MINUTES`], up to 3.
    fn rung_at(&self, now: i64, window_minutes: u32) -> u32 {
        let over = (now - self.since) / 60_000 - i64::from(window_minutes);
        if over < 0 {
            return 0;
        }
        (1 + over / i64::from(STALL_RUNG_MINUTES)).min(3) as u32
    }

    /// One line for the check-in: how long, and the rung that is due.
    fn line(&self, now: i64, window_minutes: u32) -> String {
        let age = (now - self.since).max(0) / 60_000;
        let due = match self.rung_at(now, window_minutes) {
            0 | 1 => "rung 1: message it",
            2 => "rung 2: interrupt it and restart it with a brief",
            _ => "rung 3: tell the owner, first in your status",
        };
        format!(
            "{}: no progress for {age} minutes. Due: {due}.",
            self.subject
        )
    }
}

/// What the host sees of claims on this computer: each app chat's last
/// output and each live terminal run's last activity by its harness session
/// (`claude-code:ID`, as claim markers name it), and the last time a build
/// ran in a worktree named for an issue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LocalProgress {
    pub chats: HashMap<String, i64>,
    pub runs: HashMap<String, i64>,
    pub builds: BTreeMap<u64, i64>,
    /// Each issue's latest output from an app chat whose title names it
    /// (`#N`), whoever holds the claim.
    pub titled: BTreeMap<u64, i64>,
}

impl LocalProgress {
    pub fn with_runs(mut self, runs: &[coder_lease::runs::RunStatus]) -> Self {
        for run in runs {
            if !run.alive {
                continue;
            }
            let harness = match run.kind.as_str() {
                "claude" | "claude-code" => "claude-code",
                other => other,
            };
            let last = run.last_activity.saturating_mul(1000).min(i64::MAX as u64) as i64;
            for session in &run.sessions {
                self.runs
                    .entry(format!("{harness}:{session}"))
                    .and_modify(|seen| *seen = (*seen).max(last))
                    .or_insert(last);
            }
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Pending {
    at: i64,
    event: Event,
}

/// A turn of the chat's own that the host stopped for going the stuck
/// window with no output (#12029). Kept until its check-in starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Stuck {
    /// The turn, by its start time: one stop per turn.
    pub turn: i64,
    /// Its last output, Unix ms.
    pub since: i64,
    /// When the host stopped it.
    pub at: i64,
    /// The tool call it was waiting on, in plain words.
    pub tool: Option<String>,
    pub checked_in: bool,
}

impl Stuck {
    /// The check-in's first line.
    fn line(&self) -> String {
        let minutes = (self.at - self.since).max(0) / 60_000;
        let minutes = if minutes == 1 {
            "1 minute".to_owned()
        } else {
            format!("{minutes} minutes")
        };
        match &self.tool {
            Some(tool) => {
                format!("Your last turn was stuck in {tool} for {minutes}, so the host stopped it.")
            }
            None => {
                format!("Your last turn produced no output for {minutes}, so the host stopped it.")
            }
        }
    }
}

/// A standing authority saved before grants (#11965). The host turns it into
/// an account grant once, then drops it (#12044).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LegacyAuthority {
    pub granted_at: i64,
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// The saved part of one agent's wake state on this host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct State {
    pub settings: WakeSettings,
    /// The agent's grants, as the account last listed them. Enforced on
    /// every command its chat sends.
    pub grants: Vec<Grant>,
    /// A standing authority from before grants, still to become a grant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority: Option<LegacyAuthority>,
    /// The agent's definition asked for standing authority when this host
    /// turned its check-ins on; the host asks the account to grant it once.
    pub wants_standing: bool,
    /// The account granted `log.write` to an agent whose log this host
    /// already kept before the grant existed (#12045). Until then that
    /// agent may keep writing; after, only the grant counts.
    pub log_grant_moved: bool,
    /// What the agent's definition declares it wakes on; empty is every wake.
    pub wakes_on: Vec<WakeOn>,
    /// The definition's coalescing window, when it sets one.
    pub coalesce_ms: Option<i64>,
    /// The account schedule this host mirrors, and the `updated` time of
    /// the copy it last wrote or adopted.
    pub schedule_id: Option<String>,
    pub schedule_updated: u64,
    /// The local copy changed and the account hasn't heard yet.
    pub schedule_dirty: bool,
    /// Unix ms of the last check-in of any kind, and of the last scheduled one.
    pub last_check_in: i64,
    pub last_scheduled: i64,
    /// Chats the agent created, messaged, steered or stopped.
    pub managed: BTreeSet<String>,
    /// Recent check-in message ids, newest last.
    pub wakes: VecDeque<String>,
    /// The check-in whose outcome isn't logged yet.
    pub open_wake: Option<String>,
    pending: Vec<Pending>,
    /// Issue number → open, from the last GitHub look.
    issues: BTreeMap<u64, bool>,
    /// Stale claims already reported, as `number:since`.
    stale: BTreeSet<String>,
    pub last_github: i64,
    /// The last turn the host stopped for being stuck.
    pub stuck: Option<Stuck>,
    /// Chats this host picked by itself because their agent declares
    /// check-ins; never picked again once the owner changes the choice.
    adopted: BTreeSet<String>,
    /// When this host picked the current chat by itself, and its agent's
    /// definition ID. Cleared when the owner picks another chat.
    pub turned_on_at: Option<i64>,
    pub agent: Option<String>,
    /// Open stalls by `chat:ID` or `claim:N`, each raised once.
    pub stalls: BTreeMap<String, Stall>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            settings: WakeSettings::default(),
            grants: Vec::new(),
            authority: None,
            wants_standing: false,
            log_grant_moved: false,
            wakes_on: Vec::new(),
            coalesce_ms: None,
            schedule_id: None,
            schedule_updated: 0,
            schedule_dirty: false,
            last_check_in: 0,
            last_scheduled: 0,
            managed: BTreeSet::new(),
            wakes: VecDeque::new(),
            open_wake: None,
            pending: Vec::new(),
            issues: BTreeMap::new(),
            stale: BTreeSet::new(),
            last_github: 0,
            stuck: None,
            adopted: BTreeSet::new(),
            turned_on_at: None,
            agent: None,
            stalls: BTreeMap::new(),
        }
    }
}

/// A saved-agent chat on this host whose agent declares check-ins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Declaring {
    pub chat_id: String,
    pub title: String,
    /// The agent's definition ID.
    pub agent: Option<String>,
    pub declared: Declared,
}

/// What [`WakeEngine::adopt`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Adopt {
    /// A chat is already chosen, or no agent here declares check-ins.
    Nothing,
    /// The one declaring chat now checks in.
    Picked(Declaring),
    /// Two or more declaring chats: the owner chooses. Their titles.
    Choose(Vec<String>),
}

/// A check-in to start: the message id (a deterministic UUID, so a retry
/// never queues it twice) and the prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Wake {
    pub id: String,
    pub text: String,
}

/// Status a managed chat had at the last look.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Mark {
    status: SessionStatus,
    completed: Option<String>,
}

/// The chat's own turn while it works: its start and its last output, Unix
/// ms. Not saved: a restart ends the run anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OwnTurn {
    turn: i64,
    last_output: i64,
}

/// A working row's turn start and last output, falling back to what was
/// seen of the same turn before, then to `now`.
fn own_turn(row: &Session, before: Option<OwnTurn>, now: i64) -> OwnTurn {
    let started = row.started_at.map(|t| t.timestamp_millis());
    let output = row.last_output_at.map(|t| t.timestamp_millis());
    let turn = started.or(before.map(|o| o.turn)).unwrap_or(now);
    let before = before.filter(|o| o.turn == turn);
    let last_output = output
        .into_iter()
        .chain(started)
        .chain(before.map(|o| o.last_output))
        .max()
        .unwrap_or(now);
    OwnTurn { turn, last_output }
}

/// One agent's wake engine on this host.
pub(crate) struct WakeEngine {
    /// The agent's definition ID.
    agent: String,
    root: PathBuf,
    state: Mutex<State>,
    /// Not saved: the first look after a start only records a baseline.
    marks: Mutex<HashMap<String, Mark>>,
    /// The agent's own chat was seen working since the open check-in began.
    wake_seen_working: Mutex<bool>,
    /// The chat's own turn, while it works.
    own: Mutex<Option<OwnTurn>>,
    problem: Mutex<Option<String>>,
    /// When each throttled job last ran, Unix ms.
    ran: Mutex<HashMap<&'static str, i64>>,
    /// Working-chat observations and claims, keyed by session.
    claims: Mutex<crate::claim_activity::Claims>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl WakeEngine {
    /// The engine for the agent `agent`, saved under `root`.
    pub fn open(root: &Path, agent: &str) -> Self {
        let state = std::fs::read(root.join("state.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<State>(&b).ok())
            .unwrap_or_default();
        Self {
            agent: agent.to_owned(),
            root: root.to_path_buf(),
            state: Mutex::new(state),
            marks: Mutex::new(HashMap::new()),
            wake_seen_working: Mutex::new(false),
            own: Mutex::new(None),
            problem: Mutex::new(None),
            ran: Mutex::new(HashMap::new()),
            claims: Mutex::new(crate::claim_activity::Claims::default()),
        }
    }

    /// Whether `job` is due again `every` ms after its last run; marks it run.
    pub fn throttle(&self, job: &'static str, every: i64, now: i64) -> bool {
        let mut ran = lock(&self.ran);
        let due = ran.get(job).is_none_or(|last| now - last >= every);
        if due {
            ran.insert(job, now);
        }
        due
    }

    /// The agent's definition ID.
    pub fn agent(&self) -> &str {
        &self.agent
    }

    pub fn state(&self) -> State {
        lock(&self.state).clone()
    }

    pub fn chat(&self) -> Option<String> {
        lock(&self.state).settings.chat_id.clone()
    }

    pub fn problem(&self) -> Option<String> {
        lock(&self.problem).clone()
    }

    pub fn set_problem(&self, problem: Option<String>) {
        *lock(&self.problem) = problem;
    }

    /// Change the state under the lock and save it before anyone acts on it.
    pub fn update<R>(&self, f: impl FnOnce(&mut State) -> R) -> Result<R, EngineError> {
        let mut state = lock(&self.state);
        let mut next = state.clone();
        let out = f(&mut next);
        if next != *state {
            write_atomic(
                &self.root.join("state.json"),
                &serde_json::to_vec_pretty(&next).map_err(|e| EngineError::Other(e.to_string()))?,
            )?;
            *state = next;
        }
        Ok(out)
    }

    /// The owner's settings.
    pub fn set(&self, settings: WakeSettings, now: i64) -> Result<(), EngineError> {
        let settings = WakeSettings {
            every_minutes: settings
                .every_minutes
                .clamp(MIN_EVERY_MINUTES, MAX_EVERY_MINUTES),
            chat_id: settings.chat_id.filter(|c| !c.trim().is_empty()),
            stuck_minutes: settings
                .stuck_minutes
                .clamp(MIN_STUCK_MINUTES, MAX_STUCK_MINUTES),
            stall_minutes: settings
                .stall_minutes
                .clamp(MIN_STALL_MINUTES, MAX_STALL_MINUTES),
            ..settings
        };
        self.update(|s| {
            let schedule_changed = s.settings.chat_id != settings.chat_id
                || s.settings.every_minutes != settings.every_minutes
                || s.settings.paused != settings.paused;
            if s.settings.chat_id != settings.chat_id {
                s.stalls.clear();
                s.turned_on_at = None;
                s.agent = None;
                s.managed.clear();
                s.pending.clear();
                s.open_wake = None;
                s.last_scheduled = now;
            }
            if s.settings.paused && !settings.paused {
                // Resuming starts the clock again instead of firing at once.
                s.last_scheduled = now;
            }
            if schedule_changed {
                s.schedule_dirty = true;
            }
            s.settings = settings.clone();
        })
    }

    /// The agent's grants as the account lists them. Each grant that
    /// appears or goes away is logged.
    pub fn set_grants(&self, grants: Vec<Grant>, now: i64) -> Result<(), EngineError> {
        let (added, removed) = self.update(|s| {
            let added: Vec<Grant> = grants
                .iter()
                .filter(|g| !s.grants.iter().any(|h| h.id == g.id))
                .cloned()
                .collect();
            let removed: Vec<Grant> = s
                .grants
                .iter()
                .filter(|h| !grants.iter().any(|g| g.id == h.id))
                .cloned()
                .collect();
            s.grants = grants;
            (added, removed)
        })?;
        for (granted, grant) in added
            .into_iter()
            .map(|g| (true, g))
            .chain(removed.into_iter().map(|g| (false, g)))
        {
            self.append(&DecisionEntry::Grant {
                at: now,
                granted,
                id: grant.id,
                scopes: grant.scopes,
                over: grant.over,
            })?;
        }
        Ok(())
    }

    /// What the agent's definition declares: the events it wakes on (all
    /// when it lists none) and its coalescing window.
    pub fn declare(
        &self,
        wakes_on: Vec<WakeOn>,
        coalesce_ms: Option<i64>,
    ) -> Result<(), EngineError> {
        self.update(|s| {
            s.wakes_on = wakes_on;
            s.coalesce_ms = coalesce_ms;
        })
    }

    /// Whether the agent wakes on `kind`.
    pub fn wakes_on(&self, kind: WakeOn) -> bool {
        let s = lock(&self.state);
        s.wakes_on.is_empty() || s.wakes_on.contains(&kind)
    }

    /// On by default (#12030): with no chat chosen yet, pick the one chat on
    /// this host of this agent, whose definition declares check-ins, and
    /// turn check-ins on; when it asks for standing authority, the host asks
    /// the account to grant it. Two or more chats of the agent pick none:
    /// the owner chooses. A chat picked once is never picked again, so an
    /// owner who changes the choice keeps it.
    pub fn adopt(&self, declaring: &[Declaring], now: i64) -> Result<Adopt, EngineError> {
        let s = self.state();
        if s.settings.chat_id.is_some() {
            return Ok(Adopt::Nothing);
        }
        let fresh: Vec<&Declaring> = declaring
            .iter()
            .filter(|d| d.declared.check_ins && !s.adopted.contains(&d.chat_id))
            .collect();
        let pick = match fresh.as_slice() {
            [] => return Ok(Adopt::Nothing),
            [one] => (*one).clone(),
            many => {
                return Ok(Adopt::Choose(
                    many.iter().map(|d| d.title.clone()).collect(),
                ));
            }
        };
        self.set(
            WakeSettings {
                chat_id: Some(pick.chat_id.clone()),
                every_minutes: pick
                    .declared
                    .every_minutes
                    .unwrap_or(s.settings.every_minutes),
                paused: false,
                stall_minutes: pick
                    .declared
                    .stall_minutes
                    .unwrap_or(s.settings.stall_minutes),
                ..s.settings
            },
            now,
        )?;
        self.update(|st| {
            st.adopted.insert(pick.chat_id.clone());
            st.turned_on_at = Some(now);
            st.agent = pick.agent.clone();
            st.wants_standing = pick.declared.authority;
        })?;
        Ok(Adopt::Picked(pick))
    }

    /// Whether the agent's chat may use `scope` on the chat `chat` in the
    /// project `project`. Always: agents message, start, steer and stop
    /// chats without per-scope grants (owner, 2026-10-18, #12135).
    pub fn admits(
        &self,
        _scope: Scope,
        _chat: &str,
        _project: Option<&str>,
        _check_in_turn: bool,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Record a chat the agent acted on, so its turns wake the agent later.
    pub fn manage(&self, chat: &str) -> Result<(), EngineError> {
        self.update(|s| {
            if s.settings.chat_id.as_deref() != Some(chat) && s.managed.len() < MAX_MANAGED {
                s.managed.insert(chat.to_owned());
            }
        })
    }

    pub fn is_wake(&self, message_id: &str) -> bool {
        lock(&self.state).wakes.iter().any(|w| w == message_id)
    }

    fn push_events(&self, events: Vec<Event>, now: i64) -> Result<(), EngineError> {
        if events.is_empty() {
            return Ok(());
        }
        self.update(|s| {
            for event in events {
                if !(s.wakes_on.is_empty() || s.wakes_on.contains(&event.kind())) {
                    continue;
                }
                if !s.pending.iter().any(|p| p.event == event) {
                    s.pending.push(Pending { at: now, event });
                }
            }
            let over = s.pending.len().saturating_sub(MAX_EVENTS);
            s.pending.drain(..over);
        })
    }

    /// Look at every device's session rows: managed chats' finished, errored
    /// and waiting turns become events, and the end of the agent's own
    /// check-in turn is logged. `is_child(chat)` is whether the agent's chat
    /// is its parent; `title(chat)` names it.
    pub fn observe(
        &self,
        rows: &[Session],
        is_child: impl Fn(&str) -> bool,
        title: impl Fn(&str) -> String,
        now: i64,
    ) -> Result<(), EngineError> {
        let state = self.state();
        let Some(own_chat) = state.settings.chat_id.clone() else {
            return Ok(());
        };
        let mut events = Vec::new();
        // Managed chats with their last output while they work.
        let mut stalls: Vec<(String, Option<i64>)> = Vec::new();
        let mut marks = lock(&self.marks);
        for row in rows {
            let mark = Mark {
                status: row.status.clone(),
                completed: row.last_completed_turn.clone(),
            };
            if row.chat_id == own_chat {
                let mut own = lock(&self.own);
                if mark.status == SessionStatus::Working {
                    *lock(&self.wake_seen_working) = true;
                    *own = Some(own_turn(row, *own, now));
                } else {
                    *own = None;
                }
                continue;
            }
            if !(state.managed.contains(&row.chat_id) || is_child(&row.chat_id)) {
                continue;
            }
            stalls.push((row.chat_id.clone(), working_since(row)));
            let previous = marks.insert(row.chat_id.clone(), mark.clone());
            let Some(previous) = previous else { continue };
            let name = || title(&row.chat_id);
            if mark.completed.is_some() && mark.completed != previous.completed {
                events.push(Event::ChatFinished {
                    chat: row.chat_id.clone(),
                    title: name(),
                });
            }
            if mark.status != previous.status {
                match mark.status {
                    SessionStatus::Errored => events.push(Event::ChatErrored {
                        chat: row.chat_id.clone(),
                        title: name(),
                    }),
                    SessionStatus::AwaitingInput => events.push(Event::ChatAwaitingInput {
                        chat: row.chat_id.clone(),
                        title: name(),
                    }),
                    _ => {}
                }
            }
        }
        drop(marks);
        let own_row = rows.iter().find(|r| r.chat_id == own_chat);
        if let (Some(wake), Some(row)) = (state.open_wake.clone(), own_row) {
            let ended = matches!(row.status, SessionStatus::Idle | SessionStatus::Errored);
            let started = *lock(&self.wake_seen_working)
                || now.saturating_sub(state.last_check_in) > 2 * 60_000;
            if ended && started {
                self.update(|s| s.open_wake = None)?;
                *lock(&self.wake_seen_working) = false;
                self.append(&DecisionEntry::Outcome {
                    at: now,
                    id: wake,
                    status: if row.status == SessionStatus::Errored {
                        "errored".into()
                    } else {
                        "finished".into()
                    },
                })?;
            }
        }
        let window = i64::from(state.settings.stall_minutes) * 60_000;
        let raised = self.update(|s| {
            let mut raised = Vec::new();
            for (chat, last) in &stalls {
                let key = format!("chat:{chat}");
                match last {
                    Some(last) if now - last >= window => {
                        let fresh = s.stalls.get(&key).is_none_or(|k| k.since != *last);
                        if fresh {
                            let name = title(chat);
                            s.stalls.insert(
                                key,
                                Stall {
                                    subject: format!("Chat \"{name}\" ({})", short(chat)),
                                    since: *last,
                                    rung: 0,
                                    told: false,
                                },
                            );
                            raised.push(Event::ChatStalled {
                                chat: chat.clone(),
                                title: name,
                                since: *last,
                            });
                        }
                    }
                    _ => {
                        s.stalls.remove(&key);
                    }
                }
            }
            raised
        })?;
        events.extend(raised);
        self.push_events(events, now)
    }

    /// The watchdog over the chat's own turn (#12029). When the chat is busy
    /// and its turn has gone the stuck window with no output, record the
    /// stop once for that turn, log the stopped turn as `stuck`, and return
    /// it: the caller interrupts the turn, and `due` starts the check-in that
    /// says why once the chat is idle. A turn that keeps producing output is
    /// never stopped. `tool` names the call the turn waits on.
    pub fn watchdog(
        &self,
        busy: bool,
        now: i64,
        tool: impl FnOnce() -> Option<String>,
    ) -> Result<Option<Stuck>, EngineError> {
        let Some(own) = *lock(&self.own) else {
            return Ok(None);
        };
        let s = self.state();
        if !busy || s.settings.chat_id.is_none() || s.settings.paused {
            return Ok(None);
        }
        let window = i64::from(s.settings.stuck_minutes) * 60_000;
        if now - own.last_output < window || s.stuck.as_ref().is_some_and(|k| k.turn == own.turn) {
            return Ok(None);
        }
        let stuck = Stuck {
            turn: own.turn,
            since: own.last_output,
            at: now,
            tool: tool().map(|t| bounded(&t, 200)),
            checked_in: false,
        };
        let stopped = self.update(|st| {
            st.stuck = Some(stuck.clone());
            st.open_wake.take()
        })?;
        *lock(&self.wake_seen_working) = false;
        self.append(&DecisionEntry::Outcome {
            at: now,
            id: stopped.unwrap_or_else(|| format!("turn:{}", own.turn)),
            status: "stuck".into(),
        })?;
        Ok(Some(stuck))
    }

    /// When the next scheduled check-in is due.
    pub fn next_scheduled(&self) -> Option<i64> {
        let s = lock(&self.state);
        (s.settings.chat_id.is_some() && !s.settings.paused)
            .then(|| s.last_scheduled + i64::from(s.settings.every_minutes) * 60_000)
    }

    /// The check-in to start now, if any. `busy` is whether the agent's chat
    /// has a turn in flight; nothing starts over it. Saves the check-in and
    /// logs it before returning, so the caller only has to queue it.
    pub fn due(&self, busy: bool, now: i64) -> Result<Option<Wake>, EngineError> {
        if busy {
            return Ok(None);
        }
        let s = self.state();
        let Some(chat) = s.settings.chat_id.clone() else {
            return Ok(None);
        };
        if s.settings.paused {
            return Ok(None);
        }
        let stuck = s.stuck.clone().filter(|k| !k.checked_in);
        let scheduled = self.next_scheduled().is_some_and(|at| now >= at);
        let events_ready = !s.pending.is_empty() && {
            let first = s.pending.iter().map(|p| p.at).min().unwrap_or(now);
            let last = s.pending.iter().map(|p| p.at).max().unwrap_or(now);
            (now - last >= s.coalesce_ms.unwrap_or(QUIET_MS) || now - first >= MAX_WAIT_MS)
                && now - s.last_check_in >= MIN_GAP_MS
        };
        let window = s.settings.stall_minutes;
        let rung_due = s.stalls.values().any(|k| k.rung_at(now, window) > k.rung);
        if stuck.is_none() && !scheduled && !events_ready && !rung_due {
            return Ok(None);
        }
        let reason = if stuck.is_some() {
            WakeReason::Stuck
        } else if scheduled {
            WakeReason::Schedule
        } else {
            WakeReason::Events
        };
        let events: Vec<String> = s.pending.iter().map(|p| p.event.line()).collect();
        let slot = match reason {
            WakeReason::Schedule => format!("schedule:{}", s.last_scheduled),
            WakeReason::Events => match s.pending.first() {
                Some(p) => format!("events:{}", p.at),
                None => format!("stall:{now}"),
            },
            WakeReason::Stuck => format!("stuck:{}", stuck.as_ref().map_or(0, |k| k.turn)),
        };
        let id = uuid_from(&format!("wake:{}:{chat}:{slot}", self.agent));
        let recent = self.read(PROMPT_ENTRIES);
        let lead = stuck.as_ref().map(Stuck::line);
        let mut open: Vec<&Stall> = s.stalls.values().collect();
        open.sort_by_key(|k| k.since);
        let stalls: Vec<String> = open
            .iter()
            .map(|k| k.line(now, s.settings.stall_minutes))
            .collect();
        let mut text = prompt(
            reason,
            now,
            lead.as_deref(),
            &events,
            &stalls,
            &s.grants,
            &recent,
        );
        text.push_str(
            &lock(&self.claims).summary(now, i64::from(s.settings.stall_minutes) * 60_000),
        );
        self.update(|st| {
            st.pending.clear();
            st.last_check_in = now;
            if let Some(k) = st.stuck.as_mut() {
                k.checked_in = true;
            }
            for k in st.stalls.values_mut() {
                k.rung = k.rung_at(now, window);
            }
            if scheduled {
                st.last_scheduled = now;
            }
            if !st.wakes.contains(&id) {
                st.wakes.push_back(id.clone());
            }
            while st.wakes.len() > MAX_WAKES {
                st.wakes.pop_front();
            }
            st.open_wake = Some(id.clone());
        })?;
        *lock(&self.wake_seen_working) = false;
        self.append(&DecisionEntry::Wake {
            at: now,
            id: id.clone(),
            reason,
            events,
        })?;
        Ok(Some(Wake { id, text }))
    }

    /// GitHub's issue list, newest updates first, into opened and closed
    /// events. The first look only records what's there.
    pub fn observe_issues(&self, issues: &[GhIssue], now: i64) -> Result<(), EngineError> {
        let events = self.update(|s| {
            let first = s.issues.is_empty();
            let highest = s.issues.keys().next_back().copied().unwrap_or(0);
            let mut events = Vec::new();
            for issue in issues {
                let open = issue.state.eq_ignore_ascii_case("open");
                if !open {
                    // A closed issue's claim is no stall, whatever the claims
                    // search (which can lag) still says.
                    s.stalls.remove(&format!("claim:{}", issue.number));
                    s.pending.retain(|p| {
                        !matches!(
                            &p.event,
                            Event::ClaimNoProgress { number, .. } | Event::StaleClaim { number, .. }
                                if *number == issue.number
                        )
                    });
                }
                match s.issues.insert(issue.number, open) {
                    None if !first && open && issue.number > highest => {
                        events.push(Event::IssueOpened {
                            number: issue.number,
                            title: issue.title.clone(),
                        });
                    }
                    Some(true) if !open => events.push(Event::IssueClosed {
                        number: issue.number,
                        title: issue.title.clone(),
                    }),
                    Some(false) if open => events.push(Event::IssueOpened {
                        number: issue.number,
                        title: issue.title.clone(),
                    }),
                    _ => {}
                }
            }
            while s.issues.len() > MAX_ISSUES {
                s.issues.pop_first();
            }
            s.last_github = now;
            events
        })?;
        {
            let mut claims = lock(&self.claims);
            for issue in issues
                .iter()
                .filter(|issue| !issue.state.eq_ignore_ascii_case("open"))
            {
                claims.remove(issue.number);
            }
        }
        self.push_events(events, now)
    }

    /// Open issues with their latest comments, commits and linked branches,
    /// and what this computer sees of each claim, into events: a claim with
    /// no progress for the stall window (#12032), and a claim with no chat at
    /// all and no newer comment for six hours. Each is reported once.
    pub fn observe_claims(
        &self,
        issues: &[GhClaimIssue],
        local: &LocalProgress,
        now: i64,
    ) -> Result<(), EngineError> {
        let state = self.state();
        let window = i64::from(state.settings.stall_minutes) * 60_000;
        let observed: Vec<_> = issues
            .iter()
            .filter(|issue| state.issues.get(&issue.number) != Some(&false))
            .filter_map(|issue| {
                let claim = active_claim(issue)?;
                let progress = claim_progress(issue, &claim)
                    .into_iter()
                    .chain(local.chats.get(&claim.session).copied())
                    .chain(local.runs.get(&claim.session).copied())
                    .chain(local.titled.get(&issue.number).copied())
                    .chain(local.builds.get(&issue.number).copied())
                    .max()
                    .unwrap_or(claim.at)
                    .max(claim.at);
                Some(crate::claim_activity::Claim {
                    number: issue.number,
                    holder: claim.session,
                    claimed_at: claim.at,
                    last_progress_at: progress,
                })
            })
            .collect();
        let mut claims = lock(&self.claims);
        let progress: BTreeMap<_, _> = observed
            .iter()
            .map(|c| {
                (
                    c.number,
                    c.last_progress_at
                        .max(claims.progress(&c.holder).unwrap_or(0)),
                )
            })
            .collect();
        claims.replace(observed);
        drop(claims);
        let events = self.update(|s| {
            let mut events = Vec::new();
            let mut claimed = BTreeSet::new();
            for issue in issues {
                if s.issues.get(&issue.number) == Some(&false) {
                    continue;
                }
                let Some(claim) = active_claim(issue) else {
                    continue;
                };
                claimed.insert(format!("claim:{}", issue.number));
                let chat = local.chats.get(&claim.session).copied();
                let run = local.runs.get(&claim.session).copied();
                let titled = local.titled.get(&issue.number).copied();
                let last = progress.get(&issue.number).copied().unwrap_or(claim.at);
                let key = format!("claim:{}", issue.number);
                if now - last >= window {
                    if s.stalls.get(&key).is_none_or(|k| k.since != last) {
                        s.stalls.insert(
                            key,
                            Stall {
                                subject: format!(
                                    "The claim on #{} ({}) by {}",
                                    issue.number, issue.title, claim.session
                                ),
                                since: last,
                                rung: 0,
                                told: false,
                            },
                        );
                        events.push(Event::ClaimNoProgress {
                            number: issue.number,
                            title: issue.title.clone(),
                            session: claim.session.clone(),
                            since: last,
                        });
                    }
                } else {
                    s.stalls.remove(&key);
                }
                if chat.is_some() || run.is_some() || titled.is_some() {
                    continue;
                }
                let Some((session, since)) = stale_claim(issue, now) else {
                    continue;
                };
                if s.stale.insert(format!("{}:{since}", issue.number)) {
                    events.push(Event::StaleClaim {
                        number: issue.number,
                        title: issue.title.clone(),
                        session,
                        since,
                    });
                }
            }
            while s.stale.len() > MAX_ISSUES {
                s.stale.pop_first();
            }
            // A claim that was released, or whose issue closed, is no stall.
            s.stalls
                .retain(|key, _| !key.starts_with("claim:") || claimed.contains(key));
            events
        })?;
        self.push_events(events, now)
    }

    pub fn observe_claim_chats(
        &self,
        chats: &[openagents_proto::Chat],
        rows: &[Session],
        now: i64,
    ) {
        lock(&self.claims).observe_chats(chats, rows, now);
    }

    /// Stalls that reached the last rung of the ladder and the owner hasn't
    /// been told about, each returned once (#12032). The caller pushes them
    /// to the owner's phone.
    pub fn owner_notices(&self, now: i64) -> Result<Vec<OwnerNotice>, EngineError> {
        self.update(|s| {
            let window = s.settings.stall_minutes;
            let mut notices = Vec::new();
            for (key, stall) in s.stalls.iter_mut() {
                if stall.told || stall.rung_at(now, window) < 3 {
                    continue;
                }
                stall.told = true;
                let minutes = (now - stall.since).max(0) / 60_000;
                notices.push(OwnerNotice {
                    request_id: hex64(&format!("stall:{key}:{}", stall.since)),
                    title: "Work has stalled".into(),
                    body: format!(
                        "{}: no progress for {minutes} minutes, after a message and a restart.",
                        stall.subject
                    ),
                });
            }
            notices
        })
    }

    /// Whether the agent holds `log.write`, or kept a log here before the
    /// grant existed and the account hasn't taken that grant yet (#12045).
    pub fn may_log(&self) -> bool {
        let state = self.state();
        state
            .grants
            .iter()
            .any(|g| g.scopes.contains(&Scope::LogWrite))
            || (!state.log_grant_moved && self.kept_a_log())
    }

    /// Whether the agent wrote any decision of its own here.
    pub fn kept_a_log(&self) -> bool {
        let Ok(file) = std::fs::File::open(self.log_path()) else {
            return false;
        };
        std::io::BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str::<DecisionEntry>(&line).ok())
            .any(|entry| matches!(entry, DecisionEntry::Decision { .. }))
    }

    /// The agent's own record of a check-in, with the receipts of the
    /// judgments it used.
    pub fn record(
        &self,
        saw: &str,
        decided: &str,
        did: &str,
        judgments: Vec<JudgmentReceipt>,
        now: i64,
    ) -> Result<(), EngineError> {
        let field = |text: &str| bounded(text.trim(), MAX_FIELD);
        if saw.trim().is_empty() && decided.trim().is_empty() && did.trim().is_empty() {
            return Err(EngineError::Other(
                "Say what you saw, decided or did.".into(),
            ));
        }
        self.append(&DecisionEntry::Decision {
            at: now,
            saw: field(saw),
            decided: field(decided),
            did: field(did),
            judgments: judgments
                .into_iter()
                .take(MAX_JUDGMENTS)
                .map(|mut judgment| {
                    judgment.answers = bounded(judgment.answers.trim(), MAX_FIELD);
                    judgment
                })
                .collect(),
        })
    }

    fn log_path(&self) -> PathBuf {
        self.root.join("decisions.jsonl")
    }

    /// Append one line and flush it to disk; lines are never rewritten.
    pub fn append(&self, entry: &DecisionEntry) -> Result<(), EngineError> {
        std::fs::create_dir_all(&self.root)?;
        let mut line = serde_json::to_vec(entry).map_err(|e| EngineError::Other(e.to_string()))?;
        line.push(b'\n');
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())?;
        file.write_all(&line)?;
        file.sync_data()?;
        Ok(())
    }

    fn mirror_path(&self) -> PathBuf {
        self.root.join("decisions.mirror.json")
    }

    /// Every readable line of the log as the account keeps it, with the
    /// request ID that makes sending it again a no-op: the same entry gets
    /// the same ID on every host (#12046).
    fn mirror_lines(&self) -> Vec<(String, openagents_sdk::agents::DecisionEntry)> {
        let Ok(file) = std::fs::File::open(self.log_path()) else {
            return Vec::new();
        };
        std::io::BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str(&line).ok())
            .map(|entry: openagents_sdk::agents::DecisionEntry| {
                let canonical = serde_json::to_string(&entry).unwrap_or_default();
                (hex64(&format!("decision:{}:{canonical}", self.agent)), entry)
            })
            .collect()
    }

    /// Copy this log to the account and the account's entries it lacks back
    /// into the file, which stays the cache. A host that starts with an empty
    /// file takes the account's whole log. Both directions are safe to repeat.
    pub async fn mirror_decisions(
        &self,
        agents: &openagents_sdk::agents::Agents<'_>,
        cancel: &openagents_sdk::CancellationToken,
    ) -> Result<(), EngineError> {
        #[derive(Default, Serialize, Deserialize)]
        struct Mirror {
            /// Local readable lines already on the account.
            pushed: usize,
            /// The account cursor read up to.
            pulled: Option<String>,
        }
        let mut mirror: Mirror = std::fs::read(self.mirror_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let service = EngineError::AgentService;
        // Pull first, so entries from other hosts reach the file.
        let mut have: std::collections::HashSet<String> =
            self.mirror_lines().into_iter().map(|(id, _)| id).collect();
        loop {
            let page = agents
                .decisions(&self.agent, mirror.pulled.as_deref(), cancel)
                .await
                .map_err(service)?;
            for stored in &page.data {
                let Ok(entry) = serde_json::from_value::<DecisionEntry>(
                    serde_json::to_value(&stored.entry).unwrap_or_default(),
                ) else {
                    continue;
                };
                if have.insert(stored.request_id.clone()) {
                    self.append(&entry)?;
                }
            }
            if let Some(last) = page.data.last() {
                mirror.pulled = Some(last.cursor.clone());
            }
            if page.next.is_none() {
                break;
            }
        }
        // Then push what the account hasn't seen.
        let lines = self.mirror_lines();
        let start = mirror.pushed.min(lines.len());
        let mut pushed = start;
        for (id, entry) in &lines[start..] {
            agents
                .record_decision(&self.agent, id, entry, cancel)
                .await
                .map_err(service)?;
            pushed += 1;
        }
        mirror.pushed = pushed;
        let bytes = serde_json::to_vec(&mirror).map_err(|e| EngineError::Other(e.to_string()))?;
        std::fs::create_dir_all(&self.root)?;
        std::fs::write(self.mirror_path(), bytes)?;
        Ok(())
    }

    /// The latest `limit` entries, oldest first. Unreadable lines are skipped.
    pub fn read(&self, limit: usize) -> Vec<DecisionEntry> {
        let Ok(file) = std::fs::File::open(self.log_path()) else {
            return Vec::new();
        };
        let mut out = VecDeque::with_capacity(limit + 1);
        for line in std::io::BufReader::new(file).lines() {
            let Ok(line) = line else { break };
            if let Ok(entry) = serde_json::from_str::<DecisionEntry>(&line) {
                out.push_back(entry);
                if out.len() > limit {
                    out.pop_front();
                }
            }
        }
        out.into()
    }
}

/// The check-in prompt. Plain words: it shows in the agent's chat.
pub(crate) fn prompt(
    reason: WakeReason,
    now: i64,
    lead: Option<&str>,
    events: &[String],
    stalls: &[String],
    grants: &[Grant],
    recent: &[DecisionEntry],
) -> String {
    let mut text = format!(
        "Check-in, {} ({}).\n\n",
        clock(now),
        match reason {
            WakeReason::Schedule => "scheduled",
            WakeReason::Events => "something changed",
            WakeReason::Stuck => "your turn was stuck",
        }
    );
    if let Some(lead) = lead {
        text.push_str(lead);
        text.push_str(
            " Put a timeout on every probe (`timeout 60 CMD`, `curl -m 20`) and carry on.\n\n",
        );
    }
    if events.is_empty() {
        text.push_str("Nothing new was reported since the last check-in.\n");
    } else {
        text.push_str("Since the last check-in:\n");
        for event in events {
            text.push_str(&format!("- {event}\n"));
        }
    }
    if !stalls.is_empty() {
        text.push_str(
            "\nStalled work, oldest first. Act on the rung that is due, not an offer, and record it with record_decision:\n",
        );
        for stall in stalls {
            text.push_str(&format!("- {stall}\n"));
        }
    }
    text.push('\n');
    text.push_str(
        "You may message, start, steer and stop chats on the owner's computers without asking. Use it for routine coordination.\n",
    );
    text.push_str("Money, keys and secrets still need the owner's approval.\n");
    let decisions: Vec<&DecisionEntry> = recent
        .iter()
        .filter(|e| matches!(e, DecisionEntry::Decision { .. }))
        .collect();
    if !decisions.is_empty() {
        text.push_str("\nYour recent decisions, oldest first:\n");
        for entry in decisions {
            if let DecisionEntry::Decision {
                at,
                saw,
                decided,
                did,
                judgments,
            } = entry
            {
                let judged: Vec<&str> = judgments.iter().map(|j| j.answers.as_str()).collect();
                let judged = if judged.is_empty() {
                    String::new()
                } else {
                    format!(" (judged: {})", judged.join("; "))
                };
                text.push_str(&format!(
                    "- {}: saw {saw}; decided {decided}; did {did}{judged}\n",
                    clock(*at)
                ));
            }
        }
    }
    text.push_str(
        "\nGather fresh facts and act on what needs it. Then, even when nothing changed, post a short status of 3 to 6 lines in this chat: what changed, what you did, and what's next. Finally call record_decision once with what you saw, what you decided and what you did, and the receipts of any judge calls you used.",
    );
    text
}

/// `gh issue list --json number,title,state`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GhIssue {
    pub number: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub state: String,
}

/// One open issue from the claims search, with its latest comments.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GhClaimIssue {
    pub number: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub comments: GhComments,
    /// Commits that name the issue, as GitHub records them when pushed.
    #[serde(default)]
    pub timeline_items: GhNodes<GhReference>,
    /// Branches linked to the issue, with their head commit.
    #[serde(default)]
    pub linked_branches: GhNodes<GhLinkedBranch>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct GhComments {
    #[serde(default)]
    pub nodes: Vec<GhComment>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub(crate) struct GhNodes<T> {
    #[serde(default)]
    pub nodes: Vec<Option<T>>,
}

impl<T> Default for GhNodes<T> {
    fn default() -> Self {
        Self { nodes: Vec::new() }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GhReference {
    #[serde(default)]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GhLinkedBranch {
    #[serde(default, rename = "ref")]
    pub reference: Option<GhRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GhRef {
    #[serde(default)]
    pub target: Option<GhCommit>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GhCommit {
    #[serde(default)]
    pub committed_date: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GhComment {
    #[serde(default)]
    pub body: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// The GraphQL search for open issues, their claims, commits and branches.
pub(crate) const CLAIMS_QUERY: &str = "{search(query:\"repo:OpenAgentsInc/openagents is:issue is:open\",type:ISSUE,first:100){nodes{... on Issue{number title comments(last:20){nodes{body createdAt}} timelineItems(last:10,itemTypes:[REFERENCED_EVENT]){nodes{... on ReferencedEvent{createdAt}}} linkedBranches(first:5){nodes{ref{target{... on Commit{committedDate}}}}}}}}}";

/// `gh api graphql` answer for [`CLAIMS_QUERY`].
pub(crate) fn claim_issues(answer: &serde_json::Value) -> Vec<GhClaimIssue> {
    answer["data"]["search"]["nodes"]
        .as_array()
        .map(|nodes| {
            nodes
                .iter()
                .filter_map(|n| serde_json::from_value(n.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Issue numbers a chat title names as `#N` (4 to 6 digits).
pub(crate) fn issue_refs(title: &str) -> Vec<u64> {
    title
        .split('#')
        .skip(1)
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            (4..=6)
                .contains(&digits.len())
                .then(|| digits.parse().ok())
                .flatten()
        })
        .collect()
}

/// An issue's latest unreleased claim.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Claim {
    /// The comment's index among the issue's latest comments.
    index: usize,
    session: String,
    at: i64,
}

fn active_claim(issue: &GhClaimIssue) -> Option<Claim> {
    let comments = &issue.comments.nodes;
    let index = comments.iter().rposition(|c| c.body.contains(CLAIM_MARK))?;
    if comments[index..]
        .iter()
        .any(|c| c.body.contains(RELEASE_MARK))
    {
        return None;
    }
    let claim = &comments[index];
    let fields = claim.body.split_once(CLAIM_MARK)?.1;
    let fields = fields.split_once("-->").map_or(fields, |(f, _)| f);
    let session = fields
        .split_whitespace()
        .find_map(|w| w.strip_prefix("session="))
        .unwrap_or("an agent")
        .to_owned();
    Some(Claim {
        index,
        session,
        at: claim.created_at.timestamp_millis(),
    })
}

/// The latest progress GitHub shows on a claimed issue: a comment after the
/// claim, a commit naming it, or its linked branch's head commit.
fn claim_progress(issue: &GhClaimIssue, claim: &Claim) -> Option<i64> {
    let comments = issue.comments.nodes[claim.index + 1..]
        .iter()
        .map(|c| c.created_at.timestamp_millis());
    let commits = issue
        .timeline_items
        .nodes
        .iter()
        .flatten()
        .filter_map(|r| r.created_at)
        .map(|t| t.timestamp_millis());
    let branches = issue
        .linked_branches
        .nodes
        .iter()
        .flatten()
        .filter_map(|b| b.reference.as_ref()?.target.as_ref()?.committed_date)
        .map(|t| t.timestamp_millis());
    comments
        .chain(commits)
        .chain(branches)
        .filter(|t| *t > claim.at)
        .max()
}

/// A managed chat's last output while it works: its last output, else the
/// start of its turn. `None` when it isn't working or the row can't say.
fn working_since(row: &Session) -> Option<i64> {
    if row.status != SessionStatus::Working {
        return None;
    }
    row.last_output_at
        .into_iter()
        .chain(row.started_at)
        .map(|t| t.timestamp_millis())
        .max()
}

/// The issue's latest claim, when it is unreleased, has no newer comment,
/// and is older than [`STALE_CLAIM_MS`]: its session and time.
fn stale_claim(issue: &GhClaimIssue, now: i64) -> Option<(String, i64)> {
    let comments = &issue.comments.nodes;
    let index = comments.iter().rposition(|c| c.body.contains(CLAIM_MARK))?;
    if index + 1 != comments.len() {
        return None;
    }
    let claim = &comments[index];
    if claim.body.contains(RELEASE_MARK) {
        return None;
    }
    let since = claim.created_at.timestamp_millis();
    if now - since < STALE_CLAIM_MS {
        return None;
    }
    let fields = claim.body.split_once(CLAIM_MARK)?.1;
    let fields = fields.split_once("-->").map_or(fields, |(f, _)| f);
    let session = fields
        .split_whitespace()
        .find_map(|w| w.strip_prefix("session="))
        .unwrap_or("an agent")
        .to_owned();
    Some((session, since))
}

/// A tool call in plain words for the stuck check-in: the tool and, for a
/// shell command, URL or search, its start, like ``Bash` (`curl …`)`.
pub(crate) fn tool_label(call: &ToolCall) -> String {
    let (name, detail): (&str, Option<&str>) = match call {
        ToolCall::Exec { command } => ("Bash", Some(command)),
        ToolCall::ReadFile { path } => ("Read", Some(path)),
        ToolCall::WriteFile { path, .. } => ("Write", Some(path)),
        ToolCall::EditFile { path, .. } => ("Edit", Some(path)),
        ToolCall::ApplyPatch { path } => ("ApplyPatch", path.as_deref()),
        ToolCall::Search { pattern, .. } => ("Grep", Some(pattern)),
        ToolCall::Glob { pattern } => ("Glob", Some(pattern)),
        ToolCall::WebFetch { url, .. } => ("WebFetch", Some(url)),
        ToolCall::WebSearch { query } => ("WebSearch", Some(query)),
        ToolCall::Todo { .. } => ("TodoWrite", None),
        ToolCall::Mcp { tool, .. } => (tool, None),
        ToolCall::Unknown { name, .. } => (name, None),
    };
    let name = name.replace('`', "'");
    match detail.map(|d| d.lines().next().unwrap_or("").trim().replace('`', "'")) {
        Some(d) if !d.is_empty() => {
            let cut: String = d.chars().take(60).collect();
            let more = if cut.len() < d.len() { " …" } else { "" };
            format!("`{name}` (`{cut}{more}`)")
        }
        _ => format!("`{name}`"),
    }
}

/// 64 lowercase hex characters derived from `seed`.
fn hex64(seed: &str) -> String {
    Sha256::digest(seed.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn uuid_from(seed: &str) -> String {
    let hash = Sha256::digest(seed.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    uuid::Builder::from_sha1_bytes(bytes)
        .into_uuid()
        .to_string()
}

fn short(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

fn clock(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_default()
}

fn bounded(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), EngineError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Every agent's wake engine on this host, each saved under
/// `ROOT/<definition ID>/`.
pub(crate) struct AgentWakes {
    root: PathBuf,
    engines: Mutex<BTreeMap<String, Arc<WakeEngine>>>,
}

/// Whether `id` is an agent definition ID (64 lowercase hex characters).
pub(crate) fn valid_agent(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl AgentWakes {
    /// Open every engine saved under `root`.
    pub fn open(root: &Path) -> Self {
        let mut engines = BTreeMap::new();
        if let Ok(dirs) = std::fs::read_dir(root) {
            for dir in dirs.flatten() {
                let name = dir.file_name().to_string_lossy().into_owned();
                if valid_agent(&name) && dir.path().join("state.json").is_file() {
                    engines.insert(name.clone(), Arc::new(WakeEngine::open(&dir.path(), &name)));
                }
            }
        }
        Self {
            root: root.to_path_buf(),
            engines: Mutex::new(engines),
        }
    }

    /// The engine for `agent`, if it has one.
    pub fn engine(&self, agent: &str) -> Option<Arc<WakeEngine>> {
        lock(&self.engines).get(agent).cloned()
    }

    /// The engine for `agent`, made on first use.
    pub fn ensure(&self, agent: &str) -> Result<Arc<WakeEngine>, EngineError> {
        if !valid_agent(agent) {
            return Err(EngineError::Other("Choose a saved agent.".into()));
        }
        let mut engines = lock(&self.engines);
        Ok(engines
            .entry(agent.to_owned())
            .or_insert_with(|| Arc::new(WakeEngine::open(&self.root.join(agent), agent)))
            .clone())
    }

    pub fn all(&self) -> Vec<Arc<WakeEngine>> {
        lock(&self.engines).values().cloned().collect()
    }

    /// The engine whose agent runs in the chat `chat`.
    pub fn by_chat(&self, chat: &str) -> Option<Arc<WakeEngine>> {
        lock(&self.engines)
            .values()
            .find(|e| e.chat().as_deref() == Some(chat))
            .cloned()
    }

    /// Move one agent's state saved before #12044 (a single `foreman/`
    /// folder with `state.json` and `decisions.jsonl`) under `agent`, once,
    /// keeping its decision log whole. The old folder is renamed, not
    /// deleted. Returns whether anything moved.
    pub fn migrate(&self, legacy: &Path, agent: &str) -> Result<bool, EngineError> {
        if !legacy.join("state.json").is_file() || !valid_agent(agent) {
            return Ok(false);
        }
        let to = self.root.join(agent);
        if to.join("state.json").is_file() {
            return Ok(false);
        }
        std::fs::create_dir_all(&to)?;
        for file in ["decisions.jsonl", "working.json", "state.json"] {
            let from = legacy.join(file);
            if from.is_file() {
                std::fs::copy(&from, to.join(file))?;
            }
        }
        let mut done = legacy.as_os_str().to_owned();
        done.push(".migrated");
        std::fs::rename(legacy, PathBuf::from(done))?;
        lock(&self.engines).insert(agent.to_owned(), Arc::new(WakeEngine::open(&to, agent)));
        Ok(true)
    }

    /// The chat a pre-#12044 state saved in `legacy` ran in.
    pub fn legacy_chat(legacy: &Path) -> Option<String> {
        let state: State =
            serde_json::from_slice(&std::fs::read(legacy.join("state.json")).ok()?).ok()?;
        state.settings.chat_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;
    const AGENT: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";

    fn standing(id: &str, at: i64) -> Grant {
        Grant {
            id: id.into(),
            scopes: Scope::CHAT.to_vec(),
            over: openagents_proto::agent_wakes::Over::Account,
            granted_at: at,
        }
    }

    fn row(chat: &str, status: SessionStatus, completed: Option<&str>) -> Session {
        Session {
            last_completed_turn: completed.map(str::to_owned),
            chat_id: chat.into(),
            device_id: "dev".into(),
            status,
            started_at: None,
            updated_at: chrono::Utc::now(),
            running_subagents: 0,
            background_tasks: 0,
            last_output_at: None,
        }
    }

    fn configured(dir: &Path) -> WakeEngine {
        let f = WakeEngine::open(dir, AGENT);
        f.set(
            WakeSettings {
                chat_id: Some("own".into()),
                every_minutes: 20,
                paused: false,
                ..WakeSettings::default()
            },
            0,
        )
        .unwrap();
        f
    }

    #[test]
    fn a_schedule_tick_starts_one_check_in_and_logs_it() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        assert_eq!(f.due(false, 19 * MIN).unwrap(), None);
        assert_eq!(f.due(true, 20 * MIN).unwrap(), None, "never over a turn");
        let wake = f.due(false, 20 * MIN).unwrap().expect("due");
        assert!(wake.text.contains("scheduled"));
        assert!(wake.text.contains("record_decision"));
        assert!(
            wake.text
                .contains("even when nothing changed, post a short status of 3 to 6 lines")
        );
        assert!(
            wake.text
                .contains("You may message, start, steer and stop chats")
        );
        assert_eq!(f.due(false, 21 * MIN).unwrap(), None, "one per slot");
        assert!(f.is_wake(&wake.id));
        let log = f.read(10);
        assert!(matches!(
            &log[0],
            DecisionEntry::Wake { reason: WakeReason::Schedule, id, .. } if *id == wake.id
        ));
    }

    #[test]
    fn a_burst_of_chat_events_coalesces_into_one_check_in() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        f.manage("a").unwrap();
        let child = |c: &str| c == "b";
        let title = |c: &str| format!("Chat {c}");
        let base = [
            row("a", SessionStatus::Working, None),
            row("b", SessionStatus::Working, None),
            row("x", SessionStatus::Working, None),
        ];
        f.observe(&base, child, title, 4 * MIN).unwrap();
        let after = [
            row("a", SessionStatus::Idle, Some("t1")),
            row("b", SessionStatus::AwaitingInput, None),
            row("x", SessionStatus::Errored, None),
        ];
        f.observe(&after, child, title, 4 * MIN).unwrap();
        // Repeating the same rows adds nothing.
        f.observe(&after, child, title, 4 * MIN + 1000).unwrap();
        assert_eq!(
            f.due(false, 4 * MIN + 30_000).unwrap(),
            None,
            "waits for quiet"
        );
        let wake = f.due(false, 5 * MIN + 1000).unwrap().expect("one check-in");
        assert!(wake.text.contains("Chat \"Chat a\" (a) finished its turn."));
        assert!(
            wake.text
                .contains("Chat \"Chat b\" (b) is waiting for an answer.")
        );
        assert!(!wake.text.contains("Chat x"), "unmanaged chats stay quiet");
        assert_eq!(f.due(false, 6 * MIN).unwrap(), None);
        match &f.read(10)[..] {
            [DecisionEntry::Wake { reason, events, .. }] => {
                assert_eq!(*reason, WakeReason::Events);
                assert_eq!(events.len(), 2);
            }
            other => panic!("{other:?}"),
        }
        // The check-in turn ending is logged once.
        f.observe(
            &[row("own", SessionStatus::Working, None)],
            child,
            title,
            6 * MIN,
        )
        .unwrap();
        f.observe(
            &[row("own", SessionStatus::Idle, Some("w"))],
            child,
            title,
            7 * MIN,
        )
        .unwrap();
        f.observe(
            &[row("own", SessionStatus::Idle, Some("w"))],
            child,
            title,
            8 * MIN,
        )
        .unwrap();
        let outcomes = f
            .read(10)
            .into_iter()
            .filter(|e| matches!(e, DecisionEntry::Outcome { status, .. } if status == "finished"))
            .count();
        assert_eq!(outcomes, 1);
    }

    #[test]
    fn grants_are_logged_but_never_gate_an_agent() {
        use openagents_proto::agent_wakes::Over;
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        assert!(
            f.admits(Scope::ChatMessage, "w", None, false).is_ok(),
            "owner turns unchanged"
        );
        // No grant is needed (#12135): grants are still recorded and logged.
        assert!(f.admits(Scope::ChatMessage, "w", None, true).is_ok());
        let project = Grant {
            id: "g1".into(),
            scopes: vec![Scope::ChatMessage, Scope::ChatSteer],
            over: Over::Project("p".into()),
            granted_at: 4,
        };
        f.set_grants(vec![project.clone()], 5).unwrap();
        assert!(f.admits(Scope::ChatSteer, "w", Some("p"), true).is_ok());
        assert!(f.admits(Scope::ChatSteer, "w", Some("q"), true).is_ok());
        assert!(f.admits(Scope::ChatStop, "w", Some("p"), true).is_ok());
        let text = prompt(
            WakeReason::Schedule,
            6,
            None,
            &[],
            &[],
            &f.state().grants,
            &[],
        );
        assert!(
            text.contains("You may message, start, steer and stop chats"),
            "{text}"
        );
        // Standing authority is every chat scope over the account.
        f.set_grants(vec![project, standing("g2", 6)], 7).unwrap();
        assert!(f.admits(Scope::ChatStop, "z", None, true).is_ok());
        let text = prompt(
            WakeReason::Schedule,
            8,
            None,
            &[],
            &[],
            &f.state().grants,
            &[],
        );
        assert!(text.contains("You may message, start, steer and stop chats"));
        f.set_grants(vec![], 9).unwrap();
        assert!(f.admits(Scope::ChatStart, "z", None, true).is_ok());
        let log: Vec<(bool, String)> = f
            .read(10)
            .into_iter()
            .filter_map(|e| match e {
                DecisionEntry::Grant { granted, id, .. } => Some((granted, id)),
                _ => None,
            })
            .collect();
        assert_eq!(
            log,
            [
                (true, "g1".into()),
                (true, "g2".into()),
                (false, "g1".into()),
                (false, "g2".into())
            ]
        );
    }

    /// #12044: an agent wakes only on what its definition declares.
    #[test]
    fn an_agent_wakes_only_on_what_it_declares() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        f.declare(vec![WakeOn::IssueClosed], Some(10_000)).unwrap();
        let issue = |number, state: &str| GhIssue {
            number,
            title: format!("T{number}"),
            state: state.into(),
        };
        f.observe_issues(&[issue(10, "OPEN"), issue(9, "OPEN")], 0)
            .unwrap();
        f.observe_issues(&[issue(11, "OPEN"), issue(10, "CLOSED")], 4 * MIN)
            .unwrap();
        // Its 10-second window, not the default minute of quiet.
        assert_eq!(f.due(false, 4 * MIN + 5_000).unwrap(), None);
        let wake = f
            .due(false, 4 * MIN + 10_000)
            .unwrap()
            .expect("its own window");
        assert!(wake.text.contains("Issue #10 closed"));
        assert!(!wake.text.contains("#11 opened"), "not declared");
    }

    /// #12044: every agent has its own engine under its definition ID, and
    /// state saved before grants moves over once with its whole log.
    #[test]
    fn old_state_moves_under_its_agent_with_its_log() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("foreman");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(
            legacy.join("state.json"),
            r#"{"settings":{"chatId":"c1","everyMinutes":10,"paused":false,"authority":true},"authority":{"grantedAt":3,"scopes":["message","create","steer","stop"]}}"#,
        )
        .unwrap();
        let lines = [
            r#"{"kind":"authority","at":3,"granted":true,"scopes":["message","create","steer","stop"]}"#,
            r#"{"kind":"decision","at":4,"saw":"a","decided":"b","did":"c"}"#,
        ];
        std::fs::write(legacy.join("decisions.jsonl"), lines.join("\n") + "\n").unwrap();
        let wakes = AgentWakes::open(&dir.path().join("agents"));
        assert!(wakes.all().is_empty());
        assert_eq!(AgentWakes::legacy_chat(&legacy).as_deref(), Some("c1"));
        assert!(wakes.migrate(&legacy, AGENT).unwrap());
        assert!(!legacy.exists() && dir.path().join("foreman.migrated").is_dir());
        let engine = wakes.engine(AGENT).expect("moved");
        assert_eq!(engine.chat().as_deref(), Some("c1"));
        assert_eq!(engine.read(10).len(), 2, "the whole log");
        let old = engine
            .state()
            .authority
            .expect("kept until it becomes a grant");
        assert_eq!(old.granted_at, 3);
        assert!(wakes.by_chat("c1").is_some());
        assert!(!wakes.migrate(&legacy, AGENT).unwrap(), "once");
        // A restart finds it by its definition ID.
        let again = AgentWakes::open(&dir.path().join("agents"));
        assert_eq!(again.engine(AGENT).unwrap().read(10).len(), 2);
        // Two agents, two engines.
        let other = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
        again.ensure(other).unwrap();
        assert_eq!(again.all().len(), 2);
        assert!(again.ensure("Foreman").is_err(), "a name is no agent ID");
    }

    #[test]
    fn state_and_the_log_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        f.manage("a").unwrap();
        f.record(
            "two idle issues",
            "start one",
            "created a chat",
            Vec::new(),
            3,
        )
        .unwrap();
        let wake = f.due(false, 20 * MIN).unwrap().unwrap();
        assert!(
            wake.text
                .contains("saw two idle issues; decided start one; did created a chat")
        );
        drop(f);
        let f = WakeEngine::open(dir.path(), AGENT);
        assert!(f.state().managed.contains("a"));
        assert!(f.is_wake(&wake.id));
        assert_eq!(f.read(10).len(), 2);
        assert_eq!(f.due(false, 21 * MIN).unwrap(), None);
    }

    #[test]
    fn a_decision_keeps_the_judgments_it_used_for_the_next_check_in() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        let receipt = |answers: &str| JudgmentReceipt {
            question_set: "openagents.run-progress.v1".into(),
            decision: "dec_1".into(),
            model: "clef-flash".into(),
            door: Some("clef".into()),
            receipt: None,
            answers: answers.into(),
        };
        let many: Vec<JudgmentReceipt> = (0..20).map(|_| receipt("progressed 0.91")).collect();
        f.record("Beta silent", "message it", "sent a nudge", many, 3)
            .unwrap();
        let entries = f.read(10);
        let Some(DecisionEntry::Decision { judgments, .. }) = entries.last() else {
            panic!("a decision");
        };
        assert_eq!(judgments.len(), MAX_JUDGMENTS);
        assert_eq!(judgments[0].decision, "dec_1");
        let wake = f.due(false, 20 * MIN).unwrap().unwrap();
        assert!(
            wake.text
                .contains("did sent a nudge (judged: progressed 0.91; progressed 0.91;"),
            "{}",
            wake.text
        );
    }

    #[test]
    fn issues_and_stale_claims_become_events() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        let issue = |number, state: &str| GhIssue {
            number,
            title: format!("T{number}"),
            state: state.into(),
        };
        f.observe_issues(&[issue(10, "OPEN"), issue(9, "OPEN")], 0)
            .unwrap();
        f.observe_issues(
            &[issue(11, "OPEN"), issue(10, "CLOSED"), issue(9, "OPEN")],
            MIN,
        )
        .unwrap();
        let now = 10 * 3600 * 1000;
        let answer = serde_json::json!({"data":{"search":{"nodes":[
            {"number":20,"title":"Stale","comments":{"nodes":[
                {"body":"hi","createdAt":"1970-01-01T00:00:00Z"},
                {"body":"<!-- openagents-coder-claim cli session=codex:abc -->","createdAt":"1970-01-01T01:00:00Z"}]}},
            {"number":21,"title":"Fresh","comments":{"nodes":[
                {"body":"<!-- openagents-coder-claim cli session=codex:def -->","createdAt":"1970-01-01T09:00:00Z"}]}},
            {"number":22,"title":"Active","comments":{"nodes":[
                {"body":"<!-- openagents-coder-claim cli session=codex:ghi -->","createdAt":"1970-01-01T01:00:00Z"},
                {"body":"progress","createdAt":"1970-01-01T02:00:00Z"}]}}
        ]}}});
        let issues = claim_issues(&answer);
        let local = LocalProgress::default();
        f.observe_claims(&issues, &local, now).unwrap();
        f.observe_claims(&issues, &local, now + MIN).unwrap();
        let wake = f.due(false, now + 10 * MIN).unwrap().unwrap();
        assert!(wake.text.contains("Issue #11 opened: T11"));
        assert!(wake.text.contains("Issue #10 closed: T10"));
        assert!(!wake.text.contains("#9"));
        assert!(
            wake.text
                .contains("The claim on #20 (Stale) by codex:abc has had no activity since")
        );
        assert_eq!(wake.text.matches("has had no activity since").count(), 1);
    }

    #[test]
    fn a_new_agent_checks_in_at_least_every_ten_minutes_with_a_status() {
        let dir = tempfile::tempdir().unwrap();
        let f = WakeEngine::open(dir.path(), AGENT);
        let settings = WakeSettings {
            chat_id: Some("own".into()),
            ..WakeSettings::default()
        };
        assert!(settings.every_minutes <= 10);
        f.set(settings, 0).unwrap();
        assert_eq!(f.next_scheduled(), Some(10 * MIN));
        let quiet = f.due(false, 10 * MIN).unwrap().expect("a quiet check-in");
        assert!(quiet.text.contains("Nothing new was reported"));
        assert!(quiet.text.contains("post a short status of 3 to 6 lines"));
        assert!(f.due(false, 20 * MIN).unwrap().is_some(), "and again");
    }

    /// The chat's own row while it works, with its turn start and last
    /// output in minutes.
    fn own(status: SessionStatus, started: i64, output: i64) -> Session {
        let at = |m: i64| chrono::DateTime::from_timestamp_millis(m * MIN);
        Session {
            started_at: at(started),
            last_output_at: at(output),
            ..row("own", status, None)
        }
    }

    fn stuck_outcomes(f: &WakeEngine) -> usize {
        f.read(50)
            .iter()
            .filter(|e| matches!(e, DecisionEntry::Outcome { status, .. } if status == "stuck"))
            .count()
    }

    /// #12029: a turn that stays `working` with no output for 10 minutes is
    /// stopped once, even while the interrupt takes a while to land, and
    /// exactly one check-in names the tool and how long it ran.
    #[test]
    fn a_turn_with_no_output_for_ten_minutes_is_stopped_once_and_checked_in() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        let title = |c: &str| c.to_owned();
        let curl = || Some("`Bash` (`curl https://openagents.com/api/health`)".to_owned());
        // A scheduled check-in starts, and its turn hangs in one call at 1 min.
        f.observe(&[own(SessionStatus::Working, 1, 1)], |_| false, title, MIN)
            .unwrap();
        assert_eq!(f.watchdog(true, 10 * MIN, curl).unwrap(), None, "9 minutes");
        assert_eq!(f.watchdog(false, 11 * MIN, curl).unwrap(), None, "not busy");
        let stuck = f.watchdog(true, 11 * MIN, curl).unwrap().expect("stopped");
        assert_eq!((stuck.turn, stuck.since), (MIN, MIN));
        // The interrupt takes minutes to land; nothing fires again.
        for minute in [12, 15, 30] {
            f.observe(
                &[own(SessionStatus::Working, 1, 1)],
                |_| false,
                title,
                minute * MIN,
            )
            .unwrap();
            assert_eq!(f.watchdog(true, minute * MIN, curl).unwrap(), None);
            assert_eq!(
                f.due(true, minute * MIN).unwrap(),
                None,
                "never over a turn"
            );
        }
        assert_eq!(stuck_outcomes(&f), 1);
        // Once the chat is idle, one check-in says why.
        f.observe(
            &[own(SessionStatus::Idle, 1, 1)],
            |_| false,
            title,
            31 * MIN,
        )
        .unwrap();
        let wake = f.due(false, 31 * MIN).unwrap().expect("a check-in");
        assert!(wake.text.contains("(your turn was stuck)"), "{}", wake.text);
        assert!(wake.text.contains(
            "Your last turn was stuck in `Bash` (`curl https://openagents.com/api/health`) for 10 minutes, so the host stopped it."
        ));
        assert!(wake.text.contains("Put a timeout on every probe"));
        assert_eq!(f.due(false, 32 * MIN).unwrap(), None, "exactly one");
        let stuck_wakes = f
            .read(50)
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    DecisionEntry::Wake {
                        reason: WakeReason::Stuck,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(stuck_wakes, 1);
        assert_eq!(stuck_outcomes(&f), 1);
        // The schedule slot that was also due counts as served.
        assert_eq!(f.next_scheduled(), Some(51 * MIN));
        // A new turn that hangs is its own stop.
        f.observe(
            &[own(SessionStatus::Working, 40, 41)],
            |_| false,
            title,
            40 * MIN,
        )
        .unwrap();
        assert!(f.watchdog(true, 51 * MIN, || None).unwrap().is_some());
        assert_eq!(stuck_outcomes(&f), 2);
    }

    /// #12029: a busy turn that keeps producing output is left alone.
    #[test]
    fn a_busy_turn_that_keeps_producing_output_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        for minute in 1..=30 {
            // A long build that prints, or a wait loop with progress lines.
            let output = minute - minute % 4;
            f.observe(
                &[own(SessionStatus::Working, 0, output)],
                |_| false,
                |c| c.to_owned(),
                minute * MIN,
            )
            .unwrap();
            assert_eq!(f.watchdog(true, minute * MIN, || None).unwrap(), None);
        }
        assert_eq!(stuck_outcomes(&f), 0);
        // A chat waiting for the owner's answer isn't stuck either.
        f.observe(
            &[own(SessionStatus::AwaitingInput, 0, 0)],
            |_| false,
            |c| c.to_owned(),
            60 * MIN,
        )
        .unwrap();
        assert_eq!(f.watchdog(true, 60 * MIN, || None).unwrap(), None);
    }

    #[test]
    fn tool_labels_are_short_and_plain() {
        let exec = ToolCall::Exec {
            command: format!("curl -s {}\nlsof", "x".repeat(80)),
        };
        let label = tool_label(&exec);
        assert!(label.starts_with("`Bash` (`curl -s xxx"));
        assert!(label.ends_with(" …`)") && !label.contains("lsof"));
        assert_eq!(
            tool_label(&ToolCall::Mcp {
                server: "openagents".into(),
                tool: "wait_for_turn".into(),
                input: None
            }),
            "`wait_for_turn`"
        );
    }

    fn declaring(chat: &str, title: &str, check_ins: bool) -> Declaring {
        Declaring {
            chat_id: chat.into(),
            title: title.into(),
            agent: Some(format!("def-{chat}")),
            declared: Declared {
                check_ins,
                every_minutes: Some(10),
                authority: true,
                ..Declared::default()
            },
        }
    }

    /// #12030: the chat whose agent declares check-ins is picked whatever
    /// its name; one named "Foreman" without the declaration is not.
    #[test]
    fn the_declaring_agent_is_picked_whatever_its_name() {
        let dir = tempfile::tempdir().unwrap();
        let f = WakeEngine::open(dir.path(), AGENT);
        let chats = [
            declaring("plain", "Foreman", false),
            declaring("night", "Night shift", true),
        ];
        let Adopt::Picked(pick) = f.adopt(&chats, 5 * MIN).unwrap() else {
            panic!("picked");
        };
        assert_eq!(pick.chat_id, "night");
        let s = f.state();
        assert_eq!(s.settings.chat_id.as_deref(), Some("night"));
        assert_eq!(s.settings.every_minutes, 10);
        assert!(!s.settings.paused);
        assert_eq!(s.turned_on_at, Some(5 * MIN));
        assert_eq!(s.agent.as_deref(), Some("def-night"));
        assert!(s.wants_standing, "the host asks the account for the grant");
        assert_eq!(f.next_scheduled(), Some(15 * MIN), "first check-in in 10");
        // The account grants it; the grant is logged like one from the card.
        f.set_grants(vec![standing("g", 5 * MIN)], 5 * MIN).unwrap();
        assert!(f.admits(Scope::ChatSteer, "w", None, true).is_ok());
        assert!(matches!(
            &f.read(10)[..],
            [DecisionEntry::Grant { granted: true, scopes, .. }] if *scopes == Scope::CHAT
        ));
        let wake = f.due(false, 15 * MIN).unwrap().expect("first check-in");
        assert!(
            wake.text
                .contains("You may message, start, steer and stop chats")
        );
        // Revoking the grant changes nothing an agent may do (#12135).
        f.set_grants(vec![], 16 * MIN).unwrap();
        let wake = f.due(false, 25 * MIN).unwrap().expect("next check-in");
        assert!(
            wake.text
                .contains("You may message, start, steer and stop chats")
        );
        // Choosing another chat clears the mark, and the old one is never
        // picked again.
        let mut none = f.state().settings;
        none.chat_id = None;
        f.set(none, 26 * MIN).unwrap();
        assert_eq!(f.state().turned_on_at, None);
        assert_eq!(f.adopt(&chats, 27 * MIN).unwrap(), Adopt::Nothing);
        assert_eq!(f.state().settings.chat_id, None);
        // A name alone never picks.
        let dir = tempfile::tempdir().unwrap();
        let f = WakeEngine::open(dir.path(), AGENT);
        assert_eq!(
            f.adopt(&[declaring("plain", "Foreman", false)], 0).unwrap(),
            Adopt::Nothing
        );
    }

    /// #12030: two declaring chats on one host pick none.
    #[test]
    fn two_declaring_chats_pick_none_and_ask() {
        let dir = tempfile::tempdir().unwrap();
        let f = WakeEngine::open(dir.path(), AGENT);
        let chats = [
            declaring("a", "Foreman", true),
            declaring("b", "Release Foreman", true),
        ];
        assert_eq!(
            f.adopt(&chats, 0).unwrap(),
            Adopt::Choose(vec!["Foreman".into(), "Release Foreman".into()])
        );
        assert_eq!(f.state().settings.chat_id, None);
        assert!(f.read(10).is_empty(), "nothing granted");
    }

    fn stall_events(f: &WakeEngine, from: i64, to: i64) -> Vec<String> {
        // Drain check-ins over the range and collect every stall event.
        let mut lines = Vec::new();
        let mut at = from;
        while at <= to {
            if let Some(wake) = f.due(false, at).unwrap() {
                lines.extend(
                    wake.text
                        .lines()
                        .filter(|l| {
                            l.contains("produced no output since")
                                || l.contains("shown no progress")
                        })
                        .map(str::to_owned),
                );
            }
            at += MIN;
        }
        lines
    }

    /// #12032: a managed chat working with no output for 20 minutes raises
    /// one `ChatStalled`, however many check-ins follow, and each check-in
    /// names the rung that is due until it moves again.
    #[test]
    fn a_silent_managed_chat_raises_one_stall_and_climbs_the_ladder() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        f.manage("w").unwrap();
        let title = |_: &str| "Land #11999".to_owned();
        let row = |output: i64| Session {
            started_at: chrono::DateTime::from_timestamp_millis(0),
            last_output_at: chrono::DateTime::from_timestamp_millis(output * MIN),
            ..row("w", SessionStatus::Working, None)
        };
        // The coordinator's last output is at 1 minute; it rebases and stops.
        let mut prompts = Vec::new();
        for minute in 1..=45 {
            f.observe(&[row(1)], |_| false, title, minute * MIN)
                .unwrap();
            if let Some(wake) = f.due(false, minute * MIN).unwrap() {
                prompts.push((minute, wake.text));
            }
        }
        let raised: usize = prompts
            .iter()
            .map(|(_, t)| {
                t.matches("is working but has produced no output since")
                    .count()
            })
            .sum();
        assert_eq!(raised, 1, "one event per stall");
        let rung = |n: &str| prompts.iter().find(|(_, t)| t.contains(n)).map(|(m, _)| *m);
        // Silent since minute 1: message at 21, restart at 31, owner at 41.
        assert_eq!(rung("rung 1: message it"), Some(21));
        assert_eq!(
            rung("rung 2: interrupt it and restart it with a brief"),
            Some(31)
        );
        assert_eq!(rung("rung 3: tell the owner"), Some(41));
        // Output again ends the stall.
        f.observe(&[row(46)], |_| false, title, 46 * MIN).unwrap();
        assert!(f.state().stalls.is_empty());
    }

    /// #12032: a chat waiting on a build lease that keeps printing raises
    /// nothing.
    #[test]
    fn a_chat_that_keeps_printing_raises_no_stall() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        f.manage("w").unwrap();
        for minute in 1..=60 {
            let row = Session {
                started_at: chrono::DateTime::from_timestamp_millis(0),
                last_output_at: chrono::DateTime::from_timestamp_millis(
                    (minute - minute % 3) * MIN,
                ),
                ..row("w", SessionStatus::Working, None)
            };
            f.observe(&[row], |_| false, |c| c.to_owned(), minute * MIN)
                .unwrap();
        }
        assert!(f.state().stalls.is_empty());
        assert!(stall_events(&f, 0, 60 * MIN).is_empty());
    }

    fn claim(
        number: u64,
        title: &str,
        session: &str,
        at_min: i64,
        extra: serde_json::Value,
    ) -> serde_json::Value {
        let at = chrono::DateTime::from_timestamp_millis(at_min * MIN)
            .unwrap()
            .to_rfc3339();
        let mut node = serde_json::json!({
            "number": number,
            "title": title,
            "comments": {"nodes": [
                {"body": format!("<!-- openagents-coder-claim cli session={session} -->"), "createdAt": at}
            ]}
        });
        for (k, v) in extra.as_object().unwrap() {
            node[k] = v.clone();
        }
        node
    }

    #[test]
    fn sixteen_chatless_claims_are_unclaimed_for_planning_at_the_stall_window() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        let nodes: Vec<_> = (1..=16)
            .map(|n| {
                claim(
                    n,
                    "Work",
                    "claude-code:coordinator",
                    0,
                    serde_json::json!({}),
                )
            })
            .collect();
        let issues = claim_issues(&serde_json::json!({"data":{"search":{"nodes":nodes}}}));
        f.observe_claims(&issues, &LocalProgress::default(), 20 * MIN)
            .unwrap();
        let wake = f.due(false, 21 * MIN).unwrap().unwrap();
        assert!(wake.text.contains("Claims with no live chat: #1"));
        assert!(
            wake.text
                .contains("plan as unclaimed and use the stall ladder: #1")
        );
        assert!(wake.text.contains("#16"));
        assert!(wake.text.contains("rung 1: message it"));
        assert_eq!(f.state().stalls.len(), 16);
    }

    /// #12032, 2026-10-17's case: a claim with no live, progressing chat for
    /// 20 minutes is raised once, and the check-ins carry it up the ladder
    /// (message, then restart, then the owner); a claim whose worktree shows
    /// an active build, or whose chat keeps producing output, is progressing.
    #[test]
    fn a_claim_with_no_progress_climbs_the_ladder_but_active_work_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        let answer = serde_json::json!({"data":{"search":{"nodes":[
            claim(11999, "Wallet", "claude-code:term", 0, serde_json::json!({})),
            claim(12002, "Delete account", "claude-code:builder", 0, serde_json::json!({})),
            claim(12005, "Apple", "claude-code:chat", 0, serde_json::json!({})),
        ]}}});
        let issues = claim_issues(&answer);
        let mut prompts = Vec::new();
        for minute in 1..=45 {
            let now = minute * MIN;
            let local = LocalProgress {
                // An app chat that keeps answering.
                chats: HashMap::from([("claude-code:chat".to_owned(), now - MIN)]),
                // A worktree named for #12002 with cargo running in it.
                builds: BTreeMap::from([(12002, now)]),
                ..LocalProgress::default()
            };
            f.observe_claims(&issues, &local, now).unwrap();
            if let Some(wake) = f.due(false, now).unwrap() {
                prompts.push((minute, wake.text));
            }
        }
        let all: String = prompts.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(
            all.matches("The claim on #11999 (Wallet) by claude-code:term has shown no progress")
                .count(),
            1,
            "one event per stall"
        );
        assert!(
            !all.contains("The claim on #12002"),
            "an active build is progress"
        );
        assert!(
            !all.contains("The claim on #12005"),
            "a chat with output is progress"
        );
        for line in all
            .lines()
            .filter(|line| line.contains("plan as unclaimed"))
        {
            assert!(!line.contains("#12002"));
            assert!(!line.contains("#12005"));
        }
        let rung = |n: &str| {
            prompts
                .iter()
                .find(|(_, t)| {
                    t.contains(&format!("#11999 (Wallet) by claude-code:term: no progress"))
                        && t.contains(n)
                })
                .map(|(m, _)| *m)
        };
        // Claimed at 0 with nothing since: message at 20, restart at 30,
        // the owner at 40.
        assert_eq!(rung("rung 1: message it"), Some(20));
        assert_eq!(
            rung("rung 2: interrupt it and restart it with a brief"),
            Some(30)
        );
        assert_eq!(rung("rung 3: tell the owner"), Some(40));
        // A new commit naming the issue resets it, and a later stall is a
        // new event.
        let pushed = serde_json::json!({"data":{"search":{"nodes":[
            claim(11999, "Wallet", "claude-code:term", 0, serde_json::json!({
                "timelineItems": {"nodes": [{"createdAt": chrono::DateTime::from_timestamp_millis(46 * MIN).unwrap().to_rfc3339()}]}
            })),
        ]}}});
        let pushed = claim_issues(&pushed);
        f.observe_claims(&pushed, &LocalProgress::default(), 47 * MIN)
            .unwrap();
        assert!(f.state().stalls.is_empty(), "the commit is progress");
        f.observe_claims(&pushed, &LocalProgress::default(), 66 * MIN)
            .unwrap();
        assert_eq!(f.state().stalls.len(), 1, "silent again for 20 minutes");
        // Releasing the claim ends it.
        let released = serde_json::json!({"data":{"search":{"nodes":[
            claim(11999, "Wallet", "claude-code:term", 0, serde_json::json!({"comments": {"nodes": [
                {"body": "<!-- openagents-coder-claim cli session=claude-code:term -->", "createdAt": "1970-01-01T00:00:00Z"},
                {"body": "<!-- openagents-coder-release -->", "createdAt": "1970-01-01T01:00:00Z"}
            ]}})),
        ]}}});
        f.observe_claims(
            &claim_issues(&released),
            &LocalProgress::default(),
            70 * MIN,
        )
        .unwrap();
        assert!(f.state().stalls.is_empty());
    }

    /// #12041: terminal output counts through the same claim stall clock;
    /// a live process without output still stalls, and a dead process never
    /// contributes fresh activity.
    #[test]
    fn terminal_run_activity_keeps_a_claim_moving_until_it_goes_silent() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        let mut settings = f.state().settings;
        settings.stall_minutes = 10;
        f.set(settings, 0).unwrap();
        let answer = serde_json::json!({"data":{"search":{"nodes":[
            claim(12041, "Active terminal", "claude-code:active", 0, serde_json::json!({})),
            claim(12042, "Silent terminal", "codex:silent", 0, serde_json::json!({})),
            claim(12043, "Dead terminal", "claude-code:dead", 0, serde_json::json!({})),
        ]}}});
        let issues = claim_issues(&answer);
        let run = |kind: &str, session: &str, alive, last_activity| coder_lease::runs::RunStatus {
            id: format!("run-{session}"),
            kind: kind.into(),
            host: "test-computer".into(),
            sessions: vec![session.into()],
            root_pid: 1234,
            root_start: 1.0,
            alive,
            created: 0,
            last_activity,
        };
        let mut local = LocalProgress::default();
        for minute in 1..=20 {
            local = LocalProgress::default().with_runs(&[
                run("claude", "active", true, (minute * 60) as u64),
                run("codex", "silent", true, (minute.min(5) * 60) as u64),
                run("claude-code", "dead", false, (minute * 60) as u64),
            ]);
            f.observe_claims(&issues, &local, minute * MIN).unwrap();
            let stalls = f.state().stalls;
            assert!(!stalls.contains_key("claim:12041"), "active at {minute}");
            assert_eq!(stalls.contains_key("claim:12042"), minute >= 15);
            assert_eq!(stalls.contains_key("claim:12043"), minute >= 10);
        }
        assert_eq!(f.state().stalls["claim:12042"].since, 5 * MIN);
        assert_eq!(f.state().stalls["claim:12043"].since, 0);
        assert_eq!(
            f.state()
                .pending
                .iter()
                .filter(|p| matches!(p.event, Event::ClaimNoProgress { .. }))
                .count(),
            2,
            "one event for each stalled terminal claim"
        );

        // New terminal output clears an existing stall.
        local = local.with_runs(&[run("codex", "silent", true, 21 * 60)]);
        f.observe_claims(&issues, &local, 21 * MIN).unwrap();
        assert!(!f.state().stalls.contains_key("claim:12042"));

        // A six-hour-old claim with current terminal output is not stale.
        local = local.with_runs(&[run("claude", "active", true, 361 * 60)]);
        f.observe_claims(&issues, &local, 361 * MIN).unwrap();
        assert!(!f.state().stalls.contains_key("claim:12041"));
        assert!(
            !f.state()
                .pending
                .iter()
                .any(|p| matches!(p.event, Event::StaleClaim { number: 12041, .. }))
        );
    }

    /// #12032: the last rung tells the owner once per stall, with a request
    /// ID that stays the same across retries.
    #[test]
    fn the_owner_hears_once_per_stall_on_the_last_rung() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        let answer = serde_json::json!({"data":{"search":{"nodes":[
            claim(11999, "Wallet", "claude-code:term", 0, serde_json::json!({})),
        ]}}});
        let issues = claim_issues(&answer);
        f.observe_claims(&issues, &LocalProgress::default(), 20 * MIN)
            .unwrap();
        assert!(f.owner_notices(39 * MIN).unwrap().is_empty(), "rung 2");
        let notices = f.owner_notices(40 * MIN).unwrap();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].request_id.len(), 64);
        assert!(notices[0].body.starts_with("The claim on #11999 (Wallet)"));
        assert!(f.owner_notices(50 * MIN).unwrap().is_empty(), "once");
        assert_eq!(
            notices[0].request_id,
            hex64(&format!("stall:claim:11999:{}", 0)),
            "stable for the same stall"
        );
    }

    /// A claim on an issue that closed drops out of the stall rule at once,
    /// even while the claims search still lists it (it can lag).
    #[test]
    fn a_closed_issues_claim_is_no_stall() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        let issue = |state: &str| GhIssue {
            number: 12062,
            title: "Done".into(),
            state: state.into(),
        };
        f.observe_issues(&[issue("OPEN")], 0).unwrap();
        let answer = serde_json::json!({"data":{"search":{"nodes":[
            claim(12062, "Done", "claude-code:coord", 0, serde_json::json!({})),
        ]}}});
        let issues = claim_issues(&answer);
        f.observe_claims(&issues, &LocalProgress::default(), 40 * MIN)
            .unwrap();
        assert_eq!(f.state().stalls.len(), 1, "stalled, rung 3 due");
        f.observe_issues(&[issue("CLOSED")], 41 * MIN).unwrap();
        assert!(f.state().stalls.is_empty(), "closing it ends the stall");
        assert!(
            !lock(&f.claims)
                .summary(41 * MIN, 20 * MIN)
                .contains("#12062")
        );
        assert!(f.owner_notices(45 * MIN).unwrap().is_empty());
        // The lagging search still lists it: still no stall.
        f.observe_claims(&issues, &LocalProgress::default(), 50 * MIN)
            .unwrap();
        assert!(f.state().stalls.is_empty());
        let wake = f.due(false, 60 * MIN).unwrap().expect("check-in");
        assert!(
            !wake.text.contains("has shown no progress"),
            "{}",
            wake.text
        );
        assert!(!wake.text.contains("Stalled work"), "{}", wake.text);
        for line in wake
            .text
            .lines()
            .filter(|line| line.starts_with("Claims with"))
        {
            assert!(!line.contains("#12062"), "{line}");
        }
    }

    /// A claim held by a coordinator session counts as progressing while an
    /// app chat titled for that issue has recent output.
    #[test]
    fn an_app_chat_titled_for_the_issue_is_progress() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        let answer = serde_json::json!({"data":{"search":{"nodes":[
            claim(12041, "Terminal runs", "claude-code:a9366ded", 0, serde_json::json!({})),
            claim(12038, "Land queue", "claude-code:a9366ded", 0, serde_json::json!({})),
        ]}}});
        let issues = claim_issues(&answer);
        for minute in [20, 40, 400] {
            let now = minute * MIN;
            let local = LocalProgress {
                titled: BTreeMap::from([(12041, now - 2 * MIN)]),
                ..LocalProgress::default()
            };
            f.observe_claims(&issues, &local, now).unwrap();
        }
        let stalls = f.state().stalls;
        assert!(!stalls.contains_key("claim:12041"), "its chat is answering");
        assert!(stalls.contains_key("claim:12038"), "nothing works #12038");
        assert_eq!(
            issue_refs("#12041 Foreman: act on terminal sessions"),
            [12041]
        );
        assert_eq!(issue_refs("Cloud fan-out (#12058, #12037)"), [12058, 12037]);
        assert!(issue_refs("Fix #12 and # 12041").is_empty());
    }

    #[test]
    fn the_stall_window_is_the_agents_setting() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        let mut s = f.state().settings;
        s.stall_minutes = 10;
        f.set(s, 0).unwrap();
        let answer = serde_json::json!({"data":{"search":{"nodes":[
            claim(1234, "Fork", "codex:x", 0, serde_json::json!({})),
        ]}}});
        f.observe_claims(&claim_issues(&answer), &LocalProgress::default(), 9 * MIN)
            .unwrap();
        assert!(f.state().stalls.is_empty());
        f.observe_claims(&claim_issues(&answer), &LocalProgress::default(), 10 * MIN)
            .unwrap();
        assert_eq!(f.state().stalls.len(), 1);
    }

    #[test]
    fn paused_or_unset_never_wakes() {
        let dir = tempfile::tempdir().unwrap();
        let f = WakeEngine::open(dir.path(), AGENT);
        assert_eq!(f.due(false, 100 * MIN).unwrap(), None);
        let f = configured(dir.path());
        let mut s = f.state().settings;
        s.paused = true;
        f.set(s, 0).unwrap();
        assert_eq!(f.due(false, 100 * MIN).unwrap(), None);
        assert_eq!(f.next_scheduled(), None);
    }
}
