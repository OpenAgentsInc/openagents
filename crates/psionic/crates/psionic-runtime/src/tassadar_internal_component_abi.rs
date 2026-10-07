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
    TASSADAR_INTERNAL_COMPONENT_ABI_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
    TASSADAR_INTERNAL_COMPONENT_ABI_PROFILE_ID,
};

pub const TASSADAR_INTERNAL_COMPONENT_ABI_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_internal_component_abi_v1/tassadar_internal_component_abi_bundle.json";
pub const TASSADAR_INTERNAL_COMPONENT_ABI_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_internal_component_abi_v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalComponentAbiCaseStatus {
    ExactInterfaceParity,
    ExactRefusalParity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComponentPortDescriptor {
    pub component_ref: String,
    pub port_role_id: String,
    pub interface_id: String,
    pub type_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComponentInterfaceManifest {
    pub case_id: String,
    pub component_graph_id: String,
    pub interface_id: String,
    pub component_ports: Vec<TassadarInternalComponentPortDescriptor>,
    pub manifest_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComponentAbiCaseReceipt {
    pub case_id: String,
    pub component_graph_id: String,
    pub interface_id: String,
    pub status: TassadarInternalComponentAbiCaseStatus,
    pub component_ports: Vec<TassadarInternalComponentPortDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_manifest_path: Option<String>,
    pub exact_interface_parity: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComponentAbiRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub profile_id: String,
    pub portability_envelope_id: String,
    pub exact_interface_parity_count: u32,
    pub exact_refusal_parity_count: u32,
    pub case_receipts: Vec<TassadarInternalComponentAbiCaseReceipt>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarInternalComponentAbiBundleError {
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

#[derive(Clone)]
struct WritePlan {
    relative_path: String,
    bytes: Vec<u8>,
}

#[must_use]
pub fn build_tassadar_internal_component_abi_bundle() -> TassadarInternalComponentAbiRuntimeBundle {
    let case_receipts = vec![
        success_case(
            "session_checkpoint_counter_stack",
            "session_checkpoint_counter_stack",
            "session_counter_checkpoint_v1",
            vec![
                port(
                    "session_process_component@1.0.0",
                    "session_loop",
                    "session_counter_checkpoint_v1",
                    &["record_session_delta_v1", "record_counter_value_v1"],
                ),
                port(
                    "checkpoint_codec_component@1.0.0",
                    "checkpoint_codec",
                    "session_counter_checkpoint_v1",
                    &["option_process_snapshot_ref_v1", "result_process_snapshot_ref_v1"],
                ),
                port(
                    "process_snapshot_store_component@1.0.0",
                    "snapshot_store",
                    "session_counter_checkpoint_v1",
                    &["result_process_snapshot_ref_v1"],
                ),
            ],
            &[
                "fixtures/tassadar/reports/tassadar_session_process_profile_report.json",
                "fixtures/tassadar/reports/tassadar_process_object_report.json",
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
            ],
            "bounded internal software closure keeps one session-loop plus checkpoint plus snapshot-store stack with explicit interface contracts instead of widening to arbitrary plugin graphs",
        ),
        success_case(
            "artifact_retry_reader_stack",
            "artifact_retry_reader_stack",
            "artifact_reader_retry_job_v1",
            vec![
                port(
                    "artifact_mount_reader_component@1.0.0",
                    "artifact_reader",
                    "artifact_reader_retry_job_v1",
                    &["record_artifact_mount_ref_v1", "result_read_batch_v1"],
                ),
                port(
                    "async_retry_scheduler_component@1.0.0",
                    "retry_scheduler",
                    "artifact_reader_retry_job_v1",
                    &["record_retry_budget_v1", "record_refusal_code_v1"],
                ),
                port(
                    "work_queue_dispatch_component@1.0.0",
                    "job_dispatch",
                    "artifact_reader_retry_job_v1",
                    &["result_read_batch_v1", "record_refusal_code_v1"],
                ),
            ],
            &[
                "fixtures/tassadar/reports/tassadar_virtual_fs_mount_profile_report.json",
                "fixtures/tassadar/reports/tassadar_async_lifecycle_profile_report.json",
                "fixtures/tassadar/reports/tassadar_process_object_report.json",
            ],
            "bounded internal software closure keeps one artifact-reader plus retry-scheduler plus dispatch stack with typed retry budgets and refusal codes instead of widening to arbitrary async orchestration",
        ),
        success_case(
            "spill_resume_adapter_stack",
            "spill_resume_adapter_stack",
            "spill_resume_adapter_v1",
            vec![
                port(
                    "spill_segment_loader_component@1.0.0",
                    "spill_loader",
                    "spill_resume_adapter_v1",
                    &["record_spill_segment_refs_v1", "record_memory_window_v1"],
                ),
                port(
                    "resume_runtime_component@1.0.0",
                    "resume_runtime",
                    "spill_resume_adapter_v1",
                    &["record_tape_cursor_v1", "result_resume_token_v1"],
                ),
                port(
                    "memory_window_adapter_component@1.0.0",
                    "memory_window_adapter",
                    "spill_resume_adapter_v1",
                    &["result_resume_token_v1", "record_memory_window_v1"],
                ),
            ],
            &[
                "fixtures/tassadar/reports/tassadar_spill_tape_store_report.json",
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
                "fixtures/tassadar/reports/tassadar_effect_safe_resume_report.json",
            ],
            "bounded internal software closure keeps one spill-loader plus resumable runtime plus memory-window adapter stack with typed continuation handles instead of widening to arbitrary persistent memory services",
        ),
        refusal_case(
            "cross_profile_handle_mismatch_refusal",
            "session_checkpoint_counter_stack",
            "session_counter_checkpoint_v1",
            "cross_profile_handle_mismatch",
            &[
                "fixtures/tassadar/reports/tassadar_component_linking_profile_report.json",
                "fixtures/tassadar/reports/tassadar_process_object_report.json",
            ],
            "cross-profile snapshot and continuation handles stay as typed refusal truth instead of widening compatibility across unrelated internal-compute lanes",
        ),
        refusal_case(
            "unsupported_variant_union_refusal",
            "artifact_retry_reader_stack",
            "artifact_reader_retry_job_v1",
            "unsupported_variant_union_shape",
            &[
                "fixtures/tassadar/reports/tassadar_component_linking_profile_report.json",
                "fixtures/tassadar/reports/tassadar_virtual_fs_mount_profile_report.json",
            ],
            "variant-union payloads stay as typed refusal truth instead of implying general interface-union lowering or arbitrary runtime discovery",
        ),
    ];
    let exact_interface_parity_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarInternalComponentAbiCaseStatus::ExactInterfaceParity)
        .count() as u32;
    let exact_refusal_parity_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarInternalComponentAbiCaseStatus::ExactRefusalParity)
        .count() as u32;
    let mut bundle = TassadarInternalComponentAbiRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.internal_component_abi.runtime_bundle.v1"),
        profile_id: String::from(TASSADAR_INTERNAL_COMPONENT_ABI_PROFILE_ID),
        portability_envelope_id: String::from(
            TASSADAR_INTERNAL_COMPONENT_ABI_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
        ),
        exact_interface_parity_count,
        exact_refusal_parity_count,
        case_receipts,
        claim_boundary: String::from(
            "this runtime bundle proves one bounded internal-compute component-model ABI lane with explicit interface manifests and typed refusal on handle mismatches and unsupported union shapes. It does not claim arbitrary component-model closure, arbitrary host-import composition, or broader served publication",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Internal component ABI runtime bundle covers {} cases with exact_interface_parity={} and exact_refusal_parity={}.",
        bundle.case_receipts.len(),
        bundle.exact_interface_parity_count,
        bundle.exact_refusal_parity_count,
    );
    bundle.bundle_digest =
        stable_digest(b"psionic_tassadar_internal_component_abi_bundle|", &bundle);
    bundle
}

#[must_use]
pub fn tassadar_internal_component_abi_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_INTERNAL_COMPONENT_ABI_BUNDLE_REF)
}

pub fn write_tassadar_internal_component_abi_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarInternalComponentAbiRuntimeBundle, TassadarInternalComponentAbiBundleError> {
    let output_path = output_path.as_ref();
    let (bundle, write_plans) = build_tassadar_internal_component_abi_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarInternalComponentAbiBundleError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| {
            TassadarInternalComponentAbiBundleError::Write {
                path: path.display().to_string(),
                error,
            }
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInternalComponentAbiBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInternalComponentAbiBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn build_tassadar_internal_component_abi_materialization(
) -> Result<
    (TassadarInternalComponentAbiRuntimeBundle, Vec<WritePlan>),
    TassadarInternalComponentAbiBundleError,
> {
    let bundle = build_tassadar_internal_component_abi_bundle();
    let mut write_plans = vec![WritePlan {
        relative_path: String::from(TASSADAR_INTERNAL_COMPONENT_ABI_BUNDLE_REF),
        bytes: json_bytes(&bundle)?,
    }];
    for case in &bundle.case_receipts {
        if let Some(path) = &case.interface_manifest_path {
            let manifest = TassadarInternalComponentInterfaceManifest {
                case_id: case.case_id.clone(),
                component_graph_id: case.component_graph_id.clone(),
                interface_id: case.interface_id.clone(),
                component_ports: case.component_ports.clone(),
                manifest_digest: stable_digest(
                    b"psionic_tassadar_internal_component_interface_manifest|",
                    &(&case.case_id, &case.component_graph_id, &case.interface_id, &case.component_ports),
                ),
            };
            write_plans.push(WritePlan {
                relative_path: path.clone(),
                bytes: json_bytes(&manifest)?,
            });
        }
    }
    Ok((bundle, write_plans))
}

fn port(
    component_ref: &str,
    port_role_id: &str,
    interface_id: &str,
    type_ids: &[&str],
) -> TassadarInternalComponentPortDescriptor {
    TassadarInternalComponentPortDescriptor {
        component_ref: String::from(component_ref),
        port_role_id: String::from(port_role_id),
        interface_id: String::from(interface_id),
        type_ids: type_ids.iter().map(|value| String::from(*value)).collect(),
    }
}

fn success_case(
    case_id: &str,
    component_graph_id: &str,
    interface_id: &str,
    component_ports: Vec<TassadarInternalComponentPortDescriptor>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarInternalComponentAbiCaseReceipt {
    let interface_manifest_path = format!(
        "{}/{}_interface_manifest.json",
        TASSADAR_INTERNAL_COMPONENT_ABI_RUN_ROOT_REF, case_id
    );
    let mut receipt = TassadarInternalComponentAbiCaseReceipt {
        case_id: String::from(case_id),
        component_graph_id: String::from(component_graph_id),
        interface_id: String::from(interface_id),
        status: TassadarInternalComponentAbiCaseStatus::ExactInterfaceParity,
        component_ports,
        interface_manifest_path: Some(interface_manifest_path),
        exact_interface_parity: true,
        refusal_reason_id: None,
        benchmark_refs: benchmark_refs.iter().map(|value| String::from(*value)).collect(),
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest =
        stable_digest(b"psionic_tassadar_internal_component_abi_case|", &receipt);
    receipt
}

fn refusal_case(
    case_id: &str,
    component_graph_id: &str,
    interface_id: &str,
    refusal_reason_id: &str,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarInternalComponentAbiCaseReceipt {
    let mut receipt = TassadarInternalComponentAbiCaseReceipt {
        case_id: String::from(case_id),
        component_graph_id: String::from(component_graph_id),
        interface_id: String::from(interface_id),
        status: TassadarInternalComponentAbiCaseStatus::ExactRefusalParity,
        component_ports: Vec::new(),
        interface_manifest_path: None,
        exact_interface_parity: false,
        refusal_reason_id: Some(String::from(refusal_reason_id)),
        benchmark_refs: benchmark_refs.iter().map(|value| String::from(*value)).collect(),
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest =
        stable_digest(b"psionic_tassadar_internal_component_abi_case|", &receipt);
    receipt
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, TassadarInternalComponentAbiBundleError> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?).into_bytes())
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
) -> Result<T, TassadarInternalComponentAbiBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarInternalComponentAbiBundleError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarInternalComponentAbiBundleError::Decode {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TASSADAR_INTERNAL_COMPONENT_ABI_PROFILE_ID, build_tassadar_internal_component_abi_bundle,
        read_json, tassadar_internal_component_abi_bundle_path,
        write_tassadar_internal_component_abi_bundle,
    };

    #[test]
    fn internal_component_abi_bundle_keeps_interface_manifests_and_refusals_explicit() {
        let bundle = build_tassadar_internal_component_abi_bundle();

        assert_eq!(bundle.profile_id, TASSADAR_INTERNAL_COMPONENT_ABI_PROFILE_ID);
        assert_eq!(bundle.exact_interface_parity_count, 3);
        assert_eq!(bundle.exact_refusal_parity_count, 2);
        assert!(bundle.case_receipts.iter().any(|case| {
            case.case_id == "artifact_retry_reader_stack" && case.interface_manifest_path.is_some()
        }));
        assert!(bundle.case_receipts.iter().any(|case| {
            case.case_id == "cross_profile_handle_mismatch_refusal"
                && case.refusal_reason_id.as_deref() == Some("cross_profile_handle_mismatch")
        }));
    }

    #[test]
    fn internal_component_abi_bundle_matches_committed_truth() {
        let generated = build_tassadar_internal_component_abi_bundle();
        let committed =
            read_json(tassadar_internal_component_abi_bundle_path()).expect("committed bundle");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_internal_component_abi_bundle_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_internal_component_abi_bundle.json");
        let bundle =
            write_tassadar_internal_component_abi_bundle(&output_path).expect("write bundle");
        let persisted = read_json(&output_path).expect("persisted bundle");

        assert_eq!(bundle, persisted);
    }
}
