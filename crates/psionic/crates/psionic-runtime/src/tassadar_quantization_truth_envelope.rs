use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_precision_attention_runtime_audit_report,
    TassadarRuntimeAttentionSemanticsFamily, TassadarRuntimeNumericRegime,
    TassadarRuntimeRobustnessDriftClass, TASSADAR_PRECISION_ATTENTION_RUNTIME_AUDIT_REPORT_REF,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_quantization_truth_envelope_runtime_report.json";

/// Backend family used by one deployment truth envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarQuantizationBackendFamily {
    CpuReference,
    MetalServed,
    CudaServed,
}

impl TassadarQuantizationBackendFamily {
    /// Returns the stable backend-family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CpuReference => "cpu_reference",
            Self::MetalServed => "metal_served",
            Self::CudaServed => "cuda_served",
        }
    }
}

/// Numeric regime published by one deployment truth envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarQuantizationNumericRegime {
    Fp32Reference,
    Bf16Served,
    Fp8Served,
    Int8Served,
    Int4Served,
}

impl TassadarQuantizationNumericRegime {
    /// Returns the stable numeric-regime label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fp32Reference => "fp32_reference",
            Self::Bf16Served => "bf16_served",
            Self::Fp8Served => "fp8_served",
            Self::Int8Served => "int8_served",
            Self::Int4Served => "int4_served",
        }
    }
}

/// Quantization family published by one deployment truth envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarQuantizationSetting {
    NoneDense,
    Bf16Cast,
    Fp8Block,
    Int8Block,
    Int4Grouped,
}

impl TassadarQuantizationSetting {
    /// Returns the stable quantization-setting label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoneDense => "none_dense",
            Self::Bf16Cast => "bf16_cast",
            Self::Fp8Block => "fp8_block",
            Self::Int8Block => "int8_block",
            Self::Int4Grouped => "int4_grouped",
        }
    }
}

/// Publication posture for one deployment truth envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarQuantizationEnvelopePosture {
    PublishExact,
    PublishConstrained,
    RefusePublication,
}

/// One deployment truth envelope derived from the finite-precision audit lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarQuantizationTruthEnvelopeReceipt {
    /// Stable envelope identifier.
    pub envelope_id: String,
    /// Backend family for the envelope.
    pub backend_family: TassadarQuantizationBackendFamily,
    /// Numeric regime for the envelope.
    pub numeric_regime: TassadarQuantizationNumericRegime,
    /// Quantization family for the envelope.
    pub quantization_setting: TassadarQuantizationSetting,
    /// Runtime audit regime the envelope was projected from.
    pub audit_anchor_numeric_regime: TassadarRuntimeNumericRegime,
    /// Runtime audit attention family the envelope was projected from.
    pub audit_anchor_attention_family: TassadarRuntimeAttentionSemanticsFamily,
    /// Workload families that stay exact inside the envelope.
    pub exact_workload_families: Vec<String>,
    /// Workload families that stay bounded but not exact inside the envelope.
    pub approximate_workload_families: Vec<String>,
    /// Workload families that must refuse inside the envelope.
    pub refused_workload_families: Vec<String>,
    /// Stable refusal reasons observed in the anchor audit.
    pub refusal_reasons: Vec<String>,
    /// Whether the envelope is exact, constrained, or refused for served publication.
    pub publication_posture: TassadarQuantizationEnvelopePosture,
    /// Stable benchmark refs backing the envelope.
    pub benchmark_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language note.
    pub detail: String,
}

/// Deterministic cross-backend deployment truth report for executor quantization envelopes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarQuantizationTruthEnvelopeRuntimeReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Audit report this surface is projected from.
    pub audit_anchor_report_ref: String,
    /// Ordered envelope receipts.
    pub envelope_receipts: Vec<TassadarQuantizationTruthEnvelopeReceipt>,
    /// Number of exact publication envelopes.
    pub exact_envelope_count: u32,
    /// Number of constrained publication envelopes.
    pub constrained_envelope_count: u32,
    /// Number of refused publication envelopes.
    pub refused_envelope_count: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarQuantizationTruthEnvelopeRuntimeReport {
    fn new(envelope_receipts: Vec<TassadarQuantizationTruthEnvelopeReceipt>) -> Self {
        let exact_envelope_count = envelope_receipts
            .iter()
            .filter(|receipt| {
                receipt.publication_posture == TassadarQuantizationEnvelopePosture::PublishExact
            })
            .count() as u32;
        let constrained_envelope_count = envelope_receipts
            .iter()
            .filter(|receipt| {
                receipt.publication_posture
                    == TassadarQuantizationEnvelopePosture::PublishConstrained
            })
            .count() as u32;
        let refused_envelope_count = envelope_receipts
            .iter()
            .filter(|receipt| {
                receipt.publication_posture
                    == TassadarQuantizationEnvelopePosture::RefusePublication
            })
            .count() as u32;
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.quantization_truth_envelope.runtime_report.v1"),
            audit_anchor_report_ref: String::from(
                TASSADAR_PRECISION_ATTENTION_RUNTIME_AUDIT_REPORT_REF,
            ),
            envelope_receipts,
            exact_envelope_count,
            constrained_envelope_count,
            refused_envelope_count,
            claim_boundary: String::from(
                "this runtime report projects backend-specific deployment envelopes from the finite-precision audit lane so backend family, numeric regime, quantization family, constrained workloads, and refusal posture stay explicit. It does not assume one executor artifact preserves semantics across backend or quantization changes",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Quantization truth envelope runtime report now freezes {} deployment envelopes: {} exact, {} constrained, {} refused.",
            report.envelope_receipts.len(),
            report.exact_envelope_count,
            report.constrained_envelope_count,
            report.refused_envelope_count,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_quantization_truth_envelope_runtime_report|",
            &report,
        );
        report
    }
}

/// Runtime report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarQuantizationTruthEnvelopeRuntimeError {
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

/// Builds the committed runtime deployment-envelope report.
#[must_use]
pub fn build_tassadar_quantization_truth_envelope_runtime_report(
) -> TassadarQuantizationTruthEnvelopeRuntimeReport {
    let runtime_audit = build_tassadar_precision_attention_runtime_audit_report();
    let anchor_attention_family = TassadarRuntimeAttentionSemanticsFamily::HardSelectionReference;
    let envelope_receipts = envelope_seeds()
        .into_iter()
        .map(|seed| {
            let relevant_receipts = runtime_audit
                .receipts
                .iter()
                .filter(|receipt| {
                    receipt.numeric_regime == seed.audit_anchor_numeric_regime
                        && receipt.attention_family == anchor_attention_family
                })
                .collect::<Vec<_>>();
            let exact_workload_families = relevant_receipts
                .iter()
                .filter(|receipt| {
                    receipt.drift_class == TassadarRuntimeRobustnessDriftClass::Exact
                })
                .map(|receipt| receipt.workload_family_id.clone())
                .collect::<Vec<_>>();
            let approximate_workload_families = relevant_receipts
                .iter()
                .filter(|receipt| {
                    receipt.drift_class
                        == TassadarRuntimeRobustnessDriftClass::ApproximateBounded
                })
                .map(|receipt| receipt.workload_family_id.clone())
                .collect::<Vec<_>>();
            let refused_workload_families = relevant_receipts
                .iter()
                .filter(|receipt| {
                    receipt.drift_class == TassadarRuntimeRobustnessDriftClass::Refused
                })
                .map(|receipt| receipt.workload_family_id.clone())
                .collect::<Vec<_>>();
            let refusal_reasons = relevant_receipts
                .iter()
                .filter_map(|receipt| receipt.refusal_reason.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let publication_posture = if !refused_workload_families.is_empty() {
                TassadarQuantizationEnvelopePosture::RefusePublication
            } else if !approximate_workload_families.is_empty() {
                TassadarQuantizationEnvelopePosture::PublishConstrained
            } else {
                TassadarQuantizationEnvelopePosture::PublishExact
            };
            TassadarQuantizationTruthEnvelopeReceipt {
                envelope_id: String::from(seed.envelope_id),
                backend_family: seed.backend_family,
                numeric_regime: seed.numeric_regime,
                quantization_setting: seed.quantization_setting,
                audit_anchor_numeric_regime: seed.audit_anchor_numeric_regime,
                audit_anchor_attention_family: anchor_attention_family,
                exact_workload_families,
                approximate_workload_families,
                refused_workload_families,
                refusal_reasons,
                publication_posture,
                benchmark_refs: vec![String::from(
                    TASSADAR_PRECISION_ATTENTION_RUNTIME_AUDIT_REPORT_REF,
                )],
                claim_boundary: String::from(
                    "the envelope stays keyed to one backend family, one numeric regime, and one quantization family, and it preserves constrained workloads and refusal posture explicitly instead of silently carrying one executor claim across deployment changes",
                ),
                detail: detail_for_seed(seed, publication_posture),
            }
        })
        .collect::<Vec<_>>();
    TassadarQuantizationTruthEnvelopeRuntimeReport::new(envelope_receipts)
}

/// Returns the canonical absolute path for the committed runtime report.
#[must_use]
pub fn tassadar_quantization_truth_envelope_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_RUNTIME_REPORT_REF)
}

/// Writes the committed runtime deployment-envelope report.
pub fn write_tassadar_quantization_truth_envelope_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarQuantizationTruthEnvelopeRuntimeReport,
    TassadarQuantizationTruthEnvelopeRuntimeError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarQuantizationTruthEnvelopeRuntimeError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_quantization_truth_envelope_runtime_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarQuantizationTruthEnvelopeRuntimeError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[derive(Clone, Copy)]
struct EnvelopeSeed {
    envelope_id: &'static str,
    backend_family: TassadarQuantizationBackendFamily,
    numeric_regime: TassadarQuantizationNumericRegime,
    quantization_setting: TassadarQuantizationSetting,
    audit_anchor_numeric_regime: TassadarRuntimeNumericRegime,
}

fn envelope_seeds() -> [EnvelopeSeed; 5] {
    [
        EnvelopeSeed {
            envelope_id: "cpu_reference_fp32_dense",
            backend_family: TassadarQuantizationBackendFamily::CpuReference,
            numeric_regime: TassadarQuantizationNumericRegime::Fp32Reference,
            quantization_setting: TassadarQuantizationSetting::NoneDense,
            audit_anchor_numeric_regime: TassadarRuntimeNumericRegime::Fp32Reference,
        },
        EnvelopeSeed {
            envelope_id: "metal_served_bf16_cast",
            backend_family: TassadarQuantizationBackendFamily::MetalServed,
            numeric_regime: TassadarQuantizationNumericRegime::Bf16Served,
            quantization_setting: TassadarQuantizationSetting::Bf16Cast,
            audit_anchor_numeric_regime: TassadarRuntimeNumericRegime::Fp16Served,
        },
        EnvelopeSeed {
            envelope_id: "metal_served_fp8_block",
            backend_family: TassadarQuantizationBackendFamily::MetalServed,
            numeric_regime: TassadarQuantizationNumericRegime::Fp8Served,
            quantization_setting: TassadarQuantizationSetting::Fp8Block,
            audit_anchor_numeric_regime: TassadarRuntimeNumericRegime::Int8Served,
        },
        EnvelopeSeed {
            envelope_id: "cuda_served_int8_block",
            backend_family: TassadarQuantizationBackendFamily::CudaServed,
            numeric_regime: TassadarQuantizationNumericRegime::Int8Served,
            quantization_setting: TassadarQuantizationSetting::Int8Block,
            audit_anchor_numeric_regime: TassadarRuntimeNumericRegime::Int8Served,
        },
        EnvelopeSeed {
            envelope_id: "cuda_served_int4_grouped",
            backend_family: TassadarQuantizationBackendFamily::CudaServed,
            numeric_regime: TassadarQuantizationNumericRegime::Int4Served,
            quantization_setting: TassadarQuantizationSetting::Int4Grouped,
            audit_anchor_numeric_regime: TassadarRuntimeNumericRegime::Int8ServedWithNoise,
        },
    ]
}

fn detail_for_seed(
    seed: EnvelopeSeed,
    publication_posture: TassadarQuantizationEnvelopePosture,
) -> String {
    let posture = match publication_posture {
        TassadarQuantizationEnvelopePosture::PublishExact => {
            "stays exact across the current workload families"
        }
        TassadarQuantizationEnvelopePosture::PublishConstrained => {
            "stays publishable only with constrained workload boundaries kept explicit"
        }
        TassadarQuantizationEnvelopePosture::RefusePublication => {
            "must refuse publication for at least one workload family"
        }
    };
    format!(
        "deployment envelope `{}` maps `{}` on `{}` to the finite-precision audit anchor `{}` with hard-selection attention and {}",
        seed.envelope_id,
        seed.quantization_setting.as_str(),
        seed.backend_family.as_str(),
        seed.audit_anchor_numeric_regime.as_str(),
        posture,
    )
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
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
) -> Result<T, TassadarQuantizationTruthEnvelopeRuntimeError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarQuantizationTruthEnvelopeRuntimeError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarQuantizationTruthEnvelopeRuntimeError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_quantization_truth_envelope_runtime_report, read_repo_json,
        tassadar_quantization_truth_envelope_runtime_report_path,
        write_tassadar_quantization_truth_envelope_runtime_report,
        TassadarQuantizationEnvelopePosture, TassadarQuantizationTruthEnvelopeRuntimeReport,
        TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_RUNTIME_REPORT_REF,
    };

    #[test]
    fn quantization_truth_envelope_runtime_report_keeps_backend_posture_explicit() {
        let report = build_tassadar_quantization_truth_envelope_runtime_report();
        let cpu = report
            .envelope_receipts
            .iter()
            .find(|receipt| receipt.envelope_id == "cpu_reference_fp32_dense")
            .expect("cpu envelope");
        let int4 = report
            .envelope_receipts
            .iter()
            .find(|receipt| receipt.envelope_id == "cuda_served_int4_grouped")
            .expect("int4 envelope");

        assert_eq!(
            cpu.publication_posture,
            TassadarQuantizationEnvelopePosture::PublishExact
        );
        assert_eq!(
            int4.publication_posture,
            TassadarQuantizationEnvelopePosture::RefusePublication
        );
        assert!(int4
            .refused_workload_families
            .contains(&String::from("sudoku_class")));
    }

    #[test]
    fn quantization_truth_envelope_runtime_report_matches_committed_truth() {
        let generated = build_tassadar_quantization_truth_envelope_runtime_report();
        let committed: TassadarQuantizationTruthEnvelopeRuntimeReport =
            read_repo_json(TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_RUNTIME_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_quantization_truth_envelope_runtime_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_quantization_truth_envelope_runtime_report.json");
        let written =
            write_tassadar_quantization_truth_envelope_runtime_report(&output_path).expect("write");
        let persisted: TassadarQuantizationTruthEnvelopeRuntimeReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_quantization_truth_envelope_runtime_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_quantization_truth_envelope_runtime_report.json")
        );
    }
}
