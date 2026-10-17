//! `computer cloud` against a scripted fake of the public Computers API,
//! the same shape as `openagents-sdk`'s own fixture: each HTTP attempt
//! takes the next scripted reply and is recorded.

use super::*;
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::Request,
    response::{IntoResponse, Response},
};
use openagents_sdk::{Client, Deployment, Session, computers::RetryPolicy};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

const ID: &str = "cmp_0123456789abcdef0123";
const COMMAND: &str = "trn_0123456789abcdef0123";

fn computer(state: &str) -> Value {
    json!({"id":ID,"kind":"cloud","name":"test","size":"small","state":state,
        "desired":if state=="stopped" {"stopped"} else {"running"},"online":state=="awake"})
}
fn command(state: &str) -> Value {
    json!({"id":COMMAND,"computer":ID,"state":state,"exit_code":if state=="succeeded" {Some(0)} else {None},
        "digest":"a".repeat(64),"cancel_requested":state=="cancelled","output_limit_bytes":65536,"output_expires_unix":2000000000,
        "stdout_truncated":false,"stderr_truncated":false})
}
fn refusal(code: &str) -> Value {
    json!({"error":{"code":code,"message":"refused"}})
}

struct Step {
    status: u16,
    body: Vec<u8>,
    delay: Duration,
}
impl Step {
    fn json(status: u16, value: Value) -> Self {
        Self {
            status,
            body: serde_json::to_vec(&value).unwrap(),
            delay: Duration::ZERO,
        }
    }
    /// A reply that arrives after the client stopped waiting: a lost answer.
    fn lost(mut self) -> Self {
        self.delay = Duration::from_secs(2);
        self
    }
}
#[derive(Clone, Debug)]
struct Seen {
    method: String,
    path: String,
    key: Option<String>,
    body: Vec<u8>,
}
#[derive(Clone)]
struct Script {
    steps: Arc<Mutex<VecDeque<Step>>>,
    seen: Arc<Mutex<Vec<Seen>>>,
}
async fn serve(State(script): State<Script>, request: Request<Body>) -> Response {
    if request.uri().path() == "/v1/account" {
        return axum::Json(json!({"account":{"id":"acct_fixture","label":"Test"},"workspaces":[]}))
            .into_response();
    }
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, 5 * 1024 * 1024).await.unwrap().to_vec();
    script.seen.lock().unwrap().push(Seen {
        method: parts.method.to_string(),
        path: parts.uri.path().to_owned(),
        key: parts
            .headers
            .get("idempotency-key")
            .map(|v| v.to_str().unwrap().to_owned()),
        body,
    });
    let step = script
        .steps
        .lock()
        .unwrap()
        .pop_front()
        .expect("unexpected HTTP attempt");
    tokio::time::sleep(step.delay).await;
    Response::builder()
        .status(step.status)
        .body(Body::from(step.body))
        .unwrap()
}

struct Fake {
    client: Client,
    session: Session,
    script: Script,
    dir: tempfile::TempDir,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fake {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Fake {
    async fn new(steps: Vec<Step>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let script = Script {
            steps: Arc::new(Mutex::new(steps.into())),
            seen: Arc::new(Mutex::new(vec![])),
        };
        let router = Router::new().fallback(serve).with_state(script.clone());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let deployment = Deployment::new(&format!("{origin}/v1"), &origin).unwrap();
        let session = Client::new(deployment.clone())
            .unwrap()
            .authenticate(&format!("sess_{}", "a".repeat(64)))
            .await
            .unwrap();
        // Wide enough that a prompt reply never looks lost under a loaded
        // test run; `Step::lost` replies well after it.
        let timeout = Duration::from_secs(1);
        let client = Client::with_timeouts(deployment, timeout, timeout).unwrap();
        Self {
            client,
            session,
            script,
            dir: tempfile::tempdir().unwrap(),
            task,
        }
    }
    fn push(&self, step: Step) {
        self.script.steps.lock().unwrap().push_back(step);
    }
    fn seen(&self) -> Vec<Seen> {
        self.script.seen.lock().unwrap().clone()
    }
    /// One CLI run: a fresh client each time, as after a restart.
    async fn run(&self, words: &[&str]) -> Result<Value, Failure> {
        let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
        let args = Args::parse(&words[1..], SWITCHES).unwrap();
        let api = self
            .client
            .computers(&self.session)
            .unwrap()
            .with_retries(RetryPolicy::none())
            .unwrap();
        let context = Context {
            api,
            requests: self.dir.path().to_owned(),
            cancel: CancellationToken::new(),
        };
        dispatch(&context, &words[0], &args).await
    }
}

#[tokio::test]
async fn a_lost_create_answer_is_retried_with_the_same_key_and_never_makes_a_second_computer() {
    let fake = Fake::new(vec![Step::json(202, computer("creating")).lost()]).await;
    let lost = fake.run(&["create", "--size", "small"]).await.unwrap_err();
    assert_eq!(lost.kind, "uncertain");
    let key = lost.key.clone().expect("the request key to retry with");
    assert!(lost.message.contains(&format!("--key {key}")));
    // The request was saved before it was sent, privately and without the session.
    let saved =
        std::fs::read_to_string(fake.dir.path().join(format!("create-{key}.json"))).unwrap();
    assert!(!saved.contains("sess_"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(fake.dir.path().join(format!("create-{key}.json")))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    // A changed setting under the same key is refused before anything is sent.
    let changed = fake
        .run(&["create", "--size", "large", "--key", &key])
        .await
        .unwrap_err();
    assert_eq!(changed.kind, "key_conflict");
    assert_eq!(fake.seen().len(), 1);
    // The retry sends the saved request: same key, same bytes.
    fake.push(Step::json(200, computer("creating")));
    let created = fake.run(&["create", "--key", &key]).await.unwrap();
    assert_eq!(created["computer"]["id"], ID);
    assert_eq!(created["key"], key.as_str());
    let seen = fake.seen();
    assert_eq!(seen.len(), 2);
    for attempt in &seen {
        assert_eq!(
            (attempt.method.as_str(), attempt.path.as_str()),
            ("POST", "/v1/computers")
        );
        assert_eq!(attempt.key.as_deref(), Some(key.as_str()));
        assert_eq!(attempt.body, seen[0].body);
    }
}

#[tokio::test]
async fn a_lost_exec_answer_reaches_the_same_command_and_waiting_never_cancels_it() {
    let fake = Fake::new(vec![Step::json(202, command("queued")).lost()]).await;
    let lost = fake
        .run(&["exec", ID, "--wait", "0", "--", "cargo", "test"])
        .await
        .unwrap_err();
    assert_eq!(lost.kind, "uncertain");
    let key = lost.key.unwrap();
    fake.push(Step::json(200, command("running")));
    for _ in 0..8 {
        fake.push(Step::json(200, command("running")));
    }
    let detached = fake
        .run(&["exec", ID, "--key", &key, "--wait", "1"])
        .await
        .unwrap();
    assert_eq!(detached["running"], true);
    assert_eq!(detached["command"]["id"], COMMAND);
    let seen = fake.seen();
    let creates: Vec<_> = seen.iter().filter(|s| s.method == "POST").collect();
    assert_eq!(creates.len(), 2, "two attempts of one command");
    assert_eq!(creates[0].body, creates[1].body);
    assert!(
        creates
            .iter()
            .all(|s| s.key.as_deref() == Some(key.as_str()))
    );
    assert!(String::from_utf8_lossy(&creates[0].body).contains("cargo test"));
    assert!(
        !seen.iter().any(|s| s.path.ends_with("/cancel")),
        "a wait that ends never cancels the command"
    );
    // Cancelling is its own explicit request.
    fake.script.steps.lock().unwrap().clear();
    fake.push(Step::json(202, command("cancelled")));
    let cancelled = fake.run(&["cancel", ID, COMMAND]).await.unwrap();
    assert_eq!(cancelled["cancel_requested"], true);
    let last = fake.seen().pop().unwrap();
    assert_eq!(
        last.path,
        format!("/v1/computers/{ID}/commands/{COMMAND}/cancel")
    );
}

#[tokio::test]
async fn a_finished_command_prints_its_output() {
    let fake = Fake::new(vec![
        Step::json(202, command("succeeded")),
        Step::json(
            200,
            json!({"command":command("succeeded"),"encoding":"base64",
            "stdout":"aGVsbG8=","stderr":"","next_cursor":{"stdout":5,"stderr":0},
            "gaps":{"stdout":false,"stderr":false},"terminal":true,"has_more":false}),
        ),
    ])
    .await;
    let done = fake.run(&["exec", ID, "--", "echo hello"]).await.unwrap();
    assert_eq!(done["outcome"], "succeeded");
    assert_eq!(done["stdout"], "hello");
}

#[tokio::test]
async fn a_placement_the_service_cannot_meet_refuses_and_says_nothing_started() {
    let fake = Fake::new(vec![Step::json(422, refusal("unsupported"))]).await;
    let refused = fake
        .run(&["create", "--placement", "fast-sandbox"])
        .await
        .unwrap_err();
    assert_eq!(refused.kind, "unsupported");
    assert!(refused.message.contains("nothing was started"));
    assert!(
        String::from_utf8_lossy(&fake.seen()[0].body).contains("\"placement\":\"fast-sandbox\"")
    );
    // A placement outside the public ones never reaches the service.
    let unknown = fake
        .run(&["create", "--placement", "boat"])
        .await
        .unwrap_err();
    assert_eq!(unknown.kind, "usage");
    assert_eq!(fake.seen().len(), 1);
}

#[tokio::test]
async fn file_transfers_on_a_computer_without_them_say_so() {
    let fake = Fake::new(vec![Step::json(
        200,
        json!({"schema":"openagents.computers.capabilities.v1","profiles":["core"],"sizes":["small"],"limits":{"max_computers":20}}),
    )])
    .await;
    let out = fake.dir.path().join("copy.txt");
    let refused = fake
        .run(&["read", ID, "notes.txt", "--out", out.to_str().unwrap()])
        .await
        .unwrap_err();
    assert_eq!(refused.kind, "unsupported");
    assert!(refused.message.contains("doesn't offer file transfers"));
    assert!(!out.exists());
}

#[tokio::test]
async fn an_expired_sign_in_asks_to_log_in_again() {
    let fake = Fake::new(vec![Step::json(401, refusal("unauthorized"))]).await;
    let refused = fake.run(&["show", ID]).await.unwrap_err();
    assert_eq!(refused.kind, "signed_out");
    assert!(refused.message.contains("coder login"));
}

#[tokio::test]
async fn ids_and_waits_are_checked_before_anything_is_sent() {
    let fake = Fake::new(vec![]).await;
    assert_eq!(
        fake.run(&["show", "sbx_1"]).await.unwrap_err().kind,
        "usage"
    );
    assert_eq!(
        fake.run(&["wake", ID, "--wait", "100000"])
            .await
            .unwrap_err()
            .kind,
        "usage"
    );
    assert_eq!(fake.run(&["exec", ID]).await.unwrap_err().kind, "usage");
    assert!(fake.seen().is_empty());
}

#[test]
fn shell_words_are_quoted_only_when_needed() {
    assert_eq!(shell_word("cargo"), "cargo");
    assert_eq!(shell_word("a b"), "'a b'");
    assert_eq!(shell_word("it's"), "'it'\\''s'");
}
