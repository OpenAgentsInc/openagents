use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_PROGRAM_FAMILY_FRONTIER_CLAIM_CLASS: &str =
    "research_only_architecture_capability_truth";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramFamilyFrontierPublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub claim_class: String,
    pub architecture_families: Vec<String>,
    pub workload_families: Vec<String>,
    pub contract_ref: String,
    pub target_surfaces: Vec<String>,
    pub validation_refs: Vec<String>,
    pub support_boundaries: Vec<String>,
    pub publication_digest: String,
}

#[must_use]
pub fn tassadar_program_family_frontier_publication() -> TassadarProgramFamilyFrontierPublication {
    let mut publication = TassadarProgramFamilyFrontierPublication {
        schema_version: 1,
        publication_id: String::from("tassadar.program_family_frontier.publication.v1"),
        claim_class: String::from(TASSADAR_PROGRAM_FAMILY_FRONTIER_CLAIM_CLASS),
        architecture_families: vec![
            String::from("compiled_exact_reference"),
            String::from("learned_structured_memory"),
            String::from("verifier_attached_hybrid"),
        ],
        workload_families: vec![
            String::from("kernel_state_machine"),
            String::from("search_process_machine"),
            String::from("linked_program_bundle"),
            String::from("effectful_resume_graph"),
            String::from("multi_module_package_workflow"),
            String::from("held_out_virtual_machine"),
            String::from("held_out_message_orchestrator"),
        ],
        contract_ref: String::from("dataset://openagents/tassadar/program_family_frontier"),
        target_surfaces: vec![
            String::from("crates/psionic-data"),
            String::from("crates/psionic-models"),
            String::from("crates/psionic-train"),
            String::from("crates/psionic-eval"),
            String::from("crates/psionic-research"),
        ],
        validation_refs: vec![
            String::from(
                "fixtures/tassadar/runs/tassadar_program_family_frontier_v1/program_family_frontier_bundle.json",
            ),
            String::from("fixtures/tassadar/reports/tassadar_program_family_frontier_report.json"),
            String::from(
                "fixtures/tassadar/reports/tassadar_program_family_frontier_summary.json",
            ),
        ],
        support_boundaries: vec![
            String::from(
                "the frontier is research-only and records cross-family generalization over named program families rather than promoting broad internal compute or served capability",
            ),
            String::from(
                "held-out-family ladders remain explicit generalization checks and do not imply arbitrary Wasm, arbitrary modules, or general interactive process ownership",
            ),
            String::from(
                "verifier-attached hybrid rows remain bounded by named profiles, explicit refusal calibration, and committed benchmark artifacts instead of inheriting broad hybrid-compute closure",
            ),
        ],
        publication_digest: String::new(),
    };
    publication.publication_digest = stable_digest(
        b"psionic_tassadar_program_family_frontier_publication|",
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
        tassadar_program_family_frontier_publication, TASSADAR_PROGRAM_FAMILY_FRONTIER_CLAIM_CLASS,
    };

    #[test]
    fn program_family_frontier_publication_is_machine_legible() {
        let publication = tassadar_program_family_frontier_publication();

        assert_eq!(
            publication.claim_class,
            TASSADAR_PROGRAM_FAMILY_FRONTIER_CLAIM_CLASS
        );
        assert_eq!(publication.architecture_families.len(), 3);
        assert!(publication
            .workload_families
            .contains(&String::from("held_out_message_orchestrator")));
        assert!(!publication.publication_digest.is_empty());
    }
}
