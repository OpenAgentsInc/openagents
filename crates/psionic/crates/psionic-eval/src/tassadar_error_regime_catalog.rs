use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    TASSADAR_ERROR_REGIME_CATALOG_REPORT_REF, TASSADAR_ERROR_REGIME_SWEEP_REPORT_REF,
    TassadarErrorRegimeSweepReport,
};
use psionic_runtime::{
    TassadarErrorRegimeClass, TassadarErrorRegimeReceipt, TassadarErrorRegimeRecoverySurface,
    TassadarErrorRegimeWorkloadFamily,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Recovery-surface summary in the error-regime catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarErrorRegimeRecoverySurfaceSummary {
    /// Compared recovery surface.
    pub recovery_surface: TassadarErrorRegimeRecoverySurface,
    /// Number of workloads that self-heal under the surface.
    pub self_healing_workload_count: u32,
    /// Number of workloads that remain in slow drift under the surface.
    pub slow_drift_workload_count: u32,
    /// Number of workloads that diverge catastrophically under the surface.
    pub catastrophic_workload_count: u32,
    /// Mean exactness across workloads on the surface.
    pub mean_recovered_exactness_bps: u32,
    /// Plain-language note.
    pub note: String,
}

/// Workload-level catalog summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarErrorRegimeWorkloadCatalogSummary {
    /// Compared workload family.
    pub workload_family: TassadarErrorRegimeWorkloadFamily,
    /// Best recovery surface on the seeded sweep.
    pub best_recovery_surface: TassadarErrorRegimeRecoverySurface,
    /// Regime under the uncorrected surface.
    pub uncorrected_regime: TassadarErrorRegimeClass,
    /// Regime under the checkpoint-only surface.
    pub checkpoint_only_regime: TassadarErrorRegimeClass,
    /// Regime under the verifier-only surface.
    pub verifier_only_regime: TassadarErrorRegimeClass,
    /// Regime under the combined surface.
    pub checkpoint_and_verifier_regime: TassadarErrorRegimeClass,
    /// Plain-language note.
    pub note: String,
}

/// Eval-side catalog over the committed error-regime sweep.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarErrorRegimeCatalogReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Source run artifact ref.
    pub source_run_ref: String,
    /// Source run artifact digest.
    pub source_run_digest: String,
    /// Surface-level summaries.
    pub recovery_surface_summaries: Vec<TassadarErrorRegimeRecoverySurfaceSummary>,
    /// Workload-level summaries.
    pub workload_summaries: Vec<TassadarErrorRegimeWorkloadCatalogSummary>,
    /// Workloads that diverge catastrophically when uncorrected.
    pub catastrophic_without_correction_workloads: Vec<TassadarErrorRegimeWorkloadFamily>,
    /// Workloads whose best seeded result needs the combined recovery surface.
    pub workloads_needing_both: Vec<TassadarErrorRegimeWorkloadFamily>,
    /// Workloads with no self-healing surface in the current sweep.
    pub always_fragile_workloads: Vec<TassadarErrorRegimeWorkloadFamily>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Eval failure while building or writing the error-regime catalog.
#[derive(Debug, Error)]
pub enum TassadarErrorRegimeCatalogReportError {
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

/// Builds the committed error-regime catalog.
pub fn build_tassadar_error_regime_catalog_report()
-> Result<TassadarErrorRegimeCatalogReport, TassadarErrorRegimeCatalogReportError> {
    let sweep_report: TassadarErrorRegimeSweepReport =
        read_repo_json(TASSADAR_ERROR_REGIME_SWEEP_REPORT_REF)?;
    let recovery_surface_summaries =
        build_recovery_surface_summaries(sweep_report.runtime_receipts.as_slice());
    let workload_summaries = build_workload_summaries(sweep_report.runtime_receipts.as_slice());
    let catastrophic_without_correction_workloads = workload_summaries
        .iter()
        .filter(|summary| {
            summary.uncorrected_regime == TassadarErrorRegimeClass::CatastrophicDivergence
        })
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let workloads_needing_both = workload_summaries
        .iter()
        .filter(|summary| {
            summary.best_recovery_surface
                == TassadarErrorRegimeRecoverySurface::CheckpointAndVerifier
        })
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let always_fragile_workloads = workload_summaries
        .iter()
        .filter(|summary| {
            summary.checkpoint_and_verifier_regime != TassadarErrorRegimeClass::SelfHealing
        })
        .filter(|summary| summary.verifier_only_regime != TassadarErrorRegimeClass::SelfHealing)
        .filter(|summary| summary.checkpoint_only_regime != TassadarErrorRegimeClass::SelfHealing)
        .map(|summary| summary.workload_family)
        .collect::<Vec<_>>();
    let mut report = TassadarErrorRegimeCatalogReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.error_regime.catalog_report.v1"),
        source_run_ref: String::from(TASSADAR_ERROR_REGIME_SWEEP_REPORT_REF),
        source_run_digest: sweep_report.report_digest.clone(),
        recovery_surface_summaries,
        workload_summaries,
        catastrophic_without_correction_workloads,
        workloads_needing_both,
        always_fragile_workloads,
        claim_boundary: String::from(
            "this eval report catalogs bounded recovery regimes over the committed injected-error sweep. It keeps self-healing, slow drift, and catastrophic divergence explicit instead of blending correction paths into one headline accuracy number",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Error-regime catalog now classifies {} workloads across {} recovery surfaces, with {} catastrophic-without-correction workloads and {} always-fragile workloads.",
        report.workload_summaries.len(),
        report.recovery_surface_summaries.len(),
        report.catastrophic_without_correction_workloads.len(),
        report.always_fragile_workloads.len(),
    );
    report.report_digest = stable_digest(b"psionic_tassadar_error_regime_catalog_report|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the committed error-regime catalog.
#[must_use]
pub fn tassadar_error_regime_catalog_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ERROR_REGIME_CATALOG_REPORT_REF)
}

/// Writes the committed error-regime catalog.
pub fn write_tassadar_error_regime_catalog_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarErrorRegimeCatalogReport, TassadarErrorRegimeCatalogReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarErrorRegimeCatalogReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_error_regime_catalog_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarErrorRegimeCatalogReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_recovery_surface_summaries(
    receipts: &[TassadarErrorRegimeReceipt],
) -> Vec<TassadarErrorRegimeRecoverySurfaceSummary> {
    let mut grouped =
        BTreeMap::<TassadarErrorRegimeRecoverySurface, Vec<&TassadarErrorRegimeReceipt>>::new();
    for receipt in receipts {
        grouped
            .entry(receipt.recovery_surface)
            .or_default()
            .push(receipt);
    }
    grouped
        .into_iter()
        .map(|(recovery_surface, receipts)| TassadarErrorRegimeRecoverySurfaceSummary {
            recovery_surface,
            self_healing_workload_count: receipts
                .iter()
                .filter(|receipt| receipt.regime_class == TassadarErrorRegimeClass::SelfHealing)
                .count() as u32,
            slow_drift_workload_count: receipts
                .iter()
                .filter(|receipt| receipt.regime_class == TassadarErrorRegimeClass::SlowDrift)
                .count() as u32,
            catastrophic_workload_count: receipts
                .iter()
                .filter(|receipt| {
                    receipt.regime_class == TassadarErrorRegimeClass::CatastrophicDivergence
                })
                .count() as u32,
            mean_recovered_exactness_bps: rounded_mean(
                receipts
                    .iter()
                    .map(|receipt| u64::from(receipt.recovered_exactness_bps)),
            ),
            note: format!(
                "{} keeps self-healing, slow drift, and catastrophic counts explicit on the seeded workloads.",
                recovery_surface.as_str()
            ),
        })
        .collect()
}

fn build_workload_summaries(
    receipts: &[TassadarErrorRegimeReceipt],
) -> Vec<TassadarErrorRegimeWorkloadCatalogSummary> {
    let mut grouped =
        BTreeMap::<TassadarErrorRegimeWorkloadFamily, Vec<&TassadarErrorRegimeReceipt>>::new();
    for receipt in receipts {
        grouped
            .entry(receipt.workload_family)
            .or_default()
            .push(receipt);
    }
    grouped
        .into_iter()
        .map(|(workload_family, receipts)| {
            let uncorrected = receipt_for_surface(
                receipts.as_slice(),
                TassadarErrorRegimeRecoverySurface::Uncorrected,
            );
            let checkpoint_only = receipt_for_surface(
                receipts.as_slice(),
                TassadarErrorRegimeRecoverySurface::CheckpointOnly,
            );
            let verifier_only = receipt_for_surface(
                receipts.as_slice(),
                TassadarErrorRegimeRecoverySurface::VerifierOnly,
            );
            let checkpoint_and_verifier = receipt_for_surface(
                receipts.as_slice(),
                TassadarErrorRegimeRecoverySurface::CheckpointAndVerifier,
            );
            let best_recovery_surface = receipts
                .iter()
                .max_by_key(|receipt| receipt.recovered_exactness_bps)
                .map(|receipt| receipt.recovery_surface)
                .expect("workload group should not be empty");
            TassadarErrorRegimeWorkloadCatalogSummary {
                workload_family,
                best_recovery_surface,
                uncorrected_regime: uncorrected.regime_class,
                checkpoint_only_regime: checkpoint_only.regime_class,
                verifier_only_regime: verifier_only.regime_class,
                checkpoint_and_verifier_regime: checkpoint_and_verifier.regime_class,
                note: format!(
                    "{} is best served by {} on the current sweep.",
                    workload_family.as_str(),
                    best_recovery_surface.as_str()
                ),
            }
        })
        .collect()
}

fn receipt_for_surface<'a>(
    receipts: &[&'a TassadarErrorRegimeReceipt],
    recovery_surface: TassadarErrorRegimeRecoverySurface,
) -> &'a TassadarErrorRegimeReceipt {
    receipts
        .iter()
        .copied()
        .find(|receipt| receipt.recovery_surface == recovery_surface)
        .expect("every workload should surface every recovery mode")
}

fn rounded_mean(values: impl IntoIterator<Item = u64>) -> u32 {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        return 0;
    }
    let sum = values.iter().sum::<u64>();
    ((sum + (values.len() as u64 / 2)) / values.len() as u64) as u32
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarErrorRegimeCatalogReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarErrorRegimeCatalogReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarErrorRegimeCatalogReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarErrorRegimeCatalogReport, build_tassadar_error_regime_catalog_report,
        read_repo_json, tassadar_error_regime_catalog_report_path,
        write_tassadar_error_regime_catalog_report,
    };
    use psionic_data::TASSADAR_ERROR_REGIME_CATALOG_REPORT_REF;
    use psionic_runtime::TassadarErrorRegimeWorkloadFamily;

    #[test]
    fn error_regime_catalog_marks_fragile_and_combined_recovery_workloads() {
        let report = build_tassadar_error_regime_catalog_report().expect("error-regime catalog");

        assert!(
            report
                .catastrophic_without_correction_workloads
                .contains(&TassadarErrorRegimeWorkloadFamily::SearchKernel)
        );
        assert!(
            report
                .workloads_needing_both
                .contains(&TassadarErrorRegimeWorkloadFamily::LongHorizonControl)
        );
        assert!(
            report
                .always_fragile_workloads
                .contains(&TassadarErrorRegimeWorkloadFamily::LongHorizonControl)
        );
    }

    #[test]
    fn error_regime_catalog_matches_committed_truth() {
        let generated = build_tassadar_error_regime_catalog_report().expect("error-regime catalog");
        let committed: TassadarErrorRegimeCatalogReport =
            read_repo_json(TASSADAR_ERROR_REGIME_CATALOG_REPORT_REF)
                .expect("committed error-regime catalog");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_error_regime_catalog_persists_current_truth() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output_path = output_dir.path().join("tassadar_error_regime_catalog.json");
        let report =
            write_tassadar_error_regime_catalog_report(&output_path).expect("write catalog");
        let written = std::fs::read_to_string(&output_path).expect("written catalog");
        let reparsed: TassadarErrorRegimeCatalogReport =
            serde_json::from_str(&written).expect("written catalog should parse");

        assert_eq!(report, reparsed);
        assert_eq!(
            tassadar_error_regime_catalog_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_error_regime_catalog.json")
        );
    }
}
