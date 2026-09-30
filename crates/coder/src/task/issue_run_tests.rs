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
        (60, 2),
        "the defaults"
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
    // #10063: 100 steps a turn and two continuation turns (docs/cli/chat.md).
    assert_eq!((policy.max_steps, policy.continue_turns), (100, 2));
}

fn flow(outcome: &str) -> Flow {
    Flow {
        schema: FLOW_SCHEMA.into(),
        task: "t".into(),
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

fn judged(progress: f64, repeating: f64) -> Value {
    serde_json::json!({"extra": {"microcoder": {"event": {"event": "judged", "step": 1,
        "judgment": {"answers": [["done", 0.1], ["progress", progress], ["repeating", repeating]]}}}}})
}

fn ended(reason: &str) -> Value {
    serde_json::json!({"extra": {"microcoder": {"event": {"event": "ended",
        "outcome": {"ending": {"reason": reason}}}}}})
}

/// #10063: the loop's own judgments of its last steps decide whether a
/// turn that ran out of budget was progressing.
#[test]
fn momentum_reads_the_last_judged_steps_and_the_ending() {
    // Early repetition no longer counts once the last steps progress.
    let mut steps: Vec<Value> = (0..4).map(|_| judged(0.1, 0.9)).collect();
    steps.extend((0..MOMENTUM_STEPS).map(|_| judged(0.8, 0.1)));
    steps.push(serde_json::json!({"message": "no judgment"}));
    steps.push(ended("step_limit"));
    let (reason, found) = momentum(&steps);
    assert_eq!(reason.as_deref(), Some("step_limit"));
    let found = found.unwrap();
    assert_eq!(found.judged, MOMENTUM_STEPS);
    assert!(found.progressing(), "{found:?}");

    let (_, stuck) = momentum(&[judged(0.7, 0.2), judged(0.2, 0.9), judged(0.1, 0.8)]);
    assert!(!stuck.unwrap().progressing());
    // A judgment without answers (no Jev) is no signal.
    let bare = serde_json::json!({"extra": {"microcoder": {"event": {"event": "judged",
        "judgment": {"answers": []}}}}});
    assert_eq!(momentum(&[bare, ended("step_limit")]).1, None);
}

/// Only a budget ending that was progressing continues, and only within
/// the policy's bound; any other ending fails as before, with no extra
/// sentence.
#[test]
fn only_a_progressing_run_out_of_budget_continues_within_the_bound() {
    let going = Some(Momentum {
        judged: 5,
        progress: 0.8,
        repeating: 0.1,
    });
    let stuck = Some(Momentum {
        judged: 5,
        progress: 0.3,
        repeating: 0.7,
    });
    assert_eq!(continuable(Some("step_limit"), going, 0, 2), Ok(()));
    assert_eq!(continuable(Some("time_limit"), going, 1, 2), Ok(()));
    let bound = continuable(Some("step_limit"), going, 2, 2).unwrap_err();
    assert!(bound.contains("`continue_turns`"), "{bound}");
    let repeating = continuable(Some("step_limit"), stuck, 0, 2).unwrap_err();
    assert!(repeating.contains("not progressing"), "{repeating}");
    let unjudged = continuable(Some("step_limit"), None, 0, 2).unwrap_err();
    assert!(unjudged.contains("no judgment"), "{unjudged}");
    for other in [Some("idle"), Some("bad_replies"), Some("tests_held"), None] {
        assert_eq!(continuable(other, going, 0, 2), Err(String::new()));
    }
    assert_eq!(
        continuable(Some("step_limit"), going, 0, 0).map_err(|_| ()),
        Err(())
    );
}

#[test]
fn a_continuation_says_where_the_work_stands() {
    let text = continue_request(42, "step_limit", " a.rs | 3 ++-\n 1 file changed", &[]);
    assert!(
        text.starts_with("# Continue the work on issue #42"),
        "{text}"
    );
    assert!(text.contains("reached its step limit"));
    assert!(text.contains("a.rs | 3 ++-"));
    assert!(text.contains("have not run on this change yet"));
    let text = continue_request(42, "time_limit", "", &["lint: x".into()]);
    assert!(text.contains("reached its time limit"));
    assert!(text.contains("Nothing in the worktree changed yet."));
    assert!(text.contains("- lint: x"));
}
