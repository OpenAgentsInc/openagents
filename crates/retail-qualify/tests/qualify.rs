//! #10723: the bounded runner passes on fakes and the funded step fails
//! closed.

use retail_qualify::qualify::{FundedRefusal, Mode, PlanRefusal, fixture, run_fake, run_funded};

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
fn the_funded_step_never_runs_in_this_build() {
    let plan = fixture();
    assert_eq!(
        run_funded(&plan, "sha256:not-the-plan"),
        Err(FundedRefusal::NotConfirmed)
    );
    assert_eq!(
        run_funded(&plan, &plan.digest()),
        Err(FundedRefusal::NoLiveBinding)
    );
}
