use nostr::{
    EventTemplate, TrainingArtifactLocatorEvent, TrainingCloseoutEvent,
    TrainingNetworkContractEvent, TrainingReceiptEvent, TrainingValidatorVerdictEvent,
    TrainingWindowEvent, TrnEvent, TrnPubkeyReference,
};
use openagents_kernel_core::ids::sha256_prefixed_bytes;
use openagents_kernel_core::pylon_training::PylonTrainingReputationRecord;
use openagents_validator_service::{
    ValidatorChallengeFailureCode, ValidatorChallengeResult as ServiceValidatorChallengeResult,
    ValidatorChallengeSnapshot as ServiceValidatorChallengeSnapshot,
};
use serde_json::{Value, json};

use super::{
    ComputeAcceptedOutcomePublicationSource, ComputeAdapterContributionOutcome,
    ComputeAdapterTrainingWindow, ComputeTrainingWindowPublicationSource,
    TrainingTrnChallengeBinding, TrainingTrnNetworkContractSource, canonical_challenge_status,
    canonical_challenge_verdict, training_trn_closeout_status, training_trn_network_status,
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
    Ok((
        template,
        sha256_prefixed_bytes(fingerprint_payload.as_slice()),
    ))
}

fn optional_training_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn push_optional_tag(tags: &mut Vec<Vec<String>>, key: &str, value: &str) {
    if let Some(value) = optional_training_string(value) {
        tags.push(vec![key.to_string(), value]);
    }
}

fn push_optional_u64_tag(tags: &mut Vec<Vec<String>>, key: &str, value: Option<u64>) {
    if let Some(value) = value {
        tags.push(vec![key.to_string(), value.to_string()]);
    }
}

fn trn_sync_profile(
    work_class: openagents_kernel_core::compute::ComputeTrainingWorkClass,
) -> Option<&'static str> {
    match work_class {
        openagents_kernel_core::compute::ComputeTrainingWorkClass::GroupedReplicaStageExecution
        | openagents_kernel_core::compute::ComputeTrainingWorkClass::FullIslandLocalUpdateTraining => {
            Some("diloco")
        }
        _ => None,
    }
}

fn promotion_state(window: &super::ComputeTrainingWindowPublicationSource) -> Option<&'static str> {
    if window.window.promoted_checkpoint_ref.is_some()
        || matches!(
            window.window.promotion_disposition,
            Some(openagents_kernel_core::compute::ComputeAdapterPromotionDisposition::Promoted)
        )
    {
        Some("accepted")
    } else if window.window.accepted_aggregate_id.is_some()
        || window.window.aggregated_delta_digest.is_some()
    {
        Some("candidate")
    } else {
        None
    }
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
            "backend_families": source.backend_families.iter().cloned().collect::<Vec<_>>(),
            "environment_refs": source.environment_refs.iter().cloned().collect::<Vec<_>>(),
            "benchmark_package_refs": source.benchmark_package_refs.iter().cloned().collect::<Vec<_>>(),
            "training_run_ids": source.training_run_ids,
            "kernel_object_ids": source.training_run_ids,
            "kernel_receipt_ids": source.kernel_receipt_ids,
        }),
        model_family: None,
        window_cadence: None,
        roles: Vec::new(),
        profiles: source.sync_profiles.iter().cloned().collect(),
        address_refs: Vec::new(),
        extra_tags: {
            let mut tags = vec![vec![
                "class".to_string(),
                "training_network_contract".to_string(),
            ]];
            for sync_profile in &source.sync_profiles {
                tags.push(vec!["sync_profile".to_string(), sync_profile.clone()]);
            }
            for aggregation_rule in &source.aggregation_rules {
                tags.push(vec!["aggregation_rule".to_string(), aggregation_rule.clone()]);
            }
            for aggregation_weight_basis in &source.aggregation_weight_bases {
                tags.push(vec![
                    "aggregation_weight".to_string(),
                    aggregation_weight_basis.clone(),
                ]);
            }
            for backend_family in &source.backend_families {
                tags.push(vec!["backend".to_string(), backend_family.clone()]);
            }
            for environment_ref in &source.environment_refs {
                tags.push(vec!["environment".to_string(), environment_ref.clone()]);
            }
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
    let run_definition = source.window.metadata.get("run_definition");
    let mut extra_tags = vec![
        vec!["run".to_string(), source.window.training_run_id.clone()],
        vec!["class".to_string(), "training_window".to_string()],
        vec![
            "work_class".to_string(),
            source.window.work_class.label().to_string(),
        ],
        vec![
            "replica_type".to_string(),
            source.window.replica_type.label().to_string(),
        ],
    ];
    if let Some(sync_profile) = trn_sync_profile(source.window.work_class) {
        extra_tags.push(vec!["sync_profile".to_string(), sync_profile.to_string()]);
    }
    if let Some(run_definition_ref) = run_definition
        .and_then(|value| value.get("run_definition_ref"))
        .and_then(Value::as_str)
    {
        push_optional_tag(&mut extra_tags, "run_definition", run_definition_ref);
    }
    if let Some(round_index) = source.window.round_index {
        extra_tags.push(vec!["round".to_string(), round_index.to_string()]);
    }
    push_optional_tag(
        &mut extra_tags,
        "base_checkpoint",
        source.window.base_checkpoint_ref.as_str(),
    );
    if let Some(planned_local_step_count) = source.window.planned_local_step_count {
        extra_tags.push(vec![
            "planned_local_steps".to_string(),
            planned_local_step_count.to_string(),
        ]);
    }
    if let Some(aggregation_rule) = source.window.aggregation_rule.as_ref() {
        extra_tags.push(vec![
            "aggregation_rule".to_string(),
            aggregation_rule.clone(),
        ]);
    }
    if let Some(aggregation_weight_basis) = source.window.aggregation_weight_basis.as_ref() {
        extra_tags.push(vec![
            "aggregation_weight".to_string(),
            aggregation_weight_basis.clone(),
        ]);
    }
    if let Some(accepted_aggregate_id) = source.window.accepted_aggregate_id.as_ref() {
        extra_tags.push(vec!["aggregate".to_string(), accepted_aggregate_id.clone()]);
    }
    if let Some(promoted_checkpoint_ref) = source.window.promoted_checkpoint_ref.as_ref() {
        extra_tags.push(vec![
            "checkpoint".to_string(),
            promoted_checkpoint_ref.clone(),
        ]);
    }
    if let Some(promotion_state) = promotion_state(source) {
        extra_tags.push(vec!["promotion".to_string(), promotion_state.to_string()]);
    }
    push_optional_tag(&mut extra_tags, "backend", metadata.backend_family.as_str());
    push_optional_tag(
        &mut extra_tags,
        "environment",
        metadata.environment_ref.as_str(),
    );
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
            "environment_ref": optional_training_string(metadata.environment_ref.as_str()),
            "backend_family": optional_training_string(metadata.backend_family.as_str()),
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
            "work_class": source.window.work_class.label(),
            "replica_type": source.window.replica_type.label(),
            "sync_profile": trn_sync_profile(source.window.work_class),
            "round_index": source.window.round_index,
            "base_checkpoint_ref": source.window.base_checkpoint_ref,
            "planned_local_step_count": source.window.planned_local_step_count,
            "aggregation_rule": source.window.aggregation_rule,
            "aggregation_weight_basis": source.window.aggregation_weight_basis,
            "stage_id": source.window.stage_id,
            "contributor_set_revision_id": source.window.contributor_set_revision_id,
            "window_summary_digest": source.window.window_summary_digest,
            "aggregated_delta_digest": source.window.aggregated_delta_digest,
            "accepted_aggregate_id": source.window.accepted_aggregate_id,
            "promoted_checkpoint_ref": source.window.promoted_checkpoint_ref,
            "promotion_state": promotion_state(source),
            "accepted_outcome_id": source.window.accepted_outcome_id,
            "kernel_object_id": source.window.window_id,
            "kernel_receipt_ids": vec![source.receipt_id.clone()],
            "run_definition": run_definition.cloned(),
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
    let mut extra_tags = vec![vec![
        "run".to_string(),
        source.window.training_run_id.clone(),
    ]];
    extra_tags.push(vec![
        "work_class".to_string(),
        source.window.work_class.label().to_string(),
    ]);
    extra_tags.push(vec![
        "replica_type".to_string(),
        source.window.replica_type.label().to_string(),
    ]);
    if let Some(sync_profile) = trn_sync_profile(source.window.work_class) {
        extra_tags.push(vec!["sync_profile".to_string(), sync_profile.to_string()]);
    }
    if let Some(round_index) = source.window.round_index {
        extra_tags.push(vec!["round".to_string(), round_index.to_string()]);
    }
    push_optional_tag(
        &mut extra_tags,
        "base_checkpoint",
        source.window.base_checkpoint_ref.as_str(),
    );
    if let Some(planned_local_step_count) = source.window.planned_local_step_count {
        extra_tags.push(vec![
            "planned_local_steps".to_string(),
            planned_local_step_count.to_string(),
        ]);
    }
    if let Some(aggregation_rule) = source.window.aggregation_rule.as_ref() {
        extra_tags.push(vec![
            "aggregation_rule".to_string(),
            aggregation_rule.clone(),
        ]);
    }
    if let Some(aggregation_weight_basis) = source.window.aggregation_weight_basis.as_ref() {
        extra_tags.push(vec![
            "aggregation_weight".to_string(),
            aggregation_weight_basis.clone(),
        ]);
    }
    if let Some(promotion_state) = promotion_state(source) {
        extra_tags.push(vec!["promotion".to_string(), promotion_state.to_string()]);
    }
    push_optional_tag(&mut extra_tags, "backend", metadata.backend_family.as_str());
    push_optional_tag(
        &mut extra_tags,
        "environment",
        metadata.environment_ref.as_str(),
    );
    Ok(TrainingReceiptEvent {
        network_id: metadata.network_id.clone(),
        window_id: source.window.window_id.clone(),
        status: status.clone(),
        content: json!({
            "network_id": metadata.network_id,
            "training_run_id": source.window.training_run_id,
            "window_id": source.window.window_id,
            "environment_ref": optional_training_string(metadata.environment_ref.as_str()),
            "backend_family": optional_training_string(metadata.backend_family.as_str()),
            "status": status,
            "work_class": source.window.work_class.label(),
            "replica_type": source.window.replica_type.label(),
            "sync_profile": trn_sync_profile(source.window.work_class),
            "round_index": source.window.round_index,
            "base_checkpoint_ref": source.window.base_checkpoint_ref,
            "planned_local_step_count": source.window.planned_local_step_count,
            "aggregation_rule": source.window.aggregation_rule,
            "aggregation_weight_basis": source.window.aggregation_weight_basis,
            "accepted_aggregate_id": source.window.accepted_aggregate_id,
            "promoted_checkpoint_ref": source.window.promoted_checkpoint_ref,
            "promotion_state": promotion_state(source),
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
        extra_tags,
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
            vec![
                "contribution".to_string(),
                contribution.contribution_id.clone(),
            ],
        ],
    }
    .normalize()
}

pub(super) fn local_update_locator_event(
    contribution: &ComputeAdapterContributionOutcome,
    window: &ComputeAdapterTrainingWindow,
    network_id: &str,
    window_receipt_id: &str,
) -> TrainingArtifactLocatorEvent {
    let mut extra_tags = vec![
        vec!["run".to_string(), contribution.training_run_id.clone()],
        vec![
            "base_checkpoint".to_string(),
            contribution.base_checkpoint_ref.clone(),
        ],
    ];
    if let Some(sync_profile) = trn_sync_profile(window.work_class) {
        extra_tags.push(vec!["sync_profile".to_string(), sync_profile.to_string()]);
    }
    if let Some(round_index) = window.round_index {
        extra_tags.push(vec!["round".to_string(), round_index.to_string()]);
    }
    push_optional_u64_tag(
        &mut extra_tags,
        "local_steps",
        contribution.local_step_count,
    );
    push_optional_u64_tag(&mut extra_tags, "tokens", contribution.consumed_token_count);
    push_optional_u64_tag(
        &mut extra_tags,
        "examples",
        contribution.consumed_example_count,
    );
    push_optional_tag(
        &mut extra_tags,
        "aggregation_weight",
        contribution
            .aggregation_weight_basis
            .as_deref()
            .unwrap_or_default(),
    );
    push_optional_u64_tag(
        &mut extra_tags,
        "weight",
        contribution.aggregation_weight_value,
    );
    TrainingArtifactLocatorEvent {
        identifier: contribution.artifact_id.clone(),
        network_id: network_id.to_string(),
        status: contribution.validator_disposition.label().to_string(),
        content: json!({
            "network_id": network_id,
            "training_run_id": contribution.training_run_id,
            "window_id": contribution.window_id,
            "contribution_id": contribution.contribution_id,
            "artifact_role": "local_update",
            "sync_profile": trn_sync_profile(window.work_class),
            "work_class": contribution.work_class.label(),
            "replica_type": contribution.replica_type.label(),
            "base_checkpoint_id": contribution.base_checkpoint_ref,
            "round_index": window.round_index,
            "local_step_count": contribution.local_step_count,
            "consumed_token_count": contribution.consumed_token_count,
            "consumed_example_count": contribution.consumed_example_count,
            "aggregation_weight_basis": contribution.aggregation_weight_basis,
            "aggregation_weight_value": contribution.aggregation_weight_value,
            "validator_disposition": contribution.validator_disposition.label(),
            "kernel_object_id": contribution.contribution_id,
            "kernel_receipt_ids": vec![window_receipt_id],
        }),
        artifact_id: Some(contribution.artifact_id.clone()),
        checkpoint_id: None,
        manifest_digest: Some(contribution.manifest_digest.clone()),
        file_digest: Some(contribution.object_digest.clone()),
        url_hint: None,
        artifact_class: Some("local_update".to_string()),
        window_id: Some(contribution.window_id.clone()),
        policy_revision: Some(contribution.source_policy_revision.revision_id.clone()),
        reason_codes: Vec::new(),
        address_refs: Vec::new(),
        extra_tags,
    }
    .normalize()
}

pub(super) fn aggregate_locator_event(
    source: &super::ComputeTrainingWindowPublicationSource,
    contributions: &[ComputeAdapterContributionOutcome],
) -> Option<TrainingArtifactLocatorEvent> {
    let aggregate_id = source
        .window
        .accepted_aggregate_id
        .clone()
        .unwrap_or_else(|| format!("aggregate.{}", source.window.window_id));
    let file_digest = source.window.aggregated_delta_digest.clone()?;
    let admitted_update_count = contributions
        .iter()
        .filter(|contribution| contribution.accepted_for_aggregation)
        .count();
    let admitted_weight_total = contributions
        .iter()
        .filter(|contribution| contribution.accepted_for_aggregation)
        .filter_map(|contribution| contribution.aggregation_weight_value)
        .sum::<u64>();
    let status = promotion_state(source).unwrap_or("candidate").to_string();
    let mut extra_tags = vec![
        vec!["run".to_string(), source.window.training_run_id.clone()],
        vec!["aggregate".to_string(), aggregate_id.clone()],
        vec![
            "base_checkpoint".to_string(),
            source.window.base_checkpoint_ref.clone(),
        ],
    ];
    if let Some(sync_profile) = trn_sync_profile(source.window.work_class) {
        extra_tags.push(vec!["sync_profile".to_string(), sync_profile.to_string()]);
    }
    if let Some(round_index) = source.window.round_index {
        extra_tags.push(vec!["round".to_string(), round_index.to_string()]);
    }
    push_optional_tag(
        &mut extra_tags,
        "aggregation_rule",
        source
            .window
            .aggregation_rule
            .as_deref()
            .unwrap_or_default(),
    );
    push_optional_tag(
        &mut extra_tags,
        "aggregation_weight",
        source
            .window
            .aggregation_weight_basis
            .as_deref()
            .unwrap_or_default(),
    );
    extra_tags.push(vec!["promotion".to_string(), status.clone()]);
    Some(
        TrainingArtifactLocatorEvent {
            identifier: aggregate_id.clone(),
            network_id: super::training_window_metadata_from_value(&source.window.metadata)
                .ok()?
                .network_id,
            status: status.clone(),
            content: json!({
                "training_run_id": source.window.training_run_id,
                "window_id": source.window.window_id,
                "artifact_role": "aggregate",
                "sync_profile": trn_sync_profile(source.window.work_class),
                "base_checkpoint_id": source.window.base_checkpoint_ref,
                "round_index": source.window.round_index,
                "aggregation_rule": source.window.aggregation_rule,
                "aggregation_weight_basis": source.window.aggregation_weight_basis,
                "admitted_update_count": admitted_update_count,
                "admitted_weight_total": admitted_weight_total,
                "promotion_state": status,
                "promoted_checkpoint_id": source.window.promoted_checkpoint_ref,
                "kernel_object_id": source.window.window_id,
                "kernel_receipt_ids": vec![source.receipt_id.clone()],
            }),
            artifact_id: Some(aggregate_id.clone()),
            checkpoint_id: None,
            manifest_digest: None,
            file_digest: Some(file_digest),
            url_hint: None,
            artifact_class: Some("aggregate".to_string()),
            window_id: Some(source.window.window_id.clone()),
            policy_revision: Some(source.window.source_policy_revision.revision_id.clone()),
            reason_codes: Vec::new(),
            address_refs: Vec::new(),
            extra_tags,
        }
        .normalize(),
    )
}

pub(super) fn checkpoint_locator_event(
    source: &super::ComputeTrainingWindowPublicationSource,
) -> Option<TrainingArtifactLocatorEvent> {
    let checkpoint_pointer = source.window.output_checkpoint_pointer.as_ref()?;
    let checkpoint_id = source
        .window
        .promoted_checkpoint_ref
        .clone()
        .unwrap_or_else(|| checkpoint_pointer.checkpoint_ref.clone());
    let status = promotion_state(source).unwrap_or("accepted").to_string();
    let mut extra_tags = vec![
        vec!["run".to_string(), source.window.training_run_id.clone()],
        vec!["checkpoint".to_string(), checkpoint_id.clone()],
        vec![
            "base_checkpoint".to_string(),
            source.window.base_checkpoint_ref.clone(),
        ],
        vec!["promotion".to_string(), status.clone()],
    ];
    if let Some(sync_profile) = trn_sync_profile(source.window.work_class) {
        extra_tags.push(vec!["sync_profile".to_string(), sync_profile.to_string()]);
    }
    if let Some(round_index) = source.window.round_index {
        extra_tags.push(vec!["round".to_string(), round_index.to_string()]);
    }
    Some(
        TrainingArtifactLocatorEvent {
            identifier: format!("checkpoint.{}", source.window.window_id),
            network_id: super::training_window_metadata_from_value(&source.window.metadata)
                .ok()?
                .network_id,
            status: status.clone(),
            content: json!({
                "training_run_id": source.window.training_run_id,
                "window_id": source.window.window_id,
                "artifact_role": "checkpoint",
                "sync_profile": trn_sync_profile(source.window.work_class),
                "base_checkpoint_id": source.window.base_checkpoint_ref,
                "round_index": source.window.round_index,
                "promotion_state": status,
                "promoted_checkpoint_id": checkpoint_id,
                "kernel_object_id": source.window.window_id,
                "kernel_receipt_ids": vec![source.receipt_id.clone()],
            }),
            artifact_id: None,
            checkpoint_id: Some(checkpoint_id),
            manifest_digest: Some(checkpoint_pointer.manifest_digest.clone()),
            file_digest: Some(checkpoint_pointer.pointer_digest.clone()),
            url_hint: None,
            artifact_class: Some("checkpoint".to_string()),
            window_id: Some(source.window.window_id.clone()),
            policy_revision: Some(source.window.source_policy_revision.revision_id.clone()),
            reason_codes: Vec::new(),
            address_refs: Vec::new(),
            extra_tags,
        }
        .normalize(),
    )
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
    let work_class = source.outcome.metadata.get("work_class").cloned();
    let replica_type = source.outcome.metadata.get("replica_type").cloned();
    let progress_class = source.outcome.metadata.get("progress_class").cloned();
    let payout_projection = source.outcome.metadata.get("payout_projection").cloned();
    let contributor_tiers = source.outcome.metadata.get("contributor_tiers").cloned();
    let mut extra_tags = vec![
        vec!["run".to_string(), source.outcome.source_run_id.clone()],
        vec!["class".to_string(), "training_closeout".to_string()],
    ];
    if let Some(work_class) = work_class.as_ref().and_then(Value::as_str) {
        extra_tags.push(vec!["work_class".to_string(), work_class.to_string()]);
    }
    if let Some(replica_type) = replica_type.as_ref().and_then(Value::as_str) {
        extra_tags.push(vec!["replica_type".to_string(), replica_type.to_string()]);
    }
    if let Some(progress_class) = progress_class.as_ref().and_then(Value::as_str) {
        extra_tags.push(vec![
            "progress_class".to_string(),
            progress_class.to_string(),
        ]);
    }
    if let Some(basis) = payout_projection
        .as_ref()
        .and_then(|value| value.get("basis"))
        .and_then(Value::as_str)
    {
        extra_tags.push(vec!["payout_basis".to_string(), basis.to_string()]);
    }
    if contributor_tiers
        .as_ref()
        .and_then(|value| value.get("weak_device_bearing"))
        .and_then(Value::as_bool)
        == Some(true)
    {
        extra_tags.push(vec!["weak_device_bearing".to_string(), "true".to_string()]);
    }
    if let Some(minimum_tier) = contributor_tiers
        .as_ref()
        .and_then(|value| value.get("minimum_tier"))
        .and_then(Value::as_str)
    {
        extra_tags.push(vec![
            "minimum_contributor_tier".to_string(),
            minimum_tier.to_string(),
        ]);
    }
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
            "work_class": work_class,
            "replica_type": replica_type,
            "progress_class": progress_class,
            "payout_projection": payout_projection,
            "contributor_tiers": contributor_tiers,
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
        extra_tags,
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
        canonical_challenge_status(snapshot.status)
            .label()
            .to_string(),
        result.result_digest.clone(),
        result.challenge_result_ref.clone(),
        json!({
            "network_id": binding.network_id,
            "training_run_id": binding.training_run_id,
            "window_id": binding.window_id,
            "environment_ref": optional_training_string(binding.environment_ref.as_str()),
            "backend_family": optional_training_string(binding.backend_family.as_str()),
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
        vec!["backend".to_string(), binding.backend_family.clone()],
        vec!["environment".to_string(), binding.environment_ref.clone()],
        vec![
            "validator_verdict".to_string(),
            canonical_challenge_verdict(result.verdict)
                .label()
                .to_string(),
        ],
    ]
    .into_iter()
    .filter(|tag| !tag[1].trim().is_empty())
    .collect();
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
        status: canonical_challenge_verdict(result.verdict)
            .label()
            .to_string(),
        content: json!({
            "network_id": binding.network_id,
            "training_run_id": binding.training_run_id,
            "window_id": binding.window_id,
            "environment_ref": optional_training_string(binding.environment_ref.as_str()),
            "backend_family": optional_training_string(binding.backend_family.as_str()),
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
            vec!["backend".to_string(), binding.backend_family.clone()],
            vec!["environment".to_string(), binding.environment_ref.clone()],
            vec![
                "validator_status".to_string(),
                canonical_challenge_status(snapshot.status)
                    .label()
                    .to_string(),
            ],
            vec!["class".to_string(), "validator_verdict".to_string()],
        ]
        .into_iter()
        .filter(|tag| !tag[1].trim().is_empty())
        .collect(),
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
        ValidatorChallengeFailureCode::StaleRetainedBacklog => "stale_retained_backlog",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};

    use openagents_kernel_core::pylon_training::{
        PylonTrainingReputationLabel, PylonTrainingReputationNamespace,
    };
    use openagents_kernel_core::{
        compute::{
            ComputeAcceptedOutcome, ComputeAdapterAggregationEligibility,
            ComputeAdapterCheckpointPointer, ComputeAdapterContributionDisposition,
            ComputeAdapterContributionOutcome, ComputeAdapterPromotionDisposition,
            ComputeAdapterTrainingWindow, ComputeAdapterWindowStatus, ComputeCheckpointBinding,
            ComputeEnvironmentBinding, ComputeTrainingReplicaType, ComputeTrainingRun,
            ComputeTrainingRunStatus, ComputeTrainingSummary, ComputeTrainingWorkClass,
        },
        pylon_training::PylonTrainingReputationRecord,
    };
    use openagents_validator_service::{
        GpuFreivaldsMerkleWitness, ValidatorChallengeContext, ValidatorChallengeLease,
        ValidatorChallengeResult, ValidatorChallengeSnapshot, ValidatorChallengeStatus,
        ValidatorChallengeVerdict,
    };
    use serde_json::json;

    use super::*;

    fn fake_event(template: EventTemplate) -> nostr::Event {
        nostr::Event {
            id: "11".repeat(32),
            pubkey: "22".repeat(32),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags,
            content: template.content,
            sig: "33".repeat(64),
        }
    }

    fn network_contract_source_fixture() -> TrainingTrnNetworkContractSource {
        TrainingTrnNetworkContractSource {
            network_id: "trainnet.alpha".to_string(),
            artifact_bucket_uri: "gs://bucket".to_string(),
            worker_count: 2,
            validator_count: 1,
            recovery_source_count: 1,
            checkpoint_refs: BTreeSet::from(["checkpoint://run.alpha/0001".to_string()]),
            initial_window_ids: BTreeSet::from(["window.0001".to_string()]),
            training_policy_refs: BTreeSet::from(["policy.training.alpha".to_string()]),
            validator_policy_refs: BTreeSet::from(["policy.validator.alpha".to_string()]),
            checkpoint_families: BTreeSet::from(["checkpoint.family.alpha".to_string()]),
            backend_families: BTreeSet::from(["cuda".to_string()]),
            environment_refs: BTreeSet::from(["env.cuda.alpha".to_string()]),
            benchmark_package_refs: BTreeSet::from(["benchmark.alpha".to_string()]),
            sync_profiles: BTreeSet::new(),
            aggregation_rules: BTreeSet::new(),
            aggregation_weight_bases: BTreeSet::new(),
            statuses: BTreeSet::from(["running".to_string()]),
            training_run_ids: vec!["run.alpha".to_string()],
            kernel_receipt_ids: vec!["receipt.network.alpha".to_string()],
        }
    }

    fn training_window_source_fixture() -> ComputeTrainingWindowPublicationSource {
        let metadata = super::super::TrainingWindowMetadata {
            network_id: "trainnet.alpha".to_string(),
            artifact_bucket_uri: "gs://bucket".to_string(),
            environment_ref: "env.cuda.alpha".to_string(),
            backend_family: "cuda".to_string(),
            membership_revision: "members.rev1".to_string(),
            assignment_plans: vec![super::super::TrainingWindowAssignmentPlan {
                assignment_id: "assign.node01.window0001".to_string(),
                node_pubkey_hex: "44".repeat(32),
                contributor_node_id: "node-alpha".to_string(),
                worker_id: "worker-alpha".to_string(),
                dataset_slice: openagents_kernel_core::compute::ComputeAdapterDatasetSlice {
                    dataset_id: "dataset.alpha".to_string(),
                    split_name: "train".to_string(),
                    slice_id: "slice.0001".to_string(),
                    slice_digest: "sha256:slice-alpha".to_string(),
                },
                assignment_seed: "seed.alpha".to_string(),
            }],
            validation: None,
            planned_at_ms: 1_762_491_210_000,
            activated_at_ms: Some(1_762_491_220_000),
            sealed_at_ms: Some(1_762_491_230_000),
            reconciled_at_ms: Some(1_762_491_240_000),
            defensibility: None,
            seal_deadline_ms: 1_762_491_260_000,
        };
        ComputeTrainingWindowPublicationSource {
            window: ComputeAdapterTrainingWindow {
                window_id: "window.0001".to_string(),
                training_run_id: "run.alpha".to_string(),
                stage_id: "stage.alpha".to_string(),
                contributor_set_revision_id: "contributors.rev1".to_string(),
                validator_policy_ref: "policy.validator.alpha".to_string(),
                work_class: ComputeTrainingWorkClass::AdapterTraining,
                replica_type: ComputeTrainingReplicaType::SingleNode,
                round_index: Some(1),
                base_checkpoint_ref: "checkpoint://run.alpha/0001".to_string(),
                planned_local_step_count: Some(64),
                aggregation_rule: Some("weighted_avg".to_string()),
                aggregation_weight_basis: Some("tokens".to_string()),
                adapter_target_id: "adapter.target.alpha".to_string(),
                adapter_family: "lora".to_string(),
                base_model_ref: "model://base.alpha".to_string(),
                adapter_format: "safetensors".to_string(),
                source_policy_revision:
                    openagents_kernel_core::compute::ComputeAdapterPolicyRevision {
                        policy_family: "policy.family.alpha".to_string(),
                        revision_id: "policy.rev.alpha".to_string(),
                        revision_number: Some(1),
                        policy_digest: "sha256:policy-alpha".to_string(),
                        parent_revision_id: None,
                        produced_at_ms: 1_762_491_200_000,
                    },
                source_checkpoint_pointer: ComputeAdapterCheckpointPointer {
                    scope_kind: "run".to_string(),
                    scope_id: "run.alpha".to_string(),
                    checkpoint_family: "checkpoint.family.alpha".to_string(),
                    checkpoint_ref: "checkpoint://run.alpha/0001".to_string(),
                    manifest_digest: "sha256:checkpoint-manifest-alpha".to_string(),
                    updated_at_ms: 1_762_491_200_000,
                    pointer_digest: "sha256:pointer-alpha".to_string(),
                },
                status: ComputeAdapterWindowStatus::Reconciled,
                total_contributions: 1,
                admitted_contributions: 1,
                accepted_contributions: 1,
                quarantined_contributions: 0,
                rejected_contributions: 0,
                replay_required_contributions: 0,
                replay_checked_contributions: 1,
                held_out_average_score_bps: Some(9_500),
                benchmark_pass_rate_bps: Some(9_700),
                runtime_smoke_passed: Some(true),
                promotion_ready: true,
                gate_reason_codes: Vec::new(),
                window_summary_digest: "sha256:window-summary-alpha".to_string(),
                promotion_disposition: None,
                hold_reason_codes: Vec::new(),
                aggregated_delta_digest: Some("sha256:aggregate-alpha".to_string()),
                accepted_aggregate_id: Some("aggregate.window.0001".to_string()),
                output_policy_revision: None,
                output_checkpoint_pointer: None,
                promoted_checkpoint_ref: None,
                accepted_outcome_id: Some("accepted.training_window.window.0001".to_string()),
                recorded_at_ms: 1_762_491_240_000,
                metadata: super::super::training_window_metadata_value(&metadata),
            },
            receipt_id: "receipt.window.alpha".to_string(),
        }
    }

    fn accepted_outcome_source_fixture() -> ComputeAcceptedOutcomePublicationSource {
        let training_run = ComputeTrainingRun {
            training_run_id: "run.alpha".to_string(),
            training_policy_ref: "policy.training.alpha".to_string(),
            environment_binding: ComputeEnvironmentBinding {
                environment_ref: "env.cuda.alpha".to_string(),
                environment_version: Some("v1".to_string()),
                dataset_ref: Some("dataset://trainnet.alpha/shard-0001".to_string()),
                rubric_ref: None,
                evaluator_policy_ref: None,
            },
            checkpoint_binding: ComputeCheckpointBinding {
                checkpoint_family: "checkpoint.family.alpha".to_string(),
                latest_checkpoint_ref: Some("checkpoint://run.alpha/0002".to_string()),
                recovery_posture: Some("resume_from_latest".to_string()),
            },
            validator_policy_ref: "policy.validator.alpha".to_string(),
            work_class: ComputeTrainingWorkClass::FullIslandLocalUpdateTraining,
            replica_type: ComputeTrainingReplicaType::Island,
            benchmark_package_refs: vec!["benchmark.alpha".to_string()],
            product_id: Some("psionic.training.gradient.elastic".to_string()),
            capacity_lot_id: None,
            instrument_id: None,
            delivery_proof_id: None,
            model_ref: Some("model://base.alpha".to_string()),
            source_ref: None,
            rollout_verification_eval_run_ids: Vec::new(),
            created_at_ms: 1_762_491_200_000,
            started_at_ms: Some(1_762_491_200_100),
            finalized_at_ms: None,
            expected_step_count: Some(1024),
            completed_step_count: Some(128),
            status: ComputeTrainingRunStatus::Accepted,
            final_checkpoint_ref: Some("checkpoint://run.alpha/0002".to_string()),
            promotion_checkpoint_ref: Some("checkpoint://run.alpha/0002".to_string()),
            summary: Some(ComputeTrainingSummary {
                completed_step_count: Some(128),
                processed_token_count: Some(8_192),
                average_loss: Some(0.12),
                best_eval_score_bps: Some(9_910),
                accepted_checkpoint_ref: Some("checkpoint://run.alpha/0002".to_string()),
                aggregate_metrics: Vec::new(),
                artifacts: Vec::new(),
            }),
            metadata: json!({ "network_id": "trainnet.alpha" }),
        };
        ComputeAcceptedOutcomePublicationSource {
            outcome: ComputeAcceptedOutcome::from_training_run(
                "accepted.training_window.window.0001",
                1_762_491_250_000,
                &training_run,
                json!({
                    "network_id": "trainnet.alpha",
                    "window_id": "window.0001",
                    "closeout_status": "rewarded",
                    "payout_eligible": true,
                    "work_class": "full_island_local_update_training",
                    "replica_type": "island",
                    "progress_class": "model_update",
                "payout_projection": {
                    "basis": "aggregation_weight",
                    "weight_basis": "tokens",
                    "total_weight_value": 131072,
                    "weighted": true,
                        "shared_result": false,
                        "progress_bearing": true,
                        "participant_count": 1,
                        "progress_participant_count": 1,
                        "participants": [
                            {
                                "contributor_node_id": "node-alpha",
                                "worker_id": "worker-alpha",
                                "contribution_id": "contrib.accepted.alpha",
                                "assignment_id": "assign.node01.window0001",
                                "stage_id": "stage.alpha",
                                "share_bps": 10000,
                                "weight_basis": "tokens",
                                "weight_value": 131072,
                                "progress_credit": true,
                                "validator_disposition": "accepted"
                            }
                        ]
                    },
                    "contributor_tiers": {
                        "weak_device_bearing": true,
                        "minimum_tier": "tier2_trainer",
                        "maximum_tier": "tier3_island",
                        "tiers": [
                            {
                                "tier": "tier2_trainer",
                                "participant_count": 1
                            },
                            {
                                "tier": "tier3_island",
                                "participant_count": 1
                            }
                        ]
                    }
                }),
            ),
            receipt_id: "receipt.closeout.alpha".to_string(),
        }
    }

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
            backend_family: "cuda".to_string(),
            environment_ref: "env.cuda.alpha".to_string(),
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
        assert_eq!(
            label_event.labels[0].namespace.as_deref(),
            Some("trn/validator")
        );
        assert_eq!(label_event.labels[0].value, "poor");
        assert_eq!(label_event.targets.len(), 3);
    }

    #[test]
    fn coordinator_training_trn_mapping_roundtrips_network_window_closeout_and_score_events() {
        let network = network_contract_event(&network_contract_source_fixture()).expect("network");
        assert!(
            network
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["backend".to_string(), "cuda".to_string()])
        );
        assert!(
            network
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["environment".to_string(), "env.cuda.alpha".to_string()])
        );
        let (template, fingerprint) =
            event_template_and_fingerprint(&TrnEvent::NetworkContract(network.clone()))
                .expect("fingerprint");
        assert_eq!(template.kind, nostr::KIND_TRAINING_NETWORK_CONTRACT);
        assert!(fingerprint.starts_with("sha256:"));
        assert!(matches!(
            nostr::TrnEvent::from_event(&fake_event(template)).expect("parsed network"),
            nostr::TrnEvent::NetworkContract(event) if event == network
        ));

        let window_source = training_window_source_fixture();
        let closeout_status_by_outcome_id = HashMap::from([(
            "accepted.training_window.window.0001".to_string(),
            "rewarded".to_string(),
        )]);
        let window = window_event(&window_source, &closeout_status_by_outcome_id).expect("window");
        assert!(
            window
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["backend".to_string(), "cuda".to_string()])
        );
        assert!(
            window
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["environment".to_string(), "env.cuda.alpha".to_string()])
        );
        assert!(
            window
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["round".to_string(), "1".to_string()])
        );
        assert!(window.extra_tags.iter().any(|tag| {
            tag == &vec![
                "base_checkpoint".to_string(),
                "checkpoint://run.alpha/0001".to_string(),
            ]
        }));
        assert!(window.extra_tags.iter().any(|tag| {
            tag == &vec!["aggregation_rule".to_string(), "weighted_avg".to_string()]
        }));
        assert_eq!(
            window
                .content
                .get("base_checkpoint_ref")
                .and_then(serde_json::Value::as_str),
            Some("checkpoint://run.alpha/0001")
        );
        assert!(
            window
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["closeout".to_string(), "rewarded".to_string()])
        );
        assert!(matches!(
            nostr::TrnEvent::from_event(&fake_event(
                window.to_event_template(1_774_160_020).expect("window template"),
            ))
            .expect("parsed window"),
            nostr::TrnEvent::Window(event) if event == window
        ));

        let receipt = window_receipt_event(&window_source).expect("window receipt");
        assert_eq!(receipt.status, "reconciled");
        assert_eq!(
            receipt.classes,
            vec!["coordinator_window_state".to_string()]
        );
        assert!(
            receipt
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["backend".to_string(), "cuda".to_string()])
        );
        assert!(
            receipt
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["environment".to_string(), "env.cuda.alpha".to_string()])
        );
        assert!(
            receipt
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["round".to_string(), "1".to_string()])
        );
        assert!(matches!(
            nostr::TrnEvent::from_event(&fake_event(
                receipt.to_event_template(1_774_160_021).expect("receipt template"),
            ))
            .expect("parsed receipt"),
            nostr::TrnEvent::Receipt(event) if event == receipt
        ));

        let replay_required = replay_required_receipt_event(
            &ComputeAdapterContributionOutcome {
                contribution_id: "contrib.alpha".to_string(),
                training_run_id: "run.alpha".to_string(),
                stage_id: "stage.alpha".to_string(),
                window_id: "window.0001".to_string(),
                contributor_set_revision_id: "contributors.rev1".to_string(),
                assignment_id: "assign.node01.window0001".to_string(),
                contributor_node_id: "node-alpha".to_string(),
                worker_id: "worker-alpha".to_string(),
                validator_policy_ref: "policy.validator.alpha".to_string(),
                work_class: ComputeTrainingWorkClass::AdapterTraining,
                replica_type: ComputeTrainingReplicaType::SingleNode,
                base_checkpoint_ref: "checkpoint://run.alpha/0001".to_string(),
                adapter_target_id: "adapter.target.alpha".to_string(),
                adapter_family: "lora".to_string(),
                base_model_ref: "model://base.alpha".to_string(),
                adapter_format: "safetensors".to_string(),
                dataset_slice: openagents_kernel_core::compute::ComputeAdapterDatasetSlice {
                    dataset_id: "dataset.alpha".to_string(),
                    split_name: "train".to_string(),
                    slice_id: "slice.0001".to_string(),
                    slice_digest: "sha256:slice-alpha".to_string(),
                },
                source_policy_revision:
                    openagents_kernel_core::compute::ComputeAdapterPolicyRevision {
                        policy_family: "policy.family.alpha".to_string(),
                        revision_id: "policy.rev.alpha".to_string(),
                        revision_number: Some(1),
                        policy_digest: "sha256:policy-alpha".to_string(),
                        parent_revision_id: None,
                        produced_at_ms: 1_762_491_200_000,
                    },
                source_checkpoint_pointer: ComputeAdapterCheckpointPointer {
                    scope_kind: "run".to_string(),
                    scope_id: "run.alpha".to_string(),
                    checkpoint_family: "checkpoint.family.alpha".to_string(),
                    checkpoint_ref: "checkpoint://run.alpha/0001".to_string(),
                    manifest_digest: "sha256:checkpoint-manifest-alpha".to_string(),
                    updated_at_ms: 1_762_491_200_000,
                    pointer_digest: "sha256:pointer-alpha".to_string(),
                },
                submission_receipt_digest: "sha256:submit-alpha".to_string(),
                artifact_id: "artifact.delta.alpha".to_string(),
                manifest_digest: "sha256:manifest-alpha".to_string(),
                object_digest: "sha256:object-alpha".to_string(),
                artifact_receipt_digest: "sha256:artifact-receipt-alpha".to_string(),
                provenance_bundle_digest: "sha256:prov-alpha".to_string(),
                security_receipt_digest: "sha256:sec-alpha".to_string(),
                replay_receipt_digest: None,
                validator_disposition: Default::default(),
                validation_reason_codes: Vec::new(),
                validator_receipt_digest: "sha256:validator-alpha".to_string(),
                aggregation_eligibility: Default::default(),
                accepted_for_aggregation: false,
                local_step_count: None,
                consumed_token_count: None,
                consumed_example_count: None,
                aggregation_weight_basis: None,
                aggregation_weight_value: None,
                aggregation_weight_bps: None,
                promotion_receipt_digest: None,
                recorded_at_ms: 1_762_491_240_000,
                metadata: json!({}),
            },
            "trainnet.alpha",
            "receipt.window.alpha",
        );
        assert_eq!(replay_required.status, "replay_requested");
        assert!(
            replay_required
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["contribution".to_string(), "contrib.alpha".to_string()])
        );

        let mut diloco_window_source = training_window_source_fixture();
        diloco_window_source.window.work_class =
            ComputeTrainingWorkClass::FullIslandLocalUpdateTraining;
        diloco_window_source.window.replica_type = ComputeTrainingReplicaType::Island;
        diloco_window_source.window.promotion_disposition =
            Some(ComputeAdapterPromotionDisposition::Promoted);
        diloco_window_source.window.promoted_checkpoint_ref =
            Some("checkpoint://run.alpha/0002".to_string());
        diloco_window_source.window.output_checkpoint_pointer =
            Some(ComputeAdapterCheckpointPointer {
                scope_kind: "run".to_string(),
                scope_id: "run.alpha".to_string(),
                checkpoint_family: "checkpoint.family.alpha".to_string(),
                checkpoint_ref: "checkpoint://run.alpha/0002".to_string(),
                manifest_digest: "sha256:checkpoint-manifest-beta".to_string(),
                updated_at_ms: 1_762_491_250_000,
                pointer_digest: "sha256:pointer-beta".to_string(),
            });
        let accepted_contribution = ComputeAdapterContributionOutcome {
            contribution_id: "contrib.accepted.alpha".to_string(),
            training_run_id: "run.alpha".to_string(),
            stage_id: "stage.alpha".to_string(),
            window_id: "window.0001".to_string(),
            contributor_set_revision_id: "contributors.rev1".to_string(),
            assignment_id: "assign.node01.window0001".to_string(),
            contributor_node_id: "node-alpha".to_string(),
            worker_id: "worker-alpha".to_string(),
            validator_policy_ref: "policy.validator.alpha".to_string(),
            work_class: ComputeTrainingWorkClass::FullIslandLocalUpdateTraining,
            replica_type: ComputeTrainingReplicaType::Island,
            base_checkpoint_ref: "checkpoint://run.alpha/0001".to_string(),
            adapter_target_id: String::new(),
            adapter_family: String::new(),
            base_model_ref: "model://base.alpha".to_string(),
            adapter_format: String::new(),
            dataset_slice: openagents_kernel_core::compute::ComputeAdapterDatasetSlice::default(),
            source_policy_revision: openagents_kernel_core::compute::ComputeAdapterPolicyRevision {
                policy_family: "policy.family.alpha".to_string(),
                revision_id: "policy.rev.alpha".to_string(),
                revision_number: Some(1),
                policy_digest: "sha256:policy-alpha".to_string(),
                parent_revision_id: None,
                produced_at_ms: 1_762_491_200_000,
            },
            source_checkpoint_pointer: ComputeAdapterCheckpointPointer {
                scope_kind: "run".to_string(),
                scope_id: "run.alpha".to_string(),
                checkpoint_family: "checkpoint.family.alpha".to_string(),
                checkpoint_ref: "checkpoint://run.alpha/0001".to_string(),
                manifest_digest: "sha256:checkpoint-manifest-alpha".to_string(),
                updated_at_ms: 1_762_491_200_000,
                pointer_digest: "sha256:pointer-alpha".to_string(),
            },
            submission_receipt_digest: "sha256:submit-accepted-alpha".to_string(),
            artifact_id: "artifact.local-update.alpha".to_string(),
            manifest_digest: "sha256:manifest-accepted-alpha".to_string(),
            object_digest: "sha256:object-accepted-alpha".to_string(),
            artifact_receipt_digest: "sha256:artifact-receipt-accepted-alpha".to_string(),
            provenance_bundle_digest: "sha256:prov-accepted-alpha".to_string(),
            security_receipt_digest: "sha256:sec-accepted-alpha".to_string(),
            replay_receipt_digest: None,
            validator_disposition: ComputeAdapterContributionDisposition::Accepted,
            validation_reason_codes: Vec::new(),
            validator_receipt_digest: "sha256:validator-accepted-alpha".to_string(),
            aggregation_eligibility: ComputeAdapterAggregationEligibility::Eligible,
            accepted_for_aggregation: true,
            local_step_count: Some(64),
            consumed_token_count: Some(131_072),
            consumed_example_count: Some(256),
            aggregation_weight_basis: Some("tokens".to_string()),
            aggregation_weight_value: Some(131_072),
            aggregation_weight_bps: Some(10_000),
            promotion_receipt_digest: None,
            recorded_at_ms: 1_762_491_240_000,
            metadata: json!({}),
        };
        let local_update = local_update_locator_event(
            &accepted_contribution,
            &diloco_window_source.window,
            "trainnet.alpha",
            "receipt.window.alpha",
        );
        assert_eq!(local_update.artifact_class.as_deref(), Some("local_update"));
        assert!(
            local_update
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["sync_profile".to_string(), "diloco".to_string()])
        );
        assert!(
            local_update
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["weight".to_string(), "131072".to_string()])
        );

        let aggregate = aggregate_locator_event(
            &diloco_window_source,
            std::slice::from_ref(&accepted_contribution),
        )
        .expect("aggregate locator");
        assert_eq!(aggregate.artifact_class.as_deref(), Some("aggregate"));
        assert!(
            aggregate
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["promotion".to_string(), "accepted".to_string()])
        );

        let checkpoint = checkpoint_locator_event(&diloco_window_source).expect("checkpoint");
        assert_eq!(checkpoint.artifact_class.as_deref(), Some("checkpoint"));
        assert_eq!(
            checkpoint.checkpoint_id.as_deref(),
            Some("checkpoint://run.alpha/0002")
        );

        let closeout = closeout_event(&accepted_outcome_source_fixture()).expect("closeout");
        assert_eq!(closeout.status, "rewarded");
        assert_eq!(
            closeout
                .content
                .get("work_class")
                .and_then(serde_json::Value::as_str),
            Some("full_island_local_update_training")
        );
        assert!(
            closeout
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["progress_class".to_string(), "model_update".to_string()])
        );
        assert!(
            closeout
                .extra_tags
                .iter()
                .any(|tag| tag
                    == &vec!["payout_basis".to_string(), "aggregation_weight".to_string()])
        );
        assert!(
            closeout
                .extra_tags
                .iter()
                .any(|tag| { tag == &vec!["weak_device_bearing".to_string(), "true".to_string()] })
        );
        assert!(closeout.extra_tags.iter().any(|tag| {
            tag == &vec![
                "minimum_contributor_tier".to_string(),
                "tier2_trainer".to_string(),
            ]
        }));
        assert_eq!(
            closeout
                .content
                .get("contributor_tiers")
                .and_then(|value| value.get("weak_device_bearing"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert!(matches!(
            nostr::TrnEvent::from_event(&fake_event(
                closeout.to_event_template(1_774_160_022).expect("closeout template"),
            ))
            .expect("parsed closeout"),
            nostr::TrnEvent::Closeout(event) if event == closeout
        ));

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
            backend_family: "cuda".to_string(),
            environment_ref: "env.cuda.alpha".to_string(),
            challenge_kind: "aggregate".to_string(),
        };
        let result = snapshot.final_result.as_ref().expect("final result");
        let score_locator =
            validator_score_locator_event(&snapshot, result, &binding, "receipt.challenge.alpha");
        assert_eq!(score_locator.artifact_class.as_deref(), Some("score"));
        assert!(
            score_locator
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["backend".to_string(), "cuda".to_string()])
        );
        assert!(
            score_locator
                .extra_tags
                .iter()
                .any(|tag| tag == &vec!["environment".to_string(), "env.cuda.alpha".to_string()])
        );
        assert!(matches!(
            nostr::TrnEvent::from_event(&fake_event(
                score_locator
                    .to_event_template(1_774_160_023)
                    .expect("score locator template"),
            ))
            .expect("parsed score locator"),
            nostr::TrnEvent::ArtifactLocator(event) if event == score_locator
        ));
    }
}
