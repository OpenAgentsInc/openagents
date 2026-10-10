//! A Pylon provider sells through its own receiver behind the gateway
//! (#11149, topology 4A of the merchant-hosted receiver profile): the
//! gateway buys each job at the provider's own `payTo` before sending it,
//! so the answer is served with nothing in the split ledger and no payout
//! owed. An answer that arrives without that direct payment is not
//! served. The payer only pays invoices on the provider's registered node.
//! Test invoices and a stub provider; no wallet, no money.

mod common;

use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use gateway::config::{Config, Inference, SCHEMA};
use gateway::inference_pylon::{LedgerEarnings, Pinned};
use gateway::serve::{self, ServeState};
use inference::rates::SatsRate;
use inference::upstream::pylon::{
    DataPolicy, Done, Job, Jobs, Offered, PylonUpstream, Registration, Settlement,
};
use inference::upstream::{AttemptError, BoxFuture, Upstream};
use nostr::x402::test_invoice::{number, payee_of, signed_by, tag, words};
use pay_ledger::Ledger;
use pylon::paid::{Invoice, Network, Payer};
use serde_json::{Value, json};
use tenancy::{Registry, keys};

const MODEL: &str = "qwen/qwen3.5-9b";
const PROVIDER_NODE: [u8; 32] = [5; 32];
const OTHER_NODE: [u8; 32] = [6; 32];

/// A provider that answers every job; `paid` is what the gateway's direct
/// payment for it came to (`None`: it was never bought).
struct StubJobs {
    paid: Option<u64>,
    jobs: Mutex<Vec<String>>,
}

impl Jobs for StubJobs {
    fn run<'a>(
        &'a self,
        provider: &'a Registration,
        _job: Job,
    ) -> BoxFuture<'a, Result<Done, AttemptError>> {
        Box::pin(async move {
            let mut jobs = self.jobs.lock().unwrap();
            jobs.push(provider.pylon.clone());
            Ok(Done {
                text: "from the merchant pylon".into(),
                input_tokens: 100_000,
                output_tokens: 50_000,
                request: format!("{:064x}", jobs.len()),
                receipt: Some(format!("{:064x}", 1000 + jobs.len())),
                paid_msat: self.paid,
            })
        })
    }
}

fn registration() -> Registration {
    Registration {
        provider: "cd".repeat(32),
        pylon: "merchant-1".into(),
        label: "Merchant".into(),
        owner: Some("merchant-owner".into()),
        models: vec![Offered {
            id: MODEL.into(),
            model: "qwen3.5-9b".into(),
            context: 32_768,
            max_output: 8_192,
            input_micros: 1_000_000,
            output_micros: 2_000_000,
        }],
        policy: DataPolicy::default(),
        settlement: Settlement::Merchant {
            pay_to: hex::encode(payee_of(PROVIDER_NODE)),
            max_msat: 250_000,
        },
    }
}

fn config(dir: &std::path::Path) -> Config {
    let inference: Inference = serde_json::from_value(json!({
        "admin_token_env": "INFERENCE_PYLON_MERCHANT_TEST_ADMIN",
        "service_tenants": ["acme"],
    }))
    .unwrap();
    Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.to_path_buf(),
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
    }
}

/// Serve one merchant-settled provider whose jobs come back `paid`, with a
/// split ledger present (so a wrongly recorded share would show), and ask
/// it once. Returns the status, the body, the jobs sent, and every
/// settlement the ledger holds.
async fn ask_once(paid: Option<u64>) -> (StatusCode, Value, usize, usize) {
    let dir = tempfile::tempdir().unwrap();
    let manifest = common::manifest(&common::artifact('a'), None);
    let registry = Registry::install(dir.path(), manifest).unwrap();
    let token = keys::issue(dir.path(), registry.manifest(), "acme")
        .unwrap()
        .token;
    let earnings = Arc::new(LedgerEarnings::new(
        Ledger::open(dir.path().join("earnings.sqlite")).unwrap(),
        SatsRate {
            usd_per_btc: 100_000,
            as_of: "2026-10-09".into(),
        },
    ));
    let jobs = Arc::new(StubJobs {
        paid,
        jobs: Mutex::new(Vec::new()),
    });
    let pylon: Arc<dyn Upstream> =
        Arc::new(PylonUpstream::new(registration(), jobs.clone(), earnings.clone()).unwrap());
    let state = ServeState::open_with_upstreams(config(dir.path()), Some(vec![pylon])).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    let response = reqwest::Client::new()
        .post(format!("{address}/v1/responses"))
        .bearer_auth(&token)
        .json(&json!({"model": MODEL, "input": "hi",
                      "openagents": {"privacy": "standard"}}))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    let sent = jobs.jobs.lock().unwrap().len();
    let recorded = earnings
        .with(|ledger| ledger.since(0).unwrap().len())
        .unwrap();
    (status, body, sent, recorded)
}

#[tokio::test]
async fn a_merchant_pylon_is_paid_at_its_own_receiver_and_nothing_enters_the_ledger() {
    let (status, body, sent, recorded) = ask_once(Some(210_000)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["output"][0]["content"][0]["text"],
        "from the merchant pylon"
    );
    assert_eq!(body["openagents"]["upstream"], "pylon:merchant-1");
    assert_eq!(sent, 1);
    // Paid directly: no share recorded, so the payout worker owes nothing.
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn an_answer_without_the_direct_payment_is_not_served() {
    let (status, _body, sent, recorded) = ask_once(None).await;
    assert_ne!(status, StatusCode::OK);
    assert!(sent >= 1);
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn a_payment_over_the_registered_ceiling_is_not_served() {
    let (status, _body, _sent, recorded) = ask_once(Some(250_001)).await;
    assert_ne!(status, StatusCode::OK);
    assert_eq!(recorded, 0);
}

/// A payer that records what it was asked to pay.
#[derive(Default)]
struct Recording(Mutex<Vec<String>>);

impl Payer for Recording {
    fn network(&self) -> Network {
        Network::Testnet
    }
    fn pay(&self, invoice: &Invoice) -> Result<String, String> {
        self.0.lock().unwrap().push(invoice.bolt11.clone());
        Ok("00".repeat(32))
    }
}

fn invoice(node: [u8; 32], amount_msat: u64) -> Invoice {
    let mut fields = tag(1, &words(&[3; 32]));
    fields.extend(tag(16, &words(&[2; 32])));
    fields.extend(tag(23, &words(&[4; 32])));
    fields.extend(tag(6, &number(300)));
    Invoice {
        bolt11: signed_by(
            node,
            &format!("lntb{}n", amount_msat / 100),
            fields,
            false,
            false,
            1_800_000_000,
        ),
        payment_hash: hex::encode([3u8; 32]),
        amount_msat,
    }
}

#[test]
fn the_payer_pays_only_the_registered_node() {
    let inner = Arc::new(Recording::default());
    let pinned = Pinned::new(inner.clone(), &hex::encode(payee_of(PROVIDER_NODE)));
    assert_eq!(pinned.network(), Network::Testnet);

    // The provider's own invoice is paid.
    assert!(pinned.pay(&invoice(PROVIDER_NODE, 21_000)).is_ok());
    assert_eq!(inner.0.lock().unwrap().len(), 1);

    // An invoice on any other node is refused before anything is paid.
    assert!(pinned.pay(&invoice(OTHER_NODE, 21_000)).is_err());
    // And one whose amount is not the offered one.
    let mut changed = invoice(PROVIDER_NODE, 21_000);
    changed.amount_msat = 1_000;
    assert!(pinned.pay(&changed).is_err());
    assert_eq!(inner.0.lock().unwrap().len(), 1);
}
