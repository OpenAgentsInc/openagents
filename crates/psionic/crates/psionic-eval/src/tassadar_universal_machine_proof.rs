use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_compiler::{
    build_tassadar_universal_machine_encoding_report, TassadarUniversalMachineEncodingReport,
    TassadarUniversalMachineEncodingReportError, TASSADAR_UNIVERSAL_MACHINE_ENCODING_REPORT_REF,
};
use psionic_runtime::{
    build_tassadar_tcm_v1_runtime_contract_report,
    build_tassadar_universal_machine_simulation_bundle, TassadarTcmV1RuntimeContractReportError,
    TassadarUniversalMachineSimulationBundle, TassadarUniversalMachineSimulationBundleError,
    TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF, TASSADAR_UNIVERSAL_MACHINE_SIMULATION_BUNDLE_REF,
};

pub const TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_universal_machine_proof_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalMachineProofRow {
    pub encoding_id: String,
    pub step_parity: bool,
    pub final_state_parity: bool,
    pub checkpoint_resume_equivalent: bool,
    pub satisfied: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalMachineProofReport {
    pub schema_version: u16,
    pub report_id: String,
    pub encoding_report_ref: String,
    pub encoding_report: TassadarUniversalMachineEncodingReport,
    pub simulation_bundle_ref: String,
    pub simulation_bundle: TassadarUniversalMachineSimulationBundle,
    pub runtime_contract_ref: String,
    pub proof_rows: Vec<TassadarUniversalMachineProofRow>,
    pub green_encoding_ids: Vec<String>,
    pub overall_green: bool,
    pub theory_statement: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarUniversalMachineProofReportError {
    #[error(transparent)]
    Encoding(#[from] TassadarUniversalMachineEncodingReportError),
    #[error(transparent)]
    RuntimeContract(#[from] TassadarTcmV1RuntimeContractReportError),
    #[error(transparent)]
    Simulation(#[from] TassadarUniversalMachineSimulationBundleError),
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

pub fn build_tassadar_universal_machine_proof_report(
) -> Result<TassadarUniversalMachineProofReport, TassadarUniversalMachineProofReportError> {
    let encoding_report = build_tassadar_universal_machine_encoding_report();
    let simulation_bundle = build_tassadar_universal_machine_simulation_bundle()?;
    let runtime_contract = build_tassadar_tcm_v1_runtime_contract_report()?;
    let proof_rows = encoding_report
        .encoding_rows
        .iter()
        .map(|encoding| {
            let simulation = simulation_bundle
                .receipts
                .iter()
                .find(|receipt| receipt.encoding_id == encoding.encoding_id)
                .expect("simulation should cover each encoding");
            let step_parity = simulation.trace.last().map(|state| state.step_index)
                == Some(encoding.final_state_digest.step_index);
            let final_state_parity =
                simulation.final_state_digest == encoding.final_state_digest.digest;
            let checkpoint_resume_equivalent =
                simulation.checkpoint_resume_equivalent && encoding.checkpoint_resume_equivalent;
            TassadarUniversalMachineProofRow {
                encoding_id: encoding.encoding_id.clone(),
                step_parity,
                final_state_parity,
                checkpoint_resume_equivalent,
                satisfied: step_parity && final_state_parity && checkpoint_resume_equivalent,
                note: format!(
                    "encoding `{}` matches the runtime witness trace under runtime envelope `{}`",
                    encoding.encoding_id, runtime_contract.runtime_envelope
                ),
            }
        })
        .collect::<Vec<_>>();
    let green_encoding_ids = proof_rows
        .iter()
        .filter(|row| row.satisfied)
        .map(|row| row.encoding_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarUniversalMachineProofReport {
        schema_version: 1,
        report_id: String::from("tassadar.universal_machine_proof.report.v1"),
        encoding_report_ref: String::from(TASSADAR_UNIVERSAL_MACHINE_ENCODING_REPORT_REF),
        encoding_report,
        simulation_bundle_ref: String::from(TASSADAR_UNIVERSAL_MACHINE_SIMULATION_BUNDLE_REF),
        simulation_bundle,
        runtime_contract_ref: String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
        proof_rows,
        green_encoding_ids,
        overall_green: false,
        theory_statement: String::from(
            "TCM.v1 now has explicit witness encodings and exact runtime simulation targets for a two-register machine and a single-tape machine.",
        ),
        claim_boundary: String::from(
            "this proof report closes the explicit witness construction only. It does not yet constitute the full witness benchmark suite or the final universality gate.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.green_encoding_ids.len() == report.proof_rows.len();
    report.summary = format!(
        "Universal-machine proof report keeps proof_rows={}, green_encoding_ids={}, overall_green={}.",
        report.proof_rows.len(),
        report.green_encoding_ids.len(),
        report.overall_green,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_universal_machine_proof_report|", &report);
    Ok(report)
}

#[must_use]
pub fn tassadar_universal_machine_proof_report_path() -> PathBuf {
    repo_root().join(TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF)
}

pub fn write_tassadar_universal_machine_proof_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarUniversalMachineProofReport, TassadarUniversalMachineProofReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarUniversalMachineProofReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_universal_machine_proof_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarUniversalMachineProofReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarUniversalMachineProofReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarUniversalMachineProofReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarUniversalMachineProofReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_universal_machine_proof_report, read_json,
        tassadar_universal_machine_proof_report_path, TassadarUniversalMachineProofReport,
        TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF,
    };

    #[test]
    fn universal_machine_proof_report_keeps_green_witness_rows() {
        let report = build_tassadar_universal_machine_proof_report().expect("report");

        assert!(report.overall_green);
        assert_eq!(report.proof_rows.len(), 2);
        assert_eq!(report.green_encoding_ids.len(), 2);
    }

    #[test]
    fn universal_machine_proof_report_matches_committed_truth() {
        let generated = build_tassadar_universal_machine_proof_report().expect("report");
        let committed: TassadarUniversalMachineProofReport =
            read_json(tassadar_universal_machine_proof_report_path()).expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_universal_machine_proof_report.json"
        );
    }
}
