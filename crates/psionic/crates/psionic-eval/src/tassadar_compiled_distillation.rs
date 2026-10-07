use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    tassadar_compiled_distillation_contract, TassadarCompiledDistillationMode,
    TassadarCompiledDistillationWorkloadFamily, TASSADAR_COMPILED_DISTILLATION_REPORT_REF,
    TASSADAR_COMPILED_DISTILLATION_TRAINING_EVIDENCE_BUNDLE_REF,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompiledDistillationSupportPosture {
    Supported,
    Refuse,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationRegimeEvidence {
    pub workload_family: TassadarCompiledDistillationWorkloadFamily,
    pub regime: TassadarCompiledDistillationMode,
    pub final_output_exactness_bps: u32,
    pub later_window_exactness_bps: u32,
    pub held_out_family_exactness_bps: u32,
    pub support_posture: TassadarCompiledDistillationSupportPosture,
    pub authority_case_id: String,
    pub evidence_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationInvarianceAblation {
    pub workload_family: TassadarCompiledDistillationWorkloadFamily,
    pub mixed_with_invariance_later_window_bps: u32,
    pub mixed_without_invariance_later_window_bps: u32,
    pub delta_bps: i32,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationTrainingEvidenceBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub contract_digest: String,
    pub target_bundle_digest: String,
    pub regime_evidence: Vec<TassadarCompiledDistillationRegimeEvidence>,
    pub invariance_ablations: Vec<TassadarCompiledDistillationInvarianceAblation>,
    pub claim_boundary: String,
    pub bundle_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationRegimeSummary {
    pub regime: TassadarCompiledDistillationMode,
    pub mean_final_output_exactness_bps: u32,
    pub mean_later_window_exactness_bps: u32,
    pub mean_held_out_family_exactness_bps: u32,
    pub refusal_workload_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationWorkloadSummary {
    pub workload_family: TassadarCompiledDistillationWorkloadFamily,
    pub best_regime: TassadarCompiledDistillationMode,
    pub mixed_distillation_gain_over_io_only_bps: i32,
    pub full_trace_gap_bps: i32,
    pub io_only_refused: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationReport {
    pub schema_version: u16,
    pub report_id: String,
    pub contract_digest: String,
    pub evidence_bundle: TassadarCompiledDistillationTrainingEvidenceBundle,
    pub regime_summaries: Vec<TassadarCompiledDistillationRegimeSummary>,
    pub workload_summaries: Vec<TassadarCompiledDistillationWorkloadSummary>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarCompiledDistillationReport {
    fn new(
        evidence_bundle: TassadarCompiledDistillationTrainingEvidenceBundle,
        regime_summaries: Vec<TassadarCompiledDistillationRegimeSummary>,
        workload_summaries: Vec<TassadarCompiledDistillationWorkloadSummary>,
    ) -> Self {
        let contract = tassadar_compiled_distillation_contract();
        let io_only_refusal_count = workload_summaries
            .iter()
            .filter(|summary| summary.io_only_refused)
            .count();
        let mixed_wins = workload_summaries
            .iter()
            .filter(|summary| summary.mixed_distillation_gain_over_io_only_bps > 0)
            .count();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.compiled_distillation.report.v1"),
            contract_digest: contract.contract_digest,
            evidence_bundle,
            regime_summaries,
            workload_summaries,
            claim_boundary: String::from(
                "this report compares lighter supervision regimes against full-trace supervision on bounded compiled/reference-backed workload families only. It keeps weaker supervision, later-window degradation, and explicit refusal separate from any broader learned closure claim",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Compiled distillation report now compares {} supervision regimes across {} workload families, with mixed distillation beating io-only on {} families and io-only refusal remaining explicit on {} families.",
            report.regime_summaries.len(),
            report.workload_summaries.len(),
            mixed_wins,
            io_only_refusal_count,
        );
        report.report_digest =
            stable_digest(b"psionic_tassadar_compiled_distillation_report|", &report);
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarCompiledDistillationReportError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_compiled_distillation_report(
) -> Result<TassadarCompiledDistillationReport, TassadarCompiledDistillationReportError> {
    let evidence_bundle: TassadarCompiledDistillationTrainingEvidenceBundle =
        read_repo_json(TASSADAR_COMPILED_DISTILLATION_TRAINING_EVIDENCE_BUNDLE_REF)?;
    let regime_summaries = build_regime_summaries(&evidence_bundle.regime_evidence);
    let workload_summaries = build_workload_summaries(&evidence_bundle.regime_evidence);
    Ok(TassadarCompiledDistillationReport::new(
        evidence_bundle,
        regime_summaries,
        workload_summaries,
    ))
}

#[must_use]
pub fn tassadar_compiled_distillation_report_path() -> PathBuf {
    repo_root().join(TASSADAR_COMPILED_DISTILLATION_REPORT_REF)
}

pub fn write_tassadar_compiled_distillation_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarCompiledDistillationReport, TassadarCompiledDistillationReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarCompiledDistillationReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_compiled_distillation_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarCompiledDistillationReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_regime_summaries(
    evidence: &[TassadarCompiledDistillationRegimeEvidence],
) -> Vec<TassadarCompiledDistillationRegimeSummary> {
    let mut grouped =
        BTreeMap::<TassadarCompiledDistillationMode, Vec<&TassadarCompiledDistillationRegimeEvidence>>::new();
    for row in evidence {
        grouped.entry(row.regime).or_default().push(row);
    }
    grouped
        .into_iter()
        .map(|(regime, rows)| TassadarCompiledDistillationRegimeSummary {
            regime,
            mean_final_output_exactness_bps: mean_u32(
                rows.iter().map(|row| row.final_output_exactness_bps),
            ),
            mean_later_window_exactness_bps: mean_u32(
                rows.iter().map(|row| row.later_window_exactness_bps),
            ),
            mean_held_out_family_exactness_bps: mean_u32(
                rows.iter().map(|row| row.held_out_family_exactness_bps),
            ),
            refusal_workload_count: rows
                .iter()
                .filter(|row| row.support_posture == TassadarCompiledDistillationSupportPosture::Refuse)
                .count() as u32,
        })
        .collect()
}

fn build_workload_summaries(
    evidence: &[TassadarCompiledDistillationRegimeEvidence],
) -> Vec<TassadarCompiledDistillationWorkloadSummary> {
    let mut grouped =
        BTreeMap::<TassadarCompiledDistillationWorkloadFamily, Vec<&TassadarCompiledDistillationRegimeEvidence>>::new();
    for row in evidence {
        grouped.entry(row.workload_family).or_default().push(row);
    }
    grouped
        .into_iter()
        .map(|(workload_family, rows)| {
            let best = rows
                .iter()
                .filter(|row| row.support_posture == TassadarCompiledDistillationSupportPosture::Supported)
                .max_by_key(|row| row.held_out_family_exactness_bps)
                .expect("supported regime should exist");
            let io_only = rows
                .iter()
                .find(|row| row.regime == TassadarCompiledDistillationMode::IoOnly)
                .expect("io_only row");
            let mixed = rows
                .iter()
                .find(|row| row.regime == TassadarCompiledDistillationMode::MixedDistillation)
                .expect("mixed row");
            let full_trace = rows
                .iter()
                .find(|row| row.regime == TassadarCompiledDistillationMode::FullTrace)
                .expect("full_trace row");
            TassadarCompiledDistillationWorkloadSummary {
                workload_family,
                best_regime: best.regime,
                mixed_distillation_gain_over_io_only_bps: mixed.held_out_family_exactness_bps as i32
                    - io_only.held_out_family_exactness_bps as i32,
                full_trace_gap_bps: full_trace.held_out_family_exactness_bps as i32
                    - mixed.held_out_family_exactness_bps as i32,
                io_only_refused: io_only.support_posture
                    == TassadarCompiledDistillationSupportPosture::Refuse,
            }
        })
        .collect()
}

fn mean_u32(values: impl Iterator<Item = u32>) -> u32 {
    let collected = values.collect::<Vec<_>>();
    if collected.is_empty() {
        0
    } else {
        collected.iter().copied().sum::<u32>() / collected.len() as u32
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, TassadarCompiledDistillationReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarCompiledDistillationReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarCompiledDistillationReportError::Deserialize {
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
        build_tassadar_compiled_distillation_report, read_repo_json,
        tassadar_compiled_distillation_report_path, write_tassadar_compiled_distillation_report,
        TassadarCompiledDistillationReport,
    };
    use psionic_data::{
        TassadarCompiledDistillationMode, TassadarCompiledDistillationWorkloadFamily,
        TASSADAR_COMPILED_DISTILLATION_REPORT_REF,
    };

    #[test]
    fn compiled_distillation_report_keeps_mixed_vs_io_and_refusal_explicit() {
        let report = build_tassadar_compiled_distillation_report().expect("report");

        assert_eq!(report.regime_summaries.len(), 5);
        assert!(report.workload_summaries.iter().any(|summary| {
            summary.workload_family == TassadarCompiledDistillationWorkloadFamily::SudokuSearch
                && summary.best_regime == TassadarCompiledDistillationMode::FullTrace
                && summary.io_only_refused
        }));
    }

    #[test]
    fn compiled_distillation_report_matches_committed_truth() {
        let generated = build_tassadar_compiled_distillation_report().expect("report");
        let committed: TassadarCompiledDistillationReport =
            read_repo_json(TASSADAR_COMPILED_DISTILLATION_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_compiled_distillation_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_compiled_distillation_report.json");
        let written = write_tassadar_compiled_distillation_report(&output_path)
            .expect("write report");
        let persisted: TassadarCompiledDistillationReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_compiled_distillation_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_compiled_distillation_report.json")
        );
    }
}
