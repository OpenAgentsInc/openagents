//! The Streamable HTTP transport for `oak`'s MCP server — the same
//! dispatch [`crate::mcp::serve`] runs over stdio, behind the session
//! and protocol-version checks the specification adds on the wire.
//!
//! One endpoint, `POST /mcp`: a JSON-RPC request answers 200 with the
//! response document; a notification or a client response answers 202
//! with no body. `initialize` mints a session and returns its id in
//! `Mcp-Session-Id`; every later call carries that header, and `DELETE
//! /mcp` ends the session. `GET /mcp` would be the server's SSE stream
//! — this server never initiates messages, so it answers 405. The
//! server card is `GET /mcp/card`.
//!
//! Each call may present `Authorization: Bearer <key>`; the key is
//! forwarded to the inference tools for that call only, never stored.
//! When no key is presented the tools fall back to the operator
//! configuration — the same `OPENAGENTS_API_KEY` and config file the
//! stdio server reads.

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use secp256k1::rand::Rng;
use serde_json::{Value, json};

use crate::MAX_MCP_MESSAGE_BYTES;
use crate::mcp::{self, Options, PROTOCOL_VERSIONS, Phase};

/// The session header the specification defines.
const SESSION_HEADER: &str = "mcp-session-id";
/// The negotiated-version header the specification defines.
const VERSION_HEADER: &str = "mcp-protocol-version";
/// The most live sessions one process holds — the bound the discovery
/// surface's server card publishes.
const MAX_SESSIONS: usize = discovery::site::MCP_SESSION_BOUND;
/// The idle bound after which a session is forgotten — a stale id then
/// answers 404 and the client re-initializes. The server card publishes
/// the same number.
const SESSION_TTL: Duration = Duration::from_secs(discovery::site::MCP_SESSION_TTL_SECS);

/// Operator configuration plus the transport's own settings.
#[derive(Default)]
pub struct HttpOptions {
    /// The tool-call configuration `oak-mcp` takes: endpoint,
    /// credential file, workspace, timeouts. No credential flag.
    pub options: Options,
    /// Origins beyond localhost the server accepts — `Origin` headers
    /// from a browser client the operator chooses to trust.
    pub origins: Vec<String>,
}

/// One negotiated session: the lifecycle phase the shared dispatch
/// drives, the protocol version `initialize` settled on, and the
/// activity clock expiry reads.
struct Session {
    phase: Phase,
    version: Option<String>,
    touched: Instant,
}

/// The shared server state behind the router.
struct Server {
    options: Options,
    origins: Vec<String>,
    sessions: Mutex<HashMap<String, Arc<Mutex<Session>>>>,
}

/// The route table — `POST`/`DELETE /mcp`, the card at `GET /mcp/card`,
/// and the refused SSE stream at `GET /mcp`.
pub fn router(options: HttpOptions) -> Router {
    let server = Arc::new(Server {
        options: options.options,
        origins: options.origins,
        sessions: Mutex::new(HashMap::new()),
    });
    Router::new()
        .route("/mcp", post(post_message).delete(terminate).get(no_stream))
        .route("/mcp/card", get(card))
        .layer(DefaultBodyLimit::max(
            usize::try_from(MAX_MCP_MESSAGE_BYTES).unwrap_or(usize::MAX),
        ))
        .with_state(server)
}

/// Serve the router on an already-bound listener until the process
/// ends. The binary binds the address; tests bind port 0.
///
/// # Errors
///
/// Returns the listener's error if serving stops on one.
pub async fn serve(options: HttpOptions, listener: tokio::net::TcpListener) -> io::Result<()> {
    axum::serve(listener, router(options)).await
}

/// The server's self-description: identity, the endpoint, the served
/// protocol versions, the session contract, and the auth model — what
/// a client or operator reads before configuring against it.
async fn card() -> Json<Value> {
    Json(discovery::site::mcp_card(mcp::tool_list()["tools"].clone()))
}

/// `GET /mcp` is the server-initiated SSE stream in the specification.
/// This server never initiates messages, so there is nothing to
/// stream — the refusal is explicit rather than a hanging connection.
async fn no_stream() -> (StatusCode, Json<Value>) {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(json!({
            "error": "this server has no SSE stream; it never starts a message, and every reply comes in the response to your POST",
        })),
    )
}

/// `POST /mcp`: the whole transport — origin and version checks, the
/// session lookup or `initialize` mint, then the shared dispatch.
async fn post_message(
    State(server): State<Arc<Server>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Some(refused) = check_origin(&server, &headers) {
        return refused;
    }
    if let Some(refused) = check_accept(&headers) {
        return refused;
    }
    if !is_json(headers.get(header::CONTENT_TYPE)) {
        return problem(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "the body must be `application/json`",
        );
    }
    if let Some(version) = headers.get(VERSION_HEADER).and_then(|v| v.to_str().ok())
        && !PROTOCOL_VERSIONS.contains(&version)
    {
        return problem(
            StatusCode::BAD_REQUEST,
            &format!("`MCP-Protocol-Version: {version}` is not a version this server serves"),
        );
    }
    let credential = match bearer(&headers) {
        Ok(credential) => credential,
        Err(response) => return *response,
    };
    let message: Value = match serde_json::from_slice(&body) {
        Ok(message) => message,
        Err(_) => {
            return problem(StatusCode::BAD_REQUEST, "the body isn't a JSON-RPC message");
        }
    };
    if message.is_array() {
        return problem(
            StatusCode::BAD_REQUEST,
            "this server doesn't accept batched requests; send one message per POST",
        );
    }
    let initializing = message.get("method").and_then(Value::as_str) == Some("initialize");
    let session_id = headers
        .get(SESSION_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let (id, session) = match (session_id, initializing) {
        (Some(id), _) => match lookup(&server, &id) {
            Some(session) => (id, session),
            None => return problem(StatusCode::NOT_FOUND, "unknown `Mcp-Session-Id`"),
        },
        (None, true) => match mint(&server) {
            Some(pair) => pair,
            None => {
                return problem(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "the server has reached its session limit; end a session or retry later",
                );
            }
        },
        (None, false) => {
            return problem(
                StatusCode::BAD_REQUEST,
                "an `Mcp-Session-Id` from `initialize` is required",
            );
        }
    };
    if !initializing
        && let Some(version) = headers.get(VERSION_HEADER).and_then(|v| v.to_str().ok())
    {
        let negotiated = session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .version
            .clone();
        if negotiated
            .as_deref()
            .is_some_and(|settled| settled != version)
        {
            return problem(
                StatusCode::BAD_REQUEST,
                "`MCP-Protocol-Version` must match the version `initialize` agreed on",
            );
        }
    }
    let options = server.options.clone();
    let dispatched = tokio::task::spawn_blocking(move || {
        let mut session = session.lock().unwrap_or_else(|e| e.into_inner());
        let reply = mcp::handle(
            &mut session.phase,
            &options,
            credential.as_deref(),
            &message,
        );
        if initializing
            && let Some(version) = reply
                .as_ref()
                .and_then(|reply| reply["result"]["protocolVersion"].as_str())
        {
            session.version = Some(version.to_string());
        }
        reply
    })
    .await;
    let reply = match dispatched {
        Ok(reply) => reply,
        Err(_) => {
            return problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the server failed while handling the message",
            );
        }
    };
    let mut response_headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(&id) {
        response_headers.insert(SESSION_HEADER, value);
    }
    match reply {
        None => (StatusCode::ACCEPTED, response_headers).into_response(),
        Some(reply) => (StatusCode::OK, response_headers, Json(reply)).into_response(),
    }
}

/// `DELETE /mcp`: the client ends its session.
async fn terminate(State(server): State<Arc<Server>>, headers: HeaderMap) -> Response {
    if let Some(refused) = check_origin(&server, &headers) {
        return refused;
    }
    let Some(id) = headers.get(SESSION_HEADER).and_then(|v| v.to_str().ok()) else {
        return problem(
            StatusCode::BAD_REQUEST,
            "`Mcp-Session-Id` names the session to end",
        );
    };
    let mut sessions = server.sessions.lock().unwrap_or_else(|e| e.into_inner());
    if sessions.remove(id).is_none() {
        return problem(StatusCode::NOT_FOUND, "unknown `Mcp-Session-Id`");
    }
    StatusCode::OK.into_response()
}

/// The session a presented id names, refreshing its activity clock and
/// sweeping expired entries while the map is held.
fn lookup(server: &Server, id: &str) -> Option<Arc<Mutex<Session>>> {
    let mut sessions = server.sessions.lock().unwrap_or_else(|e| e.into_inner());
    sweep(&mut sessions);
    let session = sessions.get(id)?.clone();
    session.lock().unwrap_or_else(|e| e.into_inner()).touched = Instant::now();
    Some(session)
}

/// Mint a session id and register it — `None` when the process bound
/// is held, which the caller reports as a 503.
fn mint(server: &Server) -> Option<(String, Arc<Mutex<Session>>)> {
    let mut sessions = server.sessions.lock().unwrap_or_else(|e| e.into_inner());
    sweep(&mut sessions);
    if sessions.len() >= MAX_SESSIONS {
        return None;
    }
    let mut bytes = [0_u8; 16];
    secp256k1::rand::rng().fill(&mut bytes);
    let id = format!("oak-mcp-{}", hex(&bytes));
    let session = Arc::new(Mutex::new(Session {
        phase: Phase::Start,
        version: None,
        touched: Instant::now(),
    }));
    sessions.insert(id.clone(), session.clone());
    Some((id, session))
}

/// Drop the sessions whose clock ran past the idle bound.
fn sweep(sessions: &mut HashMap<String, Arc<Mutex<Session>>>) {
    sessions.retain(|_, session| {
        session
            .lock()
            .map(|session| session.touched.elapsed() < SESSION_TTL)
            .unwrap_or(true)
    });
}

/// The `Origin` check the specification requires: a browser's origin
/// must be localhost or one the operator named; a missing `Origin` —
/// curl and the MCP clients — is fine.
fn check_origin(server: &Server, headers: &HeaderMap) -> Option<Response> {
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok())?;
    let local = origin_host(origin).is_some_and(|host| {
        host == "localhost" || host == "127.0.0.1" || host == "::1" || host.ends_with(".localhost")
    });
    let allowed = server.origins.iter().any(|named| named == origin);
    if local || allowed {
        None
    } else {
        Some(problem(
            StatusCode::FORBIDDEN,
            "this server doesn't accept requests from this `Origin`",
        ))
    }
}

/// The host a browser-style `Origin` names — `scheme://host[:port]`.
fn origin_host(origin: &str) -> Option<&str> {
    let after = origin.split("://").nth(1)?;
    after.split(['/', ':']).next()
}

/// The `Accept` contract: the caller must take `application/json` —
/// every answer this server gives is JSON — or any type.
fn check_accept(headers: &HeaderMap) -> Option<Response> {
    let accept = headers.get(header::ACCEPT).and_then(|v| v.to_str().ok())?;
    let json_ok = accept
        .split(',')
        .map(|part| part.split(';').next().unwrap_or("").trim())
        .any(|mime| mime == "application/json" || mime == "*/*" || mime == "text/event-stream");
    if json_ok {
        None
    } else {
        Some(problem(
            StatusCode::NOT_ACCEPTABLE,
            "`Accept` must allow `application/json`",
        ))
    }
}

/// A `Bearer` credential, when the caller presents one. A non-Bearer
/// `Authorization` is refused rather than silently dropped — the
/// caller believes it authenticated.
fn bearer(headers: &HeaderMap) -> Result<Option<String>, Box<Response>> {
    let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    else {
        return Ok(None);
    };
    let token = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty());
    match token {
        Some(token) => Ok(Some(token.to_string())),
        None => Err(Box::new(problem(
            StatusCode::UNAUTHORIZED,
            "`Authorization` must be `Bearer <key>`",
        ))),
    }
}

/// Whether the content type is JSON — `application/json` or a `+json`
/// suffix, with parameters allowed.
fn is_json(content_type: Option<&HeaderValue>) -> bool {
    content_type
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            let mime = v.split(';').next().unwrap_or("").trim();
            mime == "application/json" || mime.ends_with("+json")
        })
        .unwrap_or(false)
}

/// One HTTP refusal with a JSON body.
fn problem(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({"error": message}))).into_response()
}

/// Lowercase hex, no separators — the session id's tail.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
