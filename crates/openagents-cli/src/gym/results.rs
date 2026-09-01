//! `openagents gym results` — score a Harbor job, then show / compare / trend
//! chained rows. The grading rule matches `packages/coder-effectiveness`: a
//! trial is accepted only when a verifier ran and returned a positive reward;
//! ungraded trials are counted and kept out of the success-rate denominator;
//! an unpriced cost is `unknown`, never `$0.0000`.

use crate::computer::now_iso8601;
use crate::errors::CliError;
use crate::gym::gate::{GateOutcome, evaluate_gate};
use crate::gym::rates::{RateCatalog, UsageForCost, load_rate_catalog, price_usage};
use crate::gym::schemas::{
    Delta, LaneComparison, LaneRow, RESULTS_TREND_SCHEMA, ResultsTrend, Trend, TrendStep,
};
use crate::gym::suite::suite_meta;
use clap::{Args, Subcommand};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const BENCH_RESULT_SCHEMA: &str = "openagents.bench_result.v3";
pub const BENCH_RESULT_SCHEMA_V2: &str = "openagents.bench_result.v2";
const APPEND_REFUSED_EXIT: i32 = 3;

#[derive(Args, Debug)]
pub struct ResultsArgs {
    #[command(subcommand)]
    pub action: ResultsAction,
}

#[derive(Subcommand, Debug)]
pub enum ResultsAction {
    /// Score a completed Harbor job directory.
    Score {
        /// Harbor job directory (`result.json` + per-trial dirs).
        job_dir: PathBuf,
        /// Suite id the job claims to have run.
        #[arg(long)]
        suite: String,
        /// Lane the job used. Default: proxy.
        #[arg(long)]
        lane: Option<String>,
        /// Append to `bench-results/<suite>.jsonl` when the suite is score-tier.
        #[arg(long)]
        append: bool,
    },
    /// Print chained rows for one suite.
    Show { suite_id: String },
    /// Compare lanes, or two suites side by side.
    Compare {
        suite_id: String,
        /// Optional second suite for a cross-suite compare.
        other: Option<String>,
    },
    /// Print the verified trend for one suite.
    Trend { suite_id: String },
}

pub fn run_results(args: ResultsArgs, json: bool) -> Result<(), CliError> {
    match args.action {
        ResultsAction::Score {
            job_dir,
            suite,
            lane,
            append,
        } => score_job(
            &job_dir,
            &suite,
            lane.as_deref().unwrap_or("proxy"),
            append,
            json,
        ),
        ResultsAction::Show { suite_id } => show_suite(&suite_id, json),
        ResultsAction::Compare { suite_id, other } => {
            compare_suites(&suite_id, other.as_deref(), json)
        }
        ResultsAction::Trend { suite_id } => trend_suite(&suite_id, json),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoreReport {
    pub suite: String,
    pub lane: String,
    pub job_id: Option<String>,
    pub trials_total: u64,
    pub accepted: u64,
    pub rejected: u64,
    pub ungraded: u64,
    pub graded: u64,
    pub success_rate: Option<f64>,
    pub ungraded_ratio: f64,
    /// `known`, `no_accepted_outcomes`, `unmetered_local_lane`,
    /// `cost_partial`, or `cost_unknown` — the reason the figure below is or
    /// is not a number. Never a false zero.
    pub cost_disposition: String,
    pub cost_per_accepted_outcome_usd: Option<f64>,
    /// Sum over the trials the catalog could price, or `None` when none
    /// priced. Read it next to `cost_coverage`: a `partial` sum understates.
    pub total_cost_usd: Option<f64>,
    /// `known`, `partial`, `unknown`, or `unmetered`.
    pub cost_coverage: String,
    /// The weakest basis among the rates used: a placeholder anywhere makes
    /// the whole run placeholder-priced.
    pub rate_basis: Option<String>,
    /// Version pin of `bench/rates.json` the run was priced against.
    pub rate_catalog_version: String,
    /// Distinct reasons trials went unpriced, for the report to print.
    pub unpriced_reasons: Vec<String>,
    /// Summed from the coder's ATIF trajectories; an absent total is unknown.
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub cached_input_tokens: u64,
    pub tool_calls: Option<u64>,
    pub tasks: Vec<String>,
    /// Catalog ids from the trials' trajectories, falling back to Harbor
    /// `config.json` (`ollama/<tag>` becomes `<tag>`).
    pub models: Vec<String>,
    /// `finished_at - started_at` on the Harbor envelope, when both parse.
    pub wall_clock_seconds: Option<f64>,
}

pub fn score_harbor_job(job_dir: &Path, suite: &str, lane: &str) -> Result<ScoreReport, CliError> {
    let catalog = load_rate_catalog(&repo_root()?)?;
    score_harbor_job_with(job_dir, suite, lane, &catalog)
}

/// Score against an explicit rate catalog. The seam tests use.
pub fn score_harbor_job_with(
    job_dir: &Path,
    suite: &str,
    lane: &str,
    catalog: &RateCatalog,
) -> Result<ScoreReport, CliError> {
    if !job_dir.join("result.json").is_file() {
        return Err(CliError::Input(format!(
            "not a Harbor job directory (no result.json): {}",
            job_dir.display()
        )));
    }
    let envelope: Value = read_json(&job_dir.join("result.json"))?;
    let job_id = envelope
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string);

    let mut trials = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(job_dir)
        .map_err(|e| CliError::Input(format!("could not read {}: {e}", job_dir.display())))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    entries.sort();
    for dir in entries {
        let result_path = dir.join("result.json");
        if !result_path.is_file() {
            continue;
        }
        let result = read_json(&result_path)?;
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let task = if let Some(at) = name.rfind("__") {
            name[..at].to_string()
        } else {
            name.to_string()
        };
        trials.push(read_trial(&dir, task, outcome_of(&result)));
    }

    let accepted = trials
        .iter()
        .filter(|t| t.outcome == Outcome::Accepted)
        .count() as u64;
    let rejected = trials
        .iter()
        .filter(|t| t.outcome == Outcome::Rejected)
        .count() as u64;
    let ungraded = trials
        .iter()
        .filter(|t| t.outcome == Outcome::Ungraded)
        .count() as u64;
    let graded = accepted + rejected;
    let total = trials.len() as u64;
    let success_rate = if graded == 0 {
        None
    } else {
        Some(accepted as f64 / graded as f64)
    };
    let ungraded_ratio = if total == 0 {
        0.0
    } else {
        ungraded as f64 / total as f64
    };

    // Price each trial. Unknown stays unknown; a partial numerator over a
    // full denominator is never presented as the run's cost.
    let mut priced = 0u64;
    let mut unpriced = 0u64;
    let mut unmetered = 0u64;
    let mut total_usd = 0.0f64;
    let mut bases: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut unpriced_reasons: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    for trial in &trials {
        let cost = price_usage(trial.model_id.as_deref(), trial.usage, catalog, lane);
        match cost.usd {
            Some(usd) => {
                priced += 1;
                total_usd += usd;
                if let Some(basis) = cost.rate_basis {
                    bases.insert(basis);
                }
            }
            None => {
                unpriced += 1;
                if cost.disposition == "unmetered_local_lane" {
                    unmetered += 1;
                }
                unpriced_reasons.insert(cost.reason);
            }
        }
    }
    let cost_coverage = if priced == 0 {
        if total > 0 && unmetered == total {
            "unmetered"
        } else {
            "unknown"
        }
    } else if unpriced == 0 {
        "known"
    } else {
        "partial"
    };
    let total_cost_usd = (priced > 0).then_some(total_usd);
    // A placeholder anywhere makes the whole run placeholder-priced.
    let rate_basis = if bases.contains("operator_placeholder") {
        Some("operator_placeholder".to_string())
    } else {
        bases.iter().next().cloned()
    };
    let (cost_disposition, cost_per_accepted_outcome_usd) = if accepted == 0 {
        ("no_accepted_outcomes", None)
    } else if cost_coverage == "unmetered" {
        ("unmetered_local_lane", None)
    } else if cost_coverage == "known" {
        ("known", Some(total_usd / accepted as f64))
    } else if cost_coverage == "partial" {
        ("cost_partial", None)
    } else {
        ("cost_unknown", None)
    };

    let prompt_tokens = sum_or_none(trials.iter().map(|t| t.usage.prompt_tokens));
    let completion_tokens = sum_or_none(trials.iter().map(|t| t.usage.completion_tokens));
    let cached_input_tokens = trials.iter().map(|t| t.usage.cached_input_tokens).sum();
    let tool_calls = sum_or_none(trials.iter().map(|t| t.tool_calls));

    let mut models: Vec<String> = trials.iter().filter_map(|t| t.model_id.clone()).collect();
    models.sort();
    models.dedup();
    if models.is_empty() {
        models = models_from_job(job_dir);
    }
    let mut tasks: Vec<String> = trials.into_iter().map(|t| t.task).collect();
    tasks.sort();
    let wall_clock_seconds = wall_clock_seconds_of(&envelope);

    Ok(ScoreReport {
        suite: suite.to_string(),
        lane: lane.to_string(),
        job_id,
        trials_total: total,
        accepted,
        rejected,
        ungraded,
        graded,
        success_rate,
        ungraded_ratio,
        cost_disposition: cost_disposition.to_string(),
        cost_per_accepted_outcome_usd,
        total_cost_usd,
        cost_coverage: cost_coverage.to_string(),
        rate_basis,
        rate_catalog_version: catalog.version.clone(),
        unpriced_reasons: unpriced_reasons.into_iter().collect(),
        prompt_tokens,
        completion_tokens,
        cached_input_tokens,
        tool_calls,
        tasks,
        models,
        wall_clock_seconds,
    })
}

struct TrialFacts {
    task: String,
    outcome: Outcome,
    model_id: Option<String>,
    usage: UsageForCost,
    tool_calls: Option<u64>,
}

/// Read the coder's ATIF export for one trial: the model that ran it and the
/// token counts to price. A trial the timeout killed leaves no trajectory, so
/// the model falls back to Harbor's own trial `config.json` — spelled the way
/// the coder spells it (`provider/name` loses its provider), because a run
/// mixing completed and killed trials is one model, not two.
fn read_trial(trial_dir: &Path, task: String, outcome: Outcome) -> TrialFacts {
    let trajectory = read_json(&trial_dir.join("agent").join("trajectory.json")).ok();
    let agent = trajectory.as_ref().and_then(|t| t.get("agent"));
    let model_id = agent
        .and_then(|a| a.get("model_name"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| model_from_trial_config(trial_dir));

    let final_metrics = trajectory.as_ref().and_then(|t| t.get("final_metrics"));
    let read_total = |key: &str| {
        final_metrics
            .and_then(|m| m.get(key))
            .and_then(Value::as_u64)
    };
    // Cached reads are per step, never totalled by the exporter. No cached
    // figure anywhere is honestly 0: the coder omits the field when a turn
    // read no cache, not when it failed to measure.
    let steps = trajectory
        .as_ref()
        .and_then(|t| t.get("steps"))
        .and_then(Value::as_array);
    let mut cached_input_tokens = 0u64;
    let mut tool_calls = 0u64;
    for step in steps.into_iter().flatten() {
        cached_input_tokens += step
            .get("metrics")
            .and_then(|m| m.get("extra"))
            .and_then(|e| e.get("cache_read_input_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        tool_calls += step
            .get("tool_calls")
            .and_then(Value::as_array)
            .map(|calls| calls.len() as u64)
            .unwrap_or(0);
    }

    TrialFacts {
        task,
        outcome,
        model_id,
        usage: UsageForCost {
            prompt_tokens: read_total("total_prompt_tokens"),
            completion_tokens: read_total("total_completion_tokens"),
            cached_input_tokens,
        },
        tool_calls: steps.map(|_| tool_calls),
    }
}

fn model_from_trial_config(trial_dir: &Path) -> Option<String> {
    let cfg = read_json(&trial_dir.join("config.json")).ok()?;
    let spelled = cfg
        .get("agent")
        .and_then(|a| a.get("model_name"))
        .and_then(Value::as_str)
        .or_else(|| {
            cfg.get("config")
                .and_then(|c| c.get("agent"))
                .and_then(|a| a.get("model_name"))
                .and_then(Value::as_str)
        })?;
    Some(harbor_model_id(spelled))
}

/// A sum is only known when every term is. One unknown makes the total
/// unknown, and an empty run has no total.
fn sum_or_none(values: impl Iterator<Item = Option<u64>>) -> Option<u64> {
    let mut sum = 0u64;
    let mut any = false;
    for value in values {
        sum += value?;
        any = true;
    }
    any.then_some(sum)
}

fn models_from_job(job_dir: &Path) -> Vec<String> {
    let Ok(cfg) = read_json(&job_dir.join("config.json")) else {
        return Vec::new();
    };
    let mut models = Vec::new();
    if let Some(agents) = cfg.get("agents").and_then(Value::as_array) {
        for agent in agents {
            if let Some(name) = agent.get("model_name").and_then(Value::as_str) {
                models.push(harbor_model_id(name));
            }
        }
    }
    if models.is_empty()
        && let Some(name) = cfg.get("model").and_then(Value::as_str)
    {
        models.push(harbor_model_id(name));
    }
    models
}

/// Harbor spells `provider/name`. The store records the catalog id: the name,
/// except `ollama/<tag>` which is the local-lane tag itself.
fn harbor_model_id(name: &str) -> String {
    match name.split_once('/') {
        Some((_, rest)) if !rest.is_empty() => rest.to_string(),
        _ => name.to_string(),
    }
}

fn wall_clock_seconds_of(envelope: &Value) -> Option<f64> {
    let start = envelope.get("started_at").and_then(Value::as_str)?;
    let end = envelope
        .get("finished_at")
        .and_then(Value::as_str)
        .or_else(|| envelope.get("updated_at").and_then(Value::as_str))?;
    let a = harbor_unix_seconds(start)?;
    let b = harbor_unix_seconds(end)?;
    let delta = b - a;
    (delta >= 0.0).then_some(delta)
}

/// Unix seconds for a Harbor timestamp such as `2026-08-28T14:48:43.879875`.
fn harbor_unix_seconds(stamp: &str) -> Option<f64> {
    let stamp = stamp.trim().trim_end_matches('Z');
    let (date, time) = stamp.split_once('T')?;
    let mut date_parts = date.split('-');
    let year: i32 = date_parts.next()?.parse().ok()?;
    let month: i32 = date_parts.next()?.parse().ok()?;
    let day: i32 = date_parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let time = time.split(['+', '-']).next().unwrap_or(time);
    let (hms, frac) = match time.split_once('.') {
        Some((hms, frac)) => (hms, Some(frac)),
        None => (time, None),
    };
    let mut hms_parts = hms.split(':');
    let hour: f64 = hms_parts.next()?.parse().ok()?;
    let minute: f64 = hms_parts.next()?.parse().ok()?;
    let second: f64 = hms_parts.next()?.parse().ok()?;
    let mut sub = 0.0;
    if let Some(frac) = frac {
        let digits: String = frac.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            let n: f64 = digits.parse().ok()?;
            sub = n / 10f64.powi(digits.len() as i32);
        }
    }
    let mut y = year;
    if month <= 2 {
        y -= 1;
    }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u32;
    let days = era * 146097 + doe as i32 - 719468;
    Some(days as f64 * 86400.0 + hour * 3600.0 + minute * 60.0 + second + sub)
}

fn score_job(
    job_dir: &Path,
    suite: &str,
    lane: &str,
    append: bool,
    json: bool,
) -> Result<(), CliError> {
    let report = score_harbor_job(job_dir, suite, lane)?;
    // The gate is evaluated only for a complete run of a score-tier suite:
    // floors declared over a suite mean nothing over a different set of work,
    // and an incomplete run is already unpublishable as a score.
    let meta = suite_meta(&report.suite).ok();
    let (gate, gate_note) = match &meta {
        None => (
            None,
            Some(format!(
                "gate: none ({} has no suite manifest under bench/suites)",
                report.suite
            )),
        ),
        Some(meta) if meta.tier != "score" => (
            None,
            Some(format!(
                "gate: none (suite {} is tier {}, and a smoke run has nothing to pass)",
                meta.id, meta.tier
            )),
        ),
        Some(meta) => match &meta.gate {
            None => (
                None,
                Some(format!("gate: none declared by suite {}", meta.id)),
            ),
            Some(spec) => match append_refusal(&report, meta) {
                Some(reason) => (
                    None,
                    Some(format!(
                        "gate: not evaluated — this run does not cover suite {} ({reason})",
                        meta.id
                    )),
                ),
                None => (Some(evaluate_gate(&report, &meta.id, spec)), None),
            },
        },
    };
    if json {
        println!(
            "{}",
            serde_json::to_string(&score_json(&report, gate.as_ref())).unwrap()
        );
    } else {
        for line in score_lines(&report) {
            println!("{line}");
        }
        match (&gate, &gate_note) {
            (Some(gate), _) => {
                for line in gate_lines(gate) {
                    println!("{line}");
                }
            }
            (None, Some(note)) => println!("{note}"),
            (None, None) => {}
        }
    }
    if append {
        append_report(&report, gate.as_ref())?;
    }
    // The retired TypeScript report's exit ladder, restored: 0 the gate
    // passed (or there was none to pass), 1 a floor was breached, 2 the gate
    // was unverifiable. Appending first keeps the failed run recorded — the
    // row is how a regression stays visible.
    if let Some(gate) = &gate {
        match gate.status {
            "failed" => std::process::exit(1),
            "unverifiable" => std::process::exit(2),
            _ => {}
        }
    }
    Ok(())
}

fn score_json(report: &ScoreReport, gate: Option<&GateOutcome>) -> Value {
    serde_json::json!({
        "schema": "openagents.coder_effectiveness.report.v1",
        "suite": report.suite,
        "lane": report.lane,
        "jobId": report.job_id,
        "trialsTotal": report.trials_total,
        "accepted": report.accepted,
        "rejected": report.rejected,
        "ungraded": report.ungraded,
        "graded": report.graded,
        "successRate": report.success_rate,
        "ungradedRatio": report.ungraded_ratio,
        "costPerAcceptedOutcomeUsd": report.cost_per_accepted_outcome_usd,
        "costDisposition": report.cost_disposition,
        "totalCostUsd": report.total_cost_usd,
        "costCoverage": report.cost_coverage,
        "rateBasis": report.rate_basis,
        "rateCatalogVersion": report.rate_catalog_version,
        "promptTokens": report.prompt_tokens,
        "completionTokens": report.completion_tokens,
        "cachedInputTokens": report.cached_input_tokens,
        "toolCalls": report.tool_calls,
        "tasks": report.tasks,
        "models": report.models,
        "wallClockSeconds": report.wall_clock_seconds,
        "gate": gate.map(|g| serde_json::to_value(g).unwrap_or(Value::Null)),
    })
}

fn score_lines(report: &ScoreReport) -> Vec<String> {
    let rate = match report.success_rate {
        Some(v) => format!("{:.4}", v),
        None => "unknown".to_string(),
    };
    let cost = match report.cost_per_accepted_outcome_usd {
        Some(v) => format!("${v:.4}"),
        None => "unknown".to_string(),
    };
    let total = match report.total_cost_usd {
        Some(v) => format!("${v:.4}"),
        None => "unknown".to_string(),
    };
    let mut lines = vec![
        format!(
            "{} {}  accepted={} rejected={} ungraded={} graded={}",
            report.suite,
            report.lane,
            report.accepted,
            report.rejected,
            report.ungraded,
            report.graded
        ),
        format!(
            "success_rate={rate}  cost_per_accepted_outcome={cost} ({})",
            report.cost_disposition
        ),
        format!(
            "total_cost={total}  coverage={}{}",
            report.cost_coverage,
            match &report.rate_basis {
                Some(basis) => format!("  rate_basis={basis}"),
                None => String::new(),
            }
        ),
    ];
    if report.prompt_tokens.is_some() || report.completion_tokens.is_some() {
        lines.push(format!(
            "tokens prompt={} completion={} cached={}",
            report
                .prompt_tokens
                .map_or_else(|| "unknown".into(), |v: u64| v.to_string()),
            report
                .completion_tokens
                .map_or_else(|| "unknown".into(), |v: u64| v.to_string()),
            report.cached_input_tokens
        ));
    }
    for reason in &report.unpriced_reasons {
        lines.push(format!("unpriced: {reason}"));
    }
    lines
}

fn gate_lines(gate: &GateOutcome) -> Vec<String> {
    let mut lines = vec![match gate.status {
        "passed" => format!("gate {}: passed", gate.thresholds_id),
        "failed" => format!("GATE FAILED for {}:", gate.thresholds_id),
        _ => format!("GATE UNVERIFIABLE for {}:", gate.thresholds_id),
    }];
    for criterion in &gate.criteria {
        lines.push(format!(
            "  {} {}: {}",
            match criterion.verdict {
                "passed" => "passed",
                "failed" => "FAILED",
                _ => "UNVERIFIABLE",
            },
            criterion.name,
            criterion.detail
        ));
    }
    lines
}

/// Why `--append` must refuse this report, or `None` when the row may land.
///
/// A complete run of a score-tier suite lands as `tier: score`. A complete
/// run of any other smoke-tier suite lands as `tier: smoke` — a smoke-marked
/// row, never a published score. The dedicated liveness suite `smoke` still
/// records nowhere. Incomplete coverage is `smoke_run` for every suite.
pub fn append_refusal(report: &ScoreReport, meta: &crate::gym::suite::SuiteMeta) -> Option<String> {
    if meta.id == "smoke" {
        return Some(
            "smoke_run: suite smoke is the liveness check and records nowhere".to_string(),
        );
    }
    let mut expected = meta.task_ids.clone();
    expected.sort();
    let ran = report.tasks.clone();
    let missing: Vec<_> = expected
        .iter()
        .filter(|id| !ran.contains(id))
        .cloned()
        .collect();
    let unexpected: Vec<_> = ran
        .iter()
        .filter(|id| !expected.contains(id))
        .cloned()
        .collect();
    if !missing.is_empty() || !unexpected.is_empty() {
        return Some(format!(
            "smoke_run: this run did not cover suite {} (missing [{}]; unexpected [{}])",
            meta.id,
            missing.join(", "),
            unexpected.join(", ")
        ));
    }
    None
}

fn append_report(report: &ScoreReport, gate: Option<&GateOutcome>) -> Result<(), CliError> {
    let meta = suite_meta(&report.suite)?;
    if let Some(reason) = append_refusal(report, &meta) {
        eprintln!("{reason}");
        std::process::exit(APPEND_REFUSED_EXIT);
    }

    let store = store_path(&report.suite)?;
    let rows = read_result_rows(&store)?;
    match verify_result_chain(&rows) {
        ChainVerdict::Ok { .. } => {}
        ChainVerdict::Break { index, detail } => {
            eprintln!(
                "chain_broken: {} does not verify at row {index}, so nothing was appended: {detail}",
                store.display()
            );
            std::process::exit(APPEND_REFUSED_EXIT);
        }
    }
    if let Some(job_id) = &report.job_id
        && rows
            .iter()
            .any(|row| row.get("jobId").and_then(Value::as_str) == Some(job_id.as_str()))
    {
        eprintln!(
            "duplicate_job: harbor job {job_id} is already recorded in {}; re-scoring a run does not make it a second run",
            store.display()
        );
        std::process::exit(APPEND_REFUSED_EXIT);
    }

    let previous = rows.last().and_then(|row| {
        row.get("receipt")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let row = build_result_row(report, &meta, gate, previous.as_deref());
    if let Some(parent) = store.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CliError::Output(format!("could not create {}: {e}", parent.display())))?;
    }
    let mut line = serde_json::to_string(&row).map_err(|e| CliError::Output(e.to_string()))?;
    line.push('\n');
    use std::io::Write;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&store)
        .and_then(|mut f| f.write_all(line.as_bytes()))
        .map_err(|e| CliError::Output(format!("could not append {}: {e}", store.display())))?;
    println!("appended {}", store.display());
    Ok(())
}

fn show_suite(suite_id: &str, json: bool) -> Result<(), CliError> {
    let (rows, trend) = load_verified_trend(suite_id)?;
    if json {
        println!("{}", serde_json::to_string(&trend).unwrap());
        return Ok(());
    }
    println!("{}  {} rows  chain=verified", suite_id, rows.len());
    for row in &rows {
        println!("{}", row_line(row));
    }
    Ok(())
}

fn trend_suite(suite_id: &str, json: bool) -> Result<(), CliError> {
    let (_rows, trend) = load_verified_trend(suite_id)?;
    if json {
        println!("{}", serde_json::to_string(&trend).unwrap());
        return Ok(());
    }
    crate::gym::views::emit_lines(&crate::gym::views::render_results_trend(&trend));
    Ok(())
}

fn compare_suites(suite_id: &str, other: Option<&str>, json: bool) -> Result<(), CliError> {
    let (_rows, trend) = load_verified_trend(suite_id)?;
    if let Some(other_id) = other {
        let (_other_rows, other_trend) = load_verified_trend(other_id)?;
        if json {
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "schema": RESULTS_TREND_SCHEMA,
                    "suites": [trend, other_trend],
                }))
                .unwrap()
            );
            return Ok(());
        }
        crate::gym::views::emit_lines(&crate::gym::views::render_results_trend(&trend));
        println!("---");
        crate::gym::views::emit_lines(&crate::gym::views::render_results_trend(&other_trend));
        return Ok(());
    }
    if json {
        println!("{}", serde_json::to_string(&trend).unwrap());
        return Ok(());
    }
    crate::gym::views::emit_lines(&crate::gym::views::render_results_trend(&trend));
    Ok(())
}

/// Load a verified trend document for the TUI gym pane.
pub fn load_suite_trend(suite_id: &str) -> Result<ResultsTrend, CliError> {
    load_verified_trend(suite_id).map(|(_, trend)| trend)
}

fn load_verified_trend(suite_id: &str) -> Result<(Vec<Value>, ResultsTrend), CliError> {
    let store = store_path(suite_id)?;
    let rows = read_result_rows(&store)?;
    match verify_result_chain(&rows) {
        ChainVerdict::Ok { .. } => {}
        ChainVerdict::Break { index, detail } => {
            return Err(CliError::Input(format!(
                "broken chain in {} at row {index}: {detail}",
                store.display()
            )));
        }
    }
    let trend = trend_of(suite_id, &rows);
    Ok((rows, trend))
}

fn row_line(row: &Value) -> String {
    let recorded = row.get("recordedAt").and_then(Value::as_str).unwrap_or("-");
    let lane = row.get("lane").and_then(Value::as_str).unwrap_or("-");
    let accepted = row.get("accepted").and_then(Value::as_u64).unwrap_or(0);
    let graded = row.get("graded").and_then(Value::as_u64).unwrap_or(0);
    let rate = row
        .get("successRate")
        .and_then(Value::as_f64)
        .map(|v| format!("{v:.4}"))
        .unwrap_or_else(|| "unknown".into());
    let cost = match row.get("costPerAcceptedOutcomeUsd") {
        Some(Value::Number(n)) => n
            .as_f64()
            .map(|v| format!("${v:.4}"))
            .unwrap_or_else(|| "unknown".into()),
        _ => "unknown".into(),
    };
    format!("{recorded}  {lane}  accepted={accepted}/{graded}  success_rate={rate}  cost={cost}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Accepted,
    Rejected,
    Ungraded,
}

fn outcome_of(trial: &Value) -> Outcome {
    let verifier = trial.get("verifier_result");
    if verifier.is_none() || verifier == Some(&Value::Null) {
        return Outcome::Ungraded;
    }
    let rewards = verifier
        .and_then(|v| v.get("rewards"))
        .or_else(|| trial.get("rewards"));
    match reward_value(rewards) {
        Some(r) if r > 0.0 => Outcome::Accepted,
        Some(_) => Outcome::Rejected,
        None => Outcome::Rejected,
    }
}

fn reward_value(rewards: Option<&Value>) -> Option<f64> {
    let rewards = rewards?;
    if let Some(n) = rewards.as_f64() {
        return Some(n);
    }
    if let Some(n) = rewards.get("reward").and_then(Value::as_f64) {
        return Some(n);
    }
    let obj = rewards.as_object()?;
    if obj.len() == 1 {
        return obj.values().next().and_then(Value::as_f64);
    }
    None
}

fn read_json(path: &Path) -> Result<Value, CliError> {
    let text = fs::read_to_string(path)
        .map_err(|e| CliError::Input(format!("could not read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| CliError::Input(format!("{} is not JSON: {e}", path.display())))
}

pub fn read_result_rows(path: &Path) -> Result<Vec<Value>, CliError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)
        .map_err(|e| CliError::Input(format!("could not read {}: {e}", path.display())))?;
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line).map_err(|_| {
            CliError::Input(format!("{} line {} is not JSON", path.display(), i + 1))
        })?;
        let schema = value.get("schema").and_then(Value::as_str).unwrap_or("");
        if schema == "openagents.bench_result.v1" {
            return Err(CliError::Input(format!(
                "{} line {} is a v1 row and cannot be compared; move it to an archive",
                path.display(),
                i + 1
            )));
        }
        if schema != BENCH_RESULT_SCHEMA && schema != BENCH_RESULT_SCHEMA_V2 {
            return Err(CliError::Input(format!(
                "{} line {} has schema {schema}, expected {BENCH_RESULT_SCHEMA} or {BENCH_RESULT_SCHEMA_V2}",
                path.display(),
                i + 1
            )));
        }
        rows.push(value);
    }
    Ok(rows)
}

pub enum ChainVerdict {
    Ok { rows: usize, head: Option<String> },
    Break { index: usize, detail: String },
}

pub fn verify_result_chain(rows: &[Value]) -> ChainVerdict {
    let mut previous: Option<String> = None;
    for (index, row) in rows.iter().enumerate() {
        let receipt = row
            .get("receipt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let expected = receipt_of(row);
        if receipt != expected {
            let suite = row.get("suite").and_then(Value::as_str).unwrap_or("?");
            let lane = row.get("lane").and_then(Value::as_str).unwrap_or("?");
            let recorded = row.get("recordedAt").and_then(Value::as_str).unwrap_or("?");
            return ChainVerdict::Break {
                index,
                detail: format!(
                    "row {index} ({suite} on {lane}, recorded {recorded}) carries {receipt} but its contents digest to {expected}, so it was edited after it was written"
                ),
            };
        }
        let prev = row.get("previousReceipt").and_then(|v| {
            if v.is_null() {
                None
            } else {
                v.as_str().map(str::to_string)
            }
        });
        if prev != previous {
            return ChainVerdict::Break {
                index,
                detail: format!(
                    "row {index} follows {} but the row before it is {}, so rows were inserted, removed, or reordered",
                    prev.as_deref().unwrap_or("null"),
                    previous.as_deref().unwrap_or("null")
                ),
            };
        }
        previous = Some(receipt);
    }
    ChainVerdict::Ok {
        rows: rows.len(),
        head: previous,
    }
}

/// Receipt over every field except `receipt`, matching the TypeScript store:
/// sorted top-level keys, then `JSON.stringify` of `[key, value]` pairs.
pub fn receipt_of(row: &Value) -> String {
    let obj = match row.as_object() {
        Some(o) => o,
        None => return "receipt:".to_string(),
    };
    let mut keys: Vec<&String> = obj.keys().filter(|k| *k != "receipt").collect();
    keys.sort();
    let pairs: Vec<Value> = keys
        .into_iter()
        .map(|k| {
            Value::Array(vec![
                Value::String(k.clone()),
                obj.get(k).cloned().unwrap_or(Value::Null),
            ])
        })
        .collect();
    let source = serde_json::to_string(&Value::Array(pairs)).unwrap_or_default();
    format!("receipt:{}", hex_digest(source.as_bytes()))
}

fn build_result_row(
    report: &ScoreReport,
    meta: &crate::gym::suite::SuiteMeta,
    gate: Option<&GateOutcome>,
    previous: Option<&str>,
) -> Value {
    let suite_key = suite_key_of(report, meta);
    let mut obj = Map::new();
    obj.insert("schema".into(), Value::String(BENCH_RESULT_SCHEMA.into()));
    obj.insert("recordedAt".into(), Value::String(now_iso8601()));
    obj.insert("suite".into(), Value::String(report.suite.clone()));
    obj.insert("lane".into(), Value::String(report.lane.clone()));
    obj.insert(
        "runDigest".into(),
        Value::String(format!(
            "effectiveness:{}",
            hex_digest(report.job_id.as_deref().unwrap_or("").as_bytes())
        )),
    );
    obj.insert("surfaceDigests".into(), Value::Null);
    obj.insert("suiteKey".into(), Value::String(suite_key));
    obj.insert(
        "jobId".into(),
        report
            .job_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    obj.insert("suiteId".into(), Value::String(meta.id.clone()));
    obj.insert("suiteDigest".into(), Value::String(meta.digest.clone()));
    obj.insert("tier".into(), Value::String(meta.tier.clone()));
    obj.insert(
        "models".into(),
        Value::Array(report.models.iter().cloned().map(Value::String).collect()),
    );
    obj.insert("agentVersions".into(), Value::Array(vec![]));
    obj.insert(
        "rateCatalogVersion".into(),
        Value::String(report.rate_catalog_version.clone()),
    );
    obj.insert(
        "tasks".into(),
        Value::Array(report.tasks.iter().cloned().map(Value::String).collect()),
    );
    obj.insert("trialsTotal".into(), json_u64(report.trials_total));
    obj.insert("accepted".into(), json_u64(report.accepted));
    obj.insert("rejected".into(), json_u64(report.rejected));
    obj.insert("ungraded".into(), json_u64(report.ungraded));
    obj.insert("graded".into(), json_u64(report.graded));
    obj.insert(
        "successRate".into(),
        report.success_rate.map(json_f64).unwrap_or(Value::Null),
    );
    obj.insert("ungradedRatio".into(), json_f64(report.ungraded_ratio));
    obj.insert(
        "costPerAcceptedOutcomeUsd".into(),
        report
            .cost_per_accepted_outcome_usd
            .map(json_f64)
            .unwrap_or(Value::Null),
    );
    obj.insert(
        "costDisposition".into(),
        Value::String(report.cost_disposition.clone()),
    );
    obj.insert(
        "totalCostUsd".into(),
        report.total_cost_usd.map(json_f64).unwrap_or(Value::Null),
    );
    obj.insert(
        "costCoverage".into(),
        Value::String(report.cost_coverage.clone()),
    );
    obj.insert(
        "rateBasis".into(),
        report
            .rate_basis
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    obj.insert(
        "promptTokens".into(),
        report.prompt_tokens.map(json_u64).unwrap_or(Value::Null),
    );
    obj.insert(
        "completionTokens".into(),
        report
            .completion_tokens
            .map(json_u64)
            .unwrap_or(Value::Null),
    );
    obj.insert(
        "cachedInputTokens".into(),
        json_u64(report.cached_input_tokens),
    );
    obj.insert(
        "toolCalls".into(),
        report.tool_calls.map(json_u64).unwrap_or(Value::Null),
    );
    obj.insert(
        "wallClockSeconds".into(),
        report
            .wall_clock_seconds
            .map(json_f64)
            .unwrap_or(Value::Null),
    );
    // The gate verdict, when one was evaluated. A failed gate still lands as
    // a row — the record is how a regression stays visible — with the floors
    // it breached named on the row itself. `gateDigest` pins the gate content
    // the verdict was scored against, since the suite digest deliberately
    // does not cover the gate.
    match gate {
        Some(gate) => {
            obj.insert("gateStatus".into(), Value::String(gate.status.into()));
            obj.insert(
                "thresholdsId".into(),
                Value::String(gate.thresholds_id.clone()),
            );
            obj.insert("gateDigest".into(), Value::String(gate.gate_digest.clone()));
            obj.insert(
                "gateFailures".into(),
                Value::Array(
                    gate.breaches()
                        .map(|c| {
                            serde_json::json!({
                                "name": c.name,
                                "verdict": c.verdict,
                                "detail": c.detail,
                            })
                        })
                        .collect(),
                ),
            );
        }
        None => {
            obj.insert("gateStatus".into(), Value::Null);
            obj.insert("thresholdsId".into(), Value::Null);
        }
    }
    obj.insert(
        "previousReceipt".into(),
        previous
            .map(|p| Value::String(p.to_string()))
            .unwrap_or(Value::Null),
    );
    let unreceipted = Value::Object(obj.clone());
    obj.insert("receipt".into(), Value::String(receipt_of(&unreceipted)));
    Value::Object(obj)
}

fn suite_key_of(report: &ScoreReport, meta: &crate::gym::suite::SuiteMeta) -> String {
    let source = serde_json::json!({
        "suite": report.suite,
        "suiteDigest": meta.digest,
        "tasks": report.tasks,
        "rateCatalogVersion": report.rate_catalog_version,
    });
    format!(
        "suite:{}",
        hex_digest(
            serde_json::to_string(&source)
                .unwrap_or_default()
                .as_bytes()
        )
    )
}

fn json_u64(n: u64) -> Value {
    Value::Number(n.into())
}

fn json_f64(n: f64) -> Value {
    serde_json::Number::from_f64(n)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn trend_of(suite_id: &str, rows: &[Value]) -> ResultsTrend {
    let mut groups: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for row in rows {
        let key = row
            .get("suiteKey")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        groups.entry(key).or_default().push(row);
    }

    let mut lane_comparisons = Vec::new();
    let mut trends = Vec::new();
    let mut isolated_groups = 0u64;

    for (suite_key, group) in &groups {
        let latest = latest_per_lane(group);
        if latest.len() >= 2 {
            lane_comparisons.push(lane_comparison_of(suite_id, suite_key, &latest));
        }
        let mut trends_from_group = 0u64;
        let mut by_lane: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
        for row in group {
            let lane = row
                .get("lane")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            by_lane.entry(lane).or_default().push(*row);
        }
        for (lane, lane_rows) in by_lane {
            if lane_rows.len() < 2 {
                continue;
            }
            trends.push(lane_trend_of(suite_id, suite_key, &lane, &lane_rows));
            trends_from_group += 1;
        }
        if latest.len() < 2 && trends_from_group == 0 {
            isolated_groups += 1;
        }
    }

    let suite_key = groups.keys().next().cloned().unwrap_or_default();
    ResultsTrend {
        schema: RESULTS_TREND_SCHEMA.to_string(),
        suite_id: suite_id.to_string(),
        suite_key,
        verified: true,
        lane_comparisons,
        trends,
        isolated_groups,
    }
}

fn latest_per_lane<'a>(rows: &[&'a Value]) -> Vec<&'a Value> {
    let mut by_lane: BTreeMap<String, &'a Value> = BTreeMap::new();
    for row in rows {
        let lane = row
            .get("lane")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let recorded = row.get("recordedAt").and_then(Value::as_str).unwrap_or("");
        match by_lane.get(&lane) {
            Some(existing) => {
                let prev = existing
                    .get("recordedAt")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if recorded >= prev {
                    by_lane.insert(lane, *row);
                }
            }
            None => {
                by_lane.insert(lane, *row);
            }
        }
    }
    by_lane.into_values().collect()
}

fn lane_comparison_of(suite_id: &str, suite_key: &str, rows: &[&Value]) -> LaneComparison {
    let baseline = rows
        .iter()
        .find(|r| r.get("lane").and_then(Value::as_str) == Some("proxy"))
        .copied()
        .or_else(|| rows.first().copied())
        .unwrap();
    let baseline_lane = baseline
        .get("lane")
        .and_then(Value::as_str)
        .unwrap_or("proxy")
        .to_string();
    let tasks = string_array(baseline.get("tasks"));
    let lanes = rows
        .iter()
        .map(|row| {
            let is_baseline = std::ptr::eq(*row, baseline);
            LaneRow {
                lane: row
                    .get("lane")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                run_digest: row
                    .get("runDigest")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                recorded_at: row
                    .get("recordedAt")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                cost_per_accepted_outcome_usd: row
                    .get("costPerAcceptedOutcomeUsd")
                    .and_then(Value::as_f64),
                success_rate: row.get("successRate").and_then(Value::as_f64),
                cost_delta: if is_baseline {
                    None
                } else {
                    Some(cost_delta(baseline, row))
                },
                success_rate_delta: if is_baseline {
                    None
                } else {
                    Some(success_rate_delta(baseline, row))
                },
            }
        })
        .collect();
    LaneComparison {
        suite_key: suite_key.to_string(),
        suite_id: suite_id.to_string(),
        tasks,
        baseline_lane,
        lanes,
        confounders: Vec::new(),
    }
}

fn lane_trend_of(suite_id: &str, suite_key: &str, lane: &str, rows: &[&Value]) -> Trend {
    let mut ordered = rows.to_vec();
    ordered.sort_by(|a, b| {
        let left = a.get("recordedAt").and_then(Value::as_str).unwrap_or("");
        let right = b.get("recordedAt").and_then(Value::as_str).unwrap_or("");
        left.cmp(right)
    });
    let mut steps = Vec::new();
    for window in ordered.windows(2) {
        steps.push(TrendStep {
            from_recorded_at: window[0]
                .get("recordedAt")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            to_recorded_at: window[1]
                .get("recordedAt")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            cost_delta: cost_delta(window[0], window[1]),
            success_rate_delta: success_rate_delta(window[0], window[1]),
            confounders: Vec::new(),
        });
    }
    Trend {
        suite_key: suite_key.to_string(),
        suite_id: suite_id.to_string(),
        lane: lane.to_string(),
        steps,
    }
}

fn cost_delta(from: &Value, to: &Value) -> Delta {
    let from_c = from
        .get("costPerAcceptedOutcomeUsd")
        .and_then(Value::as_f64);
    let to_c = to.get("costPerAcceptedOutcomeUsd").and_then(Value::as_f64);
    if from_c.is_none() || to_c.is_none() {
        let lane = if from_c.is_none() {
            from.get("lane").and_then(Value::as_str).unwrap_or("from")
        } else {
            to.get("lane").and_then(Value::as_str).unwrap_or("to")
        };
        let disposition = if from_c.is_none() {
            from.get("costDisposition")
                .and_then(Value::as_str)
                .unwrap_or("cost_unknown")
        } else {
            to.get("costDisposition")
                .and_then(Value::as_str)
                .unwrap_or("cost_unknown")
        };
        return Delta {
            from: from_c,
            to: to_c,
            absolute: None,
            relative: None,
            direction: "unpriced".into(),
            reason: format!("no cost delta: the {lane} lane reports {disposition}"),
        };
    }
    numeric_delta(from_c.unwrap(), to_c.unwrap(), true)
}

fn success_rate_delta(from: &Value, to: &Value) -> Delta {
    let from_r = from.get("successRate").and_then(Value::as_f64);
    let to_r = to.get("successRate").and_then(Value::as_f64);
    match (from_r, to_r) {
        (Some(a), Some(b)) => numeric_delta(a, b, false),
        _ => Delta {
            from: from_r,
            to: to_r,
            absolute: None,
            relative: None,
            direction: "unknown".into(),
            reason: "no success-rate delta: a side is ungraded".into(),
        },
    }
}

fn numeric_delta(from: f64, to: f64, lower_is_better: bool) -> Delta {
    let absolute = to - from;
    let relative = if from == 0.0 {
        None
    } else {
        Some(absolute / from)
    };
    let direction = if absolute == 0.0 {
        "unchanged"
    } else if (absolute < 0.0) == lower_is_better {
        "better"
    } else {
        "worse"
    };
    let reason = match direction {
        "unchanged" => "no change",
        "better" if lower_is_better => "cost per accepted outcome fell",
        "worse" if lower_is_better => "cost per accepted outcome rose",
        "better" => "success rate rose",
        _ => "success rate fell",
    };
    Delta {
        from: Some(from),
        to: Some(to),
        absolute: Some(absolute),
        relative,
        direction: direction.into(),
        reason: reason.into(),
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn store_path(suite_id: &str) -> Result<PathBuf, CliError> {
    Ok(repo_root()?
        .join("bench-results")
        .join(format!("{suite_id}.jsonl")))
}

fn repo_root() -> Result<PathBuf, CliError> {
    let mut dir = std::env::current_dir().map_err(|e| CliError::Configuration(e.to_string()))?;
    loop {
        if dir.join("bench").join("suites").is_dir() {
            return Ok(dir);
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => {
                return Err(CliError::Configuration(
                    "could not find bench/suites from the current directory".into(),
                ));
            }
        }
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in hash.iter() {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/gym-fixtures")
            .join(name)
    }

    fn fixture_suites_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/gym-fixtures")
    }

    fn catalog() -> RateCatalog {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        load_rate_catalog(&root).expect("bench/rates.json is checked in")
    }

    #[test]
    fn priced_lane_matches_typescript_verdict() {
        let report =
            score_harbor_job(&fixture("priced-lane"), "tb2-cross-section", "proxy").unwrap();
        assert_eq!(report.trials_total, 4);
        assert_eq!(report.accepted, 2);
        assert_eq!(report.rejected, 2);
        assert_eq!(report.ungraded, 0);
        assert_eq!(report.success_rate, Some(0.5));
        // 75_000 prompt (40_000 cached) + 3_100 completion tokens of
        // gemini-3.7-flash at the pinned placeholder rates: $0.07875 total,
        // over 2 accepted outcomes.
        assert_eq!(report.cost_disposition, "known");
        assert_eq!(report.cost_coverage, "known");
        assert!((report.total_cost_usd.unwrap() - 0.07875).abs() < 1e-12);
        assert!((report.cost_per_accepted_outcome_usd.unwrap() - 0.039375).abs() < 1e-12);
        assert_eq!(report.rate_basis.as_deref(), Some("operator_placeholder"));
        assert_eq!(report.prompt_tokens, Some(75_000));
        assert_eq!(report.completion_tokens, Some(3_100));
        assert_eq!(report.cached_input_tokens, 40_000);
        assert_eq!(report.tool_calls, Some(5));
        assert_eq!(report.models, vec!["gemini-3.7-flash".to_string()]);
    }

    #[test]
    fn unpriced_model_keeps_cost_null_and_says_why() {
        let report = score_harbor_job(&fixture("unpriced-lane"), "fixture-suite", "proxy").unwrap();
        assert_eq!(report.accepted, 2);
        assert_eq!(report.cost_per_accepted_outcome_usd, None);
        assert_eq!(report.total_cost_usd, None);
        assert_eq!(report.cost_disposition, "cost_unknown");
        assert_eq!(report.cost_coverage, "unknown");
        assert!(
            report
                .unpriced_reasons
                .iter()
                .any(|r| r.contains("gpt-5.6-luna is unpriced")),
            "{:?}",
            report.unpriced_reasons
        );
    }

    #[test]
    fn partially_priced_run_withholds_the_figure() {
        // Two gemini trials price; the gpt-5.6-luna trial does not. A partial
        // numerator over every accepted outcome would understate the cost.
        let report = score_harbor_job(&fixture("mixed-lane"), "fixture-suite", "proxy").unwrap();
        assert_eq!(report.cost_coverage, "partial");
        assert_eq!(report.cost_disposition, "cost_partial");
        assert_eq!(report.cost_per_accepted_outcome_usd, None);
        assert!(report.total_cost_usd.is_some());
    }

    #[test]
    fn local_lane_stays_unmetered_not_priced() {
        let report = score_harbor_job(&fixture("priced-lane"), "fixture-suite", "local").unwrap();
        assert_eq!(report.cost_disposition, "unmetered_local_lane");
        assert_eq!(report.cost_coverage, "unmetered");
        assert_eq!(report.cost_per_accepted_outcome_usd, None);
        assert_eq!(report.total_cost_usd, None);
    }

    #[test]
    fn gate_passes_at_the_floor_and_fails_under_it_naming_the_criterion() {
        use crate::gym::suite::suite_meta_in;
        let meta = suite_meta_in(&fixture_suites_dir(), "fixture-suite").unwrap();
        let spec = meta.gate.as_ref().expect("fixture-suite declares a gate");

        // priced-lane: 2 of 4 accepted, exactly the 0.5 floor.
        let passing = score_harbor_job_with(
            &fixture("priced-lane"),
            "fixture-suite",
            "proxy",
            &catalog(),
        )
        .unwrap();
        let gate = evaluate_gate(&passing, &meta.id, spec);
        assert_eq!(gate.status, "passed");
        assert_eq!(gate.breaches().count(), 0);

        // regressed-lane: 1 of 4 accepted, under the floor. The verdict names
        // the breached criterion.
        let failing = score_harbor_job_with(
            &fixture("regressed-lane"),
            "fixture-suite",
            "proxy",
            &catalog(),
        )
        .unwrap();
        assert_eq!(failing.success_rate, Some(0.25));
        let gate = evaluate_gate(&failing, &meta.id, spec);
        assert_eq!(gate.status, "failed");
        let breached: Vec<_> = gate.breaches().map(|c| c.name.as_str()).collect();
        assert_eq!(breached, vec!["success_rate>=0.500"]);
    }

    #[test]
    fn cost_ceiling_on_an_unpriced_lane_is_unverifiable_not_a_pass() {
        use crate::gym::gate::GateSpec;
        let report = score_harbor_job_with(
            &fixture("unpriced-lane"),
            "fixture-suite",
            "proxy",
            &catalog(),
        )
        .unwrap();
        let spec = GateSpec {
            comment: None,
            min_graded_trials: 1,
            min_success_rate: 0.25,
            max_ungraded_ratio: 1.0,
            max_cost_per_accepted_outcome_usd: Some(1.0),
            accept_placeholder_rates: false,
        };
        let gate = evaluate_gate(&report, "fixture-suite", &spec);
        assert_eq!(gate.status, "unverifiable");
        let unverifiable: Vec<_> = gate.breaches().map(|c| c.name.as_str()).collect();
        assert_eq!(unverifiable, vec!["cost_per_accepted_outcome<=$1.0000"]);
    }

    #[test]
    fn built_row_carries_the_gate_verdict_and_the_computed_cost() {
        use crate::gym::suite::suite_meta_in;
        let meta = suite_meta_in(&fixture_suites_dir(), "fixture-suite").unwrap();
        let spec = meta.gate.clone().expect("fixture-suite declares a gate");

        let report = score_harbor_job_with(
            &fixture("regressed-lane"),
            "fixture-suite",
            "proxy",
            &catalog(),
        )
        .unwrap();
        let gate = evaluate_gate(&report, &meta.id, &spec);
        let row = build_result_row(&report, &meta, Some(&gate), None);
        assert_eq!(
            row.get("gateStatus").and_then(Value::as_str),
            Some("failed")
        );
        assert_eq!(
            row.get("thresholdsId").and_then(Value::as_str),
            Some("fixture-suite")
        );
        assert!(
            row.get("gateDigest")
                .and_then(Value::as_str)
                .is_some_and(|d| d.starts_with("gate:"))
        );
        let failures = row.get("gateFailures").and_then(Value::as_array).unwrap();
        assert_eq!(
            failures[0].get("name").and_then(Value::as_str),
            Some("success_rate>=0.500")
        );
        // 205_000 prompt (15_000 cached) + 5_400 completion gemini tokens,
        // one accepted outcome: the row states the figure, not null.
        assert!(row.get("costPerAcceptedOutcomeUsd").unwrap().is_f64());
        assert_eq!(
            row.get("costDisposition").and_then(Value::as_str),
            Some("known")
        );
        assert_eq!(
            row.get("rateBasis").and_then(Value::as_str),
            Some("operator_placeholder")
        );
        assert_eq!(
            row.get("promptTokens").and_then(Value::as_u64),
            Some(205_000)
        );
        // The receipt still covers the new fields.
        let receipt = row.get("receipt").and_then(Value::as_str).unwrap();
        assert_eq!(receipt, receipt_of(&row));
    }

    #[test]
    fn ungated_suite_keeps_gate_status_null_on_the_row() {
        use crate::gym::suite::suite_meta_in;
        let meta = suite_meta_in(&fixture_suites_dir(), "fixture-suite-3").unwrap();
        assert!(meta.gate.is_none(), "fixture-suite-3 must stay ungated");
        let report = score_harbor_job_with(
            &fixture("crashed-verifier"),
            "fixture-suite-3",
            "proxy",
            &catalog(),
        )
        .unwrap();
        let row = build_result_row(&report, &meta, None, None);
        assert!(row.get("gateStatus").unwrap().is_null());
        assert!(row.get("thresholdsId").unwrap().is_null());
        assert!(row.get("gateFailures").is_none());
    }

    #[test]
    fn harbor_unix_seconds_subtracts_the_qwen38_cross_section_span() {
        let start = harbor_unix_seconds("2026-08-28T14:48:43.879875").unwrap();
        let end = harbor_unix_seconds("2026-08-28T16:36:34.054776").unwrap();
        let delta = end - start;
        assert!((delta - 6470.174901).abs() < 1e-6, "wall clock was {delta}");
    }

    #[test]
    fn crashed_verifier_stays_out_of_the_denominator() {
        let report =
            score_harbor_job(&fixture("crashed-verifier"), "tb2-cross-section", "proxy").unwrap();
        assert_eq!(report.trials_total, 3);
        assert_eq!(report.accepted, 1);
        assert_eq!(report.rejected, 0);
        assert_eq!(report.ungraded, 2);
        assert_eq!(report.success_rate, Some(1.0));
        assert!((report.ungraded_ratio - 2.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn receipt_chain_verifies_a_hand_built_store() {
        let mut first = serde_json::json!({
            "schema": BENCH_RESULT_SCHEMA_V2,
            "recordedAt": "2026-08-25T10:00:00Z",
            "suite": "fixture",
            "lane": "proxy",
            "accepted": 1,
            "previousReceipt": null,
        });
        let receipt = receipt_of(&first);
        first["receipt"] = Value::String(receipt.clone());
        let mut second = serde_json::json!({
            "schema": BENCH_RESULT_SCHEMA_V2,
            "recordedAt": "2026-08-25T11:00:00Z",
            "suite": "fixture",
            "lane": "proxy",
            "accepted": 2,
            "previousReceipt": receipt,
        });
        let receipt2 = receipt_of(&second);
        second["receipt"] = Value::String(receipt2);
        match verify_result_chain(&[first.clone(), second]) {
            ChainVerdict::Ok { rows, .. } => assert_eq!(rows, 2),
            ChainVerdict::Break { detail, .. } => panic!("{detail}"),
        }
        first["accepted"] = json_u64(9);
        match verify_result_chain(&[first]) {
            ChainVerdict::Break { index, detail } => {
                assert_eq!(index, 0);
                assert!(detail.contains("edited after it was written"), "{detail}");
            }
            ChainVerdict::Ok { .. } => panic!("tampered row must not verify"),
        }
    }

    #[test]
    fn broken_previous_receipt_is_named() {
        let mut first = serde_json::json!({
            "schema": BENCH_RESULT_SCHEMA_V2,
            "recordedAt": "2026-08-25T10:00:00Z",
            "suite": "fixture",
            "lane": "proxy",
            "previousReceipt": null,
        });
        first["receipt"] = Value::String(receipt_of(&first));
        let mut second = serde_json::json!({
            "schema": BENCH_RESULT_SCHEMA_V2,
            "recordedAt": "2026-08-25T11:00:00Z",
            "suite": "fixture",
            "lane": "proxy",
            "previousReceipt": "receipt:deadbeef",
        });
        second["receipt"] = Value::String(receipt_of(&second));
        match verify_result_chain(&[first, second]) {
            ChainVerdict::Break { index, detail } => {
                assert_eq!(index, 1);
                assert!(
                    detail.contains("inserted, removed, or reordered"),
                    "{detail}"
                );
            }
            ChainVerdict::Ok { .. } => panic!("broken chain must stop"),
        }
    }
}
