use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use thiserror::Error;

use psionic_models::{
    tassadar_internal_compute_profile_ladder_publication,
    TassadarInternalComputePortabilityPosture, TassadarInternalComputeProfileStatus,
    TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_REPORT_REF,
};
use psionic_runtime::{
    build_tassadar_quantization_truth_envelope_runtime_report,
    TassadarBroadInternalComputePortabilityReport, TassadarBroadInternalComputePortabilityRow,
    TassadarBroadInternalComputePortabilityRowStatus,
    TassadarBroadInternalComputeSuppressionReason, TassadarQuantizationBackendFamily,
    TassadarQuantizationEnvelopePosture, TASSADAR_BROAD_INTERNAL_COMPUTE_PORTABILITY_REPORT_REF,
    TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_RUNTIME_REPORT_REF,
};

use crate::{
    build_tassadar_article_cpu_reproducibility_report,
    TassadarArticleCpuReproducibilityReportError, TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF,
};

#[derive(Debug, Error)]
pub enum TassadarBroadInternalComputePortabilityReportError {
    #[error(transparent)]
    ArticleCpuReproducibility(#[from] TassadarArticleCpuReproducibilityReportError),
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

pub fn build_tassadar_broad_internal_compute_portability_report() -> Result<
    TassadarBroadInternalComputePortabilityReport,
    TassadarBroadInternalComputePortabilityReportError,
> {
    let article_report = build_tassadar_article_cpu_reproducibility_report()?;
    let quantization_report = build_tassadar_quantization_truth_envelope_runtime_report();
    let ladder = tassadar_internal_compute_profile_ladder_publication();
    let rust_toolchain_family = format!(
        "{}:{}",
        article_report.rust_toolchain_identity.compiler_family,
        article_report.rust_toolchain_identity.target,
    );
    let current_host_machine_class_id = article_report.matrix.current_host_machine_class_id.clone();
    let backend_envelopes = backend_envelopes(
        rust_toolchain_family.as_str(),
        quantization_report
            .envelope_receipts
            .iter()
            .map(|receipt| (receipt.backend_family, receipt.publication_posture))
            .collect::<BTreeSet<_>>(),
    );

    let article_supported_machine_class_ids = article_report.supported_machine_class_ids.clone();
    let current_host_measured_green = article_report.matrix.current_host_measured_green;
    let mut rows = Vec::new();
    for profile in &ladder.profiles {
        for envelope in &backend_envelopes {
            for machine_class_id in &envelope.machine_class_ids {
                rows.push(portability_row_for_profile(
                    profile,
                    envelope,
                    machine_class_id,
                    current_host_machine_class_id.as_str(),
                    article_supported_machine_class_ids.as_slice(),
                    current_host_measured_green,
                ));
            }
        }
    }

    Ok(TassadarBroadInternalComputePortabilityReport::new(
        current_host_machine_class_id,
        vec![
            String::from(TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF),
            String::from(TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_REPORT_REF),
            String::from(TASSADAR_QUANTIZATION_TRUTH_ENVELOPE_RUNTIME_REPORT_REF),
        ],
        rows,
    ))
}

#[derive(Clone)]
struct BackendEnvelope {
    backend_family: String,
    toolchain_family: String,
    machine_class_ids: Vec<String>,
    portable_publication: bool,
}

fn backend_envelopes(
    rust_toolchain_family: &str,
    quantization_envelopes: BTreeSet<(
        TassadarQuantizationBackendFamily,
        TassadarQuantizationEnvelopePosture,
    )>,
) -> Vec<BackendEnvelope> {
    let cpu_portable = quantization_envelopes.contains(&(
        TassadarQuantizationBackendFamily::CpuReference,
        TassadarQuantizationEnvelopePosture::PublishExact,
    ));
    let metal_portable = quantization_envelopes.iter().any(|(backend, posture)| {
        *backend == TassadarQuantizationBackendFamily::MetalServed
            && *posture == TassadarQuantizationEnvelopePosture::PublishExact
    });
    let cuda_portable = quantization_envelopes.iter().any(|(backend, posture)| {
        *backend == TassadarQuantizationBackendFamily::CudaServed
            && *posture == TassadarQuantizationEnvelopePosture::PublishExact
    });

    vec![
        BackendEnvelope {
            backend_family: String::from("cpu_reference"),
            toolchain_family: String::from(rust_toolchain_family),
            machine_class_ids: vec![
                String::from("host_cpu_aarch64"),
                String::from("host_cpu_x86_64"),
                String::from("other_host_cpu"),
            ],
            portable_publication: cpu_portable,
        },
        BackendEnvelope {
            backend_family: String::from("metal_served"),
            toolchain_family: format!("{rust_toolchain_family}+metal_served"),
            machine_class_ids: vec![
                String::from("host_cpu_aarch64"),
                String::from("other_host_cpu"),
            ],
            portable_publication: metal_portable,
        },
        BackendEnvelope {
            backend_family: String::from("cuda_served"),
            toolchain_family: format!("{rust_toolchain_family}+cuda_served"),
            machine_class_ids: vec![
                String::from("host_cpu_x86_64"),
                String::from("other_host_cpu"),
            ],
            portable_publication: cuda_portable,
        },
    ]
}

fn portability_row_for_profile(
    profile: &psionic_models::TassadarInternalComputeProfileSpec,
    envelope: &BackendEnvelope,
    machine_class_id: &str,
    current_host_machine_class_id: &str,
    article_supported_machine_class_ids: &[String],
    current_host_measured_green: bool,
) -> TassadarBroadInternalComputePortabilityRow {
    let profile_supported_machine_class = profile
        .supported_machine_class_ids
        .iter()
        .any(|id| id == machine_class_id);
    let backend_supported_machine_class = envelope
        .machine_class_ids
        .iter()
        .any(|id| id == machine_class_id);
    let evidence_complete = required_evidence_refs_resolved(&profile.required_evidence_refs);
    let refusal_suite_complete = !profile.refusal_classes.is_empty();
    let fully_portable = profile.status == TassadarInternalComputeProfileStatus::Implemented
        && profile.portability_posture
            == TassadarInternalComputePortabilityPosture::DeclaredCpuMatrix
        && evidence_complete
        && refusal_suite_complete
        && profile_supported_machine_class
        && envelope.backend_family == "cpu_reference"
        && envelope.portable_publication;

    let (row_status, publication_allowed, suppression_reason, note) =
        if !backend_supported_machine_class {
            (
            TassadarBroadInternalComputePortabilityRowStatus::SuppressedDriftedOutsideEnvelope,
            false,
            Some(TassadarBroadInternalComputeSuppressionReason::OutsideDeclaredEnvelope),
            format!(
                "profile `{}` is suppressed on backend `{}` / machine `{}` because that machine class drifts outside the declared backend envelope",
                profile.profile_id, envelope.backend_family, machine_class_id
            ),
        )
        } else if fully_portable {
            if machine_class_id == current_host_machine_class_id && current_host_measured_green {
                (
                TassadarBroadInternalComputePortabilityRowStatus::PublishedMeasuredCurrentHost,
                true,
                None,
                format!(
                    "profile `{}` is measured green on the current host and stays inside the declared `{}` backend envelope",
                    profile.profile_id, envelope.backend_family
                ),
            )
            } else {
                (
                TassadarBroadInternalComputePortabilityRowStatus::PublishedDeclaredClass,
                true,
                None,
                format!(
                    "profile `{}` is admitted on declared machine class `{}` under the explicit `{}` backend envelope",
                    profile.profile_id, machine_class_id, envelope.backend_family
                ),
            )
            }
        } else if !profile_supported_machine_class {
            (
            TassadarBroadInternalComputePortabilityRowStatus::SuppressedDriftedOutsideEnvelope,
            false,
            Some(TassadarBroadInternalComputeSuppressionReason::OutsideDeclaredEnvelope),
            format!(
                "profile `{}` is suppressed on backend `{}` / machine `{}` because that machine class drifts outside the declared profile envelope",
                profile.profile_id, envelope.backend_family, machine_class_id
            ),
        )
        } else if profile.status != TassadarInternalComputeProfileStatus::Implemented {
            (
            TassadarBroadInternalComputePortabilityRowStatus::SuppressedPendingPortabilityEvidence,
            false,
            Some(TassadarBroadInternalComputeSuppressionReason::ProfileNotImplemented),
            format!(
                "profile `{}` remains suppressed on backend `{}` because the named profile is still planned rather than implemented",
                profile.profile_id, envelope.backend_family
            ),
        )
        } else if profile.portability_posture
            == TassadarInternalComputePortabilityPosture::DeclaredCpuMatrix
            && envelope.backend_family != "cpu_reference"
            && article_supported_machine_class_ids
                .iter()
                .any(|id| id == machine_class_id)
        {
            (
            TassadarBroadInternalComputePortabilityRowStatus::SuppressedBackendEnvelopeConstrained,
            false,
            Some(TassadarBroadInternalComputeSuppressionReason::BackendEnvelopeConstrained),
            format!(
                "profile `{}` remains suppressed on backend `{}` / toolchain `{}` because only the CPU-reference portability envelope is currently declared for public promotion",
                profile.profile_id, envelope.backend_family, envelope.toolchain_family
            ),
        )
        } else {
            (
            TassadarBroadInternalComputePortabilityRowStatus::SuppressedPendingPortabilityEvidence,
            false,
            Some(TassadarBroadInternalComputeSuppressionReason::PortabilityEvidenceIncomplete),
            format!(
                "profile `{}` remains suppressed on backend `{}` / machine `{}` because portability posture or required evidence is still incomplete",
                profile.profile_id, envelope.backend_family, machine_class_id
            ),
        )
        };

    TassadarBroadInternalComputePortabilityRow {
        profile_id: profile.profile_id.clone(),
        backend_family: envelope.backend_family.clone(),
        toolchain_family: envelope.toolchain_family.clone(),
        machine_class_id: String::from(machine_class_id),
        row_status,
        publication_allowed,
        evidence_complete,
        refusal_suite_complete,
        suppression_reason,
        note,
    }
}

fn required_evidence_refs_resolved(required_evidence_refs: &[String]) -> bool {
    let repo_root = repo_root();
    required_evidence_refs.iter().all(|reference| {
        !reference.trim().is_empty()
            && !reference.starts_with("issue://")
            && repo_root.join(reference).exists()
    })
}

pub fn tassadar_broad_internal_compute_portability_report_path() -> PathBuf {
    repo_root().join(TASSADAR_BROAD_INTERNAL_COMPUTE_PORTABILITY_REPORT_REF)
}

pub fn write_tassadar_broad_internal_compute_portability_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarBroadInternalComputePortabilityReport,
    TassadarBroadInternalComputePortabilityReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarBroadInternalComputePortabilityReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_broad_internal_compute_portability_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("broad internal compute portability serializes");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarBroadInternalComputePortabilityReportError::Write {
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

#[cfg(test)]
pub fn load_tassadar_broad_internal_compute_portability_report(
    path: impl AsRef<Path>,
) -> Result<
    TassadarBroadInternalComputePortabilityReport,
    TassadarBroadInternalComputePortabilityReportError,
> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarBroadInternalComputePortabilityReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarBroadInternalComputePortabilityReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_broad_internal_compute_portability_report,
        load_tassadar_broad_internal_compute_portability_report,
        tassadar_broad_internal_compute_portability_report_path,
    };
    use psionic_runtime::TassadarBroadInternalComputePortabilityRowStatus;

    #[test]
    fn broad_internal_compute_portability_report_publishes_article_and_suppresses_broader_profiles()
    {
        let report = build_tassadar_broad_internal_compute_portability_report().expect("report");
        assert!(report
            .publication_allowed_profile_ids
            .contains(&String::from(
                "tassadar.internal_compute.article_closeout.v1"
            )));
        assert!(report.suppressed_profile_ids.contains(&String::from(
            "tassadar.internal_compute.generalized_abi.v1"
        )));
        assert!(report.rows.iter().any(|row| {
            row.profile_id == "tassadar.internal_compute.article_closeout.v1"
                && row.row_status
                    == TassadarBroadInternalComputePortabilityRowStatus::PublishedMeasuredCurrentHost
        }));
        assert!(report
            .backend_family_ids
            .contains(&String::from("metal_served")));
        assert!(report
            .toolchain_family_ids
            .iter()
            .any(|id| id.contains("cuda_served")));
        assert!(report.rows.iter().any(|row| {
            row.profile_id == "tassadar.internal_compute.article_closeout.v1"
                && row.backend_family == "metal_served"
                && row.row_status
                    == TassadarBroadInternalComputePortabilityRowStatus::SuppressedBackendEnvelopeConstrained
        }));
    }

    #[test]
    fn broad_internal_compute_portability_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let expected = build_tassadar_broad_internal_compute_portability_report()?;
        let committed = load_tassadar_broad_internal_compute_portability_report(
            tassadar_broad_internal_compute_portability_report_path(),
        )?;
        assert_eq!(committed, expected);
        Ok(())
    }
}
