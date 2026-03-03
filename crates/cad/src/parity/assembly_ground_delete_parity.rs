use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::assembly::{CadAssemblyJoint, CadAssemblySchema, CadJointKind, CadPartInstance};
use crate::kernel_math::Vec3;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_ASSEMBLY_GROUND_DELETE_ISSUE_ID: &str = "VCAD-PARITY-062";
pub const ASSEMBLY_GROUND_DELETE_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/assembly_ground_delete_vcad_reference.json";
const ASSEMBLY_GROUND_DELETE_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/assembly_ground_delete_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssemblyGroundDeleteParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub case_snapshots: Vec<AssemblyGroundDeleteCaseSnapshot>,
    pub behavior_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AssemblyGroundDeleteReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_case_snapshots: Vec<AssemblyGroundDeleteCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AssemblyGroundDeleteSnapshot {
    case_snapshots: Vec<AssemblyGroundDeleteCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssemblyGroundDeleteCaseSnapshot {
    pub case_id: String,
    pub ground_instance_id: Option<String>,
    pub instance_ids: Vec<String>,
    pub joint_ids: Vec<String>,
    pub removed_joint_ids: Vec<String>,
    pub cleared_ground: bool,
    pub error: Option<String>,
}

pub fn build_assembly_ground_delete_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<AssemblyGroundDeleteParityManifest> {
    let corpus: AssemblyGroundDeleteReferenceCorpus =
        serde_json::from_str(ASSEMBLY_GROUND_DELETE_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse assembly ground/delete reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 =
        sha256_hex(ASSEMBLY_GROUND_DELETE_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = run_ground_delete_cases();
    let replay_snapshot = run_ground_delete_cases();
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

    Ok(AssemblyGroundDeleteParityManifest {
        manifest_version: 1,
        issue_id: PARITY_ASSEMBLY_GROUND_DELETE_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: ASSEMBLY_GROUND_DELETE_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        case_snapshots: snapshot.case_snapshots,
        behavior_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "set_ground_instance requires a known instance id".to_string(),
            "delete_instance removes joints where parent or child references the deleted instance"
                .to_string(),
            "deleting the grounded instance clears ground_instance_id".to_string(),
            "delete_joint and delete_instance return deterministic invalid-parameter errors"
                .to_string(),
        ],
    })
}

fn run_ground_delete_cases() -> AssemblyGroundDeleteSnapshot {
    let mut case_snapshots = vec![
        case_ground_set_success(),
        case_ground_set_unknown_error(),
        case_delete_instance_cleanup(),
        case_delete_instance_unknown_error(),
        case_delete_joint_unknown_error(),
    ];
    case_snapshots.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    AssemblyGroundDeleteSnapshot { case_snapshots }
}

fn case_ground_set_success() -> AssemblyGroundDeleteCaseSnapshot {
    let mut schema = CadAssemblySchema {
        part_defs: BTreeMap::new(),
        instances: vec![instance("base-1", "base"), instance("arm-1", "arm")],
        joints: Vec::new(),
        ground_instance_id: None,
    };
    schema
        .set_ground_instance("base-1")
        .expect("set known ground instance");

    snapshot("ground_set_success", &schema, Vec::new(), false, None)
}

fn case_ground_set_unknown_error() -> AssemblyGroundDeleteCaseSnapshot {
    let mut schema = CadAssemblySchema {
        part_defs: BTreeMap::new(),
        instances: vec![instance("base-1", "base")],
        joints: Vec::new(),
        ground_instance_id: None,
    };
    let error = schema
        .set_ground_instance("missing")
        .expect_err("missing ground instance should fail")
        .to_string();

    snapshot(
        "ground_set_unknown_error",
        &schema,
        Vec::new(),
        false,
        Some(error),
    )
}

fn case_delete_instance_cleanup() -> AssemblyGroundDeleteCaseSnapshot {
    let mut schema = CadAssemblySchema {
        part_defs: BTreeMap::new(),
        instances: vec![
            instance("base-1", "base"),
            instance("arm-1", "arm"),
            instance("tool-1", "tool"),
        ],
        joints: vec![
            CadAssemblyJoint {
                id: "joint.base_arm".to_string(),
                name: None,
                parent_instance_id: Some("base-1".to_string()),
                child_instance_id: "arm-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Fixed,
                state: 0.0,
            },
            CadAssemblyJoint {
                id: "joint.arm_tool".to_string(),
                name: None,
                parent_instance_id: Some("arm-1".to_string()),
                child_instance_id: "tool-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Slider {
                    axis: Vec3::new(1.0, 0.0, 0.0),
                    limits: None,
                },
                state: 0.0,
            },
        ],
        ground_instance_id: Some("arm-1".to_string()),
    };

    let summary = schema
        .delete_instance("arm-1")
        .expect("delete instance should succeed");
    snapshot(
        "delete_instance_cleanup",
        &schema,
        summary.removed_joint_ids,
        summary.cleared_ground,
        None,
    )
}

fn case_delete_instance_unknown_error() -> AssemblyGroundDeleteCaseSnapshot {
    let mut schema = CadAssemblySchema {
        part_defs: BTreeMap::new(),
        instances: vec![instance("base-1", "base")],
        joints: Vec::new(),
        ground_instance_id: None,
    };
    let error = schema
        .delete_instance("missing")
        .expect_err("missing instance should fail")
        .to_string();

    snapshot(
        "delete_instance_unknown_error",
        &schema,
        Vec::new(),
        false,
        Some(error),
    )
}

fn case_delete_joint_unknown_error() -> AssemblyGroundDeleteCaseSnapshot {
    let mut schema = CadAssemblySchema {
        part_defs: BTreeMap::new(),
        instances: vec![instance("base-1", "base")],
        joints: vec![CadAssemblyJoint {
            id: "joint.1".to_string(),
            name: None,
            parent_instance_id: None,
            child_instance_id: "base-1".to_string(),
            parent_anchor: Vec3::new(0.0, 0.0, 0.0),
            child_anchor: Vec3::new(0.0, 0.0, 0.0),
            kind: CadJointKind::Fixed,
            state: 0.0,
        }],
        ground_instance_id: None,
    };
    let error = schema
        .delete_joint("joint.missing")
        .expect_err("missing joint should fail")
        .to_string();

    snapshot(
        "delete_joint_unknown_error",
        &schema,
        Vec::new(),
        false,
        Some(error),
    )
}

fn instance(id: &str, part_def_id: &str) -> CadPartInstance {
    CadPartInstance {
        id: id.to_string(),
        part_def_id: part_def_id.to_string(),
        name: None,
        transform: None,
        material: None,
    }
}

fn snapshot(
    case_id: &str,
    schema: &CadAssemblySchema,
    mut removed_joint_ids: Vec<String>,
    cleared_ground: bool,
    error: Option<String>,
) -> AssemblyGroundDeleteCaseSnapshot {
    let mut instance_ids: Vec<String> = schema
        .instances
        .iter()
        .map(|instance| instance.id.clone())
        .collect();
    let mut joint_ids: Vec<String> = schema.joints.iter().map(|joint| joint.id.clone()).collect();

    instance_ids.sort();
    joint_ids.sort();
    removed_joint_ids.sort();
    removed_joint_ids.dedup();

    AssemblyGroundDeleteCaseSnapshot {
        case_id: case_id.to_string(),
        ground_instance_id: schema.ground_instance_id.clone(),
        instance_ids,
        joint_ids,
        removed_joint_ids,
        cleared_ground,
        error,
    }
}

fn parity_signature(
    snapshot: &AssemblyGroundDeleteSnapshot,
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
        .expect("serialize assembly ground/delete parity payload"),
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
    use super::{AssemblyGroundDeleteCaseSnapshot, AssemblyGroundDeleteSnapshot, parity_signature};

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let snapshot = AssemblyGroundDeleteSnapshot {
            case_snapshots: vec![AssemblyGroundDeleteCaseSnapshot {
                case_id: "case".to_string(),
                ground_instance_id: None,
                instance_ids: vec!["a".to_string()],
                joint_ids: Vec::new(),
                removed_joint_ids: Vec::new(),
                cleared_ground: false,
                error: None,
            }],
        };

        let first = parity_signature(&snapshot, true, true, true, "abc");
        let second = parity_signature(&snapshot, true, true, true, "abc");
        assert_eq!(first, second);
    }
}
