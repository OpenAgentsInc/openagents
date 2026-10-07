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
    TassadarComponentLinkingProfileCompilationContract,
    compile_tassadar_component_linking_profile_contract,
};
use psionic_ir::{
    TassadarComponentLinkingProfileContract, tassadar_component_linking_profile_contract,
};
use psionic_runtime::{
    TASSADAR_COMPONENT_LINKING_RUN_ROOT_REF, TASSADAR_COMPONENT_LINKING_RUNTIME_BUNDLE_REF,
    TassadarComponentLinkingCaseReceipt, TassadarComponentLinkingCaseStatus,
    TassadarComponentLinkingRuntimeBundle, build_tassadar_component_linking_runtime_bundle,
};

/// Stable committed report ref for the bounded component/linking profile.
pub const TASSADAR_COMPONENT_LINKING_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_component_linking_profile_report.json";

/// One persisted lineage artifact for an exact component-linking row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComponentLinkingLineageArtifactRef {
    pub case_id: String,
    pub lineage_path: String,
    pub component_refs: Vec<String>,
}

/// Eval-facing case report for one bounded component-linking row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComponentLinkingCaseReport {
    pub case_id: String,
    pub topology_id: String,
    pub status: TassadarComponentLinkingCaseStatus,
    pub interface_type_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage_artifact: Option<TassadarComponentLinkingLineageArtifactRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub note: String,
}

/// Committed eval report for the bounded component/linking profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarComponentLinkingProfileReport {
    pub schema_version: u16,
    pub report_id: String,
    pub ir_contract: TassadarComponentLinkingProfileContract,
    pub compiler_contract: TassadarComponentLinkingProfileCompilationContract,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarComponentLinkingRuntimeBundle,
    pub case_reports: Vec<TassadarComponentLinkingCaseReport>,
    pub green_topology_ids: Vec<String>,
    pub lineage_artifact_case_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub exact_component_parity_count: u32,
    pub exact_refusal_parity_count: u32,
    pub overall_green: bool,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarComponentLinkingProfileReportError {
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

pub fn build_tassadar_component_linking_profile_report(
) -> Result<TassadarComponentLinkingProfileReport, TassadarComponentLinkingProfileReportError> {
    Ok(build_tassadar_component_linking_profile_materialization()?.0)
}

#[must_use]
pub fn tassadar_component_linking_profile_report_path() -> PathBuf {
    repo_root().join(TASSADAR_COMPONENT_LINKING_PROFILE_REPORT_REF)
}

pub fn write_tassadar_component_linking_profile_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarComponentLinkingProfileReport, TassadarComponentLinkingProfileReportError> {
    let output_path = output_path.as_ref();
    let (report, write_plans) = build_tassadar_component_linking_profile_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarComponentLinkingProfileReportError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| {
            TassadarComponentLinkingProfileReportError::Write {
                path: path.display().to_string(),
                error,
            }
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarComponentLinkingProfileReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarComponentLinkingProfileReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_tassadar_component_linking_profile_materialization() -> Result<
    (TassadarComponentLinkingProfileReport, Vec<WritePlan>),
    TassadarComponentLinkingProfileReportError,
> {
    let ir_contract = tassadar_component_linking_profile_contract();
    let compiler_contract = compile_tassadar_component_linking_profile_contract();
    let runtime_bundle = build_tassadar_component_linking_runtime_bundle();
    let runtime_bundle_ref = String::from(TASSADAR_COMPONENT_LINKING_RUNTIME_BUNDLE_REF);
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
        .filter(|case| case.status == TassadarComponentLinkingCaseStatus::ExactComponentParity)
        .map(|case| case.topology_id.clone())
        .collect::<Vec<_>>();
    green_topology_ids.sort();
    green_topology_ids.dedup();
    let lineage_artifact_case_ids = case_reports
        .iter()
        .filter_map(|case| case.lineage_artifact.as_ref().map(|_| case.case_id.clone()))
        .collect::<Vec<_>>();
    let mut report = TassadarComponentLinkingProfileReport {
        schema_version: 1,
        report_id: String::from("tassadar.component_linking_profile.report.v1"),
        ir_contract,
        compiler_contract,
        runtime_bundle_ref,
        runtime_bundle,
        case_reports,
        green_topology_ids,
        lineage_artifact_case_ids,
        portability_envelope_ids: vec![String::from(
            psionic_ir::TASSADAR_COMPONENT_LINKING_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
        )],
        exact_component_parity_count: 0,
        exact_refusal_parity_count: 0,
        overall_green: false,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report covers one bounded component/linking proposal profile with explicit interface-type lowering, lineage artifacts, and incompatible-interface refusal truth. It does not claim arbitrary component-model closure, unrestricted interface lowering, or broader served publication",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.exact_component_parity_count = report
        .case_reports
        .iter()
        .filter(|case| case.status == TassadarComponentLinkingCaseStatus::ExactComponentParity)
        .count() as u32;
    report.exact_refusal_parity_count = report
        .case_reports
        .iter()
        .filter(|case| case.status == TassadarComponentLinkingCaseStatus::ExactRefusalParity)
        .count() as u32;
    report.overall_green =
        report.exact_component_parity_count == 2 && report.exact_refusal_parity_count == 1;
    report.summary = format!(
        "Component-linking profile report covers {} case rows with exact_component_parity={}, exact_refusal_parity={}, lineage_artifact_cases={}, overall_green={}.",
        report.case_reports.len(),
        report.exact_component_parity_count,
        report.exact_refusal_parity_count,
        report.lineage_artifact_case_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_component_linking_profile_report|",
        &report,
    );
    Ok((report, write_plans))
}

fn build_case_materialization(
    case_receipt: &TassadarComponentLinkingCaseReceipt,
) -> Result<
    (
        TassadarComponentLinkingCaseReport,
        Vec<WritePlan>,
        Vec<String>,
    ),
    TassadarComponentLinkingProfileReportError,
> {
    if case_receipt.status == TassadarComponentLinkingCaseStatus::ExactComponentParity {
        let lineage_path = format!(
            "{}/{}_lineage.json",
            TASSADAR_COMPONENT_LINKING_RUN_ROOT_REF, case_receipt.case_id
        );
        let lineage_payload = serde_json::json!({
            "case_id": case_receipt.case_id,
            "topology_id": case_receipt.topology_id,
            "lineage_entries": case_receipt.lineage_entries,
        });
        let write_plan = WritePlan {
            relative_path: lineage_path.clone(),
            bytes: format!(
                "{}\n",
                serde_json::to_string_pretty(&lineage_payload)?
            )
            .into_bytes(),
        };
        let interface_type_ids = case_receipt
            .lineage_entries
            .iter()
            .flat_map(|entry| entry.interface_type_ids.iter().cloned())
            .collect::<Vec<_>>();
        Ok((
            TassadarComponentLinkingCaseReport {
                case_id: case_receipt.case_id.clone(),
                topology_id: case_receipt.topology_id.clone(),
                status: case_receipt.status,
                interface_type_ids,
                lineage_artifact: Some(TassadarComponentLinkingLineageArtifactRef {
                    case_id: case_receipt.case_id.clone(),
                    lineage_path: lineage_path.clone(),
                    component_refs: case_receipt
                        .lineage_entries
                        .iter()
                        .map(|entry| entry.component_ref.clone())
                        .collect(),
                }),
                refusal_reason_id: None,
                note: case_receipt.note.clone(),
            },
            vec![write_plan],
            vec![lineage_path],
        ))
    } else {
        Ok((
            TassadarComponentLinkingCaseReport {
                case_id: case_receipt.case_id.clone(),
                topology_id: case_receipt.topology_id.clone(),
                status: case_receipt.status,
                interface_type_ids: Vec::new(),
                lineage_artifact: None,
                refusal_reason_id: case_receipt.refusal_reason_id.clone(),
                note: case_receipt.note.clone(),
            },
            Vec::new(),
            Vec::new(),
        ))
    }
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
) -> Result<T, TassadarComponentLinkingProfileReportError> {
    let path = repo_root().join(relative_path);
    let json = fs::read_to_string(&path).map_err(|error| {
        TassadarComponentLinkingProfileReportError::Read {
            path: format!("{label}: {}", path.display()),
            error,
        }
    })?;
    serde_json::from_str(&json).map_err(|error| {
        TassadarComponentLinkingProfileReportError::Decode {
            path: format!("{label}: {}", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TASSADAR_COMPONENT_LINKING_PROFILE_REPORT_REF,
        build_tassadar_component_linking_profile_report, read_repo_json,
        tassadar_component_linking_profile_report_path,
        write_tassadar_component_linking_profile_report,
    };

    #[test]
    fn component_linking_profile_report_keeps_lineage_and_refusals_explicit() {
        let report = build_tassadar_component_linking_profile_report().expect("report");

        assert!(report.overall_green);
        assert_eq!(report.green_topology_ids.len(), 2);
        assert_eq!(report.lineage_artifact_case_ids.len(), 2);
        assert!(report.case_reports.iter().any(|case| {
            case.case_id == "checkpoint_resume_component_pair"
                && case
                    .lineage_artifact
                    .as_ref()
                    .map(|artifact| artifact.component_refs.len() == 2)
                    .unwrap_or(false)
        }));
        assert!(report.case_reports.iter().any(|case| {
            case.case_id == "incompatible_component_interface_refusal"
                && case.refusal_reason_id.as_deref() == Some("incompatible_component_interface")
        }));
    }

    #[test]
    fn component_linking_profile_report_matches_committed_truth() {
        let generated = build_tassadar_component_linking_profile_report().expect("report");
        let committed = read_repo_json(
            TASSADAR_COMPONENT_LINKING_PROFILE_REPORT_REF,
            "tassadar_component_linking_profile_report",
        )
        .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_component_linking_profile_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_component_linking_profile_report.json");
        let report = write_tassadar_component_linking_profile_report(&output_path)
            .expect("write report");
        let persisted = std::fs::read_to_string(&output_path).expect("read persisted report");

        assert_eq!(
            report,
            serde_json::from_str(&persisted).expect("decode persisted report")
        );
        assert_eq!(
            tassadar_component_linking_profile_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_component_linking_profile_report.json")
        );
    }
}
