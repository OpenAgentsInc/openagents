use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use arc_core::{
    ArcBenchmark, ArcEpisodeStep, ArcGameState, ArcObservation, ArcOperationMode, ArcRecording,
    ArcScorePolicyId, ArcTaskId,
};
use serde::{Deserialize, Serialize};

use crate::ArcClientError;
use crate::models::{
    ArcCompatibilityActionInput, ArcSessionFrame, action_kind_from_wire, action_kind_to_wire,
    expand_frame, flatten_remote_frame,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcRecordingTransportPolicy {
    LocalCanonical,
    OnlineJsonl,
}

impl ArcRecordingTransportPolicy {
    #[must_use]
    pub fn for_operation_mode(operation_mode: ArcOperationMode) -> Self {
        match operation_mode {
            ArcOperationMode::Normal | ArcOperationMode::Offline => Self::LocalCanonical,
            ArcOperationMode::Online | ArcOperationMode::Competition => Self::OnlineJsonl,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcJsonlImportContext {
    pub operation_mode: ArcOperationMode,
    pub score_policy_id: Option<ArcScorePolicyId>,
}

impl Default for ArcJsonlImportContext {
    fn default() -> Self {
        Self {
            operation_mode: ArcOperationMode::Online,
            score_policy_id: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcJsonlRecordingEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    pub data: ArcJsonlRecordingData,
}

impl ArcJsonlRecordingEntry {
    #[must_use]
    pub fn from_session_frame(frame: &ArcSessionFrame, include_frame_data: bool) -> Self {
        Self {
            timestamp: None,
            data: ArcJsonlRecordingData::from_session_frame(frame, include_frame_data),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcJsonlRecordingData {
    pub game_id: ArcTaskId,
    pub state: ArcGameState,
    pub levels_completed: u16,
    pub win_levels: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_input: Option<ArcCompatibilityActionInput>,
    pub guid: String,
    #[serde(default)]
    pub full_reset: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_actions: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Vec<Vec<Vec<u8>>>>,
}

impl ArcJsonlRecordingData {
    #[must_use]
    pub fn from_session_frame(frame: &ArcSessionFrame, include_frame_data: bool) -> Self {
        Self {
            game_id: frame.game_id.clone(),
            state: frame.game_state,
            levels_completed: frame.levels_completed,
            win_levels: frame.win_levels,
            action_input: Some(ArcCompatibilityActionInput::from_action(&frame.action)),
            guid: frame.guid.clone(),
            full_reset: frame.full_reset,
            available_actions: frame
                .available_actions
                .iter()
                .copied()
                .map(action_kind_to_wire)
                .collect(),
            frame: include_frame_data.then(|| {
                frame
                    .frames
                    .iter()
                    .cloned()
                    .map(expand_frame)
                    .collect::<Vec<_>>()
            }),
        }
    }
}

pub fn session_frames_to_recording(
    task_id: ArcTaskId,
    operation_mode: ArcOperationMode,
    frames: &[ArcSessionFrame],
) -> Result<ArcRecording, ArcClientError> {
    if frames.is_empty() {
        return Err(ArcClientError::JsonlRecordingEmpty);
    }

    let steps = frames
        .iter()
        .enumerate()
        .map(|(step_index, frame)| {
            let observation_frame = frame
                .frames
                .last()
                .cloned()
                .ok_or(ArcClientError::MissingObservationFrame { step_index })?;
            Ok(ArcEpisodeStep {
                step_index: u32::try_from(step_index).unwrap_or(u32::MAX),
                action: frame.action.clone(),
                observation: ArcObservation {
                    frame: observation_frame,
                    available_actions: frame.available_actions.clone(),
                    game_state: frame.game_state,
                },
                levels_completed: frame.levels_completed,
                win_levels: frame.win_levels,
                full_reset: frame.full_reset,
                terminal: is_terminal(frame.game_state),
            })
        })
        .collect::<Result<Vec<_>, ArcClientError>>()?;

    let mut recording = ArcRecording::new(ArcBenchmark::ArcAgi3, task_id, steps)?;
    recording.operation_mode = Some(operation_mode);
    Ok(recording)
}

#[must_use]
pub fn recording_to_jsonl_entries(
    recording: &ArcRecording,
    include_frame_data: bool,
) -> Vec<ArcJsonlRecordingEntry> {
    recording
        .steps
        .iter()
        .map(|step| ArcJsonlRecordingEntry {
            timestamp: None,
            data: ArcJsonlRecordingData {
                game_id: recording.task_id.clone(),
                state: step.observation.game_state,
                levels_completed: step.levels_completed,
                win_levels: step.win_levels,
                action_input: Some(ArcCompatibilityActionInput::from_action(&step.action)),
                guid: format!("recording-step-{}", step.step_index),
                full_reset: step.full_reset,
                available_actions: step
                    .observation
                    .available_actions
                    .iter()
                    .copied()
                    .map(action_kind_to_wire)
                    .collect(),
                frame: include_frame_data
                    .then(|| vec![expand_frame(step.observation.frame.clone())]),
            },
        })
        .collect()
}

pub fn jsonl_entries_to_recording(
    entries: &[ArcJsonlRecordingEntry],
    context: ArcJsonlImportContext,
) -> Result<ArcRecording, ArcClientError> {
    let first = entries.first().ok_or(ArcClientError::JsonlRecordingEmpty)?;
    let task_id = first.data.game_id.clone();

    let steps = entries
        .iter()
        .enumerate()
        .map(|(line_index, entry)| {
            if entry.data.game_id != task_id {
                return Err(ArcClientError::JsonlTaskMismatch {
                    expected: task_id.clone(),
                    actual: entry.data.game_id.clone(),
                    line_index,
                });
            }
            let action = entry
                .data
                .action_input
                .clone()
                .ok_or(ArcClientError::JsonlActionInputMissing { line_index })?
                .try_into_action()?;
            let frames = entry
                .data
                .frame
                .clone()
                .ok_or(ArcClientError::JsonlFrameDataMissing {
                    task_id: task_id.clone(),
                    line_index,
                })?
                .into_iter()
                .enumerate()
                .map(|(frame_index, frame)| flatten_remote_frame(frame_index, frame))
                .collect::<Result<Vec<_>, ArcClientError>>()?;
            let observation_frame =
                frames
                    .last()
                    .cloned()
                    .ok_or(ArcClientError::MissingObservationFrame {
                        step_index: line_index,
                    })?;
            Ok(ArcEpisodeStep {
                step_index: u32::try_from(line_index).unwrap_or(u32::MAX),
                action,
                observation: ArcObservation {
                    frame: observation_frame,
                    available_actions: entry
                        .data
                        .available_actions
                        .iter()
                        .copied()
                        .map(action_kind_from_wire)
                        .collect::<Result<Vec<_>, ArcClientError>>()?,
                    game_state: entry.data.state,
                },
                levels_completed: entry.data.levels_completed,
                win_levels: entry.data.win_levels,
                full_reset: entry.data.full_reset,
                terminal: is_terminal(entry.data.state),
            })
        })
        .collect::<Result<Vec<_>, ArcClientError>>()?;

    let mut recording = ArcRecording::new(ArcBenchmark::ArcAgi3, task_id, steps)?;
    recording.operation_mode = Some(context.operation_mode);
    recording.score_policy_id = context.score_policy_id;
    Ok(recording)
}

pub fn write_jsonl_recording_file(
    path: impl AsRef<Path>,
    entries: &[ArcJsonlRecordingEntry],
) -> Result<(), ArcClientError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    for entry in entries {
        serde_json::to_writer(&mut writer, entry)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

pub fn read_jsonl_recording_file(
    path: impl AsRef<Path>,
) -> Result<Vec<ArcJsonlRecordingEntry>, ArcClientError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        entries.push(serde_json::from_str(&line)?);
    }
    Ok(entries)
}

fn is_terminal(state: ArcGameState) -> bool {
    matches!(state, ArcGameState::Win | ArcGameState::GameOver)
}
