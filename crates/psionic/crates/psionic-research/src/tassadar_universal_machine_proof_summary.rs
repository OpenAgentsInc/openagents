use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_universal_machine_proof_report, TassadarUniversalMachineProofReport,
    TassadarUniversalMachineProofReportError,
};

pub const TASSADAR_UNIVERSAL_MACHINE_PROOF_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_universal_machine_proof_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalMachineProofSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub eval_report: TassadarUniversalMachineProofReport,
    pub witness_family_ids: Vec<String>,
    pub allowed_statement: String,
    pub explicit_non_implications: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarUniversalMachineProofSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarUniversalMachineProofReportError),
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

pub fn build_tassadar_universal_machine_proof_summary(
) -> Result<TassadarUniversalMachineProofSummary, TassadarUniversalMachineProofSummaryError> {
    let eval_report = build_tassadar_universal_machine_proof_report()?;
    let witness_family_ids = eval_report.green_encoding_ids.clone();
    let explicit_non_implications = vec![
        String::from("full witness benchmark suite"),
        String::from("minimal universal-substrate gate"),
        String::from("served universality posture"),
    ];
    let mut summary = TassadarUniversalMachineProofSummary {
        schema_version: 1,
        report_id: String::from("tassadar.universal_machine_proof.summary.v1"),
        eval_report,
        witness_family_ids,
        allowed_statement: String::from(
            "Psionic/Tassadar now has explicit witness constructions over `TCM.v1` for a two-register machine and a single-tape machine, with exact runtime parity on the committed proof targets.",
        ),
        explicit_non_implications,
        claim_boundary: String::from(
            "this summary closes the witness construction only. It still does not claim the dedicated witness benchmark suite, the minimal universal-substrate gate, or served universality posture.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    summary.summary = format!(
        "Universal-machine proof summary keeps witness_family_ids={}, explicit_non_implications={}, overall_green={}.",
        summary.witness_family_ids.len(),
        summary.explicit_non_implications.len(),
        summary.eval_report.overall_green,
    );
    summary.report_digest = stable_digest(
        b"psionic_tassadar_universal_machine_proof_summary|",
        &summary,
    );
    Ok(summary)
}

#[must_use]
pub fn tassadar_universal_machine_proof_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_UNIVERSAL_MACHINE_PROOF_SUMMARY_REF)
}

pub fn write_tassadar_universal_machine_proof_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarUniversalMachineProofSummary, TassadarUniversalMachineProofSummaryError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarUniversalMachineProofSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_universal_machine_proof_summary()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarUniversalMachineProofSummaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
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
) -> Result<T, TassadarUniversalMachineProofSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarUniversalMachineProofSummaryError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarUniversalMachineProofSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_universal_machine_proof_summary, read_repo_json,
        tassadar_universal_machine_proof_summary_path, TassadarUniversalMachineProofSummary,
        TASSADAR_UNIVERSAL_MACHINE_PROOF_SUMMARY_REF,
    };

    #[test]
    fn universal_machine_proof_summary_keeps_two_green_witnesses() {
        let summary = build_tassadar_universal_machine_proof_summary().expect("summary");

        assert_eq!(summary.witness_family_ids.len(), 2);
        assert!(summary
            .explicit_non_implications
            .contains(&String::from("minimal universal-substrate gate")));
    }

    #[test]
    fn universal_machine_proof_summary_matches_committed_truth() {
        let generated = build_tassadar_universal_machine_proof_summary().expect("summary");
        let committed: TassadarUniversalMachineProofSummary =
            read_repo_json(TASSADAR_UNIVERSAL_MACHINE_PROOF_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_universal_machine_proof_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_universal_machine_proof_summary.json")
        );
    }
}
