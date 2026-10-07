use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use psionic_train::{
    build_tassadar_trace_family_set_contract, execute_tassadar_trace_family_comparison,
    TassadarTraceFamilyComparisonError, TASSADAR_TRACE_FAMILY_COMPARISON_OUTPUT_DIR,
    TASSADAR_TRACE_FAMILY_COMPARISON_REPORT_REF,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_TRACE_FAMILY_VARIANT_REPORT_OUTPUT_DIR: &str = "fixtures/tassadar/reports";
pub const TASSADAR_TRACE_FAMILY_VARIANT_REPORT_FILE: &str =
    "tassadar_trace_family_variant_report.json";
pub const TASSADAR_TRACE_FAMILY_VARIANT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_trace_family_variant_report.json";
pub const TASSADAR_TRACE_FAMILY_VARIANT_EXAMPLE_COMMAND: &str =
    "cargo run -p psionic-research --example tassadar_trace_family_comparison";
pub const TASSADAR_TRACE_FAMILY_VARIANT_TEST_COMMAND: &str =
    "cargo test -p psionic-research trace_family_variant_report_matches_committed_truth -- --nocapture";

const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceFamilyVariantWorkloadDelta {
    pub workload: String,
    pub sequential_trace_family: String,
    pub alternate_trace_family: String,
    pub sequential_max_total_tokens: u32,
    pub alternate_max_total_tokens: u32,
    pub max_total_token_reduction_bps: u32,
    pub sequential_fit_cases: u32,
    pub alternate_fit_cases: u32,
    pub fit_case_delta: i32,
    pub alternate_matches_final_output_exactness: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceFamilyVariantReport {
    pub schema_version: u16,
    pub comparison_id: String,
    pub report_ref: String,
    pub comparison_output_ref: String,
    pub comparison_report_ref: String,
    pub comparison_report_digest: String,
    pub regeneration_commands: Vec<String>,
    pub trace_family_set_ref: String,
    pub trace_family_set_version: String,
    pub trace_family_set_digest: String,
    pub dataset_version: String,
    pub current_model_context_limit: u32,
    pub workload_deltas: Vec<TassadarTraceFamilyVariantWorkloadDelta>,
    pub alternate_fit_win_workload_count: u32,
    pub alternate_final_output_exact_workload_count: u32,
    pub claim_class: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarTraceFamilyVariantReportError {
    #[error(transparent)]
    Train(#[from] TassadarTraceFamilyComparisonError),
    #[error(transparent)]
    TraceFamilySet(#[from] psionic_data::TassadarTraceFamilySetError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to remove temporary directory `{path}`: {error}")]
    Cleanup { path: String, error: std::io::Error },
}

pub fn build_tassadar_trace_family_variant_report(
) -> Result<TassadarTraceFamilyVariantReport, TassadarTraceFamilyVariantReportError> {
    let temp_dir = create_temp_dir()?;
    let comparison = execute_tassadar_trace_family_comparison(&temp_dir)?;
    fs::remove_dir_all(&temp_dir).map_err(|error| {
        TassadarTraceFamilyVariantReportError::Cleanup {
            path: temp_dir.display().to_string(),
            error,
        }
    })?;

    let trace_family_set = build_tassadar_trace_family_set_contract()?;
    let workload_deltas = comparison
        .workload_comparisons
        .iter()
        .map(|comparison| {
            let sequential_tokens = comparison.sequential_cpu_reference.total_token_count_max;
            let alternate_tokens = comparison.alternate_trace_family.total_token_count_max;
            let reduction_bps = if sequential_tokens == 0 {
                0
            } else {
                (((u64::from(sequential_tokens.saturating_sub(alternate_tokens))) * 10_000)
                    / u64::from(sequential_tokens)) as u32
            };
            TassadarTraceFamilyVariantWorkloadDelta {
                workload: comparison.workload.clone(),
                sequential_trace_family: comparison
                    .sequential_cpu_reference
                    .trace_family
                    .label()
                    .to_string(),
                alternate_trace_family: comparison
                    .alternate_trace_family
                    .trace_family
                    .label()
                    .to_string(),
                sequential_max_total_tokens: sequential_tokens,
                alternate_max_total_tokens: alternate_tokens,
                max_total_token_reduction_bps: reduction_bps,
                sequential_fit_cases: comparison
                    .sequential_cpu_reference
                    .fits_current_model_context_case_count,
                alternate_fit_cases: comparison
                    .alternate_trace_family
                    .fits_current_model_context_case_count,
                fit_case_delta: comparison
                    .alternate_trace_family
                    .fits_current_model_context_case_count as i32
                    - comparison
                        .sequential_cpu_reference
                        .fits_current_model_context_case_count as i32,
                alternate_matches_final_output_exactness: comparison
                    .alternate_matches_final_output_exactness,
            }
        })
        .collect::<Vec<_>>();
    let alternate_fit_win_workload_count = workload_deltas
        .iter()
        .filter(|delta| delta.fit_case_delta > 0)
        .count() as u32;
    let alternate_final_output_exact_workload_count = workload_deltas
        .iter()
        .filter(|delta| delta.alternate_matches_final_output_exactness)
        .count() as u32;

    let mut report = TassadarTraceFamilyVariantReport {
        schema_version: REPORT_SCHEMA_VERSION,
        comparison_id: String::from("tassadar.parallel_trace_variants.v0"),
        report_ref: String::from(TASSADAR_TRACE_FAMILY_VARIANT_REPORT_REF),
        comparison_output_ref: String::from(TASSADAR_TRACE_FAMILY_COMPARISON_OUTPUT_DIR),
        comparison_report_ref: String::from(TASSADAR_TRACE_FAMILY_COMPARISON_REPORT_REF),
        comparison_report_digest: comparison.report_digest.clone(),
        regeneration_commands: vec![
            String::from(TASSADAR_TRACE_FAMILY_VARIANT_EXAMPLE_COMMAND),
            String::from(TASSADAR_TRACE_FAMILY_VARIANT_TEST_COMMAND),
        ],
        trace_family_set_ref: trace_family_set.trace_family_set_ref.clone(),
        trace_family_set_version: trace_family_set.version.clone(),
        trace_family_set_digest: trace_family_set.stable_digest(),
        dataset_version: comparison.dataset_version.clone(),
        current_model_context_limit: comparison.current_model_context_limit,
        workload_deltas,
        alternate_fit_win_workload_count,
        alternate_final_output_exact_workload_count,
        claim_class: String::from("learned_bounded_success"),
        claim_boundary: String::from(
            "this report compares sequential CPU-style targets against alternate wavefront or frontier target families on the seeded Sudoku and Hungarian corpora only; alternate families are still research-only final-output targets and not full CPU-trace authority or served-lane capability claims",
        ),
        summary: format!(
            "Public parallel-trace-variant report now freezes {} same-corpus Tassadar workload comparisons at context_limit={}: alternate families preserve final outputs on {}/{} workloads, improve context fit on {} workloads, and remain explicitly research-only target families while the sequential CPU trace stays the only full authority.",
            comparison.workload_comparisons.len(),
            comparison.current_model_context_limit,
            alternate_final_output_exact_workload_count,
            comparison.workload_comparisons.len(),
            alternate_fit_win_workload_count,
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(b"psionic_tassadar_trace_family_variant_report|", &report);
    Ok(report)
}

pub fn run_tassadar_trace_family_variant_report(
    output_dir: &Path,
) -> Result<TassadarTraceFamilyVariantReport, TassadarTraceFamilyVariantReportError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarTraceFamilyVariantReportError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let report = build_tassadar_trace_family_variant_report()?;
    let report_path = output_dir.join(TASSADAR_TRACE_FAMILY_VARIANT_REPORT_FILE);
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(&report_path, bytes).map_err(|error| {
        TassadarTraceFamilyVariantReportError::Write {
            path: report_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn create_temp_dir() -> Result<PathBuf, TassadarTraceFamilyVariantReportError> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "psionic-tassadar-trace-family-comparison-{}-{}",
        std::process::id(),
        unique
    ));
    fs::create_dir_all(&path).map_err(|error| {
        TassadarTraceFamilyVariantReportError::CreateDir {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(path)
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).expect("trace family variant report should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde::de::DeserializeOwned;
    use tempfile::tempdir;

    use super::{
        build_tassadar_trace_family_variant_report, run_tassadar_trace_family_variant_report,
        TassadarTraceFamilyVariantReport, TASSADAR_TRACE_FAMILY_VARIANT_REPORT_OUTPUT_DIR,
        TASSADAR_TRACE_FAMILY_VARIANT_REPORT_REF,
    };

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    fn read_repo_json<T>(repo_relative_path: &str) -> Result<T, Box<dyn std::error::Error>>
    where
        T: DeserializeOwned,
    {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn trace_family_variant_report_preserves_final_outputs(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_trace_family_variant_report()?;
        assert_eq!(report.workload_deltas.len(), 4);
        assert!(report
            .workload_deltas
            .iter()
            .all(|delta| delta.alternate_matches_final_output_exactness));
        assert!(report
            .workload_deltas
            .iter()
            .all(|delta| delta.max_total_token_reduction_bps >= 9500));
        Ok(())
    }

    #[test]
    fn trace_family_variant_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_trace_family_variant_report()?;
        let persisted: TassadarTraceFamilyVariantReport =
            read_repo_json(TASSADAR_TRACE_FAMILY_VARIANT_REPORT_REF)?;
        assert_eq!(persisted, report);
        Ok(())
    }

    #[test]
    fn trace_family_variant_report_writes_current_truth() -> Result<(), Box<dyn std::error::Error>>
    {
        let output_dir = tempdir()?;
        let report = run_tassadar_trace_family_variant_report(output_dir.path())?;
        let persisted: TassadarTraceFamilyVariantReport = serde_json::from_slice(&std::fs::read(
            output_dir
                .path()
                .join("tassadar_trace_family_variant_report.json"),
        )?)?;
        assert_eq!(persisted, report);
        assert!(TASSADAR_TRACE_FAMILY_VARIANT_REPORT_OUTPUT_DIR.contains("reports"));
        Ok(())
    }
}
