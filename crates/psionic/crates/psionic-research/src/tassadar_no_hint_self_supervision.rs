use std::{fs, path::Path};

use psionic_models::TassadarExecutorSubroutineWorkloadFamily;
use psionic_train::{
    build_tassadar_executor_no_hint_dataset_manifest, build_tassadar_executor_no_hint_signal_proxy,
    TassadarExecutorHintRegime, TassadarExecutorNoHintDatasetManifest,
    TassadarExecutorNoHintSignalProxyConfig, TassadarExecutorNoHintSignalProxyReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_NO_HINT_SELF_SUPERVISION_OUTPUT_DIR: &str = "fixtures/tassadar/reports";
pub const TASSADAR_NO_HINT_SELF_SUPERVISION_REPORT_FILE: &str =
    "tassadar_no_hint_self_supervised_report.json";
pub const TASSADAR_NO_HINT_SELF_SUPERVISION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_no_hint_self_supervised_report.json";
pub const TASSADAR_NO_HINT_SELF_SUPERVISION_EXAMPLE_COMMAND: &str =
    "cargo run -p psionic-research --example tassadar_no_hint_self_supervision";
pub const TASSADAR_NO_HINT_SELF_SUPERVISION_TEST_COMMAND: &str =
    "cargo test -p psionic-research no_hint_self_supervised_report_matches_committed_truth -- --nocapture";

const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNoHintRegimeSummary {
    pub supervision_regime: TassadarExecutorHintRegime,
    pub manifest_digest: String,
    pub explicit_hint_target_count: u32,
    pub output_target_count: u32,
    pub self_supervised_regularizer_count: u32,
    pub active_signal_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNoHintClrsRegimeDelta {
    pub supervision_regime: TassadarExecutorHintRegime,
    pub reusable_signal_bps: u32,
    pub reusable_signal_units: u32,
    pub total_signal_units: u32,
    pub explicit_hint_target_count: u32,
    pub self_supervised_regularizer_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNoHintSelfSupervisedReport {
    pub schema_version: u16,
    pub comparison_id: String,
    pub report_ref: String,
    pub regeneration_commands: Vec<String>,
    pub regime_summaries: Vec<TassadarNoHintRegimeSummary>,
    pub held_out_signal_reports: Vec<TassadarExecutorNoHintSignalProxyReport>,
    pub clrs_regime_deltas: Vec<TassadarNoHintClrsRegimeDelta>,
    pub served_lane_refusal_reason: String,
    pub claim_class: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarNoHintSelfSupervisedReportError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

pub fn build_tassadar_no_hint_self_supervised_report() -> TassadarNoHintSelfSupervisedReport {
    let regime_summaries = [
        TassadarExecutorHintRegime::FullHintTrace,
        TassadarExecutorHintRegime::SubroutineHints,
        TassadarExecutorHintRegime::NoHintOutputOnly,
        TassadarExecutorHintRegime::NoHintSelfSupervised,
    ]
    .into_iter()
    .map(|supervision_regime| {
        let manifest: TassadarExecutorNoHintDatasetManifest =
            build_tassadar_executor_no_hint_dataset_manifest(supervision_regime);
        TassadarNoHintRegimeSummary {
            supervision_regime,
            manifest_digest: manifest.manifest_digest,
            explicit_hint_target_count: manifest.explicit_hint_target_count,
            output_target_count: manifest.output_target_count,
            self_supervised_regularizer_count: manifest.self_supervised_regularizer_count,
            active_signal_count: manifest.active_signal_count,
        }
    })
    .collect::<Vec<_>>();

    let held_out_signal_reports = [
        TassadarExecutorHintRegime::FullHintTrace,
        TassadarExecutorHintRegime::SubroutineHints,
        TassadarExecutorHintRegime::NoHintOutputOnly,
        TassadarExecutorHintRegime::NoHintSelfSupervised,
    ]
    .into_iter()
    .flat_map(|supervision_regime| {
        [
            TassadarExecutorSubroutineWorkloadFamily::Sort,
            TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
            TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
        ]
        .into_iter()
        .map(move |held_out_workload_family| {
            build_tassadar_executor_no_hint_signal_proxy(&TassadarExecutorNoHintSignalProxyConfig {
                supervision_regime,
                held_out_workload_family,
            })
        })
    })
    .collect::<Vec<_>>();

    let clrs_regime_deltas = held_out_signal_reports
        .iter()
        .filter(|report| {
            report.held_out_workload_family
                == TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath
        })
        .map(|report| TassadarNoHintClrsRegimeDelta {
            supervision_regime: report.supervision_regime,
            reusable_signal_bps: report.reusable_signal_bps,
            reusable_signal_units: report.reusable_signal_units,
            total_signal_units: report.total_signal_units,
            explicit_hint_target_count: report.explicit_hint_target_count,
            self_supervised_regularizer_count: report.self_supervised_regularizer_count,
        })
        .collect::<Vec<_>>();

    let clrs_output_only_bps = clrs_regime_deltas
        .iter()
        .find(|delta| delta.supervision_regime == TassadarExecutorHintRegime::NoHintOutputOnly)
        .expect("output-only CLRS delta should exist")
        .reusable_signal_bps;
    let clrs_self_supervised_bps = clrs_regime_deltas
        .iter()
        .find(|delta| delta.supervision_regime == TassadarExecutorHintRegime::NoHintSelfSupervised)
        .expect("self-supervised CLRS delta should exist")
        .reusable_signal_bps;
    let clrs_subroutine_bps = clrs_regime_deltas
        .iter()
        .find(|delta| delta.supervision_regime == TassadarExecutorHintRegime::SubroutineHints)
        .expect("subroutine CLRS delta should exist")
        .reusable_signal_bps;
    let clrs_full_hint_bps = clrs_regime_deltas
        .iter()
        .find(|delta| delta.supervision_regime == TassadarExecutorHintRegime::FullHintTrace)
        .expect("full-hint CLRS delta should exist")
        .reusable_signal_bps;

    let mut report = TassadarNoHintSelfSupervisedReport {
        schema_version: REPORT_SCHEMA_VERSION,
        comparison_id: String::from("tassadar.no_hint_self_supervision.v0"),
        report_ref: String::from(TASSADAR_NO_HINT_SELF_SUPERVISION_REPORT_REF),
        regeneration_commands: vec![
            String::from(TASSADAR_NO_HINT_SELF_SUPERVISION_EXAMPLE_COMMAND),
            String::from(TASSADAR_NO_HINT_SELF_SUPERVISION_TEST_COMMAND),
        ],
        regime_summaries,
        held_out_signal_reports,
        clrs_regime_deltas,
        served_lane_refusal_reason: String::from(
            "refuse served promotion: the no-hint/self-supervised executor regime is only a research-only architecture proxy with reusable-signal comparisons on the seeded corpus and has no trained-model benchmark gate or served capability publication",
        ),
        claim_class: String::from("research_only_architecture"),
        claim_boundary: String::from(
            "this report compares full-hint, subroutine-hint, output-only, and no-hint plus self-supervised regularizer regimes on the seeded sort, CLRS-shortest-path, and sudoku-style corpus only; it reports reusable supervision-signal proxies rather than trained-model exactness or served-lane capability",
        ),
        summary: format!(
            "No-hint/self-supervised executor report now freezes four supervision regimes across the seeded bounded corpus: on held-out CLRS shortest-path, output-only no-hint reaches {} bps reusable signal, no-hint plus self-supervised regularizers improves that to {} bps with zero explicit hint targets, full-hint trace stays at {} bps, and reusable subroutine hints remain the upper bound at {} bps; served promotion stays explicitly refused.",
            clrs_output_only_bps,
            clrs_self_supervised_bps,
            clrs_full_hint_bps,
            clrs_subroutine_bps,
        ),
        report_digest: String::new(),
    };
    report.report_digest =
        stable_digest(b"psionic_tassadar_no_hint_self_supervised_report|", &report);
    report
}

pub fn run_tassadar_no_hint_self_supervised_report(
    output_dir: &Path,
) -> Result<TassadarNoHintSelfSupervisedReport, TassadarNoHintSelfSupervisedReportError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarNoHintSelfSupervisedReportError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let report = build_tassadar_no_hint_self_supervised_report();
    let report_path = output_dir.join(TASSADAR_NO_HINT_SELF_SUPERVISION_REPORT_FILE);
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(&report_path, bytes).map_err(|error| {
        TassadarNoHintSelfSupervisedReportError::Write {
            path: report_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("no-hint/self-supervised report should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde::de::DeserializeOwned;
    use tempfile::tempdir;

    use super::{
        build_tassadar_no_hint_self_supervised_report, run_tassadar_no_hint_self_supervised_report,
        TassadarNoHintSelfSupervisedReport, TASSADAR_NO_HINT_SELF_SUPERVISION_OUTPUT_DIR,
        TASSADAR_NO_HINT_SELF_SUPERVISION_REPORT_REF,
    };
    use psionic_train::TassadarExecutorHintRegime;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    fn read_repo_json<T>(repo_relative_path: &str) -> Result<T, Box<dyn std::error::Error>>
    where
        T: DeserializeOwned,
    {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn no_hint_self_supervised_report_keeps_clrs_regime_research_only(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_no_hint_self_supervised_report();
        let clrs_self_supervised = report
            .clrs_regime_deltas
            .iter()
            .find(|delta| {
                delta.supervision_regime == TassadarExecutorHintRegime::NoHintSelfSupervised
            })
            .expect("self-supervised CLRS delta");
        let clrs_output_only = report
            .clrs_regime_deltas
            .iter()
            .find(|delta| delta.supervision_regime == TassadarExecutorHintRegime::NoHintOutputOnly)
            .expect("output-only CLRS delta");
        assert_eq!(clrs_self_supervised.explicit_hint_target_count, 0);
        assert!(clrs_self_supervised.self_supervised_regularizer_count > 0);
        assert!(clrs_self_supervised.reusable_signal_bps > clrs_output_only.reusable_signal_bps);
        assert!(report
            .served_lane_refusal_reason
            .contains("refuse served promotion"));
        Ok(())
    }

    #[test]
    fn no_hint_self_supervised_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_no_hint_self_supervised_report();
        let persisted: TassadarNoHintSelfSupervisedReport =
            read_repo_json(TASSADAR_NO_HINT_SELF_SUPERVISION_REPORT_REF)?;
        assert_eq!(persisted, report);
        Ok(())
    }

    #[test]
    fn no_hint_self_supervised_report_writes_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output_dir = tempdir()?;
        let report = run_tassadar_no_hint_self_supervised_report(output_dir.path())?;
        let persisted: TassadarNoHintSelfSupervisedReport =
            serde_json::from_slice(&std::fs::read(
                output_dir
                    .path()
                    .join("tassadar_no_hint_self_supervised_report.json"),
            )?)?;
        assert_eq!(persisted, report);
        assert!(TASSADAR_NO_HINT_SELF_SUPERVISION_OUTPUT_DIR.contains("reports"));
        Ok(())
    }
}
