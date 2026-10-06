//! #10713: one funded task, recovered after lost acknowledgments and
//! restarts, observed by cursor.

mod common;

use common::{NOW, confirmed, delivered, funded_account, ready, request, rights};
use pay_ledger::Ledger;
use retail_cloud::Error;
use retail_cloud::dispatch::{
    CheckRun, DispatchState, ExecutorEnd, TaskStatus, Verdict, dispatch, observe, task_id, verdict,
};
use retail_cloud::fake::{FakeProvider, FakeSandbox, FakeTaskOwner};
use retail_cloud::journal::Journal;

#[test]
fn a_lost_acknowledgment_and_restarts_recover_the_same_task_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("journal.sqlite");
    let mut ledger = Ledger::in_memory().unwrap();
    let (provider, sandbox, owner) = (
        FakeProvider::new(),
        FakeSandbox::new(),
        FakeTaskOwner::new(),
    );
    let funded = {
        let mut journal = Journal::open(&path).unwrap();
        funded_account(&mut ledger, "acct", 1_000);
        let funded = confirmed(&mut journal, "acct", "cf", &request(600));
        delivered(&mut journal, &mut ledger, &provider, &sandbox, &funded);
        owner.lose_next_ack();
        let sent = dispatch(
            &mut journal,
            &ledger,
            &owner,
            &funded,
            &rights(&funded),
            NOW + 20,
        )
        .unwrap();
        assert_eq!(sent.state, DispatchState::Sent);
        funded
    };
    // The service restarts; the owner is briefly unreachable.
    let mut journal = Journal::open(&path).unwrap();
    owner.set_unreachable(true);
    let waiting = dispatch(
        &mut journal,
        &ledger,
        &owner,
        &funded,
        &rights(&funded),
        NOW + 30,
    )
    .unwrap();
    assert_eq!(waiting.state, DispatchState::Sent);
    owner.set_unreachable(false);
    let found = dispatch(
        &mut journal,
        &ledger,
        &owner,
        &funded,
        &rights(&funded),
        NOW + 40,
    )
    .unwrap();
    assert_eq!(found.state, DispatchState::Acknowledged);
    assert_eq!(found.task, task_id(&funded.execution));
    // Any number of further calls, from any client, start nothing more.
    for t in 0..3 {
        dispatch(
            &mut journal,
            &ledger,
            &owner,
            &funded,
            &rights(&funded),
            NOW + 50 + t,
        )
        .unwrap();
    }
    assert_eq!(owner.started(), 1);
}

#[test]
fn nothing_starts_before_funding_admission_source_and_computer() {
    let mut journal = Journal::in_memory().unwrap();
    let mut ledger = Ledger::in_memory().unwrap();
    let (provider, sandbox, owner) = (
        FakeProvider::new(),
        FakeSandbox::new(),
        FakeTaskOwner::new(),
    );
    funded_account(&mut ledger, "acct", 1_000);
    let funded = confirmed(&mut journal, "acct", "cf", &request(600));
    // Unfunded.
    assert!(matches!(
        dispatch(
            &mut journal,
            &ledger,
            &owner,
            &funded,
            &rights(&funded),
            NOW
        ),
        Err(Error::Invalid(_))
    ));
    // Funded and ready, but no material delivered.
    ready(&mut journal, &mut ledger, &provider, &funded);
    assert!(matches!(
        dispatch(
            &mut journal,
            &ledger,
            &owner,
            &funded,
            &rights(&funded),
            NOW
        ),
        Err(Error::Invalid(_))
    ));
    let other = confirmed(&mut journal, "acct", "cf2", &request(60));
    let resource = delivered(&mut journal, &mut ledger, &provider, &sandbox, &other);
    // Without the execute right.
    let mut current = rights(&other);
    current.execute = None;
    assert!(matches!(
        dispatch(&mut journal, &ledger, &owner, &other, &current, NOW),
        Err(Error::Denied(_))
    ));
    assert_eq!(owner.started(), 0);
    dispatch(&mut journal, &ledger, &owner, &other, &rights(&other), NOW).unwrap();
    assert_eq!(owner.started(), 1);
    let _ = resource;
}

#[test]
fn a_client_reattaches_by_cursor_without_touching_the_task() {
    let mut journal = Journal::in_memory().unwrap();
    let mut ledger = Ledger::in_memory().unwrap();
    let (provider, sandbox, owner) = (
        FakeProvider::new(),
        FakeSandbox::new(),
        FakeTaskOwner::new(),
    );
    funded_account(&mut ledger, "acct", 1_000);
    let funded = confirmed(&mut journal, "acct", "cf", &request(600));
    let resource = delivered(&mut journal, &mut ledger, &provider, &sandbox, &funded);
    dispatch(
        &mut journal,
        &ledger,
        &owner,
        &funded,
        &rights(&funded),
        NOW,
    )
    .unwrap();
    let task = task_id(&funded.execution);
    owner.emit(&resource, &task, "cloned");
    owner.emit(&resource, &task, "editing src/parse.rs");
    let page = observe(&journal, &owner, &funded, &rights(&funded), 0).unwrap();
    assert_eq!(page.events.len(), 2);
    // The client disconnects; the task goes on.
    owner.emit(&resource, &task, "running checks");
    let mut observer_only = rights(&funded);
    observer_only.execute = None;
    observer_only.spend = None;
    observer_only.disclose = None;
    let resumed = observe(&journal, &owner, &funded, &observer_only, page.next).unwrap();
    assert_eq!(resumed.events.len(), 1);
    assert_eq!(resumed.events[0].text, "running checks");
    assert_eq!(resumed.status, Some(TaskStatus::Running));
    assert_eq!(owner.started(), 1);
    let mut nobody = observer_only.clone();
    nobody.observe = None;
    assert!(matches!(
        observe(&journal, &owner, &funded, &nobody, 0),
        Err(Error::Denied(_))
    ));
}

#[test]
fn verdicts_are_honest_about_the_exact_candidate() {
    let declared = vec!["cargo test".to_owned(), "cargo fmt --check".to_owned()];
    let pass = |command: &str, candidate: &str| CheckRun {
        command: command.into(),
        candidate: candidate.into(),
        exit_status: 0,
    };
    assert_eq!(verdict(None, &declared, &[]), Verdict::Unchecked);
    assert_eq!(
        verdict(
            Some("p1"),
            &declared,
            &[pass("cargo test", "p1"), pass("cargo fmt --check", "p1")]
        ),
        Verdict::Verified
    );
    // A check that ran on another candidate does not count.
    assert_eq!(
        verdict(
            Some("p1"),
            &declared,
            &[pass("cargo test", "p1"), pass("cargo fmt --check", "p0")]
        ),
        Verdict::CheckFailed
    );
    let mut failed = pass("cargo test", "p1");
    failed.exit_status = 101;
    assert_eq!(
        verdict(
            Some("p1"),
            &declared,
            &[failed, pass("cargo fmt --check", "p1")]
        ),
        Verdict::CheckFailed
    );
    // The ended status carries everything the verdict needs.
    let ended = TaskStatus::Ended {
        end: ExecutorEnd::Completed,
        patch: Some("p1".into()),
        checks: vec![pass("cargo test", "p1")],
    };
    let TaskStatus::Ended { patch, checks, .. } = ended else {
        unreachable!()
    };
    assert_eq!(
        verdict(patch.as_deref(), &declared[..1], &checks),
        Verdict::Verified
    );
}
