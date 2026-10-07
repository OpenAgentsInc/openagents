use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{TassadarPlannerRouteFamily, TassadarWorkloadClass};

const TASSADAR_NEGATIVE_INVOCATION_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_NEGATIVE_INVOCATION_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_negative_invocation_v1/negative_invocation_evidence_bundle.json";
pub const TASSADAR_NEGATIVE_INVOCATION_ROUTE_AUDIT_REF: &str =
    "fixtures/tassadar/reports/tassadar_negative_invocation_route_audit.json";
pub const TASSADAR_NEGATIVE_INVOCATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_negative_invocation_report.json";

/// Publication status for the negative-invocation planner-learning surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNegativeInvocationPublicationStatus {
    Implemented,
}

/// Explicit penalty kind used when training planners not to over-call compute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNegativeInvocationPenaltyKind {
    UnnecessaryInternalInvocation,
    FallbackChurn,
    EvidenceQualityRegression,
    RefusalWhenBetterLaneExists,
}

impl TassadarNegativeInvocationPenaltyKind {
    /// Returns the stable penalty label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnnecessaryInternalInvocation => "unnecessary_internal_invocation",
            Self::FallbackChurn => "fallback_churn",
            Self::EvidenceQualityRegression => "evidence_quality_regression",
            Self::RefusalWhenBetterLaneExists => "refusal_when_better_lane_exists",
        }
    }
}

/// One weighted planner penalty used by the negative-invocation lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNegativeInvocationPenaltyWeight {
    /// Stable penalty kind.
    pub penalty_kind: TassadarNegativeInvocationPenaltyKind,
    /// Relative contribution in basis points.
    pub weight_bps: u16,
    /// Plain-language note.
    pub note: String,
}

/// Public publication for negative-invocation planner learning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNegativeInvocationPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Publication status.
    pub status: TassadarNegativeInvocationPublicationStatus,
    /// Claim class for this lane.
    pub claim_class: String,
    /// Route families compared by the lane.
    pub route_families: Vec<TassadarPlannerRouteFamily>,
    /// Weighted penalties used by the training bundle and audits.
    pub penalty_weights: Vec<TassadarNegativeInvocationPenaltyWeight>,
    /// Workload classes covered by the seeded evidence bundle.
    pub benchmarked_workload_classes: Vec<TassadarWorkloadClass>,
    /// Repo surfaces expected to consume the lane.
    pub target_surfaces: Vec<String>,
    /// Stable refs grounding the publication.
    pub validation_refs: Vec<String>,
    /// Plain-language support boundaries.
    pub support_boundaries: Vec<String>,
    /// Stable publication digest.
    pub publication_digest: String,
}

impl TassadarNegativeInvocationPublication {
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
            TassadarWorkloadClass::MemoryHeavyKernel,
            TassadarWorkloadClass::LongLoopKernel,
            TassadarWorkloadClass::SudokuClass,
            TassadarWorkloadClass::BranchHeavyKernel,
        ];
        benchmarked_workload_classes.sort_by_key(|class| class.as_str());
        let mut penalty_weights = vec![
            TassadarNegativeInvocationPenaltyWeight {
                penalty_kind: TassadarNegativeInvocationPenaltyKind::UnnecessaryInternalInvocation,
                weight_bps: 3_400,
                note: String::from(
                    "planners should pay an explicit penalty when the internal executor is invoked even though language-only or external delegation is the honest better lane",
                ),
            },
            TassadarNegativeInvocationPenaltyWeight {
                penalty_kind: TassadarNegativeInvocationPenaltyKind::FallbackChurn,
                weight_bps: 2_600,
                note: String::from(
                    "fallback churn should stay visible because repeated internal retries are not a success metric and often destroy cost or latency posture",
                ),
            },
            TassadarNegativeInvocationPenaltyWeight {
                penalty_kind: TassadarNegativeInvocationPenaltyKind::EvidenceQualityRegression,
                weight_bps: 2_100,
                note: String::from(
                    "a route that worsens evidence quality should be treated as a planner failure even when it still returns a locally correct answer",
                ),
            },
            TassadarNegativeInvocationPenaltyWeight {
                penalty_kind: TassadarNegativeInvocationPenaltyKind::RefusalWhenBetterLaneExists,
                weight_bps: 1_900,
                note: String::from(
                    "the planner should be penalized when it selects a route likely to refuse even though a better admissible lane already exists",
                ),
            },
        ];
        penalty_weights.sort_by_key(|weight| weight.penalty_kind.as_str());
        let mut publication = Self {
            schema_version: TASSADAR_NEGATIVE_INVOCATION_SCHEMA_VERSION,
            publication_id: String::from("tassadar.negative_invocation.publication.v1"),
            status: TassadarNegativeInvocationPublicationStatus::Implemented,
            claim_class: String::from("research_only_architecture_routing_surface"),
            route_families,
            penalty_weights,
            benchmarked_workload_classes,
            target_surfaces: vec![
                String::from("crates/psionic-models"),
                String::from("crates/psionic-train"),
                String::from("crates/psionic-router"),
                String::from("crates/psionic-eval"),
            ],
            validation_refs: vec![
                String::from(TASSADAR_NEGATIVE_INVOCATION_BUNDLE_REF),
                String::from(TASSADAR_NEGATIVE_INVOCATION_ROUTE_AUDIT_REF),
                String::from(TASSADAR_NEGATIVE_INVOCATION_REPORT_REF),
                String::from(
                    "fixtures/tassadar/reports/tassadar_planner_language_compute_policy_report.json",
                ),
            ],
            support_boundaries: vec![
                String::from(
                    "this publication defines a benchmark-bound negative-invocation vocabulary for planner learning and route auditing; it does not widen served capability or imply accepted-outcome authority",
                ),
                String::from(
                    "higher executor utilization is not treated as a success metric; unnecessary internal calls, fallback churn, evidence regressions, and route-quality failures stay explicit",
                ),
                String::from(
                    "the lane compares language-only, internal exact-compute, and external-tool routes on matched seeded cases; it does not claim global planner optimality",
                ),
            ],
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_negative_invocation_publication|",
            &publication,
        );
        publication
    }
}

/// One counterfactual route outcome used by negative-invocation training.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNegativeInvocationRouteOutcome {
    /// Compared route family.
    pub route_family: TassadarPlannerRouteFamily,
    /// Expected correctness prior in basis points.
    pub expected_correctness_bps: u32,
    /// Estimated cost in milliunits.
    pub estimated_cost_milliunits: u32,
    /// Estimated latency in milliseconds.
    pub estimated_latency_millis: u32,
    /// Evidence quality in basis points.
    pub evidence_quality_bps: u32,
    /// Count of expected fallback churn events.
    pub fallback_churn_count: u32,
    /// Whether this route would refuse while a better lane exists.
    pub would_refuse_when_better_lane_exists: bool,
    /// Plain-language note.
    pub note: String,
}

/// One seeded training case in the negative-invocation bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNegativeInvocationTrainingCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Workload class attached to the case.
    pub workload_class: TassadarWorkloadClass,
    /// Baseline route family before the negative-invocation penalties.
    pub baseline_route_family: TassadarPlannerRouteFamily,
    /// Preferred route family after explicit negative-invocation supervision.
    pub preferred_route_family: TassadarPlannerRouteFamily,
    /// Ordered route outcomes for this case.
    pub route_outcomes: Vec<TassadarNegativeInvocationRouteOutcome>,
    /// Whether the baseline route needlessly invoked the internal executor.
    pub unnecessary_internal_invocation: bool,
    /// Evidence-quality delta between preferred and baseline routes.
    pub evidence_quality_regression_bps: i32,
    /// Plain-language note.
    pub note: String,
}

/// Train-side evidence bundle for negative-invocation planner learning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNegativeInvocationEvidenceBundle {
    /// Shared publication grounding the bundle.
    pub publication: TassadarNegativeInvocationPublication,
    /// Ordered training cases.
    pub cases: Vec<TassadarNegativeInvocationTrainingCase>,
    /// Share of baseline routes that needlessly invoked the internal executor.
    pub unnecessary_internal_invocation_rate_bps: u32,
    /// Total baseline fallback churn across the bundle.
    pub baseline_fallback_churn_total: u32,
    /// Total preferred fallback churn across the bundle.
    pub preferred_fallback_churn_total: u32,
    /// Average baseline evidence quality across the bundle.
    pub baseline_average_evidence_quality_bps: u32,
    /// Average preferred evidence quality across the bundle.
    pub preferred_average_evidence_quality_bps: u32,
    /// Plain-language summary.
    pub summary: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

/// Returns the canonical negative-invocation publication.
#[must_use]
pub fn tassadar_negative_invocation_publication() -> TassadarNegativeInvocationPublication {
    TassadarNegativeInvocationPublication::new()
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
        TassadarNegativeInvocationPenaltyKind, TassadarNegativeInvocationPublicationStatus,
        tassadar_negative_invocation_publication,
    };
    use crate::{TassadarPlannerRouteFamily, TassadarWorkloadClass};

    #[test]
    fn negative_invocation_publication_is_machine_legible() {
        let publication = tassadar_negative_invocation_publication();

        assert_eq!(
            publication.status,
            TassadarNegativeInvocationPublicationStatus::Implemented
        );
        assert_eq!(publication.route_families.len(), 3);
        assert!(
            publication
                .route_families
                .contains(&TassadarPlannerRouteFamily::InternalExactCompute)
        );
        assert!(
            publication
                .benchmarked_workload_classes
                .contains(&TassadarWorkloadClass::LongLoopKernel)
        );
        assert!(publication.penalty_weights.iter().any(|weight| {
            weight.penalty_kind
                == TassadarNegativeInvocationPenaltyKind::UnnecessaryInternalInvocation
                && weight.weight_bps > 3_000
        }));
        assert_eq!(publication.validation_refs.len(), 4);
    }
}
