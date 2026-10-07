use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_INTERNAL_MODULE_LIBRARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_module_library_report.json";

/// One active module entry inside the runtime report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalModuleRuntimeEntry {
    /// Stable module identifier without version.
    pub module_id: String,
    /// Stable active version.
    pub active_version: String,
    /// Candidate version under evaluation when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_version: Option<String>,
    /// Rollback version available for explicit fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_version: Option<String>,
    /// Stable compatibility digest for the active version.
    pub compatibility_digest: String,
    /// Stable benchmark refs anchoring the active module.
    pub benchmark_refs: Vec<String>,
    /// Consumer families currently reusing the module.
    pub reusable_consumer_families: Vec<String>,
}

/// Link outcome for one consumer family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalModuleLinkPosture {
    /// The requested module set linked directly.
    ExactReuse,
    /// A candidate module drifted and the runtime linked the rollback target instead.
    RolledBack,
    /// The requested library surface was refused explicitly.
    Refused,
}

/// Stable refusal reason for one consumer family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalModuleRefusalReason {
    /// The requested module import was outside the published bounded library.
    ImportOutsideBoundedLibrary,
    /// The requested link omitted one required compatibility contract.
    MissingCompatibilityContract,
}

/// One link report for a consumer family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalModuleLinkCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable consumer program family.
    pub consumer_program_family: String,
    /// Ordered requested module refs.
    pub requested_module_refs: Vec<String>,
    /// Ordered selected module refs.
    pub selected_module_refs: Vec<String>,
    /// Final link posture.
    pub link_posture: TassadarInternalModuleLinkPosture,
    /// Refusal reason when the link was refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarInternalModuleRefusalReason>,
    /// Rollback detail when the selected modules differ from the requested candidate set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_detail: Option<String>,
    /// Whether outputs stayed exact on the link witness.
    pub exact_outputs_preserved: bool,
    /// Whether traces stayed exact on the link witness.
    pub exact_trace_match: bool,
    /// Stable benchmark refs anchoring the case.
    pub benchmark_refs: Vec<String>,
    /// Plain-language case note.
    pub note: String,
}

/// Runtime report for the internal module library lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalModuleLibraryReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable library identifier.
    pub library_id: String,
    /// Active runtime module entries.
    pub active_modules: Vec<TassadarInternalModuleRuntimeEntry>,
    /// Count of exact-reuse cases.
    pub exact_reuse_case_count: u32,
    /// Count of rollback cases.
    pub rollback_case_count: u32,
    /// Count of refused cases.
    pub refused_case_count: u32,
    /// Module ids reused by more than one consumer family.
    pub cross_program_reused_module_ids: Vec<String>,
    /// Per-consumer link reports.
    pub case_reports: Vec<TassadarInternalModuleLinkCaseReport>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Runtime failure while persisting or validating the module library report.
#[derive(Debug, Error)]
pub enum TassadarInternalModuleLibraryReportError {
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

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_internal_module_library_report_path() -> PathBuf {
    repo_root().join(TASSADAR_INTERNAL_MODULE_LIBRARY_REPORT_REF)
}

/// Builds the machine-readable internal module library report.
#[must_use]
pub fn build_tassadar_internal_module_library_report() -> TassadarInternalModuleLibraryReport {
    let active_modules = vec![
        runtime_entry(
            "frontier_relax_core",
            "1.0.0",
            None,
            None,
            "compat.frontier_relax_core.1_0_0",
            &[
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
                "fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json",
            ],
            &["clrs_shortest_path", "clrs_wasm_shortest_path"],
        ),
        runtime_entry(
            "candidate_select_core",
            "1.1.0",
            Some("1.2.0"),
            Some("1.1.0"),
            "compat.candidate_select_core.1_1_0",
            &[
                "fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json",
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
            ],
            &["hungarian_matching", "verifier_search"],
        ),
        runtime_entry(
            "checkpoint_backtrack_core",
            "1.0.0",
            None,
            None,
            "compat.checkpoint_backtrack_core.1_0_0",
            &["fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"],
            &["verifier_search"],
        ),
    ];
    let case_reports = vec![
        case_report(
            "clrs_shortest_path_reference",
            "clrs_shortest_path",
            &["frontier_relax_core@1.0.0"],
            &["frontier_relax_core@1.0.0"],
            TassadarInternalModuleLinkPosture::ExactReuse,
            None,
            None,
            true,
            true,
            &["fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"],
            "The CLRS shortest-path family reuses frontier_relax_core directly under the published compatibility digest.",
        ),
        case_report(
            "clrs_wasm_shortest_path_reference",
            "clrs_wasm_shortest_path",
            &["frontier_relax_core@1.0.0"],
            &["frontier_relax_core@1.0.0"],
            TassadarInternalModuleLinkPosture::ExactReuse,
            None,
            None,
            true,
            true,
            &["fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"],
            "The CLRS-to-Wasm bridge reuses the same frontier_relax_core module instead of compiling a separate one-off relaxer.",
        ),
        case_report(
            "verifier_search_reference",
            "verifier_search",
            &[
                "candidate_select_core@1.1.0",
                "checkpoint_backtrack_core@1.0.0",
            ],
            &[
                "candidate_select_core@1.1.0",
                "checkpoint_backtrack_core@1.0.0",
            ],
            TassadarInternalModuleLinkPosture::ExactReuse,
            None,
            None,
            true,
            true,
            &["fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"],
            "Verifier-guided search reuses the candidate selector and checkpoint core directly under the bounded library lane.",
        ),
        case_report(
            "hungarian_matching_reference",
            "hungarian_matching",
            &["candidate_select_core@1.2.0"],
            &["candidate_select_core@1.1.0"],
            TassadarInternalModuleLinkPosture::RolledBack,
            None,
            Some(
                "candidate_select_core@1.2.0 drifted on assignment-stability witnesses, so the runtime linked the explicit rollback target candidate_select_core@1.1.0"
                    .to_string(),
            ),
            true,
            true,
            &["fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"],
            "Hungarian matching keeps replacement posture explicit by rolling back from the candidate selector version under the published rollback plan.",
        ),
        case_report(
            "sudoku_search_reference",
            "sudoku_search",
            &[
                "checkpoint_backtrack_core@1.0.0",
                "branch_prune_core@0.1.0",
            ],
            &["checkpoint_backtrack_core@1.0.0"],
            TassadarInternalModuleLinkPosture::Refused,
            Some(TassadarInternalModuleRefusalReason::ImportOutsideBoundedLibrary),
            None,
            false,
            false,
            &["fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"],
            "Sudoku search refuses because branch_prune_core is not part of the published bounded library surface yet.",
        ),
    ];
    let mut report = TassadarInternalModuleLibraryReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.internal_module_library.report.v1"),
        library_id: String::from("tassadar.internal_module_library.v1"),
        active_modules,
        exact_reuse_case_count: case_reports
            .iter()
            .filter(|case| case.link_posture == TassadarInternalModuleLinkPosture::ExactReuse)
            .count() as u32,
        rollback_case_count: case_reports
            .iter()
            .filter(|case| case.link_posture == TassadarInternalModuleLinkPosture::RolledBack)
            .count() as u32,
        refused_case_count: case_reports
            .iter()
            .filter(|case| case.link_posture == TassadarInternalModuleLinkPosture::Refused)
            .count() as u32,
        cross_program_reused_module_ids: vec![
            String::from("frontier_relax_core"),
            String::from("candidate_select_core"),
        ],
        case_reports,
        claim_boundary: String::from(
            "this runtime report keeps internal module reuse bounded to the published library, compatibility digests, and rollback plans. Exact reuse, rollback, and refusal stay explicit; none of that implies arbitrary module installation or unrestricted software growth inside the model",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Internal module library report now freezes {} active modules across {} link cases, with {} exact-reuse cases, {} rollback cases, and {} refused cases.",
        report.active_modules.len(),
        report.case_reports.len(),
        report.exact_reuse_case_count,
        report.rollback_case_count,
        report.refused_case_count,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_internal_module_library_report|", &report);
    report
}

/// Writes the committed internal module library report.
pub fn write_tassadar_internal_module_library_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarInternalModuleLibraryReport, TassadarInternalModuleLibraryReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInternalModuleLibraryReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_internal_module_library_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInternalModuleLibraryReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn runtime_entry(
    module_id: &str,
    active_version: &str,
    candidate_version: Option<&str>,
    rollback_version: Option<&str>,
    compatibility_digest: &str,
    benchmark_refs: &[&str],
    reusable_consumer_families: &[&str],
) -> TassadarInternalModuleRuntimeEntry {
    TassadarInternalModuleRuntimeEntry {
        module_id: String::from(module_id),
        active_version: String::from(active_version),
        candidate_version: candidate_version.map(String::from),
        rollback_version: rollback_version.map(String::from),
        compatibility_digest: String::from(compatibility_digest),
        benchmark_refs: benchmark_refs
            .iter()
            .map(|reference| String::from(*reference))
            .collect(),
        reusable_consumer_families: reusable_consumer_families
            .iter()
            .map(|family| String::from(*family))
            .collect(),
    }
}

fn case_report(
    case_id: &str,
    consumer_program_family: &str,
    requested_module_refs: &[&str],
    selected_module_refs: &[&str],
    link_posture: TassadarInternalModuleLinkPosture,
    refusal_reason: Option<TassadarInternalModuleRefusalReason>,
    rollback_detail: Option<String>,
    exact_outputs_preserved: bool,
    exact_trace_match: bool,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarInternalModuleLinkCaseReport {
    TassadarInternalModuleLinkCaseReport {
        case_id: String::from(case_id),
        consumer_program_family: String::from(consumer_program_family),
        requested_module_refs: requested_module_refs
            .iter()
            .map(|reference| String::from(*reference))
            .collect(),
        selected_module_refs: selected_module_refs
            .iter()
            .map(|reference| String::from(*reference))
            .collect(),
        link_posture,
        refusal_reason,
        rollback_detail,
        exact_outputs_preserved,
        exact_trace_match,
        benchmark_refs: benchmark_refs
            .iter()
            .map(|reference| String::from(*reference))
            .collect(),
        note: String::from(note),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
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
) -> Result<T, TassadarInternalModuleLibraryReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarInternalModuleLibraryReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarInternalModuleLibraryReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_INTERNAL_MODULE_LIBRARY_REPORT_REF, TassadarInternalModuleLibraryReport,
        TassadarInternalModuleLinkPosture, build_tassadar_internal_module_library_report,
        read_repo_json, tassadar_internal_module_library_report_path,
        write_tassadar_internal_module_library_report,
    };

    #[test]
    fn internal_module_library_report_keeps_reuse_rollback_and_refusal_explicit() {
        let report = build_tassadar_internal_module_library_report();

        assert!(
            report
                .cross_program_reused_module_ids
                .contains(&String::from("frontier_relax_core"))
        );
        assert!(report.case_reports.iter().any(|case| {
            case.consumer_program_family == "hungarian_matching"
                && case.link_posture == TassadarInternalModuleLinkPosture::RolledBack
                && case.rollback_detail.is_some()
        }));
        assert!(report.case_reports.iter().any(|case| {
            case.consumer_program_family == "sudoku_search"
                && case.link_posture == TassadarInternalModuleLinkPosture::Refused
        }));
    }

    #[test]
    fn internal_module_library_report_matches_committed_truth() {
        let generated = build_tassadar_internal_module_library_report();
        let committed: TassadarInternalModuleLibraryReport =
            read_repo_json(TASSADAR_INTERNAL_MODULE_LIBRARY_REPORT_REF).expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_internal_module_library_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join(format!(
            "tassadar_internal_module_library_report-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ));
        let written =
            write_tassadar_internal_module_library_report(&output_path).expect("write report");
        let persisted: TassadarInternalModuleLibraryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        let _ = std::fs::remove_file(&output_path);
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_internal_module_library_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_internal_module_library_report.json")
        );
    }
}
