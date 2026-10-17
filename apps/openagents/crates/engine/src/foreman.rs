//! Foreman autonomy (#11965). The host that runs Foreman's chat wakes it
//! on its account schedule and when something it manages changes, holds the
//! standing authority the owner granted, and keeps the decision log.
//!
//! This module is the host-independent part: the saved state, the
//! append-only log, the coalescing of events into one check-in, the
//! check-in prompt, and reading GitHub's answers. `doc_host/foreman.rs`
//! drives it from the existing host loop.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use openagents_proto::foreman::{
    Authority, DecisionEntry, ForemanSettings, MAX_EVERY_MINUTES, MIN_EVERY_MINUTES, Scope,
    WakeReason,
};
use openagents_proto::{Session, SessionStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::EngineError;

/// Events wait this long for quiet before they start a check-in…
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
/// A field Foreman records is cut at this many bytes.
const MAX_FIELD: usize = 2000;
const MAX_EVENTS: usize = 200;
const MAX_WAKES: usize = 64;
const MAX_ISSUES: usize = 1000;
const MAX_MANAGED: usize = 512;
const CLAIM_MARK: &str = "<!-- openagents-coder-claim";
const RELEASE_MARK: &str = "<!-- openagents-coder-release";

/// Something that happened that Foreman should hear about.
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
}

impl Event {
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Pending {
    at: i64,
    event: Event,
}

/// The saved part of Foreman's state on this host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct State {
    pub settings: ForemanSettings,
    pub authority: Option<Authority>,
    /// The account schedule this host mirrors, and the `updated` time of
    /// the copy it last wrote or adopted.
    pub schedule_id: Option<String>,
    pub schedule_updated: u64,
    /// The local copy changed and the account hasn't heard yet.
    pub schedule_dirty: bool,
    /// Unix ms of the last check-in of any kind, and of the last scheduled one.
    pub last_check_in: i64,
    pub last_scheduled: i64,
    /// Chats Foreman created, messaged, steered or stopped.
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
}

impl Default for State {
    fn default() -> Self {
        Self {
            settings: ForemanSettings::default(),
            authority: None,
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
        }
    }
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

pub(crate) struct Foreman {
    root: PathBuf,
    state: Mutex<State>,
    /// Not saved: the first look after a start only records a baseline.
    marks: Mutex<HashMap<String, Mark>>,
    /// Foreman's own chat was seen working since the open check-in began.
    wake_seen_working: Mutex<bool>,
    problem: Mutex<Option<String>>,
    /// When each throttled job last ran, Unix ms.
    ran: Mutex<HashMap<&'static str, i64>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Foreman {
    pub fn open(root: &Path) -> Self {
        let state = std::fs::read(root.join("state.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<State>(&b).ok())
            .unwrap_or_default();
        Self {
            root: root.to_path_buf(),
            state: Mutex::new(state),
            marks: Mutex::new(HashMap::new()),
            wake_seen_working: Mutex::new(false),
            problem: Mutex::new(None),
            ran: Mutex::new(HashMap::new()),
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

    /// The owner's settings. Granting or revoking the authority is logged.
    pub fn set(&self, settings: ForemanSettings, now: i64) -> Result<(), EngineError> {
        let settings = ForemanSettings {
            every_minutes: settings
                .every_minutes
                .clamp(MIN_EVERY_MINUTES, MAX_EVERY_MINUTES),
            chat_id: settings.chat_id.filter(|c| !c.trim().is_empty()),
            ..settings
        };
        let change = self.update(|s| {
            let schedule_changed = s.settings.chat_id != settings.chat_id
                || s.settings.every_minutes != settings.every_minutes
                || s.settings.paused != settings.paused;
            if s.settings.chat_id != settings.chat_id {
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
            let was = s.authority.is_some();
            s.authority = match (settings.authority, s.authority.take()) {
                (true, Some(kept)) => Some(kept),
                (true, None) => Some(Authority {
                    granted_at: now,
                    scopes: Scope::ALL.to_vec(),
                }),
                (false, _) => None,
            };
            s.settings = settings.clone();
            (was != s.authority.is_some()).then(|| s.authority.clone())
        })?;
        if let Some(authority) = change {
            self.append(&DecisionEntry::Authority {
                at: now,
                granted: authority.is_some(),
                scopes: authority.map(|a| a.scopes).unwrap_or_default(),
            })?;
        }
        Ok(())
    }

    /// Whether Foreman's chat may do `scope` to another chat. Turns the
    /// owner starts are theirs to direct; a check-in turn needs the
    /// standing authority.
    pub fn admits(&self, scope: Scope, check_in_turn: bool) -> Result<(), String> {
        if !check_in_turn {
            return Ok(());
        }
        let state = lock(&self.state);
        match &state.authority {
            Some(a) if a.scopes.contains(&scope) => Ok(()),
            _ => Err(
                "Foreman doesn't have standing authority to change other chats. The owner can turn it on in Settings."
                    .into(),
            ),
        }
    }

    /// Record a chat Foreman acted on, so its turns wake Foreman later.
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
                if !s.pending.iter().any(|p| p.event == event) {
                    s.pending.push(Pending { at: now, event });
                }
            }
            let over = s.pending.len().saturating_sub(MAX_EVENTS);
            s.pending.drain(..over);
        })
    }

    /// Look at every device's session rows: managed chats' finished, errored
    /// and waiting turns become events, and the end of Foreman's own
    /// check-in turn is logged. `is_child(chat)` is whether Foreman's chat
    /// is its parent; `title(chat)` names it.
    pub fn observe(
        &self,
        rows: &[Session],
        is_child: impl Fn(&str) -> bool,
        title: impl Fn(&str) -> String,
        now: i64,
    ) -> Result<(), EngineError> {
        let state = self.state();
        let Some(foreman) = state.settings.chat_id.clone() else {
            return Ok(());
        };
        let mut events = Vec::new();
        let mut marks = lock(&self.marks);
        for row in rows {
            let mark = Mark {
                status: row.status.clone(),
                completed: row.last_completed_turn.clone(),
            };
            if row.chat_id == foreman {
                if mark.status == SessionStatus::Working {
                    *lock(&self.wake_seen_working) = true;
                }
                continue;
            }
            if !(state.managed.contains(&row.chat_id) || is_child(&row.chat_id)) {
                continue;
            }
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
        let foreman_row = rows.iter().find(|r| r.chat_id == foreman);
        if let (Some(wake), Some(row)) = (state.open_wake.clone(), foreman_row) {
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
        self.push_events(events, now)
    }

    /// When the next scheduled check-in is due.
    pub fn next_scheduled(&self) -> Option<i64> {
        let s = lock(&self.state);
        (s.settings.chat_id.is_some() && !s.settings.paused)
            .then(|| s.last_scheduled + i64::from(s.settings.every_minutes) * 60_000)
    }

    /// The check-in to start now, if any. `busy` is whether Foreman's chat
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
        let scheduled = self.next_scheduled().is_some_and(|at| now >= at);
        let events_ready = !s.pending.is_empty() && {
            let first = s.pending.iter().map(|p| p.at).min().unwrap_or(now);
            let last = s.pending.iter().map(|p| p.at).max().unwrap_or(now);
            (now - last >= QUIET_MS || now - first >= MAX_WAIT_MS)
                && now - s.last_check_in >= MIN_GAP_MS
        };
        if !scheduled && !events_ready {
            return Ok(None);
        }
        let reason = if scheduled {
            WakeReason::Schedule
        } else {
            WakeReason::Events
        };
        let events: Vec<String> = s.pending.iter().map(|p| p.event.line()).collect();
        let slot = match reason {
            WakeReason::Schedule => format!("schedule:{}", s.last_scheduled),
            WakeReason::Events => format!("events:{}", s.pending[0].at),
        };
        let id = uuid_from(&format!("foreman:{chat}:{slot}"));
        let recent = self.read(PROMPT_ENTRIES);
        let text = prompt(reason, now, &events, s.authority.as_ref(), &recent);
        self.update(|st| {
            st.pending.clear();
            st.last_check_in = now;
            if reason == WakeReason::Schedule {
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
        self.push_events(events, now)
    }

    /// Open issues with their latest comments into stale-claim events,
    /// each reported once per claim.
    pub fn observe_claims(&self, issues: &[GhClaimIssue], now: i64) -> Result<(), EngineError> {
        let events = self.update(|s| {
            let mut events = Vec::new();
            for issue in issues {
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
            events
        })?;
        self.push_events(events, now)
    }

    /// Foreman's own record of a check-in.
    pub fn record(&self, saw: &str, decided: &str, did: &str, now: i64) -> Result<(), EngineError> {
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

/// The check-in prompt. Plain words: it shows in Foreman's chat.
pub(crate) fn prompt(
    reason: WakeReason,
    now: i64,
    events: &[String],
    authority: Option<&Authority>,
    recent: &[DecisionEntry],
) -> String {
    let mut text = format!(
        "Foreman check-in, {} ({}).\n\n",
        clock(now),
        match reason {
            WakeReason::Schedule => "scheduled",
            WakeReason::Events => "something changed",
        }
    );
    if events.is_empty() {
        text.push_str("Nothing new was reported since the last check-in.\n");
    } else {
        text.push_str("Since the last check-in:\n");
        for event in events {
            text.push_str(&format!("- {event}\n"));
        }
    }
    text.push('\n');
    match authority {
        Some(a) => text.push_str(&format!(
            "You have the owner's standing authority (since {}) to message, start, steer and stop chats on the owner's computers without asking. Use it for routine coordination.\n",
            clock(a.granted_at)
        )),
        None => text.push_str(
            "You don't have standing authority right now: look and report, but don't message, start, steer or stop other chats.\n",
        ),
    }
    text.push_str(
        "Money, keys, secrets, deploys and closing other agents' issues still need the owner's approval.\n",
    );
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
            } = entry
            {
                text.push_str(&format!(
                    "- {}: saw {saw}; decided {decided}; did {did}\n",
                    clock(*at)
                ));
            }
        }
    }
    text.push_str(
        "\nGather fresh facts, act on what needs it, then call record_decision once with what you saw, what you decided and what you did.",
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
pub(crate) struct GhClaimIssue {
    pub number: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub comments: GhComments,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct GhComments {
    #[serde(default)]
    pub nodes: Vec<GhComment>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GhComment {
    #[serde(default)]
    pub body: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// The GraphQL search Foreman's instructions use for claims.
pub(crate) const CLAIMS_QUERY: &str = "{search(query:\"repo:OpenAgentsInc/openagents is:issue is:open\",type:ISSUE,first:100){nodes{... on Issue{number title comments(last:20){nodes{body createdAt}}}}}}";

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

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    fn row(chat: &str, status: SessionStatus, completed: Option<&str>) -> Session {
        Session {
            last_completed_turn: completed.map(str::to_owned),
            chat_id: chat.into(),
            device_id: "dev".into(),
            status,
            started_at: None,
            updated_at: chrono::Utc::now(),
            running_subagents: 0,
        }
    }

    fn configured(dir: &Path) -> Foreman {
        let f = Foreman::open(dir);
        f.set(
            ForemanSettings {
                chat_id: Some("foreman".into()),
                every_minutes: 20,
                paused: false,
                authority: false,
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
        assert!(wake.text.contains("don't have standing authority"));
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
            &[row("foreman", SessionStatus::Working, None)],
            child,
            title,
            6 * MIN,
        )
        .unwrap();
        f.observe(
            &[row("foreman", SessionStatus::Idle, Some("w"))],
            child,
            title,
            7 * MIN,
        )
        .unwrap();
        f.observe(
            &[row("foreman", SessionStatus::Idle, Some("w"))],
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
    fn authority_is_explicit_revocable_logged_and_only_gates_check_ins() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        assert!(
            f.admits(Scope::Message, false).is_ok(),
            "owner turns unchanged"
        );
        assert!(f.admits(Scope::Message, true).is_err());
        let mut on = f.state().settings;
        on.authority = true;
        f.set(on.clone(), 5).unwrap();
        assert!(f.admits(Scope::Stop, true).is_ok());
        assert!(
            prompt(
                WakeReason::Schedule,
                6,
                &[],
                f.state().authority.as_ref(),
                &[]
            )
            .contains("standing authority (since")
        );
        on.authority = false;
        f.set(on, 7).unwrap();
        assert!(f.admits(Scope::Create, true).is_err());
        let granted: Vec<bool> = f
            .read(10)
            .into_iter()
            .filter_map(|e| match e {
                DecisionEntry::Authority { granted, .. } => Some(granted),
                _ => None,
            })
            .collect();
        assert_eq!(granted, [true, false]);
    }

    #[test]
    fn state_and_the_log_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let f = configured(dir.path());
        f.manage("a").unwrap();
        f.record("two idle issues", "start one", "created a chat", 3)
            .unwrap();
        let wake = f.due(false, 20 * MIN).unwrap().unwrap();
        assert!(
            wake.text
                .contains("saw two idle issues; decided start one; did created a chat")
        );
        drop(f);
        let f = Foreman::open(dir.path());
        assert!(f.state().managed.contains("a"));
        assert!(f.is_wake(&wake.id));
        assert_eq!(f.read(10).len(), 2);
        assert_eq!(f.due(false, 21 * MIN).unwrap(), None);
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
        f.observe_claims(&issues, now).unwrap();
        f.observe_claims(&issues, now + MIN).unwrap();
        let wake = f.due(false, now + 10 * MIN).unwrap().unwrap();
        assert!(wake.text.contains("Issue #11 opened: T11"));
        assert!(wake.text.contains("Issue #10 closed: T10"));
        assert!(!wake.text.contains("#9"));
        assert!(wake.text.contains("The claim on #20 (Stale) by codex:abc"));
        assert_eq!(wake.text.matches("The claim on").count(), 1);
    }

    #[test]
    fn paused_or_unset_never_wakes() {
        let dir = tempfile::tempdir().unwrap();
        let f = Foreman::open(dir.path());
        assert_eq!(f.due(false, 100 * MIN).unwrap(), None);
        let f = configured(dir.path());
        let mut s = f.state().settings;
        s.paused = true;
        f.set(s, 0).unwrap();
        assert_eq!(f.due(false, 100 * MIN).unwrap(), None);
        assert_eq!(f.next_scheduled(), None);
    }
}
