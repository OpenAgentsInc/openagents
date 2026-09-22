//! The shared contract fixtures under `docs/decision-models/fixtures/`,
//! replayed through `oak` against a stub that serves each fixture's
//! recorded response verbatim.
//!
//! Coverage is per client surface: `oak` replays every fixture whose
//! route it speaks — `/v1/models` and `/v1/classify` — and the table
//! names the rest `jev`'s so a fixture is never silently skipped. `jev`'s
//! own replay lives in `crates/jev/tests/contract.rs`.

use std::process::{Command, Output};

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

/// The credential the run presents — the stub serves the fixture's
/// response regardless of what the request carried.
const KEY: &str = "oak_fixture.secret";

/// The directory the shared fixtures live in.
const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/decision-models/fixtures"
);

/// One loaded fixture.
struct Fixture {
    method: String,
    path: String,
    body: Option<Vec<u8>>,
    status: u16,
    headers: HeaderMap,
    response: Vec<u8>,
}

/// Load one fixture's recorded exchange.
fn load(name: &str) -> Fixture {
    let path = format!("{FIXTURES}/{name}.json");
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}")),
    )
    .unwrap();
    let mut headers = HeaderMap::new();
    for (name, value) in fixture["response"]["headers"].as_object().unwrap() {
        headers.insert(
            name.parse::<axum::http::header::HeaderName>().unwrap(),
            HeaderValue::from_str(value.as_str().unwrap()).unwrap(),
        );
    }
    Fixture {
        method: fixture["request"]["method"].as_str().unwrap().to_string(),
        path: fixture["request"]["path"].as_str().unwrap().to_string(),
        body: fixture["request"]
            .get("body")
            .map(|body| serde_json::to_vec(body).unwrap()),
        status: fixture["response"]["status"].as_u64().unwrap() as u16,
        headers,
        response: serde_json::to_vec(&fixture["response"]["body"]).unwrap(),
    }
}

/// The stub's route table — one recorded exchange per fixture.
#[derive(Clone, Default)]
struct Stub {
    routes: Vec<(String, String, u16, HeaderMap, Vec<u8>)>,
}

async fn serve(State(stub): State<Stub>, request: Request) -> Response {
    let path = request.uri().path();
    let method = request.method().as_str();
    for (route_path, route_method, status, headers, body) in &stub.routes {
        if route_path == path && route_method == method {
            let mut response = Response::builder().status(StatusCode::from_u16(*status).unwrap());
            for (name, value) in headers {
                response = response.header(name, value);
            }
            return response.body(body.clone().into()).unwrap();
        }
    }
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(
            serde_json::to_vec(
                &json!({"error": {"code": "not_found", "message": "no fixture on this route"}}),
            )
            .unwrap()
            .into(),
        )
        .unwrap()
}

/// Start the stub with the given fixtures loaded.
async fn start(fixtures: &[&Fixture]) -> String {
    let routes = fixtures
        .iter()
        .map(|f| {
            (
                f.path.clone(),
                f.method.clone(),
                f.status,
                f.headers.clone(),
                f.response.clone(),
            )
        })
        .collect();
    let app = Router::new().fallback(serve).with_state(Stub { routes });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

/// One `oak` verb against the stub.
fn oak(url: &str, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oak"));
    command
        .env("OPENAGENTS_API_KEY", KEY)
        .env("OPENAGENTS_BASE_URL", url)
        .env("OPENAGENTS_CONFIG", "/nonexistent/oak.json")
        .args(args);
    command.output().unwrap()
}

/// The revocation fixture on `GET /v1/models`: the CLI surfaces the
/// typed code and fails — revocation is not a retry's business.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn key_revocation_refuses() {
    let fixture = load("key-revocation");
    let url = start(&[&fixture]).await;
    let output = oak(&url, &["models"]);
    assert_eq!(output.status.code(), Some(1), "{:?}", output);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unauthenticated"),
        "the typed code must reach the caller: {stderr}"
    );
}

/// The partial-failure fixture on `POST /v1/classify`: the report is a
/// success the caller reads — `mixed`, each outcome counted, causes
/// named — never an exception.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn classify_partial_failure_reads() {
    let fixture = load("classify-partial-failure");
    let url = start(&[&fixture]).await;
    let dir = tempfile::tempdir().unwrap();
    let envelope = dir.path().join("envelope.json");
    std::fs::write(&envelope, fixture.body.as_ref().unwrap()).unwrap();
    let output = oak(
        &url,
        &["classify", "--envelope", envelope.to_str().unwrap()],
    );
    assert_eq!(output.status.code(), Some(5), "{:?}", output);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["outcome"], "mixed");
    assert_eq!(report["outcomes"]["answered"], 1);
    assert_eq!(report["outcomes"]["refused"], 1);
    assert_eq!(report["outcomes"]["unavailable"], 1);
    let causes: Vec<&str> = report["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["cause"].as_str())
        .collect();
    assert!(!causes.is_empty(), "unanswered work names its cause");
}

/// The null-confidence fixture: a reviewer's `confidence` can be null
/// and the report still stands — the CLI passes the document through
/// and answers.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn classify_review_null_confidence_answers() {
    let fixture = load("classify-review-null-confidence");
    let url = start(&[&fixture]).await;
    let dir = tempfile::tempdir().unwrap();
    let envelope = dir.path().join("envelope.json");
    std::fs::write(&envelope, fixture.body.as_ref().unwrap()).unwrap();
    let output = oak(
        &url,
        &["classify", "--envelope", envelope.to_str().unwrap()],
    );
    assert_eq!(output.status.code(), Some(0), "{:?}", output);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["outcome"], "answered");
    assert!(
        report["results"][0]["units"][0]["review"]["raw"]["confidence"].is_null(),
        "the null confidence survives verbatim"
    );
}

/// The jobs fixture covers a route `oak` does not speak — the test
/// names that coverage decision rather than skipping silently, and
/// `jev`'s contract test replays it.
#[test]
fn jobs_fixture_is_jevs() {
    let fixture = load("jobs-idempotency-conflict");
    assert_eq!(fixture.path, "/v1/jobs");
    assert_eq!(fixture.method, "POST");
}
