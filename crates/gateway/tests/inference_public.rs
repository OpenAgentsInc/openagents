//! The public inference API (#11065): any `oak_` key, the free tier on
//! free-capacity models, the worst-case hold settled at the reported
//! usage, the limits a key's owner sets, `GET /v1/key`,
//! `GET /v1/usage/{id}`, and the public `GET /v1/models` catalog.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use inference::request::CreateResponse;
use inference::upstream::{
    Account, AttemptError, AttemptMeter, BoxFuture, Capabilities, CostBasis, ErrorClass,
    EventStream, ModelRow, Price, PrivacyTerms, Sent, Upstream,
};
use serde_json::{Value, json};
use tenancy::{Registry, keys};

use gateway::config::{Config, Inference, SCHEMA};
use gateway::money::Money;
use gateway::serve::{self, ServeState};

const PAID: &str = "google/gemini-3.8-flash";
const FREE: &str = "openai/gpt-5.6-luna";

struct Stub {
    name: &'static str,
    account: Account,
    privacy: PrivacyTerms,
    models: Vec<ModelRow>,
    refuse: Option<u16>,
}

fn row(id: &str) -> ModelRow {
    ModelRow {
        id: id.to_owned(),
        upstream_model: id.to_owned(),
        capabilities: Capabilities {
            tools: true,
            reasoning: true,
            reasoning_always_on: false,
            json_schema: true,
            images: true,
            context: 1_000_000,
            max_output: 65_536,
        },
        price: Price::micro(300_000, 30_000, 2_500_000),
        price_source: "test",
    }
}

fn stub(name: &'static str, models: &[&str], refuse: Option<u16>) -> Arc<dyn Upstream> {
    Arc::new(Stub {
        name,
        account: Account {
            id: format!("{name}-account"),
            basis: CostBasis::PayAsYouGo,
        },
        privacy: PrivacyTerms::zero_retention("test"),
        models: models.iter().map(|id| row(id)).collect(),
        refuse,
    })
}

fn event(value: Value) -> inference::Event {
    serde_json::from_value(value).expect("event")
}

fn response(model: &str, status: &str, output: Value, usage: Value) -> Value {
    json!({"id": "resp_1", "object": "response", "created_at": 1, "status": status,
           "model": model, "output": output, "usage": usage})
}

impl Upstream for Stub {
    fn name(&self) -> &'static str {
        self.name
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
            let row = self.model(model).expect("model");
            let meter = AttemptMeter::start(self, row);
            if let Some(status) = self.refuse {
                let error =
                    AttemptError::new(ErrorClass::of_status(status), "refused").status(status);
                meter.fail(&error);
                return Err(error);
            }
            let message = json!({"type": "message", "id": "msg_1", "status": "completed",
                "role": "assistant", "content": [{"type": "output_text", "text": "hello", "annotations": []}]});
            let events: Vec<Result<inference::Event, AttemptError>> = vec![
                Ok(event(
                    json!({"type": "response.created", "sequence_number": 0,
                    "response": response(model, "in_progress", json!([]), Value::Null)}),
                )),
                Ok(event(
                    json!({"type": "response.output_item.added", "sequence_number": 1,
                    "output_index": 0, "item": {"type": "message", "id": "msg_1",
                    "status": "in_progress", "role": "assistant", "content": []}}),
                )),
                Ok(event(
                    json!({"type": "response.output_text.delta", "sequence_number": 2,
                    "item_id": "msg_1", "output_index": 0, "content_index": 0, "delta": "hello"}),
                )),
                Ok(event(
                    json!({"type": "response.completed", "sequence_number": 3,
                    "response": response(model, "completed", json!([message]),
                        json!({"input_tokens": 1000, "output_tokens": 100,
                               "input_tokens_details": {"cached_tokens": 0},
                               "output_tokens_details": {"reasoning_tokens": 0},
                               "total_tokens": 1100}))}),
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

struct Deployment {
    acme: String,
    house: String,
    poor: String,
    address: String,
    _state: Arc<ServeState>,
    _dir: tempfile::TempDir,
}

/// `book` is written as the key book before the gateway starts, with
/// `{acme}` replaced by acme's key id.
async fn deploy(public: bool, book: Option<Value>) -> Deployment {
    let dir = tempfile::tempdir().unwrap();
    let mut manifest = common::manifest(&common::artifact('a'), None);
    let acme = manifest.tenants["acme"].clone();
    for tenant in ["house", "poor"] {
        manifest.tenants.insert(tenant.to_owned(), acme.clone());
    }
    let registry = Registry::install(dir.path(), manifest).unwrap();
    let issue = |tenant: &str| keys::issue(dir.path(), registry.manifest(), tenant).unwrap();
    let acme_key = issue("acme");
    let house = issue("house").token;
    let poor = issue("poor").token;
    if let Some(book) = book {
        let text = book.to_string().replace("{acme}", &acme_key.key.id);
        std::fs::write(dir.path().join(gateway::inference_public::BOOK), text).unwrap();
    }
    let ledger = dir.path().join("money.jsonl");
    {
        let mut opened = tenancy::money::Ledger::open(&ledger).unwrap();
        for (source, operation) in [
            (
                "create",
                tenancy::money::Operation::Create {
                    currency: "USD".into(),
                    spend_limit: u64::MAX,
                    topups_allowed: false,
                },
            ),
            (
                "grant",
                tenancy::money::Operation::Credit {
                    amount: 1_000_000,
                    credit_kind: tenancy::money::CreditKind::Grant,
                },
            ),
        ] {
            opened
                .apply(tenancy::money::Mutation {
                    workspace: "acme".into(),
                    source: source.into(),
                    audit: format!("fixture:{source}"),
                    operation,
                })
                .unwrap();
        }
    }
    let mut inference = json!({
        "admin_token_env": "INFERENCE_PUBLIC_TEST_ADMIN",
        "service_tenants": ["house"],
        "classes": {
            "classes": {"chat": {"models": [{"model": PAID}], "first_token_ms": 2000}},
            "model_first_token_ms": 2000
        }
    });
    if public {
        inference["public"] = json!({"free_tier": {"requests_per_day": 2, "models": [FREE]}});
    }
    let inference: Inference = serde_json::from_value(inference).unwrap();
    let config = Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: false,
        team_policy: None,
        team_reports: None,
        inference: Some(inference),
        decisions: None,
        accounts: None,
        billing: None,
        funding: None,
        earnings: None,
        commercial: None,
        skills: None,
        money: Some(Money {
            hierarchical_budgets: false,
            ledger,
            doors: Default::default(),
        }),
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
    let state = ServeState::open_with_upstreams(
        config,
        Some(vec![
            stub("down", &[PAID], Some(503)),
            stub("up", &[PAID], None),
            stub("pro", &[FREE], None),
        ]),
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        acme: acme_key.token,
        house,
        poor,
        address,
        _state: state,
        _dir: dir,
    }
}

async fn call(d: &Deployment, token: &str, path: &str, body: Value) -> (StatusCode, Value, String) {
    let answer = reqwest::Client::new()
        .post(format!("{}{path}", d.address))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = answer.status();
    let id = answer
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let text = answer.text().await.unwrap();
    let body = serde_json::from_str(&text).unwrap_or(Value::String(text));
    (status, body, id)
}

async fn read(d: &Deployment, token: Option<&str>, path: &str) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new().get(format!("{}{path}", d.address));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let answer = request.send().await.unwrap();
    (answer.status(), answer.json().await.unwrap())
}

#[tokio::test]
async fn without_public_terms_only_services_call() {
    let d = deploy(false, None).await;
    let (status, body, _) = call(
        &d,
        &d.acme,
        "/v1/responses",
        json!({"model": PAID, "input": "hi"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, _, _) = call(
        &d,
        &d.house,
        "/v1/responses",
        json!({"model": PAID, "input": "hi"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn free_requests_then_paid_ones_settle_at_reported_usage() {
    let d = deploy(true, None).await;
    // Two free requests a day on the free model.
    let mut ids = Vec::new();
    for _ in 0..2 {
        let (status, body, id) = call(
            &d,
            &d.acme,
            "/v1/responses",
            json!({"model": FREE, "input": "hi"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        ids.push(id);
    }
    let (_, usage) = read(&d, Some(&d.acme), &format!("/v1/usage/{}", ids[0])).await;
    assert_eq!(usage["charged"]["free"], true);
    assert_eq!(usage["charged"]["usd"], "0");
    assert_eq!(usage["model"], FREE);
    assert_eq!(usage["tokens"]["input"], 1000);

    // The third is paid from the balance: 1,000 in at $0.315/M and 100
    // out at $2.625/M, each rounded up to the micro: $0.000578.
    let (status, body, third) = call(
        &d,
        &d.acme,
        "/v1/chat/completions",
        json!({"model": FREE, "messages": [{"role": "user", "content": "hi"}]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["object"], "chat.completion");
    let (_, usage) = read(&d, Some(&d.acme), &format!("/v1/usage/{third}")).await;
    assert_eq!(usage["charged"]["free"], false);
    assert_eq!(usage["charged"]["settlement"], "settled");
    assert_eq!(usage["charged"]["usd"], "0.000578");
    assert_eq!(usage["api"], "chat");

    // A paid model, streamed, through a fallback: one hold, settled when
    // the stream ends.
    let answer = reqwest::Client::new()
        .post(format!("{}/v1/responses", d.address))
        .bearer_auth(&d.acme)
        .json(&json!({"model": PAID, "input": "hi", "stream": true}))
        .send()
        .await
        .unwrap();
    assert_eq!(answer.status(), StatusCode::OK, "{:?}", answer.text().await);
    let streamed = answer.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .to_owned();
    let text = answer.text().await.unwrap();
    assert!(text.trim_end().ends_with("data: [DONE]"));
    // Settlement runs as the terminal event passes; give it a moment.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let (_, usage) = read(&d, Some(&d.acme), &format!("/v1/usage/{streamed}")).await;
    assert_eq!(usage["charged"]["usd"], "0.000578", "{usage}");
    assert_eq!(usage["attempts"][0]["outcome"], "fallback");
    assert_eq!(usage["attempts"][1]["upstream"], "up");

    let (status, key) = read(&d, Some(&d.acme), "/v1/key").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(key["free_tier"]["left_today"], 0);
    assert_eq!(key["spend"]["today_usd"], "0.001156");
    assert_eq!(key["balance"]["available"], "0.998844");
    assert_eq!(key["balance"]["reserved"], "0");

    // Another tenant can't read acme's request.
    let (status, _) = read(&d, Some(&d.poor), &format!("/v1/usage/{third}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn no_balance_is_402_and_service_keys_are_never_charged() {
    let d = deploy(true, None).await;
    let (status, body, _) = call(
        &d,
        &d.poor,
        "/v1/responses",
        json!({"model": PAID, "input": "hi"}),
    )
    .await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "{body}");
    assert_eq!(body["error"]["type"], "insufficient_balance");
    // The poor tenant still has its free requests.
    let (status, _, _) = call(
        &d,
        &d.poor,
        "/v1/responses",
        json!({"model": FREE, "input": "hi"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, id) = call(
        &d,
        &d.house,
        "/v1/responses",
        json!({"model": PAID, "input": "hi"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, usage) = read(&d, Some(&d.house), &format!("/v1/usage/{id}")).await;
    assert_eq!(usage["charged"], Value::Null);
}

#[tokio::test]
async fn limits_the_owner_set_answer_limit_reached() {
    let book = json!({"limits": {"{acme}": {
        "spend_cap": {"usd": "0.10", "period": "day"},
        "models": [PAID, FREE],
        "requests_per_minute": 3
    }}});
    let d = deploy(true, Some(book)).await;
    // Up to 65,536 output tokens could cost more than the $0.10 cap.
    let (status, body, _) = call(
        &d,
        &d.acme,
        "/v1/responses",
        json!({"model": PAID, "input": "hi"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["type"], "limit_reached");
    assert_eq!(body["error"]["param"], "limits.spend_cap");
    // A bounded answer fits.
    let (status, body, _) = call(
        &d,
        &d.acme,
        "/v1/responses",
        json!({"model": PAID, "input": "hi", "max_output_tokens": 200}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body, _) = call(
        &d,
        &d.acme,
        "/v1/responses",
        json!({"model": "openagents/chat", "input": "hi"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["param"], "limits.models");
    // A refusal before the rate check isn't counted: this is the third
    // request this minute, and the fourth is over.
    let (status, _, _) = call(
        &d,
        &d.acme,
        "/v1/responses",
        json!({"model": FREE, "input": "hi"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body, _) = call(
        &d,
        &d.acme,
        "/v1/responses",
        json!({"model": FREE, "input": "hi"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["param"], "limits.requests_per_minute");
    let (_, key) = read(&d, Some(&d.acme), "/v1/key").await;
    assert_eq!(key["limits"]["spend_cap"]["usd"], "0.10");
}

#[tokio::test]
async fn an_expired_key_is_refused() {
    let d = deploy(true, Some(json!({"limits": {"{acme}": {"expires_at": 1}}}))).await;
    let (status, body, _) = call(
        &d,
        &d.acme,
        "/v1/responses",
        json!({"model": FREE, "input": "hi"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["param"], "limits.expires_at");
}

#[tokio::test]
async fn the_model_catalog_is_public_in_the_openai_shape() {
    let d = deploy(true, None).await;
    let (status, list) = read(&d, None, "/v1/models").await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["object"], "list");
    let data = list["data"].as_array().unwrap();
    let paid = data.iter().find(|model| model["id"] == PAID).unwrap();
    assert_eq!(paid["object"], "model");
    assert_eq!(paid["owned_by"], "google");
    assert_eq!(paid["openagents"]["free"], false);
    let up = paid["openagents"]["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["upstream"] == "up")
        .unwrap();
    assert_eq!(up["context"], 1_000_000);
    assert_eq!(up["prices"][0]["input"]["price_usd"], "0.315");
    assert_eq!(up["prices"][0]["output"]["price_usd"], "2.625");
    let free = data.iter().find(|model| model["id"] == FREE).unwrap();
    assert_eq!(free["openagents"]["free"], true);
    assert!(data.iter().any(|model| model["id"] == "openagents/chat"));
    assert!(data.iter().any(|model| model["id"] == "openagents/auto"));
    // With a key, the same list.
    let (status, keyed) = read(&d, Some(&d.acme), "/v1/models").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(keyed["data"], list["data"]);
}

#[tokio::test]
async fn public_token_totals_match_answering_meter_records() {
    let d = deploy(true, None).await;
    let mut total = 0;
    for (key, model) in [(&d.acme, FREE), (&d.acme, PAID), (&d.house, PAID)] {
        let (status, body, id) = call(
            &d,
            key,
            "/v1/responses",
            json!({"model": model, "input": "hi"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (_, usage) = read(&d, Some(key), &format!("/v1/usage/{id}")).await;
        assert!(
            usage["attempts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|attempt| attempt["outcome"] == "ok")
        );
        total += usage["tokens"]["input"].as_u64().unwrap()
            + usage["tokens"]["output"].as_u64().unwrap();
    }
    let (status, report) = read(&d, None, "/v1/usage/tokens-served").await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["totals"]["all"]["total"], total);
    assert_eq!(total, 3300);
    assert_eq!(report["totals"]["all"]["answers"], 3);
    assert_eq!(report["totals"]["internal"]["free"]["total"], 1100);
    assert_eq!(report["totals"]["outside"]["free"]["total"], 1100);
    assert_eq!(report["totals"]["outside"]["paid"]["total"], 1100);
    let day = report["days"].as_object().unwrap().values().next().unwrap();
    assert_eq!(day, &report["totals"]);
    assert!(!report.to_string().contains("acme"));
}

/// A signed-in person's `sess_` session (what Coder holds after the
/// device sign-in), its account's personal workspace on `acme`, and the
/// anonymous and closed sessions that are refused.
struct Signed {
    user: String,
    anonymous: String,
    revoked: String,
}

fn sign_in(d: &Deployment) -> Signed {
    use tenancy::Accounts;
    use tenancy::accounts::WorkspaceKind;
    use tenancy::sessions::{SessionBook, Sessions};
    use tenancy::workspaces::UserId;
    let dir = d._dir.path();
    let accounts = Accounts::install(dir).unwrap();
    let ada = accounts.create_account("Ada", &[]).unwrap();
    let gone = accounts.create_account("Gone", &[]).unwrap();
    for who in [&ada, &gone] {
        accounts
            .create_workspace(&who.id, &who.label, WorkspaceKind::Personal, "acme", None)
            .unwrap();
    }
    let sessions = Sessions::install(dir, SessionBook::new(3600, 3600)).unwrap();
    let user = sessions
        .mutate(|book, _, now| book.issue(UserId::from(ada.id.as_str()), now))
        .unwrap()
        .once;
    let anonymous = sessions
        .mutate(|book, _, now| book.issue_anonymous(now))
        .unwrap()
        .once;
    let revoked = sessions
        .mutate(|book, _, now| book.issue(UserId::from(gone.id.as_str()), now))
        .unwrap()
        .once;
    sessions
        .mutate(|book, _, now| {
            book.revoke_all(&UserId::from(gone.id.as_str()), now);
            Ok(())
        })
        .unwrap();
    Signed {
        user,
        anonymous,
        revoked,
    }
}

fn chat(model: &str) -> Value {
    json!({"model": model, "stream": false, "messages": [{"role": "user", "content": "hi"}]})
}

#[tokio::test]
async fn a_signed_in_session_runs_inference_in_its_own_workspace() {
    let d = deploy(true, None).await;
    let signed = sign_in(&d);
    // Its personal workspace has no balance: the paid model is held
    // against it and refused, exactly as for a key in that workspace.
    let (status, body, _) = call(&d, &signed.user, "/v1/chat/completions", chat(PAID)).await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "{body}");
    assert_eq!(body["error"]["type"], "insufficient_balance");
    // The free tier counts per workspace: two free requests, then none.
    for _ in 0..2 {
        let (status, body, _) = call(&d, &signed.user, "/v1/chat/completions", chat(FREE)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["object"], "chat.completion");
    }
    let (status, body, _) = call(&d, &signed.user, "/v1/chat/completions", chat(FREE)).await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "{body}");
    // The tenant's own key, in no account's workspace, keeps its own.
    let (status, body, _) = call(&d, &d.acme, "/v1/chat/completions", chat(FREE)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn anonymous_closed_and_unknown_sessions_are_refused() {
    let d = deploy(true, None).await;
    let signed = sign_in(&d);
    for (token, message) in [
        (
            signed.anonymous.as_str(),
            "An anonymous session can't run inference. Sign in, or use an API key.",
        ),
        (
            signed.revoked.as_str(),
            "Your session is revoked. Sign in again.",
        ),
        (
            "sess_notarealsession",
            "Your session token isn't recognized. Sign in again.",
        ),
    ] {
        let (status, body, _) = call(&d, token, "/v1/chat/completions", chat(FREE)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
        assert_eq!(body["error"]["type"], "unauthorized");
        assert_eq!(body["error"]["message"], message);
        assert!(
            !body.to_string().contains(token),
            "the token is never echoed"
        );
    }
}

#[tokio::test]
async fn without_public_terms_a_session_is_refused_like_a_key() {
    let d = deploy(false, None).await;
    let signed = sign_in(&d);
    let (status, body, _) = call(&d, &signed.user, "/v1/chat/completions", chat(FREE)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        body["error"]["message"],
        "Inference is open to OpenAgents services only for now."
    );
}
