use super::*;
use crate::task::sales::tests::fixture as base_fixture;
use std::path::PathBuf;
use tempfile::TempDir;

fn now() -> u64 {
    2_000_000_000
}

/// The shared fixture with a clock far enough along for day arithmetic.
fn fixture() -> (TempDir, Store, Access, PathBuf) {
    let (dir, store, _, cred) = base_fixture();
    drop(store);
    let mut store = Store::open_with_clock(&dir.path().join("host"), now).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&cred).unwrap())
        .unwrap();
    let t = now();
    let command = serde_json::to_vec(&crate::task::sales::Command {
        schema: crate::task::sales::COMMAND_SCHEMA.into(),
        id: "earned-lead".into(),
        lead: None,
        expected_revision: 0,
        operation: crate::task::sales::Operation::Create {
            input: crate::task::sales::Input {
                contact: "email:prospect@synthetic.invalid".into(),
                source: "synthetic private introduction".into(),
                source_at: t - 200 * 86_400,
                details: crate::task::sales::Details {
                    account: "synthetic-account".into(),
                    jurisdiction: "synthetic jurisdiction record".into(),
                    scope: None,
                    permission: crate::task::sales::Permission {
                        state: crate::task::sales::PermissionState::Granted,
                        reference: "synthetic-consent-v1".into(),
                        recorded_at: t - 200 * 86_400,
                        expires_at: t + 100 * 86_400,
                        channels: vec!["email".into()],
                    },
                    workflow: "one synthetic repository maintenance task".into(),
                    baseline_reference: "private-baseline-reference".into(),
                    data: crate::task::sales::DataBoundary {
                        recipients: vec!["human:operator".into()],
                        permitted_use: "one agreed pilot; no marketing reuse".into(),
                        retain_until: t + 200 * 86_400,
                    },
                    stage: crate::task::sales::Stage::Active,
                    next: Some(crate::task::sales::NextAction {
                        description: "review pilot scope".into(),
                        due_at: t + 86_400,
                    }),
                    customer_decision: None,
                    readers: vec![],
                },
            },
            ownership_acceptance: "operator accepted responsibility".into(),
        },
    })
    .unwrap();
    store.apply(&owner, &command).unwrap();
    (dir, store, owner, cred)
}
use receipts::service_sale::{
    Admission, Evidence, Fulfillment, FulfillmentInput, FulfillmentTrigger,
    FulfillmentVerification, Invoice, PaymentInput, Reference, Verification,
};

fn reference(name: &str) -> Reference {
    Reference {
        path: format!("evidence/{name}"),
        sha256: digest(name.as_bytes()),
    }
}

fn sale(id: &str, admitted_at: u64) -> Sale {
    Sale {
        schema: receipts::service_sale::SCHEMA.into(),
        admission: Admission {
            id: id.into(),
            offer_version: "offer-v1".into(),
            invoice: Invoice {
                id: format!("{id}-invoice"),
                external_reference: format!("{id}-ext"),
                currency: "USD".into(),
                currency_scale: 100,
                amount_minor: 25_000,
                issued_at: admitted_at,
                due_at: admitted_at + 86_400,
                payment_route_reference: "route".into(),
                evidence: reference("invoice"),
            },
            sources: Evidence {
                agreement: reference("agreement"),
                agreement_acceptance: reference("agreement-acceptance"),
                pilot_review: reference("pilot-review"),
                handoff: reference("handoff"),
                customer_acceptance: reference("customer-acceptance"),
                support_acceptance: reference("support-acceptance"),
            },
            fulfillment: Some(Fulfillment {
                id: format!("{id}-fulfillment"),
                responsible_human: "operator".into(),
                currency: "USD".into(),
                currency_scale: 100,
                amount_minor: 25_000,
                trigger: FulfillmentTrigger::AcceptedDelivery,
                agreement: reference("agreement"),
                acceptance: reference("customer-acceptance"),
                bill: None,
                payment: None,
            }),
        },
        pipeline_lead: "lead".into(),
        pipeline_revision_at_admission: 1,
        account: "account".into(),
        admitted_by: "operator".into(),
        admitted_at,
        admission_command_digest: digest(b"admit"),
        admitted_recipients: vec!["human:operator".into()],
        retain_until: admitted_at + 365 * 86_400,
        facts: receipts::service_sale::Facts {
            task_digest: digest(b"task"),
            frozen_checks: vec![reference("check")],
            candidate_sha256: digest(b"candidate"),
            runbook: reference("runbook"),
            comparison_manifest: reference("manifest"),
            comparison_report: reference("report"),
            accepted_checks: vec![reference("check")],
            deliverables: vec![reference("deliverable")],
            accepted_at: admitted_at,
            customer_decision_maker: "customer".into(),
            customer_decision_evidence: reference("decision"),
            support_human: "operator".into(),
            invoice_retention_reference: "retention".into(),
        },
        payments: Vec::new(),
        fulfillment_reconciliations: Vec::new(),
    }
}

fn pay(sale: &mut Sale, at: u64, disposition: Disposition, reversed: Option<u64>) {
    sale.payments.push(Verification {
        input: PaymentInput {
            disposition,
            external_reference: Some(format!("{}-ext", sale.admission.id)),
            paid_minor: match disposition {
                Disposition::Pending | Disposition::Unknown => None,
                _ => Some(25_000),
            },
            reversed_minor: reversed,
            evidence: reference("payment"),
        },
        verified_by: "operator".into(),
        verified_at: at,
        command_digest: digest(b"pay"),
    });
}

fn deliver(sale: &mut Sale, at: u64) {
    sale.fulfillment_reconciliations
        .push(FulfillmentVerification {
            input: FulfillmentInput {
                bill: reference("bill"),
                payment: None,
            },
            verified_by: "operator".into(),
            verified_at: at,
            command_digest: digest(b"deliver"),
        });
}

fn with_sales(store: &mut Store, owner: &Access, sales: Vec<(&str, Sale)>) -> String {
    let _ = owner;
    let lead = store.state.leads.keys().next().unwrap().clone();
    let record = store.state.leads.get_mut(&lead).unwrap();
    for (id, mut sale) in sales {
        sale.pipeline_lead = lead.clone();
        sale.account = "synthetic-account".into();
        record.service_sales.insert(id.into(), sale);
    }
    lead
}

#[test]
fn only_settled_and_delivered_sales_earn_and_ring_once() {
    let (_dir, mut store, owner, _cred) = fixture();
    let now = (store.clock)();
    let mut paid = sale("paid", now - 100 * 86_400);
    pay(&mut paid, now - 90 * 86_400, Disposition::Paid, None);
    deliver(&mut paid, now - 89 * 86_400);
    let mut undelivered = sale("undelivered", now - 100 * 86_400);
    pay(&mut undelivered, now - 90 * 86_400, Disposition::Paid, None);
    let mut pending = sale("pending", now - 100 * 86_400);
    pay(&mut pending, now - 90 * 86_400, Disposition::Pending, None);
    deliver(&mut pending, now - 89 * 86_400);
    let unpaid = sale("unpaid", now - 100 * 86_400);
    with_sales(
        &mut store,
        &owner,
        vec![
            ("paid", paid),
            ("undelivered", undelivered),
            ("pending", pending),
            ("unpaid", unpaid),
        ],
    );
    let ledger = store.earned_ledger(&owner).unwrap();
    assert_eq!(ledger.totals.earned_sales, 1);
    assert_eq!(ledger.totals.gross_usd_millionths, 250_000_000);
    assert_eq!(ledger.totals.unknown_settlement, 2);
    let eligible: Vec<_> = ledger.rows.iter().filter(|r| r.eligible).collect();
    assert_eq!(eligible.len(), 1);
    assert_eq!(eligible[0].sale, "paid");
    assert!(ledger.rows.iter().any(|r| r.sale == "undelivered"
        && r.ineligible_because == vec!["delivery not reconciled".to_string()]));

    let rings = store.ring_earned(&owner).unwrap();
    assert_eq!(rings.len(), 1);
    assert!(store.ring_earned(&owner).unwrap().is_empty());
    drop(store);
    let mut reopened = Store::open_with_clock(&_dir.path().join("host"), self::now).unwrap();
    let owner = reopened
        .authenticate(&Store::read_credential(&_cred).unwrap())
        .unwrap();
    assert!(reopened.ring_earned(&owner).unwrap().is_empty());
    assert_eq!(reopened.earned_ledger(&owner).unwrap().totals.rung, 1);
}

#[test]
fn a_refund_lowers_net_without_a_second_ring_and_readers_cannot_ring() {
    let (_dir, mut store, owner, _cred) = fixture();
    let reader_path = _dir.path().join("reader");
    store
        .issue(
            &owner,
            "reader",
            crate::task::sales::Role::Reader,
            &reader_path,
        )
        .unwrap();
    let now = (store.clock)();
    let mut paid = sale("paid", now - 100 * 86_400);
    pay(&mut paid, now - 90 * 86_400, Disposition::Paid, None);
    deliver(&mut paid, now - 89 * 86_400);
    let lead = with_sales(&mut store, &owner, vec![("paid", paid)]);
    assert_eq!(store.ring_earned(&owner).unwrap().len(), 1);
    let sale = store
        .state
        .leads
        .get_mut(&lead)
        .unwrap()
        .service_sales
        .get_mut("paid")
        .unwrap();
    pay(sale, now - 80 * 86_400, Disposition::Reversed, Some(10_000));
    let ledger = store.earned_ledger(&owner).unwrap();
    assert_eq!(ledger.totals.net_usd_millionths, 150_000_000);
    assert_eq!(ledger.totals.refunded_usd_millionths, 100_000_000);
    assert_eq!(ledger.totals.reversals, 1);
    assert!(store.ring_earned(&owner).unwrap().is_empty());
    let reader = store
        .authenticate(&Store::read_credential(&reader_path).unwrap())
        .unwrap();
    assert!(store.ring_earned(&reader).is_err());
    assert!(store.earned_ledger(&reader).is_err());
}

#[test]
fn shared_aggregates_need_review_lag_the_books_and_expire() {
    let (_dir, mut store, owner, _) = fixture();
    let now = (store.clock)();
    let mut paid = sale("paid", now - 100 * 86_400);
    pay(&mut paid, now - 90 * 86_400, Disposition::Paid, None);
    deliver(&mut paid, now - 89 * 86_400);
    let lead = with_sales(&mut store, &owner, vec![("paid", paid)]);
    assert!(matches!(
        store.shared_aggregate().unwrap(),
        Shared::Unavailable { .. }
    ));
    assert!(store.shared_aggregate_draft(&owner, now).is_err());
    let draft = store
        .shared_aggregate_draft(&owner, now - SHARED_DELAY)
        .unwrap();
    assert_eq!(draft.aggregate.earned_sales, 1);
    assert_eq!(draft.aggregate.net_usd_millionths_floor, 200_000_000);
    assert!(draft.weekly_update.contains("1 earned sale"));
    let json = serde_json::to_string(&draft).unwrap();
    assert!(!json.contains(&lead) && !json.contains("\"paid\""));
    assert!(
        store
            .approve_shared_aggregate(
                &owner,
                now - SHARED_DELAY,
                "0".repeat(64).as_str(),
                now + 86_400
            )
            .is_err()
    );
    store
        .approve_shared_aggregate(
            &owner,
            now - SHARED_DELAY,
            &draft.aggregate.digest,
            now + 86_400,
        )
        .unwrap();
    match store.shared_aggregate().unwrap() {
        Shared::Available { aggregate, .. } => assert_eq!(aggregate, draft.aggregate),
        other => panic!("{other:?}"),
    }
    // A reversal recorded after the approval changes the projection.
    let sale = store
        .state
        .leads
        .get_mut(&lead)
        .unwrap()
        .service_sales
        .get_mut("paid")
        .unwrap();
    pay(sale, now - 85 * 86_400, Disposition::Reversed, Some(25_000));
    assert!(matches!(
        store.shared_aggregate().unwrap(),
        Shared::Unavailable { reason } if reason.contains("no longer matches")
    ));
    assert!(
        store
            .revoke_shared_aggregate(&owner, &draft.aggregate.digest)
            .unwrap()
    );
    assert!(matches!(
        store.shared_aggregate().unwrap(),
        Shared::Unavailable { reason } if reason == "no reviewed aggregate"
    ));
}
