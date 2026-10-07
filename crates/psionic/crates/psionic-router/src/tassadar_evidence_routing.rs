use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    TASSADAR_PLANNER_LANGUAGE_COMPUTE_POLICY_REPORT_REF, TassadarPlannerRouteFamily,
    TassadarWorkloadClass,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_EVIDENCE_CALIBRATED_ROUTING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_evidence_calibrated_routing_report.json";
const TASSADAR_MIXED_TRAJECTORY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_mixed_trajectory_report.json";
const TASSADAR_EXACTNESS_REFUSAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_exactness_refusal_report.json";
const LANGUAGE_ONLY_PRODUCT_ID: &str = "psionic.text_generation";
const EXTERNAL_TOOL_PRODUCT_ID: &str = "psionic.sandbox_execution";
const INTERNAL_EXECUTOR_PRODUCT_ID: &str = "psionic.planner_executor_route";

/// Explicit trust tier used when calibrating one route against mount policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEvidenceTrustTier {
    ResearchOnly,
    BenchmarkGated,
    ValidatorBacked,
    AcceptedOutcomeCompatible,
}

impl TassadarEvidenceTrustTier {
    /// Returns the stable trust-tier label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResearchOnly => "research_only",
            Self::BenchmarkGated => "benchmark_gated",
            Self::ValidatorBacked => "validator_backed",
            Self::AcceptedOutcomeCompatible => "accepted_outcome_compatible",
        }
    }
}

/// Whether one candidate route stayed usable after mount-policy checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEvidenceRouteAdmissibility {
    Admissible,
    RefusedByPolicy,
}

/// Typed reason one route cannot satisfy the current mount posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEvidenceRoutingRefusalReason {
    RouteFamilyDisallowed,
    ResearchOnlyDisallowed,
    TrustTierInsufficient,
    ValidatorRequired,
    AcceptedOutcomeEvidenceMissing,
    EvidenceBudgetExceeded,
    CostBudgetExceeded,
}

/// Explicit mount-scoped route constraints used during evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorldMountRoutePolicy {
    /// Stable mount identifier.
    pub mount_id: String,
    /// Stable mount class.
    pub mount_class: String,
    /// Lowest trust tier the mount will accept.
    pub required_trust_tier: TassadarEvidenceTrustTier,
    /// Whether accepted-outcome-compatible evidence is required.
    pub accepted_outcome_required: bool,
    /// Whether validator attachment is required.
    pub validator_required: bool,
    /// Whether research-only lanes may be selected.
    pub allow_research_only: bool,
    /// Maximum evidence burden accepted by the mount.
    pub max_evidence_burden_bps: u32,
    /// Maximum cost accepted by the mount.
    pub max_cost_milliunits: u32,
    /// Explicit allowed route families for this mount.
    pub allowed_route_families: Vec<TassadarPlannerRouteFamily>,
    /// Plain-language note.
    pub note: String,
}

/// One seeded route candidate evaluated under a mount policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEvidenceCalibratedRouteScore {
    /// Stable route family.
    pub route_family: TassadarPlannerRouteFamily,
    /// Product surface for the route.
    pub product_id: String,
    /// Expected correctness prior in basis points.
    pub expected_correctness_bps: u32,
    /// Estimated cost in milliunits.
    pub estimated_cost_milliunits: u32,
    /// Estimated evidence burden in basis points.
    pub evidence_burden_bps: u32,
    /// Workload fit in basis points.
    pub workload_fit_bps: u32,
    /// Trust tier currently published by the route.
    pub trust_tier: TassadarEvidenceTrustTier,
    /// Whether a validator artifact can be attached for this route.
    pub validator_attached: bool,
    /// Whether the route can support accepted-outcome-compatible evidence.
    pub accepted_outcome_ready: bool,
    /// Whether the route remains research-only.
    pub research_only: bool,
    /// Whether the route stayed admissible after explicit mount checks.
    pub admissibility: TassadarEvidenceRouteAdmissibility,
    /// Typed refusal reason when the route is disallowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarEvidenceRoutingRefusalReason>,
    /// Capability-only route score that ignores mount-policy constraints.
    pub capability_only_score: i32,
    /// Evidence-aware score used after mount-policy checks.
    pub evidence_calibrated_score: i32,
    /// Primary evidence surface referenced by the route.
    pub evidence_surface: String,
    /// Plain-language route note.
    pub note: String,
}

/// One mount-scoped hybrid routing case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMountScopedRouteEvaluation {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable task family.
    pub task_family: String,
    /// Workload class attached to the case.
    pub workload_class: TassadarWorkloadClass,
    /// Explicit mount policy evaluated for the case.
    pub mount_policy: TassadarWorldMountRoutePolicy,
    /// Capability-only route family that would win without mount-policy checks.
    pub capability_only_selected_route_family: TassadarPlannerRouteFamily,
    /// Route family selected after evidence-aware calibration.
    pub evidence_aware_selected_route_family: TassadarPlannerRouteFamily,
    /// Expected route family for the seeded case.
    pub expected_route_family: TassadarPlannerRouteFamily,
    /// Whether the evidence-aware route matched the seeded expectation.
    pub selection_matches_expected: bool,
    /// Whether the capability-only winner would violate the mount.
    pub capability_only_would_violate_mount: bool,
    /// Whether the evidence-aware winner satisfies the mount.
    pub evidence_aware_policy_compliant: bool,
    /// Whether the evidence-aware route avoided a capability-only misroute.
    pub misroute_avoided: bool,
    /// Whether the evidence-aware route attaches validator evidence.
    pub selected_route_validator_attached: bool,
    /// Whether the evidence-aware route is accepted-outcome-compatible.
    pub selected_route_accepted_outcome_ready: bool,
    /// Ordered candidate scores for the case.
    pub route_scores: Vec<TassadarEvidenceCalibratedRouteScore>,
    /// Plain-language case note.
    pub note: String,
}

/// Deterministic evidence-aware routing report over seeded mount-scoped cases.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEvidenceCalibratedRoutingReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Ordered seeded mount-scoped cases.
    pub evaluated_cases: Vec<TassadarMountScopedRouteEvaluation>,
    /// Stable refs grounding the report.
    pub generated_from_refs: Vec<String>,
    /// Accuracy against the seeded evidence-aware route labels.
    pub evidence_aware_selection_accuracy_bps: u32,
    /// Share of cases where the evidence-aware winner satisfied the mount.
    pub evidence_aware_policy_compliance_rate_bps: u32,
    /// Share of cases where the capability-only winner would violate the mount.
    pub capability_only_mount_violation_rate_bps: u32,
    /// Share of cases where evidence-aware routing avoided that misroute.
    pub misroute_avoidance_rate_bps: u32,
    /// Share of accepted-outcome-required cases selecting an accepted-outcome-ready route.
    pub accepted_outcome_ready_selection_rate_bps: u32,
    /// Share of validator-required cases selecting a validator-attached route.
    pub validator_requirement_satisfaction_rate_bps: u32,
    /// Average evidence burden of the evidence-aware selected routes.
    pub average_selected_evidence_burden_bps: u32,
    /// Average cost of the evidence-aware selected routes.
    pub average_selected_cost_milliunits: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// One-line summary.
    pub summary: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarEvidenceCalibratedRoutingReport {
    fn new(
        evaluated_cases: Vec<TassadarMountScopedRouteEvaluation>,
        generated_from_refs: Vec<String>,
    ) -> Self {
        let case_count = evaluated_cases.len() as u32;
        let accurate_count = evaluated_cases
            .iter()
            .filter(|case| case.selection_matches_expected)
            .count() as u32;
        let compliant_count = evaluated_cases
            .iter()
            .filter(|case| case.evidence_aware_policy_compliant)
            .count() as u32;
        let capability_only_violations = evaluated_cases
            .iter()
            .filter(|case| case.capability_only_would_violate_mount)
            .count() as u32;
        let misroutes_avoided = evaluated_cases
            .iter()
            .filter(|case| case.misroute_avoided)
            .count() as u32;
        let accepted_outcome_cases = evaluated_cases
            .iter()
            .filter(|case| case.mount_policy.accepted_outcome_required)
            .count() as u32;
        let accepted_outcome_ready = evaluated_cases
            .iter()
            .filter(|case| case.mount_policy.accepted_outcome_required)
            .filter(|case| case.selected_route_accepted_outcome_ready)
            .count() as u32;
        let validator_cases = evaluated_cases
            .iter()
            .filter(|case| case.mount_policy.validator_required)
            .count() as u32;
        let validator_ready = evaluated_cases
            .iter()
            .filter(|case| case.mount_policy.validator_required)
            .filter(|case| case.selected_route_validator_attached)
            .count() as u32;
        let selected_cost_total = evaluated_cases
            .iter()
            .map(|case| selected_route_score(case).estimated_cost_milliunits)
            .sum::<u32>();
        let selected_evidence_total = evaluated_cases
            .iter()
            .map(|case| selected_route_score(case).evidence_burden_bps)
            .sum::<u32>();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.evidence_calibrated_routing.report.v1"),
            evaluated_cases,
            generated_from_refs,
            evidence_aware_selection_accuracy_bps: ratio_bps(accurate_count, case_count),
            evidence_aware_policy_compliance_rate_bps: ratio_bps(compliant_count, case_count),
            capability_only_mount_violation_rate_bps: ratio_bps(
                capability_only_violations,
                case_count,
            ),
            misroute_avoidance_rate_bps: ratio_bps(misroutes_avoided, case_count),
            accepted_outcome_ready_selection_rate_bps: ratio_bps(
                accepted_outcome_ready,
                accepted_outcome_cases,
            ),
            validator_requirement_satisfaction_rate_bps: ratio_bps(
                validator_ready,
                validator_cases,
            ),
            average_selected_evidence_burden_bps: if case_count == 0 {
                0
            } else {
                selected_evidence_total / case_count
            },
            average_selected_cost_milliunits: if case_count == 0 {
                0
            } else {
                selected_cost_total / case_count
            },
            claim_boundary: String::from(
                "this report is a benchmark-bound routing surface over seeded mount-scoped cases. It keeps evidence burden, trust posture, validator attachment, and accepted-outcome compatibility explicit without treating route ranking as accepted-outcome authority, settlement truth, or served capability widening",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Evidence-aware routing covers {} mount-scoped cases with selection accuracy {} bps, policy compliance {} bps, capability-only mount violations {} bps, and misroute avoidance {} bps.",
            report.evaluated_cases.len(),
            report.evidence_aware_selection_accuracy_bps,
            report.evidence_aware_policy_compliance_rate_bps,
            report.capability_only_mount_violation_rate_bps,
            report.misroute_avoidance_rate_bps,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_evidence_calibrated_routing_report|",
            &report,
        );
        report
    }
}

/// Evidence-calibrated routing report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarEvidenceCalibratedRoutingReportError {
    /// Failed to create an output directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Failed to read one committed artifact.
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Failed to decode one committed artifact.
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed mount-scoped evidence-aware routing report.
pub fn build_tassadar_evidence_calibrated_routing_report()
-> Result<TassadarEvidenceCalibratedRoutingReport, TassadarEvidenceCalibratedRoutingReportError> {
    let evaluated_cases = seeded_cases()
        .iter()
        .map(build_case_evaluation)
        .collect::<Vec<_>>();
    let mut generated_from_refs = vec![
        String::from(TASSADAR_PLANNER_LANGUAGE_COMPUTE_POLICY_REPORT_REF),
        String::from(TASSADAR_MIXED_TRAJECTORY_REPORT_REF),
        String::from(TASSADAR_EXACTNESS_REFUSAL_REPORT_REF),
    ];
    generated_from_refs.sort();
    generated_from_refs.dedup();
    Ok(TassadarEvidenceCalibratedRoutingReport::new(
        evaluated_cases,
        generated_from_refs,
    ))
}

/// Returns the canonical absolute path for the committed evidence-aware routing report.
#[must_use]
pub fn tassadar_evidence_calibrated_routing_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EVIDENCE_CALIBRATED_ROUTING_REPORT_REF)
}

/// Writes the committed mount-scoped evidence-aware routing report.
pub fn write_tassadar_evidence_calibrated_routing_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarEvidenceCalibratedRoutingReport, TassadarEvidenceCalibratedRoutingReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarEvidenceCalibratedRoutingReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_evidence_calibrated_routing_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarEvidenceCalibratedRoutingReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[derive(Clone, Copy)]
struct SeededCase {
    case_id: &'static str,
    task_family: &'static str,
    workload_class: TassadarWorkloadClass,
    expected_route_family: TassadarPlannerRouteFamily,
    mount_policy: SeededMountPolicy,
    candidates: [SeededRouteCandidate; 3],
    note: &'static str,
}

#[derive(Clone, Copy)]
struct SeededMountPolicy {
    mount_id: &'static str,
    mount_class: &'static str,
    required_trust_tier: TassadarEvidenceTrustTier,
    accepted_outcome_required: bool,
    validator_required: bool,
    allow_research_only: bool,
    max_evidence_burden_bps: u32,
    max_cost_milliunits: u32,
    allowed_route_families: [TassadarPlannerRouteFamily; 3],
    note: &'static str,
}

#[derive(Clone, Copy)]
struct SeededRouteCandidate {
    route_family: TassadarPlannerRouteFamily,
    product_id: &'static str,
    expected_correctness_bps: u32,
    estimated_cost_milliunits: u32,
    evidence_burden_bps: u32,
    workload_fit_bps: u32,
    trust_tier: TassadarEvidenceTrustTier,
    validator_attached: bool,
    accepted_outcome_ready: bool,
    research_only: bool,
    evidence_surface: &'static str,
    note: &'static str,
}

fn seeded_cases() -> [SeededCase; 6] {
    [
        SeededCase {
            case_id: "language_summary_low_evidence_budget",
            task_family: "language_summary_low_evidence_budget",
            workload_class: TassadarWorkloadClass::ArithmeticMicroprogram,
            expected_route_family: TassadarPlannerRouteFamily::LanguageOnly,
            mount_policy: SeededMountPolicy {
                mount_id: "mount.article_summary.low_evidence.v1",
                mount_class: "explanatory_summary",
                required_trust_tier: TassadarEvidenceTrustTier::BenchmarkGated,
                accepted_outcome_required: false,
                validator_required: false,
                allow_research_only: false,
                max_evidence_burden_bps: 1_100,
                max_cost_milliunits: 1_600,
                allowed_route_families: [
                    TassadarPlannerRouteFamily::LanguageOnly,
                    TassadarPlannerRouteFamily::InternalExactCompute,
                    TassadarPlannerRouteFamily::ExternalTool,
                ],
                note: "cheap explanatory mount where evidence burden above the reporting budget should block heavier routes",
            },
            candidates: [
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::LanguageOnly,
                    product_id: LANGUAGE_ONLY_PRODUCT_ID,
                    expected_correctness_bps: 8_850,
                    estimated_cost_milliunits: 620,
                    evidence_burden_bps: 700,
                    workload_fit_bps: 9_400,
                    trust_tier: TassadarEvidenceTrustTier::BenchmarkGated,
                    validator_attached: false,
                    accepted_outcome_ready: false,
                    research_only: false,
                    evidence_surface: "planner_summary",
                    note: "language-only route is sufficient and cheap for explanatory output",
                },
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::InternalExactCompute,
                    product_id: INTERNAL_EXECUTOR_PRODUCT_ID,
                    expected_correctness_bps: 9_900,
                    estimated_cost_milliunits: 1_950,
                    evidence_burden_bps: 2_700,
                    workload_fit_bps: 9_200,
                    trust_tier: TassadarEvidenceTrustTier::BenchmarkGated,
                    validator_attached: true,
                    accepted_outcome_ready: true,
                    research_only: false,
                    evidence_surface: "executor_trace_and_proof_bundle",
                    note: "internal exact-compute route is accurate but too expensive and evidence-heavy for this mount",
                },
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::ExternalTool,
                    product_id: EXTERNAL_TOOL_PRODUCT_ID,
                    expected_correctness_bps: 9_650,
                    estimated_cost_milliunits: 4_100,
                    evidence_burden_bps: 3_400,
                    workload_fit_bps: 5_100,
                    trust_tier: TassadarEvidenceTrustTier::ValidatorBacked,
                    validator_attached: true,
                    accepted_outcome_ready: true,
                    research_only: false,
                    evidence_surface: "sandbox_execution_receipt",
                    note: "external tool route is overkill for this explanatory mount",
                },
            ],
            note: "capability-only scoring would over-call compute here, but the mount budget should keep the route in language",
        },
        SeededCase {
            case_id: "accepted_outcome_exact_patch",
            task_family: "accepted_outcome_exact_patch",
            workload_class: TassadarWorkloadClass::MemoryHeavyKernel,
            expected_route_family: TassadarPlannerRouteFamily::InternalExactCompute,
            mount_policy: SeededMountPolicy {
                mount_id: "mount.accepted_outcome.exact_patch.v1",
                mount_class: "accepted_outcome_exact_patch",
                required_trust_tier: TassadarEvidenceTrustTier::AcceptedOutcomeCompatible,
                accepted_outcome_required: true,
                validator_required: true,
                allow_research_only: false,
                max_evidence_burden_bps: 3_400,
                max_cost_milliunits: 3_500,
                allowed_route_families: [
                    TassadarPlannerRouteFamily::LanguageOnly,
                    TassadarPlannerRouteFamily::InternalExactCompute,
                    TassadarPlannerRouteFamily::ExternalTool,
                ],
                note: "patch-application mount where accepted-outcome and validator posture are mandatory",
            },
            candidates: [
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::LanguageOnly,
                    product_id: LANGUAGE_ONLY_PRODUCT_ID,
                    expected_correctness_bps: 5_400,
                    estimated_cost_milliunits: 680,
                    evidence_burden_bps: 850,
                    workload_fit_bps: 2_400,
                    trust_tier: TassadarEvidenceTrustTier::BenchmarkGated,
                    validator_attached: false,
                    accepted_outcome_ready: false,
                    research_only: false,
                    evidence_surface: "planner_summary",
                    note: "language-only route cannot satisfy accepted-outcome or validator requirements",
                },
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::InternalExactCompute,
                    product_id: INTERNAL_EXECUTOR_PRODUCT_ID,
                    expected_correctness_bps: 9_870,
                    estimated_cost_milliunits: 2_450,
                    evidence_burden_bps: 2_950,
                    workload_fit_bps: 9_750,
                    trust_tier: TassadarEvidenceTrustTier::AcceptedOutcomeCompatible,
                    validator_attached: true,
                    accepted_outcome_ready: true,
                    research_only: false,
                    evidence_surface: "executor_trace_and_validator_bundle",
                    note: "internal exact-compute route meets the current accepted-outcome and validator bar at lower cost than sandbox delegation",
                },
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::ExternalTool,
                    product_id: EXTERNAL_TOOL_PRODUCT_ID,
                    expected_correctness_bps: 9_800,
                    estimated_cost_milliunits: 4_600,
                    evidence_burden_bps: 3_300,
                    workload_fit_bps: 8_000,
                    trust_tier: TassadarEvidenceTrustTier::AcceptedOutcomeCompatible,
                    validator_attached: true,
                    accepted_outcome_ready: true,
                    research_only: false,
                    evidence_surface: "sandbox_execution_receipt",
                    note: "external tool route is admissible but costlier than the current bounded internal lane",
                },
            ],
            note: "evidence-aware routing should keep the accepted-outcome-capable internal lane instead of defaulting to the most expensive external proof path",
        },
        SeededCase {
            case_id: "long_loop_validator_mount",
            task_family: "long_loop_validator_mount",
            workload_class: TassadarWorkloadClass::LongLoopKernel,
            expected_route_family: TassadarPlannerRouteFamily::ExternalTool,
            mount_policy: SeededMountPolicy {
                mount_id: "mount.long_loop.validator.v1",
                mount_class: "validator_heavy_long_loop",
                required_trust_tier: TassadarEvidenceTrustTier::ValidatorBacked,
                accepted_outcome_required: true,
                validator_required: true,
                allow_research_only: false,
                max_evidence_burden_bps: 4_200,
                max_cost_milliunits: 5_000,
                allowed_route_families: [
                    TassadarPlannerRouteFamily::LanguageOnly,
                    TassadarPlannerRouteFamily::InternalExactCompute,
                    TassadarPlannerRouteFamily::ExternalTool,
                ],
                note: "long-loop mount where validator-backed robust execution matters more than the cheapest exact-looking lane",
            },
            candidates: [
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::LanguageOnly,
                    product_id: LANGUAGE_ONLY_PRODUCT_ID,
                    expected_correctness_bps: 3_900,
                    estimated_cost_milliunits: 700,
                    evidence_burden_bps: 700,
                    workload_fit_bps: 1_500,
                    trust_tier: TassadarEvidenceTrustTier::BenchmarkGated,
                    validator_attached: false,
                    accepted_outcome_ready: false,
                    research_only: false,
                    evidence_surface: "planner_summary",
                    note: "language-only route is too brittle for long-loop validator work",
                },
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::InternalExactCompute,
                    product_id: INTERNAL_EXECUTOR_PRODUCT_ID,
                    expected_correctness_bps: 9_720,
                    estimated_cost_milliunits: 2_700,
                    evidence_burden_bps: 3_100,
                    workload_fit_bps: 9_300,
                    trust_tier: TassadarEvidenceTrustTier::BenchmarkGated,
                    validator_attached: false,
                    accepted_outcome_ready: false,
                    research_only: false,
                    evidence_surface: "executor_trace_bundle",
                    note: "internal route looks attractive on capability and cost but cannot satisfy the mount's validator and accepted-outcome posture",
                },
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::ExternalTool,
                    product_id: EXTERNAL_TOOL_PRODUCT_ID,
                    expected_correctness_bps: 9_840,
                    estimated_cost_milliunits: 4_350,
                    evidence_burden_bps: 3_600,
                    workload_fit_bps: 9_550,
                    trust_tier: TassadarEvidenceTrustTier::AcceptedOutcomeCompatible,
                    validator_attached: true,
                    accepted_outcome_ready: true,
                    research_only: false,
                    evidence_surface: "sandbox_execution_receipt",
                    note: "external tool route remains the honest robust long-loop baseline under validator-heavy requirements",
                },
            ],
            note: "capability-only scoring would misroute into the internal lane, but evidence-aware routing should escalate to the validator-backed external lane",
        },
        SeededCase {
            case_id: "research_mount_search_lane",
            task_family: "research_mount_search_lane",
            workload_class: TassadarWorkloadClass::SudokuClass,
            expected_route_family: TassadarPlannerRouteFamily::InternalExactCompute,
            mount_policy: SeededMountPolicy {
                mount_id: "mount.search.research.v1",
                mount_class: "research_search_mount",
                required_trust_tier: TassadarEvidenceTrustTier::ResearchOnly,
                accepted_outcome_required: false,
                validator_required: false,
                allow_research_only: true,
                max_evidence_burden_bps: 3_200,
                max_cost_milliunits: 3_400,
                allowed_route_families: [
                    TassadarPlannerRouteFamily::LanguageOnly,
                    TassadarPlannerRouteFamily::InternalExactCompute,
                    TassadarPlannerRouteFamily::ExternalTool,
                ],
                note: "research mount where bounded research-only internal search remains admissible",
            },
            candidates: [
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::LanguageOnly,
                    product_id: LANGUAGE_ONLY_PRODUCT_ID,
                    expected_correctness_bps: 4_500,
                    estimated_cost_milliunits: 720,
                    evidence_burden_bps: 750,
                    workload_fit_bps: 2_100,
                    trust_tier: TassadarEvidenceTrustTier::BenchmarkGated,
                    validator_attached: false,
                    accepted_outcome_ready: false,
                    research_only: false,
                    evidence_surface: "planner_summary",
                    note: "language-only route is a poor fit for exact search checking",
                },
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::InternalExactCompute,
                    product_id: INTERNAL_EXECUTOR_PRODUCT_ID,
                    expected_correctness_bps: 9_650,
                    estimated_cost_milliunits: 2_900,
                    evidence_burden_bps: 2_900,
                    workload_fit_bps: 9_500,
                    trust_tier: TassadarEvidenceTrustTier::ResearchOnly,
                    validator_attached: false,
                    accepted_outcome_ready: false,
                    research_only: true,
                    evidence_surface: "research_executor_trace_bundle",
                    note: "research-only internal search lane is acceptable on this mount and cheaper than sandbox delegation",
                },
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::ExternalTool,
                    product_id: EXTERNAL_TOOL_PRODUCT_ID,
                    expected_correctness_bps: 9_600,
                    estimated_cost_milliunits: 5_050,
                    evidence_burden_bps: 3_900,
                    workload_fit_bps: 8_600,
                    trust_tier: TassadarEvidenceTrustTier::AcceptedOutcomeCompatible,
                    validator_attached: true,
                    accepted_outcome_ready: true,
                    research_only: false,
                    evidence_surface: "sandbox_execution_receipt",
                    note: "sandbox delegation is still valid but unnecessary on a research mount with a bounded search lane",
                },
            ],
            note: "research-only internal search should stay selectable when the mount explicitly allows it",
        },
        SeededCase {
            case_id: "served_search_validator_mount",
            task_family: "served_search_validator_mount",
            workload_class: TassadarWorkloadClass::SudokuClass,
            expected_route_family: TassadarPlannerRouteFamily::ExternalTool,
            mount_policy: SeededMountPolicy {
                mount_id: "mount.search.validator.v1",
                mount_class: "served_search_mount",
                required_trust_tier: TassadarEvidenceTrustTier::ValidatorBacked,
                accepted_outcome_required: false,
                validator_required: true,
                allow_research_only: false,
                max_evidence_burden_bps: 4_000,
                max_cost_milliunits: 5_200,
                allowed_route_families: [
                    TassadarPlannerRouteFamily::LanguageOnly,
                    TassadarPlannerRouteFamily::InternalExactCompute,
                    TassadarPlannerRouteFamily::ExternalTool,
                ],
                note: "served search mount where research-only internal search must not leak into a validator-facing route",
            },
            candidates: [
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::LanguageOnly,
                    product_id: LANGUAGE_ONLY_PRODUCT_ID,
                    expected_correctness_bps: 4_400,
                    estimated_cost_milliunits: 720,
                    evidence_burden_bps: 760,
                    workload_fit_bps: 2_000,
                    trust_tier: TassadarEvidenceTrustTier::BenchmarkGated,
                    validator_attached: false,
                    accepted_outcome_ready: false,
                    research_only: false,
                    evidence_surface: "planner_summary",
                    note: "language-only route cannot satisfy validator-backed search posture",
                },
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::InternalExactCompute,
                    product_id: INTERNAL_EXECUTOR_PRODUCT_ID,
                    expected_correctness_bps: 9_680,
                    estimated_cost_milliunits: 2_850,
                    evidence_burden_bps: 2_950,
                    workload_fit_bps: 9_450,
                    trust_tier: TassadarEvidenceTrustTier::ResearchOnly,
                    validator_attached: false,
                    accepted_outcome_ready: false,
                    research_only: true,
                    evidence_surface: "research_executor_trace_bundle",
                    note: "internal search is strong but remains research-only and lacks validator attachment on this mount",
                },
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::ExternalTool,
                    product_id: EXTERNAL_TOOL_PRODUCT_ID,
                    expected_correctness_bps: 9_520,
                    estimated_cost_milliunits: 4_950,
                    evidence_burden_bps: 3_700,
                    workload_fit_bps: 8_700,
                    trust_tier: TassadarEvidenceTrustTier::AcceptedOutcomeCompatible,
                    validator_attached: true,
                    accepted_outcome_ready: true,
                    research_only: false,
                    evidence_surface: "sandbox_execution_receipt",
                    note: "external route is the honest served baseline while internal search stays benchmark-only",
                },
            ],
            note: "evidence-aware routing should refuse silent promotion from research-only search to a served validator-facing lane",
        },
        SeededCase {
            case_id: "egress_bounded_exact_transform",
            task_family: "egress_bounded_exact_transform",
            workload_class: TassadarWorkloadClass::BranchHeavyKernel,
            expected_route_family: TassadarPlannerRouteFamily::InternalExactCompute,
            mount_policy: SeededMountPolicy {
                mount_id: "mount.egress_bounded_exact_transform.v1",
                mount_class: "no_external_egress_exact_transform",
                required_trust_tier: TassadarEvidenceTrustTier::BenchmarkGated,
                accepted_outcome_required: false,
                validator_required: false,
                allow_research_only: false,
                max_evidence_burden_bps: 3_300,
                max_cost_milliunits: 3_600,
                allowed_route_families: [
                    TassadarPlannerRouteFamily::LanguageOnly,
                    TassadarPlannerRouteFamily::InternalExactCompute,
                    TassadarPlannerRouteFamily::InternalExactCompute,
                ],
                note: "mount where external delegation is forbidden by egress posture, so the bounded internal exact lane must win if admissible",
            },
            candidates: [
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::LanguageOnly,
                    product_id: LANGUAGE_ONLY_PRODUCT_ID,
                    expected_correctness_bps: 5_600,
                    estimated_cost_milliunits: 700,
                    evidence_burden_bps: 820,
                    workload_fit_bps: 2_200,
                    trust_tier: TassadarEvidenceTrustTier::BenchmarkGated,
                    validator_attached: false,
                    accepted_outcome_ready: false,
                    research_only: false,
                    evidence_surface: "planner_summary",
                    note: "language-only route remains inadvisable for exact transform work",
                },
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::InternalExactCompute,
                    product_id: INTERNAL_EXECUTOR_PRODUCT_ID,
                    expected_correctness_bps: 9_830,
                    estimated_cost_milliunits: 2_600,
                    evidence_burden_bps: 2_980,
                    workload_fit_bps: 9_600,
                    trust_tier: TassadarEvidenceTrustTier::BenchmarkGated,
                    validator_attached: true,
                    accepted_outcome_ready: true,
                    research_only: false,
                    evidence_surface: "executor_trace_and_proof_bundle",
                    note: "internal bounded exact lane satisfies the no-egress mount without widening to sandbox delegation",
                },
                SeededRouteCandidate {
                    route_family: TassadarPlannerRouteFamily::ExternalTool,
                    product_id: EXTERNAL_TOOL_PRODUCT_ID,
                    expected_correctness_bps: 9_920,
                    estimated_cost_milliunits: 4_400,
                    evidence_burden_bps: 3_550,
                    workload_fit_bps: 8_400,
                    trust_tier: TassadarEvidenceTrustTier::AcceptedOutcomeCompatible,
                    validator_attached: true,
                    accepted_outcome_ready: true,
                    research_only: false,
                    evidence_surface: "sandbox_execution_receipt",
                    note: "external route is powerful but forbidden by the mount's no-egress allowlist",
                },
            ],
            note: "mount policy should explicitly keep the route internal instead of silently preferring a forbidden external lane",
        },
    ]
}

fn build_case_evaluation(spec: &SeededCase) -> TassadarMountScopedRouteEvaluation {
    let mount_policy = build_mount_policy(spec.mount_policy);
    let mut route_scores = spec
        .candidates
        .iter()
        .map(|candidate| build_route_score(*candidate, &mount_policy))
        .collect::<Vec<_>>();
    route_scores.sort_by_key(|score| score.route_family.as_str());
    let capability_only_selected_route_family = route_scores
        .iter()
        .max_by(|left, right| {
            left.capability_only_score
                .cmp(&right.capability_only_score)
                .then_with(|| {
                    left.expected_correctness_bps
                        .cmp(&right.expected_correctness_bps)
                })
                .then_with(|| {
                    right
                        .estimated_cost_milliunits
                        .cmp(&left.estimated_cost_milliunits)
                })
                .then_with(|| left.route_family.as_str().cmp(right.route_family.as_str()))
        })
        .map(|score| score.route_family)
        .expect("seeded cases should always expose at least one route");
    let evidence_aware_selected_route_family = route_scores
        .iter()
        .filter(|score| score.admissibility == TassadarEvidenceRouteAdmissibility::Admissible)
        .max_by(|left, right| {
            left.evidence_calibrated_score
                .cmp(&right.evidence_calibrated_score)
                .then_with(|| {
                    left.expected_correctness_bps
                        .cmp(&right.expected_correctness_bps)
                })
                .then_with(|| {
                    right
                        .estimated_cost_milliunits
                        .cmp(&left.estimated_cost_milliunits)
                })
                .then_with(|| left.route_family.as_str().cmp(right.route_family.as_str()))
        })
        .map(|score| score.route_family)
        .expect("seeded cases should always leave one admissible route");
    let capability_only_would_violate_mount = route_scores
        .iter()
        .find(|score| score.route_family == capability_only_selected_route_family)
        .map(|score| score.admissibility == TassadarEvidenceRouteAdmissibility::RefusedByPolicy)
        .unwrap_or(false);
    let evidence_aware_score = route_scores
        .iter()
        .find(|score| score.route_family == evidence_aware_selected_route_family)
        .expect("selected evidence-aware route should exist");
    TassadarMountScopedRouteEvaluation {
        case_id: String::from(spec.case_id),
        task_family: String::from(spec.task_family),
        workload_class: spec.workload_class,
        mount_policy,
        capability_only_selected_route_family,
        evidence_aware_selected_route_family,
        expected_route_family: spec.expected_route_family,
        selection_matches_expected: evidence_aware_selected_route_family
            == spec.expected_route_family,
        capability_only_would_violate_mount,
        evidence_aware_policy_compliant: evidence_aware_score.admissibility
            == TassadarEvidenceRouteAdmissibility::Admissible,
        misroute_avoided: capability_only_would_violate_mount
            && evidence_aware_selected_route_family != capability_only_selected_route_family,
        selected_route_validator_attached: evidence_aware_score.validator_attached,
        selected_route_accepted_outcome_ready: evidence_aware_score.accepted_outcome_ready,
        route_scores,
        note: String::from(spec.note),
    }
}

fn build_mount_policy(spec: SeededMountPolicy) -> TassadarWorldMountRoutePolicy {
    let mut allowed_route_families = spec.allowed_route_families.to_vec();
    allowed_route_families.sort_by_key(|family| family.as_str());
    allowed_route_families.dedup();
    TassadarWorldMountRoutePolicy {
        mount_id: String::from(spec.mount_id),
        mount_class: String::from(spec.mount_class),
        required_trust_tier: spec.required_trust_tier,
        accepted_outcome_required: spec.accepted_outcome_required,
        validator_required: spec.validator_required,
        allow_research_only: spec.allow_research_only,
        max_evidence_burden_bps: spec.max_evidence_burden_bps,
        max_cost_milliunits: spec.max_cost_milliunits,
        allowed_route_families,
        note: String::from(spec.note),
    }
}

fn build_route_score(
    candidate: SeededRouteCandidate,
    mount_policy: &TassadarWorldMountRoutePolicy,
) -> TassadarEvidenceCalibratedRouteScore {
    let (admissibility, refusal_reason) = route_admissibility(candidate, mount_policy);
    let cost_penalty = candidate.estimated_cost_milliunits.min(10_000) as i32;
    let capability_only_score = (candidate.expected_correctness_bps as i32 * 58 / 100)
        + (candidate.workload_fit_bps as i32 * 22 / 100)
        - (cost_penalty * 20 / 100);
    let trust_bonus = trust_bonus(candidate.trust_tier, mount_policy.required_trust_tier);
    let validator_bonus = if mount_policy.validator_required && candidate.validator_attached {
        650
    } else {
        0
    };
    let accepted_outcome_bonus =
        if mount_policy.accepted_outcome_required && candidate.accepted_outcome_ready {
            700
        } else {
            0
        };
    let evidence_calibrated_score =
        if admissibility == TassadarEvidenceRouteAdmissibility::Admissible {
            (candidate.expected_correctness_bps as i32 * 40 / 100)
                + (candidate.workload_fit_bps as i32 * 18 / 100)
                + trust_bonus
                + validator_bonus
                + accepted_outcome_bonus
                - (cost_penalty * 12 / 100)
                - (candidate.evidence_burden_bps as i32 * 15 / 100)
        } else {
            capability_only_score - 5_000
        };
    TassadarEvidenceCalibratedRouteScore {
        route_family: candidate.route_family,
        product_id: String::from(candidate.product_id),
        expected_correctness_bps: candidate.expected_correctness_bps,
        estimated_cost_milliunits: candidate.estimated_cost_milliunits,
        evidence_burden_bps: candidate.evidence_burden_bps,
        workload_fit_bps: candidate.workload_fit_bps,
        trust_tier: candidate.trust_tier,
        validator_attached: candidate.validator_attached,
        accepted_outcome_ready: candidate.accepted_outcome_ready,
        research_only: candidate.research_only,
        admissibility,
        refusal_reason,
        capability_only_score,
        evidence_calibrated_score,
        evidence_surface: String::from(candidate.evidence_surface),
        note: String::from(candidate.note),
    }
}

fn route_admissibility(
    candidate: SeededRouteCandidate,
    mount_policy: &TassadarWorldMountRoutePolicy,
) -> (
    TassadarEvidenceRouteAdmissibility,
    Option<TassadarEvidenceRoutingRefusalReason>,
) {
    if !mount_policy
        .allowed_route_families
        .contains(&candidate.route_family)
    {
        return (
            TassadarEvidenceRouteAdmissibility::RefusedByPolicy,
            Some(TassadarEvidenceRoutingRefusalReason::RouteFamilyDisallowed),
        );
    }
    if candidate.research_only && !mount_policy.allow_research_only {
        return (
            TassadarEvidenceRouteAdmissibility::RefusedByPolicy,
            Some(TassadarEvidenceRoutingRefusalReason::ResearchOnlyDisallowed),
        );
    }
    if candidate.trust_tier < mount_policy.required_trust_tier {
        return (
            TassadarEvidenceRouteAdmissibility::RefusedByPolicy,
            Some(TassadarEvidenceRoutingRefusalReason::TrustTierInsufficient),
        );
    }
    if mount_policy.validator_required && !candidate.validator_attached {
        return (
            TassadarEvidenceRouteAdmissibility::RefusedByPolicy,
            Some(TassadarEvidenceRoutingRefusalReason::ValidatorRequired),
        );
    }
    if mount_policy.accepted_outcome_required && !candidate.accepted_outcome_ready {
        return (
            TassadarEvidenceRouteAdmissibility::RefusedByPolicy,
            Some(TassadarEvidenceRoutingRefusalReason::AcceptedOutcomeEvidenceMissing),
        );
    }
    if candidate.evidence_burden_bps > mount_policy.max_evidence_burden_bps {
        return (
            TassadarEvidenceRouteAdmissibility::RefusedByPolicy,
            Some(TassadarEvidenceRoutingRefusalReason::EvidenceBudgetExceeded),
        );
    }
    if candidate.estimated_cost_milliunits > mount_policy.max_cost_milliunits {
        return (
            TassadarEvidenceRouteAdmissibility::RefusedByPolicy,
            Some(TassadarEvidenceRoutingRefusalReason::CostBudgetExceeded),
        );
    }
    (TassadarEvidenceRouteAdmissibility::Admissible, None)
}

fn selected_route_score(
    case: &TassadarMountScopedRouteEvaluation,
) -> &TassadarEvidenceCalibratedRouteScore {
    case.route_scores
        .iter()
        .find(|score| score.route_family == case.evidence_aware_selected_route_family)
        .expect("selected route should exist")
}

fn trust_bonus(
    candidate_tier: TassadarEvidenceTrustTier,
    required_tier: TassadarEvidenceTrustTier,
) -> i32 {
    match (candidate_tier as i32) - (required_tier as i32) {
        2.. => 900,
        1 => 450,
        0 => 250,
        _ => -1_000,
    }
}

fn ratio_bps(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        0
    } else {
        numerator.saturating_mul(10_000) / denominator
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-router crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarEvidenceCalibratedRoutingReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarEvidenceCalibratedRoutingReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarEvidenceCalibratedRoutingReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
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
        TassadarEvidenceCalibratedRoutingReport, TassadarEvidenceRouteAdmissibility,
        TassadarEvidenceRoutingRefusalReason, build_tassadar_evidence_calibrated_routing_report,
        read_repo_json, tassadar_evidence_calibrated_routing_report_path,
        write_tassadar_evidence_calibrated_routing_report,
    };
    use psionic_models::TassadarPlannerRouteFamily;

    #[test]
    fn evidence_calibrated_routing_changes_route_when_mount_constraints_matter() {
        let report = build_tassadar_evidence_calibrated_routing_report()
            .expect("evidence-calibrated routing report");

        assert_eq!(report.evaluated_cases.len(), 6);
        let long_loop_case = report
            .evaluated_cases
            .iter()
            .find(|case| case.case_id == "long_loop_validator_mount")
            .expect("long-loop case");
        assert_eq!(
            long_loop_case.capability_only_selected_route_family,
            TassadarPlannerRouteFamily::InternalExactCompute
        );
        assert_eq!(
            long_loop_case.evidence_aware_selected_route_family,
            TassadarPlannerRouteFamily::ExternalTool
        );
        assert!(long_loop_case.capability_only_would_violate_mount);
        assert!(long_loop_case.misroute_avoided);
    }

    #[test]
    fn evidence_calibrated_routing_keeps_refusal_reasons_machine_legible() {
        let report = build_tassadar_evidence_calibrated_routing_report()
            .expect("evidence-calibrated routing report");

        let served_search_case = report
            .evaluated_cases
            .iter()
            .find(|case| case.case_id == "served_search_validator_mount")
            .expect("served search case");
        let internal_route = served_search_case
            .route_scores
            .iter()
            .find(|score| score.route_family == TassadarPlannerRouteFamily::InternalExactCompute)
            .expect("internal route");
        assert_eq!(
            internal_route.admissibility,
            TassadarEvidenceRouteAdmissibility::RefusedByPolicy
        );
        assert_eq!(
            internal_route.refusal_reason,
            Some(TassadarEvidenceRoutingRefusalReason::ResearchOnlyDisallowed)
        );
        assert!(report.capability_only_mount_violation_rate_bps >= 4_000);
        assert_eq!(report.evidence_aware_policy_compliance_rate_bps, 10_000);
    }

    #[test]
    fn evidence_calibrated_routing_report_matches_committed_truth() {
        let report = build_tassadar_evidence_calibrated_routing_report()
            .expect("evidence-calibrated routing report");
        let committed: TassadarEvidenceCalibratedRoutingReport =
            read_repo_json(super::TASSADAR_EVIDENCE_CALIBRATED_ROUTING_REPORT_REF)
                .expect("committed routing report");

        assert_eq!(report, committed);
    }

    #[test]
    fn evidence_calibrated_routing_report_writer_uses_committed_path() {
        let path = tassadar_evidence_calibrated_routing_report_path();
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("tassadar_evidence_calibrated_routing_report.json")
        );
        assert!(path.display().to_string().ends_with(
            "fixtures/tassadar/reports/tassadar_evidence_calibrated_routing_report.json"
        ));
    }

    #[test]
    fn evidence_calibrated_routing_report_writer_round_trips() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let output_path = temp_dir
            .path()
            .join("tassadar_evidence_calibrated_routing_report.json");
        let written =
            write_tassadar_evidence_calibrated_routing_report(&output_path).expect("write report");
        let decoded = std::fs::read_to_string(&output_path).expect("read report");
        let reparsed: TassadarEvidenceCalibratedRoutingReport =
            serde_json::from_str(&decoded).expect("decode report");

        assert_eq!(written, reparsed);
    }
}
