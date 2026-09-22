//! `/v1/jobs` — durable classification jobs.
//!
//! A submission is the `openagents.classify.v1` envelope `POST
//! /v1/classify` takes, persisted before any forward runs: a crash is a
//! status read, not a lost run. Status, cancel, results, and delete
//! complete the lifecycle. An `Idempotency-Key` on submission dedupes a
//! retried post; a settled key against changed content answers
//! `idempotency_conflict` rather than running twice.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Result;
use crate::client::Client;
use crate::error::Error;
use crate::options::CallOptions;
use crate::transport::parse_body;
use reqwest::Method;

/// The route family this module calls.
const PATH: &str = "/v1/jobs";

/// The job surface, scoped to its client.
#[derive(Debug)]
pub struct Jobs<'a> {
    client: &'a Client,
}

impl<'a> Jobs<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Submit one job and read its queued status document.
    ///
    /// An explicit `Idempotency-Key` on `options` dedupes a retried
    /// submission; a keyless submission mints a random one and cannot
    /// dedupe a retry.
    ///
    /// # Errors
    ///
    /// Returns the shared call errors, plus [`Error::Config`] when the
    /// envelope is not JSON, and [`Error::ResponseValidation`] when a
    /// successful body does not decode.
    pub async fn submit(&self, request: JobSubmit) -> Result<JobStatus> {
        let body = serde_json::to_vec(&request.body())
            .map_err(|error| Error::Config(format!("the submission is not JSON: {error}")))?;
        let raw = self
            .client
            .request_read(
                Method::POST,
                PATH,
                Some(body),
                &request.options.headers,
                request.options.timeout,
                request.options.retry.clone(),
            )
            .await?;
        decode::<JobEnvelope>(&raw.bytes, raw.status, &raw.headers).map(|found| found.job)
    }

    /// Read one job's status document — state, counts, timing, digests,
    /// and the terminal receipt when finished.
    ///
    /// # Errors
    ///
    /// Returns the shared call errors; an unknown or another caller's
    /// id is [`Error::Api`] with code `job_not_found`.
    pub async fn status(&self, id: &str) -> Result<JobStatus> {
        self.read(Method::GET, &path(id), &CallOptions::new())
            .await
            .map(|found: JobEnvelope| found.job)
    }

    /// Read one job's status document under the caller's options.
    pub async fn status_with(&self, id: &str, options: &CallOptions) -> Result<JobStatus> {
        self.read(Method::GET, &path(id), options)
            .await
            .map(|found: JobEnvelope| found.job)
    }

    /// Mark a live job cancelled — the status read reports `cancelling`
    /// until the runner settles it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Api`] with `job_terminal` when the job already
    /// ended, and `job_not_found` for an unknown id.
    pub async fn cancel(&self, id: &str) -> Result<JobStatus> {
        self.read(
            Method::POST,
            &format!("{}/cancel", path(id)),
            &CallOptions::new(),
        )
        .await
        .map(|found: JobEnvelope| found.job)
    }

    /// Export a terminal job's items — one bounded page. `next_cursor`
    /// names the following page while one remains; an expired cursor
    /// answers `cursor_expired`, never a silent restart.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Api`] with `job_running` while the job still
    /// runs and `cursor_expired` for a stale cursor.
    pub async fn results(&self, id: &str, query: &ResultsQuery) -> Result<ResultsPage> {
        let mut path = format!("{}/results", path(id));
        let params = query.params();
        if !params.is_empty() {
            path.push('?');
            path.push_str(&params);
        }
        self.read(Method::GET, &path, &query.options)
            .await
            .map(|found: ResultsEnvelope| found.into_page())
    }

    /// Remove a terminal job's record before the retention sweep would.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Api`] with `job_running` for a live job and
    /// `job_not_found` for an unknown id.
    pub async fn remove(&self, id: &str) -> Result<bool> {
        self.read(Method::DELETE, &path(id), &CallOptions::new())
            .await
            .map(|found: Deleted| found.deleted)
    }

    /// The read every non-submission call shares.
    async fn read<T>(&self, method: Method, path: &str, options: &CallOptions) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let raw = self
            .client
            .request_read(
                method,
                path,
                None,
                &options.headers,
                options.timeout,
                options.retry.clone(),
            )
            .await?;
        decode(&raw.bytes, raw.status, &raw.headers)
    }
}

/// One job's route, `/v1/jobs/{id}`.
fn path(id: &str) -> String {
    format!("{PATH}/{id}")
}

/// A submission: the classify envelope, the optional notify and export
/// policy, and the call's overrides.
#[derive(Debug, Clone)]
pub struct JobSubmit {
    classify: Value,
    notify: Option<Value>,
    options: CallOptions,
}

impl JobSubmit {
    /// A submission over one `openagents.classify.v1` envelope — the
    /// same document `POST /v1/classify` takes.
    #[must_use]
    pub fn new(classify: Value) -> Self {
        Self {
            classify,
            notify: None,
            options: CallOptions::new(),
        }
    }

    /// The webhook the gateway signs and posts on terminal state —
    /// `{url, events}` as the service documents it.
    #[must_use]
    pub fn notify(mut self, notify: Value) -> Self {
        self.notify = Some(notify);
        self
    }

    /// Options for this call — retry, timeout, headers, the
    /// idempotency key a retried submission shares.
    #[must_use]
    pub fn options(mut self, options: CallOptions) -> Self {
        self.options = options;
        self
    }

    /// The wire body: `{classify, notify?}`.
    fn body(&self) -> Value {
        let mut body = serde_json::Map::new();
        body.insert("classify".to_string(), self.classify.clone());
        if let Some(notify) = &self.notify {
            body.insert("notify".to_string(), notify.clone());
        }
        Value::Object(body)
    }
}

/// A results read's page controls.
#[derive(Debug, Clone, Default)]
pub struct ResultsQuery {
    /// The opaque cursor the previous page's `next_cursor` returned.
    pub cursor: Option<String>,
    /// The page size to ask for, bounded by the service.
    pub limit: Option<u64>,
    /// The call's overrides.
    pub options: CallOptions,
}

impl ResultsQuery {
    /// The first page.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Continue from a previous page's cursor.
    #[must_use]
    pub fn cursor(mut self, cursor: impl Into<String>) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Ask for a page size.
    #[must_use]
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Options for this call.
    #[must_use]
    pub fn options(mut self, options: CallOptions) -> Self {
        self.options = options;
        self
    }

    /// The query string these controls encode.
    fn params(&self) -> String {
        let mut params = url::form_urlencoded::Serializer::new(String::new());
        if let Some(cursor) = &self.cursor {
            params.append_pair("cursor", cursor);
        }
        if let Some(limit) = self.limit {
            params.append_pair("limit", &limit.to_string());
        }
        params.finish()
    }
}

/// The status document every job route answers inside `job`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct JobStatus {
    /// The schema the document declares.
    pub v: String,
    /// The job's id.
    pub job: String,
    /// `classify` — the only kind today.
    pub kind: String,
    /// `queued`, `running`, `cancelling`, `succeeded`, `failed`, or
    /// `cancelled`.
    pub status: String,
    /// The door the job runs on.
    pub model: String,
    /// The capacity lane it runs on.
    pub capacity: String,
    /// The outcome tally.
    pub counts: JobCounts,
    /// When the job was accepted.
    pub submitted_at: String,
    /// When the first forward ran.
    #[serde(default)]
    pub started_at: Option<String>,
    /// When the job settled.
    #[serde(default)]
    pub finished_at: Option<String>,
    /// The terminal receipt, once the job has one.
    #[serde(default)]
    pub receipt: Option<Value>,
    /// Why the job ended the way it did, when it did not succeed.
    #[serde(default)]
    pub cause: Option<String>,
}

/// The outcome tally a status document reports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct JobCounts {
    /// Inputs the submission declared.
    #[serde(default)]
    pub expected: u64,
    /// Inputs dispatched at least once.
    #[serde(default)]
    pub attempted: u64,
    /// Inputs answered.
    #[serde(default)]
    pub answered: u64,
    /// Inputs refused.
    #[serde(default)]
    pub refused: u64,
    /// Inputs the door could not reach.
    #[serde(default)]
    pub unavailable: u64,
    /// Inputs never dispatched — a refused run says so honestly.
    #[serde(default)]
    pub unattempted: u64,
    /// Inputs whose dispatch may have run — a crashed run's honest
    /// count, never silently either way.
    #[serde(default)]
    pub unknown: u64,
}

/// One page of a terminal job's items.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultsPage {
    /// The finished per-input results, in submission order.
    pub items: Vec<Value>,
    /// The cursor the next page asks with, while more remain.
    pub next_cursor: Option<String>,
    /// The job's state when the page was read.
    pub terminal: String,
    /// The job's outcome tally when the page was read.
    pub counts: JobCounts,
}

/// The `{job}` envelope the status routes answer.
#[derive(Deserialize)]
struct JobEnvelope {
    job: JobStatus,
}

/// The results route's envelope.
#[derive(Deserialize)]
struct ResultsEnvelope {
    items: Vec<Value>,
    #[serde(default)]
    next_cursor: Option<String>,
    terminal: String,
    counts: JobCounts,
}

impl ResultsEnvelope {
    fn into_page(self) -> ResultsPage {
        ResultsPage {
            items: self.items,
            next_cursor: self.next_cursor,
            terminal: self.terminal,
            counts: self.counts,
        }
    }
}

/// The delete route's answer.
#[derive(Deserialize)]
struct Deleted {
    deleted: bool,
}

/// Decode one successful body, naming the route's shape on a mismatch.
fn decode<T>(bytes: &[u8], status: u16, headers: &reqwest::header::HeaderMap) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_slice::<T>(bytes).map_err(|error| Error::ResponseValidation {
        status,
        field_path: format!("job document: {error}"),
        body: parse_body(bytes).map(Box::new),
        request_id: headers
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
    })
}
