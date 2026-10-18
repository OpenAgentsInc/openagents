use super::*;

use std::fs::{File, FileTimes};
use std::time::UNIX_EPOCH;

const KIB: u64 = 1024;
/// The fixed clock: 2026-10-17 18:00 CDT.
const NOW: u64 = 1_792_278_000;

/// A fake machine: a home in a temporary folder, a fixed clock, a free
/// space figure, and a process list.
struct Fake {
    free: Option<u64>,
    procs: Result<Vec<Proc>, String>,
    now: SystemTime,
}

impl Host for Fake {
    fn processes(&self) -> Result<Vec<Proc>, String> {
        self.procs.clone()
    }

    fn free(&self, _: &Path) -> std::io::Result<u64> {
        self.free.ok_or_else(|| std::io::ErrorKind::Other.into())
    }

    fn now(&self) -> SystemTime {
        self.now
    }

    fn stamp(&self, at: SystemTime) -> String {
        let seconds = at.duration_since(UNIX_EPOCH).unwrap().as_secs();
        local_time(i64::try_from(seconds).unwrap() - 5 * 3600, "CDT")
    }
}

fn fake(free: u64, procs: Vec<Proc>) -> Fake {
    Fake {
        free: Some(free),
        procs: Ok(procs),
        now: UNIX_EPOCH + Duration::from_secs(NOW),
    }
}

struct Home {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Home {
    fn new() -> Home {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::create_dir_all(root.join("work")).unwrap();
        // The sweep has watched since 1970, so the fixtures' file times rule.
        std::fs::create_dir_all(root.join(".openagents")).unwrap();
        std::fs::write(
            root.join(".openagents/build-sweep.state.json"),
            r#"{"since":0,"used":{}}"#,
        )
        .unwrap();
        Home { _dir: dir, root }
    }

    fn layout(&self) -> Layout {
        Layout::at(&self.root, &self.root.join(".openagents/leases"))
    }

    fn work(&self, name: &str) -> PathBuf {
        self.root.join("work").join(name)
    }

    /// A file of `kib` KiB at `path`, last read and written `hours_ago`.
    fn file(&self, path: &Path, kib: u64, hours_ago: f64) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![7u8; usize::try_from(kib * KIB).unwrap()]).unwrap();
        age(path, hours_ago);
    }

    /// A crate unit in `target/debug`: its rlib, rmeta, and dep-info, and
    /// its fingerprint, all `hours_ago`.
    fn unit(&self, target: &Path, name: &str, kib: u64, hours_ago: f64) {
        let deps = target.join("debug/deps");
        self.file(&deps.join(format!("lib{name}.rlib")), kib, hours_ago);
        self.file(&deps.join(format!("lib{name}.rmeta")), kib / 4, hours_ago);
        self.file(&deps.join(format!("{name}.d")), 4, hours_ago);
        self.file(
            &target.join(format!("debug/.fingerprint/{name}/lib-{name}")),
            4,
            hours_ago,
        );
    }

    /// A target folder holding one unit of `kib` KiB untouched `hours_ago`.
    fn target(&self, name: &str, kib: u64, hours_ago: f64) -> PathBuf {
        let target = self.work(name);
        self.unit(
            &target,
            &format!("{}-0a1b", name.replace('-', "_")),
            kib,
            hours_ago,
        );
        target
    }
}

fn age(path: &Path, hours_ago: f64) {
    let at = UNIX_EPOCH + Duration::from_secs(NOW) - Duration::from_secs_f64(hours_ago * 3600.0);
    let file = File::open(path).unwrap();
    file.set_times(FileTimes::new().set_accessed(at).set_modified(at))
        .unwrap();
}

fn policy(cap_kib: u64, floor_kib: u64, critical_kib: u64) -> Policy {
    Policy {
        cap: cap_kib * KIB,
        floor: floor_kib * KIB,
        critical: critical_kib * KIB,
        lock_wait: Duration::ZERO,
        ..Policy::default()
    }
}

fn cargo(pid: u32, target_dir: Option<&Path>, cwd: Option<&Path>, args: &[&str]) -> Proc {
    Proc {
        pid,
        name: "cargo".into(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        target_dir: target_dir.map(Path::to_owned),
        build_dir: None,
        cwd: cwd.map(Path::to_owned),
    }
}

fn removed(report: &Report) -> Vec<String> {
    report
        .actions
        .iter()
        .filter(|action| action.verb == "removed" || action.verb == "would remove")
        .map(|action| {
            action
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

#[test]
fn a_target_over_its_cap_loses_its_least_recently_used_artifacts_first() {
    let home = Home::new();
    let target = home.work("openagents-target-1");
    home.unit(&target, "oldest-01", 400, 10.0);
    home.unit(&target, "older-02", 400, 8.0);
    home.unit(&target, "old-03", 400, 5.0);
    home.unit(&target, "fresh-04", 400, 0.5);
    home.file(&target.join("debug/build/serde-0f/out/lib.rs"), 400, 9.0);
    home.file(
        &target.join("debug/incremental/foo-1x/s-abc/dep-graph.bin"),
        400,
        7.0,
    );
    // An empty build folder ages by its own change time.
    let empty = target.join("debug/build/empty-0e");
    std::fs::create_dir_all(&empty).unwrap();
    age(&empty, 12.0);
    let pid = cargo(1, Some(&target), None, &["cargo", "build"]);
    // About 2.6 MiB in all; a 1.6 MiB cap evicts down to 1.2 MiB.
    let report = sweep(
        &home.layout(),
        &policy(1600, 0, 0),
        &fake(u64::MAX / 2, vec![pid]),
        false,
    );

    assert_eq!(
        removed(&report),
        [
            "empty-0e",
            "liboldest-01.*",
            "serde-0f",
            "libolder-02.*",
            "foo-1x"
        ],
        "{:#?}",
        report.actions
    );
    let deps = target.join("debug/deps");
    assert!(!deps.join("liboldest-01.rlib").exists());
    assert!(!deps.join("liboldest-01.rmeta").exists());
    assert!(!deps.join("oldest-01.d").exists());
    assert!(!target.join("debug/.fingerprint/oldest-01").exists());
    assert!(!target.join("debug/build/serde-0f").exists());
    assert!(deps.join("libold-03.rlib").exists());
    assert!(deps.join("libfresh-04.rlib").exists());
    assert!(target.join("debug/.fingerprint/fresh-04").exists());
    assert_eq!(report.actions[0].path, empty);
    assert!(report.actions[0].reason.contains("cap; untouched 12h"));
    assert!(report.actions[1].reason.contains("cap; untouched 10h"));
    // A held target is never removed whole.
    assert!(target.is_dir());
}

#[test]
fn artifacts_touched_recently_stay_even_over_the_cap() {
    let home = Home::new();
    let target = home.work("openagents-target-1");
    home.unit(&target, "a-01", 800, 1.0);
    home.unit(&target, "b-02", 800, 1.5);
    let report = sweep(
        &home.layout(),
        &policy(100, 0, 0),
        &fake(u64::MAX / 2, vec![]),
        false,
    );
    assert!(removed(&report).is_empty(), "{:#?}", report.actions);
    assert!(target.join("debug/deps/liba-01.rlib").exists());
}

#[test]
fn below_the_floor_cap_cleanup_uses_the_shorter_idle_window() {
    let home = Home::new();
    let target = home.work("openagents-target-1");
    home.unit(&target, "old-01", 800, 0.75);
    home.unit(&target, "fresh-02", 100, 0.1);
    let held = cargo(1, Some(&target), None, &["cargo", "build"]);
    let report = sweep(
        &home.layout(),
        &policy(500, 100, 0),
        &fake(0, vec![held]),
        false,
    );
    assert_eq!(removed(&report), ["libold-01.*"]);
    assert!(
        report
            .actions
            .iter()
            .any(|action| action.reason.contains("cap; untouched 45m"))
    );
    assert!(target.join("debug/deps/libfresh-02.rlib").exists());
}

#[test]
fn bounded_cleanup_rescans_after_the_build_finishes() {
    use std::os::unix::io::AsRawFd as _;
    let home = Home::new();
    let target = home.work("openagents-target-1");
    home.unit(&target, "old-01", 800, 10.0);
    home.unit(&target, "rebuilt-02", 800, 10.0);
    let lock = File::create(target.join("debug/.cargo-lock")).unwrap();
    // SAFETY: the test owns this descriptor until the simulated build ends.
    assert_eq!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) }, 0);
    let other = home.target("openagents-target-1-app", 800, 10.0);
    let layout = home.layout();
    let build_target = target.clone();
    let (send, receive) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let held = cargo(1, Some(&build_target), None, &["cargo", "build"]);
        sweep_waiting(
            &layout,
            &Policy {
                lock_wait: Duration::from_secs(3),
                ..policy(500, 0, 0)
            },
            &fake(u64::MAX / 2, vec![held]),
            false,
            |report| send.send(report.clone()).unwrap(),
        );
    });
    let until = std::time::Instant::now() + Duration::from_secs(2);
    while other
        .join("debug/deps/libopenagents_target_1_app-0a1b.rlib")
        .exists()
    {
        assert!(
            std::time::Instant::now() < until,
            "unlocked target must be swept first"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    // The waiting sweep must discard its stale scan of this now rebuilt unit.
    home.unit(&target, "rebuilt-02", 800, 0.1);
    drop(lock);
    let next = receive.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.join().unwrap();
    assert!(removed(&next).contains(&"libold-01.*".to_owned()));
    assert!(target.join("debug/deps/librebuilt-02.rlib").exists());
}

#[test]
fn cache_gc_is_bounded() {
    use std::os::unix::fs::PermissionsExt as _;
    let home = Home::new();
    let program = home.root.join("kache");
    std::fs::write(&program, "#!/bin/sh\n[ \"$1\" = gc ] || exit 1\nexit 0\n").unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(bounded_gc(&program, Duration::from_secs(1)).is_ok());
    std::fs::write(&program, "#!/bin/sh\nexec sleep 5\n").unwrap();
    let started = std::time::Instant::now();
    assert!(
        bounded_gc(&program, Duration::from_millis(50))
            .unwrap_err()
            .contains("exceeded")
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn cache_gc_runs_below_the_floor_but_never_during_a_dry_run() {
    struct CacheHost {
        machine: Fake,
        calls: std::cell::Cell<u32>,
    }
    impl Host for CacheHost {
        fn processes(&self) -> Result<Vec<Proc>, String> {
            self.machine.processes()
        }
        fn free(&self, path: &Path) -> std::io::Result<u64> {
            self.machine.free(path)
        }
        fn now(&self) -> SystemTime {
            self.machine.now()
        }
        fn stamp(&self, at: SystemTime) -> String {
            self.machine.stamp(at)
        }
        fn cache_gc(&self) -> Option<Result<(), String>> {
            self.calls.set(self.calls.get() + 1);
            Some(Ok(()))
        }
    }
    let home = Home::new();
    let mut host = CacheHost {
        machine: fake(0, vec![]),
        calls: std::cell::Cell::new(0),
    };
    let policy = policy(500, 100, 0);
    sweep_waiting(&home.layout(), &policy, &host, true, |_| {});
    assert_eq!(host.calls.get(), 0);
    let mut reports = Vec::new();
    sweep_waiting(&home.layout(), &policy, &host, false, |report| {
        reports.push(report.clone())
    });
    assert_eq!(host.calls.get(), 1);
    assert!(
        reports[0]
            .actions
            .iter()
            .any(|action| action.verb == "cache gc")
    );
    host.machine.free = Some(u64::MAX / 2);
    sweep_waiting(&home.layout(), &policy, &host, false, |_| {});
    assert_eq!(host.calls.get(), 1);
}

#[test]
fn actual_free_space_overrides_estimates_of_removed_bytes() {
    struct Actual(Fake);
    impl Host for Actual {
        fn processes(&self) -> Result<Vec<Proc>, String> {
            self.0.processes()
        }
        fn free(&self, path: &Path) -> std::io::Result<u64> {
            self.0.free(path)
        }
        fn now(&self) -> SystemTime {
            self.0.now()
        }
        fn stamp(&self, at: SystemTime) -> String {
            self.0.stamp(at)
        }
        // Unlinked files can remain open, or other builds can consume space.
        fn observed_free(&self, _: &Path) -> Option<u64> {
            Some(0)
        }
    }
    let home = Home::new();
    let target = home.work("openagents-target-1");
    home.unit(&target, "old-01", 400, 3.0);
    home.unit(&target, "older-02", 400, 4.0);
    let held = cargo(1, Some(&target), None, &["cargo", "build"]);
    let report = sweep(
        &home.layout(),
        &policy(10000, 100, 0),
        &Actual(fake(0, vec![held])),
        false,
    );
    assert_eq!(
        removed(&report).len(),
        2,
        "don't stop at an estimated floor"
    );
    assert_eq!(report.free_after, Some(0));
}

#[test]
fn a_dry_run_reports_the_same_removals_and_removes_nothing() {
    let home = Home::new();
    let target = home.work("openagents-target-1");
    home.unit(&target, "oldest-01", 800, 10.0);
    home.unit(&target, "fresh-02", 800, 0.1);
    let layout = home.layout();
    let dry = sweep(
        &layout,
        &policy(1000, 0, 0),
        &fake(u64::MAX / 2, vec![]),
        true,
    );
    assert_eq!(removed(&dry), ["liboldest-01.*"]);
    assert_eq!(dry.actions[0].verb, "would remove");
    assert!(target.join("debug/deps/liboldest-01.rlib").exists());
    let real = sweep(
        &layout,
        &policy(1000, 0, 0),
        &fake(u64::MAX / 2, vec![]),
        false,
    );
    assert_eq!(removed(&real), removed(&dry));
    assert!(!target.join("debug/deps/liboldest-01.rlib").exists());
}

#[test]
fn below_the_floor_unheld_targets_go_whole_and_held_ones_stay() {
    let home = Home::new();
    let by_env = home.target("openagents-target-1", 800, 10.0);
    let by_cwd = home.target("openagents-target-1-app", 800, 10.0);
    let by_arg = home.work("agent-targets/openagents-2775974171");
    home.unit(&by_arg, "x-01", 800, 10.0);
    let unheld = home.work("agent-targets/openagents-apps-openagents-1");
    home.unit(&unheld, "y-01", 800, 10.0);
    let procs = vec![
        cargo(10, Some(&by_env), Some(&home.root), &["cargo", "test"]),
        Proc {
            pid: 11,
            name: "rustc".into(),
            args: vec!["rustc".into()],
            target_dir: None,
            build_dir: None,
            cwd: Some(by_cwd.join("debug/build/ring-1/out")),
        },
        Proc {
            pid: 12,
            name: "rustc".into(),
            args: vec![
                "rustc".into(),
                "-L".into(),
                format!("dependency={}", by_arg.join("debug/deps").display()),
            ],
            target_dir: None,
            build_dir: None,
            cwd: Some(home.root.clone()),
        },
    ];
    // 10 KiB under a 4 MiB floor: removing every unheld target and then
    // every idle artifact still doesn't get there.
    let report = sweep(
        &home.layout(),
        &policy(u64::MAX / KIB, 8192, 0),
        &fake(10 * KIB, procs),
        false,
    );
    let names = removed(&report);
    assert_eq!(names[0], "openagents-apps-openagents-1", "{names:?}");
    assert!(!unheld.exists());
    // Held targets stay as folders; below the floor their idle artifacts go.
    for target in [&by_env, &by_cwd, &by_arg] {
        assert!(target.join("debug").is_dir(), "{}", target.display());
        assert!(!target.join("debug/deps").read_dir().unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".rlib")
        }));
    }
    let holders: Vec<_> = report
        .targets
        .iter()
        .map(|target| target.holder.clone().unwrap())
        .collect();
    assert_eq!(holders.len(), 3);
    assert!(holders.contains(&"cargo 10".to_owned()));
    assert!(holders.contains(&"rustc 11".to_owned()));
    assert!(holders.contains(&"rustc 12".to_owned()));
}

#[test]
fn the_floor_stops_the_sweep_once_it_is_back() {
    let home = Home::new();
    let older = home.target("openagents-target-1", 800, 20.0);
    let newer = home.target("openagents-target-1-app", 800, 3.0);
    // 100 KiB short of the floor: the older target alone covers it.
    let report = sweep(
        &home.layout(),
        &policy(u64::MAX / KIB, 1000, 0),
        &fake(900 * KIB, vec![]),
        false,
    );
    assert_eq!(removed(&report), ["openagents-target-1"]);
    assert!(!older.exists());
    assert!(newer.join("debug/deps").is_dir());
    assert!(report.free_after.unwrap() >= 1000 * KIB);
    assert!(report.actions[0].reason.contains("below the"));
}

#[test]
fn above_the_floor_no_whole_target_goes() {
    let home = Home::new();
    let target = home.target("openagents-target-1", 800, 20.0);
    let report = sweep(
        &home.layout(),
        &policy(u64::MAX / KIB, 1000, 0),
        &fake(2000 * KIB, vec![]),
        false,
    );
    assert!(report.actions.is_empty(), "{:#?}", report.actions);
    assert!(target.is_dir());
}

#[test]
fn the_dev_app_target_is_touched_only_when_the_disk_is_critical() {
    let home = Home::new();
    let app = home.target(DEV_APP, 800, 20.0);
    home.unit(&app, "old-01", 800, 20.0);
    let layout = home.layout();
    // Below the floor and over the cap, but not critical: untouched.
    let calm = sweep(
        &layout,
        &policy(100, 100_000, 50),
        &fake(60 * KIB, vec![]),
        false,
    );
    assert!(calm.actions.is_empty(), "{:#?}", calm.actions);
    assert!(app.join("debug/deps/libold-01.rlib").exists());
    // Critical: it goes like any unheld target.
    let critical = sweep(
        &layout,
        &policy(100, 100_000, 50),
        &fake(40 * KIB, vec![]),
        false,
    );
    assert_eq!(removed(&critical), [DEV_APP]);
    assert!(!app.exists());
}

#[test]
fn the_dev_app_target_goes_last_among_unheld_targets() {
    let home = Home::new();
    let app = home.target(DEV_APP, 800, 30.0);
    let shared = home.target("openagents-target-1", 800, 2.0);
    let report = sweep(
        &home.layout(),
        &policy(u64::MAX / KIB, 1000, 500),
        &fake(400 * KIB, vec![]),
        false,
    );
    assert_eq!(removed(&report), ["openagents-target-1"]);
    assert!(!shared.exists());
    assert!(app.is_dir());
}

#[test]
fn scratch_and_kept_paths_are_never_touched_and_links_never_followed() {
    let home = Home::new();
    let scratch = home.root.join(".openagents/scratch/claude-code-1/target");
    home.unit(&scratch, "research-01", 800, 50.0);
    // A copy that is a link into scratch, and a deps folder that is one.
    std::os::unix::fs::symlink(&scratch, home.work("openagents-target-link")).unwrap();
    let odd = home.work("openagents-target-odd");
    std::fs::create_dir_all(odd.join("debug/.fingerprint")).unwrap();
    std::os::unix::fs::symlink(scratch.join("debug/deps"), odd.join("debug/deps")).unwrap();
    let kept = home.target("openagents-target-kept", 800, 50.0);
    std::fs::create_dir_all(home.root.join(".openagents")).unwrap();
    std::fs::write(
        home.root.join(".openagents/disk-janitor.keep"),
        "# the owner's\n~/work/openagents-target-kept\n",
    )
    .unwrap();
    let report = sweep(
        &home.layout(),
        &policy(1, 1 << 30, 1 << 30),
        &fake(0, vec![]),
        false,
    );
    assert!(scratch.join("debug/deps/libresearch-01.rlib").exists());
    assert!(scratch.join("debug/.fingerprint/research-01").exists());
    assert!(kept.join("debug/deps").is_dir());
    assert!(
        report
            .actions
            .iter()
            .all(|action| !action.path.starts_with(&kept)),
        "{:#?}",
        report.actions
    );
    // The odd copy's link goes with the copy; what it pointed at stays.
    assert!(!odd.exists());
    assert!(
        home.work("openagents-target-link")
            .symlink_metadata()
            .is_ok()
    );
}

#[test]
fn copies_beyond_the_shared_targets_are_flagged_and_removed_once_idle() {
    let home = Home::new();
    let held = home.target("openagents-target-agent2-app", 800, 5.0);
    let idle = home.target("openagents-target-agent12012", 800, 3.0);
    let fresh = home.target("openagents-target-shared-cli", 800, 0.2);
    let granted = home.target("openagents-target-granted", 800, 30.0);
    std::fs::create_dir_all(home.root.join(".openagents/leases")).unwrap();
    std::fs::write(
        home.root.join(format!(".openagents/leases/{GRANTS_FILE}")),
        format!("{}\n", granted.display()),
    )
    .unwrap();
    let procs = vec![cargo(7, Some(&held), None, &["cargo", "build"])];
    let report = sweep(
        &home.layout(),
        &policy(u64::MAX / KIB, 0, 0),
        &fake(1 << 40, procs),
        false,
    );
    let by_path = |path: &Path| {
        report
            .actions
            .iter()
            .find(|action| action.path == path)
            .cloned()
    };
    let flag = by_path(&held).unwrap();
    assert_eq!(flag.verb, "flagged");
    assert!(flag.reason.contains("held by cargo 7"), "{}", flag.reason);
    assert!(held.is_dir());
    assert_eq!(by_path(&idle).unwrap().verb, "removed");
    assert!(!idle.exists());
    assert_eq!(by_path(&fresh).unwrap().verb, "flagged");
    assert!(fresh.is_dir());
    assert!(by_path(&granted).is_none());
    assert!(granted.is_dir());
}

#[test]
fn a_build_holding_cargo_lock_holds_the_target() {
    use std::os::unix::io::AsRawFd as _;
    let home = Home::new();
    let target = home.target("openagents-target-1", 800, 20.0);
    let lock = File::create(target.join("debug/.cargo-lock")).unwrap();
    // SAFETY: an exclusive lock on a descriptor the test owns.
    assert_eq!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) }, 0);
    let report = sweep(
        &home.layout(),
        &policy(u64::MAX / KIB, 1 << 30, 0),
        &fake(0, vec![]),
        false,
    );
    assert!(target.join("debug").is_dir());
    assert_eq!(
        report.targets[0].holder.as_deref(),
        Some("a build's cargo lock")
    );
    drop(lock);
    let report = sweep(
        &home.layout(),
        &policy(u64::MAX / KIB, 1 << 30, 0),
        &fake(0, vec![]),
        false,
    );
    assert_eq!(removed(&report), ["openagents-target-1"]);
}

#[test]
fn an_unreadable_process_table_holds_every_target() {
    let home = Home::new();
    let target = home.target("openagents-target-agent9", 800, 30.0);
    let host = Fake {
        procs: Err("ps: not found".into()),
        ..fake(0, vec![])
    };
    let report = sweep(
        &home.layout(),
        &policy(u64::MAX / KIB, 1 << 30, 0),
        &host,
        false,
    );
    assert!(target.join("debug").is_dir());
    assert!(
        report.actions[0].reason.contains("process table"),
        "{:#?}",
        report.actions
    );
}

#[test]
fn ps_and_lsof_output_become_build_processes() {
    let plain = "\
  101 /Users/me/.rustup/toolchains/1.97.1/bin/cargo build -p coder-lease
  102 /Users/me/.cargo/bin/kache daemon run
  103 /usr/bin/vim notes.md
  104 rustc --crate-name foo --out-dir /Users/me/work/openagents-target-1/debug/deps
  105 /Users/me/work/x/target/debug/build/ring-1/build-script-build
";
    let with_env = "\
  101 /Users/me/.rustup/toolchains/1.97.1/bin/cargo build -p coder-lease HOME=/Users/me CARGO_TARGET_DIR=/Users/me/work/openagents-target-1-app PWD=/Users/me/work/oa
  102 /Users/me/.cargo/bin/kache daemon run CARGO_BIN_EXE_openagents=/Users/me/work/openagents-target-1/debug/openagents
  103 /usr/bin/vim notes.md CARGO_TARGET_DIR=/Users/me/work/openagents-target-1
  104 rustc --crate-name foo --out-dir /Users/me/work/openagents-target-1/debug/deps CARGO_TARGET_DIR=rel/target
  105 /Users/me/work/x/target/debug/build/ring-1/build-script-build
";
    let mut procs = parse_ps(plain, with_env);
    let names: Vec<_> = procs
        .iter()
        .map(|proc| (proc.pid, proc.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            (101, "cargo"),
            (102, "kache"),
            (104, "rustc"),
            (105, "build-script-build")
        ]
    );
    add_cwds(
        &mut procs,
        "p101\nfcwd\nn/Users/me/work/oa\np104\nn/Users/me/work/openagents-target-agent3\n",
    );
    let t1 = Path::new("/Users/me/work/openagents-target-1");
    let app = Path::new("/Users/me/work/openagents-target-1-app");
    let agent3 = Path::new("/Users/me/work/openagents-target-agent3");
    assert!(procs[0].holds(app));
    assert!(!procs[0].holds(t1));
    // kache's other variables don't hold a target; only CARGO_TARGET_DIR does.
    assert!(!procs[1].holds(t1));
    // rustc: an argument and its working directory; a relative
    // CARGO_TARGET_DIR is read from the working directory.
    assert!(procs[2].holds(t1));
    assert!(procs[2].holds(agent3));
    assert_eq!(
        procs[2].target_dir.as_deref(),
        Some(Path::new("rel/target"))
    );
    assert!(procs[2].holds(&agent3.join("rel")));
    assert!(!procs[3].holds(t1));
}

#[test]
fn every_removal_is_logged_with_chicago_time_size_path_and_reason() {
    let home = Home::new();
    let target = home.work("openagents-target-1");
    home.unit(&target, "oldest-01", 800, 10.0);
    let layout = home.layout();
    let report = sweep(&layout, &policy(100, 0, 0), &fake(500 * GIB, vec![]), false);
    record(&layout, &report).unwrap();
    let log = std::fs::read_to_string(&layout.log).unwrap();
    let lines: Vec<_> = log.lines().collect();
    assert!(
        lines[0].starts_with("2026-10-17 6:00:00 PM CDT  sweep  free 500.0 GiB -> 500.0 GiB, freed 1008 KiB; artifact use watched since 1969-12-31 7:00:00 PM CDT; targets: openagents-target-1 "),
        "{log}"
    );
    let line = lines[1];
    assert!(
        line.starts_with("2026-10-17 6:00:00 PM CDT  removed  "),
        "{line}"
    );
    assert!(
        line.contains(&format!(
            "{}",
            target.join("debug/deps/liboldest-01.*").display()
        )),
        "{line}"
    );
    assert!(
        line.contains("over its 100 KiB cap; untouched 10h"),
        "{line}"
    );
    let last: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&layout.last).unwrap()).unwrap();
    assert_eq!(last["actions"][0]["verb"], "removed");
}

#[test]
fn local_time_reads_like_a_clock() {
    assert_eq!(local_time(0, "UTC"), "1970-01-01 12:00:00 AM UTC");
    assert_eq!(
        local_time(1_792_278_000 - 5 * 3600, "CDT"),
        "2026-10-17 6:00:00 PM CDT"
    );
    assert_eq!(
        local_time(1_709_208_000, "UTC"),
        "2024-02-29 12:00:00 PM UTC"
    );
}

#[test]
fn the_policy_reads_its_overrides() {
    let policy = Policy::from(&|name| match name {
        CAP_VAR => Some("80".into()),
        FLOOR_VAR => Some(" 150 ".into()),
        IDLE_VAR => Some("45".into()),
        _ => None,
    });
    assert_eq!(policy.cap, 80 * GIB);
    assert_eq!(policy.floor, 150 * GIB);
    assert_eq!(policy.critical, 40 * GIB);
    assert_eq!(policy.idle, Duration::from_secs(45 * 60));
}

#[test]
fn a_fresh_install_removes_no_artifact_until_it_has_watched_the_idle_time() {
    let home = Home::new();
    let layout = home.layout();
    std::fs::remove_file(&layout.state).unwrap();
    let target = home.work("openagents-target-1");
    home.unit(&target, "old-01", 800, 30.0);
    let first = sweep(&layout, &policy(100, 0, 0), &fake(500 * GIB, vec![]), false);
    assert!(removed(&first).is_empty(), "{:#?}", first.actions);
    let watch: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&layout.state).unwrap()).unwrap();
    assert_eq!(watch["since"], NOW);
    // An hour on: still within the two idle hours since watching began.
    let later = |hours: u64| Fake {
        now: UNIX_EPOCH + Duration::from_secs(NOW + hours * 3600),
        ..fake(500 * GIB, vec![])
    };
    assert!(removed(&sweep(&layout, &policy(100, 0, 0), &later(1), false)).is_empty());
    let third = sweep(&layout, &policy(100, 0, 0), &later(3), false);
    assert_eq!(removed(&third), ["libold-01.*"]);
    assert!(
        third.actions[0].reason.contains("untouched 3h"),
        "{:#?}",
        third.actions
    );
}

#[test]
fn a_read_artifact_counts_as_used_and_is_set_back_to_show_the_next_read() {
    let home = Home::new();
    let layout = home.layout();
    let target = home.work("openagents-target-1");
    home.unit(&target, "read-01", 800, 30.0);
    home.unit(&target, "unread-02", 800, 30.0);
    let rlib = target.join("debug/deps/libread-01.rlib");
    let times = |path: &Path| {
        let meta = path.metadata().unwrap();
        (meta.accessed().unwrap(), meta.modified().unwrap())
    };
    let read = |path: &Path| {
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut File::open(path).unwrap(), &mut bytes).unwrap();
    };
    // Equal times: macOS doesn't record a read.
    read(&rlib);
    let (accessed, modified) = times(&rlib);
    assert_eq!(accessed, modified);
    // A run sets every artifact a second back, so the next read shows.
    let first = sweep(
        &layout,
        &policy(1 << 30, 0, 0),
        &fake(500 * GIB, vec![]),
        false,
    );
    assert!(removed(&first).is_empty());
    let (accessed, modified) = times(&rlib);
    assert_eq!(accessed + Duration::from_secs(1), modified);
    read(&rlib);
    let (read_at, _) = times(&rlib);
    assert!(read_at > modified);
    let report = sweep(
        &layout,
        &policy(1200, 0, 0),
        &fake(500 * GIB, vec![]),
        false,
    );
    assert_eq!(
        removed(&report),
        ["libunread-02.*"],
        "{:#?}",
        report.actions
    );
    // Set back again, and its use kept.
    let (accessed, modified) = times(&rlib);
    assert_eq!(accessed + Duration::from_secs(1), modified);
    let watch: Watch = serde_json::from_slice(&std::fs::read(&layout.state).unwrap()).unwrap();
    assert_eq!(watch.used[&rlib.display().to_string()], unix(read_at));
    // Later runs remember the read though the file no longer shows it.
    let again = sweep(&layout, &policy(100, 0, 0), &fake(500 * GIB, vec![]), false);
    assert!(removed(&again).is_empty(), "{:#?}", again.actions);
    assert!(rlib.exists());
}

#[test]
fn artifacts_go_only_while_the_sweep_holds_the_cargo_locks() {
    use std::os::unix::io::AsRawFd as _;
    let home = Home::new();
    let target = home.work("openagents-target-1");
    home.unit(&target, "old-01", 800, 10.0);
    let lock = File::create(target.join("debug/.cargo-build-lock")).unwrap();
    // SAFETY: an exclusive lock on a descriptor the test owns.
    assert_eq!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) }, 0);
    let layout = home.layout();
    let busy = sweep(&layout, &policy(100, 0, 0), &fake(500 * GIB, vec![]), false);
    assert!(removed(&busy).is_empty(), "{:#?}", busy.actions);
    assert_eq!(busy.actions[0].verb, "skipped");
    assert_eq!(busy.actions[0].path, target.join("debug"));
    assert!(target.join("debug/deps/libold-01.rlib").exists());
    drop(lock);
    let free = sweep(&layout, &policy(100, 0, 0), &fake(500 * GIB, vec![]), false);
    assert_eq!(removed(&free), ["libold-01.*"]);
    // The sweep let go of the locks when it was done.
    let lock = File::open(target.join("debug/.cargo-lock")).unwrap();
    // SAFETY: as above.
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
}

#[test]
fn profiles_include_manifest_keyed_build_state_without_following_links() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path();
    let legacy = target.join("debug");
    let state = target.join("worktrees/ab/cdef/debug");
    let cross = target.join("worktrees/12/3456/wasm32-unknown-unknown/release");
    for profile in [&legacy, &state, &cross] {
        std::fs::create_dir_all(profile.join("deps")).unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(target.join("worktrees/ab"), target.join("worktrees/linked"))
        .unwrap();
    let mut expected = vec![legacy, state, cross];
    expected.sort();
    assert_eq!(profiles(target), expected);
}

fn worktree_state(home: &Home, target: &Path, key: &str, hours: f64, exists: bool) -> PathBuf {
    let state = target.join("worktrees/aa").join(key);
    home.unit(&state, "state-1234", 32, hours);
    let manifest = home.work(&format!("source-{key}/Cargo.toml"));
    if exists {
        home.file(&manifest, 1, hours);
    }
    let marker = state.join(".openagents-workspace");
    std::fs::write(&marker, format!("{}\n", manifest.display())).unwrap();
    age(&marker, hours);
    state
}

#[test]
fn deleted_and_idle_worktrees_lose_artifacts_but_active_and_recent_ones_stay() {
    let home = Home::new();
    let target = home.work(SHARED[0]);
    let orphan = worktree_state(&home, &target, "orphan", 0.1, false);
    let idle = worktree_state(&home, &target, "idle", 8.0, true);
    let recent = worktree_state(&home, &target, "recent", 8.0, true);
    // A Fresh build refreshes the marker even if no crate was recompiled.
    age(&recent.join(".openagents-workspace"), 1.0);
    let active = worktree_state(&home, &target, "active", 9.0, true);
    let process = Proc {
        build_dir: Some(active.clone()),
        ..cargo(10, Some(&target), None, &[])
    };
    let report = sweep(
        &home.layout(),
        &policy(10000, 0, 0),
        &fake(500 * GIB, vec![process]),
        false,
    );
    for state in [&orphan, &idle] {
        assert!(!state.join("debug/deps/libstate-1234.rlib").exists());
        // Reuse the same lock inode if a Cargo arrives during the sweep.
        assert!(state.join("debug/.cargo-lock").exists());
    }
    for state in [&recent, &active] {
        assert!(state.join("debug/deps/libstate-1234.rlib").exists());
    }
    assert!(
        report
            .actions
            .iter()
            .any(|action| action.reason.contains("worktree no longer exists"))
    );
    assert!(
        report
            .actions
            .iter()
            .any(|action| action.reason.contains("worktree build state unused"))
    );
}

#[test]
fn hashed_artifacts_count_toward_one_target_cap_and_unknown_processes_keep_them() {
    let home = Home::new();
    let target = home.work(SHARED[0]);
    let first = worktree_state(&home, &target, "first", 3.0, true);
    let second = worktree_state(&home, &target, "second", 3.0, true);
    // Younger than the whole-state idle age; the aggregate target cap applies.
    let before = sweep(
        &home.layout(),
        &policy(10000, 0, 0),
        &fake(500 * GIB, vec![]),
        true,
    );
    assert!(before.targets[0].bytes >= 2 * 32 * KIB);
    let unknown = Fake {
        procs: Err("ps unavailable".into()),
        ..fake(500 * GIB, vec![])
    };
    let mut idle_policy = policy(40, 0, 0);
    idle_policy.worktree_idle = Duration::ZERO;
    let _report = sweep(&home.layout(), &idle_policy, &unknown, false);
    for state in [&first, &second] {
        assert!(state.join("debug/deps/libstate-1234.rlib").exists());
    }
    let report = sweep(
        &home.layout(),
        &policy(40, 0, 0),
        &fake(500 * GIB, vec![]),
        false,
    );
    assert!(report.freed > 0);
    assert!(report.targets[0].bytes < before.targets[0].bytes);
}

#[cfg(unix)]
#[test]
fn a_locked_worktree_keeps_its_orphaned_artifacts() {
    use std::os::unix::io::AsRawFd as _;
    let home = Home::new();
    let target = home.work(SHARED[0]);
    let state = worktree_state(&home, &target, "locked", 10.0, false);
    let lock = File::create(state.join("debug/.cargo-lock")).unwrap();
    // SAFETY: the file remains open while the fixture holds its lock.
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    let _report = sweep(
        &home.layout(),
        &policy(10000, 0, 0),
        &fake(500 * GIB, vec![]),
        false,
    );
    assert!(state.join("debug/deps/libstate-1234.rlib").exists());
}

#[cfg(unix)]
#[test]
fn one_long_held_target_cannot_delay_other_targets_or_the_pass_summary() {
    use std::os::unix::io::AsRawFd as _;
    let home = Home::new();
    let held = home.target(SHARED[0], 800, 8.0);
    let other = home.target(SHARED[1], 800, 8.0);
    let release = held.join("release");
    home.file(&release.join("deps/libold-03.rlib"), 800, 8.0);
    let guards: Vec<_> = [held.join("debug"), release]
        .into_iter()
        .map(|profile| {
            let file = File::create(profile.join(".cargo-lock")).unwrap();
            // SAFETY: the fixture retains both guards beyond the complete sweep.
            assert_eq!(unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) }, 0);
            file
        })
        .collect();
    let layout = home.layout();
    let active = cargo(1, Some(&held), None, &[]);
    let (send, receive) = std::sync::mpsc::channel();
    let started = std::time::Instant::now();
    let worker = std::thread::spawn(move || {
        sweep_waiting(
            &layout,
            &Policy {
                lock_wait: Duration::from_millis(150),
                ..policy(1, 0, 0)
            },
            &fake(500 * GIB, vec![active]),
            false,
            |report| send.send(report.clone()).unwrap(),
        );
    });
    let report = receive
        .recv_timeout(Duration::from_secs(2))
        .expect("the pass must finish with both locks still held");
    worker.join().unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "one shared target wait budget, not one per profile"
    );
    assert!(
        !other
            .join("debug/deps/libopenagents_target_1_app-0a1b.rlib")
            .exists()
    );
    assert!(
        held.join("debug/deps/libopenagents_target_1-0a1b.rlib")
            .exists()
    );
    assert_eq!(
        report
            .actions
            .iter()
            .filter(|a| a.verb == "skipped")
            .count(),
        2
    );
    assert!(report.lines()[0].contains("sweep"));
    drop(guards);
}

#[test]
fn below_the_floor_idle_states_go_while_recent_and_active_states_stay() {
    let home = Home::new();
    let target = home.work(SHARED[0]);
    let idle = worktree_state(&home, &target, "idle", 0.75, true);
    let recent = worktree_state(&home, &target, "recent", 0.75, true);
    age(&recent.join(".openagents-workspace"), 0.1);
    let active = worktree_state(&home, &target, "active", 8.0, true);
    let process = Proc {
        build_dir: Some(active.clone()),
        ..cargo(1, Some(&target), None, &[])
    };
    let report = sweep(
        &home.layout(),
        &policy(10000, 1000, 0),
        &fake(0, vec![process]),
        false,
    );
    assert!(!idle.join("debug/deps/libstate-1234.rlib").exists());
    assert!(recent.join("debug/deps/libstate-1234.rlib").exists());
    assert!(active.join("debug/deps/libstate-1234.rlib").exists());
    assert!(report.freed > 0);
    assert!(report.targets[0].bytes >= 2 * 32 * KIB);
}

#[cfg(unix)]
#[test]
fn migrated_builds_allow_old_intermediates_beside_held_final_outputs() {
    use std::os::unix::io::AsRawFd as _;
    for legacy in [false, true] {
        let home = Home::new();
        let target = home.target(SHARED[0], 800, 3.0);
        let active = worktree_state(&home, &target, "active", 8.0, true);
        if legacy {
            age(&active.join(".openagents-workspace"), 0.1);
        }
        home.file(&target.join("debug/openagents"), 64, 8.0);
        home.file(
            &target.join("debug/build/old-build/out/generated.rs"),
            64,
            3.0,
        );
        home.file(
            &target.join("debug/incremental/old-session/graph.bin"),
            64,
            3.0,
        );
        let lock = File::create(target.join("debug/.cargo-artifact-lock")).unwrap();
        // SAFETY: retain the final-output lock while cleanup runs.
        assert_eq!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) }, 0);
        let process = Proc {
            build_dir: (!legacy).then_some(active.clone()),
            ..cargo(1, Some(&target), None, &[])
        };
        let report = sweep(
            &home.layout(),
            &policy(1, 0, 0),
            &fake(500 * GIB, vec![process]),
            false,
        );
        assert_eq!(
            target
                .join("debug/deps/libopenagents_target_1-0a1b.rlib")
                .exists(),
            legacy
        );
        assert_eq!(
            target
                .join("debug/build/old-build/out/generated.rs")
                .exists(),
            legacy
        );
        assert_eq!(
            target
                .join("debug/incremental/old-session/graph.bin")
                .exists(),
            legacy
        );
        assert!(
            target.join("debug/openagents").exists(),
            "final output stays"
        );
        assert!(
            active.join("debug/deps/libstate-1234.rlib").exists(),
            "running build state stays"
        );
        assert_eq!(report.freed > 0, !legacy);
    }
}

#[test]
fn process_queries_drain_large_output_and_have_a_deadline() {
    use std::os::unix::fs::PermissionsExt as _;
    let home = Home::new();
    let path = home.root.join("probe");
    std::fs::write(
        &path,
        "#!/bin/sh\nexec dd if=/dev/zero bs=1024 count=256 2>/dev/null\n",
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        bounded_output(path.to_str().unwrap(), &[], Duration::from_secs(2))
            .unwrap()
            .len(),
        256 * 1024
    );
    std::fs::write(&path, "#!/bin/sh\nexec sleep 5\n").unwrap();
    let started = std::time::Instant::now();
    assert!(
        bounded_output(path.to_str().unwrap(), &[], Duration::from_millis(50))
            .unwrap_err()
            .contains("exceeded")
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn exited_processes_do_not_discard_the_remaining_cwd_snapshot() {
    let known = cargo(1, None, Some(Path::new("/work")), &[]);
    let missing = cargo(2, None, None, &[]);
    let mut processes = vec![known.clone(), missing.clone()];
    finish_processes(&mut processes, |_| false).unwrap();
    assert_eq!(processes, vec![known]);
    processes.push(missing);
    assert!(finish_processes(&mut processes, |_| true).is_err());
}

#[test]
fn lsof_partial_output_survives_an_exited_pid() {
    use std::os::unix::fs::PermissionsExt as _;
    let home = Home::new();
    let path = home.root.join("lsof");
    std::fs::write(&path, "#!/bin/sh\nprintf 'p1\\nn/work\\n'\nexit 1\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        bounded_output(path.to_str().unwrap(), &[], Duration::from_secs(1)).unwrap(),
        "p1\nn/work\n"
    );
}
