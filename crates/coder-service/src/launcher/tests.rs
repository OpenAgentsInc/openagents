//! The trial, commit, and rollback state machine, including a crash in
//! every state, against fixture hosts that are shell scripts staged as
//! digest-named bundles.

use std::sync::Arc;
use std::thread::JoinHandle;

use super::*;

const KEY: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

struct Fixture {
    _temp: tempfile::TempDir,
    layout: Layout,
    state: PathBuf,
    v1: String,
    v2: String,
    exits: String,
    silent: String,
}

fn stage(bundles: &Path, script: &str) -> String {
    let digest = fsx::sha256_hex(script.as_bytes());
    let directory = bundles.join("versions").join(&digest);
    fs::create_dir_all(&directory).unwrap();
    let binary = directory.join("coder");
    fs::write(&binary, script).unwrap();
    fs::set_permissions(&binary, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let manifest = serde_json::json!({
        "schema": bundle::BUNDLE_SCHEMA,
        "binary_sha256": digest,
    });
    fs::write(directory.join("manifest.json"), manifest.to_string()).unwrap();
    digest
}

fn good(state: &Path, tag: &str) -> String {
    format!(
        concat!(
            "#!/bin/sh\n",
            "printf '%s\\n' \"$OPENAGENTS_HOST_VERSION\" > '{state}/marker'\n",
            "printf '{{\"schema\":\"openagents.coder.host-ready.v1\",\"generation\":%s,\"version\":\"%s\",",
            "\"protocol_version\":1,\"capabilities\":[\"tasks\",\"{tag}\"]}}' ",
            "\"$OPENAGENTS_HOST_GENERATION\" \"$OPENAGENTS_HOST_VERSION\" > \"$OPENAGENTS_HOST_READY_FILE.tmp\"\n",
            "mv \"$OPENAGENTS_HOST_READY_FILE.tmp\" \"$OPENAGENTS_HOST_READY_FILE\"\n",
            "exec sleep 60\n"
        ),
        state = state.display(),
        tag = tag,
    )
}

fn fixture(ready_timeout_secs: u64) -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let bundles = base.join("bundles");
    let state = base.join("tasks");
    fs::create_dir(&state).unwrap();
    fs::write(state.join("tasks.json"), "before").unwrap();
    let v1 = stage(&bundles, &good(&state, "one"));
    let v2 = stage(&bundles, &good(&state, "two"));
    let exits = stage(
        &bundles,
        &format!(
            "#!/bin/sh\nprintf broken > '{}/marker'\nexit 3\n",
            state.display()
        ),
    );
    let silent = stage(
        &bundles,
        &format!(
            "#!/bin/sh\nprintf broken > '{}/marker'\nexec sleep 60\n",
            state.display()
        ),
    );
    let layout = Layout::new(base.join("host"));
    let config = Config {
        schema: CONFIG_SCHEMA.into(),
        label: "org.openagents.test-host".into(),
        platform: Platform::Linux,
        registration_dir: base.join("registration"),
        launcher: PathBuf::from("/usr/bin/true"),
        bundle_root: bundles,
        host_args: Vec::new(),
        state_dirs: vec![state.clone()],
        listen: "127.0.0.1:47100".into(),
        host_key: KEY.into(),
        ready_timeout_secs,
        stop_grace_secs: 1,
        snapshot_max_bytes: 1 << 20,
        // The fixture hosts run `mv` and `sleep`, which some systems, such
        // as NixOS, keep outside the base directories.
        search_path: search_path(std::env::var("PATH").ok().as_deref()),
    };
    initialize(&layout, &config, &v1).unwrap();
    Fixture {
        _temp: temp,
        layout,
        state,
        v1,
        v2,
        exits,
        silent,
    }
}

fn wait_until(what: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !ready() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Reaps a host a crashed launcher left behind as soon as it exits, as
/// the service manager or init would.
fn reap(child: Option<Child>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        if let Some(mut child) = child {
            let _ = child.wait();
        }
    })
}

fn marker(fixture: &Fixture) -> Option<String> {
    fs::read_to_string(fixture.state.join("marker"))
        .ok()
        .map(|text| text.trim().to_string())
}

fn tasks(fixture: &Fixture) -> String {
    fs::read_to_string(fixture.state.join("tasks.json")).unwrap()
}

fn snapshots_empty(fixture: &Fixture) -> bool {
    fs::read_dir(fixture.layout.snapshots())
        .unwrap()
        .next()
        .is_none()
}

fn reopen(fixture: &Fixture) -> Launcher {
    Launcher::open(fixture.layout.clone()).unwrap()
}

fn assert_rolled_back(fixture: &Fixture, launcher: &Launcher, to: &str) {
    let state = launcher.state();
    assert_eq!(state.phase, Phase::Idle);
    assert_eq!(state.committed, fixture.v1);
    assert_eq!(state.last.state, UpdateState::RolledBack);
    assert_eq!(state.last.target.as_deref(), Some(to));
    assert!(state.last.reason.is_some());
    assert_eq!(tasks(fixture), "before");
    assert_eq!(marker(fixture), None, "the trial's write is restored away");
    assert!(snapshots_empty(fixture));
    assert!(
        !fixture.layout.request().exists(),
        "an answered request is not retried"
    );
    let descriptor = read_descriptor(&fixture.layout).unwrap().unwrap();
    assert_eq!(descriptor.update.state, UpdateState::RolledBack);
    assert_eq!(descriptor.version.as_deref(), Some(fixture.v1.as_str()));
}

#[test]
fn a_second_launcher_is_refused() {
    let fixture = fixture(5);
    let _first = reopen(&fixture);
    assert!(Launcher::open(fixture.layout.clone()).is_err());
}

#[test]
fn crash_while_staging_the_snapshot_keeps_the_committed_version_and_the_request() {
    let fixture = fixture(5);
    let request = request_update(&fixture.layout, &fixture.v2).unwrap();
    let mut launcher = reopen(&fixture);
    launcher.crash_at("snapshot-staged");
    assert!(matches!(launcher.prepare(&request), Err(Error::Crash(_))));
    drop(launcher);

    let mut launcher = reopen(&fixture);
    assert_eq!(launcher.recover().unwrap(), None);
    assert_eq!(launcher.state().phase, Phase::Idle);
    assert_eq!(launcher.state().committed, fixture.v1);
    assert!(snapshots_empty(&fixture), "the partial snapshot is removed");
    assert_eq!(
        launcher
            .pending_request()
            .unwrap()
            .map(|pending| pending.id),
        Some(request.id),
        "a request whose update never recorded `prepared` stays pending",
    );
}

#[test]
fn crash_in_prepared_rolls_back() {
    let fixture = fixture(5);
    let request = request_update(&fixture.layout, &fixture.v2).unwrap();
    let mut launcher = reopen(&fixture);
    launcher.crash_at("prepared");
    assert!(matches!(launcher.prepare(&request), Err(Error::Crash(_))));
    assert!(matches!(launcher.state().phase, Phase::Prepared { .. }));
    drop(launcher);

    let mut launcher = reopen(&fixture);
    assert_eq!(launcher.recover().unwrap(), Some(UpdateState::RolledBack));
    assert_rolled_back(&fixture, &launcher, &fixture.v2);
}

#[test]
fn crash_in_trial_before_ready_stops_the_orphan_and_restores_the_snapshot() {
    let fixture = fixture(5);
    let request = request_update(&fixture.layout, &fixture.silent).unwrap();
    let mut launcher = reopen(&fixture);
    launcher.prepare(&request).unwrap();
    launcher.start_trial().unwrap();
    wait_until("the trial to write state", || marker(&fixture).is_some());
    let group = launcher.state().host_group.unwrap();
    let reaper = reap(launcher.crash());

    let mut launcher = reopen(&fixture);
    assert_eq!(launcher.recover().unwrap(), Some(UpdateState::RolledBack));
    reaper.join().unwrap();
    assert!(
        !supervise::running(group),
        "the orphaned trial host is stopped"
    );
    assert_eq!(launcher.state().host_group, None);
    assert_rolled_back(&fixture, &launcher, &fixture.silent);
}

#[test]
fn crash_in_trial_after_ready_commits() {
    let fixture = fixture(5);
    let request = request_update(&fixture.layout, &fixture.v2).unwrap();
    let mut launcher = reopen(&fixture);
    launcher.prepare(&request).unwrap();
    launcher.start_trial().unwrap();
    let generation = launcher.state().generation;
    wait_until("the trial to report ready", || {
        fixture.layout.ready(generation).exists()
    });
    let reaper = reap(launcher.crash());

    let mut launcher = reopen(&fixture);
    assert_eq!(launcher.recover().unwrap(), Some(UpdateState::Committed));
    reaper.join().unwrap();
    let state = launcher.state();
    assert_eq!(state.committed, fixture.v2);
    assert_eq!(state.previous.as_deref(), Some(fixture.v1.as_str()));
    assert_eq!(state.last.state, UpdateState::Committed);
    assert_eq!(
        marker(&fixture).as_deref(),
        Some(fixture.v2.as_str()),
        "committed state is kept"
    );
    assert!(snapshots_empty(&fixture));
}

#[test]
fn crash_in_rolling_back_finishes_the_restore() {
    for point in ["rolling-back", "restore-staged", "restore-moved"] {
        let fixture = fixture(5);
        let request = request_update(&fixture.layout, &fixture.silent).unwrap();
        let mut launcher = reopen(&fixture);
        launcher.prepare(&request).unwrap();
        launcher.start_trial().unwrap();
        wait_until("the trial to write state", || marker(&fixture).is_some());
        launcher.crash_at(point);
        assert!(
            matches!(launcher.roll_back("test failure"), Err(Error::Crash(_))),
            "{point}"
        );
        assert!(matches!(launcher.state().phase, Phase::RollingBack { .. }));
        drop(launcher);

        let mut launcher = reopen(&fixture);
        assert_eq!(
            launcher.recover().unwrap(),
            Some(UpdateState::RolledBack),
            "{point}"
        );
        assert_rolled_back(&fixture, &launcher, &fixture.silent);
        assert_eq!(
            launcher.state().last.reason.as_deref(),
            Some("test failure")
        );
    }
}

#[test]
fn crash_after_the_commit_record_keeps_the_commit_and_removes_the_snapshot() {
    let fixture = fixture(5);
    let request = request_update(&fixture.layout, &fixture.v2).unwrap();
    let mut launcher = reopen(&fixture);
    launcher.prepare(&request).unwrap();
    launcher.start_trial().unwrap();
    let generation = launcher.state().generation;
    wait_until("the trial to report ready", || {
        fixture.layout.ready(generation).exists()
    });
    let ready = launcher.read_ready(generation, &fixture.v2).unwrap();
    launcher.crash_at("committed");
    assert!(matches!(launcher.commit(&ready), Err(Error::Crash(_))));
    assert!(!snapshots_empty(&fixture));
    let reaper = reap(launcher.crash());

    let mut launcher = reopen(&fixture);
    assert_eq!(launcher.recover().unwrap(), None);
    reaper.join().unwrap();
    assert_eq!(launcher.state().committed, fixture.v2);
    assert!(snapshots_empty(&fixture));
    assert!(!fixture.layout.request().exists());
}

#[test]
fn a_refused_update_is_answered_and_the_committed_version_runs() {
    let fixture = fixture(5);
    let request = request_update(&fixture.layout, &fixture.v2).unwrap();
    let mut launcher = reopen(&fixture);
    launcher.config.snapshot_max_bytes = 1;
    let outcome = launcher.update(&request, &AtomicBool::new(false)).unwrap();
    assert_eq!(outcome, UpdateState::RolledBack);
    let last = launcher.state().last.clone();
    assert!(last.reason.unwrap().contains("refused before a trial"));
    assert_eq!(
        launcher.host.as_ref().map(|host| host.version.clone()),
        Some(fixture.v1.clone())
    );
    launcher.stop_host();
    assert!(snapshots_empty(&fixture));
    assert!(!fixture.layout.request().exists());
}

fn start(fixture: &Fixture) -> (Arc<AtomicBool>, JoinHandle<Result<i32>>) {
    let stop = Arc::new(AtomicBool::new(false));
    let layout = fixture.layout.clone();
    let flag = Arc::clone(&stop);
    let handle = std::thread::spawn(move || Launcher::open(layout)?.run(&flag));
    (stop, handle)
}

fn descriptor(fixture: &Fixture) -> HostDescriptor {
    read_descriptor(&fixture.layout).unwrap().unwrap()
}

fn outcome_for(fixture: &Fixture, request: &UpdateRequest) -> HostDescriptor {
    let mut last = None;
    wait_until("the update outcome", || {
        let current = descriptor(fixture);
        let done = current.update.request.as_deref() == Some(request.id.as_str())
            && matches!(
                current.update.state,
                UpdateState::Committed | UpdateState::RolledBack
            )
            && current.state == HostState::Ready;
        last = Some(current);
        done
    });
    last.unwrap()
}

#[test]
fn a_client_asks_for_an_update_and_observes_committed_on_reconnect() {
    let fixture = fixture(5);
    let (stop, handle) = start(&fixture);
    wait_until("the first host", || {
        read_descriptor(&fixture.layout)
            .ok()
            .flatten()
            .is_some_and(|d| d.state == HostState::Ready)
    });
    let first = descriptor(&fixture);
    assert_eq!(first.version.as_deref(), Some(fixture.v1.as_str()));
    assert_eq!(first.protocol_version, Some(1));
    assert_eq!(
        first.capabilities,
        vec!["host-rollback", "host-trial-update", "one", "tasks"]
    );

    let request = request_update(&fixture.layout, &fixture.v2).unwrap();
    let after = outcome_for(&fixture, &request);
    assert_eq!(after.update.state, UpdateState::Committed);
    assert_eq!(after.update.target.as_deref(), Some(fixture.v2.as_str()));
    assert_eq!(after.update.from.as_deref(), Some(fixture.v1.as_str()));
    assert_eq!(after.version.as_deref(), Some(fixture.v2.as_str()));
    assert!(after.host_generation > first.host_generation);
    assert!(after.capabilities.contains(&"two".to_string()));
    assert_eq!(
        HostDescriptor::decode(&after.encode().unwrap()).unwrap(),
        after
    );

    stop.store(true, Ordering::SeqCst);
    assert_eq!(handle.join().unwrap().unwrap(), 0);
    assert_eq!(descriptor(&fixture).state, HostState::Stopped);
}

#[test]
fn failed_trials_roll_back_to_the_previous_version_and_snapshot() {
    let fixture = fixture(2);
    let (stop, handle) = start(&fixture);
    wait_until("the first host", || {
        read_descriptor(&fixture.layout)
            .ok()
            .flatten()
            .is_some_and(|d| d.state == HostState::Ready)
    });
    for (target, reason) in [
        (&fixture.exits, "exited with code 3"),
        (&fixture.silent, "did not report ready"),
    ] {
        let request = request_update(&fixture.layout, target).unwrap();
        let after = outcome_for(&fixture, &request);
        assert_eq!(after.update.state, UpdateState::RolledBack);
        assert_eq!(after.update.target.as_deref(), Some(target.as_str()));
        assert!(
            after.update.reason.as_deref().unwrap().contains(reason),
            "{:?}",
            after.update.reason
        );
        assert_eq!(after.version.as_deref(), Some(fixture.v1.as_str()));
        assert_eq!(tasks(&fixture), "before");
        assert_eq!(marker(&fixture).as_deref(), Some(fixture.v1.as_str()));
    }
    stop.store(true, Ordering::SeqCst);
    assert_eq!(handle.join().unwrap().unwrap(), 0);
}

#[test]
fn standalone_and_service_hosts_share_one_generation_counter() {
    let fixture = fixture(5);
    let root = fixture.layout.root().to_path_buf();
    // A standalone `coder host serve` ran from this root first.
    let standalone = generation::advance(&root).unwrap();

    let mut launcher = reopen(&fixture);
    launcher.recover().unwrap();
    launcher.start_committed().unwrap();
    let service = launcher.state().generation;
    assert!(service > standalone, "{service} <= {standalone}");
    wait_until("the service host to report ready", || {
        launcher.read_ready(service, &fixture.v1).is_some()
    });
    // The host the launcher started claims the generation it was given.
    generation::claim(&root, service).unwrap();
    assert!(generation::claim(&root, service).is_err());
    assert_eq!(
        launcher.descriptor(HostState::Ready).host_generation,
        service
    );

    // An update's trial takes the next value from the same counter.
    let request = request_update(&fixture.layout, &fixture.v2).unwrap();
    launcher.stop_host();
    launcher.prepare(&request).unwrap();
    launcher.start_trial().unwrap();
    let trial = launcher.state().generation;
    assert!(trial > service);
    wait_until("the trial to report ready", || {
        launcher.read_ready(trial, &fixture.v2).is_some()
    });
    launcher.stop_host();
    launcher.save().unwrap();
    drop(launcher);

    // Back to a standalone host, then the service again: still increasing.
    let again = generation::advance(&root).unwrap();
    assert!(again > trial);
    let mut launcher = reopen(&fixture);
    launcher.recover().unwrap();
    launcher.start_committed().unwrap();
    assert!(launcher.state().generation > again);
    launcher.stop_host();
    launcher.save().unwrap();
}

#[test]
fn a_launcher_crash_before_its_host_claims_skips_the_generation() {
    let fixture = fixture(5);
    let root = fixture.layout.root().to_path_buf();
    let mut launcher = reopen(&fixture);
    launcher.recover().unwrap();
    launcher.start_committed().unwrap();
    let lost = launcher.state().generation;
    let reaper = reap(launcher.crash());

    let mut launcher = reopen(&fixture);
    launcher.recover().unwrap();
    reaper.join().unwrap();
    launcher.start_committed().unwrap();
    let next = launcher.state().generation;
    assert!(next > lost);
    // A host still holding the lost value cannot serve with it.
    assert!(generation::claim(&root, lost).is_err());
    generation::claim(&root, next).unwrap();
    launcher.stop_host();
    launcher.save().unwrap();
}

/// Starts `sleep` as the leader of its own process group, standing in for
/// an unrelated program that took a recorded host's group number.
fn unrelated_group() -> (Child, i32) {
    let mut command = Command::new("sleep");
    command.arg("30");
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let child = command.spawn().unwrap();
    let group = i32::try_from(child.id()).unwrap();
    (child, group)
}

fn record_group(fixture: &Fixture, group: i32, identity: Option<String>) -> Launcher {
    let mut launcher = reopen(fixture);
    launcher.state.host_group = Some(group);
    launcher.state.host_identity = identity;
    launcher.save().unwrap();
    drop(launcher);
    reopen(fixture)
}

#[test]
fn recovery_spares_a_reused_group_whose_leader_does_not_match() {
    let fixture = fixture(5);
    let (mut child, group) = unrelated_group();
    let mut launcher = record_group(&fixture, group, Some("linux:other:1".into()));
    launcher.recover().unwrap();
    assert!(supervise::running(group), "the unrelated group survives");
    assert_eq!(launcher.state().host_group, None);
    assert_eq!(launcher.state().host_identity, None);
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn recovery_spares_a_live_group_when_no_identity_was_recorded() {
    let fixture = fixture(5);
    let (mut child, group) = unrelated_group();
    let mut launcher = record_group(&fixture, group, None);
    launcher.recover().unwrap();
    assert!(supervise::running(group), "an unverifiable group survives");
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn recovery_stops_a_group_whose_leader_matches() {
    let fixture = fixture(5);
    let (child, group) = unrelated_group();
    let identity = process_identity(group);
    assert!(identity.is_some(), "a live process has an identity");
    let mut launcher = record_group(&fixture, group, identity);
    let reaper = reap(Some(child));
    launcher.recover().unwrap();
    reaper.join().unwrap();
    assert!(
        !supervise::running(group),
        "the recorded host group is stopped"
    );
}

#[test]
fn process_identity_is_stable_and_distinguishes_processes() {
    let (mut first, first_group) = unrelated_group();
    let (mut second, second_group) = unrelated_group();
    let one = process_identity(first_group).unwrap();
    assert_eq!(Some(one.clone()), process_identity(first_group));
    std::thread::sleep(Duration::from_millis(50));
    let _ = second.kill();
    let _ = second.wait();
    let (mut third, third_group) = unrelated_group();
    assert_ne!(Some(one), process_identity(third_group));
    assert_eq!(process_identity(second_group), None);
    for child in [&mut first, &mut third] {
        let _ = child.kill();
        let _ = child.wait();
    }
}
