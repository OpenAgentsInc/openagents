use std::cmp::Ordering;

use chrono::DateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RuntimeCodexWorkerCandidate {
    pub worker_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub adapter: Option<String>,
    #[serde(default)]
    pub heartbeat_state: Option<String>,
    #[serde(default)]
    pub last_heartbeat_at: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub latest_seq: i64,
    #[serde(default)]
    pub metadata_source: Option<String>,
}

pub fn select_preferred_worker_id(workers: &[RuntimeCodexWorkerCandidate]) -> Option<String> {
    pool_for_selection(workers)
        .into_iter()
        .max_by(compare_workers)
        .map(|worker| worker.worker_id.clone())
}

fn pool_for_selection(
    workers: &[RuntimeCodexWorkerCandidate],
) -> Vec<&RuntimeCodexWorkerCandidate> {
    if workers.is_empty() {
        return Vec::new();
    }

    let running: Vec<&RuntimeCodexWorkerCandidate> = workers
        .iter()
        .filter(|worker| worker.status == "running")
        .collect();
    let desktop_running: Vec<&RuntimeCodexWorkerCandidate> = running
        .iter()
        .copied()
        .filter(|worker| is_desktop_worker(worker))
        .collect();

    if !desktop_running.is_empty() {
        return desktop_running;
    }

    if !running.is_empty() {
        return running;
    }

    workers.iter().collect()
}

fn compare_workers(
    lhs: &&RuntimeCodexWorkerCandidate,
    rhs: &&RuntimeCodexWorkerCandidate,
) -> Ordering {
    let lhs_key = worker_key(lhs);
    let rhs_key = worker_key(rhs);
    lhs_key.cmp(&rhs_key)
}

fn worker_key(
    worker: &RuntimeCodexWorkerCandidate,
) -> (i32, i32, Option<i64>, Option<i64>, i64, &str) {
    (
        shared_worker_rank(worker),
        freshness_rank(worker),
        timestamp_millis(worker.last_heartbeat_at.as_deref()),
        timestamp_millis(worker.started_at.as_deref()),
        worker.latest_seq,
        worker.worker_id.as_str(),
    )
}

fn is_desktop_worker(worker: &RuntimeCodexWorkerCandidate) -> bool {
    if worker.adapter.as_deref() == Some("desktop_bridge") {
        return true;
    }

    if worker.worker_id.starts_with("desktopw:") {
        return true;
    }

    worker.metadata_source.as_deref() == Some("autopilot-desktop")
}

fn freshness_rank(worker: &RuntimeCodexWorkerCandidate) -> i32 {
    match worker
        .heartbeat_state
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("fresh") => 2,
        Some("stale") => 1,
        _ => 0,
    }
}

fn shared_worker_rank(worker: &RuntimeCodexWorkerCandidate) -> i32 {
    if worker.worker_id.contains(":shared") {
        1
    } else {
        0
    }
}

fn timestamp_millis(raw: Option<&str>) -> Option<i64> {
    let value = raw?.trim();
    if value.is_empty() {
        return None;
    }

    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::{RuntimeCodexWorkerCandidate, select_preferred_worker_id};

    #[test]
    fn prefers_desktop_shared_running_worker_when_available() {
        let workers = vec![
            RuntimeCodexWorkerCandidate {
                worker_id: "runtimew:shared".to_string(),
                status: "running".to_string(),
                adapter: Some("runtime".to_string()),
                heartbeat_state: Some("fresh".to_string()),
                last_heartbeat_at: Some("2026-02-22T05:00:00Z".to_string()),
                started_at: Some("2026-02-22T04:00:00Z".to_string()),
                latest_seq: 90,
                metadata_source: Some("runtime".to_string()),
            },
            RuntimeCodexWorkerCandidate {
                worker_id: "desktopw:alpha".to_string(),
                status: "running".to_string(),
                adapter: Some("desktop_bridge".to_string()),
                heartbeat_state: Some("fresh".to_string()),
                last_heartbeat_at: Some("2026-02-22T05:01:00Z".to_string()),
                started_at: Some("2026-02-22T04:00:30Z".to_string()),
                latest_seq: 30,
                metadata_source: Some("autopilot-desktop".to_string()),
            },
            RuntimeCodexWorkerCandidate {
                worker_id: "desktopw:shared".to_string(),
                status: "running".to_string(),
                adapter: Some("desktop_bridge".to_string()),
                heartbeat_state: Some("stale".to_string()),
                last_heartbeat_at: Some("2026-02-22T04:59:00Z".to_string()),
                started_at: Some("2026-02-22T03:59:00Z".to_string()),
                latest_seq: 22,
                metadata_source: Some("autopilot-desktop".to_string()),
            },
        ];

        assert_eq!(
            select_preferred_worker_id(&workers).as_deref(),
            Some("desktopw:shared")
        );
    }

    #[test]
    fn falls_back_to_running_workers_when_no_desktop_worker_is_running() {
        let workers = vec![
            RuntimeCodexWorkerCandidate {
                worker_id: "desktopw:shared".to_string(),
                status: "stopped".to_string(),
                adapter: Some("desktop_bridge".to_string()),
                heartbeat_state: Some("stale".to_string()),
                last_heartbeat_at: Some("2026-02-22T04:55:00Z".to_string()),
                started_at: Some("2026-02-22T04:00:00Z".to_string()),
                latest_seq: 1,
                metadata_source: Some("autopilot-desktop".to_string()),
            },
            RuntimeCodexWorkerCandidate {
                worker_id: "runtimew:b".to_string(),
                status: "running".to_string(),
                adapter: Some("runtime".to_string()),
                heartbeat_state: Some("fresh".to_string()),
                last_heartbeat_at: Some("2026-02-22T04:58:00Z".to_string()),
                started_at: Some("2026-02-22T04:00:00Z".to_string()),
                latest_seq: 9,
                metadata_source: Some("runtime".to_string()),
            },
            RuntimeCodexWorkerCandidate {
                worker_id: "runtimew:a".to_string(),
                status: "running".to_string(),
                adapter: Some("runtime".to_string()),
                heartbeat_state: Some("fresh".to_string()),
                last_heartbeat_at: Some("2026-02-22T04:59:00Z".to_string()),
                started_at: Some("2026-02-22T04:00:00Z".to_string()),
                latest_seq: 9,
                metadata_source: Some("runtime".to_string()),
            },
        ];

        assert_eq!(
            select_preferred_worker_id(&workers).as_deref(),
            Some("runtimew:a")
        );
    }

    #[test]
    fn falls_back_to_all_workers_when_none_running() {
        let workers = vec![
            RuntimeCodexWorkerCandidate {
                worker_id: "desktopw:shared".to_string(),
                status: "stopped".to_string(),
                adapter: Some("desktop_bridge".to_string()),
                heartbeat_state: Some("stale".to_string()),
                last_heartbeat_at: Some("2026-02-22T04:58:00Z".to_string()),
                started_at: Some("2026-02-22T04:00:00Z".to_string()),
                latest_seq: 5,
                metadata_source: Some("autopilot-desktop".to_string()),
            },
            RuntimeCodexWorkerCandidate {
                worker_id: "desktopw:alpha".to_string(),
                status: "stopped".to_string(),
                adapter: Some("desktop_bridge".to_string()),
                heartbeat_state: Some("fresh".to_string()),
                last_heartbeat_at: Some("2026-02-22T05:00:00Z".to_string()),
                started_at: Some("2026-02-22T04:05:00Z".to_string()),
                latest_seq: 6,
                metadata_source: Some("autopilot-desktop".to_string()),
            },
        ];

        assert_eq!(
            select_preferred_worker_id(&workers).as_deref(),
            Some("desktopw:shared")
        );
    }

    #[test]
    fn breaks_ties_by_heartbeat_then_started_then_seq_then_worker_id() {
        let workers = vec![
            RuntimeCodexWorkerCandidate {
                worker_id: "desktopw:aaa".to_string(),
                status: "running".to_string(),
                adapter: Some("desktop_bridge".to_string()),
                heartbeat_state: Some("fresh".to_string()),
                last_heartbeat_at: Some("2026-02-22T05:00:00Z".to_string()),
                started_at: Some("2026-02-22T04:00:00Z".to_string()),
                latest_seq: 5,
                metadata_source: Some("autopilot-desktop".to_string()),
            },
            RuntimeCodexWorkerCandidate {
                worker_id: "desktopw:bbb".to_string(),
                status: "running".to_string(),
                adapter: Some("desktop_bridge".to_string()),
                heartbeat_state: Some("fresh".to_string()),
                last_heartbeat_at: Some("2026-02-22T05:00:01Z".to_string()),
                started_at: Some("2026-02-22T04:00:00Z".to_string()),
                latest_seq: 4,
                metadata_source: Some("autopilot-desktop".to_string()),
            },
            RuntimeCodexWorkerCandidate {
                worker_id: "desktopw:ccc".to_string(),
                status: "running".to_string(),
                adapter: Some("desktop_bridge".to_string()),
                heartbeat_state: Some("fresh".to_string()),
                last_heartbeat_at: Some("2026-02-22T05:00:01Z".to_string()),
                started_at: Some("2026-02-22T04:00:01Z".to_string()),
                latest_seq: 3,
                metadata_source: Some("autopilot-desktop".to_string()),
            },
        ];

        assert_eq!(
            select_preferred_worker_id(&workers).as_deref(),
            Some("desktopw:ccc")
        );
    }
}
