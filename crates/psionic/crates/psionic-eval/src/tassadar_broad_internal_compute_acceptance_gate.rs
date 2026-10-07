use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::{
    tassadar_internal_compute_profile_ladder_publication,
    TassadarInternalComputePortabilityPosture, TassadarInternalComputeProfileStatus,
};

use crate::{
    build_tassadar_broad_internal_compute_portability_report,
    TassadarBroadInternalComputePortabilityReportError,
    TassadarRustOnlyArticleAcceptanceGateV2Report,
    TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF,
};

pub const TASSADAR_BROAD_INTERNAL_COMPUTE_ACCEPTANCE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_internal_compute_acceptance_gate.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadInternalComputeAcceptanceStatus {
    Green,
    Suppressed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputeAcceptanceProfileRow {
    pub profile_id: String,
    pub publication_allowed_row_count: u32,
    pub missing_required_evidence_refs: Vec<String>,
    pub article_baseline_green: bool,
    pub portability_ready: bool,
    pub refusal_suite_complete: bool,
    pub gate_status: TassadarBroadInternalComputeAcceptanceStatus,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputeAcceptanceGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub portability_report_ref: String,
    pub rust_only_article_acceptance_gate_ref: String,
    pub article_baseline_gate: TassadarRustOnlyArticleAcceptanceGateV2Report,
    pub profile_rows: Vec<TassadarBroadInternalComputeAcceptanceProfileRow>,
    pub green_profile_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub failed_profile_ids: Vec<String>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarBroadInternalComputeAcceptanceGateReportError {
    #[error(transparent)]
    Portability(#[from] TassadarBroadInternalComputePortabilityReportError),
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
}

pub fn build_tassadar_broad_internal_compute_acceptance_gate_report() -> Result<
    TassadarBroadInternalComputeAcceptanceGateReport,
    TassadarBroadInternalComputeAcceptanceGateReportError,
> {
    let portability_report = build_tassadar_broad_internal_compute_portability_report()?;
    let article_baseline_gate: TassadarRustOnlyArticleAcceptanceGateV2Report =
        read_repo_json(repo_root().join(TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF))?;
    let ladder = tassadar_internal_compute_profile_ladder_publication();
    let repo_root = repo_root();

    let profile_rows = ladder
        .profiles
        .iter()
        .map(|profile| {
            let publication_allowed_row_count = portability_report
                .rows
                .iter()
                .filter(|row| row.profile_id == profile.profile_id && row.publication_allowed)
                .count() as u32;
            let missing_required_evidence_refs = profile
                .required_evidence_refs
                .iter()
                .filter(|reference| {
                    reference.trim().is_empty()
                        || reference.starts_with("issue://")
                        || !repo_root.join(reference).exists()
                })
                .cloned()
                .collect::<Vec<_>>();
            let portability_ready = publication_allowed_row_count > 0
                && profile.portability_posture
                    == TassadarInternalComputePortabilityPosture::DeclaredCpuMatrix;
            let refusal_suite_complete = !profile.refusal_classes.is_empty();
            let gate_status = if article_baseline_gate.green
                && profile.status == TassadarInternalComputeProfileStatus::Implemented
                && portability_ready
                && missing_required_evidence_refs.is_empty()
                && refusal_suite_complete
            {
                TassadarBroadInternalComputeAcceptanceStatus::Green
            } else if article_baseline_gate.green
                && missing_required_evidence_refs.is_empty()
                && refusal_suite_complete
            {
                TassadarBroadInternalComputeAcceptanceStatus::Suppressed
            } else {
                TassadarBroadInternalComputeAcceptanceStatus::Failed
            };
            let detail = match gate_status {
                TassadarBroadInternalComputeAcceptanceStatus::Green => format!(
                    "profile `{}` is green for publication because the article baseline, portability envelope, receipts, and refusal suite are all explicit",
                    profile.profile_id
                ),
                TassadarBroadInternalComputeAcceptanceStatus::Suppressed => format!(
                    "profile `{}` stays suppressed because portability publication is not green yet even though baseline receipts are present",
                    profile.profile_id
                ),
                TassadarBroadInternalComputeAcceptanceStatus::Failed => format!(
                    "profile `{}` fails the broad gate because portability or required evidence refs are still incomplete",
                    profile.profile_id
                ),
            };
            TassadarBroadInternalComputeAcceptanceProfileRow {
                profile_id: profile.profile_id.clone(),
                publication_allowed_row_count,
                missing_required_evidence_refs,
                article_baseline_green: article_baseline_gate.green,
                portability_ready,
                refusal_suite_complete,
                gate_status,
                detail,
            }
        })
        .collect::<Vec<_>>();

    let green_profile_ids = profile_rows
        .iter()
        .filter(|row| row.gate_status == TassadarBroadInternalComputeAcceptanceStatus::Green)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let suppressed_profile_ids = profile_rows
        .iter()
        .filter(|row| row.gate_status == TassadarBroadInternalComputeAcceptanceStatus::Suppressed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let failed_profile_ids = profile_rows
        .iter()
        .filter(|row| row.gate_status == TassadarBroadInternalComputeAcceptanceStatus::Failed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let overall_green = failed_profile_ids.is_empty()
        && suppressed_profile_ids.is_empty()
        && article_baseline_gate.green;

    let mut report = TassadarBroadInternalComputeAcceptanceGateReport {
        schema_version: 1,
        report_id: String::from("tassadar.broad_internal_compute_acceptance_gate.report.v1"),
        portability_report_ref: String::from(
            psionic_runtime::TASSADAR_BROAD_INTERNAL_COMPUTE_PORTABILITY_REPORT_REF,
        ),
        rust_only_article_acceptance_gate_ref: String::from(
            TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF,
        ),
        article_baseline_gate,
        profile_rows,
        green_profile_ids,
        suppressed_profile_ids,
        failed_profile_ids,
        overall_green,
        claim_boundary: String::from(
            "this acceptance gate extends the Rust-only article baseline into a broader internal-compute publication gate. It stays red until named broader profiles have explicit portability envelopes, receipts, and refusal suites, and it keeps suppression visible instead of silently widening one green lab path into a broader claim",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Broad internal-compute acceptance gate now records green_profiles={}, suppressed_profiles={}, failed_profiles={}, overall_green={}.",
        report.green_profile_ids.len(),
        report.suppressed_profile_ids.len(),
        report.failed_profile_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_broad_internal_compute_acceptance_gate_report|",
        &report,
    );
    Ok(report)
}

pub fn tassadar_broad_internal_compute_acceptance_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_BROAD_INTERNAL_COMPUTE_ACCEPTANCE_GATE_REPORT_REF)
}

pub fn write_tassadar_broad_internal_compute_acceptance_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarBroadInternalComputeAcceptanceGateReport,
    TassadarBroadInternalComputeAcceptanceGateReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarBroadInternalComputeAcceptanceGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_broad_internal_compute_acceptance_gate_report()?;
    let bytes = serde_json::to_vec_pretty(&report).expect("broad internal compute gate serializes");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarBroadInternalComputeAcceptanceGateReportError::Write {
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
pub fn load_tassadar_broad_internal_compute_acceptance_gate_report(
    path: impl AsRef<Path>,
) -> Result<
    TassadarBroadInternalComputeAcceptanceGateReport,
    TassadarBroadInternalComputeAcceptanceGateReportError,
> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarBroadInternalComputeAcceptanceGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarBroadInternalComputeAcceptanceGateReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

fn read_repo_json<T: serde::de::DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarBroadInternalComputeAcceptanceGateReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarBroadInternalComputeAcceptanceGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarBroadInternalComputeAcceptanceGateReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_broad_internal_compute_acceptance_gate_report,
        load_tassadar_broad_internal_compute_acceptance_gate_report,
        tassadar_broad_internal_compute_acceptance_gate_report_path,
        TassadarBroadInternalComputeAcceptanceStatus,
    };

    #[test]
    fn broad_internal_compute_acceptance_gate_keeps_article_green_and_broader_profiles_blocked() {
        let report =
            build_tassadar_broad_internal_compute_acceptance_gate_report().expect("report");
        assert!(report.green_profile_ids.contains(&String::from(
            "tassadar.internal_compute.article_closeout.v1"
        )));
        assert!(report.suppressed_profile_ids.contains(&String::from(
            "tassadar.internal_compute.generalized_abi.v1"
        )));
        assert!(report.suppressed_profile_ids.contains(&String::from(
            "tassadar.internal_compute.runtime_support_subset.v1"
        )));
        assert!(!report.overall_green);
        assert!(report.profile_rows.iter().any(|row| {
            row.profile_id == "tassadar.internal_compute.generalized_abi.v1"
                && row.gate_status == TassadarBroadInternalComputeAcceptanceStatus::Suppressed
        }));
    }

    #[test]
    fn broad_internal_compute_acceptance_gate_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let expected = build_tassadar_broad_internal_compute_acceptance_gate_report()?;
        let committed = load_tassadar_broad_internal_compute_acceptance_gate_report(
            tassadar_broad_internal_compute_acceptance_gate_report_path(),
        )?;
        assert_eq!(committed, expected);
        Ok(())
    }
}
