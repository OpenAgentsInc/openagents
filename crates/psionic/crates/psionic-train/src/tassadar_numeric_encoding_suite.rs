use std::{collections::BTreeSet, fs, path::Path};

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

/// Canonical output root for the structured numeric encoding suite run.
pub const TASSADAR_NUMERIC_ENCODING_SUITE_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/tassadar_numeric_encoding_suite_v1";
/// Canonical machine-readable report file for the structured numeric encoding suite.
pub const TASSADAR_NUMERIC_ENCODING_SUITE_REPORT_FILE: &str = "numeric_encoding_suite.json";
/// Canonical repo-relative report ref for the structured numeric encoding suite.
pub const TASSADAR_NUMERIC_ENCODING_SUITE_REPORT_REF: &str =
    "fixtures/tassadar/runs/tassadar_numeric_encoding_suite_v1/numeric_encoding_suite.json";

/// One legacy-vs-candidate numeric encoding comparison row.
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
    /// Number of training values in the case.
    pub train_value_count: u32,
    /// Number of held-out values in the case.
    pub held_out_value_count: u32,
    /// Training-vocabulary size for the legacy encoding.
    pub legacy_train_vocab_size: u32,
    /// Training-vocabulary size for the candidate encoding.
    pub candidate_train_vocab_size: u32,
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

/// Top-level suite report for the structured numeric encoding lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericEncodingSuiteReport {
    /// Public lane contract.
    pub lane_contract: TassadarStructuredNumericEncodingLaneContract,
    /// Public model-facing publication.
    pub publication: TassadarNumericEncodingPublication,
    /// Ordered legacy-vs-candidate comparison rows.
    pub candidate_reports: Vec<TassadarNumericEncodingCandidateReport>,
    /// Plain-language summary.
    pub summary: String,
    /// Stable report digest.
    pub report_digest: String,
}

/// Errors while materializing the structured numeric encoding suite.
#[derive(Debug, Error)]
pub enum TassadarNumericEncodingSuiteError {
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
    /// Failed to create an output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to write the report.
    #[error("failed to write numeric encoding suite report `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// JSON serialization or parsing failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Executes the seeded structured numeric encoding suite and writes the report.
pub fn execute_tassadar_numeric_encoding_suite(
    output_dir: &Path,
) -> Result<TassadarNumericEncodingSuiteReport, TassadarNumericEncodingSuiteError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarNumericEncodingSuiteError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

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

    let mean_gain_bps = if candidate_reports.is_empty() {
        0
    } else {
        candidate_reports
            .iter()
            .map(|report| i64::from(report.representation_generalization_gain_bps))
            .sum::<i64>()
            / candidate_reports.len() as i64
    };
    let mut report = TassadarNumericEncodingSuiteReport {
        lane_contract,
        publication,
        candidate_reports,
        summary: format!(
            "Structured numeric encoding suite now freezes {} legacy-vs-candidate comparisons with mean held-out vocabulary coverage gain={}bps while keeping roundtrip semantics exact.",
            0, mean_gain_bps
        ),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Structured numeric encoding suite now freezes {} legacy-vs-candidate comparisons with mean held-out vocabulary coverage gain={}bps while keeping roundtrip semantics exact.",
        report.candidate_reports.len(),
        mean_gain_bps
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_numeric_encoding_suite_report|", &report);

    let output_path = output_dir.join(TASSADAR_NUMERIC_ENCODING_SUITE_REPORT_FILE);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarNumericEncodingSuiteError::Write {
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
) -> Result<TassadarNumericEncodingCandidateReport, TassadarNumericEncodingSuiteError> {
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
    let semantic_roundtrip_exact_bps = semantic_roundtrip_exact_bps(
        case.train_values
            .iter()
            .chain(case.held_out_values.iter())
            .copied()
            .collect::<Vec<_>>()
            .as_slice(),
        legacy,
        candidate,
    )?;
    Ok(TassadarNumericEncodingCandidateReport {
        case_id: case.case_id.clone(),
        workload_family: case.workload_family,
        legacy_encoding_id: legacy.encoding_id.clone(),
        candidate_encoding_id: candidate.encoding_id.clone(),
        train_value_count: case.train_values.len() as u32,
        held_out_value_count: case.held_out_values.len() as u32,
        legacy_train_vocab_size: legacy_train_vocab.len() as u32,
        candidate_train_vocab_size: candidate_train_vocab.len() as u32,
        legacy_held_out_vocab_coverage_bps,
        candidate_held_out_vocab_coverage_bps,
        legacy_mean_tokens_per_value: mean_tokens_per_value(case.train_values.as_slice(), legacy)?,
        candidate_mean_tokens_per_value: mean_tokens_per_value(
            case.train_values.as_slice(),
            candidate,
        )?,
        semantic_roundtrip_exact_bps,
        representation_generalization_gain_bps: candidate_held_out_vocab_coverage_bps as i32
            - legacy_held_out_vocab_coverage_bps as i32,
        claim_boundary: legacy.claim_boundary.clone(),
    })
}

fn find_encoding<'a>(
    encodings: &'a [TassadarStructuredNumericEncoding],
    encoding_id: &str,
) -> Result<&'a TassadarStructuredNumericEncoding, TassadarNumericEncodingSuiteError> {
    encodings
        .iter()
        .find(|encoding| encoding.encoding_id == encoding_id)
        .ok_or_else(|| TassadarNumericEncodingSuiteError::MissingEncoding {
            encoding_id: String::from(encoding_id),
        })
}

fn training_vocab(
    values: &[u32],
    encoding: &TassadarStructuredNumericEncoding,
) -> Result<BTreeSet<String>, TassadarNumericEncodingSuiteError> {
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
) -> Result<u32, TassadarNumericEncodingSuiteError> {
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
) -> Result<u32, TassadarNumericEncodingSuiteError> {
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
) -> Result<u32, TassadarNumericEncodingSuiteError> {
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

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).expect("numeric encoding suite report should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::tempdir;

    use super::{
        execute_tassadar_numeric_encoding_suite, TassadarNumericEncodingSuiteReport,
        TASSADAR_NUMERIC_ENCODING_SUITE_OUTPUT_DIR, TASSADAR_NUMERIC_ENCODING_SUITE_REPORT_REF,
    };

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    #[test]
    fn numeric_encoding_suite_shows_candidate_vocab_gain() {
        let output_dir = tempdir().expect("temp dir");
        let report = execute_tassadar_numeric_encoding_suite(output_dir.path())
            .expect("suite report should build");
        assert!(report
            .candidate_reports
            .iter()
            .all(|candidate| candidate.semantic_roundtrip_exact_bps == 10_000));
        assert!(report
            .candidate_reports
            .iter()
            .any(|candidate| candidate.representation_generalization_gain_bps > 0));
        assert!(TASSADAR_NUMERIC_ENCODING_SUITE_OUTPUT_DIR.contains("numeric_encoding_suite"));
    }

    #[test]
    fn numeric_encoding_suite_matches_committed_truth() {
        let output_dir = tempdir().expect("temp dir");
        let report = execute_tassadar_numeric_encoding_suite(output_dir.path())
            .expect("suite report should build");
        let committed = fs::read_to_string(
            repo_root().join(TASSADAR_NUMERIC_ENCODING_SUITE_REPORT_REF),
        )
        .expect("committed numeric encoding suite should exist");
        let committed_report: TassadarNumericEncodingSuiteReport =
            serde_json::from_str(&committed)
                .expect("committed numeric encoding suite should parse");
        assert_eq!(report, committed_report);
    }
}
