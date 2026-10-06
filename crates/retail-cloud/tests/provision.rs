//! #10711: one admitted sandbox per funded execution, against a fake Boat.

mod common;

use common::{NOW, confirmed, funded_account, request, rights};
use pay_ledger::Ledger;
use retail_cloud::Error;
use retail_cloud::fake::FakeProvider;
use retail_cloud::journal::Journal;
use retail_cloud::offer::FundedRequest;
use retail_cloud::provision::{self, Provider, ProvisionState, StartRefusal, advance};
use retail_cloud::reserve;

const TEMPLATE: &str = "oa-coder-main-20261006";

fn setup(journal: &mut Journal, ledger: &mut Ledger, account: &str) -> FundedRequest {
    funded_account(ledger, account, 1_000);
    let funded = confirmed(journal, account, &format!("cf-{account}"), &request(600));
    reserve::reserve(ledger, &funded, &rights(&funded), NOW + 2).unwrap();
    funded
}

fn step(
    journal: &mut Journal,
    ledger: &Ledger,
    provider: &FakeProvider,
    funded: &FundedRequest,
    now: i64,
) -> ProvisionState {
    advance(
        journal,
        ledger,
        provider,
        funded,
        &rights(funded),
        TEMPLATE,
        now,
    )
    .unwrap()
    .state
}

#[test]
fn one_sandbox_starts_from_the_pinned_spec_and_becomes_ready() {
    let (mut journal, mut ledger) = (Journal::in_memory().unwrap(), Ledger::in_memory().unwrap());
    let funded = setup(&mut journal, &mut ledger, "acct");
    let provider = FakeProvider::new();
    let ProvisionState::Starting { resource } =
        step(&mut journal, &ledger, &provider, &funded, NOW + 3)
    else {
        panic!("expected starting")
    };
    let spec = provider.spec_of(&resource).unwrap();
    assert_eq!(spec.size, "large");
    assert!(spec.no_env);
    assert_eq!(spec.template, TEMPLATE);
    assert_eq!(spec.provisioning, format!("{}#1", funded.execution));
    assert!(matches!(
        step(&mut journal, &ledger, &provider, &funded, NOW + 4),
        ProvisionState::Ready { .. }
    ));
    // Ready is final for provisioning: more calls create nothing.
    step(&mut journal, &ledger, &provider, &funded, NOW + 5);
    assert_eq!(provider.create_calls(), 1);
}

#[test]
fn a_lost_acknowledgment_is_found_by_its_provisioning_identity_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("journal.sqlite");
    let mut ledger = Ledger::in_memory().unwrap();
    let provider = FakeProvider::new();
    let funded = {
        let mut journal = Journal::open(&path).unwrap();
        let funded = setup(&mut journal, &mut ledger, "acct");
        provider.lose_next_ack();
        assert_eq!(
            step(&mut journal, &ledger, &provider, &funded, NOW + 3),
            ProvisionState::Creating
        );
        funded
    };
    let mut journal = Journal::open(&path).unwrap();
    // A failed listing is not proof of absence: nothing is created.
    provider.set_listing_fails(true);
    assert_eq!(
        step(&mut journal, &ledger, &provider, &funded, NOW + 4),
        ProvisionState::Creating
    );
    provider.set_listing_fails(false);
    assert!(matches!(
        step(&mut journal, &ledger, &provider, &funded, NOW + 5),
        ProvisionState::Starting { .. }
    ));
    assert!(matches!(
        step(&mut journal, &ledger, &provider, &funded, NOW + 6),
        ProvisionState::Ready { .. }
    ));
    assert_eq!(provider.create_calls(), 1);
    assert_eq!(provider.active().len(), 1);
}

#[test]
fn a_slow_sandbox_is_replaced_once_then_unavailable() {
    let (mut journal, mut ledger) = (Journal::in_memory().unwrap(), Ledger::in_memory().unwrap());
    let funded = setup(&mut journal, &mut ledger, "acct");
    let provider = FakeProvider::new();
    provider.set_ready_after_polls(1_000);
    step(&mut journal, &ledger, &provider, &funded, NOW + 3);
    assert!(matches!(
        step(&mut journal, &ledger, &provider, &funded, NOW + 300),
        ProvisionState::Starting { .. }
    ));
    let ProvisionState::Starting { resource: second } =
        step(&mut journal, &ledger, &provider, &funded, NOW + 604)
    else {
        panic!("expected a replacement")
    };
    let record = journal.provisioning(&funded.execution).unwrap().unwrap();
    assert_eq!(record.attempt, 2);
    assert_eq!(
        provider.spec_of(&second).unwrap().provisioning,
        format!("{}#2", funded.execution)
    );
    assert_eq!(provider.active(), vec![second.clone()]);
    assert_eq!(
        step(&mut journal, &ledger, &provider, &funded, NOW + 1_300),
        ProvisionState::Unavailable
    );
    assert!(provider.active().is_empty());
    assert_eq!(provider.create_calls(), 2);
    let record = journal.provisioning(&funded.execution).unwrap().unwrap();
    assert_eq!(record.abandoned.unwrap().split(',').count(), 2);
}

#[test]
fn a_failed_restore_is_replaced_and_a_refused_start_stops() {
    let (mut journal, mut ledger) = (Journal::in_memory().unwrap(), Ledger::in_memory().unwrap());
    let funded = setup(&mut journal, &mut ledger, "acct");
    let provider = FakeProvider::new();
    provider.set_restore_fails(true);
    step(&mut journal, &ledger, &provider, &funded, NOW + 3);
    provider.set_restore_fails(false);
    assert!(matches!(
        step(&mut journal, &ledger, &provider, &funded, NOW + 4),
        ProvisionState::Starting { .. }
    ));
    assert!(matches!(
        step(&mut journal, &ledger, &provider, &funded, NOW + 5),
        ProvisionState::Ready { .. }
    ));

    let other = setup(&mut journal, &mut ledger, "acct-b");
    provider.refuse_next(StartRefusal::PlanLimit);
    let calls = provider.create_calls();
    assert_eq!(
        step(&mut journal, &ledger, &provider, &other, NOW + 6),
        ProvisionState::Refused {
            reason: StartRefusal::PlanLimit
        }
    );
    step(&mut journal, &ledger, &provider, &other, NOW + 7);
    assert_eq!(
        provider.create_calls(),
        calls + 1,
        "no automatic retry or widening"
    );
}

#[test]
fn unfunded_or_unadmitted_requests_make_no_create_call() {
    let (mut journal, mut ledger) = (Journal::in_memory().unwrap(), Ledger::in_memory().unwrap());
    funded_account(&mut ledger, "acct", 1_000);
    let unfunded = confirmed(&mut journal, "acct", "cf-1", &request(600));
    let provider = FakeProvider::new();
    assert!(matches!(
        advance(
            &mut journal,
            &ledger,
            &provider,
            &unfunded,
            &rights(&unfunded),
            TEMPLATE,
            NOW
        ),
        Err(Error::Invalid(_))
    ));
    reserve::reserve(&mut ledger, &unfunded, &rights(&unfunded), NOW).unwrap();
    let mut current = rights(&unfunded);
    current.execute = None;
    current.paired = true;
    assert!(matches!(
        advance(
            &mut journal,
            &ledger,
            &provider,
            &unfunded,
            &current,
            TEMPLATE,
            NOW
        ),
        Err(Error::Denied(_))
    ));
    assert_eq!(provider.create_calls(), 0);
    assert!(journal.provisioning(&unfunded.execution).unwrap().is_none());
}

#[test]
fn customers_never_share_or_see_each_others_sandboxes() {
    let (mut journal, mut ledger) = (Journal::in_memory().unwrap(), Ledger::in_memory().unwrap());
    let a = setup(&mut journal, &mut ledger, "acct-a");
    let b = setup(&mut journal, &mut ledger, "acct-b");
    let provider = FakeProvider::new();
    for funded in [&a, &b] {
        for t in 0..3 {
            step(&mut journal, &ledger, &provider, funded, NOW + 3 + t);
        }
    }
    assert_eq!(provider.active_for("acct-a"), 1);
    assert_eq!(provider.active_for("acct-b"), 1);
    let found = provider
        .find(&provision::spec(&a, 1, TEMPLATE).provisioning)
        .unwrap()
        .unwrap();
    assert_eq!(found.account, "acct-a");
    assert_ne!(
        Some(found.id),
        provider
            .find(&provision::spec(&b, 1, TEMPLATE).provisioning)
            .unwrap()
            .map(|r| r.id)
    );
}
