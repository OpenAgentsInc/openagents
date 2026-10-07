use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_ir::{
    build_tassadar_universal_substrate_model, TassadarUniversalMachineFamily,
    TassadarUniversalSubstrateModel, TASSADAR_TCM_V1_MODEL_REF,
};

pub const TASSADAR_UNIVERSAL_MACHINE_ENCODING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_universal_machine_encoding_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalMachineStateDigest {
    pub step_index: u32,
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalMachineEncodingRow {
    pub encoding_id: String,
    pub machine_family: TassadarUniversalMachineFamily,
    pub witness_program_id: String,
    pub checkpoint_resume_equivalent: bool,
    pub expected_step_count: u32,
    pub initial_state_digest: TassadarUniversalMachineStateDigest,
    pub final_state_digest: TassadarUniversalMachineStateDigest,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalMachineEncodingReport {
    pub schema_version: u16,
    pub report_id: String,
    pub substrate_model_ref: String,
    pub substrate_model: TassadarUniversalSubstrateModel,
    pub encoding_rows: Vec<TassadarUniversalMachineEncodingRow>,
    pub exact_encoding_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarUniversalMachineEncodingReportError {
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

#[must_use]
pub fn build_tassadar_universal_machine_encoding_report() -> TassadarUniversalMachineEncodingReport
{
    let substrate_model = build_tassadar_universal_substrate_model();
    let encoding_rows = vec![
        TassadarUniversalMachineEncodingRow {
            encoding_id: String::from("tcm.encoding.two_register_counter_loop.v1"),
            machine_family: TassadarUniversalMachineFamily::TwoRegisterMachine,
            witness_program_id: String::from("minsky_two_register_counter_loop"),
            checkpoint_resume_equivalent: true,
            expected_step_count: 6,
            initial_state_digest: TassadarUniversalMachineStateDigest {
                step_index: 0,
                digest: String::from("reg0=2|reg1=0|pc=0"),
            },
            final_state_digest: TassadarUniversalMachineStateDigest {
                step_index: 6,
                digest: String::from("reg0=0|reg1=2|pc=halt"),
            },
            note: String::from(
                "a minimal two-register witness that repeatedly decrements r0 and increments r1 until halt, demonstrating register-machine style control and mutable state over TCM.v1",
            ),
        },
        TassadarUniversalMachineEncodingRow {
            encoding_id: String::from("tcm.encoding.single_tape_bit_flip.v1"),
            machine_family: TassadarUniversalMachineFamily::SingleTapeMachine,
            witness_program_id: String::from("single_tape_bit_flip"),
            checkpoint_resume_equivalent: true,
            expected_step_count: 5,
            initial_state_digest: TassadarUniversalMachineStateDigest {
                step_index: 0,
                digest: String::from("tape=010|head=0|state=q0"),
            },
            final_state_digest: TassadarUniversalMachineStateDigest {
                step_index: 5,
                digest: String::from("tape=101|head=3|state=halt"),
            },
            note: String::from(
                "a minimal single-tape witness that scans and flips a bounded tape segment, demonstrating tape-style mutation and head movement over TCM.v1",
            ),
        },
    ];
    let exact_encoding_count = encoding_rows.len() as u32;
    let mut report = TassadarUniversalMachineEncodingReport {
        schema_version: 1,
        report_id: String::from("tassadar.universal_machine_encoding.report.v1"),
        substrate_model_ref: String::from(TASSADAR_TCM_V1_MODEL_REF),
        substrate_model,
        encoding_rows,
        exact_encoding_count,
        claim_boundary: String::from(
            "this compiler report declares witness encodings only. It does not yet claim a benchmark suite, a final gate, or served universality posture.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Universal-machine encoding report keeps encoding_rows={}, exact_encoding_count={}.",
        report.encoding_rows.len(),
        report.exact_encoding_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_universal_machine_encoding_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_universal_machine_encoding_report_path() -> PathBuf {
    repo_root().join(TASSADAR_UNIVERSAL_MACHINE_ENCODING_REPORT_REF)
}

pub fn write_tassadar_universal_machine_encoding_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarUniversalMachineEncodingReport, TassadarUniversalMachineEncodingReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarUniversalMachineEncodingReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_universal_machine_encoding_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarUniversalMachineEncodingReportError::Write {
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
) -> Result<T, TassadarUniversalMachineEncodingReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarUniversalMachineEncodingReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarUniversalMachineEncodingReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_universal_machine_encoding_report, read_repo_json,
        tassadar_universal_machine_encoding_report_path, TassadarUniversalMachineEncodingReport,
        TassadarUniversalMachineFamily, TASSADAR_UNIVERSAL_MACHINE_ENCODING_REPORT_REF,
    };

    #[test]
    fn universal_machine_encoding_report_keeps_two_witness_families() {
        let report = build_tassadar_universal_machine_encoding_report();

        assert_eq!(report.encoding_rows.len(), 2);
        assert_eq!(report.exact_encoding_count, 2);
        assert!(report
            .encoding_rows
            .iter()
            .any(|row| row.machine_family == TassadarUniversalMachineFamily::TwoRegisterMachine));
        assert!(report
            .encoding_rows
            .iter()
            .any(|row| row.machine_family == TassadarUniversalMachineFamily::SingleTapeMachine));
    }

    #[test]
    fn universal_machine_encoding_report_matches_committed_truth() {
        let generated = build_tassadar_universal_machine_encoding_report();
        let committed: TassadarUniversalMachineEncodingReport =
            read_repo_json(TASSADAR_UNIVERSAL_MACHINE_ENCODING_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_universal_machine_encoding_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_universal_machine_encoding_report.json")
        );
    }
}
