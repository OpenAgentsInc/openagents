//! The network client a remote executor or caller uses against a host that
//! mounts [`crate::http::router`] (feature `net`).
//!
//! The host authenticates every request from its own headers (a bearer
//! token, and whatever names the executor); this client only sends them.
//! Errors come back as the host's [`ActorError`] with its code and whether a
//! retry may help; a request that never got an answer is the retryable
//! `network`.
use crate::types::*;
use serde::Serialize;
use serde_json::{Value, json};
use std::time::Duration;

/// A connection to one host's actor routes, for one workspace.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    origin: String,
    workspace: String,
    headers: reqwest::header::HeaderMap,
}

/// Optional parts of an action call.
#[derive(Clone, Debug, Default)]
pub struct Call {
    pub input: Option<Value>,
    pub idempotency_key: Option<String>,
    pub expected_version: Option<u64>,
    pub fence: Option<WorkFence>,
}

/// One path segment, percent-encoded (`/`, `:`, and spaces included).
fn segment(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'@') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

impl Client {
    /// `origin` is the host (`https://openagents.com`); `headers` go with
    /// every request (its credential, for example). A long-poll claim waits
    /// up to 30 s, so the client allows each request 60 s.
    pub fn new(
        origin: impl Into<String>,
        workspace: impl Into<String>,
        headers: reqwest::header::HeaderMap,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|_| ActorError::new("unavailable", "The network client couldn't start."))?;
        Ok(Self::with_http(http, origin, workspace, headers))
    }

    pub fn with_http(
        http: reqwest::Client,
        origin: impl Into<String>,
        workspace: impl Into<String>,
        headers: reqwest::header::HeaderMap,
    ) -> Self {
        Self {
            http,
            origin: origin.into().trim_end_matches('/').to_owned(),
            workspace: workspace.into(),
            headers,
        }
    }

    pub fn workspace(&self) -> &str {
        &self.workspace
    }

    fn url(&self, rest: &str) -> String {
        format!("{}/v1/w/{}{rest}", self.origin, segment(&self.workspace))
    }

    async fn send<T: serde::de::DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T> {
        let response = request
            .headers(self.headers.clone())
            .send()
            .await
            .map_err(|_| ActorError::retry("network", "The service couldn't be reached."))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|_| ActorError::retry("network", "The answer was cut off."))?;
        if status.is_success() {
            return serde_json::from_slice(&bytes).map_err(|_| {
                ActorError::new("bad_reply", "The service answered in a new format.")
            });
        }
        let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        let error = &body["error"];
        let code = error["code"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                match status.as_u16() {
                    401 => "unauthorized",
                    404 => "not_found",
                    _ => "unavailable",
                }
                .to_owned()
            });
        let message = error["message"]
            .as_str()
            .unwrap_or("The service refused the request.")
            .to_owned();
        let retryable = error["retryable"]
            .as_bool()
            .unwrap_or(status.is_server_error() || status.as_u16() == 429);
        Err(ActorError {
            code,
            message,
            retryable,
        })
    }

    async fn post<T: serde::de::DeserializeOwned>(
        &self,
        rest: &str,
        body: &impl Serialize,
        idempotency_key: Option<&str>,
    ) -> Result<T> {
        let mut request = self.http.post(self.url(rest)).json(body);
        if let Some(key) = idempotency_key {
            request = request.header("idempotency-key", key);
        }
        self.send(request).await
    }

    /// Claim up to `max` items of `queue` for `target`, waiting up to `wait`
    /// (at most 30 s) when none is ready.
    pub async fn claim(
        &self,
        queue: &str,
        target: Option<&str>,
        max: u32,
        wait: Duration,
    ) -> Result<Vec<ClaimedWork>> {
        let mut body = json!({"max": max, "wait_ms": wait.as_millis().min(30_000) as u64});
        if let Some(target) = target {
            body["target"] = json!(target);
        }
        let reply: Value = self
            .post(&format!("/work/{}/claim", segment(queue)), &body, None)
            .await?;
        serde_json::from_value(reply["items"].clone())
            .map_err(|_| ActorError::new("bad_reply", "The service answered in a new format."))
    }

    fn work_path(queue: &str, claim: (&str, &str), action: &str) -> String {
        format!(
            "/work/{}/{}/{}/{action}",
            segment(queue),
            segment(claim.0),
            segment(claim.1)
        )
    }

    /// Renew claim `(uid, item)` at `epoch`, with ordered progress.
    pub async fn heartbeat(
        &self,
        queue: &str,
        claim: (&str, &str),
        epoch: u64,
        progress: Option<Progress>,
    ) -> Result<HeartbeatReply> {
        self.post(
            &Self::work_path(queue, claim, "heartbeat"),
            &json!({"epoch": epoch, "progress": progress}),
            None,
        )
        .await
    }

    /// Complete claim `(uid, item)` at `epoch` with `outcome`.
    pub async fn finish(
        &self,
        queue: &str,
        claim: (&str, &str),
        epoch: u64,
        outcome: Value,
    ) -> Result<()> {
        let _: Value = self
            .post(
                &Self::work_path(queue, claim, "finish"),
                &json!({"epoch": epoch, "outcome": outcome}),
                None,
            )
            .await?;
        Ok(())
    }

    /// Give claim `(uid, item)` up; for cancelled work, confirm it stopped.
    pub async fn release(
        &self,
        queue: &str,
        claim: (&str, &str),
        epoch: u64,
        reason: &str,
    ) -> Result<()> {
        let _: Value = self
            .post(
                &Self::work_path(queue, claim, "release"),
                &json!({"epoch": epoch, "reason": reason}),
                None,
            )
            .await?;
        Ok(())
    }

    /// Call `message` on actor `actor_type/key`.
    pub async fn call(
        &self,
        actor_type: &str,
        key: &str,
        message: &str,
        args: Value,
        call: Call,
    ) -> Result<ActionReply> {
        let mut body = json!({"args": args});
        if let Some(input) = call.input {
            body["input"] = input;
        }
        if let Some(version) = call.expected_version {
            body["expected_version"] = json!(version);
        }
        if let Some(fence) = call.fence {
            body["fence"] = json!(fence);
        }
        self.post(
            &format!(
                "/actors/{}/{}/actions/{}",
                segment(actor_type),
                segment(key),
                segment(message)
            ),
            &body,
            call.idempotency_key.as_deref(),
        )
        .await
    }

    /// The caller's view of actor `actor_type/key`.
    pub async fn view(&self, actor_type: &str, key: &str) -> Result<ViewReply> {
        let request = self.http.get(self.url(&format!(
            "/actors/{}/{}/view",
            segment(actor_type),
            segment(key)
        )));
        self.send(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_keep_reserved_characters_out_of_the_path() {
        assert_eq!(segment("mac:ab12"), "mac%3Aab12");
        assert_eq!(segment("a/b c"), "a%2Fb%20c");
        assert_eq!(segment("report@1"), "report@1");
    }
}
