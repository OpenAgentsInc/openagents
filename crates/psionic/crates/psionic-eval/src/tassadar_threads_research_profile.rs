use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TASSADAR_THREADS_RESEARCH_PROFILE_ID, TASSADAR_THREADS_RESEARCH_PROFILE_RUNTIME_REPORT_REF,
    TassadarThreadsResearchCaseStatus,
    TassadarThreadsResearchProfileReport as TassadarThreadsResearchRuntimeReport,
    build_tassadar_threads_research_profile_runtime_report,
};
use psionic_sandbox::{
    TASSADAR_THREADS_SCHEDULER_SANDBOX_BOUNDARY_REPORT_REF, TassadarThreadsSchedulerBoundaryStatus,
    TassadarThreadsSchedulerSandboxBoundaryReport,
    build_tassadar_threads_scheduler_sandbox_boundary_report,
};

pub const TASSADAR_THREADS_RESEARCH_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_threads_research_profile_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarThreadsResearchProfileCaseAudit {
    pub case_id: String,
    pub scheduler_id: String,
    pub workload_family: String,
    pub shared_memory_shape_id: String,
    pub memory_order_id: String,
    pub runtime_status: TassadarThreadsResearchCaseStatus,
    pub sandbox_status: TassadarThreadsSchedulerBoundaryStatus,
    pub exact_output_parity: bool,
    pub exact_schedule_replay: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub in_declared_scheduler_envelope: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarThreadsResearchProfileEvalReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub runtime_report_ref: String,
    pub runtime_report: TassadarThreadsResearchRuntimeReport,
    pub sandbox_boundary_report_ref: String,
    pub sandbox_boundary_report: TassadarThreadsSchedulerSandboxBoundaryReport,
    pub case_audits: Vec<TassadarThreadsResearchProfileCaseAudit>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub sandbox_negative_only_case_count: u32,
    pub green_scheduler_ids: Vec<String>,
    pub refused_scheduler_ids: Vec<String>,
    pub served_publication_allowed: bool,
    pub overall_green: bool,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarThreadsResearchProfileReportError {
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

#[must_use]
pub fn build_tassadar_threads_research_profile_report() -> TassadarThreadsResearchProfileEvalReport
{
    let runtime_report = build_tassadar_threads_research_profile_runtime_report();
    let sandbox_boundary_report = build_tassadar_threads_scheduler_sandbox_boundary_report();
    let sandbox_rows_by_case = sandbox_boundary_report
        .rows
        .iter()
        .map(|row| (row.case_id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let mut green_scheduler_ids = BTreeSet::new();
    let case_audits = runtime_report
        .rows
        .iter()
        .map(|row| {
            let sandbox_row = sandbox_rows_by_case
                .get(row.case_id.as_str())
                .expect("sandbox boundary should cover each runtime row");
            let in_declared_scheduler_envelope = matches!(
                (row.status, sandbox_row.status),
                (
                    TassadarThreadsResearchCaseStatus::ExactDeterministicParity,
                    TassadarThreadsSchedulerBoundaryStatus::AllowedDeterministic,
                ) | (
                    TassadarThreadsResearchCaseStatus::ExactRefusalParity,
                    TassadarThreadsSchedulerBoundaryStatus::RefusedOutOfEnvelope,
                )
            );
            if row.status == TassadarThreadsResearchCaseStatus::ExactDeterministicParity
                && sandbox_row.status
                    == TassadarThreadsSchedulerBoundaryStatus::AllowedDeterministic
            {
                green_scheduler_ids.insert(row.scheduler_id.clone());
            }
            TassadarThreadsResearchProfileCaseAudit {
                case_id: row.case_id.clone(),
                scheduler_id: row.scheduler_id.clone(),
                workload_family: row.workload_family.clone(),
                shared_memory_shape_id: row.shared_memory_shape_id.clone(),
                memory_order_id: sandbox_row.memory_order_id.clone(),
                runtime_status: row.status,
                sandbox_status: sandbox_row.status,
                exact_output_parity: row.exact_output_parity,
                exact_schedule_replay: row.exact_schedule_replay
                    && sandbox_row.exact_schedule_replay,
                refusal_reason_id: row
                    .refusal_reason_id
                    .clone()
                    .or_else(|| sandbox_row.refusal_reason_id.clone()),
                in_declared_scheduler_envelope,
                note: format!("{} {}", row.note, sandbox_row.note),
            }
        })
        .collect::<Vec<_>>();
    let exact_case_count = case_audits
        .iter()
        .filter(|case| {
            case.runtime_status == TassadarThreadsResearchCaseStatus::ExactDeterministicParity
                && case.in_declared_scheduler_envelope
        })
        .count() as u32;
    let refusal_case_count = case_audits
        .iter()
        .filter(|case| {
            case.runtime_status == TassadarThreadsResearchCaseStatus::ExactRefusalParity
                && case.sandbox_status
                    == TassadarThreadsSchedulerBoundaryStatus::RefusedOutOfEnvelope
        })
        .count() as u32;
    let runtime_case_ids = runtime_report
        .rows
        .iter()
        .map(|row| row.case_id.as_str())
        .collect::<BTreeSet<_>>();
    let sandbox_negative_only_case_count = sandbox_boundary_report
        .rows
        .iter()
        .filter(|row| !runtime_case_ids.contains(row.case_id.as_str()))
        .count() as u32;
    let refused_scheduler_ids = sandbox_boundary_report
        .rows
        .iter()
        .filter(|row| row.status == TassadarThreadsSchedulerBoundaryStatus::RefusedOutOfEnvelope)
        .map(|row| row.scheduler_id.clone())
        .collect::<BTreeSet<_>>();
    let mut generated_from_refs = vec![
        String::from(TASSADAR_THREADS_RESEARCH_PROFILE_RUNTIME_REPORT_REF),
        String::from(TASSADAR_THREADS_SCHEDULER_SANDBOX_BOUNDARY_REPORT_REF),
    ];
    for row in &runtime_report.rows {
        generated_from_refs.extend(row.benchmark_refs.iter().cloned());
    }
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let overall_green = case_audits
        .iter()
        .all(|case| case.in_declared_scheduler_envelope)
        && runtime_report
            .rows
            .iter()
            .all(|row| row.status != TassadarThreadsResearchCaseStatus::Drift)
        && exact_case_count == runtime_report.exact_case_count
        && refusal_case_count == runtime_report.refusal_case_count
        && sandbox_negative_only_case_count > 0;
    let mut report = TassadarThreadsResearchProfileEvalReport {
        schema_version: 1,
        report_id: String::from("tassadar.threads_research_profile.eval_report.v1"),
        profile_id: String::from(TASSADAR_THREADS_RESEARCH_PROFILE_ID),
        runtime_report_ref: String::from(TASSADAR_THREADS_RESEARCH_PROFILE_RUNTIME_REPORT_REF),
        runtime_report,
        sandbox_boundary_report_ref: String::from(
            TASSADAR_THREADS_SCHEDULER_SANDBOX_BOUNDARY_REPORT_REF,
        ),
        sandbox_boundary_report,
        case_audits,
        exact_case_count,
        refusal_case_count,
        sandbox_negative_only_case_count,
        green_scheduler_ids: green_scheduler_ids.into_iter().collect(),
        refused_scheduler_ids: refused_scheduler_ids.into_iter().collect(),
        served_publication_allowed: false,
        overall_green,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report keeps the shared-memory and threads lane research-only. A green report means one deterministic scheduler envelope and its typed refusal rows are benchmarked honestly; it does not create a served threads lane, arbitrary shared-memory closure, or general concurrency portability",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Threads research profile report covers exact_cases={}, refusal_cases={}, sandbox_negative_only_cases={}, green_schedulers={}, refused_schedulers={}, served_publication_allowed={}, overall_green={}.",
        report.exact_case_count,
        report.refusal_case_count,
        report.sandbox_negative_only_case_count,
        report.green_scheduler_ids.len(),
        report.refused_scheduler_ids.len(),
        report.served_publication_allowed,
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_threads_research_profile_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_threads_research_profile_report_path() -> PathBuf {
    repo_root().join(TASSADAR_THREADS_RESEARCH_PROFILE_REPORT_REF)
}

pub fn write_tassadar_threads_research_profile_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarThreadsResearchProfileEvalReport, TassadarThreadsResearchProfileReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarThreadsResearchProfileReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_threads_research_profile_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarThreadsResearchProfileReportError::Write {
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
) -> Result<T, TassadarThreadsResearchProfileReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarThreadsResearchProfileReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarThreadsResearchProfileReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_THREADS_RESEARCH_PROFILE_REPORT_REF, TassadarThreadsResearchProfileEvalReport,
        build_tassadar_threads_research_profile_report, read_repo_json,
        tassadar_threads_research_profile_report_path,
        write_tassadar_threads_research_profile_report,
    };

    #[test]
    fn threads_research_profile_keeps_runtime_and_sandbox_boundary_in_lockstep() {
        let report = build_tassadar_threads_research_profile_report();

        assert!(report.overall_green);
        assert_eq!(report.exact_case_count, 2);
        assert_eq!(report.refusal_case_count, 1);
        assert_eq!(report.sandbox_negative_only_case_count, 1);
        assert_eq!(
            report.green_scheduler_ids,
            vec![
                String::from("deterministic_barrier_release_v1"),
                String::from("deterministic_round_robin_v1"),
            ]
        );
        assert!(
            report
                .refused_scheduler_ids
                .contains(&String::from("host_nondeterministic_runtime"))
        );
        assert!(!report.served_publication_allowed);
    }

    #[test]
    fn threads_research_profile_report_matches_committed_truth() {
        let generated = build_tassadar_threads_research_profile_report();
        let committed: TassadarThreadsResearchProfileEvalReport =
            read_repo_json(TASSADAR_THREADS_RESEARCH_PROFILE_REPORT_REF).expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_threads_research_profile_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_threads_research_profile_report.json");
        let written =
            write_tassadar_threads_research_profile_report(&output_path).expect("write report");
        let persisted: TassadarThreadsResearchProfileEvalReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_threads_research_profile_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_threads_research_profile_report.json")
        );
    }
}
