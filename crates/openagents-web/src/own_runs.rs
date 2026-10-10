//! Own coding capacity (#11080): coding runs the inference gateway starts
//! on a person's own linked computer, with their own Codex or Claude Code
//! subscription, for their own API key's `openagents/code` requests under
//! `pay: "mine"` (`docs/inference/gateway.md`, section 4, "Own coding
//! capacity").
//!
//! Two sides meet here, both answering only one account's records:
//!
//! - **Coder on the computer** (`Authorization: Bearer sess_…`,
//!   [`crate::coder_sync::owner`]) reports each of its subscription
//!   accounts with how many more runs it can take now, takes the runs
//!   waiting for it, and reports each run's progress and answer.
//! - **The gateway** (its own token, [`Token`], from loopback only) reads
//!   the account's computers with free sessions, starts a run on one of
//!   them, reads its progress, and cancels it when the caller leaves.
//!
//! | Route | Who | What |
//! | --- | --- | --- |
//! | `POST /v1/computers/{name}/runs` `{accounts: [Account]}` | Coder | Report this computer's accounts; answers `{runs: [Taken]}`, each run handed out once |
//! | `POST /v1/computers/{name}/runs/{id}` `{lines, done?, failed?}` | Coder | A run's progress lines, then its answer or why it stopped; answers `{cancel}` |
//! | `GET /v1/own-runs/capacity?account=` | Gateway | `{accounts: [Linked]}`: the accounts on computers that reported lately |
//! | `POST /v1/own-runs` `{account, computer, run_account, agent, brief}` | Gateway | Start a run: `201 {id}`; `409 offline` when that computer or account isn't there |
//! | `GET /v1/own-runs/{id}?account=&after=&wait=[&taken=1]` | Gateway | The run: `{state, lines (from after), next, answer?, usage?, why?, limited}`, waiting up to `wait` seconds for news (with `taken`, also for Coder to take it) |
//! | `POST /v1/own-runs/{id}/cancel?account=` | Gateway | Stop the run |
//!
//! Records live beside the account's chats ([`Store::owner_key`]): the
//! accounts each computer reported ([`CAPACITY_KEY`]), the runs waiting
//! to be taken ([`QUEUE_KEY`]), and one object per run. Nothing here reads
//! a prompt to decide anything; the brief is carried, never inspected.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::App;
use crate::chat_store::{Error, Store, account_owner, now_unix};
use crate::coder_sync::{self, answer, line, refused, stored};

/// The gateway's routes, under this prefix (`crate::upstream::owned`).
pub(crate) const PREFIX: &str = "/v1/own-runs";
/// The accounts each computer reported, under the account's folder.
pub(crate) const CAPACITY_KEY: &str = "own-runs/capacity.json";
/// The runs waiting for a computer to take them.
pub(crate) const QUEUE_KEY: &str = "own-runs/queue.json";
const CAPACITY_SCHEMA: &str = "openagents.web.own-runs.capacity.v1";
const QUEUE_SCHEMA: &str = "openagents.web.own-runs.queue.v1";
const RUN_SCHEMA: &str = "openagents.web.own-runs.run.v1";

/// A computer whose last report is older than this offers nothing (Coder
/// reports every 10 seconds).
pub(crate) const ONLINE_SECONDS: u64 = 45;
/// An unchanged report is still written this often, for its time.
const REWRITE_EVERY: u64 = 20;
/// The most computers the capacity record keeps.
const MAX_COMPUTERS: usize = 16;
/// The most subscription accounts one computer reports.
pub(crate) const MAX_ACCOUNTS: usize = 8;
/// The most runs waiting at once for one account's computers.
const MAX_WAITING: usize = 16;
/// The most runs kept (finished ones go first, then the oldest).
const MAX_RUNS: usize = 32;
/// A run waiting this long for its computer is dropped from the queue
/// (the gateway gave up long before).
const WAIT_TTL: u64 = 300;
/// A finished run is kept this long, for the gateway's last read.
const KEEP_FINISHED: u64 = 3600;
/// The most progress lines a run keeps.
const MAX_LINES: usize = 512;
/// The longest progress line, in characters.
const LINE_CHARS: usize = 300;
/// The longest answer, in bytes.
const MAX_ANSWER: usize = 256 * 1024;
/// The longest brief, in bytes (the gateway's own limit is 256 KiB).
const MAX_BRIEF: usize = 272 * 1024;
/// The longest a gateway read waits for news.
const MAX_WAIT: u64 = 20;
/// A running run Coder hasn't reported on in this long counts as stopped
/// (Coder reports at least every few seconds while it runs).
const STALE_SECONDS: u64 = 90;
/// How often a waiting read looks again.
const LOOK_EVERY: Duration = Duration::from_millis(750);

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/computers/{name}/runs", post(take_route))
        .route("/v1/computers/{name}/runs/{id}", post(report_route))
        .route("/v1/own-runs", post(start_route))
        .route("/v1/own-runs/capacity", get(capacity_route))
        .route("/v1/own-runs/{id}", get(read_route))
        .route("/v1/own-runs/{id}/cancel", post(cancel_route))
}

/// Whether `path` is one of the gateway's routes here.
pub(crate) fn owns(path: &str) -> bool {
    path.strip_prefix(PREFIX)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

// ------------------------------------------------------------------ records

/// The coding agent a subscription runs, as the gateway names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Agent {
    Codex,
    ClaudeCode,
}

/// One subscription account on one computer, as Coder reports it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Account {
    /// Coder's id for it on that computer ("codex", "claude-code").
    pub id: String,
    /// Its name, as the person sees it ("Codex").
    #[serde(default)]
    pub label: String,
    pub agent: Agent,
    /// How many more runs it can take now.
    pub free_sessions: u32,
}

/// Plain ids: letters, digits, `-`, `_`, `.`, up to 64 bytes.
fn plain_id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

impl Account {
    fn checked(mut self) -> Option<Self> {
        if !plain_id(&self.id) {
            return None;
        }
        self.label = line(&self.label, 64);
        self.free_sessions = self.free_sessions.min(64);
        Some(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Reported {
    reported_unix: u64,
    accounts: Vec<Account>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Capacity {
    #[serde(default)]
    schema: String,
    /// Computer name to its last report.
    #[serde(default)]
    computers: BTreeMap<String, Reported>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Waiting {
    computer: String,
    created_unix: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Queue {
    #[serde(default)]
    schema: String,
    /// Run id to the computer it waits for.
    #[serde(default)]
    waiting: BTreeMap<String, Waiting>,
    /// Every run kept, run id to when it started, for cleanup.
    #[serde(default)]
    runs: BTreeMap<String, u64>,
}

/// One earlier turn of the conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Turn {
    pub role: String,
    pub content: String,
}

/// What the run is asked to do: the conversation as text, the task last.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Brief {
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub history: Vec<Turn>,
    pub task: String,
}

impl Brief {
    fn size(&self) -> usize {
        self.task.len()
            + self.instructions.as_ref().map_or(0, String::len)
            + self.history.iter().map(|t| t.content.len()).sum::<usize>()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RunState {
    /// Waiting for Coder on the computer to take it.
    Waiting,
    /// Taken: Coder is running it.
    Running,
    Done,
    Failed,
    Cancelled,
}

impl RunState {
    fn finished(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }

    fn word(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Run {
    schema: String,
    id: String,
    /// The computer's name.
    computer: String,
    account: String,
    agent: Agent,
    brief: Brief,
    created_unix: u64,
    updated_unix: u64,
    state: RunState,
    #[serde(default)]
    lines: Vec<String>,
    #[serde(default)]
    answer: Option<String>,
    #[serde(default)]
    usage: Option<Usage>,
    /// Why it stopped, in plain words.
    #[serde(default)]
    why: Option<String>,
    /// It stopped on the subscription's usage limit.
    #[serde(default)]
    limited: bool,
    /// The gateway asked it to stop.
    #[serde(default)]
    cancel: bool,
}

/// A computer's id as the gateway names it: its name's digest, so names
/// with spaces and quotes stay out of upstream names.
pub(crate) fn computer_id(name: &str) -> String {
    let digest = Sha256::digest(format!("openagents.web.own-runs.computer.v1\0{name}"));
    let hex: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
    format!("c{hex}")
}

fn run_key(owner: &str, id: &str) -> Result<String, Error> {
    Store::owner_key(owner, &format!("own-runs/run-{id}.json"))
}

fn new_run_id() -> String {
    let bytes: [u8; 16] = secp256k1::rand::random();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("run{hex}")
}

fn valid_run_id(id: &str) -> bool {
    id.len() == 35
        && id.starts_with("run")
        && id[3..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Read, change, and write one JSON record at `key` under compare-and-swap.
/// `change` says whether it changed anything; nothing is written when it
/// didn't.
async fn update<R, T>(
    store: &Store,
    key: &str,
    fresh: impl Fn() -> R,
    change: impl Fn(&mut R) -> (bool, T),
) -> Result<T, Error>
where
    R: Serialize + for<'de> Deserialize<'de>,
{
    for _ in 0..6 {
        let (mut record, generation) = match store.read_key(key).await? {
            Some((bytes, generation)) => (
                serde_json::from_slice::<R>(&bytes)
                    .map_err(|_| Error::Corrupt("An own-run record is invalid."))?,
                Some(generation),
            ),
            None => (fresh(), None),
        };
        let (changed, result) = change(&mut record);
        if !changed {
            return Ok(result);
        }
        let bytes = serde_json::to_vec(&record)
            .map_err(|_| Error::Invalid("An own-run record is invalid."))?;
        match store.write_key(key, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(result),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

fn fresh_capacity() -> Capacity {
    Capacity {
        schema: CAPACITY_SCHEMA.into(),
        ..Capacity::default()
    }
}

fn fresh_queue() -> Queue {
    Queue {
        schema: QUEUE_SCHEMA.into(),
        ..Queue::default()
    }
}

async fn load_run(store: &Store, owner: &str, id: &str) -> Result<Option<Run>, Error> {
    let key = run_key(owner, id)?;
    match store.read_key(&key).await? {
        Some((bytes, _)) => serde_json::from_slice::<Run>(&bytes)
            .map(Some)
            .map_err(|_| Error::Corrupt("An own run is invalid.")),
        None => Ok(None),
    }
}

/// Change the run `id` with `change`; `None` when there is no such run.
async fn update_run<T>(
    store: &Store,
    owner: &str,
    id: &str,
    change: impl Fn(&mut Run) -> (bool, T),
) -> Result<Option<T>, Error> {
    let key = run_key(owner, id)?;
    for _ in 0..6 {
        let Some((bytes, generation)) = store.read_key(&key).await? else {
            return Ok(None);
        };
        let mut run: Run =
            serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt("An own run is invalid."))?;
        let (changed, result) = change(&mut run);
        if !changed {
            return Ok(Some(result));
        }
        run.updated_unix = now_unix();
        let bytes =
            serde_json::to_vec(&run).map_err(|_| Error::Invalid("An own run is invalid."))?;
        match store.write_key(&key, bytes, Some(&generation)).await {
            Ok(_) => return Ok(Some(result)),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

// ------------------------------------------------------------ Coder's side

/// Coder on `computer` reports its accounts: keep them (when they changed,
/// or the last write is old).
pub(crate) async fn report_capacity(
    store: &Store,
    owner: &str,
    computer: &str,
    accounts: Vec<Account>,
) -> Result<(), Error> {
    let key = Store::owner_key(owner, CAPACITY_KEY)?;
    let computer = computer.to_owned();
    update(store, &key, fresh_capacity, move |capacity| {
        let now = now_unix();
        let current = capacity.computers.get(&computer);
        if current.is_some_and(|reported| {
            reported.accounts == accounts
                && now.saturating_sub(reported.reported_unix) < REWRITE_EVERY
        }) {
            return (false, ());
        }
        capacity.schema = CAPACITY_SCHEMA.into();
        capacity.computers.insert(
            computer.clone(),
            Reported {
                reported_unix: now,
                accounts: accounts.clone(),
            },
        );
        capacity
            .computers
            .retain(|_, reported| now.saturating_sub(reported.reported_unix) < 24 * 3600);
        while capacity.computers.len() > MAX_COMPUTERS {
            let Some(oldest) = capacity
                .computers
                .iter()
                .min_by_key(|(_, reported)| reported.reported_unix)
                .map(|(name, _)| name.clone())
            else {
                break;
            };
            capacity.computers.remove(&oldest);
        }
        (true, ())
    })
    .await
}

/// A run handed to Coder.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct Taken {
    pub id: String,
    pub account: String,
    pub agent: Agent,
    pub brief: Brief,
}

/// Take the runs waiting for `computer`. Each is handed out once.
pub(crate) async fn take(store: &Store, owner: &str, computer: &str) -> Result<Vec<Taken>, Error> {
    let key = Store::owner_key(owner, QUEUE_KEY)?;
    let name = computer.to_owned();
    let ids = update(store, &key, fresh_queue, move |queue| {
        let now = now_unix();
        let before = queue.waiting.len();
        queue
            .waiting
            .retain(|_, waiting| now.saturating_sub(waiting.created_unix) < WAIT_TTL);
        let mine: Vec<String> = queue
            .waiting
            .iter()
            .filter(|(_, waiting)| waiting.computer == name)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &mine {
            queue.waiting.remove(id);
        }
        (queue.waiting.len() != before, mine)
    })
    .await?;
    let mut taken = Vec::new();
    for id in ids {
        let claimed = update_run(store, owner, &id, |run| {
            if run.state != RunState::Waiting || run.cancel {
                return (false, None);
            }
            run.state = RunState::Running;
            (
                true,
                Some(Taken {
                    id: run.id.clone(),
                    account: run.account.clone(),
                    agent: run.agent,
                    brief: run.brief.clone(),
                }),
            )
        })
        .await?;
        if let Some(Some(run)) = claimed {
            taken.push(run);
        }
    }
    Ok(taken)
}

/// What Coder reports for one run.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Report {
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub done: Option<Done>,
    #[serde(default)]
    pub failed: Option<Failed>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Done {
    pub text: String,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Failed {
    pub why: String,
    /// The subscription reached its usage limit.
    #[serde(default)]
    pub limited: bool,
}

/// Keep a run's progress; the answer is whether it should stop. `None`
/// when there is no such run on `computer`.
pub(crate) async fn report(
    store: &Store,
    owner: &str,
    computer: &str,
    id: &str,
    sent: Report,
) -> Result<Option<bool>, Error> {
    let lines: Vec<String> = sent
        .lines
        .iter()
        .map(|text| line(text, LINE_CHARS))
        .filter(|text| !text.is_empty())
        .take(MAX_LINES)
        .collect();
    let done = sent.done.map(|done| {
        let mut text = done.text;
        if text.len() > MAX_ANSWER {
            let mut cut = MAX_ANSWER;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            text.truncate(cut);
        }
        let usage = (done.input_tokens.is_some() || done.output_tokens.is_some()).then(|| Usage {
            input_tokens: done.input_tokens.unwrap_or(0),
            output_tokens: done.output_tokens.unwrap_or(0),
        });
        (text, usage)
    });
    let failed = sent
        .failed
        .map(|failed| (line(&failed.why, 500), failed.limited));
    update_run(store, owner, id, |run| {
        if run.computer != computer {
            return (false, None);
        }
        if run.state.finished() || run.cancel {
            return (false, Some(true));
        }
        // A report with nothing new still says Coder is on it, now and then.
        let mut changed = now_unix().saturating_sub(run.updated_unix) >= REWRITE_EVERY;
        for text in &lines {
            if run.lines.len() >= MAX_LINES {
                run.lines.remove(0);
            }
            run.lines.push(text.clone());
            changed = true;
        }
        if run.state == RunState::Waiting {
            run.state = RunState::Running;
            changed = true;
        }
        if let Some((text, usage)) = &done {
            run.state = RunState::Done;
            run.answer = Some(text.clone());
            run.usage = *usage;
            changed = true;
        } else if let Some((why, limited)) = &failed {
            run.state = RunState::Failed;
            run.why = Some(if why.is_empty() {
                "The run stopped.".into()
            } else {
                why.clone()
            });
            run.limited = *limited;
            changed = true;
        }
        (changed, Some(false))
    })
    .await
    .map(Option::flatten)
}

// ---------------------------------------------------------- gateway's side

/// One account the gateway may start a run on, as `coder::Linked` reads it.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct Linked {
    pub computer: String,
    pub computer_label: String,
    pub account: String,
    pub account_label: String,
    pub agent: Agent,
    pub free_sessions: u32,
}

/// The accounts on the computers that reported within
/// [`ONLINE_SECONDS`], with a free session, less the runs already waiting
/// or running on them.
pub(crate) async fn capacity(store: &Store, owner: &str) -> Result<Vec<Linked>, Error> {
    let key = Store::owner_key(owner, CAPACITY_KEY)?;
    let capacity: Capacity = match store.read_key(&key).await? {
        Some((bytes, _)) => serde_json::from_slice(&bytes)
            .map_err(|_| Error::Corrupt("An own-run record is invalid."))?,
        None => return Ok(Vec::new()),
    };
    let queue_key = Store::owner_key(owner, QUEUE_KEY)?;
    let queue: Queue = match store.read_key(&queue_key).await? {
        Some((bytes, _)) => serde_json::from_slice(&bytes).unwrap_or_default(),
        None => Queue::default(),
    };
    let now = now_unix();
    let mut linked = Vec::new();
    for (name, reported) in &capacity.computers {
        if now.saturating_sub(reported.reported_unix) > ONLINE_SECONDS {
            continue;
        }
        // Runs queued for this computer since its report aren't counted
        // in its free sessions yet.
        let queued = queue
            .waiting
            .values()
            .filter(|waiting| {
                waiting.computer == *name && waiting.created_unix >= reported.reported_unix
            })
            .count();
        for account in &reported.accounts {
            let free = account
                .free_sessions
                .saturating_sub(u32::try_from(queued).unwrap_or(u32::MAX));
            if free == 0 {
                continue;
            }
            linked.push(Linked {
                computer: computer_id(name),
                computer_label: name.clone(),
                account: account.id.clone(),
                account_label: if account.label.is_empty() {
                    account.id.clone()
                } else {
                    account.label.clone()
                },
                agent: account.agent,
                free_sessions: free,
            });
        }
    }
    Ok(linked)
}

/// What the gateway asks to start.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Start {
    /// The account id (`acct_…`) the caller's key resolves to.
    pub account: String,
    /// The computer's id ([`computer_id`]).
    pub computer: String,
    /// The subscription account on it.
    pub run_account: String,
    pub agent: Agent,
    pub brief: Brief,
}

/// Why a run can't start.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Refused {
    /// The computer hasn't reported lately, or doesn't have that account.
    Offline,
    /// [`MAX_WAITING`] runs already wait.
    Busy,
    Invalid(&'static str),
}

/// Start a run for `owner`: its record, then its place in the queue.
pub(crate) async fn start(
    store: &Store,
    owner: &str,
    sent: Start,
) -> Result<Result<String, Refused>, Error> {
    if sent.brief.task.trim().is_empty() {
        return Ok(Err(Refused::Invalid("Send the task.")));
    }
    if sent.brief.size() > MAX_BRIEF || sent.brief.history.len() > 1024 {
        return Ok(Err(Refused::Invalid("The brief is too long.")));
    }
    if !plain_id(&sent.computer) || !plain_id(&sent.run_account) {
        return Ok(Err(Refused::Invalid("Name the computer and the account.")));
    }
    // The computer's name, from its report.
    let key = Store::owner_key(owner, CAPACITY_KEY)?;
    let capacity: Capacity = match store.read_key(&key).await? {
        Some((bytes, _)) => serde_json::from_slice(&bytes)
            .map_err(|_| Error::Corrupt("An own-run record is invalid."))?,
        None => return Ok(Err(Refused::Offline)),
    };
    let now = now_unix();
    let Some(name) = capacity
        .computers
        .iter()
        .find(|(name, reported)| {
            computer_id(name) == sent.computer
                && now.saturating_sub(reported.reported_unix) <= ONLINE_SECONDS
                && reported
                    .accounts
                    .iter()
                    .any(|account| account.id == sent.run_account && account.agent == sent.agent)
        })
        .map(|(name, _)| name.clone())
    else {
        return Ok(Err(Refused::Offline));
    };
    let id = new_run_id();
    let run = Run {
        schema: RUN_SCHEMA.into(),
        id: id.clone(),
        computer: name.clone(),
        account: sent.run_account,
        agent: sent.agent,
        brief: sent.brief,
        created_unix: now,
        updated_unix: now,
        state: RunState::Waiting,
        lines: Vec::new(),
        answer: None,
        usage: None,
        why: None,
        limited: false,
        cancel: false,
    };
    let bytes = serde_json::to_vec(&run).map_err(|_| Error::Invalid("An own run is invalid."))?;
    store.write_key(&run_key(owner, &id)?, bytes, None).await?;
    let queue_key = Store::owner_key(owner, QUEUE_KEY)?;
    let run_id = id.clone();
    let queued = update(store, &queue_key, fresh_queue, move |queue| {
        let now = now_unix();
        queue.schema = QUEUE_SCHEMA.into();
        queue
            .waiting
            .retain(|_, waiting| now.saturating_sub(waiting.created_unix) < WAIT_TTL);
        if queue.waiting.len() >= MAX_WAITING {
            return (false, Err(Vec::<String>::new()));
        }
        queue.waiting.insert(
            run_id.clone(),
            Waiting {
                computer: name.clone(),
                created_unix: now,
            },
        );
        queue.runs.insert(run_id.clone(), now);
        // The runs past their keeping go, their objects after this write.
        let mut gone: Vec<String> = queue
            .runs
            .iter()
            .filter(|(id, at)| {
                now.saturating_sub(**at) > KEEP_FINISHED && !queue.waiting.contains_key(*id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        let mut kept: Vec<(String, u64)> = queue
            .runs
            .iter()
            .filter(|(id, _)| !gone.contains(id))
            .map(|(id, at)| (id.clone(), *at))
            .collect();
        kept.sort_by_key(|(_, at)| *at);
        while kept.len() > MAX_RUNS {
            let (oldest, _) = kept.remove(0);
            if oldest == run_id {
                break;
            }
            gone.push(oldest);
        }
        for id in &gone {
            queue.runs.remove(id);
        }
        (true, Ok(gone))
    })
    .await?;
    match queued {
        Ok(gone) => {
            for old in gone {
                if let Ok(key) = run_key(owner, &old)
                    && let Ok(Some((_, generation))) = store.read_key(&key).await
                {
                    let _ = store.delete_key(&key, &generation).await;
                }
            }
            Ok(Ok(id))
        }
        Err(_) => {
            if let Ok(Some((_, generation))) = store.read_key(&run_key(owner, &id)?).await {
                let _ = store.delete_key(&run_key(owner, &id)?, &generation).await;
            }
            Ok(Err(Refused::Busy))
        }
    }
}

/// Stop a run: a waiting one is cancelled at once; a running one is told
/// at its next report. `None` when there is no such run.
pub(crate) async fn cancel(store: &Store, owner: &str, id: &str) -> Result<Option<()>, Error> {
    update_run(store, owner, id, |run| {
        if run.state.finished() || run.cancel {
            return (false, ());
        }
        run.cancel = true;
        if run.state == RunState::Waiting {
            run.state = RunState::Cancelled;
            run.why = Some("The run was cancelled before it started.".into());
        }
        (true, ())
    })
    .await
}

fn view(run: &Run, after: usize) -> serde_json::Value {
    let after = after.min(run.lines.len());
    json!({
        "id": run.id,
        "state": run.state.word(),
        "lines": run.lines[after..],
        "next": run.lines.len(),
        "answer": run.answer,
        "usage": run.usage,
        "why": run.why,
        "limited": run.limited,
    })
}

// ------------------------------------------------------------------- tokens

/// The gateway's token (`--own-runs-token PRIVATE_FILE`): read when asked,
/// kept a minute, so a file the gateway writes after this server started
/// is found.
pub struct Token {
    path: PathBuf,
    cached: Mutex<Option<([u8; 32], Instant)>>,
}

impl Token {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            cached: Mutex::new(None),
        }
    }

    fn digest(&self) -> Option<[u8; 32]> {
        if let Ok(cached) = self.cached.lock()
            && let Some((digest, at)) = *cached
            && at.elapsed() < Duration::from_secs(60)
        {
            return Some(digest);
        }
        let text = std::fs::read_to_string(&self.path).ok()?;
        let token = text.trim();
        if token.len() < 32 {
            return None;
        }
        let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        if let Ok(mut cached) = self.cached.lock() {
            *cached = Some((digest, Instant::now()));
        }
        Some(digest)
    }

    /// Whether `sent` is the token, compared in constant time.
    fn matches(&self, sent: &str) -> bool {
        let Some(expected) = self.digest() else {
            return false;
        };
        let sent: [u8; 32] = Sha256::digest(sent.as_bytes()).into();
        expected
            .iter()
            .zip(sent.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
    }
}

fn token_slot() -> &'static OnceLock<Token> {
    static TOKEN: OnceLock<Token> = OnceLock::new();
    &TOKEN
}

/// Set the gateway's token file, once, at start.
pub fn set_token(path: PathBuf) {
    let _ = token_slot().set(Token::new(path));
}

/// The gateway's request, from loopback with its token, or the refusal.
fn gateway(headers: &HeaderMap) -> Result<(), Response> {
    let not_found = || refused(StatusCode::NOT_FOUND, "not_found", "Not found.");
    let Some(token) = token_slot().get() else {
        return Err(not_found());
    };
    if !crate::local_request(headers) {
        return Err(not_found());
    }
    let sent = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    if sent.is_empty() || !token.matches(sent) {
        return Err(not_found());
    }
    Ok(())
}

/// The chat owner of the account the gateway names.
fn owner_of(account: &str) -> Result<String, Response> {
    if !plain_id(account) {
        return Err(refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Name the account.",
        ));
    }
    Ok(account_owner(account))
}

// ------------------------------------------------------------------- routes

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Accounts {
    #[serde(default)]
    accounts: Vec<Account>,
}

async fn take_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(name): Path<String>,
    body: Bytes,
) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let computer = line(&name, 64);
    let Some(accounts) = serde_json::from_slice::<Accounts>(&body)
        .ok()
        .filter(|sent| sent.accounts.len() <= MAX_ACCOUNTS && !computer.is_empty())
        .map(|sent| {
            sent.accounts
                .into_iter()
                .filter_map(Account::checked)
                .collect::<Vec<_>>()
        })
    else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {accounts} with at most 8 accounts.",
        );
    };
    let store = &app.config.chat_store;
    if let Err(error) = report_capacity(store, &owner, &computer, accounts).await {
        return stored(&error);
    }
    match take(store, &owner, &computer).await {
        Ok(runs) => answer(StatusCode::OK, json!({"runs": runs})),
        Err(error) => stored(&error),
    }
}

async fn report_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path((name, id)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    if !valid_run_id(&id) {
        return refused(StatusCode::NOT_FOUND, "unknown", "There is no such run.");
    }
    let Ok(sent) = serde_json::from_slice::<Report>(&body) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {lines, done?, failed?}.",
        );
    };
    let computer = line(&name, 64);
    match report(&app.config.chat_store, &owner, &computer, &id, sent).await {
        Ok(Some(cancel)) => answer(StatusCode::OK, json!({"cancel": cancel})),
        Ok(None) => refused(StatusCode::NOT_FOUND, "unknown", "There is no such run."),
        Err(error) => stored(&error),
    }
}

#[derive(Deserialize)]
struct AccountQuery {
    #[serde(default)]
    account: String,
    #[serde(default)]
    after: usize,
    #[serde(default)]
    wait: u64,
    /// A read that also counts the run being taken as news.
    #[serde(default)]
    taken: u8,
}

async fn capacity_route(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<AccountQuery>,
) -> Response {
    if let Err(response) = gateway(&headers) {
        return response;
    }
    let owner = match owner_of(&query.account) {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match capacity(&app.config.chat_store, &owner).await {
        Ok(accounts) => answer(StatusCode::OK, json!({"accounts": accounts})),
        Err(error) => stored(&error),
    }
}

async fn start_route(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    if let Err(response) = gateway(&headers) {
        return response;
    }
    let Ok(sent) = serde_json::from_slice::<Start>(&body) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {account, computer, run_account, agent, brief}.",
        );
    };
    let owner = match owner_of(&sent.account) {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match start(&app.config.chat_store, &owner, sent).await {
        Ok(Ok(id)) => answer(StatusCode::CREATED, json!({"id": id})),
        Ok(Err(Refused::Offline)) => refused(
            StatusCode::CONFLICT,
            "offline",
            "That computer isn't reporting that account now.",
        ),
        Ok(Err(Refused::Busy)) => refused(
            StatusCode::CONFLICT,
            "busy",
            "Too many runs are waiting for this account's computers.",
        ),
        Ok(Err(Refused::Invalid(message))) => refused(StatusCode::BAD_REQUEST, "invalid", message),
        Err(error) => stored(&error),
    }
}

async fn read_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<AccountQuery>,
) -> Response {
    if let Err(response) = gateway(&headers) {
        return response;
    }
    let owner = match owner_of(&query.account) {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    if !valid_run_id(&id) {
        return refused(StatusCode::NOT_FOUND, "unknown", "There is no such run.");
    }
    let store = &app.config.chat_store;
    let until = Instant::now() + Duration::from_secs(query.wait.min(MAX_WAIT));
    loop {
        let run = match load_run(store, &owner, &id).await {
            Ok(Some(run)) => run,
            Ok(None) => {
                return refused(StatusCode::NOT_FOUND, "unknown", "There is no such run.");
            }
            Err(error) => return stored(&error),
        };
        if run.state == RunState::Running
            && now_unix().saturating_sub(run.updated_unix) > STALE_SECONDS
        {
            let stopped = update_run(store, &owner, &id, |run| {
                if run.state != RunState::Running {
                    return (false, ());
                }
                run.state = RunState::Failed;
                run.why = Some("Coder on the computer stopped answering.".into());
                (true, ())
            })
            .await;
            if let Err(error) = stopped {
                return stored(&error);
            }
            continue;
        }
        let news = run.lines.len() > query.after
            || run.state.finished()
            || (query.taken == 1 && run.state != RunState::Waiting);
        if news || Instant::now() >= until {
            return answer(StatusCode::OK, view(&run, query.after));
        }
        tokio::time::sleep(LOOK_EVERY).await;
    }
}

async fn cancel_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<AccountQuery>,
) -> Response {
    if let Err(response) = gateway(&headers) {
        return response;
    }
    let owner = match owner_of(&query.account) {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    if !valid_run_id(&id) {
        return refused(StatusCode::NOT_FOUND, "unknown", "There is no such run.");
    }
    match cancel(&app.config.chat_store, &owner, &id).await {
        Ok(Some(())) => answer(StatusCode::OK, json!({"cancelled": true})),
        Ok(None) => refused(StatusCode::NOT_FOUND, "unknown", "There is no such run."),
        Err(error) => stored(&error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        (dir, store)
    }

    fn codex(free: u32) -> Account {
        Account {
            id: "codex".into(),
            label: "Codex".into(),
            agent: Agent::Codex,
            free_sessions: free,
        }
    }

    fn brief() -> Brief {
        Brief {
            instructions: None,
            history: Vec::new(),
            task: "fix the login bug".into(),
        }
    }

    #[tokio::test]
    async fn a_run_goes_from_the_gateway_to_the_computer_and_back() {
        let (_dir, store) = store();
        let owner = account_owner("acct_owner");
        report_capacity(&store, &owner, "Chris's Studio", vec![codex(2)])
            .await
            .unwrap();
        let linked = capacity(&store, &owner).await.unwrap();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].computer_label, "Chris's Studio");
        assert!(plain_id(&linked[0].computer));
        let id = start(
            &store,
            &owner,
            Start {
                account: "acct_owner".into(),
                computer: linked[0].computer.clone(),
                run_account: "codex".into(),
                agent: Agent::Codex,
                brief: brief(),
            },
        )
        .await
        .unwrap()
        .unwrap();
        // One run queued: one fewer free session offered.
        assert_eq!(capacity(&store, &owner).await.unwrap()[0].free_sessions, 1);
        // Another computer takes nothing; this one takes it once.
        assert!(take(&store, &owner, "Laptop").await.unwrap().is_empty());
        let taken = take(&store, &owner, "Chris's Studio").await.unwrap();
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].brief.task, "fix the login bug");
        assert!(
            take(&store, &owner, "Chris's Studio")
                .await
                .unwrap()
                .is_empty()
        );
        let cancel = report(
            &store,
            &owner,
            "Chris's Studio",
            &id,
            Report {
                lines: vec!["Reading the code.".into()],
                ..Report::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(cancel, Some(false));
        report(
            &store,
            &owner,
            "Chris's Studio",
            &id,
            Report {
                done: Some(Done {
                    text: "Opened the pull request.".into(),
                    input_tokens: Some(10),
                    output_tokens: Some(5),
                }),
                ..Report::default()
            },
        )
        .await
        .unwrap();
        let run = load_run(&store, &owner, &id).await.unwrap().unwrap();
        assert_eq!(run.state, RunState::Done);
        assert_eq!(run.lines, ["Reading the code."]);
        assert_eq!(run.answer.as_deref(), Some("Opened the pull request."));
        // Another computer can't report on it.
        assert_eq!(
            report(&store, &owner, "Laptop", &id, Report::default())
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn an_unknown_or_quiet_computer_is_offline_and_a_cancel_reaches_coder() {
        let (_dir, store) = store();
        let owner = account_owner("acct_owner");
        let refused = start(
            &store,
            &owner,
            Start {
                account: "acct_owner".into(),
                computer: computer_id("Studio"),
                run_account: "codex".into(),
                agent: Agent::Codex,
                brief: brief(),
            },
        )
        .await
        .unwrap();
        assert_eq!(refused, Err(Refused::Offline));
        report_capacity(&store, &owner, "Studio", vec![codex(1)])
            .await
            .unwrap();
        let id = start(
            &store,
            &owner,
            Start {
                account: "acct_owner".into(),
                computer: computer_id("Studio"),
                run_account: "codex".into(),
                agent: Agent::Codex,
                brief: brief(),
            },
        )
        .await
        .unwrap()
        .unwrap();
        take(&store, &owner, "Studio").await.unwrap();
        cancel(&store, &owner, &id).await.unwrap();
        let told = report(&store, &owner, "Studio", &id, Report::default())
            .await
            .unwrap();
        assert_eq!(told, Some(true));
        // Another account sees none of it.
        let other = account_owner("acct_other");
        assert!(capacity(&store, &other).await.unwrap().is_empty());
        assert!(load_run(&store, &other, &id).await.unwrap().is_none());
    }

    #[test]
    fn run_ids_and_paths_are_checked() {
        assert!(valid_run_id(&new_run_id()));
        assert!(!valid_run_id("run../x"));
        assert!(owns("/v1/own-runs") && owns("/v1/own-runs/capacity"));
        assert!(!owns("/v1/own-runsx"));
        assert_ne!(computer_id("Studio"), computer_id("Laptop"));
    }
}
