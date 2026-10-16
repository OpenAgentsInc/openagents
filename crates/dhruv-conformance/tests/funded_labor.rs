//! MKT/LAB delivery across separately run participants (`labor-roles`)
//! joined to the central ledger (`pay-ledger`), with an interrupted send,
//! an unavailable buyer, cancellation after acceptance, competing finals,
//! and a cost statement that keeps unknown costs unknown. Fake funds only.
#[path = "support/treasury.rs"]
mod treasury;
use labor_roles::book::{Collection, Final};
use labor_roles::host::Host;
use labor_roles::records::*;
use labor_roles::terms::{Deliverable, Relationship, Rights, Terms};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const NOW: u64 = 1_800_000_000;
const PRICE: u64 = 50_000;
const BUYER: u8 = 1;
const PROVIDER: u8 = 2;
const WORKER: u8 = 3;
const REVIEWER: u8 = 4;
const RESOLVER: u8 = 5;

fn secret(n: u8) -> secp256k1::SecretKey {
    secp256k1::SecretKey::from_byte_array([n; 32]).unwrap()
}
fn key(n: u8) -> String {
    secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &secret(n))
        .x_only_public_key()
        .0
        .to_string()
}
fn order() -> String {
    "0e".repeat(32)
}
fn d(tag: &str) -> String {
    format!("sha256:{}", tag.repeat(32))
}

fn terms() -> Terms {
    let roles = [BUYER, PROVIDER, WORKER, REVIEWER, RESOLVER].map(key);
    let operators = [
        "buyer-co",
        "provider-co",
        "provider-co",
        "review-co",
        "resolve-co",
    ];
    Terms {
        v: labor_roles::terms::TERMS_V.into(),
        market: "ad".repeat(32),
        order: order(),
        buyer: roles[0].clone(),
        provider: roles[1].clone(),
        worker: roles[2].clone(),
        reviewer: roles[3].clone(),
        resolver: roles[4].clone(),
        resolver_policy: "labor-evidence-v1".into(),
        cancellation: "evaluate-delivered-work-v1".into(),
        partial_delivery: "no-partial-payment-v1".into(),
        buyer_unavailable: "resolver-required-v1".into(),
        role_relationships: roles
            .iter()
            .zip(operators)
            .map(|(k, o)| Relationship {
                pubkey: k.clone(),
                operator: o.into(),
            })
            .collect(),
        deliverables: vec![Deliverable {
            id: "patch".into(),
            max_bytes: 8192,
        }],
        criteria: vec!["tests-pass".into()],
        max_reworks: 1,
        delivery_due_at: NOW + 1000,
        rework_due_at: Some(NOW + 2000),
        review_due_at: NOW + 3000,
        dispute_due_at: NOW + 4000,
        resolution_due_at: NOW + 5000,
        payment_due_at: NOW + 6000,
        retain_until: NOW + 100_000,
        price_msat: PRICE,
        rights: Rights {
            v: labor_roles::terms::RIGHTS_V.into(),
            license: "12".repeat(32),
            input_use: "perform-and-review-order".into(),
            output_use: "use-under-license".into(),
            publication: "deny".into(),
            training: "deny".into(),
            evaluation_reuse: "separate-grant".into(),
            redistribution: "deny".into(),
            recipients: roles.to_vec(),
            retention: "through-market-retain-until".into(),
        },
    }
}

struct Net {
    hosts: BTreeMap<u8, Host>,
    dir: tempfile::TempDir,
}

impl Net {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let hosts = [BUYER, PROVIDER, WORKER, REVIEWER, RESOLVER]
            .into_iter()
            .map(|n| {
                (
                    n,
                    Host::open(&dir.path().join(n.to_string()), secret(n), terms(), NOW).unwrap(),
                )
            })
            .collect();
        Self { hosts, dir }
    }
    fn deliver(&mut self, from: u8, events: &[nostr::domain::Event], at: u64) {
        for e in events {
            for (n, h) in &mut self.hosts {
                if *n != from {
                    h.receive(e, at).unwrap();
                }
            }
        }
    }
    fn send(&mut self, from: u8, record: Record, at: u64) -> String {
        let digest = Host::digest(&record).unwrap();
        let events = self
            .hosts
            .get_mut(&from)
            .unwrap()
            .issue(&record, at, [from; 32])
            .unwrap();
        self.deliver(from, &events, at);
        digest
    }
}

fn submission(number: u32, previous: Option<&str>, rework: Option<&str>) -> Record {
    Record::Submission(Submission {
        v: SUBMISSION_V.into(),
        issuer: key(WORKER),
        order: order(),
        number,
        previous: previous.map(Into::into),
        rework: rework.map(Into::into),
        executions: vec![d("e1")],
        deliverables: vec![DeliveredItem {
            id: "patch".into(),
            digest: format!("sha256:{:064x}", number + 1),
            size: 300,
        }],
        run_evidence: vec![d("e2")],
        limitations: d("e3"),
    })
}
fn delivery(issuer: u8, sub: &str, at: u64) -> Record {
    Record::Delivery(Delivery {
        v: DELIVERY_V.into(),
        issuer: key(issuer),
        order: order(),
        submission: sub.into(),
        received_at: at,
        available: true,
    })
}
fn verification(sub: &str, verdict: Verdict) -> Record {
    Record::Verification(Verification {
        v: VERIFICATION_V.into(),
        issuer: key(REVIEWER),
        order: order(),
        submission: sub.into(),
        policy: d("70"),
        checker_receipts: vec![d("71")],
        criteria: vec![Criterion {
            id: "tests-pass".into(),
            verdict,
            evidence: vec![d("72")],
        }],
        verdict,
        limitations: d("73"),
    })
}
fn review(sub: &str, ver: &str, decision: ReviewDecision) -> Record {
    let criteria = if decision == ReviewDecision::Accept {
        vec![]
    } else {
        vec!["tests-pass".into()]
    };
    Record::Review(Review {
        v: REVIEW_V.into(),
        issuer: key(BUYER),
        order: order(),
        submission: sub.into(),
        verification: ver.into(),
        decision,
        criteria,
        reason: d("a1"),
    })
}

fn pins() -> BTreeMap<&'static str, String> {
    [
        ("source", "51"),
        ("checker", "52"),
        ("disclosure", "53"),
        ("execution_requirements", "54"),
    ]
    .into_iter()
    .map(|(k, v)| (k, v.repeat(32)))
    .collect()
}

/// Every cost of the order by category; categories nobody metered stay
/// null, so no complete all-in cost is claimed.
fn statement(collection: &Collection, fee_msat: Option<i64>) -> Value {
    let (price, collected) = match collection {
        Collection::Unknown { amount_msat, .. } => (Some(*amount_msat), Value::Null),
        Collection::Funded { .. } => (Some(PRICE), json!(PRICE)),
        _ => (None, Value::Null),
    };
    json!({
        "price_msat": price, "collected_msat": collected, "platform_fee_msat": fee_msat,
        "worker_compute_msat": null, "checker_compute_msat": null, "failed_attempts_msat": null, "energy_kwh": null,
        "all_in_cost_msat": null,
    })
}

#[test]
fn an_interrupted_rework_order_is_accepted_funded_once_and_costed_honestly() {
    let mut net = Net::new();
    // Interrupted: the worker journals its submission, then crashes before
    // anything reaches the relay. On restart it re-issues: the journal
    // replays it as a duplicate and the same record goes out once.
    let s0_record = submission(0, None, None);
    let lost = net
        .hosts
        .get_mut(&WORKER)
        .unwrap()
        .issue(&s0_record, NOW + 100, [3; 32])
        .unwrap();
    drop(lost);
    drop(net.hosts.remove(&WORKER));
    let reopened = Host::open(
        &net.dir.path().join(WORKER.to_string()),
        secret(WORKER),
        terms(),
        NOW + 120,
    )
    .unwrap();
    net.hosts.insert(WORKER, reopened);
    let resent = net
        .hosts
        .get_mut(&WORKER)
        .unwrap()
        .issue(&s0_record, NOW + 120, [4; 32])
        .unwrap();
    net.deliver(WORKER, &resent, NOW + 120);
    let s0 = Host::digest(&s0_record).unwrap();
    net.send(BUYER, delivery(BUYER, &s0, NOW + 130), NOW + 130);
    let v0 = net.send(REVIEWER, verification(&s0, Verdict::Failed), NOW + 140);
    let r0 = net.send(
        BUYER,
        review(&s0, &v0, ReviewDecision::RequestRework),
        NOW + 150,
    );
    let s1 = net.send(WORKER, submission(1, Some(&s0), Some(&r0)), NOW + 1500);
    net.send(BUYER, delivery(BUYER, &s1, NOW + 1510), NOW + 1510);
    let v1 = net.send(REVIEWER, verification(&s1, Verdict::Passed), NOW + 1520);
    // A buyer's accepted Pylon service receipt is not LAB acceptance: the
    // obligation still does not exist.
    let buyer_signer = nostr::domain::RelaySigner::from_secret_hex(&"01".repeat(32)).unwrap();
    let service = nostr::pylon::Receipt {
        v: nostr::pylon::RECEIPT_V.into(),
        requires: vec![],
        meta: None,
        buyer: key(BUYER),
        provider: key(PROVIDER),
        pylon: "coder-labor".into(),
        lane: nostr::pylon::Lane::CjConversation,
        capability: format!("{}:pylon/text-generation", key(PROVIDER)),
        request: "ee".repeat(32),
        request_digest: "ef".repeat(32),
        result_digest: None,
        started_at: NOW + 1500,
        finished_at: NOW + 1520,
        units: nostr::pylon::Units {
            kind: nostr::pylon::UnitKind::Tokens,
            count: 10,
        },
        outcome: nostr::pylon::Outcome::Accepted,
        payment: None,
    };
    nostr::pylon::receipt_event(&buyer_signer, &service, NOW + 1525).unwrap();
    assert_eq!(net.hosts[&PROVIDER].book.collection(), Collection::None);
    let r1 = net.send(BUYER, review(&s1, &v1, ReviewDecision::Accept), NOW + 1530);
    let acceptance = net.send(
        BUYER,
        Record::Acceptance(Acceptance {
            v: ACCEPTANCE_V.into(),
            issuer: key(BUYER),
            order: order(),
            submission: Some(s1.clone()),
            verification: Some(v1.clone()),
            outcome: Outcome::Accepted,
            basis: AcceptanceBasis::BuyerAcceptance,
            review: Some(r1.clone()),
            resolution: None,
            amount_due_msat: Some(PRICE),
            supersedes: vec![],
            evidence: vec![r1],
        }),
        NOW + 1540,
    );
    for host in net.hosts.values() {
        assert_eq!(
            host.book.final_state(),
            Final::Accepted {
                acceptance: acceptance.clone(),
                amount_msat: PRICE
            }
        );
    }
    // Accepted but uncollected: the statement keeps collection unknown.
    let unknown = statement(&net.hosts[&PROVIDER].book.collection(), None);
    assert_eq!(unknown["price_msat"], PRICE);
    assert!(unknown["collected_msat"].is_null() && unknown["all_in_cost_msat"].is_null());
    // A cancellation after acceptance changes nothing.
    net.send(
        BUYER,
        Record::Cancellation(Cancellation {
            v: CANCELLATION_V.into(),
            issuer: key(BUYER),
            order: order(),
            reason: d("cc"),
            execution: None,
        }),
        NOW + 1550,
    );
    assert!(matches!(
        net.hosts[&PROVIDER].book.final_state(),
        Final::Accepted { .. }
    ));
    // The central receive adapter's receipt reaches the ledger once.
    let ledger_dir = tempfile::tempdir().unwrap();
    let mut ledger = pay_ledger::Ledger::open(ledger_dir.path().join("ledger.sqlite")).unwrap();
    let receipt = pay_ledger::markets::worker::FundingReceipt {
        payment_hash: "9a".repeat(32),
        received_msat: PRICE as i64 + 500,
        platform_fee_msat: 500,
        received_at: (NOW + 1600) as i64,
    };
    let first = net
        .hosts
        .get_mut(&PROVIDER)
        .unwrap()
        .fund(&mut ledger, &pins(), &receipt)
        .unwrap();
    let retry = net
        .hosts
        .get_mut(&PROVIDER)
        .unwrap()
        .fund(&mut ledger, &pins(), &receipt)
        .unwrap();
    assert_eq!(first.key, retry.key);
    assert_eq!(first.received_msat, PRICE as i64 + 500);
    let funded = statement(&net.hosts[&PROVIDER].book.collection(), Some(500));
    assert_eq!(funded["collected_msat"], PRICE);
    assert_eq!(funded["platform_fee_msat"], 500);
    assert!(funded["worker_compute_msat"].is_null() && funded["all_in_cost_msat"].is_null());
    let economic = treasury::qualify(
        ledger_dir.path(),
        ledger,
        secret(PROVIDER),
        first.shares,
        NOW + 1700,
    );
    assert!(
        economic["links"]
            .as_array()
            .unwrap()
            .iter()
            .any(|link| link["kind"] == "obligation" && link["id"] == first.resource)
    );
}

#[test]
fn a_disconnected_buyer_and_competing_finals_never_pay_by_default() {
    let mut net = Net::new();
    let s0 = net.send(WORKER, submission(0, None, None), NOW + 100);
    // The buyer is disconnected; the resolver records receipt and accepts.
    net.send(RESOLVER, delivery(RESOLVER, &s0, NOW + 110), NOW + 110);
    let v0 = net.send(REVIEWER, verification(&s0, Verdict::Passed), NOW + 120);
    let dispute = net.send(
        PROVIDER,
        Record::Dispute(Dispute {
            v: DISPUTE_V.into(),
            issuer: key(PROVIDER),
            order: order(),
            subject: s0.clone(),
            cause: Cause::BuyerUnavailable,
            evidence: vec![s0.clone()],
        }),
        NOW + 3500,
    );
    let resolution = net.send(
        RESOLVER,
        Record::Resolution(Resolution {
            v: RESOLUTION_V.into(),
            issuer: key(RESOLVER),
            order: order(),
            submission: Some(s0.clone()),
            verification: Some(v0.clone()),
            disputes: vec![dispute.clone()],
            decision: Decision::Accept,
            basis: Basis::Delivered,
            evidence: vec![v0.clone(), dispute],
            reason: d("b1"),
        }),
        NOW + 3600,
    );
    let resolver_final = |outcome, amount, supersedes: Vec<String>| {
        Record::Acceptance(Acceptance {
            v: ACCEPTANCE_V.into(),
            issuer: key(RESOLVER),
            order: order(),
            submission: Some(s0.clone()),
            verification: Some(v0.clone()),
            outcome,
            basis: AcceptanceBasis::ResolverDecision,
            review: None,
            resolution: Some(resolution.clone()),
            amount_due_msat: amount,
            supersedes,
            evidence: vec![resolution.clone()],
        })
    };
    let accepted = net.send(
        RESOLVER,
        resolver_final(Outcome::Accepted, Some(PRICE), vec![]),
        NOW + 3610,
    );
    assert_eq!(
        net.hosts[&BUYER].book.final_state(),
        Final::Accepted {
            acceptance: accepted,
            amount_msat: PRICE
        }
    );
    // A competing final appears (the buyer comes back and accepts too): the
    // order is in conflict and funding stops.
    let buyer_review = net.hosts.get_mut(&BUYER).unwrap().book.apply(
        review(&s0, &v0, ReviewDecision::Accept),
        &key(BUYER),
        NOW + 3620,
    );
    assert!(
        buyer_review.is_err(),
        "the review deadline has passed; the buyer's late accept is refused"
    );
    // Two resolver finals without supersession conflict.
    let mut alt = resolver_final(Outcome::Accepted, Some(PRICE), vec![]);
    if let Record::Acceptance(a) = &mut alt {
        a.evidence.push(d("ff"));
    }
    net.send(RESOLVER, alt, NOW + 3630);
    assert!(matches!(
        net.hosts[&PROVIDER].book.final_state(),
        Final::Conflict(_)
    ));
    let ledger_dir = tempfile::tempdir().unwrap();
    let mut ledger = pay_ledger::Ledger::open(ledger_dir.path().join("ledger.sqlite")).unwrap();
    let receipt = pay_ledger::markets::worker::FundingReceipt {
        payment_hash: "9b".repeat(32),
        received_msat: PRICE as i64,
        platform_fee_msat: 0,
        received_at: (NOW + 3700) as i64,
    };
    assert!(
        net.hosts
            .get_mut(&PROVIDER)
            .unwrap()
            .fund(&mut ledger, &pins(), &receipt)
            .is_err()
    );
}
