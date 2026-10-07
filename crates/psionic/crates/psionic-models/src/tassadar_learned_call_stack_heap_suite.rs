use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ModelDescriptor;

pub const TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_CLAIM_CLASS: &str =
    "learned_bounded_research_architecture";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnedCallStackHeapSuitePublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub claim_class: String,
    pub model_variants: Vec<ModelDescriptor>,
    pub workload_families: Vec<String>,
    pub contract_ref: String,
    pub target_surfaces: Vec<String>,
    pub validation_refs: Vec<String>,
    pub support_boundaries: Vec<String>,
    pub publication_digest: String,
}

#[must_use]
pub fn tassadar_learned_call_stack_heap_suite_publication()
-> TassadarLearnedCallStackHeapSuitePublication {
    let mut publication = TassadarLearnedCallStackHeapSuitePublication {
        schema_version: 1,
        publication_id: String::from("tassadar.learned_call_stack_heap_suite.publication.v1"),
        claim_class: String::from(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_CLAIM_CLASS),
        model_variants: vec![
            ModelDescriptor::new(
                "tassadar-call-stack-heap-baseline-v0",
                "tassadar_call_stack_heap_baseline",
                "v0",
            ),
            ModelDescriptor::new(
                "tassadar-call-stack-heap-structured-v0",
                "tassadar_call_stack_heap_structured",
                "v0",
            ),
        ],
        workload_families: vec![
            String::from("recursive_evaluator"),
            String::from("parser_frame_machine"),
            String::from("bump_allocator_heap"),
            String::from("free_list_allocator_heap"),
            String::from("resumable_process_heap"),
            String::from("held_out_continuation_machine"),
            String::from("held_out_allocator_scheduler"),
        ],
        contract_ref: String::from("dataset://openagents/tassadar/learned_call_stack_heap_suite"),
        target_surfaces: vec![
            String::from("crates/psionic-data"),
            String::from("crates/psionic-models"),
            String::from("crates/psionic-train"),
            String::from("crates/psionic-eval"),
            String::from("crates/psionic-research"),
        ],
        validation_refs: vec![
            String::from(
                "fixtures/tassadar/runs/tassadar_learned_call_stack_heap_suite_v1/learned_call_stack_heap_suite_bundle.json",
            ),
            String::from(
                "fixtures/tassadar/reports/tassadar_learned_call_stack_heap_suite_report.json",
            ),
            String::from(
                "fixtures/tassadar/reports/tassadar_learned_call_stack_heap_suite_summary.json",
            ),
        ],
        support_boundaries: vec![
            String::from(
                "the suite is research-only and measures learned call-stack and heap generalization rather than promoting broad process ownership",
            ),
            String::from(
                "held-out-family rows remain explicit generalization checks and do not imply arbitrary Wasm, broad internal compute, or served capability widening",
            ),
            String::from(
                "refusal calibration remains part of the honest claim surface whenever later-window state drifts beyond the seeded envelope",
            ),
        ],
        publication_digest: String::new(),
    };
    publication.publication_digest = stable_digest(
        b"psionic_tassadar_learned_call_stack_heap_suite_publication|",
        &publication,
    );
    publication
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
        TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_CLAIM_CLASS,
        tassadar_learned_call_stack_heap_suite_publication,
    };

    #[test]
    fn learned_call_stack_heap_suite_publication_is_machine_legible() {
        let publication = tassadar_learned_call_stack_heap_suite_publication();

        assert_eq!(
            publication.claim_class,
            TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_CLAIM_CLASS
        );
        assert_eq!(publication.model_variants.len(), 2);
        assert!(
            publication
                .workload_families
                .contains(&String::from("held_out_continuation_machine"))
        );
        assert!(!publication.publication_digest.is_empty());
    }
}
