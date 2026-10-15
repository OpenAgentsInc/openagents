//! Provider observations. These describe execution; they grant no authority.
//!
//! Sequence numbers start at one for each generation. `Started` opens a new
//! generation and clears its previous state. Text snapshots replace an item;
//! deltas append only while its history is complete. `Gap` invalidates deltas
//! until a new item snapshot arrives. `Ended` closes provider output, not the
//! task: the task owner still decides completion, checks, and publishing.

use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "openagents.coder.observation.v1";
pub const TRACE_KEY: &str = "coder_observation";
pub const MAX_TEXT: usize = 32 * 1024;
pub const MAX_ITEMS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    pub host: String,
    pub task: String,
    pub turn: usize,
    /// Increasing within a task turn, including retries and fallback sessions.
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub schema: String,
    pub scope: Scope,
    pub seq: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_session: Option<String>,
    /// Native item/tool-call identity, or an adapter-assigned stable identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child: Option<String>,
    pub body: Body,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextMode {
    Delta,
    Snapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Text,
    Reasoning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanItem {
    pub text: String,
    pub status: PlanStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    Question,
    Approval,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ending {
    Completed,
    Interrupted,
    Failed,
}

/// Measured token counts. None is unknown; Some(0) is an observed zero.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Usage {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Body {
    Child {
        parent: Option<String>,
        title: String,
        harness: String,
        model: Option<String>,
        status: crate::children::Status,
    },
    Started {
        provider: String,
        model: String,
    },
    Session {
        model: Option<String>,
        version: Option<String>,
    },
    Text {
        channel: Channel,
        mode: TextMode,
        text: String,
        complete: bool,
        #[serde(default)]
        truncated: bool,
    },
    ToolStarted {
        name: String,
        /// Display text, not executable arguments or an authorization request.
        input: String,
    },
    ToolResult {
        text: String,
        failed: bool,
        #[serde(default)]
        truncated: bool,
    },
    Plan {
        items: Vec<PlanItem>,
    },
    Request {
        id: String,
        request: RequestKind,
        text: String,
        choices: Vec<String>,
    },
    Artifact {
        path: String,
        change: String,
    },
    Context {
        used: Option<u64>,
        capacity: Option<u64>,
    },
    Usage {
        usage: Usage,
    },
    RateLimit {
        /// Provider-reported status, kept separate from tokens and money.
        status: String,
        resets_at: Option<u64>,
    },
    Policy {
        tool: String,
        decision: String,
    },
    Ended {
        ending: Ending,
        message: Option<String>,
    },
    Gap {
        /// First sequence known to be missing, when reported by the owner.
        after: Option<u64>,
    },
    #[serde(other)]
    Unknown,
}

/// Bound text by bytes without cutting a UTF-8 character.
pub fn bounded(text: &str) -> String {
    let mut end = text.len().min(MAX_TEXT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

impl Observation {
    pub fn applies_to(&self, host: &str, task: &str, turn: usize) -> bool {
        self.schema == SCHEMA
            && self.scope.host == host
            && self.scope.task == task
            && self.scope.turn == turn
            && !host.is_empty()
            && !task.is_empty()
            && turn > 0
            && self.scope.generation > 0
            && self.seq > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_observations_and_unknown_usage_remain_distinct_from_zero() {
        let body: Body = serde_json::from_str(r#"{"kind":"future"}"#).unwrap();
        assert_eq!(body, Body::Unknown);
        let usage: Usage = serde_json::from_str(r#"{"output":0}"#).unwrap();
        assert_eq!(usage.input, None);
        assert_eq!(usage.output, Some(0));
        let text = "日".repeat(MAX_TEXT);
        assert!(bounded(&text).len() <= MAX_TEXT);
    }
}
