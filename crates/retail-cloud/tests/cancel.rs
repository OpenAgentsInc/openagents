//! Cancellation fixtures preserve separate effect, cleanup, and charge states.
mod common;
use common::*;
use pay_ledger::Ledger;
use retail_cloud::{
    Error, Result,
    cancel::{self, StopEvidence, StopOwner},
    dispatch::{self, ExecutorEnd, OwnerError, TaskStatus},
    fake::{FakeProvider, FakeSandbox, FakeTaskOwner},
    journal::Journal,
    meter,
    offer::FundedRequest,
    retain::{Artifacts, Manifest},
};
use std::cell::{Cell, RefCell};
struct Stopper {
    next: StopEvidence,
    stored: RefCell<Option<StopEvidence>>,
    calls: Cell<usize>,
    lost: Cell<bool>,
    unreachable: Cell<bool>,
}
impl Stopper {
    fn new(started: bool) -> Self {
        Self {
            next: StopEvidence {
                at: NOW + 23,
                started,
                status: TaskStatus::Cancelled,
                effects: if started {
                    vec!["a".repeat(64)]
                } else {
                    vec![]
                },
            },
            stored: RefCell::new(None),
            calls: Cell::new(0),
            lost: Cell::new(false),
            unreachable: Cell::new(false),
        }
    }
}
impl StopOwner for Stopper {
    fn stop(
        &self,
        _: &str,
        _: &str,
        request: &str,
    ) -> std::result::Result<StopEvidence, OwnerError> {
        assert!(request.starts_with("stop:rx_"));
        self.calls.set(self.calls.get() + 1);
        *self.stored.borrow_mut() = Some(self.next.clone());
        if self.lost.replace(false) {
            Err(OwnerError::Unknown("stop reply lost".into()))
        } else {
            Ok(self.next.clone())
        }
    }
    fn stopped(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> std::result::Result<Option<StopEvidence>, OwnerError> {
        if self.unreachable.get() {
            Err(OwnerError::Unknown("owner unreachable".into()))
        } else {
            Ok(self.stored.borrow().clone())
        }
    }
}
struct NoArtifacts;
impl Artifacts for NoArtifacts {
    fn manifest(&self, _: &str, _: &str) -> Result<Manifest> {
        Err(Error::Invalid("artifacts unavailable"))
    }
    fn read(&self, _: &str, _: &str, _: &str, _: usize) -> Result<Vec<u8>> {
        Err(Error::Invalid("artifacts unavailable"))
    }
}
fn started(j: &mut Journal, l: &mut Ledger, p: &FakeProvider) -> (FundedRequest, String) {
    funded_account(l, "acct", 1000);
    let f = confirmed(j, "acct", "offer", &request(600));
    let r = delivered(j, l, p, &FakeSandbox::new(), &f);
    p.set_usage(&r, 45);
    meter::dispatch_metered(
        j,
        l,
        p,
        &FakeTaskOwner::new(),
        &f,
        &rights(&f),
        &retail_cloud::contract::price_book(),
        NOW + 20,
    )
    .unwrap();
    p.set_usage(&r, 106);
    (f, r)
}
#[test]
fn cancel_before_provision_is_known_inert_and_does_not_spend() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    funded_account(&mut l, "acct", 1000);
    let f = confirmed(&mut j, "acct", "offer", &request(600));
    let mut current = rights(&f);
    current.spend = None;
    cancel::request(&mut j, &f, &current, NOW + 21).unwrap();
    let owner = Stopper::new(false);
    let receipt =
        cancel::advance(&mut j, &l, &p, &owner, &NoArtifacts, &f.execution, NOW + 22).unwrap();
    assert_eq!(receipt.charge.charge_sats, Some(0));
    assert_eq!(receipt.remaining_hold_msat, 0);
    assert_eq!(receipt.charge.released_sats, 0);
    assert_eq!(owner.calls.get(), 0);
    assert_eq!(p.create_calls(), 0);
    assert!(
        retail_cloud::provision::advance(&mut j, &l, &p, &f, &rights(&f), TEMPLATE, NOW + 23)
            .is_err()
    );
}
#[test]
fn a_lost_stop_reply_keeps_cost_unknown_until_exact_receipt_reconciliation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("journal.sqlite");
    let mut j = Journal::open(&path).unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let (f, _) = started(&mut j, &mut l, &p);
    let owner = Stopper::new(true);
    owner.lost.set(true);
    cancel::request(&mut j, &f, &rights(&f), NOW + 21).unwrap();
    let uncertain =
        cancel::advance(&mut j, &l, &p, &owner, &NoArtifacts, &f.execution, NOW + 23).unwrap();
    assert!(uncertain.stop_sent && uncertain.provider_deleted);
    assert!(uncertain.executor.is_none());
    assert_eq!(uncertain.charge.charge_sats, None);
    assert_eq!(uncertain.remaining_hold_msat, 124000);
    drop(j);
    let mut j = Journal::open(&path).unwrap();
    let known =
        cancel::advance(&mut j, &l, &p, &owner, &NoArtifacts, &f.execution, NOW + 24).unwrap();
    assert_eq!(known.charge.charge_sats, Some(103));
    assert_eq!(known.stop_latency_seconds, Some(2));
    assert_eq!(known.executor.unwrap().effects, vec!["a".repeat(64)]);
    assert_eq!(owner.calls.get(), 1);
    assert_eq!(p.delete_calls(), 1);
    assert_eq!(l.compute_balance("acct").unwrap().settled_msat, 0);
}
#[test]
fn lost_dispatch_acknowledgment_still_stops_the_original_task() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    funded_account(&mut l, "acct", 1000);
    let f = confirmed(&mut j, "acct", "offer", &request(600));
    let r = delivered(&mut j, &mut l, &p, &FakeSandbox::new(), &f);
    let task_owner = FakeTaskOwner::new();
    task_owner.lose_next_ack();
    meter::dispatch_metered(
        &mut j,
        &l,
        &p,
        &task_owner,
        &f,
        &rights(&f),
        &retail_cloud::contract::price_book(),
        NOW + 20,
    )
    .unwrap();
    p.set_usage(&r, 61);
    cancel::request(&mut j, &f, &rights(&f), NOW + 21).unwrap();
    let stopper = Stopper::new(true);
    let receipt = cancel::advance(
        &mut j,
        &l,
        &p,
        &stopper,
        &NoArtifacts,
        &f.execution,
        NOW + 23,
    )
    .unwrap();
    assert!(receipt.executor.is_some());
    assert_eq!(task_owner.started(), 1);
    assert_eq!(stopper.calls.get(), 1);
    assert_eq!(receipt.charge.charge_sats, Some(103));
}
#[test]
fn cancellation_after_effects_or_during_checks_retains_the_effect_identity() {
    for end in [ExecutorEnd::Failed, ExecutorEnd::Completed] {
        let mut j = Journal::in_memory().unwrap();
        let mut l = Ledger::in_memory().unwrap();
        let p = FakeProvider::new();
        let (f, _) = started(&mut j, &mut l, &p);
        let mut owner = Stopper::new(true);
        owner.next.status = TaskStatus::Ended {
            end,
            patch: Some("a".repeat(64)),
            checks: vec![],
        };
        owner.next.at = NOW + 20;
        cancel::request(&mut j, &f, &rights(&f), NOW + 21).unwrap();
        let receipt =
            cancel::advance(&mut j, &l, &p, &owner, &NoArtifacts, &f.execution, NOW + 23).unwrap();
        assert_eq!(receipt.stop_latency_seconds, Some(0));
        assert_eq!(receipt.charge.charge_sats, Some(103));
        assert_eq!(receipt.executor.unwrap().effects, vec!["a".repeat(64)]);
    }
}
#[test]
fn revocation_stops_new_work_and_preserves_cleanup_without_client_rights() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let (f, _) = started(&mut j, &mut l, &p);
    let mut revoked = rights(&f);
    revoked.execute.as_mut().unwrap().revoked = true;
    revoked.observe = None;
    revoked.spend = None;
    cancel::revoke(&mut j, &f, &revoked, NOW + 21).unwrap();
    assert!(
        dispatch::dispatch(&mut j, &l, &FakeTaskOwner::new(), &f, &rights(&f), NOW + 22).is_err()
    );
    let receipt = cancel::advance(
        &mut j,
        &l,
        &p,
        &Stopper::new(true),
        &NoArtifacts,
        &f.execution,
        NOW + 23,
    )
    .unwrap();
    assert_eq!(receipt.reason, cancel::Reason::Revoked);
    assert!(receipt.provider_deleted);
    assert_eq!(receipt.remaining_hold_msat, 124000);
}
#[test]
fn observation_after_reconnect_does_not_grant_control_or_spending() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let (f, _) = started(&mut j, &mut l, &p);
    cancel::request(&mut j, &f, &rights(&f), NOW + 21).unwrap();
    let owner = Stopper::new(true);
    cancel::advance(&mut j, &l, &p, &owner, &NoArtifacts, &f.execution, NOW + 23).unwrap();
    let mut observer = rights(&f);
    observer.execute = None;
    observer.spend = None;
    observer.disclose = None;
    assert!(
        cancel::observe(&j, &l, &f, &observer, NOW + 24)
            .unwrap()
            .is_some()
    );
    assert!(matches!(
        cancel::request(&mut j, &f, &observer, NOW + 24),
        Err(Error::Denied(_))
    ));
    assert!(cancel::revoke(&mut j, &f, &observer, NOW + 24).is_err());
    assert_eq!(owner.calls.get(), 1);
}
#[test]
fn delayed_final_usage_reconciles_unknown_provider_cost_after_stop() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let (f, _) = started(&mut j, &mut l, &p);
    cancel::request(&mut j, &f, &rights(&f), NOW + 21).unwrap();
    p.set_usage_unreadable(true);
    let owner = Stopper::new(true);
    let unknown =
        cancel::advance(&mut j, &l, &p, &owner, &NoArtifacts, &f.execution, NOW + 23).unwrap();
    assert_eq!(unknown.charge.charge_sats, None);
    assert_eq!(unknown.remaining_hold_msat, 124000);
    p.set_usage_unreadable(false);
    let known =
        cancel::advance(&mut j, &l, &p, &owner, &NoArtifacts, &f.execution, NOW + 24).unwrap();
    assert_eq!(known.charge.charge_sats, Some(103));
    assert_eq!(owner.calls.get(), 1);
}
