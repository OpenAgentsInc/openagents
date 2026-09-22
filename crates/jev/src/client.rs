//! The client, the request it sends, and the loop that retries one.
//!
//! One attempt is one HTTP round trip under one timeout. A failure the policy
//! retries waits and runs again, and the wait is the policy's own unless the
//! server asked for a longer one. Every attempt and every retry is one `info`
//! line under the `jev` target.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Method, StatusCode};
use serde_json::{Map, Value};

use crate::Result;
use crate::account::Account;
use crate::answers::{RawResponse, SystemOneResponse};
use crate::classify::Classify;
use crate::config::{ApiKey, Config};
use crate::error::{ApiError, Error, REQUEST_ID_HEADER};
use crate::jobs::Jobs;
use crate::models::{ListOptions, Models};
use crate::questions::{Entry, Questions};
use crate::retry::{RETRY_COUNT_HEADER, RetryPolicy};
use crate::transport;

/// The route that answers questions.
const SYSTEM_ONE_PATH: &str = "/v1/systemone";

/// The route that lists models.
const MODELS_PATH: &str = "/v1/models";

/// One state, the questions to ask about it, and what this call overrides.
///
/// ```
/// use jev::{Noul, Questions, SystemOneRequest};
///
/// let request = SystemOneRequest::new(
///     "I was charged twice for one order.",
///     Questions::new().with("billing", Noul::new("Is this about a charge?")),
/// )
/// .model("jev-latest");
/// ```
#[derive(Debug, Clone)]
pub struct SystemOneRequest {
    /// The state every question reads. One state, not a batch.
    pub state: Entry,
    /// The questions to ask.
    pub questions: Questions,
    /// The model to ask, or the client's default.
    pub model: Option<String>,
    /// The policy to retry this call by, or the client's own.
    pub retry: Option<RetryPolicy>,
    /// The timeout each attempt of this call takes, or the client's own.
    pub timeout: Option<Duration>,
    /// Headers this call adds.
    pub headers: HeaderMap,
    /// Fields to merge into the body last, for a field the API takes and this
    /// crate does not model. A field here replaces one the SDK wrote.
    pub extra_body: Map<String, Value>,
}

impl SystemOneRequest {
    /// Ask questions about one state.
    #[must_use]
    pub fn new<E: Into<Entry>>(state: E, questions: Questions) -> Self {
        Self {
            state: state.into(),
            questions,
            model: None,
            retry: None,
            timeout: None,
            headers: HeaderMap::new(),
            extra_body: Map::new(),
        }
    }

    /// Ask another model than the client's default.
    #[must_use]
    pub fn model<M: Into<String>>(mut self, model: M) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Retry this call by another policy.
    ///
    /// The policy replaces the client's. Build one over the client's own with
    /// struct update syntax when you mean to change one field:
    /// `RetryPolicy { max_retries: 0, ..client.retry().clone() }`.
    #[must_use]
    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = Some(retry);
        self
    }

    /// Give each attempt of this call another timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Add headers to this call.
    #[must_use]
    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    /// Merge fields into the body last.
    #[must_use]
    pub fn extra_body(mut self, extra: Map<String, Value>) -> Self {
        self.extra_body = extra;
        self
    }

    /// The body one call of this request sends, with `default_model` where
    /// the request names no model of its own. Nothing goes out: a caller
    /// that records the exchange reads it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Question`] when the question set fails its checks and
    /// [`Error::Config`] when the questions do not serialize.
    pub fn body(&self, default_model: &str) -> Result<Map<String, Value>> {
        self.questions.validate()?;
        let mut body = Map::new();
        body.insert("state".to_string(), self.state.to_value());
        body.insert(
            "model".to_string(),
            Value::String(
                self.model
                    .clone()
                    .unwrap_or_else(|| default_model.to_string()),
            ),
        );
        body.insert(
            "questions".to_string(),
            serde_json::to_value(&self.questions)
                .map_err(|error| Error::Config(format!("the questions are not JSON: {error}")))?,
        );
        for (name, value) in &self.extra_body {
            body.insert(name.clone(), value.clone());
        }
        Ok(body)
    }
}

/// A client for TypeSafe's System One API.
///
/// The client is cheap to clone and holds one HTTP connection pool, so build
/// one and share it.
#[derive(Debug, Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

/// What every request reads, behind one allocation.
#[derive(Debug)]
struct Inner {
    api_key: Option<ApiKey>,
    base_url: String,
    default_model: String,
    timeout: Duration,
    retry: RetryPolicy,
    default_headers: HeaderMap,
    http: reqwest::Client,
    requests: AtomicU64,
}

impl Client {
    /// Build a client from settings, filling in what they leave out from the
    /// environment and the defaults.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] when no key is set, the base URL is not an
    /// http or https URL, the timeout is zero, or a retry field is out of
    /// range.
    pub fn new(config: Config) -> Result<Self> {
        let resolved = config.resolve()?;
        Ok(Self {
            inner: Arc::new(Inner {
                api_key: resolved.api_key,
                base_url: resolved.base_url,
                default_model: resolved.default_model,
                timeout: resolved.timeout,
                retry: resolved.retry,
                default_headers: resolved.default_headers,
                http: resolved.http,
                requests: AtomicU64::new(0),
            }),
        })
    }

    /// Build a client from the environment and the defaults alone.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Client::new`].
    pub fn from_env() -> Result<Self> {
        Self::new(Config::new())
    }

    /// The API root, with trailing slashes dropped.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.inner.base_url
    }

    /// The model a request that names none asks.
    #[must_use]
    pub fn default_model(&self) -> &str {
        &self.inner.default_model
    }

    /// How long one attempt may take.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.inner.timeout
    }

    /// What the client retries and how long it waits.
    #[must_use]
    pub fn retry(&self) -> &RetryPolicy {
        &self.inner.retry
    }

    /// The headers every request carries, before the ones the API reads to
    /// identify it.
    #[must_use]
    pub fn default_headers(&self) -> &HeaderMap {
        &self.inner.default_headers
    }

    /// The models the account can ask.
    #[must_use]
    pub fn models(&self) -> Models<'_> {
        Models::new(self)
    }

    /// `POST /v1/classify` — an ordered batch under one selection
    /// policy, with review and fallback as policy sub-documents.
    #[must_use]
    pub fn classify(&self) -> Classify<'_> {
        Classify::new(self)
    }

    /// `/v1/jobs` — durable classification jobs: submit, status, cancel,
    /// results, and delete.
    #[must_use]
    pub fn jobs(&self) -> Jobs<'_> {
        Jobs::new(self)
    }

    /// The caller's account surface — the session the bearer names, the
    /// account and its workspaces, the monetary balance, and workspace
    /// usage, each routed only where the deployment mounts it.
    #[must_use]
    pub fn account(&self) -> Account<'_> {
        Account::new(self)
    }

    /// Ask questions about one state and read the answers.
    ///
    /// The policy's `budget`, when set, is a monotonic deadline for the whole
    /// call, including the first attempt, the body read, every wait, and
    /// every retry; each attempt's timeout is capped at the time the call has
    /// left.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Question`] when the question set fails its checks,
    /// before any request. Returns [`Error::Api`], [`Error::Connection`], or
    /// [`Error::Timeout`] when the call fails after its retries or runs out
    /// of budget, and [`Error::ResponseValidation`] when the answers do not
    /// read or do not hold to their numeric contract. Returns
    /// [`Error::MissingAnswer`] or [`Error::AnswerType`] when a question
    /// went unanswered or was answered in another type; a Choice or Score
    /// answered over other options than the question named is
    /// [`Error::ResponseValidation`]. A caller that wants the bytes
    /// regardless reads [`Client::system_one_raw`].
    pub async fn system_one(&self, request: SystemOneRequest) -> Result<SystemOneResponse> {
        let prepared = self.prepare_system_one(&request)?;
        let raw = self.send_read(&prepared).await?;
        let response = SystemOneResponse::decode(raw)?;
        response.check_against(&request.questions)?;
        Ok(response)
    }

    /// Ask questions about one state and hand back the response unread, for a
    /// caller that wants the bytes.
    ///
    /// The call is bounded the way [`Client::system_one`] is, and the
    /// returned response's body stays under the same deadline: the attempt's
    /// timeout — capped by the remaining budget — still applies while the
    /// caller reads the body, so a read past the deadline fails rather than
    /// outliving the call's budget.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Client::system_one`], other than
    /// [`Error::ResponseValidation`].
    pub async fn system_one_raw(&self, request: SystemOneRequest) -> Result<reqwest::Response> {
        let prepared = self.prepare_system_one(&request)?;
        self.send(&prepared).await
    }

    /// Everything one `POST /v1/systemone` call sends, before its first
    /// attempt.
    fn prepare_system_one(&self, request: &SystemOneRequest) -> Result<Prepared> {
        let body = request.body(&self.inner.default_model)?;
        let encoded = serde_json::to_vec(&Value::Object(body))
            .map_err(|error| Error::Config(format!("the request body is not JSON: {error}")))?;
        self.prepare(
            Method::POST,
            SYSTEM_ONE_PATH,
            Some(encoded),
            &request.headers,
            request.timeout,
            request.retry.clone(),
        )
    }

    /// One `GET /v1/models`, read into bytes.
    pub(crate) async fn list_models(&self, options: ListOptions) -> Result<RawResponse> {
        let prepared = self.prepare(
            Method::GET,
            MODELS_PATH,
            None,
            &options.headers,
            options.timeout,
            options.retry,
        )?;
        self.send_read(&prepared).await
    }

    /// One call of any of the service's routes, read into bytes: the
    /// shared retry, timeout, budget, and header path every module here
    /// uses.
    pub(crate) async fn request_read(
        &self,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
        headers: &HeaderMap,
        timeout: Option<Duration>,
        retry: Option<RetryPolicy>,
    ) -> Result<RawResponse> {
        let prepared = self.prepare(method, path, body, headers, timeout, retry)?;
        self.send_read(&prepared).await
    }

    /// Everything one call sends, before its first attempt.
    fn prepare(
        &self,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
        headers: &HeaderMap,
        timeout: Option<Duration>,
        retry: Option<RetryPolicy>,
    ) -> Result<Prepared> {
        let timeout = timeout.unwrap_or(self.inner.timeout);
        if timeout.is_zero() {
            return Err(Error::Config(
                "`timeout` must be a positive duration".to_string(),
            ));
        }
        let retry = match retry {
            Some(retry) => {
                retry.validate()?;
                retry
            }
            None => self.inner.retry.clone(),
        };
        let headers = transport::headers(
            &self.inner.default_headers,
            headers,
            self.inner.api_key.as_ref(),
            body.is_some(),
        )?;
        Ok(Prepared {
            tag: self.inner.requests.fetch_add(1, Ordering::Relaxed) + 1,
            url: format!("{}{path}", self.inner.base_url),
            endpoint: format!("{method} {}{path}", endpoint_of(&self.inner.base_url)),
            method,
            body,
            headers,
            timeout,
            retry,
        })
    }

    /// One call: the first attempt and every retry the policy allows.
    async fn send(&self, prepared: &Prepared) -> Result<reqwest::Response> {
        self.send_with(prepared, Client::attempt).await
    }

    /// One call whose body is read inside each attempt, so a body that stalls
    /// or breaks is retried the way both official SDKs retry one.
    async fn send_read(&self, prepared: &Prepared) -> Result<RawResponse> {
        self.send_with(prepared, Client::attempt_read).await
    }

    /// The retry loop both send paths share.
    ///
    /// `retry.budget`, when set, is a monotonic deadline for the whole call,
    /// measured from the first attempt's dispatch: it covers the first
    /// attempt, every retry, every wait between them, and the body a read
    /// path consumes inside its attempt. Each attempt's timeout is the
    /// smaller of the call's own timeout and the time the call has left, so
    /// a reply that arrives after the deadline cannot succeed, and a retry
    /// whose wait would reach the deadline never runs.
    async fn send_with<T>(
        &self,
        prepared: &Prepared,
        attempt: impl AsyncFn(&Client, &Prepared, u32, Duration) -> std::result::Result<T, Failed>,
    ) -> Result<T> {
        let started = Instant::now();
        // A budget too large to name on this clock is no deadline at all.
        let deadline = prepared
            .retry
            .budget
            .and_then(|budget| started.checked_add(budget));
        let mut attempt_no: u32 = 0;
        let mut last: Option<Failed> = None;
        loop {
            let timeout = match deadline {
                Some(deadline) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        // The call's time is spent. The failure already held
                        // is the answer; a call that never ran an attempt
                        // reports the bound it ran out of.
                        return Err(match last {
                            Some(failed) => failed.error,
                            None => Error::Timeout {
                                timeout: prepared
                                    .retry
                                    .budget
                                    .map_or(prepared.timeout, |b| prepared.timeout.min(b)),
                            },
                        });
                    }
                    prepared.timeout.min(remaining)
                }
                None => prepared.timeout,
            };
            match attempt(self, prepared, attempt_no, timeout).await {
                Ok(done) => return Ok(done),
                Err(failed) => {
                    let retries_left = prepared.retry.max_retries.saturating_sub(attempt_no);
                    if retries_left == 0 || !prepared.retry.retries_error(&failed.error) {
                        return Err(failed.error);
                    }
                    let delay = prepared.retry.delay(attempt_no, failed.headers.as_ref());
                    if let Some(budget) = prepared.retry.budget
                        && started.elapsed().saturating_add(delay) >= budget
                    {
                        tracing::info!(
                            target: "jev",
                            request = prepared.tag,
                            budget_ms = budget.as_millis(),
                            "the retry budget is spent; returning the last failure"
                        );
                        return Err(failed.error);
                    }
                    tracing::info!(
                        target: "jev",
                        request = prepared.tag,
                        retry = attempt_no + 1,
                        of = prepared.retry.max_retries,
                        delay_ms = delay.as_millis(),
                        reason = %failed.error,
                        "waiting before the next attempt"
                    );
                    tokio::time::sleep(delay).await;
                    attempt_no += 1;
                    last = Some(failed);
                }
            }
        }
    }

    /// One HTTP round trip under one timeout, with the body read inside the
    /// timeout as well. A body that breaks is a connection failure the policy
    /// can retry.
    async fn attempt_read(
        &self,
        prepared: &Prepared,
        attempt: u32,
        timeout: Duration,
    ) -> std::result::Result<RawResponse, Failed> {
        let response = self.attempt(prepared, attempt, timeout).await?;
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        match response.bytes().await {
            Ok(bytes) => Ok(RawResponse {
                status,
                headers,
                bytes: bytes.to_vec(),
            }),
            Err(source) => {
                // A body that stalls past the timeout is a timeout, the way
                // the Python SDK names a read timeout.
                let error = if source.is_timeout() {
                    Error::Timeout { timeout }
                } else {
                    Error::connection(source)
                };
                Err(Failed {
                    error,
                    headers: Some(headers),
                })
            }
        }
    }

    /// One HTTP round trip under one timeout: the smaller of the call's own
    /// and the time a budget leaves it. The reqwest timeout holds through the
    /// body read, so a body that stalls past the deadline ends the attempt
    /// the same way a stalled head does.
    async fn attempt(
        &self,
        prepared: &Prepared,
        attempt: u32,
        timeout: Duration,
    ) -> std::result::Result<reqwest::Response, Failed> {
        let mut headers = prepared.headers.clone();
        if attempt > 0 {
            headers.insert(RETRY_COUNT_HEADER, HeaderValue::from(attempt));
        }
        // An explicit idempotency key pairs every attempt under one
        // request id — the service's (request, attempt) settle needs the
        // attempt number beside it, one-based.
        if headers.contains_key("idempotency-key") {
            headers.insert("x-attempt", HeaderValue::from(attempt + 1));
        }
        tracing::debug!(
            target: "jev",
            request = prepared.tag,
            method = %prepared.method,
            url = %prepared.url,
            headers = %transport::redact(&headers),
            body = %prepared.body_text(),
            "sending an attempt"
        );
        let mut builder = self
            .inner
            .http
            .request(prepared.method.clone(), &prepared.url)
            .headers(headers)
            .timeout(timeout);
        if let Some(body) = prepared.body.clone() {
            builder = builder.body(body);
        }
        let began = Instant::now();
        let response = match builder.send().await {
            Ok(response) => response,
            Err(error) => {
                let timed_out = error.is_timeout();
                let failure = if timed_out {
                    Error::Timeout { timeout }
                } else {
                    Error::connection(error)
                };
                tracing::info!(
                    target: "jev",
                    request = prepared.tag,
                    method = %prepared.method,
                    url = %prepared.url,
                    elapsed_ms = began.elapsed().as_millis(),
                    reason = %failure,
                    "the attempt did not reach the API"
                );
                return Err(Failed {
                    error: failure,
                    headers: None,
                });
            }
        };
        let status = response.status();
        tracing::info!(
            target: "jev",
            request = prepared.tag,
            method = %prepared.method,
            url = %prepared.url,
            status = status.as_u16(),
            elapsed_ms = began.elapsed().as_millis(),
            request_id = request_id(response.headers()).unwrap_or("-"),
            "the API answered"
        );
        if status.is_success() {
            return Ok(response);
        }
        Err(self.failure(prepared, status, response).await)
    }

    /// The error one failed response raises, with its body read.
    async fn failure(
        &self,
        prepared: &Prepared,
        status: StatusCode,
        response: reqwest::Response,
    ) -> Failed {
        let headers = response.headers().clone();
        let bytes = response
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .unwrap_or_default();
        let body = transport::parse_body(&bytes);
        tracing::debug!(
            target: "jev",
            request = prepared.tag,
            status = status.as_u16(),
            headers = %transport::redact(&headers),
            body = %String::from_utf8_lossy(&bytes),
            "the failed response carried this body"
        );
        Failed {
            error: Error::from(ApiError::new(
                prepared.endpoint.clone(),
                status.as_u16(),
                headers.clone(),
                body,
            )),
            headers: Some(headers),
        }
    }
}

/// One attempt that failed, with the response headers when there was a
/// response, so the retry can honor a server delay.
struct Failed {
    error: Error,
    headers: Option<HeaderMap>,
}

/// Everything one call sends, reused by every attempt.
struct Prepared {
    tag: u64,
    method: Method,
    url: String,
    endpoint: String,
    body: Option<Vec<u8>>,
    headers: HeaderMap,
    timeout: Duration,
    retry: RetryPolicy,
}

impl Prepared {
    /// The body as text for a `debug` line. Bodies are logged as they are sent,
    /// the way both official SDKs log them.
    fn body_text(&self) -> String {
        match self.body.as_deref() {
            Some(bytes) => String::from_utf8_lossy(bytes).into_owned(),
            None => String::new(),
        }
    }
}

impl RawResponse {
    /// Read one response into its status, its headers, and its bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Connection`] when the body does not arrive.
    pub async fn read(response: reqwest::Response) -> Result<Self> {
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let bytes = response.bytes().await.map_err(Error::connection)?.to_vec();
        Ok(Self {
            status,
            headers,
            bytes,
        })
    }
}

/// The base URL as an error's endpoint names it: the credentials, the query,
/// and the fragment dropped, the way the Python SDK writes a request's URL.
fn endpoint_of(base_url: &str) -> String {
    let mut parsed = match url::Url::parse(base_url) {
        Ok(parsed) => parsed,
        Err(_) => return base_url.to_string(),
    };
    let _ = parsed.set_username("");
    let _ = parsed.set_password(None);
    parsed.set_query(None);
    parsed.set_fragment(None);
    parsed.to_string().trim_end_matches('/').to_string()
}

/// The request id one response carries.
fn request_id(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
}

/// A client for a caller that runs no `tokio` runtime of its own.
///
/// Each call runs on a runtime this client owns. Do not call it from inside a
/// runtime; use [`Client`] there.
#[cfg(feature = "blocking")]
#[derive(Debug)]
pub struct BlockingClient {
    client: Client,
    runtime: tokio::runtime::Runtime,
}

#[cfg(feature = "blocking")]
impl BlockingClient {
    /// Build a blocking client from settings.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Client::new`], and [`Error::Config`] when
    /// the runtime does not start.
    pub fn new(config: Config) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| Error::Config(format!("the runtime did not start: {error}")))?;
        Ok(Self {
            client: Client::new(config)?,
            runtime,
        })
    }

    /// Build a blocking client from the environment and the defaults alone.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`BlockingClient::new`].
    pub fn from_env() -> Result<Self> {
        Self::new(Config::new())
    }

    /// The async client underneath.
    #[must_use]
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Ask questions about one state and read the answers.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Client::system_one`].
    pub fn system_one(&self, request: SystemOneRequest) -> Result<SystemOneResponse> {
        self.runtime.block_on(self.client.system_one(request))
    }

    /// List the models the account can ask.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::Models::list`].
    pub fn list_models(&self, options: ListOptions) -> Result<Vec<crate::ModelCard>> {
        self.runtime.block_on(self.client.models().list(options))
    }

    /// Run one `openagents.classify.v1` envelope and read the report.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::Classify::run`].
    pub fn classify(&self, request: crate::ClassifyRequest) -> Result<crate::ClassifyReport> {
        self.runtime.block_on(self.client.classify().run(request))
    }

    /// Submit one durable classification job and read its queued status.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::Jobs::submit`].
    pub fn submit_job(&self, request: crate::JobSubmit) -> Result<crate::JobStatus> {
        self.runtime.block_on(self.client.jobs().submit(request))
    }

    /// Read one job's status document.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::Jobs::status`].
    pub fn job_status(&self, id: &str) -> Result<crate::JobStatus> {
        self.runtime.block_on(self.client.jobs().status(id))
    }

    /// Mark a live job cancelled.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::Jobs::cancel`].
    pub fn cancel_job(&self, id: &str) -> Result<crate::JobStatus> {
        self.runtime.block_on(self.client.jobs().cancel(id))
    }

    /// Export one page of a terminal job's items.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::Jobs::results`].
    pub fn job_results(&self, id: &str, query: &crate::ResultsQuery) -> Result<crate::ResultsPage> {
        self.runtime.block_on(self.client.jobs().results(id, query))
    }

    /// Remove a terminal job's record.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::Jobs::remove`].
    pub fn delete_job(&self, id: &str) -> Result<bool> {
        self.runtime.block_on(self.client.jobs().remove(id))
    }

    /// `GET /v1/session` — the session the bearer token names.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::Account::session`].
    pub fn session(&self) -> Result<crate::SessionView> {
        self.runtime.block_on(self.client.account().session())
    }

    /// `GET /v1/account` — the account and every workspace it belongs to.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::Account::details`].
    pub fn account_details(&self) -> Result<crate::AccountDetails> {
        self.runtime.block_on(self.client.account().details())
    }

    /// `GET /v1/balance` — the named workspace's monetary position.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::Account::balance`].
    pub fn balance(&self, workspace: &str) -> Result<crate::BalanceView> {
        self.runtime
            .block_on(self.client.account().balance(workspace))
    }

    /// `GET /v1/workspaces/{workspace}/usage` — the workspace's usage
    /// position.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`crate::Account::usage`].
    pub fn usage(&self, workspace: &str, query: &crate::UsageQuery) -> Result<crate::UsageView> {
        self.runtime
            .block_on(self.client.account().usage(workspace, query))
    }
}
