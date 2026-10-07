use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleRuntimeCloseoutReport, TassadarArticleRuntimeCloseoutReportError,
    build_tassadar_article_runtime_closeout_report,
};

pub const TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_runtime_closeout_summary.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleRuntimeCloseoutSummaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub closeout_report: TassadarArticleRuntimeCloseoutReport,
    pub workload_family_ids: Vec<String>,
    pub green_workload_family_ids: Vec<String>,
    pub slowest_workload_horizon_id: String,
    pub slowest_measured_steps_per_second: f64,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleRuntimeCloseoutSummaryReport {
    fn new(closeout_report: TassadarArticleRuntimeCloseoutReport) -> Self {
        let workload_family_ids = closeout_report.bundle.workload_family_ids.clone();
        let green_workload_family_ids = workload_family_ids.clone();
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_runtime_closeout.summary.v1"),
            slowest_workload_horizon_id: closeout_report.slowest_workload_horizon_id.clone(),
            slowest_measured_steps_per_second: closeout_report.slowest_measured_steps_per_second,
            closeout_report,
            workload_family_ids,
            green_workload_family_ids,
            claim_boundary: String::from(
                "this summary keeps the Rust-only article runtime floor as a research and publication-control surface over the committed long-horizon kernels. It does not turn benchmark-only long-horizon support into generic served profile or hardware-portability claims",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Article runtime closeout summary now keeps {} workload families green with stress_anchor=`{}`.",
            report.green_workload_family_ids.len(),
            report.slowest_workload_horizon_id,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_article_runtime_closeout_summary_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleRuntimeCloseoutSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarArticleRuntimeCloseoutReportError),
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

pub fn build_tassadar_article_runtime_closeout_summary_report()
-> Result<TassadarArticleRuntimeCloseoutSummaryReport, TassadarArticleRuntimeCloseoutSummaryError> {
    Ok(TassadarArticleRuntimeCloseoutSummaryReport::new(
        build_tassadar_article_runtime_closeout_report()?,
    ))
}

#[must_use]
pub fn tassadar_article_runtime_closeout_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_runtime_closeout_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleRuntimeCloseoutSummaryReport, TassadarArticleRuntimeCloseoutSummaryError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleRuntimeCloseoutSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_runtime_closeout_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleRuntimeCloseoutSummaryError::Write {
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
        .expect("psionic-research should live under <repo>/crates/psionic-research")
        .to_path_buf()
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
    artifact_kind: &str,
) -> Result<T, TassadarArticleRuntimeCloseoutSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarArticleRuntimeCloseoutSummaryError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleRuntimeCloseoutSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_SUMMARY_REPORT_REF,
        TassadarArticleRuntimeCloseoutSummaryReport,
        build_tassadar_article_runtime_closeout_summary_report, read_repo_json,
        tassadar_article_runtime_closeout_summary_report_path,
        write_tassadar_article_runtime_closeout_summary_report,
    };

    fn normalized_report_value(
        report: &TassadarArticleRuntimeCloseoutSummaryReport,
    ) -> serde_json::Value {
        let mut value = serde_json::to_value(report).expect("summary serializes");
        value["report_digest"] = serde_json::Value::Null;
        value["slowest_measured_steps_per_second"] = serde_json::Value::Null;
        value["closeout_report"]["report_digest"] = serde_json::Value::Null;
        value["closeout_report"]["slowest_measured_steps_per_second"] = serde_json::Value::Null;
        value["closeout_report"]["bundle"]["bundle_digest"] = serde_json::Value::Null;
        for receipt in value["closeout_report"]["bundle"]["horizon_receipts"]
            .as_array_mut()
            .expect("receipts")
        {
            receipt["direct_steps_per_second"] = serde_json::Value::Null;
            receipt["reference_linear"]["steps_per_second"] = serde_json::Value::Null;
        }
        value
    }

    #[test]
    fn article_runtime_closeout_summary_surfaces_green_workload_families() {
        let report = build_tassadar_article_runtime_closeout_summary_report().expect("summary");

        assert_eq!(report.workload_family_ids.len(), 2);
        assert_eq!(report.green_workload_family_ids.len(), 2);
        assert!(
            report
                .green_workload_family_ids
                .contains(&String::from("rust.long_loop_kernel"))
        );
        assert!(
            report
                .green_workload_family_ids
                .contains(&String::from("rust.state_machine_kernel"))
        );
    }

    #[test]
    fn article_runtime_closeout_summary_matches_committed_truth() {
        let generated = build_tassadar_article_runtime_closeout_summary_report().expect("summary");
        let committed: TassadarArticleRuntimeCloseoutSummaryReport = read_repo_json(
            TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_SUMMARY_REPORT_REF,
            "tassadar_article_runtime_closeout_summary_report",
        )
        .expect("committed summary");
        assert_eq!(
            normalized_report_value(&generated),
            normalized_report_value(&committed)
        );
    }

    #[test]
    fn write_article_runtime_closeout_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_runtime_closeout_summary.json");
        let written = write_tassadar_article_runtime_closeout_summary_report(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleRuntimeCloseoutSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(
            normalized_report_value(&written),
            normalized_report_value(&persisted)
        );
        assert_eq!(
            tassadar_article_runtime_closeout_summary_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_runtime_closeout_summary.json")
        );
    }
}
