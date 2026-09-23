//! Capture the first request an executor CLI sends, without inference.
//!
//! A local server records every request body and answers each call with
//! an error, so the CLI stops after its first attempt. The CLI runs with
//! the exact command a dispatch uses ([`crate::delegate::Cli::command`]),
//! pointed at the server, in a scratch home, with a dummy credential and a
//! cleared environment: no real token can reach the server, and nothing is
//! forwarded anywhere. The server never records headers.
//!
//! - Claude Code reads `ANTHROPIC_BASE_URL` and a dummy
//!   `CLAUDE_CODE_OAUTH_TOKEN`.
//! - Codex reads a `capture` model provider whose base URL is the server
//!   and whose key is a dummy variable. With a custom provider Codex can't
//!   fetch its model catalog, so the capture passes the `models` list from
//!   `~/.codex/models_cache.json` as `model_catalog_json`; that file holds
//!   model metadata only, and nothing else from `~/.codex` is read.
//!
//! [`measure`] reads the captured request: every part's size, its cache
//! markers, and whether the variant's text and the protected section
//! arrived.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::delegate::{Agent, Cli, Credential};
use crate::system::{self, Variant};

/// The schema of a capture measurement.
pub const SCHEMA: &str = "openagents.coder-one.prompt-capture.v1";

/// The dummy credential every capture uses.
pub const DUMMY_TOKEN: &str = "sk-ant-oat01-capture-only-dummy";

/// What the server answers every call with.
const ERROR_BODY: &str =
    r#"{"type":"error","error":{"type":"invalid_request_error","message":"capture only"}}"#;

/// One recorded request: its method, path, and body. Headers are never
/// kept.
#[derive(Clone, Debug)]
pub struct Captured {
    pub method: String,
    pub path: String,
    pub body: Vec<u8>,
}

/// A local server that records request bodies and answers with an error.
pub struct Server {
    port: u16,
    requests: Arc<Mutex<Vec<Captured>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    /// Listens on an ephemeral loopback port.
    ///
    /// # Errors
    ///
    /// Returns a message when the port can't be bound.
    pub fn start() -> Result<Self, String> {
        Self::listen(false)
    }

    /// Listens on an ephemeral loopback port and answers each model call
    /// with a short scripted turn instead of an error, so a real CLI runs
    /// whole sessions with no inference: see [`answer`].
    ///
    /// # Errors
    ///
    /// Returns a message when the port can't be bound.
    pub fn answering() -> Result<Self, String> {
        Self::listen(true)
    }

    fn listen(answering: bool) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("cannot listen on loopback: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let port = listener
            .local_addr()
            .map_err(|error| error.to_string())?
            .port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let requests = Arc::clone(&requests);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) if answering => {
                            // An answer can be held back on purpose, so
                            // each connection gets a thread of its own.
                            let requests = Arc::clone(&requests);
                            std::thread::spawn(move || serve(stream, &requests, true));
                        }
                        Ok((stream, _)) => {
                            serve(stream, &requests, false);
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(20)),
                    }
                }
            })
        };
        Ok(Server {
            port,
            requests,
            stop,
            thread: Some(thread),
        })
    }

    /// The server's base URL.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Every request recorded so far, in arrival order.
    #[must_use]
    pub fn requests(&self) -> Vec<Captured> {
        self.requests
            .lock()
            .map(|all| all.clone())
            .unwrap_or_default()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Reads one HTTP/1.1 request, records it, answers it with an error, and
/// closes. The request is recorded before the answer, so a client that has
/// its answer can count on the record.
fn serve(stream: TcpStream, requests: &Mutex<Vec<Captured>>, answering: bool) -> Option<()> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut length = 0usize;
    let mut chunked = false;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            break;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            if name == "content-length" {
                length = value.parse().unwrap_or(0);
            } else if name == "transfer-encoding" && value.eq_ignore_ascii_case("chunked") {
                chunked = true;
            }
        }
    }
    let mut body = Vec::new();
    if chunked {
        loop {
            let mut size = String::new();
            reader.read_line(&mut size).ok()?;
            let size = usize::from_str_radix(size.trim().split(';').next()?, 16).ok()?;
            if size == 0 {
                let mut end = String::new();
                let _ = reader.read_line(&mut end);
                break;
            }
            let mut chunk = vec![0; size];
            reader.read_exact(&mut chunk).ok()?;
            body.extend(chunk);
            let mut crlf = [0u8; 2];
            reader.read_exact(&mut crlf).ok()?;
        }
    } else if length > 0 {
        body = vec![0; length];
        reader.read_exact(&mut body).ok()?;
    }
    let reply = answering.then(|| answer(&path, &body));
    if let Ok(mut all) = requests.lock() {
        all.push(Captured { method, path, body });
    }
    let mut stream = stream;
    if let Some((delay, content_type, reply)) = reply {
        std::thread::sleep(delay);
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
            reply.len()
        );
        let _ = stream.flush();
        return Some(());
    }
    let _ = write!(
        stream,
        "HTTP/1.1 400 Bad Request\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{ERROR_BODY}",
        ERROR_BODY.len()
    );
    let _ = stream.flush();
    Some(())
}

/// How long an answer to a message containing `hang` is held back: long
/// enough that a host stops the session first.
pub const HANG: Duration = Duration::from_secs(20);

/// The word a scripted answer echoes, from the last user message: `steer`,
/// `resume`, `hang`, or `briefing`.
#[must_use]
pub fn heard(body: &Value) -> &'static str {
    let last = body
        .get("messages")
        .or_else(|| body.get("input"))
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .rev()
                .find(|item| item.get("role").and_then(Value::as_str) == Some("user"))
        })
        .map(|item| item.to_string().to_lowercase())
        .unwrap_or_default();
    // The last user message can carry the host's context too; the
    // message's own words come last.
    ["steer", "resume", "hang"]
        .into_iter()
        .filter_map(|word| last.rfind(&format!("please {word}")).map(|at| (at, word)))
        .max()
        .map_or("briefing", |(_, word)| word)
}

/// The scripted answer to one call: how long to wait, the content type,
/// and the body. A Messages call gets an assistant turn that says `heard
/// <word>`, streamed when asked; a Responses call gets the same as a
/// Responses stream; `count_tokens` gets a count; anything else gets an
/// empty object.
#[must_use]
pub fn answer(path: &str, body: &[u8]) -> (Duration, &'static str, String) {
    let request: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let word = heard(&request);
    let delay = if word == "hang" { HANG } else { Duration::ZERO };
    let text = format!("heard {word}");
    if path.contains("count_tokens") {
        return (
            delay,
            "application/json",
            json!({ "input_tokens": 10 }).to_string(),
        );
    }
    if path.starts_with("/v1/messages") {
        let usage = json!({ "input_tokens": 10, "output_tokens": 3 });
        if request.get("stream").and_then(Value::as_bool) != Some(true) {
            let message = json!({
                "id": "msg_capture", "type": "message", "role": "assistant", "model": "capture",
                "content": [{ "type": "text", "text": text }],
                "stop_reason": "end_turn", "stop_sequence": null, "usage": usage,
            });
            return (delay, "application/json", message.to_string());
        }
        let events = [
            (
                "message_start",
                json!({"type":"message_start","message":{"id":"msg_capture","type":"message","role":"assistant","model":"capture","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":1}}}),
            ),
            (
                "content_block_start",
                json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            ),
            (
                "content_block_delta",
                json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":text}}),
            ),
            (
                "content_block_stop",
                json!({"type":"content_block_stop","index":0}),
            ),
            (
                "message_delta",
                json!({"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":3}}),
            ),
            ("message_stop", json!({"type":"message_stop"})),
        ];
        return (delay, "text/event-stream", sse(&events));
    }
    if path.ends_with("/responses") || path.contains("/responses?") {
        let item = json!({"type":"message","role":"assistant","id":"msg_capture","status":"completed","content":[{"type":"output_text","text":text,"annotations":[]}]});
        let events = [
            (
                "response.created",
                json!({"type":"response.created","response":{"id":"resp_capture"}}),
            ),
            (
                "response.output_item.done",
                json!({"type":"response.output_item.done","output_index":0,"item":item}),
            ),
            (
                "response.completed",
                json!({"type":"response.completed","response":{"id":"resp_capture","usage":{"input_tokens":10,"input_tokens_details":{"cached_tokens":0},"output_tokens":3,"output_tokens_details":{"reasoning_tokens":0},"total_tokens":13}}}),
            ),
        ];
        return (delay, "text/event-stream", sse(&events));
    }
    (delay, "application/json", "{}".to_string())
}

fn sse(events: &[(&str, Value)]) -> String {
    events
        .iter()
        .map(|(name, data)| format!("event: {name}\ndata: {data}\n\n"))
        .collect()
}

/// What one capture runs.
pub struct Plan {
    pub agent: Agent,
    pub model: String,
    /// The real CLI binary.
    pub binary: PathBuf,
    pub effort: Option<String>,
    pub tools: Option<String>,
    pub prompt_cache_ttl: Option<String>,
    /// `None` captures the CLI's own default prompt.
    pub variant: Option<Variant>,
    /// A label for the variant: `default`, `core`, or another name.
    pub label: String,
    /// The briefing the CLI reads on standard input.
    pub briefing: String,
    /// For Codex: the model catalog passed as `model_catalog_json`.
    pub codex_catalog: Option<PathBuf>,
    pub timeout: Duration,
}

/// One capture's result.
#[derive(Clone, Debug)]
pub struct Capture {
    /// The first model request's body, sanitized.
    pub request: Value,
    /// Its measurement.
    pub measurement: Value,
}

/// Runs the CLI once against a local server and returns its first model
/// request.
///
/// # Errors
///
/// Returns a message when the scratch directory, the server, or the CLI
/// can't be set up, or when the CLI sent no model request.
pub fn capture(plan: &Plan) -> Result<Capture, String> {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "coder-one-capture-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    let result = capture_in(plan, &scratch);
    // `CODER_ONE_CAPTURE_KEEP` keeps the scratch directory, with the CLI's
    // own stream, for a capture that needs a closer look.
    if std::env::var_os("CODER_ONE_CAPTURE_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(&scratch);
    } else {
        eprintln!("capture ▸ kept {}", scratch.display());
    }
    result
}

fn capture_in(plan: &Plan, scratch: &Path) -> Result<Capture, String> {
    let home = scratch.join("home");
    let work = scratch.join("app");
    let artifacts = scratch.join("artifacts");
    for dir in [&home, &work, &artifacts, &home.join(".codex")] {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
    }
    let server = Server::start()?;
    let binary = match plan.agent {
        Agent::ClaudeCode => plan.binary.clone(),
        Agent::Codex => codex_wrapper(
            scratch,
            &plan.binary,
            &server.url(),
            plan.codex_catalog.as_deref(),
        )?,
    };
    let cli = Cli {
        agent: plan.agent,
        binary: Some(binary.clone()),
        model: plan.model.clone(),
        deadline: plan.timeout,
        workdir: work.clone(),
        artifacts: artifacts.clone(),
        artifacts_label: "artifacts".to_string(),
        env: Vec::new(),
        credential: match plan.agent {
            Agent::ClaudeCode => Credential::OauthToken,
            Agent::Codex => Credential::OpenAiKey,
        },
        effort: plan.effort.clone(),
        tools: plan.tools.clone(),
        prompt_cache_ttl: plan.prompt_cache_ttl.clone(),
        system: plan.variant.clone(),
        episode: crate::deadline::Deadline::unbounded(),
        gate: None,
        granted: None,
        runs: 1,
        control: Default::default(),
    };
    let briefing = artifacts.join("delegate-1.briefing.md");
    std::fs::write(&briefing, &plan.briefing).map_err(|error| error.to_string())?;
    cli.prepare()?;
    let mut command = cli.command(
        &binary,
        &briefing,
        &artifacts.join("delegate-1.stream.jsonl"),
    );
    // Keep only what the dispatch itself sets, then the capture's own
    // variables: nothing inherited, so no real credential is present.
    let explicit: Vec<(String, String)> = command
        .get_envs()
        .filter_map(|(name, value)| {
            Some((
                name.to_string_lossy().into_owned(),
                value?.to_string_lossy().into_owned(),
            ))
        })
        .collect();
    command.env_clear();
    for (name, value) in explicit {
        command.env(name, value);
    }
    command
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .env("ANTHROPIC_BASE_URL", server.url())
        .env("CLAUDE_CODE_OAUTH_TOKEN", DUMMY_TOKEN)
        .env("CAPTURE_KEY", DUMMY_TOKEN)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot run {}: {error}", binary.display()))?;
    let started = Instant::now();
    let wanted = |requests: &[Captured]| {
        requests
            .iter()
            .any(|request| model_path(plan.agent, &request.path))
    };
    loop {
        if child.try_wait().ok().flatten().is_some() {
            break;
        }
        if started.elapsed() > plan.timeout {
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        if wanted(&server.requests()) && started.elapsed() > Duration::from_secs(2) {
            // The first request is what the capture needs; a retry adds
            // nothing.
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // A CLI that ignored the redirect would have tried the real service;
    // the cleared environment means it had no credential to send, but the
    // capture is void.
    let stream =
        std::fs::read_to_string(artifacts.join("delegate-1.stream.jsonl")).unwrap_or_default();
    for host in ["api.openai.com", "chatgpt.com", "api.anthropic.com"] {
        if stream.contains(host) {
            return Err(format!(
                "{} tried to reach {host} instead of the capture server",
                plan.agent.word()
            ));
        }
    }
    let requests = server.requests();
    let first = requests
        .iter()
        .find(|request| model_path(plan.agent, &request.path))
        .ok_or_else(|| {
            format!(
                "{} sent no model request ({} requests: {})",
                plan.agent.word(),
                requests.len(),
                requests
                    .iter()
                    .map(|r| format!("{} {}", r.method, r.path))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    let body: Value = serde_json::from_slice(&first.body)
        .map_err(|error| format!("the captured request is not JSON: {error}"))?;
    // Claude Code names its memory directory after the working directory
    // with `/` spelled `-`.
    let dashed = work.to_string_lossy().replace('/', "-");
    let request = sanitize(
        body,
        &[
            (dashed.as_str(), "-app"),
            (work.to_string_lossy().as_ref(), "/app"),
            (home.to_string_lossy().as_ref(), "/root"),
            (scratch.to_string_lossy().as_ref(), "/capture"),
        ],
    );
    let mut measurement = measure(plan.agent, &request, plan.variant.as_ref());
    if let Value::Object(map) = &mut measurement {
        map.insert("label".to_string(), json!(plan.label));
        map.insert("model".to_string(), json!(plan.model));
        map.insert("effort".to_string(), json!(plan.effort));
        map.insert("tools".to_string(), json!(plan.tools));
        map.insert("prompt_cache_ttl".to_string(), json!(plan.prompt_cache_ttl));
        map.insert("path".to_string(), json!(first.path));
        map.insert("body_bytes".to_string(), json!(first.body.len()));
        map.insert(
            "catalog".to_string(),
            json!(match plan.agent {
                Agent::ClaudeCode => Value::Null,
                Agent::Codex if plan.codex_catalog.is_some() => json!("codex models cache"),
                Agent::Codex => json!("fallback metadata"),
            }),
        );
        map.insert(
            "variant".to_string(),
            plan.variant
                .as_ref()
                .map_or_else(|| system::default_record(plan.agent), Variant::record),
        );
    }
    Ok(Capture {
        request,
        measurement,
    })
}

fn model_path(agent: Agent, path: &str) -> bool {
    match agent {
        Agent::ClaudeCode => path.starts_with("/v1/messages") && !path.contains("count_tokens"),
        Agent::Codex => path.ends_with("/responses") || path.contains("/responses?"),
    }
}

/// A shell script that runs Codex with the capture provider set, so the
/// dispatch's own command runs unchanged.
pub fn codex_wrapper(
    scratch: &Path,
    codex: &Path,
    url: &str,
    catalog: Option<&Path>,
) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    let quote = |text: &str| format!("'{}'", text.replace('\'', "'\\''"));
    let mut settings = vec![
        r#"model_provider="capture""#.to_string(),
        r#"model_providers.capture.name="capture""#.to_string(),
        format!(r#"model_providers.capture.base_url="{url}/v1""#),
        r#"model_providers.capture.env_key="CAPTURE_KEY""#.to_string(),
        r#"model_providers.capture.wire_api="responses""#.to_string(),
        "model_providers.capture.request_max_retries=0".to_string(),
        "model_providers.capture.stream_max_retries=0".to_string(),
    ];
    if let Some(catalog) = catalog {
        settings.push(format!(
            "model_catalog_json={}",
            serde_json::to_string(&catalog.to_string_lossy()).unwrap_or_default()
        ));
    }
    let flags: Vec<String> = settings
        .iter()
        .map(|setting| format!("-c {}", quote(setting)))
        .collect();
    let script = format!(
        // A subcommand's own `-c` settings replace the root's, so the
        // provider goes after `exec`, beside the dispatch's settings.
        "#!/bin/sh\nsub=$1\nshift\nexec {} \"$sub\" {} \"$@\"\n",
        quote(&codex.to_string_lossy()),
        flags.join(" ")
    );
    let path = scratch.join("codex-capture");
    std::fs::write(&path, script).map_err(|error| error.to_string())?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| error.to_string())?;
    Ok(path)
}

/// Writes the `models` list of Codex's model cache to `out`, the only
/// part of `~/.codex` a capture reads.
///
/// # Errors
///
/// Returns a message when the cache is missing or doesn't read.
pub fn codex_catalog(cache: &Path, out: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(cache)
        .map_err(|error| format!("cannot read {}: {error}", cache.display()))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| format!("{} is not JSON: {error}", cache.display()))?;
    let models = value
        .get("models")
        .cloned()
        .ok_or_else(|| format!("{} has no models list", cache.display()))?;
    std::fs::write(out, json!({ "models": models }).to_string()).map_err(|error| error.to_string())
}

/// Rewrites host paths and removes per-run identifiers, so a capture can be
/// checked in and two captures of one variant compare.
#[must_use]
pub fn sanitize(mut body: Value, paths: &[(&str, &str)]) -> Value {
    fn walk(value: &mut Value, paths: &[(&str, &str)]) {
        match value {
            Value::String(text) => {
                for (from, to) in paths {
                    if !from.is_empty() && text.contains(from) {
                        *text = text.replace(from, to);
                    }
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|item| walk(item, paths)),
            Value::Object(map) => map.values_mut().for_each(|item| walk(item, paths)),
            _ => {}
        }
    }
    walk(&mut body, paths);
    if let Value::Object(map) = &mut body {
        map.remove("client_metadata");
        if let Some(metadata) = map.get_mut("metadata").and_then(Value::as_object_mut) {
            metadata.remove("user_id");
        }
        if map.contains_key("prompt_cache_key") {
            map.insert("prompt_cache_key".to_string(), json!("<per-session id>"));
        }
    }
    body
}

fn text_of(block: &Value) -> String {
    match block {
        Value::String(text) => text.clone(),
        Value::Object(map) => map
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        _ => String::new(),
    }
}

fn chars(text: &str) -> usize {
    text.chars().count()
}

/// Every string in a value, joined, for containment checks.
fn strings(value: &Value, out: &mut String) {
    match value {
        Value::String(text) => {
            out.push_str(text);
            out.push('\n');
        }
        Value::Array(items) => items.iter().for_each(|item| strings(item, out)),
        Value::Object(map) => map.values().for_each(|item| strings(item, out)),
        _ => {}
    }
}

/// Measures a captured first request: each part's size and cache marker,
/// the totals, and whether the variant's sections and the protected
/// section arrived.
#[must_use]
pub fn measure(agent: Agent, body: &Value, variant: Option<&Variant>) -> Value {
    let mut parts = Vec::new();
    let mut markers = Vec::new();
    let mut tools_chars = 0;
    let mut tools = 0;
    match agent {
        Agent::ClaudeCode => {
            // The API reads tools, then system, then messages; a cache
            // marker covers everything before it.
            for tool in body
                .get("tools")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                tools += 1;
                tools_chars += chars(&tool.to_string());
            }
            parts.push(json!({ "part": "tools", "count": tools, "chars": tools_chars, "cache": Value::Null }));
            let mut push = |part: String, block: &Value| {
                let cache = block.get("cache_control").cloned().unwrap_or(Value::Null);
                if !cache.is_null() {
                    markers.push(
                        json!({ "at": part, "type": cache.get("type"), "ttl": cache.get("ttl") }),
                    );
                }
                parts
                    .push(json!({ "part": part, "chars": chars(&text_of(block)), "cache": cache }));
            };
            for (i, block) in body
                .get("system")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
            {
                push(format!("system[{i}]"), block);
            }
            for (i, message) in body
                .get("messages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
            {
                let role = message.get("role").and_then(Value::as_str).unwrap_or("?");
                match message.get("content") {
                    Some(Value::Array(blocks)) => {
                        for (j, block) in blocks.iter().enumerate() {
                            push(format!("messages[{i}].{role}[{j}]"), block);
                        }
                    }
                    Some(content) => push(format!("messages[{i}].{role}"), content),
                    None => {}
                }
            }
        }
        Agent::Codex => {
            if let Some(instructions) = body.get("instructions").and_then(Value::as_str) {
                parts.push(json!({ "part": "instructions", "chars": chars(instructions), "cache": Value::Null }));
            }
            for tool in body
                .get("tools")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                tools += 1;
                tools_chars += chars(&tool.to_string());
            }
            for (i, item) in body
                .get("input")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
            {
                let kind = item.get("type").and_then(Value::as_str).unwrap_or("?");
                let role = item.get("role").and_then(Value::as_str).unwrap_or("");
                if let Some(extra) = item.get("tools").and_then(Value::as_array) {
                    tools += extra.len();
                    let size: usize = extra.iter().map(|tool| chars(&tool.to_string())).sum();
                    tools_chars += size;
                    parts.push(json!({ "part": format!("input[{i}].{kind}.tools"), "count": extra.len(), "chars": size, "cache": Value::Null }));
                    continue;
                }
                for (j, block) in item
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    parts.push(json!({ "part": format!("input[{i}].{role}[{j}]"), "chars": chars(&text_of(block)), "cache": Value::Null }));
                }
            }
            if body.get("prompt_cache_key").is_some() {
                markers.push(json!({ "at": "prompt_cache_key", "type": "automatic prefix cache", "ttl": Value::Null }));
            }
        }
    }
    let total: usize = parts
        .iter()
        .filter_map(|part| part.get("chars").and_then(Value::as_u64))
        .map(|n| usize::try_from(n).unwrap_or(0))
        .sum();
    let mut all = String::new();
    strings(body, &mut all);
    let security = system::section(agent, "security")
        .map(|section| section.text.trim())
        .unwrap_or_default();
    let (sections, missing) = match variant {
        Some(variant) => {
            let missing: Vec<String> = variant
                .sections()
                .iter()
                .filter(|(section, _)| !all.contains(section.text.trim()))
                .map(|(section, _)| section.id.to_string())
                .collect();
            (variant.sections().len(), missing)
        }
        None => (0, Vec::new()),
    };
    let default_main = system::default_text(agent);
    let mut checks = Map::new();
    checks.insert(
        "protected_present".to_string(),
        json!(all.contains(security)),
    );
    checks.insert(
        "default_main_prompt_present".to_string(),
        json!(all.contains(default_main.trim())),
    );
    checks.insert("variant_sections".to_string(), json!(sections));
    checks.insert("variant_sections_missing".to_string(), json!(missing));
    json!({
        "schema": SCHEMA,
        "agent": agent.word(),
        "parts": parts,
        "markers": markers,
        "totals": {
            "text_chars": total,
            "tools": tools,
            "tools_chars": tools_chars,
            "estimated_tokens": total.div_ceil(4),
        },
        "checks": checks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_records_a_body_and_answers_with_an_error() {
        let server = Server::start().unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
        let body = r#"{"model":"m"}"#;
        write!(
            stream,
            "POST /v1/messages?beta=true HTTP/1.1\r\nhost: x\r\nauthorization: Bearer secret\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        assert!(answer.starts_with("HTTP/1.1 400"));
        assert!(answer.contains("capture only"));
        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "/v1/messages?beta=true");
        assert_eq!(requests[0].body, body.as_bytes());
    }

    #[test]
    fn a_chunked_body_is_read_whole() {
        let server = Server::start().unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
        write!(
            stream,
            "POST /v1/responses HTTP/1.1\r\ntransfer-encoding: chunked\r\n\r\n5\r\n{{\"a\":\r\n2\r\n1}}\r\n0\r\n\r\n"
        )
        .unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        assert_eq!(server.requests()[0].body, br#"{"a":1}"#);
    }

    #[test]
    fn the_measurement_reads_claudes_parts_and_markers() {
        let request: Value = serde_json::from_str(include_str!(
            "../../../docs/terminal-bench/claude-code-delegate-prompt/request.json"
        ))
        .unwrap();
        let measured = measure(Agent::ClaudeCode, &request, None);
        assert_eq!(measured["checks"]["protected_present"], json!(true));
        assert_eq!(
            measured["checks"]["default_main_prompt_present"],
            json!(true)
        );
        assert_eq!(measured["totals"]["tools"], json!(6));
        let markers = measured["markers"].as_array().unwrap();
        assert_eq!(markers.len(), 3);
        assert!(markers.iter().all(|m| m["ttl"] == json!("1h")));
    }

    #[test]
    fn the_measurement_finds_a_variants_sections_in_a_codex_request() {
        let mut request: Value = serde_json::from_str(include_str!(
            "../../../docs/terminal-bench/delegate-prompts/codex-request.json"
        ))
        .unwrap();
        let measured = measure(Agent::Codex, &request, None);
        assert_eq!(measured["checks"]["protected_present"], json!(false));
        assert_eq!(
            measured["checks"]["default_main_prompt_present"],
            json!(true)
        );
        // Swap the base instructions for the core, as
        // `model_instructions_file` does.
        let variant = Variant::new(Agent::Codex, system::Policy::preset("core").unwrap());
        request["input"][1]["content"][0]["text"] = json!(variant.text());
        let measured = measure(Agent::Codex, &request, Some(&variant));
        assert_eq!(measured["checks"]["protected_present"], json!(true));
        assert_eq!(measured["checks"]["variant_sections_missing"], json!([]));
        assert_eq!(measured["markers"][0]["at"], json!("prompt_cache_key"));
    }

    #[test]
    fn sanitizing_rewrites_host_paths_and_drops_identifiers() {
        let body = json!({
            "client_metadata": { "installation_id": "x" },
            "prompt_cache_key": "01a0",
            "metadata": { "user_id": "u" },
            "input": [{ "text": "<cwd>/tmp/s/app</cwd> /tmp/s/home/.codex" }],
        });
        let clean = sanitize(body, &[("/tmp/s/app", "/app"), ("/tmp/s/home", "/root")]);
        assert!(clean.get("client_metadata").is_none());
        assert_eq!(clean["prompt_cache_key"], json!("<per-session id>"));
        assert!(clean["metadata"].get("user_id").is_none());
        assert_eq!(
            clean["input"][0]["text"],
            json!("<cwd>/app</cwd> /root/.codex")
        );
    }
}
