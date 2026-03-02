use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::assembly::{
    CadAssemblyJoint, CadAssemblySchema, CadAssemblyUiState, CadJointKind, CadPartInstance,
};
use crate::kernel_math::Vec3;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_ASSEMBLY_UI_SELECTION_EDIT_ISSUE_ID: &str = "VCAD-PARITY-063";
pub const ASSEMBLY_UI_SELECTION_EDIT_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/assembly_ui_selection_edit_vcad_reference.json";
const ASSEMBLY_UI_SELECTION_EDIT_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/assembly_ui_selection_edit_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssemblyUiSelectionEditParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub case_snapshots: Vec<AssemblyUiSelectionEditCaseSnapshot>,
    pub behavior_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AssemblyUiSelectionEditReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_case_snapshots: Vec<AssemblyUiSelectionEditCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AssemblyUiSelectionEditSnapshot {
    case_snapshots: Vec<AssemblyUiSelectionEditCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssemblyUiSelectionEditCaseSnapshot {
    pub case_id: String,
    pub selected_instance_id: Option<String>,
    pub selected_joint_id: Option<String>,
    pub schema_instance_ids: Vec<String>,
    pub schema_joint_ids: Vec<String>,
    pub arm_instance_name: Option<String>,
    pub joint_state_deg: Option<String>,
    pub was_clamped: bool,
    pub error: Option<String>,
}

pub fn build_assembly_ui_selection_edit_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<AssemblyUiSelectionEditParityManifest> {
    let corpus: AssemblyUiSelectionEditReferenceCorpus = serde_json::from_str(
        ASSEMBLY_UI_SELECTION_EDIT_REFERENCE_CORPUS_JSON,
    )
    .map_err(|error| CadError::ParseFailed {
        reason: format!("failed to parse assembly UI selection/edit reference corpus: {error}"),
    })?;

    let reference_corpus_sha256 =
        sha256_hex(ASSEMBLY_UI_SELECTION_EDIT_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = run_ui_selection_edit_cases();
    let replay_snapshot = run_ui_selection_edit_cases();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let mut expected_snapshots = corpus.expected_case_snapshots;
    expected_snapshots.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    let behavior_match = snapshot.case_snapshots == expected_snapshots;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        behavior_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(AssemblyUiSelectionEditParityManifest {
        manifest_version: 1,
        issue_id: PARITY_ASSEMBLY_UI_SELECTION_EDIT_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: ASSEMBLY_UI_SELECTION_EDIT_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        case_snapshots: snapshot.case_snapshots,
        behavior_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "assembly UI select instance/joint requires known schema ids".to_string(),
            "rename_selected_instance fails deterministically when no instance is selected"
                .to_string(),
            "set_selected_joint_state applies schema joint-limit clamping semantics".to_string(),
            "sync_with_schema clears stale selected instance/joint ids after deletes".to_string(),
        ],
    })
}

fn run_ui_selection_edit_cases() -> AssemblyUiSelectionEditSnapshot {
    let mut case_snapshots = vec![
        case_instance_select_and_rename(),
        case_joint_select_and_clamp_state(),
        case_rename_without_selection_error(),
        case_select_unknown_instance_error(),
        case_sync_after_delete_clears_selection(),
    ];
    case_snapshots.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    AssemblyUiSelectionEditSnapshot { case_snapshots }
}

fn case_instance_select_and_rename() -> AssemblyUiSelectionEditCaseSnapshot {
    let mut schema = base_schema();
    let mut ui = CadAssemblyUiState::default();

    ui.select_instance(&schema, "arm-1")
        .expect("select known instance");
    ui.rename_selected_instance(&mut schema, "Arm Segment".to_string())
        .expect("rename selected instance");

    snapshot("instance_select_and_rename", &schema, &ui, false, None)
}

fn case_joint_select_and_clamp_state() -> AssemblyUiSelectionEditCaseSnapshot {
    let mut schema = base_schema();
    let mut ui = CadAssemblyUiState::default();

    ui.select_joint(&schema, "joint.hinge")
        .expect("select known joint");
    let semantics = ui
        .set_selected_joint_state(&mut schema, 120.0)
        .expect("set selected joint state");

    snapshot(
        "joint_select_and_clamp_state",
        &schema,
        &ui,
        semantics.was_clamped,
        None,
    )
}

fn case_rename_without_selection_error() -> AssemblyUiSelectionEditCaseSnapshot {
    let mut schema = base_schema();
    let mut ui = CadAssemblyUiState::default();

    let error = ui
        .rename_selected_instance(&mut schema, "Arm Segment".to_string())
        .expect_err("rename without selected instance should fail")
        .to_string();

    snapshot(
        "rename_without_selection_error",
        &schema,
        &ui,
        false,
        Some(error),
    )
}

fn case_select_unknown_instance_error() -> AssemblyUiSelectionEditCaseSnapshot {
    let schema = base_schema();
    let mut ui = CadAssemblyUiState::default();

    let error = ui
        .select_instance(&schema, "missing")
        .expect_err("selecting unknown instance should fail")
        .to_string();

    snapshot(
        "select_unknown_instance_error",
        &schema,
        &ui,
        false,
        Some(error),
    )
}

fn case_sync_after_delete_clears_selection() -> AssemblyUiSelectionEditCaseSnapshot {
    let mut schema = base_schema();
    let mut ui = CadAssemblyUiState::default();

    ui.select_instance(&schema, "arm-1")
        .expect("select known instance");
    ui.select_joint(&schema, "joint.hinge")
        .expect("select known joint");
    schema
        .delete_instance("arm-1")
        .expect("delete selected instance and connected joint");
    ui.sync_with_schema(&schema);

    snapshot(
        "sync_after_delete_clears_selection",
        &schema,
        &ui,
        false,
        None,
    )
}

fn base_schema() -> CadAssemblySchema {
    CadAssemblySchema {
        part_defs: BTreeMap::new(),
        instances: vec![
            CadPartInstance {
                id: "base-1".to_string(),
                part_def_id: "base".to_string(),
                name: Some("Base".to_string()),
                transform: None,
                material: None,
            },
            CadPartInstance {
                id: "arm-1".to_string(),
                part_def_id: "arm".to_string(),
                name: Some("Arm".to_string()),
                transform: None,
                material: None,
            },
        ],
        joints: vec![CadAssemblyJoint {
            id: "joint.hinge".to_string(),
            name: Some("Hinge".to_string()),
            parent_instance_id: Some("base-1".to_string()),
            child_instance_id: "arm-1".to_string(),
            parent_anchor: Vec3::new(0.0, 0.0, 0.0),
            child_anchor: Vec3::new(0.0, 0.0, 0.0),
            kind: CadJointKind::Revolute {
                axis: Vec3::new(0.0, 0.0, 1.0),
                limits: Some((-90.0, 90.0)),
            },
            state: 0.0,
        }],
        ground_instance_id: Some("base-1".to_string()),
    }
}

fn snapshot(
    case_id: &str,
    schema: &CadAssemblySchema,
    ui: &CadAssemblyUiState,
    was_clamped: bool,
    error: Option<String>,
) -> AssemblyUiSelectionEditCaseSnapshot {
    let mut schema_instance_ids: Vec<String> = schema
        .instances
        .iter()
        .map(|instance| instance.id.clone())
        .collect();
    schema_instance_ids.sort();

    let mut schema_joint_ids: Vec<String> =
        schema.joints.iter().map(|joint| joint.id.clone()).collect();
    schema_joint_ids.sort();

    let arm_instance_name = schema
        .instances
        .iter()
        .find(|instance| instance.id == "arm-1")
        .and_then(|instance| instance.name.clone());

    let joint_state_deg = schema
        .joints
        .iter()
        .find(|joint| joint.id == "joint.hinge")
        .map(|joint| f(joint.state));

    AssemblyUiSelectionEditCaseSnapshot {
        case_id: case_id.to_string(),
        selected_instance_id: ui.selected_instance_id.clone(),
        selected_joint_id: ui.selected_joint_id.clone(),
        schema_instance_ids,
        schema_joint_ids,
        arm_instance_name,
        joint_state_deg,
        was_clamped,
        error,
    }
}

fn f(value: f64) -> String {
    format!("{value:.6}")
}

fn parity_signature(
    snapshot: &AssemblyUiSelectionEditSnapshot,
    reference_commit_match: bool,
    behavior_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            snapshot,
            reference_commit_match,
            behavior_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize assembly UI selection/edit parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        AssemblyUiSelectionEditCaseSnapshot, AssemblyUiSelectionEditSnapshot, parity_signature,
    };

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let snapshot = AssemblyUiSelectionEditSnapshot {
            case_snapshots: vec![AssemblyUiSelectionEditCaseSnapshot {
                case_id: "case".to_string(),
                selected_instance_id: None,
                selected_joint_id: None,
                schema_instance_ids: Vec::new(),
                schema_joint_ids: Vec::new(),
                arm_instance_name: None,
                joint_state_deg: None,
                was_clamped: false,
                error: None,
            }],
        };

        let first = parity_signature(&snapshot, true, true, true, "abc");
        let second = parity_signature(&snapshot, true, true, true, "abc");
        assert_eq!(first, second);
    }
}
