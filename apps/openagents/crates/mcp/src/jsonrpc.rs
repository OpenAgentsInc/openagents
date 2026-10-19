//! MCP stdio transport: newline-delimited JSON-RPC 2.0 on stdin/stdout.
//!
//! Requests are handled concurrently (a `wait_for_turn` may block for
//! minutes while the client keeps pinging); responses are serialized through
//! one writer task so frames never interleave. Notifications from the client
//! (`notifications/initialized`, `notifications/cancelled`) are accepted and
//! ignored — there is no server-side state to initialize or cancel.
//!
//! A server whose binary an upgrade replaced execs the new one in place
//! before it answers another tool call (#12110, [`crate::reexec`]), and a
//! tool call with a field its tool doesn't declare is refused, not dropped.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

use crate::reexec::{Binary, Input};
use crate::tools::Tools;

/// Protocol revisions this server knows. The client's requested revision
/// is echoed when it is one of these; otherwise the newest is offered and
/// the client decides whether to continue (per the MCP handshake rules).
const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
const LATEST_PROTOCOL: &str = "2025-06-18";

const INSTRUCTIONS: &str = "\
OpenAgents runs coding agents in chats, each hosted on a device, optionally in a project \
(a folder on that specific device). These tools operate the local OpenAgents engine: \
discover devices/projects/chats, create chats with a chosen harness and \
model, read transcripts, and send messages between chats.\n\
\n\
Chats are referenced by full id, a unique id prefix, or an exact title. Use \
`whoami` to learn which chat you are speaking from; messages you send are \
attributed to it and a chat cannot message itself. For request/response \
with a single chat, `send_message` with `wait: true` returns its reply. \
Every wait lasts at most 120 seconds and ends early, with outcome `interrupted`, \
when a new message arrives for you; read it, then wait again. \
For parallel delegation, use `create_chats` with a prompt for each chat or \
`send_messages` for existing chats. These batch tools launch requests concurrently \
even if your harness executes tool calls sequentially. Alternatively, launch ALL \
chats/messages with `wait: false` first, then use `wait_for_turn` to collect replies. \
Do not wait for one worker before launching the next independent worker. \
Every chat you create is a standalone session listed in Sessions, like one the \
user started. \
Discover hosts with `list_devices`, then `list_projects {device}`, \
`list_harnesses {device}` and `list_models {device, harness}`. A project fixes \
its host; with both project and device it must belong to that device. Use listed \
ids when names/paths repeat. Without project or device, creation uses the local engine. \
If a chat is `awaitingInput`, answer it with `respond_to_input`. \
Agent builds on this computer share a few build slots and wait for one on their own; \
`build_status` shows how many run and wait and the free disk, and sets the limit. \
`machine_status` reads disk, builds, sweep, grants, capacity, queues, pool, health, and owner steps with source, age, and a read-only deploy permission test; use it at each check-in instead of shell probes. \
For an issue claim whose session matches no app chat, use `list_runs` on this computer. \
Match its harness and bare session id. Report a live run with recent activity; \
with authority to manage claims, release a dead or stalled holder with a note and dispatch its issue. \
If no run matches or the read fails, report that you couldn't check it. Never type into its terminal. \
Pruning a build cache or stopping a process goes through `machine_action`, only under the \
owner's grant to this chat, and every action is logged. \
A chat holding the owner's `delegate` grant gives other chats `build.prune`, `process.stop` or \
`host.upgrade` on their own computer with `grant_machine` and ends it with `revoke_machine`; both are logged. \
To run the newest main in this computer's app, use `upgrade_host` (it needs the \
host.upgrade scope); never rebuild or restart the app yourself.\n\
Use `land_status` to read the shared landing queue through the signed-in app account: \
the integrator, waiting changes, active landings, and failures.\n\
\n\
When a chart, table, diagram, dashboard, or 3D view would say more than prose and \
`show_visual` is available, write a Luau visual script and show it with `show_visual` before \
your final reply. OpenAgents renders it with its own native interface and enforces its style: \
content sits directly on the chat with no cards or boxed panels, text uses the interface font \
and type scale, and colors are theme tokens that work on frosted glass or opaque, light or dark. \
The reader sees the visual above that reply, so don't announce or restate it.\n\
\n\
When Video Lab tools (`lab_*`) are offered, films, shots and scenes are typed documents in \
the OpenAgents Video Lab: read a document (`lab_show` for a time range, `lab_open` only to open it) before changing it, change it only with \
`lab_edit` (or the Lab's other editing tools), and if the Lab refuses an edit, say why in your \
own words and offer what the Lab would accept. Never invent a version, time, or result the \
tools didn't return. Each Lab call that returns a film shows its own card above your reply, so \
make the calls the request needs and no more (don't open a document just to show a range of it); end with the \
Lab player link from the result, which opens the player and timeline in the app's browser.";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// What a tool call gets while an upgraded server waits to restart.
pub(crate) const RESTARTING: &str = "OpenAgents was upgraded, and this tool server is restarting into the new version. Call the tool again in a few seconds.";

/// Replaces this process with the new binary; returns only on failure.
pub(crate) type Exec = dyn Fn(&Binary, &Path) -> std::io::Error + Send + Sync;

enum Out {
    Line(String),
    /// Answered once everything before it is written.
    Flush(oneshot::Sender<()>),
}

/// Serve MCP on this process's stdin/stdout until stdin closes.
pub async fn serve_stdio(tools: Arc<Tools>) -> anyhow::Result<()> {
    let resumed = std::env::var_os(crate::reexec::RESUME_ENV).is_some();
    let input = Input::stdin(crate::reexec::take_resume());
    #[cfg(unix)]
    let exec: Option<Box<Exec>> = Some(Box::new(crate::reexec::exec));
    #[cfg(not(unix))]
    let exec: Option<Box<Exec>> = None;
    let binary = exec.as_ref().and_then(|_| Binary::current());
    serve(tools, input, tokio::io::stdout(), binary, exec, resumed).await
}

/// [`serve_stdio`] over any input and output, with the binary to watch and
/// how to exec it.
pub(crate) async fn serve<W: AsyncWrite + Unpin + Send + 'static>(
    tools: Arc<Tools>,
    mut input: Input,
    mut output: W,
    binary: Option<Binary>,
    exec: Option<Box<Exec>>,
    resumed: bool,
) -> anyhow::Result<()> {
    let (out_tx, mut out_rx) = mpsc::channel::<Out>(64);
    let writer = tokio::spawn(async move {
        while let Some(out) = out_rx.recv().await {
            match out {
                Out::Line(line) => {
                    if output.write_all(line.as_bytes()).await.is_err()
                        || output.write_all(b"\n").await.is_err()
                        || output.flush().await.is_err()
                    {
                        break;
                    }
                }
                Out::Flush(done) => {
                    let _ = output.flush().await;
                    let _ = done.send(());
                }
            }
        }
    });
    let send = |out: &mpsc::Sender<Out>, value: Value| {
        let out = out.clone();
        async move {
            let _ = out.send(Out::Line(value.to_string())).await;
        }
    };
    if resumed {
        // The tools the client listed came from the old build.
        send(
            &out_tx,
            json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" }),
        )
        .await;
    }
    let in_flight = Arc::new(AtomicUsize::new(0));
    let mut exec_tried = false;
    while let Some(raw) = input.next_line().await {
        let text = String::from_utf8_lossy(&raw);
        let line = text.trim();
        if line.is_empty() {
            continue;
        }
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(err) => {
                send(
                    &out_tx,
                    error_response(Value::Null, PARSE_ERROR, &format!("parse error: {err}")),
                )
                .await;
                continue;
            }
        };
        let tool_call = message.get("method").and_then(Value::as_str) == Some("tools/call")
            && message.get("id").is_some();
        if tool_call
            && let (Some(binary), Some(exec)) = (&binary, &exec)
            && binary.replaced()
        {
            if in_flight.load(Ordering::SeqCst) > 0 {
                // Its answer would come from the old build; the exec waits
                // for the calls still running.
                let id = message.get("id").cloned().unwrap_or(Value::Null);
                send(
                    &out_tx,
                    ok_response(
                        id,
                        json!({ "content": [{ "type": "text", "text": RESTARTING }], "isError": true }),
                    ),
                )
                .await;
                continue;
            }
            if !exec_tried {
                exec_tried = true;
                let (done, flushed) = oneshot::channel();
                let _ = out_tx.send(Out::Flush(done)).await;
                let _ = flushed.await;
                match crate::reexec::write_resume(&raw, input.rest()) {
                    Ok(path) => {
                        let error = exec(binary, &path);
                        tracing::warn!(%error, "mcp: could not exec the upgraded binary");
                        let _ = std::fs::remove_file(&path);
                    }
                    Err(error) => {
                        tracing::warn!(%error, "mcp: could not hand over to the upgraded binary");
                    }
                }
            }
        }
        let Some(response) = route(&tools, message, &out_tx, &in_flight) else {
            continue;
        };
        send(&out_tx, response).await;
    }
    drop(out_tx);
    let _ = writer.await;
    Ok(())
}

/// Classify one inbound message. Requests are answered asynchronously via
/// `out`; an immediate `Some` is a synchronous error reply.
fn route(
    tools: &Arc<Tools>,
    message: Value,
    out: &mpsc::Sender<Out>,
    in_flight: &Arc<AtomicUsize>,
) -> Option<Value> {
    let Some(object) = message.as_object() else {
        return Some(error_response(
            Value::Null,
            INVALID_REQUEST,
            "expected a JSON-RPC object",
        ));
    };
    let id = object.get("id").cloned();
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let params = object.get("params").cloned().unwrap_or(Value::Null);
    match (id, method) {
        (Some(id), Some(method)) => {
            let tools = tools.clone();
            let out = out.clone();
            let in_flight = in_flight.clone();
            in_flight.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let response = handle_request(&tools, id, &method, params).await;
                let _ = out.send(Out::Line(response.to_string())).await;
                in_flight.fetch_sub(1, Ordering::SeqCst);
            });
            None
        }
        (None, Some(method)) => {
            tracing::debug!(method, "mcp: notification");
            None
        }
        // A response to a server-initiated request: we never send any.
        (Some(_), None) => None,
        (None, None) => Some(error_response(
            Value::Null,
            INVALID_REQUEST,
            "missing method",
        )),
    }
}

/// Answer one request. Public for tests and for embedding the dispatcher
/// behind another transport.
pub async fn handle_request(tools: &Tools, id: Value, method: &str, params: Value) -> Value {
    match method {
        "initialize" => {
            let requested = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(LATEST_PROTOCOL);
            let version = if PROTOCOL_VERSIONS.contains(&requested) {
                requested
            } else {
                LATEST_PROTOCOL
            };
            ok_response(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": true } },
                    "serverInfo": {
                        "name": "openagents",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "instructions": INSTRUCTIONS,
                }),
            )
        }
        "ping" => ok_response(id, json!({})),
        "tools/list" => ok_response(id, json!({ "tools": tools.list() })),
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return error_response(id, INVALID_PARAMS, "tools/call needs a tool name");
            };
            if !tools.has(name) {
                return error_response(id, INVALID_PARAMS, &format!("unknown tool: {name}"));
            }
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let schema = tools
                .list()
                .into_iter()
                .find(|tool| tool.name == name)
                .map(|tool| tool.input_schema);
            if let Some(refused) =
                schema.and_then(|schema| unknown_fields(name, &schema, &arguments))
            {
                return ok_response(
                    id,
                    json!({ "content": [{ "type": "text", "text": refused }], "isError": true }),
                );
            }
            match tools.call(name, arguments).await {
                Ok(value) => ok_response(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": pretty(&value) }],
                        "structuredContent": wrap_structured(value),
                        "isError": false,
                    }),
                ),
                Err(message) => ok_response(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": message }],
                        "isError": true,
                    }),
                ),
            }
        }
        // Optional capabilities we do not advertise; answer empty rather
        // than erroring so permissive clients do not log noise.
        "resources/list" => ok_response(id, json!({ "resources": [] })),
        "prompts/list" => ok_response(id, json!({ "prompts": [] })),
        other => error_response(id, METHOD_NOT_FOUND, &format!("unknown method: {other}")),
    }
}

/// The refusal for arguments naming a field the tool's schema doesn't
/// declare (#12110): dropping it silently loses what the caller meant.
/// Only top-level fields of a schema that lists its properties and doesn't
/// allow others are checked.
pub(crate) fn unknown_fields(name: &str, schema: &Value, arguments: &Value) -> Option<String> {
    let declared = schema.get("properties")?.as_object()?;
    if schema.get("additionalProperties") == Some(&Value::Bool(true)) {
        return None;
    }
    let unknown: Vec<&str> = arguments
        .as_object()?
        .keys()
        .map(String::as_str)
        .filter(|key| !declared.contains_key(*key))
        .collect();
    if unknown.is_empty() {
        return None;
    }
    let mut known: Vec<&str> = declared.keys().map(String::as_str).collect();
    known.sort_unstable();
    let quote = |keys: &[&str]| {
        keys.iter()
            .map(|key| format!("`{key}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    Some(format!(
        "`{name}` doesn't take {}. It takes {}. If the field is new, this tool server may be older than the app; it restarts into the new version on its own.",
        quote(&unknown),
        if known.is_empty() {
            "no fields".to_owned()
        } else {
            quote(&known)
        }
    ))
}

/// `structuredContent` must be an object; arrays and scalars are wrapped.
fn wrap_structured(value: Value) -> Value {
    if value.is_object() {
        value
    } else {
        json!({ "result": value })
    }
}

fn pretty(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

fn ok_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}
