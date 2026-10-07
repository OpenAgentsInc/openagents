use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const STATE_DESIGN_SCHEMA_VERSION: u16 = 1;

/// Comparable state-design family tracked by the Tassadar representation study.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarStateDesignFamily {
    /// Append every semantic step into one replay-complete trace.
    FullAppendOnlyTrace,
    /// Publish only state deltas and reconstruct the full trace offline.
    DeltaTrace,
    /// Preserve token truth while reshaping locality through scratchpad formatting.
    LocalityScratchpad,
    /// Carry bounded recurrent state instead of replaying the entire trace surface.
    RecurrentState,
    /// Publish a bounded working-memory state surface with explicit slot semantics.
    WorkingMemoryTier,
}

impl TassadarStateDesignFamily {
    /// Returns the stable public label for one state-design family.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullAppendOnlyTrace => "full_append_only_trace",
            Self::DeltaTrace => "delta_trace",
            Self::LocalityScratchpad => "locality_scratchpad",
            Self::RecurrentState => "recurrent_state",
            Self::WorkingMemoryTier => "working_memory_tier",
        }
    }
}

/// Replay posture admitted by one state-design family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarStateDesignReplayPosture {
    /// The family preserves the replay-complete trace directly.
    ExactReplay,
    /// The family preserves execution truth but requires deterministic reconstruction.
    ReconstructableReplay,
    /// The family preserves only a bounded published state surface.
    BoundedStatePublication,
    /// The family refuses workloads that outrun the declared state surface.
    Refused,
}

/// Public IR-level contract for one state-design family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStateDesignContract {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable contract identifier.
    pub contract_id: String,
    /// Compared state-design family.
    pub design_family: TassadarStateDesignFamily,
    /// Strongest replay posture the family admits honestly.
    pub replay_posture: TassadarStateDesignReplayPosture,
    /// Human-readable description of the surfaced state.
    pub state_surface_summary: String,
    /// Workload families the study compares for this design family.
    pub admitted_workload_families: Vec<String>,
    /// Measured study axes.
    pub measured_axes: Vec<String>,
    /// Explicit refusal boundaries for the family.
    pub refusal_boundaries: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the contract.
    pub contract_digest: String,
}

impl TassadarStateDesignContract {
    fn new(
        design_family: TassadarStateDesignFamily,
        replay_posture: TassadarStateDesignReplayPosture,
        state_surface_summary: &str,
        admitted_workload_families: &[&str],
        refusal_boundaries: &[&str],
        claim_boundary: &str,
    ) -> Self {
        let mut contract = Self {
            schema_version: STATE_DESIGN_SCHEMA_VERSION,
            contract_id: format!("tassadar.state_design.{}.v1", design_family.as_str()),
            design_family,
            replay_posture,
            state_surface_summary: String::from(state_surface_summary),
            admitted_workload_families: admitted_workload_families
                .iter()
                .map(|family| String::from(*family))
                .collect(),
            measured_axes: vec![
                String::from("locality_score_bps"),
                String::from("edit_cost_bps"),
                String::from("replayability"),
                String::from("exact_output_preservation"),
                String::from("state_bytes"),
            ],
            refusal_boundaries: refusal_boundaries
                .iter()
                .map(|boundary| String::from(*boundary))
                .collect(),
            claim_boundary: String::from(claim_boundary),
            contract_digest: String::new(),
        };
        contract.contract_digest =
            stable_digest(b"psionic_tassadar_state_design_contract|", &contract);
        contract
    }
}

/// Returns the canonical IR-level state-design study contracts.
#[must_use]
pub fn tassadar_state_design_contracts() -> Vec<TassadarStateDesignContract> {
    let workload_families = [
        "module_call_trace",
        "symbolic_locality",
        "associative_recall",
        "long_horizon_control",
        "byte_memory_loop",
    ];
    vec![
        TassadarStateDesignContract::new(
            TassadarStateDesignFamily::FullAppendOnlyTrace,
            TassadarStateDesignReplayPosture::ExactReplay,
            "append-only audit floor that keeps every declared step and state transition in one replay-complete trace",
            &workload_families,
            &[
                "edit cost grows with full trace length",
                "locality remains poor on long-horizon and memory-heavy workloads",
            ],
            "the full trace remains the replay floor for the study, but the study does not assume this representation is the best computational state design for every workload family",
        ),
        TassadarStateDesignContract::new(
            TassadarStateDesignFamily::DeltaTrace,
            TassadarStateDesignReplayPosture::ReconstructableReplay,
            "deterministic delta stream that reconstructs the full trace offline while shrinking local edit surfaces",
            &[
                workload_families[0],
                workload_families[1],
                workload_families[4],
            ],
            &[
                "refuses workloads whose semantics depend on undeclared hidden state",
                "replay remains honest only when deterministic reconstruction is available",
            ],
            "delta traces narrow the serialized surface while keeping reconstruction explicit; they do not justify hidden semantic state or opaque compression",
        ),
        TassadarStateDesignContract::new(
            TassadarStateDesignFamily::LocalityScratchpad,
            TassadarStateDesignReplayPosture::ExactReplay,
            "trace-preserving scratchpad layout that reshapes token locality without changing the recovered output stream",
            &[
                workload_families[0],
                workload_families[1],
                workload_families[4],
            ],
            &[
                "refuses workloads whose semantics require undeclared mutable state",
                "scratchpad segments must preserve recovered source token truth exactly",
            ],
            "scratchpad state design is still trace truth, not semantic state mutation; it can improve locality but it cannot silently replace missing memory semantics",
        ),
        TassadarStateDesignContract::new(
            TassadarStateDesignFamily::RecurrentState,
            TassadarStateDesignReplayPosture::BoundedStatePublication,
            "bounded recurrent carry state with explicit published terminal-state receipts instead of a replay-complete trace",
            &[
                workload_families[2],
                workload_families[3],
                workload_families[4],
            ],
            &[
                "refuses module-scale workloads that require exact intermediate-frame replay",
                "published state must stay smaller than the trace it replaces and cannot claim full replay closure",
            ],
            "recurrent state is a bounded research state surface with explicit publication receipts; it does not widen public exact replay claims",
        ),
        TassadarStateDesignContract::new(
            TassadarStateDesignFamily::WorkingMemoryTier,
            TassadarStateDesignReplayPosture::BoundedStatePublication,
            "bounded slot and associative-table state with explicit read, write, and publication semantics",
            &[
                workload_families[2],
                workload_families[3],
                workload_families[4],
            ],
            &[
                "refuses workloads that require arbitrary call-frame or host-import state",
                "state publication stays bounded to declared slots, bytes, and mutation semantics",
            ],
            "the working-memory tier is a bounded semantic-state surface for selected workloads; it does not imply arbitrary memory closure or broad served promotion",
        ),
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
    use super::{
        TassadarStateDesignFamily, TassadarStateDesignReplayPosture,
        tassadar_state_design_contracts,
    };

    #[test]
    fn state_design_contracts_are_machine_legible() {
        let contracts = tassadar_state_design_contracts();

        assert_eq!(contracts.len(), 5);
        let scratchpad = contracts
            .iter()
            .find(|contract| {
                contract.design_family == TassadarStateDesignFamily::LocalityScratchpad
            })
            .expect("scratchpad contract");
        assert_eq!(
            scratchpad.replay_posture,
            TassadarStateDesignReplayPosture::ExactReplay
        );
        assert!(
            scratchpad
                .admitted_workload_families
                .contains(&String::from("symbolic_locality"))
        );
        let recurrent = contracts
            .iter()
            .find(|contract| contract.design_family == TassadarStateDesignFamily::RecurrentState)
            .expect("recurrent contract");
        assert_eq!(
            recurrent.replay_posture,
            TassadarStateDesignReplayPosture::BoundedStatePublication
        );
        assert!(!recurrent.contract_digest.is_empty());
    }
}
