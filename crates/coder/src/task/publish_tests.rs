//! Review and publication of a local run's change on scratch repositories
//! with a local bare remote. No test reaches a real forge.

use super::*;
use coder_host::access::review::{Completeness, FileStatus};
use std::sync::atomic::{AtomicUsize, Ordering};

fn run(dir: &Path, args: &[&str]) -> String {
    let output = local::git().arg("-C").arg(dir).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A checkout whose `origin` is a bare repository, a task store, and a
/// local run's record whose worktree is a detached worktree of `main`.
struct Scratch {
    _root: tempfile::TempDir,
    remote: PathBuf,
    checkout: PathBuf,
    store: PathBuf,
    worktree: PathBuf,
    task: String,
    base: String,
}

impl Scratch {
    fn new(policy: Option<&str>) -> Self {
        let root = tempfile::tempdir().unwrap();
        let top = root.path().canonicalize().unwrap();
        let remote = top.join("remote.git");
        let checkout = top.join("checkout");
        std::fs::create_dir_all(&checkout).unwrap();
        run(
            &top,
            &[
                "init",
                "-q",
                "--bare",
                "-b",
                "main",
                remote.to_str().unwrap(),
            ],
        );
        run(&checkout, &["init", "-q", "-b", "main"]);
        for (key, value) in [
            ("user.email", "t@t"),
            ("user.name", "t"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "/dev/null"),
        ] {
            run(&checkout, &["config", key, value]);
        }
        run(
            &checkout,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        std::fs::write(checkout.join("a.txt"), "one\n").unwrap();
        std::fs::write(checkout.join("b.txt"), "keep\n").unwrap();
        if let Some(policy) = policy {
            std::fs::create_dir_all(checkout.join(".openagents")).unwrap();
            std::fs::write(checkout.join(".openagents/coder-issues.json"), policy).unwrap();
        }
        run(&checkout, &["add", "-A"]);
        run(&checkout, &["commit", "-q", "-m", "start"]);
        run(&checkout, &["push", "-q", "origin", "main"]);
        run(&checkout, &["fetch", "-q", "origin"]);
        run(
            &checkout,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );
        let base = run(&checkout, &["rev-parse", "HEAD"]);
        let worktree = top.join("worktree");
        run(
            &checkout,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                worktree.to_str().unwrap(),
                "HEAD",
            ],
        );
        let store = top.join("store");
        let task = "7".repeat(64);
        let record = local::Record {
            schema: local::RECORD_SCHEMA.into(),
            task: task.clone(),
            thread: None,
            project: "checkout".into(),
            checkout: checkout.display().to_string(),
            worktree: worktree.display().to_string(),
            base: base.clone(),
            turns: Vec::new(),
            ends: Default::default(),
        };
        std::fs::create_dir_all(store.join("local")).unwrap();
        std::fs::write(
            store.join("local").join(format!("{task}.json")),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        Self {
            _root: root,
            remote,
            checkout,
            store,
            worktree,
            task,
            base,
        }
    }

    /// Two changed files: one edited, one new.
    fn change(&self) {
        std::fs::write(self.worktree.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        std::fs::write(
            self.worktree.join("new.rs"),
            "fn answer() -> u8 {\n    42\n}\n",
        )
        .unwrap();
    }

    fn review(&self) -> coder_host::access::review::TaskReview {
        review::read(&self.task, &self.worktree, &self.base, 1024 * 1024).unwrap()
    }

    fn remote_tip(&self, branch: &str) -> Option<String> {
        let out = run(
            &self.remote,
            &[
                "for-each-ref",
                "--format=%(objectname)",
                &format!("refs/heads/{branch}"),
            ],
        );
        Some(out).filter(|out| !out.is_empty())
    }

    /// Count each push the remote receives in `received.log`; with
    /// `delay`, the hook then waits that many seconds, after the ref moved.
    fn count_receives(&self, delay: u32) -> PathBuf {
        let log = self.remote.join("received.log");
        let hook = self.remote.join("hooks/post-receive");
        std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
        std::fs::write(
            &hook,
            format!(
                "#!/bin/sh\ncat >> '{}'\nexec >/dev/null 2>&1\nsleep {delay}\n",
                log.display()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        log
    }
}

fn receives(log: &Path) -> usize {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.is_empty())
        .count()
}

fn reviewed(review: &coder_host::access::review::TaskReview) -> Reviewed {
    Reviewed {
        base: review.base.clone(),
        head_commit: review.head_commit.clone(),
        head: review.head.clone(),
    }
}

/// A forge that records what it was asked, never a real one.
#[derive(Default)]
struct FakeForge {
    opened: AtomicUsize,
    open: Mutex<Option<String>>,
    refuse: bool,
}

impl Forge for FakeForge {
    fn repository(&self, _dir: &Path) -> Result<String, String> {
        Ok("example/scratch".into())
    }
    fn find(&self, _dir: &Path, _repo: &str, _branch: &str) -> Result<Option<String>, String> {
        Ok(self.open.lock().unwrap().clone())
    }
    fn open_draft(
        &self,
        _dir: &Path,
        _repo: &str,
        branch: &str,
        base: &str,
        _title: &str,
        _body: &str,
    ) -> Result<String, String> {
        if self.refuse {
            return Err("the forge is down".into());
        }
        assert_eq!(base, "main");
        assert!(branch.starts_with("coder/review-"));
        self.opened.fetch_add(1, Ordering::SeqCst);
        let url = "https://github.com/example/scratch/pull/7".to_owned();
        *self.open.lock().unwrap() = Some(url.clone());
        Ok(url)
    }
}

#[test]
fn a_review_names_exact_revisions_and_counts_on_a_scratch_repository() {
    let scratch = Scratch::new(None);
    let review = scratch.review();
    assert_eq!(review.base, scratch.base);
    assert_eq!(review.head_commit, scratch.base);
    assert_eq!(review.files_total, 0, "nothing changed yet");
    scratch.change();
    let review = scratch.review();
    assert_eq!(review.base, scratch.base);
    assert_eq!(review.head_commit, scratch.base);
    // The head is the tree Git would commit for the worktree as it is.
    let mut expected = std::process::Command::new("git");
    let index = scratch.store.join("expected.index");
    expected
        .arg("-C")
        .arg(&scratch.worktree)
        .env("GIT_INDEX_FILE", &index);
    run(&scratch.worktree, &["status", "--short"]);
    assert!(
        expected
            .args(["read-tree", "HEAD"])
            .status()
            .unwrap()
            .success()
    );
    let mut add = std::process::Command::new("git");
    add.arg("-C")
        .arg(&scratch.worktree)
        .env("GIT_INDEX_FILE", &index);
    assert!(add.args(["add", "-A"]).status().unwrap().success());
    let tree = std::process::Command::new("git")
        .arg("-C")
        .arg(&scratch.worktree)
        .env("GIT_INDEX_FILE", &index)
        .arg("write-tree")
        .output()
        .unwrap();
    assert_eq!(review.head, String::from_utf8_lossy(&tree.stdout).trim());
    assert_eq!(review.files_total, 2);
    assert_eq!((review.added, review.removed, review.uncounted), (5, 0, 0));
    assert_eq!(review.files[0].path, "a.txt");
    assert_eq!(review.files[0].status, FileStatus::Modified);
    assert_eq!(
        (review.files[0].added, review.files[0].removed),
        (Some(2), Some(0))
    );
    assert_eq!(review.files[1].path, "new.rs");
    assert_eq!(review.files[1].status, FileStatus::Added);
    assert_eq!(review.completeness, Completeness::Complete);
    assert!(review.diff.contains("diff --git a/new.rs b/new.rs\n"));
    // Reading changed nothing the person keeps: no staged file, no new ref.
    assert_eq!(
        run(&scratch.worktree, &["diff", "--cached", "--name-only"]),
        ""
    );
    assert_eq!(run(&scratch.worktree, &["rev-parse", "HEAD"]), scratch.base);
    // A binary file counts as unknown lines, never zero.
    std::fs::write(scratch.worktree.join("logo.bin"), [0u8, 1, 2, 0, 255]).unwrap();
    let review = scratch.review();
    assert_eq!(review.files_total, 3);
    assert_eq!(review.uncounted, 1);
    let binary = review.files.iter().find(|f| f.path == "logo.bin").unwrap();
    assert_eq!((binary.added, binary.removed), (None, None));
}

#[test]
fn a_cut_diff_says_so_and_keeps_whole_counts() {
    let scratch = Scratch::new(None);
    scratch.change();
    let whole = scratch.review();
    let cut = review::read(&scratch.task, &scratch.worktree, &scratch.base, 60).unwrap();
    assert!(cut.diff.len() <= 60);
    assert!(cut.diff.is_empty() || cut.diff.ends_with('\n'));
    assert_eq!(
        cut.completeness,
        Completeness::Truncated {
            shown: cut.diff.len() as u64,
            total: Some(whole.diff.len() as u64)
        }
    );
    assert_eq!(
        (cut.files_total, cut.added),
        (whole.files_total, whole.added)
    );
    // Fitting for the wire keeps the counts and marks the cut.
    let mut big = whole.clone();
    big.diff = "+x\n".repeat(40_000);
    big.completeness = Completeness::Complete;
    review::fit(&mut big);
    big.validate().unwrap();
    assert!(matches!(
        big.completeness,
        Completeness::Truncated {
            total: Some(120_000),
            ..
        }
    ));
    // A base Git cannot find is an error, not an empty change.
    assert!(review::read(&scratch.task, &scratch.worktree, &"0".repeat(40), 1024).is_err());
}

#[test]
fn a_moved_worktree_makes_the_view_stale() {
    let scratch = Scratch::new(None);
    scratch.change();
    let first = scratch.review();
    assert_eq!(review::head(&scratch.worktree).unwrap().tree, first.head);
    std::fs::write(scratch.worktree.join("b.txt"), "moved\n").unwrap();
    let second = scratch.review();
    assert!(!first.same_head(&second), "a new edit is a new head");
    assert_ne!(first.head, second.head);
    assert_eq!(first.head_commit, second.head_commit);
    // A commit in the worktree moves the head commit too.
    run(&scratch.worktree, &["add", "-A"]);
    run(&scratch.worktree, &["commit", "-q", "-m", "local"]);
    let third = scratch.review();
    assert_ne!(third.head_commit, second.head_commit);
    assert_eq!(third.head, second.head, "same content, other commit");
    assert!(!second.same_head(&third));
}

#[test]
fn a_reviewed_change_publishes_once_as_a_draft_pull_request() {
    let scratch = Scratch::new(None);
    let log = scratch.count_receives(0);
    scratch.change();
    let review = scratch.review();
    let forge = FakeForge::default();
    let publisher = Publisher::new(&scratch.store, &forge);
    let first = publisher
        .publish(&scratch.task, &reviewed(&review))
        .unwrap();
    assert_eq!(first.state, PublishState::Published, "{}", first.note);
    assert_eq!(first.landing, Landing::DraftPullRequest);
    assert_eq!(
        first.url.as_deref(),
        Some("https://github.com/example/scratch/pull/7")
    );
    assert_eq!(
        first.operation,
        operation_id(&scratch.task, &reviewed(&review))
    );
    first.validate().unwrap();
    let branch = first.branch.clone().unwrap();
    let commit = first.commit.clone().unwrap();
    assert_eq!(
        scratch.remote_tip(&branch).as_deref(),
        Some(commit.as_str())
    );
    // The commit is exactly the reviewed tree on the reviewed head.
    assert_eq!(
        run(
            &scratch.remote,
            &["rev-parse", &format!("{commit}^{{tree}}")]
        ),
        review.head
    );
    assert_eq!(
        run(&scratch.remote, &["rev-parse", &format!("{commit}^")]),
        review.head_commit
    );
    // main did not move, and the worktree is as it was.
    assert_eq!(
        scratch.remote_tip("main").as_deref(),
        Some(scratch.base.as_str())
    );
    assert_eq!(run(&scratch.worktree, &["rev-parse", "HEAD"]), scratch.base);
    assert_eq!(receives(&log), 1);
    assert_eq!(forge.opened.load(Ordering::SeqCst), 1);
    // Publishing the same review again changes nothing.
    let again = publisher
        .publish(&scratch.task, &reviewed(&review))
        .unwrap();
    assert_eq!(again, first);
    assert_eq!(receives(&log), 1);
    assert_eq!(forge.opened.load(Ordering::SeqCst), 1);
    // The task's review carries the publication.
    assert_eq!(last(&scratch.store, &scratch.task), Some(first.clone()));
    let wire = review::read_for_wire(
        &scratch.store,
        &scratch.task,
        &scratch.worktree,
        &scratch.base,
    )
    .unwrap();
    assert_eq!(wire.publication, Some(first));
    wire.validate().unwrap();
    let _ = &scratch.checkout;
}

#[test]
fn a_policy_that_lands_on_main_pushes_onto_it_fast_forward_only() {
    let scratch = Scratch::new(Some(r#"{"land": "main"}"#));
    let log = scratch.count_receives(0);
    scratch.change();
    let review = scratch.review();
    let forge = FakeForge::default();
    let published = Publisher::new(&scratch.store, &forge)
        .publish(&scratch.task, &reviewed(&review))
        .unwrap();
    assert_eq!(
        published.state,
        PublishState::Published,
        "{}",
        published.note
    );
    assert_eq!(published.landing, Landing::Branch);
    assert_eq!(published.branch.as_deref(), Some("main"));
    let commit = published.commit.clone().unwrap();
    assert_eq!(scratch.remote_tip("main").as_deref(), Some(commit.as_str()));
    assert_eq!(
        published.url,
        Some(format!(
            "https://github.com/example/scratch/commit/{commit}"
        ))
    );
    assert_eq!(forge.opened.load(Ordering::SeqCst), 0, "no pull request");
    assert_eq!(receives(&log), 1);

    // When main moved past the base, the push is refused and nothing moves.
    let other = Scratch::new(Some(r#"{"land": "main"}"#));
    std::fs::write(other.checkout.join("b.txt"), "someone else\n").unwrap();
    run(&other.checkout, &["commit", "-q", "-am", "other"]);
    run(&other.checkout, &["push", "-q", "origin", "main"]);
    let moved = other.remote_tip("main");
    other.change();
    let review = other.review();
    let refused = Publisher::new(&other.store, &forge)
        .publish(&other.task, &reviewed(&review))
        .unwrap();
    assert_eq!(refused.state, PublishState::Refused, "{}", refused.note);
    assert_eq!(other.remote_tip("main"), moved);
}

#[test]
fn an_uncertain_push_reconciles_by_reading_the_remote_without_pushing_twice() {
    let scratch = Scratch::new(None);
    // The remote takes the push, then its hook outlasts the client's
    // patience: the push landed, but the client cannot know.
    let log = scratch.count_receives(4);
    scratch.change();
    let review = scratch.review();
    let forge = FakeForge::default();
    let uncertain = Publisher::new(&scratch.store, &forge)
        .with_push_timeout(Duration::from_millis(1500))
        .publish(&scratch.task, &reviewed(&review))
        .unwrap();
    assert_eq!(
        uncertain.state,
        PublishState::Uncertain,
        "{}",
        uncertain.note
    );
    let commit = uncertain.commit.clone().unwrap();
    let branch = uncertain.branch.clone().unwrap();
    assert_eq!(forge.opened.load(Ordering::SeqCst), 0);
    // The ref did move before the hook ran.
    assert_eq!(
        scratch.remote_tip(&branch).as_deref(),
        Some(commit.as_str())
    );
    assert_eq!(receives(&log), 1);
    // Publishing again finds the commit on the remote and only opens the
    // pull request; the remote receives nothing more.
    let settled = Publisher::new(&scratch.store, &forge)
        .publish(&scratch.task, &reviewed(&review))
        .unwrap();
    assert_eq!(settled.state, PublishState::Published, "{}", settled.note);
    assert_eq!(settled.operation, uncertain.operation);
    assert_eq!(settled.commit.as_deref(), Some(commit.as_str()));
    assert_eq!(receives(&log), 1, "no second push");
    assert_eq!(forge.opened.load(Ordering::SeqCst), 1);
}

#[test]
fn an_uncertain_push_that_did_not_land_is_pushed_once_on_retry() {
    let scratch = Scratch::new(None);
    let log = scratch.count_receives(0);
    scratch.change();
    let review = scratch.review();
    let reviewed = reviewed(&review);
    // A recorded attempt whose push never reached the remote, as after a
    // crash between the record and the push.
    let forge = FakeForge::default();
    let mut ledger = load(&scratch.store, &scratch.task);
    let commit = run(
        &scratch.worktree,
        &[
            "commit-tree",
            &review.head,
            "-p",
            &review.head_commit,
            "-m",
            "t",
        ],
    );
    let operation = operation_id(&scratch.task, &reviewed);
    let pending = Publication {
        operation: operation.clone(),
        task: scratch.task.clone(),
        base: reviewed.base.clone(),
        head_commit: reviewed.head_commit.clone(),
        head: reviewed.head.clone(),
        landing: Landing::DraftPullRequest,
        state: PublishState::Uncertain,
        branch: Some(format!(
            "coder/review-{}-{}",
            &scratch.task[..8],
            &operation[..8]
        )),
        commit: Some(commit.clone()),
        url: None,
        note: "Pushing the reviewed change.".into(),
    };
    keep(&scratch.store, &mut ledger, &pending).unwrap();
    let done = Publisher::new(&scratch.store, &forge)
        .publish(&scratch.task, &reviewed)
        .unwrap();
    assert_eq!(done.state, PublishState::Published, "{}", done.note);
    assert_eq!(done.commit.as_deref(), Some(commit.as_str()));
    assert_eq!(receives(&log), 1);
}

#[test]
fn a_stale_review_is_refused_and_nothing_is_pushed() {
    let scratch = Scratch::new(None);
    let log = scratch.count_receives(0);
    scratch.change();
    let review = scratch.review();
    // The worktree moves after the person reviewed it.
    std::fs::write(scratch.worktree.join("a.txt"), "changed again\n").unwrap();
    let forge = FakeForge::default();
    let refused = Publisher::new(&scratch.store, &forge)
        .publish(&scratch.task, &reviewed(&review))
        .unwrap();
    assert_eq!(refused.state, PublishState::Refused);
    assert!(refused.note.contains("Refresh"), "{}", refused.note);
    assert_eq!(refused.commit, None);
    assert_eq!(receives(&log), 0);
    assert_eq!(forge.opened.load(Ordering::SeqCst), 0);
    assert_eq!(
        last(&scratch.store, &scratch.task),
        None,
        "a refusal is not a publication"
    );
    // The refreshed review publishes.
    let fresh = scratch.review();
    let done = Publisher::new(&scratch.store, &forge)
        .publish(&scratch.task, &reviewed(&fresh))
        .unwrap();
    assert_eq!(done.state, PublishState::Published, "{}", done.note);
    assert_eq!(receives(&log), 1);
    // A review of nothing changed publishes nothing.
    let empty = Scratch::new(None);
    let none = Publisher::new(&empty.store, &forge)
        .publish(&empty.task, &reviewed(&empty.review()))
        .unwrap();
    assert_eq!(none.state, PublishState::Refused);
}

#[test]
fn a_forge_that_fails_leaves_the_branch_pushed_and_a_retry_opens_it_without_pushing() {
    let scratch = Scratch::new(None);
    let log = scratch.count_receives(0);
    scratch.change();
    let review = scratch.review();
    let down = FakeForge {
        refuse: true,
        ..FakeForge::default()
    };
    let pushed = Publisher::new(&scratch.store, &down)
        .publish(&scratch.task, &reviewed(&review))
        .unwrap();
    assert_eq!(pushed.state, PublishState::Pushed, "{}", pushed.note);
    assert_eq!(receives(&log), 1);
    let up = FakeForge::default();
    let opened = Publisher::new(&scratch.store, &up)
        .publish(&scratch.task, &reviewed(&review))
        .unwrap();
    assert_eq!(opened.state, PublishState::Published);
    assert_eq!(receives(&log), 1);
    assert_eq!(up.opened.load(Ordering::SeqCst), 1);
    // A task this store did not start has nothing to publish.
    assert_eq!(
        Publisher::new(&scratch.store, &up).publish(&"8".repeat(64), &reviewed(&review)),
        Err(Refusal::NoWorktree)
    );
}

#[test]
fn a_github_remote_names_its_repository() {
    assert_eq!(
        github_repository("git@github.com:OpenAgentsInc/openagents.git").as_deref(),
        Some("OpenAgentsInc/openagents")
    );
    assert_eq!(
        github_repository("https://github.com/o/r").as_deref(),
        Some("o/r")
    );
    assert_eq!(github_repository("/tmp/remote.git"), None);
    assert_eq!(github_repository("https://github.com/o/r/../x"), None);
}
