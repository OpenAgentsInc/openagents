use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_data::{
    TassadarUniversalityWitnessExpectation, TassadarUniversalityWitnessFamily,
    TASSADAR_UNIVERSALITY_WITNESS_SUITE_SUMMARY_REF,
};
use psionic_eval::{
    build_tassadar_universality_witness_suite_report, TassadarUniversalityWitnessSuiteReport,
    TassadarUniversalityWitnessSuiteReportError,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalityWitnessSuiteSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub eval_report: TassadarUniversalityWitnessSuiteReport,
    pub exact_family_ids: Vec<TassadarUniversalityWitnessFamily>,
    pub refusal_boundary_family_ids: Vec<TassadarUniversalityWitnessFamily>,
    pub allowed_statement: String,
    pub explicit_non_implications: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarUniversalityWitnessSuiteSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarUniversalityWitnessSuiteReportError),
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

pub fn build_tassadar_universality_witness_suite_summary(
) -> Result<TassadarUniversalityWitnessSuiteSummary, TassadarUniversalityWitnessSuiteSummaryError> {
    let eval_report = build_tassadar_universality_witness_suite_report()?;
    let exact_family_ids = eval_report
        .family_rows
        .iter()
        .filter(|row| row.expected_status == TassadarUniversalityWitnessExpectation::Exact)
        .filter(|row| row.satisfied)
        .map(|row| row.witness_family)
        .collect::<Vec<_>>();
    let refusal_boundary_family_ids = eval_report
        .family_rows
        .iter()
        .filter(|row| {
            row.expected_status == TassadarUniversalityWitnessExpectation::RefusalBoundary
        })
        .filter(|row| row.satisfied)
        .map(|row| row.witness_family)
        .collect::<Vec<_>>();
    let explicit_non_implications = eval_report.explicit_non_implications.clone();
    let mut summary = TassadarUniversalityWitnessSuiteSummary {
        schema_version: 1,
        report_id: String::from("tassadar.universality_witness_suite.summary.v1"),
        eval_report,
        exact_family_ids,
        refusal_boundary_family_ids,
        allowed_statement: String::from(
            "Psionic/Tassadar now has a dedicated universality witness benchmark suite over `TCM.v1` covering two-register and single-tape witnesses, a deterministic vm-style interpreter family, deterministic session-process kernels, spill/tape continuation, and explicit refusal boundaries on VM parameter ABI and open-ended external event loops.",
        ),
        explicit_non_implications,
        claim_boundary: String::from(
            "this summary closes the dedicated witness benchmark suite only. It still does not claim the minimal universal-substrate gate, the theory/operator/served verdict split, served universality posture, arbitrary Wasm, or Turing-complete closeout by itself.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    summary.summary = format!(
        "Universality witness suite summary keeps exact_family_ids={}, refusal_boundary_family_ids={}, overall_green={}.",
        summary.exact_family_ids.len(),
        summary.refusal_boundary_family_ids.len(),
        summary.eval_report.overall_green,
    );
    summary.report_digest = stable_digest(
        b"psionic_tassadar_universality_witness_suite_summary|",
        &summary,
    );
    Ok(summary)
}

#[must_use]
pub fn tassadar_universality_witness_suite_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_UNIVERSALITY_WITNESS_SUITE_SUMMARY_REF)
}

pub fn write_tassadar_universality_witness_suite_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarUniversalityWitnessSuiteSummary, TassadarUniversalityWitnessSuiteSummaryError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarUniversalityWitnessSuiteSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_universality_witness_suite_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarUniversalityWitnessSuiteSummaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(summary)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root")
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
) -> Result<T, TassadarUniversalityWitnessSuiteSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarUniversalityWitnessSuiteSummaryError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarUniversalityWitnessSuiteSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_universality_witness_suite_summary, read_repo_json,
        tassadar_universality_witness_suite_summary_path, TassadarUniversalityWitnessSuiteSummary,
    };
    use psionic_data::{
        TassadarUniversalityWitnessFamily, TASSADAR_UNIVERSALITY_WITNESS_SUITE_SUMMARY_REF,
    };

    #[test]
    fn universality_witness_suite_summary_keeps_exact_and_refusal_sets_explicit() {
        let summary = build_tassadar_universality_witness_suite_summary().expect("summary");

        assert_eq!(summary.exact_family_ids.len(), 5);
        assert_eq!(summary.refusal_boundary_family_ids.len(), 2);
        assert!(summary
            .exact_family_ids
            .contains(&TassadarUniversalityWitnessFamily::SpillTapeContinuation));
        assert!(summary
            .refusal_boundary_family_ids
            .contains(&TassadarUniversalityWitnessFamily::BytecodeVmParamBoundary));
    }

    #[test]
    fn universality_witness_suite_summary_matches_committed_truth() {
        let generated = build_tassadar_universality_witness_suite_summary().expect("summary");
        let committed: TassadarUniversalityWitnessSuiteSummary =
            read_repo_json(TASSADAR_UNIVERSALITY_WITNESS_SUITE_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_universality_witness_suite_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_universality_witness_suite_summary.json")
        );
    }
}
