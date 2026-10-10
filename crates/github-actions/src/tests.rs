//! Forms and command words read into exact actions, the confirm card says
//! every change, and running asks GitHub's REST API for exactly the
//! changes the card showed, stopping before any change when the
//! connection lacks the access (boards need `project`).

use std::sync::Mutex;

use serde_json::{Value, json};

use crate::action::{self, Action, Board, CloseReason, Fields, Tool};
use crate::argv::ArgvError;
use crate::rest::{self as run, Answer, Api, Failure, Method};

fn form(tool: &str, fields: &[(&str, &str)]) -> Fields {
    let mut form = Fields {
        tool: tool.to_string(),
        ..Fields::default()
    };
    for (name, value) in fields {
        let value = (*value).to_string();
        match *name {
            "repository" => form.repository = value,
            "title" => form.title = value,
            "body" => form.body = value,
            "number" => form.number = value,
            "board" => form.board = value,
            "status" => form.status = value,
            "head" => form.head = value,
            "base" => form.base = value,
            "comment" => form.comment = value,
            "draft" => form.draft = Some(value),
            other => panic!("no field {other}"),
        }
    }
    form
}

fn issue_on_board() -> Action {
    Action::CreateIssue {
        repository: "OpenAgentsInc/openagents".into(),
        title: "Fix the login".into(),
        body: "It loops.".into(),
        labels: Vec::new(),
        board: Some(Board {
            number: 22,
            status: "Todo".into(),
        }),
    }
}

#[test]
fn forms_read_into_exact_actions_or_say_what_to_fix() {
    let action = Action::from_fields(&form(
        "create_issue",
        &[
            (
                "repository",
                "https://github.com/OpenAgentsInc/openagents.git",
            ),
            ("title", "  Fix the login "),
            ("body", "It loops.\r\n"),
            ("board", "#22"),
            ("status", "Todo"),
        ],
    ))
    .unwrap();
    assert_eq!(action, issue_on_board());
    assert!(action.needs_board());
    assert!(action.valid());

    // No board named: just the issue, and no boards access needed.
    let plain = Action::from_fields(&form(
        "create_issue",
        &[("repository", "a/b"), ("title", "T"), ("status", "Todo")],
    ))
    .unwrap();
    assert!(!plain.needs_board());

    let closed = Action::from_fields(&form(
        "close_issue",
        &[("repository", "a/b"), ("number", "#7"), ("comment", "  ")],
    ))
    .unwrap();
    assert_eq!(
        closed,
        Action::CloseIssue {
            repository: "a/b".into(),
            number: 7,
            comment: None,
            reason: CloseReason::Completed,
        }
    );

    let pull = Action::from_fields(&form(
        "open_pull_request",
        &[
            ("repository", "a/b"),
            ("head", "fix-login"),
            ("title", "Fix login"),
            ("draft", "1"),
        ],
    ))
    .unwrap();
    assert_eq!(
        pull,
        Action::OpenPullRequest {
            repository: "a/b".into(),
            head: "fix-login".into(),
            base: None,
            title: "Fix login".into(),
            body: String::new(),
            draft: true,
        }
    );

    for (bad, fields) in [
        ("tickle", vec![("repository", "a/b")]),
        (
            "create_issue",
            vec![("repository", "not a repo"), ("title", "T")],
        ),
        ("create_issue", vec![("repository", "a/b"), ("title", "")]),
        (
            "create_issue",
            vec![("repository", "a/b"), ("title", "a\nb")],
        ),
        (
            "comment",
            vec![("repository", "a/b"), ("number", "3"), ("body", " ")],
        ),
        (
            "comment",
            vec![("repository", "a/b"), ("number", "0"), ("body", "x")],
        ),
        (
            "move_on_board",
            vec![("repository", "a/b"), ("number", "3"), ("status", "Todo")],
        ),
        (
            "move_on_board",
            vec![
                ("repository", "a/b"),
                ("number", "3"),
                ("board", "22"),
                ("status", ""),
            ],
        ),
        (
            "open_pull_request",
            vec![("repository", "a/b"), ("head", "a..b"), ("title", "T")],
        ),
        (
            "open_pull_request",
            vec![("repository", "a/b"), ("head", "-x"), ("title", "T")],
        ),
    ] {
        assert!(
            Action::from_fields(&form(bad, &fields)).is_err(),
            "{bad} {fields:?} should be refused"
        );
    }
    let long = "x".repeat(action::MAX_BODY + 1);
    assert!(
        Action::from_fields(&form(
            "comment",
            &[("repository", "a/b"), ("number", "3"), ("body", &long)]
        ))
        .is_err()
    );
}

#[test]
fn the_card_says_the_repository_and_every_change_in_order() {
    let card = issue_on_board().card();
    assert_eq!(card.heading, Tool::CreateIssue.name());
    assert_eq!(card.repository, "OpenAgentsInc/openagents");
    assert_eq!(
        card.changes,
        vec![
            "Opens a new issue titled \u{201c}Fix the login\u{201d}.".to_string(),
            "Puts it on board 22 with the status Todo.".to_string(),
        ]
    );
    assert_eq!(card.text.as_deref(), Some("It loops."));

    let close = Action::CloseIssue {
        repository: "a/b".into(),
        number: 9,
        comment: Some("Done in #10.".into()),
        reason: CloseReason::Completed,
    }
    .card();
    assert_eq!(
        close.changes,
        vec![
            "Adds a comment to #9.".to_string(),
            "Closes #9 as completed.".to_string()
        ]
    );
}

/// GitHub's REST API, answering from a script and keeping every call.
struct Fake {
    scopes: Option<Vec<String>>,
    calls: Mutex<Vec<(Method, String, Option<Value>)>>,
    answer: fn(&Method, &str) -> (u16, Value),
}

impl Fake {
    fn new(scopes: Option<&[&str]>, answer: fn(&Method, &str) -> (u16, Value)) -> Self {
        Self {
            scopes: scopes.map(|s| s.iter().map(|s| (*s).to_string()).collect()),
            calls: Mutex::new(Vec::new()),
            answer,
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .map(|(method, path, _)| format!("{method} {path}"))
            .collect()
    }

    fn sent(&self, call: &str) -> Value {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .find(|(method, path, _)| format!("{method} {path}") == call)
            .and_then(|(_, _, body)| body.clone())
            .unwrap_or(Value::Null)
    }
}

impl Api for Fake {
    fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> impl std::future::Future<Output = Result<Answer, Failure>> + Send {
        let (status, answer) = (self.answer)(&method, path);
        self.calls
            .lock()
            .unwrap()
            .push((method, path.to_string(), body));
        let scopes = self.scopes.clone();
        async move {
            Ok(Answer {
                status,
                scopes,
                body: answer,
            })
        }
    }
}

fn board_github(method: &Method, path: &str) -> (u16, Value) {
    match (method.as_str(), path) {
        ("GET", "/user") => (200, json!({"login": "octo"})),
        ("POST", "/repos/OpenAgentsInc/openagents/issues") => (
            201,
            json!({"number": 501, "id": 9001, "html_url": "https://github.com/OpenAgentsInc/openagents/issues/501"}),
        ),
        ("GET", "/orgs/OpenAgentsInc/projectsV2/22") => (200, json!({"number": 22})),
        ("GET", "/orgs/OpenAgentsInc/projectsV2/22/fields?per_page=100") => (
            200,
            json!([
                {"id": 1, "name": "Title"},
                {"id": 7, "name": "Status", "options": [
                    {"id": "f75ad846", "name": {"raw": "Todo"}},
                    {"id": "47fc9ee4", "name": {"raw": "In Progress"}},
                ]},
            ]),
        ),
        ("GET", "/orgs/OpenAgentsInc/projectsV2/22/items?per_page=100&q=12") => (
            200,
            json!([
                {"id": 3, "content": {"number": 12, "repository_url": "https://api.github.com/repos/someone/else"}},
                {"id": 4, "content": {"number": 12, "repository_url": "https://api.github.com/repos/OpenAgentsInc/openagents"}},
            ]),
        ),
        ("POST", "/orgs/OpenAgentsInc/projectsV2/22/items") => (201, json!({"id": 77})),
        ("PATCH", _) => (200, json!({"id": 77})),
        _ => (404, json!({"message": "Not Found"})),
    }
}

#[tokio::test]
async fn opening_an_issue_on_a_board_adds_it_and_sets_the_status() {
    let github = Fake::new(Some(&["repo", "read:org", "project"]), board_github);
    let done = run::run(&github, &issue_on_board()).await.unwrap();
    assert_eq!(
        done.summary,
        "Opened #501 in OpenAgentsInc/openagents and put it on board 22 as Todo."
    );
    assert_eq!(
        done.link.as_deref(),
        Some("https://github.com/OpenAgentsInc/openagents/issues/501")
    );
    assert_eq!(done.problem, None);
    assert_eq!(
        github.calls(),
        vec![
            "GET /user",
            "POST /repos/OpenAgentsInc/openagents/issues",
            "GET /orgs/OpenAgentsInc/projectsV2/22",
            "GET /orgs/OpenAgentsInc/projectsV2/22/fields?per_page=100",
            "POST /orgs/OpenAgentsInc/projectsV2/22/items",
            "PATCH /orgs/OpenAgentsInc/projectsV2/22/items/77",
        ]
    );
    assert_eq!(
        github.sent("POST /orgs/OpenAgentsInc/projectsV2/22/items"),
        json!({"type": "Issue", "id": 9001})
    );
    assert_eq!(
        github.sent("PATCH /orgs/OpenAgentsInc/projectsV2/22/items/77"),
        json!({"fields": [{"id": 7, "value": "f75ad846"}]})
    );
}

#[tokio::test]
async fn moving_an_issue_already_on_the_board_sets_its_item() {
    let github = Fake::new(Some(&["repo", "project"]), board_github);
    let action = Action::MoveOnBoard {
        repository: "OpenAgentsInc/openagents".into(),
        number: 12,
        board: Board {
            number: 22,
            status: "in progress".into(),
        },
    };
    run::run(&github, &action).await.unwrap();
    assert_eq!(
        github.calls().last().map(String::as_str),
        Some("PATCH /orgs/OpenAgentsInc/projectsV2/22/items/4")
    );
    assert_eq!(
        github.sent("PATCH /orgs/OpenAgentsInc/projectsV2/22/items/4"),
        json!({"fields": [{"id": 7, "value": "47fc9ee4"}]})
    );

    // A status the board doesn't have names the ones it does.
    let action = Action::MoveOnBoard {
        repository: "OpenAgentsInc/openagents".into(),
        number: 12,
        board: Board {
            number: 22,
            status: "Shipped".into(),
        },
    };
    let failure = run::run(&github, &action).await.unwrap_err();
    assert_eq!(
        failure.text(),
        "Board 22 has no status Shipped. Its statuses are Todo, In Progress."
    );
}

#[tokio::test]
async fn without_the_access_nothing_changes_and_the_grant_is_asked_for() {
    // Boards need `project`: only `GET /user` is called.
    let github = Fake::new(Some(&["repo", "read:org"]), board_github);
    assert_eq!(
        run::run(&github, &issue_on_board()).await,
        Err(Failure::NeedsAccess { board: true })
    );
    assert_eq!(github.calls(), vec!["GET /user"]);

    // Sign-in alone (no `repo`) can't write at all.
    let github = Fake::new(Some(&["read:user"]), board_github);
    let comment = Action::Comment {
        repository: "OpenAgentsInc/openagents".into(),
        number: 3,
        body: "Thanks".into(),
    };
    assert_eq!(
        run::run(&github, &comment).await,
        Err(Failure::NeedsAccess { board: false })
    );
    assert_eq!(github.calls(), vec!["GET /user"]);

    // A GitHub App's connection names no scopes and goes straight on.
    let github = Fake::new(None, board_github);
    assert!(run::run(&github, &issue_on_board()).await.is_ok());
}

#[tokio::test]
async fn an_issue_opened_when_the_board_move_fails_says_so() {
    fn no_board(method: &Method, path: &str) -> (u16, Value) {
        if path.contains("projectsV2") {
            (404, json!({"message": "Not Found"}))
        } else {
            board_github(method, path)
        }
    }
    let github = Fake::new(Some(&["repo", "project"]), no_board);
    let done = run::run(&github, &issue_on_board()).await.unwrap();
    assert_eq!(done.summary, "Opened #501 in OpenAgentsInc/openagents.");
    assert_eq!(
        done.problem.as_deref(),
        Some("It isn't on board 22 yet: GitHub couldn't find board 22 for OpenAgentsInc.")
    );
    assert!(
        github
            .calls()
            .contains(&"GET /users/OpenAgentsInc/projectsV2/22".to_string())
    );
}

fn words(line: &[&str]) -> Vec<String> {
    line.iter().map(|w| (*w).to_string()).collect()
}

#[test]
fn command_words_read_into_the_same_actions_and_back() {
    // `issue create` with a board, as the chat router proposes it: no
    // --repo, so the chat's project fills it.
    let proposed = words(&[
        "issue",
        "create",
        "--title",
        "Fix the login",
        "--body",
        "It loops.",
        "--project",
        "22",
        "--status",
        "Todo",
    ]);
    let action = Action::from_argv(&proposed, Some("OpenAgentsInc/openagents")).unwrap();
    assert_eq!(action, issue_on_board());
    assert_eq!(
        Action::from_argv(&proposed, None),
        Err(ArgvError::NeedsRepository)
    );
    // --repo wins over the default.
    let mut named = proposed.clone();
    named.extend(words(&["--repo", "a/b"]));
    assert_eq!(
        Action::from_argv(&named, Some("OpenAgentsInc/openagents"))
            .unwrap()
            .repository(),
        "a/b"
    );

    let samples = [
        issue_on_board(),
        Action::CreateIssue {
            repository: "a/b".into(),
            title: "T".into(),
            body: String::new(),
            labels: vec!["bug".into(), "web".into()],
            board: None,
        },
        Action::Comment {
            repository: "a/b".into(),
            number: 3,
            body: "Thanks".into(),
        },
        Action::CloseIssue {
            repository: "a/b".into(),
            number: 4,
            comment: Some("Not doing this.".into()),
            reason: CloseReason::NotPlanned,
        },
        Action::MoveOnBoard {
            repository: "a/b".into(),
            number: 5,
            board: Board {
                number: 22,
                status: "In Progress".into(),
            },
        },
        Action::OpenPullRequest {
            repository: "a/b".into(),
            head: "fix-login".into(),
            base: Some("main".into()),
            title: "Fix login".into(),
            body: "Closes #4.".into(),
            draft: true,
        },
    ];
    for action in samples {
        let argv = action.argv();
        assert!(crate::argv::is_github(&argv), "{argv:?}");
        assert_eq!(
            Action::from_argv(&argv, None),
            Ok(action.clone()),
            "{argv:?}"
        );
    }

    // `project add` with a status is the same move; labels split on commas.
    assert_eq!(
        Action::from_argv(
            &words(&[
                "project",
                "add",
                "5",
                "--project=22",
                "--status",
                "In Progress"
            ]),
            Some("a/b")
        )
        .unwrap()
        .argv()[..2],
        words(&["project", "move"])[..]
    );
    let labeled = Action::from_argv(
        &words(&["issue", "create", "--title", "T", "--label", "bug, web"]),
        Some("a/b"),
    )
    .unwrap();
    assert!(matches!(labeled, Action::CreateIssue { ref labels, .. } if labels == &["bug", "web"]));
}

#[test]
fn other_commands_and_bad_words_are_refused() {
    for (argv, want) in [
        (vec!["computer", "list"], ArgvError::NotGithub),
        (vec!["issue", "claim", "3"], ArgvError::NotGithub),
        (vec!["issue"], ArgvError::NotGithub),
    ] {
        assert_eq!(Action::from_argv(&words(&argv), Some("a/b")), Err(want));
    }
    for argv in [
        vec!["issue", "create", "--body", "no title"],
        vec!["issue", "create", "--title", "T", "--body-file", "notes.md"],
        vec!["issue", "create", "--title", "T", "--project", "22"],
        vec!["issue", "create", "--title", "T", "--title", "U"],
        vec!["issue", "create", "--title", "T", "extra"],
        vec!["issue", "comment", "3"],
        vec!["issue", "comment", "3", "4", "--body", "x"],
        vec!["issue", "close", "3", "--reason", "duplicate"],
        vec!["issue", "close", "3", "--project", "22"],
        vec!["project", "move", "3", "--status", "Todo"],
        vec!["project", "move", "3", "--project", "22"],
        vec!["pr", "open", "--head", "a..b", "--title", "T"],
        vec!["issue", "create", "--repo", "not a repo", "--title", "T"],
        vec!["issue", "create", "--title"],
    ] {
        assert!(
            matches!(
                Action::from_argv(&words(&argv), Some("a/b")),
                Err(ArgvError::Invalid(_))
            ),
            "{argv:?} should be refused"
        );
    }
}

#[tokio::test]
async fn closing_as_not_planned_and_labels_reach_github() {
    fn github(method: &Method, path: &str) -> (u16, Value) {
        match (method.as_str(), path) {
            ("GET", "/user") => (200, json!({"login": "octo"})),
            ("POST", "/repos/a/b/issues") => (201, json!({"number": 8, "id": 1})),
            ("POST", "/repos/a/b/issues/4/comments") => (201, json!({"id": 2})),
            ("PATCH", "/repos/a/b/issues/4") => (200, json!({"number": 4})),
            _ => (404, Value::Null),
        }
    }
    let api = Fake::new(Some(&["repo"]), github);
    let close = Action::CloseIssue {
        repository: "a/b".into(),
        number: 4,
        comment: Some("Not doing this.".into()),
        reason: CloseReason::NotPlanned,
    };
    run::run(&api, &close).await.unwrap();
    assert_eq!(
        api.calls(),
        vec![
            "GET /user",
            "POST /repos/a/b/issues/4/comments",
            "PATCH /repos/a/b/issues/4"
        ]
    );
    assert_eq!(
        api.sent("PATCH /repos/a/b/issues/4"),
        json!({"state": "closed", "state_reason": "not_planned"})
    );

    let api = Fake::new(Some(&["repo"]), github);
    let create = Action::CreateIssue {
        repository: "a/b".into(),
        title: "T".into(),
        body: String::new(),
        labels: vec!["bug".into()],
        board: None,
    };
    run::run(&api, &create).await.unwrap();
    assert_eq!(
        api.sent("POST /repos/a/b/issues"),
        json!({"title": "T", "body": "", "labels": ["bug"]})
    );
}
