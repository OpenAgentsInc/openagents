use std::{
    hash::{BuildHasher, Hasher},
    time::Duration,
};

use reqwest::{
    Method, Url,
    header::{AUTHORIZATION, COOKIE, HeaderMap, HeaderValue},
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::{ApiError, ApiKey, Error, Result};

/// The org header Boat reads on organization-scoped operations.
pub(crate) const ORG_HEADER: &str = "X-Boat-Org";
const IDEMPOTENCY_HEADER: &str = "Idempotency-Key";

/// When an operation may be sent again after a 429 or 5xx.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Retry {
    /// A read: retrying cannot change remote state.
    Read,
    /// A create or fork: retried only when the caller sent an
    /// `Idempotency-Key`, which is replayed with the same body.
    IfKeyed,
    /// Everything else, including commands, prompts and stops.
    Never,
}

/// Per-operation transport flags, emitted by `schema/generate.py`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Op {
    pub(crate) retry: Retry,
    /// The operation accepts `X-Boat-Org`, so the client's org applies.
    pub(crate) org: bool,
}

impl Op {
    pub(crate) const NEVER: Self = Self {
        retry: Retry::Never,
        org: false,
    };
}

/// How often a retryable request is sent again after a 429 or 5xx.
///
/// Only reads, and creates or forks carrying an `Idempotency-Key`, are ever
/// retried. Commands, prompts, stops and every other write run once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Retries after the first attempt. Zero disables retries.
    pub max_retries: u32,
    /// The first backoff; it doubles per retry, with jitter.
    pub base_delay: Duration,
    /// The longest single wait. A `Retry-After` longer than this is not
    /// waited out: the error is returned instead.
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(20),
        }
    }
}

impl RetryPolicy {
    /// Run every request once.
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            ..Self::default()
        }
    }

    fn delay(&self, retry: u32, retry_after: Option<&str>) -> Option<Duration> {
        let backoff = self
            .base_delay
            .saturating_mul(1u32 << retry.min(16))
            .min(self.max_delay);
        // Full jitter over the upper half keeps parallel callers apart.
        let half = backoff / 2;
        let jitter = Duration::from_nanos(
            random_u64() % (half.as_nanos().min(u64::MAX as u128) as u64).max(1),
        );
        let mut delay = half + jitter;
        if let Some(seconds) = retry_after.and_then(|v| v.trim().parse::<u64>().ok()) {
            let asked = Duration::from_secs(seconds);
            if asked > self.max_delay {
                return None;
            }
            delay = delay.max(asked);
        }
        Some(delay)
    }
}

fn random_u64() -> u64 {
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or_default(),
    );
    hasher.finish()
}

fn retryable(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

/// Cloneable HTTP client. Credentials are shared through the HTTP client.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: Url,
    max_json_bytes: usize,
    org: Option<String>,
    retry: RetryPolicy,
}

enum Credential {
    Key(ApiKey),
    Cookie(String),
}

pub struct ClientBuilder {
    credential: Option<Credential>,
    base: String,
    timeout: Duration,
    max_json_bytes: usize,
    org: Option<String>,
    retry: RetryPolicy,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder").finish_non_exhaustive()
    }
}

impl ClientBuilder {
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base = url.into();
        self
    }

    /// Set the deadline for each HTTP request, including its response body.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn max_json_bytes(mut self, bytes: usize) -> Self {
        self.max_json_bytes = bytes;
        self
    }

    /// Send `X-Boat-Org` on every organization-scoped operation that does not
    /// set its own.
    pub fn org(mut self, org: impl Into<String>) -> Self {
        self.org = Some(org.into());
        self
    }

    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = policy;
        self
    }

    /// Supply an interactive session cookie for session-only operations.
    ///
    /// You obtain the cookie outside this SDK. It replaces bearer authentication.
    pub fn session_cookie(mut self, cookie: impl Into<String>) -> Self {
        self.credential = Some(Credential::Cookie(cookie.into()));
        self
    }

    pub fn build(self) -> Result<Client> {
        let base = Url::parse(&self.base)
            .map_err(|_| Error::Configuration("The Boat base URL is invalid."))?;
        let local = matches!(base.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if (base.scheme() != "https" && !(base.scheme() == "http" && local))
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
        {
            return Err(Error::Configuration(
                "Use an HTTPS Boat URL without credentials, query, or fragment.",
            ));
        }
        if base.host_str().is_some_and(crate::auth::is_hosted_host) && !crate::auth::hosted() {
            return Err(Error::Configuration(
                "Hosted Boat (boat.dev) is off; our own service is the default. Set BOAT_HOSTED=1 to use boat.dev.",
            ));
        }
        if self.timeout.is_zero() || self.max_json_bytes == 0 {
            return Err(Error::Configuration(
                "Boat request limits must be greater than zero.",
            ));
        }
        if self.org.as_deref().is_some_and(|o| o.trim().is_empty()) {
            return Err(Error::Configuration("The Boat org must not be empty."));
        }
        let mut headers = HeaderMap::new();
        let (name, raw) = match self.credential {
            Some(Credential::Key(key)) => (AUTHORIZATION, format!("Bearer {}", key.expose())),
            Some(Credential::Cookie(cookie)) if !cookie.trim().is_empty() => (COOKIE, cookie),
            _ => {
                return Err(Error::Configuration(
                    "Provide a Boat API key or session cookie.",
                ));
            }
        };
        let mut value = HeaderValue::from_str(&raw)
            .map_err(|_| Error::Configuration("The Boat credential is invalid."))?;
        value.set_sensitive(true);
        headers.insert(name, value);
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(30))
            .timeout(self.timeout)
            .user_agent(concat!("openagents-boat/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| Error::Transport)?;
        Ok(Client {
            http,
            base,
            max_json_bytes: self.max_json_bytes,
            org: self.org,
            retry: self.retry,
        })
    }
}

impl Client {
    /// A client for the default base URL. An empty or malformed key is refused.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::builder(ApiKey::new(api_key)?).build()
    }

    /// `BOAT_API_KEY` (or Secret Manager `oa-boat-api-key`) and, when set,
    /// `BOAT_API_BASE`. The default base is our own service
    /// ([`crate::BASE_URL`]); `BOAT_HOSTED=1` makes it boat.dev.
    pub async fn from_env() -> Result<Self> {
        let mut builder = Self::builder(ApiKey::resolve().await?);
        if let Ok(base) = std::env::var(crate::auth::API_BASE_ENV)
            && !base.trim().is_empty()
        {
            builder = builder.base_url(base.trim());
        } else if crate::auth::hosted() {
            builder = builder.base_url(crate::HOSTED_BASE_URL);
        }
        builder.build()
    }

    pub fn builder(api_key: ApiKey) -> ClientBuilder {
        ClientBuilder {
            credential: Some(Credential::Key(api_key)),
            base: crate::BASE_URL.into(),
            timeout: Duration::from_secs(660),
            max_json_bytes: 32 * 1024 * 1024,
            org: None,
            retry: RetryPolicy::default(),
        }
    }

    /// The API origin this client speaks to, without a trailing slash.
    pub fn origin(&self) -> String {
        self.base.as_str().trim_end_matches('/').to_string()
    }

    pub(crate) fn path(&self, parts: &[&str]) -> Result<Url> {
        let mut url = self.base.clone();
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| Error::Configuration("The Boat base URL has no path."))?;
            segments.pop_if_empty();
            for part in parts {
                if part.is_empty() || matches!(*part, "." | "..") {
                    return Err(Error::Configuration(
                        "Boat path parameters must be nonempty segments.",
                    ));
                }
                segments.push(part);
            }
        }
        Ok(url)
    }

    fn may_retry(op: Op, headers: &[(&str, String)]) -> bool {
        match op.retry {
            Retry::Read => true,
            Retry::IfKeyed => headers.iter().any(|(name, value)| {
                name.eq_ignore_ascii_case(IDEMPOTENCY_HEADER) && !value.trim().is_empty()
            }),
            Retry::Never => false,
        }
    }

    pub(crate) async fn send(
        &self,
        op: Op,
        method: Method,
        url: Url,
        query: &[(&str, String)],
        headers: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<reqwest::Response> {
        let retries = if Self::may_retry(op, headers) {
            self.retry.max_retries
        } else {
            0
        };
        let mut attempt = 0;
        loop {
            let error = match self
                .send_once(
                    op,
                    method.clone(),
                    url.clone(),
                    query,
                    headers,
                    body.as_ref(),
                )
                .await
            {
                Ok(response) => return Ok(response),
                Err(error) => error,
            };
            let Error::Api(api) = &error else {
                return Err(error);
            };
            if attempt >= retries || !retryable(api.status) {
                return Err(error);
            }
            let Some(delay) = self.retry.delay(attempt, api.retry_after.as_deref()) else {
                return Err(error);
            };
            tokio::time::sleep(delay).await;
            attempt += 1;
        }
    }

    async fn send_once(
        &self,
        op: Op,
        method: Method,
        url: Url,
        query: &[(&str, String)],
        headers: &[(&str, String)],
        body: Option<&Value>,
    ) -> Result<reqwest::Response> {
        let mut request = self.http.request(method, url).query(query);
        for (name, value) in headers {
            let mut value = HeaderValue::from_str(value)
                .map_err(|_| Error::Configuration("A Boat request header is invalid."))?;
            value.set_sensitive(true);
            request = request.header(*name, value);
        }
        if op.org
            && let Some(org) = &self.org
            && !headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(ORG_HEADER))
        {
            let value = HeaderValue::from_str(org)
                .map_err(|_| Error::Configuration("The Boat org is invalid."))?;
            request = request.header(ORG_HEADER, value);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await.map_err(|_| Error::Transport)?;
        if !response.status().is_success() {
            let status = response.status();
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let mut request_id = response
                .headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let envelope: Option<crate::models::ErrorEnvelope> =
                match limited(response, 64 * 1024).await {
                    Ok(bytes) => serde_json::from_slice(&bytes).ok(),
                    Err(_) => None,
                };
            if request_id.is_none() {
                request_id = envelope.as_ref().map(|e| e.request_id.clone());
            }
            return Err(Error::Api(std::boxed::Box::new(ApiError {
                status,
                envelope,
                retry_after,
                request_id,
            })));
        }
        Ok(response)
    }

    pub(crate) async fn json<T: DeserializeOwned>(
        &self,
        op: Op,
        method: Method,
        url: Url,
        query: &[(&str, String)],
        headers: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<T> {
        let response = self.send(op, method, url, query, headers, body).await?;
        let status = response.status();
        let bytes = limited(response, self.max_json_bytes).await?;
        decode_json(status, &bytes)
    }

    pub(crate) async fn download(
        &self,
        op: Op,
        method: Method,
        url: Url,
        query: &[(&str, String)],
        headers: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<Download> {
        Ok(Download {
            response: self.send(op, method, url, query, headers, body).await?,
        })
    }

    pub(crate) fn max_json_bytes(&self) -> usize {
        self.max_json_bytes
    }
}

/// Decode a 2xx JSON body, turning an `ok: false` envelope into `Error::Api`.
pub(crate) fn decode_json<T: DeserializeOwned>(
    status: reqwest::StatusCode,
    bytes: &[u8],
) -> Result<T> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Error::Decode)?;
    if value.get("ok") == Some(&Value::Bool(false)) {
        let envelope: Option<crate::models::ErrorEnvelope> = serde_json::from_value(value).ok();
        let request_id = envelope.as_ref().map(|e| e.request_id.clone());
        return Err(Error::Api(std::boxed::Box::new(ApiError {
            status,
            envelope,
            retry_after: None,
            request_id,
        })));
    }
    serde_json::from_value(value).map_err(|_| Error::Decode)
}

pub(crate) async fn limited(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|size| size > limit as u64)
    {
        return Err(Error::ResponseTooLarge);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Error::Transport)? {
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err(Error::ResponseTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// A binary response. Read chunks or write it to your own async sink.
pub struct Download {
    response: reqwest::Response,
}

impl std::fmt::Debug for Download {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Download").finish_non_exhaustive()
    }
}

impl Download {
    pub fn content_type(&self) -> Option<&str> {
        self.response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
    }

    pub fn content_length(&self) -> Option<u64> {
        self.response.content_length()
    }

    pub async fn chunk(&mut self) -> Result<Option<Vec<u8>>> {
        Ok(self
            .response
            .chunk()
            .await
            .map_err(|_| Error::Transport)?
            .map(|bytes| bytes.to_vec()))
    }

    pub async fn bytes(self, limit: usize) -> Result<Vec<u8>> {
        limited(self.response, limit).await
    }

    pub async fn write_to<W: AsyncWrite + Unpin>(&mut self, sink: &mut W) -> Result<u64> {
        let mut written = 0;
        while let Some(chunk) = self.chunk().await? {
            sink.write_all(&chunk).await.map_err(|_| Error::Io)?;
            written += chunk.len() as u64;
        }
        Ok(written)
    }
}
