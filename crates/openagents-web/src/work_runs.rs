//! Work on an issue (#11258): "work on issue N in repository R" with the
//! briefed agent (#11211), the default engine everywhere a person asks for
//! an issue to be worked.
//!
//! The website keeps the requests; work hosts (cloud environments from the
//! `oa-coder-host` image, `scripts/work/worker.py`) take them, run the
//! engine (`scripts/work/work_issue.py`: the briefing, the briefed agent
//! with `verify`, the replayed checks, the fallback to bare Claude Code)
//! and report progress and the result back. The website never reaches a
//! host.
//!
//! | Route | Who | What |
//! | --- | --- | --- |
//! | `POST /v1/work` `{repo, issue, land?, engine?}` | The account | Ask for issue `issue` of `repo` to be worked: `201 {id, state, url, events}`; `land` is `pr` (default), `queue` (the landing queue) or `none`; `engine` is `briefed` (default) or `bare` |
//! | `GET /v1/work` | The account | `{runs: [View]}`, newest first |
//! | `GET /v1/work/{id}?after=N` | The account | One run, its progress from line `after` |
//! | `GET /v1/work/{id}/events` | The account | Server-sent events: `progress` lines, `state` changes, then `result` |
//! | `POST /v1/work/{id}/cancel` | The account | Stop the run |
//! | `POST /v1/work-hosts/{name}/claim` | A work host | The next waiting run with the requester's own Claude sign-in, or `{run: null}` |
//! | `POST /v1/work-hosts/{name}/runs/{id}` `{lines?, result?, failed?}` | A work host | Progress, then the result; answers `{cancel}` |
//!
//! The account routes take the app's own token (`Authorization: Bearer
//! sess_…`) or the site's sign-in cookie, and only the people allowed agent
//! work ([`crate::agent_work`]) may ask. A host proves itself with the work
//! host token (`OPENAGENTS_WORK_HOST_TOKEN`).
//!
//! **Whose Claude.** A run uses the requester's own Claude sign-in, saved
//! in Settings > Claude: the host gets it when it takes the run, once, and
//! keeps it only in that run's process. Without one the run stops and says
//! so; the server's own key is never used. One account's runs take turns
//! (a Claude plan runs one automated turn at a time).
//!
//! Records live in the chat store: one object per run (`work-runs/runs/`),
//! the waiting and running runs (`work-runs/queue.json`), and each
//! account's list of its runs.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color};
use openagents_ui::content::PageColumn;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::App;
use crate::account::Account;
use crate::chat_store::{Error, Store, account_owner, now_unix};
use crate::cloud::protect;
use crate::cloud::session::{CloudSession, Viewer};
use crate::coder_sync::{answer, line, refused, stored};
use crate::phone_api::Item;
use crate::ui_page::UiPage;

/// The account's API routes.
pub(crate) const PREFIX: &str = "/v1/work";
/// The work hosts' routes.
pub(crate) const HOSTS: &str = "/v1/work-hosts";
/// The pages.
pub(crate) const PAGE: &str = "/work";
/// The runs waiting and running, for every account.
const QUEUE_KEY: &str = "work-runs/queue.json";
/// Each account's runs, under its folder.
const INDEX_KEY: &str = "work-runs/index.json";
const RUN_SCHEMA: &str = "openagents.web.work-run.v1";
const QUEUE_SCHEMA: &str = "openagents.web.work-runs.queue.v1";
const INDEX_SCHEMA: &str = "openagents.web.work-runs.index.v1";

/// The most runs waiting at once, for everyone and for one account.
const MAX_WAITING: usize = 64;
const MAX_WAITING_EACH: usize = 8;
/// The most runs an account keeps (the oldest finished go first).
const MAX_RUNS: usize = 64;
/// A run no host took in this long is cancelled.
const WAIT_TTL: u64 = 6 * 3600;
/// A running run whose host said nothing for this long has stopped (a host
/// reports at least every 30 seconds while it runs).
const STALE_SECONDS: u64 = 600;
/// The most progress lines a run keeps, and one report carries.
const MAX_LINES: usize = 600;
const MAX_REPORT_LINES: usize = 200;
/// The longest a read waits for news.
const MAX_WAIT: u64 = 25;
/// How often an event stream looks again.
const LOOK_EVERY: Duration = Duration::from_secs(2);
/// How often a page with a run going reads itself again, in seconds.
const REFRESH_SECONDS: u32 = 4;
/// Finished runs shown on the boards this long.
const BOARD_SECONDS: u64 = 24 * 3600;
/// The board the runs show on, beside the account's computers.
pub(crate) const BOARD: &str = "OpenAgents Cloud";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PREFIX, post(submit_route).get(list_route))
        .route("/v1/work/{id}", get(read_route))
        .route("/v1/work/{id}/events", get(events_route))
        .route("/v1/work/{id}/cancel", post(cancel_route))
        .route("/v1/work-hosts/{name}/claim", post(claim_route))
        .route("/v1/work-hosts/{name}/runs/{id}", post(report_route))
        .route(PAGE, get(list_page).post(submit_page))
        .route("/work/{id}", get(run_page))
        .route("/work/{id}/stop", post(stop_page))
        .route(OFFER, get(offer_route))
}

/// A chat reply's "Work on this issue" card, loaded into the thread.
const OFFER: &str = "/chat/{id}/work-offer/{index}";
/// The route the chat's typed router names for work on code.
const WORK_ROUTE: &str = "work.dispatch";

/// Whether `path` is one of the API routes here.
pub(crate) fn owns(path: &str) -> bool {
    [PREFIX, HOSTS].iter().any(|prefix| {
        path.strip_prefix(prefix)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    })
}

// ------------------------------------------------------------------ records

/// How a green change lands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Land {
    /// Push a branch and open a pull request.
    #[default]
    Pr,
    /// Hand the branch to the landing queue (docs/cloud/land-queue.md).
    Queue,
    /// Commit only, on the host.
    None,
}

impl Land {
    pub(crate) fn parse(word: &str) -> Option<Self> {
        match word.trim() {
            "pr" | "pull_request" | "pull-request" | "" => Some(Self::Pr),
            "queue" | "main" => Some(Self::Queue),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Pr => "pr",
            Self::Queue => "queue",
            Self::None => "none",
        }
    }

    fn words(self) -> &'static str {
        match self {
            Self::Pr => "Opens a pull request",
            Self::Queue => "Lands through the landing queue",
            Self::None => "Commits only",
        }
    }
}

/// Which engine works the issue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Engine {
    /// The briefed agent (#11211), falling back to bare Claude Code.
    #[default]
    Briefed,
    /// Bare Claude Code, told "Complete this issue."
    Bare,
}

impl Engine {
    pub(crate) fn parse(word: &str) -> Option<Self> {
        match word.trim() {
            "briefed" | "" => Some(Self::Briefed),
            "bare" | "claude" => Some(Self::Bare),
            _ => None,
        }
    }

    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Briefed => "briefed",
            Self::Bare => "bare",
        }
    }
}

/// The engine's name as a person reads it.
fn engine_words(word: &str) -> &'static str {
    match word {
        "briefed" => "Briefed agent",
        "bare" => "Claude Code",
        _ => "Not started",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RunState {
    /// Waiting for a work host.
    Waiting,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl RunState {
    pub(crate) fn finished(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }

    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn words(self) -> &'static str {
        match self {
            Self::Waiting => "Waiting for a computer",
            Self::Running => "Working",
            Self::Done => "Done",
            Self::Failed => "Stopped",
            Self::Cancelled => "Cancelled",
        }
    }
}

/// Whose Claude sign-in a run uses: the requester's account and workspace,
/// as Settings > Claude saved it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Requester {
    pub account: String,
    pub workspace: String,
    pub members_epoch: u64,
}

/// One progress line from the host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Progress {
    /// Seconds since the run started on its host.
    #[serde(default)]
    pub secs: f64,
    #[serde(default)]
    pub phase: String,
    pub text: String,
}

/// One replayed check on the change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Check {
    pub name: String,
    pub ok: bool,
    #[serde(default)]
    pub passed: Option<u64>,
}

/// What the engine did, as the host reported it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Outcome {
    /// `briefed` or `bare`: the engine whose change was judged.
    #[serde(default)]
    pub engine: String,
    /// Why the briefed agent handed over to bare Claude Code.
    #[serde(default)]
    pub escalated: Option<String>,
    #[serde(default)]
    pub ok: bool,
    /// What Claude Code reported, at list price; `None` when it reported
    /// nothing (unknown, never zero).
    #[serde(default)]
    pub cost_usd: Option<f64>,
    #[serde(default)]
    pub secs: Option<f64>,
    #[serde(default)]
    pub checks: Vec<Check>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub added: u64,
    #[serde(default)]
    pub removed: u64,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub pr: Option<String>,
    /// The landing queue's entry.
    #[serde(default)]
    pub landing: Option<String>,
    #[serde(default)]
    pub briefing_files: Vec<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct WorkRun {
    schema: String,
    pub id: String,
    pub owner: String,
    pub requester: Requester,
    pub repo: String,
    pub issue: u64,
    #[serde(default)]
    pub title: Option<String>,
    pub land: Land,
    pub engine: Engine,
    /// Where it was asked for: `web`, `api`, `chat`, `github`, `fleet`,
    /// `cli`.
    pub source: String,
    pub created_unix: u64,
    pub updated_unix: u64,
    pub state: RunState,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub started_unix: Option<u64>,
    #[serde(default)]
    pub lines: Vec<Progress>,
    #[serde(default)]
    pub dropped: usize,
    #[serde(default)]
    pub outcome: Option<Outcome>,
    #[serde(default)]
    pub why: Option<String>,
    #[serde(default)]
    pub cancel: bool,
    #[serde(default)]
    pub finished_unix: Option<u64>,
}

impl WorkRun {
    pub(crate) fn title(&self) -> String {
        match &self.title {
            Some(title) => format!("{}#{} {title}", self.repo, self.issue),
            None => format!("{}#{}", self.repo, self.issue),
        }
    }

    /// The run's own words for its last news.
    pub(crate) fn last_line(&self) -> Option<String> {
        self.why
            .clone()
            .or_else(|| self.lines.last().map(|line| line.text.clone()))
    }

    fn end(&mut self, state: RunState, why: Option<String>) {
        self.state = state;
        self.why = why;
        self.finished_unix = Some(now_unix());
    }

    /// Settle a run no host took, or whose host went quiet. Returns whether
    /// it changed.
    fn settle(&mut self, now: u64) -> bool {
        match self.state {
            RunState::Waiting if now.saturating_sub(self.created_unix) > WAIT_TTL => {
                self.end(
                    RunState::Cancelled,
                    Some("No computer took this run in time.".into()),
                );
                true
            }
            RunState::Running if now.saturating_sub(self.updated_unix) > STALE_SECONDS => {
                self.end(
                    RunState::Failed,
                    Some("The computer working on it stopped answering.".into()),
                );
                true
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Queued {
    owner: String,
    created_unix: u64,
    /// The order runs were asked for (several can share a second).
    #[serde(default)]
    seq: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Queue {
    #[serde(default)]
    schema: String,
    #[serde(default)]
    waiting: BTreeMap<String, Queued>,
    /// Runs a host took and hasn't finished: id to owner.
    #[serde(default)]
    running: BTreeMap<String, String>,
    /// The next run's place in line.
    #[serde(default)]
    next: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Index {
    #[serde(default)]
    schema: String,
    /// Every run kept, id to when it was asked for.
    #[serde(default)]
    runs: BTreeMap<String, u64>,
}

fn run_key(id: &str) -> String {
    format!("work-runs/runs/{id}.json")
}

/// A run's id: `wrk` and 24 hex digits.
pub(crate) fn valid_id(id: &str) -> bool {
    id.len() == 27
        && id.starts_with("wrk")
        && id[3..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn new_id() -> String {
    let bytes: [u8; 12] = secp256k1::rand::random();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("wrk{hex}")
}

/// `owner/name`: GitHub's letters, digits, `-`, `_`, `.`.
pub(crate) fn valid_repo(repo: &str) -> bool {
    let mut parts = repo.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let fine = |part: &str, most: usize| {
        !part.is_empty()
            && part.len() <= most
            && !part.starts_with('.')
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    fine(owner, 39) && fine(name, 100)
}

/// The repository and issue number a GitHub issue address names
/// (`https://github.com/OWNER/NAME/issues/N`), or `OWNER/NAME#N`. Read only
/// once the request is already a work request: a bounded id field.
pub(crate) fn issue_address(text: &str) -> Option<(String, u64)> {
    let text = text.trim().trim_end_matches('/');
    if let Some((repo, number)) = text.split_once('#')
        && valid_repo(repo)
    {
        return number
            .parse()
            .ok()
            .filter(|n| *n > 0)
            .map(|n| (repo.to_owned(), n));
    }
    let rest = text
        .strip_prefix("https://github.com/")
        .or_else(|| text.strip_prefix("http://github.com/"))
        .or_else(|| text.strip_prefix("github.com/"))?;
    let mut parts = rest.split('/');
    let (owner, name, kind, number) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let repo = format!("{owner}/{name}");
    let number: u64 = number.split(['?', '#']).next()?.parse().ok()?;
    (kind == "issues" && valid_repo(&repo) && number > 0).then_some((repo, number))
}

/// Read, change, and write one JSON record at `key` under compare-and-swap.
async fn update<R, T>(
    store: &Store,
    key: &str,
    fresh: impl Fn() -> R,
    change: impl Fn(&mut R) -> (bool, T),
) -> Result<T, Error>
where
    R: Serialize + for<'de> Deserialize<'de>,
{
    for _ in 0..8 {
        let (mut record, generation) = match store.read_key(key).await? {
            Some((bytes, generation)) => (
                serde_json::from_slice::<R>(&bytes)
                    .map_err(|_| Error::Corrupt("A work run record is invalid."))?,
                Some(generation),
            ),
            None => (fresh(), None),
        };
        let (changed, result) = change(&mut record);
        if !changed {
            return Ok(result);
        }
        let bytes = serde_json::to_vec(&record)
            .map_err(|_| Error::Invalid("A work run record is invalid."))?;
        match store.write_key(key, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(result),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

fn fresh_queue() -> Queue {
    Queue {
        schema: QUEUE_SCHEMA.into(),
        ..Queue::default()
    }
}

fn fresh_index() -> Index {
    Index {
        schema: INDEX_SCHEMA.into(),
        ..Index::default()
    }
}

/// Change run `id` with `change`; `None` when there is no such run.
async fn update_run<T>(
    store: &Store,
    id: &str,
    change: impl Fn(&mut WorkRun) -> (bool, T),
) -> Result<Option<T>, Error> {
    let key = run_key(id);
    for _ in 0..8 {
        let Some((bytes, generation)) = store.read_key(&key).await? else {
            return Ok(None);
        };
        let mut run: WorkRun =
            serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt("A work run is invalid."))?;
        let (changed, result) = change(&mut run);
        if !changed {
            return Ok(Some(result));
        }
        run.updated_unix = now_unix();
        let bytes =
            serde_json::to_vec(&run).map_err(|_| Error::Invalid("A work run is invalid."))?;
        match store.write_key(&key, bytes, Some(&generation)).await {
            Ok(_) => return Ok(Some(result)),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

async fn load_plain(store: &Store, id: &str) -> Result<Option<WorkRun>, Error> {
    if !valid_id(id) {
        return Ok(None);
    }
    match store.read_key(&run_key(id)).await? {
        Some((bytes, _)) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| Error::Corrupt("A work run is invalid.")),
        None => Ok(None),
    }
}

/// Take a finished run off the queue.
async fn unqueue(store: &Store, id: &str) {
    let id = id.to_owned();
    let _ = update(store, QUEUE_KEY, fresh_queue, move |queue| {
        let gone = queue.waiting.remove(&id).is_some() | queue.running.remove(&id).is_some();
        (gone, ())
    })
    .await;
}

/// Run `id` of `owner`, settled; `None` when there is none or it is
/// another account's.
pub(crate) async fn load(store: &Store, owner: &str, id: &str) -> Result<Option<WorkRun>, Error> {
    let Some(run) = load_plain(store, id).await? else {
        return Ok(None);
    };
    if run.owner != owner {
        return Ok(None);
    }
    if run.clone().settle(now_unix()) {
        update_run(store, id, |run| (run.settle(now_unix()), ())).await?;
        unqueue(store, id).await;
        return load_plain(store, id).await;
    }
    Ok(Some(run))
}

/// What a person asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Ask {
    pub repo: String,
    pub issue: u64,
    pub land: Land,
    pub engine: Engine,
    pub source: String,
    pub title: Option<String>,
}

/// Why a request was refused.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Refused {
    /// Too many runs waiting.
    Busy,
    Invalid(&'static str),
}

/// Ask for a run. The run waits for a host.
pub(crate) async fn submit(
    store: &Store,
    owner: &str,
    requester: Requester,
    ask: Ask,
) -> Result<Result<WorkRun, Refused>, Error> {
    if !valid_repo(&ask.repo) {
        return Ok(Err(Refused::Invalid(
            "Name the repository as owner/name, for example OpenAgentsInc/openagents.",
        )));
    }
    if ask.issue == 0 || ask.issue > 100_000_000 {
        return Ok(Err(Refused::Invalid("Give the issue's number.")));
    }
    let now = now_unix();
    let id = new_id();
    let run = WorkRun {
        schema: RUN_SCHEMA.into(),
        id: id.clone(),
        owner: owner.to_owned(),
        requester,
        repo: ask.repo,
        issue: ask.issue,
        title: ask.title.map(|title| line(&title, 200)),
        land: ask.land,
        engine: ask.engine,
        source: line(&ask.source, 16),
        created_unix: now,
        updated_unix: now,
        state: RunState::Waiting,
        host: None,
        started_unix: None,
        lines: Vec::new(),
        dropped: 0,
        outcome: None,
        why: None,
        cancel: false,
        finished_unix: None,
    };
    let bytes = serde_json::to_vec(&run).map_err(|_| Error::Invalid("A work run is invalid."))?;
    store.write_key(&run_key(&id), bytes, None).await?;
    let (run_id, who) = (id.clone(), owner.to_owned());
    let queued = update(store, QUEUE_KEY, fresh_queue, move |queue| {
        queue.schema = QUEUE_SCHEMA.into();
        let theirs = queue.waiting.values().filter(|q| q.owner == who).count();
        if queue.waiting.len() >= MAX_WAITING || theirs >= MAX_WAITING_EACH {
            return (false, false);
        }
        queue.next += 1;
        queue.waiting.insert(
            run_id.clone(),
            Queued {
                owner: who.clone(),
                created_unix: now,
                seq: queue.next,
            },
        );
        (true, true)
    })
    .await?;
    if !queued {
        if let Ok(Some((_, generation))) = store.read_key(&run_key(&id)).await {
            let _ = store.delete_key(&run_key(&id), &generation).await;
        }
        return Ok(Err(Refused::Busy));
    }
    let index_key = Store::owner_key(owner, INDEX_KEY)?;
    let run_id = id.clone();
    let gone = update(store, &index_key, fresh_index, move |index| {
        index.schema = INDEX_SCHEMA.into();
        index.runs.insert(run_id.clone(), now);
        let mut gone = Vec::new();
        while index.runs.len() > MAX_RUNS {
            let Some(oldest) = index
                .runs
                .iter()
                .min_by_key(|(_, at)| **at)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            index.runs.remove(&oldest);
            gone.push(oldest);
        }
        (true, gone)
    })
    .await?;
    for old in gone {
        if let Ok(Some((_, generation))) = store.read_key(&run_key(&old)).await {
            let _ = store.delete_key(&run_key(&old), &generation).await;
        }
    }
    Ok(Ok(run))
}

/// The account's runs, newest first, settled.
pub(crate) async fn list(store: &Store, owner: &str) -> Result<Vec<WorkRun>, Error> {
    let key = Store::owner_key(owner, INDEX_KEY)?;
    let index: Index = match store.read_key(&key).await? {
        Some((bytes, _)) => serde_json::from_slice(&bytes).unwrap_or_default(),
        None => return Ok(Vec::new()),
    };
    let mut ids: Vec<(String, u64)> = index.runs.into_iter().collect();
    ids.sort_by_key(|(_, at)| std::cmp::Reverse(*at));
    let mut runs = Vec::new();
    for (id, _) in ids {
        if let Some(run) = load(store, owner, &id).await? {
            runs.push(run);
        }
    }
    Ok(runs)
}

/// Stop a run: a waiting one at once, a running one at its host's next
/// report. `None` when there is no such run of `owner`.
pub(crate) async fn cancel(store: &Store, owner: &str, id: &str) -> Result<Option<()>, Error> {
    if load(store, owner, id).await?.is_none() {
        return Ok(None);
    }
    let waiting = update_run(store, id, |run| {
        if run.state.finished() || run.cancel {
            return (false, false);
        }
        run.cancel = true;
        if run.state == RunState::Waiting {
            run.end(
                RunState::Cancelled,
                Some("Cancelled before it started.".into()),
            );
            return (true, true);
        }
        (true, false)
    })
    .await?;
    if waiting == Some(true) {
        unqueue(store, id).await;
    }
    Ok(waiting.map(|_| ()))
}

// ------------------------------------------------------------- the hosts'

/// A run handed to a host, with the requester's Claude sign-in as the
/// environment Claude Code reads.
pub(crate) struct Taken {
    pub run: WorkRun,
    pub env: BTreeMap<String, String>,
}

/// Releases a requester's own Claude sign-in for one run, as the variables
/// Claude Code reads; `Err` says why it can't.
pub(crate) trait Credentials: Sync {
    fn release(&self, requester: &Requester) -> Result<BTreeMap<String, String>, String>;
}

/// The Settings > Claude store (`cloud::byo`).
pub(crate) struct Saved<'a>(pub &'a crate::cloud::byo::Computers);

impl Credentials for Saved<'_> {
    fn release(&self, requester: &Requester) -> Result<BTreeMap<String, String>, String> {
        let owner = crate::cloud::byo::Owner {
            account: requester.account.clone(),
            workspace: requester.workspace.clone(),
            members_epoch: requester.members_epoch,
        };
        let (class, mut value) = self.0.release(&owner, now_unix())?.ok_or_else(|| {
            "Save your own Claude sign-in in Settings > Claude, then ask again.".to_owned()
        })?;
        let env = class
            .environment(&value)
            .map_err(|_| "Your saved Claude sign-in can't be used. Save it again.".to_owned());
        let mut bytes = std::mem::take(&mut value).into_bytes();
        bytes.fill(0);
        env
    }
}

/// The oldest waiting run whose account has none running, taken by `host`
/// with its requester's sign-in. A run whose sign-in can't be used stops
/// and says why.
pub(crate) async fn claim(
    store: &Store,
    credentials: &dyn Credentials,
    host: &str,
) -> Result<Option<Taken>, Error> {
    loop {
        let picked = update(store, QUEUE_KEY, fresh_queue, |queue| {
            let mut waiting: Vec<(&String, &Queued)> = queue.waiting.iter().collect();
            waiting.sort_by_key(|(_, q)| (q.created_unix, q.seq));
            let busy: Vec<&String> = queue.running.values().collect();
            let Some((id, queued)) = waiting
                .into_iter()
                .find(|(_, q)| !busy.contains(&&q.owner))
                .map(|(id, q)| (id.clone(), q.clone()))
            else {
                return (false, None);
            };
            queue.waiting.remove(&id);
            queue.running.insert(id.clone(), queued.owner);
            (true, Some(id))
        })
        .await?;
        let Some(id) = picked else {
            return Ok(None);
        };
        let host = host.to_owned();
        let started = update_run(store, &id, |run| {
            if run.state != RunState::Waiting || run.cancel {
                return (false, None);
            }
            run.state = RunState::Running;
            run.host = Some(host.clone());
            run.started_unix = Some(now_unix());
            (true, Some(run.clone()))
        })
        .await?
        .flatten();
        let Some(run) = started else {
            unqueue(store, &id).await;
            continue;
        };
        match credentials.release(&run.requester) {
            Ok(env) => return Ok(Some(Taken { run, env })),
            Err(why) => {
                update_run(store, &id, |run| {
                    run.end(RunState::Failed, Some(why.clone()));
                    (true, ())
                })
                .await?;
                unqueue(store, &id).await;
            }
        }
    }
}

/// What a host reports for one run.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct Report {
    #[serde(default)]
    pub lines: Vec<Progress>,
    /// The engine's result (`work_issue.py`'s last line).
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub failed: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

/// The engine's result as a run keeps it.
pub(crate) fn outcome(result: &Value) -> Outcome {
    let text = |value: &Value| value.as_str().map(|s| line(s, 500));
    let landed = &result["landed"];
    let checks = result["checks"]
        .as_array()
        .map(|checks| {
            checks
                .iter()
                .take(20)
                .map(|check| Check {
                    name: line(check["name"].as_str().unwrap_or("check"), 120),
                    ok: check["ok"].as_bool().unwrap_or(false),
                    passed: check["passed"].as_u64(),
                })
                .collect()
        })
        .unwrap_or_default();
    let strings = |value: &Value| -> Vec<String> {
        value
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(|s| line(s, 200)))
                    .take(40)
                    .collect()
            })
            .unwrap_or_default()
    };
    Outcome {
        engine: text(&result["engine"]).unwrap_or_default(),
        escalated: text(&result["escalated"]),
        ok: result["ok"].as_bool().unwrap_or(false),
        cost_usd: result["cost_usd"]
            .as_f64()
            .filter(|c| c.is_finite() && *c >= 0.0),
        secs: result["secs"]
            .as_f64()
            .filter(|s| s.is_finite() && *s >= 0.0),
        checks,
        files: strings(&result["diff"]["files"]),
        added: result["diff"]["added"].as_u64().unwrap_or(0),
        removed: result["diff"]["removed"].as_u64().unwrap_or(0),
        commit: text(&result["commit"]).filter(|c| c.bytes().all(|b| b.is_ascii_hexdigit())),
        pr: text(&landed["url"]).filter(|url| url.starts_with("https://github.com/")),
        landing: text(&landed["entry"]["id"]),
        briefing_files: strings(&result["briefing"]["files"]),
        error: text(&result["error"]).map(|e| secret_screen::redact(&e)),
    }
}

/// What a report is answered with.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct Heard {
    pub cancel: bool,
}

/// Keep a host's report on run `id`; `None` when the run isn't that
/// host's.
pub(crate) async fn report(
    store: &Store,
    host: &str,
    id: &str,
    sent: Report,
) -> Result<Option<Heard>, Error> {
    let lines: Vec<Progress> = sent
        .lines
        .into_iter()
        .take(MAX_REPORT_LINES)
        .map(|progress| Progress {
            secs: if progress.secs.is_finite() {
                progress.secs.max(0.0)
            } else {
                0.0
            },
            phase: line(&progress.phase, 24),
            text: secret_screen::redact(&line(&progress.text, 400)),
        })
        .filter(|progress| !progress.text.is_empty())
        .collect();
    let outcome = sent.result.as_ref().map(outcome);
    let failed = sent
        .failed
        .map(|why| secret_screen::redact(&line(&why, 500)));
    let title = sent.title.map(|title| line(&title, 200));
    let host = host.to_owned();
    let heard = update_run(store, id, |run| {
        if run.host.as_deref() != Some(host.as_str()) {
            return (false, None);
        }
        if run.state.finished() {
            return (false, Some(Heard { cancel: true }));
        }
        let mut changed = true;
        for progress in &lines {
            if run.lines.len() >= MAX_LINES {
                run.lines.remove(0);
                run.dropped += 1;
            }
            run.lines.push(progress.clone());
        }
        if run.title.is_none() && title.is_some() {
            run.title.clone_from(&title);
        }
        if let Some(outcome) = &outcome {
            run.outcome = Some(outcome.clone());
            if run.cancel {
                run.end(RunState::Cancelled, Some("Cancelled.".into()));
            } else if outcome.ok {
                run.end(RunState::Done, None);
            } else {
                let why = outcome
                    .error
                    .clone()
                    .unwrap_or_else(|| "No change passed its checks.".into());
                run.end(RunState::Failed, Some(why));
            }
        } else if let Some(why) = &failed {
            run.end(
                if run.cancel {
                    RunState::Cancelled
                } else {
                    RunState::Failed
                },
                Some(why.clone()),
            );
        } else if lines.is_empty() && now_unix().saturating_sub(run.updated_unix) < 20 {
            changed = false;
        }
        (changed, Some(Heard { cancel: run.cancel }))
    })
    .await?
    .flatten();
    if heard.is_some()
        && let Some(run) = load_plain(store, id).await?
        && run.state.finished()
    {
        unqueue(store, id).await;
    }
    Ok(heard)
}

// --------------------------------------------------------------------- views

fn money(cost: Option<f64>) -> String {
    match cost {
        Some(cost) => format!("${cost:.2}"),
        None => "unknown".into(),
    }
}

fn duration(secs: Option<f64>) -> String {
    match secs {
        Some(secs) if secs >= 60.0 => format!("{} min {} s", secs as u64 / 60, secs as u64 % 60),
        Some(secs) => format!("{} s", secs.round() as u64),
        None => "unknown".into(),
    }
}

/// The run as the API answers it, with its progress from `after`.
pub(crate) fn view(run: &WorkRun, after: usize) -> Value {
    let start = after.saturating_sub(run.dropped).min(run.lines.len());
    json!({
        "id": run.id,
        "repo": run.repo,
        "issue": run.issue,
        "title": run.title,
        "land": run.land.word(),
        "engine_requested": run.engine.word(),
        "source": run.source,
        "state": run.state.word(),
        "host": run.host,
        "created_unix": run.created_unix,
        "started_unix": run.started_unix,
        "updated_unix": run.updated_unix,
        "finished_unix": run.finished_unix,
        "why": run.why,
        "result": run.outcome.as_ref().map(|o| json!({
            "engine": o.engine,
            "escalated": o.escalated,
            "ok": o.ok,
            "cost_usd": o.cost_usd,
            "secs": o.secs,
            "checks": o.checks,
            "files": o.files,
            "added": o.added,
            "removed": o.removed,
            "commit": o.commit,
            "pr": o.pr,
            "landing": o.landing,
            "briefing_files": o.briefing_files,
        })),
        "lines": run.lines[start..],
        "next": run.dropped + run.lines.len(),
        "url": format!("{PAGE}/{}", run.id),
        "events": format!("{PREFIX}/{}/events", run.id),
    })
}

/// What a finished run did, as one message for the chat it started from.
pub(crate) fn said(run: &WorkRun) -> String {
    let name = format!("{}#{}", run.repo, run.issue);
    let Some(outcome) = &run.outcome else {
        return format!(
            "The run on {name} stopped: {}",
            run.why.as_deref().unwrap_or("it didn't finish.")
        );
    };
    let passed = outcome.checks.iter().filter(|c| c.ok).count();
    let mut text = format!(
        "{} {} {name} in {} for {}: checks {passed}/{} passed",
        engine_words(&outcome.engine),
        if outcome.ok { "finished" } else { "stopped on" },
        duration(outcome.secs),
        money(outcome.cost_usd),
        outcome.checks.len(),
    );
    if let Some(pr) = &outcome.pr {
        text.push_str(&format!(", pull request {pr}"));
    } else if let Some(entry) = &outcome.landing {
        text.push_str(&format!(", in the landing queue as {entry}"));
    } else if let Some(commit) = &outcome.commit {
        text.push_str(&format!(
            ", commit {}",
            commit.chars().take(10).collect::<String>()
        ));
    }
    text.push('.');
    if let Some(why) = &outcome.escalated {
        text.push_str(&format!(" Claude Code took over: {why}."));
    }
    if !outcome.ok
        && let Some(why) = &run.why
    {
        text.push_str(&format!(" {why}"));
    }
    text
}

/// The signed-in person of a web request, when they may hand issues to
/// the briefed agent, with the workspace their Claude sign-in is under.
pub(crate) async fn web_requester(app: &App, headers: &HeaderMap) -> Option<Requester> {
    let viewer = app
        .config
        .cloud
        .as_deref()?
        .authenticate(headers)
        .await
        .ok()?;
    crate::agent_work::permitted(app, &viewer)
        .then(|| requester(&viewer))
        .flatten()
}

/// The runs as items on the boards (`GET /v1/agents`), so the phone and
/// the web show them with the account's other agents: the ones going and
/// those that ended in the last day.
pub(crate) async fn board_items(store: &Store, owner: &str) -> Vec<(String, Item)> {
    let Ok(runs) = list(store, owner).await else {
        return Vec::new();
    };
    let now = now_unix();
    runs.iter()
        .filter(|run| {
            !run.state.finished()
                || run
                    .finished_unix
                    .is_some_and(|at| now.saturating_sub(at) < BOARD_SECONDS)
        })
        .take(crate::phone_api::MAX_ITEMS)
        .map(|run| (BOARD.to_owned(), item(run)))
        .collect()
}

/// One run as a board item.
pub(crate) fn item(run: &WorkRun) -> Item {
    let status = match run.state {
        RunState::Waiting | RunState::Running => "working",
        RunState::Done => "done",
        RunState::Failed => "failed",
        RunState::Cancelled => "stopped",
    };
    let engine = run
        .outcome
        .as_ref()
        .map_or(run.engine.word(), |o| o.engine.as_str());
    Item {
        id: run.id.clone(),
        kind: "agent".into(),
        title: line(&run.title(), 120),
        engine: Some(engine_words(engine).into()),
        status: status.into(),
        started_unix: run.created_unix,
        finished_unix: run.finished_unix,
        cost_usd: run.outcome.as_ref().and_then(|o| o.cost_usd),
        tokens: None,
        session: None,
        question: None,
        line: run.last_line().map(|text| line(&text, 200)),
    }
}

/// Whether a board item is a work run's.
pub(crate) fn is_run(item: &str) -> bool {
    valid_id(item)
}

/// The phone's Stop on a run's board item.
pub(crate) async fn act(
    store: &Store,
    owner: &str,
    id: &str,
    action: &str,
) -> Result<Result<(), (StatusCode, &'static str, &'static str)>, Error> {
    if action != "stop" {
        return Ok(Err((
            StatusCode::CONFLICT,
            "unsupported",
            "A work run takes Stop.",
        )));
    }
    Ok(match cancel(store, owner, id).await? {
        Some(()) => Ok(()),
        None => Err((
            StatusCode::NOT_FOUND,
            "unknown",
            "That run isn't there anymore.",
        )),
    })
}

// ---------------------------------------------------------------- the API

/// The signed-in person behind an API request (`Bearer sess_…`) or a
/// browser request (the sign-in cookie), with the workspace their Claude
/// sign-in is saved under.
async fn person(app: &App, headers: &HeaderMap) -> Result<(Viewer, Requester), Response> {
    let Some(service) = app.config.cloud.as_deref() else {
        return Err(refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "This site doesn't offer accounts.",
        ));
    };
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty());
    let signed_out = || {
        refused(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Sign in first: coder login, or sign in on the website.",
        )
    };
    let viewer = match bearer {
        Some(token) => {
            let mut asked = HeaderMap::new();
            if let Some(host) = headers.get(header::HOST) {
                asked.insert(header::HOST, host.clone());
            }
            let cookie = |workspace: Option<&str>| {
                let mut text = format!("oa_cloud_session={token}");
                if let Some(workspace) = workspace {
                    text.push_str("; oa_cloud_workspace=");
                    text.push_str(workspace);
                }
                HeaderValue::from_str(&text).ok()
            };
            asked.insert(header::COOKIE, cookie(None).ok_or_else(signed_out)?);
            let viewer = service
                .authenticate(&asked)
                .await
                .map_err(|_| signed_out())?;
            match (
                &viewer.workspace,
                crate::cloud::default_workspace(&viewer).map(|w| w.id.clone()),
            ) {
                (None, Some(workspace)) => {
                    asked.insert(
                        header::COOKIE,
                        cookie(Some(&workspace)).ok_or_else(signed_out)?,
                    );
                    service
                        .authenticate(&asked)
                        .await
                        .map_err(|_| signed_out())?
                }
                _ => viewer,
            }
        }
        None => service
            .authenticate(headers)
            .await
            .map_err(|_| signed_out())?,
    };
    if !crate::agent_work::permitted(app, &viewer) {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "not_allowed",
            "Working on issues isn't open to this account yet.",
        ));
    }
    let requester = requester(&viewer).ok_or_else(|| {
        refused(
            StatusCode::FORBIDDEN,
            "no_workspace",
            "Pick a workspace on the website first.",
        )
    })?;
    Ok((viewer, requester))
}

fn requester(viewer: &Viewer) -> Option<Requester> {
    let workspace = viewer.workspace.as_ref()?;
    Some(Requester {
        account: viewer.account_id.clone(),
        workspace: workspace.id.clone(),
        members_epoch: workspace.members_epoch,
    })
}

#[derive(Deserialize)]
struct Submit {
    repo: String,
    issue: Value,
    #[serde(default)]
    land: Option<String>,
    #[serde(default)]
    engine: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

const SOURCES: [&str; 7] = ["api", "web", "chat", "github", "fleet", "cli", "project"];

async fn submit_route(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let (viewer, requester) = match person(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Ok(sent) = serde_json::from_slice::<Submit>(&body) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {repo, issue, land?, engine?}; land is pr, queue or none.",
        );
    };
    let issue = match &sent.issue {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s
            .trim()
            .trim_start_matches('#')
            .parse()
            .ok()
            .or_else(|| issue_address(s).map(|(_, n)| n)),
        _ => None,
    };
    let (Some(issue), Some(land), Some(engine)) = (
        issue,
        Land::parse(sent.land.as_deref().unwrap_or("pr")),
        Engine::parse(sent.engine.as_deref().unwrap_or("briefed")),
    ) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {repo, issue, land?, engine?}; land is pr, queue or none; engine is briefed or bare.",
        );
    };
    let source = sent
        .source
        .as_deref()
        .filter(|s| SOURCES.contains(s))
        .unwrap_or("api");
    let owner = account_owner(&viewer.account_id);
    let ask = Ask {
        repo: sent.repo.trim().to_owned(),
        issue,
        land,
        engine,
        source: source.into(),
        title: None,
    };
    match submit(&app.config.chat_store, &owner, requester, ask).await {
        Ok(Ok(run)) => answer(StatusCode::CREATED, view(&run, 0)),
        Ok(Err(Refused::Busy)) => refused(
            StatusCode::TOO_MANY_REQUESTS,
            "busy",
            "Too many runs are waiting. Try again when some finish.",
        ),
        Ok(Err(Refused::Invalid(message))) => refused(StatusCode::BAD_REQUEST, "invalid", message),
        Err(error) => stored(&error),
    }
}

async fn list_route(State(app): State<App>, headers: HeaderMap) -> Response {
    let (viewer, _) = match person(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    match list(&app.config.chat_store, &owner).await {
        Ok(runs) => answer(
            StatusCode::OK,
            json!({"runs": runs.iter().map(|run| view(run, usize::MAX)).collect::<Vec<_>>()}),
        ),
        Err(error) => stored(&error),
    }
}

#[derive(Deserialize)]
struct ReadQuery {
    #[serde(default)]
    after: usize,
    #[serde(default)]
    wait: u64,
}

async fn read_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<ReadQuery>,
) -> Response {
    let (viewer, _) = match person(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let store = &app.config.chat_store;
    let deadline = std::time::Instant::now() + Duration::from_secs(query.wait.min(MAX_WAIT));
    loop {
        match load(store, &owner, &id).await {
            Ok(Some(run)) => {
                let news = run.dropped + run.lines.len() > query.after || run.state.finished();
                if news || std::time::Instant::now() >= deadline {
                    return answer(StatusCode::OK, view(&run, query.after));
                }
            }
            Ok(None) => return refused(StatusCode::NOT_FOUND, "unknown", "There is no such run."),
            Err(error) => return stored(&error),
        }
        tokio::time::sleep(LOOK_EVERY).await;
    }
}

async fn events_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (viewer, _) = match person(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let store = app.config.chat_store.clone();
    match load(&store, &owner, &id).await {
        Ok(Some(_)) => {}
        Ok(None) => return refused(StatusCode::NOT_FOUND, "unknown", "There is no such run."),
        Err(error) => return stored(&error),
    }
    let shutdown = app.config.shutdown.clone();
    // (store, owner, id, next line, last state, ended, ticks)
    let stream = futures_util::stream::unfold(
        (store, owner, id, 0usize, String::new(), false, 0u32),
        |(store, owner, id, next, last, ended, ticks)| async move {
            if ended || ticks >= 3_600 {
                return None;
            }
            if ticks > 0 {
                tokio::time::sleep(LOOK_EVERY).await;
            }
            let mut events = Vec::new();
            let (mut next, mut last, mut ended) = (next, last, ended);
            match load(&store, &owner, &id).await {
                Ok(Some(run)) => {
                    let start = next.saturating_sub(run.dropped).min(run.lines.len());
                    for progress in &run.lines[start..] {
                        events.push(
                            Event::default()
                                .event("progress")
                                .data(json!(progress).to_string()),
                        );
                    }
                    next = run.dropped + run.lines.len();
                    if run.state.word() != last {
                        last = run.state.word().to_owned();
                        events.push(
                            Event::default()
                                .event("state")
                                .data(json!({"state": last, "why": run.why}).to_string()),
                        );
                    }
                    if run.state.finished() {
                        events.push(
                            Event::default()
                                .event("result")
                                .data(view(&run, usize::MAX).to_string()),
                        );
                        ended = true;
                    }
                }
                _ => {
                    events.push(Event::default().event("gone").data("{}"));
                    ended = true;
                }
            }
            if events.is_empty() {
                events.push(Event::default().comment("current"));
            }
            let batch = futures_util::stream::iter(
                events
                    .into_iter()
                    .map(Ok::<_, Infallible>)
                    .collect::<Vec<_>>(),
            );
            Some((batch, (store, owner, id, next, last, ended, ticks + 1)))
        },
    );
    use futures_util::StreamExt as _;
    protect(
        Sse::new(shutdown.until(stream.flatten()))
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
            .into_response(),
    )
}

async fn cancel_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let (viewer, _) = match person(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    match cancel(&app.config.chat_store, &owner, &id).await {
        Ok(Some(())) => answer(StatusCode::ACCEPTED, json!({"cancelled": true})),
        Ok(None) => refused(StatusCode::NOT_FOUND, "unknown", "There is no such run."),
        Err(error) => stored(&error),
    }
}

/// Whether the request carries the work host token, compared in constant
/// time over digests.
fn host_allowed(app: &App, headers: &HeaderMap) -> bool {
    let Some(expected) = app.config.work_host_token.as_deref() else {
        return false;
    };
    let supplied = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    let a: [u8; 32] = Sha256::digest(expected.as_bytes()).into();
    let b: [u8; 32] = Sha256::digest(supplied.as_bytes()).into();
    !supplied.is_empty()
        && a.iter()
            .zip(b.iter())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

fn host_name(name: &str) -> Option<String> {
    (!name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.')))
    .then(|| name.to_owned())
}

async fn claim_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Response {
    if !host_allowed(&app, &headers) {
        return refused(StatusCode::UNAUTHORIZED, "unauthorized", "Not a work host.");
    }
    let Some(host) = host_name(&name) else {
        return refused(StatusCode::BAD_REQUEST, "invalid", "Name the host.");
    };
    let Some(computers) = app.config.cloud_byo.as_deref() else {
        return answer(StatusCode::OK, json!({"run": null}));
    };
    match claim(&app.config.chat_store, &Saved(computers), &host).await {
        Ok(Some(taken)) => {
            let mut response = answer(
                StatusCode::OK,
                json!({"run": {
                    "id": taken.run.id,
                    "repo": taken.run.repo,
                    "issue": taken.run.issue,
                    "land": taken.run.land.word(),
                    "engine": taken.run.engine.word(),
                    "env": taken.env,
                }}),
            );
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response
        }
        Ok(None) => answer(StatusCode::OK, json!({"run": null})),
        Err(error) => stored(&error),
    }
}

async fn report_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path((name, id)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    if !host_allowed(&app, &headers) {
        return refused(StatusCode::UNAUTHORIZED, "unauthorized", "Not a work host.");
    }
    let Some(host) = host_name(&name) else {
        return refused(StatusCode::BAD_REQUEST, "invalid", "Name the host.");
    };
    if !valid_id(&id) {
        return refused(StatusCode::NOT_FOUND, "unknown", "There is no such run.");
    }
    let Ok(sent) = serde_json::from_slice::<Report>(&body) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {lines?, result?, failed?, title?}.",
        );
    };
    match report(&app.config.chat_store, &host, &id, sent).await {
        Ok(Some(heard)) => answer(StatusCode::OK, json!(heard)),
        Ok(None) => refused(StatusCode::NOT_FOUND, "unknown", "There is no such run."),
        Err(error) => stored(&error),
    }
}

// ---------------------------------------------------------------- the chat

/// Where a chat reply the typed router read as work on code (the
/// `work.dispatch` route) shows "Work on this issue" for the issues its
/// message names: loaded into the thread ([`offer_route`]), nothing
/// otherwise.
pub(crate) fn thread_entry(
    chat: &str,
    index: usize,
    reply: Option<&openagents_chat::router::Meta>,
) -> Markup {
    if reply.and_then(|reply| reply.route.as_deref()) != Some(WORK_ROUTE) {
        return html! {};
    }
    let href = format!("/chat/{chat}/work-offer/{index}");
    html! { div.oa-work-offer hx-get=(href) hx-trigger="load" hx-swap="outerHTML" {} }
}

/// The issues the message before reply `index` names, read once the
/// router chose work on code: GitHub issue links, `OWNER/NAME#N`, and
/// `#N` in the chat's repository (bounded id fields).
pub(crate) fn offered(chat: &crate::chat_store::Conversation, index: usize) -> Vec<(String, u64)> {
    let Some(asked) = chat.messages[..index.min(chat.messages.len())]
        .iter()
        .rev()
        .find(|m| m.role == crate::chat_store::Role::User)
    else {
        return Vec::new();
    };
    let repository = chat
        .selection
        .as_ref()
        .and_then(|s| s.repository.as_ref())
        .map(|r| r.repository.clone());
    let mut found: Vec<(String, u64)> = Vec::new();
    for word in asked.text.split_whitespace() {
        let word = word.trim_matches(|c: char| {
            matches!(
                c,
                ',' | '.' | ';' | ':' | '(' | ')' | '<' | '>' | '"' | '\'' | '`'
            )
        });
        let named = issue_address(word).or_else(|| {
            let number = word.strip_prefix('#')?.parse::<u64>().ok()?;
            Some((repository.clone()?, number))
        });
        if let Some(issue) = named.filter(|(repo, n)| valid_repo(repo) && *n > 0)
            && !found.contains(&issue)
        {
            found.push(issue);
        }
    }
    found.truncate(4);
    found
}

async fn offer_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, index)): Path<(String, usize)>,
) -> Response {
    let empty = || protect(html! {}.into_response());
    let Ok(loaded) = crate::pages::chat::load(&app, &headers, &id).await else {
        return empty();
    };
    if web_requester(&app, &headers).await.is_none() {
        return empty();
    }
    let chat = &loaded.conversation;
    let issues = offered(chat, index);
    if issues.is_empty() {
        return empty();
    }
    protect(
        html! {
            div.oa-thread-notice.oa-work-offer {
                p { "The briefed agent can work on this here, with your own Claude sign-in: it makes the change, runs the checks, and opens a pull request." }
                @for (repo, issue) in &issues {
                    p { (repo) "#" (issue) }
                    (work_button(&app, &chat.owner, repo, *issue, "chat"))
                }
            }
        }
        .into_response(),
    )
}

// ---------------------------------------------------------------- the pages

/// "Work on this issue": a button that asks for issue `issue` of `repo` to
/// be worked by the briefed agent, from `source`.
pub(crate) fn work_button(app: &App, owner: &str, repo: &str, issue: u64, source: &str) -> Markup {
    html! {
        form method="post" action=(PAGE) class="oa-work-on-issue" {
            input type="hidden" name="csrf" value=(crate::pages::chat::csrf(app, owner));
            input type="hidden" name="repo" value=(repo);
            input type="hidden" name="issue" value=(issue);
            input type="hidden" name="source" value=(source);
            (Button::new("Work on this issue").kind(ButtonType::Submit))
        }
    }
}

fn ago(at: u64, now: u64) -> String {
    let seconds = now.saturating_sub(at);
    match seconds {
        0..=59 => "just now".into(),
        60..=3_599 => format!("{} min ago", seconds / 60),
        3_600..=86_399 => format!("{} h ago", seconds / 3_600),
        _ => format!("{} days ago", seconds / 86_400),
    }
}

fn page(
    headers: &HeaderMap,
    service: &CloudSession,
    viewer: &Viewer,
    title: &str,
    path: &str,
    refresh: bool,
    body: Markup,
) -> Response {
    let account = Account::SignedIn {
        name: viewer.account_label.clone(),
        sign_out: service.logout_csrf(headers, viewer).ok(),
        picture: viewer.avatar_url.is_some(),
        admin: viewer.admin,
    };
    let mut page = UiPage::new(title)
        .path(path)
        .account(account)
        .content(PageColumn::new(body));
    if refresh {
        page = page.head(html! {
            meta http-equiv="refresh" content=(REFRESH_SECONDS.to_string());
        });
    }
    protect(page.respond(headers))
}

/// The result line of a run: engine, time, cost, checks, and the change.
pub(crate) fn result_markup(run: &WorkRun) -> Markup {
    let Some(outcome) = &run.outcome else {
        return html! {};
    };
    let passed = outcome.checks.iter().filter(|c| c.ok).count();
    html! {
        span class="oa-settings-hint" {
            (engine_words(&outcome.engine))
            " · " (duration(outcome.secs))
            " · " (money(outcome.cost_usd))
            " · checks " (passed) "/" (outcome.checks.len())
            @if let Some(pr) = &outcome.pr { " · " a href=(pr) { "pull request" } }
            @else if let Some(entry) = &outcome.landing { " · in the landing queue (" (entry) ")" }
            @else if let Some(commit) = &outcome.commit { " · commit " (commit.chars().take(10).collect::<String>()) }
        }
        @if let Some(why) = &outcome.escalated {
            span class="oa-settings-hint" { "Handed to Claude Code: " (why) }
        }
    }
}

fn run_row(run: &WorkRun, now: u64) -> Markup {
    let href = format!("{PAGE}/{}", run.id);
    html! {
        div class="oa-settings-row" {
            div class="oa-settings-text" {
                span class="oa-settings-label" { a href=(href) { (run.title()) } }
                span class="oa-settings-hint" {
                    (run.state.words()) " · " (ago(run.created_unix, now))
                    @if !run.state.finished() { @if let Some(line) = run.last_line() { " · " (line) } }
                    @else if let Some(why) = &run.why { " · " (why) }
                }
                (result_markup(run))
            }
        }
    }
}

#[derive(Deserialize, Default)]
struct PageQuery {
    #[serde(default)]
    repo: Option<String>,
    #[serde(default)]
    issue: Option<String>,
}

async fn list_page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> Response {
    let (service, viewer) = match crate::settings::viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let runs = list(&app.config.chat_store, &owner)
        .await
        .unwrap_or_default();
    let saved = crate::cloud::byo::saved(&app, &headers).await;
    let now = now_unix();
    let going = runs.iter().any(|run| !run.state.finished());
    let repo = query
        .repo
        .filter(|repo| valid_repo(repo))
        .unwrap_or_else(|| "OpenAgentsInc/openagents".into());
    let body = html! {
        div class="oa-settings" {
            p {
                "Give an issue to the briefed agent: it reads a briefing of the files the issue "
                "needs, makes the change, runs the checks, and opens a pull request or lands it. "
                "When it can't, Claude Code takes over, and the run says why."
            }
            @if !saved {
                div class="oa-thread-notice" role="alert" {
                    p { "Runs use your own Claude sign-in. " a href="/settings/claude#key" { "Save it in Settings" } " first." }
                }
            }
            section class="oa-settings-group" aria-labelledby="work-new" {
                h2 #work-new { "Work on an issue" }
                form method="post" action=(PAGE) class="oa-settings-row" {
                    input type="hidden" name="csrf" value=(crate::pages::chat::csrf(&app, &owner));
                    input type="hidden" name="source" value="web";
                    div class="oa-settings-text" {
                        label class="oa-settings-label" for="work-repo" { "Repository" }
                        input #work-repo type="text" name="repo" value=(repo) required;
                        label class="oa-settings-label" for="work-issue" { "Issue number or link" }
                        input #work-issue type="text" name="issue" value=(query.issue.unwrap_or_default()) required;
                        label class="oa-settings-label" for="work-land" { "When it passes" }
                        select #work-land name="land" {
                            option value="pr" selected { "Open a pull request" }
                            option value="queue" { "Land through the landing queue" }
                            option value="none" { "Commit only" }
                        }
                        label class="oa-settings-hint" {
                            input type="checkbox" name="engine" value="bare";
                            " Use Claude Code alone instead of the briefed agent"
                        }
                    }
                    div class="oa-settings-control" {
                        (Button::new("Work on this issue").kind(ButtonType::Submit))
                    }
                }
            }
            section class="oa-settings-group" aria-labelledby="work-runs" {
                h2 #work-runs { "Runs" }
                @if runs.is_empty() {
                    div class="oa-settings-row" {
                        div class="oa-settings-text" {
                            span class="oa-settings-label" { "No runs yet" }
                            span class="oa-settings-hint" {
                                "Or from a terminal: curl -X POST https://openagents.com/v1/work -H \"Authorization: Bearer $TOKEN\" -d '{\"repo\":\"OWNER/NAME\",\"issue\":N}'"
                            }
                        }
                    }
                }
                @for run in &runs { (run_row(run, now)) }
            }
        }
    };
    page(
        &headers,
        service,
        &viewer,
        "Work on issues",
        PAGE,
        going,
        body,
    )
}

#[derive(Deserialize)]
struct SubmitForm {
    csrf: String,
    repo: String,
    issue: String,
    #[serde(default)]
    land: Option<String>,
    #[serde(default)]
    engine: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

fn problem(status: StatusCode, text: &str) -> Response {
    protect(
        crate::layout::problem(status, "Work on issues", text, (PAGE, "Back to work"))
            .into_response(),
    )
}

/// Whether `supplied` is this owner's form token, compared in constant time.
fn token_fits(app: &App, owner: &str, supplied: &str) -> bool {
    let expected = crate::pages::chat::csrf(app, owner);
    expected.len() == supplied.len()
        && expected
            .bytes()
            .zip(supplied.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

async fn submit_page(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<SubmitForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return problem(
            StatusCode::BAD_REQUEST,
            "Give the repository and the issue.",
        );
    };
    let (_, viewer) = match crate::settings::viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    if !token_fits(&app, &owner, &form.csrf) {
        return problem(
            StatusCode::FORBIDDEN,
            "Something went wrong. Reload the page.",
        );
    }
    let Some(requester) = requester(&viewer) else {
        return problem(StatusCode::FORBIDDEN, "Pick a workspace first.");
    };
    // The issue field takes a number or the issue's GitHub address.
    let (repo, issue) = match issue_address(&form.issue) {
        Some((repo, issue)) => (repo, Some(issue)),
        None => (
            form.repo.trim().to_owned(),
            form.issue.trim().trim_start_matches('#').parse().ok(),
        ),
    };
    let Some(issue) = issue else {
        return problem(
            StatusCode::BAD_REQUEST,
            "Give the issue's number or its link.",
        );
    };
    let source = form
        .source
        .as_deref()
        .filter(|s| SOURCES.contains(s))
        .unwrap_or("web");
    let ask = Ask {
        repo,
        issue,
        land: form
            .land
            .as_deref()
            .and_then(Land::parse)
            .unwrap_or_default(),
        engine: form
            .engine
            .as_deref()
            .and_then(Engine::parse)
            .unwrap_or_default(),
        source: source.into(),
        title: None,
    };
    match submit(&app.config.chat_store, &owner, requester, ask).await {
        Ok(Ok(run)) => protect(Redirect::to(&format!("{PAGE}/{}", run.id)).into_response()),
        Ok(Err(Refused::Busy)) => problem(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many runs are waiting. Try again when some finish.",
        ),
        Ok(Err(Refused::Invalid(message))) => problem(StatusCode::BAD_REQUEST, message),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "The run couldn't be saved. Try again in a minute.",
        ),
    }
}

async fn run_page(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let path = format!("{PAGE}/{id}");
    let (service, viewer) = match crate::settings::viewer(&app, &headers, &path).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let run = match load(&app.config.chat_store, &owner, &id).await {
        Ok(Some(run)) => run,
        _ => return protect(Redirect::to(PAGE).into_response()),
    };
    let now = now_unix();
    let issue_url = format!("https://github.com/{}/issues/{}", run.repo, run.issue);
    let stop = format!("{PAGE}/{}/stop", run.id);
    let body = html! {
        div class="oa-settings" {
            p { a href=(PAGE) { "All runs" } }
            section class="oa-settings-group" aria-labelledby="work-run" {
                h2 #work-run { a href=(issue_url) { (run.title()) } }
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { (run.state.words()) }
                        span class="oa-settings-hint" {
                            (engine_words(run.engine.word())) " · " (run.land.words())
                            " · asked " (ago(run.created_unix, now))
                        }
                        (result_markup(&run))
                        @if let Some(why) = &run.why { span class="oa-settings-hint" { (why) } }
                    }
                    @if !run.state.finished() && !run.cancel {
                        div class="oa-settings-control" {
                            form method="post" action=(stop) {
                                input type="hidden" name="csrf" value=(crate::pages::chat::csrf(&app, &owner));
                                (Button::new("Stop")
                                    .kind(ButtonType::Submit)
                                    .variant(ButtonVariant::Soft)
                                    .color(Color::Secondary))
                            }
                        }
                    }
                }
            }
            @if let Some(outcome) = &run.outcome {
                @if !outcome.checks.is_empty() || !outcome.files.is_empty() {
                    section class="oa-settings-group" aria-labelledby="work-change" {
                        h2 #work-change { "The change" }
                        @for check in &outcome.checks {
                            div class="oa-settings-row" {
                                div class="oa-settings-text" {
                                    span class="oa-settings-label" { @if check.ok { "Passed: " } @else { "Failed: " } (check.name) }
                                    @if let Some(passed) = check.passed { span class="oa-settings-hint" { (passed) " tests passed" } }
                                }
                            }
                        }
                        @if !outcome.files.is_empty() {
                            div class="oa-settings-row" {
                                div class="oa-settings-text" {
                                    span class="oa-settings-label" { (outcome.files.len()) " files, +" (outcome.added) " −" (outcome.removed) }
                                    span class="oa-settings-hint" { (outcome.files.join(", ")) }
                                }
                            }
                        }
                    }
                }
            }
            section class="oa-settings-group" aria-labelledby="work-progress" {
                h2 #work-progress { "Progress" }
                @if run.lines.is_empty() {
                    p { "Nothing yet." }
                } @else {
                    pre class="oa-work-log" style="white-space: pre-wrap; overflow-x: auto; font-size: 0.8rem" {
                        @for progress in &run.lines { (format!("{:>6.0}s  ", progress.secs)) (progress.text) "\n" }
                    }
                }
            }
        }
    };
    page(
        &headers,
        service,
        &viewer,
        "Work run",
        &path,
        !run.state.finished(),
        body,
    )
}

#[derive(Deserialize)]
struct StopForm {
    csrf: String,
}

async fn stop_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<StopForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return problem(StatusCode::BAD_REQUEST, "Reload the page and try again.");
    };
    let path = format!("{PAGE}/{id}");
    let (_, viewer) = match crate::settings::viewer(&app, &headers, &path).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    if !token_fits(&app, &owner, &form.csrf) {
        return problem(
            StatusCode::FORBIDDEN,
            "Something went wrong. Reload the page.",
        );
    }
    match cancel(&app.config.chat_store, &owner, &id).await {
        Ok(_) => protect(Redirect::to(&path).into_response()),
        Err(_) => problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "The run couldn't be stopped. Try again in a minute.",
        ),
    }
}

#[cfg(test)]
mod tests;
