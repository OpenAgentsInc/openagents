use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// First version of the shared ARC Rust-native contract surface.
pub const ARC_CORE_SCHEMA_VERSION: u32 = 1;
/// What belongs in `schema` and what does not.
pub const SCHEMA_BOUNDARY_SUMMARY: &str = "Own ARC task/grid/example value types and shared identifiers. Do not absorb benchmark policy, transport behavior, solver search state, or reusable Psionic substrate.";
/// ARC tasks are 2D grids with edge sizes in the 1..=30 range.
pub const ARC_GRID_MAX_EDGE: u8 = 30;
/// ARC-AGI-3 frame rasters are bounded by the 64x64 interaction surface.
pub const ARC_FRAME_MAX_EDGE: u8 = 64;
/// ARC palette values stay in the canonical 0..=9 range.
pub const ARC_PALETTE_SIZE: u8 = 10;
/// `ACTION6` coordinates stay inside the 0..=63 grid advertised upstream.
pub const ARC_ACTION6_COORDINATE_MAX: u8 = 63;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArcTaskId(String);

impl ArcTaskId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn new(raw: impl Into<String>) -> Result<Self, ArcTaskIdError> {
        let raw = raw.into();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ArcTaskIdError::Empty);
        }
        if trimmed.chars().any(char::is_whitespace) {
            return Err(ArcTaskIdError::ContainsWhitespace(trimmed.to_owned()));
        }
        Ok(Self(trimmed.to_owned()))
    }
}

impl fmt::Display for ArcTaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ArcTaskId {
    type Err = ArcTaskIdError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::new(raw)
    }
}

impl Serialize for ArcTaskId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ArcTaskId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcTaskIdError {
    #[error("ARC task id must not be empty")]
    Empty,
    #[error("ARC task id must not contain whitespace: {0}")]
    ContainsWhitespace(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcGrid {
    width: u8,
    height: u8,
    cells: Vec<u8>,
}

impl ArcGrid {
    pub fn new(width: u8, height: u8, cells: Vec<u8>) -> Result<Self, ArcGridError> {
        if width == 0 || width > ARC_GRID_MAX_EDGE {
            return Err(ArcGridError::InvalidWidth(width));
        }
        if height == 0 || height > ARC_GRID_MAX_EDGE {
            return Err(ArcGridError::InvalidHeight(height));
        }

        let expected_len = usize::from(width) * usize::from(height);
        if cells.len() != expected_len {
            return Err(ArcGridError::MismatchedCellCount {
                expected: expected_len,
                actual: cells.len(),
            });
        }

        if let Some(color) = cells.iter().copied().find(|cell| *cell >= ARC_PALETTE_SIZE) {
            return Err(ArcGridError::InvalidColor(color));
        }

        Ok(Self {
            width,
            height,
            cells,
        })
    }

    #[must_use]
    pub fn width(&self) -> u8 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u8 {
        self.height
    }

    #[must_use]
    pub fn cells(&self) -> &[u8] {
        &self.cells
    }

    #[must_use]
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    #[must_use]
    pub fn cell(&self, x: u8, y: u8) -> Option<u8> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let index = usize::from(y) * usize::from(self.width) + usize::from(x);
        self.cells.get(index).copied()
    }

    pub fn canonical_json(&self) -> Result<String, ContractSerializationError> {
        canonical_json_string(self)
    }

    pub fn contract_digest(&self) -> Result<String, ContractSerializationError> {
        canonical_sha256_hex(self)
    }
}

impl Serialize for ArcGrid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ArcGridWire {
            width: self.width,
            height: self.height,
            cells: self.cells.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ArcGrid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ArcGridWire::deserialize(deserializer)?;
        Self::new(wire.width, wire.height, wire.cells).map_err(de::Error::custom)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcGridError {
    #[error("ARC grid width must be in the 1..={ARC_GRID_MAX_EDGE} range, got {0}")]
    InvalidWidth(u8),
    #[error("ARC grid height must be in the 1..={ARC_GRID_MAX_EDGE} range, got {0}")]
    InvalidHeight(u8),
    #[error("ARC grid cell count mismatch: expected {expected}, got {actual}")]
    MismatchedCellCount { expected: usize, actual: usize },
    #[error("ARC grid color must be in the 0..=9 range, got {0}")]
    InvalidColor(u8),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcExample {
    pub input: ArcGrid,
    pub output: ArcGrid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcTask {
    pub id: ArcTaskId,
    pub train: Vec<ArcExample>,
    pub test: Vec<ArcGrid>,
}

impl ArcTask {
    pub fn new(
        id: ArcTaskId,
        train: Vec<ArcExample>,
        test: Vec<ArcGrid>,
    ) -> Result<Self, ArcTaskError> {
        if train.is_empty() {
            return Err(ArcTaskError::MissingTrainExamples);
        }
        if test.is_empty() {
            return Err(ArcTaskError::MissingTestInputs);
        }
        Ok(Self { id, train, test })
    }

    pub fn canonical_json(&self) -> Result<String, ContractSerializationError> {
        canonical_json_string(self)
    }

    pub fn contract_digest(&self) -> Result<String, ContractSerializationError> {
        canonical_sha256_hex(self)
    }

    pub fn canonical_body_json(&self) -> Result<String, ContractSerializationError> {
        canonical_json_string(&ArcTaskBodyWire {
            train: self.train.clone(),
            test: self.test.clone(),
        })
    }

    pub fn body_digest(&self) -> Result<String, ContractSerializationError> {
        canonical_sha256_hex(&ArcTaskBodyWire {
            train: self.train.clone(),
            test: self.test.clone(),
        })
    }

    pub fn derived_task_id(&self) -> Result<ArcTaskId, ContractSerializationError> {
        let digest = self.body_digest()?;
        Ok(ArcTaskId(format!("task-{}", &digest[..16])))
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcTaskError {
    #[error("ARC task must include at least one train example")]
    MissingTrainExamples,
    #[error("ARC task must include at least one test input")]
    MissingTestInputs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcBenchmark {
    ArcAgi1,
    ArcAgi2,
    ArcAgi3,
    InternalSynthetic,
    InternalHoldout,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArcAction {
    Reset,
    Action1,
    Action2,
    Action3,
    Action4,
    Action5,
    Action6 { x: u8, y: u8 },
    Action7,
}

impl ArcAction {
    #[must_use]
    pub fn kind(&self) -> ArcActionKind {
        match self {
            Self::Reset => ArcActionKind::Reset,
            Self::Action1 => ArcActionKind::Action1,
            Self::Action2 => ArcActionKind::Action2,
            Self::Action3 => ArcActionKind::Action3,
            Self::Action4 => ArcActionKind::Action4,
            Self::Action5 => ArcActionKind::Action5,
            Self::Action6 { .. } => ArcActionKind::Action6,
            Self::Action7 => ArcActionKind::Action7,
        }
    }

    pub fn action6(x: u8, y: u8) -> Result<Self, ArcActionError> {
        if x > ARC_ACTION6_COORDINATE_MAX {
            return Err(ArcActionError::CoordinateOutOfRange {
                axis: "x",
                value: x,
            });
        }
        if y > ARC_ACTION6_COORDINATE_MAX {
            return Err(ArcActionError::CoordinateOutOfRange {
                axis: "y",
                value: y,
            });
        }
        Ok(Self::Action6 { x, y })
    }
}

impl Serialize for ArcAction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let wire = match self {
            Self::Reset => ArcActionWire {
                kind: "RESET".to_owned(),
                x: None,
                y: None,
            },
            Self::Action1 => ArcActionWire {
                kind: "ACTION1".to_owned(),
                x: None,
                y: None,
            },
            Self::Action2 => ArcActionWire {
                kind: "ACTION2".to_owned(),
                x: None,
                y: None,
            },
            Self::Action3 => ArcActionWire {
                kind: "ACTION3".to_owned(),
                x: None,
                y: None,
            },
            Self::Action4 => ArcActionWire {
                kind: "ACTION4".to_owned(),
                x: None,
                y: None,
            },
            Self::Action5 => ArcActionWire {
                kind: "ACTION5".to_owned(),
                x: None,
                y: None,
            },
            Self::Action6 { x, y } => ArcActionWire {
                kind: "ACTION6".to_owned(),
                x: Some(*x),
                y: Some(*y),
            },
            Self::Action7 => ArcActionWire {
                kind: "ACTION7".to_owned(),
                x: None,
                y: None,
            },
        };
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ArcAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ArcActionWire::deserialize(deserializer)?;
        match wire.kind.as_str() {
            "RESET" => {
                reject_unexpected_coordinates(&wire).map_err(de::Error::custom)?;
                Ok(Self::Reset)
            }
            "ACTION1" => {
                reject_unexpected_coordinates(&wire).map_err(de::Error::custom)?;
                Ok(Self::Action1)
            }
            "ACTION2" => {
                reject_unexpected_coordinates(&wire).map_err(de::Error::custom)?;
                Ok(Self::Action2)
            }
            "ACTION3" => {
                reject_unexpected_coordinates(&wire).map_err(de::Error::custom)?;
                Ok(Self::Action3)
            }
            "ACTION4" => {
                reject_unexpected_coordinates(&wire).map_err(de::Error::custom)?;
                Ok(Self::Action4)
            }
            "ACTION5" => {
                reject_unexpected_coordinates(&wire).map_err(de::Error::custom)?;
                Ok(Self::Action5)
            }
            "ACTION6" => {
                let x = wire
                    .x
                    .ok_or_else(|| de::Error::custom(ArcActionError::MissingCoordinate("x")))?;
                let y = wire
                    .y
                    .ok_or_else(|| de::Error::custom(ArcActionError::MissingCoordinate("y")))?;
                Self::action6(x, y).map_err(de::Error::custom)
            }
            "ACTION7" => {
                reject_unexpected_coordinates(&wire).map_err(de::Error::custom)?;
                Ok(Self::Action7)
            }
            other => Err(de::Error::custom(ArcActionError::UnknownActionKind(
                other.to_owned(),
            ))),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcActionError {
    #[error("ARC action kind is unknown: {0}")]
    UnknownActionKind(String),
    #[error("ARC {0} coordinate is required for ACTION6")]
    MissingCoordinate(&'static str),
    #[error("ARC action {kind} must not carry coordinates")]
    UnexpectedCoordinates { kind: String },
    #[error(
        "ARC ACTION6 coordinate {axis} must be in the 0..={ARC_ACTION6_COORDINATE_MAX} range, got {value}"
    )]
    CoordinateOutOfRange { axis: &'static str, value: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ArcActionKind {
    Reset,
    Action1,
    Action2,
    Action3,
    Action4,
    Action5,
    Action6,
    Action7,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcOperationMode {
    Normal,
    Offline,
    Online,
    Competition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArcGameState {
    #[default]
    NotFinished,
    Win,
    GameOver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcScorePolicyId {
    ArcAgi2ExactMatchV1,
    ArcAgi3MethodologyV1,
    ArcAgi3CompetitionV1,
    ArcAgi3PreviewCompatibilityV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArcRecordingEnvelopeId(String);

impl ArcRecordingEnvelopeId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn new(raw: impl Into<String>) -> Result<Self, ArcRecordingEnvelopeIdError> {
        let raw = raw.into();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ArcRecordingEnvelopeIdError::Empty);
        }
        if trimmed.chars().any(char::is_whitespace) {
            return Err(ArcRecordingEnvelopeIdError::ContainsWhitespace(
                trimmed.to_owned(),
            ));
        }
        Ok(Self(trimmed.to_owned()))
    }
}

impl Serialize for ArcRecordingEnvelopeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ArcRecordingEnvelopeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcRecordingEnvelopeIdError {
    #[error("ARC recording envelope id must not be empty")]
    Empty,
    #[error("ARC recording envelope id must not contain whitespace: {0}")]
    ContainsWhitespace(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcFrameData {
    width: u8,
    height: u8,
    pixels: Vec<u8>,
}

impl ArcFrameData {
    pub fn new(width: u8, height: u8, pixels: Vec<u8>) -> Result<Self, ArcFrameDataError> {
        if width == 0 || width > ARC_FRAME_MAX_EDGE {
            return Err(ArcFrameDataError::InvalidWidth(width));
        }
        if height == 0 || height > ARC_FRAME_MAX_EDGE {
            return Err(ArcFrameDataError::InvalidHeight(height));
        }

        let expected_len = usize::from(width) * usize::from(height);
        if pixels.len() != expected_len {
            return Err(ArcFrameDataError::MismatchedPixelCount {
                expected: expected_len,
                actual: pixels.len(),
            });
        }

        if let Some(color) = pixels
            .iter()
            .copied()
            .find(|cell| *cell >= ARC_PALETTE_SIZE)
        {
            return Err(ArcFrameDataError::InvalidColor(color));
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    #[must_use]
    pub fn width(&self) -> u8 {
        self.width
    }

    #[must_use]
    pub fn height(&self) -> u8 {
        self.height
    }

    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn canonical_json(&self) -> Result<String, ContractSerializationError> {
        canonical_json_string(self)
    }

    pub fn contract_digest(&self) -> Result<String, ContractSerializationError> {
        canonical_sha256_hex(self)
    }
}

impl Serialize for ArcFrameData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ArcFrameDataWire {
            width: self.width,
            height: self.height,
            pixels: self.pixels.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ArcFrameData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ArcFrameDataWire::deserialize(deserializer)?;
        Self::new(wire.width, wire.height, wire.pixels).map_err(de::Error::custom)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcFrameDataError {
    #[error("ARC frame width must be in the 1..={ARC_FRAME_MAX_EDGE} range, got {0}")]
    InvalidWidth(u8),
    #[error("ARC frame height must be in the 1..={ARC_FRAME_MAX_EDGE} range, got {0}")]
    InvalidHeight(u8),
    #[error("ARC frame pixel count mismatch: expected {expected}, got {actual}")]
    MismatchedPixelCount { expected: usize, actual: usize },
    #[error("ARC frame color must be in the 0..=9 range, got {0}")]
    InvalidColor(u8),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcObservation {
    pub frame: ArcFrameData,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_actions: Vec<ArcActionKind>,
    #[serde(default, skip_serializing_if = "arc_game_state_is_default")]
    pub game_state: ArcGameState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcEpisodeStep {
    pub step_index: u32,
    pub action: ArcAction,
    pub observation: ArcObservation,
    pub terminal: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcRecording {
    pub benchmark: ArcBenchmark,
    pub task_id: ArcTaskId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envelope_id: Option<ArcRecordingEnvelopeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_mode: Option<ArcOperationMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_policy_id: Option<ArcScorePolicyId>,
    pub steps: Vec<ArcEpisodeStep>,
}

impl ArcRecording {
    pub fn new(
        benchmark: ArcBenchmark,
        task_id: ArcTaskId,
        steps: Vec<ArcEpisodeStep>,
    ) -> Result<Self, ArcRecordingError> {
        if steps.is_empty() {
            return Err(ArcRecordingError::MissingSteps);
        }
        Ok(Self {
            benchmark,
            task_id,
            envelope_id: None,
            operation_mode: None,
            score_policy_id: None,
            steps,
        })
    }

    pub fn canonical_json(&self) -> Result<String, ContractSerializationError> {
        canonical_json_string(self)
    }

    pub fn contract_digest(&self) -> Result<String, ContractSerializationError> {
        canonical_sha256_hex(self)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcRecordingError {
    #[error("ARC recording must include at least one step")]
    MissingSteps,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcScorecardMetadata {
    pub source_url: Option<String>,
    pub tags: Vec<String>,
    pub opaque: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcLevelScore {
    pub level_index: u16,
    pub action_count: u32,
    pub score: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcScorecard {
    pub benchmark: ArcBenchmark,
    pub task_id: ArcTaskId,
    pub overall_score: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_mode: Option<ArcOperationMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_policy_id: Option<ArcScorePolicyId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_envelope_id: Option<ArcRecordingEnvelopeId>,
    pub metadata: ArcScorecardMetadata,
    pub levels: Vec<ArcLevelScore>,
}

impl ArcScorecard {
    pub fn canonical_json(&self) -> Result<String, ContractSerializationError> {
        canonical_json_string(self)
    }

    pub fn contract_digest(&self) -> Result<String, ContractSerializationError> {
        canonical_sha256_hex(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ArcGridWire {
    width: u8,
    height: u8,
    cells: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ArcActionWire {
    kind: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    x: Option<u8>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    y: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ArcFrameDataWire {
    width: u8,
    height: u8,
    pixels: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct ArcTaskBodyWire {
    train: Vec<ArcExample>,
    test: Vec<ArcGrid>,
}

fn reject_unexpected_coordinates(action: &ArcActionWire) -> Result<(), ArcActionError> {
    if action.x.is_some() || action.y.is_some() {
        return Err(ArcActionError::UnexpectedCoordinates {
            kind: action.kind.clone(),
        });
    }
    Ok(())
}

fn arc_game_state_is_default(state: &ArcGameState) -> bool {
    *state == ArcGameState::NotFinished
}

pub fn canonical_json_string<T>(value: &T) -> Result<String, ContractSerializationError>
where
    T: Serialize,
{
    let value = serde_json::to_value(value)?;
    let canonical_value = canonicalize_json_value(value);
    serde_json::to_string(&canonical_value).map_err(Into::into)
}

pub fn canonical_sha256_hex<T>(value: &T) -> Result<String, ContractSerializationError>
where
    T: Serialize,
{
    let canonical_json = canonical_json_string(value)?;
    let mut hasher = Sha256::new();
    hasher.update(canonical_json.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

#[derive(Debug, Error)]
pub enum ContractSerializationError {
    #[error("failed to serialize ARC contract canonically: {0}")]
    Serde(#[from] serde_json::Error),
}

fn canonicalize_json_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(canonicalize_json_value).collect())
        }
        serde_json::Value::Object(map) => {
            let sorted = map
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json_value(value)))
                .collect::<BTreeMap<_, _>>();

            let mut canonical = serde_json::Map::new();
            for (key, value) in sorted {
                canonical.insert(key, value);
            }
            serde_json::Value::Object(canonical)
        }
        scalar => scalar,
    }
}
