use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::{
    tassadar_quantization_truth_envelope_publication,
    TassadarQuantizationTruthEnvelopePublication,
    TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_EVAL_REPORT_REF,
};
use psionic_runtime::{
    build_tassadar_quantization_truth_envelope_runtime_report, TassadarQuantizationBackendFamily,
    TassadarQuantizationEnvelopePosture, TassadarQuantizationTruthEnvelopeRuntimeError,
    TassadarQuantizationTruthEnvelopeRuntimeReport,
    TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_RUNTIME_REPORT_REF,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Aggregate summary for one backend family inside the deployment truth matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarQuantizationTruthEnvelopeBackendSummary {
    /// Backend family being summarized.
    pub backend_family: TassadarQuantizationBackendFamily,
    /// Exact publication envelope count for the backend family.
    pub exact_envelope_count: u32,
    /// Constrained publication envelope count for the backend family.
    pub constrained_envelope_count: u32,
    /// Refused publication envelope count for the backend family.
    pub refused_envelope_count: u32,
    /// Union of workload families still exact under at least one backend envelope.
    pub exact_workload_families: Vec<String>,
    /// First refused workload family when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_refused_workload_family: Option<String>,
    /// Plain-language note.
    pub note: String,
}

/// Committed eval report over backend and quantization deployment envelopes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarQuantizationTruthEnvelopeEvalReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Publication anchoring the deployment envelope lane.
    pub publication: TassadarQuantizationTruthEnvelopePublication,
    /// Runtime report reused by this eval surface.
    pub runtime_report: TassadarQuantizationTruthEnvelopeRuntimeReport,
    /// Ordered backend summaries.
    pub backend_summaries: Vec<TassadarQuantizationTruthEnvelopeBackendSummary>,
    /// Ordered refs used to generate the report.
    pub generated_from_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarQuantizationTruthEnvelopeEvalReport {
    fn new(
        publication: TassadarQuantizationTruthEnvelopePublication,
        runtime_report: TassadarQuantizationTruthEnvelopeRuntimeReport,
        backend_summaries: Vec<TassadarQuantizationTruthEnvelopeBackendSummary>,
    ) -> Self {
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.quantization_truth_envelope.eval_report.v1"),
            publication,
            runtime_report,
            backend_summaries,
            generated_from_refs: vec![
                String::from(TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_RUNTIME_REPORT_REF),
                String::from(
                    "fixtures/tassadar/reports/tassadar_precision_attention_robustness_audit.json",
                ),
            ],
            claim_boundary: String::from(
                "this eval report keeps backend-specific quantization envelopes explicit by summarizing which deployment families stay exact, which remain constrained, and which must refuse. It does not widen one executor artifact into a backend-invariant capability claim",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        let fragile_backend_count = report
            .backend_summaries
            .iter()
            .filter(|summary| summary.refused_envelope_count > 0)
            .count();
        report.summary = format!(
            "Quantization truth envelope eval report now summarizes {} backend families, with {} families carrying at least one refused deployment envelope and the rest remaining exact or constrained only where the envelope stays explicit.",
            report.backend_summaries.len(),
            fragile_backend_count,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_quantization_truth_envelope_eval_report|",
            &report,
        );
        report
    }
}

/// Quantization-envelope eval build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarQuantizationTruthEnvelopeEvalError {
    /// Building the runtime report failed.
    #[error(transparent)]
    Runtime(#[from] TassadarQuantizationTruthEnvelopeRuntimeError),
    /// Failed to create an output directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Failed to read one committed artifact.
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Failed to decode one committed artifact.
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed deployment-envelope eval report.
pub fn build_tassadar_quantization_truth_envelope_eval_report(
) -> Result<TassadarQuantizationTruthEnvelopeEvalReport, TassadarQuantizationTruthEnvelopeEvalError>
{
    let publication = tassadar_quantization_truth_envelope_publication();
    let runtime_report = build_tassadar_quantization_truth_envelope_runtime_report();
    let backend_summaries = build_backend_summaries(&runtime_report);
    Ok(TassadarQuantizationTruthEnvelopeEvalReport::new(
        publication,
        runtime_report,
        backend_summaries,
    ))
}

/// Returns the canonical absolute path for the committed eval report.
#[must_use]
pub fn tassadar_quantization_truth_envelope_eval_report_path() -> PathBuf {
    repo_root().join(TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_EVAL_REPORT_REF)
}

/// Writes the committed deployment-envelope eval report.
pub fn write_tassadar_quantization_truth_envelope_eval_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarQuantizationTruthEnvelopeEvalReport, TassadarQuantizationTruthEnvelopeEvalError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarQuantizationTruthEnvelopeEvalError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_quantization_truth_envelope_eval_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarQuantizationTruthEnvelopeEvalError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_backend_summaries(
    runtime_report: &TassadarQuantizationTruthEnvelopeRuntimeReport,
) -> Vec<TassadarQuantizationTruthEnvelopeBackendSummary> {
    let mut grouped =
        BTreeMap::<TassadarQuantizationBackendFamily, Vec<_>>::new();
    for receipt in &runtime_report.envelope_receipts {
        grouped
            .entry(receipt.backend_family)
            .or_default()
            .push(receipt);
    }
    grouped
        .into_iter()
        .map(|(backend_family, receipts)| {
            let exact_envelope_count = receipts
                .iter()
                .filter(|receipt| {
                    receipt.publication_posture == TassadarQuantizationEnvelopePosture::PublishExact
                })
                .count() as u32;
            let constrained_envelope_count = receipts
                .iter()
                .filter(|receipt| {
                    receipt.publication_posture
                        == TassadarQuantizationEnvelopePosture::PublishConstrained
                })
                .count() as u32;
            let refused_envelope_count = receipts
                .iter()
                .filter(|receipt| {
                    receipt.publication_posture
                        == TassadarQuantizationEnvelopePosture::RefusePublication
                })
                .count() as u32;
            let exact_workload_families = receipts
                .iter()
                .flat_map(|receipt| receipt.exact_workload_families.iter().cloned())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let first_refused_workload_family = receipts
                .iter()
                .flat_map(|receipt| receipt.refused_workload_families.iter().cloned())
                .next();
            TassadarQuantizationTruthEnvelopeBackendSummary {
                backend_family,
                exact_envelope_count,
                constrained_envelope_count,
                refused_envelope_count,
                exact_workload_families,
                first_refused_workload_family,
                note: format!(
                    "backend family `{}` now has {} exact, {} constrained, and {} refused deployment envelopes",
                    backend_family.as_str(),
                    exact_envelope_count,
                    constrained_envelope_count,
                    refused_envelope_count,
                ),
            }
        })
        .collect()
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

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarQuantizationTruthEnvelopeEvalError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarQuantizationTruthEnvelopeEvalError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarQuantizationTruthEnvelopeEvalError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_quantization_truth_envelope_eval_report, read_repo_json,
        tassadar_quantization_truth_envelope_eval_report_path,
        write_tassadar_quantization_truth_envelope_eval_report,
        TassadarQuantizationTruthEnvelopeEvalReport,
    };
    use psionic_models::TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_EVAL_REPORT_REF;
    use psionic_runtime::TassadarQuantizationBackendFamily;

    #[test]
    fn quantization_truth_envelope_eval_report_tracks_backend_fragility() {
        let report =
            build_tassadar_quantization_truth_envelope_eval_report().expect("eval report");
        let cuda = report
            .backend_summaries
            .iter()
            .find(|summary| summary.backend_family == TassadarQuantizationBackendFamily::CudaServed)
            .expect("cuda summary");

        assert_eq!(cuda.refused_envelope_count, 1);
        assert_eq!(cuda.first_refused_workload_family.as_deref(), Some("sudoku_class"));
    }

    #[test]
    fn quantization_truth_envelope_eval_report_matches_committed_truth() {
        let generated =
            build_tassadar_quantization_truth_envelope_eval_report().expect("eval report");
        let committed: TassadarQuantizationTruthEnvelopeEvalReport =
            read_repo_json(TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_EVAL_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_quantization_truth_envelope_eval_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_quantization_truth_envelope_eval_report.json");
        let written =
            write_tassadar_quantization_truth_envelope_eval_report(&output_path).expect("write");
        let persisted: TassadarQuantizationTruthEnvelopeEvalReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_quantization_truth_envelope_eval_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_quantization_truth_envelope_eval_report.json")
        );
    }
}
