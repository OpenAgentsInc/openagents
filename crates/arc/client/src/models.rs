use std::path::PathBuf;

use arc_core::{ArcAction, ArcActionKind, ArcFrameData, ArcGameState, ArcTaskId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcEnvironmentInfo {
    pub game_id: ArcTaskId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub private_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub level_tags: Vec<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub baseline_actions: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_package_path: Option<PathBuf>,
}

impl ArcEnvironmentInfo {
    #[must_use]
    pub fn with_local_package_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.local_package_path = Some(path.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcOpenScorecardRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opaque: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub competition_mode: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcOpenScorecardResponse {
    pub card_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcCloseScorecardRequest {
    pub card_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcRemoteSession {
    pub game_id: ArcTaskId,
    pub scorecard_id: String,
    pub guid: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcScorecardRunSummary {
    pub id: String,
    pub guid: String,
    pub score: u32,
    pub levels_completed: u16,
    pub actions: u32,
    pub resets: u32,
    pub state: ArcGameState,
    pub completed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub level_scores: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub level_actions: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub level_baseline_actions: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_of_levels: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_of_environments: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcScorecardEnvironment {
    pub id: ArcTaskId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<ArcScorecardRunSummary>,
    pub score: u32,
    pub actions: u32,
    pub levels_completed: u16,
    pub completed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level_count: Option<u32>,
    pub resets: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcTagScore {
    pub id: String,
    pub guid: String,
    pub score: u32,
    pub levels_completed: u16,
    pub actions: u32,
    pub resets: u32,
    pub state: ArcGameState,
    pub completed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_of_levels: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_of_environments: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcScorecardSummary {
    pub card_id: String,
    pub score: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_update: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_environments_completed: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_environments: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_levels_completed: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_levels: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_actions: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environments: Vec<ArcScorecardEnvironment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags_scores: Vec<ArcTagScore>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcSessionFrame {
    pub game_id: ArcTaskId,
    pub guid: String,
    pub frames: Vec<ArcFrameData>,
    pub game_state: ArcGameState,
    pub levels_completed: u16,
    pub win_levels: u16,
    pub action: ArcAction,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_actions: Vec<ArcActionKind>,
    #[serde(default)]
    pub full_reset: bool,
}
