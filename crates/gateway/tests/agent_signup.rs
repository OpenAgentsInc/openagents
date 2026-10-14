//! Agents registering themselves (#11630): the proof-of-work challenge,
//! the sign-up limits, the agent free tier and its `402`, buying credit
//! through the pay host's signed header and then paid use, claiming, and
//! revocation.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use inference::request::CreateResponse;
use inference::upstream::{
    Account, AttemptError, AttemptMeter, BoxFuture, Capabilities, CostBasis, EventStream, ModelRow,
    Price, PrivacyTerms, Sent, Upstream,
};
use nostr::domain::RelaySigner;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tenancy::Registry;

use gateway::config::{Config, Inference, SCHEMA};
use gateway::serve::{self, ServeState};

const SMALL: &str = "openai/gpt-5.6-luna";
const BIG: &str = "google/gemini-3.8-flash";
const CREDIT_URL: &str = "https://openagents.test/v1/agents/credit";
const BUY_URL: &str = "https://api.openagents.test/x/openagents-agent-credit";

struct Stub {
    account: Account,
    privacy: PrivacyTerms,
    models: Vec<ModelRow>,
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
        price: Price::micro(100_000, 10_000, 400_000),
        price_source: "test",
    }
}

fn event(value: Value) -> inference::Event {
    serde_json::from_value(value).expect("event")
}

impl Upstream for Stub {
    fn name(&self) -> &'static str {
        "up"
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
            let message = json!({"type": "message", "id": "msg_1", "status": "completed",
                "role": "assistant", "content": [{"type": "output_text", "text": "hello", "annotations": []}]});
            let done = json!({"id": "resp_1", "object": "response", "created_at": 1,
                "status": "completed", "model": model, "output": [message],
                "usage": {"input_tokens": 100, "output_tokens": 20,
                    "input_tokens_details": {"cached_tokens": 0},
                    "output_tokens_details": {"reasoning_tokens": 0}, "total_tokens": 120}});
            let events: Vec<Result<inference::Event, AttemptError>> = vec![Ok(event(
                json!({"type": "response.completed", "sequence_number": 0, "response": done}),
            ))];
            let events: EventStream = Box::pin(futures_util::stream::iter(events));
            Ok(Sent {
                events: meter.wrap(events),
                meter,
            })
        })
    }
}

struct Deployment {
    address: String,
    pay_host: RelaySigner,
    _state: Arc<ServeState>,
    _dir: tempfile::TempDir,
}

async fn deploy() -> Deployment {
    let dir = tempfile::tempdir().unwrap();
    let manifest = common::manifest(&common::artifact('a'), None);
    Registry::install(dir.path(), manifest).unwrap();
    let pay_host = RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
    let inference: Inference = serde_json::from_value(json!({
        "admin_token_env": "AGENT_SIGNUP_TEST_ADMIN",
        "service_tenants": [],
        "classes": {
            "classes": {"chat": {"models": [{"model": BIG}], "first_token_ms": 2000}},
            "model_first_token_ms": 2000
        },
        "public": {"free_tier": {"requests_per_day": 50, "models": [SMALL]}}
    }))
    .unwrap();
    let accounts = json!({
        "signup_tenant": "acme",
        "agents": {
            "difficulty_bits": 8,
            "per_ip_per_day": 2,
            "per_network_per_day": 3,
            "per_day": 10,
            "free": {
                "models": [SMALL],
                "requests_per_day": 2,
                "usd_micros_per_day": 100_000,
                "max_input_chars": 2_000,
                "max_output_tokens": 256,
                "reads_per_day": 3,
                "decisions_per_day": 1
            },
            "credit": {
                "paid_key": pay_host.pubkey(),
                "buy_url": BUY_URL,
                "upstream_url": CREDIT_URL,
                "price_sats": 1000,
                "credit_usd_micros": 1_000_000
            }
        }
    });
    let config = Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: false,
        team_policy: None,
        team_reports: None,
        inference: Some(inference),
        decisions: None,
        accounts: Some(serde_json::from_value(accounts).unwrap()),
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
    let stub: Arc<dyn Upstream> = Arc::new(Stub {
        account: Account {
            id: "up-account".into(),
            basis: CostBasis::PayAsYouGo,
        },
        privacy: PrivacyTerms::zero_retention("test"),
        models: vec![row(SMALL), row(BIG)],
    });
    let state = ServeState::open_with_upstreams(config, Some(vec![stub])).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        address,
        pay_host,
        _state: state,
        _dir: dir,
    }
}

async fn send(
    d: &Deployment,
    method: reqwest::Method,
    path: &str,
    token: Option<&str>,
    ip: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .request(method, format!("{}{path}", d.address))
        .header("x-forwarded-for", format!("198.51.100.200, {ip}"));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
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

fn solve(challenge: &str, bits: u32) -> String {
    (0_u64..)
        .map(|n| n.to_string())
        .find(|solution| {
            let digest = Sha256::digest(format!("{challenge}:{solution}").as_bytes());
            gateway::agent_accounts::leading_zero_bits(&digest) >= bits
        })
        .unwrap()
}

async fn register(d: &Deployment, name: &str, ip: &str) -> (StatusCode, Value) {
    let (status, challenge) = send(
        d,
        reqwest::Method::GET,
        "/v1/agents/challenge",
        None,
        ip,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{challenge}");
    let text = challenge["challenge"].as_str().unwrap();
    let bits = u32::try_from(challenge["difficulty_bits"].as_u64().unwrap()).unwrap();
    let solution = solve(text, bits);
    send(
        d,
        reqwest::Method::POST,
        "/v1/agents/register",
        None,
        ip,
        Some(json!({"name": name, "challenge": text, "solution": solution})),
    )
    .await
}

fn chat(model: &str) -> Value {
    json!({"model": model, "input": "Say hello."})
}

#[tokio::test]
async fn an_agent_registers_with_a_proof_of_work_and_the_limits_hold() {
    let d = deploy().await;
    // A wrong solution and a reused challenge are refused.
    let (_, challenge) = send(
        &d,
        reqwest::Method::GET,
        "/v1/agents/challenge",
        None,
        "203.0.113.1",
        None,
    )
    .await;
    let text = challenge["challenge"].as_str().unwrap().to_owned();
    let bad = (0_u64..)
        .map(|n| n.to_string())
        .find(|s| {
            gateway::agent_accounts::leading_zero_bits(&Sha256::digest(
                format!("{text}:{s}").as_bytes(),
            )) < 8
        })
        .unwrap();
    let (status, body) = send(
        &d,
        reqwest::Method::POST,
        "/v1/agents/register",
        None,
        "203.0.113.1",
        Some(json!({"name": "bot", "challenge": text, "solution": bad})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "invalid_proof");
    let good = solve(&text, 8);
    let first = send(
        &d,
        reqwest::Method::POST,
        "/v1/agents/register",
        None,
        "203.0.113.1",
        Some(json!({"name": "bot", "challenge": text, "solution": good})),
    )
    .await;
    assert_eq!(first.0, StatusCode::CREATED, "{}", first.1);
    let again = send(
        &d,
        reqwest::Method::POST,
        "/v1/agents/register",
        None,
        "203.0.113.1",
        Some(json!({"name": "bot", "challenge": text, "solution": good})),
    )
    .await;
    assert_eq!(again.1["error"]["code"], "invalid_proof", "{}", again.1);

    let made = &first.1;
    assert_eq!(made["account"]["kind"], "agent");
    assert!(made["api_key"].as_str().unwrap().starts_with("oak_"));
    assert!(made["claim"]["code"].as_str().unwrap().starts_with("clm_"));
    assert_eq!(made["free_tier"]["requests_per_day"], 2);
    assert_eq!(made["pay"]["available"], true);

    // Per address: two a day (the client's own X-Forwarded-For text is
    // ignored), then per network: three a day.
    assert_eq!(
        register(&d, "two", "203.0.113.1").await.0,
        StatusCode::CREATED
    );
    let (status, body) = register(&d, "three", "203.0.113.1").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(body["error"]["code"], "signup_limit");
    assert_eq!(
        register(&d, "four", "203.0.113.2").await.0,
        StatusCode::CREATED
    );
    let (status, _) = register(&d, "five", "203.0.113.3").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        register(&d, "six", "192.0.2.9").await.0,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn the_free_tier_ends_in_a_402_that_says_how_to_pay_then_credit_pays() {
    let d = deploy().await;
    let (_, made) = register(&d, "worker", "203.0.113.9").await;
    let key = made["api_key"].as_str().unwrap().to_owned();
    let account = made["account"]["id"].as_str().unwrap().to_owned();
    let ip = "203.0.113.9";

    // Two free requests on the small model.
    for _ in 0..2 {
        let (status, body) = send(
            &d,
            reqwest::Method::POST,
            "/v1/responses",
            Some(&key),
            ip,
            Some(chat(SMALL)),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    // The third is a 402 at once, with the free tier and how to pay.
    let started = std::time::Instant::now();
    let (status, body) = send(
        &d,
        reqwest::Method::POST,
        "/v1/responses",
        Some(&key),
        ip,
        Some(chat(SMALL)),
    )
    .await;
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "{body}");
    let error = &body["error"];
    assert_eq!(error["type"], "insufficient_balance");
    assert_eq!(error["code"], "agent_free_requests_used");
    assert_eq!(error["free_tier"]["requests_used"], 2);
    assert_eq!(error["balance_usd"], "0");
    let buy = error["pay"]["methods"][0]["url"].as_str().unwrap();
    assert_eq!(buy, format!("{BUY_URL}?account={account}"));
    assert_eq!(error["pay"]["methods"][0]["price_sats"], 1000);

    // A big model is outside the free tier from the start.
    let (status, body) = send(
        &d,
        reqwest::Method::POST,
        "/v1/responses",
        Some(&key),
        ip,
        Some(chat(BIG)),
    )
    .await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED, "{body}");
    assert_eq!(body["error"]["code"], "agent_payment_required");

    // Credit only with the pay host's signed header, once per payment.
    let query = format!("account={account}");
    let url = format!("{CREDIT_URL}?{query}");
    let path = format!("/v1/agents/credit?{query}");
    let (status, _) = send(&d, reqwest::Method::POST, &path, None, ip, None).await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let paid = |resource: &str, signer: &RelaySigner| {
        let paid = openagents_x402::hosted::Paid {
            resource: resource.into(),
            request_hash: "ab".repeat(32),
            payment_hash: "cd".repeat(32),
            settled_at: now,
            id: String::new(),
        };
        openagents_x402::hosted::paid_header(signer, &paid, "POST", &url, b"", now)
    };
    let credit = |header: String| {
        let address = format!("{}{path}", d.address);
        async move {
            let answer = reqwest::Client::new()
                .post(address)
                .header("openagents-paid", header)
                .send()
                .await
                .unwrap();
            (answer.status(), answer.json::<Value>().await.unwrap())
        }
    };
    let stranger = RelaySigner::from_secret_hex(&"22".repeat(32)).unwrap();
    assert_eq!(
        credit(paid("openagents-agent-credit", &stranger)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        credit(paid("someone-elses-resource", &d.pay_host)).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, body) = credit(paid("openagents-agent-credit", &d.pay_host)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["balance_usd"], "1");
    let (_, body) = credit(paid("openagents-agent-credit", &d.pay_host)).await;
    assert_eq!(body["already_credited"], true);
    assert_eq!(body["balance_usd"], "1");

    // Paid use: the big model now answers, held from the credit.
    let (status, body) = send(
        &d,
        reqwest::Method::POST,
        "/v1/responses",
        Some(&key),
        ip,
        Some(chat(BIG)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let (status, me) = send(
        &d,
        reqwest::Method::GET,
        "/v1/agents/me",
        Some(&key),
        ip,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{me}");
    let balance: f64 = me["balance_usd"].as_str().unwrap().parse().unwrap();
    assert!(balance < 1.0 && balance > 0.99, "{me}");
}

#[tokio::test]
async fn an_unfunded_agents_other_calls_are_capped_and_it_never_gets_a_session() {
    let d = deploy().await;
    let (_, made) = register(&d, "reader", "203.0.113.20").await;
    let key = made["api_key"].as_str().unwrap().to_owned();
    let ip = "203.0.113.20";
    // An agent's key can't become a website session.
    let (status, body) = send(
        &d,
        reqwest::Method::POST,
        "/v1/sessions",
        Some(&key),
        ip,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    for _ in 0..2 {
        let (status, _) = send(&d, reqwest::Method::GET, "/v1/key", Some(&key), ip, None).await;
        assert_ne!(status, StatusCode::TOO_MANY_REQUESTS);
    }
    let (status, body) = send(&d, reqwest::Method::GET, "/v1/key", Some(&key), ip, None).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(body["error"]["code"], "agent_daily_limit");
}

#[tokio::test]
async fn a_person_claims_an_agent_and_revokes_it() {
    let d = deploy().await;
    let (_, made) = register(&d, "helper", "203.0.113.30").await;
    let key = made["api_key"].as_str().unwrap().to_owned();
    let code = made["claim"]["code"].as_str().unwrap().to_owned();
    let agent = made["account"]["id"].as_str().unwrap().to_owned();
    let ip = "203.0.113.30";

    // A person with a session (made directly in the store, as sign-in would).
    let dir = d._dir.path();
    let accounts = tenancy::Accounts::open(dir).unwrap();
    let person = accounts.create_account("Ada", &[]).unwrap();
    let sessions = tenancy::Sessions::open(dir).unwrap();
    let issued = sessions
        .mutate(|book, _, now| {
            book.issue(tenancy::workspaces::UserId::from(person.id.as_str()), now)
        })
        .unwrap();
    let session = issued.once.clone();

    // Revoking needs the claim first (or a site admin).
    let revoke = format!("/v1/agents/{agent}/revoke");
    let (status, _) = send(&d, reqwest::Method::POST, &revoke, Some(&session), ip, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = send(
        &d,
        reqwest::Method::POST,
        "/v1/agents/claim",
        Some(&session),
        ip,
        Some(json!({"claim": code})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, mine) = send(
        &d,
        reqwest::Method::GET,
        "/v1/agents/claimed",
        Some(&session),
        ip,
        None,
    )
    .await;
    assert_eq!(mine["agents"][0]["id"], agent.as_str());
    let (status, body) = send(&d, reqwest::Method::POST, &revoke, Some(&session), ip, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body["agent"]["revoked"].is_null());
    // The key stops at once.
    let (status, _) = send(
        &d,
        reqwest::Method::POST,
        "/v1/responses",
        Some(&key),
        ip,
        Some(chat(SMALL)),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // The admin list is for site admins only.
    let (status, _) = send(
        &d,
        reqwest::Method::GET,
        "/v1/admin/agents",
        Some(&session),
        ip,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
