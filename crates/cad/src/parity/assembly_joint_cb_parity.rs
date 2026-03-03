use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::assembly::{CadAssemblyJoint, CadJointKind, CadJointMotion};
use crate::kernel_math::Vec3;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_ASSEMBLY_JOINT_CB_ISSUE_ID: &str = "VCAD-PARITY-059";
pub const ASSEMBLY_JOINT_CB_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/assembly_joint_cb_vcad_reference.json";
const ASSEMBLY_JOINT_CB_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/assembly_joint_cb_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssemblyJointCbParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub supported_joint_types: Vec<String>,
    pub case_snapshots: Vec<JointCbCaseSnapshot>,
    pub ball_axis_is_z: bool,
    pub behavior_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AssemblyJointCbReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    supported_joint_types: Vec<String>,
    ball_rotation_axis: String,
    expected_case_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct JointCbSnapshot {
    supported_joint_types: Vec<String>,
    case_snapshots: Vec<JointCbCaseSnapshot>,
    ball_axis_is_z: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JointCbCaseSnapshot {
    pub case_id: String,
    pub joint_type: String,
    pub translation_mm: [String; 3],
    pub axis: [String; 3],
    pub angle_deg: String,
}

pub fn build_assembly_joint_cb_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<AssemblyJointCbParityManifest> {
    let corpus: AssemblyJointCbReferenceCorpus =
        serde_json::from_str(ASSEMBLY_JOINT_CB_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse assembly joint CB reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(ASSEMBLY_JOINT_CB_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = run_joint_cb_cases();
    let replay_snapshot = run_joint_cb_cases();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let expected_joint_types = sorted(corpus.supported_joint_types);
    let expected_case_ids = sorted(corpus.expected_case_ids);
    let case_ids = sorted(
        snapshot
            .case_snapshots
            .iter()
            .map(|case| case.case_id.clone())
            .collect(),
    );

    let behavior_match = snapshot.supported_joint_types == expected_joint_types
        && case_ids == expected_case_ids
        && snapshot.ball_axis_is_z;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        behavior_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(AssemblyJointCbParityManifest {
        manifest_version: 1,
        issue_id: PARITY_ASSEMBLY_JOINT_CB_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: ASSEMBLY_JOINT_CB_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        supported_joint_types: snapshot.supported_joint_types,
        case_snapshots: snapshot.case_snapshots,
        ball_axis_is_z: snapshot.ball_axis_is_z,
        behavior_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "cylindrical joints follow vcad eval behavior: axis-normalized rotation with rotated child-anchor translation".to_string(),
            "cylindrical joints fallback to +Z axis for zero-length axis".to_string(),
            "ball joints use +Z axis rotation semantics in vcad eval path".to_string(),
            "cylindrical/ball parity fixtures replay deterministically".to_string(),
        ],
    })
}

fn run_joint_cb_cases() -> JointCbSnapshot {
    let mut case_snapshots = Vec::new();

    let cylindrical = CadAssemblyJoint {
        id: "joint.cylindrical.001".to_string(),
        name: None,
        parent_instance_id: Some("base-1".to_string()),
        child_instance_id: "arm-1".to_string(),
        parent_anchor: Vec3::new(10.0, 0.0, 0.0),
        child_anchor: Vec3::new(2.0, 0.0, 0.0),
        kind: CadJointKind::Cylindrical {
            axis: Vec3::new(0.0, 2.0, 0.0),
        },
        state: 90.0,
    };
    case_snapshots.push(build_case_snapshot(
        "cylindrical.axis_normalized",
        cylindrical.solve_motion(),
    ));

    let cylindrical_zero = CadAssemblyJoint {
        id: "joint.cylindrical.002".to_string(),
        name: None,
        parent_instance_id: Some("base-1".to_string()),
        child_instance_id: "arm-1".to_string(),
        parent_anchor: Vec3::new(0.0, 0.0, 0.0),
        child_anchor: Vec3::new(1.0, 0.0, 0.0),
        kind: CadJointKind::Cylindrical {
            axis: Vec3::new(0.0, 0.0, 0.0),
        },
        state: 180.0,
    };
    case_snapshots.push(build_case_snapshot(
        "cylindrical.zero_axis_fallback",
        cylindrical_zero.solve_motion(),
    ));

    let ball = CadAssemblyJoint {
        id: "joint.ball.001".to_string(),
        name: None,
        parent_instance_id: Some("base-1".to_string()),
        child_instance_id: "arm-1".to_string(),
        parent_anchor: Vec3::new(3.0, 4.0, 0.0),
        child_anchor: Vec3::new(1.0, 0.0, 0.0),
        kind: CadJointKind::Ball,
        state: 90.0,
    };
    case_snapshots.push(build_case_snapshot(
        "ball.z_axis_rotation",
        ball.solve_motion(),
    ));

    case_snapshots.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    let supported_joint_types = sorted(
        case_snapshots
            .iter()
            .map(|case| case.joint_type.clone())
            .collect(),
    );
    let ball_axis_is_z = case_snapshots
        .iter()
        .find(|case| case.case_id == "ball.z_axis_rotation")
        .is_some_and(|case| case.axis == [f(0.0), f(0.0), f(1.0)]);

    JointCbSnapshot {
        supported_joint_types,
        case_snapshots,
        ball_axis_is_z,
    }
}

fn build_case_snapshot(case_id: &str, motion: CadJointMotion) -> JointCbCaseSnapshot {
    match motion {
        CadJointMotion::Cylindrical {
            translation_mm,
            axis,
            angle_deg,
        } => JointCbCaseSnapshot {
            case_id: case_id.to_string(),
            joint_type: "Cylindrical".to_string(),
            translation_mm: vec3_to_str_array(translation_mm),
            axis: vec3_to_str_array(axis),
            angle_deg: f(angle_deg),
        },
        CadJointMotion::Ball {
            translation_mm,
            axis,
            angle_deg,
        } => JointCbCaseSnapshot {
            case_id: case_id.to_string(),
            joint_type: "Ball".to_string(),
            translation_mm: vec3_to_str_array(translation_mm),
            axis: vec3_to_str_array(axis),
            angle_deg: f(angle_deg),
        },
        other => panic!("unexpected motion variant for CB parity lane: {other:?}"),
    }
}

fn vec3_to_str_array(value: Vec3) -> [String; 3] {
    [f(value.x), f(value.y), f(value.z)]
}

fn f(value: f64) -> String {
    format!("{value:.6}")
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn parity_signature(
    snapshot: &JointCbSnapshot,
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
        .expect("serialize assembly joint CB parity payload"),
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
    use super::JointCbCaseSnapshot;
    use super::JointCbSnapshot;
    use super::parity_signature;

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let snapshot = JointCbSnapshot {
            supported_joint_types: vec!["Ball".to_string()],
            case_snapshots: vec![JointCbCaseSnapshot {
                case_id: "ball".to_string(),
                joint_type: "Ball".to_string(),
                translation_mm: [
                    "0.000000".to_string(),
                    "0.000000".to_string(),
                    "0.000000".to_string(),
                ],
                axis: [
                    "0.000000".to_string(),
                    "0.000000".to_string(),
                    "1.000000".to_string(),
                ],
                angle_deg: "0.000000".to_string(),
            }],
            ball_axis_is_z: true,
        };

        let first = parity_signature(&snapshot, true, true, true, "abc");
        let second = parity_signature(&snapshot, true, true, true, "abc");
        assert_eq!(first, second);
    }
}
