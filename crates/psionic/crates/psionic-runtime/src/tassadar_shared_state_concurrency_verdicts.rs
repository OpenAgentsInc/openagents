use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    TASSADAR_THREADS_RESEARCH_PROFILE_ID, TASSADAR_THREADS_RESEARCH_PROFILE_RUNTIME_REPORT_REF,
    TassadarThreadsResearchCaseRow, TassadarThreadsResearchCaseStatus,
    TassadarThreadsResearchProfileReport, build_tassadar_threads_research_profile_runtime_report,
};

pub const TASSADAR_SHARED_STATE_CONCURRENCY_RUNTIME_VERDICT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_shared_state_concurrency_runtime_verdict_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedStateConcurrencyRuntimeStatus {
    OperatorDeterministicGreen,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedStateConcurrencyRuntimeRow {
    pub concurrency_class_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backing_profile_id: Option<String>,
    pub scheduler_scope_id: String,
    pub memory_order_scope_id: String,
    pub status: TassadarSharedStateConcurrencyRuntimeStatus,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedStateConcurrencyRuntimeVerdictReport {
    pub schema_version: u16,
    pub report_id: String,
    pub threads_runtime_report_ref: String,
    pub threads_runtime_report: TassadarThreadsResearchProfileReport,
    pub operator_green_class_ids: Vec<String>,
    pub public_suppressed_profile_ids: Vec<String>,
    pub refused_class_ids: Vec<String>,
    pub rows: Vec<TassadarSharedStateConcurrencyRuntimeRow>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[must_use]
pub fn build_tassadar_shared_state_concurrency_runtime_verdict_report()
-> TassadarSharedStateConcurrencyRuntimeVerdictReport {
    let threads_runtime_report = build_tassadar_threads_research_profile_runtime_report();
    let round_robin = runtime_case(&threads_runtime_report, "round_robin_shared_counter");
    let barrier_reduce = runtime_case(&threads_runtime_report, "barrier_then_reduce");
    let relaxed_refusal = runtime_case(&threads_runtime_report, "relaxed_memory_order_refusal");
    let rows = vec![
        TassadarSharedStateConcurrencyRuntimeRow {
            concurrency_class_id: String::from("single_host_round_robin_shared_counter"),
            backing_profile_id: Some(String::from(TASSADAR_THREADS_RESEARCH_PROFILE_ID)),
            scheduler_scope_id: round_robin.scheduler_id.clone(),
            memory_order_scope_id: String::from("seq_cst_explicit_order"),
            status: TassadarSharedStateConcurrencyRuntimeStatus::OperatorDeterministicGreen,
            exact_case_count: u32::from(
                round_robin.status == TassadarThreadsResearchCaseStatus::ExactDeterministicParity,
            ),
            refusal_case_count: 0,
            refusal_reason_id: None,
            benchmark_refs: round_robin.benchmark_refs.clone(),
            note: String::from(
                "single-host shared-counter replay stays operator-green only inside the explicit deterministic scheduler envelope",
            ),
        },
        TassadarSharedStateConcurrencyRuntimeRow {
            concurrency_class_id: String::from("single_host_barrier_reduce"),
            backing_profile_id: Some(String::from(TASSADAR_THREADS_RESEARCH_PROFILE_ID)),
            scheduler_scope_id: barrier_reduce.scheduler_id.clone(),
            memory_order_scope_id: String::from("seq_cst_barrier_release"),
            status: TassadarSharedStateConcurrencyRuntimeStatus::OperatorDeterministicGreen,
            exact_case_count: u32::from(
                barrier_reduce.status
                    == TassadarThreadsResearchCaseStatus::ExactDeterministicParity,
            ),
            refusal_case_count: 0,
            refusal_reason_id: None,
            benchmark_refs: barrier_reduce.benchmark_refs.clone(),
            note: String::from(
                "barrier-then-reduce replay stays operator-green only for the bounded two-thread deterministic scheduler envelope",
            ),
        },
        TassadarSharedStateConcurrencyRuntimeRow {
            concurrency_class_id: String::from("host_nondeterministic_shared_counter"),
            backing_profile_id: None,
            scheduler_scope_id: String::from("host_nondeterministic_runtime"),
            memory_order_scope_id: String::from("implicit_interleaving"),
            status: TassadarSharedStateConcurrencyRuntimeStatus::Refused,
            exact_case_count: 0,
            refusal_case_count: 1,
            refusal_reason_id: Some(String::from("host_nondeterministic_scheduler")),
            benchmark_refs: round_robin.benchmark_refs.clone(),
            note: String::from(
                "host-nondeterministic scheduling remains explicit refusal truth and cannot inherit the deterministic shared-state lane",
            ),
        },
        TassadarSharedStateConcurrencyRuntimeRow {
            concurrency_class_id: String::from("relaxed_memory_order_shared_counter"),
            backing_profile_id: None,
            scheduler_scope_id: relaxed_refusal.scheduler_id.clone(),
            memory_order_scope_id: String::from("relaxed_shared_memory_order"),
            status: TassadarSharedStateConcurrencyRuntimeStatus::Refused,
            exact_case_count: 0,
            refusal_case_count: u32::from(
                relaxed_refusal.status == TassadarThreadsResearchCaseStatus::ExactRefusalParity,
            ),
            refusal_reason_id: Some(String::from("unsupported_memory_ordering")),
            benchmark_refs: relaxed_refusal.benchmark_refs.clone(),
            note: String::from(
                "relaxed shared-memory ordering remains explicit refusal truth instead of becoming a widened deterministic profile claim",
            ),
        },
        TassadarSharedStateConcurrencyRuntimeRow {
            concurrency_class_id: String::from("cross_worker_shared_heap_replication"),
            backing_profile_id: None,
            scheduler_scope_id: String::from("cluster_shared_state_fanout"),
            memory_order_scope_id: String::from("cross_worker_shared_heap"),
            status: TassadarSharedStateConcurrencyRuntimeStatus::Refused,
            exact_case_count: 0,
            refusal_case_count: 1,
            refusal_reason_id: Some(String::from("cross_worker_shared_state_unfrozen")),
            benchmark_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_linked_program_bundle_report.json",
            )],
            note: String::from(
                "cross-worker shared-state replication remains refused because the repo has linked-program evidence for the gap but not a frozen deterministic shared-heap claim",
            ),
        },
    ];
    let operator_green_class_ids = rows
        .iter()
        .filter(|row| {
            row.status == TassadarSharedStateConcurrencyRuntimeStatus::OperatorDeterministicGreen
        })
        .map(|row| row.concurrency_class_id.clone())
        .collect::<Vec<_>>();
    let refused_class_ids = rows
        .iter()
        .filter(|row| row.status == TassadarSharedStateConcurrencyRuntimeStatus::Refused)
        .map(|row| row.concurrency_class_id.clone())
        .collect::<Vec<_>>();
    let public_suppressed_profile_ids = vec![String::from(TASSADAR_THREADS_RESEARCH_PROFILE_ID)];
    let mut report = TassadarSharedStateConcurrencyRuntimeVerdictReport {
        schema_version: 1,
        report_id: String::from("tassadar.shared_state_concurrency.runtime_verdict_report.v1"),
        threads_runtime_report_ref: String::from(
            TASSADAR_THREADS_RESEARCH_PROFILE_RUNTIME_REPORT_REF,
        ),
        threads_runtime_report,
        operator_green_class_ids,
        public_suppressed_profile_ids,
        refused_class_ids,
        rows,
        overall_green: true,
        claim_boundary: String::from(
            "this runtime report keeps shared-state concurrency on a narrow operator-truth lane. Only the explicit single-host deterministic scheduler rows are operator-green; host-nondeterministic scheduling, relaxed memory ordering, and cross-worker shared heaps stay refused. Nothing here creates a served threads lane, broad concurrency closure, or public shared-state promotion",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.operator_green_class_ids.len() == 2
        && report.refused_class_ids.len() == 3
        && report.public_suppressed_profile_ids
            == vec![String::from(TASSADAR_THREADS_RESEARCH_PROFILE_ID)];
    report.summary = format!(
        "Shared-state concurrency runtime verdict report covers operator_green_classes={}, refused_classes={}, public_suppressed_profiles={}, overall_green={}.",
        report.operator_green_class_ids.len(),
        report.refused_class_ids.len(),
        report.public_suppressed_profile_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_shared_state_concurrency_runtime_verdict_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_shared_state_concurrency_runtime_verdict_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SHARED_STATE_CONCURRENCY_RUNTIME_VERDICT_REPORT_REF)
}

pub fn write_tassadar_shared_state_concurrency_runtime_verdict_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSharedStateConcurrencyRuntimeVerdictReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_shared_state_concurrency_runtime_verdict_report();
    let json = serde_json::to_string_pretty(&report).expect("report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

fn runtime_case<'a>(
    report: &'a TassadarThreadsResearchProfileReport,
    case_id: &str,
) -> &'a TassadarThreadsResearchCaseRow {
    report
        .rows
        .iter()
        .find(|row| row.case_id == case_id)
        .expect("seeded runtime row should exist")
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

#[cfg(test)]
fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Box<dyn std::error::Error>> {
    let path = path.as_ref();
    let json = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        TassadarSharedStateConcurrencyRuntimeStatus,
        TassadarSharedStateConcurrencyRuntimeVerdictReport,
        build_tassadar_shared_state_concurrency_runtime_verdict_report, read_json,
        tassadar_shared_state_concurrency_runtime_verdict_report_path,
        write_tassadar_shared_state_concurrency_runtime_verdict_report,
    };

    #[test]
    fn shared_state_concurrency_runtime_verdict_report_keeps_operator_and_refusal_classes_explicit()
    {
        let report = build_tassadar_shared_state_concurrency_runtime_verdict_report();

        assert!(report.overall_green);
        assert_eq!(report.operator_green_class_ids.len(), 2);
        assert_eq!(report.refused_class_ids.len(), 3);
        assert_eq!(report.public_suppressed_profile_ids.len(), 1);
        assert!(report.rows.iter().any(|row| {
            row.status == TassadarSharedStateConcurrencyRuntimeStatus::OperatorDeterministicGreen
                && row.concurrency_class_id == "single_host_round_robin_shared_counter"
        }));
    }

    #[test]
    fn shared_state_concurrency_runtime_verdict_report_matches_committed_truth() {
        let generated = build_tassadar_shared_state_concurrency_runtime_verdict_report();
        let committed: TassadarSharedStateConcurrencyRuntimeVerdictReport =
            read_json(tassadar_shared_state_concurrency_runtime_verdict_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_shared_state_concurrency_runtime_verdict_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_shared_state_concurrency_runtime_verdict_report.json");
        let report = write_tassadar_shared_state_concurrency_runtime_verdict_report(&output_path)
            .expect("write report");
        let persisted: TassadarSharedStateConcurrencyRuntimeVerdictReport =
            read_json(&output_path).expect("persisted report");
        assert_eq!(report, persisted);
    }
}
