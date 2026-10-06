//! Measured settlement conserves funded balance across failures and retries.
mod common;
use common::*;
use pay_ledger::Ledger;
use retail_cloud::{
    dispatch::{self, ExecutorEnd, TaskStatus},
    fake::{FakeProvider, FakeSandbox, FakeTaskOwner},
    journal::Journal,
    meter, recover, settle,
};
use route_contract::price_book::Ending;
fn started(
    j: &mut Journal,
    l: &mut Ledger,
    p: &FakeProvider,
    o: &FakeTaskOwner,
) -> (retail_cloud::offer::FundedRequest, String) {
    funded_account(l, "acct", 1000);
    let f = confirmed(j, "acct", "offer", &request(600));
    let r = delivered(j, l, p, &FakeSandbox::new(), &f);
    meter::dispatch_metered(
        j,
        l,
        p,
        o,
        &f,
        &rights(&f),
        &retail_cloud::contract::price_book(),
        NOW + 20,
    )
    .unwrap();
    (f, r)
}
fn complete(
    j: &mut Journal,
    l: &mut Ledger,
    p: &FakeProvider,
    o: &FakeTaskOwner,
    f: &retail_cloud::offer::FundedRequest,
    r: &str,
    end: ExecutorEnd,
) {
    o.set_status(
        r,
        &dispatch::task_id(&f.execution),
        TaskStatus::Ended {
            end,
            patch: None,
            checks: vec![],
        },
    );
    p.set_usage(r, 61);
    p.lose(r);
    meter::poll(j, p, &f.execution, "final", 1, NOW + 82).unwrap();
    recover::step(j, l, p, o, f, NOW + 83).unwrap();
}
#[test]
fn success_failure_and_preemption_conserve_balance_and_retry_one_debit() {
    for ending in [Ending::ExecutorEnded, Ending::ProviderLostAfterExecutor] {
        for end in [ExecutorEnd::Completed, ExecutorEnd::Failed] {
            let mut j = Journal::in_memory().unwrap();
            let mut l = Ledger::in_memory().unwrap();
            let p = FakeProvider::new();
            let o = FakeTaskOwner::new();
            let (f, r) = started(&mut j, &mut l, &p, &o);
            // Observe the owner's actual ending before provider loss hides it.
            o.set_status(
                &r,
                &dispatch::task_id(&f.execution),
                TaskStatus::Ended {
                    end,
                    patch: Some("candidate".into()),
                    checks: vec![dispatch::CheckRun {
                        command: f.task.checks[0].clone(),
                        candidate: "candidate".into(),
                        exit_status: if end == ExecutorEnd::Failed { 1 } else { 0 },
                    }],
                },
            );
            p.set_usage(&r, 61);
            recover::step(&mut j, &mut l, &p, &o, &f, NOW + 81).unwrap();
            p.lose(&r);
            meter::poll(&mut j, &p, &f.execution, "final", 1, NOW + 82).unwrap();
            if ending == Ending::ProviderLostAfterExecutor {
                recover::step(&mut j, &mut l, &p, &o, &f, NOW + 83).unwrap();
            }
            let receipt = settle::settle(&mut j, &mut l, &f, ending, NOW + 84).unwrap();
            assert_eq!(receipt.charge_msat, Some(103000));
            assert_eq!(receipt.released_msat, 21000);
            assert_eq!(
                receipt.checks,
                Some(if end == ExecutorEnd::Failed {
                    dispatch::Verdict::CheckFailed
                } else {
                    dispatch::Verdict::Verified
                })
            );
            assert_eq!(l.totals().unwrap().settlements, 1);
            assert_eq!(
                l.available_shares(pay_ledger::OPENAGENTS)
                    .unwrap()
                    .iter()
                    .map(|s| s.amount_msat)
                    .sum::<i64>(),
                103000
            );
            assert_eq!(
                settle::settle(&mut j, &mut l, &f, ending, NOW + 85).unwrap(),
                receipt
            );
            let balance = l.compute_balance("acct").unwrap();
            assert_eq!(balance.settled_msat, 103000);
            assert_eq!(balance.held_msat, 0);
            assert_eq!(balance.available_msat, 897000);
            assert!(settle::settle(&mut j, &mut l, &f, Ending::NotStarted, NOW + 86).is_err());
        }
    }
}
#[test]
fn unknown_provider_cost_stays_held_and_cannot_be_relabelled_free() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, r) = started(&mut j, &mut l, &p, &o);
    p.set_usage_unreadable(true);
    complete(&mut j, &mut l, &p, &o, &f, &r, ExecutorEnd::Completed);
    let receipt = settle::settle(
        &mut j,
        &mut l,
        &f,
        Ending::ProviderLostAfterExecutor,
        NOW + 84,
    )
    .unwrap();
    assert_eq!(receipt.charge_msat, None);
    assert_eq!(receipt.held_msat, 124000);
    assert_eq!(receipt.released_msat, 0);
    assert!(settle::settle(&mut j, &mut l, &f, Ending::NotStarted, NOW + 85).is_err());
    assert_eq!(l.compute_balance("acct").unwrap().settled_msat, 0);
}
#[test]
fn crash_after_atomic_debit_recovers_without_a_second_obligation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("journal.sqlite");
    let mut j = Journal::open(&path).unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, r) = started(&mut j, &mut l, &p, &o);
    complete(&mut j, &mut l, &p, &o, &f, &r, ExecutorEnd::Failed);
    // This is the committed money transaction at the crash boundary.
    l.settle_hold(&f.request, 103000, NOW + 84).unwrap();
    drop(j);
    let mut j = Journal::open(&path).unwrap();
    let receipt = settle::settle(
        &mut j,
        &mut l,
        &f,
        Ending::ProviderLostAfterExecutor,
        NOW + 90,
    )
    .unwrap();
    assert_eq!(receipt.settled_at, Some(NOW + 84));
    assert_eq!(receipt.source, Some(format!("debit:{}", f.request)));
    assert_eq!(l.compute_balance("acct").unwrap().settled_msat, 103000);
    assert_eq!(settle::observe(&j, &f, &rights(&f)).unwrap(), Some(receipt));
}
#[test]
fn reservation_only_releases_every_unused_sat_without_a_payment_refund() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    funded_account(&mut l, "acct", 1000);
    let f = confirmed(&mut j, "acct", "offer", &request(600));
    retail_cloud::reserve::reserve(&mut l, &f, &rights(&f), NOW + 2).unwrap();
    let receipt = settle::settle(&mut j, &mut l, &f, Ending::NotStarted, NOW + 3).unwrap();
    assert_eq!(receipt.charge_msat, Some(0));
    assert_eq!(receipt.released_msat, 124000);
    assert_eq!(receipt.source, None);
    assert_eq!(l.compute_balance("acct").unwrap().available_msat, 1000000);
}

#[test]
fn acknowledged_cancellation_charges_measured_usage_and_releases_unused_hold() {
    use retail_cloud::{
        cancel::{self, StopEvidence, StopOwner},
        dispatch::OwnerError,
        retain::{Artifacts, Manifest},
    };
    struct Stop;
    impl StopOwner for Stop {
        fn stop(&self, _: &str, _: &str, _: &str) -> Result<StopEvidence, OwnerError> {
            Ok(StopEvidence {
                at: NOW + 82,
                started: true,
                status: TaskStatus::Cancelled,
                effects: vec![],
            })
        }
        fn stopped(&self, _: &str, _: &str, _: &str) -> Result<Option<StopEvidence>, OwnerError> {
            Ok(None)
        }
    }
    impl Artifacts for Stop {
        fn manifest(&self, _: &str, _: &str) -> retail_cloud::Result<Manifest> {
            Err(retail_cloud::Error::Invalid(
                "fixture has no declared artifacts",
            ))
        }
        fn read(&self, _: &str, _: &str, _: &str, _: usize) -> retail_cloud::Result<Vec<u8>> {
            Err(retail_cloud::Error::Invalid(
                "fixture has no declared artifacts",
            ))
        }
    }
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, r) = started(&mut j, &mut l, &p, &o);
    p.set_usage(&r, 61);
    cancel::request(&mut j, &f, &rights(&f), NOW + 81).unwrap();
    let stopped = cancel::advance(&mut j, &l, &p, &Stop, &Stop, &f.execution, NOW + 82).unwrap();
    assert!(stopped.executor.is_some());
    let receipt = settle::settle(&mut j, &mut l, &f, Ending::Cancelled, NOW + 83).unwrap();
    assert_eq!(receipt.charge_msat, Some(103000));
    assert_eq!(receipt.released_msat, 21000);
    assert_eq!(l.compute_balance("acct").unwrap().held_msat, 0);
}
