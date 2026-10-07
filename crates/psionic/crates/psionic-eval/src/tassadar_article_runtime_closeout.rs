use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_ROOT_REF, TassadarArticleRuntimeCloseoutBundle,
    TassadarArticleRuntimeCloseoutError, build_tassadar_article_runtime_closeout_bundle,
    write_tassadar_article_runtime_closeout_bundle,
};

pub const TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_runtime_closeout_report.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleRuntimeCloseoutReport {
    pub schema_version: u16,
    pub report_id: String,
    pub bundle_ref: String,
    pub bundle: TassadarArticleRuntimeCloseoutBundle,
    pub exact_horizon_count: u32,
    pub floor_pass_count: u32,
    pub floor_refusal_count: u32,
    pub slowest_workload_horizon_id: String,
    pub slowest_measured_steps_per_second: f64,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleRuntimeCloseoutReport {
    fn new(bundle: TassadarArticleRuntimeCloseoutBundle) -> Self {
        let slowest_receipt = bundle
            .horizon_receipts
            .iter()
            .max_by(|left, right| {
                left.exact_step_count
                    .cmp(&right.exact_step_count)
                    .then_with(|| left.horizon_id.cmp(&right.horizon_id))
            })
            .expect("article runtime closeout should contain at least one receipt");
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_runtime_closeout.report.v1"),
            bundle_ref: format!(
                "{}/{}",
                TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_ROOT_REF,
                psionic_runtime::TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_BUNDLE_FILE
            ),
            exact_horizon_count: bundle.exact_horizon_count,
            floor_pass_count: bundle.floor_pass_count,
            floor_refusal_count: bundle.floor_refusal_count,
            slowest_workload_horizon_id: slowest_receipt.horizon_id.clone(),
            slowest_measured_steps_per_second: slowest_receipt.direct_steps_per_second,
            bundle,
            claim_boundary: String::from(
                "this eval report summarizes the Rust-only runtime floor only for the committed article-closeout kernel families and horizons. It keeps reference-linear as the baseline stress anchor, now includes direct measured HullCache companion rows on the bounded long-horizon kernel set, and still does not widen generic served profile support beyond those committed workloads",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Article runtime closeout report now binds {} exact horizon receipts with floor_passes={}, floor_refusals={}, and stress_anchor=`{}`.",
            report.exact_horizon_count,
            report.floor_pass_count,
            report.floor_refusal_count,
            report.slowest_workload_horizon_id,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_article_runtime_closeout_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleRuntimeCloseoutReportError {
    #[error(transparent)]
    Runtime(#[from] TassadarArticleRuntimeCloseoutError),
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

pub fn build_tassadar_article_runtime_closeout_report()
-> Result<TassadarArticleRuntimeCloseoutReport, TassadarArticleRuntimeCloseoutReportError> {
    Ok(TassadarArticleRuntimeCloseoutReport::new(
        build_tassadar_article_runtime_closeout_bundle()?,
    ))
}

#[must_use]
pub fn tassadar_article_runtime_closeout_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_REPORT_REF)
}

pub fn write_tassadar_article_runtime_closeout_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleRuntimeCloseoutReport, TassadarArticleRuntimeCloseoutReportError> {
    write_tassadar_article_runtime_closeout_bundle(
        repo_root().join(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_ROOT_REF),
    )?;
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleRuntimeCloseoutReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_runtime_closeout_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleRuntimeCloseoutReportError::Write {
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
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
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
) -> Result<T, TassadarArticleRuntimeCloseoutReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarArticleRuntimeCloseoutReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleRuntimeCloseoutReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_REPORT_REF, TassadarArticleRuntimeCloseoutReport,
        build_tassadar_article_runtime_closeout_report, read_repo_json,
        tassadar_article_runtime_closeout_report_path,
        write_tassadar_article_runtime_closeout_report,
    };
    use psionic_runtime::TassadarArticleRuntimeFloorStatus;

    fn normalized_report_value(report: &TassadarArticleRuntimeCloseoutReport) -> serde_json::Value {
        let mut value = serde_json::to_value(report).expect("report serializes");
        value["report_digest"] = serde_json::Value::Null;
        value["slowest_measured_steps_per_second"] = serde_json::Value::Null;
        for receipt in value["bundle"]["horizon_receipts"]
            .as_array_mut()
            .expect("receipts")
        {
            receipt["direct_steps_per_second"] = serde_json::Value::Null;
            receipt["reference_linear"]["steps_per_second"] = serde_json::Value::Null;
            receipt["hull_cache"]["steps_per_second"] = serde_json::Value::Null;
        }
        value["bundle"]["bundle_digest"] = serde_json::Value::Null;
        value
    }

    #[test]
    fn article_runtime_closeout_report_summarizes_runtime_truth() {
        let report = build_tassadar_article_runtime_closeout_report().expect("report");

        assert_eq!(
            report.bundle_ref,
            "fixtures/tassadar/runs/article_runtime_closeout_v1/article_runtime_closeout_bundle.json"
        );
        assert_eq!(report.exact_horizon_count, 4);
        assert_eq!(report.floor_pass_count, 4);
        assert_eq!(report.floor_refusal_count, 0);
        assert_eq!(
            report
                .bundle
                .horizon_receipts
                .iter()
                .filter(|receipt| receipt.floor_status == TassadarArticleRuntimeFloorStatus::Passed)
                .count(),
            4
        );
    }

    #[test]
    fn article_runtime_closeout_report_matches_committed_truth() {
        let generated = build_tassadar_article_runtime_closeout_report().expect("report");
        let committed: TassadarArticleRuntimeCloseoutReport = read_repo_json(
            TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_REPORT_REF,
            "tassadar_article_runtime_closeout_report",
        )
        .expect("committed report");
        assert_eq!(
            normalized_report_value(&generated),
            normalized_report_value(&committed)
        );
    }

    #[test]
    fn write_article_runtime_closeout_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_runtime_closeout_report.json");
        let written =
            write_tassadar_article_runtime_closeout_report(&output_path).expect("write report");
        let persisted: TassadarArticleRuntimeCloseoutReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(
            normalized_report_value(&written),
            normalized_report_value(&persisted)
        );
        assert_eq!(
            tassadar_article_runtime_closeout_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_runtime_closeout_report.json")
        );
    }
}
