//! The account's attention stream: events derived from run presence and work
//! runs, kept in a per-account log so clients resume from a cursor.
use crate::{
    App,
    chat_store::{Error, Store},
    coder_sync::{refused, stored},
    runs::{RunPresence, account_runs},
    work_runs::{self, RunState, WorkRun},
};
use axum::{
    Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::get,
};
use openagents_sdk::{
    attention::{Attention, AttentionEvent, CLAIM_IDLE_AFTER_MS, STALL_AFTER_MS},
    runs::RunKind,
    sessions::SessionStatus,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, convert::Infallible, time::Duration};

const KEY: &str = "attention/log.json";
/// Events kept per account; older ones are dropped.
const MAX_EVENTS: usize = 1000;
/// Sources older than this raise no new events.
const WINDOW_MS: i64 = 24 * 60 * 60 * 1000;
/// Dedupe keys outlive the window so an old condition never repeats.
const SEEN_MS: i64 = 2 * WINDOW_MS;
/// At most one sweep of the sources per account in this interval.
const SWEEP_EVERY_MS: i64 = 10_000;
const LOOK_EVERY: Duration = Duration::from_secs(2);
/// One stream lasts about two hours; clients reconnect with their cursor.
const MAX_TICKS: u32 = 3_600;

pub(crate) fn owns(path: &str) -> bool {
    path == "/v1/attention"
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

/// A finished work run, as the sweep reads it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WorkFact {
    pub(crate) id: String,
    pub(crate) issue: u64,
    pub(crate) state: RunState,
    pub(crate) finished_at: i64,
    pub(crate) landing: Option<String>,
    pub(crate) queued: bool,
    pub(crate) why: Option<String>,
}
impl WorkFact {
    fn from_run(run: &WorkRun) -> Option<Self> {
        if !run.state.finished() {
            return None;
        }
        let finished = run.finished_unix.unwrap_or(run.updated_unix);
        Some(Self {
            id: run.id.clone(),
            issue: run.issue,
            state: run.state,
            finished_at: (finished as i64).saturating_mul(1000),
            landing: run.outcome.as_ref().and_then(|o| o.landing.clone()),
            queued: run.land == work_runs::Land::Queue,
            why: run.why.clone(),
        })
    }
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Log {
    next: u64,
    events: Vec<AttentionEvent>,
    /// Dedupe key to the time it was first raised.
    seen: BTreeMap<String, i64>,
    /// `run:issue` to when it was first seen holding the issue with no progress time.
    watch: BTreeMap<String, i64>,
    swept_at: i64,
}
impl Log {
    fn raise(&mut self, key: String, at: i64, agent: Option<String>, attention: Attention) -> bool {
        if self.seen.contains_key(&key) {
            return false;
        }
        self.seen.insert(key, at);
        self.next += 1;
        self.events.push(AttentionEvent {
            seq: self.next,
            at,
            agent,
            attention,
        });
        true
    }

    /// Raise each new condition once. Returns whether it raised any event.
    pub(crate) fn sweep(&mut self, runs: &[RunPresence], work: &[WorkFact], at: i64) -> bool {
        let before = self.next;
        self.seen.retain(|_, t| at.saturating_sub(*t) <= SEEN_MS);
        let mut watching = BTreeMap::new();
        let mut runs: Vec<_> = runs.iter().collect();
        runs.sort_by(|a, b| {
            a.received_at
                .cmp(&b.received_at)
                .then_with(|| a.beat.run.cmp(&b.beat.run))
        });
        for p in runs {
            let b = &p.beat;
            if p.ended_at.is_some() || at.saturating_sub(p.received_at) > WINDOW_MS {
                continue;
            }
            let agent = b.agent.clone();
            let note = b.note.clone().unwrap_or_default();
            match b.status {
                SessionStatus::Working if at.saturating_sub(b.last_output_at) >= STALL_AFTER_MS => {
                    self.raise(
                        format!("stalled:{}:{}", b.run, b.last_output_at),
                        at,
                        agent.clone(),
                        Attention::RunStalled {
                            run: b.run.clone(),
                            since: b.last_output_at,
                        },
                    );
                }
                SessionStatus::AwaitingInput => {
                    self.raise(
                        format!("input:{}:{}", b.run, b.last_output_at),
                        at,
                        agent.clone(),
                        Attention::RunAwaitingInput {
                            run: b.run.clone(),
                            question: note,
                        },
                    );
                }
                SessionStatus::Errored if b.kind == RunKind::Landing => {
                    self.raise(
                        format!("landing:{}:{}", b.run, b.last_output_at),
                        at,
                        agent.clone(),
                        Attention::LandingFailed {
                            entry: b.run.clone(),
                            issue: b.issues.first().copied(),
                            reason: note,
                        },
                    );
                }
                SessionStatus::Errored => {
                    self.raise(
                        format!("errored:{}:{}", b.run, b.last_output_at),
                        at,
                        agent.clone(),
                        Attention::RunErrored {
                            run: b.run.clone(),
                            reason: note,
                        },
                    );
                }
                _ => {}
            }
            if !p.is_live(at) {
                continue;
            }
            let holder = b.harness_session.clone().unwrap_or_else(|| b.run.clone());
            for issue in &b.issues {
                let since = match b.last_progress_at {
                    Some(t) => t,
                    None => {
                        let key = format!("{}:{issue}", b.run);
                        let first = *self.watch.get(&key).unwrap_or(&at);
                        watching.insert(key, first);
                        first
                    }
                };
                if at.saturating_sub(since) >= CLAIM_IDLE_AFTER_MS {
                    self.raise(
                        format!("claim:{issue}:{}:{since}", b.run),
                        at,
                        agent.clone(),
                        Attention::ClaimNoProgress {
                            issue: *issue,
                            holder: holder.clone(),
                            since,
                        },
                    );
                }
            }
        }
        self.watch = watching;
        let mut work: Vec<_> = work
            .iter()
            .filter(|w| at.saturating_sub(w.finished_at) <= WINDOW_MS)
            .collect();
        work.sort_by(|a, b| {
            a.finished_at
                .cmp(&b.finished_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        for w in work {
            if w.state == RunState::Failed && (w.queued || w.landing.is_some()) {
                self.raise(
                    format!("work-landing:{}", w.id),
                    at,
                    None,
                    Attention::LandingFailed {
                        entry: w.landing.clone().unwrap_or_else(|| w.id.clone()),
                        issue: Some(w.issue),
                        reason: w.why.clone().unwrap_or_default(),
                    },
                );
            }
            self.raise(
                format!("work:{}", w.id),
                at,
                None,
                Attention::WorkFinished {
                    work: w.id.clone(),
                    issue: w.issue,
                    outcome: w.state.word().to_owned(),
                },
            );
        }
        if self.events.len() > MAX_EVENTS {
            self.events.drain(..self.events.len() - MAX_EVENTS);
        }
        self.swept_at = at;
        self.next != before
    }

    pub(crate) fn after(&self, cursor: u64) -> impl Iterator<Item = &AttentionEvent> {
        self.events.iter().filter(move |e| e.seq > cursor)
    }
}

async fn load(store: &Store, owner: &str) -> Result<(Log, Option<String>), Error> {
    match store.read_key(&Store::owner_key(owner, KEY)?).await? {
        Some((bytes, generation)) => Ok((
            serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("The attention log could not be read."))?,
            Some(generation),
        )),
        None => Ok((Log::default(), None)),
    }
}

/// Read the sources and record new events, at most once per sweep interval.
pub(crate) async fn sweep(store: &Store, owner: &str, at: i64) -> Result<Log, Error> {
    for _ in 0..8 {
        let (mut log, generation) = load(store, owner).await?;
        if generation.is_some() && at.saturating_sub(log.swept_at) < SWEEP_EVERY_MS {
            return Ok(log);
        }
        let runs = account_runs(store, owner).await?;
        let work: Vec<_> = work_runs::list(store, owner)
            .await?
            .iter()
            .filter_map(WorkFact::from_run)
            .collect();
        log.sweep(&runs, &work, at);
        let bytes = serde_json::to_vec(&log)
            .map_err(|_| Error::Invalid("The attention log could not be saved."))?;
        match store
            .write_key(&Store::owner_key(owner, KEY)?, bytes, generation.as_deref())
            .await
        {
            Ok(_) => return Ok(log),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/v1/attention", get(stream_route))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    after: Option<u64>,
}

fn event(e: &AttentionEvent) -> Event {
    Event::default()
        .event("attention")
        .id(e.seq.to_string())
        .data(serde_json::to_string(e).unwrap_or_default())
}

async fn stream_route(
    State(app): State<App>,
    headers: HeaderMap,
    query: Result<Query<Cursor>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let Ok(Query(cursor)) = query else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Check the event number and try again.",
        );
    };
    let last_id = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok());
    let after = cursor.after.or(last_id).unwrap_or(0);
    let store = app.config.chat_store.clone();
    if let Err(e) = sweep(&store, &owner, now()).await {
        return stored(&e);
    }
    let shutdown = app.config.shutdown.clone();
    let stream = futures_util::stream::unfold(
        (store, owner, after, 0u32, false),
        |(store, owner, next, ticks, ended)| async move {
            if ended || ticks >= MAX_TICKS {
                return None;
            }
            if ticks > 0 {
                tokio::time::sleep(LOOK_EVERY).await;
            }
            let mut events = Vec::new();
            let mut next = next;
            let mut ended = false;
            match sweep(&store, &owner, now()).await {
                Ok(log) => {
                    for e in log.after(next) {
                        events.push(event(e));
                        next = e.seq;
                    }
                }
                Err(_) => ended = true,
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
            Some((batch, (store, owner, next, ticks + 1, ended)))
        },
    );
    use futures_util::StreamExt as _;
    crate::cloud::protect(
        Sse::new(shutdown.until(stream.flatten()))
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
            .into_response(),
    )
}

#[cfg(test)]
mod tests;
