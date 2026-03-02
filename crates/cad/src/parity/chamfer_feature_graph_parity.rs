use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::feature_graph::{FeatureGraph, FeatureNode};
use crate::finishing_ops::{
    CHAMFER_OPERATION_KEY, ChamferFeatureOp, FinishingContext, FinishingFailureClass,
    FinishingStatus, evaluate_chamfer_feature,
};
use crate::params::{ParameterStore, ScalarUnit, ScalarValue};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_CHAMFER_FEATURE_GRAPH_ISSUE_ID: &str = "VCAD-PARITY-031";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChamferFeatureGraphParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub chamfer_node_snapshot: ChamferNodeSnapshot,
    pub graph_topo_order: Vec<String>,
    pub applied_snapshot: ChamferEvalSnapshot,
    pub fallback_snapshot: ChamferEvalSnapshot,
    pub fallback_warning_code: String,
    pub edge_selection_affects_hash: bool,
    pub invalid_distance_error: String,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChamferNodeSnapshot {
    pub operation_key: String,
    pub depends_on: Vec<String>,
    pub distance_param: String,
    pub edge_refs: Vec<String>,
    pub allow_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChamferEvalSnapshot {
    pub status: String,
    pub geometry_hash: String,
    pub failure_classification: Option<String>,
    pub warning_count: usize,
}

pub fn build_chamfer_feature_graph_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> ChamferFeatureGraphParityManifest {
    let source_context = FinishingContext {
        source_geometry_hash: "chamfer-source-geometry-hash".to_string(),
        source_min_thickness_mm: Some(20.0),
        source_volume_mm3: Some(2_000_000.0),
    };

    let op = ChamferFeatureOp {
        feature_id: "feature.chamfer.primary".to_string(),
        source_feature_id: "feature.base".to_string(),
        distance_param: "chamfer_distance_mm".to_string(),
        edge_refs: vec![
            "edge.003".to_string(),
            "edge.001".to_string(),
            "edge.001".to_string(),
        ],
        allow_fallback: false,
    };
    let node = op.to_feature_node().expect("chamfer node should build");
    let parsed = ChamferFeatureOp::from_feature_node(&node).expect("chamfer node should parse");

    let chamfer_node_snapshot = ChamferNodeSnapshot {
        operation_key: node.operation_key.clone(),
        depends_on: node.depends_on.clone(),
        distance_param: parsed.distance_param.clone(),
        edge_refs: parsed.edge_refs.clone(),
        allow_fallback: parsed.allow_fallback,
    };

    let graph = FeatureGraph {
        nodes: vec![
            FeatureNode {
                id: "feature.chamfer.primary".to_string(),
                name: "Chamfer".to_string(),
                operation_key: CHAMFER_OPERATION_KEY.to_string(),
                depends_on: vec!["feature.base".to_string()],
                params: node.params.clone(),
            },
            FeatureNode {
                id: "feature.base".to_string(),
                name: "Base".to_string(),
                operation_key: "primitive.box.v1".to_string(),
                depends_on: Vec::new(),
                params: std::collections::BTreeMap::new(),
            },
        ],
    };
    let graph_topo_order = graph
        .deterministic_topo_order()
        .expect("chamfer feature graph should topo-sort");

    let applied = evaluate_chamfer_feature(&parsed, &chamfer_params(1.5), &source_context)
        .expect("safe chamfer should apply");
    let applied_snapshot = ChamferEvalSnapshot {
        status: status_label(applied.status),
        geometry_hash: applied.geometry_hash,
        failure_classification: applied.failure_classification.map(class_label),
        warning_count: applied.warnings.len(),
    };

    let fallback_op = ChamferFeatureOp {
        feature_id: "feature.chamfer.fallback".to_string(),
        source_feature_id: "feature.base".to_string(),
        distance_param: "chamfer_distance_mm".to_string(),
        edge_refs: vec!["edge.010".to_string()],
        allow_fallback: true,
    };
    let fallback = evaluate_chamfer_feature(&fallback_op, &chamfer_params(12.0), &source_context)
        .expect("risky chamfer should fallback when enabled");
    let fallback_snapshot = ChamferEvalSnapshot {
        status: status_label(fallback.status),
        geometry_hash: fallback.geometry_hash,
        failure_classification: fallback.failure_classification.map(class_label),
        warning_count: fallback.warnings.len(),
    };
    let fallback_warning_code = fallback
        .warnings
        .first()
        .map(|warning| warning.code.stable_code().to_string())
        .unwrap_or_default();

    let edge_hash_a = evaluate_chamfer_feature(
        &ChamferFeatureOp {
            feature_id: "feature.chamfer.selection".to_string(),
            source_feature_id: "feature.base".to_string(),
            distance_param: "chamfer_distance_mm".to_string(),
            edge_refs: vec!["edge.001".to_string()],
            allow_fallback: false,
        },
        &chamfer_params(1.5),
        &source_context,
    )
    .expect("edge selection A should evaluate")
    .geometry_hash;
    let edge_hash_b = evaluate_chamfer_feature(
        &ChamferFeatureOp {
            feature_id: "feature.chamfer.selection".to_string(),
            source_feature_id: "feature.base".to_string(),
            distance_param: "chamfer_distance_mm".to_string(),
            edge_refs: vec!["edge.002".to_string()],
            allow_fallback: false,
        },
        &chamfer_params(1.5),
        &source_context,
    )
    .expect("edge selection B should evaluate")
    .geometry_hash;
    let edge_selection_affects_hash = edge_hash_a != edge_hash_b;

    let invalid_distance_error =
        evaluate_chamfer_feature(&parsed, &chamfer_params(0.0), &source_context)
            .expect_err("zero chamfer distance should fail")
            .to_string();

    let deterministic_signature = parity_signature(
        &chamfer_node_snapshot,
        &graph_topo_order,
        &applied_snapshot,
        &fallback_snapshot,
        &fallback_warning_code,
        edge_selection_affects_hash,
        &invalid_distance_error,
    );

    ChamferFeatureGraphParityManifest {
        manifest_version: 1,
        issue_id: PARITY_CHAMFER_FEATURE_GRAPH_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        chamfer_node_snapshot,
        graph_topo_order,
        applied_snapshot,
        fallback_snapshot,
        fallback_warning_code,
        edge_selection_affects_hash,
        invalid_distance_error,
        deterministic_signature,
        parity_contracts: vec![
            "chamfer.v2 feature nodes round-trip deterministically in feature graph serialization"
                .to_string(),
            "chamfer edge refs are canonicalized (sorted + deduplicated) on node conversion"
                .to_string(),
            "planar-safe chamfer path applies deterministically for safe distances".to_string(),
            "chamfer fallback emits deterministic TOPOLOGY_RISK diagnostics".to_string(),
            "chamfer fallback warning code is CAD-WARN-CHAMFER-FAILED".to_string(),
        ],
    }
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

fn class_label(classification: FinishingFailureClass) -> String {
    classification.code().to_string()
}

fn parity_signature(
    chamfer_node_snapshot: &ChamferNodeSnapshot,
    graph_topo_order: &[String],
    applied_snapshot: &ChamferEvalSnapshot,
    fallback_snapshot: &ChamferEvalSnapshot,
    fallback_warning_code: &str,
    edge_selection_affects_hash: bool,
    invalid_distance_error: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            chamfer_node_snapshot,
            graph_topo_order,
            applied_snapshot,
            fallback_snapshot,
            fallback_warning_code,
            edge_selection_affects_hash,
            invalid_distance_error,
        ))
        .expect("serialize chamfer feature graph parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        PARITY_CHAMFER_FEATURE_GRAPH_ISSUE_ID, build_chamfer_feature_graph_parity_manifest,
    };
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
    fn build_manifest_tracks_chamfer_feature_graph_contracts() {
        let manifest =
            build_chamfer_feature_graph_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_CHAMFER_FEATURE_GRAPH_ISSUE_ID);
        assert_eq!(manifest.chamfer_node_snapshot.operation_key, "chamfer.v2");
        assert_eq!(manifest.chamfer_node_snapshot.edge_refs.len(), 2);
        assert_eq!(manifest.graph_topo_order[0], "feature.base");
        assert_eq!(manifest.applied_snapshot.status, "applied");
        assert_eq!(manifest.fallback_snapshot.status, "fallback_kept_source");
        assert_eq!(manifest.fallback_warning_code, "CAD-WARN-CHAMFER-FAILED");
        assert!(manifest.edge_selection_affects_hash);
        assert!(
            manifest
                .invalid_distance_error
                .contains("must be finite and > 0")
        );
    }
}
