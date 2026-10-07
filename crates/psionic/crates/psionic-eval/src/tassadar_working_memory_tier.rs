use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_runtime::{
    TASSADAR_WORKING_MEMORY_TIER_RUNTIME_REPORT_REF, TassadarWorkingMemoryCaseReport,
    TassadarWorkingMemoryKernelFamily, TassadarWorkingMemorySupportPosture,
    TassadarWorkingMemoryTierRuntimeReport,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_WORKING_MEMORY_TIER_EVAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_working_memory_tier_eval_report.json";

/// Eval-side classification of one working-memory case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkingMemoryOutcomeClass {
    /// The bounded working-memory tier materially widened efficient execution.
    EfficiencyWidening,
    /// The tier mostly reshaped trace state without enough gain to justify promotion.
    TraceShapingOnly,
    /// The tier refused the case under explicit boundaries.
    Refuse,
}

/// Family-level summary for the working-memory tier eval report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarWorkingMemoryFamilySummary {
    /// Stable workload family.
    pub workload_family: TassadarWorkingMemoryKernelFamily,
    /// Number of committed cases in the family.
    pub case_count: u32,
    /// Number of efficiency-widening cases in the family.
    pub widening_case_count: u32,
    /// Number of trace-shaping-only cases in the family.
    pub trace_shaping_only_case_count: u32,
    /// Number of refused cases in the family.
    pub refused_case_count: u32,
    /// Best direct speedup over pure trace inside the family.
    pub best_direct_speedup_over_pure_trace: f64,
    /// Plain-language family note.
    pub note: String,
}

/// Outcome projection for one committed working-memory case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarWorkingMemoryCaseOutcome {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable workload family.
    pub workload_family: TassadarWorkingMemoryKernelFamily,
    /// Eval-side outcome class.
    pub outcome_class: TassadarWorkingMemoryOutcomeClass,
    /// Pure-trace throughput.
    pub pure_trace_steps_per_second: f64,
    /// Working-memory throughput when direct.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_memory_steps_per_second: Option<f64>,
    /// Direct speedup over pure trace when direct.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speedup_over_pure_trace: Option<f64>,
    /// Direct trace-byte reduction when direct.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_byte_reduction_ratio: Option<f64>,
    /// Plain-language note.
    pub note: String,
}

/// Eval report for the working-memory tier lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarWorkingMemoryTierEvalReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable runtime report reference consumed by this eval report.
    pub runtime_report_ref: String,
    /// Stable runtime report digest consumed by this eval report.
    pub runtime_report_digest: String,
    /// Family-level summaries.
    pub family_summaries: Vec<TassadarWorkingMemoryFamilySummary>,
    /// Case-level outcome projections.
    pub case_outcomes: Vec<TassadarWorkingMemoryCaseOutcome>,
    /// Case identifiers where working memory materially widened efficient execution.
    pub widening_case_ids: Vec<String>,
    /// Case identifiers where working memory was trace-shaping-only.
    pub trace_shaping_only_case_ids: Vec<String>,
    /// Case identifiers refused under explicit boundaries.
    pub refused_case_ids: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Eval failure while building or writing the working-memory tier report.
#[derive(Debug, Error)]
pub enum TassadarWorkingMemoryTierEvalReportError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the machine-readable working-memory tier eval report.
pub fn build_tassadar_working_memory_tier_eval_report()
-> Result<TassadarWorkingMemoryTierEvalReport, TassadarWorkingMemoryTierEvalReportError> {
    let runtime_report: TassadarWorkingMemoryTierRuntimeReport =
        read_repo_json(TASSADAR_WORKING_MEMORY_TIER_RUNTIME_REPORT_REF)?;
    let family_summaries = build_family_summaries(runtime_report.case_reports.as_slice());
    let case_outcomes = runtime_report
        .case_reports
        .iter()
        .map(case_outcome)
        .collect::<Vec<_>>();
    let widening_case_ids = case_outcomes
        .iter()
        .filter(|outcome| {
            outcome.outcome_class == TassadarWorkingMemoryOutcomeClass::EfficiencyWidening
        })
        .map(|outcome| outcome.case_id.clone())
        .collect::<Vec<_>>();
    let trace_shaping_only_case_ids = case_outcomes
        .iter()
        .filter(|outcome| {
            outcome.outcome_class == TassadarWorkingMemoryOutcomeClass::TraceShapingOnly
        })
        .map(|outcome| outcome.case_id.clone())
        .collect::<Vec<_>>();
    let refused_case_ids = case_outcomes
        .iter()
        .filter(|outcome| outcome.outcome_class == TassadarWorkingMemoryOutcomeClass::Refuse)
        .map(|outcome| outcome.case_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarWorkingMemoryTierEvalReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.working_memory_tier.eval_report.v1"),
        runtime_report_ref: String::from(TASSADAR_WORKING_MEMORY_TIER_RUNTIME_REPORT_REF),
        runtime_report_digest: runtime_report.report_digest.clone(),
        family_summaries,
        case_outcomes,
        widening_case_ids,
        trace_shaping_only_case_ids,
        refused_case_ids,
        claim_boundary: String::from(
            "this eval report classifies one committed working-memory runtime artifact into efficiency-widening, trace-shaping-only, and refusal outcomes. It keeps exact bounded state publication separate from promotion and does not treat one bounded memory tier as proof of arbitrary-memory or general learned closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Working-memory tier eval report now classifies {} committed cases across {} workload families, keeping {} widening cases, {} trace-shaping-only cases, and {} refused cases explicit.",
        report.case_outcomes.len(),
        report.family_summaries.len(),
        report.widening_case_ids.len(),
        report.trace_shaping_only_case_ids.len(),
        report.refused_case_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_working_memory_tier_eval_report|",
        &report,
    );
    Ok(report)
}

/// Returns the canonical absolute path for the committed eval report.
#[must_use]
pub fn tassadar_working_memory_tier_eval_report_path() -> PathBuf {
    repo_root().join(TASSADAR_WORKING_MEMORY_TIER_EVAL_REPORT_REF)
}

/// Writes the committed working-memory tier eval report.
pub fn write_tassadar_working_memory_tier_eval_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarWorkingMemoryTierEvalReport, TassadarWorkingMemoryTierEvalReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarWorkingMemoryTierEvalReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_working_memory_tier_eval_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarWorkingMemoryTierEvalReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_family_summaries(
    case_reports: &[TassadarWorkingMemoryCaseReport],
) -> Vec<TassadarWorkingMemoryFamilySummary> {
    let mut grouped =
        BTreeMap::<TassadarWorkingMemoryKernelFamily, Vec<&TassadarWorkingMemoryCaseReport>>::new();
    for report in case_reports {
        grouped
            .entry(report.workload_family)
            .or_default()
            .push(report);
    }
    grouped
        .into_iter()
        .map(
            |(workload_family, reports)| TassadarWorkingMemoryFamilySummary {
                workload_family,
                case_count: reports.len() as u32,
                widening_case_count: reports
                    .iter()
                    .filter(|report| report.efficiency_widening)
                    .count() as u32,
                trace_shaping_only_case_count: reports
                    .iter()
                    .filter(|report| report.trace_shaping_only)
                    .count() as u32,
                refused_case_count: reports
                    .iter()
                    .filter(|report| {
                        report.working_memory_posture == TassadarWorkingMemorySupportPosture::Refuse
                    })
                    .count() as u32,
                best_direct_speedup_over_pure_trace: round_metric(
                    reports
                        .iter()
                        .filter_map(|report| report.working_memory.as_ref())
                        .map(|metrics| metrics.speedup_over_pure_trace)
                        .fold(0.0, f64::max),
                ),
                note: family_note(workload_family, reports.as_slice()),
            },
        )
        .collect()
}

fn case_outcome(case: &TassadarWorkingMemoryCaseReport) -> TassadarWorkingMemoryCaseOutcome {
    let outcome_class =
        if case.working_memory_posture == TassadarWorkingMemorySupportPosture::Refuse {
            TassadarWorkingMemoryOutcomeClass::Refuse
        } else if case.efficiency_widening {
            TassadarWorkingMemoryOutcomeClass::EfficiencyWidening
        } else {
            TassadarWorkingMemoryOutcomeClass::TraceShapingOnly
        };
    TassadarWorkingMemoryCaseOutcome {
        case_id: case.case_id.clone(),
        workload_family: case.workload_family,
        outcome_class,
        pure_trace_steps_per_second: case.pure_trace.steps_per_second,
        working_memory_steps_per_second: case
            .working_memory
            .as_ref()
            .map(|metrics| metrics.steps_per_second),
        speedup_over_pure_trace: case
            .working_memory
            .as_ref()
            .map(|metrics| metrics.speedup_over_pure_trace),
        trace_byte_reduction_ratio: case
            .working_memory
            .as_ref()
            .map(|metrics| metrics.trace_byte_reduction_ratio),
        note: case.note.clone(),
    }
}

fn family_note(
    workload_family: TassadarWorkingMemoryKernelFamily,
    reports: &[&TassadarWorkingMemoryCaseReport],
) -> String {
    let widening_case_count = reports
        .iter()
        .filter(|report| report.efficiency_widening)
        .count();
    let refused_case_count = reports
        .iter()
        .filter(|report| {
            report.working_memory_posture == TassadarWorkingMemorySupportPosture::Refuse
        })
        .count();
    let trace_shaping_only_case_count = reports
        .iter()
        .filter(|report| report.trace_shaping_only)
        .count();
    match workload_family {
        TassadarWorkingMemoryKernelFamily::CopyWindow => String::from(
            "copy-window kernels look like a credible bounded widening case under explicit slot publication",
        ),
        TassadarWorkingMemoryKernelFamily::StableSort => String::from(
            "stable-sort kernels currently look more like trace reshaping than a genuine widening of efficient execution",
        ),
        TassadarWorkingMemoryKernelFamily::AssociativeRecall => format!(
            "associative-recall kernels currently show {} widening case(s) but also {} refused overflow case(s), so the gain remains bounded and fragile",
            widening_case_count, refused_case_count
        ),
        TassadarWorkingMemoryKernelFamily::LongCarryAccumulator => format!(
            "long carry-propagation kernels currently show {} widening case(s) with {} trace-shaping-only case(s)",
            widening_case_count, trace_shaping_only_case_count
        ),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarWorkingMemoryTierEvalReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarWorkingMemoryTierEvalReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarWorkingMemoryTierEvalReportError::Deserialize {
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

fn round_metric(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_WORKING_MEMORY_TIER_EVAL_REPORT_REF, TassadarWorkingMemoryOutcomeClass,
        TassadarWorkingMemoryTierEvalReport, build_tassadar_working_memory_tier_eval_report,
        read_repo_json, tassadar_working_memory_tier_eval_report_path,
        write_tassadar_working_memory_tier_eval_report,
    };
    use psionic_runtime::TassadarWorkingMemoryKernelFamily;

    #[test]
    fn working_memory_eval_report_keeps_widening_trace_shaping_and_refusal_explicit() {
        let report = build_tassadar_working_memory_tier_eval_report().expect("report");

        assert!(report.case_outcomes.iter().any(|case| {
            case.case_id == "copy_window_256"
                && case.outcome_class == TassadarWorkingMemoryOutcomeClass::EfficiencyWidening
        }));
        assert!(report.case_outcomes.iter().any(|case| {
            case.workload_family == TassadarWorkingMemoryKernelFamily::StableSort
                && case.outcome_class == TassadarWorkingMemoryOutcomeClass::TraceShapingOnly
        }));
        assert!(report.case_outcomes.iter().any(|case| {
            case.case_id == "associative_recall_overflow"
                && case.outcome_class == TassadarWorkingMemoryOutcomeClass::Refuse
        }));
    }

    #[test]
    fn working_memory_eval_report_matches_committed_truth() {
        let generated = build_tassadar_working_memory_tier_eval_report().expect("report");
        let committed: TassadarWorkingMemoryTierEvalReport =
            read_repo_json(TASSADAR_WORKING_MEMORY_TIER_EVAL_REPORT_REF).expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_working_memory_eval_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_working_memory_tier_eval_report.json");
        let written =
            write_tassadar_working_memory_tier_eval_report(&output_path).expect("write report");
        let persisted: TassadarWorkingMemoryTierEvalReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_working_memory_tier_eval_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_working_memory_tier_eval_report.json")
        );
    }
}
