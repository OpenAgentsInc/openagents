use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    TassadarScalingAxis, TassadarScalingPhase, TASSADAR_KERNEL_MODULE_SCALING_SUMMARY_REPORT_REF,
};
use psionic_eval::{
    build_tassadar_kernel_module_scaling_report, TassadarKernelModuleScalingPosture,
    TassadarKernelModuleScalingReport, TassadarKernelModuleScalingReportError,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Research-facing summary over kernel-vs-module scaling laws.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarKernelModuleScalingSummaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub scaling_report: TassadarKernelModuleScalingReport,
    pub phase_exact_trace_thresholds: BTreeMap<String, u64>,
    pub kernel_cost_degraded_family_ids: Vec<String>,
    pub bridge_cost_degraded_family_ids: Vec<String>,
    pub module_exact_family_ids: Vec<String>,
    pub refusal_boundary_family_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Summary build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarKernelModuleScalingSummaryReportError {
    #[error(transparent)]
    Eval(#[from] TassadarKernelModuleScalingReportError),
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

/// Builds the committed kernel-vs-module scaling summary.
pub fn build_tassadar_kernel_module_scaling_summary_report(
) -> Result<TassadarKernelModuleScalingSummaryReport, TassadarKernelModuleScalingSummaryReportError>
{
    let scaling_report = build_tassadar_kernel_module_scaling_report()?;
    let phase_exact_trace_thresholds = scaling_report
        .route_thresholds
        .iter()
        .filter(|threshold| threshold.axis == TassadarScalingAxis::TraceLength)
        .map(|threshold| {
            (
                threshold.phase.as_str().to_string(),
                threshold.max_exact_value,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let kernel_cost_degraded_family_ids =
        cost_degraded_family_ids(&scaling_report, TassadarScalingPhase::KernelScale);
    let bridge_cost_degraded_family_ids =
        cost_degraded_family_ids(&scaling_report, TassadarScalingPhase::BridgeScale);
    let module_exact_family_ids = scaling_report
        .family_reports
        .iter()
        .filter(|family| family.phase == TassadarScalingPhase::ModuleScale)
        .filter(|family| family.exact_point_count > 0)
        .filter(|family| family.refusal_point_count == 0)
        .map(|family| family.workload_family.as_str().to_string())
        .collect::<Vec<_>>();
    let refusal_boundary_family_ids = scaling_report
        .family_reports
        .iter()
        .filter(|family| family.refusal_point_count > 0)
        .map(|family| family.workload_family.as_str().to_string())
        .collect::<Vec<_>>();
    let mut report = TassadarKernelModuleScalingSummaryReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.kernel_module_scaling.summary.v1"),
        scaling_report,
        phase_exact_trace_thresholds,
        kernel_cost_degraded_family_ids,
        bridge_cost_degraded_family_ids,
        module_exact_family_ids,
        refusal_boundary_family_ids,
        claim_boundary: String::from(
            "this summary remains a research-only interpretation over the committed kernel-vs-module scaling report. It keeps exact trace ceilings, cost-degraded families, and refusal boundaries explicit instead of using scaling observations to widen served capability claims or module-closure language",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Kernel-vs-module scaling summary keeps trace ceilings {:?}, {} kernel cost-degraded families, {} bridge cost-degraded families, {} exact module families, and {} refusal-boundary families explicit.",
        report.phase_exact_trace_thresholds,
        report.kernel_cost_degraded_family_ids.len(),
        report.bridge_cost_degraded_family_ids.len(),
        report.module_exact_family_ids.len(),
        report.refusal_boundary_family_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_kernel_module_scaling_summary_report|",
        &report,
    );
    Ok(report)
}

/// Returns the canonical absolute path for the committed scaling summary.
#[must_use]
pub fn tassadar_kernel_module_scaling_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_KERNEL_MODULE_SCALING_SUMMARY_REPORT_REF)
}

/// Writes the committed kernel-vs-module scaling summary.
pub fn write_tassadar_kernel_module_scaling_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarKernelModuleScalingSummaryReport, TassadarKernelModuleScalingSummaryReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarKernelModuleScalingSummaryReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_kernel_module_scaling_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarKernelModuleScalingSummaryReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn cost_degraded_family_ids(
    report: &TassadarKernelModuleScalingReport,
    phase: TassadarScalingPhase,
) -> Vec<String> {
    report
        .family_reports
        .iter()
        .filter(|family| family.phase == phase)
        .filter(|family| {
            family.points.iter().any(|point| {
                point.posture == TassadarKernelModuleScalingPosture::ExactButCostDegraded
            })
        })
        .map(|family| family.workload_family.as_str().to_string())
        .collect::<Vec<_>>()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
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
) -> Result<T, TassadarKernelModuleScalingSummaryReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarKernelModuleScalingSummaryReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarKernelModuleScalingSummaryReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_kernel_module_scaling_summary_report, read_repo_json,
        tassadar_kernel_module_scaling_summary_report_path,
        write_tassadar_kernel_module_scaling_summary_report,
        TassadarKernelModuleScalingSummaryReport,
    };
    use psionic_data::TASSADAR_KERNEL_MODULE_SCALING_SUMMARY_REPORT_REF;

    #[test]
    fn kernel_module_scaling_summary_marks_breakpoints_and_refusal_boundaries() {
        let report =
            build_tassadar_kernel_module_scaling_summary_report().expect("scaling summary");

        assert!(report
            .kernel_cost_degraded_family_ids
            .contains(&String::from("backward_loop_kernel")));
        assert!(report
            .refusal_boundary_family_ids
            .contains(&String::from("module_host_import_boundary")));
        assert!(report
            .module_exact_family_ids
            .contains(&String::from("module_parsing")));
        assert_eq!(
            report.phase_exact_trace_thresholds.get("module_scale"),
            Some(&16)
        );
    }

    #[test]
    fn kernel_module_scaling_summary_matches_committed_truth() {
        let generated =
            build_tassadar_kernel_module_scaling_summary_report().expect("scaling summary");
        let committed: TassadarKernelModuleScalingSummaryReport =
            read_repo_json(TASSADAR_KERNEL_MODULE_SCALING_SUMMARY_REPORT_REF)
                .expect("committed scaling summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_kernel_module_scaling_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_kernel_module_scaling_summary.json");
        let written = write_tassadar_kernel_module_scaling_summary_report(&output_path)
            .expect("write scaling summary");
        let persisted: TassadarKernelModuleScalingSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");

        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_kernel_module_scaling_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_kernel_module_scaling_summary.json")
        );
    }
}
