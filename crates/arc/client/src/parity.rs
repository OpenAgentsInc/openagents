use arc_core::{ArcAction, ArcActionKind, ArcFrameData, ArcGameState};
use serde::{Deserialize, Serialize};

use crate::ArcSessionFrame;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcParityTraceStep {
    pub action: ArcAction,
    pub frames: Vec<ArcFrameData>,
    pub game_state: ArcGameState,
    pub levels_completed: u16,
    pub win_levels: u16,
    pub available_actions: Vec<ArcActionKind>,
    pub full_reset: bool,
}

impl From<ArcSessionFrame> for ArcParityTraceStep {
    fn from(frame: ArcSessionFrame) -> Self {
        Self {
            action: frame.action,
            frames: frame.frames,
            game_state: frame.game_state,
            levels_completed: frame.levels_completed,
            win_levels: frame.win_levels,
            available_actions: frame.available_actions,
            full_reset: frame.full_reset,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcLocalRemoteParityReport {
    pub case_id: String,
    pub compared_steps: u32,
    pub local_steps: Vec<ArcParityTraceStep>,
    pub remote_steps: Vec<ArcParityTraceStep>,
    pub outcome: ArcLocalRemoteParityOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcLocalRemoteParityOutcome {
    Match,
    Mismatch(ArcLocalRemoteParityMismatch),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcLocalRemoteParityMismatch {
    pub step_index: Option<usize>,
    pub field: ArcLocalRemoteParityField,
    pub local: String,
    pub remote: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcLocalRemoteParityField {
    StepCount,
    Action,
    FrameCount,
    FramePixels,
    GameState,
    LevelsCompleted,
    WinLevels,
    AvailableActions,
    FullReset,
}

#[must_use]
pub fn compare_local_remote_traces(
    case_id: impl Into<String>,
    local_steps: &[ArcSessionFrame],
    remote_steps: &[ArcSessionFrame],
) -> ArcLocalRemoteParityReport {
    let local_steps = local_steps
        .iter()
        .cloned()
        .map(ArcParityTraceStep::from)
        .collect::<Vec<_>>();
    let remote_steps = remote_steps
        .iter()
        .cloned()
        .map(ArcParityTraceStep::from)
        .collect::<Vec<_>>();
    let outcome = find_first_mismatch(&local_steps, &remote_steps).map_or(
        ArcLocalRemoteParityOutcome::Match,
        ArcLocalRemoteParityOutcome::Mismatch,
    );

    ArcLocalRemoteParityReport {
        case_id: case_id.into(),
        compared_steps: u32::try_from(local_steps.len().min(remote_steps.len()))
            .unwrap_or(u32::MAX),
        local_steps,
        remote_steps,
        outcome,
    }
}

fn find_first_mismatch(
    local_steps: &[ArcParityTraceStep],
    remote_steps: &[ArcParityTraceStep],
) -> Option<ArcLocalRemoteParityMismatch> {
    if local_steps.len() != remote_steps.len() {
        return Some(ArcLocalRemoteParityMismatch {
            step_index: None,
            field: ArcLocalRemoteParityField::StepCount,
            local: local_steps.len().to_string(),
            remote: remote_steps.len().to_string(),
        });
    }

    for (step_index, (local, remote)) in local_steps.iter().zip(remote_steps.iter()).enumerate() {
        if local.action != remote.action {
            return Some(mismatch(
                step_index,
                ArcLocalRemoteParityField::Action,
                &local.action,
                &remote.action,
            ));
        }
        if local.frames.len() != remote.frames.len() {
            return Some(ArcLocalRemoteParityMismatch {
                step_index: Some(step_index),
                field: ArcLocalRemoteParityField::FrameCount,
                local: local.frames.len().to_string(),
                remote: remote.frames.len().to_string(),
            });
        }
        if local.frames != remote.frames {
            return Some(mismatch(
                step_index,
                ArcLocalRemoteParityField::FramePixels,
                &local.frames,
                &remote.frames,
            ));
        }
        if local.game_state != remote.game_state {
            return Some(mismatch(
                step_index,
                ArcLocalRemoteParityField::GameState,
                &local.game_state,
                &remote.game_state,
            ));
        }
        if local.levels_completed != remote.levels_completed {
            return Some(ArcLocalRemoteParityMismatch {
                step_index: Some(step_index),
                field: ArcLocalRemoteParityField::LevelsCompleted,
                local: local.levels_completed.to_string(),
                remote: remote.levels_completed.to_string(),
            });
        }
        if local.win_levels != remote.win_levels {
            return Some(ArcLocalRemoteParityMismatch {
                step_index: Some(step_index),
                field: ArcLocalRemoteParityField::WinLevels,
                local: local.win_levels.to_string(),
                remote: remote.win_levels.to_string(),
            });
        }
        if local.available_actions != remote.available_actions {
            return Some(mismatch(
                step_index,
                ArcLocalRemoteParityField::AvailableActions,
                &local.available_actions,
                &remote.available_actions,
            ));
        }
        if local.full_reset != remote.full_reset {
            return Some(ArcLocalRemoteParityMismatch {
                step_index: Some(step_index),
                field: ArcLocalRemoteParityField::FullReset,
                local: local.full_reset.to_string(),
                remote: remote.full_reset.to_string(),
            });
        }
    }

    None
}

fn mismatch<T>(
    step_index: usize,
    field: ArcLocalRemoteParityField,
    local: &T,
    remote: &T,
) -> ArcLocalRemoteParityMismatch
where
    T: Serialize + std::fmt::Debug,
{
    ArcLocalRemoteParityMismatch {
        step_index: Some(step_index),
        field,
        local: serialize_debuggable(local),
        remote: serialize_debuggable(remote),
    }
}

fn serialize_debuggable<T>(value: &T) -> String
where
    T: Serialize + std::fmt::Debug,
{
    serde_json::to_string(value).unwrap_or_else(|_| format!("{value:?}"))
}

#[cfg(test)]
mod tests {
    use arc_core::{ArcFrameData, ArcTaskId};

    use super::*;

    fn trace_step(available_actions: Vec<ArcActionKind>) -> ArcSessionFrame {
        ArcSessionFrame {
            game_id: ArcTaskId::new("bt11-fd9df0622a1a").expect("task id should validate"),
            guid: "local-guid".to_owned(),
            frames: vec![ArcFrameData::new(2, 2, vec![0, 1, 2, 3]).expect("frame should validate")],
            game_state: ArcGameState::NotFinished,
            levels_completed: 0,
            win_levels: 1,
            action: ArcAction::Action3,
            available_actions,
            full_reset: false,
        }
    }

    #[test]
    fn parity_report_marks_first_structural_mismatch() {
        let local = vec![trace_step(vec![
            ArcActionKind::Action3,
            ArcActionKind::Action4,
        ])];
        let remote = vec![trace_step(vec![ArcActionKind::Action3])];
        let report = compare_local_remote_traces("available-actions", &local, &remote);

        assert_eq!(
            report.outcome,
            ArcLocalRemoteParityOutcome::Mismatch(ArcLocalRemoteParityMismatch {
                step_index: Some(0),
                field: ArcLocalRemoteParityField::AvailableActions,
                local: "[\"ACTION3\",\"ACTION4\"]".to_owned(),
                remote: "[\"ACTION3\"]".to_owned(),
            })
        );
    }
}
