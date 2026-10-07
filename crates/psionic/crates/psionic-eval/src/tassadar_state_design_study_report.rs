use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{TASSADAR_STATE_DESIGN_STUDY_REPORT_REF, tassadar_state_design_study_contract};
use psionic_ir::{TassadarStateDesignFamily, TassadarStateDesignReplayPosture};
use psionic_models::tassadar_state_design_study_publication;
use psionic_runtime::{
    TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF, TassadarStateDesignCaseReport,
    TassadarStateDesignRuntimeReportError, TassadarStateDesignWorkloadFamily,
    build_tassadar_state_design_runtime_report,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Family-level summary for the state-design study report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStateDesignFamilySummary {
    /// Compared state-design family.
    pub design_family: TassadarStateDesignFamily,
    /// Exact-replay case count.
    pub exact_replay_case_count: u32,
    /// Reconstructable-replay case count.
    pub reconstructable_replay_case_count: u32,
    /// Bounded-state-publication case count.
    pub bounded_state_publication_case_count: u32,
    /// Refused case count.
    pub refused_case_count: u32,
    /// Mean locality score across non-refused cases.
    pub average_locality_score_bps: u32,
    /// Mean edit cost across non-refused cases.
    pub average_edit_cost_bps: u32,
    /// Workloads where this design is recommended by the current study.
    pub preferred_workload_families: Vec<TassadarStateDesignWorkloadFamily>,
    /// Plain-language note.
    pub note: String,
}

/// Workload-level summary for the state-design study report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStateDesignWorkloadSummary {
    /// Compared workload family.
    pub workload_family: TassadarStateDesignWorkloadFamily,
    /// Families that remain exact replay on this workload.
    pub exact_replay_families: Vec<TassadarStateDesignFamily>,
    /// Families that remain reconstructable on this workload.
    pub reconstructable_replay_families: Vec<TassadarStateDesignFamily>,
    /// Families that remain bounded-state-publication studies on this workload.
    pub bounded_state_publication_families: Vec<TassadarStateDesignFamily>,
    /// Families refused on this workload.
    pub refused_families: Vec<TassadarStateDesignFamily>,
    /// Current best representation fit for the workload.
    pub recommended_design_family: TassadarStateDesignFamily,
    /// Plain-language note.
    pub note: String,
}

/// Eval report joining the public study contract, publication, and runtime truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStateDesignStudyReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Source contract reference.
    pub study_contract_ref: String,
    /// Source contract digest.
    pub study_contract_digest: String,
    /// Model publication digest.
    pub publication_digest: String,
    /// Runtime report reference.
    pub runtime_report_ref: String,
    /// Runtime report digest.
    pub runtime_report_digest: String,
    /// Family-level summaries.
    pub family_summaries: Vec<TassadarStateDesignFamilySummary>,
    /// Workload-level summaries.
    pub workload_summaries: Vec<TassadarStateDesignWorkloadSummary>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Eval failure while building or writing the report.
#[derive(Debug, Error)]
pub enum TassadarStateDesignStudyReportError {
    #[error(transparent)]
    Runtime(#[from] TassadarStateDesignRuntimeReportError),
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

/// Builds the machine-readable state-design study report.
pub fn build_tassadar_state_design_study_report()
-> Result<TassadarStateDesignStudyReport, TassadarStateDesignStudyReportError> {
    let contract = tassadar_state_design_study_contract();
    let publication = tassadar_state_design_study_publication();
    let runtime_report = build_tassadar_state_design_runtime_report();
    let workload_summaries = build_workload_summaries(runtime_report.case_reports.as_slice());
    let family_summaries = build_family_summaries(
        runtime_report.case_reports.as_slice(),
        workload_summaries.as_slice(),
    );

    let mut report = TassadarStateDesignStudyReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.state_design.study_report.v1"),
        study_contract_ref: contract.contract_ref.clone(),
        study_contract_digest: contract.contract_digest.clone(),
        publication_digest: publication.publication_digest.clone(),
        runtime_report_ref: String::from(TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF),
        runtime_report_digest: runtime_report.report_digest.clone(),
        family_summaries,
        workload_summaries,
        claim_boundary: String::from(
            "this eval report compares representation designs on the same workloads while keeping replay posture and refusal thresholds explicit. It does not promote the recommended design for one workload into a broad executor or product claim",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "State-design study report now joins contract {}, publication {}, and runtime {} across {} design families and {} workload families.",
        contract.contract_ref,
        publication.publication_id,
        runtime_report.report_id,
        report.family_summaries.len(),
        report.workload_summaries.len(),
    );
    report.report_digest = stable_digest(b"psionic_tassadar_state_design_study_report|", &report);
    Ok(report)
}

/// Returns the canonical absolute path for the committed study report.
#[must_use]
pub fn tassadar_state_design_study_report_path() -> PathBuf {
    repo_root().join(TASSADAR_STATE_DESIGN_STUDY_REPORT_REF)
}

/// Writes the committed state-design study report.
pub fn write_tassadar_state_design_study_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarStateDesignStudyReport, TassadarStateDesignStudyReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarStateDesignStudyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_state_design_study_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarStateDesignStudyReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_family_summaries(
    case_reports: &[TassadarStateDesignCaseReport],
    workload_summaries: &[TassadarStateDesignWorkloadSummary],
) -> Vec<TassadarStateDesignFamilySummary> {
    let mut grouped =
        BTreeMap::<TassadarStateDesignFamily, Vec<&TassadarStateDesignCaseReport>>::new();
    for case in case_reports {
        grouped.entry(case.design_family).or_default().push(case);
    }
    grouped
        .into_iter()
        .map(|(design_family, cases)| {
            let preferred_workload_families = workload_summaries
                .iter()
                .filter(|summary| summary.recommended_design_family == design_family)
                .map(|summary| summary.workload_family)
                .collect::<Vec<_>>();
            TassadarStateDesignFamilySummary {
                design_family,
                exact_replay_case_count: count_posture(
                    cases.as_slice(),
                    TassadarStateDesignReplayPosture::ExactReplay,
                ),
                reconstructable_replay_case_count: count_posture(
                    cases.as_slice(),
                    TassadarStateDesignReplayPosture::ReconstructableReplay,
                ),
                bounded_state_publication_case_count: count_posture(
                    cases.as_slice(),
                    TassadarStateDesignReplayPosture::BoundedStatePublication,
                ),
                refused_case_count: count_posture(
                    cases.as_slice(),
                    TassadarStateDesignReplayPosture::Refused,
                ),
                average_locality_score_bps: rounded_mean(
                    cases
                        .iter()
                        .filter_map(|case| case.locality_score_bps.map(u64::from)),
                ),
                average_edit_cost_bps: rounded_mean(
                    cases
                        .iter()
                        .filter_map(|case| case.edit_cost_bps.map(u64::from)),
                ),
                preferred_workload_families,
                note: family_note(design_family),
            }
        })
        .collect()
}

fn build_workload_summaries(
    case_reports: &[TassadarStateDesignCaseReport],
) -> Vec<TassadarStateDesignWorkloadSummary> {
    let mut grouped =
        BTreeMap::<TassadarStateDesignWorkloadFamily, Vec<&TassadarStateDesignCaseReport>>::new();
    for case in case_reports {
        grouped.entry(case.workload_family).or_default().push(case);
    }
    grouped
        .into_iter()
        .map(|(workload_family, cases)| {
            let exact_replay_families = families_for_posture(
                cases.as_slice(),
                TassadarStateDesignReplayPosture::ExactReplay,
            );
            let reconstructable_replay_families = families_for_posture(
                cases.as_slice(),
                TassadarStateDesignReplayPosture::ReconstructableReplay,
            );
            let bounded_state_publication_families = families_for_posture(
                cases.as_slice(),
                TassadarStateDesignReplayPosture::BoundedStatePublication,
            );
            let refused_families =
                families_for_posture(cases.as_slice(), TassadarStateDesignReplayPosture::Refused);
            let recommended_design_family = cases
                .iter()
                .max_by_key(|case| recommendation_score(case))
                .map(|case| case.design_family)
                .expect("workload group should not be empty");
            TassadarStateDesignWorkloadSummary {
                workload_family,
                exact_replay_families,
                reconstructable_replay_families,
                bounded_state_publication_families,
                refused_families,
                recommended_design_family,
                note: workload_note(workload_family, recommended_design_family),
            }
        })
        .collect()
}

fn count_posture(
    cases: &[&TassadarStateDesignCaseReport],
    posture: TassadarStateDesignReplayPosture,
) -> u32 {
    cases
        .iter()
        .filter(|case| case.replay_posture == posture)
        .count() as u32
}

fn families_for_posture(
    cases: &[&TassadarStateDesignCaseReport],
    posture: TassadarStateDesignReplayPosture,
) -> Vec<TassadarStateDesignFamily> {
    cases
        .iter()
        .filter(|case| case.replay_posture == posture)
        .map(|case| case.design_family)
        .collect()
}

fn recommendation_score(case: &TassadarStateDesignCaseReport) -> i64 {
    let replay_bonus = match case.replay_posture {
        TassadarStateDesignReplayPosture::ExactReplay => 1_500,
        TassadarStateDesignReplayPosture::ReconstructableReplay => 900,
        TassadarStateDesignReplayPosture::BoundedStatePublication => 600,
        TassadarStateDesignReplayPosture::Refused => -100_000,
    };
    replay_bonus + i64::from(case.locality_score_bps.unwrap_or_default())
        - i64::from(case.edit_cost_bps.unwrap_or(10_000))
}

fn family_note(design_family: TassadarStateDesignFamily) -> String {
    match design_family {
        TassadarStateDesignFamily::FullAppendOnlyTrace => String::from(
            "append-only traces remain the replay floor, but they are rarely the best locality or edit-cost design on the current workload set",
        ),
        TassadarStateDesignFamily::DeltaTrace => String::from(
            "delta traces win where changed addresses or frames stay reconstructable, but they refuse workloads that would hide semantic state behind opaque deltas",
        ),
        TassadarStateDesignFamily::LocalityScratchpad => String::from(
            "scratchpad layouts win where token-trace truth must stay exact but locality can still be reshaped safely",
        ),
        TassadarStateDesignFamily::RecurrentState => String::from(
            "recurrent state wins long-horizon workloads only under explicit bounded-state publication rather than replay-complete claims",
        ),
        TassadarStateDesignFamily::WorkingMemoryTier => String::from(
            "working-memory publication wins semantic memory workloads when slot and lookup semantics stay explicit and bounded",
        ),
    }
}

fn workload_note(
    workload_family: TassadarStateDesignWorkloadFamily,
    recommended_design_family: TassadarStateDesignFamily,
) -> String {
    format!(
        "{} currently favors {} as the best representation tradeoff on the seeded study because it preserves the relevant truth surface with lower edit cost than the audit-floor full trace.",
        workload_family.as_str(),
        recommended_design_family.as_str()
    )
}

fn rounded_mean(values: impl IntoIterator<Item = u64>) -> u32 {
    let values = values.into_iter().collect::<Vec<_>>();
    if values.is_empty() {
        return 0;
    }
    let sum = values.iter().sum::<u64>();
    ((sum + (values.len() as u64 / 2)) / values.len() as u64) as u32
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
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
) -> Result<T, TassadarStateDesignStudyReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarStateDesignStudyReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarStateDesignStudyReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarStateDesignStudyReport, build_tassadar_state_design_study_report, read_repo_json,
        tassadar_state_design_study_report_path, write_tassadar_state_design_study_report,
    };
    use psionic_data::TASSADAR_STATE_DESIGN_STUDY_REPORT_REF;
    use psionic_ir::TassadarStateDesignFamily;
    use psionic_runtime::TassadarStateDesignWorkloadFamily;

    #[test]
    fn state_design_study_report_recommends_richer_state_surfaces_selectively() {
        let report = build_tassadar_state_design_study_report().expect("state-design study report");

        let associative = report
            .workload_summaries
            .iter()
            .find(|summary| {
                summary.workload_family == TassadarStateDesignWorkloadFamily::AssociativeRecall
            })
            .expect("associative summary");
        assert_eq!(
            associative.recommended_design_family,
            TassadarStateDesignFamily::WorkingMemoryTier
        );

        let control = report
            .workload_summaries
            .iter()
            .find(|summary| {
                summary.workload_family == TassadarStateDesignWorkloadFamily::LongHorizonControl
            })
            .expect("control summary");
        assert_eq!(
            control.recommended_design_family,
            TassadarStateDesignFamily::RecurrentState
        );
    }

    #[test]
    fn state_design_study_report_matches_committed_truth() {
        let generated =
            build_tassadar_state_design_study_report().expect("state-design study report");
        let committed: TassadarStateDesignStudyReport =
            read_repo_json(TASSADAR_STATE_DESIGN_STUDY_REPORT_REF)
                .expect("committed state-design study report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_state_design_study_report_persists_current_truth() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output_path = output_dir
            .path()
            .join("tassadar_state_design_study_report.json");
        let report = write_tassadar_state_design_study_report(&output_path).expect("write report");
        let written = std::fs::read_to_string(&output_path).expect("written study report");
        let reparsed: TassadarStateDesignStudyReport =
            serde_json::from_str(&written).expect("written study report should parse");

        assert_eq!(report, reparsed);
        assert_eq!(
            tassadar_state_design_study_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_state_design_study_report.json")
        );
    }
}
