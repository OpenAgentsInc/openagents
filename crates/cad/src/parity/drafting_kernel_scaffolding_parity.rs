use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::drafting;
use crate::drafting::ViewDirection;
use crate::parity::scorecard::ParityScorecard;
use crate::{CadError, CadResult};

pub const PARITY_DRAFTING_KERNEL_SCAFFOLDING_ISSUE_ID: &str = "VCAD-PARITY-067";
pub const DRAFTING_KERNEL_SCAFFOLDING_REFERENCE_CORPUS_PATH: &str =
    "crates/cad/parity/fixtures/drafting_kernel_scaffolding_vcad_reference.json";
const DRAFTING_KERNEL_SCAFFOLDING_REFERENCE_CORPUS_JSON: &str =
    include_str!("../../parity/fixtures/drafting_kernel_scaffolding_vcad_reference.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftingKernelScaffoldingParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub reference_corpus_path: String,
    pub reference_corpus_sha256: String,
    pub reference_source: String,
    pub reference_commit_match: bool,
    pub top_level_modules: Vec<String>,
    pub dimension_modules: Vec<String>,
    pub public_exports: Vec<String>,
    pub view_direction_variants: Vec<String>,
    pub scaffold_module_match: bool,
    pub deterministic_replay_match: bool,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DraftingKernelScaffoldingReferenceCorpus {
    manifest_version: u64,
    issue_id: String,
    vcad_commit: String,
    source: String,
    top_level_modules: Vec<String>,
    dimension_modules: Vec<String>,
    public_exports: Vec<String>,
    view_direction_variants: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct DraftingScaffoldSnapshot {
    top_level_modules: Vec<String>,
    dimension_modules: Vec<String>,
    public_exports: Vec<String>,
    view_direction_variants: Vec<String>,
}

pub fn build_drafting_kernel_scaffolding_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<DraftingKernelScaffoldingParityManifest> {
    let corpus: DraftingKernelScaffoldingReferenceCorpus = serde_json::from_str(
        DRAFTING_KERNEL_SCAFFOLDING_REFERENCE_CORPUS_JSON,
    )
    .map_err(|error| CadError::ParseFailed {
        reason: format!("failed to parse drafting kernel scaffolding reference corpus: {error}"),
    })?;

    let reference_corpus_sha256 =
        sha256_hex(DRAFTING_KERNEL_SCAFFOLDING_REFERENCE_CORPUS_JSON.as_bytes());
    let reference_commit_match = corpus.vcad_commit == scorecard.vcad_commit;

    let snapshot = collect_snapshot()?;
    let replay_snapshot = collect_snapshot()?;
    let deterministic_replay_match = snapshot == replay_snapshot;

    let scaffold_module_match = snapshot.top_level_modules == sorted(corpus.top_level_modules)
        && snapshot.dimension_modules == sorted(corpus.dimension_modules)
        && snapshot.public_exports == sorted(corpus.public_exports)
        && snapshot.view_direction_variants == sorted(corpus.view_direction_variants);

    let deterministic_signature = parity_signature(
        &snapshot,
        reference_commit_match,
        scaffold_module_match,
        deterministic_replay_match,
        &reference_corpus_sha256,
    );

    Ok(DraftingKernelScaffoldingParityManifest {
        manifest_version: 1,
        issue_id: PARITY_DRAFTING_KERNEL_SCAFFOLDING_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        reference_corpus_path: DRAFTING_KERNEL_SCAFFOLDING_REFERENCE_CORPUS_PATH.to_string(),
        reference_corpus_sha256,
        reference_source: corpus.source,
        reference_commit_match,
        top_level_modules: snapshot.top_level_modules,
        dimension_modules: snapshot.dimension_modules,
        public_exports: snapshot.public_exports,
        view_direction_variants: snapshot.view_direction_variants,
        scaffold_module_match,
        deterministic_replay_match,
        deterministic_signature,
        parity_contracts: vec![
            "drafting scaffold includes vcad-aligned top-level modules detail/dimension/edge_extract/hidden_line/projection/section/types".to_string(),
            "drafting dimension scaffold includes linear/angular/radial/ordinate/gdt/style/render/layer/geometry_ref modules".to_string(),
            "drafting scaffolding exports remain deterministic and replay-stable".to_string(),
            "view direction tags include front/back/top/bottom/right/left/isometric".to_string(),
        ],
    })
}

fn collect_snapshot() -> CadResult<DraftingScaffoldSnapshot> {
    let top_level_modules = sorted(
        drafting::DRAFTING_TOP_LEVEL_MODULES
            .iter()
            .map(|module| (*module).to_string())
            .collect(),
    );
    let dimension_modules = sorted(
        drafting::dimension::DRAFTING_DIMENSION_MODULES
            .iter()
            .map(|module| (*module).to_string())
            .collect(),
    );
    let public_exports = sorted(
        drafting::DRAFTING_PUBLIC_EXPORTS
            .iter()
            .map(|export| (*export).to_string())
            .collect(),
    );
    let view_direction_variants = sorted(collect_view_direction_variants()?);

    Ok(DraftingScaffoldSnapshot {
        top_level_modules,
        dimension_modules,
        public_exports,
        view_direction_variants,
    })
}

fn collect_view_direction_variants() -> CadResult<Vec<String>> {
    let samples = [
        ViewDirection::Front,
        ViewDirection::Back,
        ViewDirection::Top,
        ViewDirection::Bottom,
        ViewDirection::Right,
        ViewDirection::Left,
        ViewDirection::ISOMETRIC_STANDARD,
    ];

    let mut variants = Vec::with_capacity(samples.len());
    for sample in samples {
        variants.push(view_direction_tag(sample)?);
    }
    Ok(variants)
}

fn view_direction_tag(direction: ViewDirection) -> CadResult<String> {
    let value = serde_json::to_value(direction).map_err(|error| CadError::Serialization {
        reason: format!("failed to serialize drafting view direction for tag extraction: {error}"),
    })?;

    if let Some(tag) = value.as_str() {
        return Ok(tag.to_string());
    }
    if let Some(object) = value.as_object()
        && let Some((tag, _)) = object.iter().next()
    {
        return Ok(tag.to_string());
    }

    Err(CadError::ParseFailed {
        reason: format!("drafting view direction serialized to unsupported tag shape: {value:?}"),
    })
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn parity_signature(
    snapshot: &DraftingScaffoldSnapshot,
    reference_commit_match: bool,
    scaffold_module_match: bool,
    deterministic_replay_match: bool,
    reference_corpus_sha256: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            snapshot,
            reference_commit_match,
            scaffold_module_match,
            deterministic_replay_match,
            reference_corpus_sha256,
        ))
        .expect("serialize drafting kernel scaffolding parity payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::DraftingScaffoldSnapshot;
    use super::parity_signature;

    #[test]
    fn parity_signature_is_stable_for_identical_inputs() {
        let snapshot = DraftingScaffoldSnapshot {
            top_level_modules: vec!["detail".to_string()],
            dimension_modules: vec!["linear".to_string()],
            public_exports: vec!["ViewDirection".to_string()],
            view_direction_variants: vec!["front".to_string()],
        };

        let first = parity_signature(&snapshot, true, true, true, "sha");
        let second = parity_signature(&snapshot, true, true, true, "sha");
        assert_eq!(first, second);
    }
}
