use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_only_article_acceptance_gate_v2.json";
pub const TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_CHECKER_REF: &str =
    "scripts/check-tassadar-rust-only-article-acceptance-v2.sh";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRustOnlyArticleAcceptancePrerequisiteStatus {
    Passed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustOnlyArticleAcceptancePrerequisite {
    pub prerequisite_id: String,
    pub artifact_refs: Vec<String>,
    pub validation_commands: Vec<String>,
    pub status: TassadarRustOnlyArticleAcceptancePrerequisiteStatus,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_artifact_refs: Vec<String>,
}

impl TassadarRustOnlyArticleAcceptancePrerequisite {
    #[must_use]
    pub fn passed(
        prerequisite_id: impl Into<String>,
        artifact_refs: Vec<String>,
        validation_commands: Vec<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            prerequisite_id: prerequisite_id.into(),
            artifact_refs,
            validation_commands,
            status: TassadarRustOnlyArticleAcceptancePrerequisiteStatus::Passed,
            detail: detail.into(),
            missing_artifact_refs: Vec::new(),
        }
    }

    #[must_use]
    pub fn failed(
        prerequisite_id: impl Into<String>,
        artifact_refs: Vec<String>,
        validation_commands: Vec<String>,
        detail: impl Into<String>,
        missing_artifact_refs: Vec<String>,
    ) -> Self {
        Self {
            prerequisite_id: prerequisite_id.into(),
            artifact_refs,
            validation_commands,
            status: TassadarRustOnlyArticleAcceptancePrerequisiteStatus::Failed,
            detail: detail.into(),
            missing_artifact_refs,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustOnlyArticleAcceptanceGateV2Report {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub prerequisite_count: u32,
    pub passed_prerequisite_count: u32,
    pub failed_prerequisite_ids: Vec<String>,
    pub prerequisites: Vec<TassadarRustOnlyArticleAcceptancePrerequisite>,
    pub green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarRustOnlyArticleAcceptanceGateV2Report {
    #[must_use]
    pub fn from_prerequisites(
        prerequisites: Vec<TassadarRustOnlyArticleAcceptancePrerequisite>,
    ) -> Self {
        let passed_prerequisite_count = prerequisites
            .iter()
            .filter(|prerequisite| {
                prerequisite.status == TassadarRustOnlyArticleAcceptancePrerequisiteStatus::Passed
            })
            .count() as u32;
        let failed_prerequisite_ids = prerequisites
            .iter()
            .filter(|prerequisite| {
                prerequisite.status == TassadarRustOnlyArticleAcceptancePrerequisiteStatus::Failed
            })
            .map(|prerequisite| prerequisite.prerequisite_id.clone())
            .collect::<Vec<_>>();
        let green = failed_prerequisite_ids.is_empty();
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.rust_only_article_acceptance_gate.v2"),
            checker_script_ref: String::from(
                TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_CHECKER_REF,
            ),
            prerequisite_count: prerequisites.len() as u32,
            passed_prerequisite_count,
            failed_prerequisite_ids,
            prerequisites,
            green,
            claim_boundary: String::from(
                "this report is the anti-hand-wave gate for the Rust-only article claim. It turns green only when every committed prerequisite artifact is present and passes its bounded check, and it does not widen the claim beyond the explicitly enumerated Rust-only source, profile, ABI, reproducer, runtime, direct-proof, and CPU-portability surfaces",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Rust-only article acceptance gate v2 now records passed_prerequisites={}/{} and green={}.",
            report.passed_prerequisite_count, report.prerequisite_count, report.green,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_rust_only_article_acceptance_gate_v2_report|",
            &report,
        );
        report
    }
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
        TassadarRustOnlyArticleAcceptanceGateV2Report,
        TassadarRustOnlyArticleAcceptancePrerequisite,
    };

    fn passing_prerequisite(id: &str) -> TassadarRustOnlyArticleAcceptancePrerequisite {
        TassadarRustOnlyArticleAcceptancePrerequisite::passed(
            id,
            vec![format!("fixtures/{id}.json")],
            vec![format!("check-{id}")],
            format!("{id} is green"),
        )
    }

    #[test]
    fn acceptance_gate_v2_turns_green_only_when_all_prerequisites_pass() {
        let report = TassadarRustOnlyArticleAcceptanceGateV2Report::from_prerequisites(vec![
            passing_prerequisite("rust_source_canon"),
            passing_prerequisite("profile_completeness"),
            passing_prerequisite("article_abi"),
            passing_prerequisite("hungarian_article_reproducer"),
            passing_prerequisite("sudoku_article_reproducer"),
            passing_prerequisite("article_runtime_closeout"),
            passing_prerequisite("direct_model_weight_execution_proof"),
            passing_prerequisite("article_cpu_reproducibility"),
        ]);

        assert!(report.green);
        assert_eq!(report.prerequisite_count, 8);
        assert_eq!(report.passed_prerequisite_count, 8);
        assert!(report.failed_prerequisite_ids.is_empty());
    }

    #[test]
    fn acceptance_gate_v2_fails_each_missing_prerequisite_individually() {
        let prerequisite_ids = [
            "rust_source_canon",
            "profile_completeness",
            "article_abi",
            "hungarian_article_reproducer",
            "sudoku_article_reproducer",
            "article_runtime_closeout",
            "direct_model_weight_execution_proof",
            "article_cpu_reproducibility",
        ];

        for failing_id in prerequisite_ids {
            let prerequisites = prerequisite_ids
                .iter()
                .map(|id| {
                    if *id == failing_id {
                        TassadarRustOnlyArticleAcceptancePrerequisite::failed(
                            *id,
                            vec![format!("fixtures/{id}.json")],
                            vec![format!("check-{id}")],
                            format!("{id} is missing"),
                            vec![format!("fixtures/{id}.json")],
                        )
                    } else {
                        passing_prerequisite(id)
                    }
                })
                .collect::<Vec<_>>();
            let report =
                TassadarRustOnlyArticleAcceptanceGateV2Report::from_prerequisites(prerequisites);

            assert!(!report.green, "{failing_id} should force the gate red");
            assert_eq!(report.passed_prerequisite_count, 7);
            assert_eq!(report.failed_prerequisite_ids, vec![String::from(failing_id)]);
        }
    }
}
