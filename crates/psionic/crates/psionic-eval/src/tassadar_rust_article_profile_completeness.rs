use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    tassadar_rust_article_profile_completeness_publication,
    TassadarRustArticleProfileCompletenessPublication,
    TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_REPORT_REF,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TassadarRustArticleProfileCompletenessReportError {
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read committed report `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode committed report `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

pub fn build_tassadar_rust_article_profile_completeness_report(
) -> TassadarRustArticleProfileCompletenessPublication {
    tassadar_rust_article_profile_completeness_publication()
}

pub fn tassadar_rust_article_profile_completeness_report_path() -> PathBuf {
    repo_root().join(TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_REPORT_REF)
}

pub fn write_tassadar_rust_article_profile_completeness_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarRustArticleProfileCompletenessPublication,
    TassadarRustArticleProfileCompletenessReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarRustArticleProfileCompletenessReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_rust_article_profile_completeness_report();
    let bytes = serde_json::to_vec_pretty(&report)
        .expect("Rust article profile completeness report should serialize");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarRustArticleProfileCompletenessReportError::Write {
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
mod tests {
    use super::{
        build_tassadar_rust_article_profile_completeness_report,
        tassadar_rust_article_profile_completeness_report_path,
        write_tassadar_rust_article_profile_completeness_report,
        TassadarRustArticleProfileCompletenessReportError,
    };
    use psionic_models::{
        TassadarRustArticleProfileCategory, TassadarRustArticleProfileCompletenessPublication,
        TassadarRustArticleProfileRowStatus, TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_REPORT_REF,
    };

    #[test]
    fn rust_article_profile_completeness_report_is_machine_legible() {
        let report = build_tassadar_rust_article_profile_completeness_report();

        assert_eq!(
            report.report_ref,
            TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_REPORT_REF
        );
        assert!(report.rows.iter().any(|row| row.category
            == TassadarRustArticleProfileCategory::AbiShape
            && row.status == TassadarRustArticleProfileRowStatus::Refused));
        assert!(report.rows.iter().any(|row| row.category
            == TassadarRustArticleProfileCategory::ControlFlowFamily
            && row.status == TassadarRustArticleProfileRowStatus::Supported));
    }

    #[test]
    fn rust_article_profile_completeness_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_rust_article_profile_completeness_report();
        let path = tassadar_rust_article_profile_completeness_report_path();
        let bytes = std::fs::read(&path)?;
        let committed: TassadarRustArticleProfileCompletenessPublication =
            serde_json::from_slice(&bytes)?;
        assert_eq!(report, committed);
        Ok(())
    }

    #[test]
    fn write_rust_article_profile_completeness_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let report_path = temp_dir
            .path()
            .join("tassadar_rust_article_profile_completeness_report.json");
        let report = write_tassadar_rust_article_profile_completeness_report(&report_path)?;
        let persisted: TassadarRustArticleProfileCompletenessPublication =
            serde_json::from_slice(&std::fs::read(&report_path)?)?;
        assert_eq!(report, persisted);
        Ok(())
    }

    #[test]
    fn write_report_surfaces_write_failures() {
        let err = write_tassadar_rust_article_profile_completeness_report("/dev/null/report.json")
            .expect_err("writing below /dev/null should fail");
        assert!(matches!(
            err,
            TassadarRustArticleProfileCompletenessReportError::CreateDir { .. }
                | TassadarRustArticleProfileCompletenessReportError::Write { .. }
        ));
    }
}
