use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Typed effect class admitted or refused by the widened effect model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEffectClass {
    /// Deterministic zero-side-effect stub that stays fully internal.
    DeterministicInternalStub,
    /// Deterministic host-backed state that requires explicit snapshots.
    DeterministicHostState,
    /// Explicit sandbox delegation with challengeable evidence.
    ExternalSandboxDelegation,
    /// Bounded nondeterministic input with one explicit receipt window.
    BoundedNondeterministicInput,
    /// Unsafe side effect that remains refused.
    UnsafeSideEffect,
}

/// Runtime boundary for one effect class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEffectExecutionBoundary {
    InternalOnly,
    HostStateOnly,
    SandboxDelegationOnly,
    ReceiptBoundNondeterministic,
    Refused,
}

/// Replay posture for one effect class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEffectReplayPosture {
    ExactReplay,
    SnapshotBoundedReplay,
    ChallengeableDelegation,
    ReceiptBoundReplay,
    Refused,
}

/// Evidence required before one effect class may execute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEffectEvidenceRequirement {
    CapabilityPublicationOnly,
    StateSnapshotAndReceipt,
    SandboxDescriptorAndChallengeReceipt,
    InputReceiptOnly,
    Refused,
}

/// One effect entry in the widened runtime taxonomy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectTaxonomyEntry {
    pub effect_ref: String,
    pub effect_class: TassadarEffectClass,
    pub execution_boundary: TassadarEffectExecutionBoundary,
    pub replay_posture: TassadarEffectReplayPosture,
    pub evidence_requirement: TassadarEffectEvidenceRequirement,
    pub max_replays: u32,
    pub note: String,
}

/// Reusable runtime taxonomy for the widened effect model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectTaxonomy {
    pub taxonomy_id: String,
    pub entries: Vec<TassadarEffectTaxonomyEntry>,
    pub kernel_policy_dependency_marker: String,
    pub world_mount_dependency_marker: String,
    pub claim_boundary: String,
    pub taxonomy_digest: String,
}

/// Replay-limit facts attached to one effect receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectReplayLimit {
    pub max_replays: u32,
    pub observed_replay_attempt: u32,
    pub state_snapshot_required: bool,
    pub durable_state_receipt_required: bool,
    pub sandbox_challenge_receipt_required: bool,
    pub nondeterministic_input_receipt_required: bool,
}

/// Request negotiated against the widened effect taxonomy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectRequest {
    pub request_id: String,
    pub effect_ref: String,
    pub allow_external_delegation: bool,
    pub policy_allows_delegation: bool,
    pub state_snapshot_present: bool,
    pub durable_state_receipt_present: bool,
    pub sandbox_descriptor_present: bool,
    pub challenge_receipt_present: bool,
    pub nondeterministic_input_receipt_present: bool,
    pub replay_attempt: u32,
}

/// Typed refusal reason emitted before an effect class is admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEffectRefusalReason {
    UnknownEffect,
    ExternalDelegationDisallowed,
    PolicyDeniedDelegation,
    StateSnapshotMissing,
    DurableStateReceiptMissing,
    SandboxDescriptorMissing,
    ChallengeReceiptMissing,
    NondeterministicInputReceiptMissing,
    ReplayLimitExceeded,
    UnsafeEffectClass,
}

/// Runtime receipt for one admitted effect execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectReceipt {
    pub request_id: String,
    pub effect_ref: String,
    pub effect_class: TassadarEffectClass,
    pub execution_boundary: TassadarEffectExecutionBoundary,
    pub replay_posture: TassadarEffectReplayPosture,
    pub replay_limit: TassadarEffectReplayLimit,
    pub evidence_refs: Vec<String>,
    pub route_label: String,
    pub note: String,
    pub receipt_digest: String,
}

/// Returns the widened runtime effect taxonomy.
#[must_use]
pub fn tassadar_effect_taxonomy() -> TassadarEffectTaxonomy {
    let entries = vec![
        TassadarEffectTaxonomyEntry {
            effect_ref: String::from("env.clock_stub"),
            effect_class: TassadarEffectClass::DeterministicInternalStub,
            execution_boundary: TassadarEffectExecutionBoundary::InternalOnly,
            replay_posture: TassadarEffectReplayPosture::ExactReplay,
            evidence_requirement: TassadarEffectEvidenceRequirement::CapabilityPublicationOnly,
            max_replays: 4_096,
            note: String::from(
                "deterministic zero-side-effect stub stays inside the internal exact-compute lane",
            ),
        },
        TassadarEffectTaxonomyEntry {
            effect_ref: String::from("state.counter_slot_read"),
            effect_class: TassadarEffectClass::DeterministicHostState,
            execution_boundary: TassadarEffectExecutionBoundary::HostStateOnly,
            replay_posture: TassadarEffectReplayPosture::SnapshotBoundedReplay,
            evidence_requirement: TassadarEffectEvidenceRequirement::StateSnapshotAndReceipt,
            max_replays: 3,
            note: String::from(
                "deterministic host-backed counter state is admissible only with explicit snapshots and durable-state receipts",
            ),
        },
        TassadarEffectTaxonomyEntry {
            effect_ref: String::from("sandbox.math_eval"),
            effect_class: TassadarEffectClass::ExternalSandboxDelegation,
            execution_boundary: TassadarEffectExecutionBoundary::SandboxDelegationOnly,
            replay_posture: TassadarEffectReplayPosture::ChallengeableDelegation,
            evidence_requirement:
                TassadarEffectEvidenceRequirement::SandboxDescriptorAndChallengeReceipt,
            max_replays: 1,
            note: String::from(
                "sandbox math evaluation stays explicit delegation with challengeable evidence instead of being rebranded as internal compute",
            ),
        },
        TassadarEffectTaxonomyEntry {
            effect_ref: String::from("input.relay_sample"),
            effect_class: TassadarEffectClass::BoundedNondeterministicInput,
            execution_boundary: TassadarEffectExecutionBoundary::ReceiptBoundNondeterministic,
            replay_posture: TassadarEffectReplayPosture::ReceiptBoundReplay,
            evidence_requirement: TassadarEffectEvidenceRequirement::InputReceiptOnly,
            max_replays: 1,
            note: String::from(
                "bounded nondeterministic relay input is admissible only when the observed value is bound to an explicit receipt window",
            ),
        },
        TassadarEffectTaxonomyEntry {
            effect_ref: String::from("host.fs_write"),
            effect_class: TassadarEffectClass::UnsafeSideEffect,
            execution_boundary: TassadarEffectExecutionBoundary::Refused,
            replay_posture: TassadarEffectReplayPosture::Refused,
            evidence_requirement: TassadarEffectEvidenceRequirement::Refused,
            max_replays: 0,
            note: String::from(
                "filesystem writes remain refused because they collapse bounded execution into ambient side-effect authority",
            ),
        },
    ];
    let mut taxonomy = TassadarEffectTaxonomy {
        taxonomy_id: String::from("tassadar.effect_taxonomy.v1"),
        entries,
        kernel_policy_dependency_marker: String::from(
            "kernel-policy remains the authority owner for settlement-grade effect admission outside standalone psionic",
        ),
        world_mount_dependency_marker: String::from(
            "world-mounts remain the authority owner for task-scoped effect admission outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this taxonomy keeps deterministic internal stubs, deterministic host-backed state, sandbox delegation, bounded nondeterministic inputs, and refused side effects distinct. It does not collapse external delegation or host-backed state into internal exact compute",
        ),
        taxonomy_digest: String::new(),
    };
    taxonomy.taxonomy_digest = stable_digest(b"psionic_tassadar_effect_taxonomy|", &taxonomy);
    taxonomy
}

/// Negotiates one effect request against the widened taxonomy.
pub fn negotiate_tassadar_effect_request(
    request: &TassadarEffectRequest,
    taxonomy: &TassadarEffectTaxonomy,
) -> Result<TassadarEffectReceipt, TassadarEffectRefusalReason> {
    let entry = taxonomy
        .entries
        .iter()
        .find(|entry| entry.effect_ref == request.effect_ref)
        .ok_or(TassadarEffectRefusalReason::UnknownEffect)?;
    if entry.effect_class == TassadarEffectClass::UnsafeSideEffect {
        return Err(TassadarEffectRefusalReason::UnsafeEffectClass);
    }
    if request.replay_attempt > entry.max_replays {
        return Err(TassadarEffectRefusalReason::ReplayLimitExceeded);
    }
    match entry.effect_class {
        TassadarEffectClass::DeterministicInternalStub => {}
        TassadarEffectClass::DeterministicHostState => {
            if !request.state_snapshot_present {
                return Err(TassadarEffectRefusalReason::StateSnapshotMissing);
            }
            if !request.durable_state_receipt_present {
                return Err(TassadarEffectRefusalReason::DurableStateReceiptMissing);
            }
        }
        TassadarEffectClass::ExternalSandboxDelegation => {
            if !request.allow_external_delegation {
                return Err(TassadarEffectRefusalReason::ExternalDelegationDisallowed);
            }
            if !request.policy_allows_delegation {
                return Err(TassadarEffectRefusalReason::PolicyDeniedDelegation);
            }
            if !request.sandbox_descriptor_present {
                return Err(TassadarEffectRefusalReason::SandboxDescriptorMissing);
            }
            if !request.challenge_receipt_present {
                return Err(TassadarEffectRefusalReason::ChallengeReceiptMissing);
            }
        }
        TassadarEffectClass::BoundedNondeterministicInput => {
            if !request.nondeterministic_input_receipt_present {
                return Err(TassadarEffectRefusalReason::NondeterministicInputReceiptMissing);
            }
        }
        TassadarEffectClass::UnsafeSideEffect => unreachable!("handled above"),
    }
    let replay_limit = TassadarEffectReplayLimit {
        max_replays: entry.max_replays,
        observed_replay_attempt: request.replay_attempt,
        state_snapshot_required: entry.effect_class == TassadarEffectClass::DeterministicHostState,
        durable_state_receipt_required: entry.effect_class
            == TassadarEffectClass::DeterministicHostState,
        sandbox_challenge_receipt_required: entry.effect_class
            == TassadarEffectClass::ExternalSandboxDelegation,
        nondeterministic_input_receipt_required: entry.effect_class
            == TassadarEffectClass::BoundedNondeterministicInput,
    };
    let evidence_refs = effect_evidence_refs(entry);
    let route_label = match entry.effect_class {
        TassadarEffectClass::DeterministicInternalStub => "internal_exact_effect",
        TassadarEffectClass::DeterministicHostState => "host_state_replay_bound",
        TassadarEffectClass::ExternalSandboxDelegation => "sandbox_delegated_effect",
        TassadarEffectClass::BoundedNondeterministicInput => "receipt_bound_input",
        TassadarEffectClass::UnsafeSideEffect => "refused",
    };
    let mut receipt = TassadarEffectReceipt {
        request_id: request.request_id.clone(),
        effect_ref: request.effect_ref.clone(),
        effect_class: entry.effect_class,
        execution_boundary: entry.execution_boundary,
        replay_posture: entry.replay_posture,
        replay_limit,
        evidence_refs,
        route_label: String::from(route_label),
        note: entry.note.clone(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(b"psionic_tassadar_effect_receipt|", &receipt);
    Ok(receipt)
}

fn effect_evidence_refs(entry: &TassadarEffectTaxonomyEntry) -> Vec<String> {
    match entry.effect_class {
        TassadarEffectClass::DeterministicInternalStub => vec![String::from(
            "fixtures/tassadar/reports/tassadar_import_policy_matrix_report.json",
        )],
        TassadarEffectClass::DeterministicHostState => vec![
            String::from("receipt://tassadar/state.counter_slot.snapshot"),
            String::from("receipt://tassadar/state.counter_slot.read"),
        ],
        TassadarEffectClass::ExternalSandboxDelegation => vec![
            String::from("receipt://sandbox/math_eval.descriptor"),
            String::from("receipt://sandbox/math_eval.challenge"),
        ],
        TassadarEffectClass::BoundedNondeterministicInput => {
            vec![String::from("receipt://tassadar/input.relay_sample.window")]
        }
        TassadarEffectClass::UnsafeSideEffect => Vec::new(),
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
        TassadarEffectClass, TassadarEffectRefusalReason, TassadarEffectReplayPosture,
        TassadarEffectRequest, negotiate_tassadar_effect_request, tassadar_effect_taxonomy,
    };

    #[test]
    fn effect_taxonomy_is_machine_legible() {
        let taxonomy = tassadar_effect_taxonomy();

        assert_eq!(taxonomy.entries.len(), 5);
        assert!(
            taxonomy
                .entries
                .iter()
                .any(|entry| entry.effect_ref == "state.counter_slot_read"
                    && entry.effect_class == TassadarEffectClass::DeterministicHostState)
        );
        assert!(taxonomy.entries.iter().any(|entry| {
            entry.effect_ref == "input.relay_sample"
                && entry.replay_posture == TassadarEffectReplayPosture::ReceiptBoundReplay
        }));
    }

    #[test]
    fn deterministic_and_stateful_effects_enforce_replay_limits() {
        let taxonomy = tassadar_effect_taxonomy();
        let durable_request = TassadarEffectRequest {
            request_id: String::from("req.durable_ok"),
            effect_ref: String::from("state.counter_slot_read"),
            allow_external_delegation: false,
            policy_allows_delegation: false,
            state_snapshot_present: true,
            durable_state_receipt_present: true,
            sandbox_descriptor_present: false,
            challenge_receipt_present: false,
            nondeterministic_input_receipt_present: false,
            replay_attempt: 2,
        };
        let receipt = negotiate_tassadar_effect_request(&durable_request, &taxonomy)
            .expect("durable state should be admitted");
        assert_eq!(receipt.replay_limit.max_replays, 3);
        assert_eq!(receipt.route_label, "host_state_replay_bound");

        let mut over_limit = durable_request;
        over_limit.request_id = String::from("req.durable_over_limit");
        over_limit.replay_attempt = 4;
        let err = negotiate_tassadar_effect_request(&over_limit, &taxonomy)
            .expect_err("replay limit should refuse");
        assert_eq!(err, TassadarEffectRefusalReason::ReplayLimitExceeded);
    }

    #[test]
    fn delegated_nondeterministic_and_unsafe_effects_keep_boundaries_explicit() {
        let taxonomy = tassadar_effect_taxonomy();
        let delegated_request = TassadarEffectRequest {
            request_id: String::from("req.delegate_ok"),
            effect_ref: String::from("sandbox.math_eval"),
            allow_external_delegation: true,
            policy_allows_delegation: true,
            state_snapshot_present: false,
            durable_state_receipt_present: false,
            sandbox_descriptor_present: true,
            challenge_receipt_present: true,
            nondeterministic_input_receipt_present: false,
            replay_attempt: 1,
        };
        let delegated = negotiate_tassadar_effect_request(&delegated_request, &taxonomy)
            .expect("delegation should be admitted");
        assert_eq!(delegated.route_label, "sandbox_delegated_effect");

        let mut missing_policy = delegated_request;
        missing_policy.request_id = String::from("req.delegate_denied");
        missing_policy.policy_allows_delegation = false;
        let err = negotiate_tassadar_effect_request(&missing_policy, &taxonomy)
            .expect_err("policy denial should refuse");
        assert_eq!(err, TassadarEffectRefusalReason::PolicyDeniedDelegation);

        let nondeterministic_request = TassadarEffectRequest {
            request_id: String::from("req.input_ok"),
            effect_ref: String::from("input.relay_sample"),
            allow_external_delegation: false,
            policy_allows_delegation: false,
            state_snapshot_present: false,
            durable_state_receipt_present: false,
            sandbox_descriptor_present: false,
            challenge_receipt_present: false,
            nondeterministic_input_receipt_present: true,
            replay_attempt: 1,
        };
        let nondeterministic =
            negotiate_tassadar_effect_request(&nondeterministic_request, &taxonomy)
                .expect("receipt-bound input should be admitted");
        assert_eq!(nondeterministic.route_label, "receipt_bound_input");

        let unsafe_request = TassadarEffectRequest {
            request_id: String::from("req.fs_write"),
            effect_ref: String::from("host.fs_write"),
            allow_external_delegation: false,
            policy_allows_delegation: false,
            state_snapshot_present: false,
            durable_state_receipt_present: false,
            sandbox_descriptor_present: false,
            challenge_receipt_present: false,
            nondeterministic_input_receipt_present: false,
            replay_attempt: 0,
        };
        let err = negotiate_tassadar_effect_request(&unsafe_request, &taxonomy)
            .expect_err("unsafe side effect should refuse");
        assert_eq!(err, TassadarEffectRefusalReason::UnsafeEffectClass);
    }
}
