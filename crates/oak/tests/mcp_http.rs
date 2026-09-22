//! `oak-mcp-http` against an upstream stub over real TCP: the session
//! lifecycle, the protocol-version headers, per-call bearer
//! forwarding, invalid arguments, typed refusal passthrough, DELETE,
//! and the origin check.
#![cfg(feature = "mcp-http")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use oak::mcp::{Options, PROTOCOL_VERSIONS};
use oak::mcp_http::{HttpOptions, serve};
use serde_json::{Value, json};

/// The key the operator's config file carries.
const OPERATOR_KEY: &str = "oak_op.secret";
/// The key an HTTP caller presents.
const CALLER_KEY: &str = "oak_caller.secret";

/// What the upstream remembers: every `Authorization` it saw, and the
/// classify calls it answered.
#[derive(Default)]
struct Upstream {
    authorizations: Mutex<Vec<Option<String>>>,
    classify_calls: AtomicUsize,
}

/// The upstream's admission: a missing or malformed bearer refuses
/// typed; both test keys pass so the test can tell which arrived.
fn admitted(upstream: &Upstream, headers: &HeaderMap) -> Option<Response> {
    let authorization = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    upstream
        .authorizations
        .lock()
        .unwrap()
        .push(authorization.clone());
    let key = authorization
        .as_deref()
        .and_then(|v| v.strip_prefix("Bearer "));
    match key {
        Some(OPERATOR_KEY) | Some(CALLER_KEY) => None,
        _ => Some(
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": {"code": "unauthenticated", "message": "the credential was refused"}})),
            )
                .into_response(),
        ),
    }
}

/// One answered classify report; an input id `UNCERTAIN` flags the
/// unit uncertain.
async fn classify(
    State(upstream): State<Arc<Upstream>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    upstream.classify_calls.fetch_add(1, Ordering::SeqCst);
    if let Some(refusal) = admitted(&upstream, &headers) {
        return refusal;
    }
    let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let labels: Vec<String> = request["labels"]
        .as_array()
        .map(|labels| {
            labels
                .iter()
                .filter_map(|l| l["id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_else(|| vec!["a".to_string(), "b".to_string()]);
    let labels = if labels.is_empty() {
        vec!["a".to_string(), "b".to_string()]
    } else {
        labels
    };
    let mut probabilities = serde_json::Map::new();
    for (index, label) in labels.iter().enumerate() {
        probabilities.insert(label.clone(), json!(if index == 0 { 0.9 } else { 0.1 }));
    }
    let mut results = Vec::new();
    for input in request["inputs"].as_array().into_iter().flatten() {
        let id = input["id"].as_str().unwrap_or("?").to_string();
        results.push(json!({
            "input": id,
            "outcome": "answered",
            "units": [{
                "dimension": "d",
                "mode": request["mode"].as_str().unwrap_or("single-label"),
                "outcome": "answered",
                "selected": labels[0],
                "uncertain": input["text"].as_str() == Some("UNCERTAIN"),
                "raw": {"probabilities": Value::Object(probabilities.clone())},
            }],
        }));
    }
    (
        StatusCode::OK,
        Json(json!({
            "v": "openagents.classify.v1",
            "outcome": "answered",
            "model": "stub-v1",
            "results": results,
            "outcomes": {"answered": 1},
            "usage": {"input_tokens": 10, "output_tokens": 2},
        })),
    )
        .into_response()
}

/// The native decision route — one noul answer per question.
async fn systemone(
    State(upstream): State<Arc<Upstream>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if let Some(refusal) = admitted(&upstream, &headers) {
        return refusal;
    }
    let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let mut answers = serde_json::Map::new();
    if let Some(questions) = request["questions"].as_object() {
        for id in questions.keys() {
            answers.insert(id.clone(), json!({"type": "noul", "noul": 0.9}));
        }
    }
    (
        StatusCode::OK,
        Json(json!({"model": "stub-v1", "answers": answers, "usage": {"input_tokens": 10, "output_tokens": 2}})),
    )
        .into_response()
}

/// `GET /v1/models` — the discovery route `list_models` reads.
async fn models(State(upstream): State<Arc<Upstream>>, headers: HeaderMap) -> Response {
    if let Some(refusal) = admitted(&upstream, &headers) {
        return refusal;
    }
    Json(json!({"models": [{"name": "stub-v1", "description": "the test door"}]})).into_response()
}

/// The upstream stub's listen address and shared state.
async fn upstream() -> (String, Arc<Upstream>) {
    let shared = Arc::new(Upstream::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new()
        .route("/v1/systemone", post(systemone))
        .route("/v1/models", get(models))
        .route("/v1/classify", post(classify))
        .with_state(shared.clone());
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (address, shared)
}

/// A `0600` config file carrying the operator credential.
fn config(dir: &tempfile::TempDir, url: &str) -> String {
    let path = dir.path().join("oak.json");
    std::fs::write(
        &path,
        format!("{{\"api_key\": \"{OPERATOR_KEY}\", \"base_url\": \"{url}\"}}"),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    path.to_str().unwrap().to_string()
}

/// The MCP server under test: `oak-mcp-http`'s own `serve` on a bound
/// port, pointed at the upstream through the config file.
async fn server(dir: &tempfile::TempDir, upstream_url: &str) -> String {
    server_with(dir, upstream_url, Vec::new()).await
}

/// `server` with explicit allowed origins.
async fn server_with(dir: &tempfile::TempDir, upstream_url: &str, origins: Vec<String>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let options = HttpOptions {
        options: Options {
            config: Some(config(dir, upstream_url).into()),
            timeout: Duration::from_secs(10),
            retries: 1,
            ..Options::default()
        },
        origins,
    };
    tokio::spawn(async move {
        serve(options, listener).await.unwrap();
    });
    address
}

/// A `tools/call` request message.
fn call(id: u64, name: &str, arguments: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": {"name": name, "arguments": arguments}})
}

/// The initialize request every session starts with.
fn initialize(version: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": version,
        "capabilities": {},
        "clientInfo": {"name": "test", "version": "1.0"},
    }})
}

/// POST one JSON-RPC message with the given headers; the returned
/// session header is the client's next credential.
async fn send(
    client: &reqwest::Client,
    url: &str,
    session: Option<&str>,
    bearer: Option<&str>,
    message: &Value,
) -> reqwest::Response {
    let mut request = client
        .post(format!("{url}/mcp"))
        .header("Accept", "application/json, text/event-stream")
        .json(message);
    if let Some(session) = session {
        request = request.header("Mcp-Session-Id", session);
    }
    if let Some(bearer) = bearer {
        request = request.bearer_auth(bearer);
    }
    request.send().await.unwrap()
}

/// Initialize a session and mark it initialized — the preamble every
/// operational test shares.
async fn open_session(client: &reqwest::Client, url: &str) -> String {
    let response = send(client, url, None, None, &initialize("2025-11-25")).await;
    assert_eq!(response.status(), 200);
    let session = response
        .headers()
        .get("Mcp-Session-Id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(session.starts_with("oak-mcp-"));
    let response = send(
        client,
        url,
        Some(&session),
        None,
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    )
    .await;
    assert_eq!(response.status(), 202);
    session
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn session_lifecycle_notifications_and_the_server_card() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _upstream) = upstream().await;
    let base = server(&dir, &url).await;
    let client = reqwest::Client::new();

    // The server card is public — no session, no credential.
    let card: Value = client
        .get(format!("{base}/mcp/card"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(card["v"], "openagents.mcp-server.v1");
    assert_eq!(card["protocol_versions"], json!(PROTOCOL_VERSIONS));
    // The card the discovery surface mirrors is generated from this same
    // answer — the bundled snapshot is what the gateway serves, so the
    // two surfaces cannot drift apart.
    assert_eq!(card["tools"], discovery::site::mcp_tools());
    let tools = card["tools"].as_array().unwrap();
    for name in [
        "classify_texts",
        "classify_dimensions",
        "classify_multi_label",
        "count_labels",
        "review_uncertain",
        "decide",
        "classify",
        "list_models",
    ] {
        assert!(tools.iter().any(|tool| tool["name"] == name), "{name}");
    }

    // `GET /mcp` would be the SSE stream — refused explicitly.
    let response = client.get(format!("{base}/mcp")).send().await.unwrap();
    assert_eq!(response.status(), 405);

    // No session id on a non-initialize call is a 400.
    let response = send(
        &client,
        &base,
        None,
        None,
        &call(9, "list_models", json!({})),
    )
    .await;
    assert_eq!(response.status(), 400);
    // A bogus session id is a 404.
    let response = send(
        &client,
        &base,
        Some("oak-mcp-bogus"),
        None,
        &call(9, "list_models", json!({})),
    )
    .await;
    assert_eq!(response.status(), 404);
    // Calls before `initialized` answer a JSON-RPC error, not an HTTP one.
    let fresh = send(&client, &base, None, None, &initialize("2025-11-25")).await;
    let fresh_session = fresh
        .headers()
        .get("Mcp-Session-Id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let response = send(
        &client,
        &base,
        Some(&fresh_session),
        None,
        &json!({"jsonrpc": "2.0", "id": 9, "method": "tools/list"}),
    )
    .await;
    assert_eq!(response.status(), 200);
    let reply: Value = response.json().await.unwrap();
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not initialized")
    );

    let session = open_session(&client, &base).await;
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        response
            .headers()
            .get("Mcp-Session-Id")
            .unwrap()
            .to_str()
            .unwrap(),
        session
    );
    let reply: Value = response.json().await.unwrap();
    assert_eq!(reply["result"]["tools"].as_array().unwrap().len(), 12);
    // `ping` answers; an unknown notification is a 202.
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}),
    )
    .await;
    assert_eq!(response.status(), 200);
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &json!({"jsonrpc": "2.0", "method": "notifications/progress", "params": {}}),
    )
    .await;
    assert_eq!(response.status(), 202);
    // Malformed JSON is a 400; a batch is refused.
    let response = client
        .post(format!("{base}/mcp"))
        .header("content-type", "application/json")
        .body("{not json")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    let response = client
        .post(format!("{base}/mcp"))
        .header("content-type", "application/json")
        .json(&json!([initialize("2025-11-25")]))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn protocol_version_headers_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _upstream) = upstream().await;
    let base = server(&dir, &url).await;
    let client = reqwest::Client::new();

    // An unsupported version on the wire is a 400.
    let response = client
        .post(format!("{base}/mcp"))
        .header("MCP-Protocol-Version", "1999-01-01")
        .json(&initialize("2025-11-25"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);

    // Negotiate 2025-06-18: the settled version is the client's.
    let response = send(&client, &base, None, None, &initialize("2025-06-18")).await;
    let session = response
        .headers()
        .get("Mcp-Session-Id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let reply: Value = response.json().await.unwrap();
    assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
    // The settled version passes; a different served version refuses.
    let response = client
        .post(format!("{base}/mcp"))
        .header("Mcp-Session-Id", &session)
        .header("MCP-Protocol-Version", "2025-06-18")
        .json(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 202);
    let response = client
        .post(format!("{base}/mcp"))
        .header("Mcp-Session-Id", &session)
        .header("MCP-Protocol-Version", "2025-11-25")
        .json(&json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn caller_bearer_forwards_per_request() {
    let dir = tempfile::tempdir().unwrap();
    let (url, upstream) = upstream().await;
    let base = server(&dir, &url).await;
    let client = reqwest::Client::new();
    let session = open_session(&client, &base).await;

    // No caller key — the operator's config file key reaches upstream.
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &call(1, "list_models", json!({})),
    )
    .await;
    assert_eq!(response.status(), 200);
    let reply: Value = response.json().await.unwrap();
    assert_eq!(reply["result"]["isError"], false, "{reply}");

    // A caller key is forwarded for that call only.
    let response = send(
        &client,
        &base,
        Some(&session),
        Some(CALLER_KEY),
        &call(2, "list_models", json!({})),
    )
    .await;
    assert_eq!(response.status(), 200);
    // A wrong caller key surfaces the typed refusal, not an HTTP error.
    let response = send(
        &client,
        &base,
        Some(&session),
        Some("oak_wrong.secret"),
        &call(3, "list_models", json!({})),
    )
    .await;
    assert_eq!(response.status(), 200);
    let reply: Value = response.json().await.unwrap();
    assert_eq!(reply["result"]["isError"], true);
    assert_eq!(
        reply["result"]["structuredContent"]["error"]["code"],
        "unauthenticated"
    );
    // A non-Bearer Authorization refuses at the transport.
    let response = client
        .post(format!("{base}/mcp"))
        .header("Mcp-Session-Id", &session)
        .header("Authorization", "Basic abc")
        .json(&call(4, "list_models", json!({})))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);

    let authorizations = upstream.authorizations.lock().unwrap();
    assert_eq!(authorizations.len(), 3);
    assert_eq!(authorizations[0].as_deref(), Some("Bearer oak_op.secret"));
    assert_eq!(
        authorizations[1].as_deref(),
        Some("Bearer oak_caller.secret")
    );
    assert_eq!(
        authorizations[2].as_deref(),
        Some("Bearer oak_wrong.secret")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn invalid_arguments_and_tool_results() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _upstream) = upstream().await;
    let base = server(&dir, &url).await;
    let client = reqwest::Client::new();
    let session = open_session(&client, &base).await;

    // Unknown argument — refused before any upstream call.
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &call(
            1,
            "classify_texts",
            json!({"texts": ["hi"], "labels": ["a", "b"], "bogus": true}),
        ),
    )
    .await;
    let reply: Value = response.json().await.unwrap();
    assert_eq!(reply["result"]["isError"], true);
    assert_eq!(
        reply["result"]["structuredContent"]["error"]["code"],
        "invalid_arguments"
    );
    // Unknown tool — a JSON-RPC params error.
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &call(2, "does_not_exist", json!({})),
    )
    .await;
    let reply: Value = response.json().await.unwrap();
    assert_eq!(reply["error"]["code"], -32602);
    // A good call answers the report — and `decide` reaches
    // `/v1/systemone` through the same transport.
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &call(
            3,
            "classify_texts",
            json!({"texts": ["refund please"], "labels": ["billing", "other"], "model": "stub-v1"}),
        ),
    )
    .await;
    let reply: Value = response.json().await.unwrap();
    assert_eq!(reply["result"]["isError"], false, "{reply}");
    assert_eq!(
        reply["result"]["structuredContent"]["v"],
        "openagents.classify.v1"
    );
    // `classify_multi_label` and `classify_dimensions` carry the same
    // report — the caller's threshold and per-dimension label sets.
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &call(
            7,
            "classify_multi_label",
            json!({"texts": ["overlapping"], "labels": ["a", "b"], "threshold": 0.5, "model": "stub-v1"}),
        ),
    )
    .await;
    let reply: Value = response.json().await.unwrap();
    assert_eq!(reply["result"]["isError"], false, "{reply}");
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &call(
            8,
            "classify_dimensions",
            json!({"inputs": [{"id": "x1", "text": "mixed work"}], "dimensions": [
                {"id": "topic", "mode": "single-label", "labels": ["a", "b"]},
                {"id": "flag", "mode": "binary", "labels": ["flagged"]},
            ], "threshold": 0.5, "model": "stub-v1"}),
        ),
    )
    .await;
    let reply: Value = response.json().await.unwrap();
    assert_eq!(reply["result"]["isError"], false, "{reply}");
    assert_eq!(
        reply["result"]["structuredContent"]["v"],
        "openagents.classify.v1"
    );
    // A dimension set missing `threshold` for its binary member is a
    // caller fault refused before the envelope leaves.
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &call(
            9,
            "classify_dimensions",
            json!({"inputs": ["x"], "dimensions": [
                {"id": "flag", "mode": "binary", "labels": ["flagged"]},
            ], "model": "stub-v1"}),
        ),
    )
    .await;
    let reply: Value = response.json().await.unwrap();
    assert_eq!(reply["result"]["isError"], true);
    assert!(
        reply["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("threshold")
    );
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &call(
            4,
            "decide",
            json!({"state": "a customer asks for a refund", "questions": {"refund": {"type": "noul", "instructions": "money back?"}}}),
        ),
    )
    .await;
    let reply: Value = response.json().await.unwrap();
    assert_eq!(reply["result"]["isError"], false, "{reply}");
    assert_eq!(
        reply["result"]["structuredContent"]["answers"]["refund"]["type"],
        "noul"
    );
    // `count_labels` reduces to aggregates; `review_uncertain` lists
    // only the flagged units.
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &call(
            5,
            "count_labels",
            json!({"texts": ["one", "two"], "labels": ["billing", "other"], "model": "stub-v1"}),
        ),
    )
    .await;
    let reply: Value = response.json().await.unwrap();
    let content = &reply["result"]["structuredContent"];
    assert_eq!(content["v"], "openagents.mcp-counts.v1");
    assert_eq!(content["counts"]["billing"], 2);
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &call(
            6,
            "review_uncertain",
            json!({"texts": ["sure", "UNCERTAIN"], "labels": ["billing", "other"], "uncertain_below": 0.95, "model": "stub-v1"}),
        ),
    )
    .await;
    let reply: Value = response.json().await.unwrap();
    let content = &reply["result"]["structuredContent"];
    assert_eq!(content["v"], "openagents.mcp-uncertain.v1");
    assert_eq!(content["uncertain_count"], 1);
    assert_eq!(content["uncertain"][0]["input"], "i1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn delete_ends_the_session_and_reconnect_reinitializes() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _upstream) = upstream().await;
    let base = server(&dir, &url).await;
    let client = reqwest::Client::new();
    let session = open_session(&client, &base).await;

    // DELETE without a session header is a 400; unknown is a 404.
    let response = client.delete(format!("{base}/mcp")).send().await.unwrap();
    assert_eq!(response.status(), 400);
    let response = client
        .delete(format!("{base}/mcp"))
        .header("Mcp-Session-Id", "oak-mcp-bogus")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);

    let response = client
        .delete(format!("{base}/mcp"))
        .header("Mcp-Session-Id", &session)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    // The ended session is gone — the client must re-initialize.
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &call(9, "list_models", json!({})),
    )
    .await;
    assert_eq!(response.status(), 404);
    let response = client
        .delete(format!("{base}/mcp"))
        .header("Mcp-Session-Id", &session)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    // Reconnect: a fresh initialize works and mints a new session.
    let session = open_session(&client, &base).await;
    let response = send(
        &client,
        &base,
        Some(&session),
        None,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}),
    )
    .await;
    assert_eq!(response.status(), 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn origin_check_refuses_unnamed_browser_origins() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _upstream) = upstream().await;
    let base = server_with(&dir, &url, vec!["https://app.example".to_string()]).await;
    let client = reqwest::Client::new();

    for (origin, expected) in [
        ("https://evil.example", 403),
        ("http://localhost:3000", 200),
        ("http://127.0.0.1:5173", 200),
        ("https://app.example", 200),
    ] {
        let response = client
            .post(format!("{base}/mcp"))
            .header("Origin", origin)
            .json(&initialize("2025-11-25"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "{origin}");
    }
}
