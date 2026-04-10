use nostr::{
    EventTemplate, TrainingArtifactLocatorEvent, TrainingCloseoutEvent,
    TrainingNetworkContractEvent, TrainingReceiptEvent, TrainingValidatorVerdictEvent,
    TrainingWindowEvent, TrnEvent, TrnPubkeyReference,
};
use openagents_kernel_core::pylon_training::PylonTrainingReputationRecord;
use openagents_validator_service::{
    ValidatorChallengeFailureCode, ValidatorChallengeResult as ServiceValidatorChallengeResult,
    ValidatorChallengeSnapshot as ServiceValidatorChallengeSnapshot,
};
use serde_json::{Value, json};

use super::{
    ComputeAcceptedOutcomePublicationSource, ComputeAdapterContributionOutcome,
    ComputeTrainingWindowPublicationSource, TrainingTrnChallengeBinding,
    TrainingTrnNetworkContractSource, canonical_challenge_status,
    canonical_challenge_verdict, sha256_prefixed_bytes, training_trn_closeout_status,
    training_trn_network_status,
};

pub(super) fn event_template_and_fingerprint(
    event: &TrnEvent,
) -> Result<(EventTemplate, String), String> {
    let template = event
        .normalize()
        .to_event_template(
            nostr::nip01::unix_now_secs()
                .map_err(|error| format!("training_trn_timestamp_failed:{error}"))?,
        )
        .map_err(|error| format!("training_trn_template_invalid:{error}"))?;
    let content: Value = serde_json::from_str(template.content.as_str())
        .map_err(|error| format!("training_trn_content_decode_failed:{error}"))?;
    let fingerprint_payload = serde_json::to_vec(&json!({
        "kind": template.kind,
        "tags": template.tags,
        "content": content,
    }))
    .map_err(|error| format!("training_trn_fingerprint_encode_failed:{error}"))?;
    Ok((template, sha256_prefixed_bytes(fingerprint_payload.as_slice())))
}

pub(super) fn network_contract_event(
    source: &TrainingTrnNetworkContractSource,
) -> Result<TrainingNetworkContractEvent, String> {
    Ok(TrainingNetworkContractEvent {
        identifier: source.network_id.clone(),
        network_id: source.network_id.clone(),
        status: training_trn_network_status(source).to_string(),
        content: json!({
            "network_id": source.network_id,
            "artifact_bucket_uri": source.artifact_bucket_uri,
            "worker_count": source.worker_count,
            "validator_count": source.validator_count,
            "recovery_source_count": source.recovery_source_count,
            "checkpoint_refs": source.checkpoint_refs.iter().cloned().collect::<Vec<_>>(),
            "initial_window_ids": source.initial_window_ids.iter().cloned().collect::<Vec<_>>(),
            "training_policy_refs": source.training_policy_refs.iter().cloned().collect::<Vec<_>>(),
            "validator_policy_refs": source.validator_policy_refs.iter().cloned().collect::<Vec<_>>(),
            "checkpoint_families": source.checkpoint_families.iter().cloned().collect::<Vec<_>>(),
            "environment_refs": source.environment_refs.iter().cloned().collect::<Vec<_>>(),
            "benchmark_package_refs": source.benchmark_package_refs.iter().cloned().collect::<Vec<_>>(),
            "training_run_ids": source.training_run_ids,
            "kernel_object_ids": source.training_run_ids,
            "kernel_receipt_ids": source.kernel_receipt_ids,
        }),
        model_family: None,
        window_cadence: None,
        roles: Vec::new(),
        profiles: Vec::new(),
        address_refs: Vec::new(),
        extra_tags: {
            let mut tags = vec![vec![
                "class".to_string(),
                "training_network_contract".to_string(),
            ]];
            for training_run_id in &source.training_run_ids {
                tags.push(vec!["run".to_string(), training_run_id.clone()]);
            }
            tags
        },
    }
    .normalize())
}

pub(super) fn window_event(
    source: &ComputeTrainingWindowPublicationSource,
    closeout_status_by_outcome_id: &std::collections::HashMap<String, String>,
) -> Result<TrainingWindowEvent, String> {
    let metadata = super::training_window_metadata_from_value(&source.window.metadata)?;
    let mut extra_tags = vec![
        vec!["run".to_string(), source.window.training_run_id.clone()],
        vec!["class".to_string(), "training_window".to_string()],
    ];
    if let Some(accepted_outcome_id) = source.window.accepted_outcome_id.as_ref()
        && let Some(closeout_status) = closeout_status_by_outcome_id.get(accepted_outcome_id)
    {
        extra_tags.push(vec!["closeout".to_string(), closeout_status.clone()]);
    }
    Ok(TrainingWindowEvent {
        identifier: source.window.window_id.clone(),
        network_id: metadata.network_id.clone(),
        status: source.window.status.label().to_string(),
        content: json!({
            "network_id": metadata.network_id,
            "artifact_bucket_uri": metadata.artifact_bucket_uri,
            "membership_revision": metadata.membership_revision,
            "assignment_plan_count": metadata.assignment_plans.len(),
            "planned_at_ms": metadata.planned_at_ms,
            "activated_at_ms": metadata.activated_at_ms,
            "sealed_at_ms": metadata.sealed_at_ms,
            "reconciled_at_ms": metadata.reconciled_at_ms,
            "seal_deadline_ms": metadata.seal_deadline_ms,
            "training_run_id": source.window.training_run_id,
            "window_id": source.window.window_id,
            "status": source.window.status.label(),
            "stage_id": source.window.stage_id,
            "contributor_set_revision_id": source.window.contributor_set_revision_id,
            "window_summary_digest": source.window.window_summary_digest,
            "aggregated_delta_digest": source.window.aggregated_delta_digest,
            "accepted_outcome_id": source.window.accepted_outcome_id,
            "kernel_object_id": source.window.window_id,
            "kernel_receipt_ids": vec![source.receipt_id.clone()],
        }),
        policy_revision: Some(source.window.validator_policy_ref.clone()),
        assignment_seed: None,
        workload_family: None,
        address_refs: Vec::new(),
        extra_tags,
    }
    .normalize())
}

pub(super) fn window_receipt_event(
    source: &ComputeTrainingWindowPublicationSource,
) -> Result<TrainingReceiptEvent, String> {
    let metadata = super::training_window_metadata_from_value(&source.window.metadata)?;
    let status = source.window.status.label().to_string();
    Ok(TrainingReceiptEvent {
        network_id: metadata.network_id.clone(),
        window_id: source.window.window_id.clone(),
        status: status.clone(),
        content: json!({
            "network_id": metadata.network_id,
            "training_run_id": source.window.training_run_id,
            "window_id": source.window.window_id,
            "status": status,
            "kernel_object_id": source.window.window_id,
            "kernel_receipt_ids": vec![source.receipt_id.clone()],
        }),
        assignment_id: None,
        policy_revision: None,
        role: None,
        artifact_id: None,
        checkpoint_id: None,
        actors: Vec::new(),
        reason_codes: Vec::new(),
        classes: vec!["coordinator_window_state".to_string()],
        address_refs: Vec::new(),
        event_refs: Vec::new(),
        extra_tags: vec![vec!["run".to_string(), source.window.training_run_id.clone()]],
    }
    .normalize())
}

pub(super) fn replay_required_receipt_event(
    contribution: &ComputeAdapterContributionOutcome,
    network_id: &str,
    window_receipt_id: &str,
) -> TrainingReceiptEvent {
    TrainingReceiptEvent {
        network_id: network_id.to_string(),
        window_id: contribution.window_id.clone(),
        status: "replay_requested".to_string(),
        content: json!({
            "network_id": network_id,
            "training_run_id": contribution.training_run_id,
            "window_id": contribution.window_id,
            "assignment_id": contribution.assignment_id,
            "contribution_id": contribution.contribution_id,
            "manifest_digest": contribution.manifest_digest,
            "object_digest": contribution.object_digest,
            "kernel_object_id": contribution.contribution_id,
            "kernel_receipt_ids": vec![window_receipt_id],
        }),
        assignment_id: Some(contribution.assignment_id.clone()),
        policy_revision: None,
        role: None,
        artifact_id: None,
        checkpoint_id: None,
        actors: Vec::new(),
        reason_codes: Vec::new(),
        classes: vec!["contribution_replay_request".to_string()],
        address_refs: Vec::new(),
        event_refs: Vec::new(),
        extra_tags: vec![
            vec!["run".to_string(), contribution.training_run_id.clone()],
            vec!["contribution".to_string(), contribution.contribution_id.clone()],
        ],
    }
    .normalize()
}

pub(super) fn closeout_event(
    source: &ComputeAcceptedOutcomePublicationSource,
) -> Result<TrainingCloseoutEvent, String> {
    let network_id = source
        .outcome
        .metadata
        .get("network_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "training_closeout_network_missing".to_string())?
        .to_string();
    let window_id = source
        .outcome
        .metadata
        .get("window_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "training_closeout_window_missing".to_string())?
        .to_string();
    let closeout_status = training_trn_closeout_status(source);
    Ok(TrainingCloseoutEvent {
        network_id,
        window_id,
        status: closeout_status.clone(),
        content: json!({
            "network_id": source.outcome.metadata.get("network_id"),
            "training_run_id": source.outcome.source_run_id,
            "window_id": source.outcome.metadata.get("window_id"),
            "closeout_status": closeout_status,
            "payout_eligible": source.outcome.metadata.get("payout_eligible").cloned(),
            "training_summary": source.outcome.training_summary,
            "kernel_object_id": source.outcome.outcome_id,
            "kernel_receipt_ids": vec![source.receipt_id.clone()],
        }),
        assignment_id: None,
        artifact_id: None,
        policy_revision: None,
        amount_msats: None,
        actors: Vec::new(),
        reason_codes: Vec::new(),
        event_refs: Vec::new(),
        extra_tags: vec![
            vec!["run".to_string(), source.outcome.source_run_id.clone()],
            vec!["class".to_string(), "training_closeout".to_string()],
        ],
    }
    .normalize())
}

pub(super) fn validator_score_locator_event(
    snapshot: &ServiceValidatorChallengeSnapshot,
    result: &ServiceValidatorChallengeResult,
    binding: &TrainingTrnChallengeBinding,
    receipt_id: &str,
) -> TrainingArtifactLocatorEvent {
    let mut event = TrainingArtifactLocatorEvent::score_snapshot(
        snapshot.request.context.challenge_id.clone(),
        binding.network_id.clone(),
        canonical_challenge_status(snapshot.status).label().to_string(),
        result.result_digest.clone(),
        result.challenge_result_ref.clone(),
        json!({
            "network_id": binding.network_id,
            "training_run_id": binding.training_run_id,
            "window_id": binding.window_id,
            "challenge_id": snapshot.request.context.challenge_id,
            "challenge_kind": binding.challenge_kind,
            "validator_status": canonical_challenge_status(snapshot.status).label(),
            "validator_verdict": canonical_challenge_verdict(result.verdict).label(),
            "verified_row_count": result.verified_row_count,
            "challenge_result_ref": result.challenge_result_ref,
            "result_digest": result.result_digest,
            "kernel_object_id": snapshot.request.context.challenge_id,
            "kernel_receipt_ids": vec![receipt_id],
        }),
    );
    event.window_id = Some(binding.window_id.clone());
    if let Some(reason_code) = result.reason_code {
        event.reason_codes = vec![failure_code_label(reason_code).to_string()];
    }
    event.extra_tags = vec![
        vec!["run".to_string(), binding.training_run_id.clone()],
        vec![
            "challenge".to_string(),
            snapshot.request.context.challenge_id.clone(),
        ],
        vec![
            "validator_verdict".to_string(),
            canonical_challenge_verdict(result.verdict).label().to_string(),
        ],
    ];
    event.normalize()
}

#[allow(dead_code)]
pub(super) fn validator_verdict_event(
    snapshot: &ServiceValidatorChallengeSnapshot,
    result: &ServiceValidatorChallengeResult,
    binding: &TrainingTrnChallengeBinding,
    receipt_id: &str,
) -> TrainingValidatorVerdictEvent {
    TrainingValidatorVerdictEvent {
        network_id: binding.network_id.clone(),
        window_id: binding.window_id.clone(),
        status: canonical_challenge_verdict(result.verdict).label().to_string(),
        content: json!({
            "network_id": binding.network_id,
            "training_run_id": binding.training_run_id,
            "window_id": binding.window_id,
            "challenge_id": snapshot.request.context.challenge_id,
            "challenge_kind": binding.challenge_kind,
            "validator_status": canonical_challenge_status(snapshot.status).label(),
            "validator_verdict": canonical_challenge_verdict(result.verdict).label(),
            "verified_row_count": result.verified_row_count,
            "challenge_result_ref": result.challenge_result_ref,
            "result_digest": result.result_digest,
            "kernel_object_id": snapshot.request.context.challenge_id,
            "kernel_receipt_ids": vec![receipt_id],
        }),
        assignment_id: None,
        artifact_id: None,
        policy_revision: None,
        validator_policy: None,
        digest: Some(result.result_digest.clone()),
        actors: snapshot
            .active_lease
            .as_ref()
            .map(|lease| vec![TrnPubkeyReference::validator(lease.validator_id.clone())])
            .unwrap_or_default(),
        reason_codes: result
            .reason_code
            .map(|reason| vec![failure_code_label(reason).to_string()])
            .unwrap_or_default(),
        event_refs: Vec::new(),
        extra_tags: vec![
            vec!["run".to_string(), binding.training_run_id.clone()],
            vec![
                "challenge".to_string(),
                snapshot.request.context.challenge_id.clone(),
            ],
            vec![
                "validator_status".to_string(),
                canonical_challenge_status(snapshot.status).label().to_string(),
            ],
            vec!["class".to_string(), "validator_verdict".to_string()],
        ],
    }
    .normalize()
}

#[allow(dead_code)]
pub(super) fn reputation_label_event(
    record: &PylonTrainingReputationRecord,
) -> Result<nostr::nip32::LabelEvent, String> {
    nostr::pylon_training_reputation_to_label_event(record)
}

fn failure_code_label(code: ValidatorChallengeFailureCode) -> &'static str {
    match code {
        ValidatorChallengeFailureCode::DimensionMismatch => "dimension_mismatch",
        ValidatorChallengeFailureCode::FieldMismatch => "field_mismatch",
        ValidatorChallengeFailureCode::RowOpeningMissing => "row_opening_missing",
        ValidatorChallengeFailureCode::MerkleProofInvalid => "merkle_proof_invalid",
        ValidatorChallengeFailureCode::FreivaldsMismatch => "freivalds_mismatch",
        ValidatorChallengeFailureCode::LeaseExpired => "lease_expired",
        ValidatorChallengeFailureCode::RetryBudgetExhausted => "retry_budget_exhausted",
    }
}

#[cfg(test)]
mod tests {
    use openagents_kernel_core::pylon_training::{
        PylonTrainingReputationLabel, PylonTrainingReputationNamespace,
    };
    use openagents_validator_service::{
        GpuFreivaldsMerkleWitness, ValidatorChallengeContext, ValidatorChallengeLease,
        ValidatorChallengeResult, ValidatorChallengeSnapshot, ValidatorChallengeStatus,
        ValidatorChallengeVerdict,
    };

    use super::*;

    #[test]
    fn validator_snapshot_maps_to_typed_verdict_event() {
        let snapshot = ValidatorChallengeSnapshot {
            request: openagents_validator_service::ValidatorChallengeRequest::new(
                ValidatorChallengeContext {
                    challenge_id: "challenge.alpha".to_string(),
                    proof_bundle_digest: "sha256:proof".to_string(),
                    request_digest: "sha256:request".to_string(),
                    delivery_proof_id: None,
                    product_id: "openagents.train".to_string(),
                    runtime_backend: "cuda".to_string(),
                    model_id: Some("qwen".to_string()),
                    validator_pool_ref: Some("pool://alpha".to_string()),
                    created_at_ms: 1_700_000_000_000,
                    max_attempts: 2,
                    lease_timeout_ms: 30_000,
                },
                GpuFreivaldsMerkleWitness::from_matrices(&[vec![1]], &[vec![1]], &[vec![1]])
                    .expect("witness"),
            ),
            status: ValidatorChallengeStatus::Verified,
            attempts_used: 1,
            active_lease: Some(ValidatorChallengeLease {
                challenge_id: "challenge.alpha".to_string(),
                attempt: 1,
                validator_id: "validator-alpha".to_string(),
                leased_at_ms: 1_700_000_000_100,
                expires_at_ms: 1_700_000_030_100,
            }),
            final_result: Some(ValidatorChallengeResult {
                challenge_id: "challenge.alpha".to_string(),
                proof_bundle_digest: "sha256:proof".to_string(),
                protocol_id: "openagents.validator.gpu_freivalds_merkle.v1".to_string(),
                attempt: 1,
                status: ValidatorChallengeStatus::Verified,
                verdict: ValidatorChallengeVerdict::Verified,
                reason_code: None,
                detail: "verified".to_string(),
                created_at_ms: 1_700_000_000_100,
                finalized_at_ms: 1_700_000_000_500,
                challenge_seed_digest: None,
                verified_row_count: Some(64),
                result_digest: "sha256:result".to_string(),
                challenge_result_ref: "gs://bucket/result.json".to_string(),
            }),
        };
        let binding = TrainingTrnChallengeBinding {
            network_id: "trainnet.alpha".to_string(),
            training_run_id: "run.alpha".to_string(),
            window_id: "window.0001".to_string(),
            challenge_kind: "aggregate".to_string(),
        };
        let result = snapshot.final_result.as_ref().expect("final result");
        let verdict = validator_verdict_event(&snapshot, result, &binding, "receipt.alpha");
        assert_eq!(verdict.network_id, "trainnet.alpha");
        assert_eq!(verdict.status, "verified");
        assert_eq!(verdict.digest.as_deref(), Some("sha256:result"));
        assert_eq!(verdict.actors.len(), 1);
        assert_eq!(verdict.actors[0].marker.as_deref(), Some("validator"));
    }

    #[test]
    fn reputation_record_maps_to_label_event() {
        let record = PylonTrainingReputationRecord::new(
            PylonTrainingReputationNamespace::Validator,
            PylonTrainingReputationLabel::Poor,
            Some("validator-alpha".to_string()),
            Some("event-alpha".to_string()),
            Some("39512:pubkey:challenge.alpha".to_string()),
        )
        .expect("record");
        let label_event = reputation_label_event(&record).expect("label event");
        assert_eq!(label_event.labels.len(), 1);
        assert_eq!(label_event.labels[0].namespace.as_deref(), Some("trn/validator"));
        assert_eq!(label_event.labels[0].value, "poor");
        assert_eq!(label_event.targets.len(), 3);
    }
}
