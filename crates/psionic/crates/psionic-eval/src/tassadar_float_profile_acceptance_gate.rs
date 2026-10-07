use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_ir::{
    TassadarMixedNumericSupportPosture, TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID,
    TASSADAR_NUMERIC_PROFILE_F32_ONLY_ID, TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID,
};
use psionic_runtime::TASSADAR_NUMERIC_PORTABILITY_REPORT_REF;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_float_semantics_comparison_matrix_report,
    build_tassadar_mixed_numeric_profile_ladder_report, build_tassadar_numeric_portability_report,
    TassadarFloatSemanticsReportError, TassadarMixedNumericProfileLadderReportError,
    TassadarNumericPortabilityReportError, TASSADAR_FLOAT_SEMANTICS_REPORT_REF,
    TASSADAR_MIXED_NUMERIC_PROFILE_LADDER_REPORT_REF,
};

pub const TASSADAR_FLOAT_PROFILE_ACCEPTANCE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_float_profile_acceptance_gate_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFloatProfileAcceptanceStatus {
    Green,
    Suppressed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFloatProfileAcceptanceGateRow {
    pub profile_id: String,
    pub support_posture: TassadarMixedNumericSupportPosture,
    pub float_evidence_ready: bool,
    pub mixed_numeric_evidence_ready: bool,
    pub portability_ready: bool,
    pub publication_allowed_row_count: u32,
    pub gate_status: TassadarFloatProfileAcceptanceStatus,
    pub public_profile_allowed: bool,
    pub default_served_profile_allowed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFloatProfileAcceptanceGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub float_semantics_report_ref: String,
    pub mixed_numeric_report_ref: String,
    pub numeric_portability_report_ref: String,
    pub profile_rows: Vec<TassadarFloatProfileAcceptanceGateRow>,
    pub green_profile_ids: Vec<String>,
    pub public_profile_allowed_profile_ids: Vec<String>,
    pub default_served_profile_allowed_profile_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub failed_profile_ids: Vec<String>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarFloatProfileAcceptanceGateReportError {
    #[error(transparent)]
    FloatSemantics(#[from] TassadarFloatSemanticsReportError),
    #[error(transparent)]
    MixedNumeric(#[from] TassadarMixedNumericProfileLadderReportError),
    #[error(transparent)]
    NumericPortability(#[from] TassadarNumericPortabilityReportError),
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

pub fn build_tassadar_float_profile_acceptance_gate_report(
) -> Result<TassadarFloatProfileAcceptanceGateReport, TassadarFloatProfileAcceptanceGateReportError>
{
    let float_report = build_tassadar_float_semantics_comparison_matrix_report()?;
    let mixed_numeric_report = build_tassadar_mixed_numeric_profile_ladder_report()?;
    let numeric_portability_report = build_tassadar_numeric_portability_report()?;

    let profile_rows = vec![
        build_exact_row(
            TASSADAR_NUMERIC_PROFILE_F32_ONLY_ID,
            &numeric_portability_report,
            !float_report.cases.is_empty(),
        ),
        build_exact_row(
            TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID,
            &numeric_portability_report,
            !mixed_numeric_report.cases.is_empty(),
        ),
        build_bounded_row(&numeric_portability_report),
    ];
    let green_profile_ids = profile_rows
        .iter()
        .filter(|row| row.gate_status == TassadarFloatProfileAcceptanceStatus::Green)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let public_profile_allowed_profile_ids = profile_rows
        .iter()
        .filter(|row| row.public_profile_allowed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let default_served_profile_allowed_profile_ids = profile_rows
        .iter()
        .filter(|row| row.default_served_profile_allowed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let suppressed_profile_ids = profile_rows
        .iter()
        .filter(|row| row.gate_status == TassadarFloatProfileAcceptanceStatus::Suppressed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let failed_profile_ids = profile_rows
        .iter()
        .filter(|row| row.gate_status == TassadarFloatProfileAcceptanceStatus::Failed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let overall_green = failed_profile_ids.is_empty();

    let mut report = TassadarFloatProfileAcceptanceGateReport {
        schema_version: 1,
        report_id: String::from("tassadar.float_profile_acceptance_gate.report.v1"),
        float_semantics_report_ref: String::from(TASSADAR_FLOAT_SEMANTICS_REPORT_REF),
        mixed_numeric_report_ref: String::from(TASSADAR_MIXED_NUMERIC_PROFILE_LADDER_REPORT_REF),
        numeric_portability_report_ref: String::from(TASSADAR_NUMERIC_PORTABILITY_REPORT_REF),
        profile_rows,
        green_profile_ids,
        public_profile_allowed_profile_ids,
        default_served_profile_allowed_profile_ids,
        suppressed_profile_ids,
        failed_profile_ids,
        overall_green,
        claim_boundary: String::from(
            "this gate keeps float-enabled publication bounded to named exact numeric profiles with explicit cpu-reference portability. A green row means the profile may influence public named-profile posture only; it does not imply default served publication, arbitrary Wasm float closure, backend-invariant float exactness, or full f64 promotion",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Float profile acceptance gate now records green_profiles={}, public_profile_allowed_profiles={}, default_served_profile_allowed_profiles={}, suppressed_profiles={}, failed_profiles={}, overall_green={}.",
        report.green_profile_ids.len(),
        report.public_profile_allowed_profile_ids.len(),
        report.default_served_profile_allowed_profile_ids.len(),
        report.suppressed_profile_ids.len(),
        report.failed_profile_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_float_profile_acceptance_gate_report|",
        &report,
    );
    Ok(report)
}

fn build_exact_row(
    profile_id: &str,
    numeric_portability_report: &psionic_runtime::TassadarNumericPortabilityReport,
    evidence_ready: bool,
) -> TassadarFloatProfileAcceptanceGateRow {
    let publication_allowed_row_count = numeric_portability_report
        .rows
        .iter()
        .filter(|row| row.profile_id == profile_id && row.publication_allowed)
        .count() as u32;
    let portability_ready = publication_allowed_row_count > 0;
    let gate_status = if evidence_ready && portability_ready {
        TassadarFloatProfileAcceptanceStatus::Green
    } else {
        TassadarFloatProfileAcceptanceStatus::Failed
    };
    TassadarFloatProfileAcceptanceGateRow {
        profile_id: String::from(profile_id),
        support_posture: TassadarMixedNumericSupportPosture::Exact,
        float_evidence_ready: evidence_ready,
        mixed_numeric_evidence_ready: evidence_ready,
        portability_ready,
        publication_allowed_row_count,
        gate_status,
        public_profile_allowed: gate_status == TassadarFloatProfileAcceptanceStatus::Green,
        default_served_profile_allowed: false,
        detail: if gate_status == TassadarFloatProfileAcceptanceStatus::Green {
            format!(
                "numeric profile `{profile_id}` is green for named public publication because exact evidence and cpu-reference portability rows are explicit, but it remains non-default for served publication"
            )
        } else {
            format!(
                "numeric profile `{profile_id}` fails the float gate because evidence or cpu-reference portability rows are incomplete"
            )
        },
    }
}

fn build_bounded_row(
    numeric_portability_report: &psionic_runtime::TassadarNumericPortabilityReport,
) -> TassadarFloatProfileAcceptanceGateRow {
    let profile_id = TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID;
    let publication_allowed_row_count = numeric_portability_report
        .rows
        .iter()
        .filter(|row| row.profile_id == profile_id && row.publication_allowed)
        .count() as u32;
    TassadarFloatProfileAcceptanceGateRow {
        profile_id: String::from(profile_id),
        support_posture: TassadarMixedNumericSupportPosture::BoundedApproximate,
        float_evidence_ready: true,
        mixed_numeric_evidence_ready: true,
        portability_ready: publication_allowed_row_count > 0,
        publication_allowed_row_count,
        gate_status: TassadarFloatProfileAcceptanceStatus::Suppressed,
        public_profile_allowed: false,
        default_served_profile_allowed: false,
        detail: String::from(
            "bounded-approximate f64 narrowing remains suppressed because the repo keeps approximate numeric widening separate from public exact-profile publication",
        ),
    }
}

pub fn tassadar_float_profile_acceptance_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_FLOAT_PROFILE_ACCEPTANCE_GATE_REPORT_REF)
}

pub fn write_tassadar_float_profile_acceptance_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarFloatProfileAcceptanceGateReport,
    TassadarFloatProfileAcceptanceGateReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarFloatProfileAcceptanceGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_float_profile_acceptance_gate_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("float profile acceptance gate serializes");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarFloatProfileAcceptanceGateReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_float_profile_acceptance_gate_report(
    path: impl AsRef<Path>,
) -> Result<
    TassadarFloatProfileAcceptanceGateReport,
    TassadarFloatProfileAcceptanceGateReportError,
> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarFloatProfileAcceptanceGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarFloatProfileAcceptanceGateReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .expect("workspace root")
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
        build_tassadar_float_profile_acceptance_gate_report,
        load_tassadar_float_profile_acceptance_gate_report,
        tassadar_float_profile_acceptance_gate_report_path,
        write_tassadar_float_profile_acceptance_gate_report,
    };

    #[test]
    fn float_profile_acceptance_gate_keeps_exact_profiles_green_and_bounded_f64_suppressed() {
        let report = build_tassadar_float_profile_acceptance_gate_report().expect("report");

        assert!(report
            .green_profile_ids
            .contains(&String::from("tassadar.numeric_profile.f32_only.v1")));
        assert!(report
            .green_profile_ids
            .contains(&String::from("tassadar.numeric_profile.mixed_i32_f32.v1")));
        assert!(!report.public_profile_allowed_profile_ids.is_empty());
        assert!(report
            .suppressed_profile_ids
            .contains(&String::from(
                "tassadar.numeric_profile.bounded_f64_conversion.v1"
            )));
        assert!(report.default_served_profile_allowed_profile_ids.is_empty());
    }

    #[test]
    fn float_profile_acceptance_gate_matches_committed_truth() {
        let generated = build_tassadar_float_profile_acceptance_gate_report().expect("report");
        let committed = load_tassadar_float_profile_acceptance_gate_report(
            tassadar_float_profile_acceptance_gate_report_path(),
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_float_profile_acceptance_gate_persists_current_truth() {
        let output_path =
            std::env::temp_dir().join("tassadar_float_profile_acceptance_gate_report.json");
        let generated =
            write_tassadar_float_profile_acceptance_gate_report(&output_path).expect("report");
        let reloaded =
            load_tassadar_float_profile_acceptance_gate_report(&output_path).expect("reloaded");
        assert_eq!(generated, reloaded);
        let _ = std::fs::remove_file(output_path);
    }
}
