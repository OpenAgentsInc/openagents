use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::features::{
    CircularPatternFeatureOp, LinearPatternFeatureOp, evaluate_circular_pattern_feature,
    evaluate_linear_pattern_feature,
};
use crate::params::{ParameterStore, ScalarUnit, ScalarValue};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_PATTERN_ISSUE_ID: &str = "VCAD-PARITY-028";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatternParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub linear_snapshot: LinearPatternSnapshot,
    pub circular_snapshot: CircularPatternSnapshot,
    pub deterministic_replay_match: bool,
    pub invalid_linear_count_error: String,
    pub invalid_circular_axis_error: String,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LinearPatternSnapshot {
    pub instance_count: usize,
    pub pattern_indexes: Vec<u32>,
    pub first_translation_mm: [f64; 3],
    pub last_translation_mm: [f64; 3],
    pub pattern_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CircularPatternSnapshot {
    pub instance_count: usize,
    pub pattern_indexes: Vec<u32>,
    pub first_center_mm: [f64; 3],
    pub second_center_mm: [f64; 3],
    pub last_center_mm: [f64; 3],
    pub angle_samples_deg: Vec<f64>,
    pub pattern_hash: String,
    pub full_span_without_duplicate_endpoint: bool,
}

pub fn build_pattern_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> PatternParityManifest {
    let source_hash = "pattern-source-baseline-hash";

    let linear_op = LinearPatternFeatureOp {
        feature_id: "feature.linear.pattern".to_string(),
        source_feature_id: "feature.base.hole".to_string(),
        count_param: "linear_count".to_string(),
        spacing_param: "linear_spacing_mm".to_string(),
        direction_unit_xyz: [1.0, 1.0, 0.0],
        start_index: 10,
    };
    let linear_params_store = linear_params(5.0, 12.0);
    let linear = evaluate_linear_pattern_feature(&linear_op, &linear_params_store, source_hash)
        .expect("linear pattern parity sample should evaluate");

    let linear_snapshot = LinearPatternSnapshot {
        instance_count: linear.instances.len(),
        pattern_indexes: linear
            .instances
            .iter()
            .map(|entry| entry.pattern_index)
            .collect(),
        first_translation_mm: linear.instances[0].translation_mm,
        last_translation_mm: linear
            .instances
            .last()
            .expect("linear instances should have tail")
            .translation_mm,
        pattern_hash: linear.pattern_hash.clone(),
    };

    let circular_op = CircularPatternFeatureOp {
        feature_id: "feature.circular.pattern".to_string(),
        source_feature_id: "feature.base.hole".to_string(),
        count_param: "circular_count".to_string(),
        angle_deg_param: "circular_span_deg".to_string(),
        radius_param: "circular_radius_mm".to_string(),
        axis_origin_mm: [5.0, -2.0, 1.0],
        axis_direction_xyz: [0.0, 0.0, 1.0],
        start_index: 50,
    };
    let circular_params_store = circular_params(6.0, 360.0, 20.0);
    let circular =
        evaluate_circular_pattern_feature(&circular_op, &circular_params_store, source_hash)
            .expect("circular pattern parity sample should evaluate");

    let circular_snapshot = CircularPatternSnapshot {
        instance_count: circular.instances.len(),
        pattern_indexes: circular
            .instances
            .iter()
            .map(|entry| entry.pattern_index)
            .collect(),
        first_center_mm: circular.instances[0].center_mm,
        second_center_mm: circular.instances[1].center_mm,
        last_center_mm: circular
            .instances
            .last()
            .expect("circular instances should have tail")
            .center_mm,
        angle_samples_deg: circular
            .instances
            .iter()
            .take(4)
            .map(|entry| entry.angle_deg)
            .collect(),
        pattern_hash: circular.pattern_hash.clone(),
        full_span_without_duplicate_endpoint: circular
            .instances
            .iter()
            .map(|entry| entry.center_mm)
            .collect::<Vec<_>>()
            .windows(2)
            .all(|pair| pair[0] != pair[1]),
    };

    let replay_linear =
        evaluate_linear_pattern_feature(&linear_op, &linear_params_store, source_hash)
            .expect("linear replay should evaluate");
    let replay_circular =
        evaluate_circular_pattern_feature(&circular_op, &circular_params_store, source_hash)
            .expect("circular replay should evaluate");
    let deterministic_replay_match = linear == replay_linear && circular == replay_circular;

    let invalid_linear_count_error =
        evaluate_linear_pattern_feature(&linear_op, &linear_params(2.5, 12.0), source_hash)
            .expect_err("fractional linear count should fail")
            .to_string();

    let invalid_circular_axis_error = evaluate_circular_pattern_feature(
        &CircularPatternFeatureOp {
            feature_id: "feature.circular.invalid".to_string(),
            source_feature_id: "feature.base.hole".to_string(),
            count_param: "circular_count".to_string(),
            angle_deg_param: "circular_span_deg".to_string(),
            radius_param: "circular_radius_mm".to_string(),
            axis_origin_mm: [0.0, 0.0, 0.0],
            axis_direction_xyz: [0.0, 0.0, 0.0],
            start_index: 0,
        },
        &circular_params_store,
        source_hash,
    )
    .expect_err("zero circular axis should fail")
    .to_string();

    let deterministic_signature = parity_signature(
        &linear_snapshot,
        &circular_snapshot,
        deterministic_replay_match,
        &invalid_linear_count_error,
        &invalid_circular_axis_error,
    );

    PatternParityManifest {
        manifest_version: 1,
        issue_id: PARITY_PATTERN_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        linear_snapshot,
        circular_snapshot,
        deterministic_replay_match,
        invalid_linear_count_error,
        invalid_circular_axis_error,
        deterministic_signature,
        parity_contracts: vec![
            "linear pattern count includes original instance and uses normalized direction + spacing"
                .to_string(),
            "linear pattern instance indexes are deterministic and stable across replays"
                .to_string(),
            "circular pattern uses uniform angular steps (span/count) around axis origin + direction"
                .to_string(),
            "circular pattern span excludes duplicated endpoint copy for closed 360-degree loops"
                .to_string(),
            "invalid linear count and circular axis map to stable parameter/primitive diagnostics"
                .to_string(),
        ],
    }
}

fn linear_params(count: f64, spacing_mm: f64) -> ParameterStore {
    let mut params = ParameterStore::default();
    params
        .set(
            "linear_count",
            ScalarValue {
                value: count,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("linear count should set");
    params
        .set(
            "linear_spacing_mm",
            ScalarValue {
                value: spacing_mm,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("linear spacing should set");
    params
}

fn circular_params(count: f64, span_deg: f64, radius_mm: f64) -> ParameterStore {
    let mut params = ParameterStore::default();
    params
        .set(
            "circular_count",
            ScalarValue {
                value: count,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("circular count should set");
    params
        .set(
            "circular_span_deg",
            ScalarValue {
                value: span_deg,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("circular span should set");
    params
        .set(
            "circular_radius_mm",
            ScalarValue {
                value: radius_mm,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("circular radius should set");
    params
}

fn parity_signature(
    linear_snapshot: &LinearPatternSnapshot,
    circular_snapshot: &CircularPatternSnapshot,
    deterministic_replay_match: bool,
    invalid_linear_count_error: &str,
    invalid_circular_axis_error: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            linear_snapshot,
            circular_snapshot,
            deterministic_replay_match,
            invalid_linear_count_error,
            invalid_circular_axis_error,
        ))
        .expect("serialize pattern parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_PATTERN_ISSUE_ID, build_pattern_parity_manifest};
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
    fn build_manifest_tracks_linear_and_circular_pattern_contracts() {
        let manifest = build_pattern_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_PATTERN_ISSUE_ID);
        assert_eq!(manifest.linear_snapshot.instance_count, 5);
        assert_eq!(manifest.circular_snapshot.instance_count, 6);
        assert!(manifest.deterministic_replay_match);
        assert!(
            manifest
                .invalid_linear_count_error
                .contains("integer value")
        );
        assert!(
            manifest
                .invalid_circular_axis_error
                .contains("axis direction")
        );
        assert_eq!(manifest.deterministic_signature.len(), 16);
    }
}
