//! A Pylon provider earns from an API request (#11070): the provider's
//! registration is an upstream with its own price, the caller's cost is
//! that price plus the margin, privacy follows the provider's stated
//! policy, the provider's share lands in the split ledger, and the payout
//! worker pays it out over the rails (a fake wallet here; no real money).

mod common;

use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use inference::rates::SatsRate;
use inference::upstream::pylon::{
    DataPolicy, Done, Job, Jobs, Offered, PylonUpstream, Registration,
};
use inference::upstream::{AttemptError, BoxFuture, Upstream};
use pay_ledger::payout::{Invoice, Lookup, Outcome, Policy, Rails, Step, tick};
use pay_ledger::{Ledger, Payee};
use serde_json::{Value, json};
use tenancy::{Registry, keys};

use gateway::config::{Config, Inference, SCHEMA};
use gateway::inference_pylon::LedgerEarnings;
use gateway::serve::{self, ServeState};

const MODEL: &str = "qwen/qwen3.5-9b";
const SPARK: &str = "spark1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq";

/// A provider that answers every job, reporting 100,000 tokens in and
/// 50,000 out, and keeps the jobs it got.
#[derive(Default)]
struct StubJobs {
    jobs: Mutex<Vec<(String, Job)>>,
}

impl Jobs for StubJobs {
    fn run<'a>(
        &'a self,
        provider: &'a Registration,
        job: Job,
    ) -> BoxFuture<'a, Result<Done, AttemptError>> {
        Box::pin(async move {
            let mut jobs = self.jobs.lock().unwrap();
            jobs.push((provider.pylon.clone(), job));
            Ok(Done {
                text: "from the pylon".into(),
                input_tokens: 100_000,
                output_tokens: 50_000,
                request: format!("{:064x}", jobs.len()),
                receipt: Some(format!("{:064x}", 1000 + jobs.len())),
            })
        })
    }
}

/// The payout rails, faked: records what would have been sent.
#[derive(Default)]
struct FakeRails {
    paid: Mutex<Vec<(String, u64)>>,
}

impl Rails for FakeRails {
    fn lightning_invoice(&self, _address: &str, _amount_msat: i64) -> Result<Invoice, String> {
        Err("not used".into())
    }
    fn pay_lightning(&self, _invoice: &Invoice, _max_fee_msat: i64) -> Outcome {
        Outcome::Failed("not used".into())
    }
    fn lookup_lightning(&self, _payment_hash: &str) -> Result<Lookup, String> {
        Ok(Lookup::Absent)
    }
    fn fund_spark(&self, _amount_sats: u64) -> Result<(), String> {
        Ok(())
    }
    fn pay_spark(&self, address: &str, amount_sats: u64, _key: &str) -> Outcome {
        self.paid
            .lock()
            .unwrap()
            .push((address.to_owned(), amount_sats));
        Outcome::Sent { fee_msat: 0 }
    }
    fn lookup_spark(&self, _key: &str) -> Result<Lookup, String> {
        Ok(Lookup::Sent { fee_msat: 0 })
    }
}

fn registration() -> Registration {
    Registration {
        provider: "ab".repeat(32),
        pylon: "box-1".into(),
        label: "Box".into(),
        owner: Some("pylon-owner".into()),
        models: vec![Offered {
            id: MODEL.into(),
            model: "qwen3.5-9b".into(),
            context: 32_768,
            max_output: 8_192,
            // $1 in and $2 out per million tokens: the provider's price.
            input_micros: 1_000_000,
            output_micros: 2_000_000,
        }],
        // Stated nothing about retention: standard requests only.
        policy: DataPolicy::default(),
    }
}

#[tokio::test]
async fn a_pylon_provider_earns_from_an_api_request_and_is_paid_out() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = common::manifest(&common::artifact('a'), None);
    let registry = Registry::install(dir.path(), manifest).unwrap();
    let token = keys::issue(dir.path(), registry.manifest(), "acme")
        .unwrap()
        .token;
    let ledger_path = dir.path().join("earnings.sqlite");
    let earnings = Arc::new(LedgerEarnings::new(
        Ledger::open(&ledger_path).unwrap(),
        SatsRate {
            usd_per_btc: 100_000,
            as_of: "2026-10-09".into(),
        },
    ));
    let jobs = Arc::new(StubJobs::default());
    let pylon: Arc<dyn Upstream> =
        Arc::new(PylonUpstream::new(registration(), jobs.clone(), earnings.clone()).unwrap());
    let inference: Inference = serde_json::from_value(json!({
        "admin_token_env": "INFERENCE_PYLON_TEST_ADMIN",
        "service_tenants": ["acme"],
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
    let state = ServeState::open_with_upstreams(config, Some(vec![pylon])).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    let post = |body: Value| {
        reqwest::Client::new()
            .post(format!("{address}/v1/responses"))
            .bearer_auth(&token)
            .json(&body)
            .send()
    };

    // Under `strict` (the default) a provider with no stated zero-retention
    // policy is not eligible.
    let strict = post(json!({"model": MODEL, "input": "hi"})).await.unwrap();
    assert_eq!(strict.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(jobs.jobs.lock().unwrap().is_empty());

    // Under `standard` the provider answers.
    let answer = post(json!({"model": MODEL, "instructions": "Be brief.",
                             "input": "hi", "openagents": {"privacy": "standard"}}))
    .await
    .unwrap();
    assert_eq!(answer.status(), StatusCode::OK);
    let body: Value = answer.json().await.unwrap();
    assert_eq!(body["output"][0]["content"][0]["text"], "from the pylon");
    assert_eq!(body["openagents"]["upstream"], "pylon:box-1");
    // The provider's price ($0.10 + $0.10) plus the 5% margin.
    assert_eq!(body["openagents"]["cost"]["upstream_usd"], "0.2");
    assert_eq!(body["openagents"]["cost"]["margin_usd"], "0.01");
    assert_eq!(body["openagents"]["cost"]["price_usd"], "0.21");
    let (pylon, job) = jobs.jobs.lock().unwrap()[0].clone();
    assert_eq!(pylon, "box-1");
    assert_eq!(job.model, "qwen3.5-9b");
    assert_eq!(job.task, "hi");
    assert_eq!(job.instructions.as_deref(), Some("Be brief."));

    // The provider's share is exactly its price: $0.20 at $100,000 a
    // bitcoin is 200 sats, owed to the pylon's owner.
    let owed = earnings
        .with(|ledger| ledger.accrued("pylon-owner").unwrap())
        .unwrap();
    assert_eq!(owed, 200_000);

    // The payout worker pays it over the rails once the owner has a
    // destination.
    earnings
        .with(|ledger| {
            ledger
                .register_payee(Payee {
                    party: "pylon-owner".into(),
                    destination_kind: "spark".into(),
                    destination_value: SPARK.into(),
                    source: "test".into(),
                    verified_at: 1,
                })
                .unwrap();
        })
        .unwrap();
    let rails = FakeRails::default();
    let policy = Policy {
        spark_threshold_msat: 1_000,
        ..Policy::default()
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let steps = earnings
        .with(|ledger| {
            let mut resolve = |l: &mut Ledger, party: &str, _: i64| l.payee(party);
            let mut next = 0u32;
            let mut new_id = || {
                next += 1;
                format!("00000000-0000-4000-8000-{next:012x}")
            };
            tick(ledger, &rails, &policy, now, &mut resolve, &mut new_id).unwrap()
        })
        .unwrap();
    assert!(
        steps
            .iter()
            .any(|step| matches!(step, Step::Payout { party, amount_msat: 200_000, .. } if party == "pylon-owner")),
        "{steps:?}"
    );
    assert_eq!(*rails.paid.lock().unwrap(), [(SPARK.to_owned(), 200)]);
    let owed = earnings
        .with(|ledger| ledger.accrued("pylon-owner").unwrap())
        .unwrap();
    assert_eq!(owed, 0);
}
