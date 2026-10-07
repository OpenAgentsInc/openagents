use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleRouteMinimalityAuditReport, TassadarArticleRouteMinimalityPublicPosture,
    TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
};
use psionic_models::TassadarExecutorFixture;
use psionic_research::TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_SUMMARY_REF;

use crate::EXECUTOR_TRACE_PRODUCT_ID;

pub const TASSADAR_ARTICLE_ROUTE_MINIMALITY_PUBLICATION_VERDICT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_route_minimality_publication_verdict.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRouteMinimalityPublicationVerdict {
    pub product_id: String,
    pub model_id: String,
    pub report_ref: String,
    pub summary_ref: String,
    pub canonical_claim_route_id: String,
    pub route_descriptor_digest: String,
    pub selected_decode_mode: String,
    pub operator_verdict_green: bool,
    pub public_posture: TassadarArticleRouteMinimalityPublicPosture,
    pub public_verdict_green: bool,
    pub public_blocked_issue_ids: Vec<String>,
    pub route_minimality_audit_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub publication_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleRouteMinimalityPublicationVerdictError {
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
    #[error("route-minimality verdict is not publishable: {detail}")]
    Invalid { detail: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_route_minimality_publication_verdict() -> Result<
    TassadarArticleRouteMinimalityPublicationVerdict,
    TassadarArticleRouteMinimalityPublicationVerdictError,
> {
    let report: TassadarArticleRouteMinimalityAuditReport = read_repo_json(
        TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
        "article_route_minimality_audit_report",
    )?;
    if !report.route_minimality_audit_green {
        return Err(
            TassadarArticleRouteMinimalityPublicationVerdictError::Invalid {
                detail: String::from(
                    "route-minimality audit must be green before publication verdict is allowed",
                ),
            },
        );
    }
    let mut publication = TassadarArticleRouteMinimalityPublicationVerdict {
        product_id: String::from(EXECUTOR_TRACE_PRODUCT_ID),
        model_id: if report.canonical_claim_route_review.transformer_model_id.is_empty() {
            String::from(TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID)
        } else {
            report
                .canonical_claim_route_review
                .transformer_model_id
                .clone()
        },
        report_ref: String::from(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF),
        summary_ref: String::from(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_SUMMARY_REF),
        canonical_claim_route_id: report
            .canonical_claim_route_review
            .canonical_claim_route_id
            .clone(),
        route_descriptor_digest: report
            .canonical_claim_route_review
            .projected_route_descriptor_digest
            .clone(),
        selected_decode_mode: report
            .canonical_claim_route_review
            .selected_decode_mode
            .as_str()
            .to_string(),
        operator_verdict_green: report.operator_verdict_review.operator_verdict_green,
        public_posture: report.public_verdict_review.posture,
        public_verdict_green: report.public_verdict_review.public_verdict_green,
        public_blocked_issue_ids: report.public_verdict_review.blocked_issue_ids.clone(),
        route_minimality_audit_green: report.route_minimality_audit_green,
        article_equivalence_green: report.article_equivalence_green,
        claim_boundary: String::from(
            "this served publication verdict cites the canonical TAS-185A route-minimality audit for the direct HullCache article route only. It freezes the bounded public verdict for that direct deterministic route and it does not widen the served claim to planner-mediated, hybrid, resumed, or stochastic orchestration lanes.",
        ),
        publication_digest: String::new(),
    };
    publication.publication_digest = stable_digest(
        b"psionic_tassadar_article_route_minimality_publication_verdict|",
        &publication,
    );
    Ok(publication)
}

pub fn tassadar_article_route_minimality_publication_verdict_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_ROUTE_MINIMALITY_PUBLICATION_VERDICT_REF)
}

pub fn write_tassadar_article_route_minimality_publication_verdict(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleRouteMinimalityPublicationVerdict,
    TassadarArticleRouteMinimalityPublicationVerdictError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleRouteMinimalityPublicationVerdictError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let publication = build_tassadar_article_route_minimality_publication_verdict()?;
    let json = serde_json::to_string_pretty(&publication)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleRouteMinimalityPublicationVerdictError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(publication)
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

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleRouteMinimalityPublicationVerdictError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleRouteMinimalityPublicationVerdictError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleRouteMinimalityPublicationVerdictError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_route_minimality_publication_verdict,
        tassadar_article_route_minimality_publication_verdict_path,
        write_tassadar_article_route_minimality_publication_verdict,
        TassadarArticleRouteMinimalityPublicationVerdict,
        TASSADAR_ARTICLE_ROUTE_MINIMALITY_PUBLICATION_VERDICT_REF,
    };
    use psionic_eval::TassadarArticleRouteMinimalityPublicPosture;

    fn read_committed_publication(
    ) -> Result<TassadarArticleRouteMinimalityPublicationVerdict, Box<dyn std::error::Error>> {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let publication_path =
            repo_root.join(TASSADAR_ARTICLE_ROUTE_MINIMALITY_PUBLICATION_VERDICT_REF);
        Ok(serde_json::from_slice(&std::fs::read(publication_path)?)?)
    }

    #[test]
    fn article_route_minimality_publication_verdict_is_green_bounded(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let publication = build_tassadar_article_route_minimality_publication_verdict()?;

        assert_eq!(publication.product_id, crate::EXECUTOR_TRACE_PRODUCT_ID);
        assert_eq!(
            publication.canonical_claim_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(
            publication.selected_decode_mode,
            "tassadar.decode.hull_cache.v1"
        );
        assert!(publication.operator_verdict_green);
        assert_eq!(
            publication.public_posture,
            TassadarArticleRouteMinimalityPublicPosture::GreenBounded
        );
        assert!(publication.public_verdict_green);
        assert!(publication.public_blocked_issue_ids.is_empty());
        assert!(publication.route_minimality_audit_green);
        assert!(publication.article_equivalence_green);
        Ok(())
    }

    #[test]
    fn article_route_minimality_publication_verdict_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_article_route_minimality_publication_verdict()?;
        let committed = read_committed_publication()?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_route_minimality_publication_verdict_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_route_minimality_publication_verdict.json");
        let written = write_tassadar_article_route_minimality_publication_verdict(&output_path)?;
        let persisted: TassadarArticleRouteMinimalityPublicationVerdict =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_route_minimality_publication_verdict_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_article_route_minimality_publication_verdict.json")
        );
        Ok(())
    }
}
