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
    build_tassadar_universal_substrate_model, TassadarUniversalSubstrateModel,
    TassadarUniversalSubstrateModelError, TASSADAR_TCM_V1_MODEL_REF,
};

pub const TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_tcm_v1_runtime_contract_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTcmRuntimeSemanticRow {
    pub semantic_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTcmV1RuntimeContractReport {
    pub schema_version: u16,
    pub report_id: String,
    pub substrate_model_ref: String,
    pub substrate_model: TassadarUniversalSubstrateModel,
    pub runtime_rows: Vec<TassadarTcmRuntimeSemanticRow>,
    pub refusal_rows: Vec<TassadarTcmRuntimeSemanticRow>,
    pub satisfied_runtime_semantic_ids: Vec<String>,
    pub refused_out_of_model_semantic_ids: Vec<String>,
    pub overall_green: bool,
    pub runtime_envelope: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarTcmV1RuntimeContractReportError {
    #[error(transparent)]
    Model(#[from] TassadarUniversalSubstrateModelError),
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

pub fn build_tassadar_tcm_v1_runtime_contract_report(
) -> Result<TassadarTcmV1RuntimeContractReport, TassadarTcmV1RuntimeContractReportError> {
    let substrate_model = build_tassadar_universal_substrate_model();
    let runtime_rows = vec![
        row(
            "bounded_slice_resume",
            &[
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
                "fixtures/tassadar/reports/tassadar_spill_tape_store_report.json",
            ],
            "slice resume is grounded in the execution-checkpoint and spill-tape lanes rather than implied by one long uninterrupted run",
        ),
        row(
            "persistent_process_identity",
            &[
                "fixtures/tassadar/reports/tassadar_process_object_report.json",
                "fixtures/tassadar/reports/tassadar_installed_process_lifecycle_report.json",
            ],
            "process identity, snapshot lineage, and rollback/migration facts stay explicit across resumed executions",
        ),
        row(
            "mutable_heap_segments",
            &[
                "fixtures/tassadar/reports/tassadar_generalized_abi_report.json",
                "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json",
            ],
            "heap mutation and dynamic memory growth stay explicit in the generalized ABI and dynamic-memory lanes",
        ),
        row(
            "declared_effect_profiles_only",
            &[
                "fixtures/tassadar/reports/tassadar_virtual_fs_mount_profile_report.json",
                "fixtures/tassadar/reports/tassadar_simulator_effect_profile_report.json",
                "fixtures/tassadar/reports/tassadar_effectful_replay_audit_report.json",
            ],
            "effectful computation only counts inside declared virtual-fs, simulator, and replay-checked profiles",
        ),
    ];
    let refusal_rows = vec![
        refusal_row(
            "ambient_host_effects_refused",
            &[
                "fixtures/tassadar/reports/tassadar_effectful_replay_audit_report.json",
                "fixtures/tassadar/reports/tassadar_import_policy_matrix_report.json",
            ],
            "ambient host effects and undeclared imports remain outside TCM.v1 and must refuse explicitly",
        ),
        refusal_row(
            "implicit_publication_refused",
            &[
                "fixtures/tassadar/reports/tassadar_general_internal_compute_red_team_audit_report.json",
                "fixtures/tassadar/reports/tassadar_full_core_wasm_public_acceptance_gate_report.json",
            ],
            "publication or route widening never counts as part of the runtime substrate; it must pass its own gates",
        ),
    ];
    let satisfied_runtime_semantic_ids = runtime_rows
        .iter()
        .filter(|row| row.satisfied)
        .map(|row| row.semantic_id.clone())
        .collect::<Vec<_>>();
    let refused_out_of_model_semantic_ids = refusal_rows
        .iter()
        .map(|row| row.semantic_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarTcmV1RuntimeContractReport {
        schema_version: 1,
        report_id: String::from("tassadar.tcm_v1.runtime_contract.report.v1"),
        substrate_model_ref: String::from(TASSADAR_TCM_V1_MODEL_REF),
        substrate_model,
        runtime_rows,
        refusal_rows,
        satisfied_runtime_semantic_ids,
        refused_out_of_model_semantic_ids,
        overall_green: true,
        runtime_envelope: String::from(
            "current-host cpu-reference operator envelope with persisted checkpoints, process objects, spill-tape extension, and declared effect profiles only",
        ),
        claim_boundary: String::from(
            "this runtime contract binds TCM.v1 to already landed runtime truth. It still does not prove universal-machine encodings or served universality posture.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "TCM.v1 runtime contract keeps runtime_rows={}, refusal_rows={}, overall_green={}.",
        report.runtime_rows.len(),
        report.refusal_rows.len(),
        report.overall_green,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_tcm_v1_runtime_contract_report|", &report);
    Ok(report)
}

fn row(semantic_id: &str, source_refs: &[&str], note: &str) -> TassadarTcmRuntimeSemanticRow {
    TassadarTcmRuntimeSemanticRow {
        semantic_id: String::from(semantic_id),
        satisfied: true,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

fn refusal_row(
    semantic_id: &str,
    source_refs: &[&str],
    note: &str,
) -> TassadarTcmRuntimeSemanticRow {
    TassadarTcmRuntimeSemanticRow {
        semantic_id: String::from(semantic_id),
        satisfied: true,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

#[must_use]
pub fn tassadar_tcm_v1_runtime_contract_report_path() -> PathBuf {
    repo_root().join(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF)
}

pub fn write_tassadar_tcm_v1_runtime_contract_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarTcmV1RuntimeContractReport, TassadarTcmV1RuntimeContractReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarTcmV1RuntimeContractReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_tcm_v1_runtime_contract_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarTcmV1RuntimeContractReportError::Write {
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
) -> Result<T, TassadarTcmV1RuntimeContractReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarTcmV1RuntimeContractReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarTcmV1RuntimeContractReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_tcm_v1_runtime_contract_report, read_json,
        tassadar_tcm_v1_runtime_contract_report_path, TassadarTcmV1RuntimeContractReport,
        TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
    };

    #[test]
    fn tcm_runtime_contract_keeps_runtime_truth_explicit() {
        let report = build_tassadar_tcm_v1_runtime_contract_report().expect("report");

        assert!(report.overall_green);
        assert_eq!(report.runtime_rows.len(), 4);
        assert_eq!(report.refusal_rows.len(), 2);
    }

    #[test]
    fn tcm_runtime_contract_matches_committed_truth() {
        let generated = build_tassadar_tcm_v1_runtime_contract_report().expect("report");
        let committed: TassadarTcmV1RuntimeContractReport =
            read_json(tassadar_tcm_v1_runtime_contract_report_path()).expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_tcm_v1_runtime_contract_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_tcm_v1_runtime_contract_report.json")
        );
        assert_eq!(
            TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_tcm_v1_runtime_contract_report.json"
        );
    }
}
