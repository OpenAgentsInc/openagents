//! #10724: the service advertises no paid capacity before a funded
//! qualification, and operators see and reconcile stuck work.

use retail_cloud::offer::Capacity;
use retail_cloud::recover;
use retail_qualify::harness::{World, request, rights};
use retail_qualify::launch::{Alert, Closed, Gate, advertise, health};
use retail_qualify::qualify::{Mode, fixture, run_fake};

const FREE: Capacity = Capacity {
    running: 0,
    plan_starts_left: Some(10),
};

#[test]
fn paid_capacity_stays_closed_until_a_funded_qualification() {
    let plan = fixture();
    let fake = run_fake(&plan);
    assert!(fake.qualified);
    let gate = |confirmed, receipt, capacity| Gate {
        contract_confirmed: confirmed,
        qualification: receipt,
        supported_plan: plan.digest(),
        capacity,
    };
    let closed = |g: &Gate| advertise(g).closed;
    assert_eq!(
        closed(&gate(false, None, FREE)),
        Some(Closed::ContractUnconfirmed)
    );
    assert_eq!(
        closed(&gate(true, None, FREE)),
        Some(Closed::NoFundedQualification)
    );
    // A fake receipt never opens the gate.
    assert_eq!(
        closed(&gate(true, Some(fake.clone()), FREE)),
        Some(Closed::QualificationNotValid)
    );
    let mut funded = fake.clone();
    funded.mode = Mode::Funded;
    let mut failed = funded.clone();
    failed.qualified = false;
    assert_eq!(
        closed(&gate(true, Some(failed), FREE)),
        Some(Closed::QualificationNotValid)
    );
    let mut other = funded.clone();
    other.plan = "sha256:another-configuration".into();
    assert_eq!(
        closed(&gate(true, Some(other), FREE)),
        Some(Closed::QualificationNotValid)
    );
    let full = Capacity {
        running: 4,
        plan_starts_left: Some(1),
    };
    assert_eq!(
        closed(&gate(true, Some(funded.clone()), full)),
        Some(Closed::NoCapacity)
    );
    let open = advertise(&gate(true, Some(funded), FREE));
    assert_eq!(open.paid_capacity.as_deref(), Some("retail-boat-large-v1"));
    assert!(open.hosted_inference.contains("not for sale"));
    assert!(open.operator_placements.contains("not for sale"));
    // Closed advertisements still label the other placements.
    let shut = advertise(&gate(false, None, FREE));
    assert!(shut.paid_capacity.is_none());
    assert!(shut.hosted_inference.contains("not a paid computer"));
}

#[test]
fn operators_see_stuck_work_and_reconcile_without_new_execution() {
    let mut w = World::new().unwrap();
    w.account("acct", 600).unwrap();
    // A reservation that never provisions.
    let stuck = w.confirm("acct", "cf-stuck", &request(600)).unwrap();
    retail_cloud::reserve::reserve(&mut w.ledger, &stuck, &rights(&stuck), w.now).unwrap();
    // A dispatch whose acknowledgment is lost.
    let lost = w.confirm("acct", "cf-lost", &request(600)).unwrap();
    let resource = w.ready(&lost).unwrap();
    w.deliver(&lost, &resource).unwrap();
    w.owner.lose_next_ack();
    w.dispatch(&lost).unwrap();
    let later = w.tick(20 * 60);
    let alerts = health(&w.journal, &w.ledger, later).unwrap();
    assert!(alerts.iter().any(|a| matches!(a, Alert::StuckReservation { execution, .. } if execution == &stuck.execution)), "{alerts:#?}");
    assert!(alerts.iter().any(|a| matches!(a, Alert::UncertainDispatch { execution, .. } if execution == &lost.execution)), "{alerts:#?}");
    assert!(
        !alerts
            .iter()
            .any(|a| matches!(a, Alert::LedgerDrift { .. }))
    );

    // The operator reconciles: the original task is found, nothing new runs.
    recover::step(
        &mut w.journal,
        &mut w.ledger,
        &w.provider,
        &w.owner,
        &lost,
        later + 1,
    )
    .unwrap();
    assert_eq!(w.owner.started(), 1);
    assert_eq!(w.provider.create_calls(), 1);
    let alerts = health(&w.journal, &w.ledger, later + 2).unwrap();
    assert!(
        !alerts
            .iter()
            .any(|a| matches!(a, Alert::UncertainDispatch { .. })),
        "{alerts:#?}"
    );
    // Liabilities stay: both holds are still held.
    assert_eq!(w.ledger.compute_balance("acct").unwrap().held_msat, 248_000);
}
