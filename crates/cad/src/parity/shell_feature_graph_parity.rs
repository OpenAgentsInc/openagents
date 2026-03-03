use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::feature_graph::{FeatureGraph, FeatureNode};
use crate::finishing_ops::{
    FinishingContext, FinishingFailureClass, FinishingStatus, SHELL_OPERATION_KEY, ShellFeatureOp,
    evaluate_shell_feature,
};
use crate::params::{ParameterStore, ScalarUnit, ScalarValue};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_SHELL_FEATURE_GRAPH_ISSUE_ID: &str = "VCAD-PARITY-029";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShellFeatureGraphParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub shell_node_snapshot: ShellNodeSnapshot,
    pub graph_topo_order: Vec<String>,
    pub applied_snapshot: ShellEvalSnapshot,
    pub fallback_snapshot: ShellEvalSnapshot,
    pub empty_remove_faces_supported: bool,
    pub remove_face_selection_affects_hash: bool,
    pub invalid_thickness_error: String,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellNodeSnapshot {
    pub operation_key: String,
    pub depends_on: Vec<String>,
    pub thickness_param: String,
    pub remove_face_refs: Vec<String>,
    pub allow_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellEvalSnapshot {
    pub status: String,
    pub geometry_hash: String,
    pub failure_classification: Option<String>,
    pub warning_count: usize,
}

pub fn build_shell_feature_graph_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> ShellFeatureGraphParityManifest {
    let source_context = FinishingContext {
        source_geometry_hash: "shell-source-geometry-hash".to_string(),
        source_min_thickness_mm: Some(20.0),
        source_volume_mm3: Some(2_000_000.0),
    };

    let op = ShellFeatureOp {
        feature_id: "feature.shell.primary".to_string(),
        source_feature_id: "feature.base".to_string(),
        thickness_param: "shell_thickness_mm".to_string(),
        remove_face_refs: vec![
            "face.003".to_string(),
            "face.001".to_string(),
            "face.001".to_string(),
        ],
        allow_fallback: false,
    };
    let node = op.to_feature_node().expect("shell node should build");
    let parsed = ShellFeatureOp::from_feature_node(&node).expect("shell node should parse");

    let shell_node_snapshot = ShellNodeSnapshot {
        operation_key: node.operation_key.clone(),
        depends_on: node.depends_on.clone(),
        thickness_param: parsed.thickness_param.clone(),
        remove_face_refs: parsed.remove_face_refs.clone(),
        allow_fallback: parsed.allow_fallback,
    };

    let graph = FeatureGraph {
        nodes: vec![
            FeatureNode {
                id: "feature.shell.primary".to_string(),
                name: "Shell".to_string(),
                operation_key: SHELL_OPERATION_KEY.to_string(),
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
        .expect("shell feature graph should topo-sort");

    let applied = evaluate_shell_feature(&parsed, &shell_params(1.0), &source_context)
        .expect("safe shell should apply");
    let applied_snapshot = ShellEvalSnapshot {
        status: status_label(applied.status),
        geometry_hash: applied.geometry_hash,
        failure_classification: applied.failure_classification.map(class_label),
        warning_count: applied.warnings.len(),
    };

    let fallback_op = ShellFeatureOp {
        feature_id: "feature.shell.fallback".to_string(),
        source_feature_id: "feature.base".to_string(),
        thickness_param: "shell_thickness_mm".to_string(),
        remove_face_refs: vec!["face.010".to_string()],
        allow_fallback: true,
    };
    let fallback = evaluate_shell_feature(&fallback_op, &shell_params(10.0), &source_context)
        .expect("risky shell should fallback when enabled");
    let fallback_snapshot = ShellEvalSnapshot {
        status: status_label(fallback.status),
        geometry_hash: fallback.geometry_hash,
        failure_classification: fallback.failure_classification.map(class_label),
        warning_count: fallback.warnings.len(),
    };

    let empty_remove_faces_supported = evaluate_shell_feature(
        &ShellFeatureOp {
            feature_id: "feature.shell.closed".to_string(),
            source_feature_id: "feature.base".to_string(),
            thickness_param: "shell_thickness_mm".to_string(),
            remove_face_refs: Vec::new(),
            allow_fallback: false,
        },
        &shell_params(1.0),
        &source_context,
    )
    .is_ok();

    let selection_hash_a = evaluate_shell_feature(
        &ShellFeatureOp {
            feature_id: "feature.shell.selection".to_string(),
            source_feature_id: "feature.base".to_string(),
            thickness_param: "shell_thickness_mm".to_string(),
            remove_face_refs: vec!["face.001".to_string()],
            allow_fallback: false,
        },
        &shell_params(1.0),
        &source_context,
    )
    .expect("selection A should evaluate")
    .geometry_hash;
    let selection_hash_b = evaluate_shell_feature(
        &ShellFeatureOp {
            feature_id: "feature.shell.selection".to_string(),
            source_feature_id: "feature.base".to_string(),
            thickness_param: "shell_thickness_mm".to_string(),
            remove_face_refs: vec!["face.002".to_string()],
            allow_fallback: false,
        },
        &shell_params(1.0),
        &source_context,
    )
    .expect("selection B should evaluate")
    .geometry_hash;
    let remove_face_selection_affects_hash = selection_hash_a != selection_hash_b;

    let invalid_thickness_error =
        evaluate_shell_feature(&parsed, &shell_params(0.0), &source_context)
            .expect_err("zero thickness should fail")
            .to_string();

    let deterministic_signature = parity_signature(
        &shell_node_snapshot,
        &graph_topo_order,
        &applied_snapshot,
        &fallback_snapshot,
        empty_remove_faces_supported,
        remove_face_selection_affects_hash,
        &invalid_thickness_error,
    );

    ShellFeatureGraphParityManifest {
        manifest_version: 1,
        issue_id: PARITY_SHELL_FEATURE_GRAPH_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        shell_node_snapshot,
        graph_topo_order,
        applied_snapshot,
        fallback_snapshot,
        empty_remove_faces_supported,
        remove_face_selection_affects_hash,
        invalid_thickness_error,
        deterministic_signature,
        parity_contracts: vec![
            "shell.v1 feature nodes round-trip deterministically in feature graph serialization"
                .to_string(),
            "shell remove-face refs are canonicalized (sorted + deduplicated) on node conversion"
                .to_string(),
            "closed-shell flow supports empty remove-face sets for production shell parity"
                .to_string(),
            "shell geometry hash includes remove-face selection signature for deterministic replay"
                .to_string(),
            "shell risk fallback maps to ZERO_THICKNESS_RISK with deterministic warning payload"
                .to_string(),
        ],
    }
}

fn shell_params(thickness_mm: f64) -> ParameterStore {
    let mut params = ParameterStore::default();
    params
        .set(
            "shell_thickness_mm",
            ScalarValue {
                value: thickness_mm,
                unit: ScalarUnit::Millimeter,
            },
        )
        .expect("shell thickness should set");
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
    shell_node_snapshot: &ShellNodeSnapshot,
    graph_topo_order: &[String],
    applied_snapshot: &ShellEvalSnapshot,
    fallback_snapshot: &ShellEvalSnapshot,
    empty_remove_faces_supported: bool,
    remove_face_selection_affects_hash: bool,
    invalid_thickness_error: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            shell_node_snapshot,
            graph_topo_order,
            applied_snapshot,
            fallback_snapshot,
            empty_remove_faces_supported,
            remove_face_selection_affects_hash,
            invalid_thickness_error,
        ))
        .expect("serialize shell feature graph parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_SHELL_FEATURE_GRAPH_ISSUE_ID, build_shell_feature_graph_parity_manifest};
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
    fn build_manifest_tracks_shell_feature_graph_contracts() {
        let manifest =
            build_shell_feature_graph_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_SHELL_FEATURE_GRAPH_ISSUE_ID);
        assert_eq!(manifest.shell_node_snapshot.operation_key, "shell.v1");
        assert_eq!(manifest.shell_node_snapshot.remove_face_refs.len(), 2);
        assert_eq!(manifest.graph_topo_order[0], "feature.base");
        assert_eq!(manifest.applied_snapshot.status, "applied");
        assert_eq!(manifest.fallback_snapshot.status, "fallback_kept_source");
        assert!(manifest.empty_remove_faces_supported);
        assert!(manifest.remove_face_selection_affects_hash);
        assert!(
            manifest
                .invalid_thickness_error
                .contains("must be finite and > 0")
        );
    }
}
