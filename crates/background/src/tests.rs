//! The disk monitor end to end, under a temporary home with fake volumes
//! and a scratch task list. Nothing here reads or changes the real home.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use crate::inuse::{Processes, Snapshot, System};
use crate::paths::Layout;
use crate::plan::{Env, TaskFact, plan};
use crate::rule::{GB, Rule, disk};
use crate::run::{self, Cause, Outcome};
use crate::volume::{Space, Volumes};

/// A volume with a chosen size and free space.
struct Fixed {
    free: u64,
    total: u64,
}

impl Volumes for Fixed {
    fn space(&self, path: &Path) -> std::io::Result<Space> {
        use std::os::unix::fs::MetadataExt;
        Ok(Space {
            device: std::fs::metadata(path)?.dev(),
            free: self.free,
            total: self.total,
        })
    }
}

/// No process uses anything.
struct Idle;

impl Processes for Idle {
    fn snapshot(&self) -> Result<Snapshot, String> {
        Ok(Snapshot::default())
    }
}

/// One home at a time: a test that spawns a process (git, a child holding
/// a file) forks while another test's lock file is open, and the forked
/// child keeps that lock for a moment after the test drops it.
static SERIAL: Mutex<()> = Mutex::new(());

struct Home {
    _serial: std::sync::MutexGuard<'static, ()>,
    _dir: tempfile::TempDir,
    layout: Layout,
    tasks: Mutex<Vec<TaskFact>>,
}

impl Home {
    fn new() -> Self {
        let serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path(), None).unwrap();
        std::fs::create_dir_all(&layout.openagents).unwrap();
        Self {
            _serial: serial,
            _dir: dir,
            layout,
            tasks: Mutex::new(Vec::new()),
        }
    }

    fn task(&self, id: &str, worktree: &Path, target: &Path, ended: bool) {
        self.tasks.lock().unwrap().push(TaskFact {
            id: id.into(),
            worktree: worktree.to_owned(),
            target: target.to_owned(),
            ended,
            ..TaskFact::default()
        });
    }

    fn facts(&self) -> impl Fn() -> Result<Vec<TaskFact>, String> + Send + Sync + '_ {
        move || Ok(self.tasks.lock().unwrap().clone())
    }
}

fn env<'a>(
    home: &'a Home,
    facts: &'a (dyn crate::plan::Facts + 'a),
    volumes: &'a Fixed,
    processes: &'a dyn Processes,
) -> Env<'a> {
    Env {
        layout: &home.layout,
        facts: Some(facts),
        volumes,
        processes,
        now: crate::paths::now(),
        kache: None,
    }
}

/// A 1 TB volume with 20 GB free: below the start level, far from stop.
fn low() -> Fixed {
    Fixed {
        free: 20 * GB,
        total: 1_000 * GB,
    }
}

/// Make a fake Cargo target directory holding `size` bytes.
fn target(path: &Path, size: usize) {
    std::fs::create_dir_all(path.join("debug/incremental")).unwrap();
    std::fs::create_dir_all(path.join("debug/.fingerprint")).unwrap();
    std::fs::write(path.join("debug/.cargo-lock"), "").unwrap();
    std::fs::write(path.join("debug/deps.bin"), vec![7u8; size]).unwrap();
    std::fs::write(path.join("debug/incremental/cache"), vec![7u8; size]).unwrap();
    std::fs::write(
        path.join("CACHEDIR.TAG"),
        "Signature: 8a477f597d28d172789f06886806bc55",
    )
    .unwrap();
}

/// Set everything under `path` (and its slot lock) to `days` ago.
fn age(path: &Path, days: u64) {
    age_secs(path, days * 86_400);
}

/// Set everything under `path` (and its slot lock) to `secs` ago.
fn age_secs(path: &Path, secs: u64) {
    let when = SystemTime::now() - Duration::from_secs(secs);
    let mut lock = path.as_os_str().to_owned();
    lock.push(".lock");
    let mut stack = vec![path.to_owned(), PathBuf::from(lock)];
    while let Some(next) = stack.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&next) else {
            continue;
        };
        if meta.is_dir() {
            for entry in std::fs::read_dir(&next).unwrap() {
                stack.push(entry.unwrap().path());
            }
        }
        if !meta.file_type().is_symlink() {
            let _ = std::fs::File::open(&next).and_then(|file| file.set_modified(when));
        }
    }
    // Parents first changed their mtimes as children were written; set
    // the top again last.
    let _ = std::fs::File::open(path).and_then(|file| file.set_modified(when));
}

fn planned(report: &crate::plan::Plan) -> Vec<PathBuf> {
    report.items().map(|item| item.path.clone()).collect()
}

#[test]
fn ended_tasks_targets_go_and_running_tasks_targets_stay() {
    let home = Home::new();
    let targets = home.layout.targets();
    let ended = targets.join("openagents-aaaa-1111");
    let running = targets.join("openagents-bbbb-2222");
    let unknown = targets.join("openagents-cccc-3333");
    for path in [&ended, &running, &unknown] {
        target(path, 4096);
    }
    let worktrees = home.layout.worktrees();
    home.task("aaaa", &worktrees.join("a"), &ended, true);
    home.task("bbbb", &worktrees.join("b"), &running, false);
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    let report = run::run(&env, &disk(), Cause::Manual, false, true).unwrap();
    assert!(!ended.exists());
    assert!(running.exists());
    assert!(unknown.exists());
    let record = report.record.unwrap();
    assert_eq!(record.actions.len(), 1);
    assert_eq!(record.actions[0].evidence.task.as_deref(), Some("aaaa"));
    assert!(report.notice.unwrap().starts_with("Freed "));
}

#[test]
fn a_held_slot_lock_or_cargo_lock_keeps_a_directory() {
    let home = Home::new();
    let slot = home.layout.targets().join("openagents-abc-slot-1");
    let other = home.layout.agent_dir("openagents-target-agent2");
    target(&slot, 4096);
    target(&other, 4096);
    std::fs::write(home.layout.targets().join("openagents-abc-slot-1.lock"), "").unwrap();
    age(&slot, 10);
    age(&other, 10);
    let slot_lock = std::fs::File::options()
        .write(true)
        .open(home.layout.targets().join("openagents-abc-slot-1.lock"))
        .unwrap();
    slot_lock.lock().unwrap();
    let cargo = std::fs::File::options()
        .write(true)
        .open(other.join("debug/.cargo-lock"))
        .unwrap();
    cargo.lock().unwrap();
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    let report = run::run(&env, &disk(), Cause::Manual, false, true).unwrap();
    assert!(slot.exists());
    assert!(other.exists());
    let kept: Vec<_> = report
        .plan
        .kept
        .iter()
        .map(|kept| kept.why.clone())
        .collect();
    assert!(kept.iter().any(|why| why.contains("held")), "{kept:?}");
    drop((slot_lock, cargo));
    run::run(&env, &disk(), Cause::Manual, false, true).unwrap();
    assert!(!slot.exists());
    assert!(!other.exists());
}

#[test]
fn a_process_with_an_open_file_inside_keeps_it() {
    let home = Home::new();
    let agent = home.layout.agent_dir("openagents-target-agent7");
    target(&agent, 4096);
    age(&agent, 10);
    let mut child = Command::new("tail")
        .arg("-f")
        .arg(agent.join("debug/deps.bin"))
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    // Wait until the process table shows it.
    for _ in 0..50 {
        if System.snapshot().unwrap().inside(&agent) > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &System);
    let report = run::run(&env, &disk(), Cause::Manual, false, true).unwrap();
    let _ = child.kill();
    let _ = child.wait();
    assert!(agent.exists());
    assert!(
        report
            .plan
            .kept
            .iter()
            .any(|kept| kept.why.starts_with("in use"))
    );
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A repository with a remote, and `names` worktrees of ended tasks.
fn repo(home: &Home, names: &[&str]) -> (PathBuf, Vec<PathBuf>) {
    let root = home.layout.home.join("src");
    std::fs::create_dir_all(&root).unwrap();
    git(&root, &["init", "-q", "--bare", "remote.git"]);
    git(&root, &["init", "-q", "-b", "main", "repo"]);
    let repo = root.join("repo");
    std::fs::write(repo.join("file"), "one").unwrap();
    git(&repo, &["add", "file"]);
    git(&repo, &["commit", "-q", "-m", "one"]);
    git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            root.join("remote.git").to_str().unwrap(),
        ],
    );
    git(&repo, &["push", "-q", "origin", "main"]);
    let mut trees = Vec::new();
    for name in names {
        let path = home.layout.worktrees().join(name);
        std::fs::create_dir_all(home.layout.worktrees()).unwrap();
        git(
            &repo,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                name,
                path.to_str().unwrap(),
                "main",
            ],
        );
        home.task(name, &path, &home.layout.targets().join(name), true);
        trees.push(path);
    }
    (repo, trees)
}

#[test]
fn unsaved_worktrees_stay_and_a_clean_pushed_one_goes_and_comes_back() {
    let home = Home::new();
    let (repo, trees) = repo(&home, &["clean", "unpushed", "dirty", "stashed"]);
    let [clean, unpushed, dirty, stashed] = [&trees[0], &trees[1], &trees[2], &trees[3]];
    std::fs::write(unpushed.join("file"), "two").unwrap();
    git(unpushed, &["commit", "-q", "-am", "two"]);
    std::fs::write(dirty.join("new"), "untracked").unwrap();
    std::fs::write(stashed.join("file"), "stash me").unwrap();
    git(stashed, &["stash", "-q"]);
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    let report = run::run(&env, &disk(), Cause::Manual, false, true).unwrap();
    assert!(!clean.exists());
    for kept in [unpushed, dirty, stashed] {
        assert!(kept.exists(), "{}", kept.display());
    }
    let reasons: Vec<String> = report
        .plan
        .kept
        .iter()
        .map(|kept| kept.why.clone())
        .collect();
    for why in [
        "commits not on any remote",
        "uncommitted changes",
        "a stash was made on it",
    ] {
        assert!(reasons.iter().any(|r| r == why), "{why}: {reasons:?}");
    }
    let record = report.record.unwrap();
    let removed: Vec<_> = record
        .actions
        .iter()
        .filter(|action| action.outcome == Outcome::Removed)
        .collect();
    assert_eq!(removed.len(), 1);
    assert!(!git(&repo, &["worktree", "list"]).contains("/clean "));
    let restored = run::undo(&home.layout, &record.run).unwrap();
    assert!(
        restored.iter().all(|(_, result)| result.is_ok()),
        "{restored:?}"
    );
    assert!(clean.join("file").exists());
    assert_eq!(git(clean, &["symbolic-ref", "--short", "HEAD"]), "clean");
}

#[test]
fn ignored_files_keep_a_worktree_and_ignored_caches_do_not() {
    let home = Home::new();
    let (repo, trees) = repo(&home, &["env", "private", "cache"]);
    let [env_tree, private, cache] = [&trees[0], &trees[1], &trees[2]];
    std::fs::write(
        repo.join(".git/info/exclude"),
        ".env\nprivate/\ntarget/\nnode_modules/\n",
    )
    .unwrap();
    std::fs::write(env_tree.join(".env"), "SECRET=1").unwrap();
    std::fs::create_dir_all(private.join("private")).unwrap();
    std::fs::write(private.join("private/data"), "mine").unwrap();
    std::fs::create_dir_all(cache.join("target/debug")).unwrap();
    std::fs::write(cache.join("target/debug/out"), "built").unwrap();
    std::fs::create_dir_all(cache.join("web/node_modules/x")).unwrap();
    std::fs::write(cache.join("web/node_modules/x/index.js"), "").unwrap();
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    let dry = run::run(&env, &disk(), Cause::Manual, true, true).unwrap();
    let reasons: Vec<(PathBuf, String)> = dry
        .plan
        .kept
        .iter()
        .map(|kept| (kept.path.clone(), kept.why.clone()))
        .collect();
    assert!(
        reasons.contains(&(env_tree.clone(), "holds ignored files: .env".into())),
        "{reasons:?}"
    );
    assert!(
        reasons.contains(&(private.clone(), "holds ignored files: private/".into())),
        "{reasons:?}"
    );
    let real = run::run(&env, &disk(), Cause::Manual, false, true).unwrap();
    assert!(env_tree.join(".env").exists());
    assert!(private.join("private/data").exists());
    assert!(!cache.exists(), "{:?} {reasons:?}", real.record);
}

#[test]
fn symlinks_are_not_followed_and_other_volumes_are_refused() {
    let home = Home::new();
    let outside = home.layout.home.join("precious");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("keep"), "x").unwrap();
    std::fs::create_dir_all(home.layout.targets()).unwrap();
    // A slot past the count that is a link: never followed.
    std::os::unix::fs::symlink(&outside, home.layout.targets().join("p-slot-9")).unwrap();
    // A real one with a link inside to the outside folder.
    let slot = home.layout.targets().join("p-slot-8");
    target(&slot, 10);
    std::os::unix::fs::symlink(&outside, slot.join("debug/link")).unwrap();
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    run::run(&env, &disk(), Cause::Manual, false, true).unwrap();
    assert!(!slot.exists());
    assert!(outside.join("keep").exists());
    assert!(home.layout.targets().join("p-slot-9").is_symlink());
    assert!(crate::paths::mount_point(Path::new("/dev")));
    assert!(!crate::paths::mount_point(&home.layout.openagents));
}

#[test]
fn the_deny_list_beats_every_class() {
    let home = Home::new();
    let (_, _) = repo(&home, &["done"]);
    let ended = home.layout.targets().join("p-slot-5");
    target(&ended, 10);
    let agent = home.layout.agent_dir("openagents-target-agent1");
    target(&agent, 10);
    age(&agent, 30);
    let gate = home.layout.gate().join("pool/1/target");
    target(&gate, 10);
    age(&gate, 30);
    let mut rule = disk();
    rule.safety.deny = vec![
        "~/.openagents/targets".into(),
        "~/.openagents/worktrees".into(),
        "~/.openagents/gate".into(),
        "~/work".into(),
    ];
    let facts = home.facts();
    let volumes = Fixed {
        free: GB,
        total: 1_000 * GB,
    };
    let env = env(&home, &facts, &volumes, &Idle);
    let report = run::run(&env, &rule, Cause::Manual, false, true).unwrap();
    assert_eq!(report.plan.items().count(), 0);
    assert!(
        report
            .plan
            .kept
            .iter()
            .all(|kept| kept.why == "on the deny list")
    );
    assert!(ended.exists() && agent.exists() && gate.exists());
}

#[test]
fn a_checkout_target_needs_cachedir_tag_and_git_ignore() {
    let home = Home::new();
    let work = home.layout.home.join("work");
    for name in ["tagged", "untagged"] {
        let top = work.join(name);
        std::fs::create_dir_all(&top).unwrap();
        git(&top, &["init", "-q"]);
        std::fs::write(top.join(".gitignore"), "/target\n").unwrap();
        target(&top.join("target"), 10);
        age(&top.join("target"), 30);
    }
    std::fs::remove_file(work.join("untagged/target/CACHEDIR.TAG")).unwrap();
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    run::run(&env, &disk(), Cause::Manual, false, true).unwrap();
    assert!(!work.join("tagged/target").exists());
    assert!(work.join("untagged/target").exists());
    assert!(work.join("tagged/.gitignore").exists());
}

#[test]
fn the_dry_run_is_the_executed_plan_and_records_match_deletions() {
    let home = Home::new();
    let mut expected = 0;
    for n in 0..3 {
        let path = home
            .layout
            .agent_dir(&format!("openagents-target-agent{n}"));
        target(&path, 100_000 * (n + 1));
        age(&path, 10 + n as u64);
    }
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    let dry = run::run(&env, &disk(), Cause::Manual, true, true).unwrap();
    assert!(dry.record.is_none());
    for item in dry.plan.items() {
        expected += crate::paths::measure(&item.path, &home.layout.home)
            .unwrap()
            .bytes;
    }
    let real = run::run(&env, &disk(), Cause::Manual, false, true).unwrap();
    assert_eq!(planned(&dry.plan), planned(&real.plan));
    let record = real.record.unwrap();
    let done: Vec<PathBuf> = record.actions.iter().map(|a| a.path.clone()).collect();
    assert_eq!(done, planned(&dry.plan));
    assert_eq!(record.freed_sum, expected);
    assert!(
        record
            .actions
            .iter()
            .all(|a| a.outcome == Outcome::Deleted && a.bytes > 0)
    );
    let logged = crate::store::read_log(&home.layout);
    assert_eq!(logged.last().unwrap().freed_sum, expected);
}

#[test]
fn classes_run_in_order_oldest_first_until_the_goal() {
    let home = Home::new();
    // Class 2 candidates, oldest first.
    let old = home.layout.agent_dir("openagents-target-agent1");
    let older = home.layout.agent_dir("openagents-target-agent2");
    target(&old, 2_000_000);
    target(&older, 2_000_000);
    age(&old, 5);
    age(&older, 9);
    // A class 1 candidate.
    let leftover = home.layout.targets().join("p-slot-6");
    target(&leftover, 2_000_000);
    let facts = home.facts();
    // Free space 1 MB short of the stop level (300 GB on a 1 TB volume):
    // one item meets the goal.
    let total = 1_000 * GB;
    let volumes = Fixed {
        free: 300 * GB - 1_000_000,
        total,
    };
    let env = env(&home, &facts, &volumes, &Idle);
    let found = plan(&env, &disk(), true);
    assert_eq!(planned(&found), vec![leftover.clone()]);
    // More needed: class 1 first, then class 2 oldest first.
    let volumes = Fixed {
        free: 300 * GB - 10_000_000,
        total,
    };
    let env = Env {
        volumes: &volumes,
        ..env
    };
    let found = plan(&env, &disk(), true);
    assert_eq!(planned(&found), vec![leftover, older, old]);
    // A trigger (not forced) plans nothing above the start level.
    let volumes = Fixed {
        free: 250 * GB,
        total,
    };
    let env = Env {
        volumes: &volumes,
        ..env
    };
    assert_eq!(crate::plan::plan(&env, &disk(), false).items().count(), 0);
}

#[test]
fn cooldown_waits_and_an_emergency_does_not() {
    let mut rule: Rule = disk();
    rule.enabled = true;
    let total = 1_000 * GB;
    let now = 1_000_000;
    let start = rule.goal.start.of(total);
    let emergency = rule.goal.emergency.of(total);
    use crate::runner::decide;
    assert!(decide(&rule, &[(start + 1, total)], None, now).is_none());
    assert!(decide(&rule, &[(start - 1, total)], None, now).is_some());
    assert!(decide(&rule, &[(start - 1, total)], Some(now - 60), now).is_none());
    assert!(decide(&rule, &[(start - 1, total)], Some(now - 601), now).is_some());
    assert_eq!(
        decide(&rule, &[(emergency - 1, total)], Some(now - 60), now),
        Some(true)
    );
    let mut paused = rule.clone();
    paused.paused_until = Some(now + 10);
    assert!(decide(&paused, &[(emergency - 1, total)], None, now).is_none());
}

#[test]
fn the_trash_empties_only_in_an_emergency() {
    let home = Home::new();
    let trash = home.layout.trash().join("old-run/dir");
    std::fs::create_dir_all(&trash).unwrap();
    std::fs::write(trash.join("f"), vec![1u8; 4096]).unwrap();
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    run::run(&env, &disk(), Cause::Manual, false, true).unwrap();
    assert!(trash.exists());
    let volumes = Fixed {
        free: GB,
        total: 1_000 * GB,
    };
    let env = Env {
        volumes: &volumes,
        ..env
    };
    let report = run::run(&env, &disk(), Cause::Threshold, false, false).unwrap();
    assert!(!trash.parent().unwrap().exists());
    assert!(report.notice.unwrap().starts_with("Disk almost full"));
}

/// A home with one ended task's target on a low volume, checked for
/// `cause` under `rule`: whether the target was deleted, and the run's
/// recorded trigger.
fn checked(rule: &Rule, cause: Cause) -> (bool, Option<Cause>) {
    let home = Home::new();
    let ended = home.layout.targets().join("openagents-aaaa-1111");
    target(&ended, 4096);
    home.task("aaaa", &home.layout.worktrees().join("a"), &ended, true);
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    let report = crate::runner::check(&env, rule, cause).map(Result::unwrap);
    let trigger = report.and_then(|report| report.record).map(|r| r.trigger);
    (!ended.exists(), trigger)
}

#[test]
fn each_automatic_check_needs_its_own_trigger() {
    use crate::rule::Trigger;
    let enabled = || {
        let mut rule = disk();
        rule.enabled = true;
        rule
    };
    let automatic = [
        (Cause::HostStart, Trigger::HostStart),
        (Cause::TaskEnded, Trigger::TaskEnded),
        (Cause::Interval, Trigger::Interval { every_secs: 300 }),
    ];
    // Every trigger named: each automatic check runs.
    for (cause, _) in &automatic {
        assert!(
            checked(&enabled(), *cause).0,
            "{cause:?} with every trigger"
        );
    }
    // One trigger removed: only that path stops.
    for (_, removed) in &automatic {
        let mut rule = enabled();
        rule.triggers.retain(|trigger| trigger != removed);
        for (cause, trigger) in &automatic {
            let ran = checked(&rule, *cause).0;
            assert_eq!(ran, trigger != removed, "{cause:?} without {removed:?}");
        }
    }
    // No interval trigger: no interval schedule at all.
    let mut rule = enabled();
    rule.triggers
        .retain(|trigger| !matches!(trigger, Trigger::Interval { .. }));
    assert_eq!(rule.interval(), None);
    assert!(!crate::runner::fires(&rule, Cause::Interval));
    assert!(!crate::runner::fires(&rule, Cause::Threshold));
    // An interval check records `threshold` when the rule names it, and
    // `interval` when it does not.
    assert_eq!(
        checked(&enabled(), Cause::Interval).1,
        Some(Cause::Threshold)
    );
    let mut rule = enabled();
    rule.triggers
        .retain(|trigger| *trigger != Trigger::Threshold);
    assert_eq!(checked(&rule, Cause::Interval).1, Some(Cause::Interval));
}

#[test]
fn no_triggers_means_no_automatic_runs_but_a_manual_run_still_works() {
    let mut rule = disk();
    rule.triggers.clear();
    for cause in [
        Cause::HostStart,
        Cause::TaskEnded,
        Cause::Interval,
        Cause::Threshold,
    ] {
        assert!(!crate::runner::fires(&rule, cause));
        assert_eq!(checked(&rule, cause), (false, None), "{cause:?}");
    }
    assert!(crate::runner::fires(&rule, Cause::Manual));
    // A check is never the manual path, even for a rule with every trigger.
    assert_eq!(checked(&disk(), Cause::Manual), (false, None));
    // The run someone asks for ignores triggers.
    let home = Home::new();
    let ended = home.layout.targets().join("openagents-aaaa-1111");
    target(&ended, 4096);
    home.task("aaaa", &home.layout.worktrees().join("a"), &ended, true);
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    let report = run::run(&env, &rule, Cause::Manual, false, true).unwrap();
    assert!(!ended.exists());
    assert_eq!(report.record.unwrap().trigger, Cause::Manual);
}

// Plugins' background rules (docs/background/2026-10-02-disk-cleanup-plugin.md).

/// The built-in disk rule as a plugin's rule document asks for it.
fn plugin_rule(id: &str) -> Rule {
    let mut rule = disk();
    rule.id = id.into();
    rule.name = "Disk cleanup".into();
    rule.origin = crate::rule::Origin::Plugin {
        plugin: "whatever the file says".into(),
        version: "9".into(),
    };
    rule.needs = crate::rule::Needs {
        delete: crate::rule::Class::DELETABLE.to_vec(),
        tasks: true,
        notify: true,
        coder: false,
    };
    rule
}

/// Install a plugin pinning `rule` under the home's extensions.
fn install(home: &Home, slug: &str, rule: &Rule) -> PathBuf {
    let dir = home
        .layout
        .extensions()
        .join(crate::plugins::LOCAL_KEY)
        .join(slug)
        .join("0.1.0");
    std::fs::create_dir_all(dir.join("background")).unwrap();
    let text = serde_json::to_string_pretty(rule).unwrap();
    std::fs::write(dir.join(format!("background/{}.json", rule.id)), &text).unwrap();
    let record = serde_json::json!({
        "v": 1, "slug": slug, "name": "Disk cleanup", "summary": "Keeps the disk from filling up.",
        "version": "0.1.0", "publisher": "",
        "background": [{"name": rule.id, "digest": crate::plugins::digest(&text)}],
    });
    std::fs::write(
        dir.join("package.json"),
        serde_json::to_string_pretty(&record).unwrap(),
    )
    .unwrap();
    dir
}

/// The rule ids listed, without the phase 3 built-ins (all off here).
fn ids(home: &Home) -> Vec<String> {
    crate::store::list(&home.layout)
        .into_iter()
        .filter_map(Result::ok)
        .map(|rule| rule.id)
        .filter(|id| id == "disk" || crate::rule::built_in(id).is_none())
        .collect()
}

#[test]
fn an_installed_plugin_is_off_until_turned_on_and_runs_nothing() {
    let home = Home::new();
    install(&home, "disk-cleanup", &plugin_rule("disk-cleanup"));
    let installed = crate::plugins::installed(&home.layout);
    assert_eq!(installed.len(), 1);
    assert!(!installed[0].enabled);
    assert_eq!(ids(&home), vec!["disk"]);
    assert!(crate::store::load(&home.layout, "disk-cleanup").is_err());
}

#[test]
fn an_enabled_plugin_rule_plans_and_runs_like_the_built_in_and_disabling_stops_it() {
    let home = Home::new();
    install(&home, "disk-cleanup", &plugin_rule("disk-cleanup"));
    let targets = home.layout.targets();
    let ended = targets.join("openagents-aaaa-1111");
    let running = targets.join("openagents-bbbb-2222");
    target(&ended, 4096);
    target(&running, 4096);
    for n in 0..2 {
        let path = home
            .layout
            .agent_dir(&format!("openagents-target-agent{n}"));
        target(&path, 50_000);
        age(&path, 10);
    }
    let worktrees = home.layout.worktrees();
    home.task("aaaa", &worktrees.join("a"), &ended, true);
    home.task("bbbb", &worktrees.join("b"), &running, false);

    let on = crate::plugins::set_enabled(&home.layout, "disk-cleanup", true).unwrap();
    assert!(on.enabled);
    assert_eq!(ids(&home), vec!["disk", "disk-cleanup"]);
    let rule = crate::store::load(&home.layout, "disk-cleanup").unwrap();
    // The host set the origin, not the file.
    assert_eq!(
        rule.origin,
        crate::rule::Origin::Plugin {
            plugin: format!("{}:disk-cleanup", crate::plugins::LOCAL_KEY),
            version: "0.1.0".into(),
        }
    );
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    let built_in = run::run(&env, &disk(), Cause::Manual, true, true).unwrap();
    let from_plugin = run::run(&env, &rule, Cause::Manual, true, true).unwrap();
    assert!(!planned(&built_in.plan).is_empty());
    assert_eq!(planned(&built_in.plan), planned(&from_plugin.plan));
    let real = run::run(&env, &rule, Cause::Manual, false, true).unwrap();
    assert_eq!(planned(&real.plan), planned(&from_plugin.plan));
    assert!(!ended.exists());
    assert!(running.exists());
    assert_eq!(real.record.unwrap().rule, "disk-cleanup");

    // Pausing it from the computer keeps it the plugin's rule.
    crate::view::pause(&home.layout, "disk-cleanup", None, false).unwrap();
    assert!(
        !crate::store::load(&home.layout, "disk-cleanup")
            .unwrap()
            .enabled
    );
    crate::view::pause(&home.layout, "disk-cleanup", None, true).unwrap();

    crate::plugins::set_enabled(&home.layout, "disk-cleanup", false).unwrap();
    assert_eq!(ids(&home), vec!["disk"]);
    assert!(crate::store::load(&home.layout, "disk-cleanup").is_err());
}

#[test]
fn the_host_refuses_a_plugin_rule_that_asks_for_more_than_it_grants() {
    let refused = |change: &dyn Fn(&mut Rule), why: &str| {
        let home = Home::new();
        let mut rule = plugin_rule("disk-cleanup");
        change(&mut rule);
        install(&home, "disk-cleanup", &rule);
        let error = crate::plugins::set_enabled(&home.layout, "disk-cleanup", true).unwrap_err();
        assert!(error.contains(why), "{error}");
        assert!(crate::plugins::enabled(&home.layout).is_empty());
        assert_eq!(ids(&home), vec!["disk"]);
    };
    refused(
        &|rule| rule.safety.allow.push("~/Documents".into()),
        "outside the places the host cleans",
    );
    refused(
        &|rule| rule.safety.allow.push("~/.openagents/../Documents".into()),
        "`~/.openagents/../Documents`",
    );
    refused(
        &|rule| rule.classes.agent_targets = vec!["~/*".into()],
        "outside the places the host cleans",
    );
    refused(
        &|rule| rule.needs.delete = vec![crate::rule::Class::Incremental],
        "without asking for them",
    );
    refused(&|rule| rule.needs.tasks = false, "need the task store");
    refused(
        &|rule| rule.id = "disk".into(),
        "cannot replace the built-in",
    );
    refused(
        &|rule| rule.triggers = vec![crate::rule::Trigger::Interval { every_secs: 5 }],
        "a minute apart",
    );

    // Bytes that moved after the record pinned them are refused.
    let home = Home::new();
    let dir = install(&home, "disk-cleanup", &plugin_rule("disk-cleanup"));
    let mut widened = plugin_rule("disk-cleanup");
    widened.safety.allow.push("/".into());
    std::fs::write(
        dir.join("background/disk-cleanup.json"),
        serde_json::to_string_pretty(&widened).unwrap(),
    )
    .unwrap();
    let error = crate::plugins::set_enabled(&home.layout, "disk-cleanup", true).unwrap_err();
    assert!(error.contains("changed since it was pinned"), "{error}");
}

#[test]
fn an_edit_on_this_computer_cannot_widen_a_plugin_rule_and_its_files_are_never_candidates() {
    let home = Home::new();
    install(&home, "disk-cleanup", &plugin_rule("disk-cleanup"));
    crate::plugins::set_enabled(&home.layout, "disk-cleanup", true).unwrap();
    let mut rule = crate::store::load(&home.layout, "disk-cleanup").unwrap();
    rule.safety.allow.push("~/Documents".into());
    rule.needs.delete.clear();
    assert!(crate::store::save(&home.layout, &rule).is_err());
    // Written by hand, it does not load either.
    std::fs::create_dir_all(home.layout.rules()).unwrap();
    std::fs::write(
        home.layout.rules().join("disk-cleanup.json"),
        serde_json::to_vec(&rule).unwrap(),
    )
    .unwrap();
    assert!(crate::store::load(&home.layout, "disk-cleanup").is_err());
    std::fs::remove_file(home.layout.rules().join("disk-cleanup.json")).unwrap();
    // Installed plugins are on the host's own deny list.
    let deny = home.layout.deny(&disk());
    assert!(deny.contains(&home.layout.extensions()));
}

#[test]
fn shipped_disk_cleanup_package_is_pinned_opt_in_and_matches_host_policy() {
    let home = Home::new();
    let text = include_str!("../../../plugins/disk-cleanup/background/disk-cleanup.json");
    let package: serde_json::Value =
        serde_json::from_str(include_str!("../../../plugins/disk-cleanup/package.json")).unwrap();
    assert_eq!(
        package["background"][0]["digest"],
        crate::plugins::digest(text)
    );
    let rule: Rule = serde_json::from_str(text).unwrap();
    assert!(!rule.enabled);
    assert!(!crate::store::load(&home.layout, "disk").unwrap().enabled);
    assert_eq!(rule.goal, disk().goal);
    assert_eq!(rule.triggers, disk().triggers);
    assert_eq!(rule.actions, disk().actions);
    assert_eq!(rule.needs.delete, crate::rule::Class::DELETABLE);
    install(&home, "disk-cleanup", &rule);
    assert!(crate::plugins::rules(&home.layout).is_empty());
    crate::plugins::set_enabled(&home.layout, "disk-cleanup", true).unwrap();
    let admitted = crate::store::load(&home.layout, "disk-cleanup").unwrap();
    assert!(!admitted.active(crate::paths::now()));
    let ended = home.layout.targets().join("openagents-ended-1111");
    target(&ended, 4096);
    home.task(
        "ended",
        &home.layout.worktrees().join("ended"),
        &ended,
        true,
    );
    let facts = home.facts();
    let volumes = Fixed {
        free: 20 * GB,
        total: 100 * GB,
    };
    let environment = env(&home, &facts, &volumes, &Idle);
    let preview = run::run(&environment, &admitted, Cause::Manual, true, true).unwrap();
    assert!(preview.record.is_none());
    assert!(ended.exists());
    assert!(planned(&preview.plan).contains(&ended));
    let mut resumed = admitted.clone();
    resumed.paused_until = None;
    resumed.enabled = true;
    crate::store::save(&home.layout, &resumed).unwrap();
    assert!(
        crate::store::load(&home.layout, "disk-cleanup")
            .unwrap()
            .active(environment.now)
    );
    crate::plugins::set_enabled(&home.layout, "disk-cleanup", false).unwrap();
    assert!(crate::plugins::rules(&home.layout).is_empty());
}

#[test]
fn shipped_plugin_worktree_plan_matches_builtin_including_old_orphans() {
    let home = Home::new();
    let (repository, trees) = repo(
        &home,
        &[
            "ended",
            "running",
            "old-orphan",
            "recent-orphan",
            "dirty-orphan",
            "unpushed-orphan",
        ],
    );
    home.tasks
        .lock()
        .unwrap()
        .retain(|task| task.id == "ended" || task.id == "running");
    home.tasks
        .lock()
        .unwrap()
        .iter_mut()
        .find(|task| task.id == "running")
        .unwrap()
        .ended = false;
    std::fs::write(trees[4].join("file"), "unsaved").unwrap();
    std::fs::write(trees[5].join("file"), "unpushed").unwrap();
    git(&trees[5], &["commit", "-qam", "unpushed"]);
    for index in [1, 2, 4, 5] {
        age(&trees[index], 8);
    }
    age(&trees[3], 6);
    let rule: Rule = serde_json::from_str(include_str!(
        "../../../plugins/disk-cleanup/background/disk-cleanup.json"
    ))
    .unwrap();
    install(&home, "disk-cleanup", &rule);
    crate::plugins::set_enabled(&home.layout, "disk-cleanup", true).unwrap();
    let admitted = crate::store::load(&home.layout, "disk-cleanup").unwrap();
    let facts = home.facts();
    let volumes = low();
    let environment = env(&home, &facts, &volumes, &Idle);
    let builtin = plan(&environment, &disk(), false);
    let preview = plan(&environment, &admitted, false);
    assert_eq!(
        serde_json::to_value(&builtin).unwrap(),
        serde_json::to_value(&preview).unwrap()
    );
    let paths = planned(&preview);
    assert!(paths.contains(&trees[0]));
    assert!(paths.contains(&trees[2]));
    for index in [1, 3, 4, 5] {
        assert!(
            !paths.contains(&trees[index]),
            "{} must stay",
            trees[index].display()
        );
    }
    assert_eq!(git(&repository, &["worktree", "list"]).lines().count(), 7);
    assert!(trees.iter().all(|tree| tree.exists()));
}

#[test]
fn checking_many_worktrees_at_once_agrees_with_checking_each() {
    let home = Home::new();
    let (repo, trees) = repo(
        &home,
        &["clean", "unpushed", "dirty", "stashed", "env", "detached"],
    );
    std::fs::write(trees[1].join("file"), "two").unwrap();
    git(&trees[1], &["commit", "-q", "-am", "two"]);
    std::fs::write(trees[2].join("new"), "untracked").unwrap();
    std::fs::write(trees[3].join("file"), "stash me").unwrap();
    git(&trees[3], &["stash", "-q"]);
    std::fs::write(repo.join(".git/info/exclude"), ".env\n").unwrap();
    std::fs::write(trees[4].join(".env"), "SECRET=1").unwrap();
    git(&trees[5], &["checkout", "-q", "--detach"]);
    let mut paths = trees.clone();
    // A full checkout and a folder that is no worktree at all.
    paths.push(repo.clone());
    paths.push(home.layout.home.clone());
    let all = crate::git::removable_all(&paths);
    let each: Vec<_> = paths
        .iter()
        .map(|path| crate::git::removable(path))
        .collect();
    assert_eq!(all, each);
    assert!(all[0].is_ok() && all[5].is_ok(), "{all:?}");
    assert_eq!(all[5].as_ref().unwrap().branch, None);
    assert_eq!(all[0].as_ref().unwrap().branch.as_deref(), Some("clean"));
    for (index, why) in [
        (1, "commits not on any remote"),
        (2, "uncommitted changes"),
        (3, "a stash was made on it"),
        (4, "holds ignored files: .env"),
        (6, "a full checkout, not a worktree"),
    ] {
        assert_eq!(
            all[index].as_ref().err().map(String::as_str),
            Some(why),
            "{index}"
        );
    }
}

// Claude Code worktrees, the kache class, and the faster defaults (#10759).

/// Processes whose open files or working folders are these paths.
struct Inside(Vec<PathBuf>);

impl Processes for Inside {
    fn snapshot(&self) -> Result<Snapshot, String> {
        Ok(Snapshot::new(self.0.clone()))
    }
}

/// A checkout at `~/code/repo` pushed to a bare remote, with Claude Code
/// worktrees `names` under its `.claude/worktrees`, each on its own branch
/// at the pushed `main`.
fn claude_checkout(home: &Home, names: &[&str]) -> (PathBuf, Vec<PathBuf>) {
    let code = home.layout.home.join("code");
    std::fs::create_dir_all(&code).unwrap();
    git(&code, &["init", "-q", "--bare", "remote.git"]);
    git(&code, &["init", "-q", "-b", "main", "repo"]);
    let repo = code.join("repo");
    std::fs::write(repo.join(".gitignore"), ".claude/\n").unwrap();
    std::fs::write(repo.join("file"), "one").unwrap();
    git(&repo, &["add", "file", ".gitignore"]);
    git(&repo, &["commit", "-q", "-m", "one"]);
    let remote = code.join("remote.git");
    git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&repo, &["push", "-q", "origin", "main"]);
    let dir = repo.join(".claude/worktrees");
    std::fs::create_dir_all(&dir).unwrap();
    let trees = names
        .iter()
        .map(|name| {
            let path = dir.join(name);
            let branch = format!("worktree-{name}");
            git(
                &repo,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    &branch,
                    path.to_str().unwrap(),
                    "main",
                ],
            );
            path
        })
        .collect();
    (repo, trees)
}

/// Make a linked worktree look idle for `secs`: the folder, its `.git`
/// file, and Git's administrative files for it.
fn idle_worktree(path: &Path, secs: u64) {
    let admin = crate::paths::git_dir(path).unwrap();
    age_secs(&admin, secs);
    age_secs(path, secs);
}

#[test]
fn a_clean_pushed_idle_claude_worktree_goes_and_others_stay() {
    let home = Home::new();
    let (repo, trees) = claude_checkout(
        &home,
        &["clean", "unpushed", "live", "locked", "recent", "dirty"],
    );
    let [clean, unpushed, live, locked, recent, dirty] = [
        &trees[0], &trees[1], &trees[2], &trees[3], &trees[4], &trees[5],
    ];
    std::fs::write(unpushed.join("file"), "two").unwrap();
    git(unpushed, &["commit", "-q", "-am", "local only"]);
    std::fs::write(dirty.join("file"), "unsaved").unwrap();
    git(
        &repo,
        &[
            "worktree",
            "lock",
            "--reason",
            "claude agent locked (pid 1)",
            locked.to_str().unwrap(),
        ],
    );
    for tree in [clean, unpushed, live, locked, dirty] {
        idle_worktree(tree, 3 * 3600);
    }
    let facts = home.facts();
    let volumes = low();
    let using = Inside(vec![live.clone()]);
    let env = env(&home, &facts, &volumes, &using);
    let rule = disk();
    let found = plan(&env, &rule, true);
    let claude: Vec<&crate::plan::Item> = found
        .items()
        .filter(|item| item.class == crate::rule::Class::ClaudeWorktrees)
        .collect();
    assert_eq!(claude.len(), 1, "{:?}", found.kept);
    assert_eq!(&claude[0].path, clean);
    assert!(
        claude[0]
            .why
            .starts_with("Claude Code worktree, unused 3 hours")
    );
    assert!(claude[0].bytes > 0);
    let why = |path: &Path| {
        found
            .kept
            .iter()
            .find(|kept| kept.path == path)
            .map(|kept| kept.why.clone())
            .unwrap_or_default()
    };
    assert_eq!(why(unpushed), "commits not on any remote");
    assert!(why(live).starts_with("in use"), "{}", why(live));
    assert!(
        why(locked).starts_with("locked: claude agent"),
        "{}",
        why(locked)
    );
    assert!(why(recent).contains("used 0 hours ago"), "{}", why(recent));
    assert_eq!(why(dirty), "uncommitted changes");
    // The run removes only the clean one, and its undo brings it back.
    let report = run::run(&env, &rule, Cause::Manual, false, true).unwrap();
    assert!(!clean.exists());
    for tree in [unpushed, live, locked, recent, dirty] {
        assert!(tree.exists(), "{}", tree.display());
    }
    let record = report.record.unwrap();
    let removed: Vec<&run::Action> = record
        .actions
        .iter()
        .filter(|action| action.outcome == Outcome::Removed)
        .collect();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].class, crate::rule::Class::ClaudeWorktrees);
    let undone = run::undo(&home.layout, &record.run).unwrap();
    assert!(
        undone.iter().all(|(_, result)| result.is_ok()),
        "{undone:?}"
    );
    assert!(clean.join("file").exists());
}

#[test]
fn claude_worktrees_outside_the_named_checkouts_are_not_candidates() {
    let home = Home::new();
    let (_, trees) = claude_checkout(&home, &["clean"]);
    idle_worktree(&trees[0], 3 * 3600);
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    let mut rule = disk();
    rule.classes.claude_checkouts = vec!["~/work/*".into()];
    assert_eq!(plan(&env, &rule, true).items().count(), 0);
}

/// A stand-in `kache` whose store is `store` bytes against a cap of 100,
/// all of it private, and whose collector brings it to 80.
fn fake_kache(dir: &Path, cache: &Path, store: u64) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let disk = |store: u64| {
        format!(
            r#"{{"store_bytes":{store},"store_limit_bytes":100,"disk_private_bytes":{store},"cloned_into_targets_bytes":0}}"#
        )
    };
    let stats = format!(
        r#"{{"disk":{},"stores":[{{"path":"{}"}}]}}"#,
        disk(store),
        cache.display()
    );
    let gc = format!(
        r#"{{"skipped":false,"disk":{},"entries_dropped":7,"store_bytes_removed":420,"disk_bytes_reclaimed":410}}"#,
        disk(80)
    );
    let script = dir.join("kache");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\ncase \"$1\" in\n  stats) echo '{stats}' ;;\n  gc) echo '{gc}' ;;\n  *) exit 2 ;;\nesac\n"
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

#[test]
fn the_kache_class_plans_a_collection_and_never_deletes_store_files() {
    let home = Home::new();
    let cache = home.layout.home.join("Library/Caches/kache");
    let store = cache.join("store");
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(store.join("blob"), vec![1u8; 4096]).unwrap();
    let bin = home.layout.home.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let kache = crate::kache::Kache {
        program: fake_kache(&bin, &cache, 500),
        attempts: 1,
        wait: Duration::ZERO,
    };
    let facts = home.facts();
    let volumes = low();
    let env = Env {
        kache: Some(&kache),
        ..env(&home, &facts, &volumes, &Idle)
    };
    let rule = disk();
    let found = plan(&env, &rule, true);
    let items: Vec<&crate::plan::Item> = found.items().collect();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].class, crate::rule::Class::Kache);
    assert_eq!(items[0].path, store);
    // 400 bytes over the cap, all private.
    assert_eq!(items[0].bytes, 400);
    assert!(items[0].why.contains("kache's collector reclaims it"));
    let lines = run::describe(&found, &home.layout.home, false);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("~/Library/Caches/kache/store")),
        "{lines:?}"
    );
    let report = run::run(&env, &rule, Cause::Manual, false, true).unwrap();
    let record = report.record.unwrap();
    assert_eq!(record.actions.len(), 1);
    assert_eq!(record.actions[0].outcome, Outcome::Collected);
    assert_eq!(record.actions[0].bytes, 410);
    assert_eq!(record.freed_sum, 410);
    // kache's collector decides what goes; this crate removed nothing.
    assert!(store.join("blob").exists());
    // Under its cap, the store is kept and says why.
    let under = crate::kache::Kache {
        program: fake_kache(&bin, &cache, 90),
        ..kache
    };
    let env = Env {
        kache: Some(&under),
        ..env
    };
    let found = plan(&env, &rule, true);
    assert_eq!(found.items().count(), 0);
    assert!(found.kept.iter().any(
        |kept| kept.class == crate::rule::Class::Kache && kept.why.starts_with("under its cap")
    ));
}

#[test]
fn agent_builds_are_stale_after_six_hours() {
    let home = Home::new();
    let old = home.layout.agent_dir("openagents-target-agent1");
    let fresh = home.layout.agent_dir("openagents-target-agent2");
    target(&old, 4096);
    target(&fresh, 4096);
    age_secs(&old, 7 * 3600);
    age_secs(&fresh, 3600);
    let facts = home.facts();
    let volumes = low();
    let env = env(&home, &facts, &volumes, &Idle);
    let mut rule = disk();
    let found = plan(&env, &rule, true);
    let agent: Vec<PathBuf> = found
        .items()
        .filter(|item| item.class == crate::rule::Class::StaleTargets)
        .map(|item| item.path.clone())
        .collect();
    assert_eq!(agent, vec![old.clone()]);
    assert!(
        found
            .kept
            .iter()
            .any(|kept| kept.path == fresh && kept.why == "agent build, used 1 hour ago")
    );
    // The rule's own setting moves it.
    rule.classes.agent_idle_hours = 8;
    let found = plan(&env, &rule, true);
    assert!(
        !found
            .items()
            .any(|item| item.class == crate::rule::Class::StaleTargets)
    );
}

#[test]
fn rule_files_without_the_new_settings_get_the_defaults() {
    let mut value = serde_json::to_value(disk()).unwrap();
    let classes = value["classes"].as_object_mut().unwrap();
    classes.remove("agent_idle_hours");
    classes.remove("claude_worktree_hours");
    classes.remove("claude_checkouts");
    value["goal"]
        .as_object_mut()
        .unwrap()
        .remove("pressure_secs");
    let rule: Rule = serde_json::from_value(value).unwrap();
    assert_eq!(rule.classes.agent_idle_hours, crate::rule::AGENT_IDLE_HOURS);
    assert_eq!(rule.classes.agent_idle_hours, 6);
    assert_eq!(rule.classes.claude_worktree_hours, 2);
    assert!(rule.classes.claude_checkouts.is_empty());
    assert_eq!(rule.goal.pressure_secs, None);
    rule.validate().unwrap();
    // The built-in rule: 200 GB or 15% to start, a minute under pressure.
    let built_in = disk();
    assert_eq!(built_in.goal.start.of(1_000 * GB), 200 * GB);
    assert_eq!(built_in.goal.start.of(2_000 * GB), 300 * GB);
    assert_eq!(built_in.goal.pressure_secs, Some(60));
    assert_eq!(
        built_in.classes.claude_checkouts,
        vec!["~/code/*".to_owned(), "~/work/*".to_owned()]
    );
    let mut bad = disk();
    bad.goal.pressure_secs = Some(30);
    assert!(bad.validate().is_err());
    let mut bad = disk();
    bad.classes.agent_idle_hours = 0;
    assert!(bad.validate().is_err());
}

#[test]
fn checks_come_every_minute_under_pressure_and_every_five_otherwise() {
    let rule = disk();
    let total = 1_000 * GB;
    assert_eq!(rule.interval_at(None, None), Some(300));
    assert_eq!(rule.interval_at(Some(500 * GB), Some(total)), Some(300));
    assert_eq!(rule.interval_at(Some(150 * GB), Some(total)), Some(60));
    let mut calm = disk();
    calm.goal.pressure_secs = None;
    assert_eq!(calm.interval_at(Some(150 * GB), Some(total)), Some(300));
    // A check records the shorter interval as the next one.
    let home = Home::new();
    let facts = home.facts();
    let volumes = Fixed {
        free: 150 * GB,
        total,
    };
    let env = env(&home, &facts, &volumes, &Idle);
    let mut on = disk();
    on.enabled = true;
    let _ = crate::runner::check(&env, &on, Cause::Interval);
    let state = crate::store::State::load(&home.layout);
    let seen = &state.rules["disk"];
    assert_eq!(seen.next_check, Some(env.now + 60));
}
