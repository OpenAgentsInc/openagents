use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::features::{TransformFeatureOp, compose_transform_sequence, evaluate_transform_feature};
use crate::kernel_math::{Point3, Transform};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_TRANSFORM_ISSUE_ID: &str = "VCAD-PARITY-027";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransformParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub compose_snapshot: TransformComposeSnapshot,
    pub sequence_snapshot: TransformSequenceSnapshot,
    pub invalid_scale_error: String,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransformComposeSnapshot {
    pub feature_matrix_row_major: [f64; 16],
    pub kernel_reference_matrix_row_major: [f64; 16],
    pub feature_probe_point: [f64; 3],
    pub kernel_reference_probe_point: [f64; 3],
    pub matrix_match_within_tolerance: bool,
    pub geometry_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransformSequenceSnapshot {
    pub ab_matrix_row_major: [f64; 16],
    pub ba_matrix_row_major: [f64; 16],
    pub ab_probe_point: [f64; 3],
    pub ba_probe_point: [f64; 3],
    pub order_sensitive: bool,
    pub deterministic_replay_match: bool,
}

pub fn build_transform_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> TransformParityManifest {
    let source_geometry_hash = "transform-source-baseline-hash";
    let op = TransformFeatureOp {
        feature_id: "feature.transform.main".to_string(),
        source_feature_id: "feature.base".to_string(),
        translation_mm: [12.0, -3.0, 8.0],
        rotation_deg_xyz: [15.0, 30.0, 45.0],
        scale_xyz: [1.5, 0.5, 2.0],
    };

    let eval = evaluate_transform_feature(&op, source_geometry_hash)
        .expect("transform feature parity sample should evaluate");
    let kernel_reference_matrix = kernel_reference_matrix_row_major(&op);
    let probe = Point3::new(2.0, -1.0, 0.5);
    let feature_probe_point = apply_matrix_to_point(&eval.matrix_row_major, probe);
    let kernel_probe_point = apply_matrix_to_point(&kernel_reference_matrix, probe);

    let compose_snapshot = TransformComposeSnapshot {
        feature_matrix_row_major: eval.matrix_row_major,
        kernel_reference_matrix_row_major: kernel_reference_matrix,
        feature_probe_point,
        kernel_reference_probe_point: kernel_probe_point,
        matrix_match_within_tolerance: matrices_match_within_tolerance(
            &eval.matrix_row_major,
            &kernel_reference_matrix,
            1e-12,
        ) && points_match_within_tolerance(
            &feature_probe_point,
            &kernel_probe_point,
            1e-12,
        ),
        geometry_hash: eval.geometry_hash,
    };

    let op_a = TransformFeatureOp {
        feature_id: "feature.transform.a".to_string(),
        source_feature_id: "feature.base".to_string(),
        translation_mm: [5.0, 0.0, -2.0],
        rotation_deg_xyz: [0.0, 25.0, 0.0],
        scale_xyz: [1.0, 1.0, 1.0],
    };
    let op_b = TransformFeatureOp {
        feature_id: "feature.transform.b".to_string(),
        source_feature_id: "feature.transform.a".to_string(),
        translation_mm: [0.0, 4.0, 0.0],
        rotation_deg_xyz: [0.0, 0.0, 90.0],
        scale_xyz: [2.0, 1.0, 1.0],
    };

    let ab = compose_transform_sequence(&[op_a.clone(), op_b.clone()])
        .expect("transform compose sample should evaluate");
    let ba =
        compose_transform_sequence(&[op_b, op_a]).expect("transform compose order should evaluate");
    let replay = compose_transform_sequence(&[
        TransformFeatureOp {
            feature_id: "feature.transform.a".to_string(),
            source_feature_id: "feature.base".to_string(),
            translation_mm: [5.0, 0.0, -2.0],
            rotation_deg_xyz: [0.0, 25.0, 0.0],
            scale_xyz: [1.0, 1.0, 1.0],
        },
        TransformFeatureOp {
            feature_id: "feature.transform.b".to_string(),
            source_feature_id: "feature.transform.a".to_string(),
            translation_mm: [0.0, 4.0, 0.0],
            rotation_deg_xyz: [0.0, 0.0, 90.0],
            scale_xyz: [2.0, 1.0, 1.0],
        },
    ])
    .expect("transform replay compose should evaluate");

    let sequence_probe = Point3::new(1.0, 2.0, 3.0);
    let sequence_snapshot = TransformSequenceSnapshot {
        ab_matrix_row_major: ab,
        ba_matrix_row_major: ba,
        ab_probe_point: apply_matrix_to_point(&ab, sequence_probe),
        ba_probe_point: apply_matrix_to_point(&ba, sequence_probe),
        order_sensitive: ab != ba,
        deterministic_replay_match: ab == replay,
    };

    let invalid_scale_error = evaluate_transform_feature(
        &TransformFeatureOp {
            feature_id: "feature.invalid".to_string(),
            source_feature_id: "feature.base".to_string(),
            translation_mm: [0.0, 0.0, 0.0],
            rotation_deg_xyz: [0.0, 0.0, 0.0],
            scale_xyz: [0.0, 1.0, 1.0],
        },
        source_geometry_hash,
    )
    .expect_err("invalid transform scale should fail")
    .to_string();

    let deterministic_signature =
        parity_signature(&compose_snapshot, &sequence_snapshot, &invalid_scale_error);

    TransformParityManifest {
        manifest_version: 1,
        issue_id: PARITY_TRANSFORM_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        compose_snapshot,
        sequence_snapshot,
        invalid_scale_error,
        deterministic_signature,
        parity_contracts: vec![
            "transform feature matrix composes in vcad canonical order: T * Rz * Ry * Rx * S"
                .to_string(),
            "feature transform matrix matches kernel-math reference composition within tolerance"
                .to_string(),
            "compose_transform_sequence is deterministic for repeated runs with identical op order"
                .to_string(),
            "compose_transform_sequence preserves input order (A->B differs from B->A)".to_string(),
            "invalid non-positive scale maps to stable CadError::InvalidPrimitive diagnostics"
                .to_string(),
        ],
    }
}

fn kernel_reference_matrix_row_major(op: &TransformFeatureOp) -> [f64; 16] {
    let rx = Transform::rotation_x(op.rotation_deg_xyz[0].to_radians());
    let ry = Transform::rotation_y(op.rotation_deg_xyz[1].to_radians());
    let rz = Transform::rotation_z(op.rotation_deg_xyz[2].to_radians());
    let scale = Transform::scale(op.scale_xyz[0], op.scale_xyz[1], op.scale_xyz[2]);
    let translation = Transform::translation(
        op.translation_mm[0],
        op.translation_mm[1],
        op.translation_mm[2],
    );

    translation
        .then(&rz)
        .then(&ry)
        .then(&rx)
        .then(&scale)
        .matrix_row_major
}

fn apply_matrix_to_point(matrix: &[f64; 16], point: Point3) -> [f64; 3] {
    [
        matrix[0] * point.x + matrix[1] * point.y + matrix[2] * point.z + matrix[3],
        matrix[4] * point.x + matrix[5] * point.y + matrix[6] * point.z + matrix[7],
        matrix[8] * point.x + matrix[9] * point.y + matrix[10] * point.z + matrix[11],
    ]
}

fn matrices_match_within_tolerance(lhs: &[f64; 16], rhs: &[f64; 16], tolerance: f64) -> bool {
    lhs.iter()
        .zip(rhs.iter())
        .all(|(left, right)| (left - right).abs() <= tolerance)
}

fn points_match_within_tolerance(lhs: &[f64; 3], rhs: &[f64; 3], tolerance: f64) -> bool {
    lhs.iter()
        .zip(rhs.iter())
        .all(|(left, right)| (left - right).abs() <= tolerance)
}

fn parity_signature(
    compose_snapshot: &TransformComposeSnapshot,
    sequence_snapshot: &TransformSequenceSnapshot,
    invalid_scale_error: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(compose_snapshot, sequence_snapshot, invalid_scale_error))
            .expect("serialize transform parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_TRANSFORM_ISSUE_ID, build_transform_parity_manifest};
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
    fn build_manifest_tracks_transform_parity_contracts() {
        let manifest = build_transform_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_TRANSFORM_ISSUE_ID);
        assert!(manifest.compose_snapshot.matrix_match_within_tolerance);
        assert!(manifest.sequence_snapshot.order_sensitive);
        assert!(manifest.sequence_snapshot.deterministic_replay_match);
        assert!(manifest.invalid_scale_error.contains("transform scale x"));
        assert_eq!(manifest.deterministic_signature.len(), 16);
    }
}
