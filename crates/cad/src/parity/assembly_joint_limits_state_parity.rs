use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::assembly::{CadAssemblyJoint, CadJointKind};
use crate::kernel_math::Vec3;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_ASSEMBLY_JOINT_LIMITS_STATE_ISSUE_ID: &str = "VCAD-PARITY-060";
pub const ASSEMBLY_JOINT_LIMITS_STATE_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/assembly_joint_limits_state_vcad_reference.json";
const ASSEMBLY_JOINT_LIMITS_STATE_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/assembly_joint_limits_state_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssemblyJointLimitsStateParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub dof_by_joint_type: BTreeMap<String, usize>,
    pub state_unit_by_joint_type: BTreeMap<String, String>,
    pub case_snapshots: Vec<JointLimitsStateCaseSnapshot>,
    pub conversion_roundtrip_match: bool,
    pub behavior_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct AssemblyJointLimitsStateReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    expected_dof_by_joint_type: BTreeMap<String, usize>,
    expected_state_unit_by_joint_type: BTreeMap<String, String>,
    expected_case_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct JointLimitsStateSnapshot {
    dof_by_joint_type: BTreeMap<String, usize>,
    state_unit_by_joint_type: BTreeMap<String, String>,
    case_snapshots: Vec<JointLimitsStateCaseSnapshot>,
    conversion_roundtrip_match: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JointLimitsStateCaseSnapshot {
    pub case_id: String,
    pub joint_type: String,
    pub requested_state: String,
    pub effective_state: String,
    pub limits: Option<[String; 2]>,
    pub was_clamped: bool,
    pub physics_state: String,
    pub roundtrip_state: String,
    pub dof: usize,
    pub state_unit: String,
}

pub fn build_assembly_joint_limits_state_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<AssemblyJointLimitsStateParityManifest> {
    let corpus: AssemblyJointLimitsStateReferenceCorpus = serde_json::from_str(
        ASSEMBLY_JOINT_LIMITS_STATE_REFERENCE_CORPUS_JSON,
    )
    .map_err(|error| CadError::ParseFailed {
        reason: format!("failed to parse assembly joint limits/state reference corpus: {error}"),
    })?;

    let reference_corpus_sha256 =
        sha256_hex(ASSEMBLY_JOINT_LIMITS_STATE_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = run_joint_limits_state_cases();
    let replay_snapshot = run_joint_limits_state_cases();
    let deterministic_replay_match = snapshot == replay_snapshot;

    let case_ids = sorted(
        snapshot
            .case_snapshots
            .iter()
            .map(|case| case.case_id.clone())
            .collect(),
    );
    let expected_case_ids = sorted(corpus.expected_case_ids);

    let behavior_match = snapshot.dof_by_joint_type == corpus.expected_dof_by_joint_type
        && snapshot.state_unit_by_joint_type == corpus.expected_state_unit_by_joint_type
        && case_ids == expected_case_ids
        && snapshot.conversion_roundtrip_match;

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        behavior_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(AssemblyJointLimitsStateParityManifest {
        manifest_version: 1,
        issue_id: PARITY_ASSEMBLY_JOINT_LIMITS_STATE_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: ASSEMBLY_JOINT_LIMITS_STATE_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        dof_by_joint_type: snapshot.dof_by_joint_type,
        state_unit_by_joint_type: snapshot.state_unit_by_joint_type,
        case_snapshots: snapshot.case_snapshots,
        conversion_roundtrip_match: snapshot.conversion_roundtrip_match,
        behavior_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "joint dof semantics match vcad physics mapping (fixed=0, rev/slider/cyl=1, ball=3)"
                .to_string(),
            "state units and conversions match vcad semantics (deg<->rad, mm<->m, fixed=0)"
                .to_string(),
            "revolute/slider limits clamp requested state before conversion".to_string(),
            "joint limits/state parity fixtures replay deterministically".to_string(),
        ],
    })
}

fn run_joint_limits_state_cases() -> JointLimitsStateSnapshot {
    let mut dof_by_joint_type = BTreeMap::new();
    let mut state_unit_by_joint_type = BTreeMap::new();
    let mut case_snapshots = Vec::new();

    let mut revolute = joint_template(
        "joint.revolute.limit",
        CadJointKind::Revolute {
            axis: Vec3::new(0.0, 0.0, 1.0),
            limits: Some((-90.0, 90.0)),
        },
    );
    case_snapshots.push(build_case_snapshot(
        "revolute.limit_clamp",
        &mut revolute,
        120.0,
    ));

    let mut slider = joint_template(
        "joint.slider.limit",
        CadJointKind::Slider {
            axis: Vec3::new(1.0, 0.0, 0.0),
            limits: Some((100.0, -100.0)),
        },
    );
    case_snapshots.push(build_case_snapshot(
        "slider.limit_clamp",
        &mut slider,
        150.0,
    ));

    let mut fixed = joint_template("joint.fixed.state", CadJointKind::Fixed);
    case_snapshots.push(build_case_snapshot("fixed.force_zero", &mut fixed, 42.0));

    let mut cylindrical = joint_template(
        "joint.cylindrical.state",
        CadJointKind::Cylindrical {
            axis: Vec3::new(0.0, 0.0, 1.0),
        },
    );
    case_snapshots.push(build_case_snapshot(
        "cylindrical.rad_conversion",
        &mut cylindrical,
        180.0,
    ));

    let mut ball = joint_template("joint.ball.state", CadJointKind::Ball);
    case_snapshots.push(build_case_snapshot(
        "ball.rad_conversion",
        &mut ball,
        -180.0,
    ));

    for (joint_type, joint) in [
        ("Fixed", &fixed),
        ("Revolute", &revolute),
        ("Slider", &slider),
        ("Cylindrical", &cylindrical),
        ("Ball", &ball),
    ] {
        dof_by_joint_type.insert(joint_type.to_string(), joint.joint_dof());
        state_unit_by_joint_type.insert(
            joint_type.to_string(),
            joint.resolve_state_semantics(0.0).state_unit,
        );
    }

    case_snapshots.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    let conversion_roundtrip_match = case_snapshots.iter().all(|case| {
        let requested = parse_f64(&case.requested_state);
        let roundtrip = parse_f64(&case.roundtrip_state);
        let effective = parse_f64(&case.effective_state);
        if case.joint_type == "Fixed" {
            roundtrip.abs() <= 1e-9 && effective.abs() <= 1e-9
        } else if case.was_clamped {
            (roundtrip - effective).abs() <= 1e-9
        } else {
            (roundtrip - requested).abs() <= 1e-9
        }
    });

    JointLimitsStateSnapshot {
        dof_by_joint_type,
        state_unit_by_joint_type,
        case_snapshots,
        conversion_roundtrip_match,
    }
}

fn joint_template(id: &str, kind: CadJointKind) -> CadAssemblyJoint {
    CadAssemblyJoint {
        id: id.to_string(),
        name: None,
        parent_instance_id: Some("base-1".to_string()),
        child_instance_id: "arm-1".to_string(),
        parent_anchor: Vec3::new(0.0, 0.0, 0.0),
        child_anchor: Vec3::new(0.0, 0.0, 0.0),
        kind,
        state: 0.0,
    }
}

fn build_case_snapshot(
    case_id: &str,
    joint: &mut CadAssemblyJoint,
    requested_state: f64,
) -> JointLimitsStateCaseSnapshot {
    let semantics = joint.set_state_with_limits(requested_state);
    JointLimitsStateCaseSnapshot {
        case_id: case_id.to_string(),
        joint_type: joint_type_label(&joint.kind).to_string(),
        requested_state: f(requested_state),
        effective_state: f(semantics.effective_state),
        limits: semantics.limits.map(|(lower, upper)| [f(lower), f(upper)]),
        was_clamped: semantics.was_clamped,
        physics_state: f(semantics.physics_state),
        roundtrip_state: f(joint.convert_state_from_physics_units(semantics.physics_state)),
        dof: semantics.dof,
        state_unit: semantics.state_unit,
    }
}

fn joint_type_label(kind: &CadJointKind) -> &'static str {
    match kind {
        CadJointKind::Fixed => "Fixed",
        CadJointKind::Revolute { .. } => "Revolute",
        CadJointKind::Slider { .. } => "Slider",
        CadJointKind::Cylindrical { .. } => "Cylindrical",
        CadJointKind::Ball => "Ball",
    }
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn f(value: f64) -> String {
    format!("{value:.6}")
}

fn parse_f64(value: &str) -> f64 {
    value
        .parse::<f64>()
        .expect("parse f64 from formatted value")
}

fn parity_signature(
    snapshot: &JointLimitsStateSnapshot,
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
        .expect("serialize assembly joint limits/state parity payload"),
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
    use super::JointLimitsStateCaseSnapshot;
    use super::JointLimitsStateSnapshot;
    use super::parity_signature;

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let snapshot = JointLimitsStateSnapshot {
            dof_by_joint_type: BTreeMap::from([("Fixed".to_string(), 0)]),
            state_unit_by_joint_type: BTreeMap::from([("Fixed".to_string(), "fixed".to_string())]),
            case_snapshots: vec![JointLimitsStateCaseSnapshot {
                case_id: "fixed".to_string(),
                joint_type: "Fixed".to_string(),
                requested_state: "0.000000".to_string(),
                effective_state: "0.000000".to_string(),
                limits: None,
                was_clamped: false,
                physics_state: "0.000000".to_string(),
                roundtrip_state: "0.000000".to_string(),
                dof: 0,
                state_unit: "fixed".to_string(),
            }],
            conversion_roundtrip_match: true,
        };

        let first = parity_signature(&snapshot, true, true, true, "abc");
        let second = parity_signature(&snapshot, true, true, true, "abc");
        assert_eq!(first, second);
    }

    use std::collections::BTreeMap;
}
