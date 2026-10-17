use coder_sync::server::notifications::{Category, chat_title, notification_for, parse_prefs};
use openagents_doc::RegistryRow;
use openagents_proto::view::SESSION_STALE_MS;
use serde_json::{Value, json};
const NOW: i64 = 1_800_000_000_000;
fn session(status: &str, turn: Option<&str>, at: i64) -> RegistryRow {
    let mut fields = json!({"chatId":"c1","status":status,"updatedAt":at});
    if let Some(turn) = turn {
        fields["lastCompletedTurn"] = json!(turn);
    }
    serde_json::from_value(
        json!({"kind":"sessions","id":"c1","seq":1,"deleted":false,"fields":fields,"clocks":{}}),
    )
    .unwrap()
}
#[test]
fn completed_turn_notifies_once() {
    let before = session("working", None, NOW);
    let done = session("idle", Some("t1"), NOW);
    assert_eq!(
        notification_for(Some(&before), &done, NOW),
        Some(Category::Done)
    );
    assert_eq!(notification_for(Some(&done), &done, NOW), None);
}
#[test]
fn interrupted_or_expired_activity_is_silent() {
    for turn in [None, Some("old")] {
        assert_eq!(
            notification_for(
                Some(&session("working", turn, NOW)),
                &session("idle", turn, NOW),
                NOW
            ),
            None
        );
    }
}
#[test]
fn run_error_notifies_once_without_completion() {
    let failed = session("errored", Some("failed"), NOW);
    assert_eq!(
        notification_for(Some(&session("working", Some("old"), NOW)), &failed, NOW),
        Some(Category::Failed)
    );
    assert_eq!(notification_for(Some(&failed), &failed, NOW), None);
}
#[test]
fn question_notifies_once() {
    let asking = session("awaitingInput", Some("t1"), NOW);
    assert_eq!(
        notification_for(Some(&session("working", Some("t1"), NOW)), &asking, NOW),
        Some(Category::Input)
    );
    assert_eq!(notification_for(Some(&asking), &asking, NOW), None);
}
#[test]
fn coalesced_working_states_keep_completions() {
    let a = session("working", None, NOW);
    let b = session("working", Some("first"), NOW);
    let c = session("idle", Some("second"), NOW);
    assert_eq!(notification_for(Some(&a), &b, NOW), Some(Category::Done));
    assert_eq!(notification_for(Some(&b), &b, NOW), None);
    assert_eq!(notification_for(Some(&b), &c, NOW), Some(Category::Done));
}
#[test]
fn stale_completion_and_question_are_silent() {
    let a = session("working", None, NOW);
    assert_eq!(
        notification_for(
            Some(&a),
            &session("idle", Some("old"), NOW - SESSION_STALE_MS - 1),
            NOW
        ),
        None
    );
    assert_eq!(
        notification_for(Some(&a), &session("awaitingInput", None, NOW - 60_000), NOW),
        None
    );
}
#[test]
fn first_or_revived_row_only_sets_baseline() {
    assert_eq!(
        notification_for(None, &session("errored", Some("t"), NOW), NOW),
        None
    );
    let mut deleted = session("idle", None, NOW);
    deleted.deleted = true;
    assert_eq!(
        notification_for(Some(&deleted), &session("idle", Some("t"), NOW), NOW),
        None
    );
    assert_eq!(
        notification_for(Some(&session("working", None, NOW)), &deleted, NOW),
        None
    );
}
fn chat(fields: Value) -> RegistryRow {
    serde_json::from_value(json!({"kind":"chats","id":"c1","seq":1,"fields":fields,"clocks":{}}))
        .unwrap()
}
#[test]
fn titles_fall_back_and_archived_or_side_chats_are_skipped() {
    assert_eq!(
        chat_title(Some(&chat(json!({"title":"Fix login"})))),
        Some("Fix login".into())
    );
    assert_eq!(
        chat_title(Some(&chat(json!({})))),
        Some("New session".into())
    );
    assert_eq!(chat_title(None), Some("New session".into()));
    assert_eq!(chat_title(Some(&chat(json!({"parentChatId":"p"})))), None);
    assert_eq!(chat_title(Some(&chat(json!({"archived":true})))), None);
    let mut deleted = chat(json!({}));
    deleted.deleted = true;
    assert_eq!(chat_title(Some(&deleted)), None);
}
#[test]
fn prefs_default_on_and_keep_desktop_words() {
    assert_eq!(
        serde_json::to_value(parse_prefs(&Value::Null)).unwrap(),
        json!({"done":true,"input":true,"failed":true})
    );
    assert_eq!(
        serde_json::to_value(parse_prefs(&json!({"done":false}))).unwrap(),
        json!({"done":false,"input":true,"failed":true})
    );
    assert_eq!(Category::Done.body(), "Run finished");
    assert_eq!(Category::Input.body(), "Waiting on your input");
    assert_eq!(Category::Failed.body(), "Run failed");
}
