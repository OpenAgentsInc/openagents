use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_BUNDLE_REF,
    TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_REPORT_REF, TassadarCallStackHeapGeneralizationSplit,
    TassadarCallStackHeapModelVariant, TassadarCallStackHeapWorkloadFamily,
    TassadarLearnedCallStackHeapEvidenceCase, TassadarLearnedCallStackHeapSuiteBundle,
};
use psionic_models::tassadar_learned_call_stack_heap_suite_publication;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnedCallStackHeapVariantSummary {
    pub model_variant: TassadarCallStackHeapModelVariant,
    pub mean_later_window_exactness_bps: u32,
    pub mean_final_output_exactness_bps: u32,
    pub mean_refusal_calibration_bps: u32,
    pub held_out_mean_exactness_bps: u32,
    pub max_supported_call_depth: u32,
    pub max_supported_heap_cells: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnedCallStackHeapWorkloadSummary {
    pub workload_family: TassadarCallStackHeapWorkloadFamily,
    pub split: TassadarCallStackHeapGeneralizationSplit,
    pub better_variant: TassadarCallStackHeapModelVariant,
    pub structured_gain_vs_baseline_bps: i32,
    pub baseline_later_window_exactness_bps: u32,
    pub structured_later_window_exactness_bps: u32,
    pub refusal_gain_vs_baseline_bps: i32,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnedCallStackHeapSuiteReport {
    pub schema_version: u16,
    pub report_id: String,
    pub publication_digest: String,
    pub contract_ref: String,
    pub contract_digest: String,
    pub evidence_bundle_ref: String,
    pub evidence_bundle_digest: String,
    pub variant_summaries: Vec<TassadarLearnedCallStackHeapVariantSummary>,
    pub workload_summaries: Vec<TassadarLearnedCallStackHeapWorkloadSummary>,
    pub held_out_fragile_on_baseline: Vec<TassadarCallStackHeapWorkloadFamily>,
    pub structured_recoverable_workloads: Vec<TassadarCallStackHeapWorkloadFamily>,
    pub call_stack_heavy_workloads: Vec<TassadarCallStackHeapWorkloadFamily>,
    pub heap_heavy_workloads: Vec<TassadarCallStackHeapWorkloadFamily>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarLearnedCallStackHeapSuiteReportError {
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

pub fn build_tassadar_learned_call_stack_heap_suite_report()
-> Result<TassadarLearnedCallStackHeapSuiteReport, TassadarLearnedCallStackHeapSuiteReportError> {
    let publication = tassadar_learned_call_stack_heap_suite_publication();
    let bundle: TassadarLearnedCallStackHeapSuiteBundle =
        read_repo_json(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_BUNDLE_REF)?;
    let variant_summaries = build_variant_summaries(bundle.case_reports.as_slice());
    let workload_summaries = build_workload_summaries(bundle.case_reports.as_slice());
    let held_out_fragile_on_baseline = workload_summaries
        .iter()
        .filter(|summary| summary.split == TassadarCallStackHeapGeneralizationSplit::HeldOutFamily)
        .filter(|summary| summary.baseline_later_window_exactness_bps < 5_000)
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let structured_recoverable_workloads = workload_summaries
        .iter()
        .filter(|summary| summary.structured_gain_vs_baseline_bps >= 2_000)
        .filter(|summary| summary.structured_later_window_exactness_bps >= 6_500)
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let call_stack_heavy_workloads = bundle
        .case_reports
        .iter()
        .filter(|case| case.max_call_depth >= 16)
        .map(|case| case.workload_family)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let heap_heavy_workloads = bundle
        .case_reports
        .iter()
        .filter(|case| case.max_heap_cells >= 128)
        .map(|case| case.workload_family)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut report = TassadarLearnedCallStackHeapSuiteReport {
        schema_version: 1,
        report_id: String::from("tassadar.learned_call_stack_heap_suite.report.v1"),
        publication_digest: publication.publication_digest,
        contract_ref: bundle.contract.contract_ref.clone(),
        contract_digest: bundle.contract.contract_digest.clone(),
        evidence_bundle_ref: String::from(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_BUNDLE_REF),
        evidence_bundle_digest: bundle.report_digest.clone(),
        variant_summaries,
        workload_summaries,
        held_out_fragile_on_baseline,
        structured_recoverable_workloads,
        call_stack_heavy_workloads,
        heap_heavy_workloads,
        claim_boundary: String::from(
            "this report compares two learned call-stack/heap variants on seeded in-family and held-out-family workloads. It keeps held-out failures, later-window drift, and refusal calibration explicit instead of promoting the learned lane into broad process or general compute closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Learned call-stack/heap suite compares {} variants across {} workloads, with {} baseline-fragile held-out workloads and {} structured-recoverable workloads.",
        report.variant_summaries.len(),
        report.workload_summaries.len(),
        report.held_out_fragile_on_baseline.len(),
        report.structured_recoverable_workloads.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_learned_call_stack_heap_suite_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_learned_call_stack_heap_suite_report_path() -> PathBuf {
    repo_root().join(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_REPORT_REF)
}

pub fn write_tassadar_learned_call_stack_heap_suite_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarLearnedCallStackHeapSuiteReport, TassadarLearnedCallStackHeapSuiteReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarLearnedCallStackHeapSuiteReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_learned_call_stack_heap_suite_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarLearnedCallStackHeapSuiteReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_variant_summaries(
    cases: &[TassadarLearnedCallStackHeapEvidenceCase],
) -> Vec<TassadarLearnedCallStackHeapVariantSummary> {
    let mut grouped = BTreeMap::<
        TassadarCallStackHeapModelVariant,
        Vec<&TassadarLearnedCallStackHeapEvidenceCase>,
    >::new();
    for case in cases {
        grouped.entry(case.model_variant).or_default().push(case);
    }
    grouped
        .into_iter()
        .map(
            |(model_variant, rows)| TassadarLearnedCallStackHeapVariantSummary {
                model_variant,
                mean_later_window_exactness_bps: mean(
                    rows.iter().map(|row| row.later_window_exactness_bps),
                ),
                mean_final_output_exactness_bps: mean(
                    rows.iter().map(|row| row.final_output_exactness_bps),
                ),
                mean_refusal_calibration_bps: mean(
                    rows.iter().map(|row| row.refusal_calibration_bps),
                ),
                held_out_mean_exactness_bps: mean(
                    rows.iter()
                        .filter(|row| {
                            row.split == TassadarCallStackHeapGeneralizationSplit::HeldOutFamily
                        })
                        .map(|row| row.later_window_exactness_bps),
                ),
                max_supported_call_depth: rows
                    .iter()
                    .map(|row| row.max_call_depth)
                    .max()
                    .unwrap_or(0),
                max_supported_heap_cells: rows
                    .iter()
                    .map(|row| row.max_heap_cells)
                    .max()
                    .unwrap_or(0),
            },
        )
        .collect()
}

fn build_workload_summaries(
    cases: &[TassadarLearnedCallStackHeapEvidenceCase],
) -> Vec<TassadarLearnedCallStackHeapWorkloadSummary> {
    let mut grouped = BTreeMap::<
        TassadarCallStackHeapWorkloadFamily,
        Vec<&TassadarLearnedCallStackHeapEvidenceCase>,
    >::new();
    for case in cases {
        grouped.entry(case.workload_family).or_default().push(case);
    }
    grouped
        .into_iter()
        .map(|(workload_family, rows)| {
            let baseline = row_for_variant(
                rows.as_slice(),
                TassadarCallStackHeapModelVariant::BaselineTransformer,
            );
            let structured = row_for_variant(
                rows.as_slice(),
                TassadarCallStackHeapModelVariant::StructuredMemory,
            );
            TassadarLearnedCallStackHeapWorkloadSummary {
                workload_family,
                split: baseline.split,
                better_variant: if structured.later_window_exactness_bps
                    >= baseline.later_window_exactness_bps
                {
                    TassadarCallStackHeapModelVariant::StructuredMemory
                } else {
                    TassadarCallStackHeapModelVariant::BaselineTransformer
                },
                structured_gain_vs_baseline_bps: structured.later_window_exactness_bps as i32
                    - baseline.later_window_exactness_bps as i32,
                baseline_later_window_exactness_bps: baseline.later_window_exactness_bps,
                structured_later_window_exactness_bps: structured.later_window_exactness_bps,
                refusal_gain_vs_baseline_bps: structured.refusal_calibration_bps as i32
                    - baseline.refusal_calibration_bps as i32,
                note: format!(
                    "baseline failure `{}` vs structured failure `{}`",
                    baseline.dominant_failure_mode, structured.dominant_failure_mode
                ),
            }
        })
        .collect()
}

fn row_for_variant<'a>(
    rows: &'a [&TassadarLearnedCallStackHeapEvidenceCase],
    model_variant: TassadarCallStackHeapModelVariant,
) -> &'a TassadarLearnedCallStackHeapEvidenceCase {
    rows.iter()
        .copied()
        .find(|row| row.model_variant == model_variant)
        .expect("variant row should exist")
}

fn mean(values: impl Iterator<Item = u32>) -> u32 {
    let values = values.map(u64::from).collect::<Vec<_>>();
    if values.is_empty() {
        0
    } else {
        (values.iter().sum::<u64>() / values.len() as u64) as u32
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarLearnedCallStackHeapSuiteReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarLearnedCallStackHeapSuiteReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarLearnedCallStackHeapSuiteReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_learned_call_stack_heap_suite_report,
        tassadar_learned_call_stack_heap_suite_report_path,
        write_tassadar_learned_call_stack_heap_suite_report,
    };
    use psionic_data::{
        TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_REPORT_REF, TassadarCallStackHeapWorkloadFamily,
    };

    #[test]
    fn learned_call_stack_heap_suite_report_marks_held_out_fragility_and_structured_recovery() {
        let report = build_tassadar_learned_call_stack_heap_suite_report().expect("report");

        assert_eq!(report.variant_summaries.len(), 2);
        assert_eq!(report.workload_summaries.len(), 7);
        assert!(
            report
                .held_out_fragile_on_baseline
                .contains(&TassadarCallStackHeapWorkloadFamily::HeldOutContinuationMachine)
        );
        assert!(
            report
                .structured_recoverable_workloads
                .contains(&TassadarCallStackHeapWorkloadFamily::HeldOutAllocatorScheduler)
        );
    }

    #[test]
    fn learned_call_stack_heap_suite_report_matches_committed_truth() {
        let generated = build_tassadar_learned_call_stack_heap_suite_report().expect("report");
        let committed: super::TassadarLearnedCallStackHeapSuiteReport =
            super::read_repo_json(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_REPORT_REF)
                .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_learned_call_stack_heap_suite_report_persists_current_truth() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output_path = output_dir
            .path()
            .join("tassadar_learned_call_stack_heap_suite_report.json");
        let report = write_tassadar_learned_call_stack_heap_suite_report(&output_path)
            .expect("write report");
        let written: super::TassadarLearnedCallStackHeapSuiteReport =
            serde_json::from_str(&std::fs::read_to_string(&output_path).expect("written file"))
                .expect("parse");

        assert_eq!(report, written);
        assert_eq!(
            tassadar_learned_call_stack_heap_suite_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_learned_call_stack_heap_suite_report.json")
        );
    }
}
