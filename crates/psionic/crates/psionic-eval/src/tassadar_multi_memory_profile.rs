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
    TassadarMultiMemoryProfileCompilationContract,
    compile_tassadar_multi_memory_profile_contract,
};
use psionic_ir::{TassadarMultiMemoryProfileContract, tassadar_multi_memory_profile_contract};
use psionic_runtime::{
    TASSADAR_MULTI_MEMORY_RUNTIME_BUNDLE_REF, TASSADAR_MULTI_MEMORY_RUN_ROOT_REF,
    TassadarMultiMemoryCaseReceipt, TassadarMultiMemoryCaseStatus,
    TassadarMultiMemoryRuntimeBundle, build_tassadar_multi_memory_runtime_bundle,
};

/// Stable committed report ref for the bounded multi-memory routing profile.
pub const TASSADAR_MULTI_MEMORY_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_multi_memory_profile_report.json";

/// One persisted per-memory snapshot artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiMemorySnapshotArtifactRef {
    pub memory_id: String,
    pub snapshot_path: String,
    pub snapshot_digest: String,
}

/// One persisted multi-memory checkpoint artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiMemoryCheckpointArtifactRef {
    pub checkpoint_id: String,
    pub checkpoint_path: String,
    pub memory_snapshots: Vec<TassadarMultiMemorySnapshotArtifactRef>,
}

/// Eval-facing case report for one bounded multi-memory routing row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiMemoryCaseReport {
    pub case_id: String,
    pub topology_id: String,
    pub status: TassadarMultiMemoryCaseStatus,
    pub route_memory_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_artifact: Option<TassadarMultiMemoryCheckpointArtifactRef>,
    pub exact_route_parity: bool,
    pub exact_resume_parity: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub note: String,
}

/// Committed eval report for the bounded multi-memory routing profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMultiMemoryProfileReport {
    pub schema_version: u16,
    pub report_id: String,
    pub ir_contract: TassadarMultiMemoryProfileContract,
    pub compiler_contract: TassadarMultiMemoryProfileCompilationContract,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarMultiMemoryRuntimeBundle,
    pub case_reports: Vec<TassadarMultiMemoryCaseReport>,
    pub green_topology_ids: Vec<String>,
    pub checkpoint_capable_case_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub exact_routing_parity_count: u32,
    pub exact_resume_parity_count: u32,
    pub exact_refusal_parity_count: u32,
    pub overall_green: bool,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarMultiMemoryProfileReportError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
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
}

#[derive(Clone)]
struct WritePlan {
    relative_path: String,
    bytes: Vec<u8>,
}

pub fn build_tassadar_multi_memory_profile_report(
) -> Result<TassadarMultiMemoryProfileReport, TassadarMultiMemoryProfileReportError> {
    Ok(build_tassadar_multi_memory_profile_materialization()?.0)
}

#[must_use]
pub fn tassadar_multi_memory_profile_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MULTI_MEMORY_PROFILE_REPORT_REF)
}

pub fn write_tassadar_multi_memory_profile_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarMultiMemoryProfileReport, TassadarMultiMemoryProfileReportError> {
    let output_path = output_path.as_ref();
    let (report, write_plans) = build_tassadar_multi_memory_profile_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarMultiMemoryProfileReportError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| {
            TassadarMultiMemoryProfileReportError::Write {
                path: path.display().to_string(),
                error,
            }
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarMultiMemoryProfileReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarMultiMemoryProfileReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_tassadar_multi_memory_profile_materialization() -> Result<
    (TassadarMultiMemoryProfileReport, Vec<WritePlan>),
    TassadarMultiMemoryProfileReportError,
> {
    let ir_contract = tassadar_multi_memory_profile_contract();
    let compiler_contract = compile_tassadar_multi_memory_profile_contract();
    let runtime_bundle = build_tassadar_multi_memory_runtime_bundle();
    let runtime_bundle_ref = String::from(TASSADAR_MULTI_MEMORY_RUNTIME_BUNDLE_REF);
    let mut generated_from_refs = vec![runtime_bundle_ref.clone()];
    let mut write_plans = vec![WritePlan {
        relative_path: runtime_bundle_ref.clone(),
        bytes: json_bytes(&runtime_bundle)?,
    }];
    let mut case_reports = Vec::new();
    for case_receipt in &runtime_bundle.case_receipts {
        let (case_report, case_write_plans, case_generated_from_refs) =
            build_case_materialization(case_receipt)?;
        case_reports.push(case_report);
        write_plans.extend(case_write_plans);
        generated_from_refs.extend(case_generated_from_refs);
    }
    generated_from_refs.extend(
        compiler_contract
            .case_specs
            .iter()
            .flat_map(|case| case.benchmark_refs.iter().cloned()),
    );
    generated_from_refs.sort();
    generated_from_refs.dedup();

    let mut green_topology_ids = case_reports
        .iter()
        .filter(|case| case.status == TassadarMultiMemoryCaseStatus::ExactRoutingParity)
        .map(|case| case.topology_id.clone())
        .collect::<Vec<_>>();
    green_topology_ids.sort();
    green_topology_ids.dedup();

    let checkpoint_capable_case_ids = case_reports
        .iter()
        .filter_map(|case| {
            case.checkpoint_artifact
                .as_ref()
                .map(|_| case.case_id.clone())
        })
        .collect::<Vec<_>>();

    let mut report = TassadarMultiMemoryProfileReport {
        schema_version: 1,
        report_id: String::from("tassadar.multi_memory_profile.report.v1"),
        ir_contract,
        compiler_contract,
        runtime_bundle_ref,
        runtime_bundle,
        case_reports,
        green_topology_ids,
        checkpoint_capable_case_ids,
        portability_envelope_ids: vec![String::from(
            psionic_ir::TASSADAR_MULTI_MEMORY_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
        )],
        exact_routing_parity_count: 0,
        exact_resume_parity_count: 0,
        exact_refusal_parity_count: 0,
        overall_green: false,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report covers one bounded multi-memory routing profile with explicit topology ids, persisted checkpoint artifacts, and malformed-topology refusal truth. It does not claim arbitrary Wasm multi-memory closure, memory64 plus multi-memory mixing, or broader served publication",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.exact_routing_parity_count = report
        .case_reports
        .iter()
        .filter(|case| case.status == TassadarMultiMemoryCaseStatus::ExactRoutingParity)
        .count() as u32;
    report.exact_resume_parity_count = report
        .case_reports
        .iter()
        .filter(|case| case.exact_resume_parity)
        .count() as u32;
    report.exact_refusal_parity_count = report
        .case_reports
        .iter()
        .filter(|case| case.status == TassadarMultiMemoryCaseStatus::ExactRefusalParity)
        .count() as u32;
    report.overall_green =
        report.exact_routing_parity_count == 2 && report.exact_refusal_parity_count == 1;
    report.summary = format!(
        "Multi-memory profile report covers {} case rows with routing_parity={}, resume_parity={}, refusal_parity={}, checkpoint_capable_cases={}, overall_green={}.",
        report.case_reports.len(),
        report.exact_routing_parity_count,
        report.exact_resume_parity_count,
        report.exact_refusal_parity_count,
        report.checkpoint_capable_case_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_multi_memory_profile_report|", &report);
    Ok((report, write_plans))
}

fn build_case_materialization(
    case_receipt: &TassadarMultiMemoryCaseReceipt,
) -> Result<
    (
        TassadarMultiMemoryCaseReport,
        Vec<WritePlan>,
        Vec<String>,
    ),
    TassadarMultiMemoryProfileReportError,
> {
    if let Some(checkpoint) = &case_receipt.checkpoint {
        let checkpoint_stem = checkpoint_stem(checkpoint.checkpoint_id.as_str());
        let checkpoint_path = format!(
            "{}/{}_checkpoint.json",
            TASSADAR_MULTI_MEMORY_RUN_ROOT_REF, checkpoint_stem
        );
        let mut memory_snapshot_paths = Vec::new();
        let mut write_plans = vec![WritePlan {
            relative_path: checkpoint_path.clone(),
            bytes: json_bytes(checkpoint)?,
        }];
        let mut snapshot_artifacts = Vec::new();
        for (index, memory_id) in checkpoint.memory_order.iter().enumerate() {
            let snapshot_path = format!(
                "{}/{}_{}_memory_snapshot.json",
                TASSADAR_MULTI_MEMORY_RUN_ROOT_REF, checkpoint_stem, memory_id
            );
            let snapshot_payload = serde_json::json!({
                "memory_id": memory_id,
                "memory_index": index,
                "memory_digest": checkpoint
                    .per_memory_digests
                    .get(index)
                    .cloned()
                    .unwrap_or_default(),
                "checkpoint_id": checkpoint.checkpoint_id,
            });
            let snapshot_bytes = serde_json::to_vec_pretty(&snapshot_payload)?;
            write_plans.push(WritePlan {
                relative_path: snapshot_path.clone(),
                bytes: format!(
                    "{}\n",
                    String::from_utf8(snapshot_bytes).expect("json bytes are utf8")
                )
                .into_bytes(),
            });
            memory_snapshot_paths.push(snapshot_path.clone());
            snapshot_artifacts.push(TassadarMultiMemorySnapshotArtifactRef {
                memory_id: memory_id.clone(),
                snapshot_path,
                snapshot_digest: checkpoint
                    .per_memory_digests
                    .get(index)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        let generated_from_refs = std::iter::once(checkpoint_path.clone())
            .chain(memory_snapshot_paths.iter().cloned())
            .collect::<Vec<_>>();
        Ok((
            TassadarMultiMemoryCaseReport {
                case_id: case_receipt.case_id.clone(),
                topology_id: case_receipt.topology_id.clone(),
                status: case_receipt.status,
                route_memory_ids: case_receipt
                    .routes
                    .iter()
                    .map(|route| route.memory_id.clone())
                    .collect(),
                checkpoint_artifact: Some(TassadarMultiMemoryCheckpointArtifactRef {
                    checkpoint_id: checkpoint.checkpoint_id.clone(),
                    checkpoint_path,
                    memory_snapshots: snapshot_artifacts,
                }),
                exact_route_parity: case_receipt.exact_route_parity,
                exact_resume_parity: case_receipt.exact_resume_parity,
                refusal_reason_id: None,
                note: case_receipt.note.clone(),
            },
            write_plans,
            generated_from_refs,
        ))
    } else {
        Ok((
            TassadarMultiMemoryCaseReport {
                case_id: case_receipt.case_id.clone(),
                topology_id: case_receipt.topology_id.clone(),
                status: case_receipt.status,
                route_memory_ids: case_receipt
                    .routes
                    .iter()
                    .map(|route| route.memory_id.clone())
                    .collect(),
                checkpoint_artifact: None,
                exact_route_parity: case_receipt.exact_route_parity,
                exact_resume_parity: case_receipt.exact_resume_parity,
                refusal_reason_id: case_receipt.refusal_reason_id.clone(),
                note: case_receipt.note.clone(),
            },
            Vec::new(),
            Vec::new(),
        ))
    }
}

fn checkpoint_stem(checkpoint_id: &str) -> String {
    checkpoint_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?).into_bytes())
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
    label: &str,
) -> Result<T, TassadarMultiMemoryProfileReportError> {
    let path = repo_root().join(relative_path);
    let json = fs::read_to_string(&path).map_err(|error| {
        TassadarMultiMemoryProfileReportError::Read {
            path: format!("{label}: {}", path.display()),
            error,
        }
    })?;
    serde_json::from_str(&json).map_err(|error| TassadarMultiMemoryProfileReportError::Decode {
        path: format!("{label}: {}", path.display()),
        error,
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TASSADAR_MULTI_MEMORY_PROFILE_REPORT_REF, build_tassadar_multi_memory_profile_report,
        read_repo_json, tassadar_multi_memory_profile_report_path,
        write_tassadar_multi_memory_profile_report,
    };

    #[test]
    fn multi_memory_profile_report_keeps_routes_checkpoints_and_refusals_explicit() {
        let report = build_tassadar_multi_memory_profile_report().expect("report");

        assert!(report.overall_green);
        assert_eq!(report.green_topology_ids.len(), 2);
        assert_eq!(report.checkpoint_capable_case_ids.len(), 1);
        assert!(report.case_reports.iter().any(|case| {
            case.case_id == "scratch_heap_checkpoint_route"
                && case
                    .checkpoint_artifact
                    .as_ref()
                    .map(|artifact| artifact.memory_snapshots.len() == 2)
                    .unwrap_or(false)
        }));
        assert!(report.case_reports.iter().any(|case| {
            case.case_id == "malformed_memory_topology_refusal"
                && case.refusal_reason_id.as_deref() == Some("malformed_memory_topology")
        }));
    }

    #[test]
    fn multi_memory_profile_report_matches_committed_truth() {
        let generated = build_tassadar_multi_memory_profile_report().expect("report");
        let committed = read_repo_json(
            TASSADAR_MULTI_MEMORY_PROFILE_REPORT_REF,
            "tassadar_multi_memory_profile_report",
        )
        .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_multi_memory_profile_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir.path().join("tassadar_multi_memory_profile_report.json");
        let report =
            write_tassadar_multi_memory_profile_report(&output_path).expect("write report");
        let persisted = std::fs::read_to_string(&output_path).expect("read persisted report");

        assert_eq!(
            report,
            serde_json::from_str(&persisted).expect("decode persisted report")
        );
        assert_eq!(
            tassadar_multi_memory_profile_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_multi_memory_profile_report.json")
        );
    }
}
