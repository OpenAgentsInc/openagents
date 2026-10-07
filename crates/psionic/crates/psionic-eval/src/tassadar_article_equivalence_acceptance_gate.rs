use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_article_equivalence_blocker_matrix_report,
    TassadarArticleEquivalenceBlockerMatrixReport,
    TassadarArticleEquivalenceBlockerMatrixReportError, TassadarArticleEquivalenceIssueRole,
    TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
};

pub const TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_equivalence_acceptance_gate_report.json";
pub const TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_CHECKER_REF: &str =
    "scripts/check-tassadar-article-equivalence-acceptance-gate.sh";

const BLOCKER_MATRIX_CONTRACT_REQUIREMENT_ID: &str = "blocker_matrix_contract_frozen";
const OWNED_TRANSFORMER_ROUTE_BOUNDARY_REQUIREMENT_ID: &str =
    "owned_transformer_route_boundary_frozen";
const ARTICLE_EQUIVALENCE_BLOCKERS_CLOSED_REQUIREMENT_ID: &str =
    "article_equivalence_blockers_closed";
#[cfg(test)]
const OPTIONAL_RESEARCH_ISSUE_ID: &str = "TAS-R1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleEquivalenceAcceptanceStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleEquivalenceAcceptanceRequirementKind {
    Contract,
    Prerequisite,
    Gate,
    Implementation,
    FinalAudit,
    OptionalResearch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceAcceptanceRequirementRow {
    pub requirement_id: String,
    pub requirement_kind: TassadarArticleEquivalenceAcceptanceRequirementKind,
    pub required_for_green: bool,
    pub satisfied: bool,
    pub blocker_ids: Vec<String>,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceAcceptanceGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub blocker_matrix_report_ref: String,
    pub blocker_matrix_report: TassadarArticleEquivalenceBlockerMatrixReport,
    pub requirement_rows: Vec<TassadarArticleEquivalenceAcceptanceRequirementRow>,
    pub total_requirement_count: usize,
    pub required_requirement_count: usize,
    pub passed_required_requirement_count: usize,
    pub required_issue_count: usize,
    pub closed_required_issue_count: usize,
    pub green_requirement_ids: Vec<String>,
    pub failed_requirement_ids: Vec<String>,
    pub blocked_issue_ids: Vec<String>,
    pub blocked_blocker_ids: Vec<String>,
    pub optional_open_issue_ids: Vec<String>,
    pub blocker_matrix_contract_green: bool,
    pub prerequisite_transformer_boundary_green: bool,
    pub blocker_matrix_article_equivalence_green: bool,
    pub open_blocker_count: usize,
    pub open_blocker_ids: Vec<String>,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub article_equivalence_green: bool,
    pub public_claim_allowed: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleEquivalenceAcceptanceGateReportError {
    #[error(transparent)]
    BlockerMatrix(#[from] TassadarArticleEquivalenceBlockerMatrixReportError),
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

pub fn build_tassadar_article_equivalence_acceptance_gate_report() -> Result<
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
> {
    let blocker_matrix_report = build_tassadar_article_equivalence_blocker_matrix_report()?;
    Ok(build_report_from_blocker_matrix(blocker_matrix_report))
}

fn build_report_from_blocker_matrix(
    blocker_matrix_report: TassadarArticleEquivalenceBlockerMatrixReport,
) -> TassadarArticleEquivalenceAcceptanceGateReport {
    let requirement_rows = requirement_rows(&blocker_matrix_report);
    let total_requirement_count = requirement_rows.len();
    let required_requirement_count = requirement_rows
        .iter()
        .filter(|row| row.required_for_green)
        .count();
    let passed_required_requirement_count = requirement_rows
        .iter()
        .filter(|row| row.required_for_green && row.satisfied)
        .count();
    let required_issue_count = requirement_rows
        .iter()
        .filter(|row| row.required_for_green && row.requirement_kind.is_issue_kind())
        .count();
    let closed_required_issue_count = requirement_rows
        .iter()
        .filter(|row| {
            row.required_for_green && row.requirement_kind.is_issue_kind() && row.satisfied
        })
        .count();
    let green_requirement_ids = requirement_rows
        .iter()
        .filter(|row| row.required_for_green && row.satisfied)
        .map(|row| row.requirement_id.clone())
        .collect::<Vec<_>>();
    let failed_requirement_ids = requirement_rows
        .iter()
        .filter(|row| row.required_for_green && !row.satisfied)
        .map(|row| row.requirement_id.clone())
        .collect::<Vec<_>>();
    let blocked_issue_ids = requirement_rows
        .iter()
        .filter(|row| {
            row.required_for_green && !row.satisfied && row.requirement_kind.is_issue_kind()
        })
        .map(|row| row.requirement_id.clone())
        .collect::<Vec<_>>();
    let blocked_blocker_ids = requirement_rows
        .iter()
        .filter(|row| row.required_for_green && !row.satisfied)
        .flat_map(|row| row.blocker_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let optional_open_issue_ids = requirement_rows
        .iter()
        .filter(|row| {
            row.requirement_kind
                == TassadarArticleEquivalenceAcceptanceRequirementKind::OptionalResearch
                && !row.satisfied
        })
        .map(|row| row.requirement_id.clone())
        .collect::<Vec<_>>();
    let acceptance_status = if failed_requirement_ids.is_empty() {
        TassadarArticleEquivalenceAcceptanceStatus::Green
    } else {
        TassadarArticleEquivalenceAcceptanceStatus::Blocked
    };
    let article_equivalence_green =
        acceptance_status == TassadarArticleEquivalenceAcceptanceStatus::Green;
    let mut report = TassadarArticleEquivalenceAcceptanceGateReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_equivalence.acceptance_gate.report.v1"),
        checker_script_ref: String::from(TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_CHECKER_REF),
        blocker_matrix_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
        ),
        blocker_matrix_contract_green: blocker_matrix_report.matrix_contract_green,
        prerequisite_transformer_boundary_green: blocker_matrix_report
            .prerequisite_transformer_boundary_green,
        blocker_matrix_article_equivalence_green: blocker_matrix_report.article_equivalence_green,
        open_blocker_count: blocker_matrix_report.open_blocker_count,
        open_blocker_ids: blocker_matrix_report.open_blocker_ids.clone(),
        blocker_matrix_report,
        requirement_rows,
        total_requirement_count,
        required_requirement_count,
        passed_required_requirement_count,
        required_issue_count,
        closed_required_issue_count,
        green_requirement_ids,
        failed_requirement_ids,
        blocked_issue_ids,
        blocked_blocker_ids,
        optional_open_issue_ids,
        acceptance_status,
        article_equivalence_green,
        public_claim_allowed: article_equivalence_green,
        claim_boundary: String::from(
            "this is the frozen final acceptance gate for article-equivalent closure. It stays blocked until the blocker-matrix contract remains intact, the owned `psionic-transformer` route boundary remains intact, the blocker matrix itself turns article-equivalence green, and every required TAS tranche from `TAS-158` through `TAS-186` is closed. A green result here permits the bounded public article-equivalence claim inside the declared article envelope, but it still does not imply arbitrary C ingress, arbitrary Wasm ingress, or generic interpreter-in-weights closure outside that envelope.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article-equivalence acceptance gate now records passed_required_requirements={}/{}, closed_required_issues={}/{}, blocked_issues={}, blocker_matrix_article_equivalence_green={}, and article_equivalence_green={}.",
        report.passed_required_requirement_count,
        report.required_requirement_count,
        report.closed_required_issue_count,
        report.required_issue_count,
        report.blocked_issue_ids.len(),
        report.blocker_matrix_article_equivalence_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_equivalence_acceptance_gate_report|",
        &report,
    );
    report
}

fn requirement_rows(
    blocker_matrix_report: &TassadarArticleEquivalenceBlockerMatrixReport,
) -> Vec<TassadarArticleEquivalenceAcceptanceRequirementRow> {
    let mut rows = vec![
        TassadarArticleEquivalenceAcceptanceRequirementRow {
            requirement_id: String::from(BLOCKER_MATRIX_CONTRACT_REQUIREMENT_ID),
            requirement_kind: TassadarArticleEquivalenceAcceptanceRequirementKind::Contract,
            required_for_green: true,
            satisfied: blocker_matrix_report.matrix_contract_green,
            blocker_ids: blocker_matrix_report.blocker_ids.clone(),
            source_refs: vec![String::from(
                TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
            )],
            detail: String::from(
                "the blocker matrix itself must stay structurally green so the final article-equivalence bar cannot be narrowed or silently drifted later",
            ),
        },
        TassadarArticleEquivalenceAcceptanceRequirementRow {
            requirement_id: String::from(OWNED_TRANSFORMER_ROUTE_BOUNDARY_REQUIREMENT_ID),
            requirement_kind: TassadarArticleEquivalenceAcceptanceRequirementKind::Prerequisite,
            required_for_green: true,
            satisfied: blocker_matrix_report.prerequisite_transformer_boundary_green,
            blocker_ids: vec![String::from("BEQ-003")],
            source_refs: vec![
                String::from(TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF),
                String::from("crates/psionic-transformer/Cargo.toml"),
                String::from("docs/ARCHITECTURE.md"),
            ],
            detail: String::from(
                "the owned article-equivalence route must remain anchored on the `psionic-transformer` crate boundary instead of drifting back into a mixed cross-crate implementation",
            ),
        },
        TassadarArticleEquivalenceAcceptanceRequirementRow {
            requirement_id: String::from(ARTICLE_EQUIVALENCE_BLOCKERS_CLOSED_REQUIREMENT_ID),
            requirement_kind: TassadarArticleEquivalenceAcceptanceRequirementKind::Contract,
            required_for_green: true,
            satisfied: blocker_matrix_report.article_equivalence_green,
            blocker_ids: blocker_matrix_report.blocker_ids.clone(),
            source_refs: vec![String::from(
                TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
            )],
            detail: String::from(
                "the blocker matrix must itself turn article-equivalence green; closing issue rows without actually closing the blocker rows is insufficient",
            ),
        },
    ];
    rows.extend(
        blocker_matrix_report
            .issue_coverage_rows
            .iter()
            .filter(|row| row.issue_role != TassadarArticleEquivalenceIssueRole::Prerequisite)
            .map(|row| {
                let required_for_green =
                    row.issue_role != TassadarArticleEquivalenceIssueRole::OptionalResearch;
                TassadarArticleEquivalenceAcceptanceRequirementRow {
                    requirement_id: row.issue_id.clone(),
                    requirement_kind: requirement_kind_for_issue_role(row.issue_role),
                    required_for_green,
                    satisfied: row.issue_state == "closed",
                    blocker_ids: row.blocker_ids.clone(),
                    source_refs: vec![String::from(
                        TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
                    )],
                    detail: issue_requirement_detail(
                        row.issue_id.as_str(),
                        row.title.as_str(),
                        row.issue_state.as_str(),
                        required_for_green,
                        row.blocker_ids.as_slice(),
                    ),
                }
            }),
    );
    rows
}

fn requirement_kind_for_issue_role(
    issue_role: TassadarArticleEquivalenceIssueRole,
) -> TassadarArticleEquivalenceAcceptanceRequirementKind {
    match issue_role {
        TassadarArticleEquivalenceIssueRole::Prerequisite => {
            TassadarArticleEquivalenceAcceptanceRequirementKind::Prerequisite
        }
        TassadarArticleEquivalenceIssueRole::Gate => {
            TassadarArticleEquivalenceAcceptanceRequirementKind::Gate
        }
        TassadarArticleEquivalenceIssueRole::Implementation => {
            TassadarArticleEquivalenceAcceptanceRequirementKind::Implementation
        }
        TassadarArticleEquivalenceIssueRole::FinalAudit => {
            TassadarArticleEquivalenceAcceptanceRequirementKind::FinalAudit
        }
        TassadarArticleEquivalenceIssueRole::OptionalResearch => {
            TassadarArticleEquivalenceAcceptanceRequirementKind::OptionalResearch
        }
    }
}

fn issue_requirement_detail(
    issue_id: &str,
    title: &str,
    issue_state: &str,
    required_for_green: bool,
    blocker_ids: &[String],
) -> String {
    if issue_state == "closed" {
        format!(
            "{issue_id} (`{title}`) is closed in the blocker matrix, so this acceptance-gate row is currently satisfied for blocker families {}.",
            blocker_ids.join(", ")
        )
    } else if required_for_green {
        format!(
            "{issue_id} (`{title}`) remains open in the blocker matrix, so the final article-equivalence gate stays blocked on blocker families {} until that tranche closes.",
            blocker_ids.join(", ")
        )
    } else {
        format!(
            "{issue_id} (`{title}`) remains open as a visible non-blocking research follow-on for blocker families {}; it stays tracked here but does not block the final gate.",
            blocker_ids.join(", ")
        )
    }
}

#[must_use]
pub fn tassadar_article_equivalence_acceptance_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF)
}

pub fn write_tassadar_article_equivalence_acceptance_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleEquivalenceAcceptanceGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleEquivalenceAcceptanceGateReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarArticleEquivalenceAcceptanceGateReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarArticleEquivalenceAcceptanceGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleEquivalenceAcceptanceGateReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

impl TassadarArticleEquivalenceAcceptanceRequirementKind {
    fn is_issue_kind(self) -> bool {
        matches!(
            self,
            TassadarArticleEquivalenceAcceptanceRequirementKind::Gate
                | TassadarArticleEquivalenceAcceptanceRequirementKind::Implementation
                | TassadarArticleEquivalenceAcceptanceRequirementKind::FinalAudit
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_report_from_blocker_matrix,
        build_tassadar_article_equivalence_acceptance_gate_report, read_json, requirement_rows,
        tassadar_article_equivalence_acceptance_gate_report_path,
        write_tassadar_article_equivalence_acceptance_gate_report,
        TassadarArticleEquivalenceAcceptanceGateReport, TassadarArticleEquivalenceAcceptanceStatus,
        ARTICLE_EQUIVALENCE_BLOCKERS_CLOSED_REQUIREMENT_ID, BLOCKER_MATRIX_CONTRACT_REQUIREMENT_ID,
        OPTIONAL_RESEARCH_ISSUE_ID, OWNED_TRANSFORMER_ROUTE_BOUNDARY_REQUIREMENT_ID,
    };
    use crate::{
        build_tassadar_article_equivalence_blocker_matrix_report,
        TassadarArticleEquivalenceIssueRole, TassadarRepoStatus,
    };

    fn green_blocker_matrix_report() -> crate::TassadarArticleEquivalenceBlockerMatrixReport {
        let mut report =
            build_tassadar_article_equivalence_blocker_matrix_report().expect("blocker matrix");
        for blocker in &mut report.blockers {
            blocker.repo_status = TassadarRepoStatus::Implemented;
        }
        for issue_row in &mut report.issue_coverage_rows {
            if issue_row.issue_role != TassadarArticleEquivalenceIssueRole::Prerequisite
                && issue_row.issue_role != TassadarArticleEquivalenceIssueRole::OptionalResearch
            {
                issue_row.issue_state = String::from("closed");
            }
        }
        report.open_blocker_count = 0;
        report.open_blocker_ids.clear();
        report.prerequisite_transformer_boundary_green = true;
        report.matrix_contract_green = true;
        report.article_equivalence_green = true;
        report
    }

    fn required_requirement_ids() -> Vec<String> {
        requirement_rows(&green_blocker_matrix_report())
            .into_iter()
            .filter(|row| row.required_for_green)
            .map(|row| row.requirement_id)
            .collect()
    }

    #[test]
    fn article_equivalence_acceptance_gate_is_green_by_default() {
        let report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("acceptance gate");

        assert_eq!(
            report.acceptance_status,
            TassadarArticleEquivalenceAcceptanceStatus::Green
        );
        assert!(report.article_equivalence_green);
        assert!(report.public_claim_allowed);
        assert!(report.blocker_matrix_contract_green);
        assert!(report.prerequisite_transformer_boundary_green);
        assert!(report.blocker_matrix_article_equivalence_green);
        assert_eq!(report.required_issue_count, 37);
        assert_eq!(report.closed_required_issue_count, 37);
        assert_eq!(report.passed_required_requirement_count, 40);
        assert!(report
            .green_requirement_ids
            .contains(&String::from(BLOCKER_MATRIX_CONTRACT_REQUIREMENT_ID)));
        assert!(report.green_requirement_ids.contains(&String::from(
            OWNED_TRANSFORMER_ROUTE_BOUNDARY_REQUIREMENT_ID
        )));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-158")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-159")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-160")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-161")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-162")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-163")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-164")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-165")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-166")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-167")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-167A")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-168")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-169")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-169A")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-170")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-171")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-171A")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-171B")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-171C")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-172")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-173")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-174")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-175")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-176")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-177")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-178")));
        assert!(report.failed_requirement_ids.is_empty());
        assert_eq!(
            report.optional_open_issue_ids,
            vec![String::from(OPTIONAL_RESEARCH_ISSUE_ID)]
        );
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-179")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-179A")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-180")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-181")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-182")));
        assert!(report
            .green_requirement_ids
            .contains(&String::from("TAS-186")));
        assert_eq!(report.closed_required_issue_count, 37);
        assert_eq!(report.passed_required_requirement_count, 40);
        assert!(report.blocked_issue_ids.is_empty());
        assert!(report.blocked_blocker_ids.is_empty());
    }

    #[test]
    fn article_equivalence_acceptance_gate_turns_green_when_required_rows_close() {
        let report = build_report_from_blocker_matrix(green_blocker_matrix_report());

        assert_eq!(
            report.acceptance_status,
            TassadarArticleEquivalenceAcceptanceStatus::Green
        );
        assert!(report.article_equivalence_green);
        assert!(report.public_claim_allowed);
        assert_eq!(report.required_issue_count, 37);
        assert_eq!(report.closed_required_issue_count, 37);
        assert!(report.failed_requirement_ids.is_empty());
        assert_eq!(
            report.optional_open_issue_ids,
            vec![String::from(OPTIONAL_RESEARCH_ISSUE_ID)]
        );
    }

    #[test]
    fn article_equivalence_acceptance_gate_fails_each_required_row_individually() {
        for failing_id in required_requirement_ids() {
            let mut blocker_matrix_report = green_blocker_matrix_report();
            if failing_id == BLOCKER_MATRIX_CONTRACT_REQUIREMENT_ID {
                blocker_matrix_report.matrix_contract_green = false;
            } else if failing_id == OWNED_TRANSFORMER_ROUTE_BOUNDARY_REQUIREMENT_ID {
                blocker_matrix_report.prerequisite_transformer_boundary_green = false;
            } else if failing_id == ARTICLE_EQUIVALENCE_BLOCKERS_CLOSED_REQUIREMENT_ID {
                blocker_matrix_report.article_equivalence_green = false;
            } else {
                let issue_row = blocker_matrix_report
                    .issue_coverage_rows
                    .iter_mut()
                    .find(|row| row.issue_id == failing_id)
                    .expect("issue row");
                issue_row.issue_state = String::from("open");
            }
            let report = build_report_from_blocker_matrix(blocker_matrix_report);

            assert_eq!(
                report.acceptance_status,
                TassadarArticleEquivalenceAcceptanceStatus::Blocked,
                "{failing_id} should force the gate blocked"
            );
            assert!(
                report.failed_requirement_ids.contains(&failing_id),
                "{failing_id} should appear in failed_requirement_ids"
            );
            assert!(!report.article_equivalence_green);
        }
    }

    #[test]
    fn article_equivalence_acceptance_gate_matches_committed_truth() {
        let generated =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("acceptance gate");
        let committed: TassadarArticleEquivalenceAcceptanceGateReport =
            read_json(tassadar_article_equivalence_acceptance_gate_report_path())
                .expect("committed acceptance gate");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_equivalence_acceptance_gate_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_equivalence_acceptance_gate_report.json");
        let written = write_tassadar_article_equivalence_acceptance_gate_report(&output_path)
            .expect("write acceptance gate");
        let persisted: TassadarArticleEquivalenceAcceptanceGateReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_equivalence_acceptance_gate_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_equivalence_acceptance_gate_report.json")
        );
    }
}
