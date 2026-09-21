//! The pieces under adversarial and race conditions: catalog, plan,
//! and ledger exercised together the way the run wounds them —
//! reordered input, crashed writers, forged settlements, changed tasks,
//! and a backlog that collides with itself.

use std::collections::{BTreeMap, BTreeSet};

use coder_scheduler::catalog::{Catalog, Footprint, Task};
use coder_scheduler::ledger::{Ledger, LedgerError};
use coder_scheduler::plan::{self, Input, Policy, Status};
use coder_scheduler::resources::{Capacity, Resources};
use coder_scheduler::simulate::{self, Fill};

fn task(id: &str, issue: u64, writes: &[&str]) -> Task {
    Task {
        id: id.to_string(),
        issue,
        base: "sha256:base-1".to_string(),
        input: format!("sha256:input-{id}"),
        depends_on: vec![],
        footprint: Footprint::Declared {
            reads: vec![],
            writes: writes.iter().map(|path| path.to_string()).collect(),
        },
        priority: 0,
        resources: Resources::default(),
        estimate_ticks: 1,
    }
}

fn mk_catalog(tasks: Vec<Task>) -> Catalog {
    let mut catalog = Catalog::new("adversarial", tasks);
    catalog.seal().unwrap();
    catalog
}

fn host(slots: u32) -> Capacity {
    Capacity {
        executor_slots: slots,
        cpu_units: 32,
        memory_mib: 32768,
        integration_lanes: 1,
    }
}

fn input<'a>(
    catalog: &'a Catalog,
    capacity: &'a Capacity,
    states: &'a BTreeMap<String, Status>,
) -> Input<'a> {
    static DONE: BTreeSet<String> = BTreeSet::new();
    Input {
        catalog,
        capacity,
        states,
        externally_completed: &DONE,
        exclusions: &[],
        policy: &Policy { review_cap: 8 },
    }
}

/// The whole seam root will wire: register, plan, claim, settle,
/// accept — and a dependent that waits for the accept, not the settle.
#[test]
fn the_lifecycle_releases_dependents_only_on_acceptance() {
    let dir = tempfile::tempdir().unwrap();
    let mut b = task("b", 2, &["crates/b/src/lib.rs"]);
    b.depends_on = vec!["a".to_string()];
    let catalog = mk_catalog(vec![task("a", 1, &["crates/a/src/lib.rs"]), b]);
    let capacity = host(4);
    let mut ledger = Ledger::open(dir.path()).unwrap();
    ledger.register(&catalog).unwrap();

    // Round one: `a` is the only ready task; `b` waits on it.
    let states = ledger.statuses();
    let plan = plan::select(&input(&catalog, &capacity, &states));
    assert_eq!(plan.admit.len(), 1);
    assert_eq!(plan.admit[0].task, "a");

    let digest = catalog.task("a").unwrap().digest();
    let attempt = ledger.claim("a", "runner:1", &digest).unwrap();
    ledger
        .settle("a", &attempt, "runner:1", "sha256:result-a")
        .unwrap();

    // Settled is not completed: `b` still waits while `a` is in review.
    let states = ledger.statuses();
    let plan = plan::select(&input(&catalog, &capacity, &states));
    assert!(
        plan.admit.iter().all(|admission| admission.task != "b"),
        "a result awaiting review completes nothing"
    );
    assert_eq!(ledger.record("a").unwrap().status, Status::Review);

    ledger.accept("a", &attempt, "runner:1", &digest).unwrap();
    let states = ledger.statuses();
    let plan = plan::select(&input(&catalog, &capacity, &states));
    assert!(
        plan.admit.iter().any(|admission| admission.task == "b"),
        "acceptance is what releases the dependent"
    );
}

/// A crashed writer leaves its in-flight attempt `unknown`: the task
/// stays blocked, keeps its footprint against colliders, and comes back
/// only through an explicit requeue.
#[test]
fn a_crash_blocks_the_task_and_holds_its_writes() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = mk_catalog(vec![
        task("lost", 1, &["crates/shared"]),
        task("collider", 2, &["crates/shared/x.rs"]),
        task("free", 3, &["crates/free/x.rs"]),
    ]);
    let capacity = host(4);
    let digest = catalog.task("lost").unwrap().digest();
    {
        let mut ledger = Ledger::open(dir.path()).unwrap();
        ledger.register(&catalog).unwrap();
        ledger.claim("lost", "runner:1", &digest).unwrap();
        // The process dies here — the attempt never settles.
    }

    let mut ledger = Ledger::open(dir.path()).unwrap();
    assert_eq!(ledger.record("lost").unwrap().status, Status::Unknown);
    let states = ledger.statuses();
    let plan = plan::select(&input(&catalog, &capacity, &states));
    assert!(
        plan.admit
            .iter()
            .all(|admission| admission.task != "collider"),
        "an attempt nobody accounted for still holds its writes"
    );
    assert!(
        plan.admit.iter().any(|admission| admission.task == "free"),
        "disjoint work is not held hostage"
    );

    // Requeue is the operator's explicit reconciliation.
    ledger.requeue("lost").unwrap();
    let states = ledger.statuses();
    let plan = plan::select(&input(&catalog, &capacity, &states));
    assert!(
        plan.admit.iter().any(|admission| admission.task == "lost"),
        "requeued work replans"
    );
}

/// Two writers racing one catalog: the OS lock serializes them, and the
/// loser sees the winner's committed claim rather than doubling it.
#[test]
fn racing_writers_serialize_on_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = mk_catalog(vec![task("a", 1, &["crates/a/x.rs"])]);
    let digest = catalog.task("a").unwrap().digest();
    let mut first = Ledger::open(dir.path()).unwrap();
    first.register(&catalog).unwrap();

    // The second writer waits on the OS lock while the first lives.
    let path = dir.path().to_path_buf();
    let contender_catalog = catalog.clone();
    let contender = std::thread::spawn(move || {
        let mut second = Ledger::open(&path).unwrap();
        second.register(&contender_catalog).unwrap();
        second
            .record("a")
            .map(|record| record.status)
            .unwrap_or(Status::Queued)
    });
    std::thread::sleep(std::time::Duration::from_millis(100));
    first.claim("a", "runner:1", &digest).unwrap();
    drop(first);
    let observed = contender.join().unwrap();
    // The winner's claim is durable — and because the first writer went
    // away with the attempt still in flight, the contender's recovery
    // marks it `unknown` rather than trusting it.
    assert_eq!(
        observed,
        Status::Unknown,
        "the claim survived and recovered honestly"
    );
}

/// Forged and stale identities settle nothing: a wrong owner, a wrong
/// attempt id, a reused attempt after requeue.
#[test]
fn forged_settlements_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = mk_catalog(vec![task("a", 1, &["crates/a/x.rs"])]);
    let mut ledger = Ledger::open(dir.path()).unwrap();
    ledger.register(&catalog).unwrap();
    let digest = catalog.task("a").unwrap().digest();

    let attempt = ledger.claim("a", "runner:1", &digest).unwrap();
    for (attempt, owner) in [
        ("att-forged-000001", "runner:1"),
        (attempt.as_str(), "runner:2"),
        ("att-forged-000001", "runner:2"),
    ] {
        assert!(matches!(
            ledger.settle("a", attempt, owner, "sha256:r"),
            Err(LedgerError::Ownership { .. })
        ));
    }
    ledger
        .settle("a", &attempt, "runner:1", "sha256:r")
        .unwrap();
    ledger
        .reject("a", &attempt, "runner:1", "unverifiable")
        .unwrap();
    ledger.requeue("a").unwrap();
    let second = ledger.claim("a", "runner:2", &digest).unwrap();
    assert_ne!(attempt, second, "attempt ids never recycle");
}

/// A re-pinned task cannot inherit the old task's review — the bound
/// digest refuses the changed content, and `drift` says so.
#[test]
fn a_repin_invalidates_the_pending_result() {
    let dir = tempfile::tempdir().unwrap();
    let mut original = task("a", 1, &["crates/a/x.rs"]);
    original.input = "sha256:original-input".to_string();
    let catalog = mk_catalog(vec![original]);
    let mut ledger = Ledger::open(dir.path()).unwrap();
    ledger.register(&catalog).unwrap();
    let digest = catalog.task("a").unwrap().digest();
    let attempt = ledger.claim("a", "runner:1", &digest).unwrap();
    ledger
        .settle("a", &attempt, "runner:1", "sha256:r")
        .unwrap();

    // The operator re-pins `a` against a different base.
    let mut changed = task("a", 1, &["crates/a/x.rs"]);
    changed.input = "sha256:original-input".to_string();
    changed.base = "sha256:base-2".to_string();
    let catalog = mk_catalog(vec![changed]);
    ledger.register(&catalog).unwrap();

    let drift = ledger.drift(&catalog);
    assert_eq!(drift.len(), 1);
    assert_eq!(drift[0].status, Status::Review);
    assert!(matches!(
        ledger.accept("a", &attempt, "runner:1", &drift[0].current),
        Err(LedgerError::TaskChanged { .. })
    ));
    // Reject and requeue the stale result. Re-registering the re-pinned
    // catalog rebinds the queued record's digest, and the changed task
    // claims fresh under it.
    ledger
        .reject("a", &attempt, "runner:1", "task changed")
        .unwrap();
    ledger.requeue("a").unwrap();
    ledger.register(&catalog).unwrap();
    let new_attempt = ledger.claim("a", "runner:1", &drift[0].current).unwrap();
    assert_ne!(attempt, new_attempt);
    // And the old digest can no longer claim — identity follows the pin.
    assert!(matches!(
        ledger.claim("a", "runner:1", &digest),
        Err(LedgerError::Transition { .. }) | Err(LedgerError::TaskChanged { .. })
    ));
}

/// Two catalogs that only differ in task order produce the same plan —
/// determinism is against the content, not the listing.
#[test]
fn input_order_never_reaches_the_plan() {
    let capacity = host(2);
    let states: BTreeMap<String, Status> = BTreeMap::new();
    let tasks = vec![
        task("a", 3, &["crates/a/x.rs"]),
        task("b", 1, &["crates/b/x.rs"]),
        task("c", 2, &["crates/c/x.rs"]),
        task("d", 4, &["crates/d/x.rs"]),
    ];
    let forward = mk_catalog(tasks.clone());
    let mut reversed = tasks;
    reversed.reverse();
    let reversed = mk_catalog(reversed);
    let first = plan::select(&input(&forward, &capacity, &states));
    let second = plan::select(&input(&reversed, &capacity, &states));
    assert_eq!(first, second);
}

/// A poisoned catalog — a cycle under the pin — refuses to load rather
/// than plan over a guess.
#[test]
fn a_cyclic_catalog_never_reaches_the_plan() {
    let mut a = task("a", 1, &["crates/a/x.rs"]);
    a.depends_on = vec!["b".to_string()];
    let mut b = task("b", 2, &["crates/b/x.rs"]);
    b.depends_on = vec!["a".to_string()];
    let mut catalog = Catalog::new("poison", vec![a, b]);
    assert!(catalog.seal().is_err());
}

/// The benchmark catalog schedules completely at every swept capacity,
/// and refill never trails waves on the same input.
#[test]
fn the_benchmark_is_reproducible_and_refill_packs_tighter() {
    let mut tasks = Vec::new();
    for i in 0..12_u64 {
        let id = format!("s{i:02}");
        let mut t = task(&id, i, &[&format!("crates/bench/{id}/x.rs")]);
        t.estimate_ticks = 2 + i % 4;
        if i % 6 == 0 {
            t.resources.quiet_host = true;
        }
        if i % 5 == 4 {
            t.resources.cpu_units = 8;
            t.resources.memory_mib = 4096;
        }
        tasks.push(t);
    }
    tasks[5].depends_on = vec!["s01".to_string()];
    tasks[9].depends_on = vec!["s05".to_string()];
    let catalog = mk_catalog(tasks);
    for slots in [1_u32, 2, 4, 6, 8, 10] {
        let capacity = host(slots);
        let waves = simulate::run(&catalog, &capacity, Fill::Waves, &BTreeSet::new());
        let refill = simulate::run(&catalog, &capacity, Fill::Refill, &BTreeSet::new());
        assert_eq!(waves.scheduled, 12, "capacity {slots}");
        assert_eq!(refill.scheduled, 12, "capacity {slots}");
        assert!(
            refill.makespan_ticks <= waves.makespan_ticks,
            "capacity {slots}: refill {} must not trail waves {}",
            refill.makespan_ticks,
            waves.makespan_ticks
        );
        assert!(
            refill.peak.executor_slots <= slots,
            "the plan never over-admits"
        );
    }
}
