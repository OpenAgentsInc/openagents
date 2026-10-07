use psionic_ir::{
    TassadarMixedTrajectory, TassadarMixedTrajectoryEntry, TassadarMixedTrajectoryEntryKind,
    TassadarMixedTrajectoryHandoffKind, TassadarMixedTrajectoryLaneKind,
    TassadarMixedTrajectoryReceiptBoundary, TassadarMixedTrajectoryReceiptBoundaryKind,
    TassadarMixedTrajectoryVerifierEventKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_MIXED_TRAJECTORY_ABI_VERSION: &str = "psionic.tassadar.mixed_trajectory.v1";
pub const TASSADAR_MIXED_TRAJECTORY_CONTRACT_REF: &str =
    "dataset://openagents/tassadar/mixed_trajectory_suite";
pub const TASSADAR_MIXED_TRAJECTORY_SUITE_REF: &str =
    "fixtures/tassadar/runs/tassadar_mixed_trajectory_suite_v1/mixed_trajectory_suite.json";
pub const TASSADAR_MIXED_TRAJECTORY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_mixed_trajectory_report.json";

/// Hybrid workload family covered by the mixed trajectory suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMixedTrajectoryWorkloadFamily {
    ArticleHybridCompute,
    VerifierAttachedSearch,
    ExternalToolLongLoop,
}

impl TassadarMixedTrajectoryWorkloadFamily {
    /// Returns the stable workload-family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArticleHybridCompute => "article_hybrid_compute",
            Self::VerifierAttachedSearch => "verifier_attached_search",
            Self::ExternalToolLongLoop => "external_tool_long_loop",
        }
    }
}

/// One seeded mixed trajectory case in the public contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedTrajectoryCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Compared workload family.
    pub workload_family: TassadarMixedTrajectoryWorkloadFamily,
    /// Required lanes for the case.
    pub required_lanes: Vec<TassadarMixedTrajectoryLaneKind>,
    /// Expected lane handoffs for the case.
    pub expected_handoffs: Vec<TassadarMixedTrajectoryHandoffKind>,
    /// Expected receipt boundaries for the case.
    pub expected_receipt_boundaries: Vec<TassadarMixedTrajectoryReceiptBoundaryKind>,
    /// Mixed trajectory artifact for the case.
    pub trajectory: TassadarMixedTrajectory,
    /// Expected final outputs.
    pub expected_final_outputs: Vec<i32>,
    /// Plain-language note.
    pub note: String,
}

/// Public contract for the mixed trajectory suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedTrajectoryContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable contract ref.
    pub contract_ref: String,
    /// Stable version label.
    pub version: String,
    /// Ordered seeded cases.
    pub cases: Vec<TassadarMixedTrajectoryCase>,
    /// Evaluation axes surfaced by the suite.
    pub evaluation_axes: Vec<String>,
    /// Train-side suite artifact ref.
    pub suite_ref: String,
    /// Eval report ref.
    pub report_ref: String,
    /// Stable digest over the contract.
    pub contract_digest: String,
}

impl TassadarMixedTrajectoryContract {
    fn new() -> Self {
        let mut contract = Self {
            abi_version: String::from(TASSADAR_MIXED_TRAJECTORY_ABI_VERSION),
            contract_ref: String::from(TASSADAR_MIXED_TRAJECTORY_CONTRACT_REF),
            version: String::from("2026.03.18"),
            cases: seeded_cases(),
            evaluation_axes: vec![
                String::from("schema_roundtrip_ok"),
                String::from("lane_handoff_correct"),
                String::from("receipt_boundary_count"),
                String::from("trajectory_to_outcome_parity"),
            ],
            suite_ref: String::from(TASSADAR_MIXED_TRAJECTORY_SUITE_REF),
            report_ref: String::from(TASSADAR_MIXED_TRAJECTORY_REPORT_REF),
            contract_digest: String::new(),
        };
        contract
            .validate()
            .expect("mixed trajectory contract should validate");
        contract.contract_digest =
            stable_digest(b"psionic_tassadar_mixed_trajectory_contract|", &contract);
        contract
    }

    /// Validates the public contract.
    pub fn validate(&self) -> Result<(), TassadarMixedTrajectoryContractError> {
        if self.abi_version != TASSADAR_MIXED_TRAJECTORY_ABI_VERSION {
            return Err(
                TassadarMixedTrajectoryContractError::UnsupportedAbiVersion {
                    abi_version: self.abi_version.clone(),
                },
            );
        }
        if self.contract_ref.trim().is_empty() {
            return Err(TassadarMixedTrajectoryContractError::MissingContractRef);
        }
        if self.cases.is_empty() {
            return Err(TassadarMixedTrajectoryContractError::MissingCases);
        }
        for case in &self.cases {
            if case.required_lanes.is_empty() {
                return Err(
                    TassadarMixedTrajectoryContractError::CaseMissingRequiredLanes {
                        case_id: case.case_id.clone(),
                    },
                );
            }
            if case.expected_receipt_boundaries.is_empty() {
                return Err(
                    TassadarMixedTrajectoryContractError::CaseMissingReceiptBoundaries {
                        case_id: case.case_id.clone(),
                    },
                );
            }
            case.trajectory.validate().map_err(|error| {
                TassadarMixedTrajectoryContractError::InvalidTrajectory {
                    case_id: case.case_id.clone(),
                    error,
                }
            })?;
            if case.expected_final_outputs != case.trajectory.final_outputs {
                return Err(
                    TassadarMixedTrajectoryContractError::CaseFinalOutputMismatch {
                        case_id: case.case_id.clone(),
                    },
                );
            }
        }
        Ok(())
    }
}

/// Train-side case report for the mixed trajectory suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedTrajectoryTrainingCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Compared workload family.
    pub workload_family: TassadarMixedTrajectoryWorkloadFamily,
    /// Ordered lane sequence surfaced by the trajectory.
    pub lane_sequence: Vec<TassadarMixedTrajectoryLaneKind>,
    /// Number of language spans.
    pub language_span_count: u32,
    /// Number of exact-compute spans.
    pub exact_compute_span_count: u32,
    /// Number of verifier spans.
    pub verifier_span_count: u32,
    /// Number of external-tool spans.
    pub external_tool_span_count: u32,
    /// Number of receipt boundaries.
    pub receipt_boundary_count: u32,
    /// Number of explicit lane handoffs.
    pub handoff_count: u32,
    /// Whether schema roundtrip stayed exact.
    pub schema_roundtrip_ok: bool,
    /// Whether lane handoffs stayed correct.
    pub lane_handoff_correct: bool,
    /// Whether trajectory final outputs matched the replayed outcome.
    pub trajectory_to_outcome_parity: bool,
    /// Plain-language note.
    pub note: String,
}

/// Train-side suite artifact for the mixed trajectory family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMixedTrajectoryTrainingSuite {
    /// Public contract for the suite.
    pub contract: TassadarMixedTrajectoryContract,
    /// Case-level training reports.
    pub case_reports: Vec<TassadarMixedTrajectoryTrainingCase>,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the suite.
    pub report_digest: String,
}

/// Returns the canonical mixed trajectory contract.
#[must_use]
pub fn tassadar_mixed_trajectory_contract() -> TassadarMixedTrajectoryContract {
    TassadarMixedTrajectoryContract::new()
}

/// Mixed trajectory contract validation failure.
#[derive(Debug, Error)]
pub enum TassadarMixedTrajectoryContractError {
    #[error("unsupported mixed trajectory ABI version `{abi_version}`")]
    UnsupportedAbiVersion { abi_version: String },
    #[error("mixed trajectory contract is missing a contract ref")]
    MissingContractRef,
    #[error("mixed trajectory contract is missing cases")]
    MissingCases,
    #[error("mixed trajectory case `{case_id}` is missing required lanes")]
    CaseMissingRequiredLanes { case_id: String },
    #[error("mixed trajectory case `{case_id}` is missing receipt boundaries")]
    CaseMissingReceiptBoundaries { case_id: String },
    #[error("mixed trajectory case `{case_id}` has invalid trajectory: {error}")]
    InvalidTrajectory {
        case_id: String,
        error: psionic_ir::TassadarMixedTrajectoryError,
    },
    #[error("mixed trajectory case `{case_id}` has mismatched final outputs")]
    CaseFinalOutputMismatch { case_id: String },
}

fn seeded_cases() -> Vec<TassadarMixedTrajectoryCase> {
    vec![
        TassadarMixedTrajectoryCase {
            case_id: String::from("article_hybrid_compute"),
            workload_family: TassadarMixedTrajectoryWorkloadFamily::ArticleHybridCompute,
            required_lanes: vec![
                TassadarMixedTrajectoryLaneKind::LanguageReasoning,
                TassadarMixedTrajectoryLaneKind::InternalExactCompute,
            ],
            expected_handoffs: vec![
                TassadarMixedTrajectoryHandoffKind::LanguageToInternalCompute,
                TassadarMixedTrajectoryHandoffKind::InternalComputeToLanguage,
            ],
            expected_receipt_boundaries: vec![
                TassadarMixedTrajectoryReceiptBoundaryKind::PlannerPolicyReceipt,
                TassadarMixedTrajectoryReceiptBoundaryKind::ExecutorEvidenceBundle,
                TassadarMixedTrajectoryReceiptBoundaryKind::FinalOutcomeReceipt,
            ],
            trajectory: article_hybrid_trajectory(),
            expected_final_outputs: vec![42],
            note: String::from(
                "article-class hybrid case keeps planner intent, internal exact compute, executor receipt, and final language answer as distinct typed trajectory entries",
            ),
        },
        TassadarMixedTrajectoryCase {
            case_id: String::from("verifier_attached_search"),
            workload_family: TassadarMixedTrajectoryWorkloadFamily::VerifierAttachedSearch,
            required_lanes: vec![
                TassadarMixedTrajectoryLaneKind::LanguageReasoning,
                TassadarMixedTrajectoryLaneKind::InternalExactCompute,
                TassadarMixedTrajectoryLaneKind::VerifierSearch,
            ],
            expected_handoffs: vec![
                TassadarMixedTrajectoryHandoffKind::LanguageToInternalCompute,
                TassadarMixedTrajectoryHandoffKind::InternalComputeToVerifier,
                TassadarMixedTrajectoryHandoffKind::VerifierToLanguage,
            ],
            expected_receipt_boundaries: vec![
                TassadarMixedTrajectoryReceiptBoundaryKind::PlannerPolicyReceipt,
                TassadarMixedTrajectoryReceiptBoundaryKind::ExecutorEvidenceBundle,
                TassadarMixedTrajectoryReceiptBoundaryKind::VerifierCertificate,
                TassadarMixedTrajectoryReceiptBoundaryKind::FinalOutcomeReceipt,
            ],
            trajectory: verifier_search_trajectory(),
            expected_final_outputs: vec![4, 3, 2, 1],
            note: String::from(
                "verifier-attached search case keeps planner, compute, verifier, and final response boundaries explicit instead of flattening search into one opaque span",
            ),
        },
        TassadarMixedTrajectoryCase {
            case_id: String::from("external_tool_long_loop"),
            workload_family: TassadarMixedTrajectoryWorkloadFamily::ExternalToolLongLoop,
            required_lanes: vec![
                TassadarMixedTrajectoryLaneKind::LanguageReasoning,
                TassadarMixedTrajectoryLaneKind::ExternalTool,
            ],
            expected_handoffs: vec![
                TassadarMixedTrajectoryHandoffKind::LanguageToExternalTool,
                TassadarMixedTrajectoryHandoffKind::ExternalToolToLanguage,
            ],
            expected_receipt_boundaries: vec![
                TassadarMixedTrajectoryReceiptBoundaryKind::PlannerPolicyReceipt,
                TassadarMixedTrajectoryReceiptBoundaryKind::SandboxExecutionReceipt,
                TassadarMixedTrajectoryReceiptBoundaryKind::FinalOutcomeReceipt,
            ],
            trajectory: external_tool_trajectory(),
            expected_final_outputs: vec![7],
            note: String::from(
                "external-tool fallback case keeps explicit planner-to-tool delegation and sandbox receipt boundaries separate from the surrounding language spans",
            ),
        },
    ]
}

fn article_hybrid_trajectory() -> TassadarMixedTrajectory {
    TassadarMixedTrajectory::new(
        "tassadar.mixed_trajectory.article_hybrid_compute.v1",
        "article_hybrid_compute",
        vec![
            language_entry(0, "need exact checksum before drafting the answer", None),
            receipt_entry(
                1,
                TassadarMixedTrajectoryLaneKind::LanguageReasoning,
                TassadarMixedTrajectoryReceiptBoundaryKind::PlannerPolicyReceipt,
                "fixtures/tassadar/reports/tassadar_planner_language_compute_policy_report.json",
                "planner selected the internal exact-compute lane",
            ),
            compute_entry(
                2,
                Some(TassadarMixedTrajectoryHandoffKind::LanguageToInternalCompute),
                "execute exact checksum kernel",
                24,
                vec![42],
            ),
            receipt_entry(
                3,
                TassadarMixedTrajectoryLaneKind::InternalExactCompute,
                TassadarMixedTrajectoryReceiptBoundaryKind::ExecutorEvidenceBundle,
                "fixtures/tassadar/reports/tassadar_article_hybrid_workflow_artifact.json",
                "executor receipt stays explicit between exact compute and language narration",
            ),
            language_entry(
                4,
                "executor returned checksum 42; continue with the article answer",
                Some(TassadarMixedTrajectoryHandoffKind::InternalComputeToLanguage),
            ),
            receipt_entry(
                5,
                TassadarMixedTrajectoryLaneKind::LanguageReasoning,
                TassadarMixedTrajectoryReceiptBoundaryKind::FinalOutcomeReceipt,
                "fixtures/tassadar/reports/tassadar_article_hybrid_workflow_artifact.json",
                "final response boundary stays explicit and does not imply accepted-outcome closure",
            ),
        ],
        vec![42],
        "mixed article workflow remains at execution-truth scope with explicit planner, executor, and response boundaries",
    )
    .expect("seeded article trajectory")
}

fn verifier_search_trajectory() -> TassadarMixedTrajectory {
    TassadarMixedTrajectory::new(
        "tassadar.mixed_trajectory.verifier_attached_search.v1",
        "verifier_attached_search",
        vec![
            language_entry(0, "check the candidate board exactly before answering", None),
            receipt_entry(
                1,
                TassadarMixedTrajectoryLaneKind::LanguageReasoning,
                TassadarMixedTrajectoryReceiptBoundaryKind::PlannerPolicyReceipt,
                "fixtures/tassadar/reports/tassadar_planner_language_compute_policy_report.json",
                "planner selected internal exact compute plus verifier follow-up",
            ),
            compute_entry(
                2,
                Some(TassadarMixedTrajectoryHandoffKind::LanguageToInternalCompute),
                "execute bounded candidate-check program",
                31,
                vec![4, 3, 2, 1],
            ),
            receipt_entry(
                3,
                TassadarMixedTrajectoryLaneKind::InternalExactCompute,
                TassadarMixedTrajectoryReceiptBoundaryKind::ExecutorEvidenceBundle,
                "fixtures/tassadar/reports/tassadar_exactness_refusal_report.json",
                "executor evidence stays separate from the later verifier certificate",
            ),
            verifier_entry(
                4,
                Some(TassadarMixedTrajectoryHandoffKind::InternalComputeToVerifier),
                TassadarMixedTrajectoryVerifierEventKind::Verify,
                "verifier confirms the candidate state and rejects one branch",
                4,
            ),
            receipt_entry(
                5,
                TassadarMixedTrajectoryLaneKind::VerifierSearch,
                TassadarMixedTrajectoryReceiptBoundaryKind::VerifierCertificate,
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                "verifier certificate closes the bounded search event without flattening it into the compute trace",
            ),
            language_entry(
                6,
                "verifier accepted the candidate state; answer with the checked result",
                Some(TassadarMixedTrajectoryHandoffKind::VerifierToLanguage),
            ),
            receipt_entry(
                7,
                TassadarMixedTrajectoryLaneKind::LanguageReasoning,
                TassadarMixedTrajectoryReceiptBoundaryKind::FinalOutcomeReceipt,
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                "final answer boundary remains explicit after the verifier-assisted path",
            ),
        ],
        vec![4, 3, 2, 1],
        "verifier-attached mixed trajectories keep compute, verifier, and answer spans separate with explicit receipt boundaries",
    )
    .expect("seeded verifier trajectory")
}

fn external_tool_trajectory() -> TassadarMixedTrajectory {
    TassadarMixedTrajectory::new(
        "tassadar.mixed_trajectory.external_tool_long_loop.v1",
        "external_tool_long_loop",
        vec![
            language_entry(0, "long loop exceeds the current internal exact route; delegate safely", None),
            receipt_entry(
                1,
                TassadarMixedTrajectoryLaneKind::LanguageReasoning,
                TassadarMixedTrajectoryReceiptBoundaryKind::PlannerPolicyReceipt,
                "fixtures/tassadar/reports/tassadar_planner_language_compute_policy_report.json",
                "planner selected external tool fallback for the long-horizon case",
            ),
            external_tool_entry(
                2,
                Some(TassadarMixedTrajectoryHandoffKind::LanguageToExternalTool),
                "sandbox_loop_runner",
                "run long-loop kernel in bounded sandbox",
                96,
                vec![7],
            ),
            receipt_entry(
                3,
                TassadarMixedTrajectoryLaneKind::ExternalTool,
                TassadarMixedTrajectoryReceiptBoundaryKind::SandboxExecutionReceipt,
                "psionic.sandbox_execution",
                "sandbox receipt keeps the delegated execution boundary explicit",
            ),
            language_entry(
                4,
                "sandbox returned result 7; continue with the response",
                Some(TassadarMixedTrajectoryHandoffKind::ExternalToolToLanguage),
            ),
            receipt_entry(
                5,
                TassadarMixedTrajectoryLaneKind::LanguageReasoning,
                TassadarMixedTrajectoryReceiptBoundaryKind::FinalOutcomeReceipt,
                "psionic.sandbox_execution",
                "final answer boundary remains explicit after external delegation",
            ),
        ],
        vec![7],
        "external-tool mixed trajectories keep delegated execution and its receipt separate from surrounding language spans",
    )
    .expect("seeded external trajectory")
}

fn language_entry(
    entry_index: u32,
    text: &str,
    handoff_from_previous: Option<TassadarMixedTrajectoryHandoffKind>,
) -> TassadarMixedTrajectoryEntry {
    TassadarMixedTrajectoryEntry {
        entry_id: format!("entry-{entry_index}"),
        entry_index,
        entry_kind: TassadarMixedTrajectoryEntryKind::LanguageSpan,
        lane_kind: TassadarMixedTrajectoryLaneKind::LanguageReasoning,
        handoff_from_previous,
        content_summary: String::from("language_reasoning"),
        language_text: Some(String::from(text)),
        token_count: Some(18),
        step_count: None,
        verifier_event_kind: None,
        tool_name: None,
        receipt_boundary: None,
        span_outputs: Vec::new(),
    }
}

fn compute_entry(
    entry_index: u32,
    handoff_from_previous: Option<TassadarMixedTrajectoryHandoffKind>,
    content_summary: &str,
    step_count: u32,
    span_outputs: Vec<i32>,
) -> TassadarMixedTrajectoryEntry {
    TassadarMixedTrajectoryEntry {
        entry_id: format!("entry-{entry_index}"),
        entry_index,
        entry_kind: TassadarMixedTrajectoryEntryKind::ExactComputeSpan,
        lane_kind: TassadarMixedTrajectoryLaneKind::InternalExactCompute,
        handoff_from_previous,
        content_summary: String::from(content_summary),
        language_text: None,
        token_count: None,
        step_count: Some(step_count),
        verifier_event_kind: None,
        tool_name: None,
        receipt_boundary: None,
        span_outputs,
    }
}

fn verifier_entry(
    entry_index: u32,
    handoff_from_previous: Option<TassadarMixedTrajectoryHandoffKind>,
    verifier_event_kind: TassadarMixedTrajectoryVerifierEventKind,
    content_summary: &str,
    step_count: u32,
) -> TassadarMixedTrajectoryEntry {
    TassadarMixedTrajectoryEntry {
        entry_id: format!("entry-{entry_index}"),
        entry_index,
        entry_kind: TassadarMixedTrajectoryEntryKind::VerifierEventSpan,
        lane_kind: TassadarMixedTrajectoryLaneKind::VerifierSearch,
        handoff_from_previous,
        content_summary: String::from(content_summary),
        language_text: None,
        token_count: None,
        step_count: Some(step_count),
        verifier_event_kind: Some(verifier_event_kind),
        tool_name: None,
        receipt_boundary: None,
        span_outputs: Vec::new(),
    }
}

fn external_tool_entry(
    entry_index: u32,
    handoff_from_previous: Option<TassadarMixedTrajectoryHandoffKind>,
    tool_name: &str,
    content_summary: &str,
    step_count: u32,
    span_outputs: Vec<i32>,
) -> TassadarMixedTrajectoryEntry {
    TassadarMixedTrajectoryEntry {
        entry_id: format!("entry-{entry_index}"),
        entry_index,
        entry_kind: TassadarMixedTrajectoryEntryKind::ExternalToolSpan,
        lane_kind: TassadarMixedTrajectoryLaneKind::ExternalTool,
        handoff_from_previous,
        content_summary: String::from(content_summary),
        language_text: None,
        token_count: None,
        step_count: Some(step_count),
        verifier_event_kind: None,
        tool_name: Some(String::from(tool_name)),
        receipt_boundary: None,
        span_outputs,
    }
}

fn receipt_entry(
    entry_index: u32,
    lane_kind: TassadarMixedTrajectoryLaneKind,
    boundary_kind: TassadarMixedTrajectoryReceiptBoundaryKind,
    receipt_ref: &str,
    note: &str,
) -> TassadarMixedTrajectoryEntry {
    TassadarMixedTrajectoryEntry {
        entry_id: format!("entry-{entry_index}"),
        entry_index,
        entry_kind: TassadarMixedTrajectoryEntryKind::ReceiptBoundary,
        lane_kind,
        handoff_from_previous: None,
        content_summary: String::from("receipt_boundary"),
        language_text: None,
        token_count: None,
        step_count: None,
        verifier_event_kind: None,
        tool_name: None,
        receipt_boundary: Some(TassadarMixedTrajectoryReceiptBoundary {
            boundary_kind,
            receipt_ref: String::from(receipt_ref),
            note: String::from(note),
        }),
        span_outputs: Vec::new(),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarMixedTrajectoryContractError, TassadarMixedTrajectoryReceiptBoundaryKind,
        TassadarMixedTrajectoryWorkloadFamily, tassadar_mixed_trajectory_contract,
    };

    #[test]
    fn mixed_trajectory_contract_is_machine_legible() {
        let contract = tassadar_mixed_trajectory_contract();

        assert_eq!(contract.cases.len(), 3);
        assert!(contract.cases.iter().any(|case| {
            case.workload_family == TassadarMixedTrajectoryWorkloadFamily::VerifierAttachedSearch
                && case
                    .expected_receipt_boundaries
                    .contains(&TassadarMixedTrajectoryReceiptBoundaryKind::VerifierCertificate)
        }));
        assert!(
            contract
                .evaluation_axes
                .contains(&String::from("schema_roundtrip_ok"))
        );
        contract.validate().expect("contract should validate");
    }

    #[test]
    fn mixed_trajectory_cases_keep_expected_outputs_and_receipts_explicit() {
        let contract = tassadar_mixed_trajectory_contract();
        for case in &contract.cases {
            assert_eq!(case.expected_final_outputs, case.trajectory.final_outputs);
            assert!(!case.expected_receipt_boundaries.is_empty());
        }

        let mut invalid = contract.clone();
        invalid.cases[0].expected_final_outputs = vec![0];
        let err = invalid.validate().expect_err("mismatch should fail");
        assert!(matches!(
            err,
            TassadarMixedTrajectoryContractError::CaseFinalOutputMismatch { .. }
        ));
    }
}
