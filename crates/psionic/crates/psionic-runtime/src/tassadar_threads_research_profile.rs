use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_THREADS_RESEARCH_PROFILE_ID: &str =
    "tassadar.research_profile.threads_deterministic_scheduler.v1";
pub const TASSADAR_THREADS_RESEARCH_PROFILE_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_threads_research_profile_runtime_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarThreadsResearchCaseStatus {
    ExactDeterministicParity,
    ExactRefusalParity,
    Drift,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarThreadsResearchCaseRow {
    pub case_id: String,
    pub scheduler_id: String,
    pub workload_family: String,
    pub shared_memory_shape_id: String,
    pub status: TassadarThreadsResearchCaseStatus,
    pub exact_output_parity: bool,
    pub exact_schedule_replay: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarThreadsResearchProfileReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub rows: Vec<TassadarThreadsResearchCaseRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[must_use]
pub fn build_tassadar_threads_research_profile_runtime_report()
-> TassadarThreadsResearchProfileReport {
    let rows = vec![
        row(
            "round_robin_shared_counter",
            "deterministic_round_robin_v1",
            "shared_counter_increment",
            "two_thread_shared_i32",
            TassadarThreadsResearchCaseStatus::ExactDeterministicParity,
            true,
            true,
            None,
            &["fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json"],
            "two-thread round-robin shared-counter stepping remains replay-safe only inside the declared deterministic scheduler envelope",
        ),
        row(
            "barrier_then_reduce",
            "deterministic_barrier_release_v1",
            "barrier_reduce_kernel",
            "two_thread_shared_slice_i32",
            TassadarThreadsResearchCaseStatus::ExactDeterministicParity,
            true,
            true,
            None,
            &["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
            "barrier release plus reduce remains exact only when the scheduler order is explicit and fixed",
        ),
        row(
            "relaxed_memory_order_refusal",
            "out_of_profile_relaxed_order",
            "shared_counter_increment",
            "two_thread_shared_i32",
            TassadarThreadsResearchCaseStatus::ExactRefusalParity,
            false,
            false,
            Some("unsupported_memory_ordering"),
            &["fixtures/tassadar/reports/tassadar_frozen_core_wasm_window_report.json"],
            "relaxed or nondeterministic memory ordering stays typed refusal truth instead of widening from the deterministic research rows",
        ),
    ];
    let exact_case_count = rows
        .iter()
        .filter(|row| row.status == TassadarThreadsResearchCaseStatus::ExactDeterministicParity)
        .count() as u32;
    let refusal_case_count = rows
        .iter()
        .filter(|row| row.status == TassadarThreadsResearchCaseStatus::ExactRefusalParity)
        .count() as u32;
    let mut report = TassadarThreadsResearchProfileReport {
        schema_version: 1,
        report_id: String::from("tassadar.threads_research_profile.report.v1"),
        profile_id: String::from(TASSADAR_THREADS_RESEARCH_PROFILE_ID),
        exact_case_count,
        refusal_case_count,
        rows,
        claim_boundary: String::from(
            "this runtime report freezes one research-only shared-memory and threads profile with a deterministic scheduler envelope plus typed refusal on broader memory-ordering regimes. It does not claim general concurrency closure, broad shared-memory portability, or served publication",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Threads research profile report covers {} rows with exact_cases={} and refusal_cases={}.",
        report.rows.len(),
        report.exact_case_count,
        report.refusal_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_threads_research_profile_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_threads_research_profile_report_path() -> PathBuf {
    repo_root().join(TASSADAR_THREADS_RESEARCH_PROFILE_RUNTIME_REPORT_REF)
}

pub fn write_tassadar_threads_research_profile_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarThreadsResearchProfileReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_threads_research_profile_runtime_report();
    let json = serde_json::to_string_pretty(&report).expect("threads report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_threads_research_profile_runtime_report(
    path: impl AsRef<Path>,
) -> Result<TassadarThreadsResearchProfileReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

#[allow(clippy::too_many_arguments)]
fn row(
    case_id: &str,
    scheduler_id: &str,
    workload_family: &str,
    shared_memory_shape_id: &str,
    status: TassadarThreadsResearchCaseStatus,
    exact_output_parity: bool,
    exact_schedule_replay: bool,
    refusal_reason_id: Option<&str>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarThreadsResearchCaseRow {
    TassadarThreadsResearchCaseRow {
        case_id: String::from(case_id),
        scheduler_id: String::from(scheduler_id),
        workload_family: String::from(workload_family),
        shared_memory_shape_id: String::from(shared_memory_shape_id),
        status,
        exact_output_parity,
        exact_schedule_replay,
        refusal_reason_id: refusal_reason_id.map(String::from),
        benchmark_refs: benchmark_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
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
        TASSADAR_THREADS_RESEARCH_PROFILE_ID,
        build_tassadar_threads_research_profile_runtime_report,
        load_tassadar_threads_research_profile_runtime_report,
        tassadar_threads_research_profile_report_path,
        write_tassadar_threads_research_profile_runtime_report,
    };

    #[test]
    fn threads_research_profile_runtime_report_keeps_deterministic_and_refusal_rows_explicit() {
        let report = build_tassadar_threads_research_profile_runtime_report();

        assert_eq!(report.profile_id, TASSADAR_THREADS_RESEARCH_PROFILE_ID);
        assert_eq!(report.exact_case_count, 2);
        assert_eq!(report.refusal_case_count, 1);
    }

    #[test]
    fn threads_research_profile_runtime_report_matches_committed_truth() {
        let generated = build_tassadar_threads_research_profile_runtime_report();
        let committed = load_tassadar_threads_research_profile_runtime_report(
            tassadar_threads_research_profile_report_path(),
        )
        .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_threads_research_profile_runtime_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_threads_research_profile_runtime_report.json");
        let report = write_tassadar_threads_research_profile_runtime_report(&output_path)
            .expect("write report");
        let persisted = load_tassadar_threads_research_profile_runtime_report(&output_path)
            .expect("persisted report");
        assert_eq!(report, persisted);
    }
}
