//! Frozen `openagents.gym.*` schema v1.
//!
//! These documents are the rendering contract for the terminal, the coder TUI,
//! and any web surface that consumes Gym output. Each JSON object begins with a
//! `schema` field naming its document type and version.

use serde::{Deserialize, Serialize};

/// Schema name carried in every `suite_manifest_view` document.
pub const SUITE_MANIFEST_VIEW_SCHEMA: &str = "openagents.gym.suite_manifest_view.v1";

/// Schema name carried in every `env_report` document.
pub const ENV_REPORT_SCHEMA: &str = "openagents.gym.env_report.v1";

/// Schema name carried in every `run_status` document.
pub const RUN_STATUS_SCHEMA: &str = "openagents.gym.run_status.v1";

/// Schema name carried in every `results_trend` document.
pub const RESULTS_TREND_SCHEMA: &str = "openagents.gym.results_trend.v1";

/// Schema name carried in every corpus import ledger row.
pub const CORPUS_IMPORT_RECORD_SCHEMA: &str = "openagents.gym.corpus_import_record.v1";

/// `openagents.gym.suite_manifest_view.v1`
///
/// A list/show view over a pinned effectiveness suite. The manifest itself
/// stays `openagents.effectiveness_suite.v1`; this view adds the digest and a
/// compact task list that every renderer can draw from.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct SuiteManifestView {
    pub schema: String,
    pub suite_id: String,
    pub suite_digest: String,
    pub tier: String,
    pub description: String,
    pub task_count: u64,
    pub source_path: Option<String>,
    pub tasks: Vec<SuiteManifestViewTask>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct SuiteManifestViewTask {
    pub id: String,
    pub task_digest: String,
    pub environment_available: bool,
    pub rationale: Option<String>,
    pub pin: SuiteTaskPin,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "snake_case"
)]
pub enum SuiteTaskPin {
    HarborRegistry {
        dataset: String,
        git_url: String,
        commit: String,
        path: String,
    },
    TrackerClosedIssue {
        repo: String,
        issue: u64,
        accepted_commit: String,
    },
}

/// `openagents.gym.run_status.v1`
///
/// Live progress and finalization state for one Gym run. The trial list carries
/// the same task states the terminal prints: `accepted`, `rejected`, `ungraded`,
/// and the running/pending states before a verdict is known.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct RunStatus {
    pub schema: String,
    pub run_id: String,
    pub suite_id: String,
    pub lane: String,
    pub model: Option<String>,
    pub state: String,
    pub started_at: Option<String>,
    pub updated_at: Option<String>,
    pub tasks_total: u64,
    pub accepted: u64,
    pub rejected: u64,
    pub ungraded: u64,
    pub graded: u64,
    pub summary: String,
    pub trials: Vec<RunTrial>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct RunTrial {
    pub task: String,
    pub state: String,
    pub outcome: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub transcript_ref: Option<String>,
    pub cost_usd: Option<f64>,
}

/// `openagents.gym.corpus_inventory.v1`
///
/// The catalog of local trace sessions that may feed the corpus. One row per
/// session, with exclusion reasons when a row does not qualify.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CorpusInventory {
    pub schema: String,
    pub generated_at: String,
    pub row_count: u64,
    pub rows: Vec<CorpusInventoryRow>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CorpusInventoryRow {
    pub source: String,
    pub path: String,
    pub digest: String,
    pub bytes: u64,
    pub steps_est: u64,
    pub started_at: String,
    pub ended_at: String,
    pub model: Option<String>,
    pub repo_hint: Option<String>,
    pub domain: Option<String>,
    pub qualifies: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded_because: Option<Vec<String>>,
}

/// `openagents.gym.corpus_import_record.v1`
///
/// One row of the corpus ledger: a redacted, digest-pinned trace that has been
/// uploaded and recorded.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CorpusImportRecord {
    pub schema: String,
    pub digest: String,
    pub trace_uuid: String,
    pub source: String,
    pub domain: String,
    pub visibility: String,
    pub recorded_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    /// The local file this trace was produced from, recorded so `corpus verify`
    /// can re-hash it later. Absent on rows imported before it existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<std::path::PathBuf>,
    /// Digest of the pre-redaction local text (native ATIF or converted
    /// document) — the value re-hashing the source can reproduce.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_digest: Option<String>,
}

/// `openagents.gym.dataset_view.v1`
///
/// A named, versioned group of trace and task references with append-recorded
/// provenance.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct DatasetView {
    pub schema: String,
    pub dataset_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub version: u64,
    pub created_at: String,
    pub updated_at: String,
    pub members: Vec<DatasetMember>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct DatasetMember {
    #[serde(rename = "ref")]
    pub reference: String,
    pub kind: String,
    pub added_by: String,
    pub added_at: String,
    pub reason: String,
    pub tags: Vec<String>,
}

/// `openagents.gym.env_report.v1`
///
/// The result of `gym env probe` or `gym env doctor`: a checklist of the
/// conditions a scored run needs, plus the remedy for any failed check.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct EnvReport {
    pub schema: String,
    pub target: String,
    pub sufficient_for_scored_run: bool,
    pub generated_at: String,
    pub checks: Vec<EnvCheck>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct EnvCheck {
    pub name: String,
    pub passed: bool,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remedy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed: Option<String>,
}

/// `openagents.gym.results_trend.v1`
///
/// The combined output of `gym results compare` and `gym results trend`: lane
/// comparisons, trend steps, and the confounders that affect how each delta may
/// be read. Unknown numbers are `null`, not a fabricated zero.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ResultsTrend {
    pub schema: String,
    pub suite_id: String,
    pub suite_key: String,
    pub verified: bool,
    pub lane_comparisons: Vec<LaneComparison>,
    pub trends: Vec<Trend>,
    pub isolated_groups: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct LaneComparison {
    pub suite_key: String,
    pub suite_id: String,
    pub tasks: Vec<String>,
    pub baseline_lane: String,
    pub lanes: Vec<LaneRow>,
    pub confounders: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct LaneRow {
    pub lane: String,
    pub run_digest: String,
    pub recorded_at: String,
    pub cost_per_accepted_outcome_usd: Option<f64>,
    pub success_rate: Option<f64>,
    pub cost_delta: Option<Delta>,
    pub success_rate_delta: Option<Delta>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Trend {
    pub suite_key: String,
    pub suite_id: String,
    pub lane: String,
    pub steps: Vec<TrendStep>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct TrendStep {
    pub from_recorded_at: String,
    pub to_recorded_at: String,
    pub cost_delta: Delta,
    pub success_rate_delta: Delta,
    pub confounders: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absolute: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative: Option<f64>,
    pub direction: String,
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gym")
    }

    fn check_or_update<T: serde::Serialize>(name: &str, value: &T) {
        let dir = fixture_dir();
        let _ = std::fs::create_dir_all(&dir);
        let json_path = dir.join(format!("{name}.golden.json"));
        let md_path = dir.join(format!("{name}.example.md"));

        let json = serde_json::to_string(value).unwrap();
        let pretty = serde_json::to_string_pretty(value).unwrap();
        let md = format!("# `{name}`\n\n```json\n{pretty}\n```\n");

        if std::env::var("UPDATE_GYM_GOLDEN").is_ok() {
            std::fs::write(&json_path, json).unwrap();
            std::fs::write(&md_path, md).unwrap();
            return;
        }

        let raw = std::fs::read_to_string(&json_path).expect("the golden file is checked in");
        assert_eq!(raw, json, "{name} golden fixture drifted");
    }

    #[test]
    fn suite_manifest_view_golden() {
        let value = SuiteManifestView {
            schema: "openagents.gym.suite_manifest_view.v1".into(),
            suite_id: "tb2-quick".into(),
            suite_digest:
                "suite-manifest:0c0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f"
                    .into(),
            tier: "score".into(),
            description: "Two quick Terminal-Bench 2.0 tasks".into(),
            task_count: 2,
            source_path: Some("bench/suites/tb2-quick.suite.json".into()),
            tasks: vec![
                SuiteManifestViewTask {
                    id: "regex-log".into(),
                    task_digest:
                        "task:a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1".into(),
                    environment_available: true,
                    rationale: Some("near-zero tool surface".into()),
                    pin: SuiteTaskPin::HarborRegistry {
                        dataset: "terminal-bench@2.0".into(),
                        git_url: "https://github.com/laude-institute/terminal-bench-2.git".into(),
                        commit: "69671fbaac6d67a7ef0dfec016cc38a64ef7a77c".into(),
                        path: "regex-log".into(),
                    },
                },
                SuiteManifestViewTask {
                    id: "openssl-selfsigned-cert".into(),
                    task_digest:
                        "task:b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2".into(),
                    environment_available: true,
                    rationale: Some("fully specified checklist".into()),
                    pin: SuiteTaskPin::HarborRegistry {
                        dataset: "terminal-bench@2.0".into(),
                        git_url: "https://github.com/laude-institute/terminal-bench-2.git".into(),
                        commit: "69671fbaac6d67a7ef0dfec016cc38a64ef7a77c".into(),
                        path: "openssl-selfsigned-cert".into(),
                    },
                },
            ],
        };
        check_or_update("suite_manifest_view", &value);
    }

    #[test]
    fn run_status_golden() {
        let value = RunStatus {
            schema: "openagents.gym.run_status.v1".into(),
            run_id: "run-2026-001".into(),
            suite_id: "tb2-quick".into(),
            lane: "proxy".into(),
            model: Some("openai/gpt-5.6-luna".into()),
            state: "graded".into(),
            started_at: Some("2026-08-27T10:00:00Z".into()),
            updated_at: Some("2026-08-27T10:05:12Z".into()),
            tasks_total: 2,
            accepted: 1,
            rejected: 0,
            ungraded: 1,
            graded: 1,
            summary: "1 accepted, 0 rejected, 1 ungraded; 1 of 2 tasks graded".into(),
            trials: vec![
                RunTrial {
                    task: "regex-log".into(),
                    state: "accepted".into(),
                    outcome: Some("accepted".into()),
                    started_at: Some("2026-08-27T10:00:01Z".into()),
                    finished_at: Some("2026-08-27T10:02:30Z".into()),
                    transcript_ref: Some("/trace/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into()),
                    cost_usd: Some(0.0042),
                },
                RunTrial {
                    task: "openssl-selfsigned-cert".into(),
                    state: "ungraded".into(),
                    outcome: None,
                    started_at: Some("2026-08-27T10:00:01Z".into()),
                    finished_at: Some("2026-08-27T10:05:12Z".into()),
                    transcript_ref: Some("/trace/bbbbbbbb-cccc-dddd-eeee-ffffffffffff".into()),
                    cost_usd: None,
                },
            ],
        };
        check_or_update("run_status", &value);
    }

    #[test]
    fn corpus_inventory_golden() {
        let value = CorpusInventory {
            schema: "openagents.gym.corpus_inventory.v1".into(),
            generated_at: "2026-08-27T09:00:00Z".into(),
            row_count: 2,
            rows: vec![
                CorpusInventoryRow {
                    source: "openagents".into(),
                    path: "/Users/example/.openagents/exports/2026-08-26.json".into(),
                    digest:
                        "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                            .into(),
                    bytes: 1048576,
                    steps_est: 42,
                    started_at: "2026-08-26T14:00:00Z".into(),
                    ended_at: "2026-08-26T14:05:00Z".into(),
                    model: Some("openai/gpt-5.6-luna".into()),
                    repo_hint: Some("openagents".into()),
                    domain: Some("agent-building".into()),
                    qualifies: true,
                    excluded_because: None,
                },
                CorpusInventoryRow {
                    source: "codex".into(),
                    path: "/Users/example/.codex/sessions/2026-08-25.jsonl".into(),
                    digest:
                        "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                            .into(),
                    bytes: 2048,
                    steps_est: 3,
                    started_at: "2026-08-25T10:00:00Z".into(),
                    ended_at: "2026-08-25T10:01:00Z".into(),
                    model: None,
                    repo_hint: None,
                    domain: None,
                    qualifies: false,
                    excluded_because: Some(vec![
                        "too_short".into(),
                        "no_redactable_content".into(),
                    ]),
                },
            ],
        };
        check_or_update("corpus_inventory", &value);
    }

    #[test]
    fn corpus_import_record_golden() {
        let value = CorpusImportRecord {
            schema: "openagents.gym.corpus_import_record.v1".into(),
            digest: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                .into(),
            trace_uuid: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into(),
            source: "openagents".into(),
            domain: "agent-building".into(),
            visibility: "ledger".into(),
            recorded_at: "2026-08-27T09:15:00Z".into(),
            batch_id: Some("batch-2026-08-27-001".into()),
            source_path: None,
            source_digest: None,
        };
        check_or_update("corpus_import_record", &value);
    }

    #[test]
    fn dataset_view_golden() {
        let value = DatasetView {
            schema: "openagents.gym.dataset_view.v1".into(),
            dataset_id: "coderbench-agent-building-v1".into(),
            description: Some("Agent-building domain traces and pinned tasks".into()),
            version: 1,
            created_at: "2026-08-20T12:00:00Z".into(),
            updated_at: "2026-08-27T09:30:00Z".into(),
            members: vec![
                DatasetMember {
                    reference:
                        "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                            .into(),
                    kind: "trace".into(),
                    added_by: "oa_agent".into(),
                    added_at: "2026-08-21T10:00:00Z".into(),
                    reason: "qualifying trace for agent-building".into(),
                    tags: vec!["agent-building".into()],
                },
                DatasetMember {
                    reference: "bench/tasks/coderbench/regex-log".into(),
                    kind: "task".into(),
                    added_by: "oa_agent".into(),
                    added_at: "2026-08-22T11:00:00Z".into(),
                    reason: "pinned task from tb2-quick".into(),
                    tags: vec!["smoke".into()],
                },
            ],
        };
        check_or_update("dataset_view", &value);
    }

    #[test]
    fn env_report_golden() {
        let value = EnvReport {
            schema: "openagents.gym.env_report.v1".into(),
            target: "local".into(),
            sufficient_for_scored_run: false,
            generated_at: "2026-08-27T09:45:00Z".into(),
            checks: vec![
                EnvCheck {
                    name: "docker_daemon".into(),
                    passed: true,
                    required: true,
                    remedy: None,
                    observed: Some("Docker Desktop 4.40".into()),
                },
                EnvCheck {
                    name: "amd64_emulation".into(),
                    passed: false,
                    required: true,
                    remedy: Some(
                        "enable Rosetta in Docker Desktop settings and re-run gym env probe".into(),
                    ),
                    observed: Some(
                        "qemu-user-static not installed; amd64 canary segfaulted".into(),
                    ),
                },
            ],
        };
        check_or_update("env_report", &value);
    }

    #[test]
    fn results_trend_golden() {
        let value = ResultsTrend {
            schema: "openagents.gym.results_trend.v1".into(),
            suite_id: "tb2-quick".into(),
            suite_key: "suite:abc123abc123abc123abc123abc123abc123abc123abc123abc123abc123".into(),
            verified: true,
            lane_comparisons: vec![LaneComparison {
                suite_key: "suite:abc123abc123abc123abc123abc123abc123abc123abc123abc123abc123"
                    .into(),
                suite_id: "tb2-quick".into(),
                tasks: vec!["regex-log".into(), "openssl-selfsigned-cert".into()],
                baseline_lane: "proxy".into(),
                lanes: vec![
                    LaneRow {
                        lane: "proxy".into(),
                        run_digest: "run:abc123abc123abc123abc123abc123abc123".into(),
                        recorded_at: "2026-08-26T12:00:00Z".into(),
                        cost_per_accepted_outcome_usd: Some(0.75),
                        success_rate: Some(1.0),
                        cost_delta: None,
                        success_rate_delta: None,
                    },
                    LaneRow {
                        lane: "local".into(),
                        run_digest: "run:def456def456def456def456def456def456".into(),
                        recorded_at: "2026-08-27T12:00:00Z".into(),
                        cost_per_accepted_outcome_usd: None,
                        success_rate: Some(0.5),
                        cost_delta: Some(Delta {
                            from: Some(0.75),
                            to: None,
                            absolute: None,
                            relative: None,
                            direction: "unpriced".into(),
                            reason: "no cost delta: the local lane reports cost_unknown".into(),
                        }),
                        success_rate_delta: Some(Delta {
                            from: Some(1.0),
                            to: Some(0.5),
                            absolute: Some(-0.5),
                            relative: Some(-0.5),
                            direction: "worse".into(),
                            reason: "success rate fell".into(),
                        }),
                    },
                ],
                confounders: vec![
                    "model also varies (openai/gpt-5.6-luna, local/gpt-5.6-luna)".into(),
                ],
            }],
            trends: vec![Trend {
                suite_key: "suite:abc123abc123abc123abc123abc123abc123abc123abc123abc123abc123"
                    .into(),
                suite_id: "tb2-quick".into(),
                lane: "proxy".into(),
                steps: vec![TrendStep {
                    from_recorded_at: "2026-08-25T12:00:00Z".into(),
                    to_recorded_at: "2026-08-26T12:00:00Z".into(),
                    cost_delta: Delta {
                        from: Some(0.85),
                        to: Some(0.75),
                        absolute: Some(-0.1),
                        relative: Some(-0.11764705882352941),
                        direction: "better".into(),
                        reason: "cost per accepted outcome fell".into(),
                    },
                    success_rate_delta: Delta {
                        from: Some(0.9),
                        to: Some(1.0),
                        absolute: Some(0.1),
                        relative: Some(0.1111111111111111),
                        direction: "better".into(),
                        reason: "success rate rose".into(),
                    },
                    confounders: vec![],
                }],
            }],
            isolated_groups: 0,
        };
        check_or_update("results_trend", &value);
    }
}
