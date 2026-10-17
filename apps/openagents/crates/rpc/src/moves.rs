//! Moving running chats to a cloud computer and back (#11933,
//! `docs/desktop/move-running-agents.md`): the method names and the values
//! the app, the engine, and the agent tools share.
//!
//! A move plan is saved before anything is sent. Starting it and bringing a
//! chat back both need the person's approval, which only the app gives
//! ([`APPROVE_MOVE`]); the agent tools can plan, ask, and read, but can't
//! approve.

use serde::{Deserialize, Serialize};

/// Plan a move: the chats and the destination. Returns a [`PlanView`].
pub const PLAN_MOVE: &str = "PlanMove";
/// The person approves a plan, or a chat's Bring back, in the app. The
/// approved work starts at once. Returns a [`PlanView`].
pub const APPROVE_MOVE: &str = "ApproveMove";
/// Start, or continue after an interruption, an approved plan. Refused
/// while the plan waits for approval. Returns a [`PlanView`].
pub const START_MOVE: &str = "StartMove";
/// Every plan, newest first, or one plan. Returns a [`MoveStatus`].
pub const MOVE_STATUS: &str = "MoveStatus";
/// Ask to bring a moved chat back to this computer. It runs once the person
/// approves it. Returns a [`PlanView`].
pub const BRING_BACK: &str = "BringBack";
/// Drop a plan that hasn't started. Returns `{}`.
pub const DISCARD_MOVE: &str = "DiscardMove";

/// Where one chat is in its move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveState {
    /// Saved; nothing is sent yet.
    Planned,
    /// The agent is finishing its current step.
    Pausing,
    /// The project and the agent's conversation are saved on this computer.
    Captured,
    /// The cloud computer is ready.
    Provisioned,
    /// The project is on the cloud computer and the agent started there.
    Restored,
    /// The chat continues on the cloud computer.
    Continued,
    /// This computer's copy of the agent has stopped.
    Released,
    /// The chat stays on this computer; `reason` says why.
    StaysHere,
    /// The chat came back to this computer.
    BroughtBack,
}

impl MoveState {
    /// Plain words for the progress card.
    pub fn label(self) -> &'static str {
        match self {
            Self::Planned => "Ready to move",
            Self::Pausing => "Finishing the current step",
            Self::Captured => "Saved on this computer",
            Self::Provisioned => "Cloud computer ready",
            Self::Restored => "Starting on the cloud computer",
            Self::Continued => "Running on the cloud computer",
            Self::Released => "Running on the cloud computer",
            Self::StaysHere => "Stays on this computer",
            Self::BroughtBack => "Back on this computer",
        }
    }
    /// The chat runs on the cloud computer now.
    pub fn moved(self) -> bool {
        matches!(self, Self::Continued | Self::Released)
    }
    /// Nothing more happens to this chat in this plan.
    pub fn finished(self) -> bool {
        matches!(self, Self::StaysHere | Self::BroughtBack)
    }
}

/// Who asked for the move or the Bring back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Requester {
    #[default]
    Person,
    Agent,
}

/// What happens to the cloud computer after a chat comes back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AfterBringBack {
    #[default]
    Stop,
    Delete,
    Keep,
}

/// A chat's Bring back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BringBackView {
    pub after: AfterBringBack,
    pub requested_by: Requester,
    pub approved: bool,
    /// The local branch that holds the cloud computer's work.
    pub branch: Option<String>,
    pub done: bool,
}

/// One chat in a plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatProgress {
    pub chat: String,
    pub title: String,
    /// The agent's name, such as "Claude Code".
    pub agent: String,
    pub state: MoveState,
    /// Plain words, when the chat stays here or the last try failed.
    pub reason: Option<String>,
    pub bring_back: Option<BringBackView>,
}

/// A plan as the app and the tools see it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub id: String,
    pub created_unix: u64,
    pub requested_by: Requester,
    pub approved: bool,
    /// The cloud computer's ID, once there is one.
    pub computer: Option<String>,
    /// "A new cloud computer", or the existing computer's ID.
    pub destination: String,
    /// The estimated paid cloud computer time, in minutes.
    pub estimate_minutes: u32,
    pub chats: Vec<ChatProgress>,
}

impl PlanView {
    /// The plan waits for the person: to approve the move, or a Bring back.
    pub fn needs_approval(&self) -> bool {
        (!self.approved && self.chats.iter().any(|c| c.state == MoveState::Planned))
            || self.chats.iter().any(|c| {
                c.bring_back
                    .as_ref()
                    .is_some_and(|b| !b.approved && !b.done)
            })
    }
    /// Every chat is finished.
    pub fn finished(&self) -> bool {
        self.chats.iter().all(|c| c.state.finished())
    }
}

/// `PlanMove` parameters.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanMoveParams {
    /// Chat IDs. Empty means every agent running on this computer.
    #[serde(default)]
    pub chats: Vec<String>,
    /// An existing cloud computer's ID; omit for a new one.
    #[serde(default)]
    pub computer: Option<String>,
    #[serde(default)]
    pub requested_by: Requester,
}

/// `ApproveMove` parameters: the plan, or one chat's Bring back in it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveMoveParams {
    pub plan_id: String,
    #[serde(default)]
    pub bring_back: Option<String>,
}

/// `StartMove`, `DiscardMove`, and `MoveStatus` parameters.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanIdParams {
    #[serde(default)]
    pub plan_id: Option<String>,
}

/// `BringBack` parameters.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BringBackParams {
    pub chat: String,
    #[serde(default)]
    pub after: AfterBringBack,
    #[serde(default)]
    pub requested_by: Requester,
}

/// `MoveStatus` reply.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStatus {
    pub plans: Vec<PlanView>,
}

/// "About 1 hr 10 min" for an estimate in minutes.
pub fn estimate_label(minutes: u32) -> String {
    let (h, m) = (minutes / 60, minutes % 60);
    match (h, m) {
        (0, m) => format!("About {m} min of cloud computer time"),
        (h, 0) => format!("About {h} hr of cloud computer time"),
        (h, m) => format!("About {h} hr {m} min of cloud computer time"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_read_as_hours_and_minutes() {
        assert_eq!(estimate_label(45), "About 45 min of cloud computer time");
        assert_eq!(estimate_label(60), "About 1 hr of cloud computer time");
        assert_eq!(
            estimate_label(71),
            "About 1 hr 11 min of cloud computer time"
        );
    }
}
