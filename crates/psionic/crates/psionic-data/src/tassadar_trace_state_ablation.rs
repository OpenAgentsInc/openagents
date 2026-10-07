use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_TRACE_STATE_ABLATION_CANON_ID: &str =
    "psionic.tassadar_trace_state_ablation_canon.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTraceStateRepresentation {
    FullAppendOnlyTrace,
    DeltaTrace,
    LocalityScratchpad,
    RecurrentState,
    WorkingMemoryTier,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceStateAblationWorkloadCase {
    pub workload_family: String,
    pub shared_budget_label: String,
    pub representation_families: Vec<TassadarTraceStateRepresentation>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceStateAblationCanon {
    pub canon_id: String,
    pub workload_cases: Vec<TassadarTraceStateAblationWorkloadCase>,
    pub canon_digest: String,
}

#[must_use]
pub fn tassadar_trace_state_ablation_canon() -> TassadarTraceStateAblationCanon {
    let mut canon = TassadarTraceStateAblationCanon {
        canon_id: String::from(TASSADAR_TRACE_STATE_ABLATION_CANON_ID),
        workload_cases: vec![
            TassadarTraceStateAblationWorkloadCase {
                workload_family: String::from("clrs_shortest_path"),
                shared_budget_label: String::from("same_program_same_budget_v1"),
                representation_families: all_representations(),
                note: String::from(
                    "CLRS shortest-path stays in the canon as the shared algorithmic baseline",
                ),
            },
            TassadarTraceStateAblationWorkloadCase {
                workload_family: String::from("arithmetic_multi_operand"),
                shared_budget_label: String::from("same_program_same_budget_v1"),
                representation_families: all_representations(),
                note: String::from(
                    "arithmetic keeps locality-sensitive exactness visible under one fixed budget",
                ),
            },
            TassadarTraceStateAblationWorkloadCase {
                workload_family: String::from("sudoku_backtracking_search"),
                shared_budget_label: String::from("same_program_same_budget_v1"),
                representation_families: all_representations(),
                note: String::from(
                    "Sudoku search exposes branch-heavy trace and scratchpad tradeoffs",
                ),
            },
            TassadarTraceStateAblationWorkloadCase {
                workload_family: String::from("module_scale_wasm_loop"),
                shared_budget_label: String::from("same_program_same_budget_v1"),
                representation_families: all_representations(),
                note: String::from(
                    "module-scale Wasm keeps replayability and memory pressure explicit",
                ),
            },
        ],
        canon_digest: String::new(),
    };
    canon.canon_digest = stable_digest(b"psionic_tassadar_trace_state_ablation_canon|", &canon);
    canon
}

fn all_representations() -> Vec<TassadarTraceStateRepresentation> {
    vec![
        TassadarTraceStateRepresentation::FullAppendOnlyTrace,
        TassadarTraceStateRepresentation::DeltaTrace,
        TassadarTraceStateRepresentation::LocalityScratchpad,
        TassadarTraceStateRepresentation::RecurrentState,
        TassadarTraceStateRepresentation::WorkingMemoryTier,
    ]
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{TassadarTraceStateRepresentation, tassadar_trace_state_ablation_canon};

    #[test]
    fn trace_state_ablation_canon_is_machine_legible() {
        let canon = tassadar_trace_state_ablation_canon();

        assert_eq!(canon.workload_cases.len(), 4);
        assert!(canon.workload_cases.iter().all(|case| {
            case.representation_families
                .contains(&TassadarTraceStateRepresentation::WorkingMemoryTier)
        }));
    }
}
