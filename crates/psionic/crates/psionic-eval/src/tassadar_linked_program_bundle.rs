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
    tassadar_linked_program_bundle_compiler_summaries, TassadarLinkedProgramBundleCompilerSummary,
};
use psionic_router::{
    build_tassadar_linked_program_bundle_route_report, TassadarLinkedProgramBundleRouteKind,
    TASSADAR_LINKED_PROGRAM_BUNDLE_ROUTE_REPORT_REF,
};
use psionic_runtime::{
    build_tassadar_linked_program_bundle_runtime_report, TassadarLinkedProgramBundleRuntimeReport,
    TassadarRuntimeSupportModuleClass, TASSADAR_LINKED_PROGRAM_BUNDLE_RUNTIME_REPORT_REF,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_LINKED_PROGRAM_BUNDLE_EVAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_linked_program_bundle_eval_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinkedProgramBundleEvalReport {
    pub schema_version: u16,
    pub report_id: String,
    pub runtime_report: TassadarLinkedProgramBundleRuntimeReport,
    pub route_report_ref: String,
    pub compiler_summaries: Vec<TassadarLinkedProgramBundleCompilerSummary>,
    pub exact_case_count: u32,
    pub rollback_case_count: u32,
    pub refused_case_count: u32,
    pub internal_exact_route_count: u32,
    pub shared_state_route_count: u32,
    pub rollback_route_count: u32,
    pub refused_route_count: u32,
    pub module_local_only_case_count: u32,
    pub shared_state_case_count: u32,
    pub benchmark_lineage_complete_case_count: u32,
    pub helper_lineage_complete_case_count: u32,
    pub graph_valid_case_count: u32,
    pub start_order_exact_case_count: u32,
    pub runtime_support_classes: Vec<TassadarRuntimeSupportModuleClass>,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarLinkedProgramBundleEvalReportError {
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

#[must_use]
pub fn build_tassadar_linked_program_bundle_eval_report() -> TassadarLinkedProgramBundleEvalReport {
    let runtime_report = build_tassadar_linked_program_bundle_runtime_report();
    let route_report = build_tassadar_linked_program_bundle_route_report();
    let compiler_summaries = tassadar_linked_program_bundle_compiler_summaries();
    let mut runtime_support_classes = runtime_report
        .case_reports
        .iter()
        .flat_map(|case| case.runtime_support_classes.iter().copied())
        .collect::<Vec<_>>();
    runtime_support_classes.sort_by_key(|class| class.as_str());
    runtime_support_classes.dedup();
    let module_local_only_case_count = runtime_report
        .case_reports
        .iter()
        .filter(|case| case.shared_state_module_refs.is_empty())
        .count() as u32;
    let mut generated_from_refs = vec![
        String::from(TASSADAR_LINKED_PROGRAM_BUNDLE_RUNTIME_REPORT_REF),
        String::from(TASSADAR_LINKED_PROGRAM_BUNDLE_ROUTE_REPORT_REF),
    ];
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let mut report = TassadarLinkedProgramBundleEvalReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.linked_program_bundle_eval.report.v1"),
        exact_case_count: runtime_report.exact_case_count,
        rollback_case_count: runtime_report.rollback_case_count,
        refused_case_count: runtime_report.refused_case_count,
        internal_exact_route_count: route_report
            .rows
            .iter()
            .filter(|row| row.route_kind == TassadarLinkedProgramBundleRouteKind::InternalExact)
            .count() as u32,
        shared_state_route_count: route_report
            .rows
            .iter()
            .filter(|row| {
                row.route_kind == TassadarLinkedProgramBundleRouteKind::SharedStateReceiptBound
            })
            .count() as u32,
        rollback_route_count: route_report
            .rows
            .iter()
            .filter(|row| row.route_kind == TassadarLinkedProgramBundleRouteKind::RollbackPinnedHelper)
            .count() as u32,
        refused_route_count: route_report
            .rows
            .iter()
            .filter(|row| row.route_kind == TassadarLinkedProgramBundleRouteKind::Refused)
            .count() as u32,
        module_local_only_case_count,
        shared_state_case_count: runtime_report.shared_state_case_count,
        benchmark_lineage_complete_case_count: runtime_report
            .benchmark_lineage_complete_case_count,
        helper_lineage_complete_case_count: runtime_report.helper_lineage_complete_case_count,
        graph_valid_case_count: runtime_report.graph_valid_case_count,
        start_order_exact_case_count: runtime_report.start_order_exact_case_count,
        runtime_support_classes,
        generated_from_refs,
        runtime_report,
        route_report_ref: String::from(TASSADAR_LINKED_PROGRAM_BUNDLE_ROUTE_REPORT_REF),
        compiler_summaries,
        claim_boundary: String::from(
            "this eval report freezes bounded linked-program bundle truth across runtime descriptors, compiler summaries, and router route posture. It widens module composition only through explicit helper, state, lineage, rollback, and refusal facts rather than implying arbitrary linked software closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Linked-program bundle eval report covers exact={}, rollback={}, refused={}, module_local_only={}, shared_state={}, lineage_complete={}, helper_lineage_complete={}, graph_valid={}, start_order_exact={}, and {} runtime-support classes.",
        report.exact_case_count,
        report.rollback_case_count,
        report.refused_case_count,
        report.module_local_only_case_count,
        report.shared_state_case_count,
        report.benchmark_lineage_complete_case_count,
        report.helper_lineage_complete_case_count,
        report.graph_valid_case_count,
        report.start_order_exact_case_count,
        report.runtime_support_classes.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_linked_program_bundle_eval_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_linked_program_bundle_eval_report_path() -> PathBuf {
    repo_root().join(TASSADAR_LINKED_PROGRAM_BUNDLE_EVAL_REPORT_REF)
}

pub fn write_tassadar_linked_program_bundle_eval_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarLinkedProgramBundleEvalReport, TassadarLinkedProgramBundleEvalReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarLinkedProgramBundleEvalReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_linked_program_bundle_eval_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarLinkedProgramBundleEvalReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_linked_program_bundle_eval_report(
    path: impl AsRef<Path>,
) -> Result<TassadarLinkedProgramBundleEvalReport, TassadarLinkedProgramBundleEvalReportError> {
    read_json(path)
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
) -> Result<T, TassadarLinkedProgramBundleEvalReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarLinkedProgramBundleEvalReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarLinkedProgramBundleEvalReportError::Deserialize {
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
        build_tassadar_linked_program_bundle_eval_report,
        load_tassadar_linked_program_bundle_eval_report,
        tassadar_linked_program_bundle_eval_report_path,
    };
    use psionic_runtime::TassadarRuntimeSupportModuleClass;

    #[test]
    fn linked_program_bundle_eval_report_tracks_runtime_route_and_compiler_truth() {
        let report = build_tassadar_linked_program_bundle_eval_report();
        assert_eq!(report.exact_case_count, 2);
        assert_eq!(report.rollback_case_count, 1);
        assert_eq!(report.refused_case_count, 1);
        assert_eq!(report.internal_exact_route_count, 1);
        assert_eq!(report.shared_state_route_count, 1);
        assert_eq!(report.rollback_route_count, 1);
        assert_eq!(report.refused_route_count, 1);
        assert_eq!(report.helper_lineage_complete_case_count, 3);
        assert_eq!(report.graph_valid_case_count, 3);
        assert_eq!(report.start_order_exact_case_count, 3);
        assert_eq!(report.compiler_summaries.len(), 4);
        assert!(report
            .runtime_support_classes
            .contains(&TassadarRuntimeSupportModuleClass::CheckpointBacktrack));
    }

    #[test]
    fn linked_program_bundle_eval_report_matches_committed_truth() {
        let expected = build_tassadar_linked_program_bundle_eval_report();
        let committed = load_tassadar_linked_program_bundle_eval_report(
            tassadar_linked_program_bundle_eval_report_path(),
        )
        .expect("committed report");
        assert_eq!(committed, expected);
    }
}
