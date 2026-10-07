//! Product-regression guardrails for benchmark-optimized legal agent modules.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{DataClassification, Metadata, stable_json_digest};

pub const LEGAL_BENCHMARK_PRODUCT_REGRESSION_SCHEMA_VERSION: u16 = 1;

pub const REQUIRED_PRODUCT_REGRESSION_SURFACES: [AutopilotProductSurface; 7] = [
    AutopilotProductSurface::Chat,
    AutopilotProductSurface::Coder,
    AutopilotProductSurface::WorkOrder,
    AutopilotProductSurface::GithubProvider,
    AutopilotProductSurface::Crm,
    AutopilotProductSurface::Memory,
    AutopilotProductSurface::ProviderToolRouting,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutopilotProductSurface {
    Chat,
    Coder,
    WorkOrder,
    GithubProvider,
    Crm,
    Memory,
    ProviderToolRouting,
}

impl AutopilotProductSurface {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Coder => "coder",
            Self::WorkOrder => "work_order",
            Self::GithubProvider => "github_provider",
            Self::Crm => "crm",
            Self::Memory => "memory",
            Self::ProviderToolRouting => "provider_tool_routing",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionSuite {
    pub schema_version: u16,
    pub suite_id: String,
    pub suite_version: String,
    pub tasks: Vec<ProductRegressionTaskFixture>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionTaskFixture {
    pub task_id: String,
    pub surface: AutopilotProductSurface,
    pub title: String,
    pub instructions: String,
    pub expected_capabilities: Vec<String>,
    pub minimum_score_bps: u32,
    pub baseline_score_bps: u32,
    pub max_allowed_drop_bps: u32,
    pub data_policy: ProductRegressionDataPolicy,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionDataPolicy {
    pub data_classification: DataClassification,
    pub live_user_data_allowed: bool,
    pub harvey_hidden_criteria_allowed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionGateConfig {
    pub schema_version: u16,
    pub gate_id: String,
    pub minimum_suite_score_bps: u32,
    pub minimum_surface_score_bps: u32,
    pub require_all_tasks_pass: bool,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionCandidateRun {
    pub schema_version: u16,
    pub candidate_module_version_id: String,
    pub benchmark_summary: ProductRegressionBenchmarkSummary,
    pub outcomes: Vec<ProductRegressionTaskOutcome>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionBenchmarkSummary {
    pub benchmark_suite_id: String,
    pub benchmark_target_score_bps: u32,
    pub benchmark_candidate_score_bps: u32,
    pub previous_benchmark_score_bps: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sweep_matrix_export_ref: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionTaskOutcome {
    pub task_id: String,
    pub surface: AutopilotProductSurface,
    pub passed: bool,
    pub score_bps: u32,
    pub latency_ms: u64,
    pub cost_micro_usd: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductRegressionGateStatus {
    Passed,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductRegressionFailureKind {
    MissingRequiredSurface,
    MissingOutcome,
    SurfaceMismatch,
    TaskFailed,
    ScoreBelowMinimum,
    BaselineRegression,
    SuiteBelowMinimum,
    SurfaceBelowMinimum,
    BenchmarkTargetMissed,
    UnsafeFixture,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionFailure {
    pub failure_id: String,
    pub failure_kind: ProductRegressionFailureKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub surface: AutopilotProductSurface,
    pub score_bps: u32,
    pub threshold_bps: u32,
    pub diagnostic: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionSurfaceScore {
    pub surface: AutopilotProductSurface,
    pub task_count: u64,
    pub passed_count: u64,
    pub score_bps: u32,
    pub pass_rate_bps: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionScoreSummary {
    pub task_count: u64,
    pub passed_count: u64,
    pub suite_score_bps: u32,
    pub pass_rate_bps: u32,
    pub by_surface: Vec<ProductRegressionSurfaceScore>,
    pub total_latency_ms: u64,
    pub total_cost_micro_usd: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionWorkOrder {
    pub work_order_id: String,
    pub title: String,
    pub severity: ProductRegressionWorkOrderSeverity,
    pub surface: AutopilotProductSurface,
    pub blocking_failure_ids: Vec<String>,
    pub candidate_module_version_id: String,
    pub reproduction_command: String,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductRegressionWorkOrderSeverity {
    Blocker,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionAutopilot4GateImport {
    pub schema_version: u16,
    pub release_gate_id: String,
    pub gate_name: String,
    pub source_gate_report_id: String,
    pub candidate_module_version_id: String,
    pub gate_status: ProductRegressionGateStatus,
    pub promotion_blocked: bool,
    pub benchmark_target_score_bps: u32,
    pub benchmark_candidate_score_bps: u32,
    pub product_regression_score_bps: u32,
    pub product_regression_pass_rate_bps: u32,
    pub blocking_failure_ids: Vec<String>,
    pub work_order_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionGateReport {
    pub schema_version: u16,
    pub gate_report_id: String,
    pub gate_id: String,
    pub generated_at_ms: u64,
    pub suite_id: String,
    pub suite_version: String,
    pub suite_hash: String,
    pub candidate_module_version_id: String,
    pub benchmark_summary: ProductRegressionBenchmarkSummary,
    pub product_summary: ProductRegressionScoreSummary,
    pub status: ProductRegressionGateStatus,
    pub failures: Vec<ProductRegressionFailure>,
    pub work_orders: Vec<ProductRegressionWorkOrder>,
    pub autopilot4_release_gate_import: ProductRegressionAutopilot4GateImport,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductRegressionFixtureIssue {
    pub issue_id: String,
    pub issue_kind: ProductRegressionFixtureIssueKind,
    pub surface: AutopilotProductSurface,
    pub diagnostic: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductRegressionFixtureIssueKind {
    MissingRequiredSurface,
    UnsafeFixture,
}

pub fn validate_product_regression_suite(
    suite: &ProductRegressionSuite,
) -> Vec<ProductRegressionFixtureIssue> {
    let mut issues = Vec::new();
    let present = suite
        .tasks
        .iter()
        .map(|task| task.surface)
        .collect::<BTreeSet<_>>();
    for surface in REQUIRED_PRODUCT_REGRESSION_SURFACES {
        if !present.contains(&surface) {
            issues.push(ProductRegressionFixtureIssue {
                issue_id: format!("missing_surface.{}", surface.as_str()),
                issue_kind: ProductRegressionFixtureIssueKind::MissingRequiredSurface,
                surface,
                diagnostic: String::from("regression suite is missing a required product surface"),
            });
        }
    }
    for task in &suite.tasks {
        if task.data_policy.live_user_data_allowed
            || task.data_policy.harvey_hidden_criteria_allowed
        {
            issues.push(ProductRegressionFixtureIssue {
                issue_id: format!("unsafe_fixture.{}", task.task_id),
                issue_kind: ProductRegressionFixtureIssueKind::UnsafeFixture,
                surface: task.surface,
                diagnostic: String::from(
                    "regression fixtures must not require live user data or Harvey hidden criteria",
                ),
            });
        }
    }
    issues
}

pub fn evaluate_product_regression_gate(
    suite: &ProductRegressionSuite,
    config: &ProductRegressionGateConfig,
    candidate: &ProductRegressionCandidateRun,
) -> Result<ProductRegressionGateReport, serde_json::Error> {
    let mut failures = Vec::new();
    let outcomes_by_task = candidate
        .outcomes
        .iter()
        .map(|outcome| (outcome.task_id.clone(), outcome))
        .collect::<BTreeMap<_, _>>();
    let mut scored = Vec::new();

    for fixture_issue in validate_product_regression_suite(suite) {
        failures.push(ProductRegressionFailure {
            failure_id: fixture_issue.issue_id,
            failure_kind: match fixture_issue.issue_kind {
                ProductRegressionFixtureIssueKind::MissingRequiredSurface => {
                    ProductRegressionFailureKind::MissingRequiredSurface
                }
                ProductRegressionFixtureIssueKind::UnsafeFixture => {
                    ProductRegressionFailureKind::UnsafeFixture
                }
            },
            task_id: None,
            surface: fixture_issue.surface,
            score_bps: 0,
            threshold_bps: 10_000,
            diagnostic: fixture_issue.diagnostic,
        });
    }

    for task in &suite.tasks {
        match outcomes_by_task.get(&task.task_id) {
            Some(outcome) => {
                scored.push((*outcome).clone());
                if outcome.surface != task.surface {
                    failures.push(ProductRegressionFailure {
                        failure_id: format!("surface_mismatch.{}", task.task_id),
                        failure_kind: ProductRegressionFailureKind::SurfaceMismatch,
                        task_id: Some(task.task_id.clone()),
                        surface: task.surface,
                        score_bps: outcome.score_bps,
                        threshold_bps: task.minimum_score_bps,
                        diagnostic: String::from(
                            "candidate outcome used the wrong product surface",
                        ),
                    });
                }
                if config.require_all_tasks_pass && !outcome.passed {
                    failures.push(ProductRegressionFailure {
                        failure_id: format!("task_failed.{}", task.task_id),
                        failure_kind: ProductRegressionFailureKind::TaskFailed,
                        task_id: Some(task.task_id.clone()),
                        surface: task.surface,
                        score_bps: outcome.score_bps,
                        threshold_bps: task.minimum_score_bps,
                        diagnostic: diagnostic_or_default(
                            outcome,
                            "candidate failed a required product regression task",
                        ),
                    });
                }
                if outcome.score_bps < task.minimum_score_bps {
                    failures.push(ProductRegressionFailure {
                        failure_id: format!("score_below_minimum.{}", task.task_id),
                        failure_kind: ProductRegressionFailureKind::ScoreBelowMinimum,
                        task_id: Some(task.task_id.clone()),
                        surface: task.surface,
                        score_bps: outcome.score_bps,
                        threshold_bps: task.minimum_score_bps,
                        diagnostic: diagnostic_or_default(
                            outcome,
                            "candidate score fell below the fixture threshold",
                        ),
                    });
                }
                let allowed_floor = task
                    .baseline_score_bps
                    .saturating_sub(task.max_allowed_drop_bps);
                if outcome.score_bps < allowed_floor {
                    failures.push(ProductRegressionFailure {
                        failure_id: format!("baseline_regression.{}", task.task_id),
                        failure_kind: ProductRegressionFailureKind::BaselineRegression,
                        task_id: Some(task.task_id.clone()),
                        surface: task.surface,
                        score_bps: outcome.score_bps,
                        threshold_bps: allowed_floor,
                        diagnostic: diagnostic_or_default(
                            outcome,
                            "candidate regressed against the recorded production baseline",
                        ),
                    });
                }
            }
            None => {
                scored.push(ProductRegressionTaskOutcome {
                    task_id: task.task_id.clone(),
                    surface: task.surface,
                    passed: false,
                    score_bps: 0,
                    latency_ms: 0,
                    cost_micro_usd: 0,
                    diagnostics: vec![String::from("missing task outcome")],
                    evidence_refs: Vec::new(),
                });
                failures.push(ProductRegressionFailure {
                    failure_id: format!("missing_outcome.{}", task.task_id),
                    failure_kind: ProductRegressionFailureKind::MissingOutcome,
                    task_id: Some(task.task_id.clone()),
                    surface: task.surface,
                    score_bps: 0,
                    threshold_bps: task.minimum_score_bps,
                    diagnostic: String::from("candidate run did not report this regression task"),
                });
            }
        }
    }

    let product_summary = summarize_product_regression_scores(&scored);
    if product_summary.suite_score_bps < config.minimum_suite_score_bps {
        failures.push(ProductRegressionFailure {
            failure_id: String::from("suite_below_minimum"),
            failure_kind: ProductRegressionFailureKind::SuiteBelowMinimum,
            task_id: None,
            surface: AutopilotProductSurface::ProviderToolRouting,
            score_bps: product_summary.suite_score_bps,
            threshold_bps: config.minimum_suite_score_bps,
            diagnostic: String::from("aggregate product regression score is below gate minimum"),
        });
    }
    for surface in &product_summary.by_surface {
        if surface.score_bps < config.minimum_surface_score_bps {
            failures.push(ProductRegressionFailure {
                failure_id: format!("surface_below_minimum.{}", surface.surface.as_str()),
                failure_kind: ProductRegressionFailureKind::SurfaceBelowMinimum,
                task_id: None,
                surface: surface.surface,
                score_bps: surface.score_bps,
                threshold_bps: config.minimum_surface_score_bps,
                diagnostic: String::from("product surface score is below gate minimum"),
            });
        }
    }
    if candidate.benchmark_summary.benchmark_candidate_score_bps
        < candidate.benchmark_summary.benchmark_target_score_bps
    {
        failures.push(ProductRegressionFailure {
            failure_id: String::from("benchmark_target_missed"),
            failure_kind: ProductRegressionFailureKind::BenchmarkTargetMissed,
            task_id: None,
            surface: AutopilotProductSurface::ProviderToolRouting,
            score_bps: candidate.benchmark_summary.benchmark_candidate_score_bps,
            threshold_bps: candidate.benchmark_summary.benchmark_target_score_bps,
            diagnostic: String::from("candidate did not meet the benchmark target score"),
        });
    }

    let status = if failures.is_empty() {
        ProductRegressionGateStatus::Passed
    } else {
        ProductRegressionGateStatus::Blocked
    };
    let gate_report_id = format!(
        "report.{}.{}",
        config.gate_id, candidate.candidate_module_version_id
    );
    let work_orders = work_orders_for_failures(&candidate.candidate_module_version_id, &failures);
    let release_gate_import = ProductRegressionAutopilot4GateImport {
        schema_version: LEGAL_BENCHMARK_PRODUCT_REGRESSION_SCHEMA_VERSION,
        release_gate_id: format!("autopilot4.release_gate.{}", config.gate_id),
        gate_name: String::from("legal_benchmark_product_regression"),
        source_gate_report_id: gate_report_id.clone(),
        candidate_module_version_id: candidate.candidate_module_version_id.clone(),
        gate_status: status,
        promotion_blocked: status == ProductRegressionGateStatus::Blocked,
        benchmark_target_score_bps: candidate.benchmark_summary.benchmark_target_score_bps,
        benchmark_candidate_score_bps: candidate.benchmark_summary.benchmark_candidate_score_bps,
        product_regression_score_bps: product_summary.suite_score_bps,
        product_regression_pass_rate_bps: product_summary.pass_rate_bps,
        blocking_failure_ids: failures
            .iter()
            .map(|failure| failure.failure_id.clone())
            .collect(),
        work_order_ids: work_orders
            .iter()
            .map(|work_order| work_order.work_order_id.clone())
            .collect(),
        metadata: Metadata::new(),
    };

    Ok(ProductRegressionGateReport {
        schema_version: LEGAL_BENCHMARK_PRODUCT_REGRESSION_SCHEMA_VERSION,
        gate_report_id,
        gate_id: config.gate_id.clone(),
        generated_at_ms: now_ms(),
        suite_id: suite.suite_id.clone(),
        suite_version: suite.suite_version.clone(),
        suite_hash: product_regression_suite_hash(suite)?,
        candidate_module_version_id: candidate.candidate_module_version_id.clone(),
        benchmark_summary: candidate.benchmark_summary.clone(),
        product_summary,
        status,
        failures,
        work_orders,
        autopilot4_release_gate_import: release_gate_import,
        metadata: Metadata::new(),
    })
}

pub fn summarize_product_regression_scores(
    outcomes: &[ProductRegressionTaskOutcome],
) -> ProductRegressionScoreSummary {
    let task_count = u64::try_from(outcomes.len()).unwrap_or(u64::MAX);
    let passed_count =
        u64::try_from(outcomes.iter().filter(|outcome| outcome.passed).count()).unwrap_or(0);
    let score_sum = outcomes
        .iter()
        .map(|outcome| u64::from(outcome.score_bps))
        .sum::<u64>();
    let mut by_surface =
        BTreeMap::<AutopilotProductSurface, Vec<&ProductRegressionTaskOutcome>>::new();
    for outcome in outcomes {
        by_surface.entry(outcome.surface).or_default().push(outcome);
    }
    ProductRegressionScoreSummary {
        task_count,
        passed_count,
        suite_score_bps: average_bps(score_sum, task_count),
        pass_rate_bps: ratio_bps(passed_count, task_count),
        by_surface: by_surface
            .into_iter()
            .map(|(surface, surface_outcomes)| {
                let task_count = u64::try_from(surface_outcomes.len()).unwrap_or(u64::MAX);
                let passed_count = u64::try_from(
                    surface_outcomes
                        .iter()
                        .filter(|outcome| outcome.passed)
                        .count(),
                )
                .unwrap_or(0);
                let score_sum = surface_outcomes
                    .iter()
                    .map(|outcome| u64::from(outcome.score_bps))
                    .sum::<u64>();
                ProductRegressionSurfaceScore {
                    surface,
                    task_count,
                    passed_count,
                    score_bps: average_bps(score_sum, task_count),
                    pass_rate_bps: ratio_bps(passed_count, task_count),
                }
            })
            .collect(),
        total_latency_ms: outcomes.iter().map(|outcome| outcome.latency_ms).sum(),
        total_cost_micro_usd: outcomes.iter().map(|outcome| outcome.cost_micro_usd).sum(),
    }
}

pub fn product_regression_suite_hash(
    suite: &ProductRegressionSuite,
) -> Result<String, serde_json::Error> {
    stable_json_digest("psionic.legal_benchmark.product_regression_suite.v1", suite)
}

pub fn product_regression_candidate_hash(
    candidate: &ProductRegressionCandidateRun,
) -> Result<String, serde_json::Error> {
    stable_json_digest(
        "psionic.legal_benchmark.product_regression_candidate.v1",
        candidate,
    )
}

pub fn product_regression_gate_report_hash(
    report: &ProductRegressionGateReport,
) -> Result<String, serde_json::Error> {
    stable_json_digest(
        "psionic.legal_benchmark.product_regression_gate_report.v1",
        report,
    )
}

pub fn autopilot4_product_regression_gate_import_hash(
    export: &ProductRegressionAutopilot4GateImport,
) -> Result<String, serde_json::Error> {
    stable_json_digest(
        "psionic.legal_benchmark.product_regression_autopilot4_import.v1",
        export,
    )
}

fn work_orders_for_failures(
    candidate_module_version_id: &str,
    failures: &[ProductRegressionFailure],
) -> Vec<ProductRegressionWorkOrder> {
    let mut grouped = BTreeMap::<AutopilotProductSurface, Vec<String>>::new();
    for failure in failures {
        grouped
            .entry(failure.surface)
            .or_default()
            .push(failure.failure_id.clone());
    }
    grouped
        .into_iter()
        .map(|(surface, blocking_failure_ids)| ProductRegressionWorkOrder {
            work_order_id: format!(
                "wo.product_regression.{}.{}",
                surface.as_str(),
                candidate_module_version_id
            ),
            title: format!("Fix product regression for {}", surface.as_str()),
            severity: ProductRegressionWorkOrderSeverity::Blocker,
            surface,
            blocking_failure_ids,
            candidate_module_version_id: candidate_module_version_id.to_string(),
            reproduction_command: String::from("scripts/check-legal-benchmark-ci.sh"),
            summary: String::from(
                "Benchmark candidate is blocked from Autopilot4 promotion until this product regression is fixed or explicitly waived by a release owner.",
            ),
        })
        .collect()
}

fn diagnostic_or_default(outcome: &ProductRegressionTaskOutcome, fallback: &str) -> String {
    outcome
        .diagnostics
        .first()
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

fn ratio_bps(numerator: u64, denominator: u64) -> u32 {
    if denominator == 0 {
        return 0;
    }
    u32::try_from((numerator * 10_000) / denominator).unwrap_or(0)
}

fn average_bps(sum: u64, count: u64) -> u32 {
    if count == 0 {
        return 0;
    }
    u32::try_from(sum / count).unwrap_or(0)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn suite() -> ProductRegressionSuite {
        serde_json::from_str(include_str!(
            "../../../fixtures/legal_benchmark/product_regression_suite.json"
        ))
        .expect("suite parses")
    }

    fn failing_candidate() -> ProductRegressionCandidateRun {
        serde_json::from_str(include_str!(
            "../../../fixtures/legal_benchmark/product_regression_candidate_failure.json"
        ))
        .expect("candidate parses")
    }

    fn gate_config() -> ProductRegressionGateConfig {
        ProductRegressionGateConfig {
            schema_version: LEGAL_BENCHMARK_PRODUCT_REGRESSION_SCHEMA_VERSION,
            gate_id: String::from("legal_benchmark_product_regression"),
            minimum_suite_score_bps: 9_000,
            minimum_surface_score_bps: 8_500,
            require_all_tasks_pass: true,
            metadata: Metadata::new(),
        }
    }

    #[test]
    fn suite_fixture_covers_required_surfaces_without_private_data() {
        let suite = suite();
        assert_eq!(suite.tasks.len(), 7);
        assert!(validate_product_regression_suite(&suite).is_empty());
        let surfaces = suite
            .tasks
            .iter()
            .map(|task| task.surface)
            .collect::<BTreeSet<_>>();
        for required in REQUIRED_PRODUCT_REGRESSION_SURFACES {
            assert!(surfaces.contains(&required));
        }
        assert!(product_regression_suite_hash(&suite).is_ok());
    }

    #[test]
    fn candidate_regression_blocks_release_gate_and_creates_work_order() {
        let report =
            evaluate_product_regression_gate(&suite(), &gate_config(), &failing_candidate())
                .expect("gate report");

        assert_eq!(report.status, ProductRegressionGateStatus::Blocked);
        assert!(report.autopilot4_release_gate_import.promotion_blocked);
        assert_eq!(
            report
                .autopilot4_release_gate_import
                .benchmark_target_score_bps,
            9_500
        );
        assert_eq!(
            report
                .autopilot4_release_gate_import
                .benchmark_candidate_score_bps,
            9_800
        );
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.failure_kind == ProductRegressionFailureKind::TaskFailed)
        );
        assert!(!report.work_orders.is_empty());
        assert_eq!(
            report.autopilot4_release_gate_import.work_order_ids.len(),
            report.work_orders.len()
        );
        assert!(product_regression_gate_report_hash(&report).is_ok());
        assert!(
            autopilot4_product_regression_gate_import_hash(&report.autopilot4_release_gate_import,)
                .is_ok()
        );
    }

    #[test]
    fn passing_candidate_exports_green_gate() {
        let suite = suite();
        let candidate = ProductRegressionCandidateRun {
            schema_version: LEGAL_BENCHMARK_PRODUCT_REGRESSION_SCHEMA_VERSION,
            candidate_module_version_id: String::from("module.legal_agent_candidate.clean.v1"),
            benchmark_summary: ProductRegressionBenchmarkSummary {
                benchmark_suite_id: String::from("harvey_labs"),
                benchmark_target_score_bps: 9_500,
                benchmark_candidate_score_bps: 9_700,
                previous_benchmark_score_bps: 9_200,
                sweep_matrix_export_ref: Some(String::from("sweep.matrix.clean")),
            },
            outcomes: suite
                .tasks
                .iter()
                .map(|task| ProductRegressionTaskOutcome {
                    task_id: task.task_id.clone(),
                    surface: task.surface,
                    passed: true,
                    score_bps: task.baseline_score_bps,
                    latency_ms: 10,
                    cost_micro_usd: 20,
                    diagnostics: Vec::new(),
                    evidence_refs: vec![format!("evidence.{}", task.task_id)],
                })
                .collect(),
            metadata: Metadata::new(),
        };

        let report = evaluate_product_regression_gate(&suite, &gate_config(), &candidate)
            .expect("gate report");
        assert_eq!(report.status, ProductRegressionGateStatus::Passed);
        assert!(!report.autopilot4_release_gate_import.promotion_blocked);
        assert!(report.failures.is_empty());
        assert!(report.work_orders.is_empty());
        assert_eq!(report.product_summary.pass_rate_bps, 10_000);
    }
}
