//! Bring your own key on the inference API (#11067): a signed-in owner
//! seals their OpenRouter key, `pay: "mine"` goes only to it (with their
//! key on the wire, never ours), the key is never answered back or kept in
//! the clear, an API key cannot change it, and without a key `pay: "mine"`
//! is a plain `400`.
//!
//! And #11186: every personal workspace made by sign-up shares one registry
//! tenant, so nothing a person keeps is kept by that tenant. Two accounts
//! signed up on it never see or use each other's provider keys, free
//! requests, API keys, usage, or stored responses.

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

/// The callers' OpenRouter, as a local server shared by the tests here:
/// answers with a recorded Open Responses stream and remembers each
/// bearer it was sent. `OPENROUTER_BASE_URL` points at it, set once before
/// any adapter reads it.
fn their_openrouter() -> Arc<std::sync::Mutex<Vec<String>>> {
    static THEIRS: std::sync::OnceLock<Arc<std::sync::Mutex<Vec<String>>>> =
        std::sync::OnceLock::new();
    THEIRS
        .get_or_init(|| {
            let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
            let kept = seen.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                runtime.block_on(async move {
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
                                let frames = include_str!(
                                    "../../coder/fixtures/gateway/google-gemini-3.8-flash.sse"
                                );
                                ([("content-type", "text/event-stream")], frames)
                            }
                        }),
                    );
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    sender
                        .send(format!("http://{}", listener.local_addr().unwrap()))
                        .unwrap();
                    axum::serve(listener, app).await.unwrap();
                });
            });
            let address = receiver.recv().unwrap();
            // SAFETY: set once, before any test in this binary makes an
            // adapter that reads it.
            unsafe { std::env::set_var("OPENROUTER_BASE_URL", &address) };
            seen
        })
        .clone()
}

/// How many calls reached their OpenRouter with `key`.
fn calls_with(seen: &std::sync::Mutex<Vec<String>>, key: &str) -> usize {
    let bearer = format!("Bearer {key}");
    seen.lock()
        .unwrap()
        .iter()
        .filter(|b| **b == bearer)
        .count()
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
        "store": {"retention_days": 30, "key_env": "INFERENCE_BYOK_TEST_UNSET_KEY"},
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
            open_signup: true,
            operator_signup_token_env: None,
            session_ttl_secs: 28_800,
            recovery_ttl_secs: 3_600,
            github: None,
            github_app: None,
            invite_only: None,
            store: Default::default(),
            database_url_env: String::new(),
            import_files: false,
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
    let seen = their_openrouter();
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

    // Direct providers use the same owner-only, sealed-key routes.
    for provider in ["anthropic", "openai", "google"] {
        let direct = format!("/v1/workspaces/{workspace}/provider-keys/{provider}");
        let input = json!({"key": "stub-credential"});
        let (status, _) = send(&d, reqwest::Method::PUT, &direct, &key, Some(input.clone())).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, body) = send(&d, reqwest::Method::PUT, &direct, &session, Some(input)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["provider"], provider);
        assert!(!body.to_string().contains("stub-credential"));
        let (status, _) = send(&d, reqwest::Method::DELETE, &direct, &session, None).await;
        assert_eq!(status, StatusCode::OK);
    }

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

    assert_eq!(calls_with(&seen, "sk-or-v1-theirs"), 1);
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
    assert_eq!(calls_with(&seen, "sk-or-v1-theirs"), 1);

    // Removed, pay: mine is refused again.
    let (status, _) = send(&d, reqwest::Method::DELETE, &path, &session, None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(&d, reqwest::Method::POST, "/v1/responses", &key, Some(mine)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// One account signed up on the shared tenant (`acme` here, `signup` on
/// staging): (workspace, API key, session).
async fn sign_up(d: &Deployment, label: &str) -> (String, String, String) {
    let (status, joined) = send(
        d,
        reqwest::Method::POST,
        "/v1/accounts",
        "",
        Some(json!({"label": label})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{joined}");
    assert_eq!(joined["workspace"]["tenant"], "acme");
    (
        joined["workspace"]["id"].as_str().unwrap().to_owned(),
        joined["key_token"].as_str().unwrap().to_owned(),
        joined["session_token"].as_str().unwrap().to_owned(),
    )
}

/// The request id of a `POST /v1/responses`, and its body.
async fn respond(d: &Deployment, key: &str, body: Value) -> (StatusCode, String, Value) {
    let answer = reqwest::Client::new()
        .post(format!("{}/v1/responses", d.address))
        .bearer_auth(key)
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = answer.status();
    let id = answer
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let text = answer.text().await.unwrap();
    (
        status,
        id,
        serde_json::from_str(&text).unwrap_or(Value::String(text)),
    )
}

/// The regression for #11186: two people on the one sign-up tenant.
#[tokio::test]
async fn accounts_on_one_tenant_never_see_or_use_each_others_keys_or_state() {
    let seen = their_openrouter();
    let d = deploy().await;
    let (ada_ws, ada_key, ada_session) = sign_up(&d, "ada").await;
    let (bo_ws, bo_key, bo_session) = sign_up(&d, "bo").await;
    assert_ne!(ada_ws, bo_ws);
    let mine = json!({"model": MODEL, "input": "hi", "openagents": {"pay": "mine"}});
    let put = |ws: &str| format!("/v1/workspaces/{ws}/provider-keys/openrouter");
    let list = |ws: &str| format!("/v1/workspaces/{ws}/provider-keys");

    // Ada saves her key. Bo sees none, and his pay: mine has no key.
    let (status, _) = send(
        &d,
        reqwest::Method::PUT,
        &put(&ada_ws),
        &ada_session,
        Some(json!({"key": "sk-or-v1-ada"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, listed) = send(&d, reqwest::Method::GET, &list(&bo_ws), &bo_session, None).await;
    assert_eq!(listed["keys"], json!([]), "{listed}");
    let (status, _, body) = respond(&d, &bo_key, mine.clone()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["param"], "openagents.pay");
    assert_eq!(calls_with(&seen, "sk-or-v1-ada"), 0);

    // Bo can't read, replace or remove Ada's.
    let (status, _) = send(&d, reqwest::Method::GET, &list(&ada_ws), &bo_session, None).await;
    assert!(status.is_client_error(), "{status}");
    let (status, _) = send(
        &d,
        reqwest::Method::PUT,
        &put(&ada_ws),
        &bo_session,
        Some(json!({"key": "sk-or-v1-bo"})),
    )
    .await;
    assert!(status.is_client_error(), "{status}");
    let (status, _) = send(
        &d,
        reqwest::Method::DELETE,
        &put(&ada_ws),
        &bo_session,
        None,
    )
    .await;
    assert!(status.is_client_error(), "{status}");

    // Bo saves his own: Ada's stays hers, and each pay: mine goes out on
    // the caller's own key only.
    let (status, _) = send(
        &d,
        reqwest::Method::PUT,
        &put(&bo_ws),
        &bo_session,
        Some(json!({"key": "sk-or-v1-bo"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, listed) = send(&d, reqwest::Method::GET, &list(&ada_ws), &ada_session, None).await;
    assert_eq!(
        listed["keys"][0]["fingerprint"],
        model_access::fingerprint("sk-or-v1-ada")
    );
    assert_eq!(listed["keys"].as_array().unwrap().len(), 1);
    let (status, ada_mine, body) = respond(&d, &ada_key, mine.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            calls_with(&seen, "sk-or-v1-ada"),
            calls_with(&seen, "sk-or-v1-bo")
        ),
        (1, 0)
    );
    let (status, _, body) = respond(&d, &bo_key, mine.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            calls_with(&seen, "sk-or-v1-ada"),
            calls_with(&seen, "sk-or-v1-bo")
        ),
        (1, 1)
    );
    assert_eq!(d.ours.calls.load(Ordering::SeqCst), 0);
    // Ada removing hers leaves Bo's.
    let (status, _) = send(
        &d,
        reqwest::Method::DELETE,
        &put(&ada_ws),
        &ada_session,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = respond(&d, &bo_key, mine.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = respond(&d, &ada_key, mine).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Free requests are counted per workspace, not per tenant.
    let free = json!({"model": MODEL, "input": "hi", "store": true});
    let (status, ada_free, stored) = respond(&d, &ada_key, free).await;
    assert_eq!(status, StatusCode::OK, "{stored}");
    let (_, ada_view) = send(&d, reqwest::Method::GET, "/v1/key", &ada_key, None).await;
    let (_, bo_view) = send(&d, reqwest::Method::GET, "/v1/key", &bo_key, None).await;
    assert_eq!(ada_view["free_tier"]["used_today"], 1, "{ada_view}");
    assert_eq!(bo_view["free_tier"]["used_today"], 0, "{bo_view}");

    // A stored response is its workspace's: Bo can't read, delete or
    // continue Ada's.
    let response = stored["id"].as_str().unwrap();
    let path = format!("/v1/responses/{response}");
    let (status, _) = send(&d, reqwest::Method::GET, &path, &ada_key, None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(&d, reqwest::Method::GET, &path, &bo_key, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(&d, reqwest::Method::DELETE, &path, &bo_key, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = respond(
        &d,
        &bo_key,
        json!({"model": MODEL, "input": "and?", "previous_response_id": response}),
    )
    .await;
    assert!(status.is_client_error(), "{status}");
    let (status, _) = send(&d, reqwest::Method::GET, &path, &ada_key, None).await;
    assert_eq!(status, StatusCode::OK);

    // Usage of Ada's requests is hers.
    for id in [&ada_mine, &ada_free] {
        let usage = format!("/v1/usage/{id}");
        let (status, _) = send(&d, reqwest::Method::GET, &usage, &ada_key, None).await;
        assert_eq!(status, StatusCode::OK, "{id}");
        let (status, _) = send(&d, reqwest::Method::GET, &usage, &bo_key, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}");
    }

    // An owner of a personal workspace lists and manages its own API keys,
    // never the other people's on the tenant.
    let (_, ada_keys) = send(
        &d,
        reqwest::Method::GET,
        &format!("/v1/workspaces/{ada_ws}/keys"),
        &ada_session,
        None,
    )
    .await;
    let (_, bo_keys) = send(
        &d,
        reqwest::Method::GET,
        &format!("/v1/workspaces/{bo_ws}/keys"),
        &bo_session,
        None,
    )
    .await;
    let ids = |listed: &Value| -> Vec<String> {
        listed["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|key| key["id"].as_str().unwrap().to_owned())
            .collect()
    };
    let (ada_ids, bo_ids) = (ids(&ada_keys), ids(&bo_keys));
    assert_eq!(
        (ada_ids.len(), bo_ids.len()),
        (1, 1),
        "{ada_keys} {bo_keys}"
    );
    assert_ne!(ada_ids, bo_ids);
    let (status, _) = send(
        &d,
        reqwest::Method::POST,
        &format!("/v1/workspaces/{ada_ws}/keys/{}/pause", bo_ids[0]),
        &ada_session,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &d,
        reqwest::Method::DELETE,
        &format!("/v1/workspaces/{ada_ws}/keys/{}", bo_ids[0]),
        &ada_session,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = respond(&d, &bo_key, json!({"model": MODEL, "input": "still mine"})).await;
    assert_eq!(status, StatusCode::OK);
}
