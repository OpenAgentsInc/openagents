use super::*;
use crate::{Broker, Holder, Limits, Request, Resource, Wait};

fn facts(home: &Path, agent: &str, now: u64, free: u64, cargo: &str) -> MachineFacts {
    collect(
        home,
        &home.join(".openagents/leases"),
        agent,
        "fixture",
        now,
        Ok(free << 30),
        Ok(cargo.into()),
    )
}

#[test]
fn absent_files_are_empty_and_collection_creates_nothing() {
    let home = tempfile::tempdir().unwrap();
    let got = facts(home.path(), "agent-a", 1, 100, "");
    assert_eq!(got.disk_below_floor, Some(false));
    assert_eq!(got.build_leases, Some((0, 5)));
    assert_eq!(got.cargo_jobs, Some(0));
    assert!(got.sweep_last_run.is_none());
    assert!(got.granted_scopes.is_empty());
    assert!(std::fs::read_dir(home.path()).unwrap().next().is_none());
}

#[test]
fn reads_live_leases_sweep_and_only_the_requested_agents_grants_and_actions() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join(".openagents/leases");
    let broker = Broker::new(
        root.clone(),
        Limits {
            build: 3,
            memory_gib: 8,
            disk_floor_gb: 0,
            build_disk_gb: 0,
        },
    );
    let held = broker
        .acquire(
            Request::new(
                Resource::Build,
                Holder {
                    session: "fixture".into(),
                    agent: "test".into(),
                    pid: std::process::id(),
                    command: "cargo".into(),
                },
            )
            .wait(Wait::No),
        )
        .unwrap();
    agent_builds::set_limit(&root, 3).unwrap();
    machine::grant(&root, "agent-a", &[machine::Scope::BuildPrune], None).unwrap();
    let log = home.path().join(".openagents/logs");
    std::fs::create_dir_all(&log).unwrap();
    std::fs::write(
        log.join("build-sweep.last.json"),
        r#"{"at":"2026-10-17 9:30:00 PM CDT","freed":1073741824,"actions":[]}"#,
    )
    .unwrap();
    std::fs::write(log.join("build-sweep.log"), "2026-10-17 9:30:00 PM CDT  removed  1 GiB  /fixture/target  (over cap)\n2026-10-17 9:31:00 PM CDT  flagged  1 GiB  /fixture/held  (held)\n").unwrap();
    let line = |agent| {
        serde_json::json!({"schema":machine::ENTRY_SCHEMA,"at":"2026-10-17 9:30:00 PM CDT", "atMs":1,"agent":agent,"scope":"build.prune","outcome":"refused","reason":"low disk","detail":"no grant"}).to_string()
    };
    std::fs::write(
        root.join(machine::LOG_FILE),
        format!("{}\n{}\n", line("agent-a"), line("agent-b")),
    )
    .unwrap();
    let before = std::fs::read(root.join("table.json")).unwrap();
    let got = facts(home.path(), "agent-a", crate::now_ms(), 99, "123\n456\n");
    assert_eq!(got.build_leases, Some((1, 3)));
    assert_eq!(got.granted_scopes, vec![machine::Scope::BuildPrune]);
    assert_eq!(got.recent_machine_actions.len(), 1);
    assert_eq!(got.recent_machine_actions[0].agent, "agent-a");
    assert_eq!(got.sweep_recent_evictions.len(), 1);
    assert_eq!(
        got.sweep_last_run.as_deref(),
        Some("2026-10-17 9:30:00 PM CDT")
    );
    assert_eq!(std::fs::read(root.join("table.json")).unwrap(), before);
    let other = facts(home.path(), "agent-b", crate::now_ms(), 99, "");
    assert!(other.granted_scopes.is_empty());
    held.release(Some(0)).unwrap();
}

#[test]
fn errors_are_unknown_instead_of_zero() {
    let home = tempfile::tempdir().unwrap();
    let got = collect(
        home.path(),
        &home.path().join("leases"),
        "a",
        "fixture",
        1,
        Err("unavailable".into()),
        Err("unavailable".into()),
    );
    assert_eq!(got.disk_free_gib, None);
    assert_eq!(got.disk_below_floor, None);
    assert_eq!(got.cargo_jobs, None);
    assert!(got.errors.contains_key("disk") && got.errors.contains_key("cargo"));
}

#[test]
fn exact_pid_list_never_counts_shell_wrappers() {
    assert_eq!(cargo_count("12\n34\n").unwrap(), 2);
    assert_eq!(cargo_count("").unwrap(), 0);
    assert!(cargo_count("sh -c cargo test").is_err());
    assert!(cargo_count("0\n").is_err());
}

#[test]
fn fake_clock_alerts_fire_once_per_crossing_and_per_agent() {
    let home = tempfile::tempdir().unwrap();
    let mut alerts = Alerts::default();
    assert!(
        alerts
            .observe(&facts(home.path(), "a", 0, 100, ""))
            .is_empty()
    );
    let low = facts(home.path(), "a", 300_000, 99, "1\n2\n3\n4\n5\n6\n");
    assert_eq!(
        alerts.observe(&low),
        vec![ResourceAlert::DiskLow, ResourceAlert::BuildsOverCap]
    );
    assert!(
        alerts
            .observe(&facts(home.path(), "a", 600_000, 99, "1\n2\n3\n4\n5\n6\n"))
            .is_empty()
    );
    let mut unknown = facts(home.path(), "a", 900_000, 100, "");
    unknown.disk_below_floor = None;
    unknown.builds_over_cap = None;
    assert!(alerts.observe(&unknown).is_empty());
    assert!(alerts.observe(&low).is_empty());
    assert_eq!(
        alerts.observe(&facts(home.path(), "b", 900_000, 99, "")),
        vec![ResourceAlert::DiskLow]
    );
    assert!(
        alerts
            .observe(&facts(home.path(), "a", 1_200_000, 100, ""))
            .is_empty()
    );
    assert_eq!(alerts.observe(&low).len(), 2);
}

#[test]
fn bounded_log_tail_discards_partial_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("log");
    std::fs::write(
        &path,
        format!("{}\nlast complete line\n", "x".repeat(TAIL_BYTES as usize)),
    )
    .unwrap();
    assert_eq!(tail(&path).unwrap(), "last complete line\n");
}
