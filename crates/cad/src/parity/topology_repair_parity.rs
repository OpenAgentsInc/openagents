use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parity::scorecard::ParityScorecard;
use crate::topology_repair::{
    TopologyDefectCounts, TopologyRepairOperation, TopologyRepairRequest, TopologyRepairStatus,
    repair_topology_after_operation,
};

pub const PARITY_TOPOLOGY_REPAIR_ISSUE_ID: &str = "VCAD-PARITY-035";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TopologyRepairParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub no_repair_snapshot: TopologyRepairSnapshot,
    pub boolean_repair_snapshot: TopologyRepairSnapshot,
    pub finishing_fallback_snapshot: TopologyRepairSnapshot,
    pub deterministic_replay_match: bool,
    pub invalid_request_error: String,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TopologyRepairSnapshot {
    pub status: String,
    pub geometry_hash: String,
    pub defects_before: TopologyDefectCounts,
    pub defects_after: TopologyDefectCounts,
    pub action_count: usize,
    pub warning_codes: Vec<String>,
}

pub fn build_topology_repair_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> TopologyRepairParityManifest {
    let no_repair = repair_topology_after_operation(&TopologyRepairRequest {
        feature_id: "feature.repair.none".to_string(),
        operation: TopologyRepairOperation::Boolean,
        source_geometry_hash: "hash.source.none".to_string(),
        candidate_geometry_hash: "hash.candidate.none".to_string(),
        defects_before: TopologyDefectCounts::default(),
        allow_fallback: true,
    })
    .expect("no-repair sample should evaluate");
    let no_repair_snapshot = TopologyRepairSnapshot {
        status: status_label(no_repair.status),
        geometry_hash: no_repair.geometry_hash.clone(),
        defects_before: no_repair.defects_before,
        defects_after: no_repair.defects_after,
        action_count: no_repair.actions.len(),
        warning_codes: no_repair
            .warnings
            .iter()
            .map(|warning| warning.code.stable_code().to_string())
            .collect(),
    };

    let boolean_repair = repair_topology_after_operation(&TopologyRepairRequest {
        feature_id: "feature.repair.boolean".to_string(),
        operation: TopologyRepairOperation::Boolean,
        source_geometry_hash: "hash.source.boolean".to_string(),
        candidate_geometry_hash: "hash.candidate.boolean".to_string(),
        defects_before: TopologyDefectCounts {
            non_manifold_edges: 2,
            self_intersections: 1,
            sliver_faces: 3,
        },
        allow_fallback: true,
    })
    .expect("boolean repair sample should evaluate");
    let boolean_repair_snapshot = TopologyRepairSnapshot {
        status: status_label(boolean_repair.status),
        geometry_hash: boolean_repair.geometry_hash.clone(),
        defects_before: boolean_repair.defects_before,
        defects_after: boolean_repair.defects_after,
        action_count: boolean_repair.actions.len(),
        warning_codes: boolean_repair
            .warnings
            .iter()
            .map(|warning| warning.code.stable_code().to_string())
            .collect(),
    };

    let finishing_fallback = repair_topology_after_operation(&TopologyRepairRequest {
        feature_id: "feature.repair.finishing".to_string(),
        operation: TopologyRepairOperation::Finishing,
        source_geometry_hash: "hash.source.finishing".to_string(),
        candidate_geometry_hash: "hash.candidate.finishing".to_string(),
        defects_before: TopologyDefectCounts {
            non_manifold_edges: 11,
            self_intersections: 10,
            sliver_faces: 4,
        },
        allow_fallback: true,
    })
    .expect("finishing fallback sample should evaluate");
    let finishing_fallback_snapshot = TopologyRepairSnapshot {
        status: status_label(finishing_fallback.status),
        geometry_hash: finishing_fallback.geometry_hash.clone(),
        defects_before: finishing_fallback.defects_before,
        defects_after: finishing_fallback.defects_after,
        action_count: finishing_fallback.actions.len(),
        warning_codes: finishing_fallback
            .warnings
            .iter()
            .map(|warning| warning.code.stable_code().to_string())
            .collect(),
    };

    let replay_boolean = repair_topology_after_operation(&TopologyRepairRequest {
        feature_id: "feature.repair.boolean".to_string(),
        operation: TopologyRepairOperation::Boolean,
        source_geometry_hash: "hash.source.boolean".to_string(),
        candidate_geometry_hash: "hash.candidate.boolean".to_string(),
        defects_before: TopologyDefectCounts {
            non_manifold_edges: 2,
            self_intersections: 1,
            sliver_faces: 3,
        },
        allow_fallback: true,
    })
    .expect("boolean replay should evaluate");
    let replay_finishing = repair_topology_after_operation(&TopologyRepairRequest {
        feature_id: "feature.repair.finishing".to_string(),
        operation: TopologyRepairOperation::Finishing,
        source_geometry_hash: "hash.source.finishing".to_string(),
        candidate_geometry_hash: "hash.candidate.finishing".to_string(),
        defects_before: TopologyDefectCounts {
            non_manifold_edges: 11,
            self_intersections: 10,
            sliver_faces: 4,
        },
        allow_fallback: true,
    })
    .expect("finishing replay should evaluate");
    let deterministic_replay_match =
        boolean_repair == replay_boolean && finishing_fallback == replay_finishing;

    let invalid_request_error = repair_topology_after_operation(&TopologyRepairRequest {
        feature_id: String::new(),
        operation: TopologyRepairOperation::Boolean,
        source_geometry_hash: "hash.source.invalid".to_string(),
        candidate_geometry_hash: "hash.candidate.invalid".to_string(),
        defects_before: TopologyDefectCounts::default(),
        allow_fallback: true,
    })
    .expect_err("empty repair feature id must fail")
    .to_string();

    let deterministic_signature = parity_signature(
        &no_repair_snapshot,
        &boolean_repair_snapshot,
        &finishing_fallback_snapshot,
        deterministic_replay_match,
        &invalid_request_error,
    );

    TopologyRepairParityManifest {
        manifest_version: 1,
        issue_id: PARITY_TOPOLOGY_REPAIR_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        no_repair_snapshot,
        boolean_repair_snapshot,
        finishing_fallback_snapshot,
        deterministic_replay_match,
        invalid_request_error,
        deterministic_signature,
        parity_contracts: vec![
            "post-op topology repair emits deterministic no-op receipts when no defects exist"
                .to_string(),
            "boolean repair path applies deterministic action selection and defect reduction"
                .to_string(),
            "finishing repair path supports deterministic fallback to source geometry".to_string(),
            "fallback warning code is stable: CAD-WARN-NON-MANIFOLD".to_string(),
            "topology repair replay is deterministic for identical request payloads".to_string(),
        ],
    }
}

fn status_label(status: TopologyRepairStatus) -> String {
    match status {
        TopologyRepairStatus::NoRepairNeeded => "no_repair_needed".to_string(),
        TopologyRepairStatus::Repaired => "repaired".to_string(),
        TopologyRepairStatus::FallbackKeptSource => "fallback_kept_source".to_string(),
    }
}

fn parity_signature(
    no_repair_snapshot: &TopologyRepairSnapshot,
    boolean_repair_snapshot: &TopologyRepairSnapshot,
    finishing_fallback_snapshot: &TopologyRepairSnapshot,
    deterministic_replay_match: bool,
    invalid_request_error: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            no_repair_snapshot,
            boolean_repair_snapshot,
            finishing_fallback_snapshot,
            deterministic_replay_match,
            invalid_request_error,
        ))
        .expect("serialize topology repair parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_TOPOLOGY_REPAIR_ISSUE_ID, build_topology_repair_parity_manifest};
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
    fn build_manifest_tracks_topology_repair_contracts() {
        let manifest = build_topology_repair_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_TOPOLOGY_REPAIR_ISSUE_ID);
        assert_eq!(manifest.no_repair_snapshot.status, "no_repair_needed");
        assert_eq!(manifest.boolean_repair_snapshot.status, "repaired");
        assert_eq!(
            manifest.finishing_fallback_snapshot.status,
            "fallback_kept_source"
        );
        assert!(manifest.deterministic_replay_match);
        assert!(
            manifest
                .finishing_fallback_snapshot
                .warning_codes
                .contains(&"CAD-WARN-NON-MANIFOLD".to_string())
        );
        assert!(manifest.invalid_request_error.contains("feature id"));
    }
}
