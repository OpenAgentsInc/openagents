use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::assembly::{
    CadAssemblyJoint, CadAssemblySchema, CadJointKind, CadPartDef, CadPartInstance, CadTransform3D,
};
use crate::kernel_math::Vec3;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_ASSEMBLY_ACCEPTANCE_SCENES_ISSUE_ID: &str = "VCAD-PARITY-065";
pub const ASSEMBLY_ACCEPTANCE_SCENES_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/assembly_acceptance_scenes_vcad_reference.json";
const ASSEMBLY_ACCEPTANCE_SCENES_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/assembly_acceptance_scenes_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssemblyAcceptanceScenesParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub case_snapshots: Vec<AssemblyAcceptanceSceneCaseSnapshot>,
    pub behavior_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AssemblyAcceptanceScenesReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_case_snapshots: Vec<AssemblyAcceptanceSceneCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AssemblyAcceptanceScenesSnapshot {
    case_snapshots: Vec<AssemblyAcceptanceSceneCaseSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssemblyAcceptanceSceneCaseSnapshot {
    pub case_id: String,
    pub accepted: bool,
    pub grounded_instance_id: Option<String>,
    pub invalid_ground_instance_id: Option<String>,
    pub contains_joint_cycle: bool,
    pub instance_ids: Vec<String>,
    pub joint_ids: Vec<String>,
    pub missing_part_def_instance_ids: Vec<String>,
    pub missing_joint_child_instance_ids: Vec<String>,
    pub missing_joint_parent_instance_ids: Vec<String>,
    pub fk_resolved_instance_ids: Vec<String>,
}

pub fn build_assembly_acceptance_scenes_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<AssemblyAcceptanceScenesParityManifest> {
    let corpus: AssemblyAcceptanceScenesReferenceCorpus = serde_json::from_str(
        ASSEMBLY_ACCEPTANCE_SCENES_REFERENCE_CORPUS_JSON,
    )
    .map_err(|error| CadError::ParseFailed {
        reason: format!("failed to parse assembly acceptance scenes reference corpus: {error}"),
    })?;

    let reference_corpus_sha256 =
        sha256_hex(ASSEMBLY_ACCEPTANCE_SCENES_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = run_acceptance_scene_cases();
    let replay_snapshot = run_acceptance_scene_cases();
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

    Ok(AssemblyAcceptanceScenesParityManifest {
        manifest_version: 1,
        issue_id: PARITY_ASSEMBLY_ACCEPTANCE_SCENES_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: ASSEMBLY_ACCEPTANCE_SCENES_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        case_snapshots: snapshot.case_snapshots,
        behavior_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "acceptance scene fixtures evaluate deterministic instance/joint integrity and FK resolution coverage"
                .to_string(),
            "missing part-def refs, missing joint refs, and invalid ground ids are surfaced as non-accepted scenes"
                .to_string(),
            "joint-cycle detection in acceptance fixtures marks cyclic scenes as non-accepted"
                .to_string(),
            "acceptance scene corpus replays deterministically across runs".to_string(),
        ],
    })
}

fn run_acceptance_scene_cases() -> AssemblyAcceptanceScenesSnapshot {
    let mut case_snapshots = vec![
        snapshot_case("robot_arm_acceptance_scene", robot_arm_acceptance_scene()),
        snapshot_case(
            "world_grounded_slider_acceptance_scene",
            world_grounded_slider_acceptance_scene(),
        ),
        snapshot_case(
            "invalid_reference_acceptance_scene",
            invalid_reference_scene(),
        ),
        snapshot_case("cyclic_joint_acceptance_scene", cyclic_joint_scene()),
    ];
    case_snapshots.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    AssemblyAcceptanceScenesSnapshot { case_snapshots }
}

fn snapshot_case(case_id: &str, schema: CadAssemblySchema) -> AssemblyAcceptanceSceneCaseSnapshot {
    let report = schema.acceptance_scene_report();

    AssemblyAcceptanceSceneCaseSnapshot {
        case_id: case_id.to_string(),
        accepted: report.accepted,
        grounded_instance_id: report.grounded_instance_id,
        invalid_ground_instance_id: report.invalid_ground_instance_id,
        contains_joint_cycle: report.contains_joint_cycle,
        instance_ids: report.instance_ids,
        joint_ids: report.joint_ids,
        missing_part_def_instance_ids: report.missing_part_def_instance_ids,
        missing_joint_child_instance_ids: report.missing_joint_child_instance_ids,
        missing_joint_parent_instance_ids: report.missing_joint_parent_instance_ids,
        fk_resolved_instance_ids: report.fk_resolved_instance_ids,
    }
}

fn robot_arm_acceptance_scene() -> CadAssemblySchema {
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
                    default_material: Some("steel".to_string()),
                },
            ),
            (
                "tool".to_string(),
                CadPartDef {
                    id: "tool".to_string(),
                    name: Some("Tool".to_string()),
                    root: 3,
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
                material: None,
            },
            CadPartInstance {
                id: "tool-1".to_string(),
                part_def_id: "tool".to_string(),
                name: Some("Tool".to_string()),
                transform: Some(CadTransform3D {
                    translation: Vec3::new(4.0, 0.0, 0.0),
                    rotation: Vec3::new(0.0, 0.0, 0.0),
                    scale: Vec3::new(1.0, 1.0, 1.0),
                }),
                material: Some("polymer".to_string()),
            },
        ],
        joints: vec![
            CadAssemblyJoint {
                id: "joint.base_arm".to_string(),
                name: Some("Base Arm".to_string()),
                parent_instance_id: Some("base-1".to_string()),
                child_instance_id: "arm-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Revolute {
                    axis: Vec3::new(0.0, 0.0, 1.0),
                    limits: Some((-90.0, 90.0)),
                },
                state: 45.0,
            },
            CadAssemblyJoint {
                id: "joint.arm_tool".to_string(),
                name: Some("Arm Tool".to_string()),
                parent_instance_id: Some("arm-1".to_string()),
                child_instance_id: "tool-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Slider {
                    axis: Vec3::new(1.0, 0.0, 0.0),
                    limits: Some((0.0, 100.0)),
                },
                state: 20.0,
            },
        ],
        ground_instance_id: Some("base-1".to_string()),
    }
}

fn world_grounded_slider_acceptance_scene() -> CadAssemblySchema {
    CadAssemblySchema {
        part_defs: BTreeMap::from([(
            "carriage".to_string(),
            CadPartDef {
                id: "carriage".to_string(),
                name: Some("Carriage".to_string()),
                root: 1,
                default_material: Some("steel".to_string()),
            },
        )]),
        instances: vec![CadPartInstance {
            id: "carriage-1".to_string(),
            part_def_id: "carriage".to_string(),
            name: Some("Carriage".to_string()),
            transform: Some(CadTransform3D::identity()),
            material: None,
        }],
        joints: vec![CadAssemblyJoint {
            id: "joint.world_slider".to_string(),
            name: Some("World Slider".to_string()),
            parent_instance_id: None,
            child_instance_id: "carriage-1".to_string(),
            parent_anchor: Vec3::new(0.0, 0.0, 0.0),
            child_anchor: Vec3::new(0.0, 0.0, 0.0),
            kind: CadJointKind::Slider {
                axis: Vec3::new(1.0, 0.0, 0.0),
                limits: Some((0.0, 100.0)),
            },
            state: 30.0,
        }],
        ground_instance_id: None,
    }
}

fn invalid_reference_scene() -> CadAssemblySchema {
    CadAssemblySchema {
        part_defs: BTreeMap::from([(
            "base".to_string(),
            CadPartDef {
                id: "base".to_string(),
                name: Some("Base".to_string()),
                root: 1,
                default_material: None,
            },
        )]),
        instances: vec![
            CadPartInstance {
                id: "base-1".to_string(),
                part_def_id: "base".to_string(),
                name: None,
                transform: None,
                material: None,
            },
            CadPartInstance {
                id: "orphan-1".to_string(),
                part_def_id: "missing-def".to_string(),
                name: None,
                transform: None,
                material: None,
            },
        ],
        joints: vec![CadAssemblyJoint {
            id: "joint.missing_refs".to_string(),
            name: None,
            parent_instance_id: Some("missing-parent".to_string()),
            child_instance_id: "missing-child".to_string(),
            parent_anchor: Vec3::new(0.0, 0.0, 0.0),
            child_anchor: Vec3::new(0.0, 0.0, 0.0),
            kind: CadJointKind::Fixed,
            state: 0.0,
        }],
        ground_instance_id: Some("missing-ground".to_string()),
    }
}

fn cyclic_joint_scene() -> CadAssemblySchema {
    CadAssemblySchema {
        part_defs: BTreeMap::from([
            (
                "a".to_string(),
                CadPartDef {
                    id: "a".to_string(),
                    name: Some("A".to_string()),
                    root: 1,
                    default_material: None,
                },
            ),
            (
                "b".to_string(),
                CadPartDef {
                    id: "b".to_string(),
                    name: Some("B".to_string()),
                    root: 2,
                    default_material: None,
                },
            ),
        ]),
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
                id: "joint.a_to_b".to_string(),
                name: None,
                parent_instance_id: Some("a-1".to_string()),
                child_instance_id: "b-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Fixed,
                state: 0.0,
            },
            CadAssemblyJoint {
                id: "joint.b_to_a".to_string(),
                name: None,
                parent_instance_id: Some("b-1".to_string()),
                child_instance_id: "a-1".to_string(),
                parent_anchor: Vec3::new(0.0, 0.0, 0.0),
                child_anchor: Vec3::new(0.0, 0.0, 0.0),
                kind: CadJointKind::Fixed,
                state: 0.0,
            },
        ],
        ground_instance_id: None,
    }
}

fn parity_signature(
    snapshot: &AssemblyAcceptanceScenesSnapshot,
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
        .expect("serialize assembly acceptance scenes parity payload"),
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
        AssemblyAcceptanceSceneCaseSnapshot, AssemblyAcceptanceScenesSnapshot, parity_signature,
    };

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let snapshot = AssemblyAcceptanceScenesSnapshot {
            case_snapshots: vec![AssemblyAcceptanceSceneCaseSnapshot {
                case_id: "case".to_string(),
                accepted: true,
                grounded_instance_id: None,
                invalid_ground_instance_id: None,
                contains_joint_cycle: false,
                instance_ids: Vec::new(),
                joint_ids: Vec::new(),
                missing_part_def_instance_ids: Vec::new(),
                missing_joint_child_instance_ids: Vec::new(),
                missing_joint_parent_instance_ids: Vec::new(),
                fk_resolved_instance_ids: Vec::new(),
            }],
        };

        let first = parity_signature(&snapshot, true, true, true, "abc");
        let second = parity_signature(&snapshot, true, true, true, "abc");
        assert_eq!(first, second);
    }
}
