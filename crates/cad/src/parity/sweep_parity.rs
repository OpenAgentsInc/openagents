use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::features::{SweepFeatureOp, evaluate_sweep_feature};
use crate::params::{ParameterStore, ScalarUnit, ScalarValue};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_SWEEP_ISSUE_ID: &str = "VCAD-PARITY-033";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SweepParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub default_segments_snapshot: SweepPathSnapshot,
    pub explicit_segments_snapshot: SweepPathSnapshot,
    pub control_snapshot: SweepControlSnapshot,
    pub deterministic_replay_match: bool,
    pub invalid_zero_length_path_error: String,
    pub invalid_scale_error: String,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SweepPathSnapshot {
    pub segment_count: u32,
    pub station_count: usize,
    pub path_length_mm: f64,
    pub start_center_mm: [f64; 3],
    pub end_center_mm: [f64; 3],
    pub geometry_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SweepControlSnapshot {
    pub start_scale: f64,
    pub mid_scale: f64,
    pub end_scale: f64,
    pub start_twist_rad: f64,
    pub mid_twist_rad: f64,
    pub end_twist_rad: f64,
    pub default_segments_applied: bool,
}

pub fn build_sweep_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> SweepParityManifest {
    let source_hash = "sweep-source-baseline-hash";

    let default_segments_op = SweepFeatureOp {
        feature_id: "feature.sweep.default_segments".to_string(),
        source_feature_id: "feature.profile.base".to_string(),
        path_points_mm: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 50.0], [25.0, 0.0, 50.0]],
        twist_angle_param: "sweep_twist_rad".to_string(),
        scale_start_param: "sweep_scale_start".to_string(),
        scale_end_param: "sweep_scale_end".to_string(),
        path_segments: 0,
    };
    let controls = sweep_params(std::f64::consts::FRAC_PI_2, 1.0, 0.5);
    let default_result = evaluate_sweep_feature(&default_segments_op, &controls, source_hash)
        .expect("default-segment sweep parity sample should evaluate");
    let default_mid_station = &default_result.stations[default_result.stations.len() / 2];
    let default_segments_snapshot = SweepPathSnapshot {
        segment_count: default_result.segment_count,
        station_count: default_result.stations.len(),
        path_length_mm: default_result.path_length_mm,
        start_center_mm: default_result.stations[0].center_mm,
        end_center_mm: default_result
            .stations
            .last()
            .expect("default sweep should have end station")
            .center_mm,
        geometry_hash: default_result.geometry_hash.clone(),
    };

    let explicit_segments_op = SweepFeatureOp {
        feature_id: "feature.sweep.explicit_segments".to_string(),
        source_feature_id: "feature.profile.base".to_string(),
        path_points_mm: vec![[0.0, 0.0, 0.0], [0.0, 20.0, 0.0], [0.0, 20.0, 20.0]],
        twist_angle_param: "sweep_twist_rad".to_string(),
        scale_start_param: "sweep_scale_start".to_string(),
        scale_end_param: "sweep_scale_end".to_string(),
        path_segments: 8,
    };
    let explicit_result = evaluate_sweep_feature(&explicit_segments_op, &controls, source_hash)
        .expect("explicit-segment sweep parity sample should evaluate");
    let explicit_segments_snapshot = SweepPathSnapshot {
        segment_count: explicit_result.segment_count,
        station_count: explicit_result.stations.len(),
        path_length_mm: explicit_result.path_length_mm,
        start_center_mm: explicit_result.stations[0].center_mm,
        end_center_mm: explicit_result
            .stations
            .last()
            .expect("explicit sweep should have end station")
            .center_mm,
        geometry_hash: explicit_result.geometry_hash.clone(),
    };

    let control_snapshot = SweepControlSnapshot {
        start_scale: default_result.stations[0].scale,
        mid_scale: default_mid_station.scale,
        end_scale: default_result
            .stations
            .last()
            .expect("default sweep should have end station")
            .scale,
        start_twist_rad: default_result.stations[0].twist_angle_rad,
        mid_twist_rad: default_mid_station.twist_angle_rad,
        end_twist_rad: default_result
            .stations
            .last()
            .expect("default sweep should have end station")
            .twist_angle_rad,
        default_segments_applied: default_result.segment_count == 32,
    };

    let replay_default = evaluate_sweep_feature(&default_segments_op, &controls, source_hash)
        .expect("default-segment sweep replay should evaluate");
    let replay_explicit = evaluate_sweep_feature(&explicit_segments_op, &controls, source_hash)
        .expect("explicit-segment sweep replay should evaluate");
    let deterministic_replay_match =
        default_result == replay_default && explicit_result == replay_explicit;

    let invalid_zero_length_path_error = evaluate_sweep_feature(
        &SweepFeatureOp {
            feature_id: "feature.sweep.invalid.path".to_string(),
            source_feature_id: "feature.profile.base".to_string(),
            path_points_mm: vec![[10.0, 10.0, 10.0], [10.0, 10.0, 10.0]],
            twist_angle_param: "sweep_twist_rad".to_string(),
            scale_start_param: "sweep_scale_start".to_string(),
            scale_end_param: "sweep_scale_end".to_string(),
            path_segments: 8,
        },
        &controls,
        source_hash,
    )
    .expect_err("zero-length sweep path must fail")
    .to_string();

    let invalid_scale_error = evaluate_sweep_feature(
        &default_segments_op,
        &sweep_params(0.0, 1.0, 0.0),
        source_hash,
    )
    .expect_err("non-positive sweep scale should fail")
    .to_string();

    let deterministic_signature = parity_signature(
        &default_segments_snapshot,
        &explicit_segments_snapshot,
        &control_snapshot,
        deterministic_replay_match,
        &invalid_zero_length_path_error,
        &invalid_scale_error,
    );

    SweepParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SWEEP_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        default_segments_snapshot,
        explicit_segments_snapshot,
        control_snapshot,
        deterministic_replay_match,
        invalid_zero_length_path_error,
        invalid_scale_error,
        deterministic_signature,
        parity_contracts: vec![
            "sweep path controls support deterministic polyline path sampling".to_string(),
            "path_segments=0 maps to vcad-aligned default of 32 path segments".to_string(),
            "twist and scale controls interpolate deterministically from start to end along path"
                .to_string(),
            "zero-length paths and non-positive scales emit stable CAD diagnostics".to_string(),
            "repeated sweep evaluations replay deterministically for identical inputs".to_string(),
        ],
    }
}

fn sweep_params(twist_angle_rad: f64, scale_start: f64, scale_end: f64) -> ParameterStore {
    let mut params = ParameterStore::default();
    params
        .set(
            "sweep_twist_rad",
            ScalarValue {
                value: twist_angle_rad,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("sweep twist should set");
    params
        .set(
            "sweep_scale_start",
            ScalarValue {
                value: scale_start,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("sweep start scale should set");
    params
        .set(
            "sweep_scale_end",
            ScalarValue {
                value: scale_end,
                unit: ScalarUnit::Unitless,
            },
        )
        .expect("sweep end scale should set");
    params
}

fn parity_signature(
    default_segments_snapshot: &SweepPathSnapshot,
    explicit_segments_snapshot: &SweepPathSnapshot,
    control_snapshot: &SweepControlSnapshot,
    deterministic_replay_match: bool,
    invalid_zero_length_path_error: &str,
    invalid_scale_error: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            default_segments_snapshot,
            explicit_segments_snapshot,
            control_snapshot,
            deterministic_replay_match,
            invalid_zero_length_path_error,
            invalid_scale_error,
        ))
        .expect("serialize sweep parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_SWEEP_ISSUE_ID, build_sweep_parity_manifest};
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
    fn build_manifest_tracks_sweep_parity_contracts() {
        let manifest = build_sweep_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_SWEEP_ISSUE_ID);
        assert_eq!(manifest.default_segments_snapshot.segment_count, 32);
        assert_eq!(manifest.explicit_segments_snapshot.segment_count, 8);
        assert!(manifest.control_snapshot.default_segments_applied);
        assert!(manifest.deterministic_replay_match);
        assert!(
            manifest
                .invalid_zero_length_path_error
                .contains("zero length")
        );
        assert!(manifest.invalid_scale_error.contains("scale"));
    }
}
