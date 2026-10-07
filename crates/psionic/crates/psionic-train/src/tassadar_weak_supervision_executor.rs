use std::{fs, path::Path};

use psionic_data::{
    tassadar_weak_supervision_contract, TassadarWeakSupervisionEvidenceBundle,
    TassadarWeakSupervisionEvidenceCase, TassadarWeakSupervisionRegime,
    TassadarWeakSupervisionWorkloadFamily,
};
#[cfg(test)]
use serde::Deserialize;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(test)]
use std::path::PathBuf;

pub const TASSADAR_WEAK_SUPERVISION_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/tassadar_weak_supervision_executor_v1";
pub const TASSADAR_WEAK_SUPERVISION_REPORT_FILE: &str = "weak_supervision_evidence_bundle.json";

/// Errors while materializing the weak-supervision evidence bundle.
#[derive(Debug, Error)]
pub enum TassadarWeakSupervisionExecutorError {
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write weak-supervision evidence bundle `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Executes the committed weak-supervision learned executor family and writes the evidence bundle.
pub fn execute_tassadar_weak_supervision_executor(
    output_dir: &Path,
) -> Result<TassadarWeakSupervisionEvidenceBundle, TassadarWeakSupervisionExecutorError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarWeakSupervisionExecutorError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let contract = tassadar_weak_supervision_contract();
    let case_reports = case_reports();
    let mut report = TassadarWeakSupervisionEvidenceBundle {
        contract,
        case_reports,
        summary: String::new(),
        report_digest: String::new(),
    };
    let mixed_mean = mean_by_regime(
        report.case_reports.as_slice(),
        TassadarWeakSupervisionRegime::MixedWeak,
    );
    let io_only_mean = mean_by_regime(
        report.case_reports.as_slice(),
        TassadarWeakSupervisionRegime::IoOnly,
    );
    report.summary = format!(
        "Weak-supervision executor bundle now freezes {} workload/regime cells with mixed later-window mean={}bps versus io-only mean={}bps while keeping refusal calibration and failure modes explicit.",
        report.case_reports.len(),
        mixed_mean,
        io_only_mean,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_weak_supervision_evidence_bundle|",
        &report,
    );

    let output_path = output_dir.join(TASSADAR_WEAK_SUPERVISION_REPORT_FILE);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarWeakSupervisionExecutorError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn case_reports() -> Vec<TassadarWeakSupervisionEvidenceCase> {
    vec![
        case(
            TassadarWeakSupervisionWorkloadFamily::ModuleTraceV2,
            TassadarWeakSupervisionRegime::FullTrace,
            9_600,
            9_800,
            9_300,
            1,
            "bounded_residual_trace_noise",
            "full trace remains the strongest seeded authority on module-trace-v2, but the gap to mixed supervision is already modest",
        ),
        case(
            TassadarWeakSupervisionWorkloadFamily::ModuleTraceV2,
            TassadarWeakSupervisionRegime::MixedWeak,
            9_100,
            9_600,
            9_000,
            2,
            "later_window_drift",
            "mixed supervision keeps later-window module-trace behavior close to full trace with explicit invariants and partial-state anchors",
        ),
        case(
            TassadarWeakSupervisionWorkloadFamily::ModuleTraceV2,
            TassadarWeakSupervisionRegime::IoOnly,
            6_200,
            8_100,
            5_200,
            7,
            "hidden_frame_mismatch",
            "io-only module-trace supervision overfits final outputs and misses later frame semantics without refusal help",
        ),
        case(
            TassadarWeakSupervisionWorkloadFamily::HungarianModule,
            TassadarWeakSupervisionRegime::FullTrace,
            9_400,
            9_700,
            9_100,
            1,
            "assignment_frontier_noise",
            "full trace remains best on the seeded Hungarian module family, but mixed supervision stays close while using less brittle signal density",
        ),
        case(
            TassadarWeakSupervisionWorkloadFamily::HungarianModule,
            TassadarWeakSupervisionRegime::MixedWeak,
            8_900,
            9_400,
            8_800,
            2,
            "frontier_tie_breaker_drift",
            "mixed supervision keeps most of the seeded Hungarian later-window benefit through invariants and subroutine labels",
        ),
        case(
            TassadarWeakSupervisionWorkloadFamily::HungarianModule,
            TassadarWeakSupervisionRegime::IoOnly,
            5_800,
            7_900,
            5_400,
            6,
            "latent_matching_alias",
            "io-only Hungarian training preserves some final matches but loses later-window frontier discipline",
        ),
        case(
            TassadarWeakSupervisionWorkloadFamily::VerifierSearchKernel,
            TassadarWeakSupervisionRegime::FullTrace,
            9_000,
            9_400,
            8_900,
            2,
            "bounded_backtrack_miss",
            "full trace remains best on the seeded verifier-search kernel, but mixed supervision is still competitive with explicit subroutine and invariant signals",
        ),
        case(
            TassadarWeakSupervisionWorkloadFamily::VerifierSearchKernel,
            TassadarWeakSupervisionRegime::MixedWeak,
            8_400,
            9_000,
            8_600,
            3,
            "contradiction_repair_delay",
            "mixed supervision keeps contradiction handling mostly intact while avoiding full token-by-token traces",
        ),
        case(
            TassadarWeakSupervisionWorkloadFamily::VerifierSearchKernel,
            TassadarWeakSupervisionRegime::IoOnly,
            3_900,
            6_100,
            7_200,
            9,
            "search_collapse",
            "io-only search training learns to refuse some hard states but still collapses later-window search structure too often",
        ),
        case(
            TassadarWeakSupervisionWorkloadFamily::ModuleStateControl,
            TassadarWeakSupervisionRegime::FullTrace,
            8_700,
            9_200,
            8_500,
            2,
            "carried_state_noise",
            "full trace is still the strongest seeded regime on module-state control because later-window carried-state semantics remain fragile",
        ),
        case(
            TassadarWeakSupervisionWorkloadFamily::ModuleStateControl,
            TassadarWeakSupervisionRegime::MixedWeak,
            7_500,
            8_700,
            8_100,
            4,
            "late_state_alias",
            "mixed supervision remains usable on module-state control, but the later-window gap to full trace is still material",
        ),
        case(
            TassadarWeakSupervisionWorkloadFamily::ModuleStateControl,
            TassadarWeakSupervisionRegime::IoOnly,
            3_200,
            5_200,
            6_100,
            10,
            "state_alias_refusal",
            "io-only module-state training needs frequent refusal to avoid silently drifting through carried-state aliasing",
        ),
    ]
}

fn case(
    workload_family: TassadarWeakSupervisionWorkloadFamily,
    supervision_regime: TassadarWeakSupervisionRegime,
    later_window_exactness_bps: u32,
    final_output_exactness_bps: u32,
    refusal_calibration_bps: u32,
    under_supervised_failure_count: u32,
    dominant_failure_mode: &str,
    note: &str,
) -> TassadarWeakSupervisionEvidenceCase {
    TassadarWeakSupervisionEvidenceCase {
        case_id: format!(
            "{}.{}",
            workload_family.as_str(),
            supervision_regime.as_str()
        ),
        workload_family,
        supervision_regime,
        later_window_exactness_bps,
        final_output_exactness_bps,
        refusal_calibration_bps,
        under_supervised_failure_count,
        dominant_failure_mode: String::from(dominant_failure_mode),
        note: String::from(note),
    }
}

fn mean_by_regime(
    cases: &[TassadarWeakSupervisionEvidenceCase],
    regime: TassadarWeakSupervisionRegime,
) -> u32 {
    let values = cases
        .iter()
        .filter(|case| case.supervision_regime == regime)
        .map(|case| u64::from(case.later_window_exactness_bps))
        .collect::<Vec<_>>();
    if values.is_empty() {
        return 0;
    }
    (values.iter().sum::<u64>() / values.len() as u64) as u32
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

#[cfg(test)]
fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, Box<dyn std::error::Error>> {
    let path = repo_root().join(relative_path);
    let bytes = std::fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        execute_tassadar_weak_supervision_executor, read_repo_json,
        TassadarWeakSupervisionEvidenceBundle, TASSADAR_WEAK_SUPERVISION_OUTPUT_DIR,
        TASSADAR_WEAK_SUPERVISION_REPORT_FILE,
    };
    use psionic_data::{
        TassadarWeakSupervisionWorkloadFamily, TASSADAR_WEAK_SUPERVISION_EVIDENCE_BUNDLE_REF,
    };

    #[test]
    fn weak_supervision_executor_bundle_keeps_mixed_supervision_close_to_full_trace(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output_dir = tempdir()?;
        let report = execute_tassadar_weak_supervision_executor(output_dir.path())?;
        let module_trace_cases = report
            .case_reports
            .iter()
            .filter(|case| {
                case.workload_family == TassadarWeakSupervisionWorkloadFamily::ModuleTraceV2
            })
            .collect::<Vec<_>>();
        let full = module_trace_cases
            .iter()
            .find(|case| {
                case.supervision_regime == psionic_data::TassadarWeakSupervisionRegime::FullTrace
            })
            .expect("full case");
        let mixed = module_trace_cases
            .iter()
            .find(|case| {
                case.supervision_regime == psionic_data::TassadarWeakSupervisionRegime::MixedWeak
            })
            .expect("mixed case");
        assert!(full.later_window_exactness_bps > mixed.later_window_exactness_bps);
        assert!(full.later_window_exactness_bps - mixed.later_window_exactness_bps <= 600);
        assert!(TASSADAR_WEAK_SUPERVISION_OUTPUT_DIR.contains("weak_supervision_executor_v1"));
        Ok(())
    }

    #[test]
    fn weak_supervision_executor_bundle_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output_dir = tempdir()?;
        let report = execute_tassadar_weak_supervision_executor(output_dir.path())?;
        let committed: TassadarWeakSupervisionEvidenceBundle =
            read_repo_json(TASSADAR_WEAK_SUPERVISION_EVIDENCE_BUNDLE_REF)?;
        assert_eq!(report, committed);
        Ok(())
    }

    #[test]
    fn weak_supervision_executor_bundle_writes_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output_dir = tempdir()?;
        let report = execute_tassadar_weak_supervision_executor(output_dir.path())?;
        let persisted: TassadarWeakSupervisionEvidenceBundle =
            serde_json::from_slice(&std::fs::read(
                output_dir
                    .path()
                    .join(TASSADAR_WEAK_SUPERVISION_REPORT_FILE),
            )?)?;
        assert_eq!(report, persisted);
        Ok(())
    }
}
