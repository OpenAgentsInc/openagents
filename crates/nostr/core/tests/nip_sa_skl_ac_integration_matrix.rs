//! SA/SKL/AC integration matrix and hardening checks.

use nostr::nip_ac::{ScopeReference, ScopeType};
use nostr::nip90::{JobInput, JobRequest, JobResult, KIND_JOB_SUMMARIZATION};
use nostr::{
    CreditEnvelope, CreditIntent, CreditIntentContent, CreditOffer, CreditOfferContent,
    CreditSettlementContent, CreditSettlementReceipt, CreditSpendAuthorization, CreditSpendContent,
    EnvelopeAuthorityState, EnvelopeStatus, Event, SkillDelivery, SkillDeliveryContent,
    SkillLicense, SkillLicenseContent, SkillManifest, TrustDecision, TrustPolicy,
    evaluate_fulfillment_gate,
};

const CONSTRAINTS_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn fixture_manifest_event() -> Event {
    let manifest = SkillManifest::new(
        "summarize-text",
        "Summarize Text",
        "0.1.0",
        "Summarize arbitrary input safely",
        CONSTRAINTS_HASH,
        vec!["summarize".to_string()],
        1_803_003_600,
    )
    .with_content("name: summarize-text\nversion: 0.1.0");

    let template = manifest
        .to_event_template("skill-pubkey", 1_703_003_000)
        .expect("manifest template");

    Event {
        id: "manifest-event-id".to_string(),
        pubkey: "skill-pubkey".to_string(),
        created_at: template.created_at,
        kind: template.kind,
        tags: template.tags,
        content: template.content,
        sig: "sig".to_string(),
    }
}

fn fixture_trust_label(skill_address: &str, manifest_event_id: &str) -> Event {
    Event {
        id: "label-audit-pass".to_string(),
        pubkey: "auditor-pubkey".to_string(),
        created_at: 1_703_003_010,
        kind: nostr::nip32::KIND_LABEL as u16,
        tags: vec![
            vec!["L".to_string(), "skill-security".to_string()],
            vec![
                "l".to_string(),
                "audit-passed".to_string(),
                "skill-security".to_string(),
            ],
            vec!["a".to_string(), skill_address.to_string()],
            vec!["e".to_string(), manifest_event_id.to_string()],
            vec!["p".to_string(), "skill-pubkey".to_string()],
        ],
        content: String::new(),
        sig: "sig".to_string(),
    }
}

fn fixture_revocation(skill_address: &str, manifest_event_id: &str) -> Event {
    Event {
        id: "delete-manifest".to_string(),
        pubkey: "skill-pubkey".to_string(),
        created_at: 1_703_003_100,
        kind: nostr::nip09::DELETION_REQUEST_KIND,
        tags: vec![
            vec!["e".to_string(), manifest_event_id.to_string()],
            vec!["a".to_string(), skill_address.to_string()],
            vec!["k".to_string(), "33400".to_string()],
        ],
        content: "critical-vuln".to_string(),
        sig: "sig".to_string(),
    }
}

fn fixture_scope() -> ScopeReference {
    ScopeReference::new(ScopeType::Skill, "33400:skill-pubkey:summarize-text:0.1.0")
        .with_constraints_hash(CONSTRAINTS_HASH)
}

#[test]
fn test_discover_trust_license_job_and_settle_credit_flow() {
    let manifest_event = fixture_manifest_event();
    let skill_address = "33400:skill-pubkey:summarize-text";

    let license = SkillLicense::new(
        SkillLicenseContent::new(
            "license-001",
            "summarize-text",
            "0.1.0",
            1_703_003_020,
            vec!["summarize".to_string()],
        ),
        "agent-pubkey",
        skill_address,
        manifest_event.id.clone(),
        1_200,
    );
    license
        .validate(1_703_003_050)
        .expect("license should validate");

    let delivery = SkillDelivery::new(
        SkillDeliveryContent::new(
            "license-001",
            "fn summarize(input: &str) -> String { input.to_string() }",
            "rust",
            "hash-001",
        ),
        "agent-pubkey",
        skill_address,
        manifest_event.id.clone(),
        "license-event-id",
    );

    let trust_policy = TrustPolicy {
        minimum_positive_labels: 1,
        ..Default::default()
    };
    let trust_labels = vec![fixture_trust_label(skill_address, &manifest_event.id)];
    let gate = evaluate_fulfillment_gate(
        &license,
        &delivery,
        &manifest_event,
        &trust_labels,
        &[],
        &trust_policy,
    );
    assert!(
        gate.allowed,
        "skill fulfillment gate should pass: {}",
        gate.reason
    );

    let request = JobRequest::new(KIND_JOB_SUMMARIZATION)
        .expect("valid request kind")
        .add_input(JobInput::text("Summarize this document"))
        .add_param("model", "agent-runtime")
        .with_bid(1_200);
    let request_tags = request.to_tags();
    assert!(request_tags.iter().any(|tag| tag[0] == "i"));

    let result = JobResult::new(
        KIND_JOB_SUMMARIZATION,
        "request-event-id",
        "agent-pubkey",
        "summary output",
    )
    .expect("result should build")
    .with_amount(1_200, Some("lnbc1200n1...".to_string()));
    assert_eq!(result.kind, 6_001);

    let scope = fixture_scope();
    let intent = CreditIntent::new(
        scope.clone(),
        1_200,
        1_703_004_000,
        CreditIntentContent::new("execute summarize.text", 1_200),
    )
    .with_skill_reference(skill_address, &manifest_event.id)
    .with_provider("agent-pubkey");
    intent.validate().expect("intent validates");

    let offer = CreditOffer::new(
        "agent-pubkey",
        scope.clone(),
        1_200,
        "200bps",
        1_703_004_000,
        "issuer-pubkey",
        CreditOfferContent::new(1_200, "200bps"),
    )
    .with_skill_reference(skill_address, &manifest_event.id);
    offer.validate().expect("offer validates");

    let envelope = CreditEnvelope::new(
        "envelope-1",
        "agent-pubkey",
        "issuer-pubkey",
        scope,
        1_200,
        1_703_004_000,
    )
    .with_skill_reference(skill_address, &manifest_event.id)
    .with_status(EnvelopeStatus::Accepted);
    envelope.validate().expect("envelope validates");

    let spend = CreditSpendAuthorization::new(
        "issuer-pubkey",
        "envelope-1",
        fixture_scope(),
        1_200,
        1_703_004_000,
        CreditSpendContent::new(1_000).with_reason("job settlement"),
    )
    .with_skill_reference(skill_address, &manifest_event.id);
    spend
        .validate_against_envelope(&envelope, 1_703_003_900)
        .expect("spend should be allowed");

    let settlement = CreditSettlementReceipt::new(
        "envelope-1",
        "agent-pubkey",
        "issuer-pubkey",
        fixture_scope(),
        CreditSettlementContent::new(1_000, 20, "success"),
    )
    .with_skill_reference(skill_address, &manifest_event.id)
    .with_outcome_event("result-event-id");
    settlement.validate().expect("settlement validates");

    assert_eq!(settlement.content.spent_sats, 1_000);
    assert_eq!(
        settlement.outcome_event_id.as_deref(),
        Some("result-event-id")
    );
}

#[test]
fn test_trust_gate_denies_revoked_manifest() {
    let manifest_event = fixture_manifest_event();
    let skill_address = "33400:skill-pubkey:summarize-text";

    let license = SkillLicense::new(
        SkillLicenseContent::new(
            "license-002",
            "summarize-text",
            "0.1.0",
            1_703_003_020,
            vec!["summarize".to_string()],
        ),
        "agent-pubkey",
        skill_address,
        manifest_event.id.clone(),
        900,
    );
    let delivery = SkillDelivery::new(
        SkillDeliveryContent::new("license-002", "skill-bytes", "binary", "hash-002"),
        "agent-pubkey",
        skill_address,
        manifest_event.id.clone(),
        "license-event-id",
    );

    let policy = TrustPolicy {
        minimum_positive_labels: 1,
        ..Default::default()
    };
    let labels = vec![fixture_trust_label(skill_address, &manifest_event.id)];
    let deletion_events = vec![fixture_revocation(skill_address, &manifest_event.id)];

    let gate = evaluate_fulfillment_gate(
        &license,
        &delivery,
        &manifest_event,
        &labels,
        &deletion_events,
        &policy,
    );
    assert!(!gate.allowed);
    assert!(gate.reason.contains("revoked"));

    let trust = nostr::evaluate_skill_trust(
        skill_address,
        Some(&manifest_event.id),
        "skill-pubkey",
        &labels,
        &policy,
    );
    assert_eq!(trust.decision, TrustDecision::Trusted);
}

#[test]
fn test_envelope_replay_is_idempotent_and_stale_events_are_ignored() {
    let scope = fixture_scope();
    let envelope = CreditEnvelope::new(
        "envelope-2",
        "agent-pubkey",
        "issuer-pubkey",
        scope,
        2_000,
        1_703_004_000,
    )
    .with_status(EnvelopeStatus::Accepted)
    .with_skill_reference("33400:skill-pubkey:summarize-text", "manifest-event-id");

    let template = envelope.to_event_template(1_703_003_000).expect("template");
    let event = Event {
        id: "env-accepted-1".to_string(),
        pubkey: "issuer-pubkey".to_string(),
        created_at: template.created_at,
        kind: template.kind,
        tags: template.tags.clone(),
        content: template.content,
        sig: "sig".to_string(),
    };

    let mut authority = EnvelopeAuthorityState::new();
    assert!(authority.apply_event(&event).expect("first apply updates"));
    assert!(
        !authority
            .apply_event(&event)
            .expect("replay should be ignored")
    );

    let stale = Event {
        id: "env-accepted-0".to_string(),
        created_at: 1_703_002_000,
        ..event.clone()
    };
    assert!(!authority.apply_event(&stale).expect("stale ignored"));

    let spent_template = envelope
        .clone()
        .with_status(EnvelopeStatus::Spent)
        .to_event_template(1_703_003_500)
        .expect("spent template");
    let spent_event = Event {
        id: "env-spent-1".to_string(),
        pubkey: "issuer-pubkey".to_string(),
        created_at: spent_template.created_at,
        kind: spent_template.kind,
        tags: spent_template.tags,
        content: spent_template.content,
        sig: "sig".to_string(),
    };
    assert!(
        authority
            .apply_event(&spent_event)
            .expect("forward transition accepted")
    );

    let invalid_template = envelope
        .clone()
        .with_status(EnvelopeStatus::Accepted)
        .to_event_template(1_703_003_800)
        .expect("invalid template");
    let invalid_event = Event {
        id: "env-accepted-2".to_string(),
        pubkey: "issuer-pubkey".to_string(),
        created_at: invalid_template.created_at,
        kind: invalid_template.kind,
        tags: invalid_template.tags,
        content: invalid_template.content,
        sig: "sig".to_string(),
    };
    let err = authority
        .apply_event(&invalid_event)
        .expect_err("spent -> accepted transition must fail");
    assert!(matches!(
        err,
        nostr::EnvelopeError::InvalidTransition { .. }
    ));
}

#[test]
fn test_scope_and_spend_failure_modes_are_deterministic() {
    let invalid_scope =
        ScopeReference::new(ScopeType::Skill, "33400:skill-pubkey:summarize-text:0.1.0");

    let invalid_intent = CreditIntent::new(
        invalid_scope,
        1_000,
        1_703_004_000,
        CreditIntentContent::new("missing constraints", 1_000),
    )
    .with_skill_reference("33400:skill-pubkey:summarize-text", "manifest-event-id");
    assert!(invalid_intent.validate().is_err());

    let valid_envelope = CreditEnvelope::new(
        "envelope-3",
        "agent-pubkey",
        "issuer-pubkey",
        fixture_scope(),
        500,
        1_703_004_000,
    )
    .with_skill_reference("33400:skill-pubkey:summarize-text", "manifest-event-id");
    valid_envelope.validate().expect("valid envelope");

    let over_spend = CreditSpendAuthorization::new(
        "issuer-pubkey",
        "envelope-3",
        fixture_scope(),
        500,
        1_703_004_000,
        CreditSpendContent::new(700),
    )
    .with_skill_reference("33400:skill-pubkey:summarize-text", "manifest-event-id");
    assert!(
        over_spend
            .validate_against_envelope(&valid_envelope, 1_703_003_900)
            .is_err()
    );

    let expired_spend = CreditSpendAuthorization::new(
        "issuer-pubkey",
        "envelope-3",
        fixture_scope(),
        500,
        1_703_004_000,
        CreditSpendContent::new(400),
    )
    .with_skill_reference("33400:skill-pubkey:summarize-text", "manifest-event-id");
    assert!(
        expired_spend
            .validate_against_envelope(&valid_envelope, 1_703_004_000)
            .is_err()
    );
}
