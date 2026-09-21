//! `oak` — the caller implementation the `oak` CLI and the `oak-mcp`
//! stdio server share.
//!
//! The shared half holds what both surfaces need identically: where the
//! endpoint, credential, and workspace come from; how a config file is
//! trusted; how a bounded read works; and how one HTTP call runs — its
//! timeout, its retry loop honoring `Retry-After`, and its
//! `(Idempotency-Key, X-Attempt)` settlement pair. Read
//! `docs/decision-models/caller.md` and
//! `docs/decision-models/classification-callers.md`.

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
        let file = load_config(config)?;
        let base_url = url
            .or_else(|| env("OPENAGENTS_BASE_URL"))
            .or(file.base_url)
            .ok_or_else(|| {
                "no service root: pass --url, set OPENAGENTS_BASE_URL, or name \
                 `base_url` in the config file"
                    .to_string()
            })?;
        let api_key = env("OPENAGENTS_API_KEY").or(file.api_key).ok_or_else(|| {
            "no credential: set OPENAGENTS_API_KEY or name `api_key` in the config \
             file — a key never goes on the command line"
                .to_string()
        })?;
        let model = model.or_else(|| env("OPENAGENTS_MODEL")).or(file.model);
        let workspace = workspace
            .or_else(|| env("OPENAGENTS_WORKSPACE"))
            .or(file.workspace);
        if let Some(workspace) = &workspace
            && HeaderValue::from_str(workspace).is_err()
        {
            return Err("the workspace id is not a header value".to_string());
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
            let value = HeaderValue::from_str(workspace)
                .map_err(|_| "the workspace id is not a header value".to_string())?;
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
            .map_err(|error| format!("cannot stat {}: {error}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "{} is readable by group or others; run `chmod 600 {}`",
                path.display(),
                path.display()
            ));
        }
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("{} does not parse: {error}", path.display()))
}

/// Read a bounded input: a file, a pipe, or a tool argument's bytes. A
/// source past `limit` is refused whole rather than truncated.
pub fn read_bounded(mut reader: impl Read, limit: u64, what: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {what}: {error}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("the {what} exceeds the {}-byte bound", limit));
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
            .map_err(|error| format!("the runtime did not build: {error}"))?;
        let http = reqwest::Client::builder()
            .build()
            .map_err(|error| format!("the HTTP client did not build: {error}"))?;
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

    /// `POST /v1/classify`, sending the envelope bytes as received.
    ///
    /// # Errors
    ///
    /// Returns [`CallError`] when the call never produced a usable reply.
    /// A classify report — whatever its outcome — and a typed refusal are
    /// both a [`Reply`].
    pub fn post_classify(&self, envelope: &[u8], opts: &CallOpts) -> Result<Reply, CallError> {
        self.exchange(reqwest::Method::POST, CLASSIFY_PATH, Some(envelope), opts)
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
                    if attempt <= opts.retries {
                        tokio::time::sleep(delay.min(MAX_RETRY_AFTER)).await;
                        delay = (delay * 2).min(Duration::from_secs(5));
                        continue;
                    }
                    return Err(CallError {
                        code: "unavailable",
                        message: format!("the call did not reach the service: {error}"),
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
                    message: format!("the response exceeds the {MAX_RESPONSE_BYTES}-byte bound"),
                });
            }
            if let Some(error) = body_error {
                if attempt <= opts.retries {
                    tokio::time::sleep(delay.min(MAX_RETRY_AFTER)).await;
                    delay = (delay * 2).min(Duration::from_secs(5));
                    continue;
                }
                return Err(CallError {
                    code: "unavailable",
                    message: format!("the response body did not read: {error}"),
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
            .unwrap_or_else(|| format!("the service answered {status}"));
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
            format!("the service answered {status} with no body")
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
        Reply::Document { status, .. } => (*status, ""),
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

    use serde_json::{Value, json};

    use super::{
        CLASSIFY_SCHEMA, CallOpts, MAX_ENVELOPE_BYTES, MAX_MCP_MESSAGE_BYTES, MAX_REQUEST_ID_CHARS,
        Reply, Settings, Transport,
    };

    /// The protocol version this build reports when it cannot answer the
    /// client's — the latest it serves.
    pub const PROTOCOL_VERSION: &str = "2025-06-18";

    /// Every protocol version this build serves, latest first.
    pub const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

    /// The `serverInfo.name` the handshake reports.
    pub const SERVER_NAME: &str = "oak-mcp";

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
    #[derive(Default)]
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
    enum Phase {
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
                            "the message exceeds the byte bound",
                        ),
                    ) {
                        return 0;
                    }
                    continue;
                }
                Ok(Line::Read) => {}
                Err(error) => {
                    let _ = writeln!(log, "oak-mcp: cannot read standard input: {error}");
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
            if let Some(response) = handle(&mut phase, options, &message)
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
    /// notification, or a response addressed to us.
    fn handle(phase: &mut Phase, options: &Options, message: &Value) -> Option<Value> {
        let id = message.get("id").cloned();
        if message.get("method").is_none() {
            // A response to a request this server never sends is
            // ignored; anything else without a method is malformed.
            return match (message.get("result"), message.get("error")) {
                (None, None) => Some(error(
                    id.unwrap_or(Value::Null),
                    INVALID_REQUEST,
                    "the message is not a request, notification, or response",
                )),
                _ => None,
            };
        }
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Some(error(
                id.unwrap_or(Value::Null),
                INVALID_REQUEST,
                "the method is not a string",
            ));
        };
        if message.get("jsonrpc") != Some(&json!("2.0")) {
            return Some(error(
                id.unwrap_or(Value::Null),
                INVALID_REQUEST,
                "the message does not declare `jsonrpc: \"2.0\"`",
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
                call_tool(options, &id, message.get("params"))
            }
            "tools/list" | "tools/call" => Some(error(
                id,
                INVALID_PARAMS,
                "the session is not initialized — send `initialize`, then `notifications/initialized`",
            )),
            _ if method.starts_with("notifications/") => None,
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
                "initialize params must carry `protocolVersion`",
            ));
        };
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
                    "title": "oak — the decision API caller",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "instructions": "Tools call the configured decision service. `list_models` lists the doors the configured credential may name; `classify` posts an openagents.classify.v1 envelope to /v1/classify and returns the report.",
            }),
        ))
    }

    /// The tools this server serves. Neither schema accepts a credential
    /// or an endpoint: those stay in operator configuration.
    fn tool_list() -> Value {
        json!({
            "tools": [
                {
                    "name": "list_models",
                    "title": "List reachable doors",
                    "description": "Return the gateway's GET /v1/models document: the doors the configured credential may name, each with its bound artifact, lane, and classification discovery object.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "additionalProperties": false,
                    },
                },
                {
                    "name": "classify",
                    "title": "Classify inputs through a door",
                    "description": "POST an openagents.classify.v1 envelope to the configured gateway's /v1/classify and return the report verbatim: ordered per-input results, selections, per-unit aggregates, and usage.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "request": {
                                "type": "object",
                                "description": "The openagents.classify.v1 envelope: v, model, capacity, policy, inputs, and labels, levels, or dimensions.",
                            },
                            "request_id": {
                                "type": "string",
                                "description": "An idempotency key for the call; retries reuse it and bump the attempt.",
                                "maxLength": MAX_REQUEST_ID_CHARS,
                            },
                        },
                        "required": ["request"],
                        "additionalProperties": false,
                    },
                },
            ],
        })
    }

    /// One `tools/call`: the tool's name and bounded arguments, then the
    /// shared caller.
    fn call_tool(options: &Options, id: &Value, params: Option<&Value>) -> Option<Value> {
        let Some(name) = params
            .and_then(|params| params.get("name"))
            .and_then(Value::as_str)
        else {
            return Some(error(
                id.clone(),
                INVALID_PARAMS,
                "tools/call params must carry a tool `name`",
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
        let reply = match name {
            "list_models" => {
                if !arguments.is_empty() {
                    return Some(error(
                        id.clone(),
                        INVALID_PARAMS,
                        "`list_models` takes no arguments",
                    ));
                }
                run(options, |transport, opts| transport.get_models(&opts))
            }
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
                                    "unknown argument `{key}` — the endpoint and credential come from operator configuration"
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
                                "`request_id` is empty, overlong, or not a header value",
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
                        "`request` must be the openagents.classify.v1 envelope object",
                    ));
                }
                let envelope = serde_json::to_vec(request).unwrap_or_default();
                if envelope.len() as u64 > MAX_ENVELOPE_BYTES {
                    return Some(error(
                        id.clone(),
                        INVALID_PARAMS,
                        "the envelope exceeds the byte bound",
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
                run_with(options, opts_extra, |transport, opts| {
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

    /// Resolve the operator configuration and run one call.
    fn run(
        options: &Options,
        call: impl FnOnce(&Transport, CallOpts) -> Result<Reply, super::CallError>,
    ) -> Value {
        run_with(options, None, call)
    }

    /// Resolve the operator configuration and run one call with an
    /// idempotency key.
    fn run_with(
        options: &Options,
        request_id: Option<String>,
        call: impl FnOnce(&Transport, CallOpts) -> Result<Reply, super::CallError>,
    ) -> Value {
        let outcome = (|| {
            let settings = Settings::resolve(
                options.url.clone(),
                None,
                options.workspace.clone(),
                options.config.clone(),
            )?;
            let transport = settings.transport()?;
            call(
                &transport,
                CallOpts {
                    timeout: options.timeout,
                    retries: options.retries,
                    request_id,
                },
            )
            .map_err(|error| error.message)
        })();
        match outcome {
            Err(message) => tool_error(
                format!("oak-mcp: {message}"),
                json!({"error": {"code": "unavailable", "message": message}}),
            ),
            Ok(Reply::Document { body, .. }) => tool_result(&body),
            Ok(Reply::Refused {
                status,
                code,
                message,
                request_id,
                ..
            }) => {
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
}
