//! Account-scoped run presence. Host reports do not grant execution authority.
use crate::{
    App,
    chat_store::{Error, Store},
    coder_sync::{answer, refused, stored},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    routing::{get, post, put},
};
use openagents_sdk::runs::{
    HEARTBEAT_INTERVAL_MS, LIVE_WINDOW_MS, RunFilter, RunHeartbeat, valid_id,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

const KEY: &str = "runs/presence.json";
const MAX_RUNS: usize = 512;
const RETENTION_MS: i64 = 24 * 60 * 60 * 1000;
const MAX_BODY: usize = 8192;

pub(crate) fn owns(path: &str) -> bool {
    path == "/v1/runs" || path.starts_with("/v1/runs/")
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[derive(Default, Serialize, Deserialize)]
struct Book {
    runs: BTreeMap<String, Entry>,
    /// Accepted writes in the last minute, including status changes.
    writes: Vec<i64>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    beat: RunHeartbeat,
    received_at: i64,
    ended_at: Option<i64>,
    writes: Vec<i64>,
}
#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    Invalid,
    Missing,
    Ended,
    Conflict,
    Limited(i64),
    Full,
}
impl Refusal {
    fn response(self) -> Response {
        let (status, code, message) = match self {
            Self::Invalid => (
                StatusCode::BAD_REQUEST,
                "invalid_run",
                "Check the run details and try again.",
            ),
            Self::Missing => (StatusCode::NOT_FOUND, "not_found", "No such run."),
            Self::Ended => (
                StatusCode::CONFLICT,
                "run_ended",
                "This run has ended. Start with a new run ID.",
            ),
            Self::Conflict => (
                StatusCode::CONFLICT,
                "run_conflict",
                "This run changed. Check its latest details.",
            ),
            Self::Limited(_) => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Wait before sending another heartbeat.",
            ),
            Self::Full => (
                StatusCode::TOO_MANY_REQUESTS,
                "quota_exhausted",
                "Your run list is full. Try again later.",
            ),
        };
        let mut response = refused(status, code, message);
        if let Self::Limited(ms) = self {
            response.headers_mut().insert(
                header::RETRY_AFTER,
                ((ms.max(1) + 999) / 1000).to_string().parse().unwrap(),
            );
        }
        response
    }
}
impl Book {
    fn prune(&mut self, at: i64) {
        self.runs
            .retain(|_, e| at.saturating_sub(e.ended_at.unwrap_or(e.received_at)) <= RETENTION_MS);
        self.writes
            .retain(|t| at.saturating_sub(*t) < HEARTBEAT_INTERVAL_MS);
        for e in self.runs.values_mut() {
            e.writes
                .retain(|t| at.saturating_sub(*t) < HEARTBEAT_INTERVAL_MS);
        }
    }
    fn heartbeat(&mut self, beat: &RunHeartbeat, at: i64) -> Result<bool, Refusal> {
        if !beat.valid()
            || beat.last_output_at > at.saturating_add(60_000)
            || beat
                .last_progress_at
                .is_some_and(|t| t > at.saturating_add(60_000))
        {
            return Err(Refusal::Invalid);
        }
        self.prune(at);
        if let Some(e) = self.runs.get(&beat.run) {
            if e.ended_at.is_some() {
                return Err(Refusal::Ended);
            }
            let elapsed = at.saturating_sub(e.received_at);
            // Retrying a lost response does not refresh presence or consume quota.
            if e.beat == *beat && elapsed < HEARTBEAT_INTERVAL_MS {
                return Ok(false);
            }
            if at < e.received_at
                || e.beat.kind != beat.kind
                || e.beat.harness != beat.harness
                || e.beat.computer != beat.computer
                || e.beat.agent != beat.agent
                || e.beat.harness_session != beat.harness_session
                || beat.last_output_at < e.beat.last_output_at
                || (e.beat.last_progress_at.is_some()
                    && beat.last_progress_at < e.beat.last_progress_at)
            {
                return Err(Refusal::Conflict);
            }
            if beat.status == e.beat.status && elapsed < HEARTBEAT_INTERVAL_MS {
                return Err(Refusal::Limited(HEARTBEAT_INTERVAL_MS - elapsed));
            }
            if e.writes.len() >= 12 {
                return Err(Refusal::Limited(
                    HEARTBEAT_INTERVAL_MS - at.saturating_sub(e.writes[0]),
                ));
            }
        } else if self.runs.len() >= MAX_RUNS {
            return Err(Refusal::Full);
        }
        if self.writes.len() >= 120 {
            return Err(Refusal::Limited(
                HEARTBEAT_INTERVAL_MS - at.saturating_sub(self.writes[0]),
            ));
        }
        let mut writes = self
            .runs
            .get(&beat.run)
            .map(|e| e.writes.clone())
            .unwrap_or_default();
        writes.push(at);
        self.runs.insert(
            beat.run.clone(),
            Entry {
                beat: beat.clone(),
                received_at: at,
                ended_at: None,
                writes,
            },
        );
        self.writes.push(at);
        Ok(true)
    }
    fn end(&mut self, id: &str, at: i64) -> Result<bool, Refusal> {
        self.prune(at);
        let e = self.runs.get_mut(id).ok_or(Refusal::Missing)?;
        if e.ended_at.is_some() {
            return Ok(false);
        }
        e.ended_at = Some(at);
        Ok(true)
    }
    fn list(&self, filter: RunFilter, at: i64) -> Vec<RunHeartbeat> {
        let mut rows: Vec<_> = self
            .runs
            .values()
            .filter(|e| {
                let age = at.saturating_sub(e.received_at);
                let live = e.ended_at.is_none() && age < LIVE_WINDOW_MS;
                at.saturating_sub(e.ended_at.unwrap_or(e.received_at)) <= RETENTION_MS
                    && filter.live.is_none_or(|want| want == live)
                    && filter.issue.is_none_or(|n| e.beat.issues.contains(&n))
            })
            .collect();
        rows.sort_by(|a, b| {
            b.received_at
                .cmp(&a.received_at)
                .then_with(|| a.beat.run.cmp(&b.beat.run))
        });
        rows.into_iter().map(|e| e.beat.clone()).collect()
    }
}
async fn load(store: &Store, owner: &str) -> Result<(Book, Option<String>), Error> {
    match store.read_key(&Store::owner_key(owner, KEY)?).await? {
        Some((bytes, generation)) => Ok((
            serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("The run list could not be read."))?,
            Some(generation),
        )),
        None => Ok((Book::default(), None)),
    }
}

/// One account's run report and the server times that determine its presence.
#[derive(Clone)]
pub(crate) struct RunPresence {
    pub(crate) beat: RunHeartbeat,
    pub(crate) received_at: i64,
    pub(crate) ended_at: Option<i64>,
}

impl RunPresence {
    pub(crate) fn is_live(&self, at: i64) -> bool {
        self.ended_at.is_none() && at.saturating_sub(self.received_at) < LIVE_WINDOW_MS
    }
}

impl From<Entry> for RunPresence {
    fn from(entry: Entry) -> Self {
        Self {
            beat: entry.beat,
            received_at: entry.received_at,
            ended_at: entry.ended_at,
        }
    }
}

/// Retained reports for an account. Callers authenticate before choosing `owner`.
pub(crate) async fn account_runs(store: &Store, owner: &str) -> Result<Vec<RunPresence>, Error> {
    let (mut book, _) = load(store, owner).await?;
    book.prune(now());
    Ok(book.runs.into_values().map(RunPresence::from).collect())
}

/// A retained run at `at`. Callers authenticate before choosing `owner`.
pub(crate) async fn lookup(
    store: &Store,
    owner: &str,
    id: &str,
    at: i64,
) -> Result<Option<RunPresence>, Error> {
    let (mut book, _) = load(store, owner).await?;
    book.prune(at);
    Ok(book.runs.remove(id).map(RunPresence::from))
}

async fn change(
    store: &Store,
    owner: &str,
    at: i64,
    f: impl Fn(&mut Book, i64) -> Result<bool, Refusal>,
) -> Result<Result<(), Refusal>, Error> {
    for _ in 0..8 {
        let (mut book, generation) = load(store, owner).await?;
        match f(&mut book, at) {
            Err(e) => return Ok(Err(e)),
            Ok(false) => return Ok(Ok(())),
            Ok(true) => {}
        }
        let bytes =
            serde_json::to_vec(&book).map_err(|_| Error::Invalid("The run could not be saved."))?;
        match store
            .write_key(&Store::owner_key(owner, KEY)?, bytes, generation.as_deref())
            .await
        {
            Ok(_) => return Ok(Ok(())),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}
fn changed(result: Result<Result<(), Refusal>, Error>) -> Response {
    match result {
        Ok(Ok(())) => answer(StatusCode::OK, json!({"ok": true})),
        Ok(Err(e)) => e.response(),
        Err(e) => stored(&e),
    }
}
pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/runs", get(list))
        .route("/v1/runs/{id}/heartbeat", put(heartbeat))
        .route("/v1/runs/{id}/end", post(end))
        .layer(DefaultBodyLimit::max(MAX_BODY))
}
async fn list(
    State(app): State<App>,
    headers: HeaderMap,
    query: Result<Query<RunFilter>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let Ok(Query(filter)) = query else {
        return Refusal::Invalid.response();
    };
    if filter.issue == Some(0) {
        return Refusal::Invalid.response();
    }
    match load(&app.config.chat_store, &owner).await {
        Ok((book, _)) => answer(StatusCode::OK, json!({"runs": book.list(filter, now())})),
        Err(e) => stored(&e),
    }
}
async fn heartbeat(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Bytes, axum::extract::rejection::BytesRejection>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let body = match body {
        Ok(body) => body,
        Err(error) => return refused(error.status(), "too_large", "Send a smaller heartbeat."),
    };
    let Ok(beat) = serde_json::from_slice::<RunHeartbeat>(&body) else {
        return Refusal::Invalid.response();
    };
    if beat.run != id {
        return Refusal::Invalid.response();
    }
    changed(
        change(&app.config.chat_store, &owner, now(), |b, t| {
            b.heartbeat(&beat, t)
        })
        .await,
    )
}
async fn end(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    if !valid_id(&id) {
        return Refusal::Invalid.response();
    }
    changed(change(&app.config.chat_store, &owner, now(), |b, t| b.end(&id, t)).await)
}

#[cfg(test)]
mod tests;
