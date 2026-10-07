//! Resumable legal benchmark sweep planning and manifests.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{Metadata, stable_json_digest};

pub const LEGAL_BENCHMARK_SWEEP_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkSweepScope {
    TaskIds(Vec<String>),
    PracticeArea(String),
    Workflow(String),
    Slice { name: String, task_ids: Vec<String> },
    All,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepBudget {
    pub max_cost_micro_usd: Option<u64>,
    pub max_wall_time_ms: Option<u64>,
    pub max_tokens: Option<u64>,
    pub max_failures: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepConfig {
    pub schema_version: u16,
    pub sweep_id: String,
    pub scope: LegalBenchmarkSweepScope,
    pub task_ids: Vec<String>,
    pub run_config_hashes: Vec<String>,
    pub max_parallelism: u16,
    pub budget: LegalBenchmarkSweepBudget,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepMatrixConfig {
    pub schema_version: u16,
    pub matrix_id: String,
    pub scope: LegalBenchmarkSweepScope,
    pub task_ids: Vec<String>,
    pub providers: Vec<LegalBenchmarkSweepProviderAxis>,
    pub reasoning_efforts: Vec<LegalBenchmarkSweepNamedAxis>,
    pub context_budgets: Vec<LegalBenchmarkSweepContextBudgetAxis>,
    pub extraction_policies: Vec<LegalBenchmarkSweepNamedAxis>,
    pub tool_policies: Vec<LegalBenchmarkSweepNamedAxis>,
    pub max_parallelism: u16,
    pub budget: LegalBenchmarkSweepBudget,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepProviderAxis {
    pub provider_id: String,
    pub provider_family: String,
    pub model: String,
    pub route_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepNamedAxis {
    pub id: String,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepContextBudgetAxis {
    pub id: String,
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepMatrixRunConfig {
    pub run_config_hash: String,
    pub provider_id: String,
    pub provider_family: String,
    pub model: String,
    pub route_id: String,
    pub reasoning_effort_id: String,
    pub context_budget_id: String,
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
    pub extraction_policy_id: String,
    pub tool_policy_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepMatrixPlan {
    pub schema_version: u16,
    pub matrix_id: String,
    pub sweep_config: LegalBenchmarkSweepConfig,
    pub run_configs: Vec<LegalBenchmarkSweepMatrixRunConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkSweepJobStatus {
    Pending,
    Skipped,
    Resumed,
    Succeeded,
    Failed,
    Blocked,
    BudgetExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepJob {
    pub job_id: String,
    pub task_id: String,
    pub run_config_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepJobRecord {
    pub job_id: String,
    pub task_id: String,
    pub run_config_hash: String,
    pub status: LegalBenchmarkSweepJobStatus,
    pub run_id: Option<String>,
    pub score_report_id: Option<String>,
    pub score_report_hash: Option<String>,
    pub cost_micro_usd: u64,
    pub wall_time_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_pass: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub criterion_pass_rate_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_coverage_bps: Option<u32>,
    pub failure_kind: Option<String>,
    pub failure_detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepResumeState {
    pub schema_version: u16,
    pub sweep_id: String,
    pub completed_jobs: Vec<LegalBenchmarkSweepJobRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepManifest {
    pub schema_version: u16,
    pub sweep_id: String,
    pub config_hash: String,
    pub generated_at_ms: u64,
    pub total_jobs: u64,
    pub skipped_jobs: u64,
    pub resumed_jobs: u64,
    pub succeeded_jobs: u64,
    pub failed_jobs: u64,
    pub blocked_jobs: u64,
    pub budget_exhausted_jobs: u64,
    pub total_cost_micro_usd: u64,
    pub total_wall_time_ms: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub jobs: Vec<LegalBenchmarkSweepJobRecord>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepMatrixConfigSummary {
    pub run_config_hash: String,
    pub total_jobs: u64,
    pub succeeded_jobs: u64,
    pub failed_jobs: u64,
    pub all_pass_rate_bps: u32,
    pub criterion_pass_rate_bps: u32,
    pub document_coverage_bps: u32,
    pub reliability_bps: u32,
    pub total_cost_micro_usd: u64,
    pub avg_wall_time_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub pareto_front: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSweepMatrixExport {
    pub schema_version: u16,
    pub matrix_id: String,
    pub sweep_id: String,
    pub generated_at_ms: u64,
    pub manifest_hash: String,
    pub config_hashes: Vec<String>,
    pub summaries: Vec<LegalBenchmarkSweepMatrixConfigSummary>,
    pub pareto_front_config_hashes: Vec<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegalBenchmarkSweepJobOutcome {
    pub status: LegalBenchmarkSweepJobStatus,
    pub run_id: Option<String>,
    pub score_report_id: Option<String>,
    pub score_report_hash: Option<String>,
    pub cost_micro_usd: u64,
    pub wall_time_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub all_pass: Option<bool>,
    pub criterion_pass_rate_bps: Option<u32>,
    pub document_coverage_bps: Option<u32>,
    pub failure_kind: Option<String>,
    pub failure_detail: Option<String>,
}

pub trait LegalBenchmarkSweepExecutor {
    fn execute(&mut self, job: &LegalBenchmarkSweepJob) -> LegalBenchmarkSweepJobOutcome;
}

#[derive(Clone, Debug, Default)]
pub struct MockLegalBenchmarkSweepExecutor {
    outcomes: BTreeMap<String, LegalBenchmarkSweepJobOutcome>,
}

impl MockLegalBenchmarkSweepExecutor {
    pub fn new(outcomes: BTreeMap<String, LegalBenchmarkSweepJobOutcome>) -> Self {
        Self { outcomes }
    }
}

impl LegalBenchmarkSweepExecutor for MockLegalBenchmarkSweepExecutor {
    fn execute(&mut self, job: &LegalBenchmarkSweepJob) -> LegalBenchmarkSweepJobOutcome {
        self.outcomes
            .remove(&job.job_id)
            .unwrap_or_else(|| LegalBenchmarkSweepJobOutcome {
                status: LegalBenchmarkSweepJobStatus::Succeeded,
                run_id: Some(format!("run.{}", job.job_id)),
                score_report_id: Some(format!("score.{}", job.job_id)),
                score_report_hash: Some(String::from(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                )),
                cost_micro_usd: 1,
                wall_time_ms: 1,
                input_tokens: 1,
                output_tokens: 1,
                all_pass: Some(true),
                criterion_pass_rate_bps: Some(10_000),
                document_coverage_bps: Some(10_000),
                failure_kind: None,
                failure_detail: None,
            })
    }
}

pub fn expand_legal_benchmark_sweep_matrix(
    matrix: &LegalBenchmarkSweepMatrixConfig,
) -> Result<LegalBenchmarkSweepMatrixPlan, serde_json::Error> {
    let mut run_configs = Vec::new();
    for provider in &matrix.providers {
        for reasoning in &matrix.reasoning_efforts {
            for context in &matrix.context_budgets {
                for extraction in &matrix.extraction_policies {
                    for tool_policy in &matrix.tool_policies {
                        let mut run_config = LegalBenchmarkSweepMatrixRunConfig {
                            run_config_hash: String::new(),
                            provider_id: provider.provider_id.clone(),
                            provider_family: provider.provider_family.clone(),
                            model: provider.model.clone(),
                            route_id: provider.route_id.clone(),
                            reasoning_effort_id: reasoning.id.clone(),
                            context_budget_id: context.id.clone(),
                            max_input_tokens: context.max_input_tokens,
                            max_output_tokens: context.max_output_tokens,
                            extraction_policy_id: extraction.id.clone(),
                            tool_policy_id: tool_policy.id.clone(),
                        };
                        run_config.run_config_hash =
                            legal_benchmark_sweep_matrix_run_config_hash(&run_config)?;
                        run_configs.push(run_config);
                    }
                }
            }
        }
    }
    let sweep_config = LegalBenchmarkSweepConfig {
        schema_version: LEGAL_BENCHMARK_SWEEP_SCHEMA_VERSION,
        sweep_id: format!("sweep.{}", matrix.matrix_id),
        scope: matrix.scope.clone(),
        task_ids: matrix.task_ids.clone(),
        run_config_hashes: run_configs
            .iter()
            .map(|config| config.run_config_hash.clone())
            .collect(),
        max_parallelism: matrix.max_parallelism,
        budget: matrix.budget.clone(),
        metadata: matrix.metadata.clone(),
    };
    Ok(LegalBenchmarkSweepMatrixPlan {
        schema_version: LEGAL_BENCHMARK_SWEEP_SCHEMA_VERSION,
        matrix_id: matrix.matrix_id.clone(),
        sweep_config,
        run_configs,
    })
}

pub fn plan_legal_benchmark_sweep_jobs(
    config: &LegalBenchmarkSweepConfig,
) -> Vec<LegalBenchmarkSweepJob> {
    let mut jobs = Vec::new();
    let task_ids = scoped_task_ids(config);
    for task_id in task_ids {
        for run_config_hash in &config.run_config_hashes {
            jobs.push(LegalBenchmarkSweepJob {
                job_id: format!(
                    "job.{}.{}",
                    stable_label(&task_id),
                    stable_label(run_config_hash)
                ),
                task_id: task_id.clone(),
                run_config_hash: run_config_hash.clone(),
            });
        }
    }
    jobs
}

pub fn run_legal_benchmark_sweep<E>(
    config: &LegalBenchmarkSweepConfig,
    resume_state: Option<&LegalBenchmarkSweepResumeState>,
    executor: &mut E,
) -> Result<LegalBenchmarkSweepManifest, serde_json::Error>
where
    E: LegalBenchmarkSweepExecutor,
{
    let config_hash = legal_benchmark_sweep_config_hash(config)?;
    let resume_by_job = resume_state
        .map(|state| {
            state
                .completed_jobs
                .iter()
                .map(|record| (record.job_id.clone(), record.clone()))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut jobs = Vec::new();
    let mut totals = SweepTotals::default();
    let mut exhausted = false;
    for job in plan_legal_benchmark_sweep_jobs(config) {
        if let Some(resumed) = resume_by_job.get(&job.job_id) {
            let mut record = resumed.clone();
            record.status = LegalBenchmarkSweepJobStatus::Resumed;
            totals.add(&record);
            jobs.push(record);
            continue;
        }
        if exhausted || totals.exceeds_before_start(&config.budget) {
            let record = blocked_job_record(&job, LegalBenchmarkSweepJobStatus::BudgetExhausted);
            totals.add(&record);
            jobs.push(record);
            exhausted = true;
            continue;
        }
        let outcome = executor.execute(&job);
        let record = job_record_from_outcome(&job, outcome);
        totals.add(&record);
        if totals.exceeds_after_finish(&config.budget) {
            exhausted = true;
        }
        jobs.push(record);
    }
    Ok(manifest_from_jobs(config, config_hash, jobs, totals))
}

pub fn legal_benchmark_sweep_config_hash(
    config: &LegalBenchmarkSweepConfig,
) -> Result<String, serde_json::Error> {
    stable_json_digest("psionic.legal_benchmark.sweep_config.v1", config)
}

pub fn legal_benchmark_sweep_manifest_hash(
    manifest: &LegalBenchmarkSweepManifest,
) -> Result<String, serde_json::Error> {
    stable_json_digest("psionic.legal_benchmark.sweep_manifest.v1", manifest)
}

pub fn legal_benchmark_sweep_matrix_config_hash(
    matrix: &LegalBenchmarkSweepMatrixConfig,
) -> Result<String, serde_json::Error> {
    stable_json_digest("psionic.legal_benchmark.sweep_matrix_config.v1", matrix)
}

pub fn legal_benchmark_sweep_matrix_run_config_hash(
    run_config: &LegalBenchmarkSweepMatrixRunConfig,
) -> Result<String, serde_json::Error> {
    let mut canonical = run_config.clone();
    canonical.run_config_hash.clear();
    stable_json_digest(
        "psionic.legal_benchmark.sweep_matrix_run_config.v1",
        &canonical,
    )
}

pub fn generate_legal_benchmark_sweep_matrix_export(
    matrix_id: impl Into<String>,
    manifest: &LegalBenchmarkSweepManifest,
) -> Result<LegalBenchmarkSweepMatrixExport, serde_json::Error> {
    let mut summaries = summarize_matrix_configs(manifest);
    mark_pareto_front(summaries.as_mut_slice());
    let pareto_front_config_hashes = summaries
        .iter()
        .filter(|summary| summary.pareto_front)
        .map(|summary| summary.run_config_hash.clone())
        .collect::<Vec<_>>();
    let config_hashes = summaries
        .iter()
        .map(|summary| summary.run_config_hash.clone())
        .collect::<Vec<_>>();
    Ok(LegalBenchmarkSweepMatrixExport {
        schema_version: LEGAL_BENCHMARK_SWEEP_SCHEMA_VERSION,
        matrix_id: matrix_id.into(),
        sweep_id: manifest.sweep_id.clone(),
        generated_at_ms: now_ms(),
        manifest_hash: legal_benchmark_sweep_manifest_hash(manifest)?,
        config_hashes,
        summaries,
        pareto_front_config_hashes,
        metadata: Metadata::new(),
    })
}

fn scoped_task_ids(config: &LegalBenchmarkSweepConfig) -> Vec<String> {
    match &config.scope {
        LegalBenchmarkSweepScope::TaskIds(task_ids) => task_ids.clone(),
        LegalBenchmarkSweepScope::Slice { task_ids, .. } => task_ids.clone(),
        _ => config.task_ids.clone(),
    }
}

fn job_record_from_outcome(
    job: &LegalBenchmarkSweepJob,
    outcome: LegalBenchmarkSweepJobOutcome,
) -> LegalBenchmarkSweepJobRecord {
    LegalBenchmarkSweepJobRecord {
        job_id: job.job_id.clone(),
        task_id: job.task_id.clone(),
        run_config_hash: job.run_config_hash.clone(),
        status: outcome.status,
        run_id: outcome.run_id,
        score_report_id: outcome.score_report_id,
        score_report_hash: outcome.score_report_hash,
        cost_micro_usd: outcome.cost_micro_usd,
        wall_time_ms: outcome.wall_time_ms,
        input_tokens: outcome.input_tokens,
        output_tokens: outcome.output_tokens,
        all_pass: outcome.all_pass,
        criterion_pass_rate_bps: outcome.criterion_pass_rate_bps,
        document_coverage_bps: outcome.document_coverage_bps,
        failure_kind: outcome.failure_kind,
        failure_detail: outcome.failure_detail,
    }
}

fn blocked_job_record(
    job: &LegalBenchmarkSweepJob,
    status: LegalBenchmarkSweepJobStatus,
) -> LegalBenchmarkSweepJobRecord {
    LegalBenchmarkSweepJobRecord {
        job_id: job.job_id.clone(),
        task_id: job.task_id.clone(),
        run_config_hash: job.run_config_hash.clone(),
        status,
        run_id: None,
        score_report_id: None,
        score_report_hash: None,
        cost_micro_usd: 0,
        wall_time_ms: 0,
        input_tokens: 0,
        output_tokens: 0,
        all_pass: None,
        criterion_pass_rate_bps: None,
        document_coverage_bps: None,
        failure_kind: Some(String::from("budget")),
        failure_detail: Some(String::from("budget exhausted before starting job")),
    }
}

fn manifest_from_jobs(
    config: &LegalBenchmarkSweepConfig,
    config_hash: String,
    jobs: Vec<LegalBenchmarkSweepJobRecord>,
    totals: SweepTotals,
) -> LegalBenchmarkSweepManifest {
    LegalBenchmarkSweepManifest {
        schema_version: LEGAL_BENCHMARK_SWEEP_SCHEMA_VERSION,
        sweep_id: config.sweep_id.clone(),
        config_hash,
        generated_at_ms: now_ms(),
        total_jobs: u64::try_from(jobs.len()).unwrap_or(u64::MAX),
        skipped_jobs: totals.status_count(LegalBenchmarkSweepJobStatus::Skipped),
        resumed_jobs: totals.status_count(LegalBenchmarkSweepJobStatus::Resumed),
        succeeded_jobs: totals.status_count(LegalBenchmarkSweepJobStatus::Succeeded),
        failed_jobs: totals.status_count(LegalBenchmarkSweepJobStatus::Failed),
        blocked_jobs: totals.status_count(LegalBenchmarkSweepJobStatus::Blocked),
        budget_exhausted_jobs: totals.status_count(LegalBenchmarkSweepJobStatus::BudgetExhausted),
        total_cost_micro_usd: totals.cost_micro_usd,
        total_wall_time_ms: totals.wall_time_ms,
        total_input_tokens: totals.input_tokens,
        total_output_tokens: totals.output_tokens,
        jobs,
        metadata: Metadata::new(),
    }
}

fn summarize_matrix_configs(
    manifest: &LegalBenchmarkSweepManifest,
) -> Vec<LegalBenchmarkSweepMatrixConfigSummary> {
    let mut grouped = BTreeMap::<String, Vec<&LegalBenchmarkSweepJobRecord>>::new();
    for job in &manifest.jobs {
        grouped
            .entry(job.run_config_hash.clone())
            .or_default()
            .push(job);
    }
    grouped
        .into_iter()
        .map(|(run_config_hash, jobs)| {
            let total_jobs = u64::try_from(jobs.len()).unwrap_or(u64::MAX);
            let succeeded_jobs = u64::try_from(
                jobs.iter()
                    .filter(|job| job.status == LegalBenchmarkSweepJobStatus::Succeeded)
                    .count(),
            )
            .unwrap_or(u64::MAX);
            let failed_jobs = u64::try_from(
                jobs.iter()
                    .filter(|job| job.status == LegalBenchmarkSweepJobStatus::Failed)
                    .count(),
            )
            .unwrap_or(u64::MAX);
            let all_pass_count =
                u64::try_from(jobs.iter().filter(|job| job.all_pass == Some(true)).count())
                    .unwrap_or(u64::MAX);
            let criterion_sum = jobs
                .iter()
                .filter_map(|job| job.criterion_pass_rate_bps)
                .map(u64::from)
                .sum::<u64>();
            let document_sum = jobs
                .iter()
                .filter_map(|job| job.document_coverage_bps)
                .map(u64::from)
                .sum::<u64>();
            let scored_count = u64::try_from(
                jobs.iter()
                    .filter(|job| job.criterion_pass_rate_bps.is_some())
                    .count(),
            )
            .unwrap_or(0);
            let total_cost_micro_usd = jobs.iter().map(|job| job.cost_micro_usd).sum();
            let wall_time_sum = jobs.iter().map(|job| job.wall_time_ms).sum::<u64>();
            LegalBenchmarkSweepMatrixConfigSummary {
                run_config_hash,
                total_jobs,
                succeeded_jobs,
                failed_jobs,
                all_pass_rate_bps: ratio_bps(all_pass_count, total_jobs),
                criterion_pass_rate_bps: average_bps(criterion_sum, scored_count),
                document_coverage_bps: average_bps(document_sum, scored_count),
                reliability_bps: ratio_bps(succeeded_jobs, total_jobs),
                total_cost_micro_usd,
                avg_wall_time_ms: if total_jobs == 0 {
                    0
                } else {
                    wall_time_sum / total_jobs
                },
                input_tokens: jobs.iter().map(|job| job.input_tokens).sum(),
                output_tokens: jobs.iter().map(|job| job.output_tokens).sum(),
                pareto_front: false,
            }
        })
        .collect()
}

fn mark_pareto_front(summaries: &mut [LegalBenchmarkSweepMatrixConfigSummary]) {
    let snapshot = summaries.to_vec();
    for summary in summaries {
        summary.pareto_front = !snapshot.iter().any(|other| {
            other.run_config_hash != summary.run_config_hash && dominates(other, summary)
        });
    }
}

fn dominates(
    candidate: &LegalBenchmarkSweepMatrixConfigSummary,
    incumbent: &LegalBenchmarkSweepMatrixConfigSummary,
) -> bool {
    let at_least_as_good = candidate.all_pass_rate_bps >= incumbent.all_pass_rate_bps
        && candidate.criterion_pass_rate_bps >= incumbent.criterion_pass_rate_bps
        && candidate.reliability_bps >= incumbent.reliability_bps
        && candidate.total_cost_micro_usd <= incumbent.total_cost_micro_usd
        && candidate.avg_wall_time_ms <= incumbent.avg_wall_time_ms;
    let strictly_better = candidate.all_pass_rate_bps > incumbent.all_pass_rate_bps
        || candidate.criterion_pass_rate_bps > incumbent.criterion_pass_rate_bps
        || candidate.reliability_bps > incumbent.reliability_bps
        || candidate.total_cost_micro_usd < incumbent.total_cost_micro_usd
        || candidate.avg_wall_time_ms < incumbent.avg_wall_time_ms;
    at_least_as_good && strictly_better
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

#[derive(Default)]
struct SweepTotals {
    cost_micro_usd: u64,
    wall_time_ms: u64,
    input_tokens: u64,
    output_tokens: u64,
    failures: u64,
    statuses: BTreeMap<LegalBenchmarkSweepJobStatus, u64>,
}

impl SweepTotals {
    fn add(&mut self, record: &LegalBenchmarkSweepJobRecord) {
        self.cost_micro_usd = self.cost_micro_usd.saturating_add(record.cost_micro_usd);
        self.wall_time_ms = self.wall_time_ms.saturating_add(record.wall_time_ms);
        self.input_tokens = self.input_tokens.saturating_add(record.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(record.output_tokens);
        if record.status == LegalBenchmarkSweepJobStatus::Failed {
            self.failures = self.failures.saturating_add(1);
        }
        *self.statuses.entry(record.status).or_default() += 1;
    }

    fn status_count(&self, status: LegalBenchmarkSweepJobStatus) -> u64 {
        self.statuses.get(&status).copied().unwrap_or(0)
    }

    fn exceeds_before_start(&self, budget: &LegalBenchmarkSweepBudget) -> bool {
        budget.max_failures.is_some_and(|max| self.failures >= max)
            || budget
                .max_cost_micro_usd
                .is_some_and(|max| self.cost_micro_usd >= max)
            || budget
                .max_wall_time_ms
                .is_some_and(|max| self.wall_time_ms >= max)
            || budget
                .max_tokens
                .is_some_and(|max| self.input_tokens.saturating_add(self.output_tokens) >= max)
    }

    fn exceeds_after_finish(&self, budget: &LegalBenchmarkSweepBudget) -> bool {
        self.exceeds_before_start(budget)
    }
}

fn stable_label(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '.'
            }
        })
        .collect()
}

fn now_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
        Err(_) => 0,
    }
}

impl Ord for LegalBenchmarkSweepJobStatus {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl PartialOrd for LegalBenchmarkSweepJobStatus {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> LegalBenchmarkSweepConfig {
        LegalBenchmarkSweepConfig {
            schema_version: LEGAL_BENCHMARK_SWEEP_SCHEMA_VERSION,
            sweep_id: String::from("sweep.mock"),
            scope: LegalBenchmarkSweepScope::Slice {
                name: String::from("smoke"),
                task_ids: vec![String::from("task.a"), String::from("task.b")],
            },
            task_ids: Vec::new(),
            run_config_hashes: vec![String::from("config.1"), String::from("config.2")],
            max_parallelism: 2,
            budget: LegalBenchmarkSweepBudget {
                max_cost_micro_usd: Some(100),
                max_wall_time_ms: Some(1000),
                max_tokens: Some(1000),
                max_failures: Some(2),
            },
            metadata: Metadata::new(),
        }
    }

    fn matrix_config() -> LegalBenchmarkSweepMatrixConfig {
        LegalBenchmarkSweepMatrixConfig {
            schema_version: LEGAL_BENCHMARK_SWEEP_SCHEMA_VERSION,
            matrix_id: String::from("matrix.mock"),
            scope: LegalBenchmarkSweepScope::Slice {
                name: String::from("smoke"),
                task_ids: vec![String::from("task.a")],
            },
            task_ids: Vec::new(),
            providers: vec![
                LegalBenchmarkSweepProviderAxis {
                    provider_id: String::from("gemini3_flash"),
                    provider_family: String::from("google_vertex_gemini"),
                    model: String::from("gemini-3-flash-preview"),
                    route_id: String::from("google.gemini3.flash.primary"),
                },
                LegalBenchmarkSweepProviderAxis {
                    provider_id: String::from("local"),
                    provider_family: String::from("psionic_compatible"),
                    model: String::from("local-rust"),
                    route_id: String::from("route.local"),
                },
            ],
            reasoning_efforts: vec![
                LegalBenchmarkSweepNamedAxis {
                    id: String::from("standard"),
                    metadata: Metadata::new(),
                },
                LegalBenchmarkSweepNamedAxis {
                    id: String::from("deep"),
                    metadata: Metadata::new(),
                },
            ],
            context_budgets: vec![LegalBenchmarkSweepContextBudgetAxis {
                id: String::from("32k"),
                max_input_tokens: 32_000,
                max_output_tokens: 4_000,
            }],
            extraction_policies: vec![LegalBenchmarkSweepNamedAxis {
                id: String::from("native-first"),
                metadata: Metadata::new(),
            }],
            tool_policies: vec![
                LegalBenchmarkSweepNamedAxis {
                    id: String::from("basic"),
                    metadata: Metadata::new(),
                },
                LegalBenchmarkSweepNamedAxis {
                    id: String::from("document-tools"),
                    metadata: Metadata::new(),
                },
            ],
            max_parallelism: 2,
            budget: LegalBenchmarkSweepBudget {
                max_cost_micro_usd: Some(1_000),
                max_wall_time_ms: Some(10_000),
                max_tokens: Some(100_000),
                max_failures: Some(3),
            },
            metadata: Metadata::new(),
        }
    }

    #[test]
    fn plans_cross_product_jobs_for_slice_scope() {
        let jobs = plan_legal_benchmark_sweep_jobs(&config());
        assert_eq!(jobs.len(), 4);
        assert_eq!(jobs[0].job_id, "job.task.a.config.1");
        assert_eq!(jobs[3].job_id, "job.task.b.config.2");
    }

    #[test]
    fn sweep_resumes_completed_jobs_and_keeps_running_failures() {
        let cfg = config();
        let resumed = LegalBenchmarkSweepJobRecord {
            job_id: String::from("job.task.a.config.1"),
            task_id: String::from("task.a"),
            run_config_hash: String::from("config.1"),
            status: LegalBenchmarkSweepJobStatus::Succeeded,
            run_id: Some(String::from("run.old")),
            score_report_id: Some(String::from("score.old")),
            score_report_hash: Some(String::from("hash.old")),
            cost_micro_usd: 3,
            wall_time_ms: 4,
            input_tokens: 5,
            output_tokens: 6,
            all_pass: Some(true),
            criterion_pass_rate_bps: Some(10_000),
            document_coverage_bps: Some(10_000),
            failure_kind: None,
            failure_detail: None,
        };
        let mut outcomes = BTreeMap::new();
        outcomes.insert(
            String::from("job.task.a.config.2"),
            LegalBenchmarkSweepJobOutcome {
                status: LegalBenchmarkSweepJobStatus::Failed,
                run_id: Some(String::from("run.failed")),
                score_report_id: None,
                score_report_hash: None,
                cost_micro_usd: 2,
                wall_time_ms: 3,
                input_tokens: 4,
                output_tokens: 5,
                all_pass: Some(false),
                criterion_pass_rate_bps: Some(5_000),
                document_coverage_bps: Some(8_000),
                failure_kind: Some(String::from("provider_failure")),
                failure_detail: Some(String::from("mock failure")),
            },
        );
        let resume = LegalBenchmarkSweepResumeState {
            schema_version: LEGAL_BENCHMARK_SWEEP_SCHEMA_VERSION,
            sweep_id: cfg.sweep_id.clone(),
            completed_jobs: vec![resumed],
        };
        let mut executor = MockLegalBenchmarkSweepExecutor::new(outcomes);
        let manifest =
            run_legal_benchmark_sweep(&cfg, Some(&resume), &mut executor).expect("sweep");
        assert_eq!(manifest.total_jobs, 4);
        assert_eq!(manifest.resumed_jobs, 1);
        assert_eq!(manifest.failed_jobs, 1);
        assert_eq!(manifest.succeeded_jobs, 2);
        assert_eq!(manifest.budget_exhausted_jobs, 0);
        assert!(legal_benchmark_sweep_manifest_hash(&manifest).is_ok());
    }

    #[test]
    fn budget_exhaustion_blocks_new_work() {
        let mut cfg = config();
        cfg.budget.max_cost_micro_usd = Some(1);
        let mut executor = MockLegalBenchmarkSweepExecutor::default();
        let manifest = run_legal_benchmark_sweep(&cfg, None, &mut executor).expect("sweep");
        assert_eq!(manifest.succeeded_jobs, 1);
        assert!(manifest.budget_exhausted_jobs >= 1);
    }

    #[test]
    fn matrix_expands_provider_reasoning_context_extraction_and_tool_axes() {
        let plan = expand_legal_benchmark_sweep_matrix(&matrix_config()).expect("matrix plan");
        assert_eq!(plan.run_configs.len(), 8);
        assert_eq!(plan.sweep_config.run_config_hashes.len(), 8);
        assert!(
            plan.run_configs
                .iter()
                .any(|config| config.tool_policy_id == "document-tools")
        );
        assert!(legal_benchmark_sweep_matrix_config_hash(&matrix_config()).is_ok());
    }

    #[test]
    fn matrix_smoke_fixture_expands_to_recorded_config_hashes() {
        let matrix: LegalBenchmarkSweepMatrixConfig = serde_json::from_str(include_str!(
            "../../../fixtures/legal_benchmark/sweep_matrix_smoke_config.json"
        ))
        .expect("matrix fixture parses");
        let plan = expand_legal_benchmark_sweep_matrix(&matrix).expect("matrix plan");
        assert_eq!(plan.run_configs.len(), 8);
        assert!(
            plan.sweep_config
                .run_config_hashes
                .iter()
                .all(|hash| hash.len() == 64)
        );
    }

    #[test]
    fn matrix_export_marks_pareto_front_configs() {
        let manifest = LegalBenchmarkSweepManifest {
            schema_version: LEGAL_BENCHMARK_SWEEP_SCHEMA_VERSION,
            sweep_id: String::from("sweep.matrix.mock"),
            config_hash: String::from("matrix-hash"),
            generated_at_ms: 1,
            total_jobs: 6,
            skipped_jobs: 0,
            resumed_jobs: 0,
            succeeded_jobs: 6,
            failed_jobs: 0,
            blocked_jobs: 0,
            budget_exhausted_jobs: 0,
            total_cost_micro_usd: 0,
            total_wall_time_ms: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            jobs: vec![
                scored_job("config.fast", "task.a", false, 9_000, 10, 10),
                scored_job("config.fast", "task.b", false, 9_000, 10, 10),
                scored_job("config.slow", "task.a", false, 9_000, 20, 20),
                scored_job("config.slow", "task.b", false, 9_000, 20, 20),
                scored_job("config.accurate", "task.a", true, 10_000, 40, 30),
                scored_job("config.accurate", "task.b", true, 10_000, 40, 30),
            ],
            metadata: Metadata::new(),
        };
        let export =
            generate_legal_benchmark_sweep_matrix_export("matrix.mock", &manifest).expect("export");
        assert_eq!(export.config_hashes.len(), 3);
        assert!(
            export
                .pareto_front_config_hashes
                .contains(&String::from("config.fast"))
        );
        assert!(
            export
                .pareto_front_config_hashes
                .contains(&String::from("config.accurate"))
        );
        assert!(
            !export
                .pareto_front_config_hashes
                .contains(&String::from("config.slow"))
        );
    }

    fn scored_job(
        run_config_hash: &str,
        task_id: &str,
        all_pass: bool,
        criterion_pass_rate_bps: u32,
        cost_micro_usd: u64,
        wall_time_ms: u64,
    ) -> LegalBenchmarkSweepJobRecord {
        LegalBenchmarkSweepJobRecord {
            job_id: format!("job.{task_id}.{run_config_hash}"),
            task_id: task_id.to_string(),
            run_config_hash: run_config_hash.to_string(),
            status: LegalBenchmarkSweepJobStatus::Succeeded,
            run_id: Some(format!("run.{task_id}.{run_config_hash}")),
            score_report_id: Some(format!("score.{task_id}.{run_config_hash}")),
            score_report_hash: Some(String::from(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )),
            cost_micro_usd,
            wall_time_ms,
            input_tokens: 100,
            output_tokens: 20,
            all_pass: Some(all_pass),
            criterion_pass_rate_bps: Some(criterion_pass_rate_bps),
            document_coverage_bps: Some(10_000),
            failure_kind: None,
            failure_detail: None,
        }
    }
}
