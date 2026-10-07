use super::*;
use receipts::sales_funnel::{
    Admission, Consent, EventInput, EvidenceClass, FailureInput, FailureReason, FinancialIdentity,
    Kind, Lane,
};

fn setup() -> (TempDir, Store, Access, String, PathBuf, Admission) {
    let (dir, mut store, owner, _) = fixture();
    let lead = store.apply(&owner, &create("funnel-lead")).unwrap().lead;
    let root = dir.path().join("evidence");
    std::fs::create_dir(&root).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let admission = admission(&root);
    (dir, store, owner, lead, root, admission)
}
fn admission(root: &Path) -> Admission {
    Admission {
        id: "journey".into(),
        offer_version: "offer-v1".into(),
        cohort: "fixture-cohort".into(),
        lane: Lane::Assisted,
        classification: EvidenceClass::Fixture,
        consent: Consent {
            evidence: service_fixture::retain(
                root,
                "telemetry-consent",
                b"synthetic separate telemetry and aggregate consent",
            ),
            at: 999,
            expires_at: 1500,
            aggregate_counts: true,
        },
    }
}

#[test]
fn full_ordinary_history_reserves_telemetry_withdrawal_and_contact_cleanup() {
    let (_dir, mut store, owner, lead, root, admitted) = setup();
    enroll(&mut store, &owner, &root, &lead, admitted);
    assert_eq!(
        store.ordinary_history_limit(0),
        MAX_RECEIPTS - MAX_LEADS - 1
    );
    let prior = store.state.receipts.values().next().unwrap().clone();
    while store.state.receipts.len() < store.ordinary_history_limit(0) {
        let key = digest(format!("funnel-history-{}", store.state.receipts.len()).as_bytes());
        store.state.receipts.insert(key, prior.clone());
    }
    let ordinary = command(
        "full-funnel-observation",
        Some(&lead),
        2,
        Operation::RecordFunnelEvent {
            journey: "journey".into(),
            event: event(&root, "acquisition"),
        },
    );
    assert!(
        store
            .apply_with_evidence_root(&owner, &ordinary, Some(&root))
            .unwrap_err()
            .contains("ordinary history")
    );
    let before = serde_json::to_vec_pretty(&store.state).unwrap().len();
    let withdraw = command(
        "full-funnel-withdrawal",
        Some(&lead),
        2,
        Operation::RevokeFunnelConsent {
            journey: "journey".into(),
            reference: "synthetic separate telemetry withdrawal".into(),
        },
    );
    let receipt = store.apply(&owner, &withdraw).unwrap();
    assert_eq!(store.apply(&owner, &withdraw).unwrap(), receipt);
    assert!(store.funnel_show(&owner, &lead, "journey").is_err());
    let after = serde_json::to_vec_pretty(&store.state).unwrap().len();
    assert!(after.saturating_sub(before) < FUNNEL_CLEANUP_BYTES);
    let delete = command(
        "after-funnel-delete",
        Some(&lead),
        3,
        Operation::Delete {
            reference: "synthetic contact privacy cleanup".into(),
        },
    );
    let receipt = store.apply(&owner, &delete).unwrap();
    assert_eq!(store.apply(&owner, &delete).unwrap(), receipt);
    assert!(store.show(&owner, &lead).is_err());
}

#[test]
fn already_used_contact_cleanup_slots_do_not_block_remaining_telemetry_withdrawal() {
    let (_dir, mut store, owner, lead, root, admitted) = setup();
    enroll(&mut store, &owner, &root, &lead, admitted);
    let mut other = store.state.leads[&lead].clone();
    other.id = "historical-other-lead".into();
    other.contact = "email:historical-other@synthetic.invalid".into();
    other.funnel_journeys.clear();
    store.state.leads.insert(other.id.clone(), other.clone());
    // Represent a retained history after other contacts used their reserved
    // cleanup slots, with one journey and two contacts still present.
    let prior = store.state.receipts.values().next().unwrap().clone();
    while store.state.receipts.len() < MAX_RECEIPTS - 3 {
        let key = digest(format!("prior-cleanup-{}", store.state.receipts.len()).as_bytes());
        store.state.receipts.insert(key, prior.clone());
    }
    store
        .apply(
            &owner,
            &command(
                "other-contact-cleanup",
                Some(&other.id),
                other.revision,
                Operation::Delete {
                    reference: "synthetic other contact withdrawal".into(),
                },
            ),
        )
        .unwrap();
    let withdraw = command(
        "remaining-telemetry",
        Some(&lead),
        2,
        Operation::RevokeFunnelConsent {
            journey: "journey".into(),
            reference: "synthetic tracking withdrawal".into(),
        },
    );
    let receipt = store.apply(&owner, &withdraw).unwrap();
    assert_eq!(store.apply(&owner, &withdraw).unwrap(), receipt);
    store
        .apply(
            &owner,
            &command(
                "last-contact-cleanup",
                Some(&lead),
                3,
                Operation::Delete {
                    reference: "synthetic final contact withdrawal".into(),
                },
            ),
        )
        .unwrap();
    assert_eq!(store.state.receipts.len(), MAX_RECEIPTS);
    assert!(store.state.leads.is_empty());
}
fn enroll(
    store: &mut Store,
    owner: &Access,
    root: &Path,
    lead: &str,
    admission: Admission,
) -> Receipt {
    service_apply(
        store,
        owner,
        root,
        lead,
        "enroll",
        Operation::RecordFunnelJourney { admission },
    )
}
fn event(root: &Path, id: &str) -> EventInput {
    EventInput {
        id: id.into(),
        at: 1000,
        kind: Kind::Acquisition {
            source: None,
            evidence: service_fixture::retain(
                root,
                "source-observation",
                b"synthetic explicitly unknown source",
            ),
        },
    }
}
fn failure(root: &Path) -> FailureInput {
    FailureInput {
        event: "acquisition".into(),
        reason: FailureReason::UnknownAttribution,
        responsible_human: "operator".into(),
        next_action: "privately verify source; no outreach authorized".into(),
        due_at: 1100,
        evidence: service_fixture::retain(
            root,
            "failure-review",
            b"synthetic owner accepted failed conversion responsibility",
        ),
        resolved: false,
    }
}
#[test]
fn consented_funnel_custody_replays_after_restart_without_tracking_others() {
    let (dir, mut store, owner, lead, root, admission) = setup();
    let input = command(
        "enroll",
        Some(&lead),
        1,
        Operation::RecordFunnelJourney { admission },
    );
    assert!(store.apply(&owner, &input).is_err());
    let first = store
        .apply_with_evidence_root(&owner, &input, Some(&root))
        .unwrap();
    assert_eq!(
        store
            .apply_with_evidence_root(&owner, &input, Some(&root))
            .unwrap(),
        first
    );
    let recorded = service_apply(
        &mut store,
        &owner,
        &root,
        &lead,
        "observe",
        Operation::RecordFunnelEvent {
            journey: "journey".into(),
            event: event(&root, "acquisition"),
        },
    );
    let snapshot = store.funnel_snapshot(&owner, &lead, "journey").unwrap();
    assert_eq!(snapshot.journey.events.len(), 1);
    assert_eq!(
        snapshot.journey.admission.classification,
        EvidenceClass::Fixture
    );
    assert!(
        store
            .funnel_snapshot(&owner, &lead, "not-enrolled")
            .is_err()
    );
    let output = dir.path().join("private-funnel.json");
    store
        .funnel_export(&owner, &lead, "journey", &output)
        .unwrap();
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let bytes = std::fs::read_to_string(&output).unwrap();
    assert!(!bytes.contains("prospect@"));
    assert!(!bytes.contains(&Store::read_credential(&dir.path().join("operator")).unwrap()));
    assert!(
        store
            .funnel_export(&owner, &lead, "journey", &output)
            .is_err()
    );
    drop(store);
    let mut reopened = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
    let owner = reopened
        .authenticate(&Store::read_credential(&dir.path().join("operator")).unwrap())
        .unwrap();
    let retry = command(
        "observe",
        Some(&lead),
        2,
        Operation::RecordFunnelEvent {
            journey: "journey".into(),
            event: event(&root, "acquisition"),
        },
    );
    assert_eq!(
        reopened
            .apply_with_evidence_root(&owner, &retry, Some(&root))
            .unwrap(),
        recorded
    );
    assert_eq!(
        reopened.funnel_snapshot(&owner, &lead, "journey").unwrap(),
        snapshot
    );
}
#[test]
fn funnel_original_scope_current_credentials_and_revocation_bound_private_reads() {
    let (dir, mut store, owner, lead, root, admitted) = setup();
    let writer = grant(&dir, &mut store, &owner, "writer-a", Role::Writer);
    let input = command(
        "writer-enroll",
        Some(&lead),
        1,
        Operation::RecordFunnelJourney {
            admission: admitted.clone(),
        },
    );
    assert!(
        store
            .apply_with_evidence_root(&writer, &input, Some(&root))
            .is_err()
    );
    enroll(&mut store, &owner, &root, &lead, admitted);
    let reader = grant(&dir, &mut store, &owner, "new-reader", Role::Reader);
    let mut changed = store.show(&owner, &lead).unwrap().details;
    changed.account = "later-account".into();
    changed.readers.push("new-reader".into());
    changed.data.recipients.push("human:new-reader".into());
    changed.permission.reference = "fresh explicit scope".into();
    store
        .apply(
            &owner,
            &command(
                "new-scope",
                Some(&lead),
                2,
                Operation::Update { details: changed },
            ),
        )
        .unwrap();
    assert_eq!(
        store.funnel_show(&owner, &lead, "journey").unwrap().account,
        "synthetic-account"
    );
    assert!(
        store
            .show(&reader, &lead)
            .unwrap()
            .funnel_journeys
            .is_empty()
    );
    assert!(
        store.list(&reader, None, 10).unwrap()[0]
            .funnel_journeys
            .is_empty()
    );
    assert!(store.funnel_show(&reader, &lead, "journey").is_err());
    store.revoke(&owner, "new-reader").unwrap();
    assert!(store.show(&reader, &lead).is_err());
    service_apply(
        &mut store,
        &owner,
        &root,
        &lead,
        "withdraw-telemetry",
        Operation::RevokeFunnelConsent {
            journey: "journey".into(),
            reference: "synthetic telemetry withdrawal".into(),
        },
    );
    assert!(store.funnel_show(&owner, &lead, "journey").is_err());
    assert!(!dir.path().join("host/money").exists());
}
#[test]
fn failed_conversion_requires_accepted_human_and_stale_snapshots_cannot_publish() {
    let (dir, mut store, owner, lead, root, admitted) = setup();
    enroll(&mut store, &owner, &root, &lead, admitted);
    service_apply(
        &mut store,
        &owner,
        &root,
        &lead,
        "observe",
        Operation::RecordFunnelEvent {
            journey: "journey".into(),
            event: event(&root, "acquisition"),
        },
    );
    let stale = store.funnel_snapshot(&owner, &lead, "journey").unwrap();
    grant(&dir, &mut store, &owner, "writer-a", Role::Writer);
    let mut input = failure(&root);
    input.responsible_human = "writer-a".into();
    let bytes = command(
        "wrong-owner",
        Some(&lead),
        3,
        Operation::RecordConversionFailure {
            journey: "journey".into(),
            failure: input,
        },
    );
    assert!(
        store
            .apply_with_evidence_root(&owner, &bytes, Some(&root))
            .is_err()
    );
    service_apply(
        &mut store,
        &owner,
        &root,
        &lead,
        "failure",
        Operation::RecordConversionFailure {
            journey: "journey".into(),
            failure: failure(&root),
        },
    );
    assert!(store.authorize_funnel_snapshots(&owner, &[stale]).is_err());
    let current = store.funnel_snapshot(&owner, &lead, "journey").unwrap();
    store
        .authorize_funnel_snapshots(&owner, &[current.clone()])
        .unwrap();
    assert!(
        store
            .funnel_write(
                &owner,
                &[current],
                &dir.path().join("host/sales/private-report"),
                b"report"
            )
            .is_err()
    );
    store
        .apply(
            &owner,
            &command(
                "delete",
                Some(&lead),
                4,
                Operation::Delete {
                    reference: "synthetic removal".into(),
                },
            ),
        )
        .unwrap();
    assert!(store.funnel_show(&owner, &lead, "journey").is_err());
}
#[test]
fn service_snapshot_refresh_and_original_funnel_retention_survive_lead_changes() {
    let (dir, mut store, owner, lead, sale) = service_setup();
    let root = dir.path().join("evidence");
    service_apply(
        &mut store,
        &owner,
        &root,
        &lead,
        "sale",
        Operation::RecordServiceSale { admission: sale },
    );
    enroll(&mut store, &owner, &root, &lead, admission(&root));
    service_apply(
        &mut store,
        &owner,
        &root,
        &lead,
        "purchase-observation",
        Operation::RecordFunnelEvent {
            journey: "journey".into(),
            event: EventInput {
                id: "purchase".into(),
                at: 1000,
                kind: Kind::Purchase {
                    financial_offer: "offer".into(),
                    entry: "invoice".into(),
                    source: FinancialIdentity::ServiceSale {
                        sale: "synthetic-sale".into(),
                    },
                    evidence: service_fixture::retain(
                        &root,
                        "payment-observation",
                        b"synthetic unverified observation",
                    ),
                },
            },
        },
    );
    let stale = store.funnel_snapshot(&owner, &lead, "journey").unwrap();
    service_apply(
        &mut store,
        &owner,
        &root,
        &lead,
        "paid",
        Operation::ReconcileServicePayment {
            sale: "synthetic-sale".into(),
            payment: receipts::service_sale::PaymentInput {
                disposition: receipts::service_sale::Disposition::Paid,
                external_reference: Some("synthetic-external-payment".into()),
                paid_minor: Some(25000),
                reversed_minor: None,
                evidence: service_fixture::retain(
                    &root,
                    "paid-proof",
                    b"synthetic owner verified exact external collection",
                ),
            },
        },
    );
    assert!(store.authorize_funnel_snapshots(&owner, &[stale]).is_err());
    let mut changed = store.show(&owner, &lead).unwrap().details;
    changed.data.retain_until = 4000;
    changed.permission.expires_at = 3000;
    changed.permission.reference = "fresh retention extension".into();
    let revision = store.show(&owner, &lead).unwrap().revision;
    store
        .apply(
            &owner,
            &command(
                "extend",
                Some(&lead),
                revision,
                Operation::Update { details: changed },
            ),
        )
        .unwrap();
    drop(store);
    let mut later = Store::open_with_clock(&dir.path().join("host"), later).unwrap();
    let owner = later
        .authenticate(&Store::read_credential(&dir.path().join("operator")).unwrap())
        .unwrap();
    assert!(
        later
            .show(&owner, &lead)
            .unwrap()
            .funnel_journeys
            .is_empty()
    );
}
