use super::*;
use std::time::{Duration, UNIX_EPOCH};

struct Idle;
impl Processes for Idle {
    fn snapshot(&self) -> Result<Snapshot, String> {
        Ok(Snapshot::default())
    }
}

struct Home {
    _serial: std::sync::MutexGuard<'static, ()>,
    _temp: tempfile::TempDir,
    layout: Layout,
    now: u64,
}
impl Home {
    fn new() -> Self {
        let serial = crate::tests::SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temp = tempfile::tempdir().unwrap();
        let layout = Layout::new(temp.path(), None).unwrap();
        Self {
            _serial: serial,
            _temp: temp,
            layout,
            now: crate::paths::now() + 3 * 86_400,
        }
    }
    fn target(&self, relative: &str) -> PathBuf {
        let root = self.layout.home.join(relative);
        let profile = root.join("debug");
        std::fs::create_dir_all(profile.join("deps")).unwrap();
        std::fs::write(
            root.join("CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55",
        )
        .unwrap();
        std::fs::write(profile.join(".cargo-lock"), "").unwrap();
        std::fs::write(profile.join("deps/libold.rlib"), b"old output").unwrap();
        root
    }
    fn scan(&self) -> Plan {
        scan_at(&self.layout, Options::default(), None, &Idle, self.now).unwrap()
    }
    fn git(&self, path: &Path, words: &[&str]) {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(words)
            .env("GIT_AUTHOR_NAME", "Cleanup test")
            .env("GIT_COMMITTER_NAME", "Cleanup test")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {words:?}");
    }
    fn worktree(&self, relative: &str) -> PathBuf {
        let source = self.layout.home.join("openagents");
        let remote = self.layout.home.join("remote.git");
        if !source.exists() {
            std::fs::create_dir(&source).unwrap();
            self.git(
                &self.layout.home,
                &[
                    "init",
                    "--bare",
                    "-q",
                    "-b",
                    "main",
                    remote.to_str().unwrap(),
                ],
            );
            self.git(&source, &["init", "-q", "-b", "main"]);
            std::fs::write(source.join(".gitignore"), "notes.txt\ntarget/\n").unwrap();
            std::fs::write(source.join("source.txt"), "saved source").unwrap();
            self.git(&source, &["add", "."]);
            self.git(&source, &["commit", "-qm", "Fixture"]);
            self.git(
                &source,
                &["remote", "add", "origin", remote.to_str().unwrap()],
            );
            self.git(&source, &["push", "-q", "origin", "main"]);
        }
        let path = self.layout.home.join(relative);
        self.git(
            &source,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                path.to_str().unwrap(),
                "main",
            ],
        );
        path
    }
}

#[test]
fn finds_shared_custom_nested_and_registered_targets() {
    let home = Home::new();
    let roots: Vec<_> = [
        "wt/target-fixture",
        "work/agent-targets/openagents",
        "clef-m1/target",
        "work/pylon-p1/oa-target",
        ".openagents/update/openagents/target",
    ]
    .into_iter()
    .map(|p| home.target(p))
    .collect();
    let package = home
        .layout
        .home
        .join("go/pkg/mod/vendor/module/balancer/weightedtarget");
    std::fs::create_dir_all(package).unwrap();
    let plan = home.scan();
    assert!(plan.uncovered.is_empty(), "{:?}", plan.uncovered);
    for root in roots {
        assert!(
            plan.entries.iter().any(|entry| entry.target == root),
            "{}",
            root.display()
        );
    }
    let runtime = home
        .layout
        .home
        .join(".local/share/openagents-native-target/profiles");
    std::fs::create_dir_all(&runtime).unwrap();
    std::fs::write(runtime.join("notes.txt"), "application state").unwrap();
    assert!(
        !home
            .scan()
            .entries
            .iter()
            .any(|e| e.path.starts_with(&runtime))
    );
}

#[test]
fn fresh_nested_artifact_is_kept_and_old_sibling_is_removed() {
    let home = Home::new();
    let target = home.target("work/shared-target");
    let profile = target.join("worktrees/aa/key/debug");
    std::fs::create_dir_all(profile.join("deps")).unwrap();
    std::fs::write(profile.join(".cargo-lock"), "").unwrap();
    let fresh = profile.join("deps/libfresh.rlib");
    std::fs::write(&fresh, "fresh output").unwrap();
    File::open(&fresh)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(home.now))
        .unwrap();
    assert!(crate::paths::touched(&target) >= home.now);
    let plan = home.scan();
    assert!(!plan.entries.iter().any(|entry| entry.path == profile));
    let receipt = apply_with(&home.layout, &plan, None, &Idle).unwrap();
    assert!(receipt.actions.iter().any(|action| action.files > 0));
    assert!(fresh.exists());
    assert!(!target.join("debug/deps/libold.rlib").exists());
    assert!(target.join("debug/.cargo-lock").exists());
    assert!(target.exists());
}

#[test]
fn changed_outputs_and_tampered_plan_are_refused() {
    let home = Home::new();
    let root = home.target("wt/target-old");
    let mut plan = home.scan();
    std::fs::write(root.join("debug/deps/libold.rlib"), "changed output").unwrap();
    let result = apply_with(&home.layout, &plan, None, &Idle).unwrap();
    assert!(result.actions.iter().all(|a| a.files == 0));
    assert!(root.join("debug/deps/libold.rlib").exists());
    plan.created = crate::paths::now();
    plan.seal().unwrap();
    plan.options.worktrees = false;
    assert!(plan.verify(&home.layout.home).is_err());
}

#[test]
fn held_nested_cargo_lock_and_unreadable_process_table_keep_outputs() {
    let home = Home::new();
    let root = home.target("work/openagents-target-1");
    let profile = root.join("worktrees/aa/key/debug");
    std::fs::create_dir_all(profile.join("deps")).unwrap();
    std::fs::write(profile.join(".cargo-lock"), "").unwrap();
    std::fs::write(profile.join("deps/libold.rlib"), "old output").unwrap();
    let file = File::create(profile.join(".cargo-build-lock")).unwrap();
    file.try_lock().unwrap();
    let plan = home.scan();
    assert!(plan.kept.iter().any(|entry| entry.path == profile));
    struct Unknown;
    impl Processes for Unknown {
        fn snapshot(&self) -> Result<Snapshot, String> {
            Err("unreadable".into())
        }
    }
    assert!(scan_at(&home.layout, Options::default(), None, &Unknown, home.now).is_err());
}

#[test]
fn registered_worktrees_are_found_and_ignored_notes_survive() {
    let home = Home::new();
    let clean = home.worktree("wt/old-clean");
    let notes = home.worktree("work/old-notes");
    std::fs::write(notes.join("notes.txt"), "unsaved notes").unwrap();
    let plan = home.scan();
    assert!(
        plan.entries
            .iter()
            .any(|e| e.path == clean && e.kind == Kind::Worktree)
    );
    assert!(
        plan.kept
            .iter()
            .any(|e| e.path == notes && e.reason.contains("ignored"))
    );
    let receipt = apply_with(&home.layout, &plan, None, &Idle).unwrap();
    assert!(!clean.exists());
    assert!(notes.join("notes.txt").exists());
    assert!(
        receipt
            .actions
            .iter()
            .any(|a| a.undo.is_some() && a.files == 1)
    );
}

#[test]
fn strict_age_keep_file_and_symlinks_cannot_be_bypassed() {
    let home = Home::new();
    let root = home.target("wt/target-old");
    std::fs::create_dir_all(home.layout.openagents.clone()).unwrap();
    std::fs::write(
        home.layout.openagents.join("disk-janitor.keep"),
        format!("{}\n", root.display()),
    )
    .unwrap();
    assert!(home.scan().entries.is_empty());
    assert!(
        scan_at(
            &home.layout,
            Options {
                minimum_age_secs: 3_600,
                ..Default::default()
            },
            None,
            &Idle,
            home.now
        )
        .is_err()
    );
    let linked = home.layout.home.join("work/target-link");
    std::fs::create_dir_all(linked.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&root, &linked).unwrap();
    assert!(inventory::safe(&home.layout.home, &linked).is_err());
}

#[test]
fn registry_finds_custom_targets_beyond_the_bounded_search() {
    let home = Home::new();
    let root = home.target("custom/a/b/c/d/e/f/g/h/output");
    let registry = home.layout.openagents.join("build-targets");
    std::fs::create_dir_all(&registry).unwrap();
    std::fs::write(registry.join("fixture"), format!("{}\n", root.display())).unwrap();
    assert!(home.scan().entries.iter().any(|entry| entry.target == root));
}

#[test]
fn pushed_but_unmerged_worktree_and_live_task_stay() {
    let home = Home::new();
    let path = home.worktree("wt/unmerged");
    std::fs::write(path.join("source.txt"), "new feature").unwrap();
    home.git(&path, &["add", "."]);
    home.git(&path, &["commit", "-qm", "Unmerged fixture"]);
    home.git(&path, &["push", "-q", "origin", "HEAD:refs/heads/feature"]);
    let plan = home.scan();
    assert!(
        plan.kept
            .iter()
            .any(|entry| entry.path == path && entry.reason.contains("merged"))
    );
    let managed = home.worktree(".openagents/worktrees/task");
    let facts = || {
        Ok(vec![crate::records::TaskFact {
            worktree: managed.clone(),
            ended: false,
            ..Default::default()
        }])
    };
    let plan = scan_at(
        &home.layout,
        Options::default(),
        Some(&facts),
        &Idle,
        home.now,
    )
    .unwrap();
    assert!(
        plan.kept
            .iter()
            .any(|entry| entry.path == managed && entry.reason.contains("task"))
    );
}

#[test]
fn busy_state_does_not_hide_old_outputs_in_another_state() {
    let home = Home::new();
    let root = home.target("work/agent-targets/openagents");
    let old = root.join("worktrees/aa/old/debug");
    let busy = root.join("worktrees/bb/busy");
    std::fs::create_dir_all(old.join("deps")).unwrap();
    std::fs::write(old.join(".cargo-lock"), "").unwrap();
    std::fs::write(old.join("deps/libold.rlib"), "old").unwrap();
    struct Busy(Vec<PathBuf>);
    impl Processes for Busy {
        fn snapshot(&self) -> Result<Snapshot, String> {
            Ok(Snapshot::new(self.0.clone()))
        }
    }
    let processes = Busy(vec![root.clone(), busy]);
    let plan = scan_at(&home.layout, Options::default(), None, &processes, home.now).unwrap();
    assert!(plan.entries.iter().any(|entry| entry.path == old));
    assert!(
        !plan
            .entries
            .iter()
            .any(|entry| entry.path == root.join("debug"))
    );
    let receipt = apply_with(&home.layout, &plan, None, &processes).unwrap();
    assert!(receipt.actions.iter().any(|entry| entry.files == 1));
    assert!(root.join("debug/deps/libold.rlib").exists());
}

#[test]
fn recently_used_state_stays_even_when_its_artifacts_are_old() {
    let home = Home::new();
    let root = home.target("work/agent-targets/keyed");
    let state = root.join("worktrees/aa/key");
    let profile = state.join("debug");
    std::fs::create_dir_all(profile.join("deps")).unwrap();
    std::fs::write(profile.join(".cargo-lock"), "").unwrap();
    std::fs::write(profile.join("deps/libold.rlib"), "old output").unwrap();
    let plan = home.scan();
    assert!(plan.entries.iter().any(|entry| entry.path == profile));
    let marker = state.join(".openagents-workspace");
    std::fs::write(&marker, "workspace").unwrap();
    File::open(marker)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(home.now))
        .unwrap();
    let result = apply_with(&home.layout, &plan, None, &Idle).unwrap();
    assert!(
        result
            .actions
            .iter()
            .any(|action| action.path == profile && action.files == 0)
    );
    assert!(profile.join("deps/libold.rlib").exists());
    assert!(
        home.scan()
            .kept
            .iter()
            .any(|entry| entry.path == profile && entry.reason.contains("minimum age"))
    );
}

#[test]
fn apply_rechecks_processes_and_locks_after_a_plan_is_saved() {
    let home = Home::new();
    let root = home.target("wt/target-planned");
    let plan = home.scan();
    let lock = File::open(root.join("debug/.cargo-lock")).unwrap();
    lock.try_lock().unwrap();
    let result = apply_with(&home.layout, &plan, None, &Idle).unwrap();
    assert!(result.actions.iter().all(|action| action.files == 0));
    drop(lock);
    struct Busy(PathBuf);
    impl Processes for Busy {
        fn snapshot(&self) -> Result<Snapshot, String> {
            Ok(Snapshot::new([self.0.clone()]))
        }
    }
    let result = apply_with(&home.layout, &plan, None, &Busy(root.clone())).unwrap();
    assert!(result.actions.iter().all(|action| action.files == 0));
    assert!(root.join("debug/deps/libold.rlib").exists());
}

#[test]
fn git_and_nested_cargo_locks_keep_worktrees() {
    let home = Home::new();
    let path = home.worktree("wt/locked");
    let plan = home.scan();
    let admin = crate::paths::git_dir(&path).unwrap();
    std::fs::write(admin.join("index.lock"), "").unwrap();
    assert!(
        home.scan()
            .kept
            .iter()
            .any(|entry| entry.path == path && entry.reason.contains("Git"))
    );
    assert!(
        apply_with(&home.layout, &plan, None, &Idle)
            .unwrap()
            .actions
            .iter()
            .all(|a| a.files == 0)
    );
    std::fs::remove_file(admin.join("index.lock")).unwrap();
    let target = path.join("target/debug");
    std::fs::create_dir_all(&target).unwrap();
    let lock = File::create(target.join(".cargo-lock")).unwrap();
    lock.try_lock().unwrap();
    assert!(!home.scan().entries.iter().any(|entry| entry.path == path));
    assert!(
        apply_with(&home.layout, &plan, None, &Idle)
            .unwrap()
            .actions
            .iter()
            .all(|a| a.files == 0)
    );
    assert!(path.exists());
}

#[test]
fn plans_round_trip_without_overwriting_and_reject_a_lower_age() {
    let home = Home::new();
    home.target("wt/target-saved");
    let mut plan = home.scan();
    plan.created = crate::paths::now();
    plan.seal().unwrap();
    let file = home.layout.home.join("plan.json");
    save(&file, &plan).unwrap();
    assert!(save(&file, &plan).is_err());
    let mut loaded: Plan = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    loaded.verify(&home.layout.home).unwrap();
    loaded.options.minimum_age_secs = 1;
    loaded.seal().unwrap();
    assert!(loaded.verify(&home.layout.home).is_err());
}

#[test]
fn an_unknown_debug_deps_folder_is_not_a_cargo_target() {
    let home = Home::new();
    let source = home.layout.home.join("work/debug/deps/source.rs");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, "saved source").unwrap();
    assert!(home.scan().entries.is_empty());
    assert!(source.exists());
}

#[test]
fn receipt_must_be_writable_before_any_removal() {
    let home = Home::new();
    let root = home.target("wt/target-receipt");
    let plan = home.scan();
    let result = apply_with_recorded(&home.layout, &plan, None, &Idle, |_| {
        Err("Cannot save receipt.".into())
    });
    assert!(result.is_err());
    assert!(root.join("debug/deps/libold.rlib").exists());
}
