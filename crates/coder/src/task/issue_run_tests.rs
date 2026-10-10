use super::*;
use openagents_chat::coder_events::{Finished, Stopped};

fn issue(comments: &[(&str, u64)]) -> Issue {
    Issue {
        number: 42,
        title: "Fix the docs".into(),
        body: "See #41 and https://github.com/acme/app/issues/40.".into(),
        url: "https://github.com/acme/app/issues/42".into(),
        open: true,
        comments: comments
            .iter()
            .map(|(body, at)| Comment {
                body: (*body).into(),
                at: *at,
            })
            .collect(),
    }
}

#[test]
fn a_recent_claim_holds_until_it_is_released_or_old() {
    let now = 100_000;
    assert_eq!(claimed(&issue(&[]), now, 6), None);
    assert_eq!(claimed(&issue(&[("Looks good.", now - 10)]), now, 6), None);
    let held = claimed(
        &issue(&[("Claimed: an agent is working on this now.", now - 600)]),
        now,
        6,
    )
    .unwrap();
    assert!(held.starts_with("#42 was claimed 10 minutes ago"), "{held}");
    // Coder's own marker counts wherever it sits.
    let marked = format!("Working.\n\n{CLAIM_MARK} task=abc -->");
    assert!(claimed(&issue(&[(&marked, now - 60)]), now, 6).is_some());
    // A release after the claim frees it; a claim older than the window
    // no longer holds.
    let released = format!("Coder stopped.\n\n{RELEASE_MARK}");
    assert_eq!(
        claimed(
            &issue(&[(&marked, now - 60), (&released, now - 30)]),
            now,
            6
        ),
        None
    );
    assert_eq!(
        claimed(&issue(&[("claimed by me", now - 7 * 3_600)]), now, 6),
        None
    );
}

struct Labels;

impl crate::claim::Hub for Labels {
    fn comment(&self, _: &str, _: u64, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn comments(&self, _: &str, _: u64) -> Result<Vec<Comment>, String> {
        Ok(Vec::new())
    }
    fn labeled(&self, _: &str, label: &str) -> Result<Vec<u64>, String> {
        Ok(if label == "coder-ok" {
            vec![3, 9]
        } else {
            vec![]
        })
    }
}

impl Tracker for Labels {
    fn repository(&self, _: &Path) -> Result<String, String> {
        Ok("acme/app".into())
    }
    fn issue(&self, _: &str, _: u64) -> Result<Issue, String> {
        Err("unused".into())
    }
    fn close(&self, _: &str, _: u64) -> Result<(), String> {
        Ok(())
    }
    fn pull_request(
        &self,
        _: &Path,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<String, String> {
        Err("unused".into())
    }
}

#[test]
fn a_queue_names_numbers_or_a_label() {
    assert_eq!(
        select(&Labels, "acme/app", "10050,10051").unwrap(),
        [10050, 10051]
    );
    assert_eq!(select(&Labels, "acme/app", "#5 #6 #5").unwrap(), [5, 6]);
    assert_eq!(select(&Labels, "acme/app", "coder-ok").unwrap(), [3, 9]);
    assert_eq!(
        select(&Labels, "acme/app", "label:coder-ok").unwrap(),
        [3, 9]
    );
}

#[test]
fn a_policy_file_is_read_and_its_absence_is_the_safe_default() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Policy::load(dir.path()).unwrap();
    assert_eq!(policy.land, Land::PullRequest);
    std::fs::create_dir_all(dir.path().join(".openagents")).unwrap();
    std::fs::write(
        dir.path().join(POLICY_FILE),
        r#"{"land": "main", "fmt": true, "clippy": true, "claim_hours": 2}"#,
    )
    .unwrap();
    let policy = Policy::load(dir.path()).unwrap();
    assert_eq!(
        (policy.land, policy.fmt, policy.clippy, policy.claim_hours),
        (Land::Main, true, true, 2)
    );
    assert_eq!(
        (policy.max_steps, policy.continue_turns),
        (None, None),
        "no step limit and nothing to continue (#10103)"
    );
    // An older policy that set a step limit and continuation turns still
    // reads; both are ignored.
    std::fs::write(
        dir.path().join(POLICY_FILE),
        r#"{"land": "main", "max_steps": 100, "continue_turns": 2}"#,
    )
    .unwrap();
    let policy = Policy::load(dir.path()).unwrap();
    assert_eq!(
        (policy.max_steps, policy.continue_turns),
        (Some(100), Some(2))
    );
    std::fs::write(dir.path().join(POLICY_FILE), r#"{"land": "sideways"}"#).unwrap();
    assert!(Policy::load(dir.path()).is_err());
    assert_eq!(Land::parse("pr").unwrap(), Land::PullRequest);
    assert_eq!(Land::parse("queue").unwrap(), Land::Queue);
    assert!(Land::parse("force").is_err());
    // The landing queue reads from the policy file and writes back the same.
    std::fs::write(dir.path().join(POLICY_FILE), r#"{"land": "queue"}"#).unwrap();
    let policy = Policy::load(dir.path()).unwrap();
    assert_eq!(policy.land, Land::Queue);
    let written = serde_json::to_value(&policy).unwrap();
    assert_eq!(written["land"], "queue");
    assert_eq!(serde_json::from_value::<Policy>(written).unwrap(), policy);
}

/// This repository's own policy lands on main after the checks: fmt, and
/// no clippy (8afcce4131).
#[test]
fn this_repository_lands_on_main_with_its_checks() {
    let top = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let policy = Policy::load(&top).unwrap();
    assert_eq!(policy.land, Land::Main);
    assert!(policy.fmt && !policy.clippy);
    // #10103: no step limit and no continuation turns.
    assert_eq!((policy.max_steps, policy.continue_turns), (None, None));
}

fn flow(outcome: &str) -> Flow {
    Flow {
        schema: FLOW_SCHEMA.into(),
        task: "task-fixture-1234".into(),
        link: IssueLink {
            repository: "acme/app".into(),
            number: 42,
            url: "https://github.com/acme/app/issues/42".into(),
            title: "Fix the docs".into(),
            outcome: outcome.into(),
            commits: vec!["0123456789abcdef".into()],
            pull_request: None,
            closed: outcome == "landed",
            not_landed: None,
        },
        notes: Vec::new(),
        finished: true,
        process_id: None,
        closing: "Closing words.".into(),
        files: None,
    }
}

fn result() -> CoderEvent {
    CoderEvent::Result(Finished {
        turn: 2,
        summary: "I fixed it.".into(),
        files_changed: vec![FileChange {
            path: "docs/a.md".into(),
            status: "modified".into(),
            added: Some(3),
            removed: Some(1),
            ..FileChange::default()
        }],
        insertions: 0,
        deletions: 0,
        worktree: "/w".into(),
        trajectory: "/t".into(),
        issue: None,
        pushed_to: None,
        cost_microusd: None,
    })
}

#[test]
fn the_last_ending_carries_the_issue_and_what_the_flow_did() {
    let CoderEvent::Result(landed) = flow("landed").ending(result()) else {
        panic!()
    };
    assert_eq!(landed.summary, "I fixed it.\n\nClosing words.");
    assert_eq!((landed.insertions, landed.deletions), (3, 1));
    let link = landed.issue.unwrap();
    assert!(
        link.line()
            .contains("landed 0123456789 on the default branch and closed")
    );

    // A result whose checks stayed red is a failure, not a result.
    let CoderEvent::Failure(failed) = flow("failed").ending(result()) else {
        panic!()
    };
    assert_eq!(failed.message, "Closing words.");
    assert_eq!(failed.ending.as_deref(), Some("issue_failed"));
    assert_eq!(failed.issue.unwrap().outcome, "failed");

    let CoderEvent::Stopped(stopped) = flow("stopped").ending(CoderEvent::Stopped(Stopped {
        turn: 1,
        message: "Coder stopped.".into(),
    })) else {
        panic!()
    };
    assert_eq!(stopped.message, "Coder stopped. Closing words.");
}

#[test]
fn the_prompt_carries_the_issue_its_comments_and_what_it_links() {
    let linked = vec![Issue {
        number: 41,
        title: "The parent".into(),
        body: "Parent text.".into(),
        url: String::new(),
        open: true,
        comments: Vec::new(),
    }];
    let text = prompt(
        &issue(&[
            ("Claimed: an agent is on it.", 1),
            ("Use the glossary's word.", 2),
        ]),
        &linked,
    );
    assert!(text.starts_with("# Issue #42: Fix the docs\n"), "{text}");
    assert!(text.contains("Use the glossary's word."));
    assert!(!text.contains("an agent is on it"), "claims stay out");
    assert!(text.contains("### #41: The parent\n\nParent text."));
    assert!(text.contains(coder_delegate::terminal::ISSUE_DIRECTIONS));
}

#[test]
fn iso_times_read_as_unix_seconds() {
    assert_eq!(crate::claim::iso_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(
        crate::claim::iso_seconds("2026-09-30T12:00:00Z"),
        Some(1_790_769_600)
    );
    assert_eq!(crate::claim::iso_seconds("nope"), None);
}

fn local_record() -> Record {
    Record {
        schema: local::RECORD_SCHEMA.into(),
        task: "task-fixture-1234".into(),
        thread: None,
        project: "app".into(),
        checkout: "/fixture".into(),
        worktree: "/fixture".into(),
        base: "main".into(),
        turns: Vec::new(),
        ends: Default::default(),
        requested: None,
        shape: Default::default(),
        hooks: None,
        archived: None,
    }
}

#[test]
fn an_inactive_own_claim_is_recovered_but_live_and_foreign_claims_are_not() {
    use super::super::{
        Action, COMMAND_SCHEMA, Command, RequestedConfiguration, Store, TaskIntent, Workspace,
    };
    let dir = tempfile::tempdir().unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let record = local_record();
    std::fs::create_dir(dir.path().join("local")).unwrap();
    std::fs::write(
        dir.path().join("local/task-fixture-1234.json"),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    let mut old = flow("stopped");
    save(dir.path(), &old).unwrap();
    let apply = |store: &mut Store, id: &str, action| {
        store
            .apply(
                &serde_json::to_vec(&Command {
                    schema: COMMAND_SCHEMA.into(),
                    command_id: id.into(),
                    task_id: "task-fixture-1234".into(),
                    expected_revision: (id != "submit").then_some(1),
                    action,
                })
                .unwrap(),
            )
            .unwrap();
    };
    {
        let mut store = Store::open(dir.path()).unwrap();
        apply(
            &mut store,
            "submit",
            Action::Submit {
                intent: TaskIntent {
                    title: "fixture".into(),
                    prompt: "fixture".into(),
                    workspace: Workspace {
                        path: record.worktree.clone(),
                        source_revision: None,
                    },
                    configuration: RequestedConfiguration {
                        adapter: "bounded-command".into(),
                        model: None,
                    },
                    images: vec![],
                },
            },
        );
    }
    let marked = format!("Claimed: Coder. {CLAIM_MARK} task=task-fixture-1234 -->");
    let own = issue(&[(&marked, 100)]);
    assert!(
        inactive_own_claim(dir.path(), "acme/app", &own).is_none(),
        "queued tasks are protected"
    );
    {
        let mut store = Store::open(dir.path()).unwrap();
        apply(
            &mut store,
            "cancel",
            Action::Cancel {
                reason: "stopped".into(),
            },
        );
    }
    let recovered = inactive_own_claim(dir.path(), "acme/app", &own).unwrap();
    assert!(recovered.contains("taking #42 again"));
    assert_eq!(recovered.lines().count(), 1);
    old.finished = false;
    old.process_id = Some(std::process::id());
    save(dir.path(), &old).unwrap();
    assert!(
        inactive_own_claim(dir.path(), "acme/app", &own).is_none(),
        "live flows between turns are protected"
    );
    old.process_id = None;
    save(dir.path(), &old).unwrap();
    assert!(inactive_own_claim(dir.path(), "acme/app", &own).is_some());
    assert!(inactive_own_claim(dir.path(), "other/app", &own).is_none());
    let foreign = format!("Claimed: Coder. {CLAIM_MARK} task=other-computer -->");
    assert!(inactive_own_claim(dir.path(), "acme/app", &issue(&[(&foreign, 100)])).is_none());
    assert!(
        inactive_own_claim(
            dir.path(),
            "acme/app",
            &issue(&[("Claimed: another agent", 100)])
        )
        .is_none()
    );
}

#[derive(Default)]
struct Comments(Mutex<Vec<String>>);
impl crate::claim::Hub for Comments {
    fn comment(&self, _: &str, _: u64, body: &str) -> Result<(), String> {
        self.0.lock().unwrap().push(body.into());
        Ok(())
    }
    fn comments(&self, _: &str, _: u64) -> Result<Vec<Comment>, String> {
        Ok(Vec::new())
    }
    fn labeled(&self, _: &str, _: &str) -> Result<Vec<u64>, String> {
        Ok(vec![])
    }
}

impl Tracker for Comments {
    fn repository(&self, _: &Path) -> Result<String, String> {
        Ok("acme/app".into())
    }
    fn issue(&self, _: &str, _: u64) -> Result<Issue, String> {
        Ok(issue(&[]))
    }
    fn close(&self, _: &str, _: u64) -> Result<(), String> {
        panic!("must not close")
    }
    fn pull_request(
        &self,
        _: &Path,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<String, String> {
        panic!("must not land")
    }
}
struct NoChecks;
impl Checks for NoChecks {
    fn check(&self, _: &Path, _: &Policy) -> Checked {
        panic!("must not check")
    }
}

#[test]
fn stopping_or_losing_a_process_releases_the_claim_in_one_line() {
    for stopped in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let tracker = Arc::new(Comments::default());
        let work = Work {
            store: dir.path().into(),
            local: Arc::new(Local::new(dir.path().into())),
            tracker: tracker.clone(),
            checks: Arc::new(NoChecks),
            policy: Policy::default(),
            branch: "main".into(),
            top: dir.path().into(),
            now: || 100,
            artifacts: None,
            queue: None,
        };
        let record = local_record();
        let mut flow = flow("working");
        flow.finished = false;
        let issue = issue(&[]);
        let mut run = Run {
            work: &work,
            flow: &mut flow,
            record: &record,
            issue: &issue,
            repository: "acme/app",
            worktree: dir.path(),
            turn: 1,
            summaries: vec![],
            checked: Checked::default(),
            rounds: 0,
            stranded: None,
        };
        if stopped {
            run.stopped("Stopped by the person.");
        } else {
            run.failed("The task owner process ended.", None);
        }
        assert!(flow.finished);
        let comments = tracker.0.lock().unwrap();
        let release = comments.last().unwrap();
        assert_eq!(release.lines().count(), 1);
        assert!(release.contains("released its claim"));
        assert_eq!(
            claimed(
                &self::issue(&[("Claimed: Coder", 90), (release, 100)]),
                101,
                6
            ),
            None
        );
    }
}

/// Records each upload and links it under `https://bucket.test/`.
#[derive(Default)]
struct FakeBucket(Mutex<Vec<String>>);

impl super::super::run_artifacts::Uploader for FakeBucket {
    fn upload(&self, object: &str, _: &[u8]) -> Result<String, String> {
        self.0.lock().unwrap().push(object.to_owned());
        Ok(format!("https://bucket.test/{object}"))
    }
}

#[test]
fn a_failure_comment_links_the_runs_uploaded_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let tracker = Arc::new(Comments::default());
    let bucket = Arc::new(FakeBucket::default());
    let mut record = local_record();
    record.turns.push(local::TurnStart {
        turn: 1,
        revision: 1,
        provider: "codex".into(),
        model: "gpt".into(),
        reason: "first".into(),
        fallbacks: Vec::new(),
        at: 90,
        runner: None,
        timings: None,
    });
    std::fs::write(
        dir.path().join(format!("{}.1.atif.jsonl", record.task)),
        "{}\n",
    )
    .unwrap();
    let work = Work {
        store: dir.path().into(),
        local: Arc::new(Local::new(dir.path().into())),
        tracker: tracker.clone(),
        checks: Arc::new(NoChecks),
        policy: Policy::default(),
        branch: "main".into(),
        top: dir.path().into(),
        now: || 100,
        artifacts: Some(bucket.clone()),
        queue: None,
    };
    let mut flow = flow("working");
    flow.finished = false;
    let issue = issue(&[]);
    let mut run = Run {
        work: &work,
        flow: &mut flow,
        record: &record,
        issue: &issue,
        repository: "acme/app",
        worktree: dir.path(),
        turn: 1,
        summaries: vec![],
        checked: Checked {
            problems: vec!["cargo test failed".into()],
            ..Checked::default()
        },
        rounds: 0,
        stranded: None,
    };
    run.failed("The checks fail.", None);
    let uploaded = bucket.0.lock().unwrap().clone();
    let prefix = format!("coder/acme-app/42/{}-failed", record.task.replace('.', "-"));
    assert_eq!(
        uploaded,
        vec![
            format!("{prefix}/turn-1.atif.jsonl"),
            format!("{prefix}/checks.txt"),
            format!("{prefix}/route.json"),
        ]
    );
    let comments = tracker.0.lock().unwrap();
    let failure = comments
        .iter()
        .find(|c| c.contains("did not land"))
        .unwrap();
    assert!(failure.contains("**Run artifacts**"));
    assert!(failure.contains(&format!("(<https://bucket.test/{prefix}/route.json>)")));
}

fn started_fixture(dir: &Path) -> Started {
    let tracker = Arc::new(Comments::default());
    std::fs::create_dir_all(dir.join("local")).unwrap();
    let mut flow = flow("working");
    flow.finished = false;
    flow.process_id = Some(std::process::id());
    save(dir, &flow).unwrap();
    Started {
        record: local_record(),
        issue: issue(&[]),
        repository: "acme/app".into(),
        work: Work {
            store: dir.into(),
            local: Arc::new(Local::new(dir.into())),
            tracker,
            checks: Arc::new(NoChecks),
            policy: Policy {
                land: Land::Main,
                ..Policy::default()
            },
            branch: "main".into(),
            top: dir.into(),
            now: || 100,
            artifacts: None,
            queue: None,
        },
    }
}

/// A driver that takes the flow over as `microcoder issue-flow` does: it
/// records its own process as the flow's, then stays a moment.
#[cfg(unix)]
fn fake_driver(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("driver.sh");
    std::fs::write(
        &path,
        "#!/bin/sh\n[ \"$1\" = issue-flow ] || exit 2\n\
         flow=\"$3/local/$5.issue.json\"\n\
         sed \"s/\\\"process_id\\\": [0-9a-z]*/\\\"process_id\\\": $$/\" \"$flow\" > \"$flow.new\" \
         && mv \"$flow.new\" \"$flow\"\nsleep 1\n",
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[cfg(unix)]
#[test]
fn a_started_flow_goes_to_a_process_of_its_own_with_what_it_needs() {
    let dir = tempfile::tempdir().unwrap();
    let driver = fake_driver(dir.path());
    let started = started_fixture(dir.path());
    let Handed::Detached { process } = started.hand_off(Some(&driver)) else {
        panic!("the driver took it over")
    };
    let flow = load(dir.path(), "task-fixture-1234").unwrap();
    assert_eq!(flow.process_id, Some(process));
    assert!(!orphaned(&flow), "its process works it");
    // The job carries the issue, the policy, and where to land.
    let job: Job =
        serde_json::from_slice(&std::fs::read(job_path(dir.path(), "task-fixture-1234")).unwrap())
            .unwrap();
    assert_eq!(job.issue, issue(&[]));
    assert_eq!(
        (
            job.policy.land,
            job.branch.as_str(),
            job.repository.as_str()
        ),
        (Land::Main, "main", "acme/app")
    );
    // The driver reads it back; this run has no record, so it says so
    // after reading the job, before changing anything.
    let runner = Runner {
        local: Arc::new(Local::new(dir.path().into())),
        tracker: Arc::new(Comments::default()),
        checks: Arc::new(NoChecks),
        land: None,
        skip_claimed: true,
        now: || 100,
        artifacts: None,
    };
    assert_eq!(
        drive_with(&runner, dir.path(), "task-fixture-1234").unwrap_err(),
        "This issue flow's run has no record."
    );
    // A flow that already ended is returned as it is.
    save(dir.path(), &self::flow("landed")).unwrap();
    let ended = drive_with(&runner, dir.path(), "task-fixture-1234").unwrap();
    assert_eq!(ended.link.outcome, "landed");
}

#[cfg(unix)]
#[test]
fn a_flow_whose_process_is_gone_is_orphaned() {
    let mut flow = flow("working");
    flow.finished = false;
    flow.process_id = Some(std::process::id());
    assert!(!orphaned(&flow));
    let mut child = std::process::Command::new("true").spawn().unwrap();
    let gone = child.id();
    child.wait().unwrap();
    flow.process_id = Some(gone);
    assert!(orphaned(&flow));
    flow.finished = true;
    assert!(!orphaned(&flow), "an ended flow is not");
}

#[cfg(unix)]
#[test]
fn a_driver_that_cannot_take_the_flow_leaves_it_here() {
    let dir = tempfile::tempdir().unwrap();
    // An engine older than this program exits without taking it over; a
    // missing one never starts. Either way the flow comes back to be
    // worked here, still this process's.
    for driver in ["/usr/bin/false", "/nonexistent/microcoder"] {
        let started = started_fixture(dir.path());
        let back = started.detach(Path::new(driver)).err().unwrap();
        assert_eq!(back.record.task, "task-fixture-1234");
        assert_eq!(
            load(dir.path(), "task-fixture-1234").unwrap().process_id,
            Some(std::process::id())
        );
    }
}

use crate::claim::Hub as _;

/// GitHub with a project board, for the chat flow (#10203).
struct Board(crate::claim::fake::Fake);

impl crate::claim::Hub for Board {
    fn comment(&self, repository: &str, number: u64, body: &str) -> Result<(), String> {
        self.0.comment(repository, number, body)
    }
    fn comments(&self, repository: &str, number: u64) -> Result<Vec<Comment>, String> {
        self.0.comments(repository, number)
    }
    fn labeled(&self, repository: &str, label: &str) -> Result<Vec<u64>, String> {
        self.0.labeled(repository, label)
    }
    fn viewer(&self) -> Result<String, String> {
        self.0.viewer()
    }
    fn items(
        &self,
        repository: &str,
        number: u64,
        field: &str,
    ) -> Result<Vec<crate::claim::Item>, String> {
        self.0.items(repository, number, field)
    }
}

impl Tracker for Board {
    fn repository(&self, _: &Path) -> Result<String, String> {
        Ok("acme/app".into())
    }
    fn issue(&self, repository: &str, number: u64) -> Result<Issue, String> {
        Ok(Issue {
            comments: self.0.comments(repository, number)?,
            ..issue(&[])
        })
    }
    fn close(&self, _: &str, _: u64) -> Result<(), String> {
        panic!("must not close")
    }
    fn pull_request(
        &self,
        _: &Path,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<String, String> {
        panic!("must not land")
    }
}

/// #10203: the chat flow honours project Status: an issue another agent
/// moved to "In progress", with no claim comment, is claimed, and a
/// queue leaves it alone.
#[test]
fn a_queue_leaves_an_issue_in_progress_on_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let top = dir.path().join("app");
    std::fs::create_dir_all(&top).unwrap();
    for args in [
        &["init", "-q"][..],
        &[
            "-c",
            "user.name=F",
            "-c",
            "user.email=f@example.invalid",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "first",
        ][..],
    ] {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(&top)
            .status()
            .unwrap();
        assert!(status.success());
    }
    let github = crate::claim::fake::Fake::with_project("octo", &["Todo", "In Progress", "Done"]);
    github.issue(42, Some("In Progress"), &[], &[]);
    let now = github.now;
    github
        .issues
        .lock()
        .unwrap()
        .get_mut(&42)
        .unwrap()
        .status_at = now - 60;
    let runner = Runner {
        local: Arc::new(Local::new(dir.path().join("tasks"))),
        tracker: Arc::new(Board(github)),
        checks: Arc::new(NoChecks),
        land: None,
        skip_claimed: true,
        now: || 1_000,
        artifacts: None,
    };
    let reference = Reference {
        repository: None,
        number: 42,
    };
    let why = match runner.begin(&top, &reference, None) {
        Err(Refused::Claimed(why)) => why,
        Err(other) => panic!("refused otherwise: {other:?}"),
        Ok(_) => panic!("started an issue in progress on the board"),
    };
    assert!(
        why.contains("is \"In Progress\" on the project \"Board\""),
        "{why}"
    );
}

/// A tracker whose issue #42 carries the given comments and records what
/// the release posts.
struct Claimed {
    comments: Vec<String>,
    posted: Mutex<Vec<String>>,
}

impl crate::claim::Hub for Claimed {
    fn comment(&self, _: &str, _: u64, body: &str) -> Result<(), String> {
        self.posted.lock().unwrap().push(body.into());
        Ok(())
    }
    fn comments(&self, _: &str, _: u64) -> Result<Vec<Comment>, String> {
        Ok(Vec::new())
    }
    fn labeled(&self, _: &str, _: &str) -> Result<Vec<u64>, String> {
        Ok(vec![])
    }
    fn viewer(&self) -> Result<String, String> {
        Err("no viewer here".into())
    }
}

impl Tracker for Claimed {
    fn repository(&self, _: &Path) -> Result<String, String> {
        Ok("acme/app".into())
    }
    fn issue(&self, _: &str, _: u64) -> Result<Issue, String> {
        let comments: Vec<(&str, u64)> = self.comments.iter().map(|c| (c.as_str(), 100)).collect();
        Ok(issue(&comments))
    }
    fn close(&self, _: &str, _: u64) -> Result<(), String> {
        panic!("must not close")
    }
    fn pull_request(
        &self,
        _: &Path,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<String, String> {
        panic!("must not land")
    }
}

#[test]
fn a_claim_whose_task_ended_is_stale_and_released_but_live_landed_and_foreign_ones_are_not() {
    use super::super::{
        Action, COMMAND_SCHEMA, Command, RequestedConfiguration, Store, TaskIntent, Workspace,
    };
    let dir = tempfile::tempdir().unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::create_dir(dir.path().join("local")).unwrap();
    let mut flow = flow("stopped");
    save(dir.path(), &flow).unwrap();
    let ours = format!("Claimed: Coder. {CLAIM_MARK} task=task-fixture-1234 -->");
    let tracker = Claimed {
        comments: vec![ours.clone()],
        posted: Mutex::new(Vec::new()),
    };
    let now = 2_000_000_000;
    // No task record and recently written: not stale yet.
    assert!(stale_claims(dir.path(), &tracker, now - 1_000_000_000, 6).is_empty());
    {
        let mut store = Store::open(dir.path()).unwrap();
        for (id, action) in [
            (
                "submit",
                Action::Submit {
                    intent: TaskIntent {
                        title: "fixture".into(),
                        prompt: "fixture".into(),
                        workspace: Workspace {
                            path: "/fixture".into(),
                            source_revision: None,
                        },
                        configuration: RequestedConfiguration {
                            adapter: "bounded-command".into(),
                            model: None,
                        },
                        images: vec![],
                    },
                },
            ),
            (
                "cancel",
                Action::Cancel {
                    reason: "stopped".into(),
                },
            ),
        ] {
            store
                .apply(
                    &serde_json::to_vec(&Command {
                        schema: COMMAND_SCHEMA.into(),
                        command_id: id.into(),
                        task_id: "task-fixture-1234".into(),
                        expected_revision: (id != "submit").then_some(1),
                        action,
                    })
                    .unwrap(),
                )
                .unwrap();
        }
    }
    let stale = stale_claims(dir.path(), &tracker, now, 6);
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].number, 42);
    assert_eq!(stale[0].why, "its task ended");
    release_stale(&tracker, &stale[0]).unwrap();
    let posted = tracker.posted.lock().unwrap().clone();
    assert!(posted[0].contains(RELEASE_MARK) && posted[0].contains("task-fixture-1234"));
    // A live flow, a landed one, and another computer's claim are kept.
    flow.process_id = Some(std::process::id());
    save(dir.path(), &flow).unwrap();
    assert!(stale_claims(dir.path(), &tracker, now, 6).is_empty());
    flow.process_id = None;
    flow.link.outcome = "landed".into();
    save(dir.path(), &flow).unwrap();
    assert!(stale_claims(dir.path(), &tracker, now, 6).is_empty());
    flow.link.outcome = "stopped".into();
    save(dir.path(), &flow).unwrap();
    let foreign = Claimed {
        comments: vec![format!("Claimed: Coder. {CLAIM_MARK} task=other -->")],
        posted: Mutex::new(Vec::new()),
    };
    assert!(stale_claims(dir.path(), &foreign, now, 6).is_empty());
    let released = Claimed {
        comments: vec![ours, format!("Free again. {RELEASE_MARK}")],
        posted: Mutex::new(Vec::new()),
    };
    assert!(stale_claims(dir.path(), &released, now, 6).is_empty());
}

#[test]
fn the_comments_name_where_the_run_is() {
    use super::placement_named;
    assert_eq!(placement_named(None).claim, "on this computer");
    assert_eq!(placement_named(None).run, "on the owner's computer");
    assert_eq!(placement_named(Some("boat")).claim, "on a Boat sandbox");
    assert_eq!(placement_named(Some("gce")).run, "on a GCE pool host");
    assert_eq!(placement_named(Some("other")).claim, "on this computer");
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// A worktree with one committed change on top of origin's `main`.
fn committed_change(dir: &Path) -> (PathBuf, PathBuf) {
    let origin = dir.join("origin.git");
    git(
        dir,
        &[
            "init",
            "-q",
            "--bare",
            "-b",
            "main",
            origin.to_str().unwrap(),
        ],
    );
    let work = dir.join("work");
    git(
        dir,
        &[
            "clone",
            "-q",
            origin.to_str().unwrap(),
            work.to_str().unwrap(),
        ],
    );
    git(&work, &["commit", "-q", "--allow-empty", "-m", "base"]);
    git(&work, &["push", "-q", "origin", "HEAD:refs/heads/main"]);
    std::fs::write(work.join("fix.txt"), "fixed\n").unwrap();
    git(&work, &["add", "fix.txt"]);
    git(&work, &["commit", "-q", "-m", "the fix"]);
    (origin, work)
}

fn stranded_run_comment(origin_ok: bool) -> (String, String, PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let (origin, worktree) = committed_change(dir.path());
    if !origin_ok {
        git(
            &worktree,
            &["remote", "set-url", "origin", "/nonexistent/origin.git"],
        );
    }
    let tracker = Arc::new(Comments::default());
    let record = local_record();
    let work = Work {
        store: dir.path().into(),
        local: Arc::new(Local::new(dir.path().into())),
        tracker: tracker.clone(),
        checks: Arc::new(NoChecks),
        policy: Policy::default(),
        branch: "main".into(),
        top: worktree.clone(),
        now: || 100,
        artifacts: None,
        queue: None,
    };
    let mut flow = flow("working");
    flow.finished = false;
    let issue = issue(&[]);
    let mut run = Run {
        work: &work,
        flow: &mut flow,
        record: &record,
        issue: &issue,
        repository: "acme/app",
        worktree: &worktree,
        turn: 1,
        summaries: vec![],
        checked: Checked::default(),
        rounds: 0,
        stranded: None,
    };
    let kept = run.strand();
    assert_eq!(kept, origin_ok);
    run.failed("The change conflicts with the newer main.", None);
    let comment = tracker
        .0
        .lock()
        .unwrap()
        .iter()
        .find(|c| c.contains("did not land"))
        .unwrap()
        .clone();
    let _ = origin;
    (comment, flow.closing.clone(), worktree, dir)
}

#[test]
fn a_change_that_cannot_land_is_kept_on_a_stranded_branch_on_origin() {
    let (comment, _, worktree, dir) = stranded_run_comment(true);
    let branch = super::stranded_branch("task-fixture-1234");
    assert_eq!(branch, "coder/stranded-task-fix");
    let origin = dir.path().join("origin.git");
    let head = git(&worktree, &["rev-parse", "HEAD"]);
    assert_eq!(
        git(&origin, &["rev-parse", &format!("refs/heads/{branch}")]),
        head,
        "origin keeps the committed change"
    );
    assert_eq!(
        git(&worktree, &["status", "--porcelain"]),
        "",
        "nothing is left only in the worktree"
    );
    assert!(comment.contains(&format!("https://github.com/acme/app/tree/{branch}")));
    assert!(!comment.contains("Nothing was pushed"));
}

#[test]
fn a_stranded_push_that_fails_says_nothing_was_pushed() {
    let (comment, _, _, _dir) = stranded_run_comment(false);
    assert!(comment.contains("Nothing was pushed"));
    assert!(!comment.contains("coder/stranded-"));
}

#[test]
fn the_checks_build_in_a_slot_of_the_task_store() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "t@example.com"],
    ] {
        assert!(
            std::process::Command::new("git")
                .args(&args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success()
        );
    }
    let store = dir.path().join("openagents").join("tasks");
    std::fs::create_dir_all(&store).unwrap();
    let gate = Gate {
        jev: None,
        store: Some(store.clone()),
    };
    let first = gate
        .slot(&repo)
        .expect("slot admission")
        .expect("a free slot");
    let root = store.parent().unwrap().join("targets");
    assert!(first.path.starts_with(&root), "{}", first.path.display());
    assert!(first.path.is_dir());
    // The checks stand before a push, so their build lease waits at `push`
    // unless the flow already runs at `owner` (#10757).
    let inherited = coder_lease::Priority::from_env().ok().flatten();
    assert_eq!(
        first.build_priority(),
        Some(crate::task::targets::check_priority(inherited))
    );
    if inherited.is_none() {
        assert_eq!(first.build_priority(), Some(coder_lease::Priority::Push));
    }
    // A second check at the same time takes another slot, not the same one.
    let second = gate
        .slot(&repo)
        .expect("slot admission")
        .expect("another free slot");
    assert_ne!(first.path, second.path);
    // Without a store the checks build where `confined` says.
    let unslotted = Gate {
        jev: None,
        store: None,
    };
    assert!(unslotted.slot(&repo).unwrap().is_none());
}

/// Issues live on GitHub; a checkout whose `origin` is a local bare
/// repository or another forge has none, so a coding request there runs
/// as an ordinary task instead of failing on `gh repo view` (#10398).
#[test]
fn only_a_checkout_with_a_github_origin_has_issues_to_work() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-q", "-b", "main", "repo"]);
    assert!(!super::on_github(&repo), "no origin");
    git(&["-C", "repo", "remote", "add", "origin", "../origin.git"]);
    assert!(!super::on_github(&repo), "a local bare origin");
    git(&[
        "-C",
        "repo",
        "remote",
        "set-url",
        "origin",
        "https://gitlab.com/a/b.git",
    ]);
    assert!(!super::on_github(&repo), "another forge");
    git(&[
        "-C",
        "repo",
        "remote",
        "set-url",
        "origin",
        "git@github.com:acme/app.git",
    ]);
    assert!(super::on_github(&repo));
    git(&[
        "-C",
        "repo",
        "remote",
        "set-url",
        "origin",
        "https://github.com/acme/app",
    ]);
    assert!(super::on_github(&repo));
}

struct RecoveryLaunch;
impl super::super::autostart::Launch for RecoveryLaunch {
    fn launch(
        &self,
        _: &super::super::autostart::Engine,
        _: &Path,
        _: &Path,
    ) -> Result<super::super::autostart::Launched, String> {
        Ok(super::super::autostart::Launched {
            owner_process: std::process::id(),
            grant_digest: "sha256:fake".into(),
        })
    }
}

#[test]
fn cloud_recovery_starts_from_pushed_work_or_scratch_and_checks_the_full_change() {
    for saved_branch in [
        None,
        Some("coder/stranded-losttask"),
        Some("coder/progress-losttask"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (_, top) = committed_change(dir.path());
        if let Some(branch) = saved_branch {
            git(
                &top,
                &["push", "-q", "origin", &format!("HEAD:refs/heads/{branch}")],
            );
        }
        // Upstream moved since the lost task started. Recovery must retain it.
        git(&top, &["reset", "--hard", "origin/main"]);
        std::fs::write(top.join("new-upstream.txt"), "new main\n").unwrap();
        git(&top, &["add", "new-upstream.txt"]);
        git(&top, &["commit", "-q", "-m", "new upstream"]);
        git(&top, &["push", "-q", "origin", "HEAD:main"]);
        let baseline = git(&top, &["rev-parse", "HEAD"]);
        let claim = format!("Claimed. {CLAIM_MARK} task=losttask123 -->");
        let local = Local::new(dir.path().join("tasks"))
            .with_probe(|_| super::super::capacity::Connection::Connected)
            .with_identify(|_| None)
            .with_opencode_model(|| None)
            .with_controller(std::env::current_exe().unwrap())
            .with_launcher(Box::new(RecoveryLaunch));
        let runner = Runner {
            local: Arc::new(local),
            tracker: Arc::new(Claimed {
                comments: vec![claim],
                posted: Mutex::new(vec![]),
            }),
            checks: Arc::new(NoChecks),
            land: None,
            skip_claimed: true,
            now: || 1_000,
            artifacts: None,
        };
        let reference = Reference {
            repository: Some("acme/app".into()),
            number: 42,
        };
        assert!(matches!(
            runner.begin(&top, &reference, None),
            Err(Refused::Claimed(_))
        ));
        let started = runner
            .begin_recovering(&top, &reference, None, Some("losttask123"))
            .unwrap_or_else(|e| panic!("{e}"));
        let worktree = Path::new(&started.record.worktree);
        assert_eq!(started.record.base, baseline);
        assert_eq!(
            std::fs::read_to_string(worktree.join("new-upstream.txt")).unwrap(),
            "new main\n"
        );
        assert_eq!(worktree.join("fix.txt").exists(), saved_branch.is_some());
        let staged = git(worktree, &["diff", "--cached", "--name-only"]);
        assert_eq!(
            staged,
            if saved_branch.is_some() {
                "fix.txt"
            } else {
                ""
            }
        );
        if saved_branch.is_some() {
            assert!(
                load(runner.local.store(), &started.record.task)
                    .unwrap()
                    .notes
                    .iter()
                    .any(|n| n.text.contains("Recovered pushed work"))
            );
        }
    }
}

#[test]
fn cloud_recovery_never_takes_an_unrelated_claim_or_treats_remote_errors_as_scratch() {
    let own = format!("Claimed. {CLAIM_MARK} task=losttask123 -->");
    let other = format!("Claimed. {CLAIM_MARK} task=other -->");
    assert!(recovery_claim(&issue(&[(&own, 100)]), "losttask123"));
    assert!(!recovery_claim(&issue(&[(&other, 100)]), "losttask123"));
    assert!(!recovery_claim(
        &issue(&[("Claimed by another agent", 100)]),
        "losttask123"
    ));
    let dir = tempfile::tempdir().unwrap();
    let (_, top) = committed_change(dir.path());
    git(
        &top,
        &["remote", "set-url", "origin", "/nonexistent/origin.git"],
    );
    assert!(recovery_commit(&top, "losttask123").is_err());
    assert!(recovery_commit(&top, "../bad").is_err());
}

#[test]
fn fmt_drift_outside_the_change_is_left_to_the_base_branch() {
    let output = "Diff in /w/crates/a/src/lib.rs:26:\n-fn x(){}\n+fn x() {}\n\
                  Diff in /w/crates/a/src/touched.rs:3:\n-let y=1;\n+let y = 1;\n";
    let staged = vec!["crates/a/src/touched.rs".to_owned()];
    let (kept, untouched) = fmt_drift_in_change(output, &staged);
    assert_eq!(untouched, 1);
    assert!(kept.starts_with("Diff in /w/crates/a/src/touched.rs:3:"));
    assert!(!kept.contains("lib.rs"));
    let (kept, untouched) = fmt_drift_in_change(output, &[]);
    assert!(kept.is_empty());
    assert_eq!(untouched, 2);
}

/// `--land queue` pushes the green change to `land/<entry id>` and submits
/// an entry with the issue and the pushed head; the integrator closes the
/// issue, not the flow (#11242).
#[test]
fn a_queued_change_is_pushed_and_submitted_and_the_issue_stays_open() {
    use super::super::land_queue;
    let dir = tempfile::tempdir().unwrap();
    let (origin, worktree) = committed_change(dir.path());
    git(&worktree, &["config", "user.name", "Test"]);
    git(&worktree, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(worktree.join("queued.txt"), "queued\n").unwrap();
    git(&worktree, &["add", "queued.txt"]);
    let queue_dir = dir.path().join("queue");
    let tracker = Arc::new(Comments::default());
    let record = local_record();
    let work = Work {
        store: dir.path().into(),
        local: Arc::new(Local::new(dir.path().into())),
        tracker: tracker.clone(),
        checks: Arc::new(NoChecks),
        policy: Policy {
            land: Land::Queue,
            ..Policy::default()
        },
        branch: "main".into(),
        top: worktree.clone(),
        now: || 100,
        artifacts: None,
        queue: Some(Arc::new(land_queue::Dir(queue_dir.clone()))),
    };
    let mut flow = flow("working");
    flow.finished = false;
    let issue = issue(&[]);
    let mut run = Run {
        work: &work,
        flow: &mut flow,
        record: &record,
        issue: &issue,
        repository: "acme/app",
        worktree: &worktree,
        turn: 1,
        summaries: vec![],
        checked: Checked::default(),
        rounds: 0,
        stranded: None,
    };
    // `Comments::close` panics, so reaching the end means the flow did not
    // close the issue.
    run.land_queue();
    let head = git(&worktree, &["rev-parse", "HEAD"]);
    let store = land_queue::Dir(queue_dir);
    let entries = land_queue::Queue { store: &store }.entries().unwrap();
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.issue, Some(42));
    assert!(entry.close);
    assert_eq!(entry.head, head);
    assert_eq!(entry.target, "main");
    assert_eq!(entry.summary, issue.title);
    assert_eq!(entry.state, land_queue::State::Queued);
    assert_eq!(entry.branch, format!("land/{}", entry.id));
    assert_eq!(
        git(
            &origin,
            &["rev-parse", &format!("refs/heads/{}", entry.branch)]
        ),
        head,
        "origin holds the queued head"
    );
    assert_eq!(
        git(&origin, &["rev-parse", "refs/heads/main"]),
        git(&worktree, &["rev-parse", "HEAD~2"]),
        "main is the integrator's to move"
    );
    assert_eq!(flow.link.outcome, "queued");
    assert_eq!(flow.link.commits, vec![head.clone()]);
    assert!(!flow.link.closed);
    assert!(flow.closing.contains(&entry.id) && flow.closing.contains(&entry.branch));
    let comments = tracker.0.lock().unwrap();
    let queued = comments.iter().find(|c| c.contains("queued this")).unwrap();
    assert!(queued.contains(&entry.id) && queued.contains(&entry.branch));
    assert!(
        !comments.iter().any(|c| c.contains(RELEASE_MARK)),
        "the claim holds"
    );
    let ended = flow.ending(result());
    assert!(matches!(ended, CoderEvent::Result(_)), "{ended:?}");
}
