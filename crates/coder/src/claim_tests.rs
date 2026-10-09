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
fn a_claim_adds_an_issue_missing_from_the_numbered_project_and_moves_it() {
    let github = Fake::with_project("octo", &["Todo", "In progress", "Done"]);
    github.issue(9, None, &[], &[]);
    let numbered = Project {
        number: Some(19),
        ..Project::default()
    };
    let said = claim(&github, REPO, 9, &body(), &numbered);
    let state = github.state(9);
    assert!(state.on_project, "the claim put the issue on the project");
    assert_eq!(state.status.as_deref(), Some("In progress"));
    assert!(said.contains(&"Moved #9 to \"In progress\" on the project \"Board\".".to_owned()));

    // Without a numbered project, an issue off every project stays off.
    github.issue(10, None, &[], &[]);
    claim(&github, REPO, 10, &body(), &Project::default());
    assert!(!github.state(10).on_project);
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
fn blocked_says_why_and_moves_to_blocked_keeping_the_claim() {
    let github = Fake::with_project("octo", &["Todo", "In Progress", "Blocked", "Done"]);
    github.issue(12, Some("Todo"), &[], &[]);
    let project = Project::default();
    claim(&github, REPO, 12, &body(), &project);
    let said = blocked(
        &github,
        REPO,
        12,
        "waits on the owner's App Store key",
        &project,
    );
    let state = github.state(12);
    assert_eq!(state.status.as_deref(), Some("Blocked"));
    assert_eq!(
        state.assignees,
        vec!["octo".to_owned()],
        "the assignee stays"
    );
    assert_eq!(
        state.comments.last().unwrap().body,
        "Blocked: waits on the owner's App Store key"
    );
    assert!(said.contains(&"Moved #12 to \"Blocked\" on the project \"Board\".".to_owned()));
    done(&github, REPO, 12, &project);
    assert_eq!(github.state(12).status.as_deref(), Some("Done"));
}

#[test]
fn the_rest_answers_find_the_issue_its_status_and_options() {
    let fields: Value = serde_json::from_str(
        r#"[{"id":1,"name":"Title"},
            {"id":423676229,"name":"Status","data_type":"single_select","options":[
              {"id":"ef9785a7","name":{"html":"Todo","raw":"Todo"}},
              {"id":"4af1b929","name":{"html":"In Progress","raw":"In Progress"}},
              {"id":"65989dc5","name":{"html":"Blocked","raw":"Blocked"}}]}]"#,
    )
    .unwrap();
    let (id, options) = rest_field(&fields, "status").unwrap();
    assert_eq!(id, "423676229");
    assert_eq!(options[2], ("Blocked".to_owned(), "65989dc5".to_owned()));
    assert_eq!(rest_field(&fields, "Priority"), None);

    // A search for 11108 also finds #111080 and another repository's #11108.
    let items: Value = serde_json::from_str(
        r#"[{"id":5,"content":{"number":111080,"repository_url":"https://api.github.com/repos/Acme/App"}},
            {"id":6,"content":{"number":11108,"repository_url":"https://api.github.com/repos/acme/other"}},
            {"id":267147115,"updated_at":"2026-10-09T15:55:54Z",
             "content":{"number":11108,"repository_url":"https://api.github.com/repos/Acme/App"},
             "fields":[{"id":423676229,"name":"Status","value":{"id":"ef9785a7","name":{"raw":"Todo"}}}]}]"#,
    )
    .unwrap();
    let (item, status, at) = rest_item(&items, "acme/app", 11108, "Status").unwrap();
    assert_eq!(item, "267147115");
    assert_eq!(status.as_deref(), Some("Todo"));
    assert_eq!(at, iso_seconds("2026-10-09T15:55:54Z").unwrap());
    assert_eq!(rest_item(&items, "acme/app", 42, "Status"), None);
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

fn claimant(session: &str, pid: Option<u32>) -> coder_lease::claims::Claimant {
    coder_lease::claims::Claimant {
        session: session.into(),
        agent: "codex".into(),
        pid,
    }
}

fn session_body(session: &str) -> String {
    format!(
        "Claimed: an agent is working on this.\n\n{}",
        marker("cli", session)
    )
}

/// A process that has exited, for a session that ended.
fn ended_pid() -> u32 {
    let mut child = std::process::Command::new("true").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    pid
}

#[test]
fn markers_carry_the_session_and_old_markers_still_parse() {
    let new = session_body("claude-code:abc 1");
    assert!(new.contains("session=claude-code:abc_1 -->"), "{new}");
    assert_eq!(marker_field(&new, "session"), Some("claude-code:abc_1"));
    let task = format!("Claimed. {} ", marker("task=t1", "s1"));
    assert_eq!(marker_field(&task, "task"), Some("t1"));
    assert_eq!(marker_field(&task, "session"), Some("s1"));
    let old = format!("Claimed. {CLAIM_MARK} task=t1 -->");
    assert_eq!(marker_field(&old, "task"), Some("t1"));
    assert_eq!(marker_field(&old, "session"), None);
    let comments = [Comment { body: old, at: 900 }];
    assert!(active(&comments).is_some());
    assert!(foreign_marker(7, &comments, 1_000, 6, "s1", None).is_some());
}

#[test]
fn a_second_session_is_refused_the_same_one_reclaims_and_an_ended_one_is_taken_over() {
    let leases = tempfile::tempdir().unwrap();
    let root = leases.path();
    let github = Fake::plain("octo");
    github.issue(7, None, &[], &[]);
    github.issue(8, None, &[], &[]);
    let project = Project::default();
    let now = github.now;
    let alive = claimant("codex:a", Some(std::process::id()));
    let mine = session_body("codex:a");
    let said = take(
        &github, root, REPO, 7, &mine, &project, &alive, now, 6, false,
    )
    .unwrap();
    assert_eq!(said[0], "Holding #7 for session codex:a.");
    assert!(
        github.state(7).comments[0]
            .body
            .contains("session=codex:a -->")
    );

    // Another session is refused, naming the holder and its age.
    let other = claimant("claude-code:b", None);
    let theirs = session_body("claude-code:b");
    let why = take(
        &github,
        root,
        REPO,
        7,
        &theirs,
        &project,
        &other,
        now + 120,
        6,
        false,
    )
    .unwrap_err();
    assert!(
        why.contains("session codex:a") && why.contains("2 minutes ago"),
        "{why}"
    );
    assert_eq!(
        github.state(7).comments.len(),
        1,
        "a refused claim posts nothing"
    );
    // And so is its release.
    let release_body = Some(RELEASE_MARK);
    assert!(
        give_back(
            &github,
            root,
            REPO,
            7,
            release_body,
            &project,
            "claude-code:b",
            now,
            6,
            false
        )
        .is_err()
    );

    // The same session claims again.
    let said = take(
        &github,
        root,
        REPO,
        7,
        &mine,
        &project,
        &alive,
        now + 60,
        6,
        false,
    )
    .unwrap();
    assert!(said[0].starts_with("Renewed"), "{said:?}");

    // A session whose process ended is taken over, its fresh marker too.
    let dead = claimant("codex:dead", Some(ended_pid()));
    let body = session_body("codex:dead");
    take(
        &github, root, REPO, 8, &body, &project, &dead, now, 6, false,
    )
    .unwrap();
    let said = take(
        &github,
        root,
        REPO,
        8,
        &theirs,
        &project,
        &other,
        now + 10,
        6,
        false,
    )
    .unwrap();
    assert!(said[0].contains("over from session codex:dead"), "{said:?}");
    let record = coder_lease::claims::read(root, REPO, 8).unwrap().unwrap();
    assert_eq!(record.session, "claude-code:b");

    // Release drops the hold and posts the release; the issue is free.
    let said = give_back(
        &github,
        root,
        REPO,
        7,
        release_body,
        &project,
        "codex:a",
        now,
        6,
        false,
    )
    .unwrap();
    assert!(said[0].starts_with("Dropped session codex:a"), "{said:?}");
    assert!(coder_lease::claims::read(root, REPO, 7).unwrap().is_none());
    take(
        &github,
        root,
        REPO,
        7,
        &theirs,
        &project,
        &other,
        now + 200,
        6,
        false,
    )
    .unwrap();
}

#[test]
fn a_fresh_marker_from_another_session_refuses_until_it_ages_or_force() {
    let leases = tempfile::tempdir().unwrap();
    let root = leases.path();
    let github = Fake::plain("octo");
    github.issue(9, None, &[], &[]);
    github
        .comment(REPO, 9, &session_body("elsewhere:1"))
        .unwrap();
    let project = Project::default();
    let me = claimant("codex:a", Some(std::process::id()));
    let mine = session_body("codex:a");
    let why = take(
        &github, root, REPO, 9, &mine, &project, &me, 1_100, 6, false,
    )
    .unwrap_err();
    assert!(why.contains("session elsewhere:1"), "{why}");
    assert!(coder_lease::claims::read(root, REPO, 9).unwrap().is_none());
    // Past the claim window it is free.
    let later = 1_000 + 6 * 3_600;
    take(
        &github, root, REPO, 9, &mine, &project, &me, later, 6, false,
    )
    .unwrap();
    // A forced claim takes even a live hold.
    let other = claimant("claude-code:b", None);
    let theirs = session_body("claude-code:b");
    take(
        &github,
        root,
        REPO,
        9,
        &theirs,
        &project,
        &other,
        later + 10,
        6,
        true,
    )
    .unwrap();
}
