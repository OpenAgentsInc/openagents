use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_post_article_rebased_universality_verdict_split_report,
    TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    TassadarPostArticleRebasedUniversalityVerdictSplitReportError,
    TassadarUniversalityVerdictLevel,
    TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_PUBLICATION_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_rebased_universality_verdict_publication.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleRebasedUniversalityVerdictPublication {
    pub publication_id: String,
    pub report_ref: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub current_served_internal_compute_profile_id: String,
    pub theory_green: bool,
    pub operator_green: bool,
    pub served_green: bool,
    pub operator_allowed_profile_ids: Vec<String>,
    pub served_blocked_by: Vec<String>,
    pub served_conformance_envelope_ref: String,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub claim_boundary: String,
    pub publication_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleRebasedUniversalityVerdictPublicationError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleRebasedUniversalityVerdictSplitReportError),
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
    #[error("missing `{verdict_level}` rebased verdict row")]
    MissingVerdictRow { verdict_level: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_post_article_rebased_universality_verdict_publication() -> Result<
    TassadarPostArticleRebasedUniversalityVerdictPublication,
    TassadarPostArticleRebasedUniversalityVerdictPublicationError,
> {
    let report = build_tassadar_post_article_rebased_universality_verdict_split_report()?;
    build_publication_from_report(report)
}

fn build_publication_from_report(
    report: TassadarPostArticleRebasedUniversalityVerdictSplitReport,
) -> Result<
    TassadarPostArticleRebasedUniversalityVerdictPublication,
    TassadarPostArticleRebasedUniversalityVerdictPublicationError,
> {
    let operator_row = report
        .verdict_rows
        .iter()
        .find(|row| row.verdict_level == TassadarUniversalityVerdictLevel::Operator)
        .ok_or_else(|| {
            TassadarPostArticleRebasedUniversalityVerdictPublicationError::MissingVerdictRow {
                verdict_level: String::from("operator"),
            }
        })?;
    let served_row = report
        .verdict_rows
        .iter()
        .find(|row| row.verdict_level == TassadarUniversalityVerdictLevel::Served)
        .ok_or_else(|| {
            TassadarPostArticleRebasedUniversalityVerdictPublicationError::MissingVerdictRow {
                verdict_level: String::from("served"),
            }
        })?;

    let mut publication = TassadarPostArticleRebasedUniversalityVerdictPublication {
        publication_id: String::from("tassadar.post_article_rebased_universality_verdict.publication.v1"),
        report_ref: String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
        machine_identity_id: report.machine_identity_id,
        canonical_model_id: report.canonical_model_id,
        canonical_route_id: report.canonical_route_id,
        current_served_internal_compute_profile_id: report.current_served_internal_compute_profile_id,
        theory_green: report.theory_green,
        operator_green: report.operator_green,
        served_green: report.served_green,
        operator_allowed_profile_ids: operator_row.allowed_profile_ids.clone(),
        served_blocked_by: served_row.blocked_by.clone(),
        served_conformance_envelope_ref: report.served_conformance_envelope_ref,
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        claim_boundary: String::from(
            "this served publication projects the rebased theory/operator/served verdict split on the canonical machine identity. Theory and operator truth are green on the canonical route, but served/public universality remains suppressed inside the declared served conformance envelope and no plugin-capability claim is implied.",
        ),
        publication_digest: String::new(),
    };
    publication.publication_digest = stable_digest(
        b"psionic_tassadar_post_article_rebased_universality_verdict_publication|",
        &publication,
    );
    Ok(publication)
}

#[must_use]
pub fn tassadar_post_article_rebased_universality_verdict_publication_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_PUBLICATION_REF)
}

pub fn write_tassadar_post_article_rebased_universality_verdict_publication(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleRebasedUniversalityVerdictPublication,
    TassadarPostArticleRebasedUniversalityVerdictPublicationError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleRebasedUniversalityVerdictPublicationError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let publication = build_tassadar_post_article_rebased_universality_verdict_publication()?;
    let json = serde_json::to_string_pretty(&publication)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleRebasedUniversalityVerdictPublicationError::Write {
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

#[cfg(test)]
fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarPostArticleRebasedUniversalityVerdictPublicationError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleRebasedUniversalityVerdictPublicationError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleRebasedUniversalityVerdictPublicationError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_rebased_universality_verdict_publication, read_repo_json,
        tassadar_post_article_rebased_universality_verdict_publication_path,
        write_tassadar_post_article_rebased_universality_verdict_publication,
        TassadarPostArticleRebasedUniversalityVerdictPublication,
        TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_PUBLICATION_REF,
    };

    #[test]
    fn post_article_rebased_universality_verdict_publication_keeps_served_suppressed(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let publication = build_tassadar_post_article_rebased_universality_verdict_publication()?;

        assert!(publication.theory_green);
        assert!(publication.operator_green);
        assert!(!publication.served_green);
        assert!(publication.rebase_claim_allowed);
        assert!(!publication.plugin_capability_claim_allowed);
        assert!(!publication.served_public_universality_allowed);
        assert!(!publication.arbitrary_software_capability_allowed);
        assert_eq!(
            publication.current_served_internal_compute_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
        Ok(())
    }

    #[test]
    fn post_article_rebased_universality_verdict_publication_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_post_article_rebased_universality_verdict_publication()?;
        let committed: TassadarPostArticleRebasedUniversalityVerdictPublication = read_repo_json(
            TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_PUBLICATION_REF,
            "post_article_rebased_universality_verdict_publication",
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_post_article_rebased_universality_verdict_publication_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_post_article_rebased_universality_verdict_publication.json");
        let written =
            write_tassadar_post_article_rebased_universality_verdict_publication(&output_path)?;
        let persisted: TassadarPostArticleRebasedUniversalityVerdictPublication =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_post_article_rebased_universality_verdict_publication_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_rebased_universality_verdict_publication.json")
        );
        Ok(())
    }
}
