use super::*;
use crate::chat_store::account_owner;
use crate::coder_sync::{Saved, Upload, WireMessage, save};

fn owner() -> String {
    account_owner("acct_one")
}

fn item(id: &str, status: &str) -> Item {
    Item {
        id: id.into(),
        kind: "chat".into(),
        title: "Fix the build".into(),
        engine: Some("Claude Code".into()),
        status: status.into(),
        started_unix: 1,
        finished_unix: None,
        cost_usd: Some(0.25),
        tokens: Some(1200),
        session: Some("s1".into()),
        question: None,
        line: None,
    }
}

fn command(id: &str, item: &str) -> Command {
    Command {
        id: id.into(),
        item: item.into(),
        action: "stop".into(),
        question: None,
        text: None,
    }
}

#[test]
fn owns_only_its_paths() {
    assert!(owns("/v1/threads") && owns("/v1/threads/x/messages"));
    assert!(owns("/v1/computers/Studio/sync") && owns("/v1/agents/actions"));
    assert!(!owns("/v1/threadsx") && !owns("/v1/models"));
    assert!(crate::upstream::owned("/v1/agents"));
}

#[test]
fn items_are_checked_bounded_and_screened() {
    assert!(checked_item(item("a", "working")).is_some());
    assert!(checked_item(item("a", "sleeping")).is_none());
    assert!(checked_item(item("", "working")).is_none());
    let mut long = item("a", "asking");
    long.title = "x".repeat(500);
    long.question = Some(Question {
        id: "7".into(),
        text: "Run it with sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGH?".into(),
    });
    let checked = checked_item(long).unwrap();
    assert_eq!(checked.title.chars().count(), 120);
    assert_eq!(checked.question.unwrap().text, LEFT_OUT);
}

#[tokio::test]
async fn a_report_keeps_items_and_hands_each_command_out_once() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::local(dir.path().to_path_buf());
    assert!(
        report(&store, &owner(), "Studio", vec![item("s1", "working")])
            .await
            .unwrap()
            .is_empty()
    );
    // The computer has to be online for an action to wait.
    coder_sync::check_in(&store, &owner(), "Studio")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        queue_action(&store, &owner(), "Studio", command("cmd-0001", "nope"))
            .await
            .unwrap(),
        Acted::Unknown
    );
    assert_eq!(
        queue_action(&store, &owner(), "Elsewhere", command("cmd-0001", "s1"))
            .await
            .unwrap(),
        Acted::Unknown
    );
    assert_eq!(
        queue_action(&store, &owner(), "Studio", command("cmd-0001", "s1"))
            .await
            .unwrap(),
        Acted::Queued
    );
    // The same action again waits once.
    queue_action(&store, &owner(), "Studio", command("cmd-0001", "s1"))
        .await
        .unwrap();
    // Another account sees nothing of it.
    let other = read_agents(&store, &account_owner("acct_two"))
        .await
        .unwrap();
    assert!(other.boards.is_empty() && other.commands.is_empty());
    let handed = report(&store, &owner(), "Studio", vec![item("s1", "working")])
        .await
        .unwrap();
    assert_eq!(handed, vec![command("cmd-0001", "s1")]);
    assert!(
        report(&store, &owner(), "Studio", vec![item("s1", "stopped")])
            .await
            .unwrap()
            .is_empty()
    );
    // Handed out: a resend isn't queued again.
    queue_action(&store, &owner(), "Studio", command("cmd-0001", "s1"))
        .await
        .unwrap();
    assert!(
        read_agents(&store, &owner())
            .await
            .unwrap()
            .commands
            .is_empty()
    );
    let board = &read_agents(&store, &owner()).await.unwrap().boards["Studio"];
    assert_eq!(board.items[0].status, "stopped");
}

#[tokio::test]
async fn an_offline_computer_takes_no_action() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::local(dir.path().to_path_buf());
    report(&store, &owner(), "Studio", vec![item("s1", "working")])
        .await
        .unwrap();
    assert_eq!(
        queue_action(&store, &owner(), "Studio", command("cmd-0002", "s1"))
            .await
            .unwrap(),
        Acted::Offline
    );
}

#[tokio::test]
async fn threads_list_web_terminal_and_phone_chats_with_their_lines() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::local(dir.path().to_path_buf());
    let upload = |computer: &str| Upload {
        computer: computer.into(),
        title: "Chat".into(),
        messages: vec![WireMessage {
            role: "user".into(),
            text: "Hi".into(),
        }],
    };
    let Saved::Saved { .. } = save(&store, &owner(), "s1", &upload("Studio"))
        .await
        .unwrap()
    else {
        panic!("not saved")
    };
    save(&store, &owner(), "phone-abc", &upload("iPhone"))
        .await
        .unwrap();
    let (rows, _) = listed(&store, &owner()).await.unwrap();
    assert_eq!(rows.len(), 2);
    let phone = rows.iter().find(|r| r["surface"] == "phone").unwrap();
    assert_eq!(phone["line"], "Phone · iPhone");
    assert_eq!(phone["can_reply"], false);
    let terminal = rows.iter().find(|r| r["surface"] == "terminal").unwrap();
    assert_eq!(terminal["line"], "Terminal · Studio");
    // Studio hasn't checked in: no reply box.
    assert_eq!(terminal["can_reply"], false);
    coder_sync::check_in(&store, &owner(), "Studio")
        .await
        .unwrap()
        .unwrap();
    let (rows, _) = listed(&store, &owner()).await.unwrap();
    let terminal = rows.iter().find(|r| r["surface"] == "terminal").unwrap();
    assert_eq!(terminal["can_reply"], true);
    assert!(
        listed(&store, &account_owner("acct_two"))
            .await
            .unwrap()
            .0
            .is_empty()
    );
}
