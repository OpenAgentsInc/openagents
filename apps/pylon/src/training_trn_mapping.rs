use std::collections::BTreeSet;

use anyhow::{Context, Result, anyhow};
use nostr::{
    TrainingArtifactLocatorEvent, TrainingNodeRecordEvent, TrainingReceiptEvent, TrnAddressReference,
    TrnCapability, TrnPubkeyReference,
};
use serde_json::{Map, Value, json};

use super::{
    NostrIdentity, PylonConfig, PylonTrainingArtifactObjectTransferReport,
    PylonTrainingRuntimeState, TrainingManifestInspectionContext, dedup_training_relay_urls,
    training_artifact_checkpoint_tag, training_assignment_reason,
    training_backend_family_label, training_checkpoint_serve_url,
    training_expected_artifact_class_for_role, training_manifest_role_label,
    training_node_record_status, training_observability_context, training_window_coordinate,
};

fn relay_hint_value(relay_url: &str) -> Option<String> {
    let trimmed = relay_url.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub(super) fn node_record_event(
    config: &PylonConfig,
    state: &PylonTrainingRuntimeState,
    contexts: &[&TrainingManifestInspectionContext],
) -> Result<(String, TrainingNodeRecordEvent)> {
    let network_id = contexts
        .first()
        .map(|context| context.manifest.network_id.clone())
        .ok_or_else(|| anyhow!("training node record requires at least one manifest"))?;
    let status = training_node_record_status(state, contexts);
    let roles = contexts
        .iter()
        .map(|context| training_manifest_role_label(context.manifest.role).to_string())
        .chain(
            config
                .training
                .role_claims
                .iter()
                .map(|role| role.label().to_string()),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut capabilities = Vec::new();
    for backend_family in contexts
        .iter()
        .map(|context| training_backend_family_label(context.manifest.topology.backend_family))
        .collect::<BTreeSet<_>>()
    {
        capabilities.push(TrnCapability::new("backend", backend_family));
    }
    for checkpoint_family in contexts
        .iter()
        .map(|context| context.manifest.checkpoint.checkpoint_family.clone())
        .collect::<BTreeSet<_>>()
    {
        capabilities.push(TrnCapability::new("checkpoint_family", checkpoint_family));
    }
    for environment_ref in contexts
        .iter()
        .map(|context| context.manifest.environment_ref.clone())
        .collect::<BTreeSet<_>>()
    {
        capabilities.push(TrnCapability::new("environment", environment_ref));
    }
    for world_size in contexts
        .iter()
        .map(|context| context.manifest.topology.world_size)
        .collect::<BTreeSet<_>>()
    {
        capabilities.push(TrnCapability::new("world_size", world_size.to_string()));
    }
    let active_runtime = state
        .active_runtime
        .as_ref()
        .filter(|runtime| {
            contexts.iter().any(|context| {
                context.manifest.run_id == runtime.training_run_id
                    && context.manifest.window_id == runtime.window_id
            })
        })
        .map(|runtime| {
            json!({
                "training_run_id": runtime.training_run_id,
                "window_id": runtime.window_id,
                "assignment_id": runtime.assignment_id,
                "lease_id": runtime.lease_id,
                "membership_revision": runtime.membership_revision,
                "role": runtime.role.label(),
                "desired_state": format!("{:?}", runtime.desired_state).to_ascii_lowercase(),
                "process_state": format!("{:?}", runtime.process_state).to_ascii_lowercase(),
                "manifest_path": runtime.manifest_path,
            })
        });
    let content = json!({
        "node_label": config.node_label,
        "software_version": env!("CARGO_PKG_VERSION"),
        "checkpoint_serve_url": training_checkpoint_serve_url(config),
        "manifest_digests": contexts
            .iter()
            .map(|context| context.manifest.manifest_digest.clone())
            .collect::<Vec<_>>(),
        "active_runtime": active_runtime,
    });
    Ok((
        status.to_string(),
        TrainingNodeRecordEvent {
            identifier: network_id.clone(),
            network_id,
            status: status.to_string(),
            content,
            roles,
            classes: vec!["psionic_train".to_string()],
            build_digest: None,
            capabilities,
            relay_urls: dedup_training_relay_urls(
                &contexts
                    .iter()
                    .map(|context| (*context).clone())
                    .collect::<Vec<_>>(),
            ),
            extra_tags: Vec::new(),
        }
        .normalize(),
    ))
}

pub(super) fn assignment_ack_event(
    identity: &NostrIdentity,
    context: &TrainingManifestInspectionContext,
    relay_hint: &str,
) -> Result<TrainingReceiptEvent> {
    let mut content = Map::new();
    content.insert(
        "subject_pubkey".to_string(),
        Value::String(identity.public_key_hex.clone()),
    );
    content.insert(
        "assignment_deadline_unix".to_string(),
        Value::from(context.manifest.expires_at_ms / 1000),
    );
    content.insert(
        "expected_artifact_class".to_string(),
        Value::String(training_expected_artifact_class_for_role(context.manifest.role).to_string()),
    );
    if let Some(dataset) = context.manifest.dataset.as_ref() {
        content.insert(
            "shard_digest".to_string(),
            Value::String(dataset.slice_digest.clone()),
        );
    }
    if let Some(validator) = context.manifest.validator.as_ref() {
        content.insert(
            "sample_pool_digest".to_string(),
            Value::String(
                validator
                    .expected_manifest_digests
                    .first()
                    .cloned()
                    .unwrap_or_else(|| validator.challenge_id.clone()),
            ),
        );
    }
    content.insert(
        "source_checkpoint_id".to_string(),
        Value::String(context.manifest.checkpoint.checkpoint_ref.clone()),
    );
    content.insert(
        "manifest_digest".to_string(),
        Value::String(context.manifest.manifest_digest.clone()),
    );
    content.insert(
        "membership_revision".to_string(),
        Value::String(context.manifest.membership_revision.clone()),
    );
    content.insert(
        "observability".to_string(),
        serde_json::to_value(training_observability_context(&context.manifest))
            .context("failed to encode assignment observability context")?,
    );
    let mut reason_codes = Vec::new();
    if let Some(reason) = training_assignment_reason(context) {
        reason_codes.push(reason);
    }
    Ok(TrainingReceiptEvent {
        network_id: context.manifest.network_id.clone(),
        window_id: context.manifest.window_id.clone(),
        status: "assignment_accepted".to_string(),
        content: Value::Object(content),
        assignment_id: Some(context.manifest.assignment_id.clone()),
        policy_revision: Some(context.manifest.training_policy_ref.clone()),
        role: Some(training_manifest_role_label(context.manifest.role).to_string()),
        artifact_id: None,
        checkpoint_id: Some(context.manifest.checkpoint.checkpoint_ref.clone()),
        actors: vec![
            TrnPubkeyReference::new(
                identity.public_key_hex.clone(),
                relay_hint_value(relay_hint),
                Some("subject".to_string()),
            ),
            TrnPubkeyReference::new(
                context.manifest.coordinator_pubkey.clone(),
                relay_hint_value(relay_hint),
                Some("coordinator".to_string()),
            ),
        ],
        reason_codes,
        classes: vec![training_expected_artifact_class_for_role(context.manifest.role).to_string()],
        address_refs: vec![TrnAddressReference::new(
            training_window_coordinate(&context.manifest),
            relay_hint_value(relay_hint),
            Some("window".to_string()),
        )],
        event_refs: Vec::new(),
        extra_tags: Vec::new(),
    }
    .normalize())
}

pub(super) fn artifact_locator_event(
    context: &TrainingManifestInspectionContext,
    bundle_id: &str,
    artifact_id: &str,
    artifact_class: &str,
    object: &PylonTrainingArtifactObjectTransferReport,
) -> TrainingArtifactLocatorEvent {
    TrainingArtifactLocatorEvent {
        identifier: artifact_id.to_string(),
        network_id: context.manifest.network_id.clone(),
        status: "staged".to_string(),
        content: json!({
            "bundle_id": bundle_id,
            "object_uri": object.object_uri,
            "local_path": object.local_path,
            "size_bytes": object.size_bytes,
            "digest_verified": object.digest_verified,
            "observability": training_observability_context(&context.manifest),
        }),
        artifact_id: Some(artifact_id.to_string()),
        checkpoint_id: training_artifact_checkpoint_tag(&context.manifest, artifact_class),
        manifest_digest: Some(context.manifest.manifest_digest.clone()),
        file_digest: Some(object.digest.clone()),
        url_hint: Some(object.object_uri.clone()),
        artifact_class: Some(artifact_class.to_string()),
        window_id: Some(context.manifest.window_id.clone()),
        policy_revision: Some(context.manifest.training_policy_ref.clone()),
        reason_codes: Vec::new(),
        address_refs: Vec::new(),
        extra_tags: Vec::new(),
    }
    .normalize()
}

pub(super) fn artifact_uploaded_receipt_event(
    identity: &NostrIdentity,
    context: &TrainingManifestInspectionContext,
    bundle_id: &str,
    artifact_id: &str,
    artifact_class: &str,
    locator_a_ref: &str,
    relay_hint: &str,
    object: &PylonTrainingArtifactObjectTransferReport,
) -> TrainingReceiptEvent {
    TrainingReceiptEvent {
        network_id: context.manifest.network_id.clone(),
        window_id: context.manifest.window_id.clone(),
        status: "artifact_uploaded".to_string(),
        content: json!({
            "artifact_digest": object.digest,
            "bundle_id": bundle_id,
            "object_uri": object.object_uri,
            "local_path": object.local_path,
            "manifest_digest": context.manifest.manifest_digest,
            "observability": training_observability_context(&context.manifest),
        }),
        assignment_id: Some(context.manifest.assignment_id.clone()),
        policy_revision: Some(context.manifest.training_policy_ref.clone()),
        role: Some(training_manifest_role_label(context.manifest.role).to_string()),
        artifact_id: Some(artifact_id.to_string()),
        checkpoint_id: training_artifact_checkpoint_tag(&context.manifest, artifact_class),
        actors: vec![
            TrnPubkeyReference::new(
                identity.public_key_hex.clone(),
                relay_hint_value(relay_hint),
                Some("subject".to_string()),
            ),
            TrnPubkeyReference::new(
                context.manifest.coordinator_pubkey.clone(),
                relay_hint_value(relay_hint),
                Some("coordinator".to_string()),
            ),
        ],
        reason_codes: Vec::new(),
        classes: vec![artifact_class.to_string()],
        address_refs: vec![
            TrnAddressReference::new(
                locator_a_ref.to_string(),
                relay_hint_value(relay_hint),
                Some("source".to_string()),
            ),
            TrnAddressReference::new(
                training_window_coordinate(&context.manifest),
                relay_hint_value(relay_hint),
                Some("window".to_string()),
            ),
        ],
        event_refs: Vec::new(),
        extra_tags: Vec::new(),
    }
    .normalize()
}
