use psionic_runtime::{
    TassadarModuleExecutionCapabilityReport, tassadar_module_execution_capability_report,
};
use serde::{Deserialize, Serialize};

use crate::TassadarExecutorFixture;

/// Repo-facing publication for the bounded module-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleExecutionCapabilityPublication {
    /// Stable publication identifier.
    pub publication_id: String,
    /// Stable model identifier owning the publication.
    pub model_id: String,
    /// Coarse claim class for the publication.
    pub claim_class: String,
    /// Runtime capability report projected into model/publication space.
    pub runtime_capability: TassadarModuleExecutionCapabilityReport,
    /// Stable seeded case ids that anchor the publication today.
    pub seeded_case_ids: Vec<String>,
    /// Plain-language claim boundary for the publication.
    pub claim_boundary: String,
}

/// Builds the repo-facing publication for the bounded module-execution lane.
#[must_use]
pub fn tassadar_module_execution_capability_publication(
    fixture: &TassadarExecutorFixture,
) -> TassadarModuleExecutionCapabilityPublication {
    TassadarModuleExecutionCapabilityPublication {
        publication_id: format!(
            "tassadar.module_execution_capability.{}.v1",
            fixture.descriptor().model.model_id
        ),
        model_id: fixture.descriptor().model.model_id.clone(),
        claim_class: String::from("capability_truth"),
        runtime_capability: tassadar_module_execution_capability_report(),
        seeded_case_ids: vec![
            String::from("global_state_parity"),
            String::from("call_indirect_dispatch"),
            String::from("instantiation_start_and_elements"),
            String::from("dynamic_memory_growth_copy_fill"),
            String::from("deterministic_import_stub"),
            String::from("unsupported_host_import_refusal"),
        ],
        claim_boundary: String::from(
            "this publication covers bounded module execution with i32 globals, one bounded linear memory plus active data segments, memory.size, memory.grow, memory.copy, memory.fill, funcref tables, active element-segment instantiation, zero-parameter start functions, zero-parameter direct and indirect calls, and deterministic host-import stubs only; multi-memory, arbitrary host calls, and arbitrary Wasm remain explicitly unsupported",
        ),
    }
}

impl TassadarExecutorFixture {
    /// Returns the repo-facing publication for the bounded module-execution lane.
    #[must_use]
    pub fn module_execution_capability_publication(
        &self,
    ) -> TassadarModuleExecutionCapabilityPublication {
        tassadar_module_execution_capability_publication(self)
    }
}

#[cfg(test)]
mod tests {
    use super::tassadar_module_execution_capability_publication;
    use crate::TassadarExecutorFixture;

    #[test]
    fn module_execution_capability_publication_is_machine_legible() {
        let fixture = TassadarExecutorFixture::article_i32_compute_v1();
        let publication = tassadar_module_execution_capability_publication(&fixture);
        assert_eq!(
            publication.model_id,
            TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID
        );
        assert!(
            publication
                .runtime_capability
                .supports_active_element_segments
        );
        assert!(
            publication
                .runtime_capability
                .supports_start_function_instantiation
        );
        assert!(publication.runtime_capability.supports_direct_calls);
        assert!(publication.runtime_capability.supports_call_indirect);
        assert!(publication.runtime_capability.supports_linear_memory);
        assert!(publication.runtime_capability.supports_active_data_segments);
        assert!(publication.runtime_capability.supports_memory_size);
        assert!(publication.runtime_capability.supports_memory_grow);
        assert!(publication.runtime_capability.supports_memory_copy);
        assert!(publication.runtime_capability.supports_memory_fill);
        assert_eq!(publication.seeded_case_ids.len(), 6);
        assert_eq!(publication.claim_class, "capability_truth");
    }
}
