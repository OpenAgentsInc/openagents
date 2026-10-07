use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::TassadarWorkloadClass;

const TASSADAR_PLANNER_POLICY_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_PLANNER_LANGUAGE_COMPUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_planner_language_compute_policy_report.json";

/// Machine-legible publication status for the planner policy lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerPolicyPublicationStatus {
    /// Landed as a public research-only planner policy contract.
    Implemented,
}

/// Stable route families compared by the planner policy surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerRouteFamily {
    /// Stay in ordinary language-only reasoning and response generation.
    LanguageOnly,
    /// Delegate into the internal benchmark-gated exact-compute lane.
    InternalExactCompute,
    /// Delegate to an explicit external sandbox or tool loop.
    ExternalTool,
}

impl TassadarPlannerRouteFamily {
    /// Returns the stable route-family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LanguageOnly => "language_only",
            Self::InternalExactCompute => "internal_exact_compute",
            Self::ExternalTool => "external_tool",
        }
    }
}

/// Stable scored signals used by the planner policy report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerPolicySignal {
    ExpectedCorrectness,
    EstimatedCost,
    EvidenceBurden,
    RefusalRisk,
    WorkloadFit,
}

impl TassadarPlannerPolicySignal {
    /// Returns the stable signal label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExpectedCorrectness => "expected_correctness",
            Self::EstimatedCost => "estimated_cost",
            Self::EvidenceBurden => "evidence_burden",
            Self::RefusalRisk => "refusal_risk",
            Self::WorkloadFit => "workload_fit",
        }
    }
}

/// One weighted signal in the planner policy publication.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerPolicySignalWeight {
    /// Stable signal identifier.
    pub signal: TassadarPlannerPolicySignal,
    /// Relative contribution in basis points.
    pub weight_bps: u16,
    /// Whether lower values are better for this signal.
    pub prefer_lower: bool,
    /// Plain-language note for the signal.
    pub note: String,
}

/// Public model-facing publication for planner-native language-vs-compute policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerLanguageComputePolicyPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Publication status.
    pub status: TassadarPlannerPolicyPublicationStatus,
    /// Claim class for this surface.
    pub claim_class: String,
    /// Route families compared by the policy.
    pub route_families: Vec<TassadarPlannerRouteFamily>,
    /// Scored signals and their relative weights.
    pub signal_weights: Vec<TassadarPlannerPolicySignalWeight>,
    /// Workload classes currently covered by the benchmark-bound policy report.
    pub benchmarked_workload_classes: Vec<TassadarWorkloadClass>,
    /// Repo surfaces expected to consume the policy.
    pub target_surfaces: Vec<String>,
    /// Stable artifact refs that validate the publication.
    pub validation_refs: Vec<String>,
    /// Plain-language support boundaries.
    pub support_boundaries: Vec<String>,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarPlannerLanguageComputePolicyPublication {
    fn new() -> Self {
        let mut route_families = vec![
            TassadarPlannerRouteFamily::LanguageOnly,
            TassadarPlannerRouteFamily::InternalExactCompute,
            TassadarPlannerRouteFamily::ExternalTool,
        ];
        route_families.sort_by_key(|family| family.as_str());
        let mut benchmarked_workload_classes = vec![
            TassadarWorkloadClass::ArithmeticMicroprogram,
            TassadarWorkloadClass::MemoryLookupMicroprogram,
            TassadarWorkloadClass::BranchHeavyKernel,
            TassadarWorkloadClass::MemoryHeavyKernel,
            TassadarWorkloadClass::LongLoopKernel,
            TassadarWorkloadClass::SudokuClass,
        ];
        benchmarked_workload_classes.sort_by_key(|class| class.as_str());
        let mut publication = Self {
            schema_version: TASSADAR_PLANNER_POLICY_SCHEMA_VERSION,
            publication_id: String::from("tassadar.planner_language_compute_policy.publication.v1"),
            status: TassadarPlannerPolicyPublicationStatus::Implemented,
            claim_class: String::from("routing_surface_research_only_architecture"),
            route_families,
            signal_weights: vec![
                TassadarPlannerPolicySignalWeight {
                    signal: TassadarPlannerPolicySignal::ExpectedCorrectness,
                    weight_bps: 3_600,
                    prefer_lower: false,
                    note: String::from(
                        "policy should rank lanes by expected correctness before cost, but without treating local runtime success as authority or settlement closure",
                    ),
                },
                TassadarPlannerPolicySignalWeight {
                    signal: TassadarPlannerPolicySignal::EstimatedCost,
                    weight_bps: 1_500,
                    prefer_lower: true,
                    note: String::from(
                        "cost matters once correctness remains adequate; cheap language-only routes should win when exact compute is not justified",
                    ),
                },
                TassadarPlannerPolicySignalWeight {
                    signal: TassadarPlannerPolicySignal::EvidenceBurden,
                    weight_bps: 1_800,
                    prefer_lower: true,
                    note: String::from(
                        "routes that require heavier evidence should stay explicit so the planner can trade off stronger receipts against higher burden",
                    ),
                },
                TassadarPlannerPolicySignalWeight {
                    signal: TassadarPlannerPolicySignal::RefusalRisk,
                    weight_bps: 1_300,
                    prefer_lower: true,
                    note: String::from(
                        "the planner should avoid routes that are likely to end in explicit executor or tool refusal when another lane is a better fit",
                    ),
                },
                TassadarPlannerPolicySignalWeight {
                    signal: TassadarPlannerPolicySignal::WorkloadFit,
                    weight_bps: 1_800,
                    prefer_lower: false,
                    note: String::from(
                        "workload-family fit remains explicit so the planner does not over-read one lane's wins as general closure",
                    ),
                },
            ],
            benchmarked_workload_classes,
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-router"),
                String::from("crates/psionic-provider"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_PLANNER_LANGUAGE_COMPUTE_POLICY_REPORT_REF),
                String::from(
                    "fixtures/tassadar/reports/tassadar_workload_capability_frontier_report.json",
                ),
                String::from(
                    "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json",
                ),
            ],
            support_boundaries: vec![
                String::from(
                    "this publication defines a benchmark-bound planner policy vocabulary over language-only, internal exact-compute, and external-tool routes; it does not promote any lane or collapse routing into authority closure",
                ),
                String::from(
                    "signal weights are a current public policy prior for benchmarked hybrid cases, not a proof that the same ordering is globally optimal for every task or mount",
                ),
                String::from(
                    "internal exact-compute, language-only, and external-tool routes remain distinct receipt and refusal surfaces; publication here does not widen served capability or market posture",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_planner_language_compute_policy_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical planner language-vs-compute policy publication.
#[must_use]
pub fn tassadar_planner_language_compute_policy_publication()
-> TassadarPlannerLanguageComputePolicyPublication {
    TassadarPlannerLanguageComputePolicyPublication::new()
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
        TassadarPlannerPolicyPublicationStatus, TassadarPlannerPolicySignal,
        TassadarPlannerRouteFamily, tassadar_planner_language_compute_policy_publication,
    };
    use crate::TassadarWorkloadClass;

    #[test]
    fn planner_language_compute_policy_publication_is_machine_legible() {
        let publication = tassadar_planner_language_compute_policy_publication();

        assert_eq!(
            publication.status,
            TassadarPlannerPolicyPublicationStatus::Implemented
        );
        assert_eq!(publication.route_families.len(), 3);
        assert!(
            publication
                .route_families
                .contains(&TassadarPlannerRouteFamily::LanguageOnly)
        );
        assert!(
            publication
                .route_families
                .contains(&TassadarPlannerRouteFamily::InternalExactCompute)
        );
        assert!(
            publication
                .route_families
                .contains(&TassadarPlannerRouteFamily::ExternalTool)
        );
        assert!(
            publication
                .benchmarked_workload_classes
                .contains(&TassadarWorkloadClass::LongLoopKernel)
        );
        assert!(publication.signal_weights.iter().any(|weight| {
            weight.signal == TassadarPlannerPolicySignal::EvidenceBurden
                && weight.prefer_lower
                && weight.weight_bps > 0
        }));
        assert!(publication.validation_refs.iter().any(|reference| {
            reference.ends_with("tassadar_planner_language_compute_policy_report.json")
        }));
    }
}
