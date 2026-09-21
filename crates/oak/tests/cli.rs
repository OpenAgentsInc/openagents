//! `oak` against a stub door over real TCP: answered, refused, retried,
//! conflicted, and unauthorized calls, plus the batch shapes.

use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};

/// The credential the stub accepts.
const KEY: &str = "oak_test.secret";

/// The workspace the stub accepts when it requires membership.
const WORKSPACE: &str = "ws_test";

/// What the stub counts and remembers.
#[derive(Default)]
struct Stub {
    calls: AtomicUsize,
    flaky: AtomicUsize,
    keys: Mutex<HashMap<String, String>>,
    /// When set, every route demands `X-Workspace-Id: ws_test`.
    require_workspace: AtomicBool,
    /// The workspace header each call carried, in arrival order.
    workspaces: Mutex<Vec<Option<String>>>,
    /// The `x-attempt` header each classify call carried.
    classify_attempts: Mutex<Vec<String>>,
    classify_keys: Mutex<Vec<Option<String>>>,
    /// The classify calls seen.
    classify_calls: AtomicUsize,
}

/// The typed refusal envelope the gateway writes.
fn typed(status: u16, code: &str, message: &str) -> Response {
    (
        StatusCode::from_u16(status).unwrap(),
        [("x-request-id", "req-stub")],
        Json(json!({"error": {"code": code, "message": message}})),
    )
        .into_response()
}

/// The shared admission check: the bearer key, then the workspace when
/// the stub requires membership.
fn admitted(stub: &Stub, headers: &HeaderMap) -> Option<Response> {
    stub.workspaces.lock().unwrap().push(
        headers
            .get("x-workspace-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string),
    );
    if headers.get("authorization").and_then(|v| v.to_str().ok()) != Some(&format!("Bearer {KEY}"))
    {
        return Some(typed(401, "unauthenticated", "the credential was refused"));
    }
    if stub.require_workspace.load(Ordering::SeqCst)
        && headers.get("x-workspace-id").and_then(|v| v.to_str().ok()) != Some(WORKSPACE)
    {
        return Some(typed(
            400,
            "workspace_required",
            "one X-Workspace-Id header is required",
        ));
    }
    None
}

async fn systemone(
    State(stub): State<Arc<Stub>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    stub.calls.fetch_add(1, Ordering::SeqCst);
    if let Some(refusal) = admitted(&stub, &headers) {
        return refusal;
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

async fn models(State(stub): State<Arc<Stub>>, headers: HeaderMap) -> Response {
    if let Some(refusal) = admitted(&stub, &headers) {
        return refusal;
    }
    Json(json!({"models": [{
        "name": "stub-v1",
        "description": "the test door",
        "release_date": "2026-01-01",
        "classification": {
            "supported": true,
            "max_inputs": 4,
            "max_labels": 8,
        },
    }]}))
    .into_response()
}

/// The answer probabilities each input id earns, by mode.
fn answered_unit(mode: &str, unit: &Value, id: &str) -> Value {
    let mut base = json!({"mode": mode, "outcome": "answered"});
    if let Some(dimension) = unit.get("dimension") {
        base["dimension"] = dimension.clone();
    }
    match mode {
        "single-label" => {
            let (probabilities, selected) = if id == "WEAK" {
                (json!({"a": 0.5, "b": 0.5}), Value::Null)
            } else {
                (json!({"a": 0.8, "b": 0.2}), json!("a"))
            };
            base["raw"] = json!({
                "type": "choice",
                "choice": "a",
                "confidence": 0.5,
                "probabilities": probabilities,
            });
            if selected.is_null() {
                base["no_match"] = json!(true);
            }
            base["selected"] = selected;
        }
        "multi-label" => {
            let mut raw = serde_json::Map::new();
            let mut selected = Vec::new();
            for label in unit["labels"].as_array().unwrap() {
                let label = label["id"].as_str().unwrap();
                let probability = match (id, label) {
                    ("NONE", _) => 0.1,
                    ("WEAK", _) => 0.5,
                    ("ONE", "b") => 0.2,
                    _ => 0.9,
                };
                raw.insert(
                    label.to_string(),
                    json!({"type": "noul", "noul": probability}),
                );
                if probability >= 0.5 && id != "WEAK" {
                    selected.push(json!(label));
                }
            }
            base["raw"] = Value::Object(raw);
            if selected.is_empty() {
                base["no_match"] = json!(true);
            }
            base["selected"] = Value::Array(selected);
        }
        "binary" => {
            let probability = if id == "NOPE" { 0.1 } else { 0.9 };
            base["raw"] = json!({"type": "noul", "noul": probability});
            if probability >= 0.5 {
                base["selected"] = unit["labels"][0]["id"].clone();
            } else {
                base["no_match"] = json!(true);
                base["selected"] = Value::Null;
            }
        }
        _ => {
            let score: f64 = match id {
                "HIGH" => 1.7,
                "LOW" => 0.3,
                _ => 1.0,
            };
            let levels = unit["levels"].as_array().map_or(3, |levels| levels.len());
            let probabilities: serde_json::Map<String, Value> = (0..levels)
                .map(|level| {
                    let weight = if level as f64 == score.round() {
                        0.8
                    } else {
                        0.1
                    };
                    (level.to_string(), json!(weight))
                })
                .collect();
            base["raw"] = json!({
                "type": "score",
                "score": score,
                "confidence": 0.9,
                "probabilities": probabilities,
                "legend": (0..levels).map(|level| level.to_string()).collect::<Vec<_>>(),
            });
            base["selected"] = json!(score.round() as u64);
        }
    }
    if id == "WEAK" {
        base["uncertain"] = json!(true);
    }
    base
}

/// One plan unit's report for an input, honoring the stub outcome ids.
fn unit_report(unit: &Value, id: &str, outcome: &str) -> Value {
    if outcome == "answered" {
        return answered_unit(unit["mode"].as_str().unwrap(), unit, id);
    }
    let mut failed = json!({
        "mode": unit["mode"],
        "outcome": outcome,
        "cause": format!("the {outcome} stub"),
        "selected": Value::Null,
    });
    if let Some(dimension) = unit.get("dimension") {
        failed["dimension"] = dimension.clone();
    }
    failed
}

/// The plan the request declared: the top-level unit or the dimensions.
fn plan_units(request: &Value) -> Vec<Value> {
    if let Some(dimensions) = request["dimensions"].as_array() {
        return dimensions
            .iter()
            .map(|dimension| {
                json!({
                    "mode": dimension["mode"],
                    "dimension": dimension["id"],
                    "labels": dimension.get("labels").cloned().unwrap_or(json!([])),
                    "levels": dimension.get("levels").cloned().unwrap_or(json!([])),
                })
            })
            .collect();
    }
    vec![json!({
        "mode": request["mode"],
        "labels": request.get("labels").cloned().unwrap_or(json!([])),
        "levels": request.get("levels").cloned().unwrap_or(json!([])),
    })]
}

/// The outcome an input id earns in the stub.
fn stub_outcome(id: &str, force: Option<&str>) -> &'static str {
    match force.or(match id {
        "REFUSE" => Some("refused"),
        "DROP" => Some("unavailable"),
        "PENDING" => Some("unattempted"),
        _ => None,
    }) {
        Some("refused") => "refused",
        Some("unavailable") => "unavailable",
        Some("unattempted") => "unattempted",
        _ => "answered",
    }
}

/// The classification document the gateway would return for the request.
fn classify_doc(request: &Value, force: Option<&str>) -> Value {
    let units = plan_units(request);
    let mut outcome_counts = serde_json::Map::new();
    let mut results = Vec::new();
    let mut per_unit_outcomes: Vec<serde_json::Map<String, Value>> =
        units.iter().map(|_| serde_json::Map::new()).collect();
    let mut per_unit_labels: Vec<serde_json::Map<String, Value>> =
        units.iter().map(|_| serde_json::Map::new()).collect();
    let mut per_unit_no_match = vec![0u64; units.len()];
    let mut per_unit_uncertain: Vec<Vec<Value>> = units.iter().map(|_| Vec::new()).collect();
    let mut binary_selected = Vec::new();
    let mut binary_unevaluated = Vec::new();
    let mut ranking: Vec<(String, f64)> = Vec::new();
    let mut score_unevaluated = Vec::new();
    let mut usage_complete = true;
    for input in request["inputs"].as_array().unwrap() {
        let id = input["id"].as_str().unwrap().to_string();
        let outcome = stub_outcome(&id, force);
        let entry = outcome_counts
            .entry(outcome.to_string())
            .or_insert(json!(0));
        *entry = json!(entry.as_u64().unwrap() + 1);
        let reports: Vec<Value> = units
            .iter()
            .map(|unit| unit_report(unit, &id, outcome))
            .collect();
        for (index, report) in reports.iter().enumerate() {
            let key = report["outcome"].as_str().unwrap().to_string();
            let entry = per_unit_outcomes[index].entry(key).or_insert(json!(0));
            *entry = json!(entry.as_u64().unwrap() + 1);
            if report["no_match"].as_bool().unwrap_or(false) {
                per_unit_no_match[index] += 1;
            }
            if report["uncertain"].as_bool().unwrap_or(false) {
                per_unit_uncertain[index].push(json!(id));
            }
            match report["mode"].as_str().unwrap() {
                "single-label" | "binary" => {
                    if let Some(label) = report["selected"].as_str() {
                        let entry = per_unit_labels[index]
                            .entry(label.to_string())
                            .or_insert(json!(0));
                        *entry = json!(entry.as_u64().unwrap() + 1);
                    }
                }
                "multi-label" => {
                    for label in report["selected"].as_array().into_iter().flatten() {
                        let entry = per_unit_labels[index]
                            .entry(label.as_str().unwrap().to_string())
                            .or_insert(json!(0));
                        *entry = json!(entry.as_u64().unwrap() + 1);
                    }
                }
                _ => {
                    if let Some(level) = report["selected"].as_u64() {
                        let entry = per_unit_labels[index]
                            .entry(level.to_string())
                            .or_insert(json!(0));
                        *entry = json!(entry.as_u64().unwrap() + 1);
                    }
                }
            }
            if report["mode"] == "binary" {
                if report["selected"].is_string() {
                    binary_selected.push(json!(id));
                } else {
                    binary_unevaluated.push(json!(id));
                }
            }
            if report["mode"] == "score" {
                match report["raw"]["score"].as_f64() {
                    Some(score) if outcome == "answered" => ranking.push((id.clone(), score)),
                    _ => score_unevaluated.push(json!(id)),
                }
            }
        }
        if outcome != "answered" && units.iter().any(|unit| unit["mode"] == "score") {
            score_unevaluated.push(json!(id));
        }
        if outcome != "answered" && units.iter().any(|unit| unit["mode"] == "binary") {
            binary_unevaluated.push(json!(id));
        }
        let usage = if id == "NOUSAGE" {
            usage_complete = false;
            Value::Null
        } else {
            json!({"input_tokens": 3, "output_tokens": 1})
        };
        results.push(json!({
            "input": id,
            "outcome": outcome,
            "units": reports,
            "latency_ms": 1,
            "model": "stub-v1",
            "usage": usage,
            "review_status": "not-reviewed",
        }));
    }
    ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let mut selections = Vec::new();
    if units.iter().any(|unit| unit["mode"] == "binary") {
        let label = units
            .iter()
            .find(|unit| unit["mode"] == "binary")
            .map(|unit| unit["labels"][0]["id"].clone())
            .unwrap_or(Value::Null);
        selections.push(json!({
            "mode": "binary",
            "label": label,
            "selected": binary_selected,
            "unevaluated": binary_unevaluated,
        }));
    }
    if units.iter().any(|unit| unit["mode"] == "score") {
        selections.push(json!({
            "mode": "score",
            "ranking": ranking.iter().map(|(id, _)| json!(id)).collect::<Vec<_>>(),
            "unevaluated": score_unevaluated,
        }));
    }
    let aggregates: Vec<Value> = units
        .iter()
        .enumerate()
        .map(|(index, unit)| {
            let mut aggregate = json!({"mode": unit["mode"], "outcomes": per_unit_outcomes[index]});
            if let Some(dimension) = unit.get("dimension") {
                aggregate["dimension"] = dimension.clone();
            }
            let counts_key = if unit["mode"] == "score" {
                "levels"
            } else {
                "labels"
            };
            aggregate[counts_key] = Value::Object(per_unit_labels[index].clone());
            if per_unit_no_match[index] > 0 {
                aggregate["no_match"] = json!(per_unit_no_match[index]);
            }
            if !per_unit_uncertain[index].is_empty() {
                aggregate["uncertain"] = Value::Array(per_unit_uncertain[index].clone());
            }
            aggregate
        })
        .collect();
    let outcome = match force {
        Some("refused") => "refused",
        _ => {
            if outcome_counts.len() == 1 && outcome_counts.contains_key("answered") {
                "answered"
            } else {
                "mixed"
            }
        }
    };
    let mut usage = json!({
        "forwards": results.len(),
        "input_tokens_complete": usage_complete,
        "output_tokens_complete": usage_complete,
    });
    if usage_complete {
        usage["input_tokens"] = json!(3 * results.len());
        usage["output_tokens"] = json!(results.len());
    }
    json!({
        "v": "openagents.classify.v1",
        "model": request["model"],
        "capacity": request["capacity"],
        "policy": {"v": request["policy"]["v"], "name": request["policy"]["name"]},
        "served": {"model": "stub-v1", "capacity": request["capacity"]},
        "outcome": outcome,
        "outcomes": outcome_counts,
        "results": results,
        "selections": selections,
        "aggregates": aggregates,
        "usage": usage,
        "timing": {"queued_ms": 0, "run_ms": 2},
    })
}

async fn classify(
    State(stub): State<Arc<Stub>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    stub.classify_calls.fetch_add(1, Ordering::SeqCst);
    stub.classify_keys.lock().unwrap().push(
        headers
            .get("idempotency-key")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
    );
    stub.classify_attempts.lock().unwrap().push(
        headers
            .get("x-attempt")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string(),
    );
    if let Some(refusal) = admitted(&stub, &headers) {
        return refusal;
    }
    let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    match request["model"].as_str().unwrap_or("") {
        "flaky-door" if stub.flaky.fetch_add(1, Ordering::SeqCst) == 0 => (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after-ms", "1")],
            Json(json!({"error": {"code": "rate_limited", "message": "come back shortly"}})),
        )
            .into_response(),
        "redirect" => (StatusCode::FOUND, [("location", "/v1/models")]).into_response(),
        "partial-503" => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(classify_doc(&request, None)),
        )
            .into_response(),
        "malformed" => Json(json!({"ok":true})).into_response(),
        "typed-refuse" => typed(422, "invalid_request", "the envelope is invalid"),
        "refused-door" => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(classify_doc(&request, Some("refused"))),
        )
            .into_response(),
        "slow-door" => {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            Json(classify_doc(&request, None)).into_response()
        }
        _ => Json(classify_doc(&request, None)).into_response(),
    }
}

/// The stub's listen address, once per test.
async fn stub() -> (String, Arc<Stub>) {
    let shared = Arc::new(Stub::default());
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

fn envelope(mode: &str, ids: &[&str]) -> Value {
    let mut request = json!({"v":"openagents.classify.v1","model":"stub-v1","capacity":"shared",
        "mode":mode,"policy":{"v":"openagents.classify-policy.v1","name":"fixture","select":{
            "single_label":{"ties":"no-match","min_probability":0.6,"uncertain_below":0.7,"no_match":{"kind":"null"}},
            "multi_label":{"threshold":0.6,"ties":"include-all","no_match":"empty","uncertain_below":0.7},
            "binary":{"threshold":0.6,"uncertain_below":0.7},
            "score":{"order":"descending","uncertain_below":0.7}
        }},
        "inputs":ids.iter().map(|id| json!({"id":id,"text":id})).collect::<Vec<_>>()});
    if mode == "score" {
        request["levels"] = json!(["low", "medium", "high"]);
    } else if mode == "binary" {
        request["labels"] = json!([{"id":"a","description":"A"}]);
    } else {
        request["labels"] = json!([{"id":"a","description":"A"},{"id":"b","description":"B"}]);
    }
    let parsed: gateway::classify::Request = serde_json::from_value(request.clone()).unwrap();
    parsed
        .plan(&gateway::classify::BackendLimits::product())
        .unwrap();
    request
}

fn mcp(url: &str, dir: &tempfile::TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oak-mcp"));
    command
        .env_clear()
        .env("OPENAGENTS_API_KEY", KEY)
        .env("OPENAGENTS_BASE_URL", url)
        .env("OPENAGENTS_CONFIG", dir.path().join("missing.json"));
    command
}

fn initialized() -> Vec<Value> {
    vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"fixture","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    ]
}

fn messages(values: &[Value]) -> String {
    values.iter().map(|value| format!("{value}\n")).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn classification_cli_and_mcp_preserve_all_gateway_fields() {
    let dir = tempfile::tempdir().unwrap();
    let (url, stub) = stub().await;
    stub.require_workspace.store(true, Ordering::SeqCst);
    for mode in ["single-label", "multi-label", "binary", "score"] {
        let request = envelope(
            mode,
            &["HIGH", "LOW", "NONE", "WEAK", "REFUSE", "DROP", "PENDING"],
        );
        let expected = classify_doc(&request, None);
        let mut command = oak(&url, &dir);
        command.args([
            "classify",
            "--envelope",
            "-",
            "--workspace",
            WORKSPACE,
            "--quiet",
        ]);
        let output = spawn(command, Some(&request.to_string()));
        assert_eq!(
            rows(&output),
            vec![expected.clone()],
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_ne!(
            output.status.code(),
            Some(0),
            "mixed output must not report all answered"
        );
        let mut input = initialized();
        input.push(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
        input.push(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"classify","arguments":{"request":request}}}));
        let mut command = mcp(&url, &dir);
        command.args(["--workspace", WORKSPACE]);
        let output = spawn(command, Some(&messages(&input)));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let replies = rows(&output);
        assert_eq!(replies.len(), 3);
        assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(replies[1]["result"]["tools"].as_array().unwrap().len(), 2);
        assert_eq!(replies[2]["result"]["structuredContent"], expected);
        let text: Value =
            serde_json::from_str(replies[2]["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        assert_eq!(text, expected);
    }
    assert!(
        stub.workspaces
            .lock()
            .unwrap()
            .iter()
            .all(|workspace| workspace.as_deref() == Some(WORKSPACE))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn classification_retries_timeout_refusals_and_input_bounds_are_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let (url, stub) = stub().await;
    let mut request = envelope("multi-label", &["ONE"]);
    request["model"] = json!("flaky-door");
    let path = dir.path().join("envelope.json");
    std::fs::write(&path, request.to_string()).unwrap();
    let mut command = oak(&url, &dir);
    command.args([
        "classify",
        "--envelope",
        path.to_str().unwrap(),
        "--request-id",
        "fixture-retry",
        "--quiet",
    ]);
    let output = spawn(command, None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(stub.classify_calls.load(Ordering::SeqCst), 2);
    let attempts = stub.classify_attempts.lock().unwrap().clone();
    assert_ne!(attempts[0], attempts[1]);
    assert_eq!(
        *stub.classify_keys.lock().unwrap(),
        vec![Some("fixture-retry".into()); 2]
    );
    for model in ["typed-refuse", "refused-door", "slow-door", "malformed"] {
        request["model"] = json!(model);
        let mut command = oak(&url, &dir);
        command.args([
            "classify",
            "--envelope",
            "-",
            "--timeout",
            "1",
            "--retries",
            "0",
            "--quiet",
        ]);
        let output = spawn(command, Some(&request.to_string()));
        assert!(!output.status.success(), "{model}");
        assert!(
            !output.stdout.is_empty(),
            "{model}: failure must retain a typed result"
        );
    }
    let before = stub.classify_calls.load(Ordering::SeqCst);
    for input in [
        "{".to_string(),
        " ".repeat(oak::MAX_ENVELOPE_BYTES as usize + 1),
    ] {
        let mut command = oak(&url, &dir);
        command.args(["classify", "--envelope", "-", "--quiet"]);
        assert_eq!(spawn(command, Some(&input)).status.code(), Some(2));
    }
    assert_eq!(stub.classify_calls.load(Ordering::SeqCst), before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_requires_initialization_and_rejects_caller_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let (url, stub) = stub().await;
    let mut input = vec![json!({"jsonrpc":"2.0","id":0,"method":"tools/list"})];
    input.extend(initialized());
    input.push(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"classify","arguments":{"request":envelope("binary", &["ONE"]),"api_key":"untrusted"}}}));
    input.push(json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_models","arguments":{}}}));
    let output = spawn(mcp(&url, &dir), Some(&messages(&input)));
    let replies = rows(&output);
    assert!(replies[0]["error"].is_object());
    assert!(replies[2]["error"].is_object());
    assert!(replies[3]["result"]["structuredContent"].is_object());
    assert_eq!(stub.classify_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn classification_never_replays_a_partial_report_or_follows_a_redirect() {
    let dir = tempfile::tempdir().unwrap();
    let (url, stub) = stub().await;
    for model in ["partial-503", "redirect"] {
        let mut request = envelope("multi-label", &["ONE", "DROP"]);
        request["model"] = json!(model);
        let before = stub.workspaces.lock().unwrap().len();
        let mut command = oak(&url, &dir);
        command.args([
            "classify",
            "--envelope",
            "-",
            "--request-id",
            model,
            "--retries",
            "3",
            "--quiet",
        ]);
        let output = spawn(command, Some(&request.to_string()));
        assert!(!output.status.success());
        assert_eq!(stub.workspaces.lock().unwrap().len(), before + 1);
        if model == "partial-503" {
            assert_eq!(rows(&output), vec![classify_doc(&request, None)]);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_validates_handshake_ids_and_negotiates_a_supported_version() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _) = stub().await;
    let mut initialize = initialized().remove(0);
    initialize["params"]["protocolVersion"] = json!("unknown-version");
    let input = vec![
        json!({"jsonrpc":"2.0","id":{},"method":"ping"}),
        json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}),
        initialize,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":3,"method":"notifications/unknown"}),
    ];
    let replies = rows(&spawn(mcp(&url, &dir), Some(&messages(&input))));
    assert_eq!(replies.len(), 4);
    assert_eq!(replies[0]["error"]["code"], -32600);
    assert_eq!(replies[1]["error"]["code"], -32602);
    assert_eq!(replies[2]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(replies[3]["error"]["code"], -32601);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_caller_route_sends_the_resolved_workspace() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let (url, stub) = stub().await;
    stub.require_workspace.store(true, Ordering::SeqCst);
    let config = dir.path().join("config.json");
    std::fs::write(&config, json!({"workspace":WORKSPACE}).to_string()).unwrap();
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o600)).unwrap();
    for source in ["flag", "environment", "file"] {
        for verb in ["ask", "models", "classify"] {
            let mut command = oak(&url, &dir);
            command.arg(verb).arg("--quiet");
            match verb {
                "ask" => {
                    command.args(["--questions", &questions(&dir), "fixture"]);
                }
                "classify" => {
                    command.args(["--envelope", "-"]);
                }
                _ => {}
            }
            match source {
                "flag" => {
                    command
                        .env("OPENAGENTS_WORKSPACE", "wrong")
                        .args(["--workspace", WORKSPACE]);
                }
                "environment" => {
                    command.env("OPENAGENTS_WORKSPACE", WORKSPACE);
                }
                _ => {
                    command
                        .env_remove("OPENAGENTS_WORKSPACE")
                        .args(["--config", config.to_str().unwrap()]);
                }
            }
            let output = spawn(
                command,
                (verb == "classify")
                    .then(|| envelope("multi-label", &["ONE"]).to_string())
                    .as_deref(),
            );
            assert!(
                output.status.success(),
                "{verb}/{source}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    assert_eq!(stub.workspaces.lock().unwrap().len(), 9);
    assert!(
        stub.workspaces
            .lock()
            .unwrap()
            .iter()
            .all(|workspace| workspace.as_deref() == Some(WORKSPACE))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_retains_auth_errors_and_recovers_after_an_oversized_message() {
    let dir = tempfile::tempdir().unwrap();
    let (url, _) = stub().await;
    let mut input = initialized();
    input.push(
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_models"}}),
    );
    let mut command = mcp(&url, &dir);
    command.env("OPENAGENTS_API_KEY", "oak_invalid.fixture");
    let replies = rows(&spawn(command, Some(&messages(&input))));
    assert_eq!(replies[1]["result"]["isError"], true);
    assert_eq!(replies[1]["result"]["structuredContent"]["status"], 401);
    let mut input = "x".repeat(oak::MAX_MCP_MESSAGE_BYTES as usize + 1);
    input.push('\n');
    input.push_str(&messages(&initialized()));
    let replies = rows(&spawn(mcp(&url, &dir), Some(&input)));
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0]["error"]["code"], -32600);
    assert!(replies[1]["result"]["serverInfo"].is_object());
}
