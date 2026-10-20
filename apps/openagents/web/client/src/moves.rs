//! Moving a chat to a cloud computer and back (#12282), as the browser
//! reads it: the `PlanMove` family's values (`openagents_rpc::moves`, which
//! needs the engine's runtime and doesn't build for the browser), and the
//! plain words the move card shows. The source computer does the work; the
//! browser sends it the commands through its device room.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const PLAN_MOVE: &str = "PlanMove";
pub const APPROVE_MOVE: &str = "ApproveMove";
pub const START_MOVE: &str = "StartMove";
pub const MOVE_STATUS: &str = "MoveStatus";
pub const BRING_BACK: &str = "BringBack";
pub const DISCARD_MOVE: &str = "DiscardMove";

/// Where one chat is in its move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveState {
    Planned,
    Pausing,
    Captured,
    Provisioned,
    Restored,
    Continued,
    Released,
    StaysHere,
    BroughtBack,
}

impl MoveState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Planned => "Ready to move",
            Self::Pausing => "Finishing the current step",
            Self::Captured => "Saved on this computer",
            Self::Provisioned => "Cloud computer ready",
            Self::Restored => "Starting on the cloud computer",
            Self::Continued | Self::Released => "Running on the cloud computer",
            Self::StaysHere => "Stays on this computer",
            Self::BroughtBack => "Back on this computer",
        }
    }

    pub fn moved(self) -> bool {
        matches!(self, Self::Continued | Self::Released)
    }

    pub fn finished(self) -> bool {
        matches!(self, Self::StaysHere | Self::BroughtBack)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BringBackView {
    pub approved: bool,
    pub branch: Option<String>,
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatProgress {
    pub chat: String,
    pub title: String,
    pub agent: String,
    pub state: MoveState,
    pub reason: Option<String>,
    pub bring_back: Option<BringBackView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub id: String,
    pub approved: bool,
    pub computer: Option<String>,
    pub destination: String,
    pub estimate_minutes: u32,
    pub chats: Vec<ChatProgress>,
}

impl PlanView {
    /// The plan waits for the person to approve the move.
    pub fn needs_approval(&self) -> bool {
        !self.approved && self.chats.iter().any(|c| c.state == MoveState::Planned)
    }

    pub fn finished(&self) -> bool {
        self.chats.iter().all(|c| c.state.finished())
    }

    /// Some chat stopped partway and can be tried again.
    pub fn retryable(&self) -> bool {
        self.approved && self.chats.iter().any(retryable)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MoveStatus {
    #[serde(default)]
    pub plans: Vec<PlanView>,
}

impl MoveStatus {
    /// The newest plan that holds `chat`.
    pub fn plan_for(&self, chat: &str) -> Option<&PlanView> {
        self.plans
            .iter()
            .find(|p| p.chats.iter().any(|c| c.chat == chat))
    }
}

/// A chat stopped partway and can be tried again.
pub fn retryable(chat: &ChatProgress) -> bool {
    chat.reason.is_some() && !chat.state.finished() && !chat.state.moved()
        || chat
            .bring_back
            .as_ref()
            .is_some_and(|b| b.approved && !b.done && chat.reason.is_some())
}

/// Plain words for a chat's row, as the desktop's move sheet says them.
pub fn status_line(chat: &ChatProgress) -> String {
    let line = match &chat.bring_back {
        Some(back) if back.done => match &back.branch {
            Some(branch) => format!("Back on its computer. Its work is on the branch {branch}."),
            None => chat.state.label().into(),
        },
        Some(back) if back.approved => "Coming back to its computer".into(),
        Some(_) => "Waiting for you to approve bringing it back".into(),
        None => chat.state.label().into(),
    };
    match &chat.reason {
        Some(reason) if !chat.state.moved() || chat.bring_back.is_some() => {
            format!("{line}. {reason}")
        }
        _ => line,
    }
}

/// "About 1 hr 10 min of cloud computer time".
pub fn estimate_label(minutes: u32) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    match (h, m) {
        (0, m) => format!("About {m} min of cloud computer time"),
        (h, 0) => format!("About {h} hr of cloud computer time"),
        (h, m) => format!("About {h} hr {m} min of cloud computer time"),
    }
}

/// `PlanMove` for one chat, to a new cloud computer, asked by the person.
pub fn plan_params(chat: &str) -> Value {
    json!({"chats": [chat], "requestedBy": "person"})
}

/// `ApproveMove` for the plan, or for one chat's Bring back in it.
pub fn approve_params(plan: &str, bring_back: Option<&str>) -> Value {
    json!({"planId": plan, "bringBack": bring_back})
}

pub fn plan_id_params(plan: &str) -> Value {
    json!({ "planId": plan })
}

/// `BringBack`: the cloud computer stops once the chat is back.
pub fn bring_back_params(chat: &str) -> Value {
    json!({"chat": chat, "after": "stop", "requestedBy": "person"})
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plan as the engine sends it (`openagents_rpc::moves::PlanView`).
    fn plan(state: &str, reason: Option<&str>, back: Value) -> PlanView {
        serde_json::from_value(json!({
            "id": "plan1", "createdUnix": 1, "requestedBy": "person", "approved": true,
            "computer": "cmp_0123456789abcdef0123", "destination": "A new cloud computer",
            "estimateMinutes": 70,
            "chats": [{"chat": "c1", "title": "Fix it", "agent": "Claude Code",
                "state": state, "reason": reason, "bringBack": back}],
        }))
        .unwrap()
    }

    #[test]
    fn a_plan_reads_as_the_engine_sends_it_and_says_where_each_chat_is() {
        let moving = plan("captured", None, Value::Null);
        assert_eq!(status_line(&moving.chats[0]), "Saved on this computer");
        assert!(!moving.needs_approval());
        assert!(!moving.retryable());

        let stuck = plan(
            "provisioned",
            Some("The cloud computer didn't start."),
            Value::Null,
        );
        assert!(stuck.retryable());
        assert_eq!(
            status_line(&stuck.chats[0]),
            "Cloud computer ready. The cloud computer didn't start."
        );

        let back = plan(
            "brought_back",
            None,
            json!({"after": "stop", "requestedBy": "person", "approved": true,
                "branch": "openagents/cloud-1", "done": true}),
        );
        assert!(back.finished());
        assert!(status_line(&back.chats[0]).contains("openagents/cloud-1"));

        let status: MoveStatus =
            serde_json::from_value(json!({"plans": [serde_json::to_value(json!({
                "id": "p", "approved": false, "computer": null, "destination": "A new cloud computer",
                "estimateMinutes": 45, "chats": [{"chat": "c2", "title": "", "agent": "Codex",
                "state": "planned", "reason": null, "bringBack": null}]
            })).unwrap()]}))
            .unwrap();
        assert!(status.plan_for("c2").unwrap().needs_approval());
        assert!(status.plan_for("c1").is_none());
        assert_eq!(
            estimate_label(70),
            "About 1 hr 10 min of cloud computer time"
        );
    }

    #[test]
    fn the_commands_name_the_person_and_the_plan() {
        assert_eq!(plan_params("c1")["requestedBy"], "person");
        assert_eq!(approve_params("p", Some("c1"))["bringBack"], "c1");
        assert_eq!(approve_params("p", None)["bringBack"], Value::Null);
        assert_eq!(bring_back_params("c1")["after"], "stop");
    }
}
