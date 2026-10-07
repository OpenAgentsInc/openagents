use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTraceStateRepresentationFamily {
    FullAppendOnlyTrace,
    DeltaTrace,
    LocalityScratchpad,
    RecurrentState,
    WorkingMemoryTier,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceStateAblationPublication {
    pub publication_id: String,
    pub canon_id: String,
    pub representation_families: Vec<TassadarTraceStateRepresentationFamily>,
    pub claim_boundary: String,
}

#[must_use]
pub fn tassadar_trace_state_ablation_publication() -> TassadarTraceStateAblationPublication {
    TassadarTraceStateAblationPublication {
        publication_id: String::from("tassadar.trace_state_ablation.publication.v1"),
        canon_id: String::from("psionic.tassadar_trace_state_ablation_canon.v1"),
        representation_families: vec![
            TassadarTraceStateRepresentationFamily::FullAppendOnlyTrace,
            TassadarTraceStateRepresentationFamily::DeltaTrace,
            TassadarTraceStateRepresentationFamily::LocalityScratchpad,
            TassadarTraceStateRepresentationFamily::RecurrentState,
            TassadarTraceStateRepresentationFamily::WorkingMemoryTier,
        ],
        claim_boundary: String::from(
            "this publication is a research-only state-representation ablation canon over shared workloads. It does not widen served capability or broad learned-compute claims",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::tassadar_trace_state_ablation_publication;

    #[test]
    fn trace_state_ablation_publication_is_machine_legible() {
        let publication = tassadar_trace_state_ablation_publication();

        assert_eq!(publication.representation_families.len(), 5);
        assert!(publication.claim_boundary.contains("research-only"));
    }
}
