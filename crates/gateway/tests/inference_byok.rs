//! Bring your own key on the inference API (#11067): a signed-in owner
//! seals their OpenRouter key, `pay: "mine"` goes only to it (with their
//! key on the wire, never ours), the key is never answered back or kept in
//! the clear, an API key cannot change it, and without a key `pay: "mine"`
//! is a plain `400`.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::http::StatusCode;
use inference::request::CreateResponse;
use inference::upstream::{
    Account, AttemptError, AttemptMeter, BoxFuture, Capabilities, CostBasis, EventStream, ModelRow,
    Price, PrivacyTerms, Sent, Upstream,
};
use serde_json::{Value, json};
use tenancy::Registry;

use gateway::config::{self, Config, Inference, SCHEMA};
use gateway::serve::{self, ServeState};

const MODEL: &str = "google/gemini-3.8-flash";

fn event(value: Value) -> inference::Event {
    serde_json::from_value(value).expect("event")
}

fn completed(model: &str) -> Value {
    let message = json!({"type": "message", "id": "msg_1", "status": "completed",
        "role": "assistant", "content": [{"type": "output_text", "text": "hello", "annotations": []}]});
    json!({"id": "resp_1", "object": "response", "created_at": 1, "status": "completed",
        "model": model, "output": [message],
        "usage": {"input_tokens": 10, "output_tokens": 2, "total_tokens": 12,
                  "input_tokens_details": {"cached_tokens": 0},
                  "output_tokens_details": {"reasoning_tokens": 0}}})
}

/// Our own upstream for the model, counting the calls it gets.
struct Ours {
    account: Account,
    privacy: PrivacyTerms,
    models: Vec<ModelRow>,
    calls: AtomicUsize,
}

impl Upstream for Ours {
    fn name(&self) -> &'static str {
        "openrouter"
    }
    fn account(&self) -> &Account {
        &self.account
    }
    fn privacy(&self) -> &PrivacyTerms {
        &self.privacy
    }
    fn models(&self) -> &[ModelRow] {
        &self.models
    }
    fn configured(&self) -> bool {
        true
    }
    fn send<'a>(
        &'a self,
        _request: &'a CreateResponse,
        model: &'a str,
    ) -> BoxFuture<'a, Result<Sent, AttemptError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let row = self.model(model).expect("model");
            let meter = AttemptMeter::start(self, row);
            let events: Vec<Result<inference::Event, AttemptError>> = vec![
                Ok(event(
                    json!({"type": "response.output_text.delta", "sequence_number": 0,
                    "item_id": "msg_1", "output_index": 0, "content_index": 0, "delta": "hello"}),
                )),
                Ok(event(
                    json!({"type": "response.completed", "sequence_number": 1,
                    "response": completed(model)}),
                )),
            ];
            let events: EventStream = Box::pin(futures_util::stream::iter(events));
            Ok(Sent {
                events: meter.wrap(events),
                meter,
            })
        })
    }
}

/// The caller's OpenRouter, as a local server: answers with a recorded
/// Open Responses stream and remembers the bearer it was sent.
async fn their_openrouter() -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let kept = seen.clone();
    let app = axum::Router::new().route(
        "/responses",
        axum::routing::post(move |headers: axum::http::HeaderMap| {
            let kept = kept.clone();
            async move {
                let bearer = headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or_default()
                    .to_owned();
                kept.lock().unwrap().push(bearer);
                // A recorded OpenRouter stream for the model.
                let frames =
                    include_str!("../../coder/fixtures/gateway/google-gemini-3.8-flash.sse");
                ([("content-type", "text/event-stream")], frames)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (address, seen)
}

struct Deployment {
    address: String,
    ours: Arc<Ours>,
    dir: tempfile::TempDir,
    _state: Arc<ServeState>,
}

async fn deploy() -> Deployment {
    let dir = tempfile::tempdir().unwrap();
    let registry =
        Registry::install(dir.path(), common::manifest(&common::artifact('a'), None)).unwrap();
    drop(registry);
    let secrets = tempfile::tempdir().unwrap();
    let keyring = secrets.path().join("keyring.json");
    let (_, document) = oa_seal::Keyring::scratch("k1").unwrap();
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&keyring)
            .unwrap();
        file.write_all(document.as_bytes()).unwrap();
    }
    // Keep the keyring's directory alive with the registry's.
    let keyring_dir = dir.path().join("keyring-dir");
    std::fs::rename(secrets.keep(), &keyring_dir).unwrap();
    let keyring = keyring_dir.join("keyring.json");
    let inference: Inference = serde_json::from_value(json!({
        "admin_token_env": "INFERENCE_BYOK_TEST_ADMIN",
        "public": {"free_tier": {"requests_per_day": 5, "models": [MODEL]}},
        "byok": {"keyring": keyring},
    }))
    .unwrap();
    let config = Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: false,
        team_policy: None,
        team_reports: None,
        inference: Some(inference),
        accounts: Some(config::Accounts {
            signup_tenant: Some("acme".to_string()),
            session_ttl_secs: 28_800,
            recovery_ttl_secs: 3_600,
            github: None,
            github_app: None,
            anonymous: None,
        }),
        billing: None,
        funding: None,
        earnings: None,
        commercial: None,
        skills: None,
        money: None,
        max_body_bytes: 1_048_576,
        max_response_bytes: 4_194_304,
        forward_timeout_ms: 10_000,
        classify_timeout_ms: None,
        max_tenant_classify_in_flight: None,
        reservation_ttl_secs: 300,
        max_in_flight: 8,
        max_classify_inputs: 1024,
        max_classify_inputs_per_tenant: 1024,
        max_questions: 256,
        cors_origins: vec![],
        max_options: 4096,
        doors: Default::default(),
        job_retention_ms: 604_800_000,
        job_cursor_ttl_ms: 3_600_000,
        public_origin: None,
    };
    let ours = Arc::new(Ours {
        account: Account {
            id: "openrouter".into(),
            basis: CostBasis::PayAsYouGo,
        },
        privacy: PrivacyTerms::zero_retention("test"),
        models: vec![ModelRow {
            id: MODEL.to_owned(),
            upstream_model: MODEL.to_owned(),
            capabilities: Capabilities {
                tools: true,
                reasoning: true,
                reasoning_always_on: false,
                json_schema: true,
                images: true,
                context: 1_000_000,
                max_output: 65_536,
            },
            price: Price::micro(0, 0, 0),
            price_source: "test",
        }],
        calls: AtomicUsize::new(0),
    });
    let state = ServeState::open_with_upstreams(config, Some(vec![ours.clone()])).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        address,
        ours,
        dir,
        _state: state,
    }
}

async fn send(
    d: &Deployment,
    method: reqwest::Method,
    path: &str,
    bearer: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .request(method, format!("{}{path}", d.address))
        .bearer_auth(bearer);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let answer = request.send().await.unwrap();
    let status = answer.status();
    let text = answer.text().await.unwrap();
    (
        status,
        serde_json::from_str(&text).unwrap_or(Value::String(text)),
    )
}

#[tokio::test]
async fn pay_mine_goes_only_to_the_callers_sealed_key() {
    let (theirs, seen) = their_openrouter().await;
    // SAFETY: the only test in this binary; set before any adapter is made.
    unsafe { std::env::set_var("OPENROUTER_BASE_URL", &theirs) };
    let d = deploy().await;
    let (status, joined) = send(
        &d,
        reqwest::Method::POST,
        "/v1/accounts",
        "",
        Some(json!({"label": "ada"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{joined}");
    let workspace = joined["workspace"]["id"].as_str().unwrap().to_owned();
    let key = joined["key_token"].as_str().unwrap().to_owned();
    let session = joined["session_token"].as_str().unwrap().to_owned();
    let mine = json!({"model": MODEL, "input": "hi", "openagents": {"pay": "mine"}});

    // No key of their own yet: a plain 400, and ours is not called.
    let (status, body) = send(
        &d,
        reqwest::Method::POST,
        "/v1/responses",
        &key,
        Some(mine.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["param"], "openagents.pay");
    assert_eq!(d.ours.calls.load(Ordering::SeqCst), 0);

    // An API key can't add a provider key; the signed-in owner can.
    let path = format!("/v1/workspaces/{workspace}/provider-keys/openrouter");
    let secret = json!({"key": "sk-or-v1-theirs"});
    let (status, _) = send(&d, reqwest::Method::PUT, &path, &key, Some(secret.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = send(&d, reqwest::Method::PUT, &path, &session, Some(secret)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["fingerprint"],
        model_access::fingerprint("sk-or-v1-theirs")
    );
    assert!(!body.to_string().contains("sk-or-v1-theirs"));
    let (_, listed) = send(
        &d,
        reqwest::Method::GET,
        &format!("/v1/workspaces/{workspace}/provider-keys"),
        &session,
        None,
    )
    .await;
    assert_eq!(listed["keys"][0]["provider"], "openrouter");
    assert!(!listed.to_string().contains("sk-or-v1-theirs"));
    let stored =
        std::fs::read_to_string(d.dir.path().join(gateway::inference_byok::STORE)).unwrap();
    assert!(!stored.contains("sk-or-v1-theirs"));

    // pay: mine now goes to their OpenRouter with their key, never ours.
    let answer = reqwest::Client::new()
        .post(format!("{}/v1/responses", d.address))
        .bearer_auth(&key)
        .json(&mine)
        .send()
        .await
        .unwrap();
    let id = answer.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    let body: Value = answer.json().await.unwrap();
    assert_eq!(body["status"], "completed", "{body}");

    assert_eq!(seen.lock().unwrap().as_slice(), ["Bearer sk-or-v1-theirs"]);
    assert_eq!(d.ours.calls.load(Ordering::SeqCst), 0);
    let (_, usage) = send(
        &d,
        reqwest::Method::GET,
        &format!("/v1/usage/{id}"),
        &key,
        None,
    )
    .await;
    assert_eq!(usage["payer"], "mine", "{usage}");
    assert_eq!(usage["charged"]["settlement"], "your_key");

    // Without pay: mine, their key stays unused (a free request on ours).
    let (status, _) = send(
        &d,
        reqwest::Method::POST,
        "/v1/responses",
        &key,
        Some(json!({"model": MODEL, "input": "hi"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(d.ours.calls.load(Ordering::SeqCst), 1);
    assert_eq!(seen.lock().unwrap().len(), 1);

    // Removed, pay: mine is refused again.
    let (status, _) = send(&d, reqwest::Method::DELETE, &path, &session, None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(&d, reqwest::Method::POST, "/v1/responses", &key, Some(mine)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
