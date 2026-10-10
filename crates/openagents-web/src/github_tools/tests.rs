//! The chat's GitHub tools (#11167): forms read into exact actions, the
//! confirm card says every change, a sealed card runs only for its person
//! and chat and only once, and running asks GitHub's REST API for exactly
//! the changes the card showed, stopping before any change when the
//! connection lacks the access (boards need `project`).

use std::sync::{Arc, Mutex};

use reqwest::Method;
use serde_json::{Value, json};

use super::action::{Action, Board, Tool, ToolForm};
use super::run::{self, Answer, Api, Failure};
use super::*;

fn app() -> App {
    let root = tempfile::tempdir().expect("temp dir");
    let config = crate::Config::development(root.path().join("store"));
    App(Arc::new(crate::Inner { config }))
}

fn form(tool: &str, fields: &[(&str, &str)]) -> ToolForm {
    let mut form = ToolForm {
        tool: tool.to_string(),
        ..ToolForm::default()
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
        board: Some(Board {
            number: 22,
            status: "Todo".into(),
        }),
    }
}

#[test]
fn forms_read_into_exact_actions_or_say_what_to_fix() {
    let action = Action::from_form(&form(
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
    let plain = Action::from_form(&form(
        "create_issue",
        &[("repository", "a/b"), ("title", "T"), ("status", "Todo")],
    ))
    .unwrap();
    assert!(!plain.needs_board());

    let closed = Action::from_form(&form(
        "close_issue",
        &[("repository", "a/b"), ("number", "#7"), ("comment", "  ")],
    ))
    .unwrap();
    assert_eq!(
        closed,
        Action::CloseIssue {
            repository: "a/b".into(),
            number: 7,
            comment: None
        }
    );

    let pull = Action::from_form(&form(
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
            Action::from_form(&form(bad, &fields)).is_err(),
            "{bad} {fields:?} should be refused"
        );
    }
    let long = "x".repeat(action::MAX_BODY + 1);
    assert!(
        Action::from_form(&form(
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

#[test]
fn a_sealed_card_opens_only_for_its_person_and_chat_and_runs_once() {
    let app = app();
    let sealed = seal(&app, "owner-a", "chat-1", issue_on_board());
    let opened = open(&app, "owner-a", "chat-1", &sealed).expect("opens");
    assert_eq!(opened.action, issue_on_board());
    assert!(open(&app, "owner-b", "chat-1", &sealed).is_none());
    assert!(open(&app, "owner-a", "chat-2", &sealed).is_none());

    // Another server's key, or a changed action, doesn't open.
    assert!(open(&app(), "owner-a", "chat-1", &sealed).is_none());
    let (payload, tag) = sealed.split_once('.').unwrap();
    let mut changed: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).unwrap()).unwrap();
    changed["action"]["title"] = json!("Something else");
    let forged = format!(
        "{}.{tag}",
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&changed).unwrap())
    );
    assert!(open(&app, "owner-a", "chat-1", &forged).is_none());

    assert!(claim(&opened.id));
    assert!(!claim(&opened.id), "a card runs once");
    unclaim(&opened.id);
    assert!(
        claim(&opened.id),
        "a card that waited for access runs later"
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

#[test]
fn the_pages_are_plain_words() {
    let tools = tools_markup("csrf", "chat-1", "a/b", &ToolForm::default(), None).into_string();
    crate::copy_guard::assert_plain("/chat/chat-1/github", &tools);
    assert!(tools.contains("value=\"a/b\""));
    for tool in Tool::ALL {
        assert!(tools.contains(&format!("value=\"{}\"", tool.key())));
    }

    let card = card_markup("csrf", "chat-1", &issue_on_board(), "sealed").into_string();
    crate::copy_guard::assert_plain("/chat/chat-1/github", &card);
    assert!(card.contains("Puts it on board 22 with the status Todo."));
    assert!(card.contains("Nothing changes on GitHub until you confirm."));
    assert!(card.contains("action=\"/chat/chat-1/github/run\""));

    let consent = consent_markup("chat-1", true, true).into_string();
    crate::copy_guard::assert_plain("/chat/chat-1/github", &consent);
    assert!(
        consent
            .contains("href=\"/auth/github/board?return_to=%2Fchat%2Fchat-1%2Fgithub%2Fconfirm\"")
    );

    let done = done_markup(
        "chat-1",
        &run::Done {
            summary: "Opened #501 in a/b.".into(),
            link: Some("https://github.com/a/b/issues/501".into()),
            problem: None,
        },
    )
    .into_string();
    crate::copy_guard::assert_plain("/chat/chat-1/github", &done);
}
