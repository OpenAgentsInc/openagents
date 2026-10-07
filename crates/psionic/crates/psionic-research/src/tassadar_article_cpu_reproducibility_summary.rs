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
    TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF, TassadarArticleCpuReproducibilityReport,
    TassadarArticleCpuReproducibilityReportError, build_tassadar_article_cpu_reproducibility_report,
};
use psionic_runtime::TassadarArticleCpuMachineClassStatus;

pub const TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_cpu_reproducibility_summary.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleCpuReproducibilitySummaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub portability_report_ref: String,
    pub portability_report: TassadarArticleCpuReproducibilityReport,
    pub measured_green_machine_class_ids: Vec<String>,
    pub declared_supported_machine_class_ids: Vec<String>,
    pub unsupported_machine_class_ids: Vec<String>,
    pub optional_c_path_blocks_rust_only_claim: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleCpuReproducibilitySummaryReport {
    fn new(portability_report: TassadarArticleCpuReproducibilityReport) -> Self {
        let measured_green_machine_class_ids = portability_report
            .matrix
            .rows
            .iter()
            .filter(|row| {
                row.status == TassadarArticleCpuMachineClassStatus::SupportedMeasuredCurrentHost
            })
            .map(|row| row.machine_class_id.clone())
            .collect::<Vec<_>>();
        let declared_supported_machine_class_ids = portability_report
            .matrix
            .rows
            .iter()
            .filter(|row| row.status == TassadarArticleCpuMachineClassStatus::SupportedDeclaredClass)
            .map(|row| row.machine_class_id.clone())
            .collect::<Vec<_>>();
        let unsupported_machine_class_ids = portability_report
            .unsupported_machine_class_ids
            .clone();
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_cpu_reproducibility.summary.v1"),
            portability_report_ref: String::from(TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF),
            optional_c_path_blocks_rust_only_claim: portability_report
                .optional_c_path_blocks_rust_only_claim,
            portability_report,
            measured_green_machine_class_ids,
            declared_supported_machine_class_ids,
            unsupported_machine_class_ids,
            claim_boundary: String::from(
                "this summary turns the Rust-only article CPU reproducibility matrix into operator-facing support language only for the declared CPU classes and the current host. It keeps unsupported classes and the optional non-blocking C-path boundary explicit, and it does not widen claims to other backends or machines",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Article CPU reproducibility summary now records measured_green_classes={}, declared_supported_classes={}, unsupported_classes={}, optional_c_path_blocks_claim={}.",
            report.measured_green_machine_class_ids.len(),
            report.declared_supported_machine_class_ids.len(),
            report.unsupported_machine_class_ids.len(),
            report.optional_c_path_blocks_rust_only_claim,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_article_cpu_reproducibility_summary_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleCpuReproducibilitySummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarArticleCpuReproducibilityReportError),
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

pub fn build_tassadar_article_cpu_reproducibility_summary_report()
-> Result<TassadarArticleCpuReproducibilitySummaryReport, TassadarArticleCpuReproducibilitySummaryError>
{
    Ok(TassadarArticleCpuReproducibilitySummaryReport::new(
        build_tassadar_article_cpu_reproducibility_report()?,
    ))
}

pub fn tassadar_article_cpu_reproducibility_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_cpu_reproducibility_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleCpuReproducibilitySummaryReport,
    TassadarArticleCpuReproducibilitySummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleCpuReproducibilitySummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_cpu_reproducibility_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleCpuReproducibilitySummaryError::Write {
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
) -> Result<T, TassadarArticleCpuReproducibilitySummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleCpuReproducibilitySummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleCpuReproducibilitySummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_SUMMARY_REPORT_REF,
        TassadarArticleCpuReproducibilitySummaryReport,
        build_tassadar_article_cpu_reproducibility_summary_report, read_repo_json,
        tassadar_article_cpu_reproducibility_summary_report_path,
        write_tassadar_article_cpu_reproducibility_summary_report,
    };

    #[test]
    fn article_cpu_reproducibility_summary_keeps_measured_vs_declared_split() {
        let report = build_tassadar_article_cpu_reproducibility_summary_report().expect("summary");

        assert_eq!(report.measured_green_machine_class_ids.len(), 1);
        assert_eq!(report.declared_supported_machine_class_ids.len(), 1);
        assert_eq!(report.unsupported_machine_class_ids, vec![String::from("other_host_cpu")]);
        assert!(!report.optional_c_path_blocks_rust_only_claim);
    }

    #[test]
    fn article_cpu_reproducibility_summary_matches_committed_truth() {
        let generated =
            build_tassadar_article_cpu_reproducibility_summary_report().expect("summary");
        let committed: TassadarArticleCpuReproducibilitySummaryReport = read_repo_json(
            TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_SUMMARY_REPORT_REF,
            "tassadar_article_cpu_reproducibility_summary_report",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_cpu_reproducibility_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_cpu_reproducibility_summary.json");
        let written = write_tassadar_article_cpu_reproducibility_summary_report(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleCpuReproducibilitySummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_cpu_reproducibility_summary_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_cpu_reproducibility_summary.json")
        );
    }
}
