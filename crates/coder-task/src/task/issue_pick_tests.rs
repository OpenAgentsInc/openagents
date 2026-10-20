use super::*;
use crate::claim::{CLAIM_MARK, RELEASE_MARK};

const NOW: u64 = 1_790_000_000;

fn open(number: u64, title: &str, body: &str) -> Open {
    Open {
        number,
        title: title.into(),
        body: body.into(),
        ..Open::default()
    }
}

fn comment(body: &str, ago: u64) -> Comment {
    Comment {
        body: body.into(),
        at: NOW - ago,
    }
}

#[test]
fn a_claimed_assigned_or_pulled_issue_is_never_picked() {
    let mut claimed = open(1, "Claimed by Coder", "x");
    claimed.comments = vec![comment(
        &format!("Claimed: Coder is working on this. {CLAIM_MARK} task=abc -->"),
        600,
    )];
    let mut by_hand = open(2, "Claimed by an agent", "x");
    by_hand.comments = vec![comment("Claimed by a Claude subagent.", 60)];
    let mut assigned = open(3, "Assigned", "x");
    assigned.assignees = vec!["someone".into()];
    let pulled = open(4, "In a pull request", "x");
    let branch = open(5, "On a branch", "x");
    let named = open(6, "Named by a pull request", "x");
    let mut held = open(7, "Blocked", "x");
    held.labels = vec!["Blocked".into()];
    let mut released = open(8, "Released", "a longer body than the others");
    released.comments = vec![
        comment(&format!("Claimed. {CLAIM_MARK} task=abc -->"), 900),
        comment(&format!("Released. {RELEASE_MARK}"), 300),
    ];
    let pulls = vec![
        Pull {
            closes: vec![4],
            ..Pull::default()
        },
        Pull {
            branch: "coder/issue-5".into(),
            ..Pull::default()
        },
        Pull {
            body: "Part of #6".into(),
            ..Pull::default()
        },
    ];
    let issues = [
        claimed, by_hand, assigned, pulled, branch, named, held, released,
    ];
    for issue in &issues[..7] {
        assert!(
            held_why(issue, &pulls).is_some(),
            "#{} is held",
            issue.number
        );
    }
    assert_eq!(
        choose(&issues, &pulls, &[], NOW, 6).unwrap(),
        [Picked {
            number: 8,
            title: "Released".into()
        }]
    );
}

fn held_why(issue: &Open, pulls: &[Pull]) -> Option<String> {
    held(issue, pulls, NOW, 6)
}

#[test]
fn an_old_claim_lapses_after_the_claim_window() {
    let mut issue = open(9, "Stale claim", "x");
    issue.comments = vec![comment("Claimed: working on it.", 7 * 3_600)];
    assert!(held_why(&issue, &[]).is_none());
}

#[test]
fn a_sized_independent_short_issue_comes_first() {
    let mut sized = open(
        30,
        "Sized",
        "A long body that names nothing else at all, really.",
    );
    sized.labels = vec!["coder-sized".into()];
    let dependent = open(10, "Depends", "Needs #20 first.");
    let independent = open(20, "Independent", "Short, with no dependency.");
    let issues = [dependent.clone(), independent.clone(), sized];
    // The coder-sized label is the pickup order without a project.
    assert_eq!(choose(&issues, &[], &[30], NOW, 6).unwrap()[0].number, 30);
    // Without a sized one, the issue whose dependency is still open waits.
    let issues = [dependent, independent];
    assert_eq!(choose(&issues, &[], &[], NOW, 6).unwrap()[0].number, 20);
}

#[test]
fn no_free_issue_says_why() {
    let mut assigned = open(1, "Assigned", "x");
    assigned.assignees = vec!["a".into()];
    let why = choose(&[assigned], &[], &[], NOW, 6).unwrap_err();
    assert!(why.contains("all 1 open issues"), "{why}");
    assert_eq!(
        choose(&[], &[], &[], NOW, 6).unwrap_err(),
        "there is no open issue"
    );
}

#[test]
fn gh_lists_parse() {
    let issues = parse_issues(
        r#"[{"number":5,"title":"T","body":"B","labels":[{"name":"coder-sized"}],
            "assignees":[{"login":"x"}],
            "comments":[{"body":"Claimed","createdAt":"2026-09-30T12:00:00Z"}]}]"#,
    )
    .unwrap();
    assert_eq!(issues[0].number, 5);
    assert_eq!(issues[0].labels, ["coder-sized"]);
    assert_eq!(issues[0].assignees, ["x"]);
    assert!(issues[0].comments[0].at > 0);
    let pulls = parse_pulls(
        r#"[{"title":"Fix","body":"","headRefName":"b","closingIssuesReferences":[{"number":5}]}]"#,
    )
    .unwrap();
    assert_eq!(pulls[0].closes, [5]);
    assert!(parse_issues("{}").is_err());
}

#[test]
fn the_repositorys_pickup_order_comes_first() {
    let first = open(
        5,
        "Fifth",
        "a body much longer than the other one, by a lot",
    );
    let second = open(9, "Ninth", "short");
    let issues = [first, second];
    // The project (or the coder-sized label) orders #5 first.
    let picked = choose(&issues, &[], &[5], NOW, 6).unwrap();
    assert_eq!(picked[0].number, 5);
    // Without an order, the shorter, older-looking one leads.
    assert_eq!(choose(&issues, &[], &[], NOW, 6).unwrap()[0].number, 9);
}
