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

struct Home {
    _dir: tempfile::TempDir,
    layout: Layout,
    tasks: Mutex<Vec<TaskFact>>,
}

impl Home {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path(), None).unwrap();
        std::fs::create_dir_all(&layout.openagents).unwrap();
        Self {
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
    let when = SystemTime::now() - Duration::from_secs(days * 86_400);
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
        expected += crate::paths::measure(&item.path).unwrap().bytes;
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
    // Free space 1 MB short of the stop level, on a small volume: one item
    // meets the goal.
    let total = 200 * GB;
    let volumes = Fixed {
        free: 60 * GB - 1_000_000,
        total,
    };
    let env = env(&home, &facts, &volumes, &Idle);
    let found = plan(&env, &disk(), true);
    assert_eq!(planned(&found), vec![leftover.clone()]);
    // More needed: class 1 first, then class 2 oldest first.
    let volumes = Fixed {
        free: 60 * GB - 10_000_000,
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
        free: 50 * GB,
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
    let rule: Rule = disk();
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
    let automatic = [
        (Cause::HostStart, Trigger::HostStart),
        (Cause::TaskEnded, Trigger::TaskEnded),
        (Cause::Interval, Trigger::Interval { every_secs: 300 }),
    ];
    // Every trigger named: each automatic check runs.
    for (cause, _) in &automatic {
        assert!(checked(&disk(), *cause).0, "{cause:?} with every trigger");
    }
    // One trigger removed: only that path stops.
    for (_, removed) in &automatic {
        let mut rule = disk();
        rule.triggers.retain(|trigger| trigger != removed);
        for (cause, trigger) in &automatic {
            let ran = checked(&rule, *cause).0;
            assert_eq!(ran, trigger != removed, "{cause:?} without {removed:?}");
        }
    }
    // No interval trigger: no interval schedule at all.
    let mut rule = disk();
    rule.triggers
        .retain(|trigger| !matches!(trigger, Trigger::Interval { .. }));
    assert_eq!(rule.interval(), None);
    assert!(!crate::runner::fires(&rule, Cause::Interval));
    assert!(!crate::runner::fires(&rule, Cause::Threshold));
    // An interval check records `threshold` when the rule names it, and
    // `interval` when it does not.
    assert_eq!(checked(&disk(), Cause::Interval).1, Some(Cause::Threshold));
    let mut rule = disk();
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
