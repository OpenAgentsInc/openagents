use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    TassadarLinkedProgramBundleEvalReport, TASSADAR_LINKED_PROGRAM_BUNDLE_EVAL_REPORT_REF,
};
use psionic_runtime::TassadarLinkedProgramBundlePosture;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_LINKED_PROGRAM_BUNDLE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_linked_program_bundle_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinkedProgramBundleSummaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub eval_report: TassadarLinkedProgramBundleEvalReport,
    pub exact_bundle_ids: Vec<String>,
    pub rollback_bundle_ids: Vec<String>,
    pub refused_bundle_ids: Vec<String>,
    pub shared_state_bundle_ids: Vec<String>,
    pub start_safe_bundle_ids: Vec<String>,
    pub graph_valid_bundle_ids: Vec<String>,
    pub runtime_support_classes: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarLinkedProgramBundleSummaryError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_linked_program_bundle_summary_report(
) -> Result<TassadarLinkedProgramBundleSummaryReport, TassadarLinkedProgramBundleSummaryError> {
    let eval_report: TassadarLinkedProgramBundleEvalReport =
        read_repo_json(TASSADAR_LINKED_PROGRAM_BUNDLE_EVAL_REPORT_REF)?;
    let exact_bundle_ids = eval_report
        .runtime_report
        .case_reports
        .iter()
        .filter(|case| case.posture == TassadarLinkedProgramBundlePosture::Exact)
        .map(|case| case.bundle_descriptor.bundle_id.clone())
        .collect::<Vec<_>>();
    let rollback_bundle_ids = eval_report
        .runtime_report
        .case_reports
        .iter()
        .filter(|case| case.posture == TassadarLinkedProgramBundlePosture::RolledBack)
        .map(|case| case.bundle_descriptor.bundle_id.clone())
        .collect::<Vec<_>>();
    let refused_bundle_ids = eval_report
        .runtime_report
        .case_reports
        .iter()
        .filter(|case| case.posture == TassadarLinkedProgramBundlePosture::Refused)
        .map(|case| case.bundle_descriptor.bundle_id.clone())
        .collect::<Vec<_>>();
    let shared_state_bundle_ids = eval_report
        .runtime_report
        .case_reports
        .iter()
        .filter(|case| !case.shared_state_module_refs.is_empty())
        .map(|case| case.bundle_descriptor.bundle_id.clone())
        .collect::<Vec<_>>();
    let start_safe_bundle_ids = eval_report
        .runtime_report
        .case_reports
        .iter()
        .filter(|case| case.start_order_replay_exact)
        .map(|case| case.bundle_descriptor.bundle_id.clone())
        .collect::<Vec<_>>();
    let graph_valid_bundle_ids = eval_report
        .runtime_report
        .case_reports
        .iter()
        .filter(|case| case.graph_shape_valid)
        .map(|case| case.bundle_descriptor.bundle_id.clone())
        .collect::<Vec<_>>();
    let runtime_support_classes = eval_report
        .runtime_support_classes
        .iter()
        .map(|class| String::from(class.as_str()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut report = TassadarLinkedProgramBundleSummaryReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.linked_program_bundle.summary.v1"),
        eval_report,
        exact_bundle_ids,
        rollback_bundle_ids,
        refused_bundle_ids,
        shared_state_bundle_ids,
        start_safe_bundle_ids,
        graph_valid_bundle_ids,
        runtime_support_classes,
        claim_boundary: String::from(
            "this summary keeps bounded linked-program bundle winners, rollback paths, refused shapes, shared-state posture, and runtime-support classes explicit. It does not turn bundle reuse into arbitrary program growth or unrestricted helper installation claims",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Linked-program bundle summary marks {} exact bundles, {} rollback bundles, {} refused bundles, {} shared-state bundles, {} start-safe bundles, {} graph-valid bundles, and {} runtime-support classes.",
        report.exact_bundle_ids.len(),
        report.rollback_bundle_ids.len(),
        report.refused_bundle_ids.len(),
        report.shared_state_bundle_ids.len(),
        report.start_safe_bundle_ids.len(),
        report.graph_valid_bundle_ids.len(),
        report.runtime_support_classes.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_linked_program_bundle_summary_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_linked_program_bundle_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_LINKED_PROGRAM_BUNDLE_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_linked_program_bundle_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarLinkedProgramBundleSummaryReport, TassadarLinkedProgramBundleSummaryError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarLinkedProgramBundleSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_linked_program_bundle_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarLinkedProgramBundleSummaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarLinkedProgramBundleSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarLinkedProgramBundleSummaryError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarLinkedProgramBundleSummaryError::Deserialize {
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
        build_tassadar_linked_program_bundle_summary_report, read_repo_json,
        tassadar_linked_program_bundle_summary_report_path,
        write_tassadar_linked_program_bundle_summary_report,
        TassadarLinkedProgramBundleSummaryReport,
        TASSADAR_LINKED_PROGRAM_BUNDLE_SUMMARY_REPORT_REF,
    };

    #[test]
    fn linked_program_bundle_summary_marks_exact_rollback_refused_and_shared_state_bundles() {
        let report = build_tassadar_linked_program_bundle_summary_report().expect("summary");
        assert!(report.exact_bundle_ids.contains(&String::from(
            "tassadar.linked_program_bundle.vm_checksum_parser.v1"
        )));
        assert!(report.rollback_bundle_ids.contains(&String::from(
            "tassadar.linked_program_bundle.parser_allocator_rollback.v1"
        )));
        assert!(report.refused_bundle_ids.contains(&String::from(
            "tassadar.linked_program_bundle.shared_state_gap.v1"
        )));
        assert!(report
            .runtime_support_classes
            .contains(&String::from("checkpoint_backtrack")));
        assert!(report.start_safe_bundle_ids.contains(&String::from(
            "tassadar.linked_program_bundle.checkpoint_backtrack.v1"
        )));
        assert!(report.graph_valid_bundle_ids.contains(&String::from(
            "tassadar.linked_program_bundle.vm_checksum_parser.v1"
        )));
    }

    #[test]
    fn linked_program_bundle_summary_matches_committed_truth() {
        let generated = build_tassadar_linked_program_bundle_summary_report().expect("summary");
        let committed: TassadarLinkedProgramBundleSummaryReport =
            read_repo_json(TASSADAR_LINKED_PROGRAM_BUNDLE_SUMMARY_REPORT_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_linked_program_bundle_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_linked_program_bundle_summary.json");
        let written =
            write_tassadar_linked_program_bundle_summary_report(&output_path).expect("write");
        let persisted: TassadarLinkedProgramBundleSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_linked_program_bundle_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_linked_program_bundle_summary.json")
        );
    }
}
