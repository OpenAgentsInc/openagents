//! Mac jobs (#11223): Mac-only steps a cloud environment, or an agent in
//! one, hands to a Mac linked to the same account, with the results back.
//!
//! Built on the own-runs pull channel ([`crate::own_runs`], #11080): the
//! website keeps the jobs, the Mac pulls them under its own sign-in, and
//! the website never reaches the Mac. A job is typed ([`mac_jobs::Spec`]):
//! a repository, a ref, and one named recipe with allowlisted arguments.
//!
//! | Route | Who | What |
//! | --- | --- | --- |
//! | `POST /v1/computers/{name}/mac-jobs` `{capabilities}` | The Mac | Report what it can do; answers `{jobs: [{id, spec}]}`, each handed out once |
//! | `POST /v1/computers/{name}/mac-jobs/{id}` `{lines, commit?, ask?, done?, failed?}` | The Mac | Log lines, the commit, the owner's question, then how it ended; answers `{cancel, approval?}` (the owner's answer, handed out once) |
//! | `PUT /v1/computers/{name}/mac-jobs/{id}/artifacts/{file}?part=N&last=0\|1` | The Mac | One part (at most [`PART_BYTES`]) of a file the job made |
//! | `GET /v1/mac-jobs/macs` | The account | `{macs: [{name, online, reported_unix, capabilities}]}` |
//! | `POST /v1/mac-jobs` `{repo, ref, recipe, args?, computer?}` | The account | Queue a job: `201 {id, computer, kind, approval, online}`; `409 no_mac` |
//! | `GET /v1/mac-jobs` | The account | `{jobs: [View]}`, newest first |
//! | `GET /v1/mac-jobs/{id}?after=&wait=` | The account | The job, its lines from `after`, waiting up to `wait` seconds for news |
//! | `GET /v1/mac-jobs/{id}/artifacts/{file}` | The account | A file the job made |
//! | `POST /v1/mac-jobs/{id}/cancel` | The account | Stop the job |
//!
//! Every route takes the app's own token (`Authorization: Bearer sess_…`,
//! [`crate::coder_sync::owner`]) and answers only that account's records.
//!
//! **Approvals.** A recipe that reaches outside ([`mac_jobs::Spec::outward`],
//! a TestFlight upload) waits on the Mac for the owner: the Mac reports
//! the question with the exact subject (the repository at its commit, the
//! recipe, its arguments), and the owner answers it on the phone (the job
//! is an item on the computer's board, `GET /v1/agents`, answered with
//! `POST /v1/agents/actions`) or on the web (`/settings/mac-jobs`,
//! [`crate::mac_jobs_page`]), the way #11170's approvals are answered. No
//! route here approves: the API that submits a job can't open its gate.
//! The Mac records the answer in its own approvals file and uses it once.
//! Signing identities and App Store keys never leave the Mac.
//!
//! Records live beside the account's chats ([`Store::owner_key`]): what
//! each Mac reported ([`MACS_KEY`]), the index of jobs ([`INDEX_KEY`]), one
//! object per job, and each job's files in parts under its own folder.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use mac_jobs::{Capabilities, Kind, Recipe, Spec, valid_job_id};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::App;
use crate::chat_store::{Error, Store, now_unix};
use crate::coder_sync::{self, answer, line, refused, stored};
use crate::phone_api::{Item, Question as ItemQuestion};

/// The account's routes, under this prefix (`crate::upstream::owned`).
pub(crate) const PREFIX: &str = "/v1/mac-jobs";
/// What each Mac reported, under the account's folder.
pub(crate) const MACS_KEY: &str = "mac-jobs/macs.json";
/// The jobs, and the ones waiting for a Mac.
pub(crate) const INDEX_KEY: &str = "mac-jobs/index.json";
const MACS_SCHEMA: &str = "openagents.web.mac-jobs.macs.v1";
const INDEX_SCHEMA: &str = "openagents.web.mac-jobs.index.v1";
const JOB_SCHEMA: &str = "openagents.web.mac-jobs.job.v1";

/// A Mac whose last report is older than this is offline (it reports
/// every few seconds).
pub(crate) const ONLINE_SECONDS: u64 = 60;
/// An unchanged report is still written this often, for its time.
const REWRITE_EVERY: u64 = 20;
/// A Mac not heard from in this long is forgotten.
const FORGET_MAC: u64 = 30 * 24 * 3600;
/// The most Macs kept.
const MAX_MACS: usize = 8;
/// The most jobs waiting at once.
const MAX_WAITING: usize = 16;
/// The most jobs kept (the oldest finished go first).
const MAX_JOBS: usize = 64;
/// A job no Mac took in this long is cancelled.
const WAIT_TTL: u64 = 6 * 3600;
/// A running job the Mac hasn't reported on in this long has stopped (the
/// Mac reports at least every ten seconds while it runs).
const STALE_SECONDS: u64 = 180;
/// The most log lines a job keeps (the oldest go first; the whole log is
/// one of the job's files).
const MAX_LINES: usize = 2_000;
/// The most lines one report carries.
const MAX_REPORT_LINES: usize = 500;
/// The largest part of a file, in bytes.
pub(crate) const PART_BYTES: usize = 8 * 1024 * 1024;
/// The most parts one file has (512 MiB).
const MAX_PARTS: u32 = 64;
/// The most files one job keeps.
const MAX_ARTIFACTS: usize = 64;
/// The most bytes one job's files hold in all.
const MAX_JOB_BYTES: u64 = 1024 * 1024 * 1024;
/// The longest a read waits for news.
const MAX_WAIT: u64 = 25;
/// How often a waiting read looks again.
const LOOK_EVERY: Duration = Duration::from_millis(750);
/// Finished jobs shown on the computers' boards this long.
const BOARD_SECONDS: u64 = 24 * 3600;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/computers/{name}/mac-jobs", post(take_route))
        .route("/v1/computers/{name}/mac-jobs/{id}", post(report_route))
        .route(
            "/v1/computers/{name}/mac-jobs/{id}/artifacts/{file}",
            put(part_route).layer(DefaultBodyLimit::max(PART_BYTES + 1024)),
        )
        .route("/v1/mac-jobs", post(submit_route).get(list_route))
        .route("/v1/mac-jobs/macs", get(macs_route))
        .route("/v1/mac-jobs/{id}", get(read_route))
        .route("/v1/mac-jobs/{id}/cancel", post(cancel_route))
        .route("/v1/mac-jobs/{id}/artifacts/{file}", get(artifact_route))
}

/// Whether `path` is one of the account's routes here.
pub(crate) fn owns(path: &str) -> bool {
    path.strip_prefix(PREFIX)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

// ------------------------------------------------------------------ records

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Reported {
    pub reported_unix: u64,
    pub capabilities: Capabilities,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Macs {
    #[serde(default)]
    schema: String,
    #[serde(default)]
    computers: BTreeMap<String, Reported>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Waiting {
    computer: String,
    created_unix: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Index {
    #[serde(default)]
    schema: String,
    #[serde(default)]
    waiting: BTreeMap<String, Waiting>,
    /// Every job kept, id to when it was made.
    #[serde(default)]
    jobs: BTreeMap<String, u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum JobState {
    /// Waiting for its Mac to take it.
    Waiting,
    Running,
    /// Waiting for the owner's answer.
    Asking,
    Done,
    Failed,
    Cancelled,
}

impl JobState {
    pub(crate) fn finished(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }

    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Running => "running",
            Self::Asking => "asking",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// The owner's question, as the Mac asked it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Question {
    pub id: String,
    pub text: String,
    pub subject: String,
}

/// The owner's answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Approval {
    pub question: String,
    /// `approved` or `denied`.
    pub decision: String,
    /// `web` or `phone`.
    pub via: String,
    pub at_unix: u64,
}

/// One file a job made.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Artifact {
    pub name: String,
    pub size: u64,
    pub parts: u32,
    /// Every part arrived.
    pub done: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Job {
    schema: String,
    pub id: String,
    pub computer: String,
    pub spec: Spec,
    pub created_unix: u64,
    pub updated_unix: u64,
    pub state: JobState,
    #[serde(default)]
    pub lines: Vec<String>,
    /// Lines dropped from the front, so a reader's `after` stays right.
    #[serde(default)]
    pub dropped: usize,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub question: Option<Question>,
    #[serde(default)]
    pub approval: Option<Approval>,
    /// The Mac has the answer.
    #[serde(default)]
    pub approval_taken: bool,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
    #[serde(default)]
    pub summary: Option<String>,
    /// Why it stopped, in plain words.
    #[serde(default)]
    pub why: Option<String>,
    #[serde(default)]
    pub cancel: bool,
    #[serde(default)]
    pub finished_unix: Option<u64>,
}

impl Job {
    /// The job's own words for its last news: its result, why it stopped,
    /// or its newest log line.
    pub(crate) fn last_line(&self) -> Option<String> {
        self.summary
            .clone()
            .or_else(|| self.why.clone())
            .or_else(|| self.lines.last().cloned())
    }

    fn end(&mut self, state: JobState, why: Option<String>) {
        self.state = state;
        self.why = why;
        self.finished_unix = Some(now_unix());
    }

    /// Settle a job whose Mac went quiet or that no Mac took. Returns
    /// whether it changed.
    fn settle(&mut self, now: u64) -> bool {
        match self.state {
            JobState::Waiting if now.saturating_sub(self.created_unix) > WAIT_TTL => {
                self.end(
                    JobState::Cancelled,
                    Some("No Mac took this job in time.".into()),
                );
                true
            }
            JobState::Running | JobState::Asking
                if now.saturating_sub(self.updated_unix) > STALE_SECONDS =>
            {
                self.end(JobState::Failed, Some("The Mac stopped answering.".into()));
                true
            }
            _ => false,
        }
    }
}

fn job_key(owner: &str, id: &str) -> Result<String, Error> {
    Store::owner_key(owner, &format!("mac-jobs/job-{id}.json"))
}

fn artifact_folder(owner: &str, id: &str) -> Result<String, Error> {
    Store::owner_key(owner, &format!("mac-jobs/art-{id}"))
}

fn part_key(owner: &str, id: &str, name: &str, part: u32) -> Result<String, Error> {
    Store::owner_key(owner, &format!("mac-jobs/art-{id}/{name}.p{part}"))
}

fn new_job_id() -> String {
    let bytes: [u8; 16] = secp256k1::rand::random();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("mjob{hex}")
}

/// A file name a job may store: letters, digits, `.`, `_`, `-`, not
/// starting with `.` or `-`, at most 96 characters.
fn valid_artifact(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 96
        && !name.starts_with(['.', '-'])
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
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
    for _ in 0..6 {
        let (mut record, generation) = match store.read_key(key).await? {
            Some((bytes, generation)) => (
                serde_json::from_slice::<R>(&bytes)
                    .map_err(|_| Error::Corrupt("A Mac job record is invalid."))?,
                Some(generation),
            ),
            None => (fresh(), None),
        };
        let (changed, result) = change(&mut record);
        if !changed {
            return Ok(result);
        }
        let bytes = serde_json::to_vec(&record)
            .map_err(|_| Error::Invalid("A Mac job record is invalid."))?;
        match store.write_key(key, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(result),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

fn fresh_macs() -> Macs {
    Macs {
        schema: MACS_SCHEMA.into(),
        ..Macs::default()
    }
}

fn fresh_index() -> Index {
    Index {
        schema: INDEX_SCHEMA.into(),
        ..Index::default()
    }
}

/// The job `id` of `owner`, settled; `None` when there is none.
pub(crate) async fn load(store: &Store, owner: &str, id: &str) -> Result<Option<Job>, Error> {
    if !valid_job_id(id) {
        return Ok(None);
    }
    let key = job_key(owner, id)?;
    let Some((bytes, _)) = store.read_key(&key).await? else {
        return Ok(None);
    };
    let job: Job =
        serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt("A Mac job is invalid."))?;
    if job.clone().settle(now_unix()) {
        if update_job(store, owner, id, |job| (job.settle(now_unix()), ()))
            .await?
            .is_none()
        {
            return Ok(None);
        }
        return load_plain(store, owner, id).await;
    }
    Ok(Some(job))
}

async fn load_plain(store: &Store, owner: &str, id: &str) -> Result<Option<Job>, Error> {
    match store.read_key(&job_key(owner, id)?).await? {
        Some((bytes, _)) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| Error::Corrupt("A Mac job is invalid.")),
        None => Ok(None),
    }
}

/// Change job `id` with `change`; `None` when there is no such job.
async fn update_job<T>(
    store: &Store,
    owner: &str,
    id: &str,
    change: impl Fn(&mut Job) -> (bool, T),
) -> Result<Option<T>, Error> {
    let key = job_key(owner, id)?;
    for _ in 0..6 {
        let Some((bytes, generation)) = store.read_key(&key).await? else {
            return Ok(None);
        };
        let mut job: Job =
            serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt("A Mac job is invalid."))?;
        let (changed, result) = change(&mut job);
        if !changed {
            return Ok(Some(result));
        }
        job.updated_unix = now_unix();
        let bytes =
            serde_json::to_vec(&job).map_err(|_| Error::Invalid("A Mac job is invalid."))?;
        match store.write_key(&key, bytes, Some(&generation)).await {
            Ok(_) => return Ok(Some(result)),
            Err(Error::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(Error::Conflict)
}

// -------------------------------------------------------------- the Mac's

/// The Mac `computer` reports what it can do.
pub(crate) async fn report_capabilities(
    store: &Store,
    owner: &str,
    computer: &str,
    capabilities: Capabilities,
) -> Result<(), Error> {
    let key = Store::owner_key(owner, MACS_KEY)?;
    let computer = computer.to_owned();
    let capabilities = capabilities.bounded();
    update(store, &key, fresh_macs, move |macs| {
        let now = now_unix();
        if macs.computers.get(&computer).is_some_and(|reported| {
            reported.capabilities == capabilities
                && now.saturating_sub(reported.reported_unix) < REWRITE_EVERY
        }) {
            return (false, ());
        }
        macs.schema = MACS_SCHEMA.into();
        macs.computers.insert(
            computer.clone(),
            Reported {
                reported_unix: now,
                capabilities: capabilities.clone(),
            },
        );
        macs.computers
            .retain(|_, reported| now.saturating_sub(reported.reported_unix) < FORGET_MAC);
        while macs.computers.len() > MAX_MACS {
            let Some(oldest) = macs
                .computers
                .iter()
                .min_by_key(|(_, reported)| reported.reported_unix)
                .map(|(name, _)| name.clone())
            else {
                break;
            };
            macs.computers.remove(&oldest);
        }
        (true, ())
    })
    .await
}

/// A job handed to its Mac.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct Taken {
    pub id: String,
    pub spec: Spec,
}

/// Take the jobs waiting for `computer`, each once, oldest first.
pub(crate) async fn take(store: &Store, owner: &str, computer: &str) -> Result<Vec<Taken>, Error> {
    let key = Store::owner_key(owner, INDEX_KEY)?;
    let name = computer.to_owned();
    let ids = update(store, &key, fresh_index, move |index| {
        let mut mine: Vec<(String, u64)> = index
            .waiting
            .iter()
            .filter(|(_, waiting)| waiting.computer == name)
            .map(|(id, waiting)| (id.clone(), waiting.created_unix))
            .collect();
        mine.sort_by_key(|(_, at)| *at);
        for (id, _) in &mine {
            index.waiting.remove(id);
        }
        (!mine.is_empty(), mine)
    })
    .await?;
    let mut taken = Vec::new();
    for (id, _) in ids {
        let claimed = update_job(store, owner, &id, |job| {
            if job.state != JobState::Waiting || job.cancel {
                return (false, None);
            }
            job.state = JobState::Running;
            (
                true,
                Some(Taken {
                    id: job.id.clone(),
                    spec: job.spec.clone(),
                }),
            )
        })
        .await?;
        if let Some(Some(job)) = claimed {
            taken.push(job);
        }
    }
    Ok(taken)
}

/// What the Mac reports for one job.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Report {
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub ask: Option<Question>,
    #[serde(default)]
    pub done: Option<Done>,
    #[serde(default)]
    pub failed: Option<Failed>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Done {
    pub summary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Failed {
    pub why: String,
}

/// What a report is answered with.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct Heard {
    pub cancel: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval: Option<Approval>,
}

/// A line as it is kept: one line, bounded, and never a credential.
fn kept_line(text: &str) -> String {
    let text = line(text, mac_jobs::LINE_CHARS);
    secret_screen::redact(&text)
}

fn commit_id(commit: &str) -> bool {
    (7..=64).contains(&commit.len()) && commit.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Keep a job's report; the answer says whether to stop and carries the
/// owner's answer once. `None` when there is no such job on `computer`.
pub(crate) async fn report(
    store: &Store,
    owner: &str,
    computer: &str,
    id: &str,
    sent: Report,
) -> Result<Option<Heard>, Error> {
    let lines: Vec<String> = sent
        .lines
        .iter()
        .take(MAX_REPORT_LINES)
        .map(|text| kept_line(text))
        .filter(|text| !text.is_empty())
        .collect();
    let ask = sent.ask.map(|ask| Question {
        id: line(&ask.id, 64),
        text: ask.text.chars().take(2_000).collect(),
        subject: line(&ask.subject, 500),
    });
    let commit = sent.commit.filter(|commit| commit_id(commit));
    let done = sent.done.map(|done| line(&done.summary, 500));
    let failed = sent.failed.map(|failed| line(&failed.why, 500));
    update_job(store, owner, id, |job| {
        if job.computer != computer {
            return (false, None);
        }
        if job.state.finished() || job.cancel {
            // A cancelled job still takes its last report.
            if job.cancel && !job.state.finished() && (done.is_some() || failed.is_some()) {
                job.end(JobState::Cancelled, Some("The job was cancelled.".into()));
                return (
                    true,
                    Some(Heard {
                        cancel: true,
                        approval: None,
                    }),
                );
            }
            return (
                false,
                Some(Heard {
                    cancel: true,
                    approval: None,
                }),
            );
        }
        let mut changed = now_unix().saturating_sub(job.updated_unix) >= REWRITE_EVERY;
        for text in &lines {
            if job.lines.len() >= MAX_LINES {
                job.lines.remove(0);
                job.dropped += 1;
            }
            job.lines.push(text.clone());
            changed = true;
        }
        if job.commit.is_none()
            && let Some(commit) = &commit
        {
            job.commit = Some(commit.clone());
            changed = true;
        }
        if job.state == JobState::Waiting {
            job.state = JobState::Running;
            changed = true;
        }
        if let Some(ask) = &ask
            && job.question.as_ref().is_none_or(|q| q.id != ask.id)
            && !ask.id.is_empty()
        {
            job.question = Some(ask.clone());
            job.approval = None;
            job.approval_taken = false;
            job.state = JobState::Asking;
            changed = true;
        }
        let mut heard = Heard::default();
        if let Some(approval) = &job.approval
            && !job.approval_taken
        {
            heard.approval = Some(approval.clone());
            job.approval_taken = true;
            if job.state == JobState::Asking {
                job.state = JobState::Running;
            }
            changed = true;
        }
        if let Some(summary) = &done {
            job.summary = Some(summary.clone());
            job.end(JobState::Done, None);
            changed = true;
        } else if let Some(why) = &failed {
            job.end(
                JobState::Failed,
                Some(if why.is_empty() {
                    "The job stopped.".into()
                } else {
                    why.clone()
                }),
            );
            changed = true;
        }
        (changed, Some(heard))
    })
    .await
    .map(Option::flatten)
}

/// What became of a file's part.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PartSaved {
    Saved,
    /// No such job on this Mac, or it ended.
    Unknown,
    Refused(&'static str),
}

/// Keep part `part` of file `name` of job `id`. Parts come in order; the
/// same part again replaces it.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn save_part(
    store: &Store,
    owner: &str,
    computer: &str,
    id: &str,
    name: &str,
    part: u32,
    last: bool,
    bytes: Vec<u8>,
) -> Result<PartSaved, Error> {
    if !valid_artifact(name) {
        return Ok(PartSaved::Refused(
            "Name the file with letters, digits, . _ -.",
        ));
    }
    if bytes.len() > PART_BYTES {
        return Ok(PartSaved::Refused("A part is at most 8 MiB."));
    }
    if part >= MAX_PARTS {
        return Ok(PartSaved::Refused("A file is at most 512 MiB."));
    }
    let size = bytes.len() as u64;
    // First check the part fits, then store it, then record it.
    let Some(job) = load(store, owner, id).await? else {
        return Ok(PartSaved::Unknown);
    };
    if job.computer != computer || job.state.finished() || job.cancel {
        return Ok(PartSaved::Unknown);
    }
    let fits = match job.artifacts.iter().find(|a| a.name == name) {
        Some(found) => !found.done && (part == found.parts || part + 1 == found.parts),
        None => part == 0 && job.artifacts.len() < MAX_ARTIFACTS,
    };
    let total: u64 = job.artifacts.iter().map(|a| a.size).sum();
    if !fits {
        return Ok(PartSaved::Refused("Send a file's parts in order."));
    }
    if total + size > MAX_JOB_BYTES {
        return Ok(PartSaved::Refused("This job's files are over 1 GiB."));
    }
    let key = part_key(owner, id, name, part)?;
    let generation = store
        .read_key(&key)
        .await?
        .map(|(_, generation)| generation);
    store.write_key(&key, bytes, generation.as_deref()).await?;
    let name = name.to_owned();
    let recorded = update_job(store, owner, id, |job| {
        if job.state.finished() {
            return (false, false);
        }
        match job.artifacts.iter_mut().find(|a| a.name == name) {
            Some(found) if part == found.parts => {
                found.parts += 1;
                found.size += size;
                found.done = last;
            }
            Some(found) if part + 1 == found.parts => found.done = last,
            Some(_) => return (false, false),
            None => job.artifacts.push(Artifact {
                name: name.clone(),
                size,
                parts: 1,
                done: last,
            }),
        }
        (true, true)
    })
    .await?;
    Ok(if recorded == Some(true) {
        PartSaved::Saved
    } else {
        PartSaved::Unknown
    })
}

// ------------------------------------------------------------ the account's

/// Every Mac the account has, newest report first, and whether it is
/// online.
pub(crate) async fn macs(
    store: &Store,
    owner: &str,
) -> Result<Vec<(String, Reported, bool)>, Error> {
    let key = Store::owner_key(owner, MACS_KEY)?;
    let macs: Macs = match store.read_key(&key).await? {
        Some((bytes, _)) => serde_json::from_slice(&bytes)
            .map_err(|_| Error::Corrupt("A Mac job record is invalid."))?,
        None => return Ok(Vec::new()),
    };
    let now = now_unix();
    let mut found: Vec<(String, Reported, bool)> = macs
        .computers
        .into_iter()
        .map(|(name, reported)| {
            let online = now.saturating_sub(reported.reported_unix) <= ONLINE_SECONDS;
            (name, reported, online)
        })
        .collect();
    found.sort_by_key(|(_, reported, _)| std::cmp::Reverse(reported.reported_unix));
    Ok(found)
}

/// What a job asks to run.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Submit {
    pub repo: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub recipe: Recipe,
    #[serde(default)]
    pub args: Vec<String>,
    /// The Mac by name; the account's best Mac for the recipe when unset.
    #[serde(default)]
    pub computer: Option<String>,
}

/// A queued job.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Submitted {
    pub id: String,
    pub computer: String,
    pub kind: Kind,
    pub approval: bool,
    pub online: bool,
}

/// Why a job can't be queued.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Refused {
    Invalid(String),
    /// No linked Mac can run it; the reason in plain words.
    NoMac(String),
    Busy,
}

/// Queue a job for `owner` on a Mac that can run it.
pub(crate) async fn submit(
    store: &Store,
    owner: &str,
    sent: Submit,
) -> Result<Result<Submitted, Refused>, Error> {
    let spec = Spec {
        repo: sent.repo,
        git_ref: sent.git_ref,
        recipe: sent.recipe,
        args: sent.args,
    };
    if let Err(why) = spec.check() {
        return Ok(Err(Refused::Invalid(why)));
    }
    let named = sent.computer.map(|name| line(&name, 64));
    let found = macs(store, owner).await?;
    let mut candidates: Vec<&(String, Reported, bool)> = found
        .iter()
        .filter(|(name, _, _)| named.as_ref().is_none_or(|named| named == name))
        .collect();
    if candidates.is_empty() {
        return Ok(Err(Refused::NoMac(match &named {
            Some(name) => format!("{name} isn't a Mac linked to this account."),
            None => "No Mac is linked to this account. Run openagents mac serve on one.".into(),
        })));
    }
    let mut why_not = None;
    candidates.retain(
        |(_, reported, _)| match reported.capabilities.supports(&spec) {
            Ok(()) => true,
            Err(why) => {
                why_not.get_or_insert(why);
                false
            }
        },
    );
    // Online and free first, then online, then the newest report.
    candidates.sort_by_key(|(_, reported, online)| {
        (
            !*online,
            reported.capabilities.busy,
            std::cmp::Reverse(reported.reported_unix),
        )
    });
    let Some((computer, _, online)) = candidates.first().map(|c| (c.0.clone(), c.1.clone(), c.2))
    else {
        return Ok(Err(Refused::NoMac(
            why_not.unwrap_or_else(|| "No linked Mac runs this recipe.".into()),
        )));
    };
    let now = now_unix();
    let id = new_job_id();
    let job = Job {
        schema: JOB_SCHEMA.into(),
        id: id.clone(),
        computer: computer.clone(),
        spec: spec.clone(),
        created_unix: now,
        updated_unix: now,
        state: JobState::Waiting,
        lines: Vec::new(),
        dropped: 0,
        commit: None,
        question: None,
        approval: None,
        approval_taken: false,
        artifacts: Vec::new(),
        summary: None,
        why: None,
        cancel: false,
        finished_unix: None,
    };
    let bytes = serde_json::to_vec(&job).map_err(|_| Error::Invalid("A Mac job is invalid."))?;
    store.write_key(&job_key(owner, &id)?, bytes, None).await?;
    let index_key = Store::owner_key(owner, INDEX_KEY)?;
    let (job_id, mac) = (id.clone(), computer.clone());
    let queued = update(store, &index_key, fresh_index, move |index| {
        index.schema = INDEX_SCHEMA.into();
        if index.waiting.len() >= MAX_WAITING {
            return (false, None);
        }
        index.waiting.insert(
            job_id.clone(),
            Waiting {
                computer: mac.clone(),
                created_unix: now,
            },
        );
        index.jobs.insert(job_id.clone(), now);
        // The oldest jobs past the limit go, waiting ones last.
        let mut gone = Vec::new();
        while index.jobs.len() > MAX_JOBS {
            let Some(oldest) = index
                .jobs
                .iter()
                .filter(|(id, _)| !index.waiting.contains_key(*id))
                .min_by_key(|(_, at)| **at)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            index.jobs.remove(&oldest);
            gone.push(oldest);
        }
        (true, Some(gone))
    })
    .await?;
    let Some(gone) = queued else {
        if let Ok(Some((_, generation))) = store.read_key(&job_key(owner, &id)?).await {
            let _ = store.delete_key(&job_key(owner, &id)?, &generation).await;
        }
        return Ok(Err(Refused::Busy));
    };
    for old in gone {
        forget(store, owner, &old).await;
    }
    Ok(Ok(Submitted {
        id,
        computer,
        kind: spec.kind(),
        approval: spec.outward(),
        online,
    }))
}

/// Remove a job and its files.
async fn forget(store: &Store, owner: &str, id: &str) {
    if let Ok(folder) = artifact_folder(owner, id) {
        let _ = store.remove_folder(&folder).await;
    }
    if let Ok(key) = job_key(owner, id)
        && let Ok(Some((_, generation))) = store.read_key(&key).await
    {
        let _ = store.delete_key(&key, &generation).await;
    }
}

/// The account's jobs, newest first, settled.
pub(crate) async fn list(store: &Store, owner: &str) -> Result<Vec<Job>, Error> {
    let key = Store::owner_key(owner, INDEX_KEY)?;
    let index: Index = match store.read_key(&key).await? {
        Some((bytes, _)) => serde_json::from_slice(&bytes).unwrap_or_default(),
        None => return Ok(Vec::new()),
    };
    let mut ids: Vec<(String, u64)> = index.jobs.into_iter().collect();
    ids.sort_by_key(|(_, at)| std::cmp::Reverse(*at));
    let mut jobs = Vec::new();
    for (id, _) in ids {
        if let Some(job) = load(store, owner, &id).await? {
            jobs.push(job);
        }
    }
    Ok(jobs)
}

/// Stop a job: a waiting one at once, a running one at its Mac's next
/// report. `None` when there is no such job.
pub(crate) async fn cancel(store: &Store, owner: &str, id: &str) -> Result<Option<()>, Error> {
    if !valid_job_id(id) {
        return Ok(None);
    }
    let stopped = update_job(store, owner, id, |job| {
        if job.state.finished() || job.cancel {
            return (false, ());
        }
        job.cancel = true;
        if job.state == JobState::Waiting {
            job.end(
                JobState::Cancelled,
                Some("The job was cancelled before it started.".into()),
            );
        }
        (true, ())
    })
    .await?;
    if stopped.is_some()
        && let Ok(key) = Store::owner_key(owner, INDEX_KEY)
    {
        let id = id.to_owned();
        let _ = update(store, &key, fresh_index, move |index| {
            (index.waiting.remove(&id).is_some(), ())
        })
        .await;
    }
    Ok(stopped)
}

/// What became of the owner's answer.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Answered {
    Recorded,
    /// The job isn't waiting on that question.
    NotAsking,
    Unknown,
}

/// The owner answers job `id`'s question `question` (`approve` or `deny`),
/// on `via` (`web` or `phone`). The Mac takes the answer at its next
/// report and records it in its own approvals file.
pub(crate) async fn answer_question(
    store: &Store,
    owner: &str,
    id: &str,
    question: &str,
    approve: bool,
    via: &str,
) -> Result<Answered, Error> {
    if !valid_job_id(id) {
        return Ok(Answered::Unknown);
    }
    let via = via.to_owned();
    let question = question.to_owned();
    let answered = update_job(store, owner, id, |job| {
        let asking = job.state == JobState::Asking
            && !job.cancel
            && job.approval.is_none()
            && job.question.as_ref().is_some_and(|q| q.id == question);
        if !asking {
            return (false, Answered::NotAsking);
        }
        job.approval = Some(Approval {
            question: question.clone(),
            decision: if approve { "approved" } else { "denied" }.into(),
            via: via.clone(),
            at_unix: now_unix(),
        });
        job.approval_taken = false;
        (true, Answered::Recorded)
    })
    .await?;
    Ok(answered.unwrap_or(Answered::Unknown))
}

/// The job as the API answers it, with its lines from `after`.
pub(crate) fn view(job: &Job, after: usize) -> Value {
    let start = after.saturating_sub(job.dropped).min(job.lines.len());
    json!({
        "id": job.id,
        "computer": job.computer,
        "repo": job.spec.repo,
        "ref": job.spec.git_ref,
        "recipe": job.spec.recipe,
        "args": job.spec.args,
        "kind": job.spec.kind(),
        "title": job.spec.title(),
        "state": job.state.word(),
        "created_unix": job.created_unix,
        "updated_unix": job.updated_unix,
        "finished_unix": job.finished_unix,
        "commit": job.commit,
        "question": job.question.as_ref().map(|q| json!({"id": q.id, "text": q.text, "subject": q.subject})),
        "approval": job.approval,
        "summary": job.summary,
        "why": job.why,
        "lines": job.lines[start..],
        "next": job.dropped + job.lines.len(),
        "artifacts": job.artifacts.iter().filter(|a| a.done).map(|a| json!({
            "name": a.name,
            "size": a.size,
            "url": format!("{PREFIX}/{}/artifacts/{}", job.id, a.name),
        })).collect::<Vec<_>>(),
    })
}

/// The jobs as items on their Macs' boards (`GET /v1/agents`): the ones
/// still going and those that ended in the last day, so the phone and the
/// web show them with the computer's other work, and the phone's Approve,
/// Deny, and Stop reach them ([`act`]).
pub(crate) async fn board_items(store: &Store, owner: &str) -> Vec<(String, Item)> {
    let Ok(jobs) = list(store, owner).await else {
        return Vec::new();
    };
    let now = now_unix();
    jobs.iter()
        .filter(|job| {
            !job.state.finished()
                || job
                    .finished_unix
                    .is_some_and(|at| now.saturating_sub(at) < BOARD_SECONDS)
        })
        .take(crate::phone_api::MAX_ITEMS)
        .map(|job| (job.computer.clone(), item(job)))
        .collect()
}

/// One job as a board item.
pub(crate) fn item(job: &Job) -> Item {
    let status = match job.state {
        JobState::Waiting | JobState::Running => "working",
        JobState::Asking => "asking",
        JobState::Done => "done",
        JobState::Failed => "failed",
        JobState::Cancelled => "stopped",
    };
    Item {
        id: job.id.clone(),
        kind: "agent".into(),
        title: line(&job.spec.title(), 120),
        engine: Some("Mac".into()),
        status: status.into(),
        started_unix: job.created_unix,
        finished_unix: job.finished_unix,
        cost_usd: None,
        tokens: None,
        session: None,
        question: (job.state == JobState::Asking)
            .then(|| job.question.as_ref())
            .flatten()
            .map(|q| ItemQuestion {
                id: q.id.clone(),
                text: q.text.clone(),
            }),
        line: job.last_line().map(|text| line(&text, 200)),
    }
}

/// Whether a board item is a Mac job's.
pub(crate) fn is_job(item: &str) -> bool {
    valid_job_id(item)
}

/// The phone's Approve, Deny, or Stop on a job's board item. `Err` holds
/// the refusal.
pub(crate) async fn act(
    store: &Store,
    owner: &str,
    id: &str,
    action: &str,
    question: Option<&str>,
) -> Result<Result<(), (StatusCode, &'static str, &'static str)>, Error> {
    match action {
        "approve" | "deny" => {
            let answered = answer_question(
                store,
                owner,
                id,
                question.unwrap_or_default(),
                action == "approve",
                "phone",
            )
            .await?;
            Ok(match answered {
                Answered::Recorded => Ok(()),
                Answered::NotAsking => Err((
                    StatusCode::CONFLICT,
                    "not_asking",
                    "That job isn't waiting for an answer anymore.",
                )),
                Answered::Unknown => Err((
                    StatusCode::NOT_FOUND,
                    "unknown",
                    "That job isn't there anymore.",
                )),
            })
        }
        "stop" => Ok(match cancel(store, owner, id).await? {
            Some(()) => Ok(()),
            None => Err((
                StatusCode::NOT_FOUND,
                "unknown",
                "That job isn't there anymore.",
            )),
        }),
        _ => Ok(Err((
            StatusCode::CONFLICT,
            "unsupported",
            "A Mac job takes Approve, Deny, or Stop.",
        ))),
    }
}

/// The bytes of file `name` of job `id`, part after part.
pub(crate) async fn artifact_body(
    store: &Store,
    owner: &str,
    id: &str,
    name: &str,
) -> Result<Option<(u64, Body)>, Error> {
    let Some(job) = load(store, owner, id).await? else {
        return Ok(None);
    };
    let Some(found) = job.artifacts.iter().find(|a| a.name == name && a.done) else {
        return Ok(None);
    };
    let mut keys = Vec::new();
    for part in 0..found.parts {
        keys.push(part_key(owner, id, name, part)?);
    }
    let store = store.clone();
    let stream = futures_util::stream::unfold(keys.into_iter(), move |mut keys| {
        let store = store.clone();
        async move {
            let key = keys.next()?;
            let chunk = match store.read_key(&key).await {
                Ok(Some((bytes, _))) => Ok(Bytes::from(bytes)),
                _ => Err(std::io::Error::other("A part of the file is missing.")),
            };
            Some((chunk, keys))
        }
    });
    Ok(Some((found.size, Body::from_stream(stream))))
}

/// A file's media type, from its name.
pub(crate) fn media(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".json") {
        "application/json"
    } else if lower.ends_with(".txt") || lower.ends_with(".log") {
        "text/plain; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}

/// A file as a download: the owner's only, uncached, never sniffed.
pub(crate) fn download(name: &str, size: u64, body: Body) -> Response {
    let mut response = (StatusCode::OK, body).into_response();
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(media(name)) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from(size));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("sandbox; default-src 'none'"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

// ------------------------------------------------------------------- routes

#[derive(Deserialize)]
struct Reports {
    #[serde(default)]
    capabilities: Capabilities,
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
    let Some(sent) = serde_json::from_slice::<Reports>(&body)
        .ok()
        .filter(|_| !computer.is_empty())
    else {
        return refused(StatusCode::BAD_REQUEST, "invalid", "Send {capabilities}.");
    };
    let store = &app.config.chat_store;
    // A report is a check-in too: the Mac is online.
    if let Err(error) = coder_sync::check_in(store, &owner, &computer).await {
        return stored(&error);
    }
    if let Err(error) = report_capabilities(store, &owner, &computer, sent.capabilities).await {
        return stored(&error);
    }
    match take(store, &owner, &computer).await {
        Ok(jobs) => answer(StatusCode::OK, json!({"jobs": jobs})),
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
    if !valid_job_id(&id) {
        return refused(StatusCode::NOT_FOUND, "unknown", "There is no such job.");
    }
    let Ok(sent) = serde_json::from_slice::<Report>(&body) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send {lines, commit?, ask?, done?, failed?}.",
        );
    };
    let computer = line(&name, 64);
    match report(&app.config.chat_store, &owner, &computer, &id, sent).await {
        Ok(Some(heard)) => answer(StatusCode::OK, json!(heard)),
        Ok(None) => refused(StatusCode::NOT_FOUND, "unknown", "There is no such job."),
        Err(error) => stored(&error),
    }
}

#[derive(Deserialize)]
struct PartQuery {
    #[serde(default)]
    part: u32,
    #[serde(default)]
    last: u8,
}

async fn part_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path((name, id, file)): Path<(String, String, String)>,
    Query(query): Query<PartQuery>,
    body: Bytes,
) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    if !valid_job_id(&id) {
        return refused(StatusCode::NOT_FOUND, "unknown", "There is no such job.");
    }
    let computer = line(&name, 64);
    match save_part(
        &app.config.chat_store,
        &owner,
        &computer,
        &id,
        &file,
        query.part,
        query.last == 1,
        body.to_vec(),
    )
    .await
    {
        Ok(PartSaved::Saved) => answer(StatusCode::OK, json!({"saved": true})),
        Ok(PartSaved::Unknown) => {
            refused(StatusCode::NOT_FOUND, "unknown", "There is no such job.")
        }
        Ok(PartSaved::Refused(message)) => refused(StatusCode::BAD_REQUEST, "invalid", message),
        Err(error) => stored(&error),
    }
}

async fn macs_route(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match macs(&app.config.chat_store, &owner).await {
        Ok(found) => answer(
            StatusCode::OK,
            json!({"macs": found.into_iter().map(|(name, reported, online)| json!({
                "name": name,
                "online": online,
                "reported_unix": reported.reported_unix,
                "capabilities": reported.capabilities,
            })).collect::<Vec<_>>()}),
        ),
        Err(error) => stored(&error),
    }
}

async fn submit_route(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let sent = match serde_json::from_slice::<Submit>(&body) {
        Ok(sent) => sent,
        Err(_) => {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid",
                "Send {repo, ref, recipe, args?, computer?}; recipe is ios-release-gate, \
                 ios-testflight, desktop-capture, or xcodebuild.",
            );
        }
    };
    match submit(&app.config.chat_store, &owner, sent).await {
        Ok(Ok(queued)) => answer(
            StatusCode::CREATED,
            json!({
                "id": queued.id,
                "computer": queued.computer,
                "kind": queued.kind,
                "approval": queued.approval,
                "online": queued.online,
            }),
        ),
        Ok(Err(Refused::Invalid(why))) => refused(StatusCode::BAD_REQUEST, "invalid", &why),
        Ok(Err(Refused::NoMac(why))) => refused(StatusCode::CONFLICT, "no_mac", &why),
        Ok(Err(Refused::Busy)) => refused(
            StatusCode::CONFLICT,
            "busy",
            "Too many jobs are waiting for this account's Macs.",
        ),
        Err(error) => stored(&error),
    }
}

async fn list_route(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match list(&app.config.chat_store, &owner).await {
        Ok(jobs) => {
            let jobs: Vec<Value> = jobs
                .iter()
                .map(|job| {
                    let mut value = view(job, job.dropped + job.lines.len());
                    value["last_line"] = json!(job.last_line());
                    value
                })
                .collect();
            answer(StatusCode::OK, json!({"jobs": jobs}))
        }
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
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    let store = &app.config.chat_store;
    let until = Instant::now() + Duration::from_secs(query.wait.min(MAX_WAIT));
    let mut first: Option<(JobState, usize, Option<String>)> = None;
    loop {
        let job = match load(store, &owner, &id).await {
            Ok(Some(job)) => job,
            Ok(None) => return refused(StatusCode::NOT_FOUND, "unknown", "There is no such job."),
            Err(error) => return stored(&error),
        };
        let mark = (
            job.state,
            job.artifacts.iter().filter(|a| a.done).count(),
            job.question.as_ref().map(|q| q.id.clone()),
        );
        let news = job.dropped + job.lines.len() > query.after
            || job.state.finished()
            || first.as_ref().is_some_and(|first| *first != mark);
        if news || Instant::now() >= until {
            return answer(StatusCode::OK, view(&job, query.after));
        }
        first.get_or_insert(mark);
        tokio::time::sleep(LOOK_EVERY).await;
    }
}

async fn cancel_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    match cancel(&app.config.chat_store, &owner, &id).await {
        Ok(Some(())) => answer(StatusCode::OK, json!({"cancelled": true})),
        Ok(None) => refused(StatusCode::NOT_FOUND, "unknown", "There is no such job."),
        Err(error) => stored(&error),
    }
}

async fn artifact_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, file)): Path<(String, String)>,
) -> Response {
    let owner = match coder_sync::owner(&app, &headers).await {
        Ok(owner) => owner,
        Err(response) => return response,
    };
    if !valid_artifact(&file) {
        return refused(StatusCode::NOT_FOUND, "unknown", "There is no such file.");
    }
    match artifact_body(&app.config.chat_store, &owner, &id, &file).await {
        Ok(Some((size, body))) => download(&file, size, body),
        Ok(None) => refused(StatusCode::NOT_FOUND, "unknown", "There is no such file."),
        Err(error) => stored(&error),
    }
}

#[cfg(test)]
mod tests;
