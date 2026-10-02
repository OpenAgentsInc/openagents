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

impl Tracker for Labels {
    fn repository(&self, _: &Path) -> Result<String, String> {
        Ok("acme/app".into())
    }
    fn issue(&self, _: &str, _: u64) -> Result<Issue, String> {
        Err("unused".into())
    }
    fn comment(&self, _: &str, _: u64, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn close(&self, _: &str, _: u64) -> Result<(), String> {
        Ok(())
    }
    fn labeled(&self, _: &str, label: &str) -> Result<Vec<u64>, String> {
        Ok(if label == "coder-ok" {
            vec![3, 9]
        } else {
            vec![]
        })
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
    assert!(Land::parse("force").is_err());
}

/// This repository's own policy lands on main after the checks.
#[test]
fn this_repository_lands_on_main_with_its_checks() {
    let top = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let policy = Policy::load(&top).unwrap();
    assert_eq!(policy.land, Land::Main);
    assert!(policy.fmt && policy.clippy);
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
    assert_eq!(iso_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(iso_seconds("2026-09-30T12:00:00Z"), Some(1_790_769_600));
    assert_eq!(iso_seconds("nope"), None);
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
impl Tracker for Comments {
    fn repository(&self, _: &Path) -> Result<String, String> {
        Ok("acme/app".into())
    }
    fn issue(&self, _: &str, _: u64) -> Result<Issue, String> {
        Ok(issue(&[]))
    }
    fn comment(&self, _: &str, _: u64, body: &str) -> Result<(), String> {
        self.0.lock().unwrap().push(body.into());
        Ok(())
    }
    fn close(&self, _: &str, _: u64) -> Result<(), String> {
        panic!("must not close")
    }
    fn labeled(&self, _: &str, _: &str) -> Result<Vec<u64>, String> {
        Ok(vec![])
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
