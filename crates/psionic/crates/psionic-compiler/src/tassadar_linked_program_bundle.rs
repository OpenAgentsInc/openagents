use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_runtime::{
    seeded_tassadar_linked_program_bundles, TassadarLinkedProgramStatePosture,
    TassadarRuntimeSupportModuleClass,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinkedProgramBundleCompilerSummary {
    pub bundle_id: String,
    pub bundle_source_ref: String,
    pub helper_module_count: u32,
    pub graph_edge_count: u32,
    pub start_module_count: u32,
    pub runtime_support_classes: Vec<TassadarRuntimeSupportModuleClass>,
    pub shared_bundle_state_admitted: bool,
    pub helper_lineage_complete: bool,
    pub summary_digest: String,
}

impl TassadarLinkedProgramBundleCompilerSummary {
    fn from_descriptor(
        descriptor: &psionic_runtime::TassadarLinkedProgramBundleDescriptor,
    ) -> Self {
        let mut runtime_support_classes = descriptor
            .modules
            .iter()
            .filter_map(|module| module.runtime_support_class)
            .collect::<Vec<_>>();
        runtime_support_classes.sort_by_key(|class| class.as_str());
        runtime_support_classes.dedup();
        let mut summary = Self {
            bundle_id: descriptor.bundle_id.clone(),
            bundle_source_ref: descriptor.bundle_source_ref.clone(),
            helper_module_count: descriptor.modules.len().saturating_sub(1) as u32,
            graph_edge_count: descriptor.graph_edges.len() as u32,
            start_module_count: descriptor.start_module_refs.len() as u32,
            runtime_support_classes,
            shared_bundle_state_admitted: descriptor.modules.iter().any(|module| {
                module.state_posture == TassadarLinkedProgramStatePosture::SharedBundleState
            }),
            helper_lineage_complete: descriptor
                .modules
                .iter()
                .filter(|module| module.runtime_support_class.is_some())
                .all(|module| !module.benchmark_lineage_refs.is_empty()),
            summary_digest: String::new(),
        };
        summary.summary_digest = stable_digest(
            b"psionic_tassadar_linked_program_bundle_compiler_summary|",
            &summary,
        );
        summary
    }
}

#[must_use]
pub fn tassadar_linked_program_bundle_compiler_summaries(
) -> Vec<TassadarLinkedProgramBundleCompilerSummary> {
    seeded_tassadar_linked_program_bundles()
        .iter()
        .map(TassadarLinkedProgramBundleCompilerSummary::from_descriptor)
        .collect()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::tassadar_linked_program_bundle_compiler_summaries;
    use psionic_runtime::TassadarRuntimeSupportModuleClass;

    #[test]
    fn linked_program_bundle_compiler_summaries_are_machine_legible() {
        let summaries = tassadar_linked_program_bundle_compiler_summaries();
        assert_eq!(summaries.len(), 4);
        assert!(summaries.iter().any(|summary| {
            summary.bundle_id == "tassadar.linked_program_bundle.checkpoint_backtrack.v1"
                && summary.shared_bundle_state_admitted
                && summary.graph_edge_count >= 2
                && summary.start_module_count == 3
                && summary
                    .runtime_support_classes
                    .contains(&TassadarRuntimeSupportModuleClass::CheckpointBacktrack)
        }));
    }
}
