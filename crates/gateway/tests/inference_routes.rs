//! `/v1/responses` and `/v1/chat/completions` for service keys (#11064):
//! service tenants only, both APIs, streaming and not, the route and cost
//! headers, and every attempt in the meter's admin status.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use futures_util::StreamExt;
use inference::request::CreateResponse;
use inference::upstream::{
    Account, AttemptError, AttemptMeter, BoxFuture, Capabilities, CostBasis, ErrorClass,
    EventStream, ModelRow, Price, PrivacyTerms, Sent, Upstream,
};
use serde_json::{Value, json};
use tenancy::{Registry, keys};

use gateway::config::{Config, Inference, SCHEMA};
use gateway::serve::{self, ServeState};

const MODEL: &str = "google/gemini-3.8-flash";
const ADMIN_ENV: &str = "INFERENCE_ROUTES_TEST_ADMIN";

/// One stub upstream: answers "hello" or refuses with a status.
struct Stub {
    name: &'static str,
    account: Account,
    privacy: PrivacyTerms,
    models: Vec<ModelRow>,
    refuse: Option<u16>,
}

fn stub(name: &'static str, refuse: Option<u16>) -> Arc<dyn Upstream> {
    Arc::new(Stub {
        name,
        account: Account {
            id: format!("{name}-account"),
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
            price: Price::micro(300_000, 30_000, 2_500_000),
            price_source: "test",
        }],
        refuse,
    })
}

fn event(value: Value) -> inference::Event {
    serde_json::from_value(value).expect("event")
}

fn response(status: &str, output: Value, usage: Value) -> Value {
    json!({"id": "resp_1", "object": "response", "created_at": 1, "status": status,
           "model": MODEL, "output": output, "usage": usage})
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
                    "response": response("in_progress", json!([]), Value::Null)}),
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
                    "response": response("completed", json!([message]),
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
    token: String,
    address: String,
    _state: Arc<ServeState>,
    _dir: tempfile::TempDir,
}

async fn deploy(service_tenants: Vec<String>) -> Deployment {
    let dir = tempfile::tempdir().unwrap();
    let manifest = common::manifest(&common::artifact('a'), None);
    let registry = Registry::install(dir.path(), manifest).unwrap();
    let token = keys::issue(dir.path(), registry.manifest(), "acme")
        .unwrap()
        .token;
    let inference: Inference = serde_json::from_value(json!({
        "admin_token_env": ADMIN_ENV,
        "service_tenants": service_tenants,
        "classes": {
            "classes": {"chat": {"models": [
                {"model": MODEL, "upstream": "down"},
                {"model": MODEL, "upstream": "up"}
            ], "first_token_ms": 2000}},
            "model_first_token_ms": 2000
        },
        "accounts": [{"id": "up-account", "upstream": "up", "granted": 30000000000u64,
                      "balance": 30000000000u64, "basis": "prepaid"}]
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
        accounts: None,
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
    let state = ServeState::open_with_upstreams(
        config,
        Some(vec![stub("down", Some(503)), stub("up", None)]),
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        token,
        address,
        _state: state,
        _dir: dir,
    }
}

fn post(d: &Deployment, path: &str, body: Value) -> reqwest::RequestBuilder {
    reqwest::Client::new()
        .post(format!("{}{path}", d.address))
        .bearer_auth(&d.token)
        .json(&body)
}

fn header(response: &reqwest::Response, name: &str) -> String {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test]
async fn service_keys_get_both_apis_and_every_attempt_is_metered() {
    // SAFETY: this test binary's only test that reads the variable sets
    // it before any request; nothing else in the process touches it.
    unsafe { std::env::set_var(ADMIN_ENV, "admin-secret") };
    let d = deploy(vec!["acme".into()]).await;

    // Open Responses, no stream: the fallback chose `up`.
    let answer = post(
        &d,
        "/v1/responses",
        json!({"model": "openagents/chat", "input": "hi"}),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(answer.status(), StatusCode::OK);
    assert_eq!(header(&answer, "x-openagents-upstream"), "up");
    assert_eq!(header(&answer, "x-openagents-model"), MODEL);
    assert!(header(&answer, "x-request-id").starts_with("req_"));
    // 1,000 in at $0.30/M and 100 out at $2.50/M: $0.00055, plus 5%.
    assert_eq!(header(&answer, "x-openagents-cost-usd"), "0.000578");
    let body: Value = answer.json().await.unwrap();
    assert_eq!(body["status"], "completed");
    assert_eq!(body["openagents"]["upstream"], "up");
    assert_eq!(body["openagents"]["attempts"][0]["outcome"], "fallback");
    assert_eq!(body["openagents"]["attempts"][1]["outcome"], "ok");

    // Open Responses, streamed, without asking for our events: only the
    // spec's events, numbered from zero with no gaps.
    let plain = post(
        &d,
        "/v1/responses",
        json!({"model": "openagents/chat", "input": "hi", "stream": true}),
    )
    .send()
    .await
    .unwrap()
    .text()
    .await
    .unwrap();
    assert!(!plain.contains("openagents:"), "{plain}");
    let numbers: Vec<u64> = plain
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|data| serde_json::from_str::<Value>(data).ok())
        .filter_map(|event| event["sequence_number"].as_u64())
        .collect();
    assert_eq!(numbers, (0..numbers.len() as u64).collect::<Vec<_>>());

    // Asking for them: route before output, cost before the end, then
    // [DONE].
    let streamed = post(
        &d,
        "/v1/responses",
        json!({"model": "openagents/chat", "input": "hi", "stream": true}),
    )
    .header("x-openagents-events", "route,cost")
    .send()
    .await
    .unwrap();
    assert_eq!(header(&streamed, "content-type"), "text/event-stream");
    let text = streamed.text().await.unwrap();
    let route = text.find("openagents:route").unwrap();
    let delta = text.find("response.output_text.delta").unwrap();
    let cost = text.find("openagents:cost").unwrap();
    let done = text.find("response.completed").unwrap();
    assert!(route < delta && delta < cost && cost < done, "{text}");
    assert!(text.trim_end().ends_with("data: [DONE]"), "{text}");

    // Chat Completions, both ways.
    let chat = post(
        &d,
        "/v1/chat/completions",
        json!({"model": "openagents/chat", "messages": [{"role": "user", "content": "hi"}]}),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(chat.status(), StatusCode::OK);
    let body: Value = chat.json().await.unwrap();
    assert_eq!(body["choices"][0]["message"]["content"], "hello");
    assert_eq!(body["choices"][0]["finish_reason"], "stop");
    let chunks = post(
        &d,
        "/v1/chat/completions",
        json!({"model": "openagents/chat", "stream": true,
               "stream_options": {"include_usage": true},
               "messages": [{"role": "user", "content": "hi"}]}),
    )
    .send()
    .await
    .unwrap();
    let mut stream = chunks.bytes_stream();
    let mut text = String::new();
    while let Some(chunk) = stream.next().await {
        text.push_str(&String::from_utf8_lossy(&chunk.unwrap()));
    }
    assert!(text.contains("\"content\":\"hello\""), "{text}");
    assert!(text.trim_end().ends_with("data: [DONE]"), "{text}");

    // Five requests, two attempts each, all in the meter.
    let status: Value = reqwest::Client::new()
        .get(format!("{}/v1/admin/inference/status", d.address))
        .bearer_auth("admin-secret")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["records"]["kept"], 10, "{status}");
    let up_account = status["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|account| account["account"] == "up-account")
        .unwrap();
    assert!(
        up_account["spent_today"].as_u64().unwrap() > 0,
        "{up_account}"
    );

    // The operator page shows the burn-down to the admin token.
    let page = reqwest::Client::new()
        .get(format!("{}/admin/inference", d.address))
        .bearer_auth("admin-secret")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        page.contains("Credit burn-down") && page.contains("up-account"),
        "{page}"
    );
}

#[tokio::test]
async fn keys_outside_the_service_tenants_and_stored_responses_are_refused() {
    let d = deploy(Vec::new()).await;
    let refused = post(
        &d,
        "/v1/responses",
        json!({"model": "openagents/chat", "input": "hi"}),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    let body: Value = refused.json().await.unwrap();
    assert_eq!(body["error"]["type"], "limit_reached");

    let unkeyed = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", d.address))
        .json(&json!({"model": "openagents/chat", "messages": []}))
        .send()
        .await
        .unwrap();
    assert_eq!(unkeyed.status(), StatusCode::UNAUTHORIZED);

    let d = deploy(vec!["acme".into()]).await;
    let stored = post(
        &d,
        "/v1/responses",
        json!({"model": "openagents/chat", "input": "hi", "store": true}),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(stored.status(), StatusCode::BAD_REQUEST);
}
