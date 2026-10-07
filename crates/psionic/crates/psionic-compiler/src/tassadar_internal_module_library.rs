use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const LIBRARY_SCHEMA_VERSION: u16 = 1;

/// Stable internal module family admitted by the bounded library lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalModuleKind {
    /// Frontier-relaxation and graph-propagation core.
    FrontierRelaxCore,
    /// Candidate selection and ranking core.
    CandidateSelectCore,
    /// Checkpoint and bounded backtrack core.
    CheckpointBacktrackCore,
}

/// Explicit selection posture for one consumer link manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarInternalModuleSelectionPolicy {
    /// Use the currently active module version.
    PinnedActive,
    /// Try a candidate version, but fall back explicitly when drift appears.
    CandidateWithRollback,
}

/// One versioned internal computational module artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalModuleArtifact {
    /// Stable module identifier without version.
    pub module_id: String,
    /// Semantic version carried by the module artifact.
    pub semantic_version: String,
    /// Stable module family.
    pub module_kind: TassadarInternalModuleKind,
    /// Exported callable symbols.
    pub exported_symbols: Vec<String>,
    /// Imported callable symbols or runtime hooks.
    pub imported_symbols: Vec<String>,
    /// Stable compatibility digest.
    pub compatibility_digest: String,
    /// Stable benchmark refs anchoring the module.
    pub benchmark_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
}

/// One machine-legible consumer link manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalModuleLinkManifest {
    /// Stable manifest identifier.
    pub manifest_id: String,
    /// Stable consumer program family.
    pub consumer_program_family: String,
    /// Ordered module refs required by the consumer.
    pub required_module_refs: Vec<String>,
    /// Ordered required compatibility digests.
    pub required_compatibility_digests: Vec<String>,
    /// Explicit module selection policy.
    pub selection_policy: TassadarInternalModuleSelectionPolicy,
    /// Rollback target refs when the policy allows rollback.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rollback_module_refs: Vec<String>,
    /// Stable benchmark refs anchoring the manifest.
    pub benchmark_refs: Vec<String>,
}

/// Explicit replacement and rollback record for one module family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalModuleReplacementPlan {
    /// Stable module identifier without version.
    pub module_id: String,
    /// Stable active version.
    pub active_version: String,
    /// Candidate version under evaluation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_version: Option<String>,
    /// Rollback version used when the candidate is not kept active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_version: Option<String>,
    /// Plain-language replacement note.
    pub detail: String,
}

/// Public compiler-owned internal module library artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalModuleLibrary {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable library identifier.
    pub library_id: String,
    /// Explicit claim class for the lane.
    pub claim_class: String,
    /// Versioned module artifacts.
    pub modules: Vec<TassadarInternalModuleArtifact>,
    /// Consumer link manifests over the module set.
    pub link_manifests: Vec<TassadarInternalModuleLinkManifest>,
    /// Explicit replacement and rollback plans.
    pub replacement_plans: Vec<TassadarInternalModuleReplacementPlan>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the library artifact.
    pub library_digest: String,
}

impl TassadarInternalModuleLibrary {
    fn new(
        modules: Vec<TassadarInternalModuleArtifact>,
        link_manifests: Vec<TassadarInternalModuleLinkManifest>,
        replacement_plans: Vec<TassadarInternalModuleReplacementPlan>,
    ) -> Self {
        let mut library = Self {
            schema_version: LIBRARY_SCHEMA_VERSION,
            library_id: String::from("tassadar.internal_module_library.v1"),
            claim_class: String::from(
                "compiled bounded exactness / promotion discipline / served capability",
            ),
            modules,
            link_manifests,
            replacement_plans,
            claim_boundary: String::from(
                "the internal module library is a bounded compiled artifact surface over versioned modules, link manifests, compatibility digests, and explicit replacement or rollback plans. It proves reusable bounded module composition, not arbitrary code installation, unrestricted self-extension, or general module autonomy",
            ),
            summary: String::new(),
            library_digest: String::new(),
        };
        library.summary = format!(
            "Internal module library now freezes {} versioned module artifacts, {} consumer link manifests, and {} explicit replacement or rollback plans.",
            library.modules.len(),
            library.link_manifests.len(),
            library.replacement_plans.len(),
        );
        library.library_digest =
            stable_digest(b"psionic_tassadar_internal_module_library|", &library);
        library
    }
}

/// Returns the canonical compiler-owned internal module library artifact.
#[must_use]
pub fn compile_tassadar_internal_module_library() -> TassadarInternalModuleLibrary {
    let frontier_relax_v1 = module_artifact(
        "frontier_relax_core",
        "1.0.0",
        TassadarInternalModuleKind::FrontierRelaxCore,
        &["frontier_relax_step"],
        &[],
        &[
            "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
            "fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json",
        ],
    );
    let candidate_select_v1 = module_artifact(
        "candidate_select_core",
        "1.1.0",
        TassadarInternalModuleKind::CandidateSelectCore,
        &["candidate_select_step"],
        &[],
        &[
            "fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json",
            "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
        ],
    );
    let candidate_select_v2 = module_artifact(
        "candidate_select_core",
        "1.2.0",
        TassadarInternalModuleKind::CandidateSelectCore,
        &["candidate_select_step"],
        &[],
        &["fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"],
    );
    let checkpoint_backtrack_v1 = module_artifact(
        "checkpoint_backtrack_core",
        "1.0.0",
        TassadarInternalModuleKind::CheckpointBacktrackCore,
        &["checkpoint_push", "checkpoint_pop"],
        &[],
        &["fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"],
    );
    TassadarInternalModuleLibrary::new(
        vec![
            frontier_relax_v1.clone(),
            candidate_select_v1.clone(),
            candidate_select_v2.clone(),
            checkpoint_backtrack_v1.clone(),
        ],
        vec![
            link_manifest(
                "clrs_shortest_path.link.v1",
                "clrs_shortest_path",
                &[module_ref(&frontier_relax_v1)],
                &[frontier_relax_v1.compatibility_digest.as_str()],
                TassadarInternalModuleSelectionPolicy::PinnedActive,
                &[],
                &["fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"],
            ),
            link_manifest(
                "clrs_wasm_shortest_path.link.v1",
                "clrs_wasm_shortest_path",
                &[module_ref(&frontier_relax_v1)],
                &[frontier_relax_v1.compatibility_digest.as_str()],
                TassadarInternalModuleSelectionPolicy::PinnedActive,
                &[],
                &["fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"],
            ),
            link_manifest(
                "hungarian_matching.link.v1",
                "hungarian_matching",
                &[module_ref(&candidate_select_v2)],
                &[candidate_select_v2.compatibility_digest.as_str()],
                TassadarInternalModuleSelectionPolicy::CandidateWithRollback,
                &[module_ref(&candidate_select_v1)],
                &["fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"],
            ),
            link_manifest(
                "verifier_search.link.v1",
                "verifier_search",
                &[
                    module_ref(&candidate_select_v1),
                    module_ref(&checkpoint_backtrack_v1),
                ],
                &[
                    candidate_select_v1.compatibility_digest.as_str(),
                    checkpoint_backtrack_v1.compatibility_digest.as_str(),
                ],
                TassadarInternalModuleSelectionPolicy::PinnedActive,
                &[],
                &["fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"],
            ),
        ],
        vec![TassadarInternalModuleReplacementPlan {
            module_id: String::from("candidate_select_core"),
            active_version: String::from("1.1.0"),
            candidate_version: Some(String::from("1.2.0")),
            rollback_version: Some(String::from("1.1.0")),
            detail: String::from(
                "candidate_select_core@1.2.0 stays explicit as a candidate-only replacement because assignment-stability witnesses still require rollback to 1.1.0",
            ),
        }],
    )
}

fn module_artifact(
    module_id: &str,
    semantic_version: &str,
    module_kind: TassadarInternalModuleKind,
    exported_symbols: &[&str],
    imported_symbols: &[&str],
    benchmark_refs: &[&str],
) -> TassadarInternalModuleArtifact {
    let compatibility_digest = stable_digest(
        b"psionic_tassadar_internal_module_compatibility|",
        &(
            module_id,
            semantic_version,
            module_kind,
            exported_symbols,
            imported_symbols,
            benchmark_refs,
        ),
    );
    TassadarInternalModuleArtifact {
        module_id: String::from(module_id),
        semantic_version: String::from(semantic_version),
        module_kind,
        exported_symbols: exported_symbols
            .iter()
            .map(|symbol| String::from(*symbol))
            .collect(),
        imported_symbols: imported_symbols
            .iter()
            .map(|symbol| String::from(*symbol))
            .collect(),
        compatibility_digest,
        benchmark_refs: benchmark_refs
            .iter()
            .map(|reference| String::from(*reference))
            .collect(),
        claim_boundary: String::from(
            "module artifacts stay benchmark-bound, compatibility-digested, and bounded to the declared internal library lane",
        ),
    }
}

fn module_ref(module: &TassadarInternalModuleArtifact) -> String {
    format!("{}@{}", module.module_id, module.semantic_version)
}

fn link_manifest(
    manifest_id: &str,
    consumer_program_family: &str,
    required_module_refs: &[String],
    required_compatibility_digests: &[&str],
    selection_policy: TassadarInternalModuleSelectionPolicy,
    rollback_module_refs: &[String],
    benchmark_refs: &[&str],
) -> TassadarInternalModuleLinkManifest {
    TassadarInternalModuleLinkManifest {
        manifest_id: String::from(manifest_id),
        consumer_program_family: String::from(consumer_program_family),
        required_module_refs: required_module_refs.to_vec(),
        required_compatibility_digests: required_compatibility_digests
            .iter()
            .map(|digest| String::from(*digest))
            .collect(),
        selection_policy,
        rollback_module_refs: rollback_module_refs.to_vec(),
        benchmark_refs: benchmark_refs
            .iter()
            .map(|reference| String::from(*reference))
            .collect(),
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
        TassadarInternalModuleKind, TassadarInternalModuleSelectionPolicy,
        compile_tassadar_internal_module_library,
    };

    #[test]
    fn internal_module_library_keeps_reuse_and_rollback_explicit() {
        let library = compile_tassadar_internal_module_library();

        assert_eq!(library.modules.len(), 4);
        assert!(library.link_manifests.iter().any(|manifest| {
            manifest.consumer_program_family == "clrs_shortest_path"
                && manifest
                    .required_module_refs
                    .contains(&String::from("frontier_relax_core@1.0.0"))
        }));
        assert!(library.link_manifests.iter().any(|manifest| {
            manifest.consumer_program_family == "hungarian_matching"
                && manifest.selection_policy
                    == TassadarInternalModuleSelectionPolicy::CandidateWithRollback
                && manifest
                    .rollback_module_refs
                    .contains(&String::from("candidate_select_core@1.1.0"))
        }));
        assert!(library.modules.iter().any(|module| {
            module.module_kind == TassadarInternalModuleKind::CheckpointBacktrackCore
                && !module.compatibility_digest.is_empty()
        }));
    }
}
