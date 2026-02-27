//! Integration tests for AC + SKL canonical link propagation.

use nostr::{
    CreditEnvelope, CreditSettlementContent, CreditSettlementReceipt, Event, KIND_CREDIT_ENVELOPE,
    KIND_CREDIT_SETTLEMENT, ScopeReference, ScopeType,
};

const CONSTRAINTS_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn test_skill_links_propagate_from_envelope_to_settlement() {
    let scope = ScopeReference::new(ScopeType::Skill, "33400:skill-pubkey:web-scraper:1.0.0")
        .with_constraints_hash(CONSTRAINTS_HASH);

    let envelope = CreditEnvelope::new(
        "envelope-1",
        "agent-pubkey",
        "issuer-pubkey",
        scope.clone(),
        35_000,
        1_703_003_600,
    )
    .with_skill_reference("33400:skill-pubkey:web-scraper", "manifest-event-id");

    let envelope_template = envelope.to_event_template(1_703_003_100).unwrap();
    let envelope_event = Event {
        id: "envelope-event-id".to_string(),
        pubkey: "issuer-pubkey".to_string(),
        created_at: envelope_template.created_at,
        kind: KIND_CREDIT_ENVELOPE,
        tags: envelope_template.tags.clone(),
        content: envelope_template.content.clone(),
        sig: "sig".to_string(),
    };

    let parsed_envelope = CreditEnvelope::from_event(&envelope_event).unwrap();
    assert_eq!(
        parsed_envelope.skill_address.as_deref(),
        Some("33400:skill-pubkey:web-scraper")
    );
    assert_eq!(
        parsed_envelope.manifest_event_id.as_deref(),
        Some("manifest-event-id")
    );

    let settlement = CreditSettlementReceipt::new(
        parsed_envelope.envelope_id.clone(),
        parsed_envelope.agent_pubkey.clone(),
        parsed_envelope.issuer_pubkey.clone(),
        parsed_envelope.scope.clone(),
        CreditSettlementContent::new(31_200, 600, "success"),
    )
    .with_skill_reference("33400:skill-pubkey:web-scraper", "manifest-event-id")
    .with_outcome_event("nip90-result-event-id");

    let settlement_template = settlement.to_event_template(1_703_003_800).unwrap();
    let settlement_event = Event {
        id: "settlement-event-id".to_string(),
        pubkey: "issuer-pubkey".to_string(),
        created_at: settlement_template.created_at,
        kind: KIND_CREDIT_SETTLEMENT,
        tags: settlement_template.tags.clone(),
        content: settlement_template.content.clone(),
        sig: "sig".to_string(),
    };

    let parsed_settlement = CreditSettlementReceipt::from_event(&settlement_event).unwrap();
    assert_eq!(
        parsed_settlement.skill_address.as_deref(),
        Some("33400:skill-pubkey:web-scraper")
    );
    assert_eq!(
        parsed_settlement.manifest_event_id.as_deref(),
        Some("manifest-event-id")
    );
    assert_eq!(
        parsed_settlement.outcome_event_id.as_deref(),
        Some("nip90-result-event-id")
    );
}

#[test]
fn test_skill_scope_validation_rejects_missing_constraints_or_mismatched_links() {
    let invalid_scope =
        ScopeReference::new(ScopeType::Skill, "33400:skill-pubkey:web-scraper:1.0.0");

    let missing_constraints = CreditEnvelope::new(
        "envelope-1",
        "agent-pubkey",
        "issuer-pubkey",
        invalid_scope,
        35_000,
        1_703_003_600,
    )
    .with_skill_reference("33400:skill-pubkey:web-scraper", "manifest-event-id");
    assert!(missing_constraints.validate().is_err());

    let mismatched_links = CreditEnvelope::new(
        "envelope-2",
        "agent-pubkey",
        "issuer-pubkey",
        ScopeReference::new(ScopeType::Skill, "33400:skill-pubkey:web-scraper:1.0.0")
            .with_constraints_hash(CONSTRAINTS_HASH),
        35_000,
        1_703_003_600,
    )
    .with_skill_reference("33400:other-pubkey:web-scraper", "manifest-event-id");
    assert!(mismatched_links.validate().is_err());
}
