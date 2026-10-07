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
    build_tassadar_article_equivalence_acceptance_gate_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
};

pub const TASSADAR_EXISTING_SUBSTRATE_INVENTORY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_existing_substrate_inventory_report.json";

const TIED_REQUIREMENT_ID: &str = "TAS-159";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExistingSubstrateClassification {
    ReusableAsIs,
    ReusableWithExtension,
    ResearchOnly,
    NotSufficientForArticleClosure,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExistingSubstrateClassificationCount {
    pub classification: TassadarExistingSubstrateClassification,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExistingSubstrateSurfaceRow {
    pub surface_id: String,
    pub crate_id: String,
    pub title: String,
    pub item_refs: Vec<String>,
    pub classification: TassadarExistingSubstrateClassification,
    pub blocks_article_closure: bool,
    pub current_truth: String,
    pub extension_or_gap: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExistingSubstrateAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub blocker_matrix_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub article_equivalence_green: bool,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExistingSubstrateInventoryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_tie: TassadarExistingSubstrateAcceptanceGateTie,
    pub surfaces: Vec<TassadarExistingSubstrateSurfaceRow>,
    pub required_classification_count: usize,
    pub classification_counts: Vec<TassadarExistingSubstrateClassificationCount>,
    pub surface_count: usize,
    pub blocker_surface_count: usize,
    pub non_blocker_surface_count: usize,
    pub all_required_classifications_present: bool,
    pub all_surface_ids_unique: bool,
    pub all_surfaces_have_item_refs: bool,
    pub all_surfaces_have_explicit_blocker_labels: bool,
    pub inventory_contract_green: bool,
    pub article_equivalence_green: bool,
    pub current_truth_boundary: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarExistingSubstrateInventoryReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
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

pub fn build_tassadar_existing_substrate_inventory_report(
) -> Result<TassadarExistingSubstrateInventoryReport, TassadarExistingSubstrateInventoryReportError>
{
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    Ok(build_report_from_inputs(
        acceptance_gate_report,
        substrate_surface_rows(),
    ))
}

fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    surfaces: Vec<TassadarExistingSubstrateSurfaceRow>,
) -> TassadarExistingSubstrateInventoryReport {
    let acceptance_gate_tie = TassadarExistingSubstrateAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        blocker_matrix_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate_report
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate_report.acceptance_status,
        article_equivalence_green: acceptance_gate_report.article_equivalence_green,
        blocked_issue_ids: acceptance_gate_report.blocked_issue_ids.clone(),
    };
    let classification_counts = classification_counts(surfaces.as_slice());
    let blocker_surface_count = surfaces
        .iter()
        .filter(|row| row.blocks_article_closure)
        .count();
    let non_blocker_surface_count = surfaces.len() - blocker_surface_count;
    let observed_classifications = surfaces
        .iter()
        .map(|row| row.classification)
        .collect::<BTreeSet<_>>();
    let all_required_classifications_present =
        observed_classifications == required_classifications();
    let surface_ids = surfaces
        .iter()
        .map(|row| row.surface_id.clone())
        .collect::<Vec<_>>();
    let all_surface_ids_unique =
        surface_ids.len() == surface_ids.iter().collect::<BTreeSet<_>>().len();
    let all_surfaces_have_item_refs = surfaces.iter().all(|row| !row.item_refs.is_empty());
    let all_surfaces_have_explicit_blocker_labels = surfaces
        .iter()
        .all(|row| !row.current_truth.trim().is_empty() && !row.extension_or_gap.trim().is_empty());
    let inventory_contract_green = acceptance_gate_tie.tied_requirement_satisfied
        && all_required_classifications_present
        && all_surface_ids_unique
        && all_surfaces_have_item_refs
        && all_surfaces_have_explicit_blocker_labels;
    let article_equivalence_green =
        inventory_contract_green && acceptance_gate_tie.article_equivalence_green;
    let mut report = TassadarExistingSubstrateInventoryReport {
        schema_version: 1,
        report_id: String::from("tassadar.existing_substrate_inventory.report.v1"),
        acceptance_gate_tie,
        required_classification_count: required_classifications().len(),
        classification_counts,
        surface_count: surfaces.len(),
        blocker_surface_count,
        non_blocker_surface_count,
        all_required_classifications_present,
        all_surface_ids_unique,
        all_surfaces_have_item_refs,
        all_surfaces_have_explicit_blocker_labels,
        inventory_contract_green,
        article_equivalence_green,
        surfaces,
        current_truth_boundary: String::from(
            "psionic already owns real reusable substrate for tensor contracts, bounded array execution, module state, basic layers, transformer-boundary configs, model descriptors, weight lineage metadata, runtime proof identity, and trace receipts. The current gap is not substrate absence; it is the missing owned article-equivalence stack and the current presence of bounded research or fixture-backed lanes that are not yet sufficient for canonical article closure.",
        ),
        claim_boundary: String::from(
            "this report inventories reusable substrate only. It freezes what can be reused, what needs extension, what stays research-only, and what is still not sufficient for article closure. It does not claim that the owned article-equivalence Transformer stack, the canonical article model artifact, or the final article-equivalence proof route already exist.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Existing substrate inventory now records surface_count={}, blocker_surface_count={}, non_blocker_surface_count={}, inventory_contract_green={}, tied_requirement_satisfied={}, and article_equivalence_green={}.",
        report.surface_count,
        report.blocker_surface_count,
        report.non_blocker_surface_count,
        report.inventory_contract_green,
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_existing_substrate_inventory_report|",
        &report,
    );
    report
}

fn classification_counts(
    surfaces: &[TassadarExistingSubstrateSurfaceRow],
) -> Vec<TassadarExistingSubstrateClassificationCount> {
    required_classifications()
        .into_iter()
        .map(
            |classification| TassadarExistingSubstrateClassificationCount {
                classification,
                count: surfaces
                    .iter()
                    .filter(|row| row.classification == classification)
                    .count(),
            },
        )
        .collect()
}

fn required_classifications() -> BTreeSet<TassadarExistingSubstrateClassification> {
    BTreeSet::from([
        TassadarExistingSubstrateClassification::ReusableAsIs,
        TassadarExistingSubstrateClassification::ReusableWithExtension,
        TassadarExistingSubstrateClassification::ResearchOnly,
        TassadarExistingSubstrateClassification::NotSufficientForArticleClosure,
    ])
}

fn substrate_surface_rows() -> Vec<TassadarExistingSubstrateSurfaceRow> {
    vec![
        surface_row(
            "psionic_core_tensor_contracts",
            "psionic-core",
            "Tensor contracts and typed refusal vocabulary",
            &["crates/psionic-core/src/lib.rs"],
            TassadarExistingSubstrateClassification::ReusableAsIs,
            false,
            "typed tensor ids, dtypes, shapes, devices, quantization layouts, and refusal codes already exist as shared engine contracts",
            "no article-specific closure gap lives here; higher layers consume these contracts",
        ),
        surface_row(
            "psionic_array_lazy_array_facade",
            "psionic-array",
            "Lazy array facade and bounded public materialization surface",
            &["crates/psionic-array/src/lib.rs"],
            TassadarExistingSubstrateClassification::ReusableWithExtension,
            true,
            "the repo already has a bounded graph-backed array surface with explicit evaluation, backend identities, cache posture, and debug capture hooks",
            "the owned article-equivalence stack still needs broader transformer-oriented tensor/view/math coverage than the current narrow public array surface provides",
        ),
        surface_row(
            "psionic_nn_module_state_and_basic_layers",
            "psionic-nn",
            "Module state tree and basic reusable layers",
            &["crates/psionic-nn/src/lib.rs", "crates/psionic-nn/src/layers.rs"],
            TassadarExistingSubstrateClassification::ReusableWithExtension,
            true,
            "module trees, parameter traversal, quantized wrappers, and reusable Linear/Embedding/LayerNorm/RmsNorm/Dropout layers already exist as the lower-level layer substrate",
            "the lower-level layer substrate now exists at the right boundary; the remaining gap is the paper-faithful article model and proof route above it, not missing primitive NN layers",
        ),
        surface_row(
            "psionic_transformer_architecture_boundary",
            "psionic-transformer",
            "Canonical reusable Transformer-boundary configs",
            &[
                "crates/psionic-transformer/src/lib.rs",
                "crates/psionic-transformer/src/attention.rs",
                "crates/psionic-transformer/src/blocks.rs",
                "crates/psionic-transformer/src/encoder_decoder.rs",
                "docs/ARCHITECTURE.md",
            ],
            TassadarExistingSubstrateClassification::ReusableAsIs,
            false,
            "the dedicated `psionic-transformer` crate now owns reusable decoder and AttnRes architecture primitives plus the owned scaled dot-product attention, embeddings, encoder-decoder stack, feed-forward, residual, and norm path at the intended layering boundary",
            "the remaining gap is now the article-specific vocabulary, artifact, training, and proof route on top of this boundary, not missing reusable Transformer architecture",
        ),
        surface_row(
            "psionic_models_descriptor_and_weight_lineage",
            "psionic-models",
            "Model descriptors and weight-lineage metadata",
            &["crates/psionic-models/src/lib.rs"],
            TassadarExistingSubstrateClassification::ReusableAsIs,
            false,
            "shared model descriptors, weight bundle metadata, and artifact-governance wrappers already exist for reusable model families",
            "these surfaces can be reused directly once the canonical article model artifact exists",
        ),
        surface_row(
            "psionic_models_article_transformer_wrapper",
            "psionic-models",
            "Canonical article Transformer wrapper",
            &["crates/psionic-models/src/tassadar_article_transformer.rs"],
            TassadarExistingSubstrateClassification::ReusableWithExtension,
            true,
            "the repo now has one canonical paper-faithful article wrapper that binds the owned encoder-decoder stack, paper reference, and embedding-sharing modes at the `psionic-models` boundary",
            "the wrapper still needs article trace vocabulary, artifact-backed weights, lineage, and proof-route closure before the final article-equivalence route can turn green",
        ),
        surface_row(
            "psionic_models_attnres_reference_family",
            "psionic-models",
            "AttnRes reference model family",
            &["crates/psionic-models/src/attnres.rs"],
            TassadarExistingSubstrateClassification::ResearchOnly,
            false,
            "the repo already has a bounded CPU-reference AttnRes family that is useful as a model-architecture pilot and training/control experiment surface",
            "it is not the canonical article route and remains a research-only forcing function rather than a closure blocker",
        ),
        surface_row(
            "psionic_models_executor_transformer_scaffold",
            "psionic-models",
            "Existing executor-transformer scaffold",
            &["crates/psionic-models/src/tassadar_executor_transformer.rs"],
            TassadarExistingSubstrateClassification::ReusableWithExtension,
            true,
            "the repo already has executor-transformer configs, weight bundles, forward-pass scaffolding, decode state, KV points, and refusal posture",
            "the scaffold is no longer the canonical article route; it remains a separate research and comparison lane beside the paper-faithful article wrapper",
        ),
        surface_row(
            "psionic_models_fixture_backed_article_executor_lane",
            "psionic-models",
            "Fixture-backed article executor lane",
            &["crates/psionic-models/src/tassadar.rs"],
            TassadarExistingSubstrateClassification::NotSufficientForArticleClosure,
            true,
            "the current fixture-backed article executor lane still carries the bounded Rust-only article closeout and compatibility truth",
            "canonical article-equivalence closure cannot stop at this reference-fixture lane; it must graduate to an owned Transformer-backed artifact",
        ),
        surface_row(
            "psionic_runtime_proof_identity_and_trace_receipts",
            "psionic-runtime",
            "Runtime proof identity and trace receipts",
            &["crates/psionic-runtime/src/proof.rs", "crates/psionic-runtime/src/tassadar.rs"],
            TassadarExistingSubstrateClassification::ReusableAsIs,
            false,
            "execution proof bundles, runtime identity, trace ABI contracts, trace artifacts, and replay-safe evidence surfaces already exist",
            "these runtime truth surfaces can be reused directly once the owned article route emits the right artifacts",
        ),
        surface_row(
            "psionic_runtime_article_closeout_receipts",
            "psionic-runtime",
            "Article-closeout runtime and direct-proof receipts",
            &[
                "crates/psionic-runtime/src/tassadar_article_runtime_closeout.rs",
                "crates/psionic-runtime/src/tassadar_direct_model_weight_execution_proof.rs",
            ],
            TassadarExistingSubstrateClassification::ReusableWithExtension,
            true,
            "the runtime already owns article ABI, article runtime closeout, and direct model-weight proof surfaces",
            "those receipts still need rebinding from the current bounded lanes into the canonical owned Transformer-backed article route",
        ),
    ]
}

fn surface_row(
    surface_id: &str,
    crate_id: &str,
    title: &str,
    item_refs: &[&str],
    classification: TassadarExistingSubstrateClassification,
    blocks_article_closure: bool,
    current_truth: &str,
    extension_or_gap: &str,
) -> TassadarExistingSubstrateSurfaceRow {
    TassadarExistingSubstrateSurfaceRow {
        surface_id: String::from(surface_id),
        crate_id: String::from(crate_id),
        title: String::from(title),
        item_refs: item_refs.iter().map(|item| String::from(*item)).collect(),
        classification,
        blocks_article_closure,
        current_truth: String::from(current_truth),
        extension_or_gap: String::from(extension_or_gap),
    }
}

#[must_use]
pub fn tassadar_existing_substrate_inventory_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EXISTING_SUBSTRATE_INVENTORY_REPORT_REF)
}

pub fn write_tassadar_existing_substrate_inventory_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarExistingSubstrateInventoryReport, TassadarExistingSubstrateInventoryReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarExistingSubstrateInventoryReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_existing_substrate_inventory_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarExistingSubstrateInventoryReportError::Write {
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
) -> Result<T, TassadarExistingSubstrateInventoryReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarExistingSubstrateInventoryReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarExistingSubstrateInventoryReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_report_from_inputs, build_tassadar_existing_substrate_inventory_report, read_json,
        substrate_surface_rows, tassadar_existing_substrate_inventory_report_path,
        write_tassadar_existing_substrate_inventory_report,
        TassadarExistingSubstrateInventoryReport,
    };
    use crate::build_tassadar_article_equivalence_acceptance_gate_report;

    #[test]
    fn existing_substrate_inventory_is_tied_to_closure_gate_but_not_final_green() {
        let report = build_tassadar_existing_substrate_inventory_report().expect("inventory");

        assert!(report.inventory_contract_green);
        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.article_equivalence_green);
        assert_eq!(report.surface_count, 11);
        assert_eq!(report.blocker_surface_count, 6);
        assert_eq!(report.non_blocker_surface_count, 5);
        assert_eq!(report.classification_counts[0].count, 4);
        assert_eq!(report.classification_counts[1].count, 5);
        assert_eq!(report.classification_counts[2].count, 1);
        assert_eq!(report.classification_counts[3].count, 1);
    }

    #[test]
    fn missing_item_refs_keep_inventory_contract_red() {
        let acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        let mut surfaces = substrate_surface_rows();
        surfaces[0].item_refs.clear();

        let report = build_report_from_inputs(acceptance_gate_report, surfaces);

        assert!(!report.inventory_contract_green);
        assert!(!report.all_surfaces_have_item_refs);
    }

    #[test]
    fn missing_gate_tie_keeps_inventory_contract_red() {
        let mut acceptance_gate_report =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        acceptance_gate_report
            .green_requirement_ids
            .retain(|requirement_id| requirement_id != "TAS-159");

        let report = build_report_from_inputs(acceptance_gate_report, substrate_surface_rows());

        assert!(!report.inventory_contract_green);
        assert!(!report.acceptance_gate_tie.tied_requirement_satisfied);
    }

    #[test]
    fn existing_substrate_inventory_matches_committed_truth() {
        let generated = build_tassadar_existing_substrate_inventory_report().expect("inventory");
        let committed: TassadarExistingSubstrateInventoryReport =
            read_json(tassadar_existing_substrate_inventory_report_path())
                .expect("committed inventory");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_existing_substrate_inventory_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_existing_substrate_inventory_report.json");
        let written = write_tassadar_existing_substrate_inventory_report(&output_path)
            .expect("write inventory");
        let persisted: TassadarExistingSubstrateInventoryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_existing_substrate_inventory_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_existing_substrate_inventory_report.json")
        );
    }
}
