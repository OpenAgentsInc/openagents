use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::assembly::{
    CadAssemblyJoint, CadAssemblySchema, CadAssemblyUiState, CadJointKind, CadPartDef,
    CadPartInstance, CadTransform3D,
};
use crate::kernel_math::Vec3;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_ASSEMBLY_SERIALIZATION_REPLAY_ISSUE_ID: &str = "VCAD-PARITY-064";
pub const ASSEMBLY_SERIALIZATION_REPLAY_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/assembly_serialization_replay_vcad_reference.json";
const ASSEMBLY_SERIALIZATION_REPLAY_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/assembly_serialization_replay_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssemblySerializationReplayParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub case_snapshots: Vec<AssemblySerializationReplayCaseSnapshot>,
    pub behavior_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AssemblySerializationReplayReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_case_snapshots: Vec<AssemblySerializationReplayCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AssemblySerializationReplaySnapshot {
    case_snapshots: Vec<AssemblySerializationReplayCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssemblySerializationReplayCaseSnapshot {
    pub case_id: String,
    pub schema_json_sha256: String,
    pub ui_json_sha256: String,
    pub selected_instance_id: Option<String>,
    pub selected_joint_id: Option<String>,
    pub ground_instance_id: Option<String>,
    pub instance_ids: Vec<String>,
    pub joint_ids: Vec<String>,
    pub arm_instance_name: Option<String>,
    pub hinge_state_deg: Option<String>,
    pub replay_step_count: usize,
    pub was_clamped: bool,
    pub error: Option<String>,
}

pub fn build_assembly_serialization_replay_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<AssemblySerializationReplayParityManifest> {
    let corpus: AssemblySerializationReplayReferenceCorpus = serde_json::from_str(
        ASSEMBLY_SERIALIZATION_REPLAY_REFERENCE_CORPUS_JSON,
    )
    .map_err(|error| CadError::ParseFailed {
        reason: format!("failed to parse assembly serialization/replay reference corpus: {error}"),
    })?;

    let reference_corpus_sha256 =
        sha256_hex(ASSEMBLY_SERIALIZATION_REPLAY_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = run_serialization_replay_cases();
    let replay_snapshot = run_serialization_replay_cases();
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

    Ok(AssemblySerializationReplayParityManifest {
        manifest_version: 1,
        issue_id: PARITY_ASSEMBLY_SERIALIZATION_REPLAY_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: ASSEMBLY_SERIALIZATION_REPLAY_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        case_snapshots: snapshot.case_snapshots,
        behavior_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "assembly schema serializes/deserializes with deterministic camelCase payload keys"
                .to_string(),
            "assembly UI selection state serializes/deserializes without mutating selected ids"
                .to_string(),
            "replaying the same serialized assembly mutation sequence yields byte-stable schema/ui snapshots"
                .to_string(),
            "replay error contracts are deterministic for missing selected/unknown joint ids"
                .to_string(),
        ],
    })
}

fn run_serialization_replay_cases() -> AssemblySerializationReplaySnapshot {
    let mut case_snapshots = vec![
        case_schema_roundtrip_canonical_fields(),
        case_ui_state_roundtrip_selected_entities(),
        case_serialized_replay_edit_delete_sequence(),
        case_replay_set_joint_without_selection_error(),
        case_replay_select_unknown_joint_error(),
    ];
    case_snapshots.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    AssemblySerializationReplaySnapshot { case_snapshots }
}

fn case_schema_roundtrip_canonical_fields() -> AssemblySerializationReplayCaseSnapshot {
    let schema = base_schema();
    let json = schema.to_json().expect("serialize assembly schema");
    let roundtrip = CadAssemblySchema::from_json(&json).expect("parse assembly schema");
    let ui = CadAssemblyUiState::default();

    snapshot(
        "schema_roundtrip_canonical_fields",
        &roundtrip,
        &ui,
        0,
        false,
        None,
    )
}

fn case_ui_state_roundtrip_selected_entities() -> AssemblySerializationReplayCaseSnapshot {
    let schema = base_schema();
    let mut ui = CadAssemblyUiState::default();

    ui.select_instance(&schema, "arm-1")
        .expect("select known instance");
    ui.select_joint(&schema, "joint.hinge")
        .expect("select known joint");

    let ui_json = ui.to_json().expect("serialize assembly ui state");
    let roundtrip_ui = CadAssemblyUiState::from_json(&ui_json).expect("parse assembly ui state");

    snapshot(
        "ui_state_roundtrip_selected_entities",
        &schema,
        &roundtrip_ui,
        2,
        false,
        None,
    )
}

fn case_serialized_replay_edit_delete_sequence() -> AssemblySerializationReplayCaseSnapshot {
    let schema_json = base_schema().to_json().expect("serialize initial schema");
    let ui_json = CadAssemblyUiState::default()
        .to_json()
        .expect("serialize initial ui state");

    let mut schema = CadAssemblySchema::from_json(&schema_json).expect("parse replay schema");
    let mut ui = CadAssemblyUiState::from_json(&ui_json).expect("parse replay ui state");

    ui.select_instance(&schema, "arm-1")
        .expect("select known instance");
    ui.rename_selected_instance(&mut schema, "Arm Segment".to_string())
        .expect("rename selected instance");
    ui.select_joint(&schema, "joint.hinge")
        .expect("select known joint");
    let semantics = ui
        .set_selected_joint_state(&mut schema, 120.0)
        .expect("set selected joint state with clamp");
    schema
        .delete_instance("arm-1")
        .expect("delete selected instance");
    ui.sync_with_schema(&schema);

    snapshot(
        "serialized_replay_edit_delete_sequence",
        &schema,
        &ui,
        6,
        semantics.was_clamped,
        None,
    )
}

fn case_replay_set_joint_without_selection_error() -> AssemblySerializationReplayCaseSnapshot {
    let schema_json = base_schema().to_json().expect("serialize initial schema");
    let ui_json = CadAssemblyUiState::default()
        .to_json()
        .expect("serialize initial ui state");

    let mut schema = CadAssemblySchema::from_json(&schema_json).expect("parse replay schema");
    let mut ui = CadAssemblyUiState::from_json(&ui_json).expect("parse replay ui state");

    let error = ui
        .set_selected_joint_state(&mut schema, 45.0)
        .expect_err("setting joint state without selected joint should fail")
        .to_string();

    snapshot(
        "replay_set_joint_without_selection_error",
        &schema,
        &ui,
        1,
        false,
        Some(error),
    )
}

fn case_replay_select_unknown_joint_error() -> AssemblySerializationReplayCaseSnapshot {
    let schema_json = base_schema().to_json().expect("serialize initial schema");
    let ui_json = CadAssemblyUiState::default()
        .to_json()
        .expect("serialize initial ui state");

    let schema = CadAssemblySchema::from_json(&schema_json).expect("parse replay schema");
    let mut ui = CadAssemblyUiState::from_json(&ui_json).expect("parse replay ui state");

    let error = ui
        .select_joint(&schema, "joint.missing")
        .expect_err("selecting unknown joint should fail")
        .to_string();

    snapshot(
        "replay_select_unknown_joint_error",
        &schema,
        &ui,
        1,
        false,
        Some(error),
    )
}

fn base_schema() -> CadAssemblySchema {
    CadAssemblySchema {
        part_defs: BTreeMap::from([
            (
                "base".to_string(),
                CadPartDef {
                    id: "base".to_string(),
                    name: Some("Base".to_string()),
                    root: 1,
                    default_material: Some("aluminum".to_string()),
                },
            ),
            (
                "arm".to_string(),
                CadPartDef {
                    id: "arm".to_string(),
                    name: Some("Arm".to_string()),
                    root: 2,
                    default_material: None,
                },
            ),
        ]),
        instances: vec![
            CadPartInstance {
                id: "base-1".to_string(),
                part_def_id: "base".to_string(),
                name: Some("Base".to_string()),
                transform: Some(CadTransform3D::identity()),
                material: None,
            },
            CadPartInstance {
                id: "arm-1".to_string(),
                part_def_id: "arm".to_string(),
                name: Some("Arm".to_string()),
                transform: Some(CadTransform3D {
                    translation: Vec3::new(10.0, 0.0, 0.0),
                    rotation: Vec3::new(0.0, 0.0, 0.0),
                    scale: Vec3::new(1.0, 1.0, 1.0),
                }),
                material: Some("steel".to_string()),
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
    replay_step_count: usize,
    was_clamped: bool,
    error: Option<String>,
) -> AssemblySerializationReplayCaseSnapshot {
    let schema_json = schema.to_json().expect("serialize schema snapshot");
    let ui_json = ui.to_json().expect("serialize ui snapshot");

    let mut instance_ids: Vec<String> = schema
        .instances
        .iter()
        .map(|instance| instance.id.clone())
        .collect();
    instance_ids.sort();

    let mut joint_ids: Vec<String> = schema.joints.iter().map(|joint| joint.id.clone()).collect();
    joint_ids.sort();

    let arm_instance_name = schema
        .instances
        .iter()
        .find(|instance| instance.id == "arm-1")
        .and_then(|instance| instance.name.clone());

    let hinge_state_deg = schema
        .joints
        .iter()
        .find(|joint| joint.id == "joint.hinge")
        .map(|joint| f(joint.state));

    AssemblySerializationReplayCaseSnapshot {
        case_id: case_id.to_string(),
        schema_json_sha256: sha256_hex(schema_json.as_bytes()),
        ui_json_sha256: sha256_hex(ui_json.as_bytes()),
        selected_instance_id: ui.selected_instance_id.clone(),
        selected_joint_id: ui.selected_joint_id.clone(),
        ground_instance_id: schema.ground_instance_id.clone(),
        instance_ids,
        joint_ids,
        arm_instance_name,
        hinge_state_deg,
        replay_step_count,
        was_clamped,
        error,
    }
}

fn f(value: f64) -> String {
    format!("{value:.6}")
}

fn parity_signature(
    snapshot: &AssemblySerializationReplaySnapshot,
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
        .expect("serialize assembly serialization/replay parity payload"),
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
        AssemblySerializationReplayCaseSnapshot, AssemblySerializationReplaySnapshot,
        parity_signature,
    };

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let snapshot = AssemblySerializationReplaySnapshot {
            case_snapshots: vec![AssemblySerializationReplayCaseSnapshot {
                case_id: "case".to_string(),
                schema_json_sha256: "a".to_string(),
                ui_json_sha256: "b".to_string(),
                selected_instance_id: None,
                selected_joint_id: None,
                ground_instance_id: None,
                instance_ids: Vec::new(),
                joint_ids: Vec::new(),
                arm_instance_name: None,
                hinge_state_deg: None,
                replay_step_count: 0,
                was_clamped: false,
                error: None,
            }],
        };

        let first = parity_signature(&snapshot, true, true, true, "abc");
        let second = parity_signature(&snapshot, true, true, true, "abc");
        assert_eq!(first, second);
    }
}
