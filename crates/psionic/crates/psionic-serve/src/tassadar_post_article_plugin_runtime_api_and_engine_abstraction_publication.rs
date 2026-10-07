use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_research::{
    build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_summary,
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionSummary,
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionSummaryError,
    TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_SUMMARY_REF,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_PUBLICATION_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginRuntimeApiAndEngineAbstractionPublication {
    pub publication_id: String,
    pub summary_ref: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub current_served_internal_compute_profile_id: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub served_plugin_surface_ids: Vec<String>,
    pub blocked_by: Vec<String>,
    pub operator_internal_only_posture: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub claim_boundary: String,
    pub publication_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticlePluginRuntimeApiAndEngineAbstractionPublicationError {
    #[error(transparent)]
    Research(#[from] TassadarPostArticlePluginRuntimeApiAndEngineAbstractionSummaryError),
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

pub fn build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication() -> Result<
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionPublication,
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionPublicationError,
> {
    let summary = build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_summary()?;
    Ok(build_publication_from_summary(&summary))
}

fn build_publication_from_summary(
    summary: &TassadarPostArticlePluginRuntimeApiAndEngineAbstractionSummary,
) -> TassadarPostArticlePluginRuntimeApiAndEngineAbstractionPublication {
    let mut publication = TassadarPostArticlePluginRuntimeApiAndEngineAbstractionPublication {
        publication_id: String::from(
            "tassadar.post_article_plugin_runtime_api_and_engine_abstraction.publication.v1",
        ),
        summary_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_SUMMARY_REF,
        ),
        machine_identity_id: summary.machine_identity_id.clone(),
        canonical_model_id: summary.canonical_model_id.clone(),
        canonical_route_id: summary.canonical_route_id.clone(),
        current_served_internal_compute_profile_id: String::from(
            "tassadar.internal_compute.article_closeout.v1",
        ),
        packet_abi_version: summary.packet_abi_version.clone(),
        host_owned_runtime_api_id: summary.host_owned_runtime_api_id.clone(),
        engine_abstraction_id: summary.engine_abstraction_id.clone(),
        served_plugin_surface_ids: Vec::new(),
        blocked_by: summary.deferred_issue_ids.clone(),
        operator_internal_only_posture: summary.operator_internal_only_posture,
        rebase_claim_allowed: summary.rebase_claim_allowed,
        plugin_capability_claim_allowed: summary.plugin_capability_claim_allowed,
        weighted_plugin_control_allowed: summary.weighted_plugin_control_allowed,
        plugin_publication_allowed: summary.plugin_publication_allowed,
        served_public_universality_allowed: summary.served_public_universality_allowed,
        arbitrary_software_capability_allowed: summary.arbitrary_software_capability_allowed,
        claim_boundary: String::from(
            "this served publication projects the host-owned plugin runtime API onto the current article-closeout served boundary without widening any served plugin surface. The runtime API is frozen and machine-bound, but served plugin publication stays blocked until later admissibility, controller, authority, and closeout issues land.",
        ),
        publication_digest: String::new(),
    };
    publication.publication_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication|",
        &publication,
    );
    publication
}

#[must_use]
pub fn tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication_path() -> PathBuf
{
    repo_root()
        .join(TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_PUBLICATION_REF)
}

pub fn write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionPublication,
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionPublicationError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginRuntimeApiAndEngineAbstractionPublicationError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let publication =
        build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication()?;
    let json = serde_json::to_string_pretty(&publication)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionPublicationError::Write {
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
mod tests {
    use super::{
        build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication,
        tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication_path,
        write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication,
        TassadarPostArticlePluginRuntimeApiAndEngineAbstractionPublication,
        TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_PUBLICATION_REF,
    };

    #[test]
    fn post_article_plugin_runtime_api_publication_keeps_served_surface_blocked(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let publication =
            build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication()?;

        assert_eq!(
            publication.current_served_internal_compute_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
        assert_eq!(
            publication.host_owned_runtime_api_id,
            "tassadar.plugin_runtime.host_owned_api.v1"
        );
        assert_eq!(
            publication.engine_abstraction_id,
            "tassadar.plugin_runtime.engine_abstraction.v1"
        );
        assert!(publication.served_plugin_surface_ids.is_empty());
        assert!(publication.blocked_by.is_empty());
        assert!(publication.operator_internal_only_posture);
        assert!(publication.rebase_claim_allowed);
        assert!(!publication.plugin_capability_claim_allowed);
        assert!(!publication.weighted_plugin_control_allowed);
        assert!(!publication.plugin_publication_allowed);
        assert!(!publication.served_public_universality_allowed);
        assert!(!publication.arbitrary_software_capability_allowed);
        Ok(())
    }

    #[test]
    fn post_article_plugin_runtime_api_publication_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated =
            build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication()?;
        let committed: TassadarPostArticlePluginRuntimeApiAndEngineAbstractionPublication =
            serde_json::from_slice(&std::fs::read(
                tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication_path(),
            )?)?;
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication_path()
                .strip_prefix(super::repo_root())?
                .to_string_lossy(),
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_PUBLICATION_REF
        );
        Ok(())
    }

    #[test]
    fn write_post_article_plugin_runtime_api_publication_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory.path().join(
            "tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication.json",
        );
        let written =
            write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_publication(
                &output_path,
            )?;
        let persisted: TassadarPostArticlePluginRuntimeApiAndEngineAbstractionPublication =
            serde_json::from_slice(&std::fs::read(output_path)?)?;
        assert_eq!(written, persisted);
        Ok(())
    }
}
