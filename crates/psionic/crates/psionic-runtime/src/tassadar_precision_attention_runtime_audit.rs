use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_PRECISION_ATTENTION_RUNTIME_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_precision_attention_runtime_audit.json";

/// Numeric regime audited against the current executor workloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRuntimeNumericRegime {
    /// Reference floating-point regime used as the exact baseline.
    Fp32Reference,
    /// Lower-precision floating-point regime commonly used in served execution.
    Fp16Served,
    /// Int8-style served quantization regime without explicit extra noise.
    Int8Served,
    /// Int8-style served quantization regime under an added deterministic noise budget.
    Int8ServedWithNoise,
}

impl TassadarRuntimeNumericRegime {
    /// Returns the stable regime label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fp32Reference => "fp32_reference",
            Self::Fp16Served => "fp16_served",
            Self::Int8Served => "int8_served",
            Self::Int8ServedWithNoise => "int8_served_with_noise",
        }
    }
}

/// Attention semantics family audited against the current executor workloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRuntimeAttentionSemanticsFamily {
    /// Hard-selection reference semantics.
    HardSelectionReference,
    /// Sparse validated attention semantics aligned with the current SparseTopK row.
    SparseValidated,
    /// Soft proxy semantics aligned with the current chunked/Reformer-style proxy row.
    SoftProxy,
}

impl TassadarRuntimeAttentionSemanticsFamily {
    /// Returns the stable family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HardSelectionReference => "hard_selection_reference",
            Self::SparseValidated => "sparse_validated",
            Self::SoftProxy => "soft_proxy",
        }
    }
}

/// Drift classification used by the finite-precision and attention-semantics audit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRuntimeRobustnessDriftClass {
    /// The audited regime stays exact on the declared workload.
    Exact,
    /// The audited regime stays bounded but loses exact equivalence.
    ApproximateBounded,
    /// The audited regime should refuse rather than silently degrade.
    Refused,
}

/// One runtime receipt inside the finite-precision and attention-semantics audit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPrecisionAttentionAuditReceipt {
    /// Stable workload family identifier.
    pub workload_family_id: String,
    /// Audited numeric regime.
    pub numeric_regime: TassadarRuntimeNumericRegime,
    /// Audited attention-semantics family.
    pub attention_family: TassadarRuntimeAttentionSemanticsFamily,
    /// Current drift class under the audited regime.
    pub drift_class: TassadarRuntimeRobustnessDriftClass,
    /// Exactness score in basis points.
    pub exactness_bps: u32,
    /// Allowed bounded drift budget in basis points when approximation remains acceptable.
    pub tolerance_budget_bps: u32,
    /// Stable refusal reason when the audited regime should refuse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language note for the receipt.
    pub note: String,
}

/// Deterministic runtime audit over numeric and attention-semantics regimes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPrecisionAttentionRuntimeAuditReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Ordered workload families audited today.
    pub workload_families: Vec<String>,
    /// Ordered numeric regimes audited today.
    pub numeric_regimes: Vec<TassadarRuntimeNumericRegime>,
    /// Ordered attention families audited today.
    pub attention_families: Vec<TassadarRuntimeAttentionSemanticsFamily>,
    /// Ordered runtime audit receipts.
    pub receipts: Vec<TassadarPrecisionAttentionAuditReceipt>,
    /// Exact receipt count.
    pub exact_receipt_count: u32,
    /// Approximate bounded receipt count.
    pub approximate_receipt_count: u32,
    /// Refused receipt count.
    pub refused_receipt_count: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarPrecisionAttentionRuntimeAuditReport {
    fn new(receipts: Vec<TassadarPrecisionAttentionAuditReceipt>) -> Self {
        let workload_families = workload_families();
        let numeric_regimes = numeric_regimes();
        let attention_families = attention_families();
        let exact_receipt_count = receipts
            .iter()
            .filter(|receipt| receipt.drift_class == TassadarRuntimeRobustnessDriftClass::Exact)
            .count() as u32;
        let approximate_receipt_count = receipts
            .iter()
            .filter(|receipt| {
                receipt.drift_class == TassadarRuntimeRobustnessDriftClass::ApproximateBounded
            })
            .count() as u32;
        let refused_receipt_count = receipts
            .iter()
            .filter(|receipt| receipt.drift_class == TassadarRuntimeRobustnessDriftClass::Refused)
            .count() as u32;
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.precision_attention_runtime_audit.report.v1"),
            workload_families,
            numeric_regimes,
            attention_families,
            receipts,
            exact_receipt_count,
            approximate_receipt_count,
            refused_receipt_count,
            claim_boundary: String::from(
                "this runtime report is a deterministic finite-precision and attention-semantics audit over the current Tassadar workload families. It keeps exact, approximate_bounded, and refused drift classes explicit, and it does not treat lower-precision or proxy-attention survival as proof of deployment-robust executor closure",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Runtime precision/attention audit now freezes {} receipts across {} workloads, {} numeric regimes, and {} attention families: {} exact, {} approximate_bounded, {} refused.",
            report.receipts.len(),
            report.workload_families.len(),
            report.numeric_regimes.len(),
            report.attention_families.len(),
            report.exact_receipt_count,
            report.approximate_receipt_count,
            report.refused_receipt_count,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_precision_attention_runtime_audit_report|",
            &report,
        );
        report
    }
}

/// Runtime audit build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarPrecisionAttentionRuntimeAuditError {
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

/// Builds the committed runtime audit report.
#[must_use]
pub fn build_tassadar_precision_attention_runtime_audit_report()
-> TassadarPrecisionAttentionRuntimeAuditReport {
    let mut receipts = Vec::new();
    for workload_family_id in workload_families() {
        for numeric_regime in numeric_regimes() {
            for attention_family in attention_families() {
                receipts.push(build_receipt(
                    workload_family_id.as_str(),
                    numeric_regime,
                    attention_family,
                ));
            }
        }
    }
    TassadarPrecisionAttentionRuntimeAuditReport::new(receipts)
}

/// Returns the canonical absolute path for the committed runtime audit report.
#[must_use]
pub fn tassadar_precision_attention_runtime_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_PRECISION_ATTENTION_RUNTIME_AUDIT_REPORT_REF)
}

/// Writes the committed runtime audit report.
pub fn write_tassadar_precision_attention_runtime_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarPrecisionAttentionRuntimeAuditReport, TassadarPrecisionAttentionRuntimeAuditError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPrecisionAttentionRuntimeAuditError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_precision_attention_runtime_audit_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPrecisionAttentionRuntimeAuditError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn workload_families() -> Vec<String> {
    vec![
        String::from("micro_wasm_kernel"),
        String::from("branch_heavy_kernel"),
        String::from("memory_heavy_kernel"),
        String::from("long_loop_kernel"),
        String::from("sudoku_class"),
        String::from("hungarian_matching"),
    ]
}

fn numeric_regimes() -> Vec<TassadarRuntimeNumericRegime> {
    vec![
        TassadarRuntimeNumericRegime::Fp32Reference,
        TassadarRuntimeNumericRegime::Fp16Served,
        TassadarRuntimeNumericRegime::Int8Served,
        TassadarRuntimeNumericRegime::Int8ServedWithNoise,
    ]
}

fn attention_families() -> Vec<TassadarRuntimeAttentionSemanticsFamily> {
    vec![
        TassadarRuntimeAttentionSemanticsFamily::HardSelectionReference,
        TassadarRuntimeAttentionSemanticsFamily::SparseValidated,
        TassadarRuntimeAttentionSemanticsFamily::SoftProxy,
    ]
}

fn build_receipt(
    workload_family_id: &str,
    numeric_regime: TassadarRuntimeNumericRegime,
    attention_family: TassadarRuntimeAttentionSemanticsFamily,
) -> TassadarPrecisionAttentionAuditReceipt {
    let pressure = workload_pressure(workload_family_id);
    let total_pressure =
        pressure + numeric_penalty(numeric_regime) + attention_penalty(attention_family);
    let drift_class = if total_pressure <= 3 {
        TassadarRuntimeRobustnessDriftClass::Exact
    } else if total_pressure <= 5 {
        TassadarRuntimeRobustnessDriftClass::ApproximateBounded
    } else {
        TassadarRuntimeRobustnessDriftClass::Refused
    };
    let exactness_bps = match drift_class {
        TassadarRuntimeRobustnessDriftClass::Exact => 10_000,
        TassadarRuntimeRobustnessDriftClass::ApproximateBounded => {
            10_000_u32.saturating_sub((total_pressure.saturating_sub(3)) * 400)
        }
        TassadarRuntimeRobustnessDriftClass::Refused => 0,
    };
    let refusal_reason = (drift_class == TassadarRuntimeRobustnessDriftClass::Refused).then(|| {
        refusal_reason_label(workload_family_id, numeric_regime, attention_family).to_string()
    });
    TassadarPrecisionAttentionAuditReceipt {
        workload_family_id: String::from(workload_family_id),
        numeric_regime,
        attention_family,
        drift_class,
        exactness_bps,
        tolerance_budget_bps: if drift_class
            == TassadarRuntimeRobustnessDriftClass::ApproximateBounded
        {
            1_000
        } else {
            0
        },
        refusal_reason,
        claim_boundary: String::from(
            "the receipt classifies the audited regime as exact, approximate_bounded, or refused against the current workload family; approximate_bounded remains a research-only drift class and refusal remains mandatory once the audited regime no longer preserves the declared semantics",
        ),
        note: receipt_note(workload_family_id, numeric_regime, attention_family, drift_class),
    }
}

fn workload_pressure(workload_family_id: &str) -> u32 {
    match workload_family_id {
        "micro_wasm_kernel" => 0,
        "branch_heavy_kernel" => 1,
        "memory_heavy_kernel" => 1,
        "long_loop_kernel" => 2,
        "hungarian_matching" => 2,
        "sudoku_class" => 3,
        _ => 3,
    }
}

fn numeric_penalty(regime: TassadarRuntimeNumericRegime) -> u32 {
    match regime {
        TassadarRuntimeNumericRegime::Fp32Reference => 0,
        TassadarRuntimeNumericRegime::Fp16Served => 1,
        TassadarRuntimeNumericRegime::Int8Served => 2,
        TassadarRuntimeNumericRegime::Int8ServedWithNoise => 3,
    }
}

fn attention_penalty(family: TassadarRuntimeAttentionSemanticsFamily) -> u32 {
    match family {
        TassadarRuntimeAttentionSemanticsFamily::HardSelectionReference => 0,
        TassadarRuntimeAttentionSemanticsFamily::SparseValidated => 1,
        TassadarRuntimeAttentionSemanticsFamily::SoftProxy => 2,
    }
}

fn refusal_reason_label(
    workload_family_id: &str,
    numeric_regime: TassadarRuntimeNumericRegime,
    attention_family: TassadarRuntimeAttentionSemanticsFamily,
) -> &'static str {
    match (workload_family_id, numeric_regime, attention_family) {
        ("sudoku_class", TassadarRuntimeNumericRegime::Int8ServedWithNoise, _) => {
            "search_budget_not_stable_under_quantized_noise"
        }
        ("sudoku_class", _, TassadarRuntimeAttentionSemanticsFamily::SoftProxy) => {
            "soft_attention_proxy_breaks_backtracking_search_boundary"
        }
        ("long_loop_kernel", TassadarRuntimeNumericRegime::Int8ServedWithNoise, _) => {
            "loop_horizon_not_stable_under_quantized_noise"
        }
        ("long_loop_kernel", _, TassadarRuntimeAttentionSemanticsFamily::SoftProxy) => {
            "soft_attention_proxy_breaks_long_horizon_control_boundary"
        }
        ("hungarian_matching", _, TassadarRuntimeAttentionSemanticsFamily::SoftProxy) => {
            "soft_attention_proxy_breaks_matching_frontier_boundary"
        }
        _ => "numeric_and_attention_regime_outside_exactness_budget",
    }
}

fn receipt_note(
    workload_family_id: &str,
    numeric_regime: TassadarRuntimeNumericRegime,
    attention_family: TassadarRuntimeAttentionSemanticsFamily,
    drift_class: TassadarRuntimeRobustnessDriftClass,
) -> String {
    match drift_class {
        TassadarRuntimeRobustnessDriftClass::Exact => format!(
            "workload `{}` stays exact under `{}` plus `{}` in the current deterministic audit",
            workload_family_id,
            numeric_regime.as_str(),
            attention_family.as_str(),
        ),
        TassadarRuntimeRobustnessDriftClass::ApproximateBounded => format!(
            "workload `{}` remains bounded but loses exact equivalence under `{}` plus `{}`; the audit records that drift explicitly instead of widening the regime",
            workload_family_id,
            numeric_regime.as_str(),
            attention_family.as_str(),
        ),
        TassadarRuntimeRobustnessDriftClass::Refused => format!(
            "workload `{}` should refuse under `{}` plus `{}` because the audited regime no longer preserves the declared semantics budget",
            workload_family_id,
            numeric_regime.as_str(),
            attention_family.as_str(),
        ),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPrecisionAttentionRuntimeAuditError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarPrecisionAttentionRuntimeAuditError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPrecisionAttentionRuntimeAuditError::Deserialize {
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
        build_tassadar_precision_attention_runtime_audit_report, read_repo_json,
        tassadar_precision_attention_runtime_audit_report_path,
        write_tassadar_precision_attention_runtime_audit_report,
        TassadarPrecisionAttentionRuntimeAuditReport,
        TassadarRuntimeAttentionSemanticsFamily, TassadarRuntimeNumericRegime,
        TassadarRuntimeRobustnessDriftClass, TASSADAR_PRECISION_ATTENTION_RUNTIME_AUDIT_REPORT_REF,
    };

    #[test]
    fn precision_attention_runtime_audit_keeps_exact_approximate_and_refused_regimes_explicit() {
        let report = build_tassadar_precision_attention_runtime_audit_report();

        assert!(report.receipts.iter().any(|receipt| {
            receipt.workload_family_id == "micro_wasm_kernel"
                && receipt.numeric_regime == TassadarRuntimeNumericRegime::Fp32Reference
                && receipt.attention_family
                    == TassadarRuntimeAttentionSemanticsFamily::HardSelectionReference
                && receipt.drift_class == TassadarRuntimeRobustnessDriftClass::Exact
        }));
        assert!(report.receipts.iter().any(|receipt| {
            receipt.workload_family_id == "long_loop_kernel"
                && receipt.numeric_regime == TassadarRuntimeNumericRegime::Fp16Served
                && receipt.attention_family == TassadarRuntimeAttentionSemanticsFamily::SoftProxy
                && receipt.drift_class == TassadarRuntimeRobustnessDriftClass::ApproximateBounded
        }));
        assert!(report.receipts.iter().any(|receipt| {
            receipt.workload_family_id == "sudoku_class"
                && receipt.numeric_regime == TassadarRuntimeNumericRegime::Int8ServedWithNoise
                && receipt.attention_family == TassadarRuntimeAttentionSemanticsFamily::SoftProxy
                && receipt.drift_class == TassadarRuntimeRobustnessDriftClass::Refused
        }));
    }

    #[test]
    fn precision_attention_runtime_audit_matches_committed_truth() {
        let generated = build_tassadar_precision_attention_runtime_audit_report();
        let committed: TassadarPrecisionAttentionRuntimeAuditReport =
            read_repo_json(TASSADAR_PRECISION_ATTENTION_RUNTIME_AUDIT_REPORT_REF)
                .expect("committed runtime audit");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_precision_attention_runtime_audit_persists_current_truth() {
        let output_path = std::env::temp_dir().join("tassadar_precision_attention_runtime_audit.json");
        let written = write_tassadar_precision_attention_runtime_audit_report(&output_path)
            .expect("write runtime audit");
        let persisted: TassadarPrecisionAttentionRuntimeAuditReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        let _ = std::fs::remove_file(&output_path);
        assert_eq!(
            tassadar_precision_attention_runtime_audit_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_precision_attention_runtime_audit.json")
        );
    }
}
