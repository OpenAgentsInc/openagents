use super::*;
use crate::task::issue_run::Checked;
use std::time::Duration;

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

/// A bare origin with `main` holding two files.
fn origin(root: &Path) -> PathBuf {
    let remote = root.join("origin.git");
    std::fs::create_dir_all(&remote).unwrap();
    run(&remote, &["init", "-q", "--bare", "-b", "main"]);
    let seed = root.join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    run(&seed, &["init", "-q", "-b", "main"]);
    configure(&seed);
    write(&seed, "notes/a.md", "one\ntwo\nthree\n");
    write(&seed, "notes/b.md", "alpha\n");
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

/// A machine commits `path` and pushes the change to `land/<id>` and
/// queues it, as `openagents land submit` does.
fn submit(queue: &Queue<'_>, dir: &Path, machine: &str, at: u64, path: &str, text: &str) -> Entry {
    run(dir, &["fetch", "-q", "origin"]);
    run(dir, &["checkout", "-q", "--detach", "origin/main"]);
    write(dir, path, text);
    run(dir, &["add", "-A"]);
    run(
        dir,
        &["commit", "-q", "-m", &format!("{machine} changes {path}")],
    );
    let id = new_id(at, machine);
    let branch = format!("land/{id}");
    run(
        dir,
        &["push", "-q", "origin", &format!("HEAD:refs/heads/{branch}")],
    );
    let entry = Entry {
        id,
        branch,
        target: "main".into(),
        issue: Some(7),
        close: true,
        author: "Lander".into(),
        machine: machine.into(),
        summary: format!("change {path}"),
        head: run(dir, &["rev-parse", "HEAD"]),
        submitted_at: at,
        state: State::Queued,
        updated_at: at,
        tries: 0,
        commit: None,
        reason: None,
        worker: None,
        ..Entry::default()
    };
    queue.submit(&entry).unwrap();
    entry
}

/// Checks that pass unless the change holds the word `RED`.
struct Fake;

impl Checks for Fake {
    fn check(&self, worktree: &Path, _policy: &Policy) -> Checked {
        let diff = run(worktree, &["diff", "--cached"]);
        Checked {
            problems: if diff.contains("RED") {
                vec!["a test fails".into()]
            } else {
                Vec::new()
            },
            ran: vec![format!("fake checks on {} line(s)", diff.lines().count())],
        }
    }
}

#[derive(Default)]
struct Seen {
    landed: Vec<(String, String)>,
    bounced: Vec<(String, String)>,
    /// What the repair writes into each conflicted file; `None` fails it.
    resolve: Option<String>,
    repairs: usize,
}

impl Effects for Seen {
    fn landed(&mut self, _top: &Path, entry: &Entry, text: &str) -> Result<(), String> {
        self.landed.push((entry.id.clone(), text.to_owned()));
        Ok(())
    }
    fn bounced(&mut self, _top: &Path, entry: &Entry, text: &str) -> Result<(), String> {
        self.bounced.push((entry.id.clone(), text.to_owned()));
        Ok(())
    }
    fn repair(&mut self, worktree: &Path, _request: &str) -> Result<(), String> {
        self.repairs += 1;
        let Some(text) = self.resolve.clone() else {
            return Err("no repair here".into());
        };
        for path in run(worktree, &["diff", "--name-only", "--diff-filter=U"]).lines() {
            write(worktree, path, &text);
        }
        Ok(())
    }
}

fn integrator<'a>(
    store: &'a dyn Store,
    top: &Path,
    root: &Path,
    effects: &'a mut Seen,
) -> Integrator<'a> {
    Integrator {
        queue: Queue { store },
        top: top.to_path_buf(),
        worktree: root.join("work"),
        machine: "env".into(),
        checks: &Fake,
        effects,
        attempts: 5,
        backoff: landing::Backoff {
            first: Duration::from_millis(5),
            cap: Duration::from_millis(20),
        },
        lane: Lane::Code,
        slot: "code-1".into(),
        also: Vec::new(),
        push: None,
        regenerate: false,
    }
}

#[test]
fn two_machines_land_in_order_and_every_try_is_recorded() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let remote = origin(root);
    let store = Dir(root.join("queue"));
    let queue = Queue { store: &store };
    let mac = clone(root, &remote, "mac");
    let host = clone(root, &remote, "host");
    let first = submit(&queue, &mac, "mac", 100, "notes/mac.md", "from the mac\n");
    let second = submit(
        &queue,
        &host,
        "coder-host",
        200,
        "notes/host.md",
        "from a host\n",
    );
    let top = clone(root, &remote, "env");
    let mut seen = Seen::default();
    let mut worker = integrator(&store, &top, root, &mut seen);
    let (one, outcome) = worker.step().unwrap().unwrap();
    assert_eq!(one.id, first.id);
    assert!(matches!(outcome, Outcome::Landed(_)), "{outcome:?}");
    let (two, outcome) = worker.step().unwrap().unwrap();
    assert_eq!(two.id, second.id);
    assert!(matches!(outcome, Outcome::Landed(_)), "{outcome:?}");
    assert!(worker.step().unwrap().is_none());

    // Main holds both, the mac's first, and the queue branches are gone.
    run(&top, &["fetch", "-q", "--prune", "origin"]);
    let log = run(&top, &["log", "--format=%s", "origin/main"]);
    assert_eq!(
        log.lines().collect::<Vec<_>>(),
        [
            "coder-host changes notes/host.md",
            "mac changes notes/mac.md",
            "seed"
        ]
    );
    assert!(
        run(&top, &["branch", "-r"])
            .lines()
            .all(|b| !b.contains("land/"))
    );
    let entries = queue.entries().unwrap();
    assert!(
        entries
            .iter()
            .all(|e| e.state == State::Landed && e.commit.is_some())
    );
    let records = queue.records(&second.id).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].outcome, "landed");
    assert!(!records[0].checks.is_empty());
    assert_eq!(seen.landed.len(), 2);
    assert!(seen.landed[0].1.contains("Landed on `main`"));
}

#[test]
fn a_conflict_gets_one_repair_turn_then_lands() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let remote = origin(root);
    let store = Dir(root.join("queue"));
    let queue = Queue { store: &store };
    let mac = clone(root, &remote, "mac");
    let host = clone(root, &remote, "host");
    // Both branch from the same main and change the same line.
    run(&host, &["fetch", "-q"]);
    submit(
        &queue,
        &mac,
        "mac",
        100,
        "notes/a.md",
        "one\nTWO from mac\nthree\n",
    );
    let second = {
        write(&host, "notes/a.md", "one\nTWO from host\nthree\n");
        run(&host, &["add", "-A"]);
        run(&host, &["commit", "-q", "-m", "host changes notes/a.md"]);
        let id = new_id(200, "host");
        let branch = format!("land/{id}");
        run(
            &host,
            &["push", "-q", "origin", &format!("HEAD:refs/heads/{branch}")],
        );
        let entry = Entry {
            id,
            branch,
            target: "main".into(),
            issue: None,
            close: false,
            author: "Lander".into(),
            machine: "host".into(),
            summary: "conflicting".into(),
            head: run(&host, &["rev-parse", "HEAD"]),
            submitted_at: 200,
            state: State::Queued,
            updated_at: 200,
            tries: 0,
            commit: None,
            reason: None,
            worker: None,
            ..Entry::default()
        };
        queue.submit(&entry).unwrap();
        entry
    };
    let top = clone(root, &remote, "env");
    let mut seen = Seen {
        resolve: Some("one\nTWO from mac and host\nthree\n".into()),
        ..Seen::default()
    };
    let mut worker = integrator(&store, &top, root, &mut seen);
    assert!(matches!(
        worker.step().unwrap().unwrap().1,
        Outcome::Landed(_)
    ));
    let (entry, outcome) = worker.step().unwrap().unwrap();
    assert_eq!(entry.id, second.id);
    assert!(matches!(outcome, Outcome::Landed(_)), "{outcome:?}");
    let records = queue.records(&second.id).unwrap();
    assert!(records[0].repaired);
    assert!(
        !records[0].checks.is_empty(),
        "the checks ran after the repair"
    );
    assert_eq!(seen.repairs, 1);
    run(&top, &["fetch", "-q", "origin"]);
    assert_eq!(
        run(&top, &["show", "origin/main:notes/a.md"]),
        "one\nTWO from mac and host\nthree"
    );
}

#[test]
fn an_unrepaired_conflict_or_red_checks_bounce_to_the_author() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let remote = origin(root);
    let store = Dir(root.join("queue"));
    let queue = Queue { store: &store };
    let mac = clone(root, &remote, "mac");
    let host = clone(root, &remote, "host");
    submit(&queue, &mac, "mac", 100, "notes/b.md", "beta\n");
    // Built on the old main: conflicts with the mac's change.
    write(&host, "notes/b.md", "gamma\n");
    run(&host, &["add", "-A"]);
    run(&host, &["commit", "-q", "-m", "host changes notes/b.md"]);
    let id = new_id(200, "host");
    run(
        &host,
        &[
            "push",
            "-q",
            "origin",
            &format!("HEAD:refs/heads/land/{id}"),
        ],
    );
    let mut conflicting = Entry {
        id: id.clone(),
        branch: format!("land/{id}"),
        target: "main".into(),
        issue: Some(9),
        close: true,
        author: "Lander".into(),
        machine: "host".into(),
        summary: "conflicting".into(),
        head: String::new(),
        submitted_at: 200,
        state: State::Queued,
        updated_at: 200,
        tries: 0,
        commit: None,
        reason: None,
        worker: None,
        ..Entry::default()
    };
    queue.submit(&conflicting).unwrap();
    let red = submit(&queue, &mac, "mac", 300, "notes/c.md", "RED\n");
    let top = clone(root, &remote, "env");
    let mut seen = Seen::default();
    let mut worker = integrator(&store, &top, root, &mut seen);
    assert!(matches!(
        worker.step().unwrap().unwrap().1,
        Outcome::Landed(_)
    ));
    let (entry, outcome) = worker.step().unwrap().unwrap();
    assert_eq!(entry.id, id);
    assert!(matches!(outcome, Outcome::Bounced(_)), "{outcome:?}");
    let (entry, outcome) = worker.step().unwrap().unwrap();
    assert_eq!(entry.id, red.id);
    match outcome {
        Outcome::Bounced(why) => assert!(why.contains("a test fails"), "{why}"),
        other => panic!("{other:?}"),
    }
    assert!(worker.step().unwrap().is_none());
    assert_eq!(seen.bounced.len(), 2);
    assert!(seen.bounced[0].1.contains("bounced"));
    conflicting = queue.entry(&id).unwrap().unwrap();
    assert_eq!(conflicting.state, State::Bounced);
    assert!(conflicting.reason.is_some());
    // A bounced branch stays on origin for its author.
    run(&top, &["fetch", "-q", "origin"]);
    assert!(run(&top, &["branch", "-r"]).contains(&format!("land/{id}")));
}

#[test]
fn the_queue_is_oldest_first_and_resumes_its_own_landing() {
    let temp = tempfile::tempdir().unwrap();
    let store = Dir(temp.path().join("q"));
    let queue = Queue { store: &store };
    let mut a = Entry {
        id: new_id(200, "b"),
        branch: "x".into(),
        target: "main".into(),
        issue: None,
        close: false,
        author: String::new(),
        machine: "b".into(),
        summary: String::new(),
        head: String::new(),
        submitted_at: 200,
        state: State::Queued,
        updated_at: 200,
        tries: 0,
        commit: None,
        reason: None,
        worker: None,
        ..Entry::default()
    };
    let mut b = a.clone();
    b.id = new_id(100, "a");
    queue.submit(&a).unwrap();
    queue.submit(&b).unwrap();
    assert!(queue.submit(&b).is_err(), "an id is queued once");
    assert_eq!(queue.next("env").unwrap().unwrap().id, b.id);
    b.state = State::Landing;
    b.worker = Some("other".into());
    queue.put(&b).unwrap();
    assert_eq!(queue.next("env").unwrap().unwrap().id, a.id);
    assert_eq!(queue.next("other").unwrap().unwrap().id, b.id);
    a.state = State::Landed;
    queue.put(&a).unwrap();
    assert!(queue.next("env").unwrap().is_none());
}

#[test]
fn ids_sort_by_time_and_name_the_machine() {
    assert_eq!(stamp(0), "19700101T000000Z");
    assert_eq!(stamp(951_782_400), "20000229T000000Z");
    assert_eq!(stamp(1_791_644_462), "20261010T150102Z");
    let id = new_id(1_791_644_462, "oa-dev-env-1.c.Project");
    assert!(
        id.starts_with("20261010T150102Z-oa-dev-env-1-c-project-"),
        "{id}"
    );
    assert!(new_id(1, "z") < new_id(2, "a"));
}

#[test]
fn a_queue_url_is_a_bucket_or_a_folder() {
    assert_eq!(
        open("gs://b/land-queue/x/").unwrap().location(),
        "gs://b/land-queue/x"
    );
    assert_eq!(open("/tmp/q").unwrap().location(), "/tmp/q");
    assert_eq!(open("file:///tmp/q").unwrap().location(), "/tmp/q");
    assert!(open("gs://").is_err());
}
