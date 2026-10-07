use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_CALL_FRAME_RESUME_PROFILE_ID, TASSADAR_CALL_FRAME_RESUME_RUN_ROOT_REF,
    TassadarCallFrameResumeBundle, TassadarCallFrameResumeCaseReceipt, TassadarEffectReceipt,
    TassadarEffectRefusalReason, TassadarEffectRequest,
    build_tassadar_call_frame_resume_bundle, negotiate_tassadar_effect_request,
    tassadar_effect_taxonomy,
};

/// Stable target profile identifier for deterministic import-mediated continuation.
pub const TASSADAR_EFFECT_SAFE_RESUME_TARGET_PROFILE_ID: &str =
    "tassadar.internal_compute.deterministic_import_subset.v1";
/// Stable run root for the committed effect-safe resume bundle.
pub const TASSADAR_EFFECT_SAFE_RESUME_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_effect_safe_resume_v1";
/// Stable filename for the committed effect-safe resume bundle.
pub const TASSADAR_EFFECT_SAFE_RESUME_BUNDLE_FILE: &str =
    "tassadar_effect_safe_resume_bundle.json";

/// Decision status for one effect-safe continuation case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEffectSafeResumeDecisionStatus {
    Admitted,
    Refused,
}

/// Typed refusal posture for one effect-safe continuation case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEffectSafeResumeRefusalKind {
    HostStateUnsupported,
    ExternalDelegationUnsupported,
    NondeterministicInputUnsupported,
    TaxonomyRefusedUnsafeEffect,
}

/// One seeded effect-safe continuation case over the resumable base lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectSafeResumeCaseReceipt {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable target profile identifier.
    pub target_profile_id: String,
    /// Stable base profile identifier.
    pub base_profile_id: String,
    /// Stable base case identifier from the resumable call-frame lane.
    pub base_case_id: String,
    /// Stable checkpoint identifier carried from the resumable base lane.
    pub checkpoint_id: String,
    /// Effect request negotiated against the taxonomy.
    pub effect_request: TassadarEffectRequest,
    /// Decision status for the target profile.
    pub decision_status: TassadarEffectSafeResumeDecisionStatus,
    /// Negotiated receipt when taxonomy negotiation succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_receipt: Option<TassadarEffectReceipt>,
    /// Typed refusal kind when the target profile rejects the effect family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_kind: Option<TassadarEffectSafeResumeRefusalKind>,
    /// Underlying taxonomy refusal when negotiation itself failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taxonomy_refusal_reason: Option<TassadarEffectRefusalReason>,
    /// Whether the base exact-resume proof remains green under the admitted effect receipt.
    pub exact_resume_parity: bool,
    /// Plain-language detail.
    pub note: String,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

/// Canonical runtime bundle for deterministic import-mediated continuation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEffectSafeResumeBundle {
    /// Schema version.
    pub schema_version: u16,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Stable target profile identifier.
    pub target_profile_id: String,
    /// Stable base resumable profile identifier.
    pub base_profile_id: String,
    /// Stable source run-root reference for the resumable base lane.
    pub base_run_root_ref: String,
    /// Stable deterministic-stub-only effect refs admitted by the target profile.
    pub continuation_safe_effect_refs: Vec<String>,
    /// Stable effect refs that remain outside the target profile.
    pub continuation_refused_effect_refs: Vec<String>,
    /// Case receipts covered by the bundle.
    pub case_receipts: Vec<TassadarEffectSafeResumeCaseReceipt>,
    /// Number of admitted effect-safe continuation rows.
    pub admitted_case_count: u32,
    /// Number of refused effect-safe continuation rows.
    pub refusal_case_count: u32,
    /// Dependency marker for authority-owned effect admission.
    pub kernel_policy_dependency_marker: String,
    /// Dependency marker for mount-owned effect admission.
    pub world_mount_dependency_marker: String,
    /// Claim boundary.
    pub claim_boundary: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarEffectSafeResumeError {
    #[error("call-frame resume bundle did not contain seeded case `{case_id}`")]
    MissingBaseCase { case_id: String },
}

/// Builds the canonical deterministic import-mediated continuation bundle.
pub fn build_tassadar_effect_safe_resume_bundle(
) -> Result<TassadarEffectSafeResumeBundle, TassadarEffectSafeResumeError> {
    let taxonomy = tassadar_effect_taxonomy();
    let base_bundle = build_tassadar_call_frame_resume_bundle()
        .expect("call-frame resume bundle should build before effect-safe promotion");
    let recursive_sum_case = base_case(&base_bundle, "recursive_sum_pause_mid_stack")?;
    let multi_function_case = base_case(&base_bundle, "multi_function_pause_after_direct_call")?;

    let mut case_receipts = vec![
        admitted_case(
            recursive_sum_case,
            TassadarEffectRequest {
                request_id: String::from("req.effect_safe_resume.clock_stub.recursive_sum"),
                effect_ref: String::from("env.clock_stub"),
                allow_external_delegation: false,
                policy_allows_delegation: false,
                state_snapshot_present: false,
                durable_state_receipt_present: false,
                sandbox_descriptor_present: false,
                challenge_receipt_present: false,
                nondeterministic_input_receipt_present: false,
                replay_attempt: 12,
            },
            "deterministic internal stubs stay admissible for resumable continuation because their receipts preserve the exact base replay contract",
            &taxonomy,
        ),
        admitted_case(
            multi_function_case,
            TassadarEffectRequest {
                request_id: String::from("req.effect_safe_resume.clock_stub.multi_function"),
                effect_ref: String::from("env.clock_stub"),
                allow_external_delegation: false,
                policy_allows_delegation: false,
                state_snapshot_present: false,
                durable_state_receipt_present: false,
                sandbox_descriptor_present: false,
                challenge_receipt_present: false,
                nondeterministic_input_receipt_present: false,
                replay_attempt: 9,
            },
            "deterministic stub receipts keep multi-function continuation exact without widening the target profile into host-backed or delegated effects",
            &taxonomy,
        ),
        refused_case(
            recursive_sum_case,
            TassadarEffectRequest {
                request_id: String::from("req.effect_safe_resume.host_state.refused"),
                effect_ref: String::from("state.counter_slot_read"),
                allow_external_delegation: false,
                policy_allows_delegation: false,
                state_snapshot_present: true,
                durable_state_receipt_present: true,
                sandbox_descriptor_present: false,
                challenge_receipt_present: false,
                nondeterministic_input_receipt_present: false,
                replay_attempt: 1,
            },
            TassadarEffectSafeResumeRefusalKind::HostStateUnsupported,
            "host-backed state stays outside the deterministic import subset even when a snapshot and durable-state receipt exist",
            &taxonomy,
        ),
        refused_case(
            multi_function_case,
            TassadarEffectRequest {
                request_id: String::from("req.effect_safe_resume.sandbox.refused"),
                effect_ref: String::from("sandbox.math_eval"),
                allow_external_delegation: true,
                policy_allows_delegation: true,
                state_snapshot_present: false,
                durable_state_receipt_present: false,
                sandbox_descriptor_present: true,
                challenge_receipt_present: true,
                nondeterministic_input_receipt_present: false,
                replay_attempt: 1,
            },
            TassadarEffectSafeResumeRefusalKind::ExternalDelegationUnsupported,
            "sandbox delegation remains challengeable external execution and is not silently rebranded as deterministic internal continuation",
            &taxonomy,
        ),
        refused_case(
            recursive_sum_case,
            TassadarEffectRequest {
                request_id: String::from("req.effect_safe_resume.input.refused"),
                effect_ref: String::from("input.relay_sample"),
                allow_external_delegation: false,
                policy_allows_delegation: false,
                state_snapshot_present: false,
                durable_state_receipt_present: false,
                sandbox_descriptor_present: false,
                challenge_receipt_present: false,
                nondeterministic_input_receipt_present: true,
                replay_attempt: 1,
            },
            TassadarEffectSafeResumeRefusalKind::NondeterministicInputUnsupported,
            "receipt-bound nondeterministic input remains explicit but outside the deterministic import subset for exact resumable continuation",
            &taxonomy,
        ),
        unsafe_refused_case(
            multi_function_case,
            TassadarEffectRequest {
                request_id: String::from("req.effect_safe_resume.unsafe.refused"),
                effect_ref: String::from("host.fs_write"),
                allow_external_delegation: false,
                policy_allows_delegation: false,
                state_snapshot_present: false,
                durable_state_receipt_present: false,
                sandbox_descriptor_present: false,
                challenge_receipt_present: false,
                nondeterministic_input_receipt_present: false,
                replay_attempt: 1,
            },
            "unsafe side effects remain refused by the taxonomy before continuation policy can even consider them",
            &taxonomy,
        ),
    ];
    case_receipts.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    let admitted_case_count = case_receipts
        .iter()
        .filter(|receipt| receipt.decision_status == TassadarEffectSafeResumeDecisionStatus::Admitted)
        .count() as u32;
    let refusal_case_count = case_receipts.len() as u32 - admitted_case_count;
    let continuation_safe_effect_refs = vec![String::from("env.clock_stub")];
    let continuation_refused_effect_refs = vec![
        String::from("host.fs_write"),
        String::from("input.relay_sample"),
        String::from("sandbox.math_eval"),
        String::from("state.counter_slot_read"),
    ];

    let mut bundle = TassadarEffectSafeResumeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.effect_safe_resume.bundle.v1"),
        target_profile_id: String::from(TASSADAR_EFFECT_SAFE_RESUME_TARGET_PROFILE_ID),
        base_profile_id: String::from(TASSADAR_CALL_FRAME_RESUME_PROFILE_ID),
        base_run_root_ref: String::from(TASSADAR_CALL_FRAME_RESUME_RUN_ROOT_REF),
        continuation_safe_effect_refs,
        continuation_refused_effect_refs,
        case_receipts,
        admitted_case_count,
        refusal_case_count,
        kernel_policy_dependency_marker: taxonomy.kernel_policy_dependency_marker,
        world_mount_dependency_marker: taxonomy.world_mount_dependency_marker,
        claim_boundary: String::from(
            "this bundle promotes the resumable call-frame lane only into a deterministic-import subset. It admits exact resumable continuation for deterministic internal stubs with explicit effect receipts and keeps host-backed state, sandbox delegation, bounded nondeterministic input, and unsafe side effects on typed refusal paths.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_digest(b"tassadar_effect_safe_resume_bundle|", &bundle);
    Ok(bundle)
}

fn base_case<'a>(
    bundle: &'a TassadarCallFrameResumeBundle,
    case_id: &str,
) -> Result<&'a TassadarCallFrameResumeCaseReceipt, TassadarEffectSafeResumeError> {
    bundle
        .case_receipts
        .iter()
        .find(|case| case.case_id == case_id)
        .ok_or_else(|| TassadarEffectSafeResumeError::MissingBaseCase {
            case_id: String::from(case_id),
        })
}

fn admitted_case(
    base_case: &TassadarCallFrameResumeCaseReceipt,
    effect_request: TassadarEffectRequest,
    note: &str,
    taxonomy: &crate::TassadarEffectTaxonomy,
) -> TassadarEffectSafeResumeCaseReceipt {
    let effect_receipt =
        negotiate_tassadar_effect_request(&effect_request, taxonomy).expect("effect should admit");
    let mut receipt = TassadarEffectSafeResumeCaseReceipt {
        case_id: format!("{}.{}", base_case.case_id, "clock_stub"),
        target_profile_id: String::from(TASSADAR_EFFECT_SAFE_RESUME_TARGET_PROFILE_ID),
        base_profile_id: String::from(TASSADAR_CALL_FRAME_RESUME_PROFILE_ID),
        base_case_id: base_case.case_id.clone(),
        checkpoint_id: base_case.checkpoint.checkpoint_id.clone(),
        effect_request,
        decision_status: TassadarEffectSafeResumeDecisionStatus::Admitted,
        effect_receipt: Some(effect_receipt),
        refusal_kind: None,
        taxonomy_refusal_reason: None,
        exact_resume_parity: base_case.exact_resume_parity,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(b"tassadar_effect_safe_resume_case|", &receipt);
    receipt
}

fn refused_case(
    base_case: &TassadarCallFrameResumeCaseReceipt,
    effect_request: TassadarEffectRequest,
    refusal_kind: TassadarEffectSafeResumeRefusalKind,
    note: &str,
    taxonomy: &crate::TassadarEffectTaxonomy,
) -> TassadarEffectSafeResumeCaseReceipt {
    let effect_receipt =
        negotiate_tassadar_effect_request(&effect_request, taxonomy).expect("effect should admit");
    let mut receipt = TassadarEffectSafeResumeCaseReceipt {
        case_id: format!("{}.{}", base_case.case_id, effect_request.effect_ref.replace('.', "_")),
        target_profile_id: String::from(TASSADAR_EFFECT_SAFE_RESUME_TARGET_PROFILE_ID),
        base_profile_id: String::from(TASSADAR_CALL_FRAME_RESUME_PROFILE_ID),
        base_case_id: base_case.case_id.clone(),
        checkpoint_id: base_case.checkpoint.checkpoint_id.clone(),
        effect_request,
        decision_status: TassadarEffectSafeResumeDecisionStatus::Refused,
        effect_receipt: Some(effect_receipt),
        refusal_kind: Some(refusal_kind),
        taxonomy_refusal_reason: None,
        exact_resume_parity: false,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(b"tassadar_effect_safe_resume_case|", &receipt);
    receipt
}

fn unsafe_refused_case(
    base_case: &TassadarCallFrameResumeCaseReceipt,
    effect_request: TassadarEffectRequest,
    note: &str,
    taxonomy: &crate::TassadarEffectTaxonomy,
) -> TassadarEffectSafeResumeCaseReceipt {
    let taxonomy_refusal_reason = negotiate_tassadar_effect_request(&effect_request, taxonomy)
        .expect_err("unsafe side effect should refuse in the taxonomy");
    let mut receipt = TassadarEffectSafeResumeCaseReceipt {
        case_id: format!("{}.{}", base_case.case_id, "unsafe_effect"),
        target_profile_id: String::from(TASSADAR_EFFECT_SAFE_RESUME_TARGET_PROFILE_ID),
        base_profile_id: String::from(TASSADAR_CALL_FRAME_RESUME_PROFILE_ID),
        base_case_id: base_case.case_id.clone(),
        checkpoint_id: base_case.checkpoint.checkpoint_id.clone(),
        effect_request,
        decision_status: TassadarEffectSafeResumeDecisionStatus::Refused,
        effect_receipt: None,
        refusal_kind: Some(TassadarEffectSafeResumeRefusalKind::TaxonomyRefusedUnsafeEffect),
        taxonomy_refusal_reason: Some(taxonomy_refusal_reason),
        exact_resume_parity: false,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(b"tassadar_effect_safe_resume_case|", &receipt);
    receipt
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
        TassadarEffectSafeResumeDecisionStatus, TassadarEffectSafeResumeRefusalKind,
        TASSADAR_EFFECT_SAFE_RESUME_TARGET_PROFILE_ID, build_tassadar_effect_safe_resume_bundle,
    };

    #[test]
    fn effect_safe_resume_bundle_admits_only_deterministic_stub_cases() {
        let bundle = build_tassadar_effect_safe_resume_bundle().expect("bundle");
        assert_eq!(
            bundle.target_profile_id,
            TASSADAR_EFFECT_SAFE_RESUME_TARGET_PROFILE_ID
        );
        assert_eq!(bundle.admitted_case_count, 2);
        assert_eq!(bundle.refusal_case_count, 4);
        assert!(bundle.case_receipts.iter().any(|receipt| {
            receipt.decision_status == TassadarEffectSafeResumeDecisionStatus::Admitted
                && receipt.effect_request.effect_ref == "env.clock_stub"
                && receipt.exact_resume_parity
        }));
    }

    #[test]
    fn effect_safe_resume_bundle_refuses_nondeterministic_and_unsafe_effects() {
        let bundle = build_tassadar_effect_safe_resume_bundle().expect("bundle");
        assert!(bundle.case_receipts.iter().any(|receipt| {
            receipt.refusal_kind == Some(TassadarEffectSafeResumeRefusalKind::HostStateUnsupported)
        }));
        assert!(bundle.case_receipts.iter().any(|receipt| {
            receipt.refusal_kind
                == Some(TassadarEffectSafeResumeRefusalKind::ExternalDelegationUnsupported)
        }));
        assert!(bundle.case_receipts.iter().any(|receipt| {
            receipt.refusal_kind
                == Some(TassadarEffectSafeResumeRefusalKind::NondeterministicInputUnsupported)
        }));
        assert!(bundle.case_receipts.iter().any(|receipt| {
            receipt.refusal_kind
                == Some(TassadarEffectSafeResumeRefusalKind::TaxonomyRefusedUnsafeEffect)
        }));
    }
}
