//! Plain-text renderers over frozen `openagents.gym.*` schema v1.
//!
//! Every Gym command's `--json` document is the rendering contract. These
//! functions are the terminal (and TUI) face of that contract: they print
//! fields the schema carries, and they print `unknown` for a missing optional
//! measurement. They never invent a zero.

use crate::gym::schemas::{
    CorpusImportRecord, CorpusInventory, DatasetView, EnvReport, ResultsTrend, RunStatus,
    SuiteManifestView, SuiteTaskPin,
};

/// Missing optional text. Empty strings are missing too.
pub fn unknown_str(value: Option<&str>) -> &str {
    match value {
        Some(value) if !value.is_empty() => value,
        _ => "unknown",
    }
}

/// Missing optional number. A `None` is `unknown`, never `0`.
pub fn unknown_f64(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{value:.4}"),
        None => "unknown".to_string(),
    }
}

/// Missing optional USD amount. A `None` is `unknown`, never `$0.0000`.
pub fn unknown_cost(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("${value:.4}"),
        None => "unknown".to_string(),
    }
}

/// `openagents.gym.suite_manifest_view.v1`
pub fn render_suite_manifest_view(view: &SuiteManifestView) -> Vec<String> {
    let mut lines = vec![format!(
        "{}  {}  {} task{}  {}",
        view.suite_id,
        view.tier,
        view.task_count,
        if view.task_count == 1 { "" } else { "s" },
        view.suite_digest
    )];
    if !view.description.is_empty() {
        lines.push(format!("  {}", view.description));
    }
    if let Some(path) = &view.source_path {
        lines.push(format!("  {}", path));
    }
    for task in &view.tasks {
        let env = if task.environment_available {
            "env=yes"
        } else {
            "env=no"
        };
        let rationale = unknown_str(task.rationale.as_deref());
        let pin = match &task.pin {
            SuiteTaskPin::HarborRegistry {
                dataset,
                git_url,
                commit,
                path,
            } => format!("harbor-registry  {dataset}  {git_url}  {commit}  {path}"),
            SuiteTaskPin::TrackerClosedIssue {
                repo,
                issue,
                accepted_commit,
            } => format!("tracker-closed-issue  {repo}  #{issue}  {accepted_commit}"),
        };
        lines.push(format!(
            "  {}  {}  {env}  rationale={rationale}  {pin}",
            task.id, task.task_digest
        ));
    }
    lines
}

/// Compact one-line form used by `gym suite list`.
pub fn render_suite_list_line(view: &SuiteManifestView) -> String {
    format!(
        "{}  {}  {}  {}",
        view.suite_id, view.tier, view.task_count, view.suite_digest
    )
}

/// `openagents.gym.run_status.v1`
pub fn render_run_status(status: &RunStatus) -> Vec<String> {
    let mut lines = vec![
        format!(
            "{}  {}  {}  {}",
            status.run_id, status.suite_id, status.state, status.summary
        ),
        format!(
            "  lane={}  model={}  started_at={}  updated_at={}",
            status.lane,
            unknown_str(status.model.as_deref()),
            unknown_str(status.started_at.as_deref()),
            unknown_str(status.updated_at.as_deref())
        ),
        format!(
            "  tasks_total={}  accepted={}  rejected={}  ungraded={}  graded={}",
            status.tasks_total, status.accepted, status.rejected, status.ungraded, status.graded
        ),
    ];
    for trial in &status.trials {
        lines.push(format!(
            "  {}  {}  outcome={}  cost={}  started_at={}  finished_at={}  transcript={}",
            trial.task,
            trial.state,
            unknown_str(trial.outcome.as_deref()),
            unknown_cost(trial.cost_usd),
            unknown_str(trial.started_at.as_deref()),
            unknown_str(trial.finished_at.as_deref()),
            unknown_str(trial.transcript_ref.as_deref())
        ));
    }
    lines
}

/// Compact one-line form used by `gym run list`.
pub fn render_run_list_line(status: &RunStatus) -> String {
    format!(
        "{}  {}  {}  {}/{}/{}/{}",
        status.run_id,
        status.suite_id,
        status.state,
        status.accepted,
        status.rejected,
        status.ungraded,
        status.tasks_total
    )
}

/// `openagents.gym.corpus_inventory.v1`
pub fn render_corpus_inventory(doc: &CorpusInventory) -> Vec<String> {
    let mut lines = vec![format!(
        "generated_at={}  row_count={}",
        doc.generated_at, doc.row_count
    )];
    for row in &doc.rows {
        let excluded = match &row.excluded_because {
            Some(reasons) if !reasons.is_empty() => format!("  excluded={}", reasons.join(",")),
            _ => String::new(),
        };
        lines.push(format!(
            "  {}  {}  {}  bytes={}  steps_est={}  started_at={}  ended_at={}  model={}  repo={}  domain={}  qualifies={}{excluded}",
            row.source,
            row.path,
            row.digest,
            row.bytes,
            row.steps_est,
            row.started_at,
            row.ended_at,
            unknown_str(row.model.as_deref()),
            unknown_str(row.repo_hint.as_deref()),
            unknown_str(row.domain.as_deref()),
            row.qualifies
        ));
    }
    lines
}

/// `openagents.gym.corpus_import_record.v1`
pub fn render_corpus_import_record(row: &CorpusImportRecord) -> Vec<String> {
    vec![format!(
        "{}  {}  {}  {}  {}  {}  batch={}",
        row.digest,
        row.trace_uuid,
        row.source,
        row.domain,
        row.visibility,
        row.recorded_at,
        unknown_str(row.batch_id.as_deref())
    )]
}

/// `openagents.gym.dataset_view.v1`
pub fn render_dataset_view(view: &DatasetView) -> Vec<String> {
    let mut lines = vec![format!(
        "{}  v{}  created {}  updated {}",
        view.dataset_id, view.version, view.created_at, view.updated_at
    )];
    lines.push(format!("  {}", unknown_str(view.description.as_deref())));
    lines.push(format!("  {} member(s)", view.members.len()));
    for member in &view.members {
        lines.push(format!(
            "  {}  {}  {}  {}  {}  tags=[{}]",
            member.reference,
            member.kind,
            member.added_by,
            member.added_at,
            member.reason,
            member.tags.join(",")
        ));
    }
    lines
}

/// `openagents.gym.env_report.v1`
pub fn render_env_report(report: &EnvReport) -> Vec<String> {
    let mut lines = vec![format!(
        "target={}  sufficient_for_scored_run={}  generated_at={}",
        report.target, report.sufficient_for_scored_run, report.generated_at
    )];
    for check in &report.checks {
        let passed = if check.passed { "passed" } else { "failed" };
        let required = if check.required {
            "required"
        } else {
            "optional"
        };
        lines.push(format!(
            "  {}  {passed}  {required}  observed={}",
            check.name,
            unknown_str(check.observed.as_deref())
        ));
        if let Some(remedy) = &check.remedy
            && !remedy.is_empty()
        {
            lines.push(format!("    remedy: {remedy}"));
        }
    }
    lines
}

/// `openagents.gym.results_trend.v1` — also the shareable compare artifact.
pub fn render_results_trend(trend: &ResultsTrend) -> Vec<String> {
    let chain = if trend.verified { "verified" } else { "broken" };
    let mut lines = vec![format!(
        "{}  {}  chain={chain}  isolated_groups={}",
        trend.suite_id, trend.suite_key, trend.isolated_groups
    )];
    if trend.lane_comparisons.is_empty() && trend.trends.is_empty() {
        lines.push("  no comparable rows".to_string());
        return lines;
    }
    for cmp in &trend.lane_comparisons {
        lines.push(format!(
            "  lanes vs {}  tasks=[{}]",
            cmp.baseline_lane,
            cmp.tasks.join(",")
        ));
        for lane in &cmp.lanes {
            lines.push(format!(
                "    {}  rate={}  cost={}  recorded_at={}",
                lane.lane,
                unknown_f64(lane.success_rate),
                unknown_cost(lane.cost_per_accepted_outcome_usd),
                lane.recorded_at
            ));
            if let Some(delta) = &lane.cost_delta {
                lines.push(format!(
                    "      cost_delta={}  from={}  to={}  reason={}",
                    delta.direction,
                    unknown_f64(delta.from),
                    unknown_f64(delta.to),
                    delta.reason
                ));
            }
            if let Some(delta) = &lane.success_rate_delta {
                lines.push(format!(
                    "      success_rate_delta={}  from={}  to={}  reason={}",
                    delta.direction,
                    unknown_f64(delta.from),
                    unknown_f64(delta.to),
                    delta.reason
                ));
            }
        }
        for confounder in &cmp.confounders {
            lines.push(format!("    confounder: {confounder}"));
        }
    }
    for lane_trend in &trend.trends {
        lines.push(format!("  trend {}", lane_trend.lane));
        for step in &lane_trend.steps {
            lines.push(format!(
                "    {} → {}  success {}  cost {}",
                step.from_recorded_at,
                step.to_recorded_at,
                step.success_rate_delta.direction,
                step.cost_delta.direction
            ));
        }
    }
    lines
}

/// Print one renderer's lines.
pub fn emit_lines(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}

/// Document shown in the coder TUI gym pane. Rendered from the frozen schema,
/// never from scraped human text.
#[derive(Debug, Clone, PartialEq)]
pub enum GymPanel {
    Trend(ResultsTrend),
    Run(RunStatus),
}

impl GymPanel {
    pub fn render_lines(&self) -> Vec<String> {
        match self {
            Self::Trend(trend) => render_results_trend(trend),
            Self::Run(status) => render_run_status(status),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gym::schemas::{
        CorpusInventoryRow, DatasetMember, Delta, EnvCheck, LaneComparison, LaneRow, RunTrial,
        Trend, TrendStep,
    };
    use std::path::PathBuf;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gym")
    }

    fn check_or_update_plain(name: &str, lines: &[String]) {
        let path = fixture_dir().join(format!("{name}.plain.txt"));
        let rendered = {
            let mut text = lines.join("\n");
            text.push('\n');
            text
        };
        if std::env::var("UPDATE_GYM_GOLDEN").is_ok() {
            std::fs::write(&path, &rendered).unwrap();
            return;
        }
        let raw = std::fs::read_to_string(&path).expect("the plain golden is checked in");
        assert_eq!(raw, rendered, "{name} plain renderer drifted");
    }

    fn load_json<T: serde::de::DeserializeOwned>(name: &str) -> T {
        let path = fixture_dir().join(format!("{name}.golden.json"));
        let raw = std::fs::read_to_string(&path).expect("the json golden is checked in");
        serde_json::from_str(&raw).expect("golden json deserializes")
    }

    #[test]
    fn suite_manifest_view_plain_golden() {
        let view = load_json("suite_manifest_view");
        check_or_update_plain("suite_manifest_view", &render_suite_manifest_view(&view));
    }

    #[test]
    fn run_status_plain_golden() {
        let status = load_json("run_status");
        check_or_update_plain("run_status", &render_run_status(&status));
    }

    #[test]
    fn corpus_inventory_plain_golden() {
        let doc = load_json("corpus_inventory");
        check_or_update_plain("corpus_inventory", &render_corpus_inventory(&doc));
    }

    #[test]
    fn corpus_import_record_plain_golden() {
        let row = load_json("corpus_import_record");
        check_or_update_plain("corpus_import_record", &render_corpus_import_record(&row));
    }

    #[test]
    fn dataset_view_plain_golden() {
        let view = load_json("dataset_view");
        check_or_update_plain("dataset_view", &render_dataset_view(&view));
    }

    #[test]
    fn env_report_plain_golden() {
        let report = load_json("env_report");
        check_or_update_plain("env_report", &render_env_report(&report));
    }

    #[test]
    fn results_trend_plain_golden() {
        let trend = load_json("results_trend");
        check_or_update_plain("results_trend", &render_results_trend(&trend));
    }

    #[test]
    fn run_status_prints_unknown_never_a_fabricated_zero() {
        let status = RunStatus {
            schema: crate::gym::schemas::RUN_STATUS_SCHEMA.to_string(),
            run_id: "run-x".into(),
            suite_id: "tb2-quick".into(),
            lane: "proxy".into(),
            model: None,
            state: "running".into(),
            started_at: None,
            updated_at: None,
            tasks_total: 1,
            accepted: 0,
            rejected: 0,
            ungraded: 1,
            graded: 0,
            summary: "still running".into(),
            trials: vec![RunTrial {
                task: "regex-log".into(),
                state: "ungraded".into(),
                outcome: None,
                started_at: None,
                finished_at: None,
                transcript_ref: None,
                cost_usd: None,
            }],
        };
        let text = render_run_status(&status).join("\n");
        assert!(text.contains("model=unknown"), "{text}");
        assert!(text.contains("outcome=unknown"), "{text}");
        assert!(text.contains("cost=unknown"), "{text}");
        assert!(text.contains("started_at=unknown"), "{text}");
        assert!(
            !text.contains("cost=$0"),
            "missing cost must not become a zero: {text}"
        );
    }

    #[test]
    fn results_trend_prints_unknown_never_a_fabricated_zero() {
        let trend = ResultsTrend {
            schema: crate::gym::schemas::RESULTS_TREND_SCHEMA.to_string(),
            suite_id: "tb2-quick".into(),
            suite_key: "suite:abc".into(),
            verified: true,
            lane_comparisons: vec![LaneComparison {
                suite_key: "suite:abc".into(),
                suite_id: "tb2-quick".into(),
                tasks: vec!["regex-log".into()],
                baseline_lane: "proxy".into(),
                lanes: vec![LaneRow {
                    lane: "local".into(),
                    run_digest: "run:def".into(),
                    recorded_at: "2026-08-27T12:00:00Z".into(),
                    cost_per_accepted_outcome_usd: None,
                    success_rate: None,
                    cost_delta: Some(Delta {
                        from: Some(0.75),
                        to: None,
                        absolute: None,
                        relative: None,
                        direction: "unpriced".into(),
                        reason: "no cost delta".into(),
                    }),
                    success_rate_delta: None,
                }],
                confounders: vec![],
            }],
            trends: vec![Trend {
                suite_key: "suite:abc".into(),
                suite_id: "tb2-quick".into(),
                lane: "proxy".into(),
                steps: vec![TrendStep {
                    from_recorded_at: "2026-08-25T12:00:00Z".into(),
                    to_recorded_at: "2026-08-26T12:00:00Z".into(),
                    cost_delta: Delta {
                        from: None,
                        to: None,
                        absolute: None,
                        relative: None,
                        direction: "unknown".into(),
                        reason: "unpriced".into(),
                    },
                    success_rate_delta: Delta {
                        from: Some(0.9),
                        to: Some(1.0),
                        absolute: Some(0.1),
                        relative: Some(0.1111),
                        direction: "better".into(),
                        reason: "success rate rose".into(),
                    },
                    confounders: vec![],
                }],
            }],
            isolated_groups: 0,
        };
        let text = render_results_trend(&trend).join("\n");
        assert!(text.contains("rate=unknown"), "{text}");
        assert!(text.contains("cost=unknown"), "{text}");
        assert!(text.contains("to=unknown"), "{text}");
        assert!(
            !text.contains("cost=$0"),
            "missing cost must not become a zero: {text}"
        );
        assert!(
            !text.contains("rate=0.0000"),
            "missing rate must not become a zero: {text}"
        );
    }

    #[test]
    fn env_report_failed_check_prints_remedy_and_unknown_observed() {
        let report = EnvReport {
            schema: crate::gym::schemas::ENV_REPORT_SCHEMA.to_string(),
            target: "local".into(),
            sufficient_for_scored_run: false,
            generated_at: "2026-08-27T09:45:00Z".into(),
            checks: vec![EnvCheck {
                name: "disk_headroom".into(),
                passed: false,
                required: true,
                remedy: Some("free 5 GB".into()),
                observed: None,
            }],
        };
        let text = render_env_report(&report).join("\n");
        assert!(text.contains("observed=unknown"), "{text}");
        assert!(text.contains("remedy: free 5 GB"), "{text}");
        assert!(text.contains("failed"), "{text}");
    }

    #[test]
    fn corpus_and_dataset_optional_fields_print_unknown() {
        let inventory = CorpusInventory {
            schema: "openagents.gym.corpus_inventory.v1".into(),
            generated_at: "2026-08-27T09:00:00Z".into(),
            row_count: 1,
            rows: vec![CorpusInventoryRow {
                source: "codex".into(),
                path: "/tmp/session.jsonl".into(),
                digest: "sha256:ab".into(),
                bytes: 12,
                steps_est: 3,
                started_at: "2026-08-25T10:00:00Z".into(),
                ended_at: "2026-08-25T10:01:00Z".into(),
                model: None,
                repo_hint: None,
                domain: None,
                qualifies: false,
                excluded_because: None,
            }],
        };
        let inventory_text = render_corpus_inventory(&inventory).join("\n");
        assert!(inventory_text.contains("model=unknown"), "{inventory_text}");
        assert!(inventory_text.contains("repo=unknown"), "{inventory_text}");
        assert!(
            inventory_text.contains("domain=unknown"),
            "{inventory_text}"
        );

        let dataset = DatasetView {
            schema: "openagents.gym.dataset_view.v1".into(),
            dataset_id: "example".into(),
            description: None,
            version: 0,
            created_at: "2026-08-20T12:00:00Z".into(),
            updated_at: "2026-08-20T12:00:00Z".into(),
            members: vec![DatasetMember {
                reference: "sha256:ab".into(),
                kind: "trace".into(),
                added_by: "oa_agent".into(),
                added_at: "2026-08-21T10:00:00Z".into(),
                reason: "qualifying".into(),
                tags: vec![],
            }],
        };
        let dataset_text = render_dataset_view(&dataset).join("\n");
        assert!(dataset_text.contains("unknown"), "{dataset_text}");
        assert!(dataset_text.contains("tags=[]"), "{dataset_text}");

        let import = CorpusImportRecord {
            schema: crate::gym::schemas::CORPUS_IMPORT_RECORD_SCHEMA.to_string(),
            digest: "sha256:ab".into(),
            trace_uuid: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into(),
            source: "openagents".into(),
            domain: "agent-building".into(),
            visibility: "ledger".into(),
            recorded_at: "2026-08-27T09:15:00Z".into(),
            batch_id: None,
        };
        let import_text = render_corpus_import_record(&import).join("\n");
        assert!(import_text.contains("batch=unknown"), "{import_text}");
    }
}
