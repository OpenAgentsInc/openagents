//! #10723, #10748: the bounded runner passes on fakes, the live adapters
//! qualify against simulated backends, and the funded step fails closed
//! without live bindings.

use retail_qualify::bindings::{self, BindingRefusal, Bindings};
use retail_qualify::qualify::{
    FundedRefusal, Mode, PlanRefusal, fixture, run_fake, run_funded, run_simulated,
};

#[test]
fn the_fixture_plan_qualifies_on_fakes_with_every_identity_retained() {
    let plan = fixture();
    plan.check().unwrap();
    let receipt = run_fake(&plan);
    assert!(receipt.qualified, "{receipt:#?}");
    assert_eq!(receipt.mode, Mode::Fake);
    assert!(receipt.label.starts_with("FAKE QUALIFICATION"));
    for identity in [
        &receipt.invoice_payment_hash,
        &receipt.execution,
        &receipt.hold,
        &receipt.sandbox,
        &receipt.task,
        &receipt.settlement_source,
    ] {
        assert!(identity.is_some(), "{receipt:#?}");
    }
    assert_eq!(receipt.check.as_deref(), Some("verified"));
    assert_eq!(receipt.charge_msat, Some(104_000));
    assert_eq!(receipt.released_msat, Some(20_000));
    assert_eq!(receipt.unknown_held_msat, 0);
    assert!(receipt.teardown_acknowledged && receipt.ledger_conserved);
    assert_eq!(receipt.plan, plan.digest());
}

#[test]
fn plans_outside_the_contract_refuse() {
    let base = fixture();
    let mut p = base.clone();
    p.top_up_sats = 5_000;
    assert_eq!(p.check(), Err(PlanRefusal::TopUp));
    let mut p = base.clone();
    p.computer_class = "gce-pool".into();
    assert_eq!(p.check(), Err(PlanRefusal::Class));
    let mut p = base.clone();
    p.source.repository = "https://gitlab.com/x/y".into();
    assert_eq!(p.check(), Err(PlanRefusal::Request));
    let mut p = base.clone();
    p.ceiling_sats = 100;
    assert_eq!(p.check(), Err(PlanRefusal::Ceiling));
    let mut p = base.clone();
    p.disclosure.pop();
    assert_eq!(p.check(), Err(PlanRefusal::Disclosure));
    let mut p = base.clone();
    p.cleanup_deadline_seconds = 60;
    assert_eq!(p.check(), Err(PlanRefusal::Cleanup));
    let receipt = run_fake(&p);
    assert!(!receipt.qualified);
    assert!(receipt.failure.unwrap().contains("Cleanup"));
}

#[test]
fn the_funded_step_refuses_without_live_bindings() {
    let plan = fixture();
    let env = |_: &str| None;
    assert_eq!(
        run_funded(&plan, "sha256:not-the-plan", None, &env),
        Err(FundedRefusal::NotConfirmed)
    );
    assert_eq!(
        run_funded(&plan, &plan.digest(), None, &env),
        Err(FundedRefusal::NoLiveBinding)
    );
}

fn live_bindings(state: &std::path::Path) -> Bindings {
    Bindings {
        schema: bindings::SCHEMA.into(),
        simulation: false,
        wallet_home: state.join("no-wallet-here"),
        boat_api_base: bindings::BOAT_API.into(),
        boat_org: Some("retail".into()),
        template: "oa-coder-main-20261006".into(),
        state_dir: state.join("state"),
        model_provider: "openai".into(),
        payment_wait_seconds: 900,
        poll_millis: 5_000,
    }
}

#[test]
fn funded_bindings_are_checked_before_anything_is_reached() {
    let plan = fixture();
    let tmp = tempfile::tempdir().unwrap();
    let keys = |name: &str| match name {
        bindings::BOAT_KEY_ENV => Some("retail-key".to_owned()),
        bindings::MODEL_KEY_ENV => Some("sk-test-customer".to_owned()),
        _ => None,
    };
    let refused = |b: &Bindings, env: &dyn Fn(&str) -> Option<String>| match run_funded(
        &plan,
        &plan.digest(),
        Some(b),
        env,
    ) {
        Err(FundedRefusal::Bindings { refusal }) => refusal,
        other => panic!("{other:?}"),
    };
    let base = live_bindings(tmp.path());

    let mut b = base.clone();
    b.simulation = true;
    assert_eq!(refused(&b, &keys), BindingRefusal::Simulation);
    let mut b = base.clone();
    b.boat_api_base = "http://127.0.0.1:9/api/v1".into();
    assert_eq!(refused(&b, &keys), BindingRefusal::BoatBase);
    let mut b = base.clone();
    b.template = "oa-coder-main-latest".into();
    assert_eq!(refused(&b, &keys), BindingRefusal::Template);
    assert_eq!(
        refused(&base, &|_: &str| None),
        BindingRefusal::BoatKeyMissing
    );
    // The operator allowance's key is never a retail key.
    let operator = |name: &str| match name {
        bindings::OPERATOR_BOAT_KEY_ENV => Some("retail-key".to_owned()),
        other => keys(other),
    };
    assert_eq!(
        refused(&base, &operator),
        BindingRefusal::OperatorCredential
    );
    let no_model = |name: &str| (name == bindings::BOAT_KEY_ENV).then(|| "retail-key".to_owned());
    assert_eq!(refused(&base, &no_model), BindingRefusal::ModelKeyMissing);
    std::fs::create_dir_all(base.state_dir.join("old")).unwrap();
    assert_eq!(refused(&base, &keys), BindingRefusal::StateDirInUse);
    std::fs::remove_dir_all(&base.state_dir).unwrap();
    // Every check passes; no resident wallet answers, so nothing is reached.
    assert_eq!(refused(&base, &keys), BindingRefusal::WalletUnreachable);
}

#[test]
fn the_live_adapters_qualify_against_simulated_backends() {
    let plan = fixture();
    let receipt = run_simulated(&plan);
    assert!(receipt.qualified, "{receipt:#?}");
    assert_eq!(receipt.mode, Mode::Simulated);
    assert!(receipt.label.starts_with("SIMULATION"));
    assert!(receipt.preimage_verified);
    assert_eq!(receipt.check.as_deref(), Some("verified"));
    assert_eq!(receipt.provider_seconds, Some(90));
    assert_eq!(receipt.unknown_held_msat, 0);
    assert!(receipt.teardown_acknowledged && receipt.ledger_conserved);
    assert!(
        receipt
            .charge_msat
            .is_some_and(|c| c > 0 && c <= i64::try_from(plan.ceiling_sats * 1000).unwrap())
    );
    let report = receipt.simulation.unwrap();
    // The first create reply was lost; the keyed retry found the same sandbox.
    assert_eq!(report.boat_create_requests, 2);
    assert_eq!(report.boat_sandboxes_created, 1);
    assert_eq!(report.boat_sandboxes_left, 0);
    assert_eq!(report.executors_started, 1);
    assert!(report.key_kept_off_command_lines && report.key_removed);
    assert_eq!(report.unauthorized_requests, 0);
}
