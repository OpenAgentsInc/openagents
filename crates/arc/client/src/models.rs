use std::path::PathBuf;

use arc_core::{ArcAction, ArcActionKind, ArcFrameData, ArcGameState, ArcTaskId};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ArcClientError;

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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    pub opaque: Option<serde_json::Value>,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcResetCommand {
    pub game_id: ArcTaskId,
    pub card_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcSimpleActionCommand {
    pub game_id: ArcTaskId,
    pub guid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcComplexActionCommand {
    pub game_id: ArcTaskId,
    pub guid: String,
    pub x: u8,
    pub y: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcCompatibilityActionInput {
    pub id: u8,
    #[serde(default)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcCompatibilityFrameResponse {
    pub game_id: ArcTaskId,
    pub guid: String,
    pub frame: Vec<Vec<Vec<u8>>>,
    #[serde(
        serialize_with = "serialize_game_state_wire",
        deserialize_with = "deserialize_game_state_wire"
    )]
    pub state: ArcGameState,
    pub levels_completed: u16,
    pub win_levels: u16,
    pub action_input: ArcCompatibilityActionInput,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_actions: Vec<u8>,
    #[serde(default)]
    pub full_reset: bool,
}

impl ArcCompatibilityFrameResponse {
    pub fn from_session_frame(frame: ArcSessionFrame) -> Self {
        Self {
            game_id: frame.game_id,
            guid: frame.guid,
            frame: frame.frames.into_iter().map(expand_frame).collect(),
            state: frame.game_state,
            levels_completed: frame.levels_completed,
            win_levels: frame.win_levels,
            action_input: ArcCompatibilityActionInput::from_action(&frame.action),
            available_actions: frame
                .available_actions
                .into_iter()
                .map(action_kind_to_wire)
                .collect(),
            full_reset: frame.full_reset,
        }
    }

    pub fn try_into_session_frame(self) -> Result<ArcSessionFrame, ArcClientError> {
        Ok(ArcSessionFrame {
            game_id: self.game_id,
            guid: self.guid,
            frames: self
                .frame
                .into_iter()
                .enumerate()
                .map(|(frame_index, frame)| flatten_remote_frame(frame_index, frame))
                .collect::<Result<Vec<_>, _>>()?,
            game_state: self.state,
            levels_completed: self.levels_completed,
            win_levels: self.win_levels,
            action: self.action_input.try_into_action()?,
            available_actions: self
                .available_actions
                .into_iter()
                .map(action_kind_from_wire)
                .collect::<Result<Vec<_>, _>>()?,
            full_reset: self.full_reset,
        })
    }
}

impl ArcCompatibilityActionInput {
    #[must_use]
    pub fn from_action(action: &ArcAction) -> Self {
        let (id, data) = match action {
            ArcAction::Reset => (0, serde_json::Map::new()),
            ArcAction::Action1 => (1, serde_json::Map::new()),
            ArcAction::Action2 => (2, serde_json::Map::new()),
            ArcAction::Action3 => (3, serde_json::Map::new()),
            ArcAction::Action4 => (4, serde_json::Map::new()),
            ArcAction::Action5 => (5, serde_json::Map::new()),
            ArcAction::Action6 { x, y } => {
                let mut data = serde_json::Map::new();
                data.insert("x".to_owned(), serde_json::Value::from(*x));
                data.insert("y".to_owned(), serde_json::Value::from(*y));
                (6, data)
            }
            ArcAction::Action7 => (7, serde_json::Map::new()),
        };
        Self { id, data }
    }

    pub fn try_into_action(self) -> Result<ArcAction, ArcClientError> {
        match self.id {
            0 => Ok(ArcAction::Reset),
            1 => Ok(ArcAction::Action1),
            2 => Ok(ArcAction::Action2),
            3 => Ok(ArcAction::Action3),
            4 => Ok(ArcAction::Action4),
            5 => Ok(ArcAction::Action5),
            6 => {
                let x = self
                    .data
                    .get("x")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or(ArcClientError::MissingAction6Coordinate { axis: "x" })?;
                let y = self
                    .data
                    .get("y")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or(ArcClientError::MissingAction6Coordinate { axis: "y" })?;
                let x = u8::try_from(x)
                    .map_err(|_| ArcClientError::MissingAction6Coordinate { axis: "x" })?;
                let y = u8::try_from(y)
                    .map_err(|_| ArcClientError::MissingAction6Coordinate { axis: "y" })?;
                ArcAction::action6(x, y).map_err(Into::into)
            }
            7 => Ok(ArcAction::Action7),
            id => Err(ArcClientError::UnsupportedActionId { id }),
        }
    }
}

pub(crate) fn action_kind_from_wire(id: u8) -> Result<ArcActionKind, ArcClientError> {
    match id {
        1 => Ok(ArcActionKind::Action1),
        2 => Ok(ArcActionKind::Action2),
        3 => Ok(ArcActionKind::Action3),
        4 => Ok(ArcActionKind::Action4),
        5 => Ok(ArcActionKind::Action5),
        6 => Ok(ArcActionKind::Action6),
        7 => Ok(ArcActionKind::Action7),
        other => Err(ArcClientError::UnsupportedActionId { id: other }),
    }
}

#[must_use]
pub(crate) fn action_kind_to_wire(kind: ArcActionKind) -> u8 {
    match kind {
        ArcActionKind::Reset => 0,
        ArcActionKind::Action1 => 1,
        ArcActionKind::Action2 => 2,
        ArcActionKind::Action3 => 3,
        ArcActionKind::Action4 => 4,
        ArcActionKind::Action5 => 5,
        ArcActionKind::Action6 => 6,
        ArcActionKind::Action7 => 7,
    }
}

pub(crate) fn flatten_remote_frame(
    frame_index: usize,
    rows: Vec<Vec<u8>>,
) -> Result<ArcFrameData, ArcClientError> {
    let height =
        u8::try_from(rows.len()).map_err(|_| arc_core::ArcFrameDataError::InvalidHeight(255))?;
    let width = rows.first().map(std::vec::Vec::len).unwrap_or_default();
    let width = u8::try_from(width).map_err(|_| arc_core::ArcFrameDataError::InvalidWidth(255))?;
    let mut pixels = Vec::with_capacity(usize::from(width) * usize::from(height));

    for (row_index, row) in rows.into_iter().enumerate() {
        if row.len() != usize::from(width) {
            return Err(ArcClientError::RaggedRemoteFrame {
                frame_index,
                row_index,
                expected: usize::from(width),
                actual: row.len(),
            });
        }
        pixels.extend(row);
    }

    ArcFrameData::new(width, height, pixels).map_err(Into::into)
}

pub(crate) fn expand_frame(frame: ArcFrameData) -> Vec<Vec<u8>> {
    frame
        .pixels()
        .chunks(usize::from(frame.width()))
        .map(|row| row.to_vec())
        .collect()
}

fn serialize_game_state_wire<S>(state: &ArcGameState, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(match state {
        ArcGameState::NotStarted => "NOT_STARTED",
        ArcGameState::NotFinished => "NOT_FINISHED",
        ArcGameState::Win => "WIN",
        ArcGameState::GameOver => "GAME_OVER",
    })
}

fn deserialize_game_state_wire<'de, D>(deserializer: D) -> Result<ArcGameState, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    match raw.as_str() {
        "NOT_STARTED" => Ok(ArcGameState::NotStarted),
        "NOT_FINISHED" => Ok(ArcGameState::NotFinished),
        "WIN" => Ok(ArcGameState::Win),
        "GAME_OVER" => Ok(ArcGameState::GameOver),
        other => Err(D::Error::custom(format!(
            "unsupported ARC game state `{other}`"
        ))),
    }
}
