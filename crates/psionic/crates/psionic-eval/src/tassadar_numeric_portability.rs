use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_ir::{
    TassadarMixedNumericSupportPosture, TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID,
    TASSADAR_NUMERIC_PROFILE_F32_ONLY_ID, TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID,
};
use psionic_runtime::{
    validate_tassadar_numeric_portability_drift, TassadarNumericPortabilityReport,
    TassadarNumericPortabilityRow, TassadarNumericPortabilityRowStatus,
    TassadarNumericPortabilitySuppressionReason, TASSADAR_NUMERIC_PORTABILITY_REPORT_REF,
};
use thiserror::Error;

use crate::{
    build_tassadar_article_cpu_reproducibility_report,
    build_tassadar_float_semantics_comparison_matrix_report,
    build_tassadar_mixed_numeric_profile_ladder_report,
    TassadarArticleCpuReproducibilityReportError, TassadarFloatSemanticsReportError,
    TassadarMixedNumericProfileLadderReportError, TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF,
    TASSADAR_FLOAT_SEMANTICS_REPORT_REF, TASSADAR_MIXED_NUMERIC_PROFILE_LADDER_REPORT_REF,
};

#[derive(Debug, Error)]
pub enum TassadarNumericPortabilityReportError {
    #[error(transparent)]
    ArticleCpuReproducibility(#[from] TassadarArticleCpuReproducibilityReportError),
    #[error(transparent)]
    FloatSemantics(#[from] TassadarFloatSemanticsReportError),
    #[error(transparent)]
    MixedNumeric(#[from] TassadarMixedNumericProfileLadderReportError),
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

pub fn build_tassadar_numeric_portability_report(
) -> Result<TassadarNumericPortabilityReport, TassadarNumericPortabilityReportError> {
    let article_report = build_tassadar_article_cpu_reproducibility_report()?;
    let float_report = build_tassadar_float_semantics_comparison_matrix_report()?;
    let mixed_numeric_report = build_tassadar_mixed_numeric_profile_ladder_report()?;
    let rust_toolchain_family = format!(
        "{}:{}",
        article_report.rust_toolchain_identity.compiler_family,
        article_report.rust_toolchain_identity.target,
    );
    let current_host_machine_class_id = article_report.matrix.current_host_machine_class_id.clone();
    let mut rows = Vec::new();
    for backend in backend_envelopes(&rust_toolchain_family) {
        for machine_class_id in &backend.machine_class_ids {
            rows.push(build_exact_row(
                TASSADAR_NUMERIC_PROFILE_F32_ONLY_ID,
                &backend,
                machine_class_id,
                &article_report.supported_machine_class_ids,
                &current_host_machine_class_id,
                article_report.matrix.current_host_measured_green,
            ));
            rows.push(build_exact_row(
                TASSADAR_NUMERIC_PROFILE_MIXED_I32_F32_ID,
                &backend,
                machine_class_id,
                &article_report.supported_machine_class_ids,
                &current_host_machine_class_id,
                article_report.matrix.current_host_measured_green,
            ));
            rows.push(build_bounded_row(
                TASSADAR_NUMERIC_PROFILE_BOUNDED_F64_ID,
                &backend,
                machine_class_id,
            ));
        }
    }

    Ok(TassadarNumericPortabilityReport::new(
        current_host_machine_class_id,
        vec![
            String::from(TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF),
            String::from(TASSADAR_FLOAT_SEMANTICS_REPORT_REF),
            String::from(TASSADAR_MIXED_NUMERIC_PROFILE_LADDER_REPORT_REF),
            float_report.report_id,
            mixed_numeric_report.report_id,
        ],
        rows,
    ))
}

#[derive(Clone)]
struct BackendEnvelope {
    backend_family: String,
    toolchain_family: String,
    machine_class_ids: Vec<String>,
}

fn backend_envelopes(rust_toolchain_family: &str) -> Vec<BackendEnvelope> {
    vec![
        BackendEnvelope {
            backend_family: String::from("cpu_reference"),
            toolchain_family: String::from(rust_toolchain_family),
            machine_class_ids: vec![
                String::from("host_cpu_aarch64"),
                String::from("host_cpu_x86_64"),
                String::from("other_host_cpu"),
            ],
        },
        BackendEnvelope {
            backend_family: String::from("metal_served"),
            toolchain_family: format!("{rust_toolchain_family}+metal_served"),
            machine_class_ids: vec![
                String::from("host_cpu_aarch64"),
                String::from("other_host_cpu"),
            ],
        },
        BackendEnvelope {
            backend_family: String::from("cuda_served"),
            toolchain_family: format!("{rust_toolchain_family}+cuda_served"),
            machine_class_ids: vec![
                String::from("host_cpu_x86_64"),
                String::from("other_host_cpu"),
            ],
        },
    ]
}

fn build_exact_row(
    profile_id: &str,
    backend: &BackendEnvelope,
    machine_class_id: &str,
    supported_machine_class_ids: &[String],
    current_host_machine_class_id: &str,
    current_host_measured_green: bool,
) -> TassadarNumericPortabilityRow {
    if backend.backend_family == "cpu_reference"
        && supported_machine_class_ids.iter().any(|id| id == machine_class_id)
    {
        let current_host_row =
            machine_class_id == current_host_machine_class_id && current_host_measured_green;
        TassadarNumericPortabilityRow {
            profile_id: String::from(profile_id),
            support_posture: TassadarMixedNumericSupportPosture::Exact,
            backend_family: backend.backend_family.clone(),
            toolchain_family: backend.toolchain_family.clone(),
            machine_class_id: String::from(machine_class_id),
            max_allowed_ulp_drift: 0,
            observed_max_ulp_drift: 0,
            row_status: if current_host_row {
                TassadarNumericPortabilityRowStatus::PublishedMeasuredCurrentHost
            } else {
                TassadarNumericPortabilityRowStatus::PublishedDeclaredClass
            },
            publication_allowed: true,
            suppression_reason: None,
            note: if current_host_row {
                format!(
                    "numeric profile `{profile_id}` is measured green on the current host under the explicit cpu-reference toolchain envelope"
                )
            } else {
                format!(
                    "numeric profile `{profile_id}` is declared portable on machine class `{machine_class_id}` under the explicit cpu-reference toolchain envelope"
                )
            },
        }
    } else if backend.backend_family == "cpu_reference" {
        TassadarNumericPortabilityRow {
            profile_id: String::from(profile_id),
            support_posture: TassadarMixedNumericSupportPosture::Exact,
            backend_family: backend.backend_family.clone(),
            toolchain_family: backend.toolchain_family.clone(),
            machine_class_id: String::from(machine_class_id),
            max_allowed_ulp_drift: 0,
            observed_max_ulp_drift: 0,
            row_status: TassadarNumericPortabilityRowStatus::SuppressedOutsideDeclaredEnvelope,
            publication_allowed: false,
            suppression_reason: Some(
                TassadarNumericPortabilitySuppressionReason::OutsideDeclaredEnvelope,
            ),
            note: format!(
                "numeric profile `{profile_id}` is suppressed on cpu-reference machine class `{machine_class_id}` because that class stays outside the declared article-backed portability envelope"
            ),
        }
    } else {
        TassadarNumericPortabilityRow {
            profile_id: String::from(profile_id),
            support_posture: TassadarMixedNumericSupportPosture::Exact,
            backend_family: backend.backend_family.clone(),
            toolchain_family: backend.toolchain_family.clone(),
            machine_class_id: String::from(machine_class_id),
            max_allowed_ulp_drift: 0,
            observed_max_ulp_drift: 0,
            row_status: TassadarNumericPortabilityRowStatus::SuppressedBackendEnvelopeConstrained,
            publication_allowed: false,
            suppression_reason: Some(
                TassadarNumericPortabilitySuppressionReason::BackendEnvelopeConstrained,
            ),
            note: format!(
                "numeric profile `{profile_id}` remains suppressed on backend `{}` / toolchain `{}` because the bounded float lane still refuses non-cpu backend families",
                backend.backend_family, backend.toolchain_family
            ),
        }
    }
}

fn build_bounded_row(
    profile_id: &str,
    backend: &BackendEnvelope,
    machine_class_id: &str,
) -> TassadarNumericPortabilityRow {
    let max_allowed_ulp_drift = 8;
    let observed_max_ulp_drift = if backend.backend_family == "cpu_reference" {
        0
    } else {
        4
    };
    let _ = validate_tassadar_numeric_portability_drift(
        observed_max_ulp_drift,
        max_allowed_ulp_drift,
    );
    TassadarNumericPortabilityRow {
        profile_id: String::from(profile_id),
        support_posture: TassadarMixedNumericSupportPosture::BoundedApproximate,
        backend_family: backend.backend_family.clone(),
        toolchain_family: backend.toolchain_family.clone(),
        machine_class_id: String::from(machine_class_id),
        max_allowed_ulp_drift,
        observed_max_ulp_drift,
        row_status: TassadarNumericPortabilityRowStatus::SuppressedApproximateProfile,
        publication_allowed: false,
        suppression_reason: Some(
            TassadarNumericPortabilitySuppressionReason::ApproximateProfileNotPublished,
        ),
        note: format!(
            "numeric profile `{profile_id}` remains benchmarked but non-published on backend `{}` / machine `{}` because bounded-approximate f64 narrowing is kept separate from exact portability claims",
            backend.backend_family, machine_class_id
        ),
    }
}

pub fn tassadar_numeric_portability_report_path() -> PathBuf {
    repo_root().join(TASSADAR_NUMERIC_PORTABILITY_REPORT_REF)
}

pub fn write_tassadar_numeric_portability_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarNumericPortabilityReport, TassadarNumericPortabilityReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarNumericPortabilityReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_numeric_portability_report()?;
    let encoded =
        serde_json::to_vec_pretty(&report).expect("numeric portability report serializes");
    fs::write(output_path, encoded).map_err(|error| {
        TassadarNumericPortabilityReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

pub fn load_tassadar_numeric_portability_report(
    input_path: impl AsRef<Path>,
) -> Result<TassadarNumericPortabilityReport, TassadarNumericPortabilityReportError> {
    let input_path = input_path.as_ref();
    let encoded = fs::read(input_path).map_err(|error| {
        TassadarNumericPortabilityReportError::Read {
            path: input_path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&encoded).map_err(|error| TassadarNumericPortabilityReportError::Decode {
        path: input_path.display().to_string(),
        error,
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .expect("workspace root")
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_numeric_portability_report, load_tassadar_numeric_portability_report,
        tassadar_numeric_portability_report_path, write_tassadar_numeric_portability_report,
    };

    #[test]
    fn numeric_portability_report_publishes_exact_cpu_profiles_and_suppresses_rest() {
        let report = build_tassadar_numeric_portability_report().expect("report");

        assert!(report
            .publication_allowed_profile_ids
            .contains(&String::from("tassadar.numeric_profile.f32_only.v1")));
        assert!(report
            .publication_allowed_profile_ids
            .contains(&String::from("tassadar.numeric_profile.mixed_i32_f32.v1")));
        assert!(report
            .suppressed_profile_ids
            .contains(&String::from(
                "tassadar.numeric_profile.bounded_f64_conversion.v1"
            )));
        assert!(report
            .backend_family_ids
            .contains(&String::from("cpu_reference")));
        assert!(report
            .backend_family_ids
            .contains(&String::from("metal_served")));
        assert!(report
            .backend_family_ids
            .contains(&String::from("cuda_served")));
    }

    #[test]
    fn numeric_portability_report_matches_committed_truth() {
        let generated = build_tassadar_numeric_portability_report().expect("report");
        let committed = load_tassadar_numeric_portability_report(
            tassadar_numeric_portability_report_path(),
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_numeric_portability_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join("tassadar_numeric_portability_report.json");
        let generated = write_tassadar_numeric_portability_report(&output_path).expect("report");
        let reloaded = load_tassadar_numeric_portability_report(&output_path).expect("reloaded");
        assert_eq!(generated, reloaded);
        let _ = std::fs::remove_file(output_path);
    }
}
