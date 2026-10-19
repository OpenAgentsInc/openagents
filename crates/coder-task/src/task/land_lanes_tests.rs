//! A queue simulation: a bare Git origin, fake checks that take a while on
//! code and record when they ran, and the lanes over a folder queue.

use super::*;
use crate::task::issue_run::{Checked, Policy};
use crate::task::land_plan::{Facts, Graph, classify};
use crate::task::land_queue::{Dir, new_id};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn run(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
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

fn origin(root: &Path) -> PathBuf {
    let remote = root.join("origin.git");
    std::fs::create_dir_all(&remote).unwrap();
    run(&remote, &["init", "-q", "--bare", "-b", "main"]);
    let seed = root.join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    run(&seed, &["init", "-q", "-b", "main"]);
    configure(&seed);
    write(&seed, "crates/a/src/lib.rs", "// a\n");
    write(&seed, "crates/b/src/lib.rs", "// b\n");
    write(&seed, "docs/readme.md", "Read me.\n");
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

fn clone(root: &Path, remote: &Path, name: &str) -> PathBuf {
    let dir = root.join(name);
    let out = Command::new("git")
        .args([
            "clone",
            "-q",
            remote.to_str().unwrap(),
            dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    configure(&dir);
    dir
}

/// Commits `path` on top of `main` and queues it, as `land submit` does.
fn submit(queue: &Queue<'_>, dir: &Path, at: u64, path: &str) -> Entry {
    run(dir, &["fetch", "-q", "origin"]);
    run(dir, &["checkout", "-q", "--detach", "origin/main"]);
    write(dir, path, &format!("changed at {at}\n"));
    run(dir, &["add", "-A"]);
    run(dir, &["commit", "-q", "-m", &format!("change {path}")]);
    let id = new_id(at, "mac");
    let branch = format!("land/{id}");
    run(
        dir,
        &["push", "-q", "origin", &format!("HEAD:refs/heads/{branch}")],
    );
    let entry = Entry {
        id,
        branch,
        target: "main".into(),
        author: "Lander".into(),
        machine: "mac".into(),
        summary: format!("change {path}"),
        head: run(dir, &["rev-parse", "HEAD"]),
        submitted_at: at,
        updated_at: at,
        ..Entry::default()
    };
    queue.submit(&entry).unwrap();
    entry
}

/// Plans from the branch's changed paths: `crates/<x>/` is package `x`;
/// `a` and `b` do not depend on each other.
struct FakePlanner {
    top: PathBuf,
}

impl Planner for FakePlanner {
    fn plan(&self, entry: &Entry) -> Plan {
        let _lock = landing::fetch_lock(&self.top).unwrap();
        run(
            &self.top,
            &[
                "fetch",
                "-q",
                "origin",
                &format!("+refs/heads/{0}:refs/remotes/origin/{0}", entry.branch),
                "+refs/heads/main:refs/remotes/origin/main",
            ],
        );
        let changed = run(
            &self.top,
            &[
                "diff",
                "--name-only",
                &format!("origin/main...origin/{}", entry.branch),
            ],
        );
        let files: Vec<String> = changed.lines().map(str::to_owned).collect();
        let package_of = |path: &str| {
            path.strip_prefix("crates/")
                .and_then(|rest| rest.split_once('/'))
                .map(|(name, _)| name.to_owned())
        };
        let graph: Graph = vec![("a".into(), vec![]), ("b".into(), vec![])];
        classify(
            &files,
            &Facts {
                package_of: &package_of,
                literals: &[],
                graph: Some(&graph),
            },
        )
    }
}

/// When each code check ran, by the first changed file.
#[derive(Default)]
struct Timeline {
    checks: Vec<(String, Instant, Instant)>,
    landed: Vec<(String, Instant)>,
}

/// Code checks take a while and count how many run at once.
struct SlowChecks {
    log: Arc<Mutex<Timeline>>,
    now: AtomicUsize,
    most: AtomicUsize,
}

impl Checks for SlowChecks {
    fn check(&self, worktree: &Path, _policy: &Policy) -> Checked {
        let file = run(worktree, &["diff", "--cached", "--name-only"])
            .lines()
            .next()
            .unwrap_or("")
            .to_owned();
        let at = self.now.fetch_add(1, Ordering::SeqCst) + 1;
        self.most.fetch_max(at, Ordering::SeqCst);
        let start = Instant::now();
        std::thread::sleep(Duration::from_millis(1500));
        self.now.fetch_sub(1, Ordering::SeqCst);
        self.log
            .lock()
            .unwrap()
            .checks
            .push((file.clone(), start, Instant::now()));
        Checked {
            problems: Vec::new(),
            ran: vec![format!("slow checks on {file}")],
        }
    }
}

struct Shared(Arc<Mutex<Timeline>>);

impl Effects for Shared {
    fn landed(&mut self, _top: &Path, entry: &Entry, _text: &str) -> Result<(), String> {
        self.0
            .lock()
            .unwrap()
            .landed
            .push((entry.summary.clone(), Instant::now()));
        Ok(())
    }
    fn bounced(&mut self, _top: &Path, entry: &Entry, text: &str) -> Result<(), String> {
        panic!("{} bounced: {text}", entry.id);
    }
    fn repair(&mut self, _worktree: &Path, _request: &str) -> Result<(), String> {
        Err("no repair here".into())
    }
}

#[test]
fn documents_land_at_once_and_unrelated_code_checks_side_by_side() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let remote = origin(root);
    let store = Dir(root.join("queue"));
    let queue = Queue { store: &store };
    let mac = clone(root, &remote, "mac");
    // In submission order: two unrelated code entries, a third that
    // overlaps the first (same package), and a document last.
    let a = submit(&queue, &mac, 100, "crates/a/src/one.rs");
    let b = submit(&queue, &mac, 200, "crates/b/src/one.rs");
    let a2 = submit(&queue, &mac, 300, "crates/a/src/two.rs");
    let docs = submit(&queue, &mac, 400, "docs/guide.md");
    let top = clone(root, &remote, "env");
    let planner_top = clone(root, &remote, "planner");
    let log = Arc::new(Mutex::new(Timeline::default()));
    let checks = SlowChecks {
        log: log.clone(),
        now: AtomicUsize::new(0),
        most: AtomicUsize::new(0),
    };
    let effects_log = log.clone();
    let effects = move || -> Box<dyn Effects> { Box::new(Shared(effects_log.clone())) };
    let planner = FakePlanner { top: planner_top };
    let lanes = Lanes {
        store: &store,
        top: top.clone(),
        root: root.join("lanes"),
        machine: "env".into(),
        code_slots: 2,
        checks: &checks,
        planner: &planner,
        effects: &effects,
        attempts: 6,
        backoff: landing::Backoff {
            first: Duration::from_millis(5),
            cap: Duration::from_millis(20),
        },
        instance: None,
        regenerate: false,
        every: Duration::from_millis(50),
    };
    let started = Instant::now();
    let mut outcomes = Vec::new();
    let mut done = |entry: &Entry, outcome: &Outcome| {
        outcomes.push((entry.clone(), outcome.clone()));
    };
    let draining = || false;
    let busy = |_: bool| {};
    let ready = || None;
    lanes
        .run(&mut Control {
            once: true,
            draining: &draining,
            busy: &busy,
            done: &mut done,
            ready: &ready,
        })
        .unwrap();
    eprintln!("the simulation took {:?}", started.elapsed());

    // Everything landed, each in its lane.
    assert_eq!(outcomes.len(), 4, "{outcomes:?}");
    for (entry, outcome) in &outcomes {
        assert!(matches!(outcome, Outcome::Landed(_)), "{outcome:?}");
        let lane = if entry.id == docs.id {
            Lane::Fast
        } else {
            Lane::Code
        };
        assert_eq!(entry.lane, Some(lane), "{}", entry.id);
    }
    let log = log.lock().unwrap();
    let landed = |summary: &str| {
        log.landed
            .iter()
            .find(|(s, _)| s == summary)
            .map(|(_, at)| *at)
            .unwrap_or_else(|| panic!("{summary} did not land"))
    };
    let checked = |file: &str| {
        log.checks
            .iter()
            .find(|(f, _, _)| f == file)
            .map(|(_, start, end)| (*start, *end))
            .unwrap_or_else(|| panic!("{file} was not checked"))
    };
    // The document, submitted last, landed while code was still checking.
    let docs_at = landed(&docs.summary);
    assert!(docs_at < landed(&a.summary) && docs_at < landed(&b.summary));
    assert!(
        log.checks.iter().all(|(f, _, _)| !f.starts_with("docs/")),
        "the document skipped the build"
    );
    // a and b checked at the same time; a2 waited for a to land.
    assert_eq!(checks.most.load(Ordering::SeqCst), 2);
    let (a_start, a_end) = checked("crates/a/src/one.rs");
    let (b_start, b_end) = checked("crates/b/src/one.rs");
    assert!(
        a_start < b_end && b_start < a_end,
        "a and b overlapped in time"
    );
    let (a2_start, _) = checked("crates/a/src/two.rs");
    assert!(a2_start >= landed(&a.summary), "a2 waited for a");

    // main holds all four; every try is recorded with its lane and slot.
    run(&top, &["fetch", "-q", "origin"]);
    let log_lines = run(&top, &["log", "--format=%s", "origin/main"]);
    for entry in [&a, &b, &a2, &docs] {
        assert!(log_lines.contains(&entry.summary), "{log_lines}");
        let records = queue.records(&entry.id).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].outcome, "landed");
        assert!(!records[0].slot.is_empty());
    }
    let fast = queue.records(&docs.id).unwrap();
    assert_eq!(
        (fast[0].lane.as_str(), fast[0].slot.as_str()),
        ("fast", "fast")
    );
    let a2_entry = queue.entry(&a2.id).unwrap().unwrap();
    assert_eq!(a2_entry.state, State::Landed);
}

#[test]
fn a_worker_that_cannot_build_takes_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let remote = origin(root);
    let store = Dir(root.join("queue"));
    let queue = Queue { store: &store };
    let mac = clone(root, &remote, "mac");
    let entry = submit(&queue, &mac, 100, "crates/a/src/one.rs");
    let top = clone(root, &remote, "env");
    let checks = SlowChecks {
        log: Arc::default(),
        now: AtomicUsize::new(0),
        most: AtomicUsize::new(0),
    };
    let effects = || -> Box<dyn Effects> { Box::new(Shared(Arc::default())) };
    let planner = FakePlanner {
        top: clone(root, &remote, "planner"),
    };
    let lanes = Lanes {
        store: &store,
        top,
        root: root.join("lanes"),
        machine: "env".into(),
        code_slots: 2,
        checks: &checks,
        planner: &planner,
        effects: &effects,
        attempts: 3,
        backoff: landing::Backoff::LANDING,
        instance: None,
        regenerate: false,
        every: Duration::from_millis(20),
    };
    let mut ran = Vec::new();
    let mut done = |entry: &Entry, _: &Outcome| ran.push(entry.id.clone());
    lanes
        .run(&mut Control {
            once: true,
            draining: &|| false,
            busy: &|_| {},
            done: &mut done,
            ready: &|| Some("cargo is not on PATH".into()),
        })
        .unwrap();
    assert!(ran.is_empty(), "nothing should run: {ran:?}");
    let left = queue.entry(&entry.id).unwrap().unwrap();
    assert_eq!(left.state, State::Queued);
    assert_eq!(left.tries, 0);
    assert!(
        queue.worker().unwrap().is_some(),
        "the heartbeat still beats"
    );
}
