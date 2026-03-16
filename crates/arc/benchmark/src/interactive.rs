use arc_core::{
    ArcAction, ArcBenchmark, ArcGameState, ArcLevelScore, ArcOperationMode, ArcRecording,
    ArcScorePolicyId, ArcScorecard, ArcScorecardMetadata, ArcTaskId,
};
use serde::{Deserialize, Serialize};

use crate::ArcBenchmarkError;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveStepSummary {
    pub step_index: u32,
    pub action: ArcAction,
    pub total_actions: u32,
    pub resets: u32,
    pub levels_completed: u16,
    pub win_levels: u16,
    pub game_state: ArcGameState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_level_index: Option<u16>,
    pub current_level_action_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_level_index: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_level_action_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_level_score: Option<f32>,
    #[serde(default)]
    pub full_reset: bool,
    #[serde(default)]
    pub terminal: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveRunReport {
    pub benchmark: ArcBenchmark,
    pub task_id: ArcTaskId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_mode: Option<ArcOperationMode>,
    pub score_policy_id: ArcScorePolicyId,
    pub recording_digest: String,
    pub total_actions: u32,
    pub resets: u32,
    pub levels_completed: u16,
    pub win_levels: u16,
    pub completed: bool,
    pub final_state: ArcGameState,
    pub scorecard: ArcScorecard,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub step_summaries: Vec<ArcInteractiveStepSummary>,
}

pub fn score_interactive_recording(
    recording: &ArcRecording,
    metadata: ArcScorecardMetadata,
    baseline_actions: &[u32],
) -> Result<ArcInteractiveRunReport, ArcBenchmarkError> {
    ensure_interactive_benchmark(recording.benchmark)?;
    ensure_supported_operation_mode(recording.operation_mode)?;

    let score_policy_id = recording
        .score_policy_id
        .unwrap_or(ArcScorePolicyId::ArcAgi3MethodologyV1);
    ensure_supported_interactive_policy(score_policy_id)?;

    let first_step =
        recording
            .steps
            .first()
            .ok_or_else(|| ArcBenchmarkError::MissingInitialFullReset {
                task_id: recording.task_id.clone(),
            })?;
    if first_step.action != ArcAction::Reset || !first_step.full_reset {
        return Err(ArcBenchmarkError::MissingInitialFullReset {
            task_id: recording.task_id.clone(),
        });
    }
    if first_step.win_levels == 0 {
        return Err(ArcBenchmarkError::MissingWinLevels {
            task_id: recording.task_id.clone(),
            step_index: first_step.step_index,
        });
    }

    let win_levels = first_step.win_levels;
    if baseline_actions.len() != usize::from(win_levels) {
        return Err(ArcBenchmarkError::BaselineActionLengthMismatch {
            task_id: recording.task_id.clone(),
            expected: usize::from(win_levels),
            actual: baseline_actions.len(),
        });
    }
    for (level_index, baseline_action_count) in baseline_actions.iter().enumerate() {
        if *baseline_action_count == 0 {
            return Err(ArcBenchmarkError::InvalidBaselineActionCount {
                task_id: recording.task_id.clone(),
                level_index: u16::try_from(level_index + 1).unwrap_or(u16::MAX),
            });
        }
    }

    let mut total_actions = 0u32;
    let mut resets = 0u32;
    let mut levels_completed = 0u16;
    let mut completed_level_total_actions = 0u32;
    let mut level_scores = Vec::with_capacity(usize::from(win_levels));
    let mut step_summaries = Vec::with_capacity(recording.steps.len());
    let mut final_state = ArcGameState::NotStarted;

    for (expected_step_index, step) in recording.steps.iter().enumerate() {
        let expected_step_index = u32::try_from(expected_step_index).unwrap_or(u32::MAX);
        validate_step(
            &recording.task_id,
            expected_step_index,
            step,
            win_levels,
            levels_completed,
        )?;

        if !step.full_reset {
            total_actions = total_actions.saturating_add(1);
            if step.action == ArcAction::Reset {
                resets = resets.saturating_add(1);
            }
        }

        let mut completed_level_index = None;
        let mut completed_level_action_count = None;
        let mut completed_level_score = None;

        if step.levels_completed > levels_completed {
            let level_index = levels_completed.saturating_add(1);
            let action_count = total_actions.saturating_sub(completed_level_total_actions);
            if action_count == 0 {
                return Err(ArcBenchmarkError::ZeroActionCompletedLevel {
                    task_id: recording.task_id.clone(),
                    level_index,
                });
            }

            let baseline = baseline_actions[usize::from(level_index.saturating_sub(1))];
            let score = methodology_level_score(baseline, action_count);
            level_scores.push(ArcLevelScore {
                level_index,
                action_count,
                score,
            });
            completed_level_total_actions = total_actions;
            completed_level_index = Some(level_index);
            completed_level_action_count = Some(action_count);
            completed_level_score = Some(score);
        }

        levels_completed = step.levels_completed;
        final_state = step.observation.game_state;

        let active_level_index = if levels_completed < win_levels {
            Some(levels_completed.saturating_add(1))
        } else {
            None
        };
        let current_level_action_count = if active_level_index.is_some() {
            total_actions.saturating_sub(completed_level_total_actions)
        } else {
            0
        };

        step_summaries.push(ArcInteractiveStepSummary {
            step_index: step.step_index,
            action: step.action.clone(),
            total_actions,
            resets,
            levels_completed,
            win_levels,
            game_state: final_state,
            active_level_index,
            current_level_action_count,
            completed_level_index,
            completed_level_action_count,
            completed_level_score,
            full_reset: step.full_reset,
            terminal: step.terminal,
        });
    }

    if levels_completed == win_levels && final_state != ArcGameState::Win {
        return Err(ArcBenchmarkError::TerminalStateMismatch {
            task_id: recording.task_id.clone(),
            state: final_state,
        });
    }

    let remaining_level_actions = total_actions.saturating_sub(completed_level_total_actions);
    for level_index in levels_completed.saturating_add(1)..=win_levels {
        let action_count = if level_index == levels_completed.saturating_add(1) {
            remaining_level_actions
        } else {
            0
        };
        level_scores.push(ArcLevelScore {
            level_index,
            action_count,
            score: 0.0,
        });
    }

    let scorecard = ArcScorecard {
        benchmark: recording.benchmark,
        task_id: recording.task_id.clone(),
        overall_score: methodology_weighted_average(&level_scores),
        operation_mode: recording.operation_mode,
        score_policy_id: Some(score_policy_id),
        recording_envelope_id: recording.envelope_id.clone(),
        metadata,
        levels: level_scores,
    };

    Ok(ArcInteractiveRunReport {
        benchmark: recording.benchmark,
        task_id: recording.task_id.clone(),
        operation_mode: recording.operation_mode,
        score_policy_id,
        recording_digest: recording.contract_digest()?,
        total_actions,
        resets,
        levels_completed,
        win_levels,
        completed: final_state == ArcGameState::Win,
        final_state,
        scorecard,
        step_summaries,
    })
}

fn validate_step(
    task_id: &ArcTaskId,
    expected_step_index: u32,
    step: &arc_core::ArcEpisodeStep,
    win_levels: u16,
    previous_levels_completed: u16,
) -> Result<(), ArcBenchmarkError> {
    if step.step_index != expected_step_index {
        return Err(ArcBenchmarkError::NonSequentialStepIndex {
            task_id: task_id.clone(),
            expected: expected_step_index,
            actual: step.step_index,
        });
    }
    if step.win_levels == 0 {
        return Err(ArcBenchmarkError::MissingWinLevels {
            task_id: task_id.clone(),
            step_index: step.step_index,
        });
    }
    if step.win_levels != win_levels {
        return Err(ArcBenchmarkError::WinLevelMismatch {
            task_id: task_id.clone(),
            step_index: step.step_index,
            expected: win_levels,
            actual: step.win_levels,
        });
    }
    if step.levels_completed > win_levels {
        return Err(ArcBenchmarkError::LevelsCompletedExceedsWinLevels {
            task_id: task_id.clone(),
            step_index: step.step_index,
            levels_completed: step.levels_completed,
            win_levels,
        });
    }
    if step.full_reset && step.action != ArcAction::Reset {
        return Err(ArcBenchmarkError::FullResetActionMismatch {
            task_id: task_id.clone(),
            step_index: step.step_index,
        });
    }
    if step.full_reset && step.step_index != 0 {
        return Err(ArcBenchmarkError::UnexpectedFullReset {
            task_id: task_id.clone(),
            step_index: step.step_index,
        });
    }
    if step.levels_completed < previous_levels_completed {
        return Err(ArcBenchmarkError::LevelsCompletedRegression {
            task_id: task_id.clone(),
            step_index: step.step_index,
            previous: previous_levels_completed,
            current: step.levels_completed,
        });
    }
    if step
        .levels_completed
        .saturating_sub(previous_levels_completed)
        > 1
    {
        return Err(ArcBenchmarkError::LevelsCompletedJump {
            task_id: task_id.clone(),
            step_index: step.step_index,
            previous: previous_levels_completed,
            current: step.levels_completed,
        });
    }
    Ok(())
}

fn ensure_interactive_benchmark(benchmark: ArcBenchmark) -> Result<(), ArcBenchmarkError> {
    match benchmark {
        ArcBenchmark::ArcAgi3 => Ok(()),
        other => Err(ArcBenchmarkError::UnsupportedInteractiveBenchmark { benchmark: other }),
    }
}

fn ensure_supported_operation_mode(
    operation_mode: Option<ArcOperationMode>,
) -> Result<(), ArcBenchmarkError> {
    if operation_mode == Some(ArcOperationMode::Competition) {
        return Err(ArcBenchmarkError::UnsupportedInteractiveOperationMode {
            operation_mode: ArcOperationMode::Competition,
        });
    }
    Ok(())
}

fn ensure_supported_interactive_policy(
    score_policy_id: ArcScorePolicyId,
) -> Result<(), ArcBenchmarkError> {
    match score_policy_id {
        ArcScorePolicyId::ArcAgi3MethodologyV1 => Ok(()),
        other => Err(ArcBenchmarkError::UnsupportedInteractiveScorePolicy {
            score_policy_id: other,
        }),
    }
}

fn methodology_level_score(baseline_actions: u32, action_count: u32) -> f32 {
    let ratio = baseline_actions as f32 / action_count as f32;
    (ratio * ratio).min(1.0)
}

fn methodology_weighted_average(level_scores: &[ArcLevelScore]) -> f32 {
    let total_weight = level_scores
        .iter()
        .map(|level| u32::from(level.level_index))
        .sum::<u32>();
    if total_weight == 0 {
        return 0.0;
    }

    let weighted_score = level_scores
        .iter()
        .map(|level| level.score * f32::from(level.level_index))
        .sum::<f32>();
    weighted_score / total_weight as f32
}
