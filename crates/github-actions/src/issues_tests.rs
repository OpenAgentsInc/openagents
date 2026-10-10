use super::fake::FakeGithub;
use super::*;

const REPO: &str = "acme/app";
const STATUSES: &[&str] = &["Todo", "In Progress", "Blocked", "Done"];

fn hub() -> FakeGithub {
    let fake = FakeGithub::new(REPO);
    fake.board(22, "V1 Launch", STATUSES);
    fake
}

#[test]
fn create_comment_close_and_reopen_an_issue() {
    let fake = hub();
    let github = Github::new(&fake, REPO).unwrap();
    let issue = github
        .create("Add the board verbs", "Body text", &["coder-sized".into()])
        .unwrap();
    assert_eq!(issue.state, "open");
    assert_eq!(issue.labels, ["coder-sized"]);
    assert!(issue.url.ends_with(&format!("/issues/{}", issue.number)));
    let link = github.comment(issue.number, "Working on it").unwrap();
    assert!(link.contains("issuecomment"));
    let closed = github
        .close(issue.number, "not-planned", Some("Not needed"))
        .unwrap();
    assert_eq!(closed.state, "closed");
    assert_eq!(closed.state_reason.as_deref(), Some("not_planned"));
    let state = fake.issue_state(issue.number).unwrap();
    assert_eq!(state.comments, ["Working on it", "Not needed"]);
    let open = github.reopen(issue.number, None).unwrap();
    assert_eq!(open.state, "open");
    assert!(github.close(issue.number, "maybe", None).is_err());
    assert!(github.create("  ", "", &[]).is_err());
    assert!(github.comment(issue.number, " ").is_err());
}

#[test]
fn list_skips_pull_requests_follows_pages_and_view_reads_comments() {
    let fake = hub();
    for number in 1..=5 {
        fake.issue(number, &format!("Issue {number}"), number != 3);
    }
    let github = Github::new(&fake, REPO).unwrap();
    let open = github.list("open", &[], 3).unwrap();
    assert_eq!(open.iter().map(|i| i.number).collect::<Vec<_>>(), [5, 4, 2]);
    let all = github.list("all", &[], 100).unwrap();
    assert_eq!(all.len(), 5);
    assert!(github.list("sideways", &[], 3).is_err());
    github.comment(2, "first").unwrap();
    let (issue, notes) = github.view(2).unwrap();
    assert_eq!(issue.body.as_deref(), Some(""));
    assert_eq!(notes[0].body, "first");
    assert!(github.view(99).unwrap_err().contains("404"));
}

#[test]
fn a_move_adds_to_the_board_when_asked_and_sets_the_status() {
    let fake = hub();
    fake.issue(7, "Seven", true);
    let github = Github::new(&fake, REPO).unwrap();
    let board = github.board(22).unwrap();
    assert_eq!(board.title, "V1 Launch");
    assert_eq!(
        github.move_on(&board, 7, "Status", "Todo", false).unwrap(),
        None
    );
    assert!(fake.writes().is_empty(), "{:?}", fake.writes());
    let moved = github
        .move_on(&board, 7, "status", "in progress", true)
        .unwrap()
        .unwrap();
    assert!(moved.added);
    assert_eq!(moved.to, "In Progress");
    assert_eq!(fake.status(22, 7), Some(Some("In Progress".into())));
    let moved = github
        .move_on(&board, 7, "Status", "Done", false)
        .unwrap()
        .unwrap();
    assert_eq!(moved.from.as_deref(), Some("In Progress"));
    assert!(!moved.added);
    let wrong = github
        .move_on(&board, 7, "Status", "Shipped", true)
        .unwrap_err();
    assert!(wrong.contains("\"Done\""), "{wrong}");
    // Only REST: no GraphQL path is ever sent.
    assert!(fake.calls().iter().all(|call| !call.contains("graphql")));
}

#[test]
fn items_by_status_keep_board_order_and_this_repository() {
    let fake = hub();
    for number in [1, 2, 3] {
        fake.issue(number, &format!("Issue {number}"), true);
    }
    fake.place(22, REPO, 1, Some("Todo"));
    fake.place(22, "acme/other", 2, Some("Todo"));
    fake.place(22, REPO, 3, None);
    fake.place(22, REPO, 2, Some("Done"));
    let github = Github::new(&fake, REPO).unwrap();
    let board = github.board(22).unwrap();
    let todo = github.items(&board, "Status", Some("todo"), false).unwrap();
    assert_eq!(todo.len(), 1);
    assert_eq!(todo[0].number, Some(1));
    let every = github.items(&board, "Status", Some("Todo"), true).unwrap();
    assert_eq!(every.len(), 2);
    let none = github.items(&board, "Status", Some("none"), false).unwrap();
    assert_eq!(none[0].number, Some(3));
    let all = github.items(&board, "Status", None, false).unwrap();
    assert_eq!(
        all.iter().map(|i| i.number.unwrap()).collect::<Vec<_>>(),
        [1, 3, 2]
    );
    assert!(github.items(&board, "Status", Some("Nope"), false).is_err());
}

#[test]
fn closing_moves_the_issue_to_done_on_every_board_it_is_on() {
    let fake = hub();
    fake.board(19, "Coder", &["Todo", "Done"]);
    fake.board(5, "No done here", &["Todo"]);
    fake.issue(8, "Eight", true);
    fake.place(22, REPO, 8, Some("In Progress"));
    fake.place(5, REPO, 8, Some("Todo"));
    let github = Github::new(&fake, REPO).unwrap();
    github.close(8, "completed", None).unwrap();
    let (moved, skipped) = github.move_everywhere(8, "Status", "Done").unwrap();
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0].project, 22);
    assert_eq!(fake.status(22, 8), Some(Some("Done".into())));
    assert_eq!(fake.status(19, 8), None, "not added to a board it was off");
    assert_eq!(skipped.len(), 1);
    assert!(skipped[0].contains("project 5"));
}

#[test]
fn a_token_without_the_project_scope_is_told_how_to_fix_it() {
    let mut fake = hub();
    fake.no_project_scope = true;
    let github = Github::new(&fake, REPO).unwrap();
    let why = github.board(22).unwrap_err();
    assert!(why.contains("gh auth refresh -s project"), "{why}");
    assert!(Github::new(&fake, "nope").is_err());
}
