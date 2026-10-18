//! Foreman autonomy (#11965): the check-in schedule, the standing authority
//! over chats the owner grants it, and its append-only decision log. The
//! host that runs Foreman's chat keeps all three; the desktop settings card
//! and the app's agent tools read them over the engine RPC.

use serde::{Deserialize, Serialize};

/// The default and allowed check-in intervals, in minutes. The default
/// keeps a visible status in Foreman's chat at least every 10 minutes.
pub const DEFAULT_EVERY_MINUTES: u32 = 10;
pub const MIN_EVERY_MINUTES: u32 = 5;
pub const MAX_EVERY_MINUTES: u32 = 24 * 60;
/// How long the agent's own turn may go with no output before the host
/// stops it and checks in (#12029), and the allowed range.
pub const DEFAULT_STUCK_MINUTES: u32 = 10;
pub const MIN_STUCK_MINUTES: u32 = 2;
pub const MAX_STUCK_MINUTES: u32 = 120;

fn default_stuck_minutes() -> u32 {
    DEFAULT_STUCK_MINUTES
}

/// What a standing authority lets Foreman do in other chats without asking.
/// Money, keys, secrets, deploys and closing other agents' issues are never
/// in it; those stay behind the existing approval rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Send a message to a chat or answer its question.
    Message,
    /// Start a new chat.
    Create,
    /// Steer a running chat.
    Steer,
    /// Stop a chat's turn.
    Stop,
}

impl Scope {
    pub const ALL: [Scope; 4] = [Scope::Message, Scope::Create, Scope::Steer, Scope::Stop];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Authority {
    /// Unix milliseconds.
    pub granted_at: i64,
    pub scopes: Vec<Scope>,
}

/// What the owner sets: which chat is Foreman, how often it checks in,
/// whether wake-ups are paused, and whether it holds standing authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForemanSettings {
    pub chat_id: Option<String>,
    pub every_minutes: u32,
    pub paused: bool,
    pub authority: bool,
    /// Minutes the chat's turn may go with no output before the host stops
    /// it and starts a check-in that says why.
    #[serde(default = "default_stuck_minutes")]
    pub stuck_minutes: u32,
}

impl Default for ForemanSettings {
    fn default() -> Self {
        Self {
            chat_id: None,
            every_minutes: DEFAULT_EVERY_MINUTES,
            paused: false,
            authority: false,
            stuck_minutes: DEFAULT_STUCK_MINUTES,
        }
    }
}

/// A saved agent's chat on this computer that can be Foreman.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForemanCandidate {
    pub chat_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeReason {
    Schedule,
    Events,
    /// The host stopped the chat's own turn after it went the stuck window
    /// with no output (#12029).
    Stuck,
}

/// One line of the decision log, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DecisionEntry {
    /// The host started a check-in turn.
    #[serde(rename_all = "camelCase")]
    Wake {
        at: i64,
        id: String,
        reason: WakeReason,
        events: Vec<String>,
    },
    /// What Foreman recorded about one check-in.
    #[serde(rename_all = "camelCase")]
    Decision {
        at: i64,
        saw: String,
        decided: String,
        did: String,
    },
    /// How a check-in turn ended.
    #[serde(rename_all = "camelCase")]
    Outcome { at: i64, id: String, status: String },
    /// The owner granted or revoked the standing authority.
    #[serde(rename_all = "camelCase")]
    Authority {
        at: i64,
        granted: bool,
        scopes: Vec<Scope>,
    },
}

impl DecisionEntry {
    pub fn at(&self) -> i64 {
        match self {
            Self::Wake { at, .. }
            | Self::Decision { at, .. }
            | Self::Outcome { at, .. }
            | Self::Authority { at, .. } => *at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForemanStatus {
    pub settings: ForemanSettings,
    pub chat_title: Option<String>,
    pub candidates: Vec<ForemanCandidate>,
    pub authority: Option<Authority>,
    /// Unix milliseconds of the next scheduled check-in, when one is due.
    pub next_check_in: Option<i64>,
    /// Something the owner should know, in plain words.
    pub problem: Option<String>,
    /// Unix ms when this computer turned check-ins on for the chosen chat by
    /// itself, because its agent's definition asks for them (#12030).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turned_on_at: Option<i64>,
    /// The latest entries, oldest first.
    pub decisions: Vec<DecisionEntry>,
}

/// `RecordForemanDecision`, sent by Foreman's own chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordDecision {
    pub origin_chat_id: Option<String>,
    pub saw: String,
    pub decided: String,
    pub did: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_round_trip_with_their_kind() {
        let entry = DecisionEntry::Wake {
            at: 5,
            id: "w".into(),
            reason: WakeReason::Events,
            events: vec!["Chat \"Fix\" finished.".into()],
        };
        let wire = serde_json::to_value(&entry).unwrap();
        assert_eq!(wire["kind"], "wake");
        assert_eq!(wire["reason"], "events");
        assert_eq!(
            serde_json::from_value::<DecisionEntry>(wire).unwrap(),
            entry
        );
        assert_eq!(entry.at(), 5);
    }
}
