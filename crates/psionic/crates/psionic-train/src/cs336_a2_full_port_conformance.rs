use std::{collections::BTreeSet, fs, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH, CS336_A2_DDP_BUCKETED_RECEIPT_FIXTURE_PATH,
    CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_FIXTURE_PATH,
    CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_FIXTURE_PATH,
    CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_FIXTURE_PATH,
    CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_FIXTURE_PATH,
    CS336_A2_FSDP_FULL_PARAMS_RECEIPT_FIXTURE_PATH, CS336_A2_FSDP_WRAPPER_RECEIPT_FIXTURE_PATH,
    CS336_A2_SHARDED_OPTIMIZER_RECEIPT_FIXTURE_PATH,
};

pub const CS336_A2_FULL_PORT_CONFORMANCE_REPORT_SCHEMA_VERSION: &str =
    "psion.cs336_a2.full_port_conformance_report.v1";
pub const CS336_A2_FULL_PORT_CONFORMANCE_REPORT_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a2_full_port_conformance_report_v1.json";
pub const CS336_A2_FULL_PORT_MATRIX_DOC_PATH: &str = "docs/PSION_CS336_A2_FULL_PORT_MATRIX.md";
pub const CS336_A2_REFERENCE_LANE_DOC_PATH_FOR_CONFORMANCE: &str =
    "docs/PSION_CS336_A2_REFERENCE_LANE.md";

const FLASHATTENTION_REFERENCE_SOURCE_PATH: &str =
    "crates/psionic-train/src/cs336_a2_flashattention_reference_receipt.rs";
const FLASHATTENTION_REFERENCE_IMPL_PATH: &str =
    "crates/psionic-models/src/cs336_a2_flashattention_reference.rs";
const FLASHATTENTION_FUSED_SOURCE_PATH: &str =
    "crates/psionic-train/src/cs336_a2_flashattention_fused_cuda_receipt.rs";
const FLASHATTENTION_FUSED_IMPL_PATH: &str = "crates/psionic-backend-cuda/src/lib.rs";
const DDP_INDIVIDUAL_SOURCE_PATH: &str =
    "crates/psionic-train/src/cs336_a2_ddp_individual_parameters_receipt.rs";
const SHARDED_OPTIMIZER_SOURCE_PATH: &str =
    "crates/psionic-train/src/cs336_a2_sharded_optimizer_receipt.rs";
const FSDP_AFTER_BACKWARD_SOURCE_PATH: &str =
    "crates/psionic-train/src/cs336_a2_fsdp_after_backward_receipt.rs";
const FSDP_FULL_PARAMS_SOURCE_PATH: &str =
    "crates/psionic-train/src/cs336_a2_fsdp_full_params_receipt.rs";
const FSDP_WRAPPER_SOURCE_PATH: &str = "crates/psionic-train/src/cs336_a2_fsdp_wrapper_receipt.rs";

const EXPECTED_STANFORD_ADAPTERS: [&str; 8] = [
    "get_flashattention_autograd_function_pytorch",
    "get_flashattention_autograd_function_triton",
    "get_ddp",
    "ddp_on_after_backward",
    "get_fsdp",
    "fsdp_on_after_backward",
    "fsdp_gather_full_params",
    "get_sharded_optimizer",
];

#[derive(Debug, Error)]
pub enum Cs336A2FullPortConformanceError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("missing or invalid proof surface: {0}")]
    MissingProofSurface(String),
    #[error("missing or invalid implementation surface: {0}")]
    MissingImplementationSurface(String),
    #[error("invalid CS336 A2 full-port conformance report: {0}")]
    InvalidReport(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2ConformanceProofSurface {
    pub proof_kind: String,
    pub repo_relative_path: String,
    pub selector: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2ConformanceRow {
    pub stanford_adapter_name: String,
    pub category: String,
    pub status: String,
    pub implementation_surfaces: Vec<String>,
    pub proof_surfaces: Vec<Cs336A2ConformanceProofSurface>,
    pub follow_up_issue_urls: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2FullPortConformanceReport {
    pub schema_version: String,
    pub completion_matrix_path: String,
    pub reference_lane_doc_path: String,
    pub retained_proof_bundle_paths: Vec<String>,
    pub row_count: usize,
    pub green_row_count: usize,
    pub fully_green: bool,
    pub rows: Vec<Cs336A2ConformanceRow>,
    pub claim_boundary: String,
}

pub fn build_cs336_a2_full_port_conformance_report(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2FullPortConformanceReport, Cs336A2FullPortConformanceError> {
    let repo_root = repo_root.as_ref();
    let rows = expected_rows();
    for row in &rows {
        validate_row(repo_root, row)?;
    }
    let report = Cs336A2FullPortConformanceReport {
        schema_version: String::from(CS336_A2_FULL_PORT_CONFORMANCE_REPORT_SCHEMA_VERSION),
        completion_matrix_path: String::from(CS336_A2_FULL_PORT_MATRIX_DOC_PATH),
        reference_lane_doc_path: String::from(CS336_A2_REFERENCE_LANE_DOC_PATH_FOR_CONFORMANCE),
        retained_proof_bundle_paths: vec![
            String::from(CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH),
            String::from(CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_FIXTURE_PATH),
            String::from(CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_FIXTURE_PATH),
            String::from(CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_FIXTURE_PATH),
            String::from(CS336_A2_DDP_BUCKETED_RECEIPT_FIXTURE_PATH),
            String::from(CS336_A2_SHARDED_OPTIMIZER_RECEIPT_FIXTURE_PATH),
            String::from(CS336_A2_FSDP_WRAPPER_RECEIPT_FIXTURE_PATH),
            String::from(CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_FIXTURE_PATH),
            String::from(CS336_A2_FSDP_FULL_PARAMS_RECEIPT_FIXTURE_PATH),
        ],
        row_count: rows.len(),
        green_row_count: rows
            .iter()
            .filter(|row| row.status == "green_bounded_reference")
            .count(),
        fully_green: rows
            .iter()
            .all(|row| row.status == "green_bounded_reference"),
        rows,
        claim_boundary: String::from(
            "This report tracks the current Spring 2026 Stanford CS336 Assignment 2 adapter surface as a bounded psionic reference lane. It no longer claims full current A2 parity: FlashAttention and sharded-optimizer surfaces have retained bounded proofs, DDP is mapped to bounded host-reference receipts under the current get_ddp/ddp_on_after_backward names, and the current FSDP adapter names now have bounded wrapper, after-backward, and full-parameter gather receipts. This does not promote the bounded systems lane into the actual Psion pretraining operator lane, does not claim admitted distributed throughput or transport-backed cluster execution, and is not a prerequisite for a1_minimal_distributed_lm_001.",
        ),
    };
    validate_report(&report)?;
    Ok(report)
}

pub fn write_cs336_a2_full_port_conformance_report(
    output_root: impl AsRef<Path>,
    report: &Cs336A2FullPortConformanceReport,
) -> Result<(), Cs336A2FullPortConformanceError> {
    validate_report(report)?;
    let report_path = output_root
        .as_ref()
        .join(CS336_A2_FULL_PORT_CONFORMANCE_REPORT_FIXTURE_PATH);
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(report_path, serde_json::to_vec_pretty(report)?)?;
    Ok(())
}

pub fn write_cs336_a2_full_port_conformance_fixture(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2FullPortConformanceReport, Cs336A2FullPortConformanceError> {
    let report = build_cs336_a2_full_port_conformance_report(&repo_root)?;
    write_cs336_a2_full_port_conformance_report(repo_root, &report)?;
    Ok(report)
}

fn validate_report(
    report: &Cs336A2FullPortConformanceReport,
) -> Result<(), Cs336A2FullPortConformanceError> {
    if report.schema_version != CS336_A2_FULL_PORT_CONFORMANCE_REPORT_SCHEMA_VERSION {
        return Err(Cs336A2FullPortConformanceError::InvalidReport(format!(
            "expected schema version `{CS336_A2_FULL_PORT_CONFORMANCE_REPORT_SCHEMA_VERSION}`, got `{}`",
            report.schema_version
        )));
    }
    if report.row_count != report.rows.len() {
        return Err(Cs336A2FullPortConformanceError::InvalidReport(format!(
            "row_count {} does not match {} rows",
            report.row_count,
            report.rows.len()
        )));
    }
    let green_row_count = report
        .rows
        .iter()
        .filter(|row| row.status == "green_bounded_reference")
        .count();
    if report.green_row_count != green_row_count {
        return Err(Cs336A2FullPortConformanceError::InvalidReport(format!(
            "green_row_count {} does not match computed count {green_row_count}",
            report.green_row_count
        )));
    }
    let adapter_set = report
        .rows
        .iter()
        .map(|row| row.stanford_adapter_name.as_str())
        .collect::<BTreeSet<_>>();
    let expected = EXPECTED_STANFORD_ADAPTERS
        .into_iter()
        .collect::<BTreeSet<_>>();
    if adapter_set != expected {
        return Err(Cs336A2FullPortConformanceError::InvalidReport(format!(
            "adapter set mismatch: expected {:?}, got {:?}",
            expected, adapter_set
        )));
    }
    if report.fully_green {
        return Err(Cs336A2FullPortConformanceError::InvalidReport(
            "current A2 report must not claim fully_green while FSDP rows are tracked gaps".into(),
        ));
    }
    Ok(())
}

fn validate_row(
    repo_root: &Path,
    row: &Cs336A2ConformanceRow,
) -> Result<(), Cs336A2FullPortConformanceError> {
    let admitted_status = matches!(
        row.status.as_str(),
        "green_bounded_reference" | "partial_bounded_reference" | "missing_tracked"
    );
    if !admitted_status {
        return Err(Cs336A2FullPortConformanceError::InvalidReport(format!(
            "row `{}` has unsupported status `{}`",
            row.stanford_adapter_name, row.status
        )));
    }
    if row.status == "missing_tracked" {
        if row.follow_up_issue_urls.is_empty() {
            return Err(Cs336A2FullPortConformanceError::InvalidReport(format!(
                "missing row `{}` must cite at least one follow-up issue",
                row.stanford_adapter_name
            )));
        }
        return Ok(());
    }
    if row.implementation_surfaces.is_empty() {
        return Err(Cs336A2FullPortConformanceError::InvalidReport(format!(
            "row `{}` is missing implementation surfaces",
            row.stanford_adapter_name
        )));
    }
    if row.proof_surfaces.is_empty() {
        return Err(Cs336A2FullPortConformanceError::InvalidReport(format!(
            "row `{}` is missing proof surfaces",
            row.stanford_adapter_name
        )));
    }
    for implementation_surface in &row.implementation_surfaces {
        let implementation_path = repo_root.join(implementation_surface);
        if !implementation_path.exists() {
            return Err(
                Cs336A2FullPortConformanceError::MissingImplementationSurface(format!(
                    "`{implementation_surface}` does not exist"
                )),
            );
        }
    }
    for proof in &row.proof_surfaces {
        validate_proof_surface(repo_root, proof)?;
    }
    Ok(())
}

fn validate_proof_surface(
    repo_root: &Path,
    proof: &Cs336A2ConformanceProofSurface,
) -> Result<(), Cs336A2FullPortConformanceError> {
    let proof_path = repo_root.join(&proof.repo_relative_path);
    if !proof_path.exists() {
        return Err(Cs336A2FullPortConformanceError::MissingProofSurface(
            format!("`{}` does not exist", proof.repo_relative_path),
        ));
    }
    if let Some(selector) = &proof.selector {
        let contents = fs::read_to_string(&proof_path)?;
        if !contents.contains(selector) {
            return Err(Cs336A2FullPortConformanceError::MissingProofSurface(
                format!(
                    "`{}` does not contain selector `{selector}`",
                    proof.repo_relative_path
                ),
            ));
        }
    }
    if proof.repo_relative_path.ends_with(".json") {
        let bytes = fs::read(&proof_path)?;
        let _: serde_json::Value = serde_json::from_slice(&bytes)?;
    }
    Ok(())
}

fn expected_rows() -> Vec<Cs336A2ConformanceRow> {
    vec![
        row(
            "get_flashattention_autograd_function_pytorch",
            "attention",
            "green_bounded_reference",
            vec![
                FLASHATTENTION_REFERENCE_IMPL_PATH,
                FLASHATTENTION_REFERENCE_SOURCE_PATH,
            ],
            vec![
                source_proof(
                    FLASHATTENTION_REFERENCE_SOURCE_PATH,
                    "build_cs336_a2_flashattention_reference_receipt",
                    "Owned bounded reference receipt over the PyTorch-style FlashAttention2 path.",
                ),
                json_proof(
                    CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_FIXTURE_PATH,
                    "Retained parity and memory-surface receipt for the reference path.",
                ),
            ],
            vec![],
            "The PyTorch-only FlashAttention adapter is closed by the owned tiled forward/backward reference implementation plus the retained parity receipt.",
        ),
        row(
            "get_flashattention_autograd_function_triton",
            "attention",
            "partial_bounded_reference",
            vec![FLASHATTENTION_FUSED_IMPL_PATH, FLASHATTENTION_FUSED_SOURCE_PATH],
            vec![
                source_proof(
                    FLASHATTENTION_FUSED_SOURCE_PATH,
                    "build_cs336_a2_flashattention_fused_cuda_receipt",
                    "Owned bounded fused CUDA receipt family for the Triton-class adapter surface.",
                ),
                json_proof(
                    CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_FIXTURE_PATH,
                    "Retained admitted-path or explicit-refusal receipt for the fused CUDA lane.",
                ),
            ],
            vec![],
            "The Triton-class FlashAttention adapter is mapped to the owned fused CUDA receipt family, with explicit refusal posture on non-CUDA hosts. This is a bounded fused-backend analogue, not a claim that Psionic ships the Stanford Triton kernel surface.",
        ),
        row(
            "get_ddp",
            "distributed",
            "partial_bounded_reference",
            vec![DDP_INDIVIDUAL_SOURCE_PATH],
            vec![
                source_proof(
                    DDP_INDIVIDUAL_SOURCE_PATH,
                    "build_cs336_a2_ddp_individual_parameters_receipt",
                    "Owned bounded constructor surface for the individual-parameter DDP lane.",
                ),
                json_proof(
                    CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_FIXTURE_PATH,
                    "Retained two-rank parity receipt for the bounded individual-parameter DDP lane.",
                ),
            ],
            vec![],
            "The current get_ddp adapter is mapped to the owned two-rank individual-parameter receipt lane above the A1 trainer. It proves bounded broadcast and per-parameter averaging, but not asynchronous overlap or transport-backed collectives.",
        ),
        row(
            "ddp_on_after_backward",
            "distributed",
            "partial_bounded_reference",
            vec![DDP_INDIVIDUAL_SOURCE_PATH],
            vec![
                source_proof(
                    DDP_INDIVIDUAL_SOURCE_PATH,
                    "parameter_syncs",
                    "The after-backward hook is represented by retained per-parameter synchronization receipts before the optimizer step.",
                ),
                json_proof(
                    CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_FIXTURE_PATH,
                    "Retained proof that the bounded after-backward sync path stays aligned with the non-parallel baseline.",
                ),
            ],
            vec![],
            "The current DDP after-backward hook is mapped to retained per-parameter sync receipts and bounded parity proof. It does not yet claim true async gradient communication overlap.",
        ),
        row(
            "get_fsdp",
            "fsdp",
            "partial_bounded_reference",
            vec![FSDP_WRAPPER_SOURCE_PATH],
            vec![
                source_proof(
                    FSDP_WRAPPER_SOURCE_PATH,
                    "build_cs336_a2_fsdp_wrapper_receipt",
                    "Owned bounded constructor and parameter-lifecycle receipt for the get_fsdp adapter surface.",
                ),
                json_proof(
                    CS336_A2_FSDP_WRAPPER_RECEIPT_FIXTURE_PATH,
                    "Retained proof that Linear and Embedding parameters are sharded while replicated parameters remain explicit.",
                ),
            ],
            vec![],
            "The current get_fsdp adapter is mapped to a bounded host-reference wrapper receipt over the ToyFSDPModel parameter families. It proves sharded Linear/Embedding layout, pre-forward and pre-backward all-gather planning, fp32 master restoration, fp16 compute-dtype admission, and full-state reconstruction. It does not claim transport-backed FSDP execution.",
        ),
        row(
            "fsdp_on_after_backward",
            "fsdp",
            "partial_bounded_reference",
            vec![FSDP_AFTER_BACKWARD_SOURCE_PATH],
            vec![
                source_proof(
                    FSDP_AFTER_BACKWARD_SOURCE_PATH,
                    "build_cs336_a2_fsdp_after_backward_receipt",
                    "Owned bounded after-backward receipt for the fsdp_on_after_backward adapter surface.",
                ),
                json_proof(
                    CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_FIXTURE_PATH,
                    "Retained proof for sharded-gradient reduce-scatter, replicated-gradient all-reduce equivalence, fp32 master-gradient restoration, and fp32/fp16 bounded parity before optimizer.step.",
                ),
            ],
            vec![],
            "The current fsdp_on_after_backward adapter is mapped to a bounded host-reference receipt over the ToyFSDPModel parameter families. It proves reduce-scatter-equivalent synchronization for sharded Linear/Embedding gradients, replicated-gradient equality for non-FSDP parameters, fp32 master-gradient restoration before optimizer.step, and fp32/fp16 parity against a deterministic non-parallel baseline. It does not claim transport-backed FSDP execution.",
        ),
        row(
            "fsdp_gather_full_params",
            "fsdp",
            "partial_bounded_reference",
            vec![FSDP_FULL_PARAMS_SOURCE_PATH],
            vec![
                source_proof(
                    FSDP_FULL_PARAMS_SOURCE_PATH,
                    "build_cs336_a2_fsdp_full_params_receipt",
                    "Owned bounded full-state reconstruction receipt for the fsdp_gather_full_params adapter surface.",
                ),
                json_proof(
                    CS336_A2_FSDP_FULL_PARAMS_RECEIPT_FIXTURE_PATH,
                    "Retained proof that sharded parameters are all-gathered into full tensors and replicated parameters are returned as-is after each bounded training step for fp32 and fp16 cases.",
                ),
            ],
            vec![],
            "The current fsdp_gather_full_params adapter is mapped to a bounded host-reference receipt over the ToyFSDPModel parameter families. It proves every trainable parameter name is present after each bounded training step, distinguishes sharded Linear/Embedding reconstruction from replicated parameter return-as-is handling, and retains comparison digests against a deterministic non-parallel baseline for fp32/fp16 cases. It does not claim transport-backed FSDP execution.",
        ),
        row(
            "get_sharded_optimizer",
            "optimizer",
            "green_bounded_reference",
            vec![SHARDED_OPTIMIZER_SOURCE_PATH],
            vec![
                source_proof(
                    SHARDED_OPTIMIZER_SOURCE_PATH,
                    "build_cs336_a2_sharded_optimizer_receipt",
                    "Owned bounded constructor surface for the sharded-optimizer lane.",
                ),
                json_proof(
                    CS336_A2_SHARDED_OPTIMIZER_RECEIPT_FIXTURE_PATH,
                    "Retained proof that disjoint optimizer-state ownership reconstructs the non-sharded baseline after each bounded step.",
                ),
            ],
            vec![],
            "The sharded optimizer adapter is mapped to the owned bounded ZeRO-stage-1-style receipt lane with retained checkpoint-state reconstruction proof.",
        ),
    ]
}

fn row(
    stanford_adapter_name: &str,
    category: &str,
    status: &str,
    implementation_surfaces: Vec<&str>,
    proof_surfaces: Vec<Cs336A2ConformanceProofSurface>,
    follow_up_issue_urls: Vec<&str>,
    detail: &str,
) -> Cs336A2ConformanceRow {
    Cs336A2ConformanceRow {
        stanford_adapter_name: String::from(stanford_adapter_name),
        category: String::from(category),
        status: String::from(status),
        implementation_surfaces: implementation_surfaces
            .into_iter()
            .map(String::from)
            .collect(),
        proof_surfaces,
        follow_up_issue_urls: follow_up_issue_urls.into_iter().map(String::from).collect(),
        detail: String::from(detail),
    }
}

fn source_proof(
    repo_relative_path: &str,
    selector: &str,
    detail: &str,
) -> Cs336A2ConformanceProofSurface {
    Cs336A2ConformanceProofSurface {
        proof_kind: String::from("source_selector"),
        repo_relative_path: String::from(repo_relative_path),
        selector: Some(String::from(selector)),
        detail: String::from(detail),
    }
}

fn json_proof(repo_relative_path: &str, detail: &str) -> Cs336A2ConformanceProofSurface {
    Cs336A2ConformanceProofSurface {
        proof_kind: String::from("retained_fixture"),
        repo_relative_path: String::from(repo_relative_path),
        selector: None,
        detail: String::from(detail),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        build_cs336_a2_full_port_conformance_report, write_cs336_a2_full_port_conformance_fixture,
        CS336_A2_FULL_PORT_CONFORMANCE_REPORT_FIXTURE_PATH,
    };

    #[test]
    fn a2_full_port_report_tracks_current_adapter_surface() -> Result<(), Box<dyn std::error::Error>>
    {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let report = build_cs336_a2_full_port_conformance_report(repo_root)?;
        assert_eq!(report.row_count, 8);
        assert!(!report.fully_green);
        Ok(())
    }

    #[test]
    fn a2_full_port_report_covers_expected_adapter_set() -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let report = build_cs336_a2_full_port_conformance_report(repo_root)?;
        assert_eq!(report.rows.len(), 8);
        assert!(report
            .rows
            .iter()
            .any(|row| row.stanford_adapter_name == "get_fsdp"
                && row.status == "partial_bounded_reference"));
        assert!(report
            .rows
            .iter()
            .any(|row| row.stanford_adapter_name == "fsdp_on_after_backward"
                && row.status == "partial_bounded_reference"));
        assert!(report
            .rows
            .iter()
            .any(|row| row.stanford_adapter_name == "fsdp_gather_full_params"
                && row.status == "partial_bounded_reference"));
        assert!(!report
            .rows
            .iter()
            .any(|row| row.status == "missing_tracked"));
        assert!(report
            .rows
            .iter()
            .any(|row| row.stanford_adapter_name == "get_ddp"
                && row.status == "partial_bounded_reference"));
        Ok(())
    }

    #[test]
    fn a2_full_port_writer_emits_json_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let report = write_cs336_a2_full_port_conformance_fixture(repo_root)?;
        let fixture_path = repo_root.join(CS336_A2_FULL_PORT_CONFORMANCE_REPORT_FIXTURE_PATH);
        let bytes = std::fs::read(&fixture_path)?;
        let written: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            written["schema_version"].as_str(),
            Some(report.schema_version.as_str())
        );
        Ok(())
    }
}
