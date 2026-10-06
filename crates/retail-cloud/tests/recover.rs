//! Boundary faults reconcile one funded identity without creating effects.
mod common;
use common::*;
use pay_ledger::{Ledger, compute::HoldState};
use retail_cloud::{
    Error,
    dispatch::{self, CheckRun, DispatchState, ExecutorEnd, TaskStatus, Verdict},
    fake::{FakeProvider, FakeSandbox, FakeTaskOwner},
    journal::Journal,
    meter,
    offer::FundedRequest,
    recover::{self, State},
};
fn funded(j: &mut Journal, l: &mut Ledger) -> FundedRequest {
    funded_account(l, "acct", 1000);
    confirmed(j, "acct", "offer", &request(600))
}
fn running(
    j: &mut Journal,
    l: &mut Ledger,
    p: &FakeProvider,
    o: &FakeTaskOwner,
) -> (FundedRequest, String) {
    let f = funded(j, l);
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
#[test]
fn recovery_before_reservation_provision_or_dispatch_adds_no_effect() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let f = funded(&mut j, &mut l);
    assert!(matches!(
        recover::step(&mut j, &mut l, &p, &o, &f, NOW + 2)
            .unwrap()
            .state,
        State::AwaitingReservation
    ));
    retail_cloud::reserve::reserve(&mut l, &f, &rights(&f), NOW + 3).unwrap();
    assert!(matches!(
        recover::step(&mut j, &mut l, &p, &o, &f, NOW + 4)
            .unwrap()
            .state,
        State::AwaitingProvision
    ));
    assert_eq!(p.create_calls(), 0);
    assert_eq!(o.started(), 0);
    assert_eq!(l.compute_balance("acct").unwrap().held_msat, 124000);
}
#[test]
fn a_crash_after_provider_create_recovers_the_original_resource_observation() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let f = funded(&mut j, &mut l);
    retail_cloud::reserve::reserve(&mut l, &f, &rights(&f), NOW + 2).unwrap();
    p.lose_next_ack();
    retail_cloud::provision::advance(&mut j, &l, &p, &f, &rights(&f), TEMPLATE, NOW + 3).unwrap();
    let snapshot = recover::step(&mut j, &mut l, &p, &o, &f, NOW + 4).unwrap();
    assert!(matches!(snapshot.state, State::AwaitingMaterial { .. }));
    assert!(matches!(
        j.provisioning(&f.execution).unwrap().unwrap().state,
        retail_cloud::provision::ProvisionState::Ready { .. }
    ));
    assert_eq!(p.create_calls(), 1);
    assert_eq!(o.started(), 0);
}
#[test]
fn lost_dispatch_ack_and_process_restart_observe_one_original_task() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("journal.sqlite");
    let mut j = Journal::open(&path).unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    o.lose_next_ack();
    let (f, _) = running(&mut j, &mut l, &p, &o);
    assert_eq!(
        j.dispatch(&f.execution).unwrap().unwrap().state,
        DispatchState::Sent
    );
    drop(j);
    let mut j = Journal::open(&path).unwrap();
    let snapshot = recover::step(&mut j, &mut l, &p, &o, &f, NOW + 21).unwrap();
    assert!(matches!(
        snapshot.state,
        State::TaskObserved {
            status: TaskStatus::Running,
            ..
        }
    ));
    assert_eq!(
        j.dispatch(&f.execution).unwrap().unwrap().state,
        DispatchState::Acknowledged
    );
    assert_eq!(
        recover::step(&mut j, &mut l, &p, &o, &f, NOW + 22).unwrap(),
        snapshot
    );
    assert_eq!(recover::observe(&j, &f, &rights(&f)).unwrap().len(), 1);
    assert_eq!(o.started(), 1);
    assert_eq!(p.create_calls(), 1);
}
#[test]
fn failed_listing_and_stream_loss_never_replace_or_resubmit() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, _) = running(&mut j, &mut l, &p, &o);
    p.set_listing_fails(true);
    let unknown = recover::step(&mut j, &mut l, &p, &o, &f, NOW + 21).unwrap();
    assert!(matches!(
        unknown.state,
        State::Unknown { provider: true, .. }
    ));
    assert_eq!(
        l.hold(&f.request).unwrap().unwrap().state,
        HoldState::Unknown
    );
    p.set_listing_fails(false);
    o.set_unreachable(true);
    assert!(matches!(
        recover::step(&mut j, &mut l, &p, &o, &f, NOW + 22)
            .unwrap()
            .state,
        State::Unknown { task: true, .. }
    ));
    o.set_unreachable(false);
    assert!(matches!(
        recover::step(&mut j, &mut l, &p, &o, &f, NOW + 23)
            .unwrap()
            .state,
        State::TaskObserved { .. }
    ));
    assert_eq!(o.started(), 1);
    assert_eq!(p.create_calls(), 1);
    assert_eq!(l.compute_balance("acct").unwrap().settled_msat, 0);
}
#[test]
fn verified_provider_loss_requires_a_new_offer_and_preserves_prior_cost() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, r) = running(&mut j, &mut l, &p, &o);
    p.set_usage(&r, 61);
    p.lose(&r);
    let lost = recover::step(&mut j, &mut l, &p, &o, &f, NOW + 21).unwrap();
    assert!(matches!(lost.state, State::NewOfferRequired { .. }));
    assert_eq!(lost.usage.unwrap().seconds, Some(61));
    assert_eq!(lost.held_msat, 124000);
    assert!(lost.replacement.is_some());
    assert_eq!(o.started(), 1);
    assert_eq!(p.create_calls(), 1);
}
#[test]
fn original_independent_checks_remain_bound_to_the_exact_candidate() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, r) = running(&mut j, &mut l, &p, &o);
    let task = dispatch::task_id(&f.execution);
    o.set_status(
        &r,
        &task,
        TaskStatus::Ended {
            end: ExecutorEnd::Completed,
            patch: Some("candidate".into()),
            checks: vec![CheckRun {
                command: f.task.checks[0].clone(),
                candidate: "another-candidate".into(),
                exit_status: 0,
            }],
        },
    );
    let observed = recover::step(&mut j, &mut l, &p, &o, &f, NOW + 21).unwrap();
    assert_eq!(observed.checks, Some(Verdict::CheckFailed));
    assert_eq!(o.started(), 1);
}
#[test]
fn recovery_readers_cannot_relabel_the_original_funded_identity() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, _) = running(&mut j, &mut l, &p, &o);
    recover::step(&mut j, &mut l, &p, &o, &f, NOW + 21).unwrap();
    let mut forged = f.clone();
    forged.account = "another".into();
    assert!(matches!(
        recover::step(&mut j, &mut l, &p, &o, &forged, NOW + 22),
        Err(Error::Conflict(_))
    ));
    let mut revoked = rights(&f);
    revoked.observe.as_mut().unwrap().revoked = true;
    assert!(matches!(
        recover::observe(&j, &f, &revoked),
        Err(Error::Denied(_))
    ));
}

#[test]
fn recovery_names_only_a_retained_unexpired_checkpoint_without_resubmission() {
    use retail_cloud::retain::{self, Artifact, Artifacts, Kind, Manifest};
    struct Patch(Manifest, Vec<u8>);
    impl Artifacts for Patch {
        fn manifest(&self, _: &str, _: &str) -> retail_cloud::Result<Manifest> {
            Ok(self.0.clone())
        }
        fn read(&self, _: &str, _: &str, _: &str, _: usize) -> retail_cloud::Result<Vec<u8>> {
            Ok(self.1.clone())
        }
    }
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, r) = running(&mut j, &mut l, &p, &o);
    let bytes = b"diff --git a/parser.rs b/parser.rs\n".to_vec();
    let digest = retail_cloud::sha256_hex(&bytes);
    let artifacts = Patch(
        Manifest {
            execution: f.execution.clone(),
            task: dispatch::task_id(&f.execution),
            resource: r.clone(),
            source: f.admission.source.clone(),
            engine: "codex".into(),
            artifacts: vec![Artifact {
                name: "patch".into(),
                kind: Kind::Patch,
                digest: digest.clone(),
                size: bytes.len(),
            }],
        },
        bytes,
    );
    p.lose(&r);
    retain::request(&mut j, &f, NOW + 21).unwrap();
    let receipt = retain::advance(&mut j, &p, &artifacts, &f.execution, NOW + 22).unwrap();
    let snapshot = recover::step(&mut j, &mut l, &p, &o, &f, NOW + 23).unwrap();
    assert_eq!(snapshot.replacement.unwrap().checkpoint, Some(digest));
    let expired = recover::step(&mut j, &mut l, &p, &o, &f, receipt.expires_at).unwrap();
    assert_eq!(expired.replacement.unwrap().checkpoint, None);
    assert_eq!(o.started(), 1);
    assert_eq!(p.create_calls(), 1);
    assert_eq!(l.compute_balance("acct").unwrap().settled_msat, 0);
}
