use std::{collections::BTreeMap, fs, path::Path};

#[cfg(test)]
use psionic_data::{
    TASSADAR_ERROR_REGIME_SWEEP_OUTPUT_DIR, TASSADAR_ERROR_REGIME_SWEEP_REPORT_REF,
};
use psionic_data::{
    TASSADAR_ERROR_REGIME_SWEEP_REPORT_FILE, TassadarErrorRegimeSweepReport,
    TassadarErrorRegimeWorkloadSweepSummary, tassadar_error_regime_catalog_contract,
};
use psionic_runtime::{
    TassadarErrorRegimeClass, TassadarErrorRegimeReceipt, TassadarErrorRegimeRecoverySurface,
    TassadarErrorRegimeWorkloadFamily, tassadar_error_regime_receipts,
};
#[cfg(test)]
use serde::Deserialize;
use serde::Serialize;
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::path::PathBuf;
use thiserror::Error;

/// Errors while materializing the error-regime sweep artifact.
#[derive(Debug, Error)]
pub enum TassadarErrorRegimeSweepError {
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write error-regime sweep report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Executes the committed error-regime sweep and writes the run artifact.
pub fn execute_tassadar_error_regime_catalog(
    output_dir: &Path,
) -> Result<TassadarErrorRegimeSweepReport, TassadarErrorRegimeSweepError> {
    fs::create_dir_all(output_dir).map_err(|error| TassadarErrorRegimeSweepError::CreateDir {
        path: output_dir.display().to_string(),
        error,
    })?;

    let catalog_contract = tassadar_error_regime_catalog_contract();
    let runtime_receipts = tassadar_error_regime_receipts();
    let workload_summaries = build_workload_summaries(runtime_receipts.as_slice());
    let mut report = TassadarErrorRegimeSweepReport {
        version: String::from("2026.03.18"),
        catalog_contract,
        runtime_receipts,
        workload_summaries,
        claim_boundary: String::from(
            "this run artifact freezes one bounded injected-error sweep over declared workloads and correction surfaces. It keeps recovery bounded to the seeded workloads, checkpoint schedules, and verifier assumptions instead of treating repair machinery as broad exactness proof",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    let self_healing_receipt_count = report
        .runtime_receipts
        .iter()
        .filter(|receipt| receipt.regime_class == TassadarErrorRegimeClass::SelfHealing)
        .count();
    let catastrophic_receipt_count = report
        .runtime_receipts
        .iter()
        .filter(|receipt| receipt.regime_class == TassadarErrorRegimeClass::CatastrophicDivergence)
        .count();
    report.summary = format!(
        "Error-regime sweep now freezes {} workload/surface cells with {} self-healing and {} catastrophic outcomes while keeping checkpoint and verifier deltas explicit.",
        report.runtime_receipts.len(),
        self_healing_receipt_count,
        catastrophic_receipt_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_error_regime_sweep_report|", &report);

    let output_path = output_dir.join(TASSADAR_ERROR_REGIME_SWEEP_REPORT_FILE);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarErrorRegimeSweepError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_workload_summaries(
    receipts: &[TassadarErrorRegimeReceipt],
) -> Vec<TassadarErrorRegimeWorkloadSweepSummary> {
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
            TassadarErrorRegimeWorkloadSweepSummary {
                workload_family,
                best_recovery_surface,
                verifier_only_exactness_delta_bps: verifier_only.recovered_exactness_bps as i32
                    - uncorrected.recovered_exactness_bps as i32,
                checkpoint_only_exactness_delta_bps: checkpoint_only.recovered_exactness_bps as i32
                    - uncorrected.recovered_exactness_bps as i32,
                combined_exactness_delta_bps: checkpoint_and_verifier.recovered_exactness_bps
                    as i32
                    - uncorrected.recovered_exactness_bps as i32,
                self_healing_surface_count: receipts
                    .iter()
                    .filter(|receipt| receipt.regime_class == TassadarErrorRegimeClass::SelfHealing)
                    .count() as u32,
                catastrophic_surface_count: receipts
                    .iter()
                    .filter(|receipt| {
                        receipt.regime_class == TassadarErrorRegimeClass::CatastrophicDivergence
                    })
                    .count() as u32,
                note: workload_note(
                    workload_family,
                    verifier_only,
                    checkpoint_only,
                    checkpoint_and_verifier,
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

fn workload_note(
    workload_family: TassadarErrorRegimeWorkloadFamily,
    verifier_only: &TassadarErrorRegimeReceipt,
    checkpoint_only: &TassadarErrorRegimeReceipt,
    checkpoint_and_verifier: &TassadarErrorRegimeReceipt,
) -> String {
    let dominant_surface =
        if verifier_only.recovered_exactness_bps > checkpoint_only.recovered_exactness_bps {
            "verifier-first"
        } else {
            "checkpoint-first"
        };
    format!(
        "{} currently looks {} with combined exactness={}bps.",
        workload_family.as_str(),
        dominant_surface,
        checkpoint_and_verifier.recovered_exactness_bps
    )
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

#[cfg(test)]
fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, Box<dyn std::error::Error>> {
    let path = repo_root().join(relative_path);
    let bytes = std::fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TASSADAR_ERROR_REGIME_SWEEP_OUTPUT_DIR, TASSADAR_ERROR_REGIME_SWEEP_REPORT_FILE,
        TASSADAR_ERROR_REGIME_SWEEP_REPORT_REF, TassadarErrorRegimeSweepReport,
        execute_tassadar_error_regime_catalog, read_repo_json,
    };
    use psionic_runtime::TassadarErrorRegimeWorkloadFamily;

    #[test]
    fn error_regime_sweep_report_surfaces_checkpoint_and_verifier_deltas()
    -> Result<(), Box<dyn std::error::Error>> {
        let output_dir = tempdir()?;
        let report = execute_tassadar_error_regime_catalog(output_dir.path())?;
        let long_horizon = report
            .workload_summaries
            .iter()
            .find(|summary| {
                summary.workload_family == TassadarErrorRegimeWorkloadFamily::LongHorizonControl
            })
            .expect("long-horizon summary");
        assert!(
            long_horizon.combined_exactness_delta_bps
                > long_horizon.checkpoint_only_exactness_delta_bps
        );
        assert!(TASSADAR_ERROR_REGIME_SWEEP_OUTPUT_DIR.contains("error_regime_catalog_v1"));
        Ok(())
    }

    #[test]
    fn error_regime_sweep_report_matches_committed_truth() -> Result<(), Box<dyn std::error::Error>>
    {
        let output_dir = tempdir()?;
        let report = execute_tassadar_error_regime_catalog(output_dir.path())?;
        let committed: TassadarErrorRegimeSweepReport =
            read_repo_json(TASSADAR_ERROR_REGIME_SWEEP_REPORT_REF)?;
        assert_eq!(report, committed);
        Ok(())
    }

    #[test]
    fn error_regime_sweep_report_writes_current_truth() -> Result<(), Box<dyn std::error::Error>> {
        let output_dir = tempdir()?;
        let report = execute_tassadar_error_regime_catalog(output_dir.path())?;
        let persisted: TassadarErrorRegimeSweepReport = serde_json::from_slice(&std::fs::read(
            output_dir
                .path()
                .join(TASSADAR_ERROR_REGIME_SWEEP_REPORT_FILE),
        )?)?;
        assert_eq!(report, persisted);
        Ok(())
    }
}
