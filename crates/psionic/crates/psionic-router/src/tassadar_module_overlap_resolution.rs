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
    TassadarModuleOverlapCandidate, TassadarModuleOverlapResolutionError,
    TassadarModuleResolverPolicy, build_tassadar_module_catalog_report,
    resolve_tassadar_module_overlap,
};
use psionic_ir::TassadarModuleTrustPosture;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_MODULE_OVERLAP_RESOLUTION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_overlap_resolution_report.json";

/// Final outcome for one overlapping-capability resolution case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleOverlapResolutionOutcome {
    Selected,
    Refused,
}

/// One candidate row recorded inside an overlap-resolution case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleOverlapCandidateCase {
    pub module_ref: String,
    pub cost_score_bps: u16,
    pub evidence_score_bps: u16,
    pub compatibility_score_bps: u16,
}

/// One route-quality comparison case for overlapping module capability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleOverlapResolutionCaseReport {
    pub case_id: String,
    pub mount_id: String,
    pub capability_label: String,
    pub workload_family: String,
    pub candidate_cases: Vec<TassadarModuleOverlapCandidateCase>,
    pub outcome: TassadarModuleOverlapResolutionOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_module_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarModuleOverlapResolutionError>,
    pub note: String,
}

/// Router-owned report over overlapping-capability resolution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleOverlapResolutionReport {
    pub schema_version: u16,
    pub report_id: String,
    pub selected_case_count: u32,
    pub refused_case_count: u32,
    pub mount_override_case_count: u32,
    pub case_reports: Vec<TassadarModuleOverlapResolutionCaseReport>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarModuleOverlapResolutionReportError {
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

/// Builds the bounded overlap-resolution report.
#[must_use]
pub fn build_tassadar_module_overlap_resolution_report() -> TassadarModuleOverlapResolutionReport {
    let report = build_tassadar_module_catalog_report();
    let candidate_entry = report
        .entries
        .iter()
        .find(|entry| entry.module_ref == "candidate_select_core@1.1.0")
        .expect("candidate entry");
    let checkpoint_entry = report
        .entries
        .iter()
        .find(|entry| entry.module_ref == "checkpoint_backtrack_core@1.0.0")
        .expect("checkpoint entry");
    let candidate_select_candidate = TassadarModuleOverlapCandidate {
        module_ref: candidate_entry.module_ref.clone(),
        capability_label: String::from("bounded_search"),
        workload_family: String::from("verifier_search"),
        trust_posture: candidate_entry.trust_posture,
        benchmark_ref_count: candidate_entry.benchmark_refs.len() as u32,
        cost_score_bps: 3600,
        evidence_score_bps: 9200,
        compatibility_score_bps: 9000,
    };
    let checkpoint_candidate = TassadarModuleOverlapCandidate {
        module_ref: checkpoint_entry.module_ref.clone(),
        capability_label: String::from("bounded_search"),
        workload_family: String::from("verifier_search"),
        trust_posture: checkpoint_entry.trust_posture,
        benchmark_ref_count: checkpoint_entry.benchmark_refs.len() as u32,
        cost_score_bps: 3400,
        evidence_score_bps: 8500,
        compatibility_score_bps: 8800,
    };
    let default_policy = TassadarModuleResolverPolicy::new(
        "mount.default.verifier_search",
        "bounded_search",
        "verifier_search",
        TassadarModuleTrustPosture::BenchmarkGatedInternal,
        2,
        vec![],
        vec![],
    );
    let default_selection = resolve_tassadar_module_overlap(
        &[
            candidate_select_candidate.clone(),
            checkpoint_candidate.clone(),
        ],
        &default_policy,
    )
    .expect("default selection");
    let mount_override_policy = TassadarModuleResolverPolicy::new(
        "mount.checkpoint_only.verifier_search",
        "bounded_search",
        "verifier_search",
        TassadarModuleTrustPosture::BenchmarkGatedInternal,
        2,
        vec![String::from("checkpoint_backtrack_core@1.0.0")],
        vec![String::from("checkpoint_backtrack_core@1.0.0")],
    );
    let mount_override_selection = resolve_tassadar_module_overlap(
        &[
            candidate_select_candidate.clone(),
            checkpoint_candidate.clone(),
        ],
        &mount_override_policy,
    )
    .expect("mount override selection");
    let tie_candidate = TassadarModuleOverlapCandidate {
        module_ref: checkpoint_candidate.module_ref.clone(),
        capability_label: String::from("bounded_search"),
        workload_family: String::from("verifier_search"),
        trust_posture: TassadarModuleTrustPosture::ChallengeGatedInstall,
        benchmark_ref_count: 2,
        cost_score_bps: 3500,
        evidence_score_bps: 9000,
        compatibility_score_bps: 9000,
    };
    let tie_policy = TassadarModuleResolverPolicy::new(
        "mount.tie.verifier_search",
        "bounded_search",
        "verifier_search",
        TassadarModuleTrustPosture::BenchmarkGatedInternal,
        2,
        vec![],
        vec![],
    );
    let tie_error = resolve_tassadar_module_overlap(
        &[
            TassadarModuleOverlapCandidate {
                module_ref: candidate_select_candidate.module_ref.clone(),
                capability_label: String::from("bounded_search"),
                workload_family: String::from("verifier_search"),
                trust_posture: TassadarModuleTrustPosture::ChallengeGatedInstall,
                benchmark_ref_count: 2,
                cost_score_bps: 3500,
                evidence_score_bps: 9000,
                compatibility_score_bps: 9000,
            },
            tie_candidate.clone(),
        ],
        &tie_policy,
    )
    .expect_err("tie error");
    let case_reports = vec![
        case_report(
            "overlap.default.verifier_search.v1",
            "mount-default",
            &[
                candidate_select_candidate.clone(),
                checkpoint_candidate.clone(),
            ],
            TassadarModuleOverlapResolutionOutcome::Selected,
            Some(default_selection.module_ref),
            None,
            "default verifier_search resolution prefers candidate_select_core because evidence and compatibility outweigh the slight cost delta",
        ),
        case_report(
            "overlap.mount_override.verifier_search.v1",
            "mount-checkpoint-only",
            &[
                candidate_select_candidate.clone(),
                checkpoint_candidate.clone(),
            ],
            TassadarModuleOverlapResolutionOutcome::Selected,
            Some(mount_override_selection.module_ref),
            None,
            "mount-specific allowlist and preference override route verifier_search to checkpoint_backtrack_core without hiding the policy decision",
        ),
        case_report(
            "overlap.tie.verifier_search.v1",
            "mount-ambiguous",
            &[
                TassadarModuleOverlapCandidate {
                    module_ref: String::from("candidate_select_core@1.1.0"),
                    capability_label: String::from("bounded_search"),
                    workload_family: String::from("verifier_search"),
                    trust_posture: TassadarModuleTrustPosture::ChallengeGatedInstall,
                    benchmark_ref_count: 2,
                    cost_score_bps: 3500,
                    evidence_score_bps: 9000,
                    compatibility_score_bps: 9000,
                },
                tie_candidate,
            ],
            TassadarModuleOverlapResolutionOutcome::Refused,
            None,
            Some(tie_error),
            "the mount refuses when overlapping candidates tie under the explicit resolver policy instead of silently drifting to one hidden default",
        ),
    ];
    let mut report = TassadarModuleOverlapResolutionReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.module_overlap_resolution.report.v1"),
        selected_case_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarModuleOverlapResolutionOutcome::Selected)
            .count() as u32,
        refused_case_count: case_reports
            .iter()
            .filter(|case| case.outcome == TassadarModuleOverlapResolutionOutcome::Refused)
            .count() as u32,
        mount_override_case_count: 1,
        case_reports,
        claim_boundary: String::from(
            "this router report freezes overlapping-capability selection, mount-specific override, and ambiguity refusal for the bounded module catalog lane. It does not claim implicit world-mount closure; named world-mount integration remains an explicit dependency marker outside standalone psionic",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Module-overlap resolution report now freezes {} selected cases, {} refused cases, and {} mount-override cases.",
        report.selected_case_count, report.refused_case_count, report.mount_override_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_module_overlap_resolution_report|",
        &report,
    );
    report
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_module_overlap_resolution_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MODULE_OVERLAP_RESOLUTION_REPORT_REF)
}

/// Writes the committed report.
pub fn write_tassadar_module_overlap_resolution_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarModuleOverlapResolutionReport, TassadarModuleOverlapResolutionReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarModuleOverlapResolutionReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_module_overlap_resolution_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarModuleOverlapResolutionReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_module_overlap_resolution_report(
    path: impl AsRef<Path>,
) -> Result<TassadarModuleOverlapResolutionReport, TassadarModuleOverlapResolutionReportError> {
    read_json(path)
}

fn case_report(
    case_id: &str,
    mount_id: &str,
    candidates: &[TassadarModuleOverlapCandidate],
    outcome: TassadarModuleOverlapResolutionOutcome,
    selected_module_ref: Option<String>,
    refusal_reason: Option<TassadarModuleOverlapResolutionError>,
    note: &str,
) -> TassadarModuleOverlapResolutionCaseReport {
    TassadarModuleOverlapResolutionCaseReport {
        case_id: String::from(case_id),
        mount_id: String::from(mount_id),
        capability_label: String::from("bounded_search"),
        workload_family: String::from("verifier_search"),
        candidate_cases: candidates
            .iter()
            .map(|candidate| TassadarModuleOverlapCandidateCase {
                module_ref: candidate.module_ref.clone(),
                cost_score_bps: candidate.cost_score_bps,
                evidence_score_bps: candidate.evidence_score_bps,
                compatibility_score_bps: candidate.compatibility_score_bps,
            })
            .collect(),
        outcome,
        selected_module_ref,
        refusal_reason,
        note: String::from(note),
    }
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
) -> Result<T, TassadarModuleOverlapResolutionReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarModuleOverlapResolutionReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarModuleOverlapResolutionReportError::Deserialize {
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
        TassadarModuleOverlapResolutionOutcome, build_tassadar_module_overlap_resolution_report,
        load_tassadar_module_overlap_resolution_report,
        tassadar_module_overlap_resolution_report_path,
    };

    #[test]
    fn module_overlap_resolution_report_keeps_mount_overrides_and_ambiguity_explicit() {
        let report = build_tassadar_module_overlap_resolution_report();

        assert_eq!(report.selected_case_count, 2);
        assert_eq!(report.refused_case_count, 1);
        assert_eq!(report.mount_override_case_count, 1);
        assert!(report.case_reports.iter().any(|case| {
            case.mount_id == "mount-checkpoint-only"
                && case.selected_module_ref.as_deref() == Some("checkpoint_backtrack_core@1.0.0")
        }));
        assert!(report.case_reports.iter().any(|case| {
            case.outcome == TassadarModuleOverlapResolutionOutcome::Refused
                && case.refusal_reason.is_some()
        }));
    }

    #[test]
    fn module_overlap_resolution_report_matches_committed_truth() {
        let expected = build_tassadar_module_overlap_resolution_report();
        let committed = load_tassadar_module_overlap_resolution_report(
            tassadar_module_overlap_resolution_report_path(),
        )
        .expect("committed report");

        assert_eq!(committed, expected);
    }
}
