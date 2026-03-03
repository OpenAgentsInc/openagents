use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::assembly::{
    CadAssemblyJoint, CadAssemblySchema, CadJointKind, CadPartInstance, CadTransform3D,
};
use crate::kernel_math::Vec3;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_ASSEMBLY_FK_ISSUE_ID: &str = "VCAD-PARITY-061";
pub const ASSEMBLY_FK_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/assembly_fk_vcad_reference.json";
const ASSEMBLY_FK_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/assembly_fk_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssemblyFkParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub case_snapshots: Vec<AssemblyFkCaseSnapshot>,
    pub behavior_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AssemblyFkReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_case_snapshots: Vec<AssemblyFkCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AssemblyFkSnapshot {
    case_snapshots: Vec<AssemblyFkCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssemblyFkCaseSnapshot {
    pub case_id: String,
    pub world_transforms: Vec<AssemblyFkWorldTransformSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssemblyFkWorldTransformSnapshot {
    pub instance_id: String,
    pub translation_mm: [String; 3],
    pub rotation_deg: [String; 3],
    pub scale: [String; 3],
}

pub fn build_assembly_fk_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<AssemblyFkParityManifest> {
    let corpus: AssemblyFkReferenceCorpus = serde_json::from_str(ASSEMBLY_FK_REFERENCE_CORPUS_JSON)
        .map_err(|error| CadError::ParseFailed {
            reason: format!("failed to parse assembly FK reference corpus: {error}"),
        })?;

    let reference_corpus_sha256 = sha256_hex(ASSEMBLY_FK_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = run_fk_cases();
    let replay_snapshot = run_fk_cases();
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

    Ok(AssemblyFkParityManifest {
        manifest_version: 1,
        issue_id: PARITY_ASSEMBLY_FK_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: ASSEMBLY_FK_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        case_snapshots: snapshot.case_snapshots,
        behavior_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "forward kinematics solves by BFS through parent->child joint graph".to_string(),
            "world-grounded joints (parentInstanceId=null) use identity parent transform"
                .to_string(),
            "joint transform composition matches vcad order: parent * joint * local".to_string(),
            "visited-guard stabilizes cyclic chains for deterministic replay".to_string(),
        ],
    })
}

fn run_fk_cases() -> AssemblyFkSnapshot {
    let mut case_snapshots = vec![
        snapshot_case("joint_chain_compose", fk_joint_chain_compose_schema()),
        snapshot_case(
            "world_grounded_parent_none",
            fk_world_grounded_parent_none_schema(),
        ),
        snapshot_case("cyclic_chain_visited_guard", fk_cycle_guard_schema()),
    ];
    case_snapshots.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    AssemblyFkSnapshot { case_snapshots }
}

fn snapshot_case(case_id: &str, schema: CadAssemblySchema) -> AssemblyFkCaseSnapshot {
    let world = schema.solve_forward_kinematics();
    let world_transforms = world
        .iter()
        .map(
            |(instance_id, transform)| AssemblyFkWorldTransformSnapshot {
                instance_id: instance_id.clone(),
                translation_mm: vec3_to_str_array(transform.translation),
                rotation_deg: vec3_to_str_array(transform.rotation),
                scale: vec3_to_str_array(transform.scale),
            },
        )
        .collect();

    AssemblyFkCaseSnapshot {
        case_id: case_id.to_string(),
        world_transforms,
    }
}

fn fk_joint_chain_compose_schema() -> CadAssemblySchema {
    CadAssemblySchema {
        part_defs: BTreeMap::new(),
        instances: vec![
            CadPartInstance {
                id: "base-1".to_string(),
                part_def_id: "base".to_string(),
                name: None,
                transform: Some(transform(Vec3::new(100.0, 0.0, 0.0))),
                material: None,
            },
            CadPartInstance {
                id: "arm-1".to_string(),
                part_def_id: "arm".to_string(),
                name: None,
                transform: Some(transform(Vec3::new(10.0, 0.0, 0.0))),
                material: None,
            },
            CadPartInstance {
                id: "slider-1".to_string(),
                part_def_id: "slider".to_string(),
                name: None,
                transform: Some(transform(Vec3::new(5.0, 0.0, 0.0))),
                material: None,
            },
        ],
        joints: vec![
            CadAssemblyJoint {
                id: "joint.revolute.001".to_string(),
                name: None,
                parent_instance_id: Some("base-1".to_string()),
                child_instance_id: "arm-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(1.0, 0.0, 0.0),
                kind: CadJointKind::Revolute {
                    axis: Vec3::new(0.0, 0.0, 1.0),
                    limits: None,
                },
                state: 90.0,
            },
            CadAssemblyJoint {
                id: "joint.slider.001".to_string(),
                name: None,
                parent_instance_id: Some("arm-1".to_string()),
                child_instance_id: "slider-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Slider {
                    axis: Vec3::new(1.0, 0.0, 0.0),
                    limits: None,
                },
                state: 20.0,
            },
        ],
        ground_instance_id: Some("base-1".to_string()),
    }
}

fn fk_world_grounded_parent_none_schema() -> CadAssemblySchema {
    CadAssemblySchema {
        part_defs: BTreeMap::new(),
        instances: vec![CadPartInstance {
            id: "free-1".to_string(),
            part_def_id: "arm".to_string(),
            name: None,
            transform: Some(transform(Vec3::new(2.0, 3.0, 4.0))),
            material: None,
        }],
        joints: vec![CadAssemblyJoint {
            id: "joint.fixed.world".to_string(),
            name: None,
            parent_instance_id: None,
            child_instance_id: "free-1".to_string(),
            parent_anchor: Vec3::new(10.0, 0.0, 0.0),
            child_anchor: Vec3::new(1.0, 0.0, 0.0),
            kind: CadJointKind::Fixed,
            state: 0.0,
        }],
        ground_instance_id: None,
    }
}

fn fk_cycle_guard_schema() -> CadAssemblySchema {
    CadAssemblySchema {
        part_defs: BTreeMap::new(),
        instances: vec![
            CadPartInstance {
                id: "a-1".to_string(),
                part_def_id: "a".to_string(),
                name: None,
                transform: None,
                material: None,
            },
            CadPartInstance {
                id: "b-1".to_string(),
                part_def_id: "b".to_string(),
                name: None,
                transform: None,
                material: None,
            },
        ],
        joints: vec![
            CadAssemblyJoint {
                id: "joint.root".to_string(),
                name: None,
                parent_instance_id: None,
                child_instance_id: "a-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Fixed,
                state: 0.0,
            },
            CadAssemblyJoint {
                id: "joint.a_to_b".to_string(),
                name: None,
                parent_instance_id: Some("a-1".to_string()),
                child_instance_id: "b-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Slider {
                    axis: Vec3::new(1.0, 0.0, 0.0),
                    limits: None,
                },
                state: 10.0,
            },
            CadAssemblyJoint {
                id: "joint.b_to_a".to_string(),
                name: None,
                parent_instance_id: Some("b-1".to_string()),
                child_instance_id: "a-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Slider {
                    axis: Vec3::new(0.0, 1.0, 0.0),
                    limits: None,
                },
                state: 5.0,
            },
        ],
        ground_instance_id: None,
    }
}

fn transform(translation: Vec3) -> CadTransform3D {
    CadTransform3D {
        translation,
        rotation: Vec3::new(0.0, 0.0, 0.0),
        scale: Vec3::new(1.0, 1.0, 1.0),
    }
}

fn vec3_to_str_array(value: Vec3) -> [String; 3] {
    [f(value.x), f(value.y), f(value.z)]
}

fn f(value: f64) -> String {
    format!("{value:.6}")
}

fn parity_signature(
    snapshot: &AssemblyFkSnapshot,
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
        .expect("serialize assembly FK parity payload"),
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
    use super::{AssemblyFkCaseSnapshot, AssemblyFkSnapshot, parity_signature};

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let snapshot = AssemblyFkSnapshot {
            case_snapshots: vec![AssemblyFkCaseSnapshot {
                case_id: "case".to_string(),
                world_transforms: Vec::new(),
            }],
        };

        let first = parity_signature(&snapshot, true, true, true, "abc");
        let second = parity_signature(&snapshot, true, true, true, "abc");
        assert_eq!(first, second);
    }
}
