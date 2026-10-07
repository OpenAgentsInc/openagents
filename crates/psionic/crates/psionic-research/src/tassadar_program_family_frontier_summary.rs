use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    TassadarProgramFamilyWorkloadFamily, TASSADAR_PROGRAM_FAMILY_FRONTIER_SUMMARY_REF,
};
use psionic_eval::{
    build_tassadar_program_family_frontier_report, TassadarProgramFamilyFrontierReport,
    TassadarProgramFamilyFrontierReportError,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramFamilyFrontierSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub eval_report: TassadarProgramFamilyFrontierReport,
    pub compiled_anchor_families: Vec<TassadarProgramFamilyWorkloadFamily>,
    pub hybrid_frontier_families: Vec<TassadarProgramFamilyWorkloadFamily>,
    pub learned_only_in_family_families: Vec<TassadarProgramFamilyWorkloadFamily>,
    pub held_out_break_families: Vec<TassadarProgramFamilyWorkloadFamily>,
    pub cost_efficient_hybrid_families: Vec<TassadarProgramFamilyWorkloadFamily>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarProgramFamilyFrontierSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarProgramFamilyFrontierReportError),
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

pub fn build_tassadar_program_family_frontier_summary(
) -> Result<TassadarProgramFamilyFrontierSummary, TassadarProgramFamilyFrontierSummaryError> {
    let eval_report = build_tassadar_program_family_frontier_report()?;
    let compiled_anchor_families = eval_report
        .held_out_family_ladder
        .iter()
        .filter(|row| row.compiled_exact_reference_bps >= 8_900)
        .map(|row| row.workload_family)
        .collect::<Vec<_>>();
    let hybrid_frontier_families = eval_report
        .held_out_family_ladder
        .iter()
        .filter(|row| row.hybrid_gain_vs_learned_bps >= 2_000)
        .map(|row| row.workload_family)
        .collect::<Vec<_>>();
    let learned_only_in_family_families = eval_report
        .architecture_summaries
        .iter()
        .find(|summary| {
            summary.architecture_family
                == psionic_data::TassadarProgramFamilyArchitectureFamily::LearnedStructuredMemory
        })
        .filter(|summary| summary.mean_in_family_exactness_bps >= 7_500)
        .map(|_| {
            vec![
                TassadarProgramFamilyWorkloadFamily::KernelStateMachine,
                TassadarProgramFamilyWorkloadFamily::LinkedProgramBundle,
                TassadarProgramFamilyWorkloadFamily::MultiModulePackageWorkflow,
            ]
        })
        .unwrap_or_default();
    let held_out_break_families = eval_report.fragile_held_out_on_learned.clone();
    let hybrid_summary = eval_report
        .architecture_summaries
        .iter()
        .find(|summary| {
            summary.architecture_family
                == psionic_data::TassadarProgramFamilyArchitectureFamily::VerifierAttachedHybrid
        })
        .expect("hybrid summary");
    let cost_efficient_hybrid_families = eval_report
        .held_out_family_ladder
        .iter()
        .filter(|row| row.hybrid_gain_vs_learned_bps >= 2_000)
        .filter(|_| hybrid_summary.held_out_efficiency_bps_per_cost_unit >= 30)
        .map(|row| row.workload_family)
        .collect::<Vec<_>>();
    let mut summary = TassadarProgramFamilyFrontierSummary {
        schema_version: 1,
        report_id: String::from("tassadar.program_family_frontier.summary.v1"),
        eval_report,
        compiled_anchor_families,
        hybrid_frontier_families,
        learned_only_in_family_families,
        held_out_break_families,
        cost_efficient_hybrid_families,
        claim_boundary: String::from(
            "this summary interprets the committed program-family frontier as a research-only transfer map. It keeps compiled anchors, hybrid recoverability, and learned held-out breaks explicit instead of promoting any lane into broad practical internal computation or served capability",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    summary.summary = format!(
        "Program-family frontier summary marks {} compiled anchor families, {} hybrid frontier families, {} learned-only in-family families, and {} held-out break families.",
        summary.compiled_anchor_families.len(),
        summary.hybrid_frontier_families.len(),
        summary.learned_only_in_family_families.len(),
        summary.held_out_break_families.len(),
    );
    summary.report_digest = stable_digest(
        b"psionic_tassadar_program_family_frontier_summary|",
        &summary,
    );
    Ok(summary)
}

#[must_use]
pub fn tassadar_program_family_frontier_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_PROGRAM_FAMILY_FRONTIER_SUMMARY_REF)
}

pub fn write_tassadar_program_family_frontier_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarProgramFamilyFrontierSummary, TassadarProgramFamilyFrontierSummaryError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarProgramFamilyFrontierSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_program_family_frontier_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarProgramFamilyFrontierSummaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(summary)
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
) -> Result<T, TassadarProgramFamilyFrontierSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarProgramFamilyFrontierSummaryError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarProgramFamilyFrontierSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_program_family_frontier_summary, read_repo_json,
        tassadar_program_family_frontier_summary_path,
        write_tassadar_program_family_frontier_summary, TassadarProgramFamilyFrontierSummary,
    };
    use psionic_data::{
        TassadarProgramFamilyWorkloadFamily, TASSADAR_PROGRAM_FAMILY_FRONTIER_SUMMARY_REF,
    };

    #[test]
    fn program_family_frontier_summary_marks_hybrid_frontier_and_held_out_breaks() {
        let summary = build_tassadar_program_family_frontier_summary().expect("summary");

        assert!(summary
            .hybrid_frontier_families
            .contains(&TassadarProgramFamilyWorkloadFamily::HeldOutVirtualMachine));
        assert!(summary
            .held_out_break_families
            .contains(&TassadarProgramFamilyWorkloadFamily::HeldOutMessageOrchestrator));
        assert!(summary
            .cost_efficient_hybrid_families
            .contains(&TassadarProgramFamilyWorkloadFamily::HeldOutVirtualMachine));
    }

    #[test]
    fn program_family_frontier_summary_matches_committed_truth() {
        let generated = build_tassadar_program_family_frontier_summary().expect("summary");
        let committed: TassadarProgramFamilyFrontierSummary =
            read_repo_json(TASSADAR_PROGRAM_FAMILY_FRONTIER_SUMMARY_REF)
                .expect("committed summary");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_program_family_frontier_summary_persists_current_truth() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output_path = output_dir
            .path()
            .join("tassadar_program_family_frontier_summary.json");
        let summary =
            write_tassadar_program_family_frontier_summary(&output_path).expect("write summary");
        let written: TassadarProgramFamilyFrontierSummary =
            serde_json::from_str(&std::fs::read_to_string(&output_path).expect("written file"))
                .expect("parse");

        assert_eq!(summary, written);
        assert_eq!(
            tassadar_program_family_frontier_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_program_family_frontier_summary.json")
        );
    }
}
