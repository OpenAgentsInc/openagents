//! Pure registry transition rules for session notifications. `push` sends
//! what these rules decide through the push gateway.
use openagents_doc::RegistryRow;
use openagents_proto::{
    SessionStatus,
    view::{Indicator, SESSION_STALE_MS},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Done,
    Input,
    Failed,
}
fn fresh(row: &RegistryRow, now: i64) -> bool {
    now as f64
        - row
            .fields
            .get("updatedAt")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
        <= SESSION_STALE_MS as f64
}
fn indicator(row: &RegistryRow, now: i64) -> Indicator {
    let status = row
        .fields
        .get("status")
        .and_then(|v| serde_json::from_value::<SessionStatus>(v.clone()).ok());
    match status {
        Some(SessionStatus::Errored) => Indicator::Errored,
        Some(SessionStatus::Working) if fresh(row, now) => Indicator::Working,
        Some(SessionStatus::AwaitingInput) if fresh(row, now) => Indicator::AwaitingInput,
        _ => Indicator::None,
    }
}
pub fn notification_for(
    before: Option<&RegistryRow>,
    after: &RegistryRow,
    now: i64,
) -> Option<Category> {
    let before = before?;
    if before.deleted || after.deleted || after.kind != "sessions" {
        return None;
    }
    let previous = indicator(before, now);
    let next = indicator(after, now);
    if next == Indicator::Errored && previous != Indicator::Errored {
        return Some(Category::Failed);
    }
    if next == Indicator::AwaitingInput && previous != Indicator::AwaitingInput {
        return Some(Category::Input);
    }
    let turn = after
        .fields
        .get("lastCompletedTurn")
        .and_then(Value::as_str);
    if fresh(after, now)
        && turn.is_some()
        && turn
            != before
                .fields
                .get("lastCompletedTurn")
                .and_then(Value::as_str)
    {
        return Some(Category::Done);
    }
    None
}
pub fn chat_title(chat: Option<&RegistryRow>) -> Option<String> {
    if let Some(chat) = chat {
        if chat.deleted
            || chat
                .fields
                .get("parentChatId")
                .and_then(Value::as_str)
                .is_some()
            || chat.fields.get("archived") == Some(&Value::Bool(true))
        {
            return None;
        }
    }
    let title = chat
        .and_then(|row| row.fields.get("title"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("New session");
    Some(title.into())
}
impl Category {
    pub fn body(self) -> &'static str {
        match self {
            Self::Done => "Run finished",
            Self::Input => "Waiting on your input",
            Self::Failed => "Run failed",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushPrefs {
    pub done: bool,
    pub input: bool,
    pub failed: bool,
}
pub fn parse_prefs(raw: &Value) -> PushPrefs {
    PushPrefs {
        done: raw["done"] != false,
        input: raw["input"] != false,
        failed: raw["failed"] != false,
    }
}
