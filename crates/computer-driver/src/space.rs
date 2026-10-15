//! SPACE 0.8 HTTP driver (#11728). Run upstream as a separate service.
//!
//! Core and memory-preserving Suspend use the public `/v1` API. Creating from
//! a saved snapshot and forking are absent from that API at the pinned commit;
//! those profiles remain unsupported. Command attachment uses the configured
//! data-plane origin and a session token, never the management credential.
//! Secrets travel over process stdin: SPACE persists command `env` in its
//! session record, so it must not receive secret values there.

use crate::*;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures_util::{SinkExt, StreamExt};
use reqwest::{Client, Method, Response, Url};
use serde_json::{Value, json};
use std::time::Duration;
use tokio_tungstenite::tungstenite::{Message, protocol::WebSocketConfig};

const MAX_FILE: usize = 16 * 1024 * 1024;
const MAX_RESPONSE: usize = 24 * 1024 * 1024;
const MAX_OUTPUT: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub struct SpaceDriver {
    http: Client,
    api: Url,
    data: Url,
    token: Secret,
    template: String,
}

// Never include server bodies, URLs, or websocket frames in errors: an
// upstream error can echo a credential or command input.
struct Error {
    status: Option<u16>,
    message: &'static str,
}
impl Error {
    fn unknown(message: &'static str) -> Self {
        Self {
            status: None,
            message,
        }
    }
    fn outcome<T>(self) -> Outcome<T> {
        match self.status {
            Some(s) if (400..500).contains(&s) && s != 408 && s != 429 => {
                Outcome::failed(format!("SPACE: {} (HTTP {s})", self.message))
            }
            Some(s) => Outcome::unknown(format!("SPACE: {} (HTTP {s})", self.message)),
            None => Outcome::unknown(format!("SPACE: {}", self.message)),
        }
    }
}
type Result<T> = std::result::Result<T, Error>;

fn origin(value: &str) -> std::result::Result<Url, String> {
    let url = Url::parse(value).map_err(|_| "SPACE needs a valid HTTP origin.")?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if !matches!(url.scheme(), "http" | "https")
        || (url.scheme() == "http" && !loopback)
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "SPACE needs an HTTPS origin, or HTTP on loopback, without a path or credentials."
                .into(),
        );
    }
    Ok(url)
}

fn valid_id(id: &str, prefix: &str) -> bool {
    id.strip_prefix(prefix).is_some_and(|s| {
        !s.is_empty()
            && s.len() <= 128
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    })
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| Error::unknown("the response is missing a required field"))
}

fn state(value: &str) -> State {
    match value {
        "running" => State::Running,
        "stopped" | "paused" | "suspended" => State::Stopped,
        "pausing" | "suspending" | "stopping" => State::Stopping,
        "deleted" => State::Gone,
        "failed" => State::Failed,
        // Deleting is not Gone: cleanup must wait for actual deletion.
        _ => State::Starting,
    }
}

impl SpaceDriver {
    pub fn new(
        api: &str,
        data: &str,
        token: Secret,
        template: &str,
    ) -> std::result::Result<Self, String> {
        if token.expose().trim().is_empty() || template.trim().is_empty() {
            return Err("SPACE needs a token and a template.".into());
        }
        let http = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| "Couldn't create the SPACE HTTP client.")?;
        Ok(Self {
            http,
            api: origin(api)?,
            data: origin(data)?,
            token,
            template: template.into(),
        })
    }

    pub fn from_env() -> std::result::Result<Self, String> {
        let required = |name| std::env::var(name).map_err(|_| format!("Set {name} to use SPACE."));
        Self::new(
            &required("SPACE_API_URL")?,
            &required("SPACE_DATA_PLANE_URL")?,
            Secret::new(required("SPACE_API_TOKEN")?),
            &required("SPACE_TEMPLATE")?,
        )
    }

    async fn response(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        key: Option<&str>,
    ) -> Result<Response> {
        let url = self
            .api
            .join(path)
            .map_err(|_| Error::unknown("invalid API path"))?;
        let mut request = self
            .http
            .request(method, url)
            .bearer_auth(self.token.expose());
        if let Some(body) = body {
            request = request.json(&body);
        }
        if let Some(key) = key {
            request = request.header("Idempotency-Key", key);
        }
        let response = request
            .send()
            .await
            .map_err(|_| Error::unknown("the request did not complete"))?;
        if !response.status().is_success() {
            return Err(Error {
                status: Some(response.status().as_u16()),
                message: "the request was refused",
            });
        }
        Ok(response)
    }

    async fn bytes(mut response: Response, limit: usize) -> Result<Vec<u8>> {
        let mut result = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| Error::unknown("the response was interrupted"))?
        {
            if chunk.len() > limit.saturating_sub(result.len()) {
                return Err(Error::unknown("the response exceeded its size limit"));
            }
            result.extend_from_slice(&chunk);
        }
        Ok(result)
    }

    async fn json(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        key: Option<&str>,
    ) -> Result<Value> {
        let response = self.response(method, path, body, key).await?;
        serde_json::from_slice(&Self::bytes(response, MAX_RESPONSE).await?)
            .map_err(|_| Error::unknown("the response was not valid JSON"))
    }

    async fn running(&self, computer: &str) -> Result<()> {
        match self.observe(computer).await {
            Outcome::Done { value } if value.state == State::Running => Ok(()),
            Outcome::Done { .. } => Err(Error {
                status: Some(409),
                message: "the computer is not running",
            }),
            _ => Err(Error::unknown(
                "couldn't check whether the computer is running",
            )),
        }
    }

    async fn action(&self, computer: &str, action: &str) -> Outcome<String> {
        if !valid_id(computer, "sbx_") {
            return Outcome::failed("Invalid SPACE computer ID.");
        }
        // Each new lifecycle request needs a new key: one computer can be
        // suspended and woken repeatedly. Retrying an uncertain request belongs
        // to the caller's observation loop, not an automatic mutation retry.
        let result = async {
            let v = self
                .json(
                    Method::POST,
                    &format!("/v1/sandboxes/{computer}:{action}"),
                    None,
                    Some(&uuid::Uuid::new_v4().to_string()),
                )
                .await?;
            let id = field(&v, "operation_id")?;
            self.wait_operation(id).await?;
            Ok::<String, Error>(id.to_owned())
        };
        match tokio::time::timeout(Duration::from_secs(600), result).await {
            Ok(Ok(id)) => Outcome::done(id),
            Ok(Err(e)) => e.outcome(),
            Err(_) => Outcome::unknown(
                "The SPACE operation exceeded its deadline. Check the computer before trying again.",
            ),
        }
    }

    async fn wait_operation(&self, id: &str) -> Result<()> {
        if !valid_id(id, "op_") {
            return Err(Error::unknown("invalid lifecycle operation ID"));
        }
        loop {
            let operation = self
                .json(Method::GET, &format!("/v1/operations/{id}"), None, None)
                .await
                // The mutation was accepted already. A lookup refusal cannot
                // establish whether that mutation took effect.
                .map_err(|_| Error::unknown("couldn't observe the accepted lifecycle operation; check the computer before trying again"))?;
            match field(&operation, "state")? {
                "succeeded" => return Ok(()),
                "queued" | "running" => tokio::time::sleep(Duration::from_millis(100)).await,
                "failed" | "canceled" | "expired" => {
                    return Err(Error::unknown(
                        "the lifecycle operation did not succeed; check the computer before trying again",
                    ));
                }
                _ => {
                    return Err(Error::unknown(
                        "the lifecycle operation has an unknown state",
                    ));
                }
            }
        }
    }

    fn attach_url(&self, session: &str, supplied: &str) -> Result<Url> {
        if !valid_id(session, "proc_") {
            return Err(Error::unknown("invalid command session ID"));
        }
        let path = format!("/v1/sessions/{session}/attach");
        let mut expected = self
            .data
            .join(&path)
            .map_err(|_| Error::unknown("invalid attachment path"))?;
        let http_url = expected.to_string();
        let scheme = if expected.scheme() == "https" {
            "wss"
        } else {
            "ws"
        };
        expected
            .set_scheme(scheme)
            .map_err(|_| Error::unknown("invalid attachment scheme"))?;
        // Do not follow a server-supplied destination with the attach token.
        if supplied != path && supplied != expected.as_str() && supplied != http_url {
            return Err(Error::unknown(
                "the command attachment points to an unexpected destination",
            ));
        }
        Ok(expected)
    }

    async fn command(&self, computer: &str, exec: &Exec) -> Result<ExecOutput> {
        self.running(computer).await?;
        let has_secrets = !exec.secret_env.is_empty();
        let shell = if has_secrets {
            // Read assignments from stdin into this shell, then replace it.
            // Values never appear in saved argv or env. `cat` reads to EOF.
            format!("eval \"$(cat)\"; exec sh -c {}", quote(&exec.command))
        } else {
            exec.command.clone()
        };
        let mut body = json!({"command":["sh", "-c", shell], "stdin":has_secrets,
            "timeout":format!("{}s", exec.timeout_seconds.clamp(1,3600)), "env":{}});
        if let Some(cwd) = &exec.cwd {
            body["cwd"] = json!(cwd);
        }
        let session = self
            .json(
                Method::POST,
                &format!("/v1/sandboxes/{computer}/commands"),
                Some(body),
                Some(&uuid::Uuid::new_v4().to_string()),
            )
            .await?;
        let id = field(&session, "session_id")?;
        let attach = &session["attach"];
        let url = self.attach_url(id, field(attach, "url")?)?;
        let token = field(attach, "token")?;
        if token.is_empty() {
            return Err(Error::unknown("the attachment token is missing"));
        }
        let config = WebSocketConfig::default()
            .max_message_size(Some(2 * 1024 * 1024))
            .max_frame_size(Some(2 * 1024 * 1024));
        let (mut socket, _) =
            tokio_tungstenite::connect_async_with_config(url.as_str(), Some(config), false)
                .await
                .map_err(|_| Error::unknown("couldn't attach to the command"))?;
        socket
            .send(Message::Text(
                json!({"type":"attach", "token": token}).to_string().into(),
            ))
            .await
            .map_err(|_| Error::unknown("couldn't authenticate the command attachment"))?;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut sent = false;
        while let Some(message) = socket.next().await {
            let message =
                message.map_err(|_| Error::unknown("the command stream was interrupted"))?;
            let Message::Text(text) = message else {
                continue;
            };
            let frame: Value =
                serde_json::from_str(&text).map_err(|_| Error::unknown("invalid command frame"))?;
            match field(&frame, "type")? {
                "process_started" if has_secrets && !sent => {
                    let mut input = zeroize::Zeroizing::new(String::new());
                    for (name, value) in &exec.secret_env {
                        input.push_str(&format!("export {name}={}\n", quote(value.expose())));
                    }
                    let encoded = zeroize::Zeroizing::new(BASE64.encode(input.as_bytes()));
                    let payload = zeroize::Zeroizing::new(
                        json!({"type":"input", "seq":1, "data_base64":encoded.as_str()})
                            .to_string(),
                    );
                    socket
                        .send(Message::Text(payload.to_string().into()))
                        .await
                        .map_err(|_| Error::unknown("couldn't send command input"))?;
                    socket
                        .send(Message::Text(
                            json!({"type":"close_stdin", "seq":2}).to_string().into(),
                        ))
                        .await
                        .map_err(|_| Error::unknown("couldn't finish command input"))?;
                    sent = true;
                }
                "stdout" | "stderr" => {
                    let decoded = BASE64
                        .decode(field(&frame, "data_base64")?)
                        .map_err(|_| Error::unknown("invalid command output"))?;
                    if decoded.len() > MAX_OUTPUT.saturating_sub(stdout.len() + stderr.len()) {
                        return Err(Error::unknown("command output exceeded its size limit"));
                    }
                    if frame["type"] == "stdout" {
                        stdout.extend(decoded);
                    } else {
                        stderr.extend(decoded);
                    }
                }
                "process_exited" => {
                    let code = frame["exit_code"]
                        .as_i64()
                        .ok_or_else(|| Error::unknown("missing command exit code"))?;
                    let _ = socket.close(None).await;
                    return Ok(ExecOutput {
                        code,
                        stdout: String::from_utf8_lossy(&stdout).into_owned(),
                        stderr: String::from_utf8_lossy(&stderr).into_owned(),
                    });
                }
                "error" | "resync_required" | "closed" => {
                    return Err(Error::unknown("the command stream could not finish"));
                }
                _ => {}
            }
        }
        Err(Error::unknown(
            "the command stream ended without an exit result",
        ))
    }
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

impl Driver for SpaceDriver {
    fn capabilities(&self) -> Capabilities {
        Capabilities::new(
            "space",
            [Profile::Core, Profile::Suspend],
            &["small", "medium", "large"],
        )
    }

    async fn create(&self, r: &CreateRequest) -> Outcome<String> {
        if r.operation.is_empty()
            || r.operation.len() > 128
            || !r.operation.bytes().all(|b| b.is_ascii_graphic())
        {
            return Outcome::failed(
                "A SPACE create needs an operation ID of up to 128 printable ASCII characters.",
            );
        }
        let resources = match r.size.as_deref().unwrap_or("small") {
            // SPACE forces cold boot whenever a custom shape is supplied,
            // even if it matches the template. Keep the default shape absent
            // so base snapshots and warm pools remain eligible.
            "small" => None,
            "medium" => Some(json!({"vcpu":4,"memory_mib":4096})),
            "large" => Some(json!({"vcpu":8,"memory_mib":8192})),
            _ => return Outcome::failed("Choose a supported SPACE computer size."),
        };
        let mut body = json!({"template":r.image.as_deref().unwrap_or(&self.template), "runtime":"firecracker",
            "metadata":r.labels,
            // Only explicit resume may wake a stopped chat computer.
            "auto_lifecycle":{"wake":{"on":[]},"condition_sets":{},"stages":[]}});
        if let Some(resources) = resources {
            body["resources"] = resources;
        }
        match self
            .json(
                Method::POST,
                "/v1/sandboxes",
                Some(body),
                Some(&r.operation),
            )
            .await
        {
            Ok(v) => match field(&v, "sandbox_id") {
                Ok(id) if valid_id(id, "sbx_") => Outcome::done(id.into()),
                _ => Outcome::unknown(
                    "SPACE did not return a computer ID. Repeat the same create operation to recover it.",
                ),
            },
            Err(e) => e.outcome(),
        }
    }

    async fn observe(&self, computer: &str) -> Outcome<Observation> {
        if !valid_id(computer, "sbx_") {
            return Outcome::failed("Invalid SPACE computer ID.");
        }
        match self
            .json(
                Method::GET,
                &format!("/v1/sandboxes/{computer}"),
                None,
                None,
            )
            .await
        {
            Ok(v) => match field(&v, "state") {
                Ok(s) => Outcome::done(Observation {
                    state: state(s),
                    detail: None,
                }),
                Err(e) => e.outcome(),
            },
            Err(e) if e.status == Some(404) => Outcome::done(Observation {
                state: State::Gone,
                detail: None,
            }),
            Err(e) => e.outcome(),
        }
    }

    async fn exec(&self, computer: &str, exec: &Exec) -> Outcome<ExecOutput> {
        if exec.secret_env.len() > 256
            || exec
                .secret_env
                .iter()
                .map(|(k, v)| k.len() + v.expose().len())
                .sum::<usize>()
                > 65536
            || exec.secret_env.iter().any(|(name, value)| {
                !env_name_ok(name) || value.expose().contains('\0') || value.expose().len() > 8192
            })
        {
            return Outcome::failed(
                "Use valid environment variable names and values without NUL bytes.",
            );
        }
        match tokio::time::timeout(
            Duration::from_secs(exec.timeout_seconds.clamp(1, 3600) + 45),
            self.command(computer, exec),
        )
        .await
        {
            Ok(Ok(output)) => Outcome::done(output),
            Ok(Err(e)) => e.outcome(),
            Err(_) => Outcome::unknown(
                "The SPACE command exceeded its deadline. Check its outcome before running it again.",
            ),
        }
    }

    async fn put_file(&self, computer: &str, path: &str, bytes: &[u8]) -> Outcome<()> {
        if bytes.len() > MAX_FILE || path.is_empty() {
            return Outcome::failed("SPACE accepts files up to 16 MiB and needs a path.");
        }
        if let Err(e) = self.running(computer).await {
            return e.outcome();
        }
        if let Some((parent, _)) = path.rsplit_once('/')
            && !parent.is_empty()
        {
            if let Err(e) = self
                .json(
                    Method::POST,
                    &format!("/v1/sandboxes/{computer}/files:mkdir"),
                    Some(json!({"path":parent, "parents":true})),
                    Some(&uuid::Uuid::new_v4().to_string()),
                )
                .await
            {
                return e.outcome();
            }
        }
        match self
            .json(
                Method::POST,
                &format!("/v1/sandboxes/{computer}/files:write"),
                Some(json!({"path":path, "data_base64":BASE64.encode(bytes), "append":false})),
                Some(&uuid::Uuid::new_v4().to_string()),
            )
            .await
        {
            Ok(v) if v["bytes_written"].as_u64() == Some(bytes.len() as u64) => Outcome::done(()),
            Ok(_) => Outcome::unknown("SPACE did not confirm the full file write."),
            Err(e) => e.outcome(),
        }
    }

    async fn get_file(&self, computer: &str, path: &str) -> Outcome<Vec<u8>> {
        if let Err(e) = self.running(computer).await {
            return e.outcome();
        }
        // Read in chunks to detect files larger than one API read permits.
        let result = async {
            let mut bytes = Vec::new();
            loop {
                let response = self
                    .response(
                        Method::POST,
                        &format!("/v1/sandboxes/{computer}/files:read"),
                        Some(json!({"path":path, "offset":bytes.len(), "length":1024*1024})),
                        None,
                    )
                    .await?;
                let chunk = Self::bytes(response, 1024 * 1024).await?;
                let last = chunk.len() < 1024 * 1024;
                if bytes.len() + chunk.len() > MAX_FILE {
                    return Err(Error::unknown("the file exceeds 16 MiB"));
                }
                bytes.extend(chunk);
                if last {
                    return Ok(bytes);
                }
            }
        };
        match tokio::time::timeout(Duration::from_secs(120), result).await {
            Ok(Ok(bytes)) => Outcome::done(bytes),
            Ok(Err(e)) => e.outcome(),
            Err(_) => Outcome::unknown("The SPACE file read exceeded its deadline."),
        }
    }

    async fn meter(&self, computer: &str) -> Outcome<Meter> {
        self.observe(computer).await.map(|v| Meter {
            running: v.state == State::Running,
            evidence: format!("space-observe:{computer}:{:?}", v.state),
            // The public API exposes no billable-seconds counter. Never
            // substitute wall time or guest uptime for measured usage.
            seconds: None,
        })
    }

    async fn delete(&self, computer: &str) -> Outcome<()> {
        if !valid_id(computer, "sbx_") {
            return Outcome::failed("Invalid SPACE computer ID.");
        }
        match self
            .json(
                Method::DELETE,
                &format!("/v1/sandboxes/{computer}"),
                None,
                Some(&uuid::Uuid::new_v4().to_string()),
            )
            .await
        {
            Ok(_) => Outcome::done(()),
            Err(e) if e.status == Some(404) => Outcome::done(()),
            Err(e) => e.outcome(),
        }
    }

    async fn suspend(&self, computer: &str) -> Outcome<String> {
        self.action(computer, "suspend").await
    }
    async fn resume(&self, computer: &str) -> Outcome<String> {
        self.action(computer, "wake").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_stay_on_explicit_origins() {
        for bad in [
            "http://example.com",
            "https://token@example.com",
            "https://example.com/api",
            "https://example.com?token=a",
            "file:///etc/passwd",
        ] {
            assert!(origin(bad).is_err(), "{bad}");
        }
        let d = SpaceDriver::new(
            "http://127.0.0.1:8080",
            "http://127.0.0.1:8090",
            Secret::new("token"),
            "seed:latest",
        )
        .unwrap();
        assert!(
            d.attach_url("proc_123", "/v1/sessions/proc_123/attach")
                .is_ok()
        );
        assert!(
            d.attach_url(
                "proc_123",
                "http://127.0.0.1:8090/v1/sessions/proc_123/attach"
            )
            .is_ok()
        );
        for bad in [
            "wss://attacker.example/v1/sessions/proc_123/attach",
            "/v1/sessions/proc_OTHER/attach",
            "//attacker.example/attach",
        ] {
            assert!(d.attach_url("proc_123", bad).is_err());
        }
    }
    #[test]
    fn unknown_states_and_errors_do_not_claim_success() {
        assert_eq!(state("suspended"), State::Stopped);
        assert_ne!(state("deleting"), State::Gone);
        assert_ne!(state("new_future_state"), State::Running);
        for status in [None, Some(408), Some(429), Some(500)] {
            assert!(matches!(
                Error {
                    status,
                    message: "failed"
                }
                .outcome::<()>(),
                Outcome::Unknown { .. }
            ));
        }
        assert!(matches!(
            Error {
                status: Some(403),
                message: "refused"
            }
            .outcome::<()>(),
            Outcome::Failed { .. }
        ));
    }
}
