use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::contracts::CadAnalysis;
use crate::history::{
    CadHistoryCommand, CadHistorySnapshot, CadHistoryStack, CadHistoryTransition,
};
use crate::parity::scorecard::ParityScorecard;
use crate::sketch_interaction::{
    SketchInteractionState, SketchInteractionTransition, SketchShortcut, apply_exit_confirm,
    apply_face_selection_confirm, apply_shortcut, history_command_for_shortcut,
    history_command_for_transition,
};
use crate::{CadError, CadResult};

pub const PARITY_SKETCH_UNDO_REDO_ISSUE_ID: &str = "VCAD-PARITY-053";
pub const VCAD_SKETCH_UNDO_REDO_MAX_STEPS: usize = 50;

const FACE_SELECTION_CONFIRM_TRANSITION_ID: &str = "sketch.face-selection.confirm";
const EXIT_CONFIRM_TRANSITION_ID: &str = "sketch.exit.confirm";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchUndoRedoParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub vcad_reference_sources: Vec<String>,
    pub history_max_steps: usize,
    pub undo_binding: String,
    pub redo_binding: String,
    pub sequence: Vec<SketchUndoRedoStepSnapshot>,
    pub undo_trace: Vec<SketchUndoRedoTraceSnapshot>,
    pub redo_trace: Vec<SketchUndoRedoTraceSnapshot>,
    pub undo_trace_matches_reverse_sequence: bool,
    pub redo_trace_matches_forward_sequence: bool,
    pub redo_cleared_on_new_edit: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SketchUndoRedoStepSnapshot {
    pub step_id: String,
    pub transition_id: String,
    pub command_codes: Vec<String>,
    pub before_geometry_hash: String,
    pub after_geometry_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SketchUndoRedoTraceSnapshot {
    pub transition_id: String,
    pub command_codes: Vec<String>,
    pub snapshot_revision: u64,
    pub snapshot_geometry_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplayEvidence {
    sequence: Vec<SketchUndoRedoStepSnapshot>,
    undo_trace: Vec<SketchUndoRedoTraceSnapshot>,
    redo_trace: Vec<SketchUndoRedoTraceSnapshot>,
    undo_trace_matches_reverse_sequence: bool,
    redo_trace_matches_forward_sequence: bool,
    redo_cleared_on_new_edit: bool,
}

pub fn build_sketch_undo_redo_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<SketchUndoRedoParityManifest> {
    let evidence = build_replay_evidence()?;
    let replay_evidence = build_replay_evidence()?;
    let deterministic_replay_match = evidence == replay_evidence;
    let deterministic_signature = parity_signature(&evidence, deterministic_replay_match);

    Ok(SketchUndoRedoParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_UNDO_REDO_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        vcad_reference_sources: vec![
            "docs/features/undo-redo.md (snapshot stacks + redo clear-on-new-edit + 50-step history)"
                .to_string(),
            "docs/features/sketch-mode.md (undo/redo works within sketch mode)".to_string(),
            "packages/app/src/hooks/useKeyboardShortcuts.ts (Cmd/Ctrl+Z and Cmd/Ctrl+Shift+Z dispatch)"
                .to_string(),
            "packages/core/src/stores/document-store.ts (engine.can_undo/can_redo + undo/redo dispatch)"
                .to_string(),
        ],
        history_max_steps: VCAD_SKETCH_UNDO_REDO_MAX_STEPS,
        undo_binding: "Cmd/Ctrl+Z".to_string(),
        redo_binding: "Cmd/Ctrl+Shift+Z".to_string(),
        sequence: evidence.sequence.clone(),
        undo_trace: evidence.undo_trace.clone(),
        redo_trace: evidence.redo_trace.clone(),
        undo_trace_matches_reverse_sequence: evidence.undo_trace_matches_reverse_sequence,
        redo_trace_matches_forward_sequence: evidence.redo_trace_matches_forward_sequence,
        redo_cleared_on_new_edit: evidence.redo_cleared_on_new_edit,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "sketch undo/redo uses deterministic snapshot transitions represented by ApplySketchInteraction history commands".to_string(),
            "undo trace replays in strict reverse order and redo trace replays in strict forward order for recorded sketch steps".to_string(),
            "pushing a new sketch edit after undo clears redo history to prevent invalid branch replay".to_string(),
            "history stack enforces vcad parity max depth of 50 transitions for sketch editing sessions".to_string(),
        ],
    })
}

fn build_replay_evidence() -> CadResult<ReplayEvidence> {
    let mut history = CadHistoryStack::new(
        "cad.session.parity.sketch.undo-redo",
        VCAD_SKETCH_UNDO_REDO_MAX_STEPS,
    )?;
    let mut revision = 1_u64;
    let mut state = SketchInteractionState::with_has_parts(true);
    let mut sequence = Vec::new();

    record_shortcut_step(
        "enter_sketch_with_parts",
        SketchShortcut::EnterSketchMode,
        &mut state,
        &mut revision,
        &mut history,
        &mut sequence,
    )?;
    record_transition_step(
        "confirm_face_selection",
        FACE_SELECTION_CONFIRM_TRANSITION_ID,
        apply_face_selection_confirm(&state),
        &mut state,
        &mut revision,
        &mut history,
        &mut sequence,
    )?;
    record_shortcut_step(
        "set_line_tool",
        SketchShortcut::LineTool,
        &mut state,
        &mut revision,
        &mut history,
        &mut sequence,
    )?;

    state = state.with_geometry_context(2, 0, 1);
    record_shortcut_step(
        "apply_horizontal_constraint",
        SketchShortcut::HorizontalConstraint,
        &mut state,
        &mut revision,
        &mut history,
        &mut sequence,
    )?;

    state = state.with_geometry_context(2, 1, 0);
    record_shortcut_step(
        "finish_shape",
        SketchShortcut::FinishCurrentShape,
        &mut state,
        &mut revision,
        &mut history,
        &mut sequence,
    )?;

    state = state.with_geometry_context(2, 0, 0);
    record_shortcut_step(
        "request_exit",
        SketchShortcut::Escape,
        &mut state,
        &mut revision,
        &mut history,
        &mut sequence,
    )?;
    record_transition_step(
        "confirm_exit",
        EXIT_CONFIRM_TRANSITION_ID,
        apply_exit_confirm(&state),
        &mut state,
        &mut revision,
        &mut history,
        &mut sequence,
    )?;

    let redo_cleared_on_new_edit = verify_redo_cleared_on_new_edit(&history, &state, revision)?;

    let mut replay_history = history.clone();
    let undo_trace = drain_undo_trace(&mut replay_history)?;
    let redo_trace = drain_redo_trace(&mut replay_history)?;

    let expected_forward = sequence
        .iter()
        .map(|step| step.transition_id.clone())
        .collect::<Vec<_>>();
    let expected_reverse = expected_forward.iter().rev().cloned().collect::<Vec<_>>();
    let undo_trace_ids = undo_trace
        .iter()
        .map(|entry| entry.transition_id.clone())
        .collect::<Vec<_>>();
    let redo_trace_ids = redo_trace
        .iter()
        .map(|entry| entry.transition_id.clone())
        .collect::<Vec<_>>();

    Ok(ReplayEvidence {
        sequence,
        undo_trace,
        redo_trace,
        undo_trace_matches_reverse_sequence: undo_trace_ids == expected_reverse,
        redo_trace_matches_forward_sequence: redo_trace_ids == expected_forward,
        redo_cleared_on_new_edit,
    })
}

fn record_shortcut_step(
    step_id: &str,
    shortcut: SketchShortcut,
    state: &mut SketchInteractionState,
    revision: &mut u64,
    history: &mut CadHistoryStack,
    sequence: &mut Vec<SketchUndoRedoStepSnapshot>,
) -> CadResult<()> {
    let before_state = state.clone();
    let transition = apply_shortcut(&before_state, shortcut);
    let command = history_command_for_shortcut(shortcut, &transition).ok_or_else(|| {
        CadError::InvalidPolicy {
            reason: format!(
                "shortcut `{}` produced no history command for step `{step_id}`",
                shortcut.stable_id()
            ),
        }
    })?;

    record_history_step(
        step_id,
        shortcut.stable_id(),
        before_state,
        transition,
        command,
        state,
        revision,
        history,
        sequence,
    )
}

fn record_transition_step(
    step_id: &str,
    transition_id: &str,
    transition: SketchInteractionTransition,
    state: &mut SketchInteractionState,
    revision: &mut u64,
    history: &mut CadHistoryStack,
    sequence: &mut Vec<SketchUndoRedoStepSnapshot>,
) -> CadResult<()> {
    let before_state = state.clone();
    let command = history_command_for_transition(transition_id, &transition).ok_or_else(|| {
        CadError::InvalidPolicy {
            reason: format!(
                "transition `{transition_id}` produced no history command for step `{step_id}`"
            ),
        }
    })?;

    record_history_step(
        step_id,
        transition_id,
        before_state,
        transition,
        command,
        state,
        revision,
        history,
        sequence,
    )
}

#[allow(clippy::too_many_arguments)]
fn record_history_step(
    step_id: &str,
    transition_id: &str,
    before_state: SketchInteractionState,
    transition: SketchInteractionTransition,
    command: CadHistoryCommand,
    state: &mut SketchInteractionState,
    revision: &mut u64,
    history: &mut CadHistoryStack,
    sequence: &mut Vec<SketchUndoRedoStepSnapshot>,
) -> CadResult<()> {
    let (actual_transition_id, command_codes) = extract_sketch_history_payload(&command)?;
    if actual_transition_id != transition_id {
        return Err(CadError::InvalidPolicy {
            reason: format!(
                "transition mismatch for step `{step_id}`: expected `{transition_id}`, got `{actual_transition_id}`"
            ),
        });
    }

    let before_snapshot = history_snapshot_for_state(*revision, &before_state)?;
    *revision += 1;
    let after_snapshot = history_snapshot_for_state(*revision, &transition.next_state)?;
    history.push_transition(command, before_snapshot.clone(), after_snapshot.clone());
    *state = transition.next_state;
    sequence.push(SketchUndoRedoStepSnapshot {
        step_id: step_id.to_string(),
        transition_id: actual_transition_id,
        command_codes,
        before_geometry_hash: before_snapshot.geometry_hash,
        after_geometry_hash: after_snapshot.geometry_hash,
    });
    Ok(())
}

fn verify_redo_cleared_on_new_edit(
    history: &CadHistoryStack,
    terminal_state: &SketchInteractionState,
    revision: u64,
) -> CadResult<bool> {
    let mut branch = history.clone();
    if branch.undo().is_none() || branch.len_redo() == 0 {
        return Ok(false);
    }
    let transition = apply_shortcut(terminal_state, SketchShortcut::EnterSketchMode);
    let Some(command) = history_command_for_shortcut(SketchShortcut::EnterSketchMode, &transition)
    else {
        return Ok(false);
    };
    let before = history_snapshot_for_state(revision + 1, terminal_state)?;
    let after = history_snapshot_for_state(revision + 2, &transition.next_state)?;
    branch.push_transition(command, before, after);
    Ok(branch.len_redo() == 0)
}

fn drain_undo_trace(history: &mut CadHistoryStack) -> CadResult<Vec<SketchUndoRedoTraceSnapshot>> {
    let mut trace = Vec::with_capacity(history.len_undo());
    while let Some(transition) = history.undo() {
        trace.push(trace_entry_from_history_transition(transition)?);
    }
    Ok(trace)
}

fn drain_redo_trace(history: &mut CadHistoryStack) -> CadResult<Vec<SketchUndoRedoTraceSnapshot>> {
    let mut trace = Vec::with_capacity(history.len_redo());
    while let Some(transition) = history.redo() {
        trace.push(trace_entry_from_history_transition(transition)?);
    }
    Ok(trace)
}

fn trace_entry_from_history_transition(
    transition: CadHistoryTransition,
) -> CadResult<SketchUndoRedoTraceSnapshot> {
    let (transition_id, command_codes) = extract_sketch_history_payload(&transition.command)?;
    Ok(SketchUndoRedoTraceSnapshot {
        transition_id,
        command_codes,
        snapshot_revision: transition.snapshot.document_revision,
        snapshot_geometry_hash: transition.snapshot.geometry_hash,
    })
}

fn extract_sketch_history_payload(command: &CadHistoryCommand) -> CadResult<(String, Vec<String>)> {
    match command {
        CadHistoryCommand::ApplySketchInteraction {
            shortcut_id,
            command_codes,
        } => Ok((shortcut_id.clone(), command_codes.clone())),
        _ => Err(CadError::InvalidPolicy {
            reason: "sketch undo/redo parity expected ApplySketchInteraction command".to_string(),
        }),
    }
}

fn history_snapshot_for_state(
    revision: u64,
    state: &SketchInteractionState,
) -> CadResult<CadHistorySnapshot> {
    let state_hash = stable_hash_json(state)?;
    Ok(CadHistorySnapshot {
        document_revision: revision,
        geometry_hash: format!("sketch-state-{state_hash}"),
        stable_ids: BTreeMap::from([
            ("sketch_state_hash".to_string(), state_hash.clone()),
            (
                "sketch_active_tool".to_string(),
                state.active_tool.as_str().to_string(),
            ),
            (
                "sketch_mode".to_string(),
                if state.sketch_active {
                    "active".to_string()
                } else {
                    "inactive".to_string()
                },
            ),
        ]),
        warnings: Vec::new(),
        analysis: CadAnalysis {
            document_revision: revision,
            variant_id: "variant.sketch.undo-redo.parity.v1".to_string(),
            material_id: None,
            volume_mm3: None,
            mass_kg: None,
            center_of_gravity_mm: None,
            estimated_cost_usd: None,
            max_deflection_mm: None,
            estimator_metadata: BTreeMap::from([("sketch.state_hash".to_string(), state_hash)]),
            objective_scores: BTreeMap::new(),
        },
    })
}

fn stable_hash_json<T: Serialize>(value: &T) -> CadResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| CadError::ParseFailed {
        reason: format!("failed to serialize sketch undo/redo parity payload: {error}"),
    })?;
    Ok(short_sha256(&bytes))
}

fn short_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn parity_signature(evidence: &ReplayEvidence, deterministic_replay_match: bool) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            VCAD_SKETCH_UNDO_REDO_MAX_STEPS,
            &evidence.sequence,
            &evidence.undo_trace,
            &evidence.redo_trace,
            evidence.undo_trace_matches_reverse_sequence,
            evidence.redo_trace_matches_forward_sequence,
            evidence.redo_cleared_on_new_edit,
            deterministic_replay_match,
        ))
        .expect("serialize sketch undo/redo parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_SKETCH_UNDO_REDO_ISSUE_ID, build_sketch_undo_redo_parity_manifest};
    use crate::parity::scorecard::{
        ParityScorecard, ScorecardCurrent, ScorecardEvaluation, ScorecardThresholdProfile,
    };

    fn mock_scorecard() -> ParityScorecard {
        ParityScorecard {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-005".to_string(),
            vcad_commit: "vcad".to_string(),
            openagents_commit: "openagents".to_string(),
            generated_from_gap_matrix: "gap".to_string(),
            current: ScorecardCurrent {
                docs_match_rate: 0.0,
                crates_match_rate: 0.0,
                commands_match_rate: 0.0,
                overall_match_rate: 0.0,
                docs_reference_count: 0,
                crates_reference_count: 0,
                commands_reference_count: 0,
            },
            threshold_profiles: vec![ScorecardThresholdProfile {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_match_rate_min: 0.0,
                crates_match_rate_min: 0.0,
                commands_match_rate_min: 0.0,
                overall_match_rate_min: 0.0,
            }],
            evaluations: vec![ScorecardEvaluation {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_pass: true,
                crates_pass: true,
                commands_pass: true,
                overall_pass: true,
                pass: true,
            }],
        }
    }

    #[test]
    fn sketch_undo_redo_manifest_captures_replay_invariants() {
        let manifest = build_sketch_undo_redo_parity_manifest(&mock_scorecard(), "scorecard.json")
            .expect("build sketch undo/redo parity manifest");

        assert_eq!(manifest.issue_id, PARITY_SKETCH_UNDO_REDO_ISSUE_ID);
        assert_eq!(manifest.history_max_steps, 50);
        assert_eq!(manifest.undo_binding, "Cmd/Ctrl+Z");
        assert_eq!(manifest.redo_binding, "Cmd/Ctrl+Shift+Z");
        assert_eq!(manifest.sequence.len(), 7);
        assert!(
            manifest
                .sequence
                .iter()
                .any(|entry| entry.transition_id == "sketch.exit.confirm")
        );
        assert!(manifest.undo_trace_matches_reverse_sequence);
        assert!(manifest.redo_trace_matches_forward_sequence);
        assert!(manifest.redo_cleared_on_new_edit);
        assert!(manifest.deterministic_replay_match);
    }
}
