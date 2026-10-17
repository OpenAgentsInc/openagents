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
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio_tungstenite::tungstenite::{Message, protocol::WebSocketConfig};

const MAX_FILE: usize = 16 * 1024 * 1024;
const MAX_RESPONSE: usize = 24 * 1024 * 1024;
const MAX_OUTPUT: usize = 16 * 1024 * 1024;
const STORAGE_PAGE_BYTES: usize = 1024 * 1024;
const STORAGE_PAGE_LIMIT: usize = 100;
const STORAGE_MAX_PAGES: usize = 10;

#[derive(Deserialize)]
struct StoragePage {
    data: Vec<SnapshotStorage>,
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct SnapshotStorage {
    id: String,
    sandbox_id: String,
    state: String,
    size_bytes: Option<u64>,
}

mod lifecycle;
pub use lifecycle::{JournalFuture, LifecycleClaim, LifecycleJournal};

#[derive(Clone)]
pub struct SpaceDriver {
    http: Client,
    api: Url,
    data: Url,
    token: Secret,
    template: String,
    lifecycle_journal: Option<std::sync::Arc<dyn LifecycleJournal>>,
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
            lifecycle_journal: None,
        })
    }

    pub fn with_lifecycle_journal(mut self, journal: std::sync::Arc<dyn LifecycleJournal>) -> Self {
        self.lifecycle_journal = Some(journal);
        self
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

    async fn storage_pages(&self, computer: &str) -> Outcome<StorageObservation> {
        let mut observation = StorageObservation {
            computer: computer.into(),
            objects: Vec::new(),
            inventories: vec![StorageInventory {
                kind: StorageKind::Snapshot,
                coverage: StorageCoverage::Partial,
                source: StorageSource::SpaceSnapshot,
            }],
            cleanup: StorageCleanup::Unknown,
        };
        let mut cursor: Option<String> = None;
        let mut cursors = BTreeSet::new();
        for _ in 0..STORAGE_MAX_PAGES {
            let mut url = match self
                .api
                .join(&format!("/v1/sandboxes/{computer}/snapshots"))
            {
                Ok(url) => url,
                Err(_) => return Outcome::unknown("Couldn't read storage information."),
            };
            url.query_pairs_mut()
                .append_pair("limit", &STORAGE_PAGE_LIMIT.to_string());
            if let Some(cursor) = &cursor {
                url.query_pairs_mut().append_pair("cursor", cursor);
            }
            let response = self.response(Method::GET, url.as_str(), None, None).await;
            let page = match response {
                Ok(response) => match Self::bytes(response, STORAGE_PAGE_BYTES).await {
                    Ok(bytes) => serde_json::from_slice::<StoragePage>(&bytes),
                    Err(_) => {
                        return Outcome::unknown("Couldn't read storage information.");
                    }
                },
                // The public API hides some artifacts, including ones being
                // deleted. Even a missing sandbox cannot prove their absence.
                Err(error) if error.status == Some(404) => break,
                Err(_) if !observation.objects.is_empty() => break,
                Err(_) => return Outcome::unknown("Couldn't read storage information."),
            };
            let page = match page {
                Ok(page) if page.data.len() <= STORAGE_PAGE_LIMIT => page,
                _ => return Outcome::unknown("The storage response was invalid."),
            };
            for snapshot in page.data {
                if snapshot.sandbox_id != computer || !valid_id(&snapshot.id, "snap_") {
                    return Outcome::unknown(
                        "The storage response contained an invalid object ID.",
                    );
                }
                observation.objects.push(StorageObject {
                    id: snapshot.id,
                    kind: StorageKind::Snapshot,
                    state: match snapshot.state.as_str() {
                        "ready" | "creating" => StorageObjectState::Present,
                        "deleting" => StorageObjectState::Deleting,
                        _ => StorageObjectState::Unknown,
                    },
                    bytes: snapshot
                        .size_bytes
                        .filter(|bytes| *bytes <= i64::MAX as u64),
                    // SPACE does not specify one physical-storage meaning for
                    // this field across all snapshot writers.
                    byte_kind: StorageByteKind::ProviderReported,
                    source: StorageSource::SpaceSnapshot,
                });
            }
            if observation.validate().is_err() {
                return Outcome::unknown("The storage response was invalid.");
            }
            cursor = match page.next_cursor {
                None => break,
                Some(cursor)
                    if !cursor.is_empty()
                        && cursor.len() <= 2048
                        && cursor.chars().all(|c| c.is_ascii() && !c.is_control())
                        && cursors.insert(cursor.clone()) =>
                {
                    Some(cursor)
                }
                _ => break,
            };
        }
        if observation
            .objects
            .iter()
            .any(|object| object.state == StorageObjectState::Deleting)
        {
            observation.cleanup = StorageCleanup::Pending;
        }
        Outcome::done(observation)
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

    async fn action(&self, computer: &str, action: &str, operation: &str) -> Outcome<String> {
        if !valid_id(computer, "sbx_") {
            return Outcome::failed("Invalid SPACE computer ID.");
        }
        if operation.is_empty()
            || operation.len() > 128
            || !operation.bytes().all(|b| b.is_ascii_graphic())
        {
            return Outcome::failed("A lifecycle operation needs a valid request ID.");
        }
        // The lifecycle saves this identity before the effect. A restarted
        // caller repeats the same request and waits for the original operation.
        let result = async {
            if let Some(journal) = &self.lifecycle_journal {
                match journal
                    .claim(operation, computer, action)
                    .await
                    .map_err(|_| Error::unknown("couldn't check the earlier request"))?
                {
                    LifecycleClaim::Accepted(id) => {
                        self.wait_operation(&id).await?;
                        return Ok(id);
                    }
                    LifecycleClaim::Uncertain => {
                        return Err(Error::unknown("the earlier request couldn't be confirmed"));
                    }
                    LifecycleClaim::Rejected => {
                        if action == "delete"
                            && matches!(self.observe(computer).await, Outcome::Done { value } if value.state == State::Gone)
                        {
                            return Ok(format!("missing:{computer}"));
                        }
                        return Err(Error {
                            status: Some(409),
                            message: "the earlier request was refused",
                        });
                    }
                    LifecycleClaim::Fresh => {}
                }
            }
            let (method, path) = if action == "delete" {
                (Method::DELETE, format!("/v1/sandboxes/{computer}"))
            } else {
                (Method::POST, format!("/v1/sandboxes/{computer}:{action}"))
            };
            let v = match self.json(method, &path, None, Some(operation)).await {
                Ok(value) => value,
                Err(error) => {
                    if error.status.is_some_and(|status| {
                        (400..500).contains(&status) && status != 408 && status != 429
                    }) {
                        if let Some(journal) = &self.lifecycle_journal {
                            journal.rejected(operation).await.map_err(|_| {
                                Error::unknown("couldn't record the request refusal")
                            })?;
                        }
                    }
                    if action == "delete" && error.status == Some(404) {
                        return Ok(format!("missing:{computer}"));
                    }
                    return Err(error);
                }
            };
            let id = field(&v, "operation_id")?;
            if !valid_id(id, "op_") {
                return Err(Error::unknown("invalid lifecycle operation ID"));
            }
            if let Some(journal) = &self.lifecycle_journal {
                journal
                    .accepted(operation, id)
                    .await
                    .map_err(|_| Error::unknown("couldn't record the request"))?;
            }
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
                    exec.output.append(frame["type"] == "stderr", &decoded);
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
        let mut caps = Capabilities::new(
            "space",
            [Profile::Core, Profile::Suspend],
            &["small", "medium", "large"],
        );
        caps.memory_suspend = true;
        let artifact = crate::Artifact {
            kind: crate::ArtifactKind::SpaceTemplate,
            reference: self.template.clone(),
            provider_identity: None,
            disk_gib: None,
        };
        if artifact.validate().is_ok() {
            caps.default_artifact = Some(artifact);
        }
        // Small uses the template's shape; do not guess it from the size name.
        caps.shapes = [("medium", 4, 4096), ("large", 8, 8192)]
            .into_iter()
            .map(|(name, vcpus, memory_mib)| {
                (
                    name.into(),
                    crate::Shape {
                        vcpus,
                        memory_mib,
                        min_disk_gib: None,
                    },
                )
            })
            .collect();
        caps
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
        let create = async {
            let response = self
                .response(
                    Method::POST,
                    "/v1/sandboxes",
                    Some(body),
                    Some(&r.operation),
                )
                .await?;
            let accepted = response.status() == reqwest::StatusCode::ACCEPTED;
            let value: Value = serde_json::from_slice(&Self::bytes(response, MAX_RESPONSE).await?)
                .map_err(|_| Error::unknown("the response was not valid JSON"))?;
            let id = field(&value, "sandbox_id")?;
            if !valid_id(id, "sbx_") {
                return Err(Error::unknown(
                    "the create response has an invalid computer ID",
                ));
            }
            // An accepted create can precede the sandbox row. Do not expose
            // it to observation until its operation completes: an early 404
            // would otherwise look like a deleted computer.
            if let Some(operation) = value.get("operation_id") {
                let operation = operation
                    .as_str()
                    .ok_or_else(|| Error::unknown("invalid create operation ID"))?;
                self.wait_operation(operation).await?;
            } else if accepted {
                return Err(Error::unknown("the accepted create has no operation ID"));
            }
            Ok::<String, Error>(id.to_owned())
        };
        match tokio::time::timeout(Duration::from_secs(600), create).await {
            Ok(Ok(id)) => Outcome::done(id),
            Ok(Err(error)) => error.outcome(),
            Err(_) => Outcome::unknown(
                "The SPACE create exceeded its deadline. Repeat the same create operation to check it.",
            ),
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

    async fn storage(&self, computer: &str) -> Outcome<StorageObservation> {
        if !valid_id(computer, "sbx_") {
            return Outcome::failed("Invalid SPACE computer ID.");
        }
        match tokio::time::timeout(Duration::from_secs(30), self.storage_pages(computer)).await {
            Ok(observation) => observation,
            Err(_) => Outcome::unknown("The storage check exceeded its deadline."),
        }
    }

    async fn delete(&self, computer: &str) -> Outcome<()> {
        // A provider resource has one terminal deletion. Include its origin so
        // separate deployments cannot share an operation by resource ID alone.
        let mut digest = Sha256::new();
        digest.update(self.api.as_str());
        digest.update([0]);
        digest.update(computer);
        let operation = format!("delete:{:x}", digest.finalize());
        self.action(computer, "delete", &operation)
            .await
            .map(|_| ())
    }

    async fn suspend(&self, computer: &str) -> Outcome<String> {
        self.action(computer, "suspend", &uuid::Uuid::new_v4().to_string())
            .await
    }
    async fn resume(&self, computer: &str) -> Outcome<String> {
        self.action(computer, "wake", &uuid::Uuid::new_v4().to_string())
            .await
    }
    async fn suspend_operation(&self, computer: &str, operation: &str) -> Outcome<String> {
        self.action(computer, "suspend", operation).await
    }
    async fn resume_operation(&self, computer: &str, operation: &str) -> Outcome<String> {
        self.action(computer, "wake", operation).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_server(
        responses: Vec<(&'static str, u16, Value)>,
    ) -> (SpaceDriver, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let driver =
            SpaceDriver::new(&address, &address, Secret::new("test"), "seed:latest").unwrap();
        let server = std::thread::spawn(move || {
            for (path, status, body) in responses {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(std::time::Instant::now() < deadline, "missing request");
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("accept failed: {error}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 2048];
                loop {
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0 && request.len() + count <= 65536);
                    request.extend_from_slice(&buffer[..count]);
                    if let Some(end) = request.windows(4).position(|v| v == b"\r\n\r\n") {
                        let headers = std::str::from_utf8(&request[..end]).unwrap();
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if request.len() >= end + 4 + length {
                            assert_eq!(
                                headers.lines().next().unwrap().split_whitespace().nth(1),
                                Some(path)
                            );
                            if path == "/v1/sandboxes" {
                                assert!(headers.lines().any(|line| {
                                    line.split_once(':').is_some_and(|(name, value)| {
                                        name.eq_ignore_ascii_case("idempotency-key")
                                            && value.trim() == "stable-create"
                                    })
                                }));
                            }
                            break;
                        }
                    }
                }
                let body = body.to_string();
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        (driver, server)
    }

    fn create_request() -> CreateRequest {
        CreateRequest {
            operation: "stable-create".into(),
            size: Some("small".into()),
            image: None,
            ttl_seconds: None,
            labels: Default::default(),
        }
    }

    fn snapshot(id: &str, size: Value) -> Value {
        json!({"id":id,"sandbox_id":"sbx_test","state":"ready","size_bytes":size})
    }

    #[tokio::test]
    async fn storage_paginates_without_claiming_complete_physical_inventory() {
        let (driver, server) = create_server(vec![
            (
                "/v1/sandboxes/sbx_test/snapshots?limit=100",
                200,
                json!({"data":[snapshot("snap_one", Value::Null)],"next_cursor":"next"}),
            ),
            (
                "/v1/sandboxes/sbx_test/snapshots?limit=100&cursor=next",
                200,
                json!({"data":[snapshot("snap_two", json!(0))],"next_cursor":null}),
            ),
        ]);
        let observation = driver.storage("sbx_test").await.value().unwrap();
        server.join().unwrap();
        assert_eq!(observation.objects.len(), 2);
        assert_eq!(observation.objects[0].bytes, None);
        assert_eq!(observation.objects[1].bytes, Some(0));
        assert_eq!(
            observation.objects[0].byte_kind,
            StorageByteKind::ProviderReported
        );
        assert_eq!(
            observation.inventories[0].coverage,
            StorageCoverage::Partial
        );
        assert_eq!(observation.cleanup, StorageCleanup::Unknown);
    }

    #[tokio::test]
    async fn missing_sandbox_or_repeated_cursor_keeps_cleanup_unknown() {
        let (driver, server) = create_server(vec![(
            "/v1/sandboxes/sbx_test/snapshots?limit=100",
            404,
            json!({"error":"missing"}),
        )]);
        let observation = driver.storage("sbx_test").await.value().unwrap();
        server.join().unwrap();
        assert!(observation.objects.is_empty());
        assert_eq!(observation.cleanup, StorageCleanup::Unknown);
        assert_eq!(
            observation.inventories[0].coverage,
            StorageCoverage::Partial
        );
        let (driver, server) = create_server(vec![
            (
                "/v1/sandboxes/sbx_test/snapshots?limit=100",
                200,
                json!({"data":[snapshot("snap_one", json!(10))],"next_cursor":"next"}),
            ),
            (
                "/v1/sandboxes/sbx_test/snapshots?limit=100&cursor=next",
                200,
                json!({"data":[snapshot("snap_two", json!(20))],"next_cursor":"next"}),
            ),
        ]);
        let observation = driver.storage("sbx_test").await.value().unwrap();
        server.join().unwrap();
        assert_eq!(observation.objects.len(), 2);
        assert_eq!(
            observation.inventories[0].coverage,
            StorageCoverage::Partial
        );
        assert_eq!(observation.cleanup, StorageCleanup::Unknown);
    }

    #[tokio::test]
    async fn invalid_storage_identity_duplicates_and_negative_sizes_fail_closed() {
        let mut foreign = snapshot("snap_one", json!(10));
        foreign["sandbox_id"] = json!("sbx_other");
        for data in [
            json!([foreign]),
            json!([snapshot("snap_one", json!(-1))]),
            json!([
                snapshot("snap_one", json!(10)),
                snapshot("snap_one", json!(10))
            ]),
        ] {
            let (driver, server) = create_server(vec![(
                "/v1/sandboxes/sbx_test/snapshots?limit=100",
                200,
                json!({"data":data,"next_cursor":null}),
            )]);
            assert!(matches!(
                driver.storage("sbx_test").await,
                Outcome::Unknown { .. }
            ));
            server.join().unwrap();
        }
    }

    #[tokio::test]
    async fn accepted_create_waits_for_its_operation_before_observation() {
        let (driver, server) = create_server(vec![
            (
                "/v1/sandboxes",
                202,
                json!({"sandbox_id":"sbx_test", "operation_id":"op_test"}),
            ),
            ("/v1/operations/op_test", 200, json!({"state":"queued"})),
            ("/v1/operations/op_test", 200, json!({"state":"running"})),
            ("/v1/operations/op_test", 200, json!({"state":"succeeded"})),
            ("/v1/sandboxes/sbx_test", 200, json!({"state":"running"})),
        ]);
        assert_eq!(
            driver.create(&create_request()).await.value().as_deref(),
            Some("sbx_test")
        );
        assert_eq!(
            driver.observe("sbx_test").await.value().unwrap().state,
            State::Running
        );
        server.join().unwrap();
    }

    #[tokio::test]
    async fn accepted_create_lookup_failure_remains_unknown_and_retry_keeps_identity() {
        let (driver, server) = create_server(vec![
            (
                "/v1/sandboxes",
                202,
                json!({"sandbox_id":"sbx_test", "operation_id":"op_test"}),
            ),
            (
                "/v1/operations/op_test",
                403,
                json!({"error":"private provider detail"}),
            ),
            (
                "/v1/sandboxes",
                202,
                json!({"sandbox_id":"sbx_test", "operation_id":"op_test"}),
            ),
            ("/v1/operations/op_test", 200, json!({"state":"succeeded"})),
        ]);
        assert!(matches!(
            driver.create(&create_request()).await,
            Outcome::Unknown { .. }
        ));
        assert_eq!(
            driver.create(&create_request()).await.value().as_deref(),
            Some("sbx_test")
        );
        server.join().unwrap();
    }

    #[tokio::test]
    async fn unconfirmed_create_never_claims_completion() {
        for (status, value, operation) in [
            (202, json!({"sandbox_id":"sbx_test"}), None),
            (
                202,
                json!({"sandbox_id":"sbx_test", "operation_id":"op_test"}),
                Some(json!({"state":"failed"})),
            ),
            (
                200,
                json!({"sandbox_id":"sbx_test", "operation_id":"op_test"}),
                Some(json!({"state":"future_state"})),
            ),
        ] {
            let mut responses = vec![("/v1/sandboxes", status, value)];
            if let Some(operation) = operation {
                responses.push(("/v1/operations/op_test", 200, operation));
            }
            let (driver, server) = create_server(responses);
            assert!(matches!(
                driver.create(&create_request()).await,
                Outcome::Unknown { .. }
            ));
            server.join().unwrap();
        }
        let (driver, server) = create_server(vec![(
            "/v1/sandboxes",
            201,
            json!({"sandbox_id":"sbx_sync"}),
        )]);
        assert_eq!(
            driver.create(&create_request()).await.value().as_deref(),
            Some("sbx_sync")
        );
        server.join().unwrap();
    }

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
