use super::*;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};

fn run(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn write(dir: &Path, path: &str, text: &str) {
    let file = dir.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

fn configure(dir: &Path) {
    run(dir, &["config", "user.name", "Lander"]);
    run(dir, &["config", "user.email", "lander@example.com"]);
    run(dir, &["config", "commit.gpgsign", "false"]);
}

/// A bare `origin` holding a three-package workspace (`a` depends on `c`;
/// `b` stands alone) and a docs file.
fn origin(root: &Path) -> PathBuf {
    let remote = root.join("origin.git");
    std::fs::create_dir_all(&remote).unwrap();
    run(&remote, &["init", "-q", "--bare", "-b", "main"]);
    let seed = root.join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    run(&seed, &["init", "-q", "-b", "main"]);
    configure(&seed);
    write(
        &seed,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/a\", \"crates/b\", \"crates/c\"]\nresolver = \"2\"\n",
    );
    write(&seed, ".gitignore", "target\nCargo.lock\n");
    for (name, extra) in [
        ("a", "\n[dependencies]\nc = { path = \"../c\" }\n"),
        ("b", ""),
        ("c", ""),
    ] {
        write(
            &seed,
            &format!("crates/{name}/Cargo.toml"),
            &format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{extra}"
            ),
        );
        write(&seed, &format!("crates/{name}/src/lib.rs"), "\n");
    }
    write(&seed, "docs/readme.md", "# Readme\n\nfirst line\n");
    run(&seed, &["add", "-A"]);
    run(&seed, &["commit", "-q", "-m", "seed"]);
    run(
        &seed,
        &[
            "push",
            "-q",
            remote.to_str().unwrap(),
            "HEAD:refs/heads/main",
        ],
    );
    remote
}

/// A host's clone of `remote`, with `path` written and committed.
fn host(root: &Path, remote: &Path, name: &str, path: &str, text: &str) -> PathBuf {
    let dir = root.join(name);
    let output = std::process::Command::new("git")
        .args([
            "clone",
            "-q",
            remote.to_str().unwrap(),
            dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    configure(&dir);
    write(&dir, path, text);
    run(&dir, &["add", "-A"]);
    run(
        &dir,
        &["commit", "-q", "-m", &format!("{name} changes {path}")],
    );
    dir
}

#[derive(Default)]
struct Counting {
    checks: usize,
    red: bool,
    notes: Vec<String>,
}

impl Hooks for Counting {
    fn check(&mut self) -> Vec<String> {
        self.checks += 1;
        if self.red {
            vec!["a test fails".into()]
        } else {
            Vec::new()
        }
    }
    fn note(&mut self, text: &str) {
        self.notes.push(text.to_owned());
    }
    fn stopping(&self) -> bool {
        false
    }
}

fn plan(worktree: &Path) -> Plan<'_> {
    Plan {
        worktree,
        branch: "main",
        attempts: 30,
        backoff: Backoff {
            first: Duration::from_millis(10),
            cap: Duration::from_millis(120),
        },
    }
}

#[test]
fn linked_worktrees_share_the_fetch_lock_but_other_repositories_do_not() {
    let dir = tempfile::tempdir().unwrap();
    let remote = origin(dir.path());
    let first = host(dir.path(), &remote, "first", "docs/first.md", "first");
    let linked = dir.path().join("linked");
    run(
        &first,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            linked.to_str().unwrap(),
        ],
    );
    let other = host(dir.path(), &remote, "other", "docs/other.md", "other");
    let held = fetch_lock(&first).unwrap();
    held.lock().unwrap();
    let contender = fetch_lock(&linked).unwrap();
    assert!(matches!(
        contender.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    fetch_lock(&other).unwrap().try_lock().unwrap();
    held.unlock().unwrap();
    contender.try_lock().unwrap();
}

#[test]
fn concurrent_fetches_from_linked_worktrees_all_observe_the_remote() {
    const FETCHERS: usize = 8;
    let dir = tempfile::tempdir().unwrap();
    let remote = origin(dir.path());
    let first = host(dir.path(), &remote, "first", "docs/first.md", "first");
    let mut worktrees = vec![first.clone()];
    for i in 1..FETCHERS {
        let linked = dir.path().join(format!("linked-{i}"));
        run(
            &first,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                linked.to_str().unwrap(),
            ],
        );
        worktrees.push(linked);
    }
    let seed = dir.path().join("seed");
    for round in 0..3 {
        write(&seed, "docs/readme.md", &format!("round {round}"));
        run(&seed, &["commit", "-qam", "advance origin"]);
        run(
            &seed,
            &[
                "push",
                "-q",
                remote.to_str().unwrap(),
                "HEAD:refs/heads/main",
            ],
        );
        let barrier = Barrier::new(FETCHERS);
        std::thread::scope(|scope| {
            for worktree in &worktrees {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    fetch(worktree, "main").unwrap();
                });
            }
        });
        assert_eq!(
            run(&first, &["rev-parse", "origin/main"]),
            run(&seed, &["rev-parse", "HEAD"])
        );
    }
}

#[test]
fn a_failed_fetch_releases_the_repository_lock() {
    let dir = tempfile::tempdir().unwrap();
    let remote = origin(dir.path());
    let worktree = host(dir.path(), &remote, "first", "docs/first.md", "first");
    assert!(fetch(&worktree, "missing-branch").is_err());
    fetch_lock(&worktree).unwrap().try_lock().unwrap();
    fetch(&worktree, "main").unwrap();
}

/// Many hosts land at once against one bare `origin`: every change lands,
/// none replaces another, and the history stays a line.
#[test]
fn many_hosts_landing_at_once_all_land_in_a_line() {
    const HOSTS: usize = 8;
    let dir = tempfile::tempdir().unwrap();
    let remote = origin(dir.path());
    let paths: Vec<String> = (0..HOSTS)
        .map(|i| match i % 4 {
            0 => format!("crates/a/src/f{i}.rs"),
            1 => format!("crates/b/src/f{i}.rs"),
            2 => format!("docs/note{i}.md"),
            _ => format!("crates/c/src/f{i}.rs"),
        })
        .collect();
    let hosts: Vec<PathBuf> = paths
        .iter()
        .enumerate()
        .map(|(i, path)| host(dir.path(), &remote, &format!("host{i}"), path, "// x\n"))
        .collect();
    let barrier = Arc::new(Barrier::new(HOSTS));
    let landers: Vec<_> = hosts
        .into_iter()
        .map(|worktree| {
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let mut hooks = Counting::default();
                barrier.wait();
                let landed = land(&plan(&worktree), &mut hooks);
                (landed, hooks.checks)
            })
        })
        .collect();
    let mut commits = Vec::new();
    let mut refused = 0;
    let mut checks = 0;
    for lander in landers {
        let (landed, ran) = lander.join().unwrap();
        let landed = landed.unwrap_or_else(|not| panic!("a host did not land: {not:?}"));
        refused += landed
            .attempts
            .iter()
            .filter(|a| a.refused.is_some())
            .count();
        checks += ran;
        eprintln!("{}", summary(&landed.attempts, "main"));
        commits.push(landed.commit);
    }
    eprintln!("{HOSTS} hosts landed with {refused} refused push(es) and {checks} re-check(s).");
    assert_eq!(run(&remote, &["rev-list", "--count", "main"]), "9");
    assert_eq!(run(&remote, &["rev-list", "--merges", "main"]), "");
    for commit in &commits {
        run(&remote, &["merge-base", "--is-ancestor", commit, "main"]);
    }
    let tree = run(&remote, &["ls-tree", "-r", "--name-only", "main"]);
    for path in &paths {
        assert!(tree.lines().any(|line| line == path), "{path} was lost");
    }
}

/// A host that loses the race to an unrelated package rebases and pushes
/// without re-running its checks; one whose dependency moved re-runs them.
#[test]
fn only_commits_that_can_affect_the_change_re_run_its_checks() {
    let dir = tempfile::tempdir().unwrap();
    let remote = origin(dir.path());
    let a = host(dir.path(), &remote, "a", "crates/a/src/x.rs", "// a\n");
    let b = host(dir.path(), &remote, "b", "crates/b/src/x.rs", "// b\n");
    land(&plan(&b), &mut Counting::default()).unwrap();
    let mut hooks = Counting::default();
    let landed = land(&plan(&a), &mut hooks).unwrap();
    assert_eq!(hooks.checks, 0, "{:?}", landed.attempts);
    assert_eq!(landed.attempts[0].moved, 1);
    assert!(matches!(landed.attempts[0].recheck, Recheck::Skipped(_)));

    let a2 = host(dir.path(), &remote, "a2", "crates/a/src/y.rs", "// a\n");
    let c = host(dir.path(), &remote, "c", "crates/c/src/y.rs", "// c\n");
    land(&plan(&c), &mut Counting::default()).unwrap();
    let mut hooks = Counting::default();
    let landed = land(&plan(&a2), &mut hooks).unwrap();
    assert_eq!(hooks.checks, 1);
    let Recheck::Ran { why, passed } = &landed.attempts[0].recheck else {
        panic!("{:?}", landed.attempts);
    };
    assert!(passed);
    assert!(why.contains("`c`"), "{why}");
}

#[test]
fn what_the_newly_landed_commits_can_affect() {
    let dir = tempfile::tempdir().unwrap();
    let remote = origin(dir.path());
    let base = run(&remote, &["rev-parse", "main"]);
    // Each case: what landed, what the change touches, whether to re-run.
    for (i, landed, changed, rerun) in [
        ("docs/other.md", "crates/a/src/z.rs", false),
        ("crates/b/src/z.rs", "crates/a/src/z.rs", false),
        ("crates/c/src/z.rs", "crates/a/src/z.rs", true),
        ("crates/a/src/z.rs", "crates/c/src/z.rs", true),
        ("crates/a/src/z.rs", "crates/a/src/w.rs", true),
        ("crates/b/src/z.rs", "docs/other.md", false),
        ("Cargo.toml", "crates/b/src/z.rs", true),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (landed, changed, rerun))| (i, landed, changed, rerun))
    {
        let name = format!("case{i}");
        let other = host(dir.path(), &remote, &format!("{name}-o"), landed, "# new\n");
        let worktree = host(dir.path(), &remote, &format!("{name}-w"), changed, "// w\n");
        run(&other, &["push", "-q", "origin", "HEAD:refs/heads/main"]);
        run(&worktree, &["fetch", "-q", "origin", "main"]);
        run(&worktree, &["rebase", "-q", "origin/main"]);
        let upstream = run(&worktree, &["rev-parse", "origin/main"]);
        let why = affects(&worktree, &base, &upstream);
        assert_eq!(why.is_some(), rerun, "{landed} vs {changed}: {why:?}");
        // Back to the seed for the next case.
        run(
            &other,
            &[
                "push",
                "-q",
                "-f",
                "origin",
                &format!("{base}:refs/heads/main"),
            ],
        );
    }
    // A deleted file can break a docs change's links.
    let other = dir.path().join("deleter");
    run(
        dir.path(),
        &[
            "clone",
            "-q",
            remote.to_str().unwrap(),
            other.to_str().unwrap(),
        ],
    );
    configure(&other);
    run(&other, &["rm", "-q", "docs/readme.md"]);
    run(&other, &["commit", "-q", "-m", "delete"]);
    let worktree = host(
        dir.path(),
        &remote,
        "linker",
        "docs/new.md",
        "[r](readme.md)\n",
    );
    run(&other, &["push", "-q", "origin", "HEAD:refs/heads/main"]);
    run(&worktree, &["fetch", "-q", "origin", "main"]);
    run(&worktree, &["rebase", "-q", "origin/main"]);
    let upstream = run(&worktree, &["rev-parse", "origin/main"]);
    assert!(affects(&worktree, &base, &upstream).is_some());
}

/// A conflict or red checks after the rebase push nothing and leave the
/// change committed in the worktree.
#[test]
fn a_conflict_or_red_checks_push_nothing_and_keep_the_change() {
    let dir = tempfile::tempdir().unwrap();
    let remote = origin(dir.path());
    let first = host(dir.path(), &remote, "first", "docs/readme.md", "# One\n");
    let second = host(dir.path(), &remote, "second", "docs/readme.md", "# Two\n");
    land(&plan(&first), &mut Counting::default()).unwrap();
    let tip = run(&remote, &["rev-parse", "main"]);
    let not = land(&plan(&second), &mut Counting::default()).unwrap_err();
    assert!(matches!(not.failure, Failure::Conflict(_)), "{not:?}");
    assert_eq!(run(&remote, &["rev-parse", "main"]), tip);
    assert_eq!(
        run(&second, &["log", "-1", "--format=%s"]),
        "second changes docs/readme.md"
    );

    let red = host(dir.path(), &remote, "red", "crates/c/src/r.rs", "// r\n");
    let other = host(dir.path(), &remote, "other", "crates/c/src/o.rs", "// o\n");
    land(&plan(&other), &mut Counting::default()).unwrap();
    let mut hooks = Counting {
        red: true,
        ..Counting::default()
    };
    let not = land(&plan(&red), &mut hooks).unwrap_err();
    assert_eq!(not.failure, Failure::Red(vec!["a test fails".into()]));
    assert_eq!(
        run(&red, &["log", "-1", "--format=%s"]),
        "red changes crates/c/src/r.rs"
    );
}

/// A remote that refuses every push while nothing lands is not a race: the
/// landing gives up after two refusals instead of retrying to the bound.
#[test]
fn a_remote_that_refuses_while_nothing_lands_is_given_up_on() {
    let dir = tempfile::tempdir().unwrap();
    let remote = origin(dir.path());
    let hook = remote.join("hooks").join("pre-receive");
    std::fs::write(&hook, "#!/bin/sh\necho refused by policy >&2\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let worktree = host(dir.path(), &remote, "host", "crates/b/src/q.rs", "// q\n");
    let not = land(&plan(&worktree), &mut Counting::default()).unwrap_err();
    let Failure::GaveUp(why) = &not.failure else {
        panic!("{not:?}");
    };
    assert!(why.contains("not a race"), "{why}");
    assert_eq!(not.attempts.len(), 2);
    assert!(summary(&not.attempts, "main").contains("2 refused push(es)"));
    assert_eq!(
        run(&worktree, &["log", "-1", "--format=%s"]),
        "host changes crates/b/src/q.rs"
    );
}

#[test]
fn the_backoff_grows_to_its_cap_with_jitter() {
    let backoff = Backoff {
        first: Duration::from_millis(100),
        cap: Duration::from_millis(1_000),
    };
    for _ in 0..50 {
        let first = backoff.delay(1);
        assert!(first >= Duration::from_millis(50) && first <= Duration::from_millis(100));
        let late = backoff.delay(30);
        assert!(late >= Duration::from_millis(500) && late <= Duration::from_millis(1_000));
    }
    let waits: std::collections::BTreeSet<Duration> = (0..20).map(|_| backoff.delay(3)).collect();
    assert!(waits.len() > 1, "the waits do not vary");
}

/// One machine's landing lock, as [`Hooks::enter`] takes it.
#[derive(Clone, Default)]
struct Gate(Arc<std::sync::atomic::AtomicBool>);

struct Held(Arc<std::sync::atomic::AtomicBool>);

impl Drop for Held {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Gate {
    fn take(&self) -> Box<dyn std::any::Any> {
        use std::sync::atomic::Ordering::SeqCst;
        while self
            .0
            .compare_exchange(false, true, SeqCst, SeqCst)
            .is_err()
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        Box::new(Held(Arc::clone(&self.0)))
    }

    fn held(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Hooks behind a landing lock, with something to do while the checks run.
struct Gated {
    gate: Gate,
    checks: usize,
    held_during_check: bool,
    notes: Vec<String>,
    during_check: Option<Box<dyn FnMut()>>,
}

impl Hooks for Gated {
    fn check(&mut self) -> Vec<String> {
        self.checks += 1;
        self.held_during_check |= self.gate.held();
        if let Some(mut during) = self.during_check.take() {
            during();
        }
        Vec::new()
    }
    fn note(&mut self, text: &str) {
        self.notes.push(text.to_owned());
    }
    fn stopping(&self) -> bool {
        false
    }
    fn enter(&mut self) -> Option<Box<dyn std::any::Any>> {
        Some(self.gate.take())
    }
}

/// #10391: the landing lock covers fetch → rebase → push, never the checks.
/// While one run re-checks its rebased change, another run on the same
/// machine lands; the first then sees the branch moved again, rebases once
/// more (re-checking only if the new commit can affect it) and lands.
#[test]
fn the_checks_run_outside_the_landing_lock_so_others_land_meanwhile() {
    let dir = tempfile::tempdir().unwrap();
    let remote = origin(dir.path());
    let a = host(dir.path(), &remote, "a", "crates/a/src/x.rs", "// a\n");
    let c = host(dir.path(), &remote, "c", "crates/c/src/x.rs", "// c\n");
    let b = host(dir.path(), &remote, "b", "crates/b/src/x.rs", "// b\n");
    // `c` lands first, so `a` (which depends on `c`) must re-run its checks.
    land(&plan(&c), &mut Counting::default()).unwrap();
    let gate = Gate::default();
    let other = gate.clone();
    let landed_meanwhile = Arc::new(std::sync::Mutex::new(None));
    let record = Arc::clone(&landed_meanwhile);
    let mut hooks = Gated {
        gate: gate.clone(),
        checks: 0,
        held_during_check: false,
        notes: Vec::new(),
        during_check: Some(Box::new(move || {
            // Another run on this machine lands while `a`'s checks run; it
            // would block forever if `a` still held the lock.
            let mut theirs = Gated {
                gate: other.clone(),
                checks: 0,
                held_during_check: false,
                notes: Vec::new(),
                during_check: None,
            };
            let landed = land(&plan(&b), &mut theirs).unwrap();
            *record.lock().unwrap() = Some(landed.commit);
        })),
    };
    let landed = land(&plan(&a), &mut hooks).unwrap();
    assert!(!hooks.held_during_check, "the checks ran under the lock");
    assert!(!gate.held(), "the lock was left held");
    assert_eq!(hooks.checks, 1, "{:?}", landed.attempts);
    assert!(
        hooks.notes.iter().any(|note| note.contains("moved again")),
        "{:?}",
        hooks.notes
    );
    let theirs = landed_meanwhile.lock().unwrap().clone().unwrap();
    run(&remote, &["merge-base", "--is-ancestor", &theirs, "main"]);
    run(
        &remote,
        &["merge-base", "--is-ancestor", &landed.commit, "main"],
    );
    assert_eq!(run(&remote, &["rev-list", "--count", "main"]), "4");
}
