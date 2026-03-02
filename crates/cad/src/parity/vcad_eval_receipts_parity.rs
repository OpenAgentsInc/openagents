use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::eval::{DeterministicRebuildReceipt, evaluate_feature_graph_deterministic};
use crate::feature_graph::{FeatureGraph, FeatureNode};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_VCAD_EVAL_RECEIPTS_ISSUE_ID: &str = "VCAD-PARITY-037";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VcadEvalReceiptsParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub baseline_snapshot: EvalReceiptSnapshot,
    pub dependency_snapshot: EvalReceiptSnapshot,
    pub deterministic_replay_match: bool,
    pub timing_contract_match: bool,
    pub missing_dependency_error: String,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalReceiptSnapshot {
    pub ordered_feature_ids: Vec<String>,
    pub rebuild_hash: String,
    pub feature_count: usize,
    pub total_ms: f64,
    pub tessellate_ms: f64,
    pub clash_ms: f64,
    pub assembly_ms: f64,
    pub parse_ms: Option<f64>,
    pub serialize_ms: Option<f64>,
    pub node_timings: Vec<EvalNodeTimingSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalNodeTimingSnapshot {
    pub feature_id: String,
    pub op: String,
    pub eval_ms: f64,
    pub mesh_ms: f64,
}

pub fn build_vcad_eval_receipts_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> VcadEvalReceiptsParityManifest {
    let baseline_graph = FeatureGraph {
        nodes: vec![
            node(
                "feature.base",
                "primitive.box.v1",
                &[],
                &[
                    ("depth_param", "depth_mm"),
                    ("height_param", "height_mm"),
                    ("width_param", "width_mm"),
                ],
            ),
            node(
                "feature.hole",
                "cut.hole.v1",
                &["feature.base"],
                &[
                    ("depth_param", "hole_depth_mm"),
                    ("radius_param", "hole_radius_mm"),
                ],
            ),
            node(
                "feature.fillet_marker",
                "fillet.placeholder.v1",
                &["feature.hole"],
                &[("kind", "fillet"), ("radius_param", "fillet_radius_mm")],
            ),
        ],
    };
    let baseline_receipt = evaluate_feature_graph_deterministic(&baseline_graph)
        .expect("baseline eval receipt sample should evaluate")
        .receipt();
    let baseline_snapshot = snapshot_from_receipt(&baseline_receipt);

    let dependency_graph = FeatureGraph {
        nodes: vec![
            node(
                "feature.root",
                "primitive.cylinder.v1",
                &[],
                &[("r", "root_radius_mm")],
            ),
            node(
                "feature.shell",
                "shell.v1",
                &["feature.root"],
                &[("thickness_param", "shell_thickness_mm")],
            ),
            node(
                "feature.pattern",
                "linear.pattern.v1",
                &["feature.shell"],
                &[
                    ("count_param", "pattern_count"),
                    ("spacing_param", "pattern_spacing_mm"),
                ],
            ),
            node(
                "feature.tag",
                "fillet.placeholder.v1",
                &["feature.root"],
                &[("kind", "audit")],
            ),
        ],
    };
    let dependency_receipt = evaluate_feature_graph_deterministic(&dependency_graph)
        .expect("dependency-heavy eval receipt sample should evaluate")
        .receipt();
    let dependency_snapshot = snapshot_from_receipt(&dependency_receipt);

    let replay_baseline = evaluate_feature_graph_deterministic(&baseline_graph)
        .expect("baseline replay should evaluate")
        .receipt();
    let replay_dependency = evaluate_feature_graph_deterministic(&dependency_graph)
        .expect("dependency replay should evaluate")
        .receipt();
    let deterministic_replay_match =
        baseline_receipt == replay_baseline && dependency_receipt == replay_dependency;

    let timing_contract_match =
        timing_contract_matches(&baseline_receipt) && timing_contract_matches(&dependency_receipt);

    let missing_dependency_error = evaluate_feature_graph_deterministic(&FeatureGraph {
        nodes: vec![node(
            "feature.broken",
            "fillet.placeholder.v1",
            &["feature.missing"],
            &[("kind", "broken")],
        )],
    })
    .expect_err("missing dependency graph should fail")
    .to_string();

    let deterministic_signature = parity_signature(
        &baseline_snapshot,
        &dependency_snapshot,
        deterministic_replay_match,
        timing_contract_match,
        &missing_dependency_error,
    );

    VcadEvalReceiptsParityManifest {
        manifest_version: 1,
        issue_id: PARITY_VCAD_EVAL_RECEIPTS_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        baseline_snapshot,
        dependency_snapshot,
        deterministic_replay_match,
        timing_contract_match,
        missing_dependency_error,
        deterministic_signature,
        parity_contracts: vec![
            "eval receipts expose vcad-eval timing envelope fields (total/tessellate/clash/assembly/parse/serialize)"
                .to_string(),
            "native deterministic lane keeps parse_ms and serialize_ms unset, matching vcad native behavior"
                .to_string(),
            "per-node timing snapshots are keyed by feature id and include op/eval_ms/mesh_ms"
                .to_string(),
            "timing totals are self-consistent: total_ms = sum(node eval_ms + mesh_ms) + clash_ms + assembly_ms"
                .to_string(),
            "identical feature-graph inputs replay byte-identical eval receipts across runs"
                .to_string(),
        ],
    }
}

fn snapshot_from_receipt(receipt: &DeterministicRebuildReceipt) -> EvalReceiptSnapshot {
    let timing = &receipt.vcad_eval_timing;
    let node_timings = timing
        .nodes
        .iter()
        .map(|(feature_id, timing)| EvalNodeTimingSnapshot {
            feature_id: feature_id.clone(),
            op: timing.op.clone(),
            eval_ms: timing.eval_ms,
            mesh_ms: timing.mesh_ms,
        })
        .collect();

    EvalReceiptSnapshot {
        ordered_feature_ids: receipt.ordered_feature_ids.clone(),
        rebuild_hash: receipt.rebuild_hash.clone(),
        feature_count: receipt.feature_count,
        total_ms: timing.total_ms,
        tessellate_ms: timing.tessellate_ms,
        clash_ms: timing.clash_ms,
        assembly_ms: timing.assembly_ms,
        parse_ms: timing.parse_ms,
        serialize_ms: timing.serialize_ms,
        node_timings,
    }
}

fn timing_contract_matches(receipt: &DeterministicRebuildReceipt) -> bool {
    let timing = &receipt.vcad_eval_timing;
    if timing.parse_ms.is_some() || timing.serialize_ms.is_some() {
        return false;
    }

    let node_eval_total = timing
        .nodes
        .values()
        .fold(0.0, |sum, entry| sum + entry.eval_ms + entry.mesh_ms);
    let node_mesh_total = timing
        .nodes
        .values()
        .fold(0.0, |sum, entry| sum + entry.mesh_ms);

    approx_eq(timing.tessellate_ms, round_ms(node_mesh_total))
        && approx_eq(
            timing.total_ms,
            round_ms(node_eval_total + timing.clash_ms + timing.assembly_ms),
        )
        && timing
            .nodes
            .values()
            .all(|entry| !entry.op.trim().is_empty())
}

fn node(
    id: &str,
    operation_key: &str,
    depends_on: &[&str],
    params: &[(&str, &str)],
) -> FeatureNode {
    FeatureNode {
        id: id.to_string(),
        name: id.to_string(),
        operation_key: operation_key.to_string(),
        depends_on: depends_on
            .iter()
            .map(|dependency| (*dependency).to_string())
            .collect(),
        params: params
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn parity_signature(
    baseline_snapshot: &EvalReceiptSnapshot,
    dependency_snapshot: &EvalReceiptSnapshot,
    deterministic_replay_match: bool,
    timing_contract_match: bool,
    missing_dependency_error: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            baseline_snapshot,
            dependency_snapshot,
            deterministic_replay_match,
            timing_contract_match,
            missing_dependency_error,
        ))
        .expect("serialize vcad-eval receipt parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn round_ms(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9
}

#[cfg(test)]
mod tests {
    use super::{PARITY_VCAD_EVAL_RECEIPTS_ISSUE_ID, build_vcad_eval_receipts_parity_manifest};
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
    fn build_manifest_tracks_vcad_eval_receipt_contracts() {
        let manifest =
            build_vcad_eval_receipts_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_VCAD_EVAL_RECEIPTS_ISSUE_ID);
        assert!(manifest.deterministic_replay_match);
        assert!(manifest.timing_contract_match);
        assert_eq!(manifest.baseline_snapshot.parse_ms, None);
        assert_eq!(manifest.baseline_snapshot.serialize_ms, None);
        assert!(
            manifest
                .missing_dependency_error
                .contains("missing dependency hash")
        );
    }
}
