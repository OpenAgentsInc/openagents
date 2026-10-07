use std::{collections::BTreeSet, fs, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::CS336_A1_REFERENCE_TINY_TRAINING_BUNDLE_FIXTURE_PATH;

pub const CS336_A1_FULL_PORT_CONFORMANCE_REPORT_SCHEMA_VERSION: &str =
    "psion.cs336_a1.full_port_conformance_report.v1";
pub const CS336_A1_FULL_PORT_CONFORMANCE_REPORT_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a1_full_port_conformance_report_v1.json";
pub const CS336_A1_FULL_PORT_MATRIX_DOC_PATH: &str = "docs/PSION_CS336_A1_FULL_PORT_MATRIX.md";

const BPE_SOURCE_PATH: &str = "crates/psionic-data/src/cs336_a1_bpe.rs";
const TOKENIZER_SOURCE_PATH: &str = "crates/psionic-models/src/cs336_a1_tokenizer.rs";
const REFERENCE_STACK_SOURCE_PATH: &str = "crates/psionic-models/src/cs336_a1_reference_stack.rs";
const TRAINING_SOURCE_PATH: &str = "crates/psionic-train/src/cs336_a1_reference_training.rs";

const EXPECTED_STANFORD_ADAPTERS: [&str; 21] = [
    "run_linear",
    "run_embedding",
    "run_swiglu",
    "run_scaled_dot_product_attention",
    "run_multihead_self_attention",
    "run_multihead_self_attention_with_rope",
    "run_rope",
    "run_transformer_block",
    "run_transformer_lm",
    "run_rmsnorm",
    "run_silu",
    "run_get_batch",
    "run_softmax",
    "run_cross_entropy",
    "run_gradient_clipping",
    "get_adamw_cls",
    "run_get_lr_cosine_schedule",
    "run_save_checkpoint",
    "run_load_checkpoint",
    "get_tokenizer",
    "run_train_bpe",
];

#[derive(Debug, Error)]
pub enum Cs336A1FullPortConformanceError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("missing or invalid proof surface: {0}")]
    MissingProofSurface(String),
    #[error("invalid CS336 A1 full-port conformance report: {0}")]
    InvalidReport(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A1ConformanceProofSurface {
    pub proof_kind: String,
    pub repo_relative_path: String,
    pub selector: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A1ConformanceRow {
    pub stanford_adapter_name: String,
    pub category: String,
    pub status: String,
    pub implementation_surfaces: Vec<String>,
    pub proof_surfaces: Vec<Cs336A1ConformanceProofSurface>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A1FullPortConformanceReport {
    pub schema_version: String,
    pub completion_matrix_path: String,
    pub retained_training_bundle_path: String,
    pub row_count: usize,
    pub green_row_count: usize,
    pub fully_green: bool,
    pub rows: Vec<Cs336A1ConformanceRow>,
    pub claim_boundary: String,
}

pub fn build_cs336_a1_full_port_conformance_report(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A1FullPortConformanceReport, Cs336A1FullPortConformanceError> {
    let repo_root = repo_root.as_ref();
    let rows = expected_rows();
    for row in &rows {
        validate_row(repo_root, row)?;
    }
    let report = Cs336A1FullPortConformanceReport {
        schema_version: String::from(CS336_A1_FULL_PORT_CONFORMANCE_REPORT_SCHEMA_VERSION),
        completion_matrix_path: String::from(CS336_A1_FULL_PORT_MATRIX_DOC_PATH),
        retained_training_bundle_path: String::from(
            CS336_A1_REFERENCE_TINY_TRAINING_BUNDLE_FIXTURE_PATH,
        ),
        row_count: rows.len(),
        green_row_count: rows.iter().filter(|row| row.status == "green").count(),
        fully_green: rows.iter().all(|row| row.status == "green"),
        rows,
        claim_boundary: String::from(
            "This report closes Stanford CS336 Assignment 1 only as a bounded psionic reference lane. It proves every Stanford adapter family is mapped to owned Rust code plus in-repo proof surfaces. It does not promote the bounded lane into the actual pretraining operator lane, and it does not claim scalable broader-pretraining backward support beyond the tiny finite-difference reference trainer.",
        ),
    };
    validate_report(&report)?;
    Ok(report)
}

pub fn write_cs336_a1_full_port_conformance_report(
    output_root: impl AsRef<Path>,
    report: &Cs336A1FullPortConformanceReport,
) -> Result<(), Cs336A1FullPortConformanceError> {
    validate_report(report)?;
    let report_path = output_root
        .as_ref()
        .join(CS336_A1_FULL_PORT_CONFORMANCE_REPORT_FIXTURE_PATH);
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(report_path, serde_json::to_vec_pretty(report)?)?;
    Ok(())
}

pub fn write_cs336_a1_full_port_conformance_fixture(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A1FullPortConformanceReport, Cs336A1FullPortConformanceError> {
    let report = build_cs336_a1_full_port_conformance_report(&repo_root)?;
    write_cs336_a1_full_port_conformance_report(repo_root, &report)?;
    Ok(report)
}

fn validate_report(
    report: &Cs336A1FullPortConformanceReport,
) -> Result<(), Cs336A1FullPortConformanceError> {
    if report.schema_version != CS336_A1_FULL_PORT_CONFORMANCE_REPORT_SCHEMA_VERSION {
        return Err(Cs336A1FullPortConformanceError::InvalidReport(format!(
            "expected schema version `{CS336_A1_FULL_PORT_CONFORMANCE_REPORT_SCHEMA_VERSION}`, got `{}`",
            report.schema_version
        )));
    }
    if report.row_count != report.rows.len() {
        return Err(Cs336A1FullPortConformanceError::InvalidReport(format!(
            "row_count {} does not match {} rows",
            report.row_count,
            report.rows.len()
        )));
    }
    let green_row_count = report
        .rows
        .iter()
        .filter(|row| row.status == "green")
        .count();
    if report.green_row_count != green_row_count {
        return Err(Cs336A1FullPortConformanceError::InvalidReport(format!(
            "green_row_count {} does not match computed count {green_row_count}",
            report.green_row_count
        )));
    }
    if !report.fully_green || report.green_row_count != report.row_count {
        return Err(Cs336A1FullPortConformanceError::InvalidReport(
            "full-port report is not fully green".into(),
        ));
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
        return Err(Cs336A1FullPortConformanceError::InvalidReport(format!(
            "adapter set mismatch: expected {:?}, got {:?}",
            expected, adapter_set
        )));
    }
    Ok(())
}

fn validate_row(
    repo_root: &Path,
    row: &Cs336A1ConformanceRow,
) -> Result<(), Cs336A1FullPortConformanceError> {
    if row.status != "green" {
        return Err(Cs336A1FullPortConformanceError::InvalidReport(format!(
            "row `{}` is not green",
            row.stanford_adapter_name
        )));
    }
    if row.implementation_surfaces.is_empty() {
        return Err(Cs336A1FullPortConformanceError::InvalidReport(format!(
            "row `{}` is missing implementation surfaces",
            row.stanford_adapter_name
        )));
    }
    if row.proof_surfaces.is_empty() {
        return Err(Cs336A1FullPortConformanceError::InvalidReport(format!(
            "row `{}` is missing proof surfaces",
            row.stanford_adapter_name
        )));
    }
    for proof in &row.proof_surfaces {
        validate_proof_surface(repo_root, proof)?;
    }
    Ok(())
}

fn validate_proof_surface(
    repo_root: &Path,
    proof: &Cs336A1ConformanceProofSurface,
) -> Result<(), Cs336A1FullPortConformanceError> {
    let proof_path = repo_root.join(&proof.repo_relative_path);
    if !proof_path.exists() {
        return Err(Cs336A1FullPortConformanceError::MissingProofSurface(
            format!("`{}` does not exist", proof.repo_relative_path),
        ));
    }
    if let Some(selector) = &proof.selector {
        let contents = fs::read_to_string(&proof_path)?;
        if !contents.contains(selector) {
            return Err(Cs336A1FullPortConformanceError::MissingProofSurface(
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

fn expected_rows() -> Vec<Cs336A1ConformanceRow> {
    vec![
        row(
            "run_linear",
            "model",
            vec!["crates/psionic-models/src/cs336_a1_reference_stack.rs"],
            vec![source_proof(
                REFERENCE_STACK_SOURCE_PATH,
                "linear_matches_manual_projection",
                "Direct unit test for the A1 linear helper.",
            )],
            "Linear projection is covered by the bounded reference stack test surface.",
        ),
        row(
            "run_embedding",
            "model",
            vec!["crates/psionic-models/src/cs336_a1_reference_stack.rs"],
            vec![source_proof(
                REFERENCE_STACK_SOURCE_PATH,
                "embedding_matches_table_lookup",
                "Direct unit test for the A1 embedding helper.",
            )],
            "Embedding table lookup is covered by the bounded reference stack test surface.",
        ),
        row(
            "run_swiglu",
            "model",
            vec!["crates/psionic-models/src/cs336_a1_reference_stack.rs"],
            vec![source_proof(
                REFERENCE_STACK_SOURCE_PATH,
                "swiglu_composes_gate_and_value_paths",
                "Direct unit test for the A1 SwiGLU helper.",
            )],
            "SwiGLU is covered directly by the reference stack tests.",
        ),
        row(
            "run_scaled_dot_product_attention",
            "model",
            vec!["crates/psionic-models/src/cs336_a1_reference_stack.rs"],
            vec![source_proof(
                REFERENCE_STACK_SOURCE_PATH,
                "scaled_dot_product_attention_matches_manual_example",
                "Direct unit test for scaled dot-product attention.",
            )],
            "Scaled dot-product attention is covered directly by the reference stack tests.",
        ),
        row(
            "run_multihead_self_attention",
            "model",
            vec!["crates/psionic-models/src/cs336_a1_reference_stack.rs"],
            vec![source_proof(
                REFERENCE_STACK_SOURCE_PATH,
                "multihead_self_attention_supports_identity_projection_path",
                "Direct unit test for batched multi-head self-attention.",
            )],
            "Batched multi-head self-attention is covered directly by the reference stack tests.",
        ),
        row(
            "run_multihead_self_attention_with_rope",
            "model",
            vec!["crates/psionic-models/src/cs336_a1_reference_stack.rs"],
            vec![source_proof(
                REFERENCE_STACK_SOURCE_PATH,
                "multihead_self_attention_with_rope_executes_end_to_end",
                "Direct unit test for RoPE-enabled multi-head self-attention.",
            )],
            "RoPE-enabled multi-head self-attention is covered directly by the reference stack tests.",
        ),
        row(
            "run_rope",
            "model",
            vec!["crates/psionic-models/src/cs336_a1_reference_stack.rs"],
            vec![source_proof(
                REFERENCE_STACK_SOURCE_PATH,
                "rope_rotates_even_odd_pairs",
                "Direct unit test for the RoPE helper.",
            )],
            "RoPE is covered directly by the reference stack tests.",
        ),
        row(
            "run_transformer_block",
            "model",
            vec!["crates/psionic-models/src/cs336_a1_reference_stack.rs"],
            vec![source_proof(
                REFERENCE_STACK_SOURCE_PATH,
                "transformer_block_with_zero_submodules_is_identity",
                "Direct unit test for the bounded transformer block.",
            )],
            "The pre-norm transformer block is covered directly by the reference stack tests.",
        ),
        row(
            "run_transformer_lm",
            "model",
            vec!["crates/psionic-models/src/cs336_a1_reference_stack.rs"],
            vec![source_proof(
                REFERENCE_STACK_SOURCE_PATH,
                "transformer_lm_executes_end_to_end_and_exposes_expected_state_dict_keys",
                "Direct unit test for the bounded transformer LM.",
            )],
            "The bounded transformer LM is covered directly by the reference stack tests.",
        ),
        row(
            "run_rmsnorm",
            "model",
            vec!["crates/psionic-models/src/cs336_a1_reference_stack.rs"],
            vec![source_proof(
                REFERENCE_STACK_SOURCE_PATH,
                "rms_norm_scales_by_root_mean_square",
                "Direct unit test for RMSNorm.",
            )],
            "RMSNorm is covered directly by the reference stack tests.",
        ),
        row(
            "run_silu",
            "model",
            vec!["crates/psionic-models/src/cs336_a1_reference_stack.rs"],
            vec![source_proof(
                REFERENCE_STACK_SOURCE_PATH,
                "silu_matches_reference_formula",
                "Direct unit test for SiLU.",
            )],
            "SiLU is covered directly by the reference stack tests.",
        ),
        row(
            "run_get_batch",
            "training",
            vec!["crates/psionic-train/src/cs336_a1_reference_training.rs"],
            vec![source_proof(
                TRAINING_SOURCE_PATH,
                "get_batch_cycles_deterministically",
                "Direct unit test for deterministic batch construction.",
            )],
            "The bounded trainer exposes deterministic next-token batch construction.",
        ),
        row(
            "run_softmax",
            "training",
            vec!["crates/psionic-train/src/cs336_a1_reference_training.rs"],
            vec![source_proof(
                TRAINING_SOURCE_PATH,
                "softmax_normalizes_requested_dimension",
                "Direct unit test for the softmax helper.",
            )],
            "Softmax is covered directly by the bounded training helper tests.",
        ),
        row(
            "run_cross_entropy",
            "training",
            vec!["crates/psionic-train/src/cs336_a1_reference_training.rs"],
            vec![source_proof(
                TRAINING_SOURCE_PATH,
                "cross_entropy_matches_expected_average",
                "Direct unit test for cross-entropy loss.",
            )],
            "Cross-entropy is covered directly by the bounded training helper tests.",
        ),
        row(
            "run_gradient_clipping",
            "training",
            vec!["crates/psionic-train/src/cs336_a1_reference_training.rs"],
            vec![source_proof(
                TRAINING_SOURCE_PATH,
                "gradient_clipping_enforces_global_norm_bound",
                "Direct unit test for global gradient clipping.",
            )],
            "Global gradient clipping is covered directly by the bounded training helper tests.",
        ),
        row(
            "get_adamw_cls",
            "training",
            vec!["crates/psionic-train/src/cs336_a1_reference_training.rs"],
            vec![source_proof(
                TRAINING_SOURCE_PATH,
                "adamw_config_updates_parameters_on_first_step",
                "Direct unit test for the bounded AdamW helper.",
            )],
            "AdamW update behavior is covered directly by the bounded training helper tests.",
        ),
        row(
            "run_get_lr_cosine_schedule",
            "training",
            vec!["crates/psionic-train/src/cs336_a1_reference_training.rs"],
            vec![source_proof(
                TRAINING_SOURCE_PATH,
                "cosine_schedule_warms_up_and_decays",
                "Direct unit test for the cosine learning-rate schedule.",
            )],
            "The cosine schedule is covered directly by the bounded training helper tests.",
        ),
        row(
            "run_save_checkpoint",
            "training",
            vec!["crates/psionic-train/src/cs336_a1_reference_training.rs"],
            vec![
                source_proof(
                    TRAINING_SOURCE_PATH,
                    "checkpoint_round_trip_preserves_iteration_and_state_digests",
                    "Direct unit test for checkpoint save/load round-trip behavior.",
                ),
                artifact_proof(
                    "fixtures/training/cs336_a1_reference_tiny_checkpoint_step2.json",
                    "Committed retained checkpoint proving the step-2 save surface.",
                ),
                artifact_proof(
                    CS336_A1_REFERENCE_TINY_TRAINING_BUNDLE_FIXTURE_PATH,
                    "Committed retained training bundle proving save/load/resume behavior.",
                ),
            ],
            "Checkpoint save is covered by the direct round-trip test plus retained bundle artifacts.",
        ),
        row(
            "run_load_checkpoint",
            "training",
            vec!["crates/psionic-train/src/cs336_a1_reference_training.rs"],
            vec![
                source_proof(
                    TRAINING_SOURCE_PATH,
                    "checkpoint_round_trip_preserves_iteration_and_state_digests",
                    "Direct unit test for checkpoint save/load round-trip behavior.",
                ),
                artifact_proof(
                    "fixtures/training/cs336_a1_reference_tiny_checkpoint_step4.json",
                    "Committed retained checkpoint proving the resumed load surface.",
                ),
                artifact_proof(
                    CS336_A1_REFERENCE_TINY_TRAINING_BUNDLE_FIXTURE_PATH,
                    "Committed retained training bundle proving load/resume exactness.",
                ),
            ],
            "Checkpoint load is covered by the direct round-trip test plus retained bundle artifacts.",
        ),
        row(
            "get_tokenizer",
            "tokenizer",
            vec!["crates/psionic-models/src/cs336_a1_tokenizer.rs"],
            vec![
                source_proof(
                    TOKENIZER_SOURCE_PATH,
                    "tokenizer_round_trips_text_and_special_tokens",
                    "Direct unit test for tokenizer construction and round-trip behavior.",
                ),
                source_proof(
                    TOKENIZER_SOURCE_PATH,
                    "tokenizer_prefers_the_longest_special_token",
                    "Direct unit test for longest-match special-token preservation.",
                ),
                source_proof(
                    TOKENIZER_SOURCE_PATH,
                    "tokenizer_streaming_surface_matches_direct_encoding",
                    "Direct unit test for the bounded streaming tokenizer surface.",
                ),
            ],
            "Tokenizer construction from vocab, merges, and special tokens is covered directly by the runtime tokenizer tests.",
        ),
        row(
            "run_train_bpe",
            "tokenizer",
            vec!["crates/psionic-data/src/cs336_a1_bpe.rs"],
            vec![
                source_proof(
                    BPE_SOURCE_PATH,
                    "trainer_uses_lexicographically_greatest_pair_for_ties",
                    "Direct unit test for the Stanford tie-break rule.",
                ),
                source_proof(
                    BPE_SOURCE_PATH,
                    "trainer_emits_reconstructible_artifacts",
                    "Direct unit test for retained BPE artifact reconstruction.",
                ),
            ],
            "Byte-level BPE training is covered directly by the owned trainer tests.",
        ),
    ]
}

fn row(
    stanford_adapter_name: &str,
    category: &str,
    implementation_surfaces: Vec<&str>,
    proof_surfaces: Vec<Cs336A1ConformanceProofSurface>,
    detail: &str,
) -> Cs336A1ConformanceRow {
    Cs336A1ConformanceRow {
        stanford_adapter_name: String::from(stanford_adapter_name),
        category: String::from(category),
        status: String::from("green"),
        implementation_surfaces: implementation_surfaces
            .into_iter()
            .map(String::from)
            .collect(),
        proof_surfaces,
        detail: String::from(detail),
    }
}

fn source_proof(
    repo_relative_path: &str,
    selector: &str,
    detail: &str,
) -> Cs336A1ConformanceProofSurface {
    Cs336A1ConformanceProofSurface {
        proof_kind: String::from("unit_test"),
        repo_relative_path: String::from(repo_relative_path),
        selector: Some(String::from(selector)),
        detail: String::from(detail),
    }
}

fn artifact_proof(repo_relative_path: &str, detail: &str) -> Cs336A1ConformanceProofSurface {
    Cs336A1ConformanceProofSurface {
        proof_kind: String::from("retained_artifact"),
        repo_relative_path: String::from(repo_relative_path),
        selector: None,
        detail: String::from(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CS336_A1_FULL_PORT_CONFORMANCE_REPORT_FIXTURE_PATH, EXPECTED_STANFORD_ADAPTERS,
        build_cs336_a1_full_port_conformance_report, write_cs336_a1_full_port_conformance_report,
    };
    use tempfile::tempdir;

    #[test]
    fn conformance_report_covers_every_stanford_adapter_family()
    -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = repo_root()?;
        let report = build_cs336_a1_full_port_conformance_report(&repo_root)?;
        assert!(report.fully_green);
        assert_eq!(report.row_count, EXPECTED_STANFORD_ADAPTERS.len());
        assert_eq!(report.green_row_count, EXPECTED_STANFORD_ADAPTERS.len());
        Ok(())
    }

    #[test]
    fn conformance_report_writer_emits_green_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = repo_root()?;
        let report = build_cs336_a1_full_port_conformance_report(&repo_root)?;
        let output_root = tempdir()?;
        write_cs336_a1_full_port_conformance_report(output_root.path(), &report)?;
        let fixture_path = output_root
            .path()
            .join(CS336_A1_FULL_PORT_CONFORMANCE_REPORT_FIXTURE_PATH);
        let decoded: super::Cs336A1FullPortConformanceReport =
            serde_json::from_slice(&std::fs::read(fixture_path)?)?;
        assert!(decoded.fully_green);
        Ok(())
    }

    fn repo_root() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .ancestors()
            .nth(2)
            .map(std::path::PathBuf::from)
            .ok_or_else(|| "failed to resolve repo root".into())
    }
}
