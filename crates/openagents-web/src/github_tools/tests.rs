//! The chat's GitHub tools (#11167): a sealed card runs only for its
//! person and chat and only once, a command the chat proposed becomes the
//! same card in the thread, and the pages are plain words. The actions
//! and their REST calls are tested in the `github-actions` crate.

use std::sync::Arc;

use github_actions::rest::Done;
use github_actions::{Action, Tool};
use serde_json::{Value, json};

use super::*;

fn app() -> App {
    let root = tempfile::tempdir().expect("temp dir");
    let config = crate::Config::development(root.path().join("store"));
    App(Arc::new(crate::Inner { config }))
}

fn issue_on_board() -> Action {
    Action::CreateIssue {
        repository: "OpenAgentsInc/openagents".into(),
        title: "Fix the login".into(),
        body: "It loops.".into(),
        labels: Vec::new(),
        board: Some(github_actions::Board {
            number: 22,
            status: "Todo".into(),
        }),
    }
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
    assert!(open(&self::app(), "owner-a", "chat-1", &sealed).is_none());
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
        &Done {
            summary: "Opened #501 in a/b.".into(),
            link: Some("https://github.com/a/b/issues/501".into()),
            problem: None,
        },
    )
    .into_string();
    crate::copy_guard::assert_plain("/chat/chat-1/github", &done);
}

#[test]
fn a_proposed_command_becomes_the_signed_card_in_the_thread() {
    let app = app();
    let argv: Vec<String> = [
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
    ]
    .iter()
    .map(|w| (*w).to_string())
    .collect();
    let proposal = proposed(&argv, Some("OpenAgentsInc/openagents")).expect("a GitHub command");
    assert_eq!(proposal, Ok(issue_on_board()));
    let html = thread_card(&app, "owner-a", "chat-1", &proposal).into_string();
    crate::copy_guard::assert_plain("/chat/chat-1", &html);
    assert!(html.contains("Puts it on board 22 with the status Todo."));
    assert!(html.contains("action=\"/chat/chat-1/github/run\""));
    // The card in the thread carries a seal that opens for this chat only.
    let sealed = html
        .split("name=\"card\" value=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("a sealed card");
    assert_eq!(
        open(&app, "owner-a", "chat-1", sealed).map(|s| s.action),
        Some(issue_on_board())
    );
    assert!(open(&app, "owner-a", "chat-2", sealed).is_none());

    // No repository: a link to the tools, not a card.
    let missing = proposed(&argv, None).expect("a GitHub command");
    let html = thread_card(&app, "owner-a", "chat-1", &missing).into_string();
    crate::copy_guard::assert_plain("/chat/chat-1", &html);
    assert!(html.contains("href=\"/chat/chat-1/github\""));
    assert!(!html.contains("name=\"card\""));

    // Someone else's command is no card at all.
    let other: Vec<String> = vec!["computer".into(), "list".into()];
    assert!(proposed(&other, Some("a/b")).is_none());
}
