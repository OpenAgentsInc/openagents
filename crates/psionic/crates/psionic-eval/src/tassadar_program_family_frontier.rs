use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    TassadarProgramFamilyArchitectureFamily, TassadarProgramFamilyFrontierBundle,
    TassadarProgramFamilyFrontierEvidenceCase, TassadarProgramFamilyGeneralizationSplit,
    TassadarProgramFamilyWorkloadFamily, TASSADAR_PROGRAM_FAMILY_FRONTIER_BUNDLE_REF,
    TASSADAR_PROGRAM_FAMILY_FRONTIER_REPORT_REF,
};
use psionic_models::tassadar_program_family_frontier_publication;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramFamilyArchitectureSummary {
    pub architecture_family: TassadarProgramFamilyArchitectureFamily,
    pub mean_in_family_exactness_bps: u32,
    pub mean_held_out_exactness_bps: u32,
    pub mean_refusal_calibration_bps: u32,
    pub mean_normalized_cost_units: u32,
    pub held_out_efficiency_bps_per_cost_unit: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramFamilyHeldOutLadderRow {
    pub workload_family: TassadarProgramFamilyWorkloadFamily,
    pub best_architecture: TassadarProgramFamilyArchitectureFamily,
    pub compiled_exact_reference_bps: u32,
    pub learned_structured_memory_bps: u32,
    pub verifier_attached_hybrid_bps: u32,
    pub hybrid_gain_vs_learned_bps: i32,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramFamilyFrontierReport {
    pub schema_version: u16,
    pub report_id: String,
    pub publication_digest: String,
    pub contract_ref: String,
    pub contract_digest: String,
    pub evidence_bundle_ref: String,
    pub evidence_bundle_digest: String,
    pub architecture_summaries: Vec<TassadarProgramFamilyArchitectureSummary>,
    pub held_out_family_ladder: Vec<TassadarProgramFamilyHeldOutLadderRow>,
    pub failure_mode_taxonomy: BTreeMap<String, u32>,
    pub fragile_held_out_on_learned: Vec<TassadarProgramFamilyWorkloadFamily>,
    pub hybrid_recoverable_families: Vec<TassadarProgramFamilyWorkloadFamily>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarProgramFamilyFrontierReportError {
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

pub fn build_tassadar_program_family_frontier_report(
) -> Result<TassadarProgramFamilyFrontierReport, TassadarProgramFamilyFrontierReportError> {
    let publication = tassadar_program_family_frontier_publication();
    let bundle: TassadarProgramFamilyFrontierBundle =
        read_repo_json(TASSADAR_PROGRAM_FAMILY_FRONTIER_BUNDLE_REF)?;
    let architecture_summaries = build_architecture_summaries(bundle.case_reports.as_slice());
    let held_out_family_ladder = build_held_out_family_ladder(bundle.case_reports.as_slice());
    let failure_mode_taxonomy = build_failure_mode_taxonomy(bundle.case_reports.as_slice());
    let fragile_held_out_on_learned = held_out_family_ladder
        .iter()
        .filter(|row| row.learned_structured_memory_bps < 6_000)
        .map(|row| row.workload_family)
        .collect::<Vec<_>>();
    let hybrid_recoverable_families = held_out_family_ladder
        .iter()
        .filter(|row| row.verifier_attached_hybrid_bps >= 7_400)
        .map(|row| row.workload_family)
        .collect::<Vec<_>>();
    let mut report = TassadarProgramFamilyFrontierReport {
        schema_version: 1,
        report_id: String::from("tassadar.program_family_frontier.report.v1"),
        publication_digest: publication.publication_digest,
        contract_ref: bundle.contract.contract_ref.clone(),
        contract_digest: bundle.contract.contract_digest.clone(),
        evidence_bundle_ref: String::from(TASSADAR_PROGRAM_FAMILY_FRONTIER_BUNDLE_REF),
        evidence_bundle_digest: bundle.report_digest.clone(),
        architecture_summaries,
        held_out_family_ladder,
        failure_mode_taxonomy,
        fragile_held_out_on_learned,
        hybrid_recoverable_families,
        claim_boundary: String::from(
            "this report keeps program-family transfer bounded to the named benchmark suite and explicit architecture families. It does not promote broad internal compute, arbitrary Wasm, or served capability; held-out-family misses and verifier-budget limits remain explicit instead of being smoothed into generic success rates",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Program-family frontier compares {} architecture families across {} held-out ladder rows, with {} learned-fragile held-out families and {} hybrid-recoverable held-out families.",
        report.architecture_summaries.len(),
        report.held_out_family_ladder.len(),
        report.fragile_held_out_on_learned.len(),
        report.hybrid_recoverable_families.len(),
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_program_family_frontier_report|", &report);
    Ok(report)
}

#[must_use]
pub fn tassadar_program_family_frontier_report_path() -> PathBuf {
    repo_root().join(TASSADAR_PROGRAM_FAMILY_FRONTIER_REPORT_REF)
}

pub fn write_tassadar_program_family_frontier_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarProgramFamilyFrontierReport, TassadarProgramFamilyFrontierReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarProgramFamilyFrontierReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_program_family_frontier_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarProgramFamilyFrontierReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_architecture_summaries(
    cases: &[TassadarProgramFamilyFrontierEvidenceCase],
) -> Vec<TassadarProgramFamilyArchitectureSummary> {
    let mut grouped = BTreeMap::<
        TassadarProgramFamilyArchitectureFamily,
        Vec<&TassadarProgramFamilyFrontierEvidenceCase>,
    >::new();
    for case in cases {
        grouped
            .entry(case.architecture_family)
            .or_default()
            .push(case);
    }
    grouped
        .into_iter()
        .map(|(architecture_family, rows)| {
            let held_out_rows = rows
                .iter()
                .copied()
                .filter(|row| row.split == TassadarProgramFamilyGeneralizationSplit::HeldOutFamily)
                .collect::<Vec<_>>();
            let mean_held_out_exactness_bps = mean(
                held_out_rows
                    .iter()
                    .map(|row| row.later_window_exactness_bps),
            );
            let mean_normalized_cost_units = mean(rows.iter().map(|row| row.normalized_cost_units));
            TassadarProgramFamilyArchitectureSummary {
                architecture_family,
                mean_in_family_exactness_bps: mean(rows.iter().filter_map(|row| {
                    (row.split == TassadarProgramFamilyGeneralizationSplit::InFamily)
                        .then_some(row.later_window_exactness_bps)
                })),
                mean_held_out_exactness_bps,
                mean_refusal_calibration_bps: mean(
                    rows.iter().map(|row| row.refusal_calibration_bps),
                ),
                mean_normalized_cost_units,
                held_out_efficiency_bps_per_cost_unit: if mean_normalized_cost_units == 0 {
                    0
                } else {
                    mean_held_out_exactness_bps / mean_normalized_cost_units
                },
            }
        })
        .collect()
}

fn build_held_out_family_ladder(
    cases: &[TassadarProgramFamilyFrontierEvidenceCase],
) -> Vec<TassadarProgramFamilyHeldOutLadderRow> {
    let mut held_out_families = BTreeSet::new();
    for case in cases {
        if case.split == TassadarProgramFamilyGeneralizationSplit::HeldOutFamily {
            held_out_families.insert(case.workload_family);
        }
    }
    held_out_families
        .into_iter()
        .map(|workload_family| {
            let compiled = case_for(
                cases,
                workload_family,
                TassadarProgramFamilyArchitectureFamily::CompiledExactReference,
            );
            let learned = case_for(
                cases,
                workload_family,
                TassadarProgramFamilyArchitectureFamily::LearnedStructuredMemory,
            );
            let hybrid = case_for(
                cases,
                workload_family,
                TassadarProgramFamilyArchitectureFamily::VerifierAttachedHybrid,
            );
            let best_architecture = if hybrid.later_window_exactness_bps
                >= compiled.later_window_exactness_bps
                && hybrid.later_window_exactness_bps >= learned.later_window_exactness_bps
            {
                TassadarProgramFamilyArchitectureFamily::VerifierAttachedHybrid
            } else if compiled.later_window_exactness_bps >= learned.later_window_exactness_bps {
                TassadarProgramFamilyArchitectureFamily::CompiledExactReference
            } else {
                TassadarProgramFamilyArchitectureFamily::LearnedStructuredMemory
            };
            TassadarProgramFamilyHeldOutLadderRow {
                workload_family,
                best_architecture,
                compiled_exact_reference_bps: compiled.later_window_exactness_bps,
                learned_structured_memory_bps: learned.later_window_exactness_bps,
                verifier_attached_hybrid_bps: hybrid.later_window_exactness_bps,
                hybrid_gain_vs_learned_bps: hybrid.later_window_exactness_bps as i32
                    - learned.later_window_exactness_bps as i32,
                note: format!(
                    "learned failure `{}` vs hybrid failure `{}`",
                    learned.dominant_failure_mode, hybrid.dominant_failure_mode
                ),
            }
        })
        .collect()
}

fn build_failure_mode_taxonomy(
    cases: &[TassadarProgramFamilyFrontierEvidenceCase],
) -> BTreeMap<String, u32> {
    let mut taxonomy = BTreeMap::new();
    for case in cases {
        *taxonomy
            .entry(case.dominant_failure_mode.clone())
            .or_insert(0) += 1;
    }
    taxonomy
}

fn case_for(
    cases: &[TassadarProgramFamilyFrontierEvidenceCase],
    workload_family: TassadarProgramFamilyWorkloadFamily,
    architecture_family: TassadarProgramFamilyArchitectureFamily,
) -> &TassadarProgramFamilyFrontierEvidenceCase {
    cases
        .iter()
        .find(|case| {
            case.workload_family == workload_family
                && case.architecture_family == architecture_family
        })
        .expect("case should exist")
}

fn mean(values: impl Iterator<Item = u32>) -> u32 {
    let values = values.map(u64::from).collect::<Vec<_>>();
    if values.is_empty() {
        0
    } else {
        (values.iter().sum::<u64>() / values.len() as u64) as u32
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarProgramFamilyFrontierReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarProgramFamilyFrontierReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarProgramFamilyFrontierReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_program_family_frontier_report, read_repo_json,
        tassadar_program_family_frontier_report_path,
        write_tassadar_program_family_frontier_report, TassadarProgramFamilyFrontierReport,
    };
    use psionic_data::{
        TassadarProgramFamilyWorkloadFamily, TASSADAR_PROGRAM_FAMILY_FRONTIER_REPORT_REF,
    };

    #[test]
    fn program_family_frontier_report_keeps_hybrid_recovery_and_failure_taxonomy_explicit() {
        let report = build_tassadar_program_family_frontier_report().expect("report");

        assert!(report
            .hybrid_recoverable_families
            .contains(&TassadarProgramFamilyWorkloadFamily::HeldOutVirtualMachine));
        assert!(report
            .fragile_held_out_on_learned
            .contains(&TassadarProgramFamilyWorkloadFamily::HeldOutMessageOrchestrator));
        assert!(report
            .failure_mode_taxonomy
            .contains_key("instruction_dispatch_collapse"));
    }

    #[test]
    fn program_family_frontier_report_matches_committed_truth() {
        let generated = build_tassadar_program_family_frontier_report().expect("report");
        let committed: TassadarProgramFamilyFrontierReport =
            read_repo_json(TASSADAR_PROGRAM_FAMILY_FRONTIER_REPORT_REF).expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_program_family_frontier_report_persists_current_truth() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output_path = output_dir
            .path()
            .join("tassadar_program_family_frontier_report.json");
        let report =
            write_tassadar_program_family_frontier_report(&output_path).expect("write report");
        let written: TassadarProgramFamilyFrontierReport =
            serde_json::from_str(&std::fs::read_to_string(&output_path).expect("written file"))
                .expect("parse");

        assert_eq!(report, written);
        assert_eq!(
            tassadar_program_family_frontier_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_program_family_frontier_report.json")
        );
    }
}
