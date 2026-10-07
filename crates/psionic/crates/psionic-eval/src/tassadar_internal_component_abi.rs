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
    TassadarInternalComponentAbiCompilationContract, compile_tassadar_internal_component_abi_contract,
};
use psionic_ir::{
    TassadarInternalComponentAbiContract, tassadar_internal_component_abi_contract,
};
use psionic_runtime::{
    TASSADAR_INTERNAL_COMPONENT_ABI_BUNDLE_REF, TassadarInternalComponentAbiCaseStatus,
    TassadarInternalComponentAbiRuntimeBundle, build_tassadar_internal_component_abi_bundle,
};

pub const TASSADAR_INTERNAL_COMPONENT_ABI_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_component_abi_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComponentAbiCaseReport {
    pub case_id: String,
    pub component_graph_id: String,
    pub interface_id: String,
    pub status: TassadarInternalComponentAbiCaseStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_manifest_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComponentAbiReport {
    pub schema_version: u16,
    pub report_id: String,
    pub ir_contract: TassadarInternalComponentAbiContract,
    pub compiler_contract: TassadarInternalComponentAbiCompilationContract,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarInternalComponentAbiRuntimeBundle,
    pub case_reports: Vec<TassadarInternalComponentAbiCaseReport>,
    pub green_component_graph_ids: Vec<String>,
    pub interface_manifest_case_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub served_publication_allowed: bool,
    pub overall_green: bool,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarInternalComponentAbiReportError {
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

pub fn build_tassadar_internal_component_abi_report(
) -> Result<TassadarInternalComponentAbiReport, TassadarInternalComponentAbiReportError> {
    let ir_contract = tassadar_internal_component_abi_contract();
    let compiler_contract = compile_tassadar_internal_component_abi_contract();
    let runtime_bundle = build_tassadar_internal_component_abi_bundle();
    let runtime_bundle_ref = String::from(TASSADAR_INTERNAL_COMPONENT_ABI_BUNDLE_REF);
    let mut generated_from_refs = vec![runtime_bundle_ref.clone()];
    let case_reports = runtime_bundle
        .case_receipts
        .iter()
        .map(|case| {
            if let Some(path) = &case.interface_manifest_path {
                generated_from_refs.push(path.clone());
            }
            TassadarInternalComponentAbiCaseReport {
                case_id: case.case_id.clone(),
                component_graph_id: case.component_graph_id.clone(),
                interface_id: case.interface_id.clone(),
                status: case.status,
                interface_manifest_path: case.interface_manifest_path.clone(),
                refusal_reason_id: case.refusal_reason_id.clone(),
                note: case.note.clone(),
            }
        })
        .collect::<Vec<_>>();
    generated_from_refs.extend(
        compiler_contract
            .case_specs
            .iter()
            .flat_map(|case| case.benchmark_refs.iter().cloned()),
    );
    generated_from_refs.sort();
    generated_from_refs.dedup();

    let mut green_component_graph_ids = case_reports
        .iter()
        .filter(|case| case.status == TassadarInternalComponentAbiCaseStatus::ExactInterfaceParity)
        .map(|case| case.component_graph_id.clone())
        .collect::<Vec<_>>();
    green_component_graph_ids.sort();
    green_component_graph_ids.dedup();

    let interface_manifest_case_ids = case_reports
        .iter()
        .filter_map(|case| case.interface_manifest_path.as_ref().map(|_| case.case_id.clone()))
        .collect::<Vec<_>>();

    let mut report = TassadarInternalComponentAbiReport {
        schema_version: 1,
        report_id: String::from("tassadar.internal_component_abi.report.v1"),
        ir_contract,
        compiler_contract,
        runtime_bundle_ref,
        runtime_bundle,
        case_reports,
        green_component_graph_ids,
        interface_manifest_case_ids,
        portability_envelope_ids: vec![String::from(
            psionic_ir::TASSADAR_INTERNAL_COMPONENT_ABI_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
        )],
        served_publication_allowed: false,
        overall_green: false,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report covers one bounded internal-compute component-model ABI lane with explicit interface manifests, typed handle-mismatch and union-shape refusals, and benchmark-only promotion posture. It does not claim arbitrary component-model closure, arbitrary host-import composition, or broader served publication",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.green_component_graph_ids.len() == 3
        && report.interface_manifest_case_ids.len() == 3
        && report
            .case_reports
            .iter()
            .filter(|case| case.status == TassadarInternalComponentAbiCaseStatus::ExactRefusalParity)
            .count()
            == 2;
    report.summary = format!(
        "Internal component ABI report covers graphs={}, interface_manifest_cases={}, served_publication_allowed={}, overall_green={}.",
        report.green_component_graph_ids.len(),
        report.interface_manifest_case_ids.len(),
        report.served_publication_allowed,
        report.overall_green,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_internal_component_abi_report|", &report);
    Ok(report)
}

#[must_use]
pub fn tassadar_internal_component_abi_report_path() -> PathBuf {
    repo_root().join(TASSADAR_INTERNAL_COMPONENT_ABI_REPORT_REF)
}

pub fn write_tassadar_internal_component_abi_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarInternalComponentAbiReport, TassadarInternalComponentAbiReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInternalComponentAbiReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_internal_component_abi_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInternalComponentAbiReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarInternalComponentAbiReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarInternalComponentAbiReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarInternalComponentAbiReportError::Decode {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_internal_component_abi_report, read_json,
        tassadar_internal_component_abi_report_path, write_tassadar_internal_component_abi_report,
    };

    #[test]
    fn internal_component_abi_report_keeps_manifest_and_benchmark_only_truth_explicit() {
        let report = build_tassadar_internal_component_abi_report().expect("report");

        assert_eq!(report.green_component_graph_ids.len(), 3);
        assert_eq!(report.interface_manifest_case_ids.len(), 3);
        assert!(!report.served_publication_allowed);
        assert!(report.overall_green);
    }

    #[test]
    fn internal_component_abi_report_matches_committed_truth() {
        let generated = build_tassadar_internal_component_abi_report().expect("report");
        let committed =
            read_json(tassadar_internal_component_abi_report_path()).expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_internal_component_abi_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_internal_component_abi_report.json");
        let report =
            write_tassadar_internal_component_abi_report(&output_path).expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
    }
}
