use std::collections::BTreeSet;

use anyhow::{Context, Result, anyhow};
use nostr::{
    TrainingArtifactLocatorEvent, TrainingNodeRecordEvent, TrainingReceiptEvent,
    TrnAddressReference, TrnCapability, TrnPubkeyReference,
};
use serde_json::{Map, Value, json};

use super::{
    NostrIdentity, PylonConfig, PylonTrainingArtifactObjectTransferReport,
    PylonTrainingRuntimeState, TrainingManifestInspectionContext, dedup_training_relay_urls,
    local_training_build_digest, local_training_release_id, training_artifact_checkpoint_tag,
    training_assignment_reason, training_backend_family_label, training_checkpoint_serve_url,
    training_expected_artifact_class_for_role, training_manifest_role_label,
    training_node_record_status, training_observability_context, training_settlement_destination,
    training_window_coordinate,
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
        "release_id": local_training_release_id(),
        "software_version": env!("CARGO_PKG_VERSION"),
        "build_digest": local_training_build_digest(),
        "settlement_destination": training_settlement_destination(config),
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
            build_digest: Some(local_training_build_digest()),
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

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use nostr::{Event, NostrIdentity, TrnEvent};
    use openagents_kernel_core::pylon_training::{
        PYLON_TRAINING_GCS_CREDENTIAL_SOURCE, PylonTrainingArtifactLayout, PylonTrainingArtifacts,
        PylonTrainingCheckpointBinding, PylonTrainingCollectiveKind,
        PylonTrainingDatasetAssignment, PylonTrainingElasticBoundary, PylonTrainingManifestRole,
        PylonTrainingRunManifestCommon, PylonTrainingRunManifestV1, PylonTrainingTopology,
        PylonTrainingTopologyBackendFamily, PylonTrainingTrn,
    };

    use super::*;

    fn fake_event(template: nostr::EventTemplate) -> Event {
        Event {
            id: "11".repeat(32),
            pubkey: "22".repeat(32),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags,
            content: template.content,
            sig: "33".repeat(64),
        }
    }

    fn identity_fixture(temp_dir: &tempfile::TempDir) -> NostrIdentity {
        NostrIdentity {
            identity_path: temp_dir.path().join("identity.mnemonic"),
            mnemonic: "legal winner thank year wave sausage worth useful legal winner thank yellow"
                .to_string(),
            npub: "npub1test".to_string(),
            nsec: "nsec1test".to_string(),
            public_key_hex: "44".repeat(32),
            private_key_hex: "55".repeat(32),
        }
    }

    fn worker_manifest_context() -> Result<(tempfile::TempDir, TrainingManifestInspectionContext)> {
        let temp_dir = tempfile::tempdir()?;
        let local_run_root = temp_dir
            .path()
            .join("training")
            .join("runs")
            .join("run.alpha");
        let common = PylonTrainingRunManifestCommon {
            manifest_id: "manifest.run.alpha.worker".to_string(),
            issued_at_ms: 1_762_491_200_000,
            expires_at_ms: 1_762_491_800_000,
            network_id: "trainnet.alpha".to_string(),
            run_id: "run.alpha".to_string(),
            window_id: "window.0001".to_string(),
            assignment_id: "assign.node01.window0001".to_string(),
            lease_id: "lease.node01.window0001".to_string(),
            lease_sequence: 1,
            membership_revision: "members.rev1".to_string(),
            node_pubkey: "44".repeat(32),
            coordinator_pubkey: "22".repeat(32),
            authority_base_url: "https://nexus.openagents.com".to_string(),
            training_policy_ref: "policy.training.alpha".to_string(),
            validator_policy_ref: "policy.validator.alpha".to_string(),
            environment_ref: "env.cuda.alpha".to_string(),
            environment_version: "v1".to_string(),
        };
        let topology = PylonTrainingTopology {
            backend_family: PylonTrainingTopologyBackendFamily::Cuda,
            world_size: 1,
            rank: 0,
            local_device_ids: vec![0],
            collective_kind: PylonTrainingCollectiveKind::DataParallel,
            elastic_boundary: PylonTrainingElasticBoundary::Window,
        };
        let checkpoint = PylonTrainingCheckpointBinding {
            checkpoint_family: "checkpoint.family.alpha".to_string(),
            checkpoint_ref: "checkpoint://run.alpha/0001".to_string(),
            manifest_digest: "sha256:checkpoint-manifest-alpha".to_string(),
            latest_pointer_ref:
                "gs://bucket/networks/trainnet.alpha/runs/run.alpha/checkpoints/latest_pointer.json"
                    .to_string(),
        };
        let artifacts = PylonTrainingArtifacts {
            bucket_uri: "gs://bucket".to_string(),
            run_prefix: "networks/trainnet.alpha/runs/run.alpha".to_string(),
            window_prefix: "networks/trainnet.alpha/runs/run.alpha/windows/window.0001".to_string(),
            local_run_root: local_run_root.display().to_string(),
            credential_source: PYLON_TRAINING_GCS_CREDENTIAL_SOURCE.to_string(),
        };
        let trn = PylonTrainingTrn {
            network_coordinate: format!("39500:{}:trainnet.alpha", common.coordinator_pubkey),
            window_coordinate: format!("39510:{}:window.0001", common.coordinator_pubkey),
            relay_urls: vec![
                "wss://relay.example.com".to_string(),
                "wss://relay.example.com".to_string(),
            ],
        };
        let manifest = PylonTrainingRunManifestV1::builder(
            PylonTrainingManifestRole::Worker,
            common,
            topology,
            checkpoint,
            artifacts,
            trn,
        )
        .dataset(PylonTrainingDatasetAssignment {
            dataset_id: "dataset.alpha".to_string(),
            slice_id: "slice.0001".to_string(),
            slice_digest: "sha256:slice-alpha".to_string(),
            assignment_seed: "seed.alpha".to_string(),
        })
        .build()
        .map_err(anyhow::Error::msg)?;
        let layout =
            PylonTrainingArtifactLayout::from_manifest(&manifest).map_err(anyhow::Error::msg)?;
        Ok((
            temp_dir,
            TrainingManifestInspectionContext {
                manifest,
                manifest_path: local_run_root.join("manifests").join("run_manifest.json"),
                local_run_root,
                layout,
            },
        ))
    }

    #[test]
    fn training_trn_mapping_roundtrips_worker_node_receipt_and_artifact_events() -> Result<()> {
        let (temp_dir, context) = worker_manifest_context()?;
        let mut config = super::super::default_config(temp_dir.path());
        config.node_label = "node-alpha".to_string();
        config.payout_destination = Some("lnbc1trainingalpha".to_string());
        config.training.checkpoint_serve_addr = "127.0.0.1:43000".to_string();
        config.training.role_claims = vec![super::super::PylonTrainingRoleClaim::Worker];
        let identity = identity_fixture(&temp_dir);
        let state = PylonTrainingRuntimeState {
            active_runtime: Some(super::super::PylonTrainingActiveRuntimeState {
                training_run_id: context.manifest.run_id.clone(),
                window_id: context.manifest.window_id.clone(),
                assignment_id: context.manifest.assignment_id.clone(),
                lease_id: context.manifest.lease_id.clone(),
                membership_revision: context.manifest.membership_revision.clone(),
                role: super::super::PylonTrainingRoleClaim::Worker,
                manifest_path: context.manifest_path.display().to_string(),
                run_root: context.local_run_root.display().to_string(),
                desired_state: super::super::PylonTrainingSupervisorDesiredState::Running,
                process_state: super::super::PylonTrainingSupervisorProcessState::Running,
                pid: Some(4243),
                stdout_log_path: context
                    .local_run_root
                    .join("stdout.log")
                    .display()
                    .to_string(),
                stderr_log_path: context
                    .local_run_root
                    .join("stderr.log")
                    .display()
                    .to_string(),
                failure_receipt_path: None,
                last_exit_code: None,
                last_heartbeat_at_ms: Some(1_762_491_240_000),
                last_failure_reason: None,
                launch_count: 1,
                restart_count: 0,
                updated_at_ms: 1_762_491_240_000,
            }),
            ..PylonTrainingRuntimeState::default()
        };

        let (status, node_record) = node_record_event(&config, &state, &[&context])?;
        assert_eq!(status, "online");
        assert_eq!(node_record.status, "online");
        assert_eq!(node_record.roles, vec!["worker".to_string()]);
        assert!(
            node_record
                .build_digest
                .as_deref()
                .is_some_and(|value| value.starts_with("sha256:"))
        );
        assert_eq!(
            node_record.relay_urls,
            vec!["wss://relay.example.com".to_string()]
        );
        assert_eq!(
            node_record
                .content
                .get("release_id")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            super::super::local_training_release_id()
        );
        assert_eq!(
            node_record
                .content
                .get("build_digest")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            node_record.build_digest.as_deref().unwrap_or_default()
        );
        assert_eq!(
            node_record
                .content
                .get("settlement_destination")
                .and_then(Value::as_str),
            Some("lnbc1trainingalpha")
        );
        assert!(
            node_record
                .capabilities
                .contains(&nostr::TrnCapability::new("backend", "cuda"))
        );
        assert!(
            node_record
                .capabilities
                .contains(&nostr::TrnCapability::new(
                    "checkpoint_family",
                    "checkpoint.family.alpha"
                ))
        );
        assert!(
            node_record
                .capabilities
                .contains(&nostr::TrnCapability::new("environment", "env.cuda.alpha"))
        );
        assert!(matches!(
            TrnEvent::from_event(&fake_event(node_record.to_event_template(1_774_160_010)?))?,
            TrnEvent::NodeRecord(event) if event == node_record
        ));

        let assignment_ack = assignment_ack_event(&identity, &context, "wss://relay.example.com")?;
        assert_eq!(assignment_ack.status, "assignment_accepted");
        assert_eq!(
            assignment_ack.reason_codes,
            vec!["dataset_slice_assigned".to_string()]
        );
        assert_eq!(assignment_ack.classes, vec!["delta".to_string()]);
        assert_eq!(assignment_ack.actors.len(), 2);
        let actor_markers = assignment_ack
            .actors
            .iter()
            .map(|actor| actor.marker.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actor_markers,
            BTreeSet::from([Some("coordinator".to_string()), Some("subject".to_string()),])
        );
        assert!(matches!(
            TrnEvent::from_event(&fake_event(assignment_ack.to_event_template(1_774_160_011)?))?,
            TrnEvent::Receipt(event) if event == assignment_ack
        ));

        let object = PylonTrainingArtifactObjectTransferReport {
            object_uri: format!(
                "{}/contributions/{}/proof_bundle.json",
                context.layout.window_root(),
                context.manifest.assignment_id
            ),
            local_path: context
                .local_run_root
                .join("windows")
                .join("window.0001")
                .join("contributions")
                .join(&context.manifest.assignment_id)
                .join("proof_bundle.json")
                .display()
                .to_string(),
            digest: "sha256:proof-alpha".to_string(),
            size_bytes: 2_048,
            uploaded: true,
            digest_verified: true,
        };
        let locator = artifact_locator_event(
            &context,
            "contribution:assign.node01.window0001",
            "artifact.proof.alpha",
            "proof",
            &object,
        );
        assert_eq!(locator.status, "staged");
        assert_eq!(locator.artifact_class.as_deref(), Some("proof"));
        assert_eq!(
            locator.manifest_digest.as_deref(),
            Some(context.manifest.manifest_digest.as_str())
        );
        assert_eq!(locator.file_digest.as_deref(), Some("sha256:proof-alpha"));
        assert!(matches!(
            TrnEvent::from_event(&fake_event(locator.to_event_template(1_774_160_012)?))?,
            TrnEvent::ArtifactLocator(event) if event == locator
        ));

        let uploaded = artifact_uploaded_receipt_event(
            &identity,
            &context,
            "contribution:assign.node01.window0001",
            "artifact.proof.alpha",
            "proof",
            &format!(
                "39520:{}:artifact.proof.alpha",
                context.manifest.coordinator_pubkey
            ),
            "wss://relay.example.com",
            &object,
        );
        assert_eq!(uploaded.status, "artifact_uploaded");
        assert_eq!(
            uploaded.assignment_id.as_deref(),
            Some("assign.node01.window0001")
        );
        assert_eq!(
            uploaded.checkpoint_id.as_deref(),
            Some("checkpoint://run.alpha/0001")
        );
        assert_eq!(uploaded.classes, vec!["proof".to_string()]);
        assert_eq!(uploaded.address_refs.len(), 2);
        assert!(matches!(
            TrnEvent::from_event(&fake_event(uploaded.to_event_template(1_774_160_013)?))?,
            TrnEvent::Receipt(event) if event == uploaded
        ));

        Ok(())
    }
}
