use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_runtime::TassadarClaimClass;
use psionic_train::TassadarExecutorPromotionGateReport;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarExecutorAttentionPromotionRunBundle, TassadarLearnedLongHorizonGuardStatus,
    TassadarLearnedLongHorizonPolicyReport, build_tassadar_learned_horizon_policy_report,
};

const TASSADAR_PROMOTION_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/sudoku_v0_promotion_v3/promotion_bundle.json";
const TASSADAR_PROMOTION_GATE_REPORT_REF: &str =
    "fixtures/tassadar/runs/sudoku_v0_promotion_v3/promotion_gate_report.json";
const TASSADAR_LEARNED_HORIZON_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_learned_horizon_policy_report.json";
const TASSADAR_EXACTNESS_REFUSAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_exactness_refusal_report.json";

/// Canonical machine-readable promotion-policy report file.
pub const TASSADAR_PROMOTION_POLICY_REPORT_FILE: &str = "tassadar_promotion_policy_report.json";
/// Canonical output directory for the promotion-policy report.
pub const TASSADAR_PROMOTION_POLICY_OUTPUT_DIR: &str = "fixtures/tassadar/reports";
/// Stable schema version for the promotion-policy report.
pub const TASSADAR_PROMOTION_POLICY_REPORT_SCHEMA_VERSION: u16 = 1;
/// Canonical checker command for the promotion-policy report.
pub const TASSADAR_PROMOTION_POLICY_CHECKER_COMMAND: &str =
    "scripts/check-tassadar-promotion-policy.sh";

/// One required promotion gate between research and served publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPromotionChecklistGateKind {
    BenchmarkEvidence,
    RefusalBehavior,
    RouteContractCompatibility,
}

/// One typed promotion gate with concrete evidence and status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPromotionChecklistGate {
    /// Stable gate kind.
    pub gate_kind: TassadarPromotionChecklistGateKind,
    /// Whether the gate passed.
    pub passed: bool,
    /// Human-readable detail for the current status.
    pub detail: String,
    /// Repo-relative evidence refs that justify the gate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
}

/// Current promotion status for the researched lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPromotionPolicyStatus {
    ResearchOnly,
    PromotionBlocked,
    PromotionEligible,
}

impl TassadarPromotionPolicyStatus {
    /// Returns whether the checklist currently allows served publication.
    #[must_use]
    pub const fn allows_served_publication(self) -> bool {
        matches!(self, Self::PromotionEligible)
    }
}

/// Public promotion-policy report for moving a research lane toward served publication.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPromotionPolicyReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable policy identifier.
    pub policy_id: String,
    /// Stable candidate run identifier.
    pub candidate_run_id: String,
    /// Stable candidate model identifier.
    pub candidate_model_id: String,
    /// Current claim class for the candidate lane.
    pub candidate_claim_class: TassadarClaimClass,
    /// Served product the candidate would need to satisfy before promotion.
    pub target_product_id: String,
    /// Served route product the candidate would need to satisfy before promotion.
    pub target_route_product_id: String,
    /// Current promotion status.
    pub status: TassadarPromotionPolicyStatus,
    /// Checklist gates in stable order.
    pub checklist: Vec<TassadarPromotionChecklistGate>,
    /// Repo-facing checker commands that should stay green before promotion.
    pub validation_commands: Vec<String>,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarPromotionPolicyReport {
    /// Returns the currently failing gate kinds in stable order.
    #[must_use]
    pub fn failed_gates(&self) -> Vec<TassadarPromotionChecklistGateKind> {
        self.checklist
            .iter()
            .filter(|gate| !gate.passed)
            .map(|gate| gate.gate_kind)
            .collect()
    }
}

/// Promotion-policy artifact errors.
#[derive(Debug, Error)]
pub enum TassadarPromotionPolicyError {
    /// Failed to read one committed artifact.
    #[error("failed to read `{artifact_kind}` from `{path}`: {error}")]
    Read {
        artifact_kind: String,
        path: String,
        error: std::io::Error,
    },
    /// Failed to decode one committed artifact.
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Decode {
        artifact_kind: String,
        path: String,
        error: serde_json::Error,
    },
    /// Failed to build the learned horizon policy report.
    #[error("failed to build learned horizon policy report: {0}")]
    LearnedHorizonPolicy(#[from] crate::TassadarAcceptanceError),
    /// Failed to create the output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the promotion-policy report.
    #[error("failed to write promotion policy report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

/// Returns the canonical path for the promotion-policy report.
#[must_use]
pub fn tassadar_promotion_policy_report_path() -> PathBuf {
    repo_root()
        .join(TASSADAR_PROMOTION_POLICY_OUTPUT_DIR)
        .join(TASSADAR_PROMOTION_POLICY_REPORT_FILE)
}

/// Builds the current promotion-policy report from the committed research artifacts.
pub fn build_tassadar_promotion_policy_report()
-> Result<TassadarPromotionPolicyReport, TassadarPromotionPolicyError> {
    let promotion_bundle: TassadarExecutorAttentionPromotionRunBundle =
        read_repo_json(TASSADAR_PROMOTION_BUNDLE_REF, "learned promotion bundle")?;
    let promotion_gate: TassadarExecutorPromotionGateReport = read_repo_json(
        TASSADAR_PROMOTION_GATE_REPORT_REF,
        "learned promotion gate report",
    )?;
    let learned_horizon_policy = build_tassadar_learned_horizon_policy_report()?;

    let checklist = vec![
        benchmark_evidence_gate(&promotion_gate),
        refusal_behavior_gate(&learned_horizon_policy),
        route_contract_gate(&promotion_bundle),
    ];
    let status = if checklist.iter().all(|gate| gate.passed) {
        TassadarPromotionPolicyStatus::PromotionEligible
    } else {
        TassadarPromotionPolicyStatus::PromotionBlocked
    };

    let mut report = TassadarPromotionPolicyReport {
        schema_version: TASSADAR_PROMOTION_POLICY_REPORT_SCHEMA_VERSION,
        policy_id: String::from("tassadar.research_to_served_promotion_policy.v1"),
        candidate_run_id: promotion_bundle.run_id.clone(),
        candidate_model_id: promotion_bundle.model_id.clone(),
        candidate_claim_class: promotion_bundle.claim_class,
        target_product_id: String::from("psionic.executor_trace"),
        target_route_product_id: String::from("psionic.planner_executor_route"),
        status,
        checklist,
        validation_commands: vec![
            String::from("scripts/check-tassadar-4x4-promotion-gate.sh"),
            String::from("scripts/check-tassadar-acceptance.sh"),
            String::from(TASSADAR_PROMOTION_POLICY_CHECKER_COMMAND),
        ],
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(b"psionic_tassadar_promotion_policy_report|", &report);
    Ok(report)
}

/// Writes the current promotion-policy report to one path.
pub fn write_tassadar_promotion_policy_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarPromotionPolicyReport, TassadarPromotionPolicyError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| TassadarPromotionPolicyError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let report = build_tassadar_promotion_policy_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("promotion policy report should serialize");
    fs::write(output_path, bytes).map_err(|error| TassadarPromotionPolicyError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(report)
}

fn benchmark_evidence_gate(
    promotion_gate: &TassadarExecutorPromotionGateReport,
) -> TassadarPromotionChecklistGate {
    TassadarPromotionChecklistGate {
        gate_kind: TassadarPromotionChecklistGateKind::BenchmarkEvidence,
        passed: promotion_gate.passed,
        detail: if promotion_gate.passed {
            String::from(
                "the bounded learned 4x4 promotion gate is green on its declared exactness bar, but the result remains bounded evidence until the other promotion gates clear",
            )
        } else {
            String::from(
                "the bounded learned 4x4 promotion gate is still red, so benchmark evidence is insufficient for served promotion",
            )
        },
        evidence_refs: vec![
            String::from(TASSADAR_PROMOTION_BUNDLE_REF),
            String::from(TASSADAR_PROMOTION_GATE_REPORT_REF),
        ],
    }
}

fn refusal_behavior_gate(
    learned_horizon_policy: &TassadarLearnedLongHorizonPolicyReport,
) -> TassadarPromotionChecklistGate {
    let passed = matches!(
        learned_horizon_policy.guard_status,
        TassadarLearnedLongHorizonGuardStatus::ExplicitRefusalPolicy
    ) && learned_horizon_policy.refusal_kind.is_some()
        && !learned_horizon_policy.refusal_reasons.is_empty()
        && !learned_horizon_policy.learned_article_class_bypass_allowed;
    TassadarPromotionChecklistGate {
        gate_kind: TassadarPromotionChecklistGateKind::RefusalBehavior,
        passed,
        detail: if passed {
            String::from(
                "the learned long-horizon policy keeps article-class promotion bounded with typed refusal reasons and no bypass around the declared guard",
            )
        } else {
            String::from(
                "the learned lane does not yet have a stable refusal policy that can bound served claims safely",
            )
        },
        evidence_refs: vec![
            String::from(TASSADAR_LEARNED_HORIZON_POLICY_REPORT_REF),
            String::from(TASSADAR_EXACTNESS_REFUSAL_REPORT_REF),
        ],
    }
}

fn route_contract_gate(
    promotion_bundle: &TassadarExecutorAttentionPromotionRunBundle,
) -> TassadarPromotionChecklistGate {
    TassadarPromotionChecklistGate {
        gate_kind: TassadarPromotionChecklistGateKind::RouteContractCompatibility,
        passed: false,
        detail: format!(
            "no served `psionic.executor_trace` capability publication or `psionic.planner_executor_route` descriptor is attached to candidate model `{}` yet, so the research lane cannot be treated as promoted",
            promotion_bundle.model_id
        ),
        evidence_refs: vec![
            String::from(TASSADAR_PROMOTION_BUNDLE_REF),
            String::from(TASSADAR_PROMOTION_GATE_REPORT_REF),
        ],
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn read_repo_json<T>(
    repo_relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarPromotionPolicyError>
where
    T: DeserializeOwned,
{
    let path = repo_root().join(repo_relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarPromotionPolicyError::Read {
        artifact_kind: String::from(artifact_kind),
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarPromotionPolicyError::Decode {
        artifact_kind: String::from(artifact_kind),
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).expect("promotion policy report should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_PROMOTION_POLICY_CHECKER_COMMAND, TassadarPromotionChecklistGateKind,
        TassadarPromotionPolicyError, TassadarPromotionPolicyStatus,
        build_tassadar_promotion_policy_report, repo_root, tassadar_promotion_policy_report_path,
        write_tassadar_promotion_policy_report,
    };
    use tempfile::tempdir;

    #[test]
    fn promotion_policy_report_blocks_promotion_until_refusal_and_route_gates_clear()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_promotion_policy_report()?;
        assert_eq!(
            report.status,
            TassadarPromotionPolicyStatus::PromotionBlocked
        );
        assert_eq!(
            report.failed_gates(),
            vec![
                TassadarPromotionChecklistGateKind::RefusalBehavior,
                TassadarPromotionChecklistGateKind::RouteContractCompatibility,
            ]
        );
        assert!(
            report
                .validation_commands
                .contains(&String::from(TASSADAR_PROMOTION_POLICY_CHECKER_COMMAND))
        );
        Ok(())
    }

    #[test]
    fn write_tassadar_promotion_policy_report_persists_current_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let tempdir = tempdir()?;
        let output_path = tempdir.path().join("tassadar_promotion_policy_report.json");
        let written = write_tassadar_promotion_policy_report(&output_path)?;
        let decoded = serde_json::from_slice::<super::TassadarPromotionPolicyReport>(
            &std::fs::read(&output_path)?,
        )?;
        assert_eq!(decoded, written);
        Ok(())
    }

    #[test]
    fn promotion_policy_report_matches_committed_truth() -> Result<(), Box<dyn std::error::Error>> {
        let expected = build_tassadar_promotion_policy_report()?;
        let committed_path = tassadar_promotion_policy_report_path();
        let committed_bytes =
            std::fs::read(&committed_path).map_err(|error| TassadarPromotionPolicyError::Read {
                artifact_kind: String::from("promotion policy report"),
                path: committed_path.display().to_string(),
                error,
            })?;
        let committed =
            serde_json::from_slice::<super::TassadarPromotionPolicyReport>(&committed_bytes)
                .map_err(|error| TassadarPromotionPolicyError::Decode {
                    artifact_kind: String::from("promotion policy report"),
                    path: committed_path.display().to_string(),
                    error,
                })?;
        assert_eq!(
            committed,
            expected,
            "committed promotion-policy report drifted; rerun `cargo run -p psionic-research --example tassadar_promotion_policy_report` from {}",
            tassadar_promotion_policy_report_path()
                .strip_prefix(repo_root())?
                .display()
        );
        Ok(())
    }
}
