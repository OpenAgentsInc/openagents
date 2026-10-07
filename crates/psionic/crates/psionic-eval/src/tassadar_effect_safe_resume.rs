use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TASSADAR_EFFECT_SAFE_RESUME_BUNDLE_FILE, TASSADAR_EFFECT_SAFE_RESUME_RUN_ROOT_REF,
    TASSADAR_EFFECT_SAFE_RESUME_TARGET_PROFILE_ID, TassadarEffectReceipt,
    TassadarEffectSafeResumeBundle, TassadarEffectSafeResumeDecisionStatus,
    build_tassadar_effect_safe_resume_bundle,
};

use crate::{
    TASSADAR_EFFECT_TAXONOMY_REPORT_REF, TASSADAR_RESUMABLE_MULTI_SLICE_PROMOTION_REPORT_REF,
    TassadarEffectTaxonomyReport, TassadarEffectTaxonomyReportError,
    TassadarResumableMultiSlicePromotionReport, TassadarResumableMultiSlicePromotionReportError,
    build_tassadar_effect_taxonomy_report, build_tassadar_resumable_multi_slice_promotion_report,
};

/// Stable committed report ref for deterministic import-mediated effect-safe resume.
pub const TASSADAR_EFFECT_SAFE_RESUME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_effect_safe_resume_report.json";

/// Materialized artifact ref for one negotiated effect receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectSafeResumeArtifactRef {
    /// Stable case identifier.
    pub case_id: String,
    /// Relative path for the effect receipt artifact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_receipt_path: Option<String>,
    /// Stable digest for the effect receipt when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_receipt_digest: Option<String>,
}

/// Eval-facing case report for one deterministic import-mediated continuation case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectSafeResumeCaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable checkpoint identifier from the base resumable lane.
    pub checkpoint_id: String,
    /// Effect ref negotiated against the taxonomy.
    pub effect_ref: String,
    /// Admitted or refused status for the target profile.
    pub decision_status: TassadarEffectSafeResumeDecisionStatus,
    /// Whether the admitted effect preserves exact base resume parity.
    pub exact_resume_parity: bool,
    /// Materialized receipt artifact ref when present.
    pub artifact_ref: TassadarEffectSafeResumeArtifactRef,
    /// Plain-language note.
    pub note: String,
}

/// Committed eval report for deterministic import-mediated effect-safe resume.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectSafeResumeReport {
    /// Schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable target profile identifier.
    pub target_profile_id: String,
    /// Stable runtime bundle ref.
    pub runtime_bundle_ref: String,
    /// Runtime bundle carried by the report.
    pub runtime_bundle: TassadarEffectSafeResumeBundle,
    /// Existing effect taxonomy report ref reused by this lane.
    pub effect_taxonomy_report_ref: String,
    /// Existing effect taxonomy report summary.
    pub effect_taxonomy_report: TassadarEffectTaxonomyReport,
    /// Existing resumable multi-slice promotion report ref reused by this lane.
    pub resumable_multi_slice_promotion_report_ref: String,
    /// Existing resumable multi-slice promotion report summary.
    pub resumable_multi_slice_promotion_report: TassadarResumableMultiSlicePromotionReport,
    /// Admitted deterministic-import continuation rows.
    pub admitted_case_count: u32,
    /// Refused continuation rows.
    pub refusal_case_count: u32,
    /// Effect refs admitted by the target profile.
    pub continuation_safe_effect_refs: Vec<String>,
    /// Effect refs refused by the target profile.
    pub continuation_refused_effect_refs: Vec<String>,
    /// Materialized case reports.
    pub case_reports: Vec<TassadarEffectSafeResumeCaseReport>,
    /// Dependency marker for authority-owned effect admission.
    pub kernel_policy_dependency_marker: String,
    /// Dependency marker for mount-owned effect admission.
    pub world_mount_dependency_marker: String,
    /// Stable refs used to derive the report.
    pub generated_from_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable report digest.
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarEffectSafeResumeReportError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    EffectTaxonomy(#[from] TassadarEffectTaxonomyReportError),
    #[error(transparent)]
    ResumablePromotion(#[from] TassadarResumableMultiSlicePromotionReportError),
    #[error(transparent)]
    Runtime(#[from] psionic_runtime::TassadarEffectSafeResumeError),
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

#[derive(Clone)]
struct WritePlan {
    relative_path: String,
    bytes: Vec<u8>,
}

pub fn build_tassadar_effect_safe_resume_report(
) -> Result<TassadarEffectSafeResumeReport, TassadarEffectSafeResumeReportError> {
    Ok(build_tassadar_effect_safe_resume_materialization()?.0)
}

#[must_use]
pub fn tassadar_effect_safe_resume_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EFFECT_SAFE_RESUME_REPORT_REF)
}

pub fn write_tassadar_effect_safe_resume_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarEffectSafeResumeReport, TassadarEffectSafeResumeReportError> {
    let output_path = output_path.as_ref();
    let (report, write_plans) = build_tassadar_effect_safe_resume_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarEffectSafeResumeReportError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| TassadarEffectSafeResumeReportError::Write {
            path: path.display().to_string(),
            error,
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarEffectSafeResumeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarEffectSafeResumeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_tassadar_effect_safe_resume_materialization(
) -> Result<(TassadarEffectSafeResumeReport, Vec<WritePlan>), TassadarEffectSafeResumeReportError>
{
    let runtime_bundle = build_tassadar_effect_safe_resume_bundle()?;
    let runtime_bundle_ref = format!(
        "{}/{}",
        TASSADAR_EFFECT_SAFE_RESUME_RUN_ROOT_REF, TASSADAR_EFFECT_SAFE_RESUME_BUNDLE_FILE
    );
    let mut write_plans = vec![WritePlan {
        relative_path: runtime_bundle_ref.clone(),
        bytes: json_bytes(&runtime_bundle)?,
    }];
    let mut generated_from_refs = vec![
        runtime_bundle_ref.clone(),
        String::from(TASSADAR_EFFECT_TAXONOMY_REPORT_REF),
        String::from(TASSADAR_RESUMABLE_MULTI_SLICE_PROMOTION_REPORT_REF),
    ];
    let mut case_reports = Vec::new();
    for case in &runtime_bundle.case_receipts {
        let (effect_receipt_path, effect_receipt_digest) = effect_receipt_artifact(case.effect_receipt.as_ref());
        if let Some(receipt) = case.effect_receipt.as_ref() {
            let effect_receipt_path = effect_receipt_path
                .clone()
                .expect("path should exist when effect receipt exists");
            generated_from_refs.push(effect_receipt_path.clone());
            write_plans.push(WritePlan {
                relative_path: effect_receipt_path,
                bytes: json_bytes(receipt)?,
            });
        }
        case_reports.push(TassadarEffectSafeResumeCaseReport {
            case_id: case.case_id.clone(),
            checkpoint_id: case.checkpoint_id.clone(),
            effect_ref: case.effect_request.effect_ref.clone(),
            decision_status: case.decision_status,
            exact_resume_parity: case.exact_resume_parity,
            artifact_ref: TassadarEffectSafeResumeArtifactRef {
                case_id: case.case_id.clone(),
                effect_receipt_path,
                effect_receipt_digest,
            },
            note: case.note.clone(),
        });
    }
    generated_from_refs.sort();
    generated_from_refs.dedup();

    let effect_taxonomy_report = build_tassadar_effect_taxonomy_report()?;
    let resumable_multi_slice_promotion_report =
        build_tassadar_resumable_multi_slice_promotion_report()?;

    let mut report = TassadarEffectSafeResumeReport {
        schema_version: 1,
        report_id: String::from("tassadar.effect_safe_resume.report.v1"),
        target_profile_id: String::from(TASSADAR_EFFECT_SAFE_RESUME_TARGET_PROFILE_ID),
        runtime_bundle_ref,
        runtime_bundle,
        effect_taxonomy_report_ref: String::from(TASSADAR_EFFECT_TAXONOMY_REPORT_REF),
        effect_taxonomy_report,
        resumable_multi_slice_promotion_report_ref: String::from(
            TASSADAR_RESUMABLE_MULTI_SLICE_PROMOTION_REPORT_REF,
        ),
        resumable_multi_slice_promotion_report,
        admitted_case_count: 0,
        refusal_case_count: 0,
        continuation_safe_effect_refs: Vec::new(),
        continuation_refused_effect_refs: Vec::new(),
        case_reports,
        kernel_policy_dependency_marker: String::new(),
        world_mount_dependency_marker: String::new(),
        generated_from_refs,
        claim_boundary: String::from(
            "this report promotes the resumable multi-slice lane only into the deterministic-import subset. It admits exact resumable continuation for deterministic internal stubs with explicit effect receipts and keeps host-backed state, sandbox delegation, nondeterministic input, and unsafe side effects on typed refusal paths.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.admitted_case_count = report.runtime_bundle.admitted_case_count;
    report.refusal_case_count = report.runtime_bundle.refusal_case_count;
    report.continuation_safe_effect_refs = report.runtime_bundle.continuation_safe_effect_refs.clone();
    report.continuation_refused_effect_refs =
        report.runtime_bundle.continuation_refused_effect_refs.clone();
    report.kernel_policy_dependency_marker = report.runtime_bundle.kernel_policy_dependency_marker.clone();
    report.world_mount_dependency_marker =
        report.runtime_bundle.world_mount_dependency_marker.clone();
    report.summary = format!(
        "Effect-safe resume report covers admitted_cases={}, refused_cases={}, safe_effect_refs={}, refused_effect_refs={}, and reuses effect taxonomy plus resumable promotion truth.",
        report.admitted_case_count,
        report.refusal_case_count,
        report.continuation_safe_effect_refs.len(),
        report.continuation_refused_effect_refs.len(),
    );
    report.report_digest = stable_digest(b"psionic_tassadar_effect_safe_resume_report|", &report);
    Ok((report, write_plans))
}

fn effect_receipt_artifact(
    receipt: Option<&TassadarEffectReceipt>,
) -> (Option<String>, Option<String>) {
    receipt.map_or((None, None), |receipt| {
        (
            Some(format!(
                "{}/{}_effect_receipt.json",
                TASSADAR_EFFECT_SAFE_RESUME_RUN_ROOT_REF, receipt.request_id
            )),
            Some(receipt.receipt_digest.clone()),
        )
    })
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

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, TassadarEffectSafeResumeReportError> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?).into_bytes())
}

#[cfg(test)]
pub fn load_tassadar_effect_safe_resume_report(
    path: impl AsRef<Path>,
) -> Result<TassadarEffectSafeResumeReport, TassadarEffectSafeResumeReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarEffectSafeResumeReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarEffectSafeResumeReportError::Decode {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_effect_safe_resume_report, load_tassadar_effect_safe_resume_report,
        tassadar_effect_safe_resume_report_path, write_tassadar_effect_safe_resume_report,
    };

    #[test]
    fn effect_safe_resume_report_is_machine_legible() {
        let report = build_tassadar_effect_safe_resume_report().expect("report");
        assert_eq!(
            report.target_profile_id,
            "tassadar.internal_compute.deterministic_import_subset.v1"
        );
        assert_eq!(report.admitted_case_count, 2);
        assert_eq!(report.refusal_case_count, 4);
        assert_eq!(report.continuation_safe_effect_refs, vec![String::from("env.clock_stub")]);
    }

    #[test]
    fn effect_safe_resume_report_matches_committed_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let expected = build_tassadar_effect_safe_resume_report()?;
        let committed = load_tassadar_effect_safe_resume_report(
            tassadar_effect_safe_resume_report_path(),
        )?;
        assert_eq!(committed, expected);
        Ok(())
    }

    #[test]
    fn write_effect_safe_resume_report_persists_current_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.path().join("tassadar_effect_safe_resume_report.json");
        let report = write_tassadar_effect_safe_resume_report(&path)?;
        let persisted: super::TassadarEffectSafeResumeReport =
            serde_json::from_slice(&std::fs::read(&path)?)?;
        assert_eq!(report, persisted);
        Ok(())
    }
}
