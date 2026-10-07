use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_ir::{TassadarModuleImportClass, seeded_tassadar_computational_module_manifests};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_MODULE_LINK_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_link_runtime_report.json";

/// Link posture observed on the runtime-facing linked-program witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleLinkRuntimePosture {
    Exact,
    RolledBack,
    Refused,
}

/// Typed refusal reason kept explicit in the runtime report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleLinkRuntimeRefusalReason {
    ConflictingVersion,
    MissingRequestedModule,
    MissingInternalDependency,
}

/// One runtime dependency edge preserved for replay-safe link evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleLinkRuntimeEdge {
    /// Importing module ref.
    pub importer_module_ref: String,
    /// Imported symbol.
    pub import_symbol: String,
    /// Provider module ref.
    pub provider_module_ref: String,
}

/// One linked-program runtime case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleLinkRuntimeCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable consumer family.
    pub consumer_family: String,
    /// Ordered requested module refs.
    pub requested_module_refs: Vec<String>,
    /// Ordered selected module refs after rollback or refusal.
    pub selected_module_refs: Vec<String>,
    /// Dependency edges realized by the selected module set.
    pub dependency_edges: Vec<TassadarModuleLinkRuntimeEdge>,
    /// Final link posture.
    pub posture: TassadarModuleLinkRuntimePosture,
    /// Typed refusal reason when posture is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarModuleLinkRuntimeRefusalReason>,
    /// Explicit rollback detail when posture is rolled back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_detail: Option<String>,
    /// Whether outputs stayed exact on the current witness.
    pub exact_outputs_preserved: bool,
    /// Whether traces stayed exact on the current witness.
    pub exact_trace_match: bool,
    /// Stable benchmark refs anchoring the case.
    pub benchmark_refs: Vec<String>,
    /// Stable dependency-graph digest for the case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_graph_digest: Option<String>,
    /// Plain-language case note.
    pub note: String,
}

/// Runtime report for the bounded module-linker lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleLinkRuntimeReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Number of exact link cases.
    pub exact_case_count: u32,
    /// Number of rollback link cases.
    pub rollback_case_count: u32,
    /// Number of refused link cases.
    pub refused_case_count: u32,
    /// Total dependency edges preserved across exact and rollback cases.
    pub dependency_edge_count: u32,
    /// Per-case runtime reports.
    pub case_reports: Vec<TassadarModuleLinkRuntimeCaseReport>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Runtime failure while persisting or validating the report.
#[derive(Debug, Error)]
pub enum TassadarModuleLinkRuntimeReportError {
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

/// Builds the machine-legible bounded module-linker runtime report.
#[must_use]
pub fn build_tassadar_module_link_runtime_report() -> TassadarModuleLinkRuntimeReport {
    let case_reports = vec![
        build_case_report(
            "link.verifier_search.exact.v1",
            "verifier_search",
            &[
                "candidate_select_core@1.1.0",
                "checkpoint_backtrack_core@1.0.0",
            ],
            &[
                "candidate_select_core@1.1.0",
                "checkpoint_backtrack_core@1.0.0",
            ],
            TassadarModuleLinkRuntimePosture::Exact,
            None,
            None,
            true,
            true,
            &["fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"],
            "verifier_search links the candidate selector and checkpoint core directly, preserving one explicit internal dependency edge and exact parity on the seeded witness",
        ),
        build_case_report(
            "link.hungarian_matching.rollback.v1",
            "hungarian_matching",
            &["candidate_select_core@1.2.0"],
            &["candidate_select_core@1.1.0"],
            TassadarModuleLinkRuntimePosture::RolledBack,
            None,
            Some(
                "candidate_select_core@1.2.0 rolled back to candidate_select_core@1.1.0 under the explicit replacement plan because the candidate build is not published as active",
            ),
            true,
            true,
            &["fixtures/tassadar/reports/tassadar_internal_module_library_report.json"],
            "hungarian_matching preserves linked-program parity by selecting the explicit rollback target rather than silently drifting across candidate versions",
        ),
        build_case_report(
            "link.hungarian_matching.conflict.v1",
            "hungarian_matching",
            &["candidate_select_core@1.1.0", "candidate_select_core@1.2.0"],
            &[],
            TassadarModuleLinkRuntimePosture::Refused,
            Some(TassadarModuleLinkRuntimeRefusalReason::ConflictingVersion),
            None,
            false,
            false,
            &["fixtures/tassadar/reports/tassadar_internal_module_library_report.json"],
            "hungarian_matching refuses when conflicting candidate_select_core versions are requested in the same bounded link set",
        ),
    ];
    let mut report = TassadarModuleLinkRuntimeReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.module_link_runtime.report.v1"),
        exact_case_count: case_reports
            .iter()
            .filter(|case| case.posture == TassadarModuleLinkRuntimePosture::Exact)
            .count() as u32,
        rollback_case_count: case_reports
            .iter()
            .filter(|case| case.posture == TassadarModuleLinkRuntimePosture::RolledBack)
            .count() as u32,
        refused_case_count: case_reports
            .iter()
            .filter(|case| case.posture == TassadarModuleLinkRuntimePosture::Refused)
            .count() as u32,
        dependency_edge_count: case_reports
            .iter()
            .map(|case| case.dependency_edges.len() as u32)
            .sum(),
        case_reports,
        claim_boundary: String::from(
            "this runtime report freezes bounded module-link resolution, dependency edges, rollback paths, and refusal posture for the current internal module lane. It does not claim arbitrary install closure or unrestricted software growth inside the model",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Module-link runtime report now freezes {} link cases with {} exact cases, {} rollback cases, {} refused cases, and {} preserved dependency edges.",
        report.case_reports.len(),
        report.exact_case_count,
        report.rollback_case_count,
        report.refused_case_count,
        report.dependency_edge_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_module_link_runtime_report|", &report);
    report
}

/// Returns the canonical absolute path for the committed runtime report.
#[must_use]
pub fn tassadar_module_link_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MODULE_LINK_RUNTIME_REPORT_REF)
}

/// Writes the committed bounded module-link runtime report.
pub fn write_tassadar_module_link_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarModuleLinkRuntimeReport, TassadarModuleLinkRuntimeReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarModuleLinkRuntimeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_module_link_runtime_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarModuleLinkRuntimeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_module_link_runtime_report(
    path: impl AsRef<Path>,
) -> Result<TassadarModuleLinkRuntimeReport, TassadarModuleLinkRuntimeReportError> {
    read_json(path)
}

fn build_case_report(
    case_id: &str,
    consumer_family: &str,
    requested_module_refs: &[&str],
    selected_module_refs: &[&str],
    posture: TassadarModuleLinkRuntimePosture,
    refusal_reason: Option<TassadarModuleLinkRuntimeRefusalReason>,
    rollback_detail: Option<&str>,
    exact_outputs_preserved: bool,
    exact_trace_match: bool,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarModuleLinkRuntimeCaseReport {
    let selected_module_refs = selected_module_refs
        .iter()
        .map(|module_ref| String::from(*module_ref))
        .collect::<Vec<_>>();
    let dependency_edges = build_dependency_edges(selected_module_refs.as_slice());
    TassadarModuleLinkRuntimeCaseReport {
        case_id: String::from(case_id),
        consumer_family: String::from(consumer_family),
        requested_module_refs: requested_module_refs
            .iter()
            .map(|module_ref| String::from(*module_ref))
            .collect(),
        selected_module_refs,
        dependency_graph_digest: if dependency_edges.is_empty() {
            None
        } else {
            Some(stable_digest(
                b"psionic_tassadar_module_link_runtime_case_graph|",
                &dependency_edges,
            ))
        },
        dependency_edges,
        posture,
        refusal_reason,
        rollback_detail: rollback_detail.map(String::from),
        exact_outputs_preserved,
        exact_trace_match,
        benchmark_refs: benchmark_refs
            .iter()
            .map(|reference| String::from(*reference))
            .collect(),
        note: String::from(note),
    }
}

fn build_dependency_edges(selected_module_refs: &[String]) -> Vec<TassadarModuleLinkRuntimeEdge> {
    let manifests = seeded_tassadar_computational_module_manifests()
        .into_iter()
        .filter(|manifest| selected_module_refs.contains(&manifest.module_ref))
        .collect::<Vec<_>>();
    let export_map = manifests
        .iter()
        .flat_map(|manifest| {
            manifest
                .exports
                .iter()
                .map(|export| (export.symbol.as_str(), manifest.module_ref.as_str()))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut edges = manifests
        .iter()
        .flat_map(|manifest| {
            manifest.imports.iter().filter_map(|import| {
                if import.import_class != TassadarModuleImportClass::InternalModuleAbi {
                    return None;
                }
                export_map
                    .get(import.symbol.as_str())
                    .map(|provider_module_ref| TassadarModuleLinkRuntimeEdge {
                        importer_module_ref: manifest.module_ref.clone(),
                        import_symbol: import.symbol.clone(),
                        provider_module_ref: String::from(*provider_module_ref),
                    })
            })
        })
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| {
        (
            left.importer_module_ref.as_str(),
            left.import_symbol.as_str(),
            left.provider_module_ref.as_str(),
        )
            .cmp(&(
                right.importer_module_ref.as_str(),
                right.import_symbol.as_str(),
                right.provider_module_ref.as_str(),
            ))
    });
    edges
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
) -> Result<T, TassadarModuleLinkRuntimeReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarModuleLinkRuntimeReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarModuleLinkRuntimeReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarModuleLinkRuntimePosture, build_tassadar_module_link_runtime_report,
        load_tassadar_module_link_runtime_report, tassadar_module_link_runtime_report_path,
    };

    #[test]
    fn module_link_runtime_report_keeps_dependency_edges_rollback_and_refusal_explicit() {
        let report = build_tassadar_module_link_runtime_report();

        assert_eq!(report.exact_case_count, 1);
        assert_eq!(report.rollback_case_count, 1);
        assert_eq!(report.refused_case_count, 1);
        assert_eq!(report.dependency_edge_count, 1);
        let verifier_case = report
            .case_reports
            .iter()
            .find(|case| case.consumer_family == "verifier_search")
            .expect("verifier case");
        assert_eq!(
            verifier_case.posture,
            TassadarModuleLinkRuntimePosture::Exact
        );
        assert_eq!(verifier_case.dependency_edges.len(), 1);
        assert!(verifier_case.exact_outputs_preserved);
        assert!(verifier_case.exact_trace_match);
        let rollback_case = report
            .case_reports
            .iter()
            .find(|case| case.posture == TassadarModuleLinkRuntimePosture::RolledBack)
            .expect("rollback case");
        assert!(rollback_case.rollback_detail.is_some());
    }

    #[test]
    fn module_link_runtime_report_matches_committed_truth() {
        let expected = build_tassadar_module_link_runtime_report();
        let committed =
            load_tassadar_module_link_runtime_report(tassadar_module_link_runtime_report_path())
                .expect("committed report");

        assert_eq!(committed, expected);
    }
}
