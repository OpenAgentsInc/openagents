use psionic_runtime::{TassadarEffectClass, TassadarEffectReplayPosture, tassadar_effect_taxonomy};
use serde::{Deserialize, Serialize};

/// Sandbox-owned durable-state profile admitted by the widened effect model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSandboxDurableStateImportProfile {
    /// Stable durable-state profile identifier.
    pub profile_id: String,
    /// Stable effect reference.
    pub effect_ref: String,
    /// Stable state namespace.
    pub state_namespace: String,
    /// Replay posture for the durable-state import.
    pub replay_posture: TassadarEffectReplayPosture,
    /// Maximum admitted replays under one snapshot lineage.
    pub max_replays: u32,
    /// Whether an explicit state snapshot is required.
    pub snapshot_required: bool,
    /// Whether a durable-state receipt is required.
    pub durable_state_receipt_required: bool,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
}

/// Sandbox-facing effect boundary for the widened taxonomy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSandboxEffectBoundary {
    /// Stable boundary identifier.
    pub boundary_id: String,
    /// Effect refs admitted for deterministic effect-safe continuation.
    pub continuation_safe_effect_refs: Vec<String>,
    /// Effect refs that stay outside deterministic effect-safe continuation.
    pub continuation_refused_effect_refs: Vec<String>,
    /// Durable-state profiles admitted by the sandbox boundary.
    pub durable_state_profiles: Vec<TassadarSandboxDurableStateImportProfile>,
    /// Effect refs that require explicit sandbox delegation.
    pub delegated_effect_refs: Vec<String>,
    /// Effect refs that are receipt-bound but nondeterministic.
    pub nondeterministic_effect_refs: Vec<String>,
    /// Effect refs that remain refused.
    pub refused_effect_refs: Vec<String>,
    /// Whether sandbox delegation requires a challenge receipt.
    pub challenge_receipt_required_for_delegation: bool,
    /// Plain-language capability summary.
    pub capability_summary: String,
}

/// Returns the sandbox-facing widened effect boundary.
#[must_use]
pub fn tassadar_sandbox_effect_boundary() -> TassadarSandboxEffectBoundary {
    let taxonomy = tassadar_effect_taxonomy();
    let durable_state_profiles = taxonomy
        .entries
        .iter()
        .filter(|entry| entry.effect_class == TassadarEffectClass::DeterministicHostState)
        .map(|entry| TassadarSandboxDurableStateImportProfile {
            profile_id: format!("tassadar.sandbox_durable_state.{}.v1", entry.effect_ref),
            effect_ref: entry.effect_ref.clone(),
            state_namespace: String::from("sandbox.counter_slot"),
            replay_posture: entry.replay_posture,
            max_replays: entry.max_replays,
            snapshot_required: true,
            durable_state_receipt_required: true,
            claim_boundary: entry.note.clone(),
        })
        .collect::<Vec<_>>();
    let delegated_effect_refs = taxonomy
        .entries
        .iter()
        .filter(|entry| entry.effect_class == TassadarEffectClass::ExternalSandboxDelegation)
        .map(|entry| entry.effect_ref.clone())
        .collect::<Vec<_>>();
    let nondeterministic_effect_refs = taxonomy
        .entries
        .iter()
        .filter(|entry| entry.effect_class == TassadarEffectClass::BoundedNondeterministicInput)
        .map(|entry| entry.effect_ref.clone())
        .collect::<Vec<_>>();
    let refused_effect_refs = taxonomy
        .entries
        .iter()
        .filter(|entry| entry.effect_class == TassadarEffectClass::UnsafeSideEffect)
        .map(|entry| entry.effect_ref.clone())
        .collect::<Vec<_>>();
    let continuation_safe_effect_refs = taxonomy
        .entries
        .iter()
        .filter(|entry| entry.effect_class == TassadarEffectClass::DeterministicInternalStub)
        .map(|entry| entry.effect_ref.clone())
        .collect::<Vec<_>>();
    let continuation_refused_effect_refs = taxonomy
        .entries
        .iter()
        .filter(|entry| entry.effect_class != TassadarEffectClass::DeterministicInternalStub)
        .map(|entry| entry.effect_ref.clone())
        .collect::<Vec<_>>();
    TassadarSandboxEffectBoundary {
        boundary_id: String::from("tassadar.sandbox_effect_boundary.v1"),
        continuation_safe_effect_refs,
        continuation_refused_effect_refs,
        durable_state_profiles,
        delegated_effect_refs,
        nondeterministic_effect_refs,
        refused_effect_refs,
        challenge_receipt_required_for_delegation: true,
        capability_summary: String::from(
            "sandbox effect boundary keeps durable host-backed state, explicit sandbox delegation, receipt-bound nondeterministic inputs, and refused side effects distinct. It does not collapse host-backed state or delegation into internal exact compute",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::tassadar_sandbox_effect_boundary;
    use psionic_runtime::TassadarEffectReplayPosture;

    #[test]
    fn sandbox_effect_boundary_is_machine_legible() {
        let boundary = tassadar_sandbox_effect_boundary();
        assert_eq!(boundary.boundary_id, "tassadar.sandbox_effect_boundary.v1");
        assert_eq!(
            boundary.continuation_safe_effect_refs,
            vec![String::from("env.clock_stub")]
        );
        assert!(
            boundary
                .continuation_refused_effect_refs
                .contains(&String::from("state.counter_slot_read"))
        );
        assert_eq!(
            boundary.delegated_effect_refs,
            vec![String::from("sandbox.math_eval")]
        );
        assert_eq!(
            boundary.nondeterministic_effect_refs,
            vec![String::from("input.relay_sample")]
        );
        assert_eq!(
            boundary.refused_effect_refs,
            vec![String::from("host.fs_write")]
        );
        assert_eq!(boundary.durable_state_profiles.len(), 1);
        assert_eq!(
            boundary.durable_state_profiles[0].replay_posture,
            TassadarEffectReplayPosture::SnapshotBoundedReplay
        );
    }
}
