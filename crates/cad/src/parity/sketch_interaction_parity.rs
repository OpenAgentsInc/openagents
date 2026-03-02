use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;
use crate::sketch_interaction::{
    SKETCH_SHORTCUT_SEQUENCE, SketchInteractionState, SketchInteractionTransition, SketchShortcut,
    apply_exit_cancel, apply_exit_confirm, apply_face_selection_confirm, apply_shortcut,
};
use crate::{CadError, CadResult};

pub const PARITY_SKETCH_INTERACTION_ISSUE_ID: &str = "VCAD-PARITY-052";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchInteractionParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub vcad_reference_sources: Vec<String>,
    pub shortcut_bindings: Vec<SketchShortcutBindingSnapshot>,
    pub shortcut_catalog_hash: String,
    pub enter_without_parts_case: SketchInteractionCaseSnapshot,
    pub enter_with_parts_case: SketchInteractionCaseSnapshot,
    pub face_selection_confirm_case: SketchInteractionCaseSnapshot,
    pub line_tool_case: SketchInteractionCaseSnapshot,
    pub rectangle_tool_case: SketchInteractionCaseSnapshot,
    pub circle_tool_case: SketchInteractionCaseSnapshot,
    pub horizontal_constraint_case: SketchInteractionCaseSnapshot,
    pub vertical_constraint_case: SketchInteractionCaseSnapshot,
    pub finish_shape_case: SketchInteractionCaseSnapshot,
    pub escape_cancel_shape_case: SketchInteractionCaseSnapshot,
    pub escape_request_exit_case: SketchInteractionCaseSnapshot,
    pub escape_keep_editing_case: SketchInteractionCaseSnapshot,
    pub escape_confirm_exit_case: SketchInteractionCaseSnapshot,
    pub constraint_shortcut_requires_single_selection: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SketchShortcutBindingSnapshot {
    pub key: String,
    pub shortcut_id: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SketchInteractionCaseSnapshot {
    pub command_codes: Vec<String>,
    pub sketch_active: bool,
    pub face_selection_mode: bool,
    pub pending_exit_confirmation: bool,
    pub active_tool: String,
    pub pending_points: usize,
    pub segment_count: usize,
    pub selected_line_count: usize,
    pub state_hash: String,
}

#[derive(Debug, Clone, PartialEq)]
struct CaseBundle {
    enter_without_parts_case: SketchInteractionCaseSnapshot,
    enter_with_parts_case: SketchInteractionCaseSnapshot,
    face_selection_confirm_case: SketchInteractionCaseSnapshot,
    line_tool_case: SketchInteractionCaseSnapshot,
    rectangle_tool_case: SketchInteractionCaseSnapshot,
    circle_tool_case: SketchInteractionCaseSnapshot,
    horizontal_constraint_case: SketchInteractionCaseSnapshot,
    vertical_constraint_case: SketchInteractionCaseSnapshot,
    finish_shape_case: SketchInteractionCaseSnapshot,
    escape_cancel_shape_case: SketchInteractionCaseSnapshot,
    escape_request_exit_case: SketchInteractionCaseSnapshot,
    escape_keep_editing_case: SketchInteractionCaseSnapshot,
    escape_confirm_exit_case: SketchInteractionCaseSnapshot,
    constraint_shortcut_requires_single_selection: bool,
}

pub fn build_sketch_interaction_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<SketchInteractionParityManifest> {
    let shortcut_bindings = SKETCH_SHORTCUT_SEQUENCE
        .iter()
        .map(|shortcut| SketchShortcutBindingSnapshot {
            key: shortcut.key_binding().to_string(),
            shortcut_id: shortcut.stable_id().to_string(),
            action: shortcut_action(*shortcut).to_string(),
        })
        .collect::<Vec<_>>();
    let shortcut_catalog_hash = stable_hash_json(&shortcut_bindings)?;

    let cases = build_case_bundle()?;
    let replay_cases = build_case_bundle()?;
    let deterministic_replay_match = cases == replay_cases;
    let deterministic_signature = parity_signature(
        &shortcut_bindings,
        &cases,
        cases.constraint_shortcut_requires_single_selection,
        deterministic_replay_match,
    );

    Ok(SketchInteractionParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SKETCH_INTERACTION_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        vcad_reference_sources: vec![
            "docs/features/sketch-mode.md (Interaction Flow + Keyboard Shortcuts)".to_string(),
            "packages/app/src/hooks/useKeyboardShortcuts.ts (sketch shortcut dispatch)".to_string(),
            "packages/core/src/stores/sketch-store.ts (editing flow + exit confirmation)"
                .to_string(),
        ],
        shortcut_bindings,
        shortcut_catalog_hash,
        enter_without_parts_case: cases.enter_without_parts_case,
        enter_with_parts_case: cases.enter_with_parts_case,
        face_selection_confirm_case: cases.face_selection_confirm_case,
        line_tool_case: cases.line_tool_case,
        rectangle_tool_case: cases.rectangle_tool_case,
        circle_tool_case: cases.circle_tool_case,
        horizontal_constraint_case: cases.horizontal_constraint_case,
        vertical_constraint_case: cases.vertical_constraint_case,
        finish_shape_case: cases.finish_shape_case,
        escape_cancel_shape_case: cases.escape_cancel_shape_case,
        escape_request_exit_case: cases.escape_request_exit_case,
        escape_keep_editing_case: cases.escape_keep_editing_case,
        escape_confirm_exit_case: cases.escape_confirm_exit_case,
        constraint_shortcut_requires_single_selection: cases
            .constraint_shortcut_requires_single_selection,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "sketch shortcut catalog maps S/L/R/C/H/V/Enter/Escape to deterministic editing actions"
                .to_string(),
            "enter-sketch behavior follows vcad flow: face selection when parts exist, direct XY sketch otherwise".to_string(),
            "Escape follows deterministic cancel/request-exit/confirm-exit progression with pending geometry awareness".to_string(),
            "constraint shortcuts only apply with valid selection cardinality and emit solver run command deterministically".to_string(),
        ],
    })
}

fn build_case_bundle() -> CadResult<CaseBundle> {
    let enter_without_parts_transition = apply_shortcut(
        &SketchInteractionState::with_has_parts(false),
        SketchShortcut::EnterSketchMode,
    );
    let enter_without_parts_case = case_snapshot(&enter_without_parts_transition)?;

    let enter_with_parts_transition = apply_shortcut(
        &SketchInteractionState::with_has_parts(true),
        SketchShortcut::EnterSketchMode,
    );
    let enter_with_parts_case = case_snapshot(&enter_with_parts_transition)?;

    let face_selection_confirm_transition =
        apply_face_selection_confirm(&enter_with_parts_transition.next_state);
    let face_selection_confirm_case = case_snapshot(&face_selection_confirm_transition)?;

    let tool_base_state = face_selection_confirm_transition
        .next_state
        .clone()
        .with_geometry_context(3, 2, 0);
    let line_tool_transition = apply_shortcut(&tool_base_state, SketchShortcut::LineTool);
    let line_tool_case = case_snapshot(&line_tool_transition)?;

    let rectangle_tool_transition = apply_shortcut(
        &line_tool_transition.next_state,
        SketchShortcut::RectangleTool,
    );
    let rectangle_tool_case = case_snapshot(&rectangle_tool_transition)?;

    let circle_tool_transition = apply_shortcut(
        &rectangle_tool_transition.next_state,
        SketchShortcut::CircleTool,
    );
    let circle_tool_case = case_snapshot(&circle_tool_transition)?;

    let constraint_state = circle_tool_transition
        .next_state
        .clone()
        .with_geometry_context(3, 0, 1);
    let horizontal_constraint_transition =
        apply_shortcut(&constraint_state, SketchShortcut::HorizontalConstraint);
    let horizontal_constraint_case = case_snapshot(&horizontal_constraint_transition)?;

    let vertical_constraint_transition =
        apply_shortcut(&constraint_state, SketchShortcut::VerticalConstraint);
    let vertical_constraint_case = case_snapshot(&vertical_constraint_transition)?;

    let invalid_constraint_transition = apply_shortcut(
        &constraint_state.clone().with_geometry_context(3, 0, 2),
        SketchShortcut::HorizontalConstraint,
    );
    let constraint_shortcut_requires_single_selection =
        invalid_constraint_transition.commands.is_empty();

    let finish_shape_transition = apply_shortcut(
        &constraint_state.clone().with_geometry_context(3, 2, 0),
        SketchShortcut::FinishCurrentShape,
    );
    let finish_shape_case = case_snapshot(&finish_shape_transition)?;

    let escape_cancel_shape_transition = apply_shortcut(
        &constraint_state.clone().with_geometry_context(3, 1, 0),
        SketchShortcut::Escape,
    );
    let escape_cancel_shape_case = case_snapshot(&escape_cancel_shape_transition)?;

    let escape_request_exit_transition = apply_shortcut(
        &escape_cancel_shape_transition.next_state,
        SketchShortcut::Escape,
    );
    let escape_request_exit_case = case_snapshot(&escape_request_exit_transition)?;

    let escape_keep_editing_transition =
        apply_exit_cancel(&escape_request_exit_transition.next_state);
    let escape_keep_editing_case = case_snapshot(&escape_keep_editing_transition)?;

    let escape_re_request_transition = apply_shortcut(
        &escape_keep_editing_transition.next_state,
        SketchShortcut::Escape,
    );
    let escape_confirm_exit_transition =
        apply_exit_confirm(&escape_re_request_transition.next_state);
    let escape_confirm_exit_case = case_snapshot(&escape_confirm_exit_transition)?;

    Ok(CaseBundle {
        enter_without_parts_case,
        enter_with_parts_case,
        face_selection_confirm_case,
        line_tool_case,
        rectangle_tool_case,
        circle_tool_case,
        horizontal_constraint_case,
        vertical_constraint_case,
        finish_shape_case,
        escape_cancel_shape_case,
        escape_request_exit_case,
        escape_keep_editing_case,
        escape_confirm_exit_case,
        constraint_shortcut_requires_single_selection,
    })
}

fn case_snapshot(
    transition: &SketchInteractionTransition,
) -> CadResult<SketchInteractionCaseSnapshot> {
    let command_codes = transition
        .commands
        .iter()
        .map(|command| command.stable_code())
        .collect::<Vec<_>>();
    let state_hash = stable_hash_json(&transition.next_state)?;
    let state = &transition.next_state;

    Ok(SketchInteractionCaseSnapshot {
        command_codes,
        sketch_active: state.sketch_active,
        face_selection_mode: state.face_selection_mode,
        pending_exit_confirmation: state.pending_exit_confirmation,
        active_tool: state.active_tool.as_str().to_string(),
        pending_points: state.pending_points,
        segment_count: state.segment_count,
        selected_line_count: state.selected_line_count,
        state_hash,
    })
}

fn shortcut_action(shortcut: SketchShortcut) -> &'static str {
    match shortcut {
        SketchShortcut::EnterSketchMode => "enter sketch mode",
        SketchShortcut::LineTool => "set line tool",
        SketchShortcut::RectangleTool => "set rectangle tool",
        SketchShortcut::CircleTool => "set circle tool",
        SketchShortcut::HorizontalConstraint => "apply horizontal constraint",
        SketchShortcut::VerticalConstraint => "apply vertical constraint",
        SketchShortcut::FinishCurrentShape => "finish current shape",
        SketchShortcut::Escape => "cancel or exit sketch flow",
    }
}

fn stable_hash_json<T: Serialize>(value: &T) -> CadResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| CadError::ParseFailed {
        reason: format!("failed to serialize sketch interaction parity payload: {error}"),
    })?;
    Ok(short_sha256(&bytes))
}

fn short_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn parity_signature(
    shortcut_bindings: &[SketchShortcutBindingSnapshot],
    cases: &CaseBundle,
    constraint_shortcut_requires_single_selection: bool,
    deterministic_replay_match: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            shortcut_bindings,
            &cases.enter_without_parts_case,
            &cases.enter_with_parts_case,
            &cases.face_selection_confirm_case,
            &cases.line_tool_case,
            &cases.rectangle_tool_case,
            &cases.circle_tool_case,
            &cases.horizontal_constraint_case,
            &cases.vertical_constraint_case,
            &cases.finish_shape_case,
            &cases.escape_cancel_shape_case,
            &cases.escape_request_exit_case,
            &cases.escape_keep_editing_case,
            &cases.escape_confirm_exit_case,
            constraint_shortcut_requires_single_selection,
            deterministic_replay_match,
        ))
        .expect("serialize sketch interaction parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_SKETCH_INTERACTION_ISSUE_ID, build_sketch_interaction_parity_manifest};
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
    fn sketch_interaction_manifest_captures_shortcuts_and_exit_flow() {
        let manifest =
            build_sketch_interaction_parity_manifest(&mock_scorecard(), "scorecard.json")
                .expect("build sketch interaction parity manifest");
        assert_eq!(manifest.issue_id, PARITY_SKETCH_INTERACTION_ISSUE_ID);
        assert_eq!(manifest.shortcut_bindings.len(), 8);
        assert!(
            manifest
                .shortcut_bindings
                .iter()
                .any(|binding| binding.key == "S" && binding.shortcut_id == "sketch.enter")
        );
        assert!(manifest.enter_without_parts_case.sketch_active);
        assert!(manifest.enter_with_parts_case.face_selection_mode);
        assert!(manifest.face_selection_confirm_case.sketch_active);
        assert!(
            manifest
                .horizontal_constraint_case
                .command_codes
                .iter()
                .any(|code| code == "SKETCH-CMD-RUN-SOLVER")
        );
        assert!(manifest.constraint_shortcut_requires_single_selection);
        assert!(manifest.escape_request_exit_case.pending_exit_confirmation);
        assert!(!manifest.escape_confirm_exit_case.sketch_active);
        assert!(manifest.deterministic_replay_match);
    }
}
