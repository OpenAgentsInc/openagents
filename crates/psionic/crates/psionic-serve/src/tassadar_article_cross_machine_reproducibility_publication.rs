use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleCrossMachineReproducibilityReport,
    TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
};
use psionic_models::TassadarExecutorFixture;
use psionic_research::TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_SUMMARY_REF;
use psionic_runtime::TassadarExecutorDecodeMode;

use crate::EXECUTOR_TRACE_PRODUCT_ID;

pub const TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_PUBLICATION_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_cross_machine_reproducibility_publication.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCrossMachineReproducibilityPublication {
    pub product_id: String,
    pub model_id: String,
    pub report_ref: String,
    pub summary_ref: String,
    pub current_host_machine_class_id: String,
    pub supported_machine_class_ids: Vec<String>,
    pub selected_decode_mode: TassadarExecutorDecodeMode,
    pub deterministic_mode_green: bool,
    pub throughput_floor_stability_green: bool,
    pub stochastic_mode_out_of_scope: bool,
    pub reproducibility_matrix_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub publication_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleCrossMachineReproducibilityPublicationError {
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
    #[error("reproducibility matrix is not publishable: {detail}")]
    Invalid { detail: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_cross_machine_reproducibility_publication() -> Result<
    TassadarArticleCrossMachineReproducibilityPublication,
    TassadarArticleCrossMachineReproducibilityPublicationError,
> {
    let report: TassadarArticleCrossMachineReproducibilityReport = read_repo_json(
        TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
        "article_cross_machine_reproducibility_matrix_report",
    )?;
    if !report.reproducibility_matrix_green {
        return Err(TassadarArticleCrossMachineReproducibilityPublicationError::Invalid {
            detail: String::from(
                "cross-machine reproducibility matrix must be green before publication is allowed",
            ),
        });
    }
    let mut publication = TassadarArticleCrossMachineReproducibilityPublication {
        product_id: String::from(EXECUTOR_TRACE_PRODUCT_ID),
        model_id: if report.route_stability_review.transformer_model_id.is_empty() {
            String::from(TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID)
        } else {
            report.route_stability_review.transformer_model_id.clone()
        },
        report_ref: String::from(TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF),
        summary_ref: String::from(
            TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_SUMMARY_REF,
        ),
        current_host_machine_class_id: report
            .machine_matrix_review
            .current_host_machine_class_id
            .clone(),
        supported_machine_class_ids: report
            .machine_matrix_review
            .supported_machine_class_ids
            .clone(),
        selected_decode_mode: report.route_stability_review.selected_decode_mode,
        deterministic_mode_green: report.deterministic_mode_green,
        throughput_floor_stability_green: report.throughput_floor_stability_green,
        stochastic_mode_out_of_scope: report.stochastic_mode_review.out_of_scope,
        reproducibility_matrix_green: report.reproducibility_matrix_green,
        article_equivalence_green: report.article_equivalence_green,
        claim_boundary: String::from(
            "this served publication cites the canonical TAS-185 cross-machine reproducibility matrix for the selected deterministic fast article route only. It freezes the declared supported CPU machine classes, the selected HullCache decode mode, and the explicit out-of-scope stochastic posture without widening the served claim to route minimality, stochastic execution, or final article-equivalence green status.",
        ),
        publication_digest: String::new(),
    };
    publication.publication_digest = stable_digest(
        b"psionic_tassadar_article_cross_machine_reproducibility_publication|",
        &publication,
    );
    Ok(publication)
}

pub fn tassadar_article_cross_machine_reproducibility_publication_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_PUBLICATION_REF)
}

pub fn write_tassadar_article_cross_machine_reproducibility_publication(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleCrossMachineReproducibilityPublication,
    TassadarArticleCrossMachineReproducibilityPublicationError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleCrossMachineReproducibilityPublicationError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let publication = build_tassadar_article_cross_machine_reproducibility_publication()?;
    let json = serde_json::to_string_pretty(&publication)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleCrossMachineReproducibilityPublicationError::Write {
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
) -> Result<T, TassadarArticleCrossMachineReproducibilityPublicationError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleCrossMachineReproducibilityPublicationError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleCrossMachineReproducibilityPublicationError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_cross_machine_reproducibility_publication,
        tassadar_article_cross_machine_reproducibility_publication_path,
        write_tassadar_article_cross_machine_reproducibility_publication,
        TassadarArticleCrossMachineReproducibilityPublication,
        TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_PUBLICATION_REF,
    };

    fn read_committed_publication(
    ) -> Result<TassadarArticleCrossMachineReproducibilityPublication, Box<dyn std::error::Error>>
    {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let publication_path =
            repo_root.join(TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_PUBLICATION_REF);
        Ok(serde_json::from_slice(&std::fs::read(publication_path)?)?)
    }

    #[test]
    fn article_cross_machine_reproducibility_publication_is_green_and_bounded(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let publication = build_tassadar_article_cross_machine_reproducibility_publication()?;

        assert_eq!(publication.product_id, crate::EXECUTOR_TRACE_PRODUCT_ID);
        assert_eq!(publication.supported_machine_class_ids.len(), 2);
        assert_eq!(
            publication.selected_decode_mode,
            psionic_runtime::TassadarExecutorDecodeMode::HullCache
        );
        assert!(publication.deterministic_mode_green);
        assert!(publication.throughput_floor_stability_green);
        assert!(publication.stochastic_mode_out_of_scope);
        assert!(publication.reproducibility_matrix_green);
        assert!(publication.article_equivalence_green);
        Ok(())
    }

    #[test]
    fn article_cross_machine_reproducibility_publication_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_article_cross_machine_reproducibility_publication()?;
        let committed = read_committed_publication()?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_cross_machine_reproducibility_publication_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_cross_machine_reproducibility_publication.json");
        let written =
            write_tassadar_article_cross_machine_reproducibility_publication(&output_path)?;
        let persisted: TassadarArticleCrossMachineReproducibilityPublication =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_cross_machine_reproducibility_publication_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_article_cross_machine_reproducibility_publication.json")
        );
        Ok(())
    }
}
