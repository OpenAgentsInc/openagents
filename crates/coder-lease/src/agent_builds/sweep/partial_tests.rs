use super::*;

#[test]
fn failed_use_record_save_does_not_rearm_observed_reads() {
    let home = Home::new();
    let layout = home.layout();
    let target = home.target("openagents-target-1", 32, 5.0);
    let artifact = std::fs::read_dir(target.join("debug/deps"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let accessed = std::fs::metadata(&artifact).unwrap().accessed().unwrap();
    std::fs::remove_file(&layout.state).unwrap();
    std::fs::create_dir(&layout.state).unwrap();
    let report = sweep(
        &layout,
        &policy(10_000, 1_000, 0),
        &fake(10_000 * KIB, Vec::new()),
        false,
    );
    assert_eq!(
        std::fs::metadata(&artifact).unwrap().accessed().unwrap(),
        accessed
    );
    assert!(
        report
            .actions
            .iter()
            .any(|action| action.path == layout.state)
    );
}

#[test]
fn newly_created_empty_copy_keeps_its_age() {
    let home = Home::new();
    let target = home.work("openagents-target-empty-copy");
    std::fs::create_dir(&target).unwrap();
    let report = sweep(
        &home.layout(),
        &policy(10_000, 1_000, 0),
        &fake(10_000 * KIB, Vec::new()),
        false,
    );
    assert!(report.complete);
    assert!(target.is_dir());
}

#[test]
fn partial_scan_prunes_measured_idle_units_but_keeps_orphans_and_whole_targets() {
    let home = Home::new();
    let layout = home.layout();
    let target = home.work("openagents-target-partial-copy");
    home.unit(&target, "old-1234", 32, 5.0);
    let orphan = worktree_state(&home, &target, "orphan", 0.1, false);
    home.file(&target.join("zz-unvisited/young.bin"), 1, 0.1);

    // Stop at the last descriptor without tying the fixture to wall-clock time
    // or to the number of metadata calls needed to discover the target.
    let probe = Budget::with_steps(usize::MAX);
    let complete = scan::scan(
        target.clone(),
        Kind::Copy,
        None,
        &layout,
        &probe,
        &mut Watch::default(),
    );
    assert!(complete.complete);
    let steps = usize::MAX - probe.steps.get().unwrap();
    let scan_budget = Budget::with_steps(steps.checked_sub(1).unwrap());
    let mut policy = policy(10_000, 1_000, 0);
    policy.stray_idle = Duration::ZERO;
    let report = sweep_with_locks(
        &layout,
        &policy,
        &fake(0, Vec::new()),
        false,
        BTreeMap::new(),
        None,
        &Budget::unlimited(),
        Some(&scan_budget),
    );

    assert!(!report.complete);
    assert_eq!(report.targets.len(), 1);
    assert!(!report.targets[0].complete);
    assert!(target.is_dir(), "a partial scan cannot remove its target");
    assert!(
        orphan.join("debug/deps/libstate-1234.rlib").is_file(),
        "a partial scan cannot authorize orphan-state cleanup"
    );
    assert!(
        !target.join("debug/deps/libold-1234.rlib").exists(),
        "a fully measured idle unit can still be pruned"
    );
    assert!(report.actions.iter().all(|action| {
        action.verb != "removed"
            || (action.path != target && !action.reason.contains("worktree no longer exists"))
    }));
}
