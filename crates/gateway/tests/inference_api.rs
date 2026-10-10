//! The public inference API's contract (#11078, #11136, #11137):
//! `GET /v1/openapi.json` held to the mounted route table, and
//! pay-per-request on `/v1/responses` and `/v1/chat/completions` through
//! the payment router (x402 and the `Payment` scheme on one invoice)
//! against a receiver that signs real BOLT11 invoices with a test key (no
//! real payment: the test buyer reads the preimage back from it, as a
//! wallet returns it after paying), with the advertised methods held to
//! the configured ones.

mod common;

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use inference::request::CreateResponse;
use inference::upstream::{
    Account, AttemptError, AttemptMeter, BoxFuture, Capabilities, CostBasis, ErrorClass,
    EventStream, ModelRow, Price, PrivacyTerms, Sent, Upstream,
};
use nostr::x402::decode_invoice;
use nostr::x402::test_invoice::{number, payee_of, signed_by, tag, words};
use openagents_x402::server::Receiver;
use openagents_x402::wire::{decode_header, decode_payment_required, encode_header};
use openagents_x402::{PaymentPayload, SettlementResponse};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tenancy::Registry;

use gateway::config::{Config, Inference, SCHEMA};
use gateway::serve::{self, ServeState};

const PAID: &str = "google/gemini-3.8-flash";
const NODE: [u8; 32] = [9; 32];

// ------------------------------------------------------------ upstream

struct Stub {
    account: Account,
    privacy: PrivacyTerms,
    models: Vec<ModelRow>,
    refuse: bool,
}

fn stub(refuse: bool) -> Arc<dyn Upstream> {
    Arc::new(Stub {
        account: Account {
            id: "up-account".into(),
            basis: CostBasis::PayAsYouGo,
        },
        privacy: PrivacyTerms::zero_retention("test"),
        models: vec![ModelRow {
            id: PAID.into(),
            upstream_model: PAID.into(),
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
           "model": PAID, "output": output, "usage": usage})
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
            if self.refuse {
                let error = AttemptError::new(ErrorClass::of_status(503), "down").status(503);
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
                        json!({"input_tokens": 10, "output_tokens": 2,
                               "input_tokens_details": {"cached_tokens": 0},
                               "output_tokens_details": {"reasoning_tokens": 0},
                               "total_tokens": 12}))}),
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

// ------------------------------------------------------------ receiver

/// Signs invoices as `NODE` and keeps each preimage, so the test buyer
/// can "pay" by reading it back.
#[derive(Default)]
struct TestNode {
    counter: AtomicU64,
    preimages: Mutex<HashMap<String, [u8; 32]>>,
}

impl TestNode {
    fn pay(&self, invoice: &str) -> String {
        let hash = hex::encode(decode_invoice(invoice).unwrap().payment_hash());
        hex::encode(self.preimages.lock().unwrap()[&hash])
    }
}

fn hrp(amount_msat: u64) -> String {
    match amount_msat {
        a if a % 100_000_000 == 0 => format!("lnbc{}m", a / 100_000_000),
        a if a % 100_000 == 0 => format!("lnbc{}u", a / 100_000),
        a if a % 100 == 0 => format!("lnbc{}n", a / 100),
        a => format!("lnbc{}p", a * 10),
    }
}

impl Receiver for TestNode {
    fn pay_to(&self) -> String {
        hex::encode(payee_of(NODE))
    }
    fn invoice(&self, amount: u64, request_hash: [u8; 32], expiry: u32) -> Result<String, String> {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut seed = request_hash.to_vec();
        seed.extend(n.to_be_bytes());
        let preimage: [u8; 32] = Sha256::digest(&seed).into();
        let payment_hash: [u8; 32] = Sha256::digest(preimage).into();
        let mut fields = tag(1, &words(&payment_hash));
        fields.extend(tag(16, &words(&[2; 32])));
        fields.extend(tag(23, &words(&request_hash)));
        fields.extend(tag(6, &number(u64::from(expiry))));
        let invoice = signed_by(
            NODE,
            &hrp(amount),
            fields,
            false,
            false,
            openagents_x402::unix_now(),
        );
        self.preimages
            .lock()
            .unwrap()
            .insert(hex::encode(payment_hash), preimage);
        Ok(invoice)
    }
}

// ------------------------------------------------------------ deployment

struct Deployment {
    address: String,
    node: Arc<TestNode>,
    state: Arc<ServeState>,
    _dir: tempfile::TempDir,
}

async fn deploy(x402: bool, refuse: bool) -> Deployment {
    deploy_with(if x402 { Pay::X402 } else { Pay::None }, refuse).await
}

/// Which pay-per-request methods a deployment is configured with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pay {
    None,
    X402,
    X402AndMpp,
}

impl Pay {
    fn ids(self) -> Vec<&'static str> {
        match self {
            Self::None => vec![],
            Self::X402 => vec!["x402"],
            Self::X402AndMpp => vec!["x402", "mpp"],
        }
    }
}

async fn deploy_with(pay: Pay, refuse: bool) -> Deployment {
    let dir = tempfile::tempdir().unwrap();
    let manifest = common::manifest(&common::artifact('a'), None);
    Registry::install(dir.path(), manifest).unwrap();
    // A keyring for workspaces' own provider keys, so those routes mount.
    let keyring = dir.path().join("keyring.json");
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
    let mut inference = json!({
        "admin_token_env": "INFERENCE_API_TEST_ADMIN",
        "service_tenants": [],
        "classes": {
            "classes": {"chat": {"models": [{"model": PAID}], "first_token_ms": 2000}},
            "model_first_token_ms": 2000
        },
        "sats_rate": {"usd_per_btc": 100_000, "as_of": "2026-10-09"},
        "public": {},
        "byok": {"keyring": keyring},
    });
    if pay != Pay::None {
        inference["x402"] = json!({
            "wallet_home": "/nonexistent",
            "receiver_node": hex::encode(payee_of(NODE)),
            "network": "bitcoin",
        });
    }
    if pay == Pay::X402AndMpp {
        inference["x402"]["mpp"] = json!({});
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
        accounts: Some(gateway::config::Accounts {
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
    let node = Arc::new(TestNode::default());
    let receiver: Arc<dyn Receiver> = node.clone();
    let state = ServeState::open_with(config, Some(vec![stub(refuse)]), Some(receiver)).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        address,
        node,
        state,
        _dir: dir,
    }
}

struct Answer {
    status: StatusCode,
    headers: reqwest::header::HeaderMap,
    text: String,
}

impl Answer {
    fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or(Value::Null)
    }
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }
}

async fn post(d: &Deployment, path: &str, body: &str, signature: Option<&str>) -> Answer {
    match signature {
        Some(signature) => post_with(d, path, body, &[("PAYMENT-SIGNATURE", signature)]).await,
        None => post_with(d, path, body, &[]).await,
    }
}

async fn post_with(d: &Deployment, path: &str, body: &str, headers: &[(&str, &str)]) -> Answer {
    let mut request = reqwest::Client::new()
        .post(format!("{}{path}", d.address))
        .header("content-type", "application/json")
        .body(body.to_owned());
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let answer = request.send().await.unwrap();
    Answer {
        status: answer.status(),
        headers: answer.headers().clone(),
        text: answer.text().await.unwrap(),
    }
}

/// The 402's terms, paid: the `PAYMENT-SIGNATURE` to send.
fn pay(d: &Deployment, challenge: &Answer) -> String {
    let required =
        decode_payment_required(challenge.header("PAYMENT-REQUIRED").expect("terms")).unwrap();
    let accepted = required.accepts[0].clone();
    let invoice = accepted.extra["invoice"].as_str().unwrap().to_owned();
    let mut payload = Map::new();
    payload.insert("preimage".into(), json!(d.node.pay(&invoice)));
    encode_header(&PaymentPayload {
        x402_version: 2,
        resource: Some(required.resource),
        accepted,
        payload,
        extensions: None,
    })
    .unwrap()
}

// ------------------------------------------------------------ x402

#[tokio::test]
async fn a_keyless_request_pays_with_x402_and_runs_once() {
    let d = deploy(true, false).await;
    let body = json!({"model": PAID, "input": "Say hello.", "max_output_tokens": 100}).to_string();

    let challenge = post(&d, "/v1/responses", &body, None).await;
    assert_eq!(
        challenge.status,
        StatusCode::PAYMENT_REQUIRED,
        "{}",
        challenge.text
    );
    let refusal = challenge.json();
    assert_eq!(refusal["error"]["type"], "payment_required");
    let required = decode_payment_required(challenge.header("PAYMENT-REQUIRED").unwrap()).unwrap();
    assert_eq!(required.x402_version, 2);
    assert_eq!(required.resource.url, format!("{}/v1/responses", d.address));
    let terms = &required.accepts[0];
    assert_eq!(terms.scheme, "exact");
    assert_eq!(terms.asset, "BTC");
    assert_eq!(terms.pay_to, hex::encode(payee_of(NODE)));
    assert_eq!(terms.amount, refusal["price_msat"].as_str().unwrap());
    let msat: u64 = terms.amount.parse().unwrap();
    assert_eq!(msat % 1_000, 0, "whole sats");
    assert_eq!(msat / 1_000, refusal["price_sats"].as_u64().unwrap());
    // The worst case from the rate card: the same request gets the same price.
    let again = post(&d, "/v1/responses", &body, None).await;
    assert_eq!(again.json()["price_msat"], refusal["price_msat"]);
    // A bigger output allowance costs more.
    let bigger =
        json!({"model": PAID, "input": "Say hello.", "max_output_tokens": 60_000}).to_string();
    let dearer = post(&d, "/v1/responses", &bigger, None).await;
    assert!(
        dearer.json()["price_sats"].as_u64().unwrap() > refusal["price_sats"].as_u64().unwrap()
    );

    let signature = pay(&d, &challenge);
    let paid = post(&d, "/v1/responses", &body, Some(&signature)).await;
    assert_eq!(paid.status, StatusCode::OK, "{}", paid.text);
    assert_eq!(paid.json()["output"][0]["content"][0]["text"], "hello");
    assert!(paid.header("x-openagents-cost-usd").is_some());
    let settlement: SettlementResponse =
        decode_header(paid.header("PAYMENT-RESPONSE").expect("settlement")).unwrap();
    assert!(settlement.success);
    assert_eq!(settlement.amount.as_deref(), Some(terms.amount.as_str()));

    // One payment, one answer.
    let replay = post(&d, "/v1/responses", &body, Some(&signature)).await;
    assert_eq!(replay.status, StatusCode::PAYMENT_REQUIRED);
    assert_eq!(replay.json()["reason"], "duplicate_settlement");
}

#[tokio::test]
async fn a_payment_buys_only_the_request_it_was_bound_to() {
    let d = deploy(true, false).await;
    let body = json!({"model": PAID, "input": "One.", "max_output_tokens": 50}).to_string();
    let challenge = post(&d, "/v1/responses", &body, None).await;
    let signature = pay(&d, &challenge);
    let other = json!({"model": PAID, "input": "Two.", "max_output_tokens": 50}).to_string();
    let refused = post(&d, "/v1/responses", &other, Some(&signature)).await;
    assert_eq!(
        refused.status,
        StatusCode::PAYMENT_REQUIRED,
        "{}",
        refused.text
    );
    assert!(refused.header("PAYMENT-RESPONSE").is_some());
    // The original still runs: the refusal consumed nothing.
    let paid = post(&d, "/v1/responses", &body, Some(&signature)).await;
    assert_eq!(paid.status, StatusCode::OK, "{}", paid.text);
    // A forged preimage is refused.
    let challenge = post(&d, "/v1/responses", &other, None).await;
    let required = decode_payment_required(challenge.header("PAYMENT-REQUIRED").unwrap()).unwrap();
    let mut payload = Map::new();
    payload.insert("preimage".into(), json!("00".repeat(32)));
    let forged = encode_header(&PaymentPayload {
        x402_version: 2,
        resource: Some(required.resource),
        accepted: required.accepts[0].clone(),
        payload,
        extensions: None,
    })
    .unwrap();
    let refused = post(&d, "/v1/responses", &other, Some(&forged)).await;
    assert_eq!(refused.status, StatusCode::PAYMENT_REQUIRED);
}

#[tokio::test]
async fn chat_completions_stream_after_payment() {
    let d = deploy(true, false).await;
    let body = json!({"model": PAID, "stream": true, "max_tokens": 64,
                      "messages": [{"role": "user", "content": "Say hello."}]})
    .to_string();
    let challenge = post(&d, "/v1/chat/completions", &body, None).await;
    assert_eq!(challenge.status, StatusCode::PAYMENT_REQUIRED);
    let required = decode_payment_required(challenge.header("PAYMENT-REQUIRED").unwrap()).unwrap();
    assert_eq!(
        required.resource.mime_type.as_deref(),
        Some("text/event-stream")
    );
    let signature = pay(&d, &challenge);
    let paid = post(&d, "/v1/chat/completions", &body, Some(&signature)).await;
    assert_eq!(paid.status, StatusCode::OK, "{}", paid.text);
    assert!(paid.header("PAYMENT-RESPONSE").is_some());
    assert!(paid.text.contains("chat.completion.chunk"));
    assert!(paid.text.contains("hello"));
    assert!(paid.text.trim_end().ends_with("data: [DONE]"));
}

#[tokio::test]
async fn an_unanswered_paid_request_gives_the_payment_back() {
    let d = deploy(true, true).await;
    let body = json!({"model": PAID, "input": "Hello?", "max_output_tokens": 10}).to_string();
    let challenge = post(&d, "/v1/responses", &body, None).await;
    let signature = pay(&d, &challenge);
    let failed = post(&d, "/v1/responses", &body, Some(&signature)).await;
    assert_eq!(failed.status, StatusCode::BAD_GATEWAY, "{}", failed.text);
    // The same proof is good again: nothing was answered.
    let again = post(&d, "/v1/responses", &body, Some(&signature)).await;
    assert_eq!(again.status, StatusCode::BAD_GATEWAY, "{}", again.text);
}

#[tokio::test]
async fn keyless_requests_that_need_an_account_are_refused_plainly() {
    let d = deploy(true, false).await;
    let stored = json!({"model": PAID, "input": "Hi", "store": true}).to_string();
    let answer = post(&d, "/v1/responses", &stored, None).await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{}", answer.text);
    assert_eq!(answer.json()["error"]["param"], "store");
    let hosted = json!({"model": PAID, "input": "Hi",
                        "tools": [{"type": "openagents:web_search"}]})
    .to_string();
    let answer = post(&d, "/v1/responses", &hosted, None).await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{}", answer.text);
    // Without x402 set up, no key is still 401.
    let plain = deploy(false, false).await;
    let answer = post(&plain, "/v1/responses", &stored, None).await;
    assert_eq!(answer.status, StatusCode::UNAUTHORIZED);
}

#[test]
fn dollars_become_whole_sats_rounded_up() {
    use gateway::inference_x402::msat_of;
    // $1 at $100,000 a bitcoin is 1,000 sats.
    assert_eq!(msat_of(1_000_000, 100_000), 1_000_000);
    // A thousandth of a cent still costs a sat.
    assert_eq!(msat_of(10, 100_000), 1_000);
    assert_eq!(msat_of(0, 100_000), 1_000);
    // 1,001 micros is 1.001 sats: two.
    assert_eq!(msat_of(1_001, 100_000), 2_000);
}

// ------------------------------------------------------------ OpenAPI

fn resolve<'a>(document: &'a Value, reference: &str) -> Option<&'a Value> {
    let pointer = reference.strip_prefix('#')?;
    document.pointer(pointer)
}

fn refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "$ref"
                    && let Some(text) = value.as_str()
                {
                    out.push(text.to_owned());
                }
                refs(value, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| refs(item, out)),
        _ => {}
    }
}

const METHODS: [&str; 5] = ["get", "post", "put", "delete", "patch"];

#[tokio::test]
async fn openapi_describes_exactly_the_mounted_inference_routes() {
    let d = deploy(true, false).await;
    let answer = reqwest::get(format!("{}/v1/openapi.json", d.address))
        .await
        .unwrap();
    assert_eq!(answer.status(), StatusCode::OK);
    let document: Value = answer.json().await.unwrap();
    assert_eq!(document["openapi"], "3.1.0");
    // The website's /openapi.json reads the same document at the root.
    let root: Value = reqwest::get(format!("{}/openapi.json", d.address))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(root, document);
    assert_eq!(document["servers"][0]["url"], format!("{}/v1", d.address));
    let paths = document["paths"].as_object().unwrap();
    // Payment is advertised because this deployment takes it.
    assert_eq!(
        paths["/responses"]["post"]["x-payment-info"]["offers"][0]["protocol"],
        "x402"
    );

    // The rate card answers in the shape the document gives it.
    let rates: Value = reqwest::get(format!("{}/v1/rates", d.address))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let schemas = &document["components"]["schemas"];
    for field in schemas["RateCard"]["required"].as_array().unwrap() {
        assert!(
            rates.get(field.as_str().unwrap()).is_some(),
            "rate card {field}"
        );
    }
    let rows = rates["rows"].as_array().unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        for field in schemas["RateRow"]["required"].as_array().unwrap() {
            assert!(
                row.get(field.as_str().unwrap()).is_some(),
                "rate row {field}"
            );
        }
    }

    // Every mounted inference path has an entry, and every entry a path.
    let documented: BTreeSet<String> = paths.keys().map(|path| format!("/v1{path}")).collect();
    let mut mounted: BTreeSet<String> = gateway::inference_openapi::inference_paths(&d.state)
        .into_iter()
        .map(str::to_owned)
        .collect();
    mounted.extend(
        gateway::inference_openapi::KEY_PATHS
            .iter()
            .map(|path| (*path).to_owned()),
    );
    // Every other PUBLIC route this deployment mounts is described too
    // (#11157): accounts, keys, workspaces, usage.
    let all = serve::mounted_paths(&d.state);
    mounted.extend(
        gateway::audience::of(gateway::audience::Audience::Public)
            .filter(|route| !route.ops.is_empty() && all.contains(&route.path))
            .map(|route| route.path.to_owned()),
    );
    assert!(mounted.contains("/v1/workspaces/{workspace}/usage"));
    assert!(mounted.contains("/v1/account"));
    assert_eq!(documented, mounted);
    for path in &documented {
        assert!(all.contains(&path.as_str()), "{path} is not mounted");
    }
    // Nothing FIRST-PARTY or INTERNAL is in the public document.
    for path in &all {
        if let Some(audience) = gateway::audience::audience(path)
            && audience != gateway::audience::Audience::Public
        {
            assert!(
                !documented.contains(*path),
                "{path} is {audience:?} but publicly documented"
            );
        }
    }
    // Every route this deployment mounts declares its audience (the
    // discovery documents aside, which are not API).
    let discovery = gateway::discovery::mounted_paths();
    for path in &all {
        assert!(
            discovery.contains(path) || gateway::audience::audience(path).is_some(),
            "{path} has no audience"
        );
    }

    // Each documented method is routed; each other one is 405.
    let client = reqwest::Client::new();
    for (path, item) in paths {
        let concrete = format!("{}/v1{}", d.address, path)
            .replace("{id}", "resp_0")
            .replace("{request_id}", "req_0")
            .replace("{workspace}", "ws_0")
            .replace("{key}", "key_0")
            .replace("{provider}", "openrouter");
        // Any other path parameter takes a placeholder id.
        let concrete: String = concrete
            .split('/')
            .map(|segment| {
                if segment.starts_with('{') && segment.ends_with('}') {
                    "x_0"
                } else {
                    segment
                }
            })
            .collect::<Vec<_>>()
            .join("/");
        for method in METHODS {
            let status = client
                .request(
                    reqwest::Method::from_bytes(method.to_uppercase().as_bytes()).unwrap(),
                    &concrete,
                )
                .header("content-type", "application/json")
                .body("{}")
                .send()
                .await
                .unwrap()
                .status();
            if item.get(method).is_some() {
                assert_ne!(
                    status,
                    StatusCode::METHOD_NOT_ALLOWED,
                    "{method} {path} is documented but not routed"
                );
            } else {
                assert_eq!(
                    status,
                    StatusCode::METHOD_NOT_ALLOWED,
                    "{method} {path} is routed but not documented"
                );
            }
        }
    }
}

#[tokio::test]
async fn openapi_is_well_formed() {
    let d = deploy(false, false).await;
    let document = gateway::inference_openapi::document(&d.address, &[]);
    assert!(
        document["paths"]["/responses"]["post"]
            .get("x-payment-info")
            .is_none(),
        "no payment advertised where none is taken"
    );
    for field in ["title", "version"] {
        assert!(document["info"][field].is_string(), "info.{field}");
    }
    // Every reference resolves.
    let mut found = Vec::new();
    refs(&document, &mut found);
    assert!(!found.is_empty());
    for reference in &found {
        assert!(
            resolve(&document, reference).is_some(),
            "{reference} does not resolve"
        );
    }
    let schemes = document["components"]["securitySchemes"]
        .as_object()
        .unwrap();
    let mut operation_ids = BTreeSet::new();
    for (path, item) in document["paths"].as_object().unwrap() {
        assert!(path.starts_with('/'), "{path}");
        let templated: BTreeSet<&str> = path
            .split('/')
            .filter_map(|segment| segment.strip_prefix('{')?.strip_suffix('}'))
            .collect();
        for (method, operation) in item.as_object().unwrap() {
            assert!(METHODS.contains(&method.as_str()), "{method} {path}");
            let id = operation["operationId"].as_str().expect("operationId");
            assert!(operation_ids.insert(id.to_owned()), "{id} twice");
            assert!(operation["summary"].is_string(), "{id} summary");
            let responses = operation["responses"].as_object().expect("responses");
            assert!(
                responses
                    .keys()
                    .any(|code| code.starts_with('2') || code == "101"),
                "{id} has no success response"
            );
            for (code, response) in responses {
                assert!(
                    code.len() == 3 && code.parse::<u16>().is_ok(),
                    "{id} {code}"
                );
                assert!(
                    response["description"].is_string(),
                    "{id} {code} description"
                );
            }
            let declared: BTreeSet<&str> = operation["parameters"]
                .as_array()
                .map(|parameters| {
                    parameters
                        .iter()
                        .filter(|parameter| parameter["in"] == "path")
                        .map(|parameter| {
                            assert_eq!(parameter["required"], true);
                            parameter["name"].as_str().unwrap()
                        })
                        .collect()
                })
                .unwrap_or_default();
            assert_eq!(declared, templated, "{id} path parameters");
            for requirement in operation["security"].as_array().expect("security") {
                for scheme in requirement.as_object().unwrap().keys() {
                    assert!(schemes.contains_key(scheme), "{id}: {scheme}");
                }
            }
        }
    }
}

// ------------------------------------------------------------ the router: MPP beside x402

/// One `Payment` challenge's auth-params, unescaped, as lnget reads them.
fn challenge_params(value: &str) -> Map<String, Value> {
    let rest = value.strip_prefix("Payment ").expect("a Payment challenge");
    let mut params = Map::new();
    let mut chars = rest.chars().peekable();
    loop {
        while matches!(chars.peek(), Some(' ' | ',')) {
            chars.next();
        }
        let name: String = chars.by_ref().take_while(|c| *c != '=').collect();
        if name.is_empty() {
            return params;
        }
        assert_eq!(chars.next(), Some('"'));
        let mut text = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => text.push(chars.next().unwrap()),
                '"' => break,
                c => text.push(c),
            }
        }
        params.insert(name, json!(text));
    }
}

fn mpp_invoice(challenge: &Answer) -> String {
    use base64::Engine;
    let params = challenge_params(
        challenge
            .header("www-authenticate")
            .expect("Payment challenge"),
    );
    let request: Value = serde_json::from_slice(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(params["request"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    request["methodDetails"]["invoice"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The 402's `Payment` challenge, paid: the `Authorization` to send.
fn pay_mpp(d: &Deployment, challenge: &Answer) -> String {
    use base64::Engine;
    let params = challenge_params(challenge.header("www-authenticate").unwrap());
    let preimage = d.node.pay(&mpp_invoice(challenge));
    let credential = json!({"challenge": params, "payload": {"preimage": preimage}});
    format!(
        "Payment {}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(credential.to_string())
    )
}

fn receipts(d: &Deployment) -> Vec<Value> {
    let dir = d._dir.path().join("inference").join("payment-receipts");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        out.push(serde_json::from_str(&text).unwrap());
    }
    out
}

#[tokio::test]
async fn one_402_carries_every_live_challenge_on_one_invoice() {
    let d = deploy_with(Pay::X402AndMpp, false).await;
    let body = json!({"model": PAID, "input": "Say hello.", "max_output_tokens": 100}).to_string();
    let challenge = post(&d, "/v1/responses", &body, None).await;
    assert_eq!(
        challenge.status,
        StatusCode::PAYMENT_REQUIRED,
        "{}",
        challenge.text
    );
    if std::env::var_os("OPENAGENTS_SHOW_402").is_some() {
        eprintln!(
            "--- 402 headers\n{:#?}\n--- 402 body\n{}",
            challenge.headers, challenge.text
        );
    }
    let required = decode_payment_required(challenge.header("PAYMENT-REQUIRED").unwrap()).unwrap();
    let invoice = required.accepts[0].extra["invoice"].as_str().unwrap();
    assert_eq!(
        invoice,
        mpp_invoice(&challenge),
        "one BOLT11 in both encodings"
    );
    let params = challenge_params(challenge.header("www-authenticate").unwrap());
    assert_eq!(params["method"], "lightning");
    assert_eq!(params["intent"], "charge");
    // No origin configured: the realm falls back to the public API's host.
    assert_eq!(params["realm"], "api.openagents.com");
    let refusal = challenge.json();
    assert_eq!(refusal["challengeId"], params["id"]);
    let methods: Vec<&str> = refusal["methods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|method| method["id"].as_str().unwrap())
        .collect();
    assert_eq!(methods, Pay::X402AndMpp.ids());
    assert_eq!(refusal["status"], 402);
    assert_eq!(
        refusal["docs"],
        "https://openagents.com/docs/api/for-agents"
    );
    assert!(
        required.extensions.as_ref().unwrap()["bazaar"]["info"]["input"]["method"] == "POST",
        "x402 Bazaar metadata"
    );
}

#[tokio::test]
async fn mpp_pays_once_and_the_same_invoice_cannot_pay_again_by_x402() {
    let d = deploy_with(Pay::X402AndMpp, false).await;
    let body = json!({"model": PAID, "input": "Say hello.", "max_output_tokens": 100}).to_string();
    let challenge = post(&d, "/v1/responses", &body, None).await;
    let authorization = pay_mpp(&d, &challenge);
    let paid = post_with(
        &d,
        "/v1/responses",
        &body,
        &[("Authorization", &authorization)],
    )
    .await;
    assert_eq!(paid.status, StatusCode::OK, "{}", paid.text);
    assert_eq!(paid.json()["output"][0]["content"][0]["text"], "hello");
    assert!(paid.header("payment-receipt").is_some(), "MPP receipt");
    assert!(paid.header("PAYMENT-RESPONSE").is_none());
    let receipt_id = paid.header("x-openagents-receipt").unwrap().to_owned();

    // The same payment, as x402: refused, with fresh terms.
    let signature = pay(&d, &challenge);
    let again = post(&d, "/v1/responses", &body, Some(&signature)).await;
    assert_eq!(again.status, StatusCode::PAYMENT_REQUIRED, "{}", again.text);
    assert_eq!(again.json()["reason"], "duplicate_settlement");
    assert!(again.header("PAYMENT-REQUIRED").is_some(), "fresh terms");
    // The same credential replayed: refused.
    let replay = post_with(
        &d,
        "/v1/responses",
        &body,
        &[("Authorization", &authorization)],
    )
    .await;
    assert_eq!(replay.status, StatusCode::PAYMENT_REQUIRED);
    assert_eq!(replay.json()["reason"], "duplicate_settlement");

    // One receipt, no bearer secret in it.
    let written = receipts(&d);
    assert_eq!(written.len(), 1);
    let receipt = &written[0];
    assert_eq!(receipt["v"], "openagents.payment-receipt.v1");
    assert_eq!(receipt["id"], receipt_id);
    assert_eq!(receipt["protocol"], "mpp");
    assert_eq!(receipt["rail"], "lightning");
    assert_eq!(receipt["outcome"], "served");
    assert_eq!(receipt["resource"], "POST /v1/responses");
    assert_eq!(receipt["amount"], required_amount(&challenge));
    let preimage = d.node.pay(&mpp_invoice(&challenge));
    let text = receipt.to_string();
    assert!(!text.contains(&preimage), "no preimage in a receipt");
    assert!(!text.contains("lnbc"), "no invoice in a receipt");
}

fn required_amount(challenge: &Answer) -> String {
    decode_payment_required(challenge.header("PAYMENT-REQUIRED").unwrap())
        .unwrap()
        .accepts[0]
        .amount
        .clone()
}

#[tokio::test]
async fn x402_pays_once_and_the_same_invoice_cannot_pay_again_by_mpp() {
    let d = deploy_with(Pay::X402AndMpp, false).await;
    let body = json!({"model": PAID, "input": "Hi.", "max_output_tokens": 40}).to_string();
    let challenge = post(&d, "/v1/responses", &body, None).await;
    let paid = post(&d, "/v1/responses", &body, Some(&pay(&d, &challenge))).await;
    assert_eq!(paid.status, StatusCode::OK, "{}", paid.text);
    assert!(paid.header("PAYMENT-RESPONSE").is_some());
    assert!(paid.header("x-openagents-receipt").is_some());
    let authorization = pay_mpp(&d, &challenge);
    let again = post_with(
        &d,
        "/v1/responses",
        &body,
        &[("Authorization", &authorization)],
    )
    .await;
    assert_eq!(again.status, StatusCode::PAYMENT_REQUIRED, "{}", again.text);
    assert_eq!(again.json()["reason"], "duplicate_settlement");
    assert!(
        again.header("www-authenticate").is_some(),
        "a fresh challenge"
    );
    let written = receipts(&d);
    assert_eq!(written.len(), 1);
    assert_eq!(written[0]["protocol"], "x402");
}

#[tokio::test]
async fn an_mpp_credential_pays_only_its_own_request() {
    let d = deploy_with(Pay::X402AndMpp, false).await;
    let body = json!({"model": PAID, "input": "One.", "max_output_tokens": 50}).to_string();
    let other = json!({"model": PAID, "input": "Two.", "max_output_tokens": 50}).to_string();
    let challenge = post(&d, "/v1/responses", &body, None).await;
    let authorization = pay_mpp(&d, &challenge);
    let refused = post_with(
        &d,
        "/v1/responses",
        &other,
        &[("Authorization", &authorization)],
    )
    .await;
    assert_eq!(
        refused.status,
        StatusCode::PAYMENT_REQUIRED,
        "{}",
        refused.text
    );
    assert_eq!(refused.json()["reason"], "digest_mismatch");
    let refused = post_with(
        &d,
        "/v1/chat/completions",
        &body,
        &[("Authorization", &authorization)],
    )
    .await;
    assert_ne!(refused.status, StatusCode::OK, "{}", refused.text);
    // Nothing was consumed: the original still runs, once.
    let paid = post_with(
        &d,
        "/v1/responses",
        &body,
        &[("Authorization", &authorization)],
    )
    .await;
    assert_eq!(paid.status, StatusCode::OK, "{}", paid.text);
    assert_eq!(receipts(&d).len(), 1);
    // A bearer key never goes the paid way, even with MPP on.
    let keyed = post_with(
        &d,
        "/v1/responses",
        &body,
        &[("Authorization", "Bearer oak_nope.nope")],
    )
    .await;
    assert_eq!(keyed.status, StatusCode::UNAUTHORIZED, "{}", keyed.text);
}

#[tokio::test]
async fn an_unanswered_mpp_payment_is_given_back_and_writes_no_receipt() {
    let d = deploy_with(Pay::X402AndMpp, true).await;
    let body = json!({"model": PAID, "input": "Hello?", "max_output_tokens": 10}).to_string();
    let challenge = post(&d, "/v1/responses", &body, None).await;
    let authorization = pay_mpp(&d, &challenge);
    for _ in 0..2 {
        let failed = post_with(
            &d,
            "/v1/responses",
            &body,
            &[("Authorization", &authorization)],
        )
        .await;
        assert_eq!(failed.status, StatusCode::BAD_GATEWAY, "{}", failed.text);
    }
    assert!(receipts(&d).is_empty());
}

/// #11137: every surface the gateway serves lists exactly the configured
/// methods; turning one off drops it everywhere.
#[tokio::test]
async fn discovery_lists_exactly_the_configured_methods() {
    for pay in [Pay::None, Pay::X402, Pay::X402AndMpp] {
        let d = deploy_with(pay, false).await;
        let document: Value = reqwest::get(format!("{}/v1/openapi.json", d.address))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let listed: Vec<&str> = document["x-openagents-payment-methods"]
            .as_array()
            .unwrap()
            .iter()
            .map(|method| method["id"].as_str().unwrap())
            .collect();
        assert_eq!(listed, pay.ids(), "{pay:?}: x-openagents-payment-methods");
        let schemes = document["components"]["securitySchemes"]
            .as_object()
            .unwrap();
        assert_eq!(schemes.contains_key("x402"), pay != Pay::None, "{pay:?}");
        assert_eq!(
            schemes.contains_key("payment"),
            pay == Pay::X402AndMpp,
            "{pay:?}"
        );
        for path in ["/responses", "/chat/completions"] {
            let operation = &document["paths"][path]["post"];
            let offers: Vec<&str> = operation["x-payment-info"]["offers"]
                .as_array()
                .map(|offers| {
                    offers
                        .iter()
                        .map(|o| o["protocol"].as_str().unwrap())
                        .collect()
                })
                .unwrap_or_default();
            assert_eq!(offers, pay.ids(), "{pay:?} {path}: offers");
            let security: Vec<String> = operation["security"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|r| r.as_object().unwrap().keys().cloned())
                .filter(|name| name != "bearer")
                .collect();
            assert_eq!(security.len(), pay.ids().len(), "{pay:?} {path}: security");
            let text = operation.to_string();
            assert_eq!(
                text.contains("PAYMENT-SIGNATURE"),
                pay != Pay::None,
                "{pay:?} {path}"
            );
            assert_eq!(
                text.contains("WWW-Authenticate"),
                pay == Pay::X402AndMpp,
                "{pay:?} {path}"
            );
        }
        let description = document["info"]["description"].as_str().unwrap();
        assert_eq!(description.contains("PAYMENT-SIGNATURE"), pay != Pay::None);
        assert_eq!(
            description.contains("Authorization: Payment"),
            pay == Pay::X402AndMpp
        );

        // The 402 itself carries the same set.
        let body = json!({"model": PAID, "input": "Hi.", "max_output_tokens": 10}).to_string();
        let answer = post(&d, "/v1/responses", &body, None).await;
        if pay == Pay::None {
            assert_eq!(answer.status, StatusCode::UNAUTHORIZED);
            continue;
        }
        assert_eq!(answer.status, StatusCode::PAYMENT_REQUIRED);
        let in_402: Vec<String> = answer.json()["methods"]
            .as_array()
            .unwrap()
            .iter()
            .map(|method| method["id"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(in_402, pay.ids());
        assert_eq!(answer.header("PAYMENT-REQUIRED").is_some(), true);
        assert_eq!(
            answer.header("www-authenticate").is_some(),
            pay == Pay::X402AndMpp
        );
    }
}
