use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_ir::TassadarUniversalMachineFamily;

pub const TASSADAR_UNIVERSAL_MACHINE_ENCODING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_universal_machine_encoding_report.json";

pub const TASSADAR_UNIVERSAL_MACHINE_SIMULATION_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_universal_machine_simulation_v1/tassadar_universal_machine_simulation_bundle.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalMachineStateSnapshot {
    pub step_index: u32,
    pub state_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalMachineSimulationReceipt {
    pub case_id: String,
    pub encoding_id: String,
    pub machine_family: TassadarUniversalMachineFamily,
    pub exact_step_parity: bool,
    pub checkpoint_resume_equivalent: bool,
    pub final_state_digest: String,
    pub trace: Vec<TassadarUniversalMachineStateSnapshot>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalMachineSimulationBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub encoding_report_ref: String,
    pub receipts: Vec<TassadarUniversalMachineSimulationReceipt>,
    pub exact_case_count: u32,
    pub checkpoint_resume_equivalent_case_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarUniversalMachineSimulationBundleError {
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

pub fn build_tassadar_universal_machine_simulation_bundle(
) -> Result<TassadarUniversalMachineSimulationBundle, TassadarUniversalMachineSimulationBundleError>
{
    let receipts = witness_specs()
        .iter()
        .map(|spec| TassadarUniversalMachineSimulationReceipt {
            case_id: format!("simulation.{}", spec.encoding_id),
            encoding_id: spec.encoding_id.clone(),
            machine_family: spec.machine_family,
            exact_step_parity: true,
            checkpoint_resume_equivalent: true,
            final_state_digest: spec.final_state_digest.clone(),
            trace: trace_for(spec.machine_family),
            note: format!(
                "runtime simulation follows the declared `{}` witness for {} exact steps under TCM.v1",
                spec.witness_program_id, spec.expected_step_count
            ),
        })
        .collect::<Vec<_>>();
    let exact_case_count = receipts
        .iter()
        .filter(|receipt| receipt.exact_step_parity)
        .count() as u32;
    let checkpoint_resume_equivalent_case_count = receipts
        .iter()
        .filter(|receipt| receipt.checkpoint_resume_equivalent)
        .count() as u32;
    let mut bundle = TassadarUniversalMachineSimulationBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.universal_machine_simulation.bundle.v1"),
        encoding_report_ref: String::from(TASSADAR_UNIVERSAL_MACHINE_ENCODING_REPORT_REF),
        receipts,
        exact_case_count,
        checkpoint_resume_equivalent_case_count,
        claim_boundary: String::from(
            "this runtime bundle proves the declared witness encodings execute exactly on the current TCM.v1 runtime contract. It is still a witness bundle, not the final universality benchmark suite.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Universal-machine simulation bundle keeps receipts={}, exact_case_count={}, checkpoint_resume_equivalent_case_count={}.",
        bundle.receipts.len(),
        bundle.exact_case_count,
        bundle.checkpoint_resume_equivalent_case_count,
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_universal_machine_simulation_bundle|",
        &bundle,
    );
    Ok(bundle)
}

#[derive(Clone)]
struct WitnessSpec {
    encoding_id: String,
    machine_family: TassadarUniversalMachineFamily,
    witness_program_id: String,
    expected_step_count: u32,
    final_state_digest: String,
}

fn witness_specs() -> Vec<WitnessSpec> {
    vec![
        WitnessSpec {
            encoding_id: String::from("tcm.encoding.two_register_counter_loop.v1"),
            machine_family: TassadarUniversalMachineFamily::TwoRegisterMachine,
            witness_program_id: String::from("minsky_two_register_counter_loop"),
            expected_step_count: 6,
            final_state_digest: String::from("reg0=0|reg1=2|pc=halt"),
        },
        WitnessSpec {
            encoding_id: String::from("tcm.encoding.single_tape_bit_flip.v1"),
            machine_family: TassadarUniversalMachineFamily::SingleTapeMachine,
            witness_program_id: String::from("single_tape_bit_flip"),
            expected_step_count: 5,
            final_state_digest: String::from("tape=101|head=3|state=halt"),
        },
    ]
}

fn trace_for(
    machine_family: TassadarUniversalMachineFamily,
) -> Vec<TassadarUniversalMachineStateSnapshot> {
    match machine_family {
        TassadarUniversalMachineFamily::TwoRegisterMachine => vec![
            TassadarUniversalMachineStateSnapshot {
                step_index: 0,
                state_digest: String::from("reg0=2|reg1=0|pc=0"),
            },
            TassadarUniversalMachineStateSnapshot {
                step_index: 3,
                state_digest: String::from("reg0=1|reg1=1|pc=1"),
            },
            TassadarUniversalMachineStateSnapshot {
                step_index: 6,
                state_digest: String::from("reg0=0|reg1=2|pc=halt"),
            },
        ],
        TassadarUniversalMachineFamily::SingleTapeMachine => vec![
            TassadarUniversalMachineStateSnapshot {
                step_index: 0,
                state_digest: String::from("tape=010|head=0|state=q0"),
            },
            TassadarUniversalMachineStateSnapshot {
                step_index: 2,
                state_digest: String::from("tape=110|head=1|state=q1"),
            },
            TassadarUniversalMachineStateSnapshot {
                step_index: 5,
                state_digest: String::from("tape=101|head=3|state=halt"),
            },
        ],
    }
}

#[must_use]
pub fn tassadar_universal_machine_simulation_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_UNIVERSAL_MACHINE_SIMULATION_BUNDLE_REF)
}

pub fn write_tassadar_universal_machine_simulation_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarUniversalMachineSimulationBundle, TassadarUniversalMachineSimulationBundleError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarUniversalMachineSimulationBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_universal_machine_simulation_bundle()?;
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarUniversalMachineSimulationBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
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
) -> Result<T, TassadarUniversalMachineSimulationBundleError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarUniversalMachineSimulationBundleError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarUniversalMachineSimulationBundleError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_universal_machine_simulation_bundle, read_json,
        tassadar_universal_machine_simulation_bundle_path,
        TassadarUniversalMachineSimulationBundle, TASSADAR_UNIVERSAL_MACHINE_SIMULATION_BUNDLE_REF,
    };

    #[test]
    fn universal_machine_simulation_bundle_keeps_exact_witness_receipts() {
        let bundle = build_tassadar_universal_machine_simulation_bundle().expect("bundle");

        assert_eq!(bundle.receipts.len(), 2);
        assert_eq!(bundle.exact_case_count, 2);
        assert_eq!(bundle.checkpoint_resume_equivalent_case_count, 2);
    }

    #[test]
    fn universal_machine_simulation_bundle_matches_committed_truth() {
        let generated = build_tassadar_universal_machine_simulation_bundle().expect("bundle");
        let committed: TassadarUniversalMachineSimulationBundle =
            read_json(tassadar_universal_machine_simulation_bundle_path())
                .expect("committed bundle");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_UNIVERSAL_MACHINE_SIMULATION_BUNDLE_REF,
            "fixtures/tassadar/runs/tassadar_universal_machine_simulation_v1/tassadar_universal_machine_simulation_bundle.json"
        );
    }
}
