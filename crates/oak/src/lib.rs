//! `oak` — the caller implementation the `oak` CLI and the `oak-mcp`
//! stdio server share.
//!
//! The shared half holds what both surfaces need identically: where the
//! endpoint, credential, and workspace come from; how a config file is
//! trusted; how a bounded read works; and how one HTTP call runs — its
//! timeout, its retry loop honoring `Retry-After`, and its
//! `(Idempotency-Key, X-Attempt)` settlement pair. Read
//! `docs/decision-models/guides/caller.md` and
//! `docs/decision-models/guides/classification-callers.md`.

pub use discovery::corpus as docs;
#[cfg(feature = "mcp-http")]
pub mod mcp_http;

use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use jev::{BlockingClient, Config, RetryPolicy};
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;

/// Every row answered.
pub const EXIT_ANSWERED: i32 = 0;
/// The run itself failed: bad credentials, an unbound door, a malformed
/// envelope — the fix is the caller's, not a retry's.
pub const EXIT_FAILURE: i32 = 1;
/// The command line or its inputs did not parse.
pub const EXIT_USAGE: i32 = 2;
/// No row answered and at least one was refused.
pub const EXIT_REFUSED: i32 = 3;
/// No row answered or refused and at least one was unavailable or invalid.
pub const EXIT_UNAVAILABLE: i32 = 4;
/// Some mix of the above.
pub const EXIT_MIXED: i32 = 5;

/// A response error the caller must fix rather than retry: credentials,
/// bindings, and envelope shape apply to every row identically, so the run
/// stops on the first.
pub const CALLER_FAULTS: &[&str] = &[
    "unauthenticated",
    "door_not_bound",
    "malformed",
    "invalid_request",
    "too_many_questions",
    "too_many_options",
];

/// The classify envelope's own caller faults, beyond [`CALLER_FAULTS`]:
/// the refusal codes `POST /v1/classify` raises before any input runs.
pub const CLASSIFY_ENVELOPE_FAULTS: &[&str] = &[
    "unsupported_schema",
    "unsupported_mode",
    "unsupported_limits",
    "unsupported_capacity",
    "invalid_id",
    "duplicate_id",
    "too_many_inputs",
    "too_many_labels",
    "too_many_levels",
    "too_many_dimensions",
    "too_many_judgments",
    "content_too_large",
    "overflow",
    "workspace_required",
    "workspace_forbidden",
];

/// Failure codes that name temporary capacity, not a caller fault.
const UNAVAILABLE_CODES: &[&str] = &[
    "rate_limited",
    "busy",
    "overloaded",
    "unavailable",
    "door_unavailable",
];

/// Unavailable-class codes a retry cannot help: the door's identity or
/// the stores behind admission failed, not its capacity.
const UNRETRYABLE_UNAVAILABLE: &[&str] = &[
    "identity_mismatch",
    "registry_unavailable",
    "ledger_unavailable",
];

/// The longest a `Retry-After` is honored before the attempt goes anyway.
pub const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

/// The schema tag a classification envelope and report carry.
pub const CLASSIFY_SCHEMA: &str = "openagents.classify.v1";

/// The schema tag the decision policy a classify envelope carries.
pub const CLASSIFY_POLICY_SCHEMA: &str = "openagents.classify-policy.v1";

/// The schema tag a review-and-fallback policy document carries.
pub const CLASSIFY_REVIEW_SCHEMA: &str = "openagents.classify-review.v1";

/// The native decision route.
pub const SYSTEMONE_PATH: &str = "/v1/systemone";

/// The classification route.
pub const CLASSIFY_PATH: &str = "/v1/classify";

/// The models route.
pub const MODELS_PATH: &str = "/v1/models";

/// The workspace membership header a deployment can require.
pub const WORKSPACE_HEADER: &str = "x-workspace-id";

/// The largest classification envelope `oak` reads, whether it arrives as
/// a file, on standard input, or inside an MCP tool argument. The
/// gateway's own `max_body_bytes` is the effective ceiling — this bound
/// keeps a caller from staging an unbounded document.
pub const MAX_ENVELOPE_BYTES: u64 = 16 * 1024 * 1024;

/// The largest response body `oak` reads. A classification report grows
/// with its inputs, so the bound sits well above the envelope's.
pub const MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;

/// The largest single JSON-RPC message the MCP surface reads — the
/// envelope bound plus protocol framing.
pub const MAX_MCP_MESSAGE_BYTES: u64 = MAX_ENVELOPE_BYTES + 256 * 1024;

/// The longest idempotency key a caller may name.
pub const MAX_REQUEST_ID_CHARS: usize = 512;

/// Settings from the flag, the environment, or the config file, in that
/// order. `api_key` is never a flag.
#[derive(Debug, Default, serde::Deserialize)]
pub struct FileConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    /// The workspace every request names in `X-Workspace-Id`, for a
    /// deployment that requires workspace membership.
    pub workspace: Option<String>,
}

/// Endpoint, credential, and optional workspace, resolved.
pub struct Settings {
    pub api_key: String,
    pub base_url: String,
    pub model: Option<String>,
    pub workspace: Option<String>,
}

impl Settings {
    /// Resolve the endpoint, credential, and workspace from the flag, the
    /// environment, then the config file.
    ///
    /// # Errors
    ///
    /// Returns a message when no endpoint or credential resolves, the
    /// config file cannot be trusted or parsed, or the workspace id is
    /// not a header value.
    pub fn resolve(
        url: Option<String>,
        model: Option<String>,
        workspace: Option<String>,
        config: Option<PathBuf>,
    ) -> Result<Self, String> {
        Self::resolve_key(url, model, workspace, config, None)
    }

    /// The same resolution with a caller-supplied credential checked
    /// first — an MCP client presenting its own `oak_` key works under
    /// its own identity without touching the operator's configuration.
    ///
    /// # Errors
    ///
    /// Same as [`Settings::resolve`]; a missing `key` falls through to
    /// the environment and config file as before.
    pub fn resolve_key(
        url: Option<String>,
        model: Option<String>,
        workspace: Option<String>,
        config: Option<PathBuf>,
        key: Option<String>,
    ) -> Result<Self, String> {
        let file = load_config(config)?;
        let base_url = url
            .or_else(|| env("OPENAGENTS_BASE_URL"))
            .or(file.base_url)
            .ok_or_else(|| {
                "no service root: pass --url, set OPENAGENTS_BASE_URL, or name \
                 `base_url` in the config file"
                    .to_string()
            })?;
        let api_key = key
            .or_else(|| env("OPENAGENTS_API_KEY"))
            .or(file.api_key)
            .ok_or_else(|| {
                "no credential: present an `oak_` bearer key, set OPENAGENTS_API_KEY, or \
             name `api_key` in the config file — a key never goes on the command line"
                    .to_string()
            })?;
        let model = model.or_else(|| env("OPENAGENTS_MODEL")).or(file.model);
        let workspace = workspace
            .or_else(|| env("OPENAGENTS_WORKSPACE"))
            .or(file.workspace);
        if let Some(workspace) = &workspace
            && HeaderValue::from_str(workspace).is_err()
        {
            return Err(
                "the workspace ID can't be sent as an HTTP header; use printable ASCII".to_string(),
            );
        }
        Ok(Self {
            api_key,
            base_url,
            model,
            workspace,
        })
    }

    /// The `systemone` client, with oak's own retry loop left to the
    /// caller: the service settles quota by `(request, attempt)`, and
    /// only the caller can bump `x-attempt`.
    pub fn client(&self, timeout: Duration) -> Result<BlockingClient, String> {
        BlockingClient::new(
            Config::new()
                .api_key(self.api_key.clone())
                .base_url(self.base_url.clone())
                .timeout(timeout)
                .default_headers(self.overlay_headers()?)
                .retry(RetryPolicy {
                    max_retries: 0,
                    ..RetryPolicy::default()
                }),
        )
        .map_err(|error| error.to_string())
    }

    /// The headers every call carries beyond the wire's own — today, the
    /// workspace membership header a `require_workspace_membership`
    /// gateway reads.
    pub fn overlay_headers(&self) -> Result<HeaderMap, String> {
        let mut headers = HeaderMap::new();
        if let Some(workspace) = &self.workspace {
            let value = HeaderValue::from_str(workspace).map_err(|_| {
                "the workspace ID can't be sent as an HTTP header; use printable ASCII".to_string()
            })?;
            headers.insert(WORKSPACE_HEADER, value);
        }
        Ok(headers)
    }

    /// The raw transport the classify and listing surfaces share — the
    /// routes the SDK does not model.
    pub fn transport(&self) -> Result<Transport, String> {
        Transport::new(self)
    }
}

/// One environment value, trimmed; an empty one reads as unset.
pub fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Read the config file. A file that does not exist is no file; a file a
/// group or world permission can read is refused rather than trusted.
pub fn load_config(path: Option<PathBuf>) -> Result<FileConfig, String> {
    let path = path
        .or_else(|| env("OPENAGENTS_CONFIG").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".config/openagents/oak.json"))
        });
    let Some(path) = path else {
        return Ok(FileConfig::default());
    };
    if !path.exists() {
        return Ok(FileConfig::default());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path)
            .map_err(|error| format!("can't check {}: {error}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "other users can read {}, which holds your API key; run `chmod 600 {}`",
                path.display(),
                path.display()
            ));
        }
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("can't read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("{} isn't valid JSON: {error}", path.display()))
}

/// Read a bounded input: a file, a pipe, or a tool argument's bytes. A
/// source past `limit` is refused whole rather than truncated.
pub fn read_bounded(mut reader: impl Read, limit: u64, what: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("can't read the {what}: {error}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!(
            "the {what} is larger than the {}-byte limit",
            limit
        ));
    }
    Ok(bytes)
}

/// What a call's per-attempt timeout, retry count, and idempotency key
/// are.
#[derive(Clone, Default)]
pub struct CallOpts {
    /// Each attempt's timeout, covering the body read.
    pub timeout: Duration,
    /// Retries after the first attempt.
    pub retries: u32,
    /// The `Idempotency-Key` the call settles quota under, when the
    /// caller names one. Every retry keeps it and bumps `x-attempt`.
    pub request_id: Option<String>,
}

/// How one HTTP call ended at the service.
#[derive(Debug)]
pub enum Reply {
    /// A response document — a classify report arrives under `200`,
    /// `422`, or `503` depending on its outcome; the models listing under
    /// `200`. The body is whatever the service sent.
    Document {
        /// The response status.
        status: u16,
        /// The `x-request-id` the call carried.
        request_id: Option<String>,
        /// The response body.
        body: Value,
    },
    /// A typed refusal envelope — `{"error": {"code", "message"}}`.
    Refused {
        /// The response status.
        status: u16,
        /// The typed code.
        code: String,
        /// The message the envelope carried.
        message: String,
        /// The `x-request-id` the call carried.
        request_id: Option<String>,
        /// The response body as sent, for verbatim reporting.
        body: Value,
    },
}

/// A call that never produced a reply the caller can use: the transport
/// or timeout after its retries, or a body that could not be read.
#[derive(Debug)]
pub struct CallError {
    /// A stable name for the failure class.
    pub code: &'static str,
    /// What happened.
    pub message: String,
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// The raw HTTP caller for the routes the SDK does not model:
/// `POST /v1/classify` and the passthrough read of `GET /v1/models`. It
/// reuses the resolved credential, the workspace header, the bounded
/// timeout, and oak's own retry loop — the same `(request, attempt)`
/// settlement pair `ask` runs.
pub struct Transport {
    /// The runtime each call parks on — the caller surfaces are
    /// synchronous, the HTTP client is not.
    runtime: tokio::runtime::Runtime,
    /// The HTTP client.
    http: reqwest::Client,
    /// The validated service root — `Settings::resolve` output.
    base_url: String,
    /// The bearer credential, redacted by `ApiKey` under `Debug`.
    api_key: jev::ApiKey,
    /// The workspace id to send, when one resolved.
    workspace: Option<String>,
}

impl Transport {
    /// Build the transport over resolved settings.
    ///
    /// # Errors
    ///
    /// Returns a message when the workspace id is not a header value, the
    /// runtime does not build, or the HTTP client does not build.
    pub fn new(settings: &Settings) -> Result<Self, String> {
        // The SDK's own resolution is the URL policy: http or https only,
        // trailing slashes dropped, credentials in the URL rejected the
        // way `Config` checks them.
        let checked = settings.client(Duration::from_secs(1))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("oak couldn't start its async runtime: {error}"))?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| format!("oak couldn't set up its HTTP client: {error}"))?;
        Ok(Self {
            runtime,
            http,
            base_url: checked.client().base_url().to_string(),
            api_key: jev::ApiKey::new(settings.api_key.clone()),
            workspace: settings.workspace.clone(),
        })
    }

    /// `GET /v1/models`, the listing document as the service wrote it.
    ///
    /// # Errors
    ///
    /// Returns [`CallError`] when the call never produced a usable reply.
    pub fn get_models(&self, opts: &CallOpts) -> Result<Reply, CallError> {
        self.exchange(reqwest::Method::GET, MODELS_PATH, None, opts)
    }

    /// `POST /v1/systemone`, sending the request bytes as received.
    ///
    /// # Errors
    ///
    /// Returns [`CallError`] when the call never produced a usable reply,
    /// or when a `2xx` body does not carry a typed `answers` object.
    pub fn post_systemone(&self, envelope: &[u8], opts: &CallOpts) -> Result<Reply, CallError> {
        let reply = self.exchange(reqwest::Method::POST, SYSTEMONE_PATH, Some(envelope), opts)?;
        if let Reply::Document { body, .. } = &reply
            && (!body.get("answers").is_some_and(Value::is_object)
                || !body.get("model").is_some_and(Value::is_string))
        {
            return Err(CallError {
                code: "invalid_response",
                message: "the service response isn't a decision answer".into(),
            });
        }
        Ok(reply)
    }

    /// `POST /v1/classify`, sending the envelope bytes as received.
    ///
    /// # Errors
    ///
    /// Returns [`CallError`] when the call never produced a usable reply.
    /// A classify report — whatever its outcome — and a typed refusal are
    /// both a [`Reply`].
    pub fn post_classify(&self, envelope: &[u8], opts: &CallOpts) -> Result<Reply, CallError> {
        let reply = self.exchange(reqwest::Method::POST, CLASSIFY_PATH, Some(envelope), opts)?;
        if let Reply::Document { body, .. } = &reply
            && (body.get("v").and_then(Value::as_str) != Some(CLASSIFY_SCHEMA)
                || !matches!(
                    body.get("outcome").and_then(Value::as_str),
                    Some("answered" | "mixed" | "refused" | "unavailable" | "unattempted")
                )
                || !body.get("results").is_some_and(Value::is_array)
                || !body.get("outcomes").is_some_and(Value::is_object))
        {
            return Err(CallError {
                code: "invalid_response",
                message: "the service response isn't a classification report".into(),
            });
        }
        Ok(reply)
    }

    /// One call's retry loop, run on the transport's runtime: the same
    /// semantics `ask` runs — retry the unavailable class, honor
    /// `Retry-After` up to the bound, keep the idempotency key and bump
    /// `x-attempt` per attempt.
    fn exchange(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&[u8]>,
        opts: &CallOpts,
    ) -> Result<Reply, CallError> {
        self.runtime
            .block_on(self.exchange_async(method, path, body, opts))
    }

    async fn exchange_async(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&[u8]>,
        opts: &CallOpts,
    ) -> Result<Reply, CallError> {
        let url = format!("{}{path}", self.base_url);
        let retry_uncertain = method == reqwest::Method::GET || opts.request_id.is_some();
        let mut attempt = 0_u32;
        let mut delay = Duration::from_millis(250);
        loop {
            attempt += 1;
            let mut request = self
                .http
                .request(method.clone(), &url)
                .timeout(opts.timeout)
                .header("accept", "application/json")
                .header("user-agent", format!("oak/{}", env!("CARGO_PKG_VERSION")))
                .bearer_auth(self.api_key.expose());
            if let Some(workspace) = &self.workspace {
                request = request.header(WORKSPACE_HEADER, workspace);
            }
            if let Some(body) = body {
                request = request
                    .header("content-type", "application/json")
                    .body(body.to_vec());
            }
            if let Some(key) = &opts.request_id {
                request = request
                    .header("idempotency-key", key.as_str())
                    .header("x-attempt", attempt.to_string());
            }
            let mut response = match request.send().await {
                Ok(response) => response,
                Err(error) => {
                    if retry_uncertain && attempt <= opts.retries {
                        tokio::time::sleep(delay.min(MAX_RETRY_AFTER)).await;
                        delay = (delay * 2).min(Duration::from_secs(5));
                        continue;
                    }
                    return Err(CallError {
                        code: "unavailable",
                        message: format!("oak couldn't reach the service: {error}"),
                    });
                }
            };
            let status = response.status().as_u16();
            let headers = response.headers().clone();
            let mut bytes = Vec::new();
            // The bound is checked by length, not by trusting a content
            // length: an unfinished or oversized body reads as a
            // failure, never as a truncated report.
            let mut body_error = None;
            let mut oversized = false;
            while let Some(chunk) = match response.chunk().await {
                Ok(chunk) => chunk,
                Err(error) => {
                    body_error = Some(error);
                    None
                }
            } {
                if bytes.len() as u64 + chunk.len() as u64 > MAX_RESPONSE_BYTES {
                    oversized = true;
                    break;
                }
                bytes.extend_from_slice(&chunk);
            }
            if oversized {
                return Err(CallError {
                    code: "response_too_large",
                    message: format!(
                        "the response is larger than the {MAX_RESPONSE_BYTES}-byte limit"
                    ),
                });
            }
            if let Some(error) = body_error {
                if retry_uncertain && attempt <= opts.retries {
                    tokio::time::sleep(delay.min(MAX_RETRY_AFTER)).await;
                    delay = (delay * 2).min(Duration::from_secs(5));
                    continue;
                }
                return Err(CallError {
                    code: "unavailable",
                    message: format!("oak couldn't read the response body: {error}"),
                });
            }
            let reply = reply_of(status, &headers, &bytes);
            if !retryable(&reply) {
                return Ok(reply);
            }
            if attempt <= opts.retries {
                let asked = jev::parse_retry_after(&headers).unwrap_or(delay);
                tokio::time::sleep(asked.min(MAX_RETRY_AFTER)).await;
                delay = (delay * 2).min(Duration::from_secs(5));
                continue;
            }
            return Ok(reply);
        }
    }
}

/// The reply a response carries: a document when the service wrote one —
/// a classify report arrives under `422` and `503` too — else a typed
/// refusal, else a status-named refusal around the body that arrived.
fn reply_of(status: u16, headers: &HeaderMap, bytes: &[u8]) -> Reply {
    let request_id = headers
        .get("x-request-id")
        .or_else(|| headers.get("x-typesafe-request-id"))
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body: Option<Value> = serde_json::from_slice(bytes).ok();
    if let Some(error) = body
        .as_ref()
        .and_then(|body| body.get("error"))
        .filter(|error| error.is_object())
    {
        let code = error
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("the service returned HTTP {status}"));
        return Reply::Refused {
            status,
            code,
            message,
            request_id,
            body: body.unwrap_or(Value::Null),
        };
    }
    if let Some(body) = body {
        return Reply::Document {
            status,
            request_id,
            body,
        };
    }
    let text = String::from_utf8_lossy(bytes);
    Reply::Refused {
        status,
        code: format!("http_{status}"),
        message: if text.trim().is_empty() {
            format!("the service returned HTTP {status} with an empty body")
        } else {
            text.chars().take(200).collect()
        },
        request_id,
        body: Value::String(text.into_owned()),
    }
}

/// Whether a reply is worth another attempt: the unavailable class —
/// `429`, `5xx`, and the temporary-capacity codes — except the failures
/// a retry cannot change.
fn retryable(reply: &Reply) -> bool {
    let (status, code) = match reply {
        // A report carries observed outcomes, even under a failing status.
        // Retrying it would discard that evidence and run the inputs again.
        Reply::Document { .. } => return false,
        Reply::Refused { status, code, .. } => (*status, code.as_str()),
    };
    let unavailable =
        status == 408 || status == 429 || status >= 500 || UNAVAILABLE_CODES.contains(&code);
    unavailable && !UNRETRYABLE_UNAVAILABLE.contains(&code)
}

/// The MCP stdio surface: the decision API caller as a Model Context
/// Protocol server.
///
/// The transport is newline-delimited JSON-RPC 2.0 on standard input and
/// output — the stdio transport the MCP specification defines — and the
/// lifecycle is the specified one: `initialize`, the `initialized`
/// notification, then `tools/list` and `tools/call`. Credentials, the
/// endpoint, and the workspace stay in operator configuration; tool
/// arguments carry only the payload.
pub mod mcp {
    use std::io::BufRead;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::Duration;

    use serde_json::{Map, Value, json};

    use super::{
        CLASSIFY_POLICY_SCHEMA, CLASSIFY_REVIEW_SCHEMA, CLASSIFY_SCHEMA, CallOpts,
        MAX_ENVELOPE_BYTES, MAX_MCP_MESSAGE_BYTES, MAX_REQUEST_ID_CHARS, Reply, Settings,
        Transport,
    };

    /// The protocol version this build reports when it cannot answer the
    /// client's — the latest it serves.
    pub const PROTOCOL_VERSION: &str = PROTOCOL_VERSIONS[0];

    /// Every protocol version this build serves, latest first — the same
    /// list the discovery surface publishes.
    pub use discovery::site::MCP_PROTOCOL_VERSIONS as PROTOCOL_VERSIONS;

    /// The `serverInfo.name` the handshake reports — the name the
    /// discovery surface's server card publishes.
    pub use discovery::site::MCP_SERVER_NAME as SERVER_NAME;

    // JSON-RPC 2.0 error codes.
    /// The message was not valid JSON — or exceeded the read bound.
    const PARSE_ERROR: i64 = -32700;
    /// The message was not a JSON-RPC request this server takes.
    const INVALID_REQUEST: i64 = -32600;
    /// The method is not one this server serves.
    const METHOD_NOT_FOUND: i64 = -32601;
    /// The params are wrong — including calls before `initialized`.
    const INVALID_PARAMS: i64 = -32602;

    /// What `oak-mcp` was started with: the operator configuration the
    /// tool calls resolve against. No flag carries a credential.
    #[derive(Clone, Default)]
    pub struct Options {
        /// `--url`: the service root override.
        pub url: Option<String>,
        /// `--config`: the credential file.
        pub config: Option<PathBuf>,
        /// `--workspace`: the `X-Workspace-Id` every call sends.
        pub workspace: Option<String>,
        /// `--timeout`: each attempt's timeout.
        pub timeout: Duration,
        /// `--retries`: retries per call.
        pub retries: u32,
    }

    /// The lifecycle phase: `initialize`, then the `initialized`
    /// notification, then operation.
    #[derive(Clone, Copy, PartialEq)]
    pub(crate) enum Phase {
        /// Nothing negotiated yet — only `initialize` and `ping` answer.
        Start,
        /// `initialize` answered; waiting on `notifications/initialized`.
        Negotiated,
        /// Normal operation.
        Ready,
    }

    /// Run the server over the given streams until standard input closes
    /// or a write fails. Standard error stays free for logging.
    pub fn serve(
        options: &Options,
        input: impl BufRead,
        mut output: impl Write,
        mut log: impl Write,
    ) -> i32 {
        let mut phase = Phase::Start;
        let mut input = input;
        loop {
            let mut line = Vec::new();
            match read_line_bounded(&mut input, &mut line, MAX_MCP_MESSAGE_BYTES) {
                Ok(Line::End) => return 0,
                Ok(Line::Oversize) => {
                    if !emit(
                        &mut output,
                        &error(
                            Value::Null,
                            INVALID_REQUEST,
                            "the message is larger than the size limit",
                        ),
                    ) {
                        return 0;
                    }
                    continue;
                }
                Ok(Line::Read) => {}
                Err(error) => {
                    let _ = writeln!(log, "oak-mcp: can't read standard input: {error}");
                    return 1;
                }
            }
            if line.iter().all(|byte| byte.is_ascii_whitespace()) {
                continue;
            }
            let message: Value = match serde_json::from_slice(&line) {
                Ok(message) => message,
                Err(_) => {
                    if !emit(&mut output, &error(Value::Null, PARSE_ERROR, "parse error")) {
                        return 0;
                    }
                    continue;
                }
            };
            if let Some(response) = handle(&mut phase, options, None, &message)
                && !emit(&mut output, &response)
            {
                return 0;
            }
        }
    }

    /// The result of reading one message line.
    enum Line {
        /// A newline-terminated message.
        Read,
        /// The input closed.
        End,
        /// The line ran past the bound; the remainder was drained to the
        /// next newline so the stream stays aligned.
        Oversize,
    }

    /// Read one newline-terminated message under `limit`; drain an
    /// overlong line to its newline so the next read starts clean.
    fn read_line_bounded(
        input: &mut impl BufRead,
        buffer: &mut Vec<u8>,
        limit: u64,
    ) -> std::io::Result<Line> {
        buffer.clear();
        let read = std::io::Read::take(&mut *input, limit + 1).read_until(b'\n', buffer)?;
        if read == 0 {
            return Ok(Line::End);
        }
        if buffer.last() == Some(&b'\n') {
            buffer.pop();
            return Ok(Line::Read);
        }
        if (read as u64) <= limit {
            // The input closed before a newline — the last line still
            // counts as a message.
            return Ok(Line::Read);
        }
        // The line never ended inside the bound — drain it.
        let mut rest = Vec::new();
        loop {
            rest.clear();
            let n = std::io::Read::take(&mut *input, 65_536).read_until(b'\n', &mut rest)?;
            if n == 0 || rest.last() == Some(&b'\n') {
                return Ok(Line::Oversize);
            }
        }
    }

    /// Write one response message; a closed pipe ends the server.
    fn emit(output: &mut impl Write, message: &Value) -> bool {
        let mut bytes = serde_json::to_vec(message).unwrap_or_default();
        bytes.push(b'\n');
        output
            .write_all(&bytes)
            .and_then(|()| output.flush())
            .is_ok()
    }

    /// One JSON-RPC error response.
    fn error(id: Value, code: i64, message: &str) -> Value {
        json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
    }

    /// One JSON-RPC result response.
    fn result(id: &Value, value: Value) -> Value {
        json!({"jsonrpc": "2.0", "id": id, "result": value})
    }

    /// A tool result that succeeded — the document as structured content
    /// and serialized text, per the tools contract.
    fn tool_result(document: &Value) -> Value {
        json!({
            "content": [{"type": "text", "text": document.to_string()}],
            "structuredContent": document,
            "isError": false,
        })
    }

    /// A tool result reporting a failed call.
    fn tool_error(message: String, detail: Value) -> Value {
        json!({
            "content": [{"type": "text", "text": message}],
            "structuredContent": detail,
            "isError": true,
        })
    }

    /// Dispatch one parsed message. `None` means nothing answers — a
    /// notification, or a response addressed to us. `credential` is a
    /// caller-presented bearer key — the HTTP transport's `Authorization`
    /// header — which inference tools prefer over operator configuration.
    pub(crate) fn handle(
        phase: &mut Phase,
        options: &Options,
        credential: Option<&str>,
        message: &Value,
    ) -> Option<Value> {
        let id = message.get("id").cloned();
        if id
            .as_ref()
            .is_some_and(|id| !id.is_string() && !id.is_number())
        {
            return Some(error(
                Value::Null,
                INVALID_REQUEST,
                "request id must be a string or number",
            ));
        }
        if message.get("method").is_none() {
            // A response to a request this server never sends is
            // ignored; anything else without a method is malformed.
            return match (message.get("result"), message.get("error")) {
                (None, None) => Some(error(
                    id.unwrap_or(Value::Null),
                    INVALID_REQUEST,
                    "the message isn't a request, notification, or response",
                )),
                _ => None,
            };
        }
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Some(error(
                id.unwrap_or(Value::Null),
                INVALID_REQUEST,
                "`method` must be a string",
            ));
        };
        if message.get("jsonrpc") != Some(&json!("2.0")) {
            return Some(error(
                id.unwrap_or(Value::Null),
                INVALID_REQUEST,
                "the message must include `jsonrpc: \"2.0\"`",
            ));
        }
        let Some(id) = id else {
            // A notification: `initialized` moves the lifecycle; every
            // other notification is received and dropped.
            if method == "notifications/initialized" && *phase == Phase::Negotiated {
                *phase = Phase::Ready;
            }
            return None;
        };
        match method {
            "initialize" => initialize(phase, &id, message.get("params")),
            "ping" => Some(result(&id, json!({}))),
            "tools/list" if *phase == Phase::Ready => Some(result(&id, tool_list())),
            "tools/call" if *phase == Phase::Ready => {
                call_tool(options, credential, &id, message.get("params"))
            }
            "tools/list" | "tools/call" => Some(error(
                id,
                INVALID_PARAMS,
                "the session is not initialized yet; send `initialize`, then `notifications/initialized`",
            )),
            _ => Some(error(id, METHOD_NOT_FOUND, "method not found")),
        }
    }

    /// The `initialize` handshake: negotiate the protocol version and
    /// report this server's capabilities.
    fn initialize(phase: &mut Phase, id: &Value, params: Option<&Value>) -> Option<Value> {
        if *phase != Phase::Start {
            return Some(error(
                id.clone(),
                INVALID_PARAMS,
                "the session is already initialized",
            ));
        }
        let Some(requested) = params
            .and_then(|params| params.get("protocolVersion"))
            .and_then(Value::as_str)
        else {
            return Some(error(
                id.clone(),
                INVALID_PARAMS,
                "`initialize` params must include `protocolVersion`",
            ));
        };
        if !params
            .and_then(|p| p.get("capabilities"))
            .is_some_and(Value::is_object)
            || !params
                .and_then(|p| p.get("clientInfo"))
                .is_some_and(|info| {
                    info.get("name").is_some_and(Value::is_string)
                        && info.get("version").is_some_and(Value::is_string)
                })
        {
            return Some(error(
                id.clone(),
                INVALID_PARAMS,
                "`initialize` needs `capabilities` and a `clientInfo` with `name` and `version`",
            ));
        }
        // The negotiated version: the client's when this build serves
        // it, else this build's latest — the client decides whether to
        // stay.
        let version = if PROTOCOL_VERSIONS.contains(&requested) {
            requested
        } else {
            PROTOCOL_VERSION
        };
        *phase = Phase::Negotiated;
        Some(result(
            id,
            json!({
                "protocolVersion": version,
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {
                    "name": SERVER_NAME,
                    "title": "oak, the OpenAgents decision API client",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "instructions": "The documentation tools read the OpenAgents documentation bundled with this server and don't need an API key. Every other tool calls a model, which can use your quota or cost money. `list_models` lists the models your API key can use; `classify` sends an openagents.classify.v1 request to /v1/classify and returns the report.",
            }),
        ))
    }

    /// The tools this server serves. No schema accepts a credential
    /// or an endpoint: those stay in operator configuration.
    pub(crate) fn tool_list() -> Value {
        let mut list = json!({
            "tools": [
                {
                    "name": "list_models",
                    "title": "List the models you can use",
                    "description": "Return the service's GET /v1/models document: the models your API key can use, each with the model version it serves, its capacity tier, and the classification options it supports.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "additionalProperties": false,
                    },
                },
                {
                    "name": "classify",
                    "title": "Classify inputs with a model",
                    "description": "Send an openagents.classify.v1 request to the service's /v1/classify and return the report unchanged: results in input order, selections, totals per unit, and usage. This call can use your quota or cost money.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "request": {
                                "type": "object",
                                "description": "The openagents.classify.v1 request: v, model, capacity, policy, inputs, and labels, levels, or dimensions.",
                            },
                            "request_id": {
                                "type": "string",
                                "description": "An idempotency key for the call. Retries reuse it, so a retry isn't charged twice.",
                                "maxLength": MAX_REQUEST_ID_CHARS,
                            },
                        },
                        "required": ["request"],
                        "additionalProperties": false,
                    },
                },
            ],
        });
        let tools = list["tools"].as_array_mut().expect("tool list is an array");
        tools[0]["annotations"] = json!({"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":true});
        tools[1]["annotations"] = json!({"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false,"openWorldHint":true});
        tools.extend(inference_tools());
        tools.extend(super::docs::tools());
        list
    }

    /// The bounded inference tools: one schema per facade shape, all of
    /// them building the same `openagents.classify.v1` envelope or the
    /// native `POST /v1/systemone` request. None accepts a credential or
    /// an endpoint — inference tools spend the presented or configured
    /// identity, so every one is annotated as a call that can consume
    /// quota or money without changing external content.
    fn inference_tools() -> Vec<Value> {
        let inference = json!({"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false,"openWorldHint":true});
        let texts = json!({
            "type": "array", "minItems": 1, "maxItems": 1000,
            "items": {"type": "string", "minLength": 1},
            "description": "The input texts, in the order results preserve.",
        });
        let labels = json!({
            "type": "array", "minItems": 1, "maxItems": 100,
            "items": {"anyOf": [
                {"type": "string", "minLength": 1},
                {"type": "object", "properties": {"id": {"type": "string"}, "description": {"type": "string"}}, "required": ["id"], "additionalProperties": false},
            ]},
            "description": "The labels to choose from: IDs, or {id, description} objects. The model reads each description as the meaning of that label.",
        });
        let cut =
            |what: &str| json!({"type": "number", "minimum": 0, "maximum": 1, "description": what});
        let model = json!({"type": "string", "description": "The model to use. If you leave it out, the tool uses the configured default model."});
        let capacity = json!({"type": "string", "description": "The capacity tier your API key uses for this model: `shared` unless your account has a dedicated tier.", "default": "shared"});
        let request_id = json!({"type": "string", "maxLength": MAX_REQUEST_ID_CHARS, "description": "An idempotency key for the call. Retries reuse it, so a retry isn't charged twice."});
        let instructions = json!({"type": "string", "maxLength": 16384, "description": "Instructions shared by every judgment in the request."});
        let meta = |required: &[&str]| {
            let mut properties = serde_json::Map::new();
            properties.insert("model".into(), model.clone());
            properties.insert("capacity".into(), capacity.clone());
            properties.insert("request_id".into(), request_id.clone());
            properties.insert("instructions".into(), instructions.clone());
            json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false})
        };
        let tool = |name: &str, title: &str, description: &str, schema: Value| json!({"name": name, "title": title, "description": description, "inputSchema": schema, "annotations": inference.clone()});
        let mut single = meta(&["texts", "labels"]);
        let props = single["properties"]
            .as_object_mut()
            .expect("an object schema");
        props.insert("texts".into(), texts.clone());
        props.insert("labels".into(), labels.clone());
        props.insert(
            "min_probability".into(),
            cut("If the top probability is below this value, return no match instead of a label."),
        );
        props.insert(
            "uncertain_below".into(),
            cut("Mark a result `uncertain` if its top probability is below this value."),
        );
        vec![
            tool(
                "classify_texts",
                "Classify texts into one label",
                "Choose exactly one label, or no match, for each text. The label probabilities for a text add up to one. Returns the full report in input order; use count_labels if you only need totals.",
                single,
            ),
            {
                let mut schema = meta(&["inputs", "dimensions"]);
                let props = schema["properties"]
                    .as_object_mut()
                    .expect("an object schema");
                props.insert("inputs".into(), json!({
                    "type": "array", "minItems": 1, "maxItems": 1000,
                    "items": {"anyOf": [
                        {"type": "string", "minLength": 1},
                        {"type": "object", "properties": {"id": {"type": "string"}, "text": {"type": "string"}, "record": {"type": "object"}}, "required": ["id"], "additionalProperties": false},
                    ]},
                    "description": "The inputs: texts, or {id, text} or {id, record} objects.",
                }));
                props.insert("dimensions".into(), json!({
                    "type": "array", "minItems": 1, "maxItems": 20,
                    "items": {"type": "object", "properties": {
                        "id": {"type": "string"},
                        "mode": {"type": "string", "enum": ["single-label", "multi-label", "binary", "score"]},
                        "labels": labels.clone(),
                        "levels": {"type": "array", "minItems": 2, "maxItems": 10, "items": {"type": "string"}},
                        "instructions": {"type": "string"},
                    }, "required": ["id", "mode"], "additionalProperties": false},
                    "description": "Named dimensions, each classified separately. A single-label dimension needs labels; multi-label and binary dimensions need labels and the request's threshold; a score dimension needs levels.",
                }));
                props.insert("threshold".into(), cut("Required if any dimension is multi-label or binary: the probability a label must reach to be selected."));
                props.insert("top_n".into(), json!({"type": "integer", "minimum": 1, "description": "The most labels to select for each multi-label dimension."}));
                props.insert(
                    "uncertain_below".into(),
                    cut("Mark a result `uncertain` if its top probability is below this value."),
                );
                tool(
                    "classify_dimensions",
                    "Classify inputs on named dimensions",
                    "Classify the same inputs on several separate dimensions, each with its own mode and its own labels or scoring levels. Returns the full report in input order.",
                    schema,
                )
            },
            {
                let mut schema = meta(&["texts", "labels", "threshold"]);
                let props = schema["properties"]
                    .as_object_mut()
                    .expect("an object schema");
                props.insert("texts".into(), texts.clone());
                props.insert("labels".into(), labels.clone());
                props.insert("threshold".into(), cut("Required. The probability a label must reach to be selected. There is no default; choose a value that fits your data."));
                props.insert("top_n".into(), json!({"type": "integer", "minimum": 1, "description": "The most labels to select for each input."}));
                props.insert(
                    "uncertain_below".into(),
                    cut("Mark a result `uncertain` if, for any label, the more likely of yes or no has a probability below this value."),
                );
                tool(
                    "classify_multi_label",
                    "Classify texts with independent labels",
                    "Get a separate probability for each label on each text; the probabilities don't add up to one. Every label at or above your threshold is selected. Returns the full report in input order.",
                    schema,
                )
            },
            {
                let mut schema = meta(&["texts", "labels"]);
                let props = schema["properties"]
                    .as_object_mut()
                    .expect("an object schema");
                props.insert("texts".into(), texts.clone());
                props.insert("labels".into(), labels.clone());
                props.insert(
                    "min_probability".into(),
                    cut("If the top probability is below this value, return no match instead of a label."),
                );
                tool(
                    "count_labels",
                    "Count texts per label",
                    "Classify texts the same way as classify_texts, but return only totals: the count for each label, the no-match and unavailable counts, and usage. The result for each text is left out, which keeps the response small.",
                    schema,
                )
            },
            {
                let mut schema = meta(&["texts", "labels", "uncertain_below"]);
                let props = schema["properties"]
                    .as_object_mut()
                    .expect("an object schema");
                props.insert("texts".into(), texts.clone());
                props.insert("labels".into(), labels.clone());
                props.insert("uncertain_below".into(), cut("Required. Mark a result `uncertain` if its top label probability is below this value."));
                props.insert("reviewer".into(), json!({"type": "string", "description": "A second model that classifies the uncertain results again. It reviews at most 200 items with at most 400 attempts in 30 seconds, and keeps the original result for any item it can't review."}));
                props.insert("show".into(), json!({"type": "integer", "minimum": 1, "maximum": 100, "default": 25, "description": "How many of the most uncertain items to list. The other inputs are left out of the response."}));
                tool(
                    "review_uncertain",
                    "List uncertain classifications",
                    "Classify texts with one label each, and return the results whose top probability is below `uncertain_below`: the input ID, the selected label, and the top probability, up to `show` items, plus counts by outcome. With `reviewer`, that model classifies the uncertain results again first.",
                    schema,
                )
            },
            {
                let schema = json!({"type": "object", "properties": {
                    "state": {"description": "The JSON or text the questions are about."},
                    "questions": {"type": "object", "minProperties": 1, "maxProperties": 64, "description": "Questions keyed by an ID you choose: {type: noul|choice|score, instructions, criteria?}."},
                    "model": model,
                    "request_id": request_id,
                }, "required": ["state", "questions"], "additionalProperties": false});
                tool(
                    "decide",
                    "Answer typed questions about a state",
                    "Send Noul, Choice, and Score questions about one state to POST /v1/systemone, and return the typed answers and their probabilities unchanged.",
                    schema,
                )
            },
        ]
    }

    /// One `tools/call`: the tool's name and bounded arguments, then the
    /// shared caller. `credential` is the caller-presented bearer key the
    /// HTTP transport forwards; `None` leaves resolution to operator
    /// configuration.
    fn call_tool(
        options: &Options,
        credential: Option<&str>,
        id: &Value,
        params: Option<&Value>,
    ) -> Option<Value> {
        let Some(name) = params
            .and_then(|params| params.get("name"))
            .and_then(Value::as_str)
        else {
            return Some(error(
                id.clone(),
                INVALID_PARAMS,
                "`tools/call` params must include a tool `name`",
            ));
        };
        let arguments = params
            .and_then(|params| params.get("arguments"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let Some(arguments) = arguments.as_object() else {
            return Some(error(
                id.clone(),
                INVALID_PARAMS,
                "tool `arguments` must be an object",
            ));
        };
        if super::docs::handles(name) {
            let reply = match super::docs::call(name, Value::Object(arguments.clone())) {
                Ok(document) => tool_result(&document),
                Err(problem) => tool_error(
                    problem.message.to_string(),
                    json!({"error":{"code":problem.code,"message":problem.message}}),
                ),
            };
            return Some(result(id, reply));
        }
        let reply = match name {
            "list_models" => {
                if !arguments.is_empty() {
                    return Some(error(
                        id.clone(),
                        INVALID_PARAMS,
                        "`list_models` takes no arguments",
                    ));
                }
                run(options, credential, |transport, opts| {
                    transport.get_models(&opts)
                })
            }
            "classify_texts"
            | "classify_multi_label"
            | "classify_dimensions"
            | "count_labels"
            | "review_uncertain" => classify_call(options, credential, name, arguments),
            "decide" => decide_call(options, credential, arguments),
            "classify" => {
                let mut request_id = None;
                for (key, value) in arguments {
                    match key.as_str() {
                        "request" | "request_id" => {}
                        _ => {
                            return Some(error(
                                id.clone(),
                                INVALID_PARAMS,
                                &format!(
                                    "unknown argument `{key}`; the service address and API key come from the server's configuration, not from tool arguments"
                                ),
                            ));
                        }
                    }
                    if key == "request_id" {
                        let Some(text) = value.as_str() else {
                            return Some(error(
                                id.clone(),
                                INVALID_PARAMS,
                                "`request_id` must be a string",
                            ));
                        };
                        if text.is_empty()
                            || text.chars().count() > MAX_REQUEST_ID_CHARS
                            || reqwest::header::HeaderValue::from_str(text).is_err()
                        {
                            return Some(error(
                                id.clone(),
                                INVALID_PARAMS,
                                "`request_id` must be 1 to 512 printable ASCII characters",
                            ));
                        }
                        request_id = Some(text.to_string());
                    }
                }
                let Some(request) = arguments.get("request") else {
                    return Some(error(
                        id.clone(),
                        INVALID_PARAMS,
                        "`classify` requires a `request` argument",
                    ));
                };
                if !request.is_object() {
                    return Some(error(
                        id.clone(),
                        INVALID_PARAMS,
                        "`request` must be an openagents.classify.v1 request object",
                    ));
                }
                let envelope = serde_json::to_vec(request).unwrap_or_default();
                if envelope.len() as u64 > MAX_ENVELOPE_BYTES {
                    return Some(error(
                        id.clone(),
                        INVALID_PARAMS,
                        "the request is larger than the size limit",
                    ));
                }
                if request.get("v").and_then(Value::as_str) != Some(CLASSIFY_SCHEMA) {
                    return Some(error(
                        id.clone(),
                        INVALID_PARAMS,
                        &format!("`request.v` must be `{CLASSIFY_SCHEMA}`"),
                    ));
                }
                let opts_extra = request_id;
                run_with(options, credential, opts_extra, |transport, opts| {
                    transport.post_classify(&envelope, &opts)
                })
            }
            _ => {
                return Some(error(
                    id.clone(),
                    INVALID_PARAMS,
                    &format!("unknown tool `{name}`"),
                ));
            }
        };
        Some(result(id, reply))
    }

    /// The most characters a tool's concise text rendering carries —
    /// beyond it the text ends with a truncation marker; the structured
    /// content always holds the full document.
    const MAX_TOOL_TEXT_CHARS: usize = 4_000;

    /// The most uncertain items `review_uncertain` lists.
    const MAX_UNCERTAIN_SHOWN: u64 = 100;

    /// The bounds `review_uncertain` puts on the facade's review phase
    /// when a caller names a reviewer door.
    const REVIEW_MAX_ITEMS: u64 = 200;
    /// Total secondary dispatches the review phase may spend.
    const REVIEW_MAX_ATTEMPTS: u64 = 400;
    /// The review phase's wall-clock bound.
    const REVIEW_LATENCY_MS: u64 = 30_000;

    /// Reject arguments a tool does not declare — silently ignoring a
    /// field the caller believes took effect is worse than refusing.
    fn check_args(arguments: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
        for (key, value) in arguments {
            if !allowed.contains(&key.as_str()) {
                return Err(format!("unknown argument `{key}`"));
            }
            if value.is_null() {
                return Err(format!(
                    "`{key}` is null; leave out optional arguments instead"
                ));
            }
        }
        Ok(())
    }

    /// One optional string argument.
    fn string_arg<'a>(
        arguments: &'a Map<String, Value>,
        key: &str,
    ) -> Result<Option<&'a str>, String> {
        match arguments.get(key) {
            None => Ok(None),
            Some(value) => value
                .as_str()
                .map(Some)
                .ok_or_else(|| format!("`{key}` must be a string")),
        }
    }

    /// One optional probability argument — finite and from 0 to 1.
    fn probability_arg(arguments: &Map<String, Value>, key: &str) -> Result<Option<f64>, String> {
        match arguments.get(key).and_then(Value::as_f64) {
            Some(value) if (0.0..=1.0).contains(&value) => Ok(Some(value)),
            Some(_) => Err(format!("`{key}` must be a probability between 0 and 1")),
            None if arguments.contains_key(key) => {
                Err(format!("`{key}` must be a number between 0 and 1"))
            }
            None => Ok(None),
        }
    }

    /// One optional count argument.
    fn count_arg(arguments: &Map<String, Value>, key: &str) -> Result<Option<u64>, String> {
        match arguments.get(key).and_then(Value::as_u64) {
            Some(value) if value >= 1 => Ok(Some(value)),
            Some(_) => Err(format!("`{key}` must be at least 1")),
            None if arguments.contains_key(key) => {
                Err(format!("`{key}` must be a positive integer"))
            }
            None => Ok(None),
        }
    }

    /// The shared `request_id` check — the same rule `classify` applies.
    fn request_id_arg(arguments: &Map<String, Value>) -> Result<Option<String>, String> {
        let Some(text) = string_arg(arguments, "request_id")? else {
            return Ok(None);
        };
        if text.is_empty()
            || text.chars().count() > MAX_REQUEST_ID_CHARS
            || reqwest::header::HeaderValue::from_str(text).is_err()
        {
            return Err("`request_id` must be 1 to 512 printable ASCII characters".to_string());
        }
        Ok(Some(text.to_string()))
    }

    /// A label entry from a string id or an `{id, description}` object.
    fn label_of(value: &Value) -> Result<Value, String> {
        match value {
            Value::String(id) if !id.is_empty() && id.chars().count() <= 256 => {
                Ok(json!({"id": id}))
            }
            Value::Object(object) => {
                for key in object.keys() {
                    if key != "id" && key != "description" {
                        return Err(format!(
                            "a label can have only `id` and `description`, not `{key}`"
                        ));
                    }
                }
                let id = object
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "a label's `id` must be a string".to_string())?;
                if id.is_empty() || id.chars().count() > 256 {
                    return Err("a label's `id` is empty or too long".to_string());
                }
                match object.get("description") {
                    None | Some(Value::Null) => Ok(json!({"id": id})),
                    Some(Value::String(text)) => Ok(json!({"id": id, "description": text})),
                    _ => Err("a label's `description` must be a string".to_string()),
                }
            }
            _ => Err("a label must be an ID string or an {id, description} object".to_string()),
        }
    }

    /// The label set argument: 1–100 unique labels.
    fn labels_arg(arguments: &Map<String, Value>) -> Result<Vec<Value>, String> {
        let values = arguments
            .get("labels")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                "`labels` must be an array of label ids or {id, description} objects".to_string()
            })?;
        if values.is_empty() || values.len() > 100 {
            return Err("`labels` must hold 1 to 100 labels".to_string());
        }
        let mut labels = Vec::with_capacity(values.len());
        let mut seen = std::collections::HashSet::new();
        for value in values {
            let label = label_of(value)?;
            if !seen.insert(label["id"].as_str().unwrap_or_default().to_string()) {
                return Err(format!("label id `{}` repeats", label["id"]));
            }
            labels.push(label);
        }
        Ok(labels)
    }

    /// `texts` as envelope inputs: strings become `{id, text}` in order.
    fn texts_arg(arguments: &Map<String, Value>) -> Result<Vec<Value>, String> {
        let values = arguments
            .get("texts")
            .and_then(Value::as_array)
            .ok_or_else(|| "`texts` must be an array of input texts".to_string())?;
        if values.is_empty() || values.len() > 1000 {
            return Err("`texts` must hold 1 to 1000 inputs".to_string());
        }
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                value
                    .as_str()
                    .filter(|text| !text.is_empty())
                    .map(|text| json!({"id": format!("i{index}"), "text": text}))
                    .ok_or_else(|| format!("text {index} must be a nonempty string"))
            })
            .collect()
    }

    /// `inputs` as envelope inputs: a string is one text, an object is
    /// `{id, text|record}`; generated ids run `i0` up in order.
    fn inputs_arg(arguments: &Map<String, Value>) -> Result<Vec<Value>, String> {
        let values = arguments
            .get("inputs")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                "`inputs` must be an array of texts or {id, text|record} objects".to_string()
            })?;
        if values.is_empty() || values.len() > 1000 {
            return Err("`inputs` must hold 1 to 1000 inputs".to_string());
        }
        let mut inputs = Vec::with_capacity(values.len());
        let mut seen = std::collections::HashSet::new();
        for (index, value) in values.iter().enumerate() {
            let input = match value {
                Value::String(text) if !text.is_empty() => {
                    json!({"id": format!("i{index}"), "text": text})
                }
                Value::Object(object) => {
                    for key in object.keys() {
                        if key != "id" && key != "text" && key != "record" {
                            return Err(format!(
                                "an input can have only `id`, `text`, and `record`, not `{key}`"
                            ));
                        }
                    }
                    let id = object
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let id = if id.is_empty() {
                        format!("i{index}")
                    } else {
                        id
                    };
                    let mut input = json!({"id": id});
                    match (object.get("text"), object.get("record")) {
                        (Some(Value::String(text)), None) => input["text"] = json!(text),
                        (None, Some(record @ Value::Object(_))) => input["record"] = record.clone(),
                        _ => {
                            return Err(format!(
                                "input {index} needs exactly one of `text` or `record`"
                            ));
                        }
                    }
                    input
                }
                _ => {
                    return Err(format!(
                        "input {index} must be a text, an {{id, text}} object, or an {{id, record}} object"
                    ));
                }
            };
            if !seen.insert(input["id"].as_str().unwrap_or_default().to_string()) {
                return Err(format!("input id `{}` repeats", input["id"]));
            }
            inputs.push(input);
        }
        Ok(inputs)
    }

    /// The selection rule for one mode, as the tool's arguments declare
    /// it. The contract's defaults are the honest ones: first-declared
    /// ties, a null no-match, an empty multi-label selection, descending
    /// score order — every cut the caller did not declare stays absent.
    fn select_for(mode: &str, arguments: &Map<String, Value>) -> Result<(String, Value), String> {
        let uncertain = probability_arg(arguments, "uncertain_below")?;
        let mut rule = serde_json::Map::new();
        match mode {
            "single-label" => {
                rule.insert("ties".into(), json!("first-declared"));
                rule.insert("no_match".into(), json!({"kind": "null"}));
                if let Some(cut) = probability_arg(arguments, "min_probability")? {
                    rule.insert("min_probability".into(), json!(cut));
                }
            }
            "multi-label" => {
                let threshold = probability_arg(arguments, "threshold")?.ok_or_else(|| {
                    "`threshold` is required for multi-label classification; there is no default, so choose a value that fits your data".to_string()
                })?;
                rule.insert("threshold".into(), json!(threshold));
                rule.insert("ties".into(), json!("include-all"));
                rule.insert("no_match".into(), json!("empty"));
                if let Some(cap) = count_arg(arguments, "top_n")? {
                    rule.insert("top_n".into(), json!(cap));
                }
            }
            "binary" => {
                let threshold = probability_arg(arguments, "threshold")?.ok_or_else(|| {
                    "`threshold` is required for binary classification; there is no default, so choose a value that fits your data".to_string()
                })?;
                rule.insert("threshold".into(), json!(threshold));
            }
            "score" => {
                rule.insert("order".into(), json!("descending"));
                if let Some(cap) = count_arg(arguments, "top_n")? {
                    rule.insert("top_n".into(), json!(cap));
                }
            }
            _ => {
                return Err(format!(
                    "`{mode}` isn't a supported mode; use single-label, multi-label, binary, or score"
                ));
            }
        }
        if let Some(cut) = uncertain {
            rule.insert("uncertain_below".into(), json!(cut));
        }
        Ok((mode.replace('-', "_"), Value::Object(rule)))
    }

    /// The shared classify envelope: the caller's arguments laid into
    /// the `openagents.classify.v1` shape, policy included.
    fn envelope_of(
        name: &str,
        model: &str,
        arguments: &Map<String, Value>,
        inputs: Vec<Value>,
        mut request: Map<String, Value>,
        select: Map<String, Value>,
        review: Option<Value>,
    ) -> Result<Vec<u8>, String> {
        let mut policy = json!({
            "v": CLASSIFY_POLICY_SCHEMA,
            "name": format!("mcp-{name}"),
            "select": Value::Object(select),
        });
        if let Some(review) = review {
            policy["review"] = review;
        }
        request.insert("v".into(), json!(CLASSIFY_SCHEMA));
        request.insert("model".into(), json!(model));
        request.insert(
            "capacity".into(),
            json!(
                string_arg(arguments, "capacity")?
                    .unwrap_or("shared")
                    .to_string()
            ),
        );
        request.insert("policy".into(), policy);
        if let Some(instructions) = string_arg(arguments, "instructions")? {
            if instructions.len() > 16_384 {
                return Err("`instructions` is longer than the 16384-byte limit".to_string());
            }
            request.insert("instructions".into(), json!(instructions));
        }
        request.insert("inputs".into(), json!(inputs));
        let envelope = serde_json::to_vec(&Value::Object(request))
            .map_err(|error| format!("oak couldn't encode the request: {error}"))?;
        if envelope.len() as u64 > MAX_ENVELOPE_BYTES {
            return Err("the request is larger than the size limit".to_string());
        }
        Ok(envelope)
    }

    /// The `model` a call resolves: the argument first, then the
    /// configured default.
    fn model_of(arguments: &Map<String, Value>, settings: &Settings) -> Result<String, String> {
        string_arg(arguments, "model")?
            .map(str::to_string)
            .or_else(|| settings.model.clone())
            .ok_or_else(|| {
                "name a `model`, or set OPENAGENTS_MODEL or `model` in the config file".to_string()
            })
    }

    /// Build the envelope for the tools that classify on one label set,
    /// then post it and shape the report per tool.
    fn classify_call(
        options: &Options,
        credential: Option<&str>,
        name: &str,
        arguments: &Map<String, Value>,
    ) -> Value {
        let invalid = |message: String| {
            tool_error(
                format!("oak-mcp: {message}"),
                json!({"error": {"code": "invalid_arguments", "message": message}}),
            )
        };
        let (settings, transport) = match connect(options, credential) {
            Ok(pair) => pair,
            Err(message) => {
                return tool_error(
                    format!("oak-mcp: {message}"),
                    json!({"error": {"code": "unavailable", "message": message}}),
                );
            }
        };
        let model = match model_of(arguments, &settings) {
            Ok(model) => model,
            Err(message) => return invalid(message),
        };
        let built = match name {
            "classify_texts" => {
                check_args(
                    arguments,
                    &[
                        "texts",
                        "labels",
                        "min_probability",
                        "uncertain_below",
                        "model",
                        "capacity",
                        "instructions",
                        "request_id",
                    ],
                )
                .and_then(|()| {
                    let labels = labels_arg(arguments)?;
                    if labels.len() < 2 {
                        return Err("`labels` needs at least two labels for a categorical choice".into());
                    }
                    let inputs = texts_arg(arguments)?;
                    let (key, rule) = select_for("single-label", arguments)?;
                    let mut select = serde_json::Map::new();
                    select.insert(key, rule);
                    let mut request = serde_json::Map::new();
                    request.insert("mode".into(), json!("single-label"));
                    request.insert("labels".into(), json!(labels));
                    envelope_of(name, &model, arguments, inputs, request, select, None)
                })
            }
            "classify_multi_label" => {
                check_args(
                    arguments,
                    &[
                        "texts",
                        "labels",
                        "threshold",
                        "top_n",
                        "uncertain_below",
                        "model",
                        "capacity",
                        "instructions",
                        "request_id",
                    ],
                )
                .and_then(|()| {
                    let labels = labels_arg(arguments)?;
                    let inputs = texts_arg(arguments)?;
                    if inputs.len() * labels.len() > 1000 {
                        return Err(format!(
                            "{} inputs times {} labels is {} judgments; one call allows at most 1000",
                            inputs.len(),
                            labels.len(),
                            inputs.len() * labels.len()
                        ));
                    }
                    let (key, rule) = select_for("multi-label", arguments)?;
                    let mut select = serde_json::Map::new();
                    select.insert(key, rule);
                    let mut request = serde_json::Map::new();
                    request.insert("mode".into(), json!("multi-label"));
                    request.insert("labels".into(), json!(labels));
                    envelope_of(name, &model, arguments, inputs, request, select, None)
                })
            }
            "classify_dimensions" => {
                check_args(
                    arguments,
                    &[
                        "inputs",
                        "dimensions",
                        "threshold",
                        "top_n",
                        "uncertain_below",
                        "model",
                        "capacity",
                        "instructions",
                        "request_id",
                    ],
                )
                .and_then(|()| {
                    let inputs = inputs_arg(arguments)?;
                    let dims = arguments
                        .get("dimensions")
                        .and_then(Value::as_array)
                        .ok_or_else(|| "`dimensions` must be an array".to_string())?;
                    if dims.is_empty() || dims.len() > 20 {
                        return Err("`dimensions` must hold 1 to 20 dimensions".to_string());
                    }
                    let mut select = serde_json::Map::new();
                    let mut out = Vec::with_capacity(dims.len());
                    let mut seen = std::collections::HashSet::new();
                    for (index, value) in dims.iter().enumerate() {
                        let object = value.as_object().ok_or_else(|| {
                            format!("dimension {index} must be an object")
                        })?;
                        for key in object.keys() {
                            if !["id", "mode", "labels", "levels", "instructions"].contains(&key.as_str()) {
                                return Err(format!("a dimension can have only `id`, `mode`, `labels`, `levels`, and `instructions`, not `{key}`"));
                            }
                        }
                        let mode = object
                            .get("mode")
                            .and_then(Value::as_str)
                            .ok_or_else(|| format!("dimension {index} needs a `mode`"))?;
                        let id_text = object
                            .get("id")
                            .and_then(Value::as_str)
                            .filter(|id| !id.is_empty())
                            .unwrap_or("d")
                            .to_string();
                        if !seen.insert(format!("{id_text}:{mode}")) {
                            return Err(format!("dimension `{id_text}` repeats"));
                        }
                        let mut dim = serde_json::Map::new();
                        dim.insert("id".into(), json!(id_text));
                        dim.insert("mode".into(), json!(mode));
                        let mut dim_args = object.clone();
                        for key in ["threshold", "top_n", "uncertain_below", "min_probability"] {
                            if let Some(v) = arguments.get(key) {
                                dim_args.insert(key.to_string(), v.clone());
                            }
                        }
                        match mode {
                            "score" => {
                                let levels = object
                                    .get("levels")
                                    .and_then(Value::as_array)
                                    .filter(|levels| (2..=10).contains(&levels.len()))
                                    .ok_or_else(|| "a score dimension needs 2 to 10 `levels`".to_string())?;
                                dim.insert("levels".into(), json!(levels));
                            }
                            "binary" => {
                                let labels = labels_arg(object)?;
                                if labels.len() != 1 {
                                    return Err("a binary dimension takes exactly one label".into());
                                }
                                dim.insert("labels".into(), json!(labels));
                            }
                            "single-label" | "multi-label" => {
                                let labels = labels_arg(object)?;
                                if mode == "single-label" && labels.len() < 2 {
                                    return Err("a single-label dimension needs at least two labels".into());
                                }
                                dim.insert("labels".into(), json!(labels));
                            }
                            other => return Err(format!("`{other}` isn't a supported dimension mode; use single-label, multi-label, binary, or score")),
                        }
                        if let Some(instructions) = object
                            .get("instructions")
                            .and_then(Value::as_str)
                        {
                            if instructions.len() > 16_384 {
                                return Err("a dimension's `instructions` is longer than the size limit".into());
                            }
                            dim.insert("instructions".into(), json!(instructions));
                        }
                        let (key, rule) = select_for(mode, &dim_args)?;
                        if select.insert(key.clone(), rule).is_some() {
                            return Err(format!("more than one dimension uses mode `{mode}`; a request can have only one dimension per mode because each mode has one selection rule"));
                        }
                        out.push(Value::Object(dim));
                    }
                    if inputs.len() * out.len() > 1000 {
                        return Err(format!(
                            "{} inputs times {} dimensions is {} judgments; one call allows at most 1000",
                            inputs.len(),
                            out.len(),
                            inputs.len() * out.len()
                        ));
                    }
                    let mut request = serde_json::Map::new();
                    request.insert("dimensions".into(), json!(out));
                    envelope_of(name, &model, arguments, inputs, request, select, None)
                })
            }
            "count_labels" => check_args(
                arguments,
                &[
                    "texts",
                    "labels",
                    "min_probability",
                    "model",
                    "capacity",
                    "instructions",
                    "request_id",
                ],
            )
            .and_then(|()| {
                let labels = labels_arg(arguments)?;
                if labels.len() < 2 {
                    return Err("`labels` needs at least two labels for a categorical choice".into());
                }
                let inputs = texts_arg(arguments)?;
                let (key, rule) = select_for("single-label", arguments)?;
                let mut select = serde_json::Map::new();
                select.insert(key, rule);
                let mut request = serde_json::Map::new();
                request.insert("mode".into(), json!("single-label"));
                request.insert("labels".into(), json!(labels));
                envelope_of(name, &model, arguments, inputs, request, select, None)
            }),
            "review_uncertain" => check_args(
                arguments,
                &[
                    "texts",
                    "labels",
                    "uncertain_below",
                    "reviewer",
                    "show",
                    "model",
                    "capacity",
                    "instructions",
                    "request_id",
                ],
            )
            .and_then(|()| {
                let labels = labels_arg(arguments)?;
                if labels.len() < 2 {
                    return Err("`labels` needs at least two labels for a categorical choice".into());
                }
                probability_arg(arguments, "uncertain_below")?
                    .ok_or_else(|| "`uncertain_below` is required; it sets which results count as uncertain".to_string())?;
                if let Some(show) = count_arg(arguments, "show")?
                    && show > MAX_UNCERTAIN_SHOWN
                {
                    return Err(format!("`show` is capped at {MAX_UNCERTAIN_SHOWN}"));
                }
                let inputs = texts_arg(arguments)?;
                let (key, rule) = select_for("single-label", arguments)?;
                let mut select = serde_json::Map::new();
                select.insert(key, rule);
                let review = match string_arg(arguments, "reviewer")? {
                    Some(reviewer) if reviewer.trim().is_empty() => {
                        return Err("`reviewer` must name a model your API key can use".into());
                    }
                    Some(reviewer) => Some(json!({
                        "v": CLASSIFY_REVIEW_SCHEMA,
                        "reviewer": reviewer,
                        "trigger": "uncertain",
                        "on_failure": "keep-original",
                        "max_items": REVIEW_MAX_ITEMS,
                        "max_attempts": REVIEW_MAX_ATTEMPTS,
                        "latency_ms": REVIEW_LATENCY_MS,
                    })),
                    None => None,
                };
                let mut request = serde_json::Map::new();
                request.insert("mode".into(), json!("single-label"));
                request.insert("labels".into(), json!(labels));
                envelope_of(name, &model, arguments, inputs, request, select, review)
            }),
            _ => unreachable!("the dispatch names only classify tools"),
        };
        let envelope = match built {
            Ok(envelope) => envelope,
            Err(message) => return invalid(message),
        };
        let request_id = match request_id_arg(arguments) {
            Ok(request_id) => request_id,
            Err(message) => return invalid(message),
        };
        let reply = transport
            .post_classify(
                &envelope,
                &CallOpts {
                    timeout: options.timeout,
                    retries: options.retries,
                    request_id,
                },
            )
            .map_err(|error| error.message);
        match reply {
            Err(message) => tool_error(
                format!("oak-mcp: {message}"),
                json!({"error": {"code": "unavailable", "message": message}}),
            ),
            Ok(Reply::Document { body, .. }) => {
                let shaped = match name {
                    "count_labels" => counts_of(&body),
                    "review_uncertain" => uncertain_of(arguments, &body),
                    _ => body,
                };
                tool_result(&shaped)
            }
            Ok(reply) => reply_value(reply, report_text),
        }
    }

    /// The native decision tool: the caller's state and typed questions
    /// straight to `POST /v1/systemone`.
    fn decide_call(
        options: &Options,
        credential: Option<&str>,
        arguments: &Map<String, Value>,
    ) -> Value {
        let envelope = (|| -> Result<Vec<u8>, String> {
            check_args(arguments, &["state", "questions", "model", "request_id"])?;
            if !arguments.contains_key("state") {
                return Err("`decide` requires a `state`".into());
            }
            let questions = arguments
                .get("questions")
                .and_then(Value::as_object)
                .ok_or_else(|| "`questions` must be an object of typed questions".to_string())?;
            if questions.is_empty() || questions.len() > 64 {
                return Err("`questions` must hold 1 to 64 questions".to_string());
            }
            for (qid, question) in questions {
                let kind = question
                    .get("type")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("question `{qid}` needs a `type`"))?;
                if !matches!(kind, "noul" | "choice" | "score") {
                    return Err(format!(
                        "question `{qid}` has type `{kind}`; use noul, choice, or score"
                    ));
                }
                if !question.get("instructions").is_some_and(|v| v.is_string()) {
                    return Err(format!("question `{qid}` needs string `instructions`"));
                }
            }
            let mut request = json!({
                "state": arguments["state"],
                "questions": arguments["questions"],
            });
            if let Some(model) = string_arg(arguments, "model")? {
                request["model"] = json!(model);
            }
            let bytes = serde_json::to_vec(&request)
                .map_err(|error| format!("oak couldn't encode the request: {error}"))?;
            if bytes.len() as u64 > MAX_ENVELOPE_BYTES {
                return Err("the request is larger than the size limit".into());
            }
            Ok(bytes)
        })();
        let envelope = match envelope {
            Ok(envelope) => envelope,
            Err(message) => {
                return tool_error(
                    format!("oak-mcp: {message}"),
                    json!({"error": {"code": "invalid_arguments", "message": message}}),
                );
            }
        };
        let request_id = match request_id_arg(arguments) {
            Ok(request_id) => request_id,
            Err(message) => {
                return tool_error(
                    format!("oak-mcp: {message}"),
                    json!({"error": {"code": "invalid_arguments", "message": message}}),
                );
            }
        };
        let call =
            move |transport: &Transport, opts: CallOpts| transport.post_systemone(&envelope, &opts);
        match call_service(options, credential, request_id, call) {
            Err(message) => tool_error(
                format!("oak-mcp: {message}"),
                json!({"error": {"code": "unavailable", "message": message}}),
            ),
            Ok(reply) => reply_value(reply, decide_text),
        }
    }

    /// Resolve the credential the caller presented or the operator
    /// configured, then build the transport from the resolved settings.
    fn connect(
        options: &Options,
        credential: Option<&str>,
    ) -> Result<(Settings, Transport), String> {
        let settings = Settings::resolve_key(
            options.url.clone(),
            None,
            options.workspace.clone(),
            options.config.clone(),
            credential.map(str::to_string),
        )?;
        let transport = settings.transport()?;
        Ok((settings, transport))
    }

    /// The shared service call: resolve the credential the caller
    /// presented or the operator configured, then run the transport.
    fn call_service(
        options: &Options,
        credential: Option<&str>,
        request_id: Option<String>,
        call: impl FnOnce(&Transport, CallOpts) -> Result<Reply, super::CallError>,
    ) -> Result<Reply, String> {
        let (_settings, transport) = connect(options, credential)?;
        call(
            &transport,
            CallOpts {
                timeout: options.timeout,
                retries: options.retries,
                request_id,
            },
        )
        .map_err(|error| error.message)
    }

    /// A reply as a tool result — a document as structured content plus
    /// the concise text `render` gives it, a refusal as a typed error.
    fn reply_value(reply: Reply, render: impl FnOnce(&Value) -> String) -> Value {
        match reply {
            Reply::Document { body, .. } => Value::Object(serde_json::Map::from_iter([
                (
                    "content".into(),
                    json!([{"type": "text", "text": render(&body)}]),
                ),
                ("structuredContent".into(), body),
                ("isError".into(), json!(false)),
            ])),
            Reply::Refused {
                status,
                code,
                message,
                request_id,
                ..
            } => {
                let mut detail = json!({
                    "error": {"code": code, "message": message},
                    "status": status,
                });
                if let Some(request_id) = request_id {
                    detail["request_id"] = json!(request_id);
                }
                tool_error(format!("oak-mcp: {status} {code}: {message}"), detail)
            }
        }
    }

    /// One line per input for a classify report: the selection, or the
    /// outcome when no selection reported.
    fn report_text(body: &Value) -> String {
        let mut text = format!(
            "outcome: {}\n",
            body["outcome"].as_str().unwrap_or("unknown")
        );
        for item in body["results"].as_array().into_iter().flatten() {
            let input = item["input"].as_str().unwrap_or("?");
            let mut parts = Vec::new();
            for unit in item["units"].as_array().into_iter().flatten() {
                let name = unit["dimension"].as_str().unwrap_or("unit");
                let selected = match unit["outcome"].as_str() {
                    Some("answered") => match &unit["selected"] {
                        Value::Null => "no-match".to_string(),
                        value => value.to_string(),
                    },
                    other => other.unwrap_or("unavailable").to_string(),
                };
                let flag = if unit["uncertain"].as_bool() == Some(true) {
                    " (uncertain)"
                } else {
                    ""
                };
                parts.push(format!("{name}: {selected}{flag}"));
            }
            text.push_str(&format!("{input} → {}\n", parts.join(", ")));
            if text.len() > MAX_TOOL_TEXT_CHARS {
                text.truncate(MAX_TOOL_TEXT_CHARS);
                text.push_str("… truncated");
                return text;
            }
        }
        text
    }

    /// One line per question for a native decision answer.
    fn decide_text(body: &Value) -> String {
        let mut text = String::new();
        for (qid, answer) in body["answers"].as_object().into_iter().flatten() {
            let line = match answer["type"].as_str() {
                Some("noul") => format!("{qid}: noul {}", answer["noul"]),
                Some("choice") => format!(
                    "{qid}: choice {} ({})",
                    answer["choice"], answer["confidence"]
                ),
                Some("score") => format!(
                    "{qid}: score {} ({})",
                    answer["score"], answer["confidence"]
                ),
                _ => format!("{qid}: {answer}"),
            };
            text.push_str(&line);
            text.push('\n');
            if text.len() > MAX_TOOL_TEXT_CHARS {
                text.truncate(MAX_TOOL_TEXT_CHARS);
                text.push_str("… truncated");
                return text;
            }
        }
        text
    }

    /// `count_labels`' reduced document: per-label counts and the
    /// failure tallies — never a per-input selection.
    fn counts_of(body: &Value) -> Value {
        let mut counts = serde_json::Map::new();
        let mut no_match = 0_u64;
        let mut unanswered = 0_u64;
        let mut inputs = 0_u64;
        for item in body["results"].as_array().into_iter().flatten() {
            inputs += 1;
            let mut counted = false;
            for unit in item["units"].as_array().into_iter().flatten() {
                if unit["outcome"].as_str() != Some("answered") {
                    continue;
                }
                counted = true;
                match &unit["selected"] {
                    Value::String(label) => {
                        let entry = counts.entry(label.clone()).or_insert_with(|| json!(0_u64));
                        *entry = json!(entry.as_u64().unwrap_or(0) + 1);
                    }
                    _ => no_match += 1,
                }
            }
            if !counted {
                unanswered += 1;
            }
        }
        json!({
            "v": "openagents.mcp-counts.v1",
            "model": body["model"],
            "outcome": body["outcome"],
            "inputs": inputs,
            "counts": counts,
            "no_match": no_match,
            "unanswered": unanswered,
            "usage": body["usage"],
        })
    }

    /// `review_uncertain`'s reduced document: the flagged units,
    /// bounded — the corpus's other inputs stay out of the context.
    fn uncertain_of(arguments: &Map<String, Value>, body: &Value) -> Value {
        let show = arguments
            .get("show")
            .and_then(Value::as_u64)
            .unwrap_or(25)
            .min(MAX_UNCERTAIN_SHOWN) as usize;
        let mut items = Vec::new();
        let mut flagged = 0_u64;
        let mut inputs = 0_u64;
        for item in body["results"].as_array().into_iter().flatten() {
            inputs += 1;
            for unit in item["units"].as_array().into_iter().flatten() {
                if unit["uncertain"].as_bool() != Some(true) {
                    continue;
                }
                flagged += 1;
                if items.len() >= show {
                    continue;
                }
                let top = unit["raw"]["probabilities"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter_map(|(_, p)| p.as_f64())
                    .fold(f64::NEG_INFINITY, f64::max);
                let top = if top.is_finite() {
                    json!(top)
                } else {
                    Value::Null
                };
                items.push(json!({
                    "input": item["input"],
                    "selected": unit["selected"],
                    "top_probability": top,
                    "review": unit.get("review").and_then(|r| r.get("selected")).cloned().unwrap_or(Value::Null),
                }));
            }
        }
        json!({
            "v": "openagents.mcp-uncertain.v1",
            "model": body["model"],
            "outcome": body["outcome"],
            "inputs": inputs,
            "uncertain": items,
            "uncertain_count": flagged,
            "uncertain_truncated": flagged as usize > items.len(),
            "usage": body["usage"],
        })
    }

    /// Resolve the operator configuration and run one call.
    fn run(
        options: &Options,
        credential: Option<&str>,
        call: impl FnOnce(&Transport, CallOpts) -> Result<Reply, super::CallError>,
    ) -> Value {
        run_with(options, credential, None, call)
    }

    /// Resolve the operator configuration and run one call with an
    /// idempotency key.
    fn run_with(
        options: &Options,
        credential: Option<&str>,
        request_id: Option<String>,
        call: impl FnOnce(&Transport, CallOpts) -> Result<Reply, super::CallError>,
    ) -> Value {
        match call_service(options, credential, request_id, call) {
            Err(message) => tool_error(
                format!("oak-mcp: {message}"),
                json!({"error": {"code": "unavailable", "message": message}}),
            ),
            Ok(reply) => reply_value(reply, |body| body.to_string()),
        }
    }
}
