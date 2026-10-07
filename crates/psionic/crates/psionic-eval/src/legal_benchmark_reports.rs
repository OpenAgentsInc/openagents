//! Static report and import-summary generation for legal benchmark scores.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{
    ArtifactManifest, ComparisonReport, CriterionCoverageFailure, CriterionResult,
    CriterionVerdict, Metadata, RunRecord, ScoreReport, comparison_report_digest,
    score_report_digest, stable_json_digest,
};

pub const LEGAL_BENCHMARK_REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkReportInput {
    pub report_id: String,
    pub score_reports: Vec<ScoreReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub run_records: Vec<RunRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_manifests: Vec<ArtifactManifest>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkReportAggregate {
    pub run_count: u64,
    pub all_pass_count: u64,
    pub all_pass_rate_bps: u32,
    pub criterion_pass_rate_bps: u32,
    pub total_cost_micro_usd: u64,
    pub total_wall_time_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub document_coverage_bps: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkTaskSummary {
    pub task_id: String,
    pub run_count: u64,
    pub best_all_pass: bool,
    pub best_criterion_pass_rate_bps: u32,
    pub latest_score_report_hash: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkModelConfigSummary {
    pub run_config_hash: String,
    pub run_count: u64,
    pub all_pass_rate_bps: u32,
    pub criterion_pass_rate_bps: u32,
    pub total_cost_micro_usd: u64,
    pub total_wall_time_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkFailureCluster {
    pub cluster_id: String,
    pub failure_family: String,
    pub task_ids: Vec<String>,
    pub criterion_ids: Vec<String>,
    pub failure_count: u64,
    pub score_delta_bps: i32,
    pub repro_command: String,
    pub affected_modules: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkAutopilotReportExport {
    pub schema_version: u16,
    pub report_id: String,
    pub generated_at_ms: u64,
    pub global: LegalBenchmarkReportAggregate,
    pub by_task: Vec<LegalBenchmarkTaskSummary>,
    pub by_model_config: Vec<LegalBenchmarkModelConfigSummary>,
    pub failure_clusters: Vec<LegalBenchmarkFailureCluster>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coverage_failure_comparisons: Vec<CriterionCoverageFailure>,
    pub score_report_hashes: Vec<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkStaticReport {
    pub markdown: String,
    pub autopilot_export: LegalBenchmarkAutopilotReportExport,
    pub comparison_reports: Vec<ComparisonReport>,
}

pub fn generate_legal_benchmark_static_report(
    input: &LegalBenchmarkReportInput,
) -> Result<LegalBenchmarkStaticReport, serde_json::Error> {
    let global = aggregate_scores(&input.score_reports);
    let by_task = summarize_by_task(&input.score_reports)?;
    let by_model_config = summarize_by_model_config(&input.score_reports, &input.run_records);
    let failure_clusters = failure_clusters(&input.score_reports);
    let coverage_failure_comparisons = input
        .score_reports
        .iter()
        .flat_map(|report| report.failure_comparisons.iter().cloned())
        .collect::<Vec<_>>();
    let score_report_hashes = input
        .score_reports
        .iter()
        .map(score_report_digest)
        .collect::<Result<Vec<_>, _>>()?;
    let comparison_reports = comparison_reports(&input.report_id, &input.score_reports)?;
    let export = LegalBenchmarkAutopilotReportExport {
        schema_version: LEGAL_BENCHMARK_REPORT_SCHEMA_VERSION,
        report_id: input.report_id.clone(),
        generated_at_ms: now_ms(),
        global,
        by_task,
        by_model_config,
        failure_clusters,
        coverage_failure_comparisons,
        score_report_hashes,
        metadata: Metadata::new(),
    };
    let markdown = render_markdown_report(input, &export, &comparison_reports)?;
    Ok(LegalBenchmarkStaticReport {
        markdown,
        autopilot_export: export,
        comparison_reports,
    })
}

pub fn legal_benchmark_report_export_hash(
    export: &LegalBenchmarkAutopilotReportExport,
) -> Result<String, serde_json::Error> {
    stable_json_digest("psionic.legal_benchmark.report_export.v1", export)
}

fn aggregate_scores(score_reports: &[ScoreReport]) -> LegalBenchmarkReportAggregate {
    let run_count = u64::try_from(score_reports.len()).unwrap_or(u64::MAX);
    let all_pass_count = u64::try_from(
        score_reports
            .iter()
            .filter(|report| report.all_pass)
            .count(),
    )
    .unwrap_or(u64::MAX);
    let criterion_sum = score_reports
        .iter()
        .map(|report| u64::from(report.criterion_pass_rate_bps))
        .sum::<u64>();
    let coverage_sum = score_reports
        .iter()
        .map(|report| u64::from(report.document_coverage_bps))
        .sum::<u64>();
    let total_cost_micro_usd = score_reports
        .iter()
        .map(|report| report.metrics.estimated_cost_micro_usd)
        .sum();
    let total_wall_time_ms = score_reports
        .iter()
        .map(|report| report.metrics.wall_time_ms)
        .sum();
    let input_tokens = score_reports
        .iter()
        .map(|report| report.metrics.input_tokens)
        .sum();
    let output_tokens = score_reports
        .iter()
        .map(|report| report.metrics.output_tokens)
        .sum();
    LegalBenchmarkReportAggregate {
        run_count,
        all_pass_count,
        all_pass_rate_bps: ratio_bps(all_pass_count, run_count),
        criterion_pass_rate_bps: average_bps(criterion_sum, run_count),
        total_cost_micro_usd,
        total_wall_time_ms,
        input_tokens,
        output_tokens,
        document_coverage_bps: average_bps(coverage_sum, run_count),
    }
}

fn summarize_by_task(
    score_reports: &[ScoreReport],
) -> Result<Vec<LegalBenchmarkTaskSummary>, serde_json::Error> {
    let mut by_task = BTreeMap::<String, Vec<&ScoreReport>>::new();
    for report in score_reports {
        by_task
            .entry(report.task_id.clone())
            .or_default()
            .push(report);
    }
    let mut summaries = Vec::new();
    for (task_id, reports) in by_task {
        let Some(latest) = reports.last() else {
            continue;
        };
        let best_criterion_pass_rate_bps = reports
            .iter()
            .map(|report| report.criterion_pass_rate_bps)
            .max()
            .unwrap_or(0);
        summaries.push(LegalBenchmarkTaskSummary {
            task_id,
            run_count: u64::try_from(reports.len()).unwrap_or(u64::MAX),
            best_all_pass: reports.iter().any(|report| report.all_pass),
            best_criterion_pass_rate_bps,
            latest_score_report_hash: score_report_digest(latest)?,
        });
    }
    Ok(summaries)
}

fn summarize_by_model_config(
    score_reports: &[ScoreReport],
    run_records: &[RunRecord],
) -> Vec<LegalBenchmarkModelConfigSummary> {
    let run_config_by_run = run_records
        .iter()
        .map(|record| (record.run_id.clone(), record.run_config_hash.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut grouped = BTreeMap::<String, Vec<&ScoreReport>>::new();
    for report in score_reports {
        let key = run_config_by_run
            .get(&report.run_id)
            .cloned()
            .unwrap_or_else(|| report.run_record_hash.clone());
        grouped.entry(key).or_default().push(report);
    }
    grouped
        .into_iter()
        .map(|(run_config_hash, reports)| {
            let run_count = u64::try_from(reports.len()).unwrap_or(u64::MAX);
            let all_pass_count =
                u64::try_from(reports.iter().filter(|report| report.all_pass).count())
                    .unwrap_or(u64::MAX);
            let criterion_sum = reports
                .iter()
                .map(|report| u64::from(report.criterion_pass_rate_bps))
                .sum();
            LegalBenchmarkModelConfigSummary {
                run_config_hash,
                run_count,
                all_pass_rate_bps: ratio_bps(all_pass_count, run_count),
                criterion_pass_rate_bps: average_bps(criterion_sum, run_count),
                total_cost_micro_usd: reports
                    .iter()
                    .map(|report| report.metrics.estimated_cost_micro_usd)
                    .sum(),
                total_wall_time_ms: reports
                    .iter()
                    .map(|report| report.metrics.wall_time_ms)
                    .sum(),
            }
        })
        .collect()
}

fn failure_clusters(score_reports: &[ScoreReport]) -> Vec<LegalBenchmarkFailureCluster> {
    let mut grouped = BTreeMap::<String, Vec<(&ScoreReport, &CriterionResult)>>::new();
    for report in score_reports {
        for criterion in &report.criterion_results {
            if criterion.verdict == CriterionVerdict::Pass {
                continue;
            }
            grouped
                .entry(failure_family(criterion))
                .or_default()
                .push((report, criterion));
        }
    }
    grouped
        .into_iter()
        .map(|(failure_family, entries)| {
            let task_ids = entries
                .iter()
                .map(|(report, _)| report.task_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let criterion_ids = entries
                .iter()
                .map(|(_, criterion)| criterion.criterion_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let diagnostics = entries
                .iter()
                .map(|(_, criterion)| criterion.reasoning.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            LegalBenchmarkFailureCluster {
                cluster_id: format!("failure.{}", stable_label(&failure_family)),
                failure_family,
                task_ids,
                criterion_ids,
                failure_count: u64::try_from(entries.len()).unwrap_or(u64::MAX),
                score_delta_bps: -10_000,
                repro_command: String::from(
                    "cargo test -p psionic-eval --no-default-features --lib legal_benchmark_reports",
                ),
                affected_modules: vec![String::from("psionic-eval/legal_benchmark")],
                diagnostics,
            }
        })
        .collect()
}

fn comparison_reports(
    report_id: &str,
    score_reports: &[ScoreReport],
) -> Result<Vec<ComparisonReport>, serde_json::Error> {
    let mut reports = Vec::new();
    for window in score_reports.windows(2) {
        let baseline = &window[0];
        let candidate = &window[1];
        let baseline_hash = score_report_digest(baseline)?;
        let candidate_hash = score_report_digest(candidate)?;
        let comparison = ComparisonReport {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            comparison_report_id: format!(
                "comparison.{report_id}.{}.{}",
                stable_label(&baseline.run_id),
                stable_label(&candidate.run_id)
            ),
            comparison_scope: report_id.to_owned(),
            baseline_score_report_hash: baseline_hash,
            candidate_score_report_hash: candidate_hash,
            all_pass_delta_bps: all_pass_bps(candidate) - all_pass_bps(baseline),
            criterion_pass_rate_delta_bps: i32::try_from(candidate.criterion_pass_rate_bps)
                .unwrap_or(0)
                - i32::try_from(baseline.criterion_pass_rate_bps).unwrap_or(0),
            estimated_cost_delta_micro_usd: i64::try_from(
                candidate.metrics.estimated_cost_micro_usd,
            )
            .unwrap_or(i64::MAX)
                - i64::try_from(baseline.metrics.estimated_cost_micro_usd).unwrap_or(i64::MAX),
            wall_time_delta_ms: i64::try_from(candidate.metrics.wall_time_ms).unwrap_or(i64::MAX)
                - i64::try_from(baseline.metrics.wall_time_ms).unwrap_or(i64::MAX),
            metadata: Metadata::new(),
        };
        let _ = comparison_report_digest(&comparison)?;
        reports.push(comparison);
    }
    Ok(reports)
}

fn render_markdown_report(
    input: &LegalBenchmarkReportInput,
    export: &LegalBenchmarkAutopilotReportExport,
    comparison_reports: &[ComparisonReport],
) -> Result<String, serde_json::Error> {
    let mut markdown = String::new();
    markdown.push_str(&format!(
        "# Legal Benchmark Report: {}\n\n",
        input.report_id
    ));
    markdown.push_str("## Global\n\n");
    markdown.push_str(&format!(
        "- runs: {}\n- all-pass rate: {} bps\n- criterion pass rate: {} bps\n- document coverage: {} bps\n- cost: {} micro-usd\n- wall time: {} ms\n- tokens: {} input / {} output\n\n",
        export.global.run_count,
        export.global.all_pass_rate_bps,
        export.global.criterion_pass_rate_bps,
        export.global.document_coverage_bps,
        export.global.total_cost_micro_usd,
        export.global.total_wall_time_ms,
        export.global.input_tokens,
        export.global.output_tokens,
    ));
    markdown.push_str("## Runs\n\n");
    for report in &input.score_reports {
        markdown.push_str(&format!(
            "### {} / {}\n\n- all pass: {}\n- criterion pass rate: {} bps\n- document coverage: {} bps\n- run hash: {}\n- output manifest hash: {}\n- cost: {} micro-usd\n- wall time: {} ms\n\n",
            report.task_id,
            report.run_id,
            report.all_pass,
            report.criterion_pass_rate_bps,
            report.document_coverage_bps,
            report.run_record_hash,
            report.output_artifact_manifest_hash,
            report.metrics.estimated_cost_micro_usd,
            report.metrics.wall_time_ms,
        ));
        let missed = report
            .criterion_results
            .iter()
            .filter(|criterion| criterion.verdict != CriterionVerdict::Pass)
            .collect::<Vec<_>>();
        if missed.is_empty() {
            markdown.push_str("All criteria passed.\n\n");
        } else {
            markdown.push_str("Missed criteria:\n\n");
            for criterion in missed {
                let coverage_class = report
                    .failure_comparisons
                    .iter()
                    .find(|comparison| comparison.criterion_id == criterion.criterion_id)
                    .map(|comparison| format!("; coverage={:?}", comparison.failure_class))
                    .unwrap_or_default();
                markdown.push_str(&format!(
                    "- {}: {:?}{}; {}\n",
                    criterion.criterion_id, criterion.verdict, coverage_class, criterion.reasoning
                ));
            }
            markdown.push('\n');
        }
    }
    markdown.push_str("## Failure Clusters\n\n");
    if export.failure_clusters.is_empty() {
        markdown.push_str("No failure clusters.\n\n");
    } else {
        for cluster in &export.failure_clusters {
            markdown.push_str(&format!(
                "- {}: {} failures across {} tasks; repro `{}`\n",
                cluster.failure_family,
                cluster.failure_count,
                cluster.task_ids.len(),
                cluster.repro_command
            ));
        }
        markdown.push('\n');
    }
    markdown.push_str("## Comparisons\n\n");
    if comparison_reports.is_empty() {
        markdown.push_str("No pairwise comparisons.\n");
    } else {
        for comparison in comparison_reports {
            markdown.push_str(&format!(
                "- {}: all-pass delta {} bps, criterion delta {} bps, cost delta {} micro-usd, wall-time delta {} ms\n",
                comparison.comparison_report_id,
                comparison.all_pass_delta_bps,
                comparison.criterion_pass_rate_delta_bps,
                comparison.estimated_cost_delta_micro_usd,
                comparison.wall_time_delta_ms
            ));
        }
    }
    let export_hash = legal_benchmark_report_export_hash(export)?;
    markdown.push_str(&format!("\n\nExport hash: `{export_hash}`\n"));
    Ok(markdown)
}

fn failure_family(criterion: &CriterionResult) -> String {
    match criterion.verdict {
        CriterionVerdict::Fail => {
            if criterion.judge_model == "deterministic_precheck" {
                String::from("deterministic_precheck")
            } else {
                String::from("judge_fail")
            }
        }
        CriterionVerdict::Ambiguous => String::from("judge_ambiguous"),
        CriterionVerdict::NotEvaluated => String::from("not_evaluated"),
        CriterionVerdict::Pass => String::from("pass"),
    }
}

fn ratio_bps(numerator: u64, denominator: u64) -> u32 {
    if denominator == 0 {
        return 0;
    }
    u32::try_from((numerator * 10_000) / denominator).unwrap_or(0)
}

fn average_bps(sum: u64, count: u64) -> u32 {
    if count == 0 {
        0
    } else {
        u32::try_from(sum / count).unwrap_or(0)
    }
}

fn all_pass_bps(report: &ScoreReport) -> i32 {
    if report.all_pass { 10_000 } else { 0 }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CriterionCoverageFailure, CriterionFailureClass, CriterionResult, RunMetrics};

    fn fixture_score_report() -> ScoreReport {
        serde_json::from_str(include_str!(
            "../../../fixtures/legal_benchmark/evaluator_mock_score_report.json"
        ))
        .expect("score fixture parses")
    }

    #[test]
    fn report_export_summarizes_fixture_score_report() {
        let score = fixture_score_report();
        let input = LegalBenchmarkReportInput {
            report_id: String::from("report.mock"),
            score_reports: vec![score],
            run_records: Vec::new(),
            output_manifests: Vec::new(),
        };
        let report = generate_legal_benchmark_static_report(&input).expect("report");

        assert!(report.markdown.contains("Legal Benchmark Report"));
        assert_eq!(report.autopilot_export.global.run_count, 1);
        assert_eq!(report.autopilot_export.global.all_pass_rate_bps, 10_000);
        assert_eq!(report.autopilot_export.by_task.len(), 1);
        assert!(report.autopilot_export.failure_clusters.is_empty());
        assert_eq!(report.autopilot_export.score_report_hashes.len(), 1);
        assert!(legal_benchmark_report_export_hash(&report.autopilot_export).is_ok());
    }

    #[test]
    fn failure_cluster_export_groups_missed_criteria() {
        let mut score = fixture_score_report();
        score.all_pass = false;
        score.criterion_pass_rate_bps = 0;
        score.criterion_results = vec![CriterionResult {
            criterion_id: String::from("criterion.reasoning"),
            passed: false,
            verdict: CriterionVerdict::Fail,
            reasoning: String::from("missing legal reasoning"),
            evidence_refs: vec![String::from("memo")],
            judge_model: String::from("mock-judge"),
            judge_prompt_hash: String::from("prompt-hash"),
            raw_response_hash: String::from("raw-hash"),
            confidence_bps: Some(9000),
            judge_latency_ms: Some(2),
            judge_cost_micro_usd: Some(1),
        }];
        score.failure_comparisons = vec![CriterionCoverageFailure {
            criterion_id: String::from("criterion.reasoning"),
            failure_class: CriterionFailureClass::CoverageGap,
            missing_source_artifact_ids: vec![String::from("source.contract")],
            missing_deliverable_ids: Vec::new(),
            evidence_refs: Vec::new(),
            diagnostic: String::from("missed source material"),
        }];
        score.metrics = RunMetrics {
            model_turns: 1,
            tool_call_count: 0,
            input_tokens: 10,
            output_tokens: 5,
            wall_time_ms: 10,
            estimated_cost_micro_usd: 1,
        };
        let input = LegalBenchmarkReportInput {
            report_id: String::from("report.fail"),
            score_reports: vec![score],
            run_records: Vec::new(),
            output_manifests: Vec::new(),
        };
        let report = generate_legal_benchmark_static_report(&input).expect("report");

        assert_eq!(report.autopilot_export.failure_clusters.len(), 1);
        assert_eq!(
            report.autopilot_export.failure_clusters[0].failure_family,
            "judge_fail"
        );
        assert_eq!(
            report.autopilot_export.coverage_failure_comparisons.len(),
            1
        );
        assert!(report.markdown.contains("Missed criteria"));
        assert!(report.markdown.contains("coverage=CoverageGap"));
    }
}
