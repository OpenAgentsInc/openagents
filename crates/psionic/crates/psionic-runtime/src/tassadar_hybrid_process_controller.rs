use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_HYBRID_PROCESS_CONTROLLER_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_hybrid_process_controller_v1";
pub const TASSADAR_HYBRID_PROCESS_CONTROLLER_BUNDLE_FILE: &str =
    "tassadar_hybrid_process_controller_runtime_bundle.json";
pub const TASSADAR_HYBRID_PROCESS_CONTROLLER_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/runs/tassadar_hybrid_process_controller_v1/tassadar_hybrid_process_controller_runtime_bundle.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarHybridProcessControllerRuntimeStatus {
    CompiledExact,
    HybridVerifierAttached,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHybridProcessControllerCaseReceipt {
    pub case_id: String,
    pub workload_family: String,
    pub runtime_status: TassadarHybridProcessControllerRuntimeStatus,
    pub selected_controller_mode: String,
    pub verifier_attached: bool,
    pub compiled_step_count: u32,
    pub learned_state_step_count: u32,
    pub search_step_count: u32,
    pub verifier_check_count: u32,
    pub verifier_on_exactness_bps: u32,
    pub verifier_off_exactness_bps: u32,
    pub challenge_path_simulated: bool,
    pub challenge_path_green: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub note: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHybridProcessControllerRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub case_receipts: Vec<TassadarHybridProcessControllerCaseReceipt>,
    pub compiled_exact_case_count: u32,
    pub hybrid_case_count: u32,
    pub refused_case_count: u32,
    pub challenge_green_case_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarHybridProcessControllerRuntimeBundleError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn build_tassadar_hybrid_process_controller_runtime_bundle(
) -> TassadarHybridProcessControllerRuntimeBundle {
    let case_receipts = vec![
        compiled_case(
            "hybrid.session_counter_resume.exact.v1",
            "session_counter_resume",
            48,
            0,
            0,
            6,
            9_900,
            9_900,
            "compiled exact remains preferred on simple resumable counters even with verifier attachment enabled",
        ),
        hybrid_case(
            "hybrid.search_frontier_resume.verifier.v1",
            "search_frontier_resume",
            18,
            24,
            16,
            12,
            9_100,
            7_300,
            "verifier-attached hybrid recovers search frontier resumes that drift when verifier attachment is removed",
        ),
        hybrid_case(
            "hybrid.linked_package_worker.verifier.v1",
            "linked_package_worker",
            22,
            20,
            10,
            9,
            8_900,
            7_100,
            "verifier attachment keeps linked worker package transitions challengeable and above the detached hybrid baseline",
        ),
        refusal_case(
            "hybrid.effectful_mailbox_transition.refusal.v1",
            "effectful_mailbox_transition",
            "unsupported_effect_transition",
            "effectful mailbox transitions remain outside the bounded hybrid controller envelope and refuse instead of silently downgrading into detached learned state updates",
        ),
    ];
    let mut bundle = TassadarHybridProcessControllerRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.hybrid_process_controller.runtime_bundle.v1"),
        case_receipts,
        compiled_exact_case_count: 0,
        hybrid_case_count: 0,
        refused_case_count: 0,
        challenge_green_case_count: 0,
        claim_boundary: String::from(
            "this runtime bundle freezes one bounded verifier-attached hybrid controller over named process-style workloads. It does not imply arbitrary hybrid execution, arbitrary effects, or served internal-compute promotion",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.compiled_exact_case_count = bundle
        .case_receipts
        .iter()
        .filter(|case| {
            case.runtime_status == TassadarHybridProcessControllerRuntimeStatus::CompiledExact
        })
        .count() as u32;
    bundle.hybrid_case_count = bundle
        .case_receipts
        .iter()
        .filter(|case| {
            case.runtime_status
                == TassadarHybridProcessControllerRuntimeStatus::HybridVerifierAttached
        })
        .count() as u32;
    bundle.refused_case_count = bundle
        .case_receipts
        .iter()
        .filter(|case| case.runtime_status == TassadarHybridProcessControllerRuntimeStatus::Refused)
        .count() as u32;
    bundle.challenge_green_case_count = bundle
        .case_receipts
        .iter()
        .filter(|case| case.challenge_path_green)
        .count() as u32;
    bundle.summary = format!(
        "Hybrid process controller runtime bundle covers compiled_exact_cases={}, hybrid_cases={}, refused_cases={}, challenge_green_cases={}.",
        bundle.compiled_exact_case_count,
        bundle.hybrid_case_count,
        bundle.refused_case_count,
        bundle.challenge_green_case_count,
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_hybrid_process_controller_runtime_bundle|",
        &bundle,
    );
    bundle
}

#[must_use]
pub fn tassadar_hybrid_process_controller_runtime_bundle_path() -> PathBuf {
    repo_root()
        .join(TASSADAR_HYBRID_PROCESS_CONTROLLER_RUN_ROOT_REF)
        .join(TASSADAR_HYBRID_PROCESS_CONTROLLER_BUNDLE_FILE)
}

pub fn write_tassadar_hybrid_process_controller_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarHybridProcessControllerRuntimeBundle,
    TassadarHybridProcessControllerRuntimeBundleError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarHybridProcessControllerRuntimeBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_hybrid_process_controller_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarHybridProcessControllerRuntimeBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn compiled_case(
    case_id: &str,
    workload_family: &str,
    compiled_step_count: u32,
    learned_state_step_count: u32,
    search_step_count: u32,
    verifier_check_count: u32,
    verifier_on_exactness_bps: u32,
    verifier_off_exactness_bps: u32,
    note: &str,
) -> TassadarHybridProcessControllerCaseReceipt {
    let mut receipt = TassadarHybridProcessControllerCaseReceipt {
        case_id: String::from(case_id),
        workload_family: String::from(workload_family),
        runtime_status: TassadarHybridProcessControllerRuntimeStatus::CompiledExact,
        selected_controller_mode: String::from("compiled_exact"),
        verifier_attached: true,
        compiled_step_count,
        learned_state_step_count,
        search_step_count,
        verifier_check_count,
        verifier_on_exactness_bps,
        verifier_off_exactness_bps,
        challenge_path_simulated: true,
        challenge_path_green: true,
        refusal_reason_id: None,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_hybrid_process_controller_case_receipt|",
        &receipt,
    );
    receipt
}

fn hybrid_case(
    case_id: &str,
    workload_family: &str,
    compiled_step_count: u32,
    learned_state_step_count: u32,
    search_step_count: u32,
    verifier_check_count: u32,
    verifier_on_exactness_bps: u32,
    verifier_off_exactness_bps: u32,
    note: &str,
) -> TassadarHybridProcessControllerCaseReceipt {
    let mut receipt = TassadarHybridProcessControllerCaseReceipt {
        case_id: String::from(case_id),
        workload_family: String::from(workload_family),
        runtime_status: TassadarHybridProcessControllerRuntimeStatus::HybridVerifierAttached,
        selected_controller_mode: String::from("hybrid_verifier_attached"),
        verifier_attached: true,
        compiled_step_count,
        learned_state_step_count,
        search_step_count,
        verifier_check_count,
        verifier_on_exactness_bps,
        verifier_off_exactness_bps,
        challenge_path_simulated: true,
        challenge_path_green: true,
        refusal_reason_id: None,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_hybrid_process_controller_case_receipt|",
        &receipt,
    );
    receipt
}

fn refusal_case(
    case_id: &str,
    workload_family: &str,
    refusal_reason_id: &str,
    note: &str,
) -> TassadarHybridProcessControllerCaseReceipt {
    let mut receipt = TassadarHybridProcessControllerCaseReceipt {
        case_id: String::from(case_id),
        workload_family: String::from(workload_family),
        runtime_status: TassadarHybridProcessControllerRuntimeStatus::Refused,
        selected_controller_mode: String::from("refused"),
        verifier_attached: true,
        compiled_step_count: 0,
        learned_state_step_count: 0,
        search_step_count: 0,
        verifier_check_count: 3,
        verifier_on_exactness_bps: 0,
        verifier_off_exactness_bps: 0,
        challenge_path_simulated: true,
        challenge_path_green: false,
        refusal_reason_id: Some(String::from(refusal_reason_id)),
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_hybrid_process_controller_case_receipt|",
        &receipt,
    );
    receipt
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
) -> Result<T, TassadarHybridProcessControllerRuntimeBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarHybridProcessControllerRuntimeBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarHybridProcessControllerRuntimeBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_hybrid_process_controller_runtime_bundle, read_json,
        tassadar_hybrid_process_controller_runtime_bundle_path,
        write_tassadar_hybrid_process_controller_runtime_bundle,
        TassadarHybridProcessControllerRuntimeBundle, TassadarHybridProcessControllerRuntimeStatus,
    };

    #[test]
    fn hybrid_process_controller_runtime_bundle_keeps_verifier_delta_and_refusal_explicit() {
        let bundle = build_tassadar_hybrid_process_controller_runtime_bundle();

        assert_eq!(bundle.compiled_exact_case_count, 1);
        assert_eq!(bundle.hybrid_case_count, 2);
        assert_eq!(bundle.refused_case_count, 1);
        assert!(bundle.case_receipts.iter().any(|case| {
            case.workload_family == "search_frontier_resume"
                && case.runtime_status
                    == TassadarHybridProcessControllerRuntimeStatus::HybridVerifierAttached
                && case.verifier_on_exactness_bps > case.verifier_off_exactness_bps
        }));
        assert!(bundle.case_receipts.iter().any(|case| {
            case.workload_family == "effectful_mailbox_transition"
                && case.runtime_status == TassadarHybridProcessControllerRuntimeStatus::Refused
                && case.refusal_reason_id.as_deref() == Some("unsupported_effect_transition")
        }));
    }

    #[test]
    fn hybrid_process_controller_runtime_bundle_matches_committed_truth() {
        let generated = build_tassadar_hybrid_process_controller_runtime_bundle();
        let committed: TassadarHybridProcessControllerRuntimeBundle =
            read_json(tassadar_hybrid_process_controller_runtime_bundle_path())
                .expect("committed bundle");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_hybrid_process_controller_runtime_bundle_persists_current_truth() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output_path = output_dir
            .path()
            .join("tassadar_hybrid_process_controller_runtime_bundle.json");
        let bundle = write_tassadar_hybrid_process_controller_runtime_bundle(&output_path)
            .expect("write bundle");
        let written: TassadarHybridProcessControllerRuntimeBundle =
            serde_json::from_str(&std::fs::read_to_string(&output_path).expect("written file"))
                .expect("parse");
        assert_eq!(bundle, written);
    }
}
