use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    tassadar_structured_numeric_encoding_lane_contract, TassadarStructuredNumericEncodingLaneContract,
    TassadarStructuredNumericEncodingLaneError,
    TassadarStructuredNumericEncodingWorkloadFamily,
};
use psionic_ir::{
    decode_tassadar_numeric_value, encode_tassadar_numeric_value,
    tassadar_structured_numeric_encodings, TassadarStructuredNumericEncoding,
    TassadarStructuredNumericEncodingError,
};
use psionic_models::{tassadar_numeric_encoding_publication, TassadarNumericEncodingPublication};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_NUMERIC_ENCODING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_numeric_encoding_report.json";
pub const TASSADAR_NUMERIC_ENCODING_SUITE_RUN_REF: &str =
    "fixtures/tassadar/runs/tassadar_numeric_encoding_suite_v1/numeric_encoding_suite.json";

const REPORT_SCHEMA_VERSION: u16 = 1;

/// One legacy-vs-candidate numeric encoding comparison row in the committed eval report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericEncodingCandidateReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Workload family covered by the case.
    pub workload_family: TassadarStructuredNumericEncodingWorkloadFamily,
    /// Stable legacy encoding identifier.
    pub legacy_encoding_id: String,
    /// Stable candidate encoding identifier.
    pub candidate_encoding_id: String,
    /// Held-out token-vocabulary coverage for the legacy encoding.
    pub legacy_held_out_vocab_coverage_bps: u32,
    /// Held-out token-vocabulary coverage for the candidate encoding.
    pub candidate_held_out_vocab_coverage_bps: u32,
    /// Mean token count per value for the legacy encoding.
    pub legacy_mean_tokens_per_value: u32,
    /// Mean token count per value for the candidate encoding.
    pub candidate_mean_tokens_per_value: u32,
    /// Roundtrip exactness across train and held-out values.
    pub semantic_roundtrip_exact_bps: u32,
    /// Candidate minus legacy held-out vocabulary coverage.
    pub representation_generalization_gain_bps: i32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
}

/// Workload-family aggregate summary in the committed eval report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericEncodingFamilySummary {
    /// Workload family covered by the summary.
    pub workload_family: TassadarStructuredNumericEncodingWorkloadFamily,
    /// Number of candidate comparisons in the family.
    pub candidate_count: u32,
    /// Mean representation gain in basis points.
    pub mean_representation_generalization_gain_bps: i32,
    /// Whether all comparisons kept roundtrip exactness.
    pub all_roundtrip_exact: bool,
}

/// Committed eval report for the structured numeric encoding lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericEncodingReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Canonical train-side suite run ref.
    pub suite_run_ref: String,
    /// Public lane contract.
    pub lane_contract: TassadarStructuredNumericEncodingLaneContract,
    /// Public model-facing publication.
    pub publication: TassadarNumericEncodingPublication,
    /// Ordered legacy-vs-candidate comparison rows.
    pub candidate_reports: Vec<TassadarNumericEncodingCandidateReport>,
    /// Workload-family aggregate summaries.
    pub family_summaries: Vec<TassadarNumericEncodingFamilySummary>,
    /// Explicit claim boundary for the lane.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Errors while building the structured numeric encoding eval report.
#[derive(Debug, Error)]
pub enum TassadarNumericEncodingReportError {
    /// Lane contract validation failed.
    #[error(transparent)]
    Contract(#[from] TassadarStructuredNumericEncodingLaneError),
    /// Structured encoding failed to encode or decode one value.
    #[error(transparent)]
    Encoding(#[from] TassadarStructuredNumericEncodingError),
    /// One encoding identifier was missing from the IR registry.
    #[error("missing structured numeric encoding `{encoding_id}`")]
    MissingEncoding {
        /// Stable encoding identifier.
        encoding_id: String,
    },
    /// Failed to create the output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write numeric encoding report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// JSON serialization or parsing failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed report for the structured numeric encoding lane.
pub fn build_tassadar_numeric_encoding_report(
) -> Result<TassadarNumericEncodingReport, TassadarNumericEncodingReportError> {
    let lane_contract = tassadar_structured_numeric_encoding_lane_contract();
    let publication = tassadar_numeric_encoding_publication();
    let encodings = tassadar_structured_numeric_encodings();
    let candidate_reports = lane_contract
        .cases
        .iter()
        .flat_map(|case| {
            case.candidate_encoding_ids
                .iter()
                .map(|candidate_encoding_id| build_candidate_report(case, candidate_encoding_id, encodings.as_slice()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let family_summaries = build_family_summaries(candidate_reports.as_slice());
    let mut report = TassadarNumericEncodingReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.numeric_encoding.report.v1"),
        suite_run_ref: String::from(TASSADAR_NUMERIC_ENCODING_SUITE_RUN_REF),
        lane_contract,
        publication,
        candidate_reports,
        family_summaries,
        claim_boundary: String::from(
            "this report compares legacy one-token-per-value numeric encodings against binary and mixed-radix alternatives on seeded bounded immediate, offset, and address families only; it proves representation-level held-out vocabulary coverage and exact roundtrip semantics, and does not imply arbitrary numeric closure, architecture-independent learned exactness, or served promotion",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(b"psionic_tassadar_numeric_encoding_report|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the committed report.
pub fn tassadar_numeric_encoding_report_path() -> PathBuf {
    repo_root().join(TASSADAR_NUMERIC_ENCODING_REPORT_REF)
}

/// Writes the committed report for the structured numeric encoding lane.
pub fn write_tassadar_numeric_encoding_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarNumericEncodingReport, TassadarNumericEncodingReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarNumericEncodingReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_numeric_encoding_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarNumericEncodingReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_candidate_report(
    case: &psionic_data::TassadarStructuredNumericEncodingCaseContract,
    candidate_encoding_id: &str,
    encodings: &[TassadarStructuredNumericEncoding],
) -> Result<TassadarNumericEncodingCandidateReport, TassadarNumericEncodingReportError> {
    let legacy = find_encoding(encodings, &case.legacy_encoding_id)?;
    let candidate = find_encoding(encodings, candidate_encoding_id)?;
    let legacy_train_vocab = training_vocab(case.train_values.as_slice(), legacy)?;
    let candidate_train_vocab = training_vocab(case.train_values.as_slice(), candidate)?;
    let legacy_held_out_vocab_coverage_bps =
        held_out_vocab_coverage_bps(case.held_out_values.as_slice(), legacy, &legacy_train_vocab)?;
    let candidate_held_out_vocab_coverage_bps = held_out_vocab_coverage_bps(
        case.held_out_values.as_slice(),
        candidate,
        &candidate_train_vocab,
    )?;
    Ok(TassadarNumericEncodingCandidateReport {
        case_id: case.case_id.clone(),
        workload_family: case.workload_family,
        legacy_encoding_id: legacy.encoding_id.clone(),
        candidate_encoding_id: candidate.encoding_id.clone(),
        legacy_held_out_vocab_coverage_bps,
        candidate_held_out_vocab_coverage_bps,
        legacy_mean_tokens_per_value: mean_tokens_per_value(case.train_values.as_slice(), legacy)?,
        candidate_mean_tokens_per_value: mean_tokens_per_value(
            case.train_values.as_slice(),
            candidate,
        )?,
        semantic_roundtrip_exact_bps: semantic_roundtrip_exact_bps(
            case.train_values
                .iter()
                .chain(case.held_out_values.iter())
                .copied()
                .collect::<Vec<_>>()
                .as_slice(),
            legacy,
            candidate,
        )?,
        representation_generalization_gain_bps: candidate_held_out_vocab_coverage_bps as i32
            - legacy_held_out_vocab_coverage_bps as i32,
        claim_boundary: legacy.claim_boundary.clone(),
    })
}

fn build_family_summaries(
    candidate_reports: &[TassadarNumericEncodingCandidateReport],
) -> Vec<TassadarNumericEncodingFamilySummary> {
    let mut grouped =
        std::collections::BTreeMap::<TassadarStructuredNumericEncodingWorkloadFamily, Vec<&TassadarNumericEncodingCandidateReport>>::new();
    for report in candidate_reports {
        grouped.entry(report.workload_family).or_default().push(report);
    }
    let mut summaries = grouped
        .into_iter()
        .map(|(workload_family, reports)| TassadarNumericEncodingFamilySummary {
            workload_family,
            candidate_count: reports.len() as u32,
            mean_representation_generalization_gain_bps: if reports.is_empty() {
                0
            } else {
                reports
                    .iter()
                    .map(|report| i64::from(report.representation_generalization_gain_bps))
                    .sum::<i64>() as i32
                    / reports.len() as i32
            },
            all_roundtrip_exact: reports
                .iter()
                .all(|report| report.semantic_roundtrip_exact_bps == 10_000),
        })
        .collect::<Vec<_>>();
    summaries.sort_by_key(|summary| summary.workload_family);
    summaries
}

fn find_encoding<'a>(
    encodings: &'a [TassadarStructuredNumericEncoding],
    encoding_id: &str,
) -> Result<&'a TassadarStructuredNumericEncoding, TassadarNumericEncodingReportError> {
    encodings
        .iter()
        .find(|encoding| encoding.encoding_id == encoding_id)
        .ok_or_else(|| TassadarNumericEncodingReportError::MissingEncoding {
            encoding_id: String::from(encoding_id),
        })
}

fn training_vocab(
    values: &[u32],
    encoding: &TassadarStructuredNumericEncoding,
) -> Result<BTreeSet<String>, TassadarNumericEncodingReportError> {
    let mut vocab = BTreeSet::new();
    for value in values {
        for token in encode_tassadar_numeric_value(*value, encoding)? {
            vocab.insert(token);
        }
    }
    Ok(vocab)
}

fn held_out_vocab_coverage_bps(
    values: &[u32],
    encoding: &TassadarStructuredNumericEncoding,
    train_vocab: &BTreeSet<String>,
) -> Result<u32, TassadarNumericEncodingReportError> {
    let mut total_tokens = 0_u32;
    let mut covered_tokens = 0_u32;
    for value in values {
        for token in encode_tassadar_numeric_value(*value, encoding)? {
            total_tokens = total_tokens.saturating_add(1);
            if train_vocab.contains(&token) {
                covered_tokens = covered_tokens.saturating_add(1);
            }
        }
    }
    Ok(if total_tokens == 0 {
        0
    } else {
        ((u64::from(covered_tokens) * 10_000) / u64::from(total_tokens)) as u32
    })
}

fn mean_tokens_per_value(
    values: &[u32],
    encoding: &TassadarStructuredNumericEncoding,
) -> Result<u32, TassadarNumericEncodingReportError> {
    if values.is_empty() {
        return Ok(0);
    }
    let total_tokens = values
        .iter()
        .map(|value| encode_tassadar_numeric_value(*value, encoding))
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .map(Vec::len)
        .sum::<usize>();
    Ok(total_tokens as u32 / values.len() as u32)
}

fn semantic_roundtrip_exact_bps(
    values: &[u32],
    legacy: &TassadarStructuredNumericEncoding,
    candidate: &TassadarStructuredNumericEncoding,
) -> Result<u32, TassadarNumericEncodingReportError> {
    if values.is_empty() {
        return Ok(0);
    }
    let mut exact_count = 0_u32;
    let total = (values.len() * 2) as u32;
    for value in values {
        let legacy_tokens = encode_tassadar_numeric_value(*value, legacy)?;
        let candidate_tokens = encode_tassadar_numeric_value(*value, candidate)?;
        if decode_tassadar_numeric_value(legacy_tokens.as_slice(), legacy)? == *value {
            exact_count = exact_count.saturating_add(1);
        }
        if decode_tassadar_numeric_value(candidate_tokens.as_slice(), candidate)? == *value {
            exact_count = exact_count.saturating_add(1);
        }
    }
    Ok(((u64::from(exact_count) * 10_000) / u64::from(total.max(1))) as u32)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
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
        build_tassadar_numeric_encoding_report, tassadar_numeric_encoding_report_path,
        write_tassadar_numeric_encoding_report,
    };

    #[test]
    fn numeric_encoding_report_captures_representation_gain() {
        let report = build_tassadar_numeric_encoding_report().expect("report");
        assert!(report
            .candidate_reports
            .iter()
            .any(|candidate| candidate.representation_generalization_gain_bps > 0));
        assert!(report
            .family_summaries
            .iter()
            .all(|summary| summary.all_roundtrip_exact));
    }

    #[test]
    fn numeric_encoding_report_matches_committed_truth() {
        let generated = build_tassadar_numeric_encoding_report().expect("report");
        let committed = std::fs::read_to_string(tassadar_numeric_encoding_report_path())
            .expect("committed report");
        let committed_report = serde_json::from_str(&committed).expect("decode report");
        assert_eq!(generated, committed_report);
    }

    #[test]
    fn write_numeric_encoding_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join("tassadar_numeric_encoding_report.json");
        let generated = write_tassadar_numeric_encoding_report(&output_path).expect("report");
        let written = std::fs::read_to_string(&output_path).expect("read written");
        let reparsed = serde_json::from_str(&written).expect("decode written report");
        assert_eq!(generated, reparsed);
        let _ = std::fs::remove_file(output_path);
    }
}
