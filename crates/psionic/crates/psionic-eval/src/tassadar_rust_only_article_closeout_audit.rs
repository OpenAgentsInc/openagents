use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF,
    TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF,
    TassadarArticleCpuReproducibilityReport, TassadarCompilePipelineMatrixCaseStatus,
    TassadarRustOnlyArticleAcceptanceGateV2Report,
};

pub const TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_only_article_closeout_audit_report.json";
pub const TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_AUDIT_CHECKER_REF: &str =
    "scripts/check-tassadar-rust-only-article-closeout-audit.sh";

const HARNESS_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_only_article_reproduction_report.json";
const DIRECT_PROOF_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_direct_model_weight_execution_proof_report.json";
const REQUIRED_SUPPORTED_MACHINE_CLASS_IDS: [&str; 2] = ["host_cpu_aarch64", "host_cpu_x86_64"];
const REQUIRED_UNSUPPORTED_MACHINE_CLASS_ID: &str = "other_host_cpu";
const REQUIRED_DIRECT_PROOF_CASE_IDS: [&str; 3] =
    ["long_loop_kernel", "sudoku_v0_test_a", "hungarian_matching"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustOnlyArticleCloseoutAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub harness_report_ref: String,
    pub acceptance_gate_report_ref: String,
    pub direct_model_weight_execution_proof_report_ref: String,
    pub cpu_reproducibility_report_ref: String,
    pub direct_route_descriptor_digest: String,
    pub direct_proof_case_ids: Vec<String>,
    pub current_host_machine_class_id: String,
    pub harness_green: bool,
    pub acceptance_gate_green: bool,
    pub direct_model_weight_proof_green: bool,
    pub cpu_reproducibility_green: bool,
    pub all_surfaces_green: bool,
    pub reproduced_claim: String,
    pub canonical_case_ids: Vec<String>,
    pub supported_machine_class_ids: Vec<String>,
    pub remaining_exclusions: Vec<String>,
    pub green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarRustOnlyArticleCloseoutAuditReport {
    #[allow(clippy::too_many_arguments)]
    fn new(
        harness_green: bool,
        acceptance_gate_green: bool,
        direct_model_weight_proof_green: bool,
        cpu_reproducibility_green: bool,
        direct_route_descriptor_digest: String,
        direct_proof_case_ids: Vec<String>,
        current_host_machine_class_id: String,
        canonical_case_ids: Vec<String>,
        supported_machine_class_ids: Vec<String>,
    ) -> Self {
        let all_surfaces_green = harness_green
            && acceptance_gate_green
            && direct_model_weight_proof_green
            && cpu_reproducibility_green;
        let green = all_surfaces_green;
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.rust_only_article_closeout_audit.v1"),
            checker_script_ref: String::from(TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_AUDIT_CHECKER_REF),
            harness_report_ref: String::from(HARNESS_REPORT_REF),
            acceptance_gate_report_ref: String::from(
                TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF,
            ),
            direct_model_weight_execution_proof_report_ref: String::from(DIRECT_PROOF_REPORT_REF),
            cpu_reproducibility_report_ref: String::from(
                TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF,
            ),
            direct_route_descriptor_digest,
            direct_proof_case_ids,
            current_host_machine_class_id,
            harness_green,
            acceptance_gate_green,
            direct_model_weight_proof_green,
            cpu_reproducibility_green,
            all_surfaces_green,
            reproduced_claim: String::from(
                "Psionic now reproduces the full Rust-only Percepta article claim end to end on the committed canonical article workloads, with one-command operator procedure, a green prerequisite gate, direct model-weight execution proof on the declared article cases, and explicit CPU portability closure on the declared host CPU classes.",
            ),
            canonical_case_ids,
            supported_machine_class_ids,
            remaining_exclusions: vec![
                String::from(
                    "no arbitrary Rust or arbitrary Wasm closure beyond the committed Rust-only article family",
                ),
                String::from("no non-CPU backend portability or backend-invariant closure"),
                String::from("no portability claim beyond host_cpu_aarch64 and host_cpu_x86_64"),
                String::from(
                    "no claim beyond the committed canonical article workloads and the direct route-bound proof surface",
                ),
                String::from(
                    "no world-mount, accepted-outcome, settlement, or market-grade closure from this audit",
                ),
            ],
            green,
            claim_boundary: String::from(
                "this report is the final publication boundary for the Rust-only article claim only. It cites the one-command harness, the v2 prerequisite gate, the direct model-weight proof surface, and the CPU reproducibility matrix, and it must not be read as a broader Wasm, backend, or product closure claim",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Rust-only article closeout audit now records harness_green={}, gate_green={}, direct_proof_green={}, cpu_green={}, all_surfaces_green={}, and green={}.",
            report.harness_green,
            report.acceptance_gate_green,
            report.direct_model_weight_proof_green,
            report.cpu_reproducibility_green,
            report.all_surfaces_green,
            report.green,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_rust_only_article_closeout_audit_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarRustOnlyArticleCloseoutAuditReportError {
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

pub fn build_tassadar_rust_only_article_closeout_audit_report() -> Result<
    TassadarRustOnlyArticleCloseoutAuditReport,
    TassadarRustOnlyArticleCloseoutAuditReportError,
> {
    let harness: Value = read_repo_json(HARNESS_REPORT_REF, "rust_only_article_reproduction")?;
    let gate: TassadarRustOnlyArticleAcceptanceGateV2Report = read_repo_json(
        TASSADAR_RUST_ONLY_ARTICLE_ACCEPTANCE_GATE_V2_REPORT_REF,
        "rust_only_article_acceptance_gate_v2",
    )?;
    let direct_proof: Value = read_repo_json(
        DIRECT_PROOF_REPORT_REF,
        "direct_model_weight_execution_proof",
    )?;
    let cpu_report: TassadarArticleCpuReproducibilityReport = read_repo_json(
        TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF,
        "article_cpu_reproducibility",
    )?;

    let canonical_case_ids = string_array(&harness["canonical_case_ids"]);
    let direct_proof_case_ids = string_array(&direct_proof["case_ids"]);
    let direct_route_descriptor_digest = direct_proof["route_descriptor_digest"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let harness_green = harness["all_components_green"].as_bool() == Some(true)
        && harness["component_count"].as_u64() == Some(9)
        && harness["green_component_count"].as_u64() == Some(9)
        && harness["components"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|component| {
                component["component_id"].as_str() == Some("rust_only_article_acceptance_gate_v2")
                    && component["green"].as_bool() == Some(true)
            });

    let required_direct_case_ids = REQUIRED_DIRECT_PROOF_CASE_IDS
        .into_iter()
        .map(String::from)
        .collect::<BTreeSet<_>>();
    let observed_direct_case_ids = direct_proof_case_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let direct_receipts = direct_proof["receipts"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let direct_model_weight_proof_green = direct_proof["direct_case_count"].as_u64() == Some(3)
        && direct_proof["fallback_free_case_count"].as_u64() == Some(3)
        && direct_proof["zero_external_call_case_count"].as_u64() == Some(3)
        && !direct_route_descriptor_digest.is_empty()
        && observed_direct_case_ids == required_direct_case_ids
        && direct_receipts.len() == REQUIRED_DIRECT_PROOF_CASE_IDS.len()
        && direct_receipts.iter().all(|receipt| {
            receipt["article_case_id"]
                .as_str()
                .map(|case_id| required_direct_case_ids.contains(case_id))
                == Some(true)
                && receipt["requested_decode_mode"].as_str() == Some("reference_linear")
                && receipt["effective_decode_mode"].as_str() == Some("reference_linear")
                && receipt["selection_state"].as_str() == Some("direct")
                && receipt["fallback_observed"].as_bool() == Some(false)
                && receipt["external_call_count"].as_u64() == Some(0)
                && receipt["external_tool_surface_observed"].as_bool() == Some(false)
                && receipt["cpu_result_substitution_observed"].as_bool() == Some(false)
                && receipt["route_binding"]["route_posture"].as_str() == Some("direct_guaranteed")
                && receipt["route_binding"]["route_descriptor_digest"].as_str()
                    == Some(direct_route_descriptor_digest.as_str())
        });

    let supported_machine_class_ids = cpu_report
        .supported_machine_class_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required_supported_machine_class_ids = REQUIRED_SUPPORTED_MACHINE_CLASS_IDS
        .into_iter()
        .map(String::from)
        .collect::<BTreeSet<_>>();
    let unsupported_machine_class_ids = cpu_report
        .unsupported_machine_class_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let cpu_reproducibility_green = cpu_report.matrix.current_host_measured_green
        && cpu_report.optional_c_path_status
            == TassadarCompilePipelineMatrixCaseStatus::CompileRefused
        && cpu_report.optional_c_path_compile_refusal_kind.as_deref()
            == Some("toolchain_unavailable")
        && !cpu_report.optional_c_path_blocks_rust_only_claim
        && cpu_report.rust_toolchain_case_count == 8
        && cpu_report.rust_toolchain_uniform
        && supported_machine_class_ids == required_supported_machine_class_ids
        && unsupported_machine_class_ids
            == BTreeSet::from([String::from(REQUIRED_UNSUPPORTED_MACHINE_CLASS_ID)]);

    Ok(TassadarRustOnlyArticleCloseoutAuditReport::new(
        harness_green,
        gate.green,
        direct_model_weight_proof_green,
        cpu_reproducibility_green,
        direct_route_descriptor_digest,
        direct_proof_case_ids,
        cpu_report.matrix.current_host_machine_class_id.clone(),
        canonical_case_ids,
        cpu_report.supported_machine_class_ids.clone(),
    ))
}

pub fn tassadar_rust_only_article_closeout_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_AUDIT_REPORT_REF)
}

pub fn write_tassadar_rust_only_article_closeout_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarRustOnlyArticleCloseoutAuditReport,
    TassadarRustOnlyArticleCloseoutAuditReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarRustOnlyArticleCloseoutAuditReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_rust_only_article_closeout_audit_report()?;
    let json = serde_json::to_string_pretty(&report)
        .expect("Rust-only article closeout audit report serializes");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarRustOnlyArticleCloseoutAuditReportError::Write {
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

fn string_array(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.as_str().map(String::from))
        .collect()
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarRustOnlyArticleCloseoutAuditReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarRustOnlyArticleCloseoutAuditReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarRustOnlyArticleCloseoutAuditReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_AUDIT_REPORT_REF,
        TassadarRustOnlyArticleCloseoutAuditReport,
        build_tassadar_rust_only_article_closeout_audit_report, read_repo_json,
        tassadar_rust_only_article_closeout_audit_report_path,
        write_tassadar_rust_only_article_closeout_audit_report,
    };

    #[test]
    fn rust_only_article_closeout_audit_is_green_on_current_truth() {
        let report = build_tassadar_rust_only_article_closeout_audit_report().expect("report");

        assert!(report.green);
        assert!(report.harness_green);
        assert!(report.acceptance_gate_green);
        assert!(report.direct_model_weight_proof_green);
        assert!(report.cpu_reproducibility_green);
        assert!(report.all_surfaces_green);
        assert_eq!(report.supported_machine_class_ids.len(), 2);
        assert_eq!(report.direct_proof_case_ids.len(), 3);
    }

    #[test]
    fn rust_only_article_closeout_audit_matches_committed_truth() {
        let generated = build_tassadar_rust_only_article_closeout_audit_report().expect("report");
        let committed: TassadarRustOnlyArticleCloseoutAuditReport = read_repo_json(
            TASSADAR_RUST_ONLY_ARTICLE_CLOSEOUT_AUDIT_REPORT_REF,
            "rust_only_article_closeout_audit",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_rust_only_article_closeout_audit_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_rust_only_article_closeout_audit_report.json");
        let written = write_tassadar_rust_only_article_closeout_audit_report(&output_path)
            .expect("write report");
        let persisted: TassadarRustOnlyArticleCloseoutAuditReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_rust_only_article_closeout_audit_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_rust_only_article_closeout_audit_report.json")
        );
    }

    #[test]
    fn closeout_audit_requires_green_acceptance_gate() {
        let report = TassadarRustOnlyArticleCloseoutAuditReport::new(
            true,
            false,
            true,
            true,
            String::from("route_digest"),
            vec![String::from("hungarian_matching")],
            String::from("host_cpu_aarch64"),
            vec![String::from("hungarian_10x10_test_a")],
            vec![
                String::from("host_cpu_aarch64"),
                String::from("host_cpu_x86_64"),
            ],
        );

        assert!(!report.green);
        assert!(!report.all_surfaces_green);
    }
}
