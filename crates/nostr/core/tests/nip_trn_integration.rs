#![allow(
    clippy::all,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::unwrap_used
)]

use nostr::{
    Event, EventTemplate, KIND_TRAINING_ARTIFACT_LOCATOR, KIND_TRAINING_CLOSEOUT,
    KIND_TRAINING_NETWORK_CONTRACT, KIND_TRAINING_NODE_RECORD, KIND_TRAINING_RECEIPT,
    KIND_TRAINING_VALIDATOR_VERDICT, KIND_TRAINING_WINDOW, NipTrnError,
    TrainingArtifactLocatorEvent, TrainingCloseoutEvent, TrainingNetworkContractEvent,
    TrainingNodeRecordEvent, TrainingReceiptEvent, TrainingValidatorVerdictEvent,
    TrainingWindowEvent, TrnAddressReference, TrnCapability, TrnEvent, TrnEventReference,
    TrnPubkeyReference, pylon_training_observed_labels_from_event,
    pylon_training_reputation_targets, pylon_training_reputation_to_label_event,
};
use openagents_kernel_core::pylon_training::{
    PylonTrainingReputationLabel, PylonTrainingReputationNamespace, PylonTrainingReputationRecord,
};
use serde_json::json;
use std::collections::BTreeSet;

fn fake_event(template: EventTemplate) -> Event {
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

#[test]
fn training_trn_public_api_roundtrips_all_event_kinds() {
    let network_contract = TrainingNetworkContractEvent {
        identifier: "trainnet.alpha".to_string(),
        network_id: "trainnet.alpha".to_string(),
        status: "active".to_string(),
        content: json!({"policy_family": "psion.train.v1"}),
        model_family: Some("psion".to_string()),
        window_cadence: Some("3600".to_string()),
        roles: vec!["validator".to_string()],
        profiles: vec!["trn-discovery".to_string()],
        address_refs: vec![TrnAddressReference::new(
            format!("39520:{}:bootstrap-artifact", "aa".repeat(32)),
            Some("wss://relay.example.com"),
            Some("bootstrap"),
        )],
        extra_tags: vec![vec!["run".to_string(), "run.alpha".to_string()]],
    }
    .normalize();
    let parsed = TrnEvent::from_event(&fake_event(
        network_contract
            .to_event_template(1_774_160_001)
            .expect("template"),
    ))
    .expect("parsed network contract");
    assert!(matches!(
        parsed,
        TrnEvent::NetworkContract(event) if event == network_contract
    ));

    let node_record = TrainingNodeRecordEvent {
        identifier: "trainnet.alpha".to_string(),
        network_id: "trainnet.alpha".to_string(),
        status: "online".to_string(),
        content: json!({"label": "validator-alpha"}),
        roles: vec!["validator".to_string()],
        classes: vec!["audited".to_string()],
        build_digest: Some("sha256:build-alpha".to_string()),
        capabilities: vec![TrnCapability::new("backend", "cuda")],
        relay_urls: vec!["wss://relay.example.com".to_string()],
        extra_tags: Vec::new(),
    }
    .normalize();
    let parsed = TrnEvent::from_event(&fake_event(
        node_record
            .to_event_template(1_774_160_002)
            .expect("template"),
    ))
    .expect("parsed node record");
    assert!(matches!(parsed, TrnEvent::NodeRecord(event) if event == node_record));

    let window = TrainingWindowEvent {
        identifier: "window.0001".to_string(),
        network_id: "trainnet.alpha".to_string(),
        status: "sealed".to_string(),
        content: json!({"window_id": "window.0001", "assignment_plan_count": 1}),
        policy_revision: Some("policy://validator/mvp/v1".to_string()),
        assignment_seed: Some("sha256:seed-alpha".to_string()),
        workload_family: Some("adapter-delta".to_string()),
        address_refs: vec![TrnAddressReference::new(
            format!("39520:{}:bootstrap-artifact", "66".repeat(32)),
            None::<String>,
            Some("bootstrap"),
        )],
        extra_tags: Vec::new(),
    }
    .normalize();
    let parsed = TrnEvent::from_event(&fake_event(
        window.to_event_template(1_774_160_003).expect("template"),
    ))
    .expect("parsed window");
    assert!(matches!(parsed, TrnEvent::Window(event) if event == window));

    let receipt = TrainingReceiptEvent {
        network_id: "trainnet.alpha".to_string(),
        window_id: "window.0001".to_string(),
        status: "assignment_published".to_string(),
        content: json!({"subject_pubkey": "77", "expected_artifact_class": "delta"}),
        assignment_id: Some("assign.alpha".to_string()),
        policy_revision: Some("policy://validator/mvp/v1".to_string()),
        role: Some("worker".to_string()),
        artifact_id: Some("artifact.delta.alpha".to_string()),
        checkpoint_id: Some("checkpoint.alpha".to_string()),
        actors: vec![
            TrnPubkeyReference::subject("77".repeat(32)),
            TrnPubkeyReference::coordinator("88".repeat(32)),
        ],
        reason_codes: vec!["ready".to_string()],
        classes: vec!["delta".to_string()],
        address_refs: vec![TrnAddressReference::new(
            format!("39510:{}:window.0001", "99".repeat(32)),
            None::<String>,
            Some("window"),
        )],
        event_refs: vec![TrnEventReference::new(
            "aa".repeat(32),
            None::<String>,
            Some("window"),
        )],
        extra_tags: Vec::new(),
    }
    .normalize();
    let parsed = TrnEvent::from_event(&fake_event(
        receipt.to_event_template(1_774_160_004).expect("template"),
    ))
    .expect("parsed receipt");
    assert!(matches!(parsed, TrnEvent::Receipt(event) if event == receipt));

    let verdict = TrainingValidatorVerdictEvent {
        network_id: "trainnet.alpha".to_string(),
        window_id: "window.0001".to_string(),
        status: "accepted".to_string(),
        content: json!({"verdict": "accepted"}),
        assignment_id: Some("assign.alpha".to_string()),
        artifact_id: Some("artifact.delta.alpha".to_string()),
        policy_revision: Some("policy://validator/mvp/v1".to_string()),
        validator_policy: Some("policy://validator/mvp/v1".to_string()),
        digest: Some("sha256:artifact-alpha".to_string()),
        actors: vec![
            TrnPubkeyReference::subject("bb".repeat(32)),
            TrnPubkeyReference::validator("cc".repeat(32)),
        ],
        reason_codes: vec!["passed".to_string()],
        event_refs: vec![TrnEventReference::new(
            "dd".repeat(32),
            None::<String>,
            Some("challenge"),
        )],
        extra_tags: Vec::new(),
    }
    .normalize();
    let parsed = TrnEvent::from_event(&fake_event(
        verdict.to_event_template(1_774_160_005).expect("template"),
    ))
    .expect("parsed verdict");
    assert!(matches!(
        parsed,
        TrnEvent::ValidatorVerdict(event) if event == verdict
    ));

    let locator = TrainingArtifactLocatorEvent {
        identifier: "artifact.alpha".to_string(),
        network_id: "trainnet.alpha".to_string(),
        status: "accepted".to_string(),
        content: json!({"artifact_id": "artifact.alpha"}),
        artifact_id: Some("artifact.alpha".to_string()),
        checkpoint_id: Some("checkpoint.alpha".to_string()),
        manifest_digest: Some("sha256:manifest-alpha".to_string()),
        file_digest: Some("sha256:file-alpha".to_string()),
        url_hint: Some("gs://bucket/object".to_string()),
        artifact_class: Some("proof".to_string()),
        window_id: Some("window.0001".to_string()),
        policy_revision: Some("policy://validator/mvp/v1".to_string()),
        reason_codes: vec!["stored".to_string()],
        address_refs: vec![TrnAddressReference::new(
            format!("39520:{}:artifact.alpha.parent", "ee".repeat(32)),
            None::<String>,
            Some("source"),
        )],
        extra_tags: Vec::new(),
    }
    .normalize();
    let parsed = TrnEvent::from_event(&fake_event(
        locator.to_event_template(1_774_160_006).expect("template"),
    ))
    .expect("parsed locator");
    assert!(matches!(
        parsed,
        TrnEvent::ArtifactLocator(event) if event == locator
    ));

    let closeout = TrainingCloseoutEvent {
        network_id: "trainnet.alpha".to_string(),
        window_id: "window.0001".to_string(),
        status: "rewarded".to_string(),
        content: json!({"closeout_status": "rewarded"}),
        assignment_id: Some("assign.alpha".to_string()),
        artifact_id: Some("artifact.delta.alpha".to_string()),
        policy_revision: Some("policy://validator/mvp/v1".to_string()),
        amount_msats: Some("1000".to_string()),
        actors: vec![
            TrnPubkeyReference::subject("ff".repeat(32)),
            TrnPubkeyReference::coordinator("00".repeat(32)),
        ],
        reason_codes: vec!["payout_eligible".to_string()],
        event_refs: vec![TrnEventReference::new(
            "11".repeat(32),
            None::<String>,
            Some("verdict"),
        )],
        extra_tags: Vec::new(),
    }
    .normalize();
    let parsed = TrnEvent::from_event(&fake_event(
        closeout.to_event_template(1_774_160_007).expect("template"),
    ))
    .expect("parsed closeout");
    assert!(matches!(parsed, TrnEvent::Closeout(event) if event == closeout));

    for kind in [
        KIND_TRAINING_NETWORK_CONTRACT,
        KIND_TRAINING_NODE_RECORD,
        KIND_TRAINING_WINDOW,
        KIND_TRAINING_RECEIPT,
        KIND_TRAINING_VALIDATOR_VERDICT,
        KIND_TRAINING_ARTIFACT_LOCATOR,
        KIND_TRAINING_CLOSEOUT,
    ] {
        assert!(nostr::is_trn_kind(kind));
    }
}

#[test]
fn training_trn_public_api_rejects_missing_required_tags() {
    let missing_network = Event {
        id: "11".repeat(32),
        pubkey: "22".repeat(32),
        created_at: 1_774_160_009,
        kind: KIND_TRAINING_RECEIPT,
        tags: vec![
            vec!["window".to_string(), "window.0001".to_string()],
            vec!["status".to_string(), "window_sealed".to_string()],
        ],
        content: json!({"ok": true}).to_string(),
        sig: "33".repeat(64),
    };
    let error = TrnEvent::from_event(&missing_network).expect_err("missing network");
    assert!(matches!(error, NipTrnError::MissingTag("network")));
}

#[test]
fn training_trn_public_api_actor_and_reputation_helpers_roundtrip() {
    let subject = TrnPubkeyReference::subject("aa".repeat(32));
    assert_eq!(
        TrnPubkeyReference::from_tag(&subject.to_tag()).expect("subject tag"),
        subject
    );
    let coordinator = TrnPubkeyReference::coordinator("bb".repeat(32));
    assert_eq!(coordinator.marker.as_deref(), Some("coordinator"));
    let validator = TrnPubkeyReference::validator("cc".repeat(32));
    assert_eq!(validator.marker.as_deref(), Some("validator"));

    let record = PylonTrainingReputationRecord::new(
        PylonTrainingReputationNamespace::Validator,
        PylonTrainingReputationLabel::Poor,
        Some("11".repeat(32)),
        Some("event.alpha".to_string()),
        Some(format!("39512:{}:challenge.alpha", "22".repeat(32))),
    )
    .expect("record");
    let targets = pylon_training_reputation_targets(&record).expect("targets");
    assert_eq!(targets.len(), 3);

    let mut label_event = pylon_training_reputation_to_label_event(&record).expect("label event");
    label_event.set_content("validator drift");
    let event = Event {
        id: "label.validator.poor.alpha".to_string(),
        pubkey: "22".repeat(32),
        created_at: 1_700_000_000,
        kind: nostr::nip32::KIND_LABEL as u16,
        tags: label_event.to_tags(),
        content: label_event.content,
        sig: "00".repeat(64),
    };
    let observed = pylon_training_observed_labels_from_event(
        &event,
        "11".repeat(32).as_str(),
        &BTreeSet::from(["event.alpha".to_string()]),
        &BTreeSet::from([format!("39512:{}:challenge.alpha", "22".repeat(32))]),
        1_700_000_000 + 86_400,
    );
    assert_eq!(observed.len(), 1);
    assert_eq!(
        observed[0].projection.namespace,
        PylonTrainingReputationNamespace::Validator
    );
    assert_eq!(
        observed[0].projection.label,
        PylonTrainingReputationLabel::Poor
    );
}
