//! Control-plane and S3 transport: envelope unwrapping and retries.

use std::time::Duration;

use bytes::Bytes;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::{ApiError, Error, Result};
use crate::retry::{
    MAX_RETRIES, RetryMode, is_connection_establishment_error, is_transient_status, retry_sleep,
};

/// `encodeURIComponent`'s unreserved set.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

pub(crate) fn enc(segment: &str) -> String {
    utf8_percent_encode(segment, COMPONENT).to_string()
}

#[derive(Debug)]
pub(crate) struct Http {
    pub client: reqwest::Client,
    pub base_url: String,
    pub s3_base_url: Option<String>,
    pub poll_interval: Duration,
}

pub(crate) struct Req {
    pub method: Method,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: Option<Value>,
    pub headers: Vec<(String, String)>,
    pub retry: RetryMode,
}

impl Req {
    pub fn new(method: Method, path: impl Into<String>, retry: RetryMode) -> Self {
        Self {
            method,
            path: path.into(),
            query: vec![],
            body: None,
            headers: vec![],
            retry,
        }
    }
    pub fn get(path: impl Into<String>) -> Self {
        Self::new(Method::GET, path, RetryMode::Transient)
    }
    pub fn query(mut self, key: &str, value: impl ToString) -> Self {
        self.query.push((key.to_string(), value.to_string()));
        self
    }
    pub fn query_opt(self, key: &str, value: Option<impl ToString>) -> Self {
        match value {
            Some(v) => self.query(key, v),
            None => self,
        }
    }
    pub fn json(mut self, body: impl serde::Serialize) -> Self {
        self.body = Some(serde_json::to_value(body).expect("request body serializes"));
        self
    }
    pub fn header(mut self, k: &str, v: impl Into<String>) -> Self {
        self.headers.push((k.to_string(), v.into()));
        self
    }
}

pub(crate) struct Resp {
    pub status: u16,
    pub body: Bytes,
}

/// A fully-buffered S3 gateway response.
pub(crate) struct S3Resp {
    pub ok: bool,
    pub status: u16,
    pub status_text: String,
    pub headers: reqwest::header::HeaderMap,
    pub body: Bytes,
}

pub(crate) struct S3Req {
    pub method: Method,
    pub disk_id: String,
    pub key: String,
    pub body: Option<Bytes>,
    pub content_type: Option<String>,
    pub query: Vec<(String, String)>,
    pub retry: bool,
    pub headers: Vec<(String, String)>,
}

impl S3Req {
    pub fn new(method: Method, disk_id: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            method,
            disk_id: disk_id.into(),
            key: key.into(),
            body: None,
            content_type: None,
            query: vec![],
            retry: true,
            headers: vec![],
        }
    }
    pub fn query(mut self, k: &str, v: impl ToString) -> Self {
        self.query.push((k.to_string(), v.to_string()));
        self
    }
    pub fn body(mut self, b: impl Into<Bytes>) -> Self {
        self.body = Some(b.into());
        self
    }
    pub fn content_type(mut self, ct: Option<impl Into<String>>) -> Self {
        self.content_type = ct.map(Into::into);
        self
    }
    pub fn headers(mut self, h: Vec<(String, String)>) -> Self {
        self.headers = h;
        self
    }
    pub fn no_retry(mut self) -> Self {
        self.retry = false;
        self
    }
}

impl Http {
    fn build(&self, req: &Req) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, req.path);
        let mut b = self.client.request(req.method.clone(), url);
        if !req.query.is_empty() {
            b = b.query(&req.query);
        }
        for (k, v) in &req.headers {
            b = b.header(k, v);
        }
        if let Some(body) = &req.body {
            b = b.json(body);
        }
        b
    }

    /// Send with the SDK retry policy. Returns the final response whatever its
    /// status; only transport failures are errors.
    pub async fn send(&self, req: &Req) -> Result<Resp> {
        let mut attempt = 0;
        loop {
            let result = async {
                let resp = self.build(req).send().await?;
                let status = resp.status().as_u16();
                let body = resp.bytes().await?;
                Ok::<_, reqwest::Error>(Resp { status, body })
            }
            .await;
            match result {
                Ok(resp) => {
                    if req.retry != RetryMode::Transient
                        || !is_transient_status(resp.status)
                        || attempt >= MAX_RETRIES
                    {
                        return Ok(resp);
                    }
                }
                Err(error) => {
                    let retry = match req.retry {
                        RetryMode::Transient => true,
                        RetryMode::Connect => is_connection_establishment_error(&error),
                    };
                    if !retry || attempt >= MAX_RETRIES {
                        return Err(error.into());
                    }
                }
            }
            retry_sleep(attempt).await;
            attempt += 1;
        }
    }

    /// Unwrap the `{success, data}` envelope into `T`.
    pub async fn call<T: DeserializeOwned>(&self, req: Req) -> Result<T> {
        Ok(self.call_page(req).await?.0)
    }

    /// Like [`call`](Self::call) but also surfaces the envelope's `nextCursor`.
    pub async fn call_page<T: DeserializeOwned>(&self, req: Req) -> Result<(T, Option<String>)> {
        let resp = self.send(&req).await?;
        let body = envelope(&resp)?;
        let data = body.get("data").cloned().unwrap_or(Value::Null);
        let next = body
            .get("nextCursor")
            .and_then(Value::as_str)
            .map(str::to_string);
        let data = serde_json::from_value(data).map_err(|e| Error::Decode(e.to_string()))?;
        Ok((data, next))
    }

    /// For responses with no payload (deletes): 204, or a successful envelope.
    pub async fn call_empty(&self, req: Req) -> Result<()> {
        let resp = self.send(&req).await?;
        if resp.status == 204 {
            return Ok(());
        }
        envelope(&resp).map(|_| ())
    }

    /// One S3 request, with its own retry loop (any transport error or
    /// transient status is retried unless `retry` is off).
    pub async fn s3(&self, req: S3Req) -> Result<S3Resp> {
        let base = self.s3_base_url.as_deref().filter(|b| !b.is_empty()).ok_or_else(|| {
            Error::Config(
                "S3 base URL not configured. Pass s3_base_url to the client options or set ARCHIL_S3_BASE_URL."
                    .into(),
            )
        })?;
        // Percent-encode each key segment but keep the `/` separators.
        let trimmed = req.key.strip_prefix('/').unwrap_or(&req.key);
        let encoded: Vec<String> = trimmed.split('/').map(enc).collect();
        let encoded = encoded.join("/");
        let disk = &req.disk_id;
        let path = if encoded.is_empty() {
            format!("/{disk}")
        } else {
            format!("/{disk}/{encoded}")
        };
        let max_retries = if req.retry { MAX_RETRIES } else { 0 };
        let mut attempt = 0;
        loop {
            let mut b = self
                .client
                .request(req.method.clone(), format!("{base}{path}"));
            if !req.query.is_empty() {
                b = b.query(&req.query);
            }
            if let Some(ct) = &req.content_type {
                b = b.header("Content-Type", ct);
            }
            for (k, v) in &req.headers {
                b = b.header(k, v);
            }
            if let Some(body) = &req.body {
                b = b.body(body.clone());
            }
            let result = async {
                let resp = b.send().await?;
                let status = resp.status();
                let headers = resp.headers().clone();
                let body = resp.bytes().await?;
                Ok::<_, reqwest::Error>(S3Resp {
                    ok: status.is_success(),
                    status: status.as_u16(),
                    status_text: status.canonical_reason().unwrap_or("").to_string(),
                    headers,
                    body,
                })
            }
            .await;
            match result {
                Ok(resp) => {
                    if !resp.ok && is_transient_status(resp.status) && attempt < max_retries {
                        retry_sleep(attempt).await;
                        attempt += 1;
                        continue;
                    }
                    return Ok(resp);
                }
                Err(e) => {
                    if attempt < max_retries {
                        retry_sleep(attempt).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }
    }
}

/// Parse and validate the API envelope, returning the JSON object on success.
fn envelope(resp: &Resp) -> Result<Value> {
    let parsed: Option<Value> = serde_json::from_slice(&resp.body).ok();
    let failure = |parsed: &Option<Value>, default: &str| -> Error {
        let message = parsed
            .as_ref()
            .and_then(|v| v.get("error"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| default.to_string());
        let code = parsed
            .as_ref()
            .and_then(|v| v.get("code"))
            .and_then(Value::as_str)
            .map(str::to_string);
        Error::Api(ApiError {
            message,
            status: resp.status,
            code,
        })
    };
    if !(200..300).contains(&resp.status) {
        return Err(failure(
            &parsed,
            &format!("API request failed with status {}", resp.status),
        ));
    }
    match parsed {
        Some(v) if v.is_object() => {
            if v.get("success").and_then(Value::as_bool) != Some(true) {
                return Err(failure(&Some(v), "Unknown API error"));
            }
            Ok(v)
        }
        other => Err(failure(
            &other,
            &format!("API request failed with status {}", resp.status),
        )),
    }
}
