use std::{collections::BTreeSet, fs, path::Path};

use psionic_eval::{
    build_tassadar_sparse_rule_compiler_audit_case_reports,
    build_tassadar_sparse_rule_compiler_audit_group_reports,
    TassadarSparseRuleCompilerAuditCaseReport, TassadarSparseRuleCompilerAuditEvalError,
    TassadarSparseRuleCompilerAuditGroupReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_SPARSE_RULE_COMPILER_AUDIT_OUTPUT_DIR: &str = "fixtures/tassadar/reports";
pub const TASSADAR_SPARSE_RULE_COMPILER_AUDIT_REPORT_FILE: &str =
    "tassadar_sparse_rule_compiler_audit_report.json";
pub const TASSADAR_SPARSE_RULE_COMPILER_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_sparse_rule_compiler_audit_report.json";
pub const TASSADAR_SPARSE_RULE_COMPILER_AUDIT_EXAMPLE_COMMAND: &str =
    "cargo run -p psionic-research --example tassadar_sparse_rule_compiler_audit";
pub const TASSADAR_SPARSE_RULE_COMPILER_AUDIT_TEST_COMMAND: &str =
    "cargo test -p psionic-research sparse_rule_compiler_audit_report_matches_committed_truth -- --nocapture";

const REPORT_SCHEMA_VERSION: u16 = 1;

/// One committed research report over the sparse-rule compiler-audit lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSparseRuleCompilerAuditReport {
    /// Stable report schema version.
    pub schema_version: u16,
    /// Stable suite identifier.
    pub suite_id: String,
    /// Stable repo-relative report reference.
    pub report_ref: String,
    /// Public regeneration commands for the report.
    pub regeneration_commands: Vec<String>,
    /// Total case count in the report.
    pub total_case_count: u32,
    /// Workload groups covered by the report.
    pub covered_workload_groups: Vec<String>,
    /// Workload groups required by the report.
    pub required_workload_groups: Vec<String>,
    /// Whether the required workload groups are fully covered.
    pub required_group_coverage_complete: bool,
    /// Per-case sparse-rule audit summaries.
    pub case_reports: Vec<TassadarSparseRuleCompilerAuditCaseReport>,
    /// Per-group sparse-rule audit summaries.
    pub group_reports: Vec<TassadarSparseRuleCompilerAuditGroupReport>,
    /// Coarse claim class for the report.
    pub claim_class: String,
    /// Plain-language boundary statement for the report.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarSparseRuleCompilerAuditReport {
    fn new(
        case_reports: Vec<TassadarSparseRuleCompilerAuditCaseReport>,
        group_reports: Vec<TassadarSparseRuleCompilerAuditGroupReport>,
    ) -> Self {
        let covered_workload_groups = case_reports
            .iter()
            .map(|case| case.workload_group_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let required_workload_groups = vec![String::from("kernel"), String::from("scan_style")];
        let required_group_coverage_complete = required_workload_groups
            .iter()
            .all(|group| covered_workload_groups.contains(group));
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            suite_id: String::from("tassadar.sparse_rule_compiler_audit.v1"),
            report_ref: String::from(TASSADAR_SPARSE_RULE_COMPILER_AUDIT_REPORT_REF),
            regeneration_commands: vec![
                String::from(TASSADAR_SPARSE_RULE_COMPILER_AUDIT_EXAMPLE_COMMAND),
                String::from(TASSADAR_SPARSE_RULE_COMPILER_AUDIT_TEST_COMMAND),
            ],
            total_case_count: case_reports.len() as u32,
            covered_workload_groups,
            required_workload_groups,
            required_group_coverage_complete,
            case_reports,
            group_reports,
            claim_class: String::from("compiled_bounded_exactness"),
            claim_boundary: String::from(
                "this report freezes sparse-rule and minimality audits for the bounded symbolic compiler lane only; it proves statement-projected dead-rule and IO-only-underconstraint facts across seeded kernel and scan-style symbolic cases, and does not imply arbitrary Wasm closure, learnability closure, or served capability widening",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_sparse_rule_compiler_audit_report|",
            &report,
        );
        report
    }
}

/// Sparse-rule compiler-audit research report failure.
#[derive(Debug, Error)]
pub enum TassadarSparseRuleCompilerAuditReportError {
    #[error(transparent)]
    Eval(#[from] TassadarSparseRuleCompilerAuditEvalError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

/// Builds the committed sparse-rule compiler-audit report.
pub fn build_tassadar_sparse_rule_compiler_audit_report(
) -> Result<TassadarSparseRuleCompilerAuditReport, TassadarSparseRuleCompilerAuditReportError> {
    let case_reports = build_tassadar_sparse_rule_compiler_audit_case_reports()?;
    let group_reports =
        build_tassadar_sparse_rule_compiler_audit_group_reports(case_reports.as_slice());
    Ok(TassadarSparseRuleCompilerAuditReport::new(
        case_reports,
        group_reports,
    ))
}

/// Writes the committed sparse-rule compiler-audit report under the supplied
/// output directory.
pub fn run_tassadar_sparse_rule_compiler_audit(
    output_dir: &Path,
) -> Result<TassadarSparseRuleCompilerAuditReport, TassadarSparseRuleCompilerAuditReportError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarSparseRuleCompilerAuditReportError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let report = build_tassadar_sparse_rule_compiler_audit_report()?;
    let report_path = output_dir.join(TASSADAR_SPARSE_RULE_COMPILER_AUDIT_REPORT_FILE);
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(&report_path, bytes).map_err(|error| {
        TassadarSparseRuleCompilerAuditReportError::Write {
            path: report_path.display().to_string(),
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde::de::DeserializeOwned;
    use tempfile::tempdir;

    use super::{
        build_tassadar_sparse_rule_compiler_audit_report, run_tassadar_sparse_rule_compiler_audit,
        TassadarSparseRuleCompilerAuditReport, TASSADAR_SPARSE_RULE_COMPILER_AUDIT_REPORT_REF,
    };

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    fn read_repo_json<T>(repo_relative_path: &str) -> Result<T, Box<dyn std::error::Error>>
    where
        T: DeserializeOwned,
    {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(&path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn sparse_rule_compiler_audit_report_covers_required_groups(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_sparse_rule_compiler_audit_report()?;
        assert!(report.required_group_coverage_complete);
        assert_eq!(
            report.covered_workload_groups,
            vec![String::from("kernel"), String::from("scan_style")]
        );
        Ok(())
    }

    #[test]
    fn sparse_rule_compiler_audit_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_sparse_rule_compiler_audit_report()?;
        let persisted: TassadarSparseRuleCompilerAuditReport =
            read_repo_json(TASSADAR_SPARSE_RULE_COMPILER_AUDIT_REPORT_REF)?;
        assert_eq!(persisted, report);
        Ok(())
    }

    #[test]
    fn sparse_rule_compiler_audit_report_writes_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let report = run_tassadar_sparse_rule_compiler_audit(temp_dir.path())?;
        let persisted: TassadarSparseRuleCompilerAuditReport =
            serde_json::from_slice(&std::fs::read(
                temp_dir
                    .path()
                    .join("tassadar_sparse_rule_compiler_audit_report.json"),
            )?)?;
        assert_eq!(persisted, report);
        Ok(())
    }
}
