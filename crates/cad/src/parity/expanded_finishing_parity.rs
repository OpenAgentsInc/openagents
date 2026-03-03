use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::finishing_ops::{
    ChamferFeatureOp, FilletFeatureOp, FinishingConstraintMode, FinishingContext, FinishingStatus,
    evaluate_chamfer_feature, evaluate_fillet_feature,
};
use crate::params::{ParameterStore, ScalarUnit, ScalarValue};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_EXPANDED_FINISHING_ISSUE_ID: &str = "VCAD-PARITY-032";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExpandedFinishingParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub fillet_expansion_snapshot: ExpandedConstraintSnapshot,
    pub chamfer_expansion_snapshot: ExpandedConstraintSnapshot,
    pub mode_roundtrip_snapshot: ModeRoundtripSnapshot,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpandedConstraintSnapshot {
    pub planar_safe_error: String,
    pub expanded_status: String,
    pub expanded_geometry_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModeRoundtripSnapshot {
    pub fillet_mode_roundtrip: String,
    pub chamfer_mode_roundtrip: String,
    pub legacy_missing_mode_default: String,
}

pub fn build_expanded_finishing_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> ExpandedFinishingParityManifest {
    let context = FinishingContext {
        source_geometry_hash: "expanded-finishing-source-hash".to_string(),
        source_min_thickness_mm: Some(20.0),
        source_volume_mm3: Some(2_000_000.0),
    };

    let fillet_planar = FilletFeatureOp {
        feature_id: "feature.fillet.planar".to_string(),
        source_feature_id: "feature.base".to_string(),
        radius_param: "fillet_radius_mm".to_string(),
        edge_refs: vec!["edge.1".to_string()],
        constraint_mode: FinishingConstraintMode::PlanarSafe,
        allow_fallback: false,
    };
    let fillet_expanded = FilletFeatureOp {
        feature_id: "feature.fillet.expanded".to_string(),
        source_feature_id: "feature.base".to_string(),
        radius_param: "fillet_radius_mm".to_string(),
        edge_refs: vec!["edge.1".to_string()],
        constraint_mode: FinishingConstraintMode::Expanded,
        allow_fallback: false,
    };
    let fillet_planar_error =
        evaluate_fillet_feature(&fillet_planar, &fillet_params(10.0), &context)
            .expect_err("planar-safe fillet should reject expanded sample")
            .to_string();
    let fillet_expanded_result =
        evaluate_fillet_feature(&fillet_expanded, &fillet_params(10.0), &context)
            .expect("expanded fillet should apply");
    let fillet_expansion_snapshot = ExpandedConstraintSnapshot {
        planar_safe_error: fillet_planar_error,
        expanded_status: status_label(fillet_expanded_result.status),
        expanded_geometry_hash: fillet_expanded_result.geometry_hash,
    };

    let chamfer_planar = ChamferFeatureOp {
        feature_id: "feature.chamfer.planar".to_string(),
        source_feature_id: "feature.base".to_string(),
        distance_param: "chamfer_distance_mm".to_string(),
        edge_refs: vec!["edge.1".to_string()],
        constraint_mode: FinishingConstraintMode::PlanarSafe,
        allow_fallback: false,
    };
    let chamfer_expanded = ChamferFeatureOp {
        feature_id: "feature.chamfer.expanded".to_string(),
        source_feature_id: "feature.base".to_string(),
        distance_param: "chamfer_distance_mm".to_string(),
        edge_refs: vec!["edge.1".to_string()],
        constraint_mode: FinishingConstraintMode::Expanded,
        allow_fallback: false,
    };
    let chamfer_planar_error =
        evaluate_chamfer_feature(&chamfer_planar, &chamfer_params(9.0), &context)
            .expect_err("planar-safe chamfer should reject expanded sample")
            .to_string();
    let chamfer_expanded_result =
        evaluate_chamfer_feature(&chamfer_expanded, &chamfer_params(9.0), &context)
            .expect("expanded chamfer should apply");
    let chamfer_expansion_snapshot = ExpandedConstraintSnapshot {
        planar_safe_error: chamfer_planar_error,
        expanded_status: status_label(chamfer_expanded_result.status),
        expanded_geometry_hash: chamfer_expanded_result.geometry_hash,
    };

    let fillet_mode_roundtrip = {
        let node = fillet_expanded
            .to_feature_node()
            .expect("fillet expanded node should build");
        let parsed =
            FilletFeatureOp::from_feature_node(&node).expect("fillet expanded node should parse");
        mode_label(parsed.constraint_mode).to_string()
    };
    let chamfer_mode_roundtrip = {
        let node = chamfer_expanded
            .to_feature_node()
            .expect("chamfer expanded node should build");
        let parsed =
            ChamferFeatureOp::from_feature_node(&node).expect("chamfer expanded node should parse");
        mode_label(parsed.constraint_mode).to_string()
    };
    let legacy_missing_mode_default = {
        let mut node = fillet_expanded
            .to_feature_node()
            .expect("legacy fillet node source should build");
        node.params.remove("constraint_mode");
        let parsed = FilletFeatureOp::from_feature_node(&node)
            .expect("legacy fillet node without mode should parse");
        mode_label(parsed.constraint_mode).to_string()
    };

    let deterministic_replay_match = {
        let first = evaluate_fillet_feature(&fillet_expanded, &fillet_params(10.0), &context)
            .expect("expanded fillet first replay");
        let second = evaluate_fillet_feature(&fillet_expanded, &fillet_params(10.0), &context)
            .expect("expanded fillet second replay");
        let third = evaluate_chamfer_feature(&chamfer_expanded, &chamfer_params(9.0), &context)
            .expect("expanded chamfer first replay");
        let fourth = evaluate_chamfer_feature(&chamfer_expanded, &chamfer_params(9.0), &context)
            .expect("expanded chamfer second replay");
        first == second && third == fourth
    };

    let mode_roundtrip_snapshot = ModeRoundtripSnapshot {
        fillet_mode_roundtrip,
        chamfer_mode_roundtrip,
        legacy_missing_mode_default,
    };

    let deterministic_signature = parity_signature(
        &fillet_expansion_snapshot,
        &chamfer_expansion_snapshot,
        &mode_roundtrip_snapshot,
        deterministic_replay_match,
    );

    ExpandedFinishingParityManifest {
        manifest_version: 1,
        issue_id: PARITY_EXPANDED_FINISHING_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        fillet_expansion_snapshot,
        chamfer_expansion_snapshot,
        mode_roundtrip_snapshot,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "finishing constraint_mode supports planar_safe and expanded parity semantics"
                .to_string(),
            "expanded mode increases deterministic risk thresholds for fillet/chamfer paths"
                .to_string(),
            "legacy nodes without constraint_mode default to planar_safe for backward compatibility"
                .to_string(),
            "expanded mode node serialization round-trips deterministically for fillet/chamfer"
                .to_string(),
            "expanded mode replay is deterministic across repeated evaluation".to_string(),
        ],
    }
}

fn fillet_params(radius_mm: f64) -> ParameterStore {
    let mut params = ParameterStore::default();
    params
        .set(
            "fillet_radius_mm",
            ScalarValue {
                value: radius_mm,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("fillet radius should set");
    params
}

fn chamfer_params(distance_mm: f64) -> ParameterStore {
    let mut params = ParameterStore::default();
    params
        .set(
            "chamfer_distance_mm",
            ScalarValue {
                value: distance_mm,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("chamfer distance should set");
    params
}

fn status_label(status: FinishingStatus) -> String {
    match status {
        FinishingStatus::Applied => "applied".to_string(),
        FinishingStatus::FallbackKeptSource => "fallback_kept_source".to_string(),
    }
}

fn mode_label(mode: FinishingConstraintMode) -> &'static str {
    match mode {
        FinishingConstraintMode::PlanarSafe => "planar_safe",
        FinishingConstraintMode::Expanded => "expanded",
    }
}

fn parity_signature(
    fillet_snapshot: &ExpandedConstraintSnapshot,
    chamfer_snapshot: &ExpandedConstraintSnapshot,
    mode_snapshot: &ModeRoundtripSnapshot,
    deterministic_replay_match: bool,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            fillet_snapshot,
            chamfer_snapshot,
            mode_snapshot,
            deterministic_replay_match,
        ))
        .expect("serialize expanded finishing parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_EXPANDED_FINISHING_ISSUE_ID, build_expanded_finishing_parity_manifest};
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
    fn build_manifest_tracks_expanded_finishing_contracts() {
        let manifest =
            build_expanded_finishing_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_EXPANDED_FINISHING_ISSUE_ID);
        assert_eq!(
            manifest.fillet_expansion_snapshot.expanded_status,
            "applied"
        );
        assert_eq!(
            manifest.chamfer_expansion_snapshot.expanded_status,
            "applied"
        );
        assert_eq!(
            manifest.mode_roundtrip_snapshot.fillet_mode_roundtrip,
            "expanded"
        );
        assert_eq!(
            manifest.mode_roundtrip_snapshot.chamfer_mode_roundtrip,
            "expanded"
        );
        assert_eq!(
            manifest.mode_roundtrip_snapshot.legacy_missing_mode_default,
            "planar_safe"
        );
        assert!(manifest.deterministic_replay_match);
        assert!(
            manifest
                .fillet_expansion_snapshot
                .planar_safe_error
                .contains("exceeded")
        );
        assert!(
            manifest
                .chamfer_expansion_snapshot
                .planar_safe_error
                .contains("exceeded")
        );
    }
}
