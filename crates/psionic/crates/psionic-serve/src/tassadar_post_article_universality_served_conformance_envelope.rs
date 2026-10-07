use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_post_article_universality_portability_minimality_matrix_report,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError,
};

use crate::{
    build_tassadar_article_cross_machine_reproducibility_publication,
    build_tassadar_article_route_minimality_publication_verdict,
    build_tassadar_broad_internal_compute_profile_publication,
    TassadarArticleCrossMachineReproducibilityPublication,
    TassadarArticleCrossMachineReproducibilityPublicationError,
    TassadarArticleRouteMinimalityPublicationVerdict,
    TassadarArticleRouteMinimalityPublicationVerdictError,
    TassadarBroadInternalComputeProfilePublication,
    TassadarBroadInternalComputeProfilePublicationError,
    TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_PUBLICATION_REF,
    TASSADAR_ARTICLE_ROUTE_MINIMALITY_PUBLICATION_VERDICT_REF,
};

pub const TASSADAR_POST_ARTICLE_UNIVERSALITY_SERVED_CONFORMANCE_ENVELOPE_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universality_served_conformance_envelope.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityServedConformanceEnvelope {
    pub schema_version: u16,
    pub publication_id: String,
    pub matrix_report_ref: String,
    pub route_minimality_publication_ref: String,
    pub cross_machine_reproducibility_publication_ref: String,
    pub served_profile_report_ref: String,
    pub served_route_policy_report_ref: String,
    pub current_served_internal_compute_profile_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub selected_decode_mode: String,
    pub supported_machine_class_ids: Vec<String>,
    pub allowed_narrower_deviation_ids: Vec<String>,
    pub required_identical_property_ids: Vec<String>,
    pub fail_closed_condition_ids: Vec<String>,
    pub matrix_green: bool,
    pub route_minimality_publication_green: bool,
    pub cross_machine_reproducibility_green: bool,
    pub served_suppression_boundary_preserved: bool,
    pub served_public_universality_allowed: bool,
    pub claim_boundary: String,
    pub publication_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleUniversalityServedConformanceEnvelopeError {
    #[error(transparent)]
    Matrix(#[from] TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError),
    #[error(transparent)]
    RouteMinimality(#[from] TassadarArticleRouteMinimalityPublicationVerdictError),
    #[error(transparent)]
    CrossMachine(#[from] TassadarArticleCrossMachineReproducibilityPublicationError),
    #[error(transparent)]
    BroadProfile(#[from] TassadarBroadInternalComputeProfilePublicationError),
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

pub fn build_tassadar_post_article_universality_served_conformance_envelope() -> Result<
    TassadarPostArticleUniversalityServedConformanceEnvelope,
    TassadarPostArticleUniversalityServedConformanceEnvelopeError,
> {
    let matrix = build_tassadar_post_article_universality_portability_minimality_matrix_report()?;
    let route_minimality = build_tassadar_article_route_minimality_publication_verdict()?;
    let cross_machine = build_tassadar_article_cross_machine_reproducibility_publication()?;
    let broad_profile = build_tassadar_broad_internal_compute_profile_publication()?;
    Ok(build_envelope_from_inputs(
        &matrix,
        &route_minimality,
        &cross_machine,
        &broad_profile,
    ))
}

fn build_envelope_from_inputs(
    matrix: &TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    route_minimality: &TassadarArticleRouteMinimalityPublicationVerdict,
    cross_machine: &TassadarArticleCrossMachineReproducibilityPublication,
    broad_profile: &TassadarBroadInternalComputeProfilePublication,
) -> TassadarPostArticleUniversalityServedConformanceEnvelope {
    let allowed_narrower_deviation_ids = vec![
        String::from("served_posture_may_remain_narrower_than_operator_universality"),
        String::from("served_posture_may_expose_only_direct_article_route"),
        String::from("served_posture_may_remain_cpu_only_with_explicit_suppression_elsewhere"),
    ];
    let required_identical_property_ids = vec![
        String::from("canonical_route_id_matches_bridge_identity"),
        String::from("canonical_route_descriptor_digest_matches_bridge_identity"),
        String::from("selected_decode_mode_matches_direct_article_route"),
        String::from("supported_machine_classes_stay_inside_declared_cpu_matrix"),
        String::from("deterministic_direct_route_only"),
    ];
    let fail_closed_condition_ids = vec![
        String::from("route_drift_or_descriptor_change"),
        String::from("machine_outside_declared_cpu_matrix"),
        String::from("nonselected_fast_route_claimed_as_universal"),
        String::from("resumable_or_public_universality_widening_attempted"),
        String::from("plugin_capability_implication_attempted"),
    ];
    let mut envelope = TassadarPostArticleUniversalityServedConformanceEnvelope {
        schema_version: 1,
        publication_id: String::from(
            "tassadar.post_article_universality_served_conformance_envelope.v1",
        ),
        matrix_report_ref: String::from(
            psionic_eval::TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
        ),
        route_minimality_publication_ref: String::from(
            TASSADAR_ARTICLE_ROUTE_MINIMALITY_PUBLICATION_VERDICT_REF,
        ),
        cross_machine_reproducibility_publication_ref: String::from(
            TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_PUBLICATION_REF,
        ),
        served_profile_report_ref: broad_profile.report_ref.clone(),
        served_route_policy_report_ref: broad_profile.route_policy_report_ref.clone(),
        current_served_internal_compute_profile_id: broad_profile
            .current_served_profile_id
            .clone(),
        canonical_route_id: matrix.canonical_route_id.clone(),
        canonical_route_descriptor_digest: route_minimality.route_descriptor_digest.clone(),
        selected_decode_mode: route_minimality.selected_decode_mode.clone(),
        supported_machine_class_ids: cross_machine.supported_machine_class_ids.clone(),
        allowed_narrower_deviation_ids,
        required_identical_property_ids,
        fail_closed_condition_ids,
        matrix_green: matrix.matrix_green,
        route_minimality_publication_green: route_minimality.public_verdict_green,
        cross_machine_reproducibility_green: cross_machine.reproducibility_matrix_green,
        served_suppression_boundary_preserved: matrix.served_suppression_boundary_preserved,
        served_public_universality_allowed: matrix.served_public_universality_allowed,
        claim_boundary: String::from(
            "this served conformance envelope freezes the narrow served posture that may exist below the rebased operator universality claim. The served lane may stay narrower than the operator lane, but it must keep the canonical direct article route identity, route descriptor digest, selected decode mode, and declared CPU machine matrix explicit; any route drift, undeclared machine widening, fast-route carrier laundering, or implication of served/public universality or plugin capability must fail closed.",
        ),
        publication_digest: String::new(),
    };
    envelope.publication_digest = stable_digest(
        b"psionic_tassadar_post_article_universality_served_conformance_envelope|",
        &envelope,
    );
    envelope
}

#[must_use]
pub fn tassadar_post_article_universality_served_conformance_envelope_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_UNIVERSALITY_SERVED_CONFORMANCE_ENVELOPE_REF)
}

pub fn write_tassadar_post_article_universality_served_conformance_envelope(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleUniversalityServedConformanceEnvelope,
    TassadarPostArticleUniversalityServedConformanceEnvelopeError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleUniversalityServedConformanceEnvelopeError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let envelope = build_tassadar_post_article_universality_served_conformance_envelope()?;
    let json = serde_json::to_string_pretty(&envelope)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleUniversalityServedConformanceEnvelopeError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(envelope)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-serve should live under <repo>/crates/psionic-serve")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarPostArticleUniversalityServedConformanceEnvelopeError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleUniversalityServedConformanceEnvelopeError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleUniversalityServedConformanceEnvelopeError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_universality_served_conformance_envelope, read_repo_json,
        tassadar_post_article_universality_served_conformance_envelope_path,
        write_tassadar_post_article_universality_served_conformance_envelope,
        TassadarPostArticleUniversalityServedConformanceEnvelope,
        TASSADAR_POST_ARTICLE_UNIVERSALITY_SERVED_CONFORMANCE_ENVELOPE_REF,
    };

    #[test]
    fn post_article_universality_served_conformance_envelope_stays_fail_closed() {
        let envelope = build_tassadar_post_article_universality_served_conformance_envelope()
            .expect("envelope");

        assert_eq!(
            envelope.current_served_internal_compute_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
        assert_eq!(
            envelope.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(envelope.supported_machine_class_ids.len(), 2);
        assert_eq!(envelope.allowed_narrower_deviation_ids.len(), 3);
        assert_eq!(envelope.required_identical_property_ids.len(), 5);
        assert_eq!(envelope.fail_closed_condition_ids.len(), 5);
        assert!(envelope.matrix_green);
        assert!(envelope.route_minimality_publication_green);
        assert!(envelope.cross_machine_reproducibility_green);
        assert!(envelope.served_suppression_boundary_preserved);
        assert!(!envelope.served_public_universality_allowed);
    }

    #[test]
    fn post_article_universality_served_conformance_envelope_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_post_article_universality_served_conformance_envelope()?;
        let committed: TassadarPostArticleUniversalityServedConformanceEnvelope = read_repo_json(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_SERVED_CONFORMANCE_ENVELOPE_REF,
            "post_article_universality_served_conformance_envelope",
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_post_article_universality_served_conformance_envelope_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_post_article_universality_served_conformance_envelope.json");
        let written =
            write_tassadar_post_article_universality_served_conformance_envelope(&output_path)?;
        let persisted: TassadarPostArticleUniversalityServedConformanceEnvelope =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_post_article_universality_served_conformance_envelope_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_universality_served_conformance_envelope.json")
        );
        Ok(())
    }
}
