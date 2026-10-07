use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_broad_family_specialization_report,
    build_tassadar_broad_internal_compute_acceptance_gate_report,
    build_tassadar_broad_internal_compute_portability_report,
    build_tassadar_hybrid_process_controller_report, TassadarBroadFamilySpecializationReport,
    TassadarBroadInternalComputeAcceptanceGateReport,
    TassadarBroadInternalComputeAcceptanceGateReportError,
    TassadarBroadInternalComputePortabilityReportError, TassadarHybridProcessControllerReport,
};

pub const TASSADAR_EFFECTIVE_UNBOUNDED_COMPUTE_CLAIM_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_effective_unbounded_compute_claim_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEffectiveUnboundedClaimStatus {
    Green,
    Suppressed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectiveUnboundedClaimPrerequisiteRow {
    pub prerequisite_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectiveUnboundedComputeClaimReport {
    pub schema_version: u16,
    pub report_id: String,
    pub prerequisite_rows: Vec<TassadarEffectiveUnboundedClaimPrerequisiteRow>,
    pub satisfied_prerequisite_ids: Vec<String>,
    pub missing_prerequisite_ids: Vec<String>,
    pub claim_status: TassadarEffectiveUnboundedClaimStatus,
    pub meaning_statement: String,
    pub out_of_scope_claims: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarEffectiveUnboundedComputeClaimReportError {
    #[error(transparent)]
    BroadGate(#[from] TassadarBroadInternalComputeAcceptanceGateReportError),
    #[error(transparent)]
    Portability(#[from] TassadarBroadInternalComputePortabilityReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_effective_unbounded_compute_claim_report() -> Result<
    TassadarEffectiveUnboundedComputeClaimReport,
    TassadarEffectiveUnboundedComputeClaimReportError,
> {
    let broad_gate = build_tassadar_broad_internal_compute_acceptance_gate_report()?;
    let portability_report = build_tassadar_broad_internal_compute_portability_report()?;
    let specialization_report = build_tassadar_broad_family_specialization_report();
    let hybrid_report = build_tassadar_hybrid_process_controller_report();

    let prerequisite_rows = prerequisite_rows(
        &broad_gate,
        &specialization_report,
        &hybrid_report,
        portability_report
            .publication_allowed_profile_ids
            .as_slice(),
    );
    let (claim_status, satisfied_prerequisite_ids, missing_prerequisite_ids) =
        evaluate_claim_status(prerequisite_rows.as_slice());
    let out_of_scope_claims = vec![
        String::from("arbitrary Wasm execution"),
        String::from("broad served internal compute"),
        String::from("arbitrary effectful process execution"),
        String::from("Turing-complete support"),
    ];
    let mut report = TassadarEffectiveUnboundedComputeClaimReport {
        schema_version: 1,
        report_id: String::from("tassadar.effective_unbounded_compute_claim.report.v1"),
        prerequisite_rows,
        satisfied_prerequisite_ids,
        missing_prerequisite_ids,
        claim_status,
        meaning_statement: String::from(
            "effective unbounded computation here means bounded execution slices plus resumable state, continuation, and refusal-safe effect handling under declared portability envelopes and explicit checkpoint or tape objects; it does not mean infinite in-core execution or arbitrary universality",
        ),
        out_of_scope_claims,
        claim_boundary: String::from(
            "this checker is a disclosure-safe claim-closure surface. It decides whether the repo may honestly say `effective unbounded computation under envelopes`, and it keeps that claim suppressed until portability, specialization safety, and broad publication prerequisites are all green",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Effective-unbounded claim checker records satisfied_prerequisites={}, missing_prerequisites={}, claim_status={:?}.",
        report.satisfied_prerequisite_ids.len(),
        report.missing_prerequisite_ids.len(),
        report.claim_status,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_effective_unbounded_compute_claim_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_effective_unbounded_compute_claim_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EFFECTIVE_UNBOUNDED_COMPUTE_CLAIM_REPORT_REF)
}

pub fn write_tassadar_effective_unbounded_compute_claim_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarEffectiveUnboundedComputeClaimReport,
    TassadarEffectiveUnboundedComputeClaimReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarEffectiveUnboundedComputeClaimReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_effective_unbounded_compute_claim_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarEffectiveUnboundedComputeClaimReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn prerequisite_rows(
    broad_gate: &TassadarBroadInternalComputeAcceptanceGateReport,
    specialization_report: &TassadarBroadFamilySpecializationReport,
    hybrid_report: &TassadarHybridProcessControllerReport,
    green_portable_profiles: &[String],
) -> Vec<TassadarEffectiveUnboundedClaimPrerequisiteRow> {
    vec![
        row(
            "resumable_continuation_objects",
            true,
            &[
                "fixtures/tassadar/reports/tassadar_process_object_report.json",
                "fixtures/tassadar/reports/tassadar_spill_tape_store_report.json",
                "fixtures/tassadar/reports/tassadar_installed_process_lifecycle_report.json",
            ],
            "checkpoint, spill-tape, and installed-process lifecycle artifacts exist, so resumable continuation objects are real under explicit envelopes",
        ),
        row(
            "effect_and_refusal_receipts",
            true,
            &[
                "fixtures/tassadar/reports/tassadar_effectful_replay_audit_report.json",
                "fixtures/tassadar/reports/tassadar_hybrid_process_controller_report.json",
            ],
            "effect receipts and typed refusal boundaries are explicit on the current bounded effect and hybrid surfaces",
        ),
        row(
            "hybrid_process_control",
            hybrid_report.hybrid_verifier_case_count > 0
                && !hybrid_report.unsupported_transition_case_ids.is_empty(),
            &["fixtures/tassadar/reports/tassadar_hybrid_process_controller_report.json"],
            "hybrid control is only counted when verifier-positive-delta cases and unsupported-transition refusals are both explicit",
        ),
        row(
            "portable_publication_envelope",
            green_portable_profiles.len() > 1,
            &["fixtures/tassadar/reports/tassadar_broad_internal_compute_portability_report.json"],
            "effective-unbounded claims require more than the article-closeout portability anchor; today the broader portability matrix is still too narrow",
        ),
        row(
            "broad_publication_gate",
            broad_gate.overall_green,
            &["fixtures/tassadar/reports/tassadar_broad_internal_compute_acceptance_gate.json"],
            "the broad internal-compute publication gate must be green before any effective-unbounded claim can be disclosed safely",
        ),
        row(
            "specialization_safety_gate",
            specialization_report.unstable_family_ids.is_empty()
                && specialization_report.non_decompilable_family_ids.is_empty()
                && !specialization_report.safety_gate_green_family_ids.is_empty(),
            &["fixtures/tassadar/reports/tassadar_broad_family_specialization_report.json"],
            "effective-unbounded claims require specialization families to stay decompilable and safe to challenge, not just benchmarked",
        ),
    ]
}

fn evaluate_claim_status(
    prerequisite_rows: &[TassadarEffectiveUnboundedClaimPrerequisiteRow],
) -> (
    TassadarEffectiveUnboundedClaimStatus,
    Vec<String>,
    Vec<String>,
) {
    let satisfied_prerequisite_ids = prerequisite_rows
        .iter()
        .filter(|row| row.satisfied)
        .map(|row| row.prerequisite_id.clone())
        .collect::<Vec<_>>();
    let missing_prerequisite_ids = prerequisite_rows
        .iter()
        .filter(|row| !row.satisfied)
        .map(|row| row.prerequisite_id.clone())
        .collect::<Vec<_>>();
    let claim_status = if missing_prerequisite_ids.is_empty() {
        TassadarEffectiveUnboundedClaimStatus::Green
    } else if satisfied_prerequisite_ids.is_empty() {
        TassadarEffectiveUnboundedClaimStatus::Failed
    } else {
        TassadarEffectiveUnboundedClaimStatus::Suppressed
    };
    (
        claim_status,
        satisfied_prerequisite_ids,
        missing_prerequisite_ids,
    )
}

fn row(
    prerequisite_id: &str,
    satisfied: bool,
    source_refs: &[&str],
    note: &str,
) -> TassadarEffectiveUnboundedClaimPrerequisiteRow {
    TassadarEffectiveUnboundedClaimPrerequisiteRow {
        prerequisite_id: String::from(prerequisite_id),
        satisfied,
        source_refs: source_refs
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

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarEffectiveUnboundedComputeClaimReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarEffectiveUnboundedComputeClaimReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarEffectiveUnboundedComputeClaimReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_effective_unbounded_compute_claim_report, evaluate_claim_status, read_json,
        tassadar_effective_unbounded_compute_claim_report_path,
        TassadarEffectiveUnboundedClaimPrerequisiteRow, TassadarEffectiveUnboundedClaimStatus,
        TassadarEffectiveUnboundedComputeClaimReport,
    };

    #[test]
    fn effective_unbounded_claim_checker_stays_suppressed_with_missing_prerequisites() {
        let report = build_tassadar_effective_unbounded_compute_claim_report().expect("report");

        assert_eq!(
            report.claim_status,
            TassadarEffectiveUnboundedClaimStatus::Suppressed
        );
        assert!(report
            .missing_prerequisite_ids
            .contains(&String::from("broad_publication_gate")));
        assert!(report
            .missing_prerequisite_ids
            .contains(&String::from("specialization_safety_gate")));
    }

    #[test]
    fn effective_unbounded_claim_checker_negative_rows_each_force_non_green_status() {
        let prerequisite_ids = [
            "resumable_continuation_objects",
            "effect_and_refusal_receipts",
            "hybrid_process_control",
            "portable_publication_envelope",
            "broad_publication_gate",
            "specialization_safety_gate",
        ];
        for prerequisite_id in prerequisite_ids {
            let rows = prerequisite_ids
                .iter()
                .map(|candidate| TassadarEffectiveUnboundedClaimPrerequisiteRow {
                    prerequisite_id: String::from(*candidate),
                    satisfied: *candidate != prerequisite_id,
                    source_refs: Vec::new(),
                    note: String::from("test"),
                })
                .collect::<Vec<_>>();
            let (status, _satisfied, missing) = evaluate_claim_status(rows.as_slice());
            assert_ne!(status, TassadarEffectiveUnboundedClaimStatus::Green);
            assert!(missing.contains(&String::from(prerequisite_id)));
        }
    }

    #[test]
    fn effective_unbounded_claim_checker_matches_committed_truth() {
        let generated = build_tassadar_effective_unbounded_compute_claim_report().expect("report");
        let committed: TassadarEffectiveUnboundedComputeClaimReport =
            read_json(tassadar_effective_unbounded_compute_claim_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }
}
