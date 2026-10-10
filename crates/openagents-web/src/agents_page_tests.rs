use super::*;
use crate::coder_sync;

fn owner() -> String {
    account_owner("acct_one")
}

/// An issue run as `openagents chat work` on a cloud environment reports it.
fn issue_run(id: &str, status: &str) -> Item {
    Item {
        id: id.into(),
        kind: "agent".into(),
        title: "Issue #11228: Fleet rows for cloud environment runs".into(),
        engine: Some("claude".into()),
        status: status.into(),
        started_unix: 1_000,
        finished_unix: (status != "working").then_some(1_090),
        cost_usd: Some(1.7),
        tokens: None,
        session: None,
        question: None,
        line: Some("Running cargo test -p agent-fleet".into()),
    }
}

#[tokio::test]
async fn an_environment_run_is_listed_and_stop_reaches_its_machine() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::local(dir.path().to_path_buf());
    let mut chat = issue_run("chat-1", "working");
    chat.kind = "chat".into();
    phone_api::report(
        &store,
        &owner(),
        "oa-dev-env-1",
        vec![issue_run("task-1", "working"), chat],
    )
    .await
    .unwrap();
    coder_sync::check_in(&store, &owner(), "oa-dev-env-1")
        .await
        .unwrap()
        .unwrap();
    let found = machines(&store, &owner()).await;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "oa-dev-env-1");
    assert!(found[0].online);
    // Only agents: the open chat is the chat's own business.
    assert_eq!(found[0].agents.len(), 1);
    assert_eq!(found[0].agents[0].id, "task-1");
    let html = settings_row(&store, &owner()).await.into_string();
    assert!(html.contains("1 working"), "{html}");

    // Stop, as the page's form queues it, reaches the environment at its
    // next report.
    let command = Command {
        id: "0f8fad5b-d9cb-469f-a165-70867728950e".into(),
        item: "task-1".into(),
        action: "stop".into(),
        question: None,
        text: None,
    };
    assert_eq!(
        phone_api::queue_action(&store, &owner(), "oa-dev-env-1", command.clone())
            .await
            .unwrap(),
        Acted::Queued
    );
    let handed = phone_api::report(
        &store,
        &owner(),
        "oa-dev-env-1",
        vec![issue_run("task-1", "working")],
    )
    .await
    .unwrap();
    assert_eq!(handed, vec![command]);
}

#[test]
fn the_list_shows_each_run_with_stop_and_message_while_it_works() {
    let machines = vec![
        Machine {
            name: "oa-dev-env-1".into(),
            online: true,
            updated_unix: 1_100,
            agents: vec![issue_run("task-1", "working"), issue_run("task-0", "done")],
        },
        Machine {
            name: "oa-pool-p1-a".into(),
            online: false,
            updated_unix: 500,
            agents: vec![issue_run("task-9", "working")],
        },
    ];
    let html = list_markup(
        &machines,
        "token",
        1_120,
        Some("Sent. It reads your message at its next step."),
    )
    .into_string();
    crate::copy_guard::assert_plain(PAGE, &html);
    assert!(html.contains("oa-dev-env-1 · Online"), "{html}");
    assert!(
        html.contains("oa-pool-p1-a · Last seen 10 min ago"),
        "{html}"
    );
    assert!(html.contains("Issue #11228: Fleet rows for cloud environment runs"));
    assert!(html.contains("Working · claude · 2m 0s · $1.70"), "{html}");
    assert!(html.contains("Running cargo test -p agent-fleet"));
    // Stop and Message on the working run of the online machine only.
    assert_eq!(html.matches(r#"action="/settings/agents/stop""#).count(), 1);
    assert!(html.contains(r#"name="item" value="task-1""#));
    assert!(html.contains("/settings/agents/message?computer=oa-dev-env-1&amp;item=task-1"));
    assert!(!html.contains("item=task-0") && !html.contains("item=task-9"));
    assert!(html.contains("Sent. It reads your message at its next step."));

    let empty = list_markup(&[], "token", 0, None).into_string();
    crate::copy_guard::assert_plain(PAGE, &empty);
    assert!(empty.contains("No agents yet"));
}

#[test]
fn the_message_form_names_its_agent_and_machine() {
    let html =
        message_markup(&issue_run("task-1", "working"), "oa-dev-env-1", "token").into_string();
    crate::copy_guard::assert_plain(MESSAGE_PAGE, &html);
    assert!(html.contains(r#"name="computer" value="oa-dev-env-1""#));
    assert!(html.contains(r#"name="item" value="task-1""#));
    assert!(html.contains("on oa-dev-env-1."));
}

#[test]
fn every_action_code_reads_as_a_sentence() {
    for code in [
        "stopping", "sent", "gone", "offline", "empty", "long", "secret", "failed",
    ] {
        assert!(notice_words(code).is_some(), "{code}");
    }
    assert_eq!(notice_words("<script>"), None);
}
