use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_eval::{
    tassadar_article_cross_machine_reproducibility_matrix_report_path,
    TassadarArticleCrossMachineReproducibilityReport,
};

pub const TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_cross_machine_reproducibility_matrix_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCrossMachineReproducibilityMatrixSummary {
    pub report_id: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub blocked_issue_frontier: String,
    pub current_host_machine_class_id: String,
    pub supported_machine_class_ids: Vec<String>,
    pub deterministic_mode_green: bool,
    pub throughput_floor_stability_green: bool,
    pub stochastic_mode_supported: bool,
    pub stochastic_mode_out_of_scope: bool,
    pub reproducibility_matrix_green: bool,
    pub article_equivalence_green: bool,
    pub detail: String,
    pub summary_digest: String,
}

pub fn build_tassadar_article_cross_machine_reproducibility_matrix_summary_from_report(
    report: &TassadarArticleCrossMachineReproducibilityReport,
) -> TassadarArticleCrossMachineReproducibilityMatrixSummary {
    let mut summary = TassadarArticleCrossMachineReproducibilityMatrixSummary {
        report_id: report.report_id.clone(),
        tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
        tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
        blocked_issue_frontier: report
            .acceptance_gate_tie
            .blocked_issue_ids
            .first()
            .cloned()
            .unwrap_or_else(|| String::from("none")),
        current_host_machine_class_id: report
            .machine_matrix_review
            .current_host_machine_class_id
            .clone(),
        supported_machine_class_ids: report
            .machine_matrix_review
            .supported_machine_class_ids
            .clone(),
        deterministic_mode_green: report.deterministic_mode_green,
        throughput_floor_stability_green: report.throughput_floor_stability_green,
        stochastic_mode_supported: report.stochastic_mode_review.stochastic_mode_supported,
        stochastic_mode_out_of_scope: report.stochastic_mode_review.out_of_scope,
        reproducibility_matrix_green: report.reproducibility_matrix_green,
        article_equivalence_green: report.article_equivalence_green,
        detail: format!(
            "TAS-185 now records current_host_machine_class_id=`{}`, supported_machine_classes={}, deterministic_mode_green={}, throughput_floor_stability_green={}, stochastic_mode_supported={}, stochastic_mode_out_of_scope={}, reproducibility_matrix_green={}, and article_equivalence_green={}.",
            report.machine_matrix_review.current_host_machine_class_id,
            report.machine_matrix_review.supported_machine_class_ids.len(),
            report.deterministic_mode_green,
            report.throughput_floor_stability_green,
            report.stochastic_mode_review.stochastic_mode_supported,
            report.stochastic_mode_review.out_of_scope,
            report.reproducibility_matrix_green,
            report.article_equivalence_green,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_article_cross_machine_reproducibility_matrix_summary|",
        &summary,
    );
    summary
}

pub fn build_tassadar_article_cross_machine_reproducibility_matrix_summary(
) -> Result<TassadarArticleCrossMachineReproducibilityMatrixSummary, Box<dyn std::error::Error>> {
    let report: TassadarArticleCrossMachineReproducibilityReport = serde_json::from_slice(
        &fs::read(tassadar_article_cross_machine_reproducibility_matrix_report_path())?,
    )?;
    Ok(build_tassadar_article_cross_machine_reproducibility_matrix_summary_from_report(&report))
}

pub fn tassadar_article_cross_machine_reproducibility_matrix_summary_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join(TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_SUMMARY_REF)
}

pub fn write_tassadar_article_cross_machine_reproducibility_matrix_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleCrossMachineReproducibilityMatrixSummary, Box<dyn std::error::Error>> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let summary = build_tassadar_article_cross_machine_reproducibility_matrix_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n"))?;
    Ok(summary)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("summary serialization"));
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_cross_machine_reproducibility_matrix_summary,
        build_tassadar_article_cross_machine_reproducibility_matrix_summary_from_report,
        tassadar_article_cross_machine_reproducibility_matrix_summary_path,
        write_tassadar_article_cross_machine_reproducibility_matrix_summary,
        TassadarArticleCrossMachineReproducibilityMatrixSummary,
        TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_SUMMARY_REF,
    };
    use psionic_eval::{
        TassadarArticleCrossMachineReproducibilityReport,
        TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
    };
    use tempfile::tempdir;

    fn read_repo_json<T: for<'de> serde::Deserialize<'de>>(
        repo_relative_path: &str,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .join(repo_relative_path);
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }

    #[test]
    fn article_cross_machine_reproducibility_summary_tracks_green_matrix(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let summary = build_tassadar_article_cross_machine_reproducibility_matrix_summary()?;

        assert_eq!(summary.tied_requirement_id, "TAS-185");
        assert_eq!(summary.blocked_issue_frontier, "none");
        assert_eq!(summary.supported_machine_class_ids.len(), 2);
        assert!(summary.deterministic_mode_green);
        assert!(summary.throughput_floor_stability_green);
        assert!(!summary.stochastic_mode_supported);
        assert!(summary.stochastic_mode_out_of_scope);
        assert!(summary.reproducibility_matrix_green);
        assert!(summary.article_equivalence_green);
        Ok(())
    }

    #[test]
    fn article_cross_machine_reproducibility_summary_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report: TassadarArticleCrossMachineReproducibilityReport =
            read_repo_json(TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF)?;
        let generated =
            build_tassadar_article_cross_machine_reproducibility_matrix_summary_from_report(
                &report,
            );
        let committed: TassadarArticleCrossMachineReproducibilityMatrixSummary =
            read_repo_json(TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_SUMMARY_REF)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_cross_machine_reproducibility_summary_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_cross_machine_reproducibility_matrix_summary.json");
        let written =
            write_tassadar_article_cross_machine_reproducibility_matrix_summary(&output_path)?;
        let persisted: TassadarArticleCrossMachineReproducibilityMatrixSummary =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_cross_machine_reproducibility_matrix_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_article_cross_machine_reproducibility_matrix_summary.json")
        );
        Ok(())
    }
}
