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

use psionic_data::{
    build_tassadar_article_interpreter_breadth_envelope,
    write_tassadar_article_interpreter_breadth_envelope, TassadarArticleInterpreterBreadthEnvelope,
    TassadarArticleInterpreterBreadthEnvelopeError, TassadarArticleInterpreterBreadthPosture,
    TassadarArticleInterpreterFamilyId, TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REF,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_interpreter_breadth_envelope_report.json";
pub const TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_CHECKER_REF: &str =
    "scripts/check-tassadar-article-interpreter-breadth-envelope.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-179";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthEnvelopeAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthEnvelopeFamilyCheck {
    pub family_id: TassadarArticleInterpreterFamilyId,
    pub posture: TassadarArticleInterpreterBreadthPosture,
    pub authority_ref_count: usize,
    pub authority_refs_exist: bool,
    pub owner_surface_ref_count: usize,
    pub owner_surface_refs_exist: bool,
    pub expected_posture_green: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthEnvelopeContractCheck {
    pub manifest_ref: String,
    pub route_anchor_green: bool,
    pub suite_follow_on_issue_green: bool,
    pub frozen_core_anchor_green: bool,
    pub current_article_profile_anchor_green: bool,
    pub declared_required_family_green: bool,
    pub refusal_taxonomy_green: bool,
    pub contract_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthEnvelopeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleInterpreterBreadthEnvelopeAcceptanceGateTie,
    pub manifest_ref: String,
    pub manifest: TassadarArticleInterpreterBreadthEnvelope,
    pub contract_check: TassadarArticleInterpreterBreadthEnvelopeContractCheck,
    pub family_checks: Vec<TassadarArticleInterpreterBreadthEnvelopeFamilyCheck>,
    pub current_floor_green_count: usize,
    pub declared_required_family_green_count: usize,
    pub research_only_family_green_count: usize,
    pub explicit_out_of_envelope_green_count: usize,
    pub envelope_contract_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleInterpreterBreadthEnvelopeReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Manifest(#[from] TassadarArticleInterpreterBreadthEnvelopeError),
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

pub fn build_tassadar_article_interpreter_breadth_envelope_report() -> Result<
    TassadarArticleInterpreterBreadthEnvelopeReport,
    TassadarArticleInterpreterBreadthEnvelopeReportError,
> {
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let manifest = build_tassadar_article_interpreter_breadth_envelope();
    Ok(build_report_from_inputs(acceptance_gate, manifest))
}

fn build_report_from_inputs(
    acceptance_gate: TassadarArticleEquivalenceAcceptanceGateReport,
    manifest: TassadarArticleInterpreterBreadthEnvelope,
) -> TassadarArticleInterpreterBreadthEnvelopeReport {
    let acceptance_gate_tie = TassadarArticleInterpreterBreadthEnvelopeAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    };
    let family_checks = build_family_checks(&manifest);
    let contract_check = build_contract_check(&manifest, &family_checks);
    let current_floor_green_count = family_checks
        .iter()
        .filter(|check| {
            check.posture == TassadarArticleInterpreterBreadthPosture::CurrentFloor && check.green
        })
        .count();
    let declared_required_family_green_count = family_checks
        .iter()
        .filter(|check| {
            check.posture == TassadarArticleInterpreterBreadthPosture::DeclaredRequiredFamily
                && check.green
        })
        .count();
    let research_only_family_green_count = family_checks
        .iter()
        .filter(|check| {
            check.posture == TassadarArticleInterpreterBreadthPosture::ResearchOnlyOutsideEnvelope
                && check.green
        })
        .count();
    let explicit_out_of_envelope_green_count = family_checks
        .iter()
        .filter(|check| {
            check.posture == TassadarArticleInterpreterBreadthPosture::ExplicitlyOutsideEnvelope
                && check.green
        })
        .count();
    let envelope_contract_green = acceptance_gate_tie.tied_requirement_satisfied
        && contract_check.contract_green
        && family_checks.iter().all(|check| check.green);
    let mut report = TassadarArticleInterpreterBreadthEnvelopeReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_interpreter_breadth_envelope.report.v1"),
        checker_script_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_CHECKER_REF),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        manifest_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REF),
        manifest,
        contract_check,
        family_checks,
        current_floor_green_count,
        declared_required_family_green_count,
        research_only_family_green_count,
        explicit_out_of_envelope_green_count,
        envelope_contract_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && envelope_contract_green,
        claim_boundary: String::from(
            "this report closes TAS-179 only. It freezes one declared article interpreter breadth envelope over the frozen core-Wasm floor, the current named article i32 profiles, and the later required search-process, long-horizon control, and module-scale Wasm-loop families while keeping linked-program bundles research-only and import-mediated processes, dynamic-memory resume, memory64, multi-memory, component-linking, exception profiles, and broader float semantics explicitly outside the article envelope. It does not yet build the breadth suite or turn final article-equivalence green.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article interpreter breadth envelope report now records tied_requirement_satisfied={}, current_floor_green={}/{}, declared_required_family_green={}/{}, research_only_family_green={}/{}, explicit_out_of_envelope_green={}/{}, envelope_contract_green={}, and article_equivalence_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.current_floor_green_count,
        report.manifest.current_floor_family_ids.len(),
        report.declared_required_family_green_count,
        report.manifest.declared_required_family_ids.len(),
        report.research_only_family_green_count,
        report.manifest.research_only_family_ids.len(),
        report.explicit_out_of_envelope_green_count,
        report.manifest.explicit_out_of_envelope_family_ids.len(),
        report.envelope_contract_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_interpreter_breadth_envelope_report|",
        &report,
    );
    report
}

fn build_family_checks(
    manifest: &TassadarArticleInterpreterBreadthEnvelope,
) -> Vec<TassadarArticleInterpreterBreadthEnvelopeFamilyCheck> {
    manifest
        .family_rows
        .iter()
        .map(|row| {
            let authority_refs_exist = row
                .authority_refs
                .iter()
                .all(|path| repo_root().join(path).is_file());
            let owner_surface_refs_exist = row
                .owner_surface_refs
                .iter()
                .all(|path| repo_root().join(path).is_file());
            let expected_posture_green = expected_posture(manifest, row.family_id) == row.posture;
            let green = authority_refs_exist && owner_surface_refs_exist && expected_posture_green;
            TassadarArticleInterpreterBreadthEnvelopeFamilyCheck {
                family_id: row.family_id,
                posture: row.posture,
                authority_ref_count: row.authority_refs.len(),
                authority_refs_exist,
                owner_surface_ref_count: row.owner_surface_refs.len(),
                owner_surface_refs_exist,
                expected_posture_green,
                green,
                detail: row.detail.clone(),
            }
        })
        .collect()
}

fn build_contract_check(
    manifest: &TassadarArticleInterpreterBreadthEnvelope,
    family_checks: &[TassadarArticleInterpreterBreadthEnvelopeFamilyCheck],
) -> TassadarArticleInterpreterBreadthEnvelopeContractCheck {
    let route_anchor_green = manifest.route_anchor == "psionic_transformer.article_route";
    let suite_follow_on_issue_green = manifest.suite_follow_on_issue_id == "TAS-179A";
    let frozen_core_anchor_green = family_row_has_authority_refs(
        manifest,
        TassadarArticleInterpreterFamilyId::FrozenCoreWasmWindow,
        &[
            "fixtures/tassadar/reports/tassadar_frozen_core_wasm_window_report.json",
            "fixtures/tassadar/reports/tassadar_frozen_core_wasm_closure_gate_report.json",
        ],
    );
    let current_article_profile_anchor_green = family_row_has_authority_refs(
        manifest,
        TassadarArticleInterpreterFamilyId::ArticleNamedI32Profiles,
        &[
            "fixtures/tassadar/reports/tassadar_rust_article_profile_completeness_report.json",
            "fixtures/tassadar/reports/tassadar_article_abi_closure_report.json",
            "fixtures/tassadar/reports/tassadar_article_frontend_compiler_envelope_report.json",
            "fixtures/tassadar/reports/tassadar_article_frontend_corpus_compile_matrix_report.json",
            "fixtures/tassadar/reports/tassadar_article_demo_frontend_parity_report.json",
        ],
    );
    let declared_required_family_green = manifest
        .declared_required_family_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        == BTreeSet::from([
            TassadarArticleInterpreterFamilyId::SearchProcessFamily,
            TassadarArticleInterpreterFamilyId::LongHorizonControlFamily,
            TassadarArticleInterpreterFamilyId::ModuleScaleWasmLoopFamily,
        ]);
    let refusal_taxonomy_green = family_checks.iter().all(|check| match check.posture {
        TassadarArticleInterpreterBreadthPosture::ExplicitlyOutsideEnvelope
        | TassadarArticleInterpreterBreadthPosture::ResearchOnlyOutsideEnvelope => check.green,
        TassadarArticleInterpreterBreadthPosture::CurrentFloor
        | TassadarArticleInterpreterBreadthPosture::DeclaredRequiredFamily => true,
    });
    let contract_green = route_anchor_green
        && suite_follow_on_issue_green
        && frozen_core_anchor_green
        && current_article_profile_anchor_green
        && declared_required_family_green
        && refusal_taxonomy_green;
    TassadarArticleInterpreterBreadthEnvelopeContractCheck {
        manifest_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REF),
        route_anchor_green,
        suite_follow_on_issue_green,
        frozen_core_anchor_green,
        current_article_profile_anchor_green,
        declared_required_family_green,
        refusal_taxonomy_green,
        contract_green,
        detail: String::from(
            "the declared article interpreter envelope must stay pinned to the canonical `psionic-transformer` article route, keep `TAS-179A` as the follow-on suite-and-gate tranche, keep the frozen core-Wasm floor plus current article profile anchors explicit, and keep every outside-the-envelope family machine-legible through concrete authority refs and owner-surface refs",
        ),
    }
}

fn family_row_has_authority_refs(
    manifest: &TassadarArticleInterpreterBreadthEnvelope,
    family_id: TassadarArticleInterpreterFamilyId,
    expected_refs: &[&str],
) -> bool {
    manifest
        .family_rows
        .iter()
        .find(|row| row.family_id == family_id)
        .map(|row| {
            let refs = row.authority_refs.iter().cloned().collect::<BTreeSet<_>>();
            expected_refs
                .iter()
                .all(|expected_ref| refs.contains(*expected_ref))
        })
        .unwrap_or(false)
}

fn expected_posture(
    manifest: &TassadarArticleInterpreterBreadthEnvelope,
    family_id: TassadarArticleInterpreterFamilyId,
) -> TassadarArticleInterpreterBreadthPosture {
    if manifest.current_floor_family_ids.contains(&family_id) {
        TassadarArticleInterpreterBreadthPosture::CurrentFloor
    } else if manifest.declared_required_family_ids.contains(&family_id) {
        TassadarArticleInterpreterBreadthPosture::DeclaredRequiredFamily
    } else if manifest
        .explicit_out_of_envelope_family_ids
        .contains(&family_id)
    {
        TassadarArticleInterpreterBreadthPosture::ExplicitlyOutsideEnvelope
    } else {
        TassadarArticleInterpreterBreadthPosture::ResearchOnlyOutsideEnvelope
    }
}

pub fn tassadar_article_interpreter_breadth_envelope_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REPORT_REF)
}

pub fn write_tassadar_article_interpreter_breadth_envelope_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleInterpreterBreadthEnvelopeReport,
    TassadarArticleInterpreterBreadthEnvelopeReportError,
> {
    write_tassadar_article_interpreter_breadth_envelope(
        repo_root().join(TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REF),
    )?;
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleInterpreterBreadthEnvelopeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_interpreter_breadth_envelope_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleInterpreterBreadthEnvelopeReportError::Write {
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

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleInterpreterBreadthEnvelopeReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleInterpreterBreadthEnvelopeReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleInterpreterBreadthEnvelopeReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_interpreter_breadth_envelope_report, read_repo_json,
        tassadar_article_interpreter_breadth_envelope_report_path,
        write_tassadar_article_interpreter_breadth_envelope_report,
        TassadarArticleInterpreterBreadthEnvelopeReport,
        TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REPORT_REF,
    };

    #[test]
    fn article_interpreter_breadth_envelope_tracks_declared_envelope_with_final_green() {
        let report = build_tassadar_article_interpreter_breadth_envelope_report().expect("report");

        assert_eq!(report.acceptance_gate_tie.tied_requirement_id, "TAS-179");
        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.acceptance_gate_tie.blocked_issue_ids.is_empty());
        assert_eq!(report.current_floor_green_count, 2);
        assert_eq!(report.declared_required_family_green_count, 3);
        assert_eq!(report.research_only_family_green_count, 1);
        assert_eq!(report.explicit_out_of_envelope_green_count, 7);
        assert!(report.envelope_contract_green);
        assert!(report.article_equivalence_green);
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn article_interpreter_breadth_envelope_matches_committed_truth() {
        let generated =
            build_tassadar_article_interpreter_breadth_envelope_report().expect("report");
        let committed: TassadarArticleInterpreterBreadthEnvelopeReport = read_repo_json(
            TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REPORT_REF,
            "article_interpreter_breadth_envelope_report",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_interpreter_breadth_envelope_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_interpreter_breadth_envelope_report.json");
        let written = write_tassadar_article_interpreter_breadth_envelope_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleInterpreterBreadthEnvelopeReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_interpreter_breadth_envelope_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_interpreter_breadth_envelope_report.json")
        );
    }
}
