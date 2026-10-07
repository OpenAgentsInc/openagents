use super::*;

fn reference(name: &str) -> Reference {
    Reference {
        path: format!("{name}.json"),
        sha256: "a".repeat(64),
    }
}
fn fixture() -> Pilot {
    Pilot {
        schema: SCHEMA.into(),
        id: "pilot".into(),
        basis: Basis::Fixture,
        target_pubkey: "b".repeat(64),
        consent: Consent {
            evidence: reference("consent"),
            approved_at_ms: 1,
            retain_until_ms: 1000,
        },
        profile_approval: None,
        profile_event: None,
        approved_public_links: vec![],
        lookups: vec![],
        funnel: vec![],
        joins: Joins::default(),
    }
}
fn accepted(id: &str, task: &str, at: u64) -> Event {
    Event::AcceptedTask {
        id: id.into(),
        at_ms: at,
        evidence: reference(id),
        task_id: task.into(),
        buyer_id: "buyer-1".into(),
        artifact_digest: "c".repeat(64),
    }
}
fn paid(id: &str, task: &str, state: PaymentState, amount: Option<u64>, at: u64) -> Event {
    Event::PaidUse {
        id: id.into(),
        at_ms: at,
        evidence: reference(id),
        accepted_task: task.into(),
        state,
        amount,
        unit: "msat".into(),
    }
}
fn lookup() -> Lookup {
    Lookup {
        id: "rank".into(),
        recorded_at_ms: 20,
        observation: reference("rank"),
        operation: Operation::Rank,
        origin: "https://api.example".into(),
        configuration_digest: "a".repeat(64),
        input_digest: "b".repeat(64),
        house_pubkey: "c".repeat(64),
        house_discovered_at_ms: 10,
        expires_at_ms: 30,
        partial: false,
        relevance: None,
        influence: Some(0.0),
        coverage: Coverage::Unknown,
        responses: [
            "/.well-known/open-ranking.json",
            "/.well-known/nostr.json?name=_",
            "/rank/pubkeys",
        ]
        .into_iter()
        .map(|endpoint| Response {
            endpoint: endpoint.into(),
            status: 200,
            algorithm: (endpoint == "/rank/pubkeys").then(|| "graperank".into()),
            fetched_at_ms: 10,
            expires_at_ms: 30,
            input_digest: if endpoint == "/rank/pubkeys" {
                "b".repeat(64)
            } else {
                "d".repeat(64)
            },
            output_digest: "e".repeat(64),
        })
        .collect(),
    }
}

#[test]
fn missing_stages_and_optional_joins_do_not_become_universal_gates() {
    let mut record = fixture();
    let summary = record.validate(100).unwrap();
    assert_eq!(summary.missing_stages.len(), 6);
    assert!(!summary.authority_granted);
    record.joins.pipeline_lead_id = Some("lead-1".into());
    record.joins.referral_attribution_id = Some("attribution-1".into());
    record.funnel.push(accepted("accepted1", "task1", 10));
    assert!(record.validate(100).is_ok());
}

#[test]
fn positive_settlement_and_repeat_remain_local_claims_for_distinct_tasks() {
    let mut record = fixture();
    record.funnel = vec![
        accepted("a1", "task1", 10),
        paid("p1", "a1", PaymentState::Settled, Some(1), 11),
        accepted("a2", "task2", 20),
        paid("p2", "a2", PaymentState::Settled, Some(2), 21),
        Event::RepeatUse {
            id: "repeat".into(),
            at_ms: 22,
            evidence: reference("repeat"),
            paid_uses: ["p1".into(), "p2".into()],
        },
    ];
    let summary = record.validate(100).unwrap();
    assert_eq!(summary.settled_payment_claims, 2);
    assert_eq!(summary.repeat_claims, 1);
    assert!(!summary.independently_verified_paid_conversion);
    assert!(!summary.authority_granted);
    if let Event::AcceptedTask { buyer_id, .. } = &mut record.funnel[2] {
        *buyer_id = "buyer-2".into();
    }
    assert!(record.validate(100).is_err());
    record.funnel[2] = accepted("a2", "task1", 20);
    assert!(record.validate(100).is_err());
}

#[test]
fn unknown_failed_and_reversed_payments_establish_no_settlement_or_repeat() {
    for state in [
        PaymentState::Unknown,
        PaymentState::Failed,
        PaymentState::Reversed,
    ] {
        let mut record = fixture();
        record.funnel = vec![
            accepted("a1", "task1", 10),
            paid("p1", "a1", state, None, 11),
        ];
        assert_eq!(record.validate(100).unwrap().settled_payment_claims, 0);
        record.funnel.push(Event::RepeatUse {
            id: "repeat".into(),
            at_ms: 20,
            evidence: reference("repeat"),
            paid_uses: ["p1".into(), "other".into()],
        });
        assert!(record.validate(100).is_err());
    }
}

#[test]
fn invented_future_or_duplicate_funnel_references_refuse() {
    let mut record = fixture();
    record.funnel = vec![paid("p1", "missing", PaymentState::Settled, Some(1), 11)];
    assert!(record.validate(100).is_err());
    record.funnel = vec![
        accepted("a1", "task1", 20),
        paid("p1", "a1", PaymentState::Settled, Some(1), 11),
    ];
    assert!(record.validate(100).is_err());
    record.funnel = vec![
        accepted("a1", "task1", 10),
        paid("p1", "a1", PaymentState::Settled, Some(0), 11),
    ];
    assert!(record.validate(100).is_err());
    record.funnel[1] = paid("p1", "a1", PaymentState::Settled, Some(1), 11);
    record
        .funnel
        .push(paid("p2", "a1", PaymentState::Settled, Some(1), 12));
    assert!(record.validate(100).is_err());
}

#[test]
fn zero_absent_unavailable_and_expired_coverage_stay_distinct() {
    let mut record = fixture();
    record.lookups.push(lookup());
    let summary = record.validate(100).unwrap();
    assert_eq!(summary.lookup_coverage[0].coverage, Coverage::Unknown);
    assert!(summary.lookup_coverage[0].expired);
    record.lookups[0].coverage = Coverage::Reported;
    assert!(record.validate(100).is_err());
    record.lookups[0].influence = None;
    record.lookups[0].coverage = Coverage::Absent;
    assert!(record.validate(100).is_ok());
    record.lookups[0].coverage = Coverage::Unavailable;
    record.lookups[0].partial = true;
    assert!(record.validate(100).is_ok());
}

#[test]
fn source_algorithms_expiry_and_provenance_cannot_be_relabelled() {
    let mut record = fixture();
    record.lookups.push(lookup());
    record.lookups[0].expires_at_ms = 31;
    assert!(record.validate(100).is_err());
    record.lookups[0].expires_at_ms = 30;
    record.lookups[0].responses[2].algorithm = Some("relevance".into());
    assert!(record.validate(100).is_err());
    record.lookups[0].responses[2].algorithm = Some("graperank".into());
    record.lookups[0].responses[2].status = 422;
    assert!(record.validate(100).is_err());
}

#[test]
fn consent_and_publication_approval_are_separate_and_bounded() {
    let mut record = fixture();
    assert!(record.validate(1000).is_err());
    record.profile_event = Some(reference("profile"));
    assert!(record.validate(100).is_err());
    record.profile_approval = Some(reference("approval"));
    assert!(record.validate(100).is_ok());
    record.approved_public_links = vec!["http://example.com".into()];
    assert!(record.validate(100).is_err());
    record.approved_public_links.clear();
    record.funnel = (0..65)
        .map(|i| Event::Referral {
            id: format!("r{i}"),
            at_ms: 20,
            evidence: reference("r"),
            lookup: None,
        })
        .collect();
    assert!(record.validate(100).is_err());
}

#[test]
fn references_reject_traversal_absolute_secret_and_unknown_fields() {
    for path in ["../secret", "/secret", "a/../secret", "a//b", "C:\\secret"] {
        assert!(
            Reference {
                path: path.into(),
                sha256: "a".repeat(64)
            }
            .validate()
            .is_err()
        );
    }
    let mut json = serde_json::to_value(fixture()).unwrap();
    json["input_ref"] = serde_json::json!("forged");
    assert!(serde_json::from_value::<Pilot>(json).is_err());
}

#[test]
fn ext_manifest_pins_keep_the_existing_tagged_digest_spelling() {
    let mut record = fixture();
    record.funnel.push(Event::Install {
        id: "install".into(),
        at_ms: 10,
        evidence: reference("install"),
        package: format!("{}:brainstorm-guidance", "a".repeat(64)),
        version: "0.1.0".into(),
        manifest_digest: format!("sha256:{}", "b".repeat(64)),
        release_id: Some("c".repeat(64)),
    });
    assert!(record.validate(100).is_ok());
}
