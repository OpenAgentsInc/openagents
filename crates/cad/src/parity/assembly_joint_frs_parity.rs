use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::assembly::{CadAssemblyJoint, CadJointKind, CadJointMotion};
use crate::kernel_math::Vec3;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_ASSEMBLY_JOINT_FRS_ISSUE_ID: &str = "VCAD-PARITY-058";
pub const ASSEMBLY_JOINT_FRS_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/assembly_joint_frs_vcad_reference.json";
const ASSEMBLY_JOINT_FRS_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/assembly_joint_frs_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssemblyJointFrsParityManifest {
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
    pub case_snapshots: Vec<JointFrsCaseSnapshot>,
    pub axis_zero_fallback_applied: bool,
    pub unsupported_joint_scope_guard: bool,
    pub behavior_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AssemblyJointFrsReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    supported_joint_types: Vec<String>,
    axis_zero_fallback: String,
    expected_case_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct JointFrsSnapshot {
    supported_joint_types: Vec<String>,
    case_snapshots: Vec<JointFrsCaseSnapshot>,
    axis_zero_fallback_applied: bool,
    unsupported_joint_scope_guard: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JointFrsCaseSnapshot {
    pub case_id: String,
    pub joint_type: String,
    pub translation_mm: [String; 3],
    pub axis: Option<[String; 3]>,
    pub angle_deg: Option<String>,
    pub offset_mm: Option<String>,
}

pub fn build_assembly_joint_frs_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<AssemblyJointFrsParityManifest> {
    let corpus: AssemblyJointFrsReferenceCorpus =
        serde_json::from_str(ASSEMBLY_JOINT_FRS_REFERENCE_CORPUS_JSON).map_err(|error| {
            CadError::ParseFailed {
                reason: format!("failed to parse assembly joint FRS reference corpus: {error}"),
            }
        })?;

    let reference_corpus_sha256 = sha256_hex(ASSEMBLY_JOINT_FRS_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = run_joint_frs_cases()?;
    let replay_snapshot = run_joint_frs_cases()?;
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
        && snapshot.axis_zero_fallback_applied
        && snapshot.unsupported_joint_scope_guard;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        behavior_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(AssemblyJointFrsParityManifest {
        manifest_version: 1,
        issue_id: PARITY_ASSEMBLY_JOINT_FRS_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: ASSEMBLY_JOINT_FRS_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        supported_joint_types: snapshot.supported_joint_types,
        case_snapshots: snapshot.case_snapshots,
        axis_zero_fallback_applied: snapshot.axis_zero_fallback_applied,
        unsupported_joint_scope_guard: snapshot.unsupported_joint_scope_guard,
        behavior_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "fixed joints preserve anchor-delta translation semantics".to_string(),
            "revolute joints normalize axes and fallback to +Z for zero-length axis".to_string(),
            "slider joints apply linear offset along normalized axis".to_string(),
            "cylindrical/ball joints are explicitly out-of-scope for the fixed/revolute/slider parity lane".to_string(),
        ],
    })
}

fn run_joint_frs_cases() -> CadResult<JointFrsSnapshot> {
    let mut supported_joint_types = Vec::new();
    let mut case_snapshots = Vec::new();

    let fixed = CadAssemblyJoint {
        id: "joint.fixed.001".to_string(),
        name: None,
        parent_instance_id: Some("base-1".to_string()),
        child_instance_id: "arm-1".to_string(),
        parent_anchor: Vec3::new(10.0, 5.0, 5.0),
        child_anchor: Vec3::new(0.0, 5.0, 5.0),
        kind: CadJointKind::Fixed,
        state: 0.0,
    };
    case_snapshots.push(build_case_snapshot(
        "fixed.anchor_delta",
        fixed.solve_fixed_revolute_slider_motion()?,
    ));

    let revolute = CadAssemblyJoint {
        id: "joint.revolute.001".to_string(),
        name: None,
        parent_instance_id: Some("base-1".to_string()),
        child_instance_id: "arm-1".to_string(),
        parent_anchor: Vec3::new(10.0, 5.0, 5.0),
        child_anchor: Vec3::new(0.0, 5.0, 5.0),
        kind: CadJointKind::Revolute {
            axis: Vec3::new(0.0, 2.0, 0.0),
            limits: None,
        },
        state: 45.0,
    };
    case_snapshots.push(build_case_snapshot(
        "revolute.axis_normalized",
        revolute.solve_fixed_revolute_slider_motion()?,
    ));

    let revolute_zero_axis = CadAssemblyJoint {
        id: "joint.revolute.002".to_string(),
        name: None,
        parent_instance_id: Some("base-1".to_string()),
        child_instance_id: "arm-1".to_string(),
        parent_anchor: Vec3::new(10.0, 5.0, 5.0),
        child_anchor: Vec3::new(0.0, 5.0, 5.0),
        kind: CadJointKind::Revolute {
            axis: Vec3::new(0.0, 0.0, 0.0),
            limits: None,
        },
        state: 90.0,
    };
    case_snapshots.push(build_case_snapshot(
        "revolute.zero_axis_fallback",
        revolute_zero_axis.solve_fixed_revolute_slider_motion()?,
    ));

    let slider = CadAssemblyJoint {
        id: "joint.slider.001".to_string(),
        name: None,
        parent_instance_id: Some("base-1".to_string()),
        child_instance_id: "arm-1".to_string(),
        parent_anchor: Vec3::new(5.0, 0.0, 0.0),
        child_anchor: Vec3::new(0.0, 0.0, 0.0),
        kind: CadJointKind::Slider {
            axis: Vec3::new(1.0, 0.0, 0.0),
            limits: None,
        },
        state: 12.5,
    };
    case_snapshots.push(build_case_snapshot(
        "slider.translation_offset",
        slider.solve_fixed_revolute_slider_motion()?,
    ));

    for snapshot in &case_snapshots {
        supported_joint_types.push(snapshot.joint_type.clone());
    }
    supported_joint_types = sorted(supported_joint_types);

    let zero_axis_case = case_snapshots
        .iter()
        .find(|case| case.case_id == "revolute.zero_axis_fallback");
    let axis_zero_fallback_applied = zero_axis_case
        .and_then(|case| case.axis.as_ref())
        .is_some_and(|axis| axis == &[f(0.0), f(0.0), f(1.0)]);

    let unsupported_joint_scope_guard = {
        let cylindrical = CadAssemblyJoint {
            id: "joint.cylindrical.scope".to_string(),
            name: None,
            parent_instance_id: Some("base-1".to_string()),
            child_instance_id: "arm-1".to_string(),
            parent_anchor: Vec3::new(0.0, 0.0, 0.0),
            child_anchor: Vec3::new(0.0, 0.0, 0.0),
            kind: CadJointKind::Cylindrical {
                axis: Vec3::new(0.0, 0.0, 1.0),
            },
            state: 0.0,
        };
        cylindrical
            .solve_fixed_revolute_slider_motion()
            .err()
            .is_some_and(|error| {
                error
                    .to_string()
                    .contains("outside fixed/revolute/slider parity scope")
            })
    };

    case_snapshots.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    Ok(JointFrsSnapshot {
        supported_joint_types,
        case_snapshots,
        axis_zero_fallback_applied,
        unsupported_joint_scope_guard,
    })
}

fn build_case_snapshot(case_id: &str, motion: CadJointMotion) -> JointFrsCaseSnapshot {
    match motion {
        CadJointMotion::Fixed { translation_mm } => JointFrsCaseSnapshot {
            case_id: case_id.to_string(),
            joint_type: "Fixed".to_string(),
            translation_mm: vec3_to_str_array(translation_mm),
            axis: None,
            angle_deg: None,
            offset_mm: None,
        },
        CadJointMotion::Revolute {
            translation_mm,
            axis,
            angle_deg,
        } => JointFrsCaseSnapshot {
            case_id: case_id.to_string(),
            joint_type: "Revolute".to_string(),
            translation_mm: vec3_to_str_array(translation_mm),
            axis: Some(vec3_to_str_array(axis)),
            angle_deg: Some(f(angle_deg)),
            offset_mm: None,
        },
        CadJointMotion::Slider {
            translation_mm,
            axis,
            offset_mm,
        } => JointFrsCaseSnapshot {
            case_id: case_id.to_string(),
            joint_type: "Slider".to_string(),
            translation_mm: vec3_to_str_array(translation_mm),
            axis: Some(vec3_to_str_array(axis)),
            angle_deg: None,
            offset_mm: Some(f(offset_mm)),
        },
        CadJointMotion::Cylindrical { .. } | CadJointMotion::Ball { .. } => {
            panic!("unexpected motion variant for FRS parity lane")
        }
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
    snapshot: &JointFrsSnapshot,
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
        .expect("serialize assembly joint FRS parity payload"),
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
    use super::JointFrsCaseSnapshot;
    use super::JointFrsSnapshot;
    use super::parity_signature;

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let snapshot = JointFrsSnapshot {
            supported_joint_types: vec!["Fixed".to_string()],
            case_snapshots: vec![JointFrsCaseSnapshot {
                case_id: "fixed".to_string(),
                joint_type: "Fixed".to_string(),
                translation_mm: [
                    "0.000000".to_string(),
                    "0.000000".to_string(),
                    "0.000000".to_string(),
                ],
                axis: None,
                angle_deg: None,
                offset_mm: None,
            }],
            axis_zero_fallback_applied: true,
            unsupported_joint_scope_guard: true,
        };

        let first = parity_signature(&snapshot, true, true, true, "abc");
        let second = parity_signature(&snapshot, true, true, true, "abc");
        assert_eq!(first, second);
    }
}
