use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_cross_profile_link_compatibility_report.json";
pub const TASSADAR_CROSS_PROFILE_LINK_PORTABILITY_ENVELOPE_ID: &str = "cpu_reference_current_host";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCrossProfileLinkStatus {
    Exact,
    Downgraded,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCrossProfileLinkRefusalReason {
    PortabilityEnvelopeMismatch,
    EffectBoundaryMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCrossProfileLinkCase {
    pub case_id: String,
    pub producer_profile_id: String,
    pub consumer_profile_id: String,
    pub requested_link_shape_id: String,
    pub semantic_window_id: String,
    pub requested_portability_envelope_id: String,
    pub status: TassadarCrossProfileLinkStatus,
    pub planned_profile_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_stack_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downgrade_target_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarCrossProfileLinkRefusalReason>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCrossProfileLinkCompatibilityReport {
    pub schema_version: u16,
    pub report_id: String,
    pub portability_envelope_id: String,
    pub case_reports: Vec<TassadarCrossProfileLinkCase>,
    pub exact_case_count: u32,
    pub downgraded_case_count: u32,
    pub refused_case_count: u32,
    pub routeable_case_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarCrossProfileLinkCompatibilityReportError {
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
pub fn build_tassadar_cross_profile_link_compatibility_report()
-> TassadarCrossProfileLinkCompatibilityReport {
    let case_reports = vec![
        exact_case(
            "compat.session_process_to_spill_tape.exact.v1",
            "tassadar.internal_compute.session_process.v1",
            "tassadar.internal_compute.spill_tape_store.v1",
            "session_checkpoint_spill_adapter_v1",
            "tassadar.current_host.process_checkpoint_window.v1",
            &[
                "tassadar.internal_compute.session_process.v1",
                "tassadar.internal_compute.spill_tape_store.v1",
            ],
            Some("session_checkpoint_spill_adapter_v1"),
            &[
                "fixtures/tassadar/reports/tassadar_session_process_profile_report.json",
                "fixtures/tassadar/reports/tassadar_spill_tape_store_report.json",
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
            ],
            "deterministic session transcripts may link to the bounded spill-backed continuation lane through one explicit checkpoint adapter without widening either profile into a generic process runtime",
        ),
        downgraded_case(
            "compat.generalized_abi_to_component_model.downgrade.v1",
            "tassadar.internal_compute.generalized_abi.v1",
            "tassadar.internal_compute.component_model_abi.v1",
            "generalized_heap_multi_value_to_component_manifest_v1",
            "tassadar.wasm.generalized_abi.v1",
            &["tassadar.internal_compute.component_model_abi.v1"],
            "tassadar.internal_compute.component_model_abi.v1",
            Some("generalized_abi_to_component_manifest_adapter_v1"),
            &[
                "fixtures/tassadar/reports/tassadar_generalized_abi_family_report.json",
                "fixtures/tassadar/reports/tassadar_internal_component_abi_report.json",
                "fixtures/tassadar/reports/tassadar_component_linking_profile_report.json",
            ],
            "generalized multi-value heap results stay linkable only by downgrading into one explicit component-model manifest adapter instead of silently pretending the broader ABI is native on both sides",
        ),
        refusal_case(
            "compat.spill_tape_to_portable_broad_family.refusal.v1",
            "tassadar.internal_compute.spill_tape_store.v1",
            "tassadar.internal_compute.portable_broad_family.v1",
            "spill_segment_portable_process_snapshot_v1",
            "tassadar.portable_broad_family.window.v1",
            TassadarCrossProfileLinkRefusalReason::PortabilityEnvelopeMismatch,
            &[
                "fixtures/tassadar/reports/tassadar_spill_tape_store_report.json",
                "fixtures/tassadar/reports/tassadar_broad_internal_compute_portability_report.json",
            ],
            "spill-backed continuation artifacts remain current-host cpu-reference objects and refuse silent widening into portable broad-family process state",
        ),
        refusal_case(
            "compat.session_process_to_deterministic_import.refusal.v1",
            "tassadar.internal_compute.session_process.v1",
            "tassadar.internal_compute.deterministic_import_subset.v1",
            "interactive_event_import_resume_v1",
            "tassadar.deterministic_import_resume.window.v1",
            TassadarCrossProfileLinkRefusalReason::EffectBoundaryMismatch,
            &[
                "fixtures/tassadar/reports/tassadar_session_process_profile_report.json",
                "fixtures/tassadar/reports/tassadar_effect_safe_resume_report.json",
                "fixtures/tassadar/reports/tassadar_simulator_effect_profile_report.json",
            ],
            "interactive session turns do not inherit deterministic-import resume authority, so cross-profile linking refuses instead of flattening effect-safe replay into generic interaction closure",
        ),
    ];
    let routeable_case_ids = case_reports
        .iter()
        .filter(|case| {
            matches!(
                case.status,
                TassadarCrossProfileLinkStatus::Exact | TassadarCrossProfileLinkStatus::Downgraded
            )
        })
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarCrossProfileLinkCompatibilityReport {
        schema_version: 1,
        report_id: String::from("tassadar.cross_profile_link_compatibility.report.v1"),
        portability_envelope_id: String::from(TASSADAR_CROSS_PROFILE_LINK_PORTABILITY_ENVELOPE_ID),
        case_reports,
        exact_case_count: 0,
        downgraded_case_count: 0,
        refused_case_count: 0,
        routeable_case_ids,
        claim_boundary: String::from(
            "this compiler-owned report freezes one bounded cross-profile linking lane over named profiles, explicit downgrade targets, and typed compatibility refusal. It does not claim arbitrary profile composition, implicit portability lifting, or broader served publication",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.exact_case_count = report
        .case_reports
        .iter()
        .filter(|case| case.status == TassadarCrossProfileLinkStatus::Exact)
        .count() as u32;
    report.downgraded_case_count = report
        .case_reports
        .iter()
        .filter(|case| case.status == TassadarCrossProfileLinkStatus::Downgraded)
        .count() as u32;
    report.refused_case_count = report
        .case_reports
        .iter()
        .filter(|case| case.status == TassadarCrossProfileLinkStatus::Refused)
        .count() as u32;
    report.summary = format!(
        "Cross-profile link compatibility report freezes {} cases across exact={}, downgraded={}, refused={}, routeable={}.",
        report.case_reports.len(),
        report.exact_case_count,
        report.downgraded_case_count,
        report.refused_case_count,
        report.routeable_case_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_cross_profile_link_compatibility_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_cross_profile_link_compatibility_report_path() -> PathBuf {
    repo_root().join(TASSADAR_CROSS_PROFILE_LINK_COMPATIBILITY_REPORT_REF)
}

pub fn write_tassadar_cross_profile_link_compatibility_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarCrossProfileLinkCompatibilityReport,
    TassadarCrossProfileLinkCompatibilityReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarCrossProfileLinkCompatibilityReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_cross_profile_link_compatibility_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarCrossProfileLinkCompatibilityReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn exact_case(
    case_id: &str,
    producer_profile_id: &str,
    consumer_profile_id: &str,
    requested_link_shape_id: &str,
    semantic_window_id: &str,
    planned_profile_ids: &[&str],
    adapter_stack_id: Option<&str>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarCrossProfileLinkCase {
    TassadarCrossProfileLinkCase {
        case_id: String::from(case_id),
        producer_profile_id: String::from(producer_profile_id),
        consumer_profile_id: String::from(consumer_profile_id),
        requested_link_shape_id: String::from(requested_link_shape_id),
        semantic_window_id: String::from(semantic_window_id),
        requested_portability_envelope_id: String::from(
            TASSADAR_CROSS_PROFILE_LINK_PORTABILITY_ENVELOPE_ID,
        ),
        status: TassadarCrossProfileLinkStatus::Exact,
        planned_profile_ids: planned_profile_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        adapter_stack_id: adapter_stack_id.map(String::from),
        downgrade_target_profile_id: None,
        refusal_reason: None,
        benchmark_refs: benchmark_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

fn downgraded_case(
    case_id: &str,
    producer_profile_id: &str,
    consumer_profile_id: &str,
    requested_link_shape_id: &str,
    semantic_window_id: &str,
    planned_profile_ids: &[&str],
    downgrade_target_profile_id: &str,
    adapter_stack_id: Option<&str>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarCrossProfileLinkCase {
    TassadarCrossProfileLinkCase {
        case_id: String::from(case_id),
        producer_profile_id: String::from(producer_profile_id),
        consumer_profile_id: String::from(consumer_profile_id),
        requested_link_shape_id: String::from(requested_link_shape_id),
        semantic_window_id: String::from(semantic_window_id),
        requested_portability_envelope_id: String::from(
            TASSADAR_CROSS_PROFILE_LINK_PORTABILITY_ENVELOPE_ID,
        ),
        status: TassadarCrossProfileLinkStatus::Downgraded,
        planned_profile_ids: planned_profile_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        adapter_stack_id: adapter_stack_id.map(String::from),
        downgrade_target_profile_id: Some(String::from(downgrade_target_profile_id)),
        refusal_reason: None,
        benchmark_refs: benchmark_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

fn refusal_case(
    case_id: &str,
    producer_profile_id: &str,
    consumer_profile_id: &str,
    requested_link_shape_id: &str,
    semantic_window_id: &str,
    refusal_reason: TassadarCrossProfileLinkRefusalReason,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarCrossProfileLinkCase {
    TassadarCrossProfileLinkCase {
        case_id: String::from(case_id),
        producer_profile_id: String::from(producer_profile_id),
        consumer_profile_id: String::from(consumer_profile_id),
        requested_link_shape_id: String::from(requested_link_shape_id),
        semantic_window_id: String::from(semantic_window_id),
        requested_portability_envelope_id: String::from(
            TASSADAR_CROSS_PROFILE_LINK_PORTABILITY_ENVELOPE_ID,
        ),
        status: TassadarCrossProfileLinkStatus::Refused,
        planned_profile_ids: Vec::new(),
        adapter_stack_id: None,
        downgrade_target_profile_id: None,
        refusal_reason: Some(refusal_reason),
        benchmark_refs: benchmark_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
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

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarCrossProfileLinkCompatibilityReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarCrossProfileLinkCompatibilityReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarCrossProfileLinkCompatibilityReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        TassadarCrossProfileLinkRefusalReason, TassadarCrossProfileLinkStatus,
        build_tassadar_cross_profile_link_compatibility_report, read_json,
        tassadar_cross_profile_link_compatibility_report_path,
        write_tassadar_cross_profile_link_compatibility_report,
    };

    #[test]
    fn cross_profile_link_compatibility_report_keeps_exact_downgraded_and_refused_cases_explicit() {
        let report = build_tassadar_cross_profile_link_compatibility_report();

        assert_eq!(report.exact_case_count, 1);
        assert_eq!(report.downgraded_case_count, 1);
        assert_eq!(report.refused_case_count, 2);
        assert_eq!(report.routeable_case_ids.len(), 2);
        assert!(report.case_reports.iter().any(|case| {
            case.status == TassadarCrossProfileLinkStatus::Downgraded
                && case.downgrade_target_profile_id.as_deref()
                    == Some("tassadar.internal_compute.component_model_abi.v1")
        }));
        assert!(report.case_reports.iter().any(|case| {
            case.refusal_reason
                == Some(TassadarCrossProfileLinkRefusalReason::EffectBoundaryMismatch)
        }));
    }

    #[test]
    fn cross_profile_link_compatibility_report_matches_committed_truth() {
        let generated = build_tassadar_cross_profile_link_compatibility_report();
        let committed = read_json(tassadar_cross_profile_link_compatibility_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_cross_profile_link_compatibility_report_persists_current_truth() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("timestamp")
            .as_nanos();
        let output_path = env::temp_dir().join(format!(
            "tassadar_cross_profile_link_compatibility_report_{unique}.json"
        ));
        let report = write_tassadar_cross_profile_link_compatibility_report(&output_path)
            .expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
        let _ = fs::remove_file(output_path);
    }
}
