//! Real retail service and worker using the original canonical pool and custodian.
use super::*;
#[path = "../../../commercial-spend/tests/support/mod.rs"]
mod shared_fixture;
use shared_fixture::{FakeWallet as Custodian, Fixture as SharedFixture};
fn open_shared(shared: &SharedFixture, template: &Fixture) -> Service<Runtime, Custodian> {
    let mut config = template.config.clone();
    config.state = shared.root.path().join("retail-service");
    config.ledger = shared.ledger.clone();
    config.grants = vec![RetailGrant {
        principal: "cli:retail".into(),
        account: "retail".into(),
        generation: 1,
        observe: true,
        execute: true,
        disclose: true,
    }];
    let review: commercial_spend::Config =
        serde_json::from_slice(&std::fs::read(&shared.config).unwrap()).unwrap();
    Service::open(config, template.runtime.clone(), shared.fake.clone())
        .unwrap()
        .with_shared_spend(shared.retail.config.clone())
        .unwrap()
        .with_commercial(commercial::Config {
            canonical_directory: shared.canonical.clone(),
            issuer: shared.retail_binding.source.issuer.clone(),
            native: review.commercial,
        })
        .unwrap()
}
fn call_shared(s: &Service<Runtime, Custodian>, value: Value, at: i64) -> Result<Value> {
    s.call(
        "cli:retail",
        "synthetic-retail",
        serde_json::from_value(value).unwrap(),
        at,
    )
}
#[test]
fn actual_shared_service_worker_settles_once_and_omitted_config_cannot_spend() {
    let shared = SharedFixture::new(None);
    let template = Fixture::new();
    let at = commercial_spend::now() as i64;
    let s = open_shared(&shared, &template);
    let topup = json!({"op":"top_up","idempotency":"original-service-funding","amount_sats":1000});
    let p = call_shared(&s, topup.clone(), at).unwrap();
    assert_eq!(p, call_shared(&s, topup, at + 1).unwrap());
    assert_eq!(shared.fake.incoming.load(Ordering::SeqCst), 1);
    let purchase = p["result"]["purchase"].as_str().unwrap();
    let paid = call_shared(
        &s,
        json!({"op":"top_up_status","purchase":purchase}),
        at + 1,
    )
    .unwrap();
    assert_eq!(paid["result"]["state"], "paid");
    assert_eq!(
        paid["result"]["commercial"],
        json!(shared.retail_binding.commercial)
    );
    let made = call_shared(
        &s,
        json!({"op":"offer","idempotency":"original-compute","task":request()}),
        at + 2,
    )
    .unwrap()["result"]
        .clone();
    assert_eq!(
        made["custody"]["terms"]["commercial"],
        json!(shared.retail_binding.commercial)
    );
    let accepted = call_shared(&s, confirmation(&made), at + 3).unwrap();
    let execution = accepted["result"]["execution"]
        .as_str()
        .unwrap()
        .to_string();
    template.runtime.provider.lose_next_ack();
    s.tick(at + 4).unwrap();
    assert_eq!(template.runtime.provider.create_calls(), 1);
    drop(s);
    let s = open_shared(&shared, &template);
    let mut resource = None;
    for n in 5..17 {
        s.tick(at + n).unwrap();
        if let Some(d) = s.lock().unwrap().journal.dispatch(&execution).unwrap() {
            resource = Some(d.resource);
            break;
        }
    }
    let resource = resource.expect("original retained compute dispatch");
    assert_eq!(template.runtime.provider.create_calls(), 1);
    assert_eq!(template.runtime.owner.started(), 1);
    let patch = retail_cloud::sha256_hex(b"synthetic patch\n");
    template.runtime.owner.set_status(
        &resource,
        &retail_cloud::dispatch::task_id(&execution),
        TaskStatus::Ended {
            end: retail_cloud::dispatch::ExecutorEnd::Completed,
            patch: Some(patch.clone()),
            checks: vec![retail_cloud::dispatch::CheckRun {
                command: "cargo test -p parser".into(),
                candidate: patch,
                exit_status: 0,
            }],
        },
    );
    template.runtime.provider.set_usage(&resource, 17);
    s.tick(at + 40).unwrap();
    let receipt = call_shared(&s, json!({"op":"receipt","execution":execution}), at + 41).unwrap();
    assert_eq!(receipt["result"]["settlement"]["checks"], "verified");
    assert_eq!(receipt["result"]["settlement"]["ending"], "executor_ended");
    assert_eq!(
        receipt["result"]["commercial"],
        json!(shared.retail_binding.commercial)
    );
    let ledger = Ledger::open_read_only(&shared.ledger).unwrap();
    let balance = ledger.compute_balance("retail").unwrap();
    assert_eq!(balance.credited_msat, 1_000_000);
    assert!(balance.settled_msat > 0);
    assert_eq!(
        balance.available_msat + balance.held_msat + balance.settled_msat,
        balance.credited_msat
    );
    let original = ledger.hold_for_execution(&execution).unwrap().unwrap();
    assert_eq!(original.charge_msat, Some(balance.settled_msat));
    s.tick(at + 42).unwrap();
    assert_eq!(ledger.compute_balance("retail").unwrap(), balance);
    drop(s);
    // The native book remains nonspendable when its controller configuration is omitted.
    let mut config = template.config.clone();
    config.state = shared.root.path().join("retail-service");
    config.ledger = shared.ledger.clone();
    config.grants = vec![RetailGrant {
        principal: "cli:retail".into(),
        account: "retail".into(),
        generation: 1,
        observe: true,
        execute: true,
        disclose: true,
    }];
    let omitted = Service::open(config, template.runtime.clone(), shared.fake.clone()).unwrap();
    assert!(
        call_shared(
            &omitted,
            json!({"op":"top_up","idempotency":"omitted","amount_sats":1}),
            at + 43
        )
        .is_err()
    );
    assert!(
        call_shared(
            &omitted,
            json!({"op":"offer","idempotency":"omitted","task":request()}),
            at + 43
        )
        .is_err()
    );
    assert_eq!(template.runtime.provider.create_calls(), 1);
    assert_eq!(template.runtime.owner.started(), 1);
    assert_eq!(shared.fake.incoming.load(Ordering::SeqCst), 1);
    assert_eq!(ledger.compute_balance("retail").unwrap(), balance);
}
