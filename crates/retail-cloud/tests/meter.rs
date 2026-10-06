//! Metering fixtures use only fake provider usage and temporary journals.
mod common;
use common::*;
use pay_ledger::Ledger;
use retail_cloud::{
    Error, contract,
    fake::{FakeProvider, FakeSandbox, FakeTaskOwner},
    journal::Journal,
    meter::{self, ModelUsage, Reading},
    offer::FundedRequest,
};
use route_contract::price_book::Ending;

fn start(
    j: &mut Journal,
    l: &mut Ledger,
    p: &FakeProvider,
    o: &FakeTaskOwner,
) -> (FundedRequest, String) {
    funded_account(l, "acct", 1000);
    let f = confirmed(j, "acct", "offer", &request(600));
    let resource = delivered(j, l, p, &FakeSandbox::new(), &f);
    p.set_usage(&resource, 45);
    meter::dispatch_metered(
        j,
        l,
        p,
        o,
        &f,
        &rights(&f),
        &contract::price_book(),
        NOW + 20,
    )
    .unwrap();
    (f, resource)
}
fn reading(resource: &str, sequence: u32, seconds: Option<u64>, stopped: bool) -> Reading {
    Reading {
        event: format!("provider:{sequence}"),
        sequence,
        resource: resource.into(),
        at: NOW + 20 + i64::from(sequence),
        seconds,
        stopped,
        model: None,
    }
}
#[test]
fn original_quote_and_baseline_survive_restart_and_redelivery() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("journal.sqlite");
    let mut j = Journal::open(&path).unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, r) = start(&mut j, &mut l, &p, &o);
    let sample = reading(&r, 1, Some(106), true);
    j.record_usage(&f.execution, &sample).unwrap();
    j.record_usage(&f.execution, &sample).unwrap();
    let first = j.usage(&f.execution).unwrap().unwrap();
    drop(j);
    let mut j = Journal::open(&path).unwrap();
    let recovered = j.usage(&f.execution).unwrap().unwrap();
    assert_eq!(first, recovered);
    assert_eq!(recovered.seconds, Some(61));
    assert_eq!(recovered.events.len(), 1);
    let charge = recovered.settlement(&f, Ending::ExecutorEnded);
    assert_eq!(charge.charge_sats, Some(103));
    assert_eq!(charge.released_sats, 21);
    let mut changed = sample;
    changed.seconds = Some(107);
    assert!(matches!(
        j.record_usage(&f.execution, &changed),
        Err(Error::Conflict(_))
    ));
    let mut book = contract::price_book();
    book.classes[0].compute_msats_per_second += 1;
    assert!(meter::dispatch_metered(&mut j, &l, &p, &o, &f, &rights(&f), &book, NOW + 50).is_err());
    assert_eq!(o.started(), 1);
}
#[test]
fn reordered_and_missing_events_are_unknown_until_complete() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, r) = start(&mut j, &mut l, &p, &o);
    j.record_usage(&f.execution, &reading(&r, 2, Some(100), true))
        .unwrap();
    let gap = j.usage(&f.execution).unwrap().unwrap();
    assert_eq!(gap.seconds, None);
    assert_eq!(gap.settlement(&f, Ending::ExecutorEnded).held_sats, 124);
    j.record_usage(&f.execution, &reading(&r, 1, None, false))
        .unwrap();
    assert_eq!(j.usage(&f.execution).unwrap().unwrap().seconds, Some(55));
}
#[test]
fn counter_regression_wrong_resource_and_duplicate_identity_refuse() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, r) = start(&mut j, &mut l, &p, &o);
    j.record_usage(&f.execution, &reading(&r, 1, Some(100), false))
        .unwrap();
    let mut bad = reading(&r, 2, Some(90), true);
    j.record_usage(&f.execution, &bad).unwrap();
    assert_eq!(j.usage(&f.execution).unwrap().unwrap().seconds, None);
    bad.resource = "another-customer".into();
    assert!(j.record_usage(&f.execution, &bad).is_err());
    let mut conflict = reading(&r, 3, Some(110), true);
    conflict.event = "provider:1".into();
    assert!(j.record_usage(&f.execution, &conflict).is_err());
}
#[test]
fn model_usage_is_attributed_but_never_added_to_compute_charge() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, r) = start(&mut j, &mut l, &p, &o);
    let mut sample = reading(&r, 1, Some(106), true);
    sample.model = Some(ModelUsage {
        provider: "openai".into(),
        reference: "provider-receipt-1".into(),
        input_tokens: Some(50000),
        output_tokens: None,
    });
    j.record_usage(&f.execution, &sample).unwrap();
    let usage = j.usage(&f.execution).unwrap().unwrap();
    assert_eq!(
        usage.settlement(&f, Ending::ExecutorEnded).charge_sats,
        Some(103)
    );
    assert_eq!(usage.events[0].model, sample.model);
    sample.sequence = 2;
    sample.event = "provider:2".into();
    sample.model.as_mut().unwrap().provider = "wrong-provider".into();
    assert!(j.record_usage(&f.execution, &sample).is_err());
}
#[test]
fn ceiling_stops_the_exact_sandbox_and_never_expands_the_charge() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, r) = start(&mut j, &mut l, &p, &o);
    p.set_usage(&r, 800);
    let usage = meter::poll(&mut j, &p, &f.execution, "poll1", 1, NOW + 700).unwrap();
    assert!(usage.ceiling_reached);
    assert_eq!(
        usage.settlement(&f, Ending::ExecutorEnded).charge_sats,
        None
    );
    let stopped = meter::enforce_ceiling(&mut j, &p, &f.execution, NOW + 700).unwrap();
    assert!(stopped.stop_requested && stopped.stop_acknowledged);
    assert!(p.active().is_empty());
    assert!(
        meter::dispatch_metered(
            &mut j,
            &l,
            &p,
            &o,
            &f,
            &rights(&f),
            &contract::price_book(),
            NOW + 701
        )
        .is_err()
    );
    let final_usage = meter::poll(&mut j, &p, &f.execution, "poll2", 2, NOW + 702).unwrap();
    assert_eq!(
        final_usage.settlement(&f, Ending::Cancelled).charge_sats,
        Some(124)
    );
    assert_eq!(o.started(), 1);
}
#[test]
fn unreadable_usage_is_held_and_wall_deadline_still_stops() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, _) = start(&mut j, &mut l, &p, &o);
    p.set_usage_unreadable(true);
    let usage = meter::poll(&mut j, &p, &f.execution, "missing", 1, NOW + 620).unwrap();
    assert_eq!(usage.seconds, None);
    assert_eq!(usage.settlement(&f, Ending::ExecutorEnded).held_sats, 124);
    assert!(
        meter::enforce_ceiling(&mut j, &p, &f.execution, NOW + 621)
            .unwrap()
            .stop_acknowledged
    );
}
#[test]
fn unknown_baseline_prevents_dispatch_and_changed_funding_is_rejected() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    funded_account(&mut l, "acct", 1000);
    let mut f = confirmed(&mut j, "acct", "offer", &request(600));
    delivered(&mut j, &mut l, &p, &FakeSandbox::new(), &f);
    p.set_usage_unreadable(true);
    assert!(
        meter::dispatch_metered(
            &mut j,
            &l,
            &p,
            &o,
            &f,
            &rights(&f),
            &contract::price_book(),
            NOW + 20
        )
        .is_err()
    );
    assert_eq!(o.started(), 0);
    p.set_usage_unreadable(false);
    f.account = "another-account".into();
    assert!(
        meter::dispatch_metered(
            &mut j,
            &l,
            &p,
            &o,
            &f,
            &rights(&f),
            &contract::price_book(),
            NOW + 20
        )
        .is_err()
    );
    assert_eq!(o.started(), 0);
}

#[test]
fn retrying_a_poll_returns_the_original_observation_without_resampling() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, r) = start(&mut j, &mut l, &p, &o);
    p.set_usage(&r, 60);
    let first = meter::poll(&mut j, &p, &f.execution, "stable-poll", 1, NOW + 21).unwrap();
    p.set_usage(&r, 90);
    assert_eq!(
        meter::poll(&mut j, &p, &f.execution, "stable-poll", 1, NOW + 22).unwrap(),
        first
    );
    assert!(meter::poll(&mut j, &p, &f.execution, "stable-poll", 2, NOW + 22).is_err());
    assert_eq!(
        meter::poll(&mut j, &p, &f.execution, "next-poll", 2, NOW + 22)
            .unwrap()
            .seconds,
        Some(45)
    );
    assert!(
        meter::dispatch_metered(
            &mut j,
            &l,
            &p,
            &o,
            &f,
            &rights(&f),
            &contract::price_book(),
            NOW + 621
        )
        .is_err()
    );
    assert_eq!(o.started(), 1);
}

#[test]
fn unknown_stop_acknowledgment_stays_unknown_until_observed() {
    let mut j = Journal::in_memory().unwrap();
    let mut l = Ledger::in_memory().unwrap();
    let p = FakeProvider::new();
    let o = FakeTaskOwner::new();
    let (f, r) = start(&mut j, &mut l, &p, &o);
    p.set_unreachable(true);
    let uncertain = meter::enforce_ceiling(&mut j, &p, &f.execution, NOW + 621).unwrap();
    assert!(uncertain.stop_requested);
    assert!(!uncertain.stop_acknowledged);
    assert_eq!(
        uncertain.settlement(&f, Ending::Cancelled).charge_sats,
        None
    );
    p.set_unreachable(false);
    p.lose(&r);
    assert!(
        meter::enforce_ceiling(&mut j, &p, &f.execution, NOW + 622)
            .unwrap()
            .stop_acknowledged
    );
    assert_eq!(p.delete_calls(), 0);
}
