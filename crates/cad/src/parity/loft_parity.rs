use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::features::{LoftFeatureOp, LoftFeatureProfile, evaluate_loft_feature};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_LOFT_ISSUE_ID: &str = "VCAD-PARITY-034";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoftParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub open_loft_snapshot: LoftSnapshot,
    pub closed_loft_snapshot: LoftSnapshot,
    pub deterministic_replay_match: bool,
    pub invalid_too_few_profiles_error: String,
    pub invalid_mismatched_segments_error: String,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoftSnapshot {
    pub profile_count: usize,
    pub vertices_per_profile: usize,
    pub transition_count: usize,
    pub lateral_patch_count: usize,
    pub cap_count: usize,
    pub first_profile_centroid_mm: [f64; 3],
    pub last_profile_centroid_mm: [f64; 3],
    pub geometry_hash: String,
}

pub fn build_loft_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> LoftParityManifest {
    let profiles = vec![
        loft_square_profile("profile.a", 0.0, 20.0),
        loft_square_profile("profile.b", 25.0, 14.0),
        loft_square_profile("profile.c", 50.0, 8.0),
    ];
    let source_feature_ids = vec![
        "feature.profile.a".to_string(),
        "feature.profile.b".to_string(),
        "feature.profile.c".to_string(),
    ];
    let source_geometry_hashes = vec![
        "hash.profile.a".to_string(),
        "hash.profile.b".to_string(),
        "hash.profile.c".to_string(),
    ];

    let open_result = evaluate_loft_feature(
        &LoftFeatureOp {
            feature_id: "feature.loft.open".to_string(),
            source_feature_ids: source_feature_ids.clone(),
            profiles: profiles.clone(),
            closed: false,
        },
        &source_geometry_hashes,
    )
    .expect("open loft parity sample should evaluate");
    let open_loft_snapshot = LoftSnapshot {
        profile_count: open_result.profile_count,
        vertices_per_profile: open_result.vertices_per_profile,
        transition_count: open_result.transition_count,
        lateral_patch_count: open_result.lateral_patch_count,
        cap_count: open_result.cap_count,
        first_profile_centroid_mm: open_result.profile_centroids_mm[0],
        last_profile_centroid_mm: *open_result
            .profile_centroids_mm
            .last()
            .expect("open loft should have profile centroids"),
        geometry_hash: open_result.geometry_hash.clone(),
    };

    let closed_result = evaluate_loft_feature(
        &LoftFeatureOp {
            feature_id: "feature.loft.closed".to_string(),
            source_feature_ids: source_feature_ids.clone(),
            profiles: profiles.clone(),
            closed: true,
        },
        &source_geometry_hashes,
    )
    .expect("closed loft parity sample should evaluate");
    let closed_loft_snapshot = LoftSnapshot {
        profile_count: closed_result.profile_count,
        vertices_per_profile: closed_result.vertices_per_profile,
        transition_count: closed_result.transition_count,
        lateral_patch_count: closed_result.lateral_patch_count,
        cap_count: closed_result.cap_count,
        first_profile_centroid_mm: closed_result.profile_centroids_mm[0],
        last_profile_centroid_mm: *closed_result
            .profile_centroids_mm
            .last()
            .expect("closed loft should have profile centroids"),
        geometry_hash: closed_result.geometry_hash.clone(),
    };

    let replay_open = evaluate_loft_feature(
        &LoftFeatureOp {
            feature_id: "feature.loft.open".to_string(),
            source_feature_ids: source_feature_ids.clone(),
            profiles: profiles.clone(),
            closed: false,
        },
        &source_geometry_hashes,
    )
    .expect("open loft replay should evaluate");
    let replay_closed = evaluate_loft_feature(
        &LoftFeatureOp {
            feature_id: "feature.loft.closed".to_string(),
            source_feature_ids: source_feature_ids.clone(),
            profiles: profiles.clone(),
            closed: true,
        },
        &source_geometry_hashes,
    )
    .expect("closed loft replay should evaluate");
    let deterministic_replay_match = open_result == replay_open && closed_result == replay_closed;

    let invalid_too_few_profiles_error = evaluate_loft_feature(
        &LoftFeatureOp {
            feature_id: "feature.loft.too_few".to_string(),
            source_feature_ids: vec!["feature.profile.only".to_string()],
            profiles: vec![loft_square_profile("profile.only", 0.0, 20.0)],
            closed: false,
        },
        &["hash.profile.only".to_string()],
    )
    .expect_err("too few loft profiles must fail")
    .to_string();

    let invalid_mismatched_segments_error = evaluate_loft_feature(
        &LoftFeatureOp {
            feature_id: "feature.loft.bad_segments".to_string(),
            source_feature_ids: vec![
                "feature.profile.a".to_string(),
                "feature.profile.b".to_string(),
            ],
            profiles: vec![
                loft_square_profile("profile.a", 0.0, 20.0),
                LoftFeatureProfile {
                    profile_id: "profile.b".to_string(),
                    vertices_mm: vec![[0.0, 0.0, 10.0], [10.0, 0.0, 10.0], [5.0, 7.0, 10.0]],
                },
            ],
            closed: false,
        },
        &["hash.profile.a".to_string(), "hash.profile.b".to_string()],
    )
    .expect_err("mismatched loft vertex counts must fail")
    .to_string();

    let deterministic_signature = parity_signature(
        &open_loft_snapshot,
        &closed_loft_snapshot,
        deterministic_replay_match,
        &invalid_too_few_profiles_error,
        &invalid_mismatched_segments_error,
    );

    LoftParityManifest {
        manifest_version: 1,
        issue_id: PARITY_LOFT_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        open_loft_snapshot,
        closed_loft_snapshot,
        deterministic_replay_match,
        invalid_too_few_profiles_error,
        invalid_mismatched_segments_error,
        deterministic_signature,
        parity_contracts: vec![
            "loft requires at least 2 profiles with deterministic profile ordering".to_string(),
            "loft profiles must provide uniform vertex counts for deterministic pairing"
                .to_string(),
            "open loft emits cap faces while closed loft removes cap faces deterministically"
                .to_string(),
            "lateral patch count follows deterministic topology contract: transitions * vertices"
                .to_string(),
            "loft replay is deterministic for identical profiles and closed option".to_string(),
        ],
    }
}

fn loft_square_profile(profile_id: &str, z_mm: f64, size_mm: f64) -> LoftFeatureProfile {
    LoftFeatureProfile {
        profile_id: profile_id.to_string(),
        vertices_mm: vec![
            [0.0, 0.0, z_mm],
            [size_mm, 0.0, z_mm],
            [size_mm, size_mm, z_mm],
            [0.0, size_mm, z_mm],
        ],
    }
}

fn parity_signature(
    open_loft_snapshot: &LoftSnapshot,
    closed_loft_snapshot: &LoftSnapshot,
    deterministic_replay_match: bool,
    invalid_too_few_profiles_error: &str,
    invalid_mismatched_segments_error: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            open_loft_snapshot,
            closed_loft_snapshot,
            deterministic_replay_match,
            invalid_too_few_profiles_error,
            invalid_mismatched_segments_error,
        ))
        .expect("serialize loft parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_LOFT_ISSUE_ID, build_loft_parity_manifest};
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
    fn build_manifest_tracks_loft_parity_contracts() {
        let manifest = build_loft_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_LOFT_ISSUE_ID);
        assert_eq!(manifest.open_loft_snapshot.transition_count, 2);
        assert_eq!(manifest.open_loft_snapshot.cap_count, 2);
        assert_eq!(manifest.closed_loft_snapshot.transition_count, 3);
        assert_eq!(manifest.closed_loft_snapshot.cap_count, 0);
        assert!(manifest.deterministic_replay_match);
        assert!(
            manifest
                .invalid_too_few_profiles_error
                .contains("at least 2 profiles")
        );
        assert!(
            manifest
                .invalid_mismatched_segments_error
                .contains("vertex count mismatch")
        );
    }
}
