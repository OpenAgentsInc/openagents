use super::fake::Fake;
use super::*;

const REPO: &str = "acme/app";

fn body() -> String {
    format!("Claimed: an agent is working on this.\n\n{CLAIM_MARK} task=t1 -->")
}

#[test]
fn without_a_project_a_claim_is_the_comment_and_the_assignee() {
    let github = Fake::plain("octo");
    github.issue(7, None, &["coder-sized"], &[]);
    let project = Project::default();
    let said = claim(&github, REPO, 7, &body(), &project);
    let state = github.state(7);
    assert_eq!(state.comments.len(), 1);
    assert!(state.comments[0].body.contains(CLAIM_MARK));
    assert_eq!(state.assignees, vec!["octo".to_owned()]);
    assert_eq!(state.status, None);
    assert_eq!(
        said,
        vec![
            "Claimed #7 with a comment on the issue.".to_owned(),
            "Assigned #7 to @octo.".to_owned()
        ]
    );
    // The comment is the claim every path reads.
    let comments = github.comments(REPO, 7).unwrap();
    let items = github.items(REPO, 7, &project.field).unwrap();
    assert!(held(7, &comments, &items, 1_100, 6, &project).is_some());

    let release_body = format!("Released.\n\n{RELEASE_MARK}");
    release(&github, REPO, 7, Some(&release_body), &project);
    let state = github.state(7);
    assert!(state.assignees.is_empty(), "release clears the assignee");
    assert_eq!(state.comments.len(), 2);
    assert_eq!(held(7, &state.comments, &[], 1_200, 6, &project), None);
}

#[test]
fn with_a_project_a_claim_moves_the_status_and_release_moves_it_back() {
    // The project spells its values its own way; names match without case.
    let github = Fake::with_project("octo", &["Todo", "In Progress", "Done"]);
    github.issue(8, Some("Todo"), &[], &[]);
    let project = Project::default();
    let said = claim(&github, REPO, 8, &body(), &project);
    let state = github.state(8);
    assert_eq!(state.status.as_deref(), Some("In Progress"));
    assert_eq!(state.assignees, vec!["octo".to_owned()]);
    assert!(said.contains(&"Moved #8 to \"In Progress\" on the project \"Board\".".to_owned()));

    // Release goes back to the first ready value the project has (no
    // "Ready" here, so "Todo").
    release(&github, REPO, 8, Some(RELEASE_MARK), &project);
    let state = github.state(8);
    assert_eq!(state.status.as_deref(), Some("Todo"));
    assert!(state.assignees.is_empty());

    claim(&github, REPO, 8, &body(), &project);
    done(&github, REPO, 8, &project);
    let state = github.state(8);
    assert_eq!(state.status.as_deref(), Some("Done"));
    assert_eq!(
        state.assignees,
        vec!["octo".to_owned()],
        "done keeps who did it"
    );
}

#[test]
fn an_in_progress_status_is_a_claim_until_released_or_old() {
    let project = Project::default();
    let item = |status: &str, at: u64| Item {
        project_title: "Board".into(),
        status: Some(status.into()),
        status_at: at,
        ..Item::default()
    };
    let now = 100_000;
    // Someone moved it to In Progress ten minutes ago, with no comment.
    let why = held(42, &[], &[item("In Progress", now - 600)], now, 6, &project).unwrap();
    assert!(
        why.starts_with("#42 is \"In Progress\" on the project \"Board\""),
        "{why}"
    );
    // Other statuses, an old status, or a release after it hold nothing.
    assert_eq!(
        held(42, &[], &[item("Ready", now - 600)], now, 6, &project),
        None
    );
    assert_eq!(
        held(
            42,
            &[],
            &[item("In progress", now - 7 * 3_600)],
            now,
            6,
            &project
        ),
        None
    );
    let released = [Comment {
        body: RELEASE_MARK.into(),
        at: now - 60,
    }];
    assert_eq!(
        held(
            42,
            &released,
            &[item("In progress", now - 600)],
            now,
            6,
            &project
        ),
        None
    );
}

#[test]
fn configured_names_are_honoured() {
    let github = Fake::with_project("octo", &["Backlog", "Doing", "Shipped"]);
    github.issue(9, Some("Backlog"), &[], &[]);
    let project: Project = serde_json::from_str(
        r#"{"in_progress": "Doing", "ready": ["Backlog"], "done": "Shipped"}"#,
    )
    .unwrap();
    assert_eq!(project.field, "Status");
    claim(&github, REPO, 9, &body(), &project);
    assert_eq!(github.state(9).status.as_deref(), Some("Doing"));
    release(&github, REPO, 9, None, &project);
    assert_eq!(github.state(9).status.as_deref(), Some("Backlog"));
    // A project without the configured value says so and keeps its status.
    let said = done(&github, REPO, 9, &Project::default());
    assert_eq!(github.state(9).status.as_deref(), Some("Backlog"));
    assert!(said[0].contains("has no Status value \"Done\""), "{said:?}");
}

#[test]
fn pickup_uses_the_project_order_status_and_blockers() {
    let github = Fake::with_project("octo", &["Todo", "Ready", "In progress", "Done"]);
    // Project order is insertion order: 30, 10, 20, 40, 50.
    github.issue(30, Some("Ready"), &[], &[]);
    github.issue(10, Some("Todo"), &["coder-sized"], &[]);
    github.issue(20, Some("Ready"), &["coder-sized"], &[50]);
    github.issue(40, Some("In progress"), &["coder-sized"], &[]);
    github.issue(50, Some("Done"), &[], &[]);
    // A labeled issue the project does not hold.
    github.issue(5, None, &["coder-sized"], &[]);
    github.issues.lock().unwrap().get_mut(&50).unwrap().open = false;
    let project = Project::default();
    let (order, from) = pickup(&github, REPO, &project, None).unwrap();
    // 20's blocker is closed, 40 is claimed on the board, 50 is done.
    assert_eq!(order, vec![30, 10, 20]);
    assert!(from.starts_with("the project's order"), "{from}");
    // An open blocker keeps an issue out.
    github.issues.lock().unwrap().get_mut(&50).unwrap().open = true;
    assert_eq!(
        pickup(&github, REPO, &project, None).unwrap().0,
        vec![30, 10]
    );
    // A label narrows the project's order, then adds labeled issues off it.
    assert_eq!(
        pickup(&github, REPO, &project, Some("coder-sized"))
            .unwrap()
            .0,
        vec![10, 5]
    );
}

#[test]
fn pickup_without_a_project_reads_the_label_in_issue_order() {
    let github = Fake::plain("octo");
    github.issue(12, None, &["coder-sized"], &[]);
    github.issue(3, None, &["coder-sized"], &[]);
    github.issue(4, None, &["other"], &[]);
    let (order, from) = pickup(&github, REPO, &Project::default(), None).unwrap();
    assert_eq!(order, vec![3, 12]);
    assert_eq!(from, "issues labeled `coder-sized`, oldest first");
}

#[test]
fn the_items_answer_parses_status_options_and_time() {
    let value: Value = serde_json::from_str(
        r#"{"data":{"repository":{"issue":{"projectItems":{"nodes":[
          {"id":"PVTI_1","project":{"id":"PVT_1","title":"Gym","closed":false,
            "field":{"id":"F1","options":[{"id":"a","name":"Todo"},{"id":"b","name":"In Progress"}]}},
            "fieldValueByName":{"name":"Todo","updatedAt":"2026-09-22T14:01:09Z"}},
          {"id":"PVTI_2","project":{"id":"PVT_2","title":"Old","closed":true,"field":null},
            "fieldValueByName":null}]}}}}}"#,
    )
    .unwrap();
    let items = items_of(&value);
    assert_eq!(items.len(), 1, "a closed project is not read");
    assert_eq!(items[0].field.as_deref(), Some("F1"));
    assert_eq!(items[0].option("in progress").unwrap().1, "b");
    assert_eq!(items[0].status.as_deref(), Some("Todo"));
    assert_eq!(
        items[0].status_at,
        iso_seconds("2026-09-22T14:01:09Z").unwrap()
    );
}

#[test]
fn the_board_answer_keeps_project_order_and_reads_blockers() {
    let value: Value = serde_json::from_str(
        r#"{"nodes":[
          {"fieldValueByName":{"name":"Ready"},"content":{"__typename":"Issue","number":9,"state":"OPEN",
            "repository":{"nameWithOwner":"acme/app"},"labels":{"nodes":[{"name":"x"}]},
            "blockedBy":{"pageInfo":{"hasNextPage":false},"nodes":[{"state":"OPEN"}]}}},
          {"fieldValueByName":null,"content":{"__typename":"PullRequest"}},
          {"fieldValueByName":{"name":"Todo"},"content":{"__typename":"Issue","number":2,"state":"OPEN",
            "repository":{"nameWithOwner":"other/repo"},"labels":{"nodes":[]},
            "blockedBy":{"pageInfo":{"hasNextPage":false},"nodes":[]}}},
          {"fieldValueByName":{"name":"Todo"},"content":{"__typename":"Issue","number":1,"state":"OPEN",
            "repository":{"nameWithOwner":"acme/app"},"labels":{"nodes":[]},
            "blockedBy":{"pageInfo":{"hasNextPage":false},"nodes":[{"state":"CLOSED"}]}}}]}"#,
    )
    .unwrap();
    let board = board_of(&value, REPO);
    assert_eq!(
        board
            .iter()
            .map(|item| (item.number, item.blocked))
            .collect::<Vec<_>>(),
        vec![(9, true), (1, false)]
    );
}

#[test]
fn a_token_without_the_project_scope_is_said_once_and_the_claim_goes_on() {
    let mut github = Fake::with_project("octo", &["Todo", "In Progress", "Done"]);
    github.unreadable =
        Some("Your token has not been granted the required scopes: read:project".to_owned());
    github.issue(9, Some("Todo"), &[], &[]);
    let project = Project::default();
    let repository = "acme/unreadable-board";
    let said = claim(&github, repository, 9, &body(), &project);
    let state = github.state(9);
    assert_eq!(state.comments.len(), 1, "the claim comment still lands");
    assert_eq!(state.assignees, vec!["octo".to_owned()]);
    assert_eq!(state.status.as_deref(), Some("Todo"), "the status stays");
    let board: Vec<_> = said
        .iter()
        .filter(|line| line.contains("project board"))
        .collect();
    assert_eq!(board.len(), 1, "{said:?}");
    assert!(board[0].contains("`project` scope"), "{}", board[0]);

    // Release and close say nothing more about it.
    let said = release(&github, repository, 9, Some(RELEASE_MARK), &project);
    assert!(
        !said.iter().any(|line| line.contains("project")),
        "{said:?}"
    );
}
