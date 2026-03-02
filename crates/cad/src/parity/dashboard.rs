use serde::{Deserialize, Serialize};

use crate::parity::ci_artifacts::ParityCiArtifactManifest;
use crate::parity::risk_register::ParityRiskRegister;
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_DASHBOARD_ISSUE_ID: &str = "VCAD-PARITY-010";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityDashboard {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from: DashboardSources,
    pub summary: DashboardSummary,
    pub profile_status: Vec<DashboardProfileStatus>,
    pub artifacts: DashboardArtifacts,
    pub phase_status: String,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardSources {
    pub scorecard_path: String,
    pub risk_register_path: String,
    pub ci_manifest_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DashboardSummary {
    pub docs_match_rate: f64,
    pub crates_match_rate: f64,
    pub commands_match_rate: f64,
    pub overall_match_rate: f64,
    pub open_risks: usize,
    pub open_hard_blockers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardProfileStatus {
    pub profile_id: String,
    pub lane: String,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DashboardArtifacts {
    pub source_artifact_count: usize,
    pub artifact_ids: Vec<String>,
}

pub fn build_dashboard(
    scorecard: &ParityScorecard,
    risk_register: &ParityRiskRegister,
    ci_manifest: &ParityCiArtifactManifest,
    scorecard_path: &str,
    risk_register_path: &str,
    ci_manifest_path: &str,
) -> ParityDashboard {
    let summary = DashboardSummary {
        docs_match_rate: scorecard.current.docs_match_rate,
        crates_match_rate: scorecard.current.crates_match_rate,
        commands_match_rate: scorecard.current.commands_match_rate,
        overall_match_rate: scorecard.current.overall_match_rate,
        open_risks: risk_register.summary.open_total,
        open_hard_blockers: risk_register.summary.open_hard_blockers,
    };

    let mut profile_status = Vec::new();
    profile_status.extend(
        scorecard
            .evaluations
            .iter()
            .map(|evaluation| DashboardProfileStatus {
                profile_id: evaluation.profile_id.clone(),
                lane: "scorecard".to_string(),
                pass: evaluation.pass,
            }),
    );
    profile_status.extend(risk_register.workflow.evaluations.iter().map(|evaluation| {
        DashboardProfileStatus {
            profile_id: evaluation.profile_id.clone(),
            lane: "risk_register".to_string(),
            pass: evaluation.pass,
        }
    }));
    profile_status.sort_by(|left, right| {
        left.profile_id
            .cmp(&right.profile_id)
            .then_with(|| left.lane.cmp(&right.lane))
    });

    let mut artifact_ids: Vec<String> = ci_manifest
        .artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect();
    artifact_ids.sort();
    let artifacts = DashboardArtifacts {
        source_artifact_count: ci_manifest.source_artifact_count,
        artifact_ids,
    };

    let baseline_scorecard_pass = scorecard
        .evaluations
        .iter()
        .any(|evaluation| evaluation.profile_id == "phase_a_baseline_v1" && evaluation.pass);
    let baseline_risk_pass = risk_register
        .workflow
        .evaluations
        .iter()
        .any(|evaluation| evaluation.profile_id == "phase_a_baseline_v1" && evaluation.pass);
    let has_phase_c_checkpoint = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "core_modeling_checkpoint_parity_manifest");
    let has_phase_d_entity_set = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_entity_set_parity_manifest");
    let has_phase_d_sketch_plane = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_plane_parity_manifest");
    let has_phase_d_constraint_enum = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_constraint_enum_parity_manifest");
    let has_phase_d_iterative_lm = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_iterative_lm_parity_manifest");
    let has_phase_d_jacobian_residual = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_jacobian_residual_parity_manifest");
    let has_phase_d_constraint_status = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_constraint_status_parity_manifest");
    let has_phase_d_extrude = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_extrude_parity_manifest");
    let has_phase_d_revolve = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_revolve_parity_manifest");
    let has_phase_d_sweep = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_sweep_parity_manifest");
    let has_phase_d_loft = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_loft_parity_manifest");
    let has_phase_d_profile_validity = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_profile_validity_parity_manifest");
    let has_phase_d_interaction = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_interaction_parity_manifest");
    let has_phase_d_undo_redo = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_undo_redo_parity_manifest");
    let has_phase_d_fixture_equivalence = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_fixture_equivalence_parity_manifest");
    let has_phase_d_checkpoint = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "sketch_constraints_checkpoint_parity_manifest");
    let has_phase_e_assembly_schema = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "assembly_schema_parity_manifest");
    let has_phase_e_part_instance = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "assembly_part_instance_parity_manifest");
    let has_phase_e_joint_frs = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "assembly_joint_frs_parity_manifest");
    let has_phase_e_joint_cb = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "assembly_joint_cb_parity_manifest");
    let has_phase_e_joint_limits_state = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "assembly_joint_limits_state_parity_manifest");
    let has_phase_e_fk = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "assembly_fk_parity_manifest");
    let has_phase_e_ground_delete = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "assembly_ground_delete_parity_manifest");
    let has_phase_e_ui_selection_edit = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "assembly_ui_selection_edit_parity_manifest");
    let has_phase_e_serialization_replay = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "assembly_serialization_replay_parity_manifest");
    let has_phase_e_acceptance_scenes = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "assembly_acceptance_scenes_parity_manifest");
    let has_phase_e_checkpoint = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "assembly_checkpoint_parity_manifest");
    let has_phase_f_drafting_kernel_scaffolding = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "drafting_kernel_scaffolding_parity_manifest");
    let has_phase_f_projection = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "drafting_projection_parity_manifest");
    let has_phase_f_hidden_line = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "drafting_hidden_line_parity_manifest");
    let has_phase_f_dimension = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "drafting_dimension_parity_manifest");
    let has_phase_f_gdt = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "drafting_gdt_parity_manifest");
    let has_phase_f_section = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "drafting_section_parity_manifest");
    let has_phase_f_detail = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "drafting_detail_parity_manifest");
    let has_phase_f_drawing_mode_ui = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "drafting_drawing_mode_ui_parity_manifest");
    let has_phase_f_drawing_persistence = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "drafting_persistence_parity_manifest");
    let has_phase_f_dxf_export = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "drafting_dxf_export_parity_manifest");
    let has_phase_f_pdf_export = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "drafting_pdf_export_parity_manifest");
    let has_phase_f_checkpoint = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "drafting_checkpoint_parity_manifest");
    let has_phase_g_step_import = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "step_import_entity_parity_manifest");
    let has_phase_g_step_export_post_boolean = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "step_export_post_boolean_parity_manifest");
    let has_phase_g_stl_import_export = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "stl_import_export_parity_manifest");
    let has_phase_g_glb_export = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "glb_export_parity_manifest");
    let has_phase_g_cad_cli_scaffold = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "cad_cli_scaffold_parity_manifest");
    let has_phase_g_cad_cli_commands = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "cad_cli_commands_parity_manifest");
    let has_phase_g_cad_mcp_tools = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "cad_mcp_tools_parity_manifest");
    let has_phase_g_compact_ir = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "compact_ir_parity_manifest");
    let has_phase_g_intent_modeling = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "intent_modeling_parity_manifest");
    let has_phase_g_text_to_cad = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "text_to_cad_parity_manifest");
    let has_phase_g_text_to_cad_dataset = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "text_to_cad_dataset_parity_manifest");
    let has_phase_g_text_to_cad_training_eval = artifacts
        .artifact_ids
        .iter()
        .any(|artifact_id| artifact_id == "text_to_cad_training_eval_parity_manifest");
    let phase_status = if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
        && has_phase_f_drawing_persistence
        && has_phase_f_dxf_export
        && has_phase_f_pdf_export
        && has_phase_f_checkpoint
        && has_phase_g_step_import
        && has_phase_g_step_export_post_boolean
        && has_phase_g_stl_import_export
        && has_phase_g_glb_export
        && has_phase_g_cad_cli_scaffold
        && has_phase_g_cad_cli_commands
        && has_phase_g_cad_mcp_tools
        && has_phase_g_compact_ir
        && has_phase_g_intent_modeling
    {
        if has_phase_g_text_to_cad {
            if has_phase_g_text_to_cad_dataset {
                if has_phase_g_text_to_cad_training_eval {
                    "phase_g_training_eval_hooks_complete".to_string()
                } else {
                    "phase_g_text_to_cad_dataset_complete".to_string()
                }
            } else {
                "phase_g_text_to_cad_complete".to_string()
            }
        } else {
            "phase_g_intent_modeling_complete".to_string()
        }
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
        && has_phase_f_drawing_persistence
        && has_phase_f_dxf_export
        && has_phase_f_pdf_export
        && has_phase_f_checkpoint
        && has_phase_g_step_import
        && has_phase_g_step_export_post_boolean
        && has_phase_g_stl_import_export
        && has_phase_g_glb_export
        && has_phase_g_cad_cli_scaffold
        && has_phase_g_cad_cli_commands
        && has_phase_g_cad_mcp_tools
        && has_phase_g_compact_ir
    {
        "phase_g_compact_ir_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
        && has_phase_f_drawing_persistence
        && has_phase_f_dxf_export
        && has_phase_f_pdf_export
        && has_phase_f_checkpoint
        && has_phase_g_step_import
        && has_phase_g_step_export_post_boolean
        && has_phase_g_stl_import_export
        && has_phase_g_glb_export
        && has_phase_g_cad_cli_scaffold
        && has_phase_g_cad_cli_commands
        && has_phase_g_cad_mcp_tools
    {
        "phase_g_cad_mcp_tools_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
        && has_phase_f_drawing_persistence
        && has_phase_f_dxf_export
        && has_phase_f_pdf_export
        && has_phase_f_checkpoint
        && has_phase_g_step_import
        && has_phase_g_step_export_post_boolean
        && has_phase_g_stl_import_export
        && has_phase_g_glb_export
        && has_phase_g_cad_cli_scaffold
        && has_phase_g_cad_cli_commands
    {
        "phase_g_cad_cli_commands_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
        && has_phase_f_drawing_persistence
        && has_phase_f_dxf_export
        && has_phase_f_pdf_export
        && has_phase_f_checkpoint
        && has_phase_g_step_import
        && has_phase_g_step_export_post_boolean
        && has_phase_g_stl_import_export
        && has_phase_g_glb_export
        && has_phase_g_cad_cli_scaffold
    {
        "phase_g_cad_cli_scaffold_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
        && has_phase_f_drawing_persistence
        && has_phase_f_dxf_export
        && has_phase_f_pdf_export
        && has_phase_f_checkpoint
        && has_phase_g_step_import
        && has_phase_g_step_export_post_boolean
        && has_phase_g_stl_import_export
        && has_phase_g_glb_export
    {
        "phase_g_glb_export_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
        && has_phase_f_drawing_persistence
        && has_phase_f_dxf_export
        && has_phase_f_pdf_export
        && has_phase_f_checkpoint
        && has_phase_g_step_import
        && has_phase_g_step_export_post_boolean
        && has_phase_g_stl_import_export
    {
        "phase_g_stl_import_export_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
        && has_phase_f_drawing_persistence
        && has_phase_f_dxf_export
        && has_phase_f_pdf_export
        && has_phase_f_checkpoint
        && has_phase_g_step_import
        && has_phase_g_step_export_post_boolean
    {
        "phase_g_step_export_post_boolean_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
        && has_phase_f_drawing_persistence
        && has_phase_f_dxf_export
        && has_phase_f_pdf_export
        && has_phase_f_checkpoint
        && has_phase_g_step_import
    {
        "phase_g_step_import_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
        && has_phase_f_drawing_persistence
        && has_phase_f_dxf_export
        && has_phase_f_pdf_export
        && has_phase_f_checkpoint
    {
        "phase_f_checkpoint_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
        && has_phase_f_drawing_persistence
        && has_phase_f_dxf_export
        && has_phase_f_pdf_export
    {
        "phase_f_pdf_export_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
        && has_phase_f_drawing_persistence
        && has_phase_f_dxf_export
    {
        "phase_f_dxf_export_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
        && has_phase_f_drawing_persistence
    {
        "phase_f_drawing_persistence_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
        && has_phase_f_drawing_mode_ui
    {
        "phase_f_drawing_mode_ui_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
        && has_phase_f_detail
    {
        "phase_f_detail_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
        && has_phase_f_section
    {
        "phase_f_section_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
        && has_phase_f_gdt
    {
        "phase_f_gdt_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
        && has_phase_f_dimension
    {
        "phase_f_dimension_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
        && has_phase_f_hidden_line
    {
        "phase_f_hidden_line_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
        && has_phase_f_projection
    {
        "phase_f_projection_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
        && has_phase_f_drafting_kernel_scaffolding
    {
        "phase_f_drafting_kernel_scaffolding_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
        && has_phase_e_checkpoint
    {
        "phase_e_checkpoint_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
        && has_phase_e_acceptance_scenes
    {
        "phase_e_acceptance_scenes_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
        && has_phase_e_serialization_replay
    {
        "phase_e_serialization_replay_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
        && has_phase_e_ui_selection_edit
    {
        "phase_e_ui_selection_edit_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
        && has_phase_e_ground_delete
    {
        "phase_e_ground_delete_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
        && has_phase_e_fk
    {
        "phase_e_fk_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
        && has_phase_e_joint_limits_state
    {
        "phase_e_joint_limits_state_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
        && has_phase_e_joint_cb
    {
        "phase_e_joint_cb_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
        && has_phase_e_joint_frs
    {
        "phase_e_joint_frs_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
        && has_phase_e_part_instance
    {
        "phase_e_part_instance_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
        && has_phase_e_assembly_schema
    {
        "phase_e_assembly_schema_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
        && has_phase_d_checkpoint
    {
        "phase_d_checkpoint_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
        && has_phase_d_fixture_equivalence
    {
        "phase_d_fixture_equivalence_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
        && has_phase_d_undo_redo
    {
        "phase_d_undo_redo_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
        && has_phase_d_interaction
    {
        "phase_d_interaction_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
        && has_phase_d_profile_validity
    {
        "phase_d_profile_validity_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
        && has_phase_d_loft
    {
        "phase_d_loft_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
        && has_phase_d_sweep
    {
        "phase_d_sweep_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
        && has_phase_d_revolve
    {
        "phase_d_revolve_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
        && has_phase_d_extrude
    {
        "phase_d_extrude_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
        && has_phase_d_constraint_status
    {
        "phase_d_constraint_status_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
        && has_phase_d_jacobian_residual
    {
        "phase_d_jacobian_residual_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
        && has_phase_d_iterative_lm
    {
        "phase_d_iterative_lm_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
        && has_phase_d_constraint_enum
    {
        "phase_d_constraint_enum_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
        && has_phase_d_sketch_plane
    {
        "phase_d_sketch_plane_complete".to_string()
    } else if baseline_scorecard_pass
        && baseline_risk_pass
        && has_phase_c_checkpoint
        && has_phase_d_entity_set
    {
        "phase_d_sketch_entity_set_complete".to_string()
    } else if baseline_scorecard_pass && baseline_risk_pass && has_phase_c_checkpoint {
        "phase_c_core_modeling_complete".to_string()
    } else if baseline_scorecard_pass && baseline_risk_pass {
        "phase_a_baseline_complete".to_string()
    } else {
        "phase_a_baseline_at_risk".to_string()
    };
    let next_actions = if phase_status == "phase_c_core_modeling_complete" {
        vec![
            "Execute VCAD-PARITY-041 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_g_training_eval_hooks_complete" {
        vec![
            "Execute VCAD-PARITY-091 through VCAD-PARITY-092 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_g_text_to_cad_dataset_complete" {
        vec![
            "Execute VCAD-PARITY-090 through VCAD-PARITY-092 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_g_text_to_cad_complete" {
        vec![
            "Execute VCAD-PARITY-089 through VCAD-PARITY-092 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_g_intent_modeling_complete" {
        vec![
            "Execute VCAD-PARITY-088 through VCAD-PARITY-092 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_g_compact_ir_complete" {
        vec![
            "Execute VCAD-PARITY-087 through VCAD-PARITY-092 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_g_cad_mcp_tools_complete" {
        vec![
            "Execute VCAD-PARITY-086 through VCAD-PARITY-092 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_g_cad_cli_commands_complete" {
        vec![
            "Execute VCAD-PARITY-085 through VCAD-PARITY-092 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_g_cad_cli_scaffold_complete" {
        vec![
            "Execute VCAD-PARITY-084 through VCAD-PARITY-092 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_g_glb_export_complete" {
        vec![
            "Execute VCAD-PARITY-083 through VCAD-PARITY-092 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_g_stl_import_export_complete" {
        vec![
            "Execute VCAD-PARITY-082 through VCAD-PARITY-092 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_g_step_export_post_boolean_complete" {
        vec![
            "Execute VCAD-PARITY-081 through VCAD-PARITY-092 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_g_step_import_complete" {
        vec![
            "Execute VCAD-PARITY-080 through VCAD-PARITY-092 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_f_checkpoint_complete" {
        vec![
            "Execute VCAD-PARITY-079 through VCAD-PARITY-092 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_f_pdf_export_complete" {
        vec![
            "Execute VCAD-PARITY-078 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_f_dxf_export_complete" {
        vec![
            "Execute VCAD-PARITY-077 through VCAD-PARITY-078 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_f_drawing_persistence_complete" {
        vec![
            "Execute VCAD-PARITY-076 through VCAD-PARITY-078 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_f_drawing_mode_ui_complete" {
        vec![
            "Execute VCAD-PARITY-075 through VCAD-PARITY-078 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_f_detail_complete" {
        vec![
            "Execute VCAD-PARITY-074 through VCAD-PARITY-078 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_f_section_complete" {
        vec![
            "Execute VCAD-PARITY-073 through VCAD-PARITY-078 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_f_gdt_complete" {
        vec![
            "Execute VCAD-PARITY-072 through VCAD-PARITY-078 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_f_dimension_complete" {
        vec![
            "Execute VCAD-PARITY-071 through VCAD-PARITY-078 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_f_hidden_line_complete" {
        vec![
            "Execute VCAD-PARITY-070 through VCAD-PARITY-078 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_f_projection_complete" {
        vec![
            "Execute VCAD-PARITY-069 through VCAD-PARITY-078 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_f_drafting_kernel_scaffolding_complete" {
        vec![
            "Execute VCAD-PARITY-068 through VCAD-PARITY-078 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_e_checkpoint_complete" {
        vec![
            "Execute VCAD-PARITY-067 through VCAD-PARITY-078 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_e_acceptance_scenes_complete" {
        vec![
            "Execute VCAD-PARITY-066 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_e_serialization_replay_complete" {
        vec![
            "Execute VCAD-PARITY-065 through VCAD-PARITY-066 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_e_ui_selection_edit_complete" {
        vec![
            "Execute VCAD-PARITY-064 through VCAD-PARITY-066 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_e_ground_delete_complete" {
        vec![
            "Execute VCAD-PARITY-063 through VCAD-PARITY-066 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_e_fk_complete" {
        vec![
            "Execute VCAD-PARITY-062 through VCAD-PARITY-066 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_e_joint_limits_state_complete" {
        vec![
            "Execute VCAD-PARITY-061 through VCAD-PARITY-066 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_e_joint_cb_complete" {
        vec![
            "Execute VCAD-PARITY-060 through VCAD-PARITY-066 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_e_joint_frs_complete" {
        vec![
            "Execute VCAD-PARITY-059 through VCAD-PARITY-066 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_e_part_instance_complete" {
        vec![
            "Execute VCAD-PARITY-058 through VCAD-PARITY-066 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_e_assembly_schema_complete" {
        vec![
            "Execute VCAD-PARITY-057 through VCAD-PARITY-066 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_sketch_entity_set_complete" {
        vec![
            "Execute VCAD-PARITY-042 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_sketch_plane_complete" {
        vec![
            "Execute VCAD-PARITY-043 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_constraint_enum_complete" {
        vec![
            "Execute VCAD-PARITY-044 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_iterative_lm_complete" {
        vec![
            "Execute VCAD-PARITY-045 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_jacobian_residual_complete" {
        vec![
            "Execute VCAD-PARITY-046 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_checkpoint_complete" {
        vec![
            "Execute VCAD-PARITY-056 through VCAD-PARITY-066 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_fixture_equivalence_complete" {
        vec![
            "Execute VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_undo_redo_complete" {
        vec![
            "Execute VCAD-PARITY-054 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_interaction_complete" {
        vec![
            "Execute VCAD-PARITY-053 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_profile_validity_complete" {
        vec![
            "Execute VCAD-PARITY-052 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_loft_complete" {
        vec![
            "Execute VCAD-PARITY-051 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_sweep_complete" {
        vec![
            "Execute VCAD-PARITY-050 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_revolve_complete" {
        vec![
            "Execute VCAD-PARITY-049 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_extrude_complete" {
        vec![
            "Execute VCAD-PARITY-048 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else if phase_status == "phase_d_constraint_status_complete" {
        vec![
            "Execute VCAD-PARITY-047 through VCAD-PARITY-055 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    } else {
        vec![
            "Execute VCAD-PARITY-011 through VCAD-PARITY-025 sequentially".to_string(),
            "Keep phase_a_baseline_v1 profile passing in scorecard and risk register lanes"
                .to_string(),
            "Refresh parity dashboard after each closed parity issue".to_string(),
        ]
    };

    ParityDashboard {
        manifest_version: 1,
        issue_id: PARITY_DASHBOARD_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from: DashboardSources {
            scorecard_path: scorecard_path.to_string(),
            risk_register_path: risk_register_path.to_string(),
            ci_manifest_path: ci_manifest_path.to_string(),
        },
        summary,
        profile_status,
        artifacts,
        phase_status,
        next_actions,
    }
}

pub fn render_dashboard_markdown(dashboard: &ParityDashboard) -> String {
    let mut lines = Vec::new();
    lines.push("# Baseline Parity Dashboard".to_string());
    lines.push(String::new());
    lines.push(format!("Issue coverage: `{}`", dashboard.issue_id));
    lines.push(String::new());
    lines.push("## Snapshot".to_string());
    lines.push(String::new());
    lines.push("| Metric | Value |".to_string());
    lines.push("| --- | --- |".to_string());
    lines.push(format!("| vcad commit | `{}` |", dashboard.vcad_commit));
    lines.push(format!(
        "| openagents commit (plan baseline) | `{}` |",
        dashboard.openagents_commit
    ));
    lines.push(format!("| phase status | `{}` |", dashboard.phase_status));
    lines.push(format!(
        "| overall match rate | `{:.6}` |",
        dashboard.summary.overall_match_rate
    ));
    lines.push(format!(
        "| docs match rate | `{:.6}` |",
        dashboard.summary.docs_match_rate
    ));
    lines.push(format!(
        "| crates match rate | `{:.6}` |",
        dashboard.summary.crates_match_rate
    ));
    lines.push(format!(
        "| commands match rate | `{:.6}` |",
        dashboard.summary.commands_match_rate
    ));
    lines.push(format!(
        "| open risks | `{}` |",
        dashboard.summary.open_risks
    ));
    lines.push(format!(
        "| open hard blockers (p0) | `{}` |",
        dashboard.summary.open_hard_blockers
    ));
    lines.push(format!(
        "| CI source artifact count | `{}` |",
        dashboard.artifacts.source_artifact_count
    ));
    lines.push(String::new());
    lines.push("## Profile Gates".to_string());
    lines.push(String::new());
    lines.push("| Lane | Profile | Pass |".to_string());
    lines.push("| --- | --- | --- |".to_string());
    for profile in &dashboard.profile_status {
        lines.push(format!(
            "| `{}` | `{}` | `{}` |",
            profile.lane, profile.profile_id, profile.pass
        ));
    }
    lines.push(String::new());
    lines.push("## CI Evidence Artifacts".to_string());
    lines.push(String::new());
    for artifact_id in &dashboard.artifacts.artifact_ids {
        lines.push(format!("- `{}`", artifact_id));
    }
    lines.push(String::new());
    lines.push("## Next Actions".to_string());
    lines.push(String::new());
    for action in &dashboard.next_actions {
        lines.push(format!("- {}", action));
    }
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{ParityDashboard, render_dashboard_markdown};

    #[test]
    fn render_dashboard_markdown_emits_header() {
        let dashboard = ParityDashboard {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-010".to_string(),
            vcad_commit: "vcad".to_string(),
            openagents_commit: "openagents".to_string(),
            generated_from: super::DashboardSources {
                scorecard_path: "a".to_string(),
                risk_register_path: "b".to_string(),
                ci_manifest_path: "c".to_string(),
            },
            summary: super::DashboardSummary {
                docs_match_rate: 0.1,
                crates_match_rate: 0.1,
                commands_match_rate: 0.1,
                overall_match_rate: 0.1,
                open_risks: 1,
                open_hard_blockers: 1,
            },
            profile_status: vec![super::DashboardProfileStatus {
                profile_id: "phase_a_baseline_v1".to_string(),
                lane: "scorecard".to_string(),
                pass: true,
            }],
            artifacts: super::DashboardArtifacts {
                source_artifact_count: 1,
                artifact_ids: vec!["artifact".to_string()],
            },
            phase_status: "phase_d_constraint_enum_complete".to_string(),
            next_actions: vec!["x".to_string()],
        };
        let markdown = render_dashboard_markdown(&dashboard);
        assert!(markdown.contains("# Baseline Parity Dashboard"));
        assert!(markdown.contains("phase_d_constraint_enum_complete"));
    }
}
