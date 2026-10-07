use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_cross_profile_link_compatibility_v1";
pub const TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_BUNDLE_FILE: &str =
    "tassadar_cross_profile_link_compatibility_bundle.json";
pub const TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_cross_profile_link_compatibility_report.json";
pub const TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_REPORT_ID: &str =
    "tassadar.cross_profile_link_compatibility.report.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCrossProfileLinkRuntimeStatus {
    Exact,
    Downgraded,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCrossProfileLinkRuntimeCaseReceipt {
    pub case_id: String,
    pub producer_profile_id: String,
    pub consumer_profile_id: String,
    pub runtime_status: TassadarCrossProfileLinkRuntimeStatus,
    pub realized_profile_ids: Vec<String>,
    pub effective_portability_envelope_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_stack_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downgrade_target_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub exact_trace_parity: bool,
    pub exact_output_parity: bool,
    pub downgrade_preserves_determinism: bool,
    pub note: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCrossProfileLinkCompatibilityRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub compiler_report_ref: String,
    pub compiler_report_id: String,
    pub case_receipts: Vec<TassadarCrossProfileLinkRuntimeCaseReceipt>,
    pub exact_case_count: u32,
    pub downgraded_case_count: u32,
    pub refused_case_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarCrossProfileLinkCompatibilityRuntimeBundleError {
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
pub fn build_tassadar_cross_profile_link_compatibility_runtime_bundle()
-> TassadarCrossProfileLinkCompatibilityRuntimeBundle {
    let case_receipts = vec![
        exact_case(
            "compat.session_process_to_spill_tape.exact.v1",
            "tassadar.internal_compute.session_process.v1",
            "tassadar.internal_compute.spill_tape_store.v1",
            &[
                "tassadar.internal_compute.session_process.v1",
                "tassadar.internal_compute.spill_tape_store.v1",
            ],
            Some("session_checkpoint_spill_adapter_v1"),
            "deterministic session transcripts may link to the bounded spill-backed continuation lane through one explicit checkpoint adapter without widening either profile into a generic process runtime",
        ),
        downgraded_case(
            "compat.generalized_abi_to_component_model.downgrade.v1",
            "tassadar.internal_compute.generalized_abi.v1",
            "tassadar.internal_compute.component_model_abi.v1",
            &["tassadar.internal_compute.component_model_abi.v1"],
            Some("generalized_abi_to_component_manifest_adapter_v1"),
            "tassadar.internal_compute.component_model_abi.v1",
            "generalized multi-value heap results stay linkable only by downgrading into one explicit component-model manifest adapter instead of silently pretending the broader ABI is native on both sides",
        ),
        refusal_case(
            "compat.spill_tape_to_portable_broad_family.refusal.v1",
            "tassadar.internal_compute.spill_tape_store.v1",
            "tassadar.internal_compute.portable_broad_family.v1",
            "portability_envelope_mismatch",
            "spill-backed continuation artifacts remain current-host cpu-reference objects and refuse silent widening into portable broad-family process state",
        ),
        refusal_case(
            "compat.session_process_to_deterministic_import.refusal.v1",
            "tassadar.internal_compute.session_process.v1",
            "tassadar.internal_compute.deterministic_import_subset.v1",
            "effect_boundary_mismatch",
            "interactive session turns do not inherit deterministic-import resume authority, so cross-profile linking refuses instead of flattening effect-safe replay into generic interaction closure",
        ),
    ];
    let mut bundle = TassadarCrossProfileLinkCompatibilityRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.cross_profile_link_compatibility.bundle.v1"),
        compiler_report_ref: String::from(TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_REPORT_REF),
        compiler_report_id: String::from(TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_REPORT_ID),
        case_receipts,
        exact_case_count: 0,
        downgraded_case_count: 0,
        refused_case_count: 0,
        claim_boundary: String::from(
            "this runtime bundle freezes one bounded cross-profile linking lane with explicit exact parity, downgrade-preserving parity, and typed refusal. It does not claim arbitrary profile interop, implicit portability lifting, or general internal-compute composition",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.exact_case_count = bundle
        .case_receipts
        .iter()
        .filter(|case| case.runtime_status == TassadarCrossProfileLinkRuntimeStatus::Exact)
        .count() as u32;
    bundle.downgraded_case_count = bundle
        .case_receipts
        .iter()
        .filter(|case| case.runtime_status == TassadarCrossProfileLinkRuntimeStatus::Downgraded)
        .count() as u32;
    bundle.refused_case_count = bundle
        .case_receipts
        .iter()
        .filter(|case| case.runtime_status == TassadarCrossProfileLinkRuntimeStatus::Refused)
        .count() as u32;
    bundle.summary = format!(
        "Cross-profile link runtime bundle covers exact_cases={}, downgraded_cases={}, refused_cases={}, case_receipts={}.",
        bundle.exact_case_count,
        bundle.downgraded_case_count,
        bundle.refused_case_count,
        bundle.case_receipts.len(),
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_cross_profile_link_compatibility_runtime_bundle|",
        &bundle,
    );
    bundle
}

#[must_use]
pub fn tassadar_cross_profile_link_compatibility_runtime_bundle_path() -> PathBuf {
    repo_root()
        .join(TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_RUN_ROOT_REF)
        .join(TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_BUNDLE_FILE)
}

pub fn write_tassadar_cross_profile_link_compatibility_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarCrossProfileLinkCompatibilityRuntimeBundle,
    TassadarCrossProfileLinkCompatibilityRuntimeBundleError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarCrossProfileLinkCompatibilityRuntimeBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_cross_profile_link_compatibility_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarCrossProfileLinkCompatibilityRuntimeBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn exact_case(
    case_id: &str,
    producer_profile_id: &str,
    consumer_profile_id: &str,
    realized_profile_ids: &[&str],
    adapter_stack_id: Option<&str>,
    note: &str,
) -> TassadarCrossProfileLinkRuntimeCaseReceipt {
    let mut receipt = TassadarCrossProfileLinkRuntimeCaseReceipt {
        case_id: String::from(case_id),
        producer_profile_id: String::from(producer_profile_id),
        consumer_profile_id: String::from(consumer_profile_id),
        runtime_status: TassadarCrossProfileLinkRuntimeStatus::Exact,
        realized_profile_ids: realized_profile_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        effective_portability_envelope_id: String::from("cpu_reference_current_host"),
        adapter_stack_id: adapter_stack_id.map(String::from),
        downgrade_target_profile_id: None,
        refusal_reason_id: None,
        exact_trace_parity: true,
        exact_output_parity: true,
        downgrade_preserves_determinism: false,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_cross_profile_link_runtime_case_receipt|",
        &receipt,
    );
    receipt
}

fn downgraded_case(
    case_id: &str,
    producer_profile_id: &str,
    consumer_profile_id: &str,
    realized_profile_ids: &[&str],
    adapter_stack_id: Option<&str>,
    downgrade_target_profile_id: &str,
    note: &str,
) -> TassadarCrossProfileLinkRuntimeCaseReceipt {
    let mut receipt = TassadarCrossProfileLinkRuntimeCaseReceipt {
        case_id: String::from(case_id),
        producer_profile_id: String::from(producer_profile_id),
        consumer_profile_id: String::from(consumer_profile_id),
        runtime_status: TassadarCrossProfileLinkRuntimeStatus::Downgraded,
        realized_profile_ids: realized_profile_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        effective_portability_envelope_id: String::from("cpu_reference_current_host"),
        adapter_stack_id: adapter_stack_id.map(String::from),
        downgrade_target_profile_id: Some(String::from(downgrade_target_profile_id)),
        refusal_reason_id: None,
        exact_trace_parity: true,
        exact_output_parity: true,
        downgrade_preserves_determinism: true,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_cross_profile_link_runtime_case_receipt|",
        &receipt,
    );
    receipt
}

fn refusal_case(
    case_id: &str,
    producer_profile_id: &str,
    consumer_profile_id: &str,
    refusal_reason_id: &str,
    note: &str,
) -> TassadarCrossProfileLinkRuntimeCaseReceipt {
    let mut receipt = TassadarCrossProfileLinkRuntimeCaseReceipt {
        case_id: String::from(case_id),
        producer_profile_id: String::from(producer_profile_id),
        consumer_profile_id: String::from(consumer_profile_id),
        runtime_status: TassadarCrossProfileLinkRuntimeStatus::Refused,
        realized_profile_ids: Vec::new(),
        effective_portability_envelope_id: String::from("cpu_reference_current_host"),
        adapter_stack_id: None,
        downgrade_target_profile_id: None,
        refusal_reason_id: Some(String::from(refusal_reason_id)),
        exact_trace_parity: false,
        exact_output_parity: false,
        downgrade_preserves_determinism: false,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(
        b"psionic_tassadar_cross_profile_link_runtime_case_receipt|",
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
) -> Result<T, TassadarCrossProfileLinkCompatibilityRuntimeBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarCrossProfileLinkCompatibilityRuntimeBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarCrossProfileLinkCompatibilityRuntimeBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TassadarCrossProfileLinkRuntimeStatus,
        build_tassadar_cross_profile_link_compatibility_runtime_bundle, read_json,
        tassadar_cross_profile_link_compatibility_runtime_bundle_path,
        write_tassadar_cross_profile_link_compatibility_runtime_bundle,
    };

    #[test]
    fn cross_profile_link_runtime_bundle_keeps_exact_downgraded_and_refused_parity_explicit() {
        let bundle = build_tassadar_cross_profile_link_compatibility_runtime_bundle();

        assert_eq!(bundle.exact_case_count, 1);
        assert_eq!(bundle.downgraded_case_count, 1);
        assert_eq!(bundle.refused_case_count, 2);
        assert!(bundle.case_receipts.iter().any(|case| {
            case.runtime_status == TassadarCrossProfileLinkRuntimeStatus::Downgraded
                && case.downgrade_preserves_determinism
        }));
    }

    #[test]
    fn cross_profile_link_runtime_bundle_matches_committed_truth() {
        let generated = build_tassadar_cross_profile_link_compatibility_runtime_bundle();
        let committed = read_json(tassadar_cross_profile_link_compatibility_runtime_bundle_path())
            .expect("committed bundle");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_cross_profile_link_runtime_bundle_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_cross_profile_link_compatibility_bundle.json");
        let bundle = write_tassadar_cross_profile_link_compatibility_runtime_bundle(&output_path)
            .expect("write bundle");
        let persisted = read_json(&output_path).expect("persisted bundle");

        assert_eq!(bundle, persisted);
    }
}
