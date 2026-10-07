use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    build_tassadar_effectful_replay_audit_bundle, TassadarEffectfulReplayAuditBundle,
    TassadarEffectfulReplayCaseStatus, TASSADAR_EFFECTFUL_REPLAY_AUDIT_BUNDLE_FILE,
    TASSADAR_EFFECTFUL_REPLAY_AUDIT_PROFILE_ID, TASSADAR_EFFECTFUL_REPLAY_AUDIT_RUN_ROOT_REF,
};

pub const TASSADAR_EFFECTFUL_REPLAY_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_effectful_replay_audit_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectfulReplayAuditCaseRef {
    pub case_id: String,
    pub effect_surface_id: String,
    pub status: TassadarEffectfulReplayCaseStatus,
    pub has_effect_receipt: bool,
    pub has_challenge_receipt: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectfulReplayAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarEffectfulReplayAuditBundle,
    pub case_refs: Vec<TassadarEffectfulReplayAuditCaseRef>,
    pub challengeable_case_count: u32,
    pub refusal_case_count: u32,
    pub replay_safe_effect_family_ids: Vec<String>,
    pub refused_effect_family_ids: Vec<String>,
    pub kernel_policy_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarEffectfulReplayAuditReportError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
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

pub fn build_tassadar_effectful_replay_audit_report(
) -> Result<TassadarEffectfulReplayAuditReport, TassadarEffectfulReplayAuditReportError> {
    let runtime_bundle = build_tassadar_effectful_replay_audit_bundle();
    let runtime_bundle_ref = format!(
        "{}/{}",
        TASSADAR_EFFECTFUL_REPLAY_AUDIT_RUN_ROOT_REF, TASSADAR_EFFECTFUL_REPLAY_AUDIT_BUNDLE_FILE
    );
    let mut generated_from_refs = vec![runtime_bundle_ref.clone()];
    let case_refs = runtime_bundle
        .case_receipts
        .iter()
        .map(|case| {
            if let Some(effect_receipt) = &case.effect_receipt {
                generated_from_refs.push(format!(
                    "{}/effect_receipts/{}.json",
                    TASSADAR_EFFECTFUL_REPLAY_AUDIT_RUN_ROOT_REF, effect_receipt.receipt_id
                ));
            }
            if let Some(challenge_receipt) = &case.challenge_receipt {
                generated_from_refs.push(format!(
                    "{}/challenge_receipts/{}.json",
                    TASSADAR_EFFECTFUL_REPLAY_AUDIT_RUN_ROOT_REF,
                    challenge_receipt.challenge_receipt_id
                ));
            }
            TassadarEffectfulReplayAuditCaseRef {
                case_id: case.case_id.clone(),
                effect_surface_id: case.effect_surface_id.clone(),
                status: case.status,
                has_effect_receipt: case.effect_receipt.is_some(),
                has_challenge_receipt: case.challenge_receipt.is_some(),
                refusal_reason_id: case.refusal_reason_id.clone(),
                note: case.note.clone(),
            }
        })
        .collect::<Vec<_>>();
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let mut report = TassadarEffectfulReplayAuditReport {
        schema_version: 1,
        report_id: String::from("tassadar.effectful_replay_audit.report.v1"),
        profile_id: String::from(TASSADAR_EFFECTFUL_REPLAY_AUDIT_PROFILE_ID),
        runtime_bundle_ref,
        runtime_bundle,
        case_refs,
        challengeable_case_count: 0,
        refusal_case_count: 0,
        replay_safe_effect_family_ids: Vec::new(),
        refused_effect_family_ids: Vec::new(),
        kernel_policy_dependency_marker: String::new(),
        nexus_dependency_marker: String::new(),
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report covers one bounded effectful replay-and-challenge lane with explicit effect receipts, replay digests, and challenge receipts. It keeps missing-effect evidence, missing challenge evidence, and unsafe effect families on explicit refusal paths instead of implying authority closure or settlement readiness inside standalone psionic",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.challengeable_case_count = report.runtime_bundle.challengeable_case_count;
    report.refusal_case_count = report.runtime_bundle.refusal_case_count;
    report.replay_safe_effect_family_ids =
        report.runtime_bundle.replay_safe_effect_family_ids.clone();
    report.refused_effect_family_ids = report.runtime_bundle.refused_effect_family_ids.clone();
    report.kernel_policy_dependency_marker = report
        .runtime_bundle
        .kernel_policy_dependency_marker
        .clone();
    report.nexus_dependency_marker = report.runtime_bundle.nexus_dependency_marker.clone();
    report.summary = format!(
        "Effectful replay audit report covers challengeable_cases={}, refusal_cases={}, replay_safe_families={}, refused_families={}.",
        report.challengeable_case_count,
        report.refusal_case_count,
        report.replay_safe_effect_family_ids.len(),
        report.refused_effect_family_ids.len(),
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_effectful_replay_audit_report|", &report);
    Ok(report)
}

#[must_use]
pub fn tassadar_effectful_replay_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EFFECTFUL_REPLAY_AUDIT_REPORT_REF)
}

pub fn write_tassadar_effectful_replay_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarEffectfulReplayAuditReport, TassadarEffectfulReplayAuditReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarEffectfulReplayAuditReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_effectful_replay_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarEffectfulReplayAuditReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
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
) -> Result<T, TassadarEffectfulReplayAuditReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarEffectfulReplayAuditReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarEffectfulReplayAuditReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_effectful_replay_audit_report, read_json,
        tassadar_effectful_replay_audit_report_path, write_tassadar_effectful_replay_audit_report,
    };
    use tempfile::tempdir;

    #[test]
    fn effectful_replay_audit_report_keeps_dependency_markers_and_refusals_explicit() {
        let report = build_tassadar_effectful_replay_audit_report().expect("report");

        assert_eq!(report.challengeable_case_count, 3);
        assert_eq!(report.refusal_case_count, 3);
        assert_eq!(report.replay_safe_effect_family_ids.len(), 3);
        assert_eq!(report.refused_effect_family_ids.len(), 3);
        assert!(report
            .kernel_policy_dependency_marker
            .contains("kernel-policy"));
        assert!(report.nexus_dependency_marker.contains("nexus"));
    }

    #[test]
    fn effectful_replay_audit_report_matches_committed_truth() {
        let generated = build_tassadar_effectful_replay_audit_report().expect("report");
        let committed =
            read_json(tassadar_effectful_replay_audit_report_path()).expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_effectful_replay_audit_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_effectful_replay_audit_report.json");
        let report =
            write_tassadar_effectful_replay_audit_report(&output_path).expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
    }
}
