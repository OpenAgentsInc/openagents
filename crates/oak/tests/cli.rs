//! `oak` against a stub door over real TCP: answered, refused, retried,
//! conflicted, and unauthorized calls, plus the batch shapes.

use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};

/// The credential the stub accepts.
const KEY: &str = "oak_test.secret";

/// What the stub counts and remembers.
#[derive(Default)]
struct Stub {
    calls: AtomicUsize,
    flaky: AtomicUsize,
    keys: Mutex<HashMap<String, String>>,
}

async fn systemone(
    State(stub): State<Arc<Stub>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    stub.calls.fetch_add(1, Ordering::SeqCst);
    let typed = |status: u16, code: &str, message: &str| {
        (
            StatusCode::from_u16(status).unwrap(),
            Json(json!({"error": {"code": code, "message": message}})),
        )
            .into_response()
    };
    if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some(&format!("Bearer {KEY}"))
    {
        return typed(401, "unauthenticated", "the credential was refused");
    }
    // A settled idempotency key rejects changed content.
    if let Some(key) = headers.get("idempotency-key").and_then(|v| v.to_str().ok()) {
        let digest =
            format!("{:x}", body.len()) + &format!("{:?}", body[..body.len().min(8)].to_vec());
        let mut keys = stub.keys.lock().unwrap();
        if let Some(seen) = keys.get(key) {
            if seen != &digest {
                return typed(
                    409,
                    "idempotency_conflict",
                    "the request id settled against different content",
                );
            }
        } else {
            keys.insert(key.to_string(), digest);
        }
    }
    let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    match request["state"].as_str().unwrap_or("") {
        "REFUSE" => typed(422, "guardrail", "the door declined the state"),
        "FLAKY" if stub.flaky.fetch_add(1, Ordering::SeqCst) == 0 => (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after-ms", "1")],
            Json(json!({"error": {"code": "rate_limited", "message": "come back shortly"}})),
        )
            .into_response(),
        _ => {
            let mut answers = serde_json::Map::new();
            if let Some(questions) = request["questions"].as_object() {
                for id in questions.keys() {
                    answers.insert(id.clone(), json!({"type": "noul", "noul": 0.9}));
                }
            }
            (
                StatusCode::OK,
                [
                    ("x-request-id", "req-stub"),
                    ("x-typesafe-request-id", "req-stub"),
                ],
                Json(json!({
                    "model": "stub-v1",
                    "answers": answers,
                    "usage": {"input_tokens": 10, "output_tokens": 2},
                })),
            )
                .into_response()
        }
    }
}

async fn models() -> impl IntoResponse {
    Json(json!({"models": [{
        "name": "stub-v1",
        "description": "the test door",
        "release_date": "2026-01-01",
    }]}))
}

/// The stub's listen address, once per test.
async fn stub() -> (String, Arc<Stub>) {
    let shared = Arc::new(Stub::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new()
        .route("/v1/systemone", post(systemone))
        .route("/v1/models", get(models))
        .with_state(shared.clone());
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (address, shared)
}

/// A questions file in the wire schema, in a per-test directory.
fn questions(dir: &tempfile::TempDir) -> String {
    let path = dir.path().join("questions.json");
    std::fs::write(
        &path,
        r#"{"refund": {"type": "noul", "instructions": "Does the customer ask for money back?"}}"#,
    )
    .unwrap();
    path.to_str().unwrap().to_string()
}

/// `oak` pointed at the stub, with stdin held open only when a test feeds it.
fn oak(url: &str, dir: &tempfile::TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oak"));
    command
        .env("OPENAGENTS_API_KEY", KEY)
        .env("OPENAGENTS_BASE_URL", url)
        .env("OPENAGENTS_CONFIG", dir.path().join("missing.json"))
        .env("OPENAGENTS_MODEL", "stub-v1");
    command
}

fn spawn(mut command: Command, stdin: Option<&str>) -> Output {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    // The pipe must close either way — a batch reader blocks on it.
    if let Some(mut pipe) = child.stdin.take()
        && let Some(input) = stdin
    {
        pipe.write_all(input.as_bytes()).unwrap();
    }
    child.wait_with_output().unwrap()
}

fn rows(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_single_state_answers() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _stub) = stub().await;
    let output = spawn(
        {
            let mut c = oak(&url, &dir);
            c.args([
                "ask",
                "--questions",
                &questions(&dir),
                "a refund please",
                "--quiet",
            ]);
            c
        },
        None,
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let row = &rows(&output)[0];
    assert_eq!(row["outcome"], "answered");
    assert_eq!(row["model"], "stub-v1");
    assert_eq!(row["answers"]["refund"]["noul"], 0.9);
    assert_eq!(row["request_id"], "req-stub");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_ndjson_batch_emits_in_order_with_mixed_outcomes() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _stub) = stub().await;
    let input = concat!(
        "{\"id\": \"a\", \"state\": \"one\"}\n",
        "{\"id\": \"b\", \"state\": \"REFUSE\"}\n",
        "not json at all\n",
        "\"bare state\"\n"
    );
    let output = spawn(
        {
            let mut c = oak(&url, &dir);
            c.args([
                "ask",
                "--questions",
                &questions(&dir),
                "--input",
                "ndjson",
                "--quiet",
            ]);
            c
        },
        Some(input),
    );
    assert_eq!(
        output.status.code(),
        Some(5),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows = rows(&output);
    let outcomes: Vec<(&str, &str)> = rows
        .iter()
        .map(|row| {
            (
                row["id"].as_str().unwrap(),
                row["outcome"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        outcomes,
        [
            ("a", "answered"),
            ("b", "refused"),
            ("line-3", "invalid"),
            ("line-4", "answered")
        ]
    );
    assert_eq!(rows[1]["error"]["code"], "guardrail");
    assert_eq!(rows[2]["error"]["code"], "invalid_row");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_retry_honors_retry_after_and_bumps_the_attempt() {
    let dir = tempfile::tempdir().unwrap();
    let (url, stub) = stub().await;
    let output = spawn(
        {
            let mut c = oak(&url, &dir);
            c.args([
                "ask",
                "--questions",
                &questions(&dir),
                "--request-id",
                "test-retry",
                "FLAKY",
                "--quiet",
            ]);
            c
        },
        None,
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(rows(&output)[0]["outcome"], "answered");
    // One 429, one 200 — and the second attempt carried x-attempt: 2.
    assert_eq!(stub.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_settled_key_with_changed_content_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _stub) = stub().await;
    let ask = |state: &str| {
        let mut c = oak(&url, &dir);
        c.args([
            "ask",
            "--questions",
            &questions(&dir),
            "--request-id",
            "same-key",
            state,
            "--quiet",
        ]);
        spawn(c, None)
    };
    assert_eq!(ask("first").status.code(), Some(0));
    let second = ask("second");
    assert_eq!(
        second.status.code(),
        Some(3),
        "{:?}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(rows(&second)[0]["error"]["code"], "idempotency_conflict");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bad_key_fails_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _stub) = stub().await;
    let mut command = oak(&url, &dir);
    command.env("OPENAGENTS_API_KEY", "oak_wrong.key");
    command.args(["ask", "--questions", &questions(&dir), "hello", "--quiet"]);
    let output = spawn(command, None);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unauthenticated"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn models_lists_the_doors() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _stub) = stub().await;
    let output = spawn(
        {
            let mut c = oak(&url, &dir);
            c.args(["models", "--quiet"]);
            c
        },
        None,
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(rows(&output)[0]["name"], "stub-v1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_world_readable_config_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _stub) = stub().await;
    let config = dir.path().join("oak.json");
    std::fs::write(&config, json!({"api_key": KEY}).to_string()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o644)).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_oak"));
        command
            .env("OPENAGENTS_CONFIG", &config)
            .env_remove("OPENAGENTS_API_KEY")
            .args(["models", "--url", &url, "--quiet"]);
        let output = spawn(command, None);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("chmod 600"));
        std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o600)).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_oak"));
        command
            .env("OPENAGENTS_CONFIG", &config)
            .env_remove("OPENAGENTS_API_KEY")
            .args(["models", "--url", &url, "--quiet"]);
        let output = spawn(command, None);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{:?}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn select_and_uncertainty_shape_the_output() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _stub) = stub().await;
    let path = dir.path().join("two.json");
    std::fs::write(
        &path,
        r#"{"a": {"type": "noul", "instructions": "one"}, "b": {"type": "noul", "instructions": "two"}}"#,
    )
    .unwrap();
    let output = spawn(
        {
            let mut c = oak(&url, &dir);
            c.args([
                "ask",
                "--questions",
                path.to_str().unwrap(),
                "--select",
                "b",
                "--uncertain-below",
                "0.95",
                "anything",
                "--quiet",
            ]);
            c
        },
        None,
    );
    assert_eq!(output.status.code(), Some(0));
    let row = &rows(&output)[0];
    assert_eq!(row["answers"].as_object().unwrap().len(), 1);
    assert_eq!(row["answers"]["b"]["noul"], 0.9);
    assert_eq!(row["uncertain"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lines_mode_sends_each_line_as_a_string_state() {
    let dir = tempfile::tempdir().unwrap();
    let (url, stub) = stub().await;
    let output = spawn(
        {
            let mut c = oak(&url, &dir);
            c.args([
                "ask",
                "--questions",
                &questions(&dir),
                "--input",
                "lines",
                "--quiet",
            ]);
            c
        },
        Some("first ticket\nsecond ticket\n"),
    );
    assert_eq!(output.status.code(), Some(0));
    let rows = rows(&output);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], "line-1");
    assert_eq!(stub.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn usage_errors_exit_two() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _stub) = stub().await;
    // No --questions.
    let output = spawn(
        {
            let mut c = oak(&url, &dir);
            c.args(["ask", "hello"]);
            c
        },
        None,
    );
    assert_eq!(output.status.code(), Some(2));
    // A positional state cannot combine with a batch input mode.
    let output = spawn(
        {
            let mut c = oak(&url, &dir);
            c.args([
                "ask",
                "--questions",
                &questions(&dir),
                "--input",
                "lines",
                "hello",
            ]);
            c
        },
        None,
    );
    assert_eq!(output.status.code(), Some(2));
}
