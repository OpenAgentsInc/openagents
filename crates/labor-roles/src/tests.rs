//! Four isolated participants (buyer, provider/worker side, reviewer,
//! resolver), each with its own key, directory and journal, exchanging
//! sealed records; obligations recorded in a real pay-ledger with fake
//! funding receipts. No money moves.
use crate::book::{Applied, Collection, Final};
use crate::host::Host;
use crate::records::*;
use crate::terms::{Deliverable, Grant, Relationship, Rights, Terms, Use};
use std::collections::BTreeMap;
use std::path::Path;

const NOW: u64 = 1_800_000_000;
const PRICE: u64 = 21_000;

fn secret(n: u8) -> secp256k1::SecretKey {
    secp256k1::SecretKey::from_byte_array([n; 32]).unwrap()
}
fn key(n: u8) -> String {
    secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &secret(n))
        .x_only_public_key()
        .0
        .to_string()
}
const BUYER: u8 = 1;
const PROVIDER: u8 = 2;
const WORKER: u8 = 3;
const REVIEWER: u8 = 4;
const RESOLVER: u8 = 5;

fn terms(max_reworks: u8) -> Terms {
    let roles = [BUYER, PROVIDER, WORKER, REVIEWER, RESOLVER].map(key);
    Terms {
        v: crate::terms::TERMS_V.into(),
        market: "aa".repeat(32),
        order: "0d".repeat(32),
        buyer: roles[0].clone(),
        provider: roles[1].clone(),
        worker: roles[2].clone(),
        reviewer: roles[3].clone(),
        resolver: roles[4].clone(),
        resolver_policy: "labor-evidence-v1".into(),
        cancellation: "evaluate-delivered-work-v1".into(),
        partial_delivery: "no-partial-payment-v1".into(),
        buyer_unavailable: "resolver-required-v1".into(),
        role_relationships: vec![
            Relationship {
                pubkey: roles[0].clone(),
                operator: "buyer-co".into(),
            },
            Relationship {
                pubkey: roles[1].clone(),
                operator: "provider-co".into(),
            },
            Relationship {
                pubkey: roles[2].clone(),
                operator: "provider-co".into(),
            },
            Relationship {
                pubkey: roles[3].clone(),
                operator: "review-co".into(),
            },
            Relationship {
                pubkey: roles[4].clone(),
                operator: "resolve-co".into(),
            },
        ],
        deliverables: vec![Deliverable {
            id: "patch".into(),
            max_bytes: 4096,
        }],
        criteria: vec!["tests-pass".into(), "returns-42".into()],
        max_reworks,
        delivery_due_at: NOW + 1000,
        rework_due_at: (max_reworks > 0).then_some(NOW + 2000),
        review_due_at: NOW + 3000,
        dispute_due_at: NOW + 4000,
        resolution_due_at: NOW + 5000,
        payment_due_at: NOW + 6000,
        retain_until: NOW + 100_000,
        price_msat: PRICE,
        rights: Rights {
            v: crate::terms::RIGHTS_V.into(),
            license: "11".repeat(32),
            input_use: "perform-and-review-order".into(),
            output_use: "use-under-license".into(),
            publication: "deny".into(),
            training: "separate-grant".into(),
            evaluation_reuse: "deny".into(),
            redistribution: "deny".into(),
            recipients: roles.to_vec(),
            retention: "through-market-retain-until".into(),
        },
    }
}

struct Net {
    hosts: BTreeMap<u8, Host>,
    _dirs: tempfile::TempDir,
}

impl Net {
    fn new(t: &Terms) -> Self {
        let dirs = tempfile::tempdir().unwrap();
        let hosts = [BUYER, PROVIDER, WORKER, REVIEWER, RESOLVER]
            .into_iter()
            .map(|n| {
                (
                    n,
                    Host::open(&dirs.path().join(n.to_string()), secret(n), t.clone(), NOW)
                        .unwrap(),
                )
            })
            .collect();
        Self { hosts, _dirs: dirs }
    }
    fn path(&self, n: u8) -> std::path::PathBuf {
        self._dirs.path().join(n.to_string())
    }
    /// `who` issues `record` and every other participant receives it.
    fn send(&mut self, who: u8, record: Record, at: u64) -> Result<String, String> {
        let digest = Host::digest(&record)?;
        let events = self
            .hosts
            .get_mut(&who)
            .unwrap()
            .issue(&record, at, [who; 32])?;
        for event in &events {
            for (n, host) in &mut self.hosts {
                if *n != who {
                    host.receive(event, at)?;
                }
            }
        }
        Ok(digest)
    }
    fn fin(&self, n: u8) -> Final {
        self.hosts[&n].book.final_state()
    }
}

fn submission(number: u32, previous: Option<&str>, rework: Option<&str>, size: u64) -> Record {
    Record::Submission(Submission {
        v: SUBMISSION_V.into(),
        issuer: key(WORKER),
        order: "0d".repeat(32),
        number,
        previous: previous.map(Into::into),
        rework: rework.map(Into::into),
        executions: vec![format!("sha256:{}", "e1".repeat(32))],
        deliverables: vec![DeliveredItem {
            id: "patch".into(),
            digest: format!("sha256:{:064x}", number + 7),
            size,
        }],
        run_evidence: vec![format!("sha256:{}", "e2".repeat(32))],
        limitations: format!("sha256:{}", "e3".repeat(32)),
    })
}

fn delivery(issuer: u8, sub: &str, at: u64) -> Record {
    Record::Delivery(Delivery {
        v: DELIVERY_V.into(),
        issuer: key(issuer),
        order: "0d".repeat(32),
        submission: sub.into(),
        received_at: at,
        available: true,
    })
}

fn verification(sub: &str, failed: bool) -> Record {
    let c = |id: &str, verdict| Criterion {
        id: id.into(),
        verdict,
        evidence: vec![format!("sha256:{}", "c1".repeat(32))],
    };
    Record::Verification(Verification {
        v: VERIFICATION_V.into(),
        issuer: key(REVIEWER),
        order: "0d".repeat(32),
        submission: sub.into(),
        policy: format!("sha256:{}", "70".repeat(32)),
        checker_receipts: vec![format!("sha256:{}", "c2".repeat(32))],
        criteria: vec![
            c("tests-pass", Verdict::Passed),
            c(
                "returns-42",
                if failed {
                    Verdict::Failed
                } else {
                    Verdict::Passed
                },
            ),
        ],
        verdict: if failed {
            Verdict::Failed
        } else {
            Verdict::Passed
        },
        limitations: format!("sha256:{}", "c3".repeat(32)),
    })
}

fn review(sub: &str, ver: &str, decision: ReviewDecision, criteria: &[&str]) -> Record {
    Record::Review(Review {
        v: REVIEW_V.into(),
        issuer: key(BUYER),
        order: "0d".repeat(32),
        submission: sub.into(),
        verification: ver.into(),
        decision,
        criteria: criteria.iter().map(|c| (*c).to_string()).collect(),
        reason: format!("sha256:{}", "a1".repeat(32)),
    })
}

fn buyer_acceptance(sub: &str, ver: &str, rev: &str) -> Record {
    Record::Acceptance(Acceptance {
        v: ACCEPTANCE_V.into(),
        issuer: key(BUYER),
        order: "0d".repeat(32),
        submission: Some(sub.into()),
        verification: Some(ver.into()),
        outcome: Outcome::Accepted,
        basis: AcceptanceBasis::BuyerAcceptance,
        review: Some(rev.into()),
        resolution: None,
        amount_due_msat: Some(PRICE),
        supersedes: vec![],
        evidence: vec![rev.into()],
    })
}

fn dispute(issuer: u8, subject: &str, cause: Cause) -> Record {
    Record::Dispute(Dispute {
        v: DISPUTE_V.into(),
        issuer: key(issuer),
        order: "0d".repeat(32),
        subject: subject.into(),
        cause,
        evidence: vec![subject.into()],
    })
}

fn resolution(
    sub: Option<&str>,
    ver: Option<&str>,
    disputes: &[&str],
    decision: Decision,
    basis: Basis,
    evidence: &[&str],
) -> Record {
    Record::Resolution(Resolution {
        v: RESOLUTION_V.into(),
        issuer: key(RESOLVER),
        order: "0d".repeat(32),
        submission: sub.map(Into::into),
        verification: ver.map(Into::into),
        disputes: disputes.iter().map(|d| (*d).to_string()).collect(),
        decision,
        basis,
        evidence: evidence.iter().map(|d| (*d).to_string()).collect(),
        reason: format!("sha256:{}", "b1".repeat(32)),
    })
}

fn resolver_acceptance(
    res: &str,
    sub: Option<&str>,
    ver: Option<&str>,
    outcome: Outcome,
    amount: Option<u64>,
    supersedes: &[&str],
) -> Record {
    Record::Acceptance(Acceptance {
        v: ACCEPTANCE_V.into(),
        issuer: key(RESOLVER),
        order: "0d".repeat(32),
        submission: sub.map(Into::into),
        verification: ver.map(Into::into),
        outcome,
        basis: AcceptanceBasis::ResolverDecision,
        review: None,
        resolution: Some(res.into()),
        amount_due_msat: amount,
        supersedes: supersedes.iter().map(|d| (*d).to_string()).collect(),
        evidence: vec![res.into()],
    })
}

fn cancellation(issuer: u8) -> Record {
    Record::Cancellation(Cancellation {
        v: CANCELLATION_V.into(),
        issuer: key(issuer),
        order: "0d".repeat(32),
        reason: format!("sha256:{}", "cc".repeat(32)),
        execution: None,
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

fn receipt(hash: u8, fee: i64, at: u64) -> pay_ledger::markets::worker::FundingReceipt {
    pay_ledger::markets::worker::FundingReceipt {
        payment_hash: format!("{hash:02x}").repeat(32),
        received_msat: PRICE as i64 + fee,
        platform_fee_msat: fee,
        received_at: at as i64,
    }
}

fn ledger(dir: &Path) -> pay_ledger::Ledger {
    pay_ledger::Ledger::open(dir.join("ledger.sqlite")).unwrap()
}

#[test]
fn rework_then_acceptance_records_an_obligation_that_only_funding_settles() {
    let t = terms(1);
    let mut net = Net::new(&t);
    let s0 = net
        .send(WORKER, submission(0, None, None, 100), NOW + 100)
        .unwrap();
    net.send(BUYER, delivery(BUYER, &s0, NOW + 110), NOW + 110)
        .unwrap();
    let v0 = net
        .send(REVIEWER, verification(&s0, true), NOW + 120)
        .unwrap();
    // An accept of a failed verification refuses; rework of an unknown criterion too.
    assert!(
        net.send(
            BUYER,
            review(&s0, &v0, ReviewDecision::Accept, &[]),
            NOW + 130
        )
        .is_err()
    );
    assert!(
        net.send(
            BUYER,
            review(
                &s0,
                &v0,
                ReviewDecision::RequestRework,
                &["new-requirement"]
            ),
            NOW + 130
        )
        .is_err()
    );
    let r0 = net
        .send(
            BUYER,
            review(&s0, &v0, ReviewDecision::RequestRework, &["returns-42"]),
            NOW + 130,
        )
        .unwrap();
    // A replacement without the rework request, or out of order, refuses.
    assert!(
        net.send(WORKER, submission(1, Some(&s0), None, 100), NOW + 140)
            .is_err()
    );
    assert!(
        net.send(WORKER, submission(2, Some(&s0), Some(&r0), 100), NOW + 140)
            .is_err()
    );
    let s1 = net
        .send(WORKER, submission(1, Some(&s0), Some(&r0), 120), NOW + 1500)
        .unwrap();
    // The rework allowance is spent.
    assert!(
        net.send(WORKER, submission(2, Some(&s1), Some(&r0), 100), NOW + 1600)
            .is_err()
    );
    net.send(BUYER, delivery(BUYER, &s1, NOW + 1510), NOW + 1510)
        .unwrap();
    let v1 = net
        .send(REVIEWER, verification(&s1, false), NOW + 1520)
        .unwrap();
    assert!(
        net.send(
            BUYER,
            review(&s1, &v1, ReviewDecision::RequestRework, &["returns-42"]),
            NOW + 1530
        )
        .unwrap_err()
        .contains("no rework")
    );
    // A passed check alone owes nothing.
    assert_eq!(net.hosts[&PROVIDER].book.collection(), Collection::None);
    let r1 = net
        .send(
            BUYER,
            review(&s1, &v1, ReviewDecision::Accept, &[]),
            NOW + 1530,
        )
        .unwrap();
    let a = net
        .send(BUYER, buyer_acceptance(&s1, &v1, &r1), NOW + 1540)
        .unwrap();
    for n in [BUYER, PROVIDER, WORKER, REVIEWER, RESOLVER] {
        assert_eq!(
            net.fin(n),
            Final::Accepted {
                acceptance: a.clone(),
                amount_msat: PRICE
            }
        );
    }
    // Accepted, not yet collected: unknown, retained across a restart.
    assert_eq!(
        net.hosts[&PROVIDER].book.collection(),
        Collection::Unknown {
            acceptance: a.clone(),
            amount_msat: PRICE
        }
    );
    drop(net.hosts.remove(&PROVIDER));
    let reopened =
        Host::open(&net.path(PROVIDER), secret(PROVIDER), t.clone(), NOW + 1600).unwrap();
    assert_eq!(
        reopened.book.collection(),
        Collection::Unknown {
            acceptance: a.clone(),
            amount_msat: PRICE
        }
    );
    net.hosts.insert(PROVIDER, reopened);
    // Funding from the central receive adapter reaches the ledger once.
    let ledger_dir = tempfile::tempdir().unwrap();
    let mut l = ledger(ledger_dir.path());
    let host = net.hosts.get_mut(&PROVIDER).unwrap();
    assert!(
        host.fund(&mut l, &pins(), &receipt(0x99, 100, NOW + 7000))
            .is_err(),
        "funding after the payment deadline refuses"
    );
    let first = host
        .fund(&mut l, &pins(), &receipt(0x99, 100, NOW + 1700))
        .unwrap();
    let again = host
        .fund(&mut l, &pins(), &receipt(0x99, 100, NOW + 1700))
        .unwrap();
    assert_eq!(first.key, again.key);
    assert!(
        host.fund(&mut l, &pins(), &receipt(0x98, 100, NOW + 1700))
            .is_err(),
        "a second payment for one obligation conflicts"
    );
    assert!(matches!(host.book.collection(), Collection::Funded { .. }));
    drop(net.hosts.remove(&PROVIDER));
    let reopened = Host::open(&net.path(PROVIDER), secret(PROVIDER), t, NOW + 1800).unwrap();
    assert!(matches!(
        reopened.book.collection(),
        Collection::Funded { .. }
    ));
    net.hosts.insert(PROVIDER, reopened);
    // No submission can follow acceptance.
    assert!(
        net.send(WORKER, submission(2, Some(&s1), Some(&r0), 100), NOW + 1900)
            .is_err()
    );
}

#[test]
fn signers_deliverables_numbers_and_deadlines_are_enforced() {
    let t = terms(1);
    let mut net = Net::new(&t);
    // The provider is not the worker; the worker is not the reviewer.
    let mut forged = submission(0, None, None, 100);
    if let Record::Submission(s) = &mut forged {
        s.issuer = key(PROVIDER);
    }
    assert!(net.send(PROVIDER, forged, NOW).is_err());
    let s0 = net
        .send(WORKER, submission(0, None, None, 100), NOW + 100)
        .unwrap();
    let mut by_worker = verification(&s0, false);
    if let Record::Verification(v) = &mut by_worker {
        v.issuer = key(WORKER);
    }
    assert!(net.send(WORKER, by_worker, NOW + 110).is_err());
    // A host refuses a record whose issuer field names someone else.
    let mut lying = verification(&s0, false);
    if let Record::Verification(v) = &mut lying {
        v.issuer = key(REVIEWER);
    }
    assert!(
        net.hosts
            .get_mut(&WORKER)
            .unwrap()
            .book
            .apply(lying, &key(WORKER), NOW)
            .is_err()
    );
    // Same-number substitution, oversized and missing deliverables.
    assert!(
        net.send(WORKER, submission(0, None, None, 101), NOW + 120)
            .unwrap_err()
            .contains("substitution")
    );
    assert!(
        net.send(WORKER, submission(1, Some(&s0), None, 9999), NOW + 120)
            .is_err()
    );
    let mut missing = submission(0, None, None, 100);
    if let Record::Submission(s) = &mut missing {
        s.deliverables.clear();
    }
    assert!(net.send(WORKER, missing, NOW + 120).is_err());
    // A late delivery cannot support acceptance, and a verdict must follow its criteria.
    net.send(BUYER, delivery(BUYER, &s0, NOW + 1200), NOW + 1200)
        .unwrap();
    let mut wrong = verification(&s0, true);
    if let Record::Verification(v) = &mut wrong {
        v.verdict = Verdict::Passed;
    }
    assert!(net.send(REVIEWER, wrong, NOW + 1210).is_err());
    let v0 = net
        .send(REVIEWER, verification(&s0, false), NOW + 1210)
        .unwrap();
    assert!(
        net.send(
            BUYER,
            review(&s0, &v0, ReviewDecision::Accept, &[]),
            NOW + 1220
        )
        .unwrap_err()
        .contains("deadline")
    );
    // A delivery receipt cannot claim a future time; rework after its deadline refuses.
    assert!(
        net.send(BUYER, delivery(BUYER, &s0, NOW + 9000), NOW + 1230)
            .is_err()
    );
    assert!(
        net.send(
            BUYER,
            review(&s0, &v0, ReviewDecision::RequestRework, &["tests-pass"]),
            NOW + 2100
        )
        .unwrap_err()
        .contains("late")
    );
    // Duplicates replay as duplicates.
    let events = net
        .hosts
        .get_mut(&WORKER)
        .unwrap()
        .issue(&submission(0, None, None, 100), NOW + 130, [9; 32])
        .unwrap();
    let buyer_event = events
        .iter()
        .find(|e| e.tag_values("p").next() == Some(key(BUYER).as_str()))
        .unwrap();
    assert!(matches!(
        net.hosts
            .get_mut(&BUYER)
            .unwrap()
            .receive(buyer_event, NOW + 131)
            .unwrap(),
        Some(Applied::Duplicate(_))
    ));
}

#[test]
fn an_unavailable_buyer_does_not_forfeit_timely_conforming_work() {
    let t = terms(0);
    let mut net = Net::new(&t);
    let s0 = net
        .send(WORKER, submission(0, None, None, 100), NOW + 100)
        .unwrap();
    // The buyer never answers; the resolver records receipt.
    net.send(RESOLVER, delivery(RESOLVER, &s0, NOW + 150), NOW + 150)
        .unwrap();
    let v0 = net
        .send(REVIEWER, verification(&s0, false), NOW + 200)
        .unwrap();
    let d = net
        .send(
            PROVIDER,
            dispute(PROVIDER, &s0, Cause::BuyerUnavailable),
            NOW + 3500,
        )
        .unwrap();
    // A rejection without positive evidence refuses; unresolved is allowed but owes nothing yet.
    assert!(
        net.send(
            RESOLVER,
            resolution(
                None,
                None,
                &[&d],
                Decision::Reject,
                Basis::InsufficientEvidence,
                &[&d]
            ),
            NOW + 3600
        )
        .is_err()
    );
    // A resolution that skips a known dispute refuses.
    assert!(
        net.send(
            RESOLVER,
            resolution(
                Some(&s0),
                Some(&v0),
                &[],
                Decision::Accept,
                Basis::Delivered,
                &[&v0]
            ),
            NOW + 3600
        )
        .is_err()
    );
    let res = net
        .send(
            RESOLVER,
            resolution(
                Some(&s0),
                Some(&v0),
                &[&d],
                Decision::Accept,
                Basis::Delivered,
                &[&v0, &d],
            ),
            NOW + 3600,
        )
        .unwrap();
    assert!(
        net.send(
            RESOLVER,
            resolver_acceptance(&res, Some(&s0), Some(&v0), Outcome::Accepted, Some(0), &[]),
            NOW + 3610
        )
        .unwrap_err()
        .contains("amount")
    );
    let a = net
        .send(
            RESOLVER,
            resolver_acceptance(
                &res,
                Some(&s0),
                Some(&v0),
                Outcome::Accepted,
                Some(PRICE),
                &[],
            ),
            NOW + 3610,
        )
        .unwrap();
    assert_eq!(
        net.fin(BUYER),
        Final::Accepted {
            acceptance: a,
            amount_msat: PRICE
        }
    );
}

#[test]
fn unresolved_is_not_zero_and_a_late_decision_cannot_settle() {
    let t = terms(0);
    let mut net = Net::new(&t);
    let s0 = net
        .send(WORKER, submission(0, None, None, 100), NOW + 100)
        .unwrap();
    net.send(BUYER, delivery(BUYER, &s0, NOW + 110), NOW + 110)
        .unwrap();
    let v0 = net
        .send(REVIEWER, verification(&s0, false), NOW + 120)
        .unwrap();
    let r0 = net
        .send(
            BUYER,
            review(&s0, &v0, ReviewDecision::Reject, &["returns-42"]),
            NOW + 130,
        )
        .unwrap();
    let d = net
        .send(PROVIDER, dispute(PROVIDER, &r0, Cause::Review), NOW + 200)
        .unwrap();
    // The verification passed, so the resolver cannot reject on criteria.
    assert!(
        net.send(
            RESOLVER,
            resolution(
                Some(&s0),
                Some(&v0),
                &[&d],
                Decision::Reject,
                Basis::Criteria,
                &[&v0]
            ),
            NOW + 300
        )
        .is_err()
    );
    assert!(
        net.send(
            RESOLVER,
            resolution(
                Some(&s0),
                Some(&v0),
                &[&d],
                Decision::Accept,
                Basis::Delivered,
                &[&v0]
            ),
            NOW + 6000
        )
        .unwrap_err()
        .contains("deadline")
    );
    let res = net
        .send(
            RESOLVER,
            resolution(
                Some(&s0),
                Some(&v0),
                &[&d],
                Decision::Unresolved,
                Basis::InsufficientEvidence,
                &[&d],
            ),
            NOW + 6000,
        )
        .unwrap();
    let a = net
        .send(
            RESOLVER,
            resolver_acceptance(&res, Some(&s0), Some(&v0), Outcome::Unresolved, None, &[]),
            NOW + 6010,
        )
        .unwrap();
    assert_eq!(net.fin(PROVIDER), Final::Unresolved { acceptance: a });
    assert_eq!(net.hosts[&PROVIDER].book.collection(), Collection::None);
    let ledger_dir = tempfile::tempdir().unwrap();
    assert!(
        net.hosts
            .get_mut(&PROVIDER)
            .unwrap()
            .fund(
                &mut ledger(ledger_dir.path()),
                &pins(),
                &receipt(0x97, 0, NOW + 5500)
            )
            .is_err()
    );
}

#[test]
fn cancellation_before_during_and_after_delivery() {
    // Before any delivery: both cancel, zero is owed.
    let t = terms(0);
    let mut net = Net::new(&t);
    let cb = net.send(BUYER, cancellation(BUYER), NOW + 50).unwrap();
    let mutual = |evidence: Vec<String>| {
        Record::Acceptance(Acceptance {
            v: ACCEPTANCE_V.into(),
            issuer: key(BUYER),
            order: "0d".repeat(32),
            submission: None,
            verification: None,
            outcome: Outcome::Rejected,
            basis: AcceptanceBasis::MutualCancellation,
            review: None,
            resolution: None,
            amount_due_msat: Some(0),
            supersedes: vec![],
            evidence,
        })
    };
    // One side alone cannot close the order.
    assert!(net.send(BUYER, mutual(vec![cb.clone()]), NOW + 60).is_err());
    assert_eq!(net.fin(PROVIDER), Final::Open);
    let cp = net
        .send(PROVIDER, cancellation(PROVIDER), NOW + 70)
        .unwrap();
    let a = net.send(BUYER, mutual(vec![cb, cp]), NOW + 80).unwrap();
    assert_eq!(net.fin(WORKER), Final::Rejected { acceptance: a });

    // After conforming delivery: the buyer cannot cancel to avoid assessing it.
    let mut net = Net::new(&t);
    let s0 = net
        .send(WORKER, submission(0, None, None, 100), NOW + 100)
        .unwrap();
    net.send(BUYER, delivery(BUYER, &s0, NOW + 110), NOW + 110)
        .unwrap();
    let v0 = net
        .send(REVIEWER, verification(&s0, false), NOW + 120)
        .unwrap();
    let cb = net.send(BUYER, cancellation(BUYER), NOW + 130).unwrap();
    let cp = net
        .send(PROVIDER, cancellation(PROVIDER), NOW + 140)
        .unwrap();
    assert!(
        net.send(BUYER, mutual(vec![cb.clone(), cp.clone()]), NOW + 150)
            .unwrap_err()
            .contains("qualifying delivery")
    );
    // Nor can the resolver reject on cancellation; it can accept the work.
    let d = net
        .send(
            PROVIDER,
            dispute(PROVIDER, &cb, Cause::Cancellation),
            NOW + 160,
        )
        .unwrap();
    assert!(
        net.send(
            RESOLVER,
            resolution(
                None,
                None,
                &[&d],
                Decision::Reject,
                Basis::Cancellation,
                &[&cb]
            ),
            NOW + 170
        )
        .is_err()
    );
    let res = net
        .send(
            RESOLVER,
            resolution(
                Some(&s0),
                Some(&v0),
                &[&d],
                Decision::Accept,
                Basis::Delivered,
                &[&v0, &cb, &cp],
            ),
            NOW + 170,
        )
        .unwrap();
    let a = net
        .send(
            RESOLVER,
            resolver_acceptance(
                &res,
                Some(&s0),
                Some(&v0),
                Outcome::Accepted,
                Some(PRICE),
                &[],
            ),
            NOW + 180,
        )
        .unwrap();
    // A later cancellation does not undo acceptance.
    net.send(BUYER, cancellation(BUYER).clone(), NOW + 190).ok();
    assert_eq!(
        net.fin(BUYER),
        Final::Accepted {
            acceptance: a,
            amount_msat: PRICE
        }
    );

    // During unknown execution: a one-sided cancellation leaves the order open,
    // and the resolver rejects on cancellation only with no qualifying delivery.
    let mut net = Net::new(&t);
    let cb = net.send(BUYER, cancellation(BUYER), NOW + 50).unwrap();
    let d = net
        .send(
            PROVIDER,
            dispute(PROVIDER, &cb, Cause::Cancellation),
            NOW + 60,
        )
        .unwrap();
    assert_eq!(net.fin(PROVIDER), Final::Open);
    let res = net
        .send(
            RESOLVER,
            resolution(
                None,
                None,
                &[&d],
                Decision::Reject,
                Basis::Cancellation,
                &[&cb],
            ),
            NOW + 70,
        )
        .unwrap();
    let a = net
        .send(
            RESOLVER,
            resolver_acceptance(&res, None, None, Outcome::Rejected, Some(0), &[]),
            NOW + 80,
        )
        .unwrap();
    assert_eq!(net.fin(BUYER), Final::Rejected { acceptance: a });
}

#[test]
fn superseding_a_funded_acceptance_owes_a_refund_and_competing_finals_conflict() {
    let t = terms(0);
    let mut net = Net::new(&t);
    let s0 = net
        .send(WORKER, submission(0, None, None, 100), NOW + 100)
        .unwrap();
    net.send(BUYER, delivery(BUYER, &s0, NOW + 110), NOW + 110)
        .unwrap();
    let v0 = net
        .send(REVIEWER, verification(&s0, false), NOW + 120)
        .unwrap();
    let r0 = net
        .send(
            BUYER,
            review(&s0, &v0, ReviewDecision::Accept, &[]),
            NOW + 130,
        )
        .unwrap();
    let a0 = net
        .send(BUYER, buyer_acceptance(&s0, &v0, &r0), NOW + 140)
        .unwrap();
    let ledger_dir = tempfile::tempdir().unwrap();
    let mut l = ledger(ledger_dir.path());
    net.hosts
        .get_mut(&PROVIDER)
        .unwrap()
        .fund(&mut l, &pins(), &receipt(0x96, 0, NOW + 150))
        .unwrap();
    // An uncontested acceptance cannot be superseded.
    let late_v = net
        .send(REVIEWER, verification(&s0, true), NOW + 200)
        .unwrap();
    assert!(
        net.send(
            RESOLVER,
            resolution(
                Some(&s0),
                Some(&late_v),
                &[],
                Decision::Reject,
                Basis::Criteria,
                &[&late_v, &a0]
            ),
            NOW + 210
        )
        .is_ok()
    );
    let res_unc = Host::digest(&resolution(
        Some(&s0),
        Some(&late_v),
        &[],
        Decision::Reject,
        Basis::Criteria,
        &[&late_v, &a0],
    ))
    .unwrap();
    assert!(
        net.send(
            RESOLVER,
            resolver_acceptance(&res_unc, None, None, Outcome::Rejected, Some(0), &[&a0]),
            NOW + 220
        )
        .unwrap_err()
        .contains("uncontested")
    );
    // With a dispute on record, the resolver can supersede it: the funded
    // payment becomes a refund owed, with no automatic transfer.
    let d = net
        .send(BUYER, dispute(BUYER, &a0, Cause::Conflict), NOW + 230)
        .unwrap();
    let res = net
        .send(
            RESOLVER,
            resolution(
                Some(&s0),
                Some(&late_v),
                &[&d],
                Decision::Reject,
                Basis::Criteria,
                &[&late_v, &a0, &d],
            ),
            NOW + 240,
        )
        .unwrap();
    net.send(
        RESOLVER,
        resolver_acceptance(
            &res,
            Some(&s0),
            Some(&late_v),
            Outcome::Rejected,
            Some(0),
            &[&a0],
        ),
        NOW + 250,
    )
    .unwrap();
    assert!(matches!(
        net.hosts[&PROVIDER].book.collection(),
        Collection::RefundOwed { .. }
    ));

    // Two finals that do not supersede each other halt settlement.
    let mut net = Net::new(&t);
    let s0 = net
        .send(WORKER, submission(0, None, None, 100), NOW + 100)
        .unwrap();
    net.send(BUYER, delivery(BUYER, &s0, NOW + 110), NOW + 110)
        .unwrap();
    let v0 = net
        .send(REVIEWER, verification(&s0, false), NOW + 120)
        .unwrap();
    let r0 = net
        .send(
            BUYER,
            review(&s0, &v0, ReviewDecision::Accept, &[]),
            NOW + 130,
        )
        .unwrap();
    net.send(BUYER, buyer_acceptance(&s0, &v0, &r0), NOW + 140)
        .unwrap();
    let res = net
        .send(
            RESOLVER,
            resolution(
                Some(&s0),
                Some(&v0),
                &[],
                Decision::Unresolved,
                Basis::InsufficientEvidence,
                &[&v0],
            ),
            NOW + 150,
        )
        .unwrap();
    net.send(
        RESOLVER,
        resolver_acceptance(&res, Some(&s0), Some(&v0), Outcome::Unresolved, None, &[]),
        NOW + 160,
    )
    .unwrap();
    assert!(matches!(net.fin(PROVIDER), Final::Conflict(_)));
    let ledger_dir = tempfile::tempdir().unwrap();
    assert!(
        net.hosts
            .get_mut(&PROVIDER)
            .unwrap()
            .fund(
                &mut ledger(ledger_dir.path()),
                &pins(),
                &receipt(0x95, 0, NOW + 170)
            )
            .is_err()
    );
}

#[test]
fn terms_admit_only_independent_roles_and_rights_need_grants() {
    let base = terms(1);
    base.admit(NOW).unwrap();
    let mut shared = base.clone();
    shared.role_relationships[3].operator = "provider-co".into();
    assert!(shared.admit(NOW).unwrap_err().contains("reviewer shares"));
    let mut resolver_buyer = base.clone();
    resolver_buyer.role_relationships[4].operator = "buyer-co".into();
    assert!(
        resolver_buyer
            .admit(NOW)
            .unwrap_err()
            .contains("resolver shares")
    );
    let mut no_deadline = base.clone();
    no_deadline.rework_due_at = None;
    assert!(no_deadline.admit(NOW).is_err());
    let mut too_many = base.clone();
    too_many.max_reworks = 4;
    assert!(too_many.admit(NOW).is_err());
    let mut narrow = base.clone();
    narrow.rights.recipients.pop();
    assert!(narrow.admit(NOW).unwrap_err().contains("recipients"));
    let mut undisclosed = base.clone();
    undisclosed.role_relationships.pop();
    assert!(undisclosed.admit(NOW).is_err());
    // A key that holds no role cannot open a host.
    let dir = tempfile::tempdir().unwrap();
    assert!(Host::open(dir.path(), secret(9), base.clone(), NOW).is_err());
    // Rights: review is allowed to recipients, training only with both grants.
    let r = &base.rights;
    assert!(r.permits(&base, Use::PerformAndReview, &key(REVIEWER), &[]));
    assert!(!r.permits(&base, Use::PerformAndReview, &key(9), &[]));
    assert!(r.permits(&base, Use::UseOutput, &key(BUYER), &[]));
    assert!(!r.permits(&base, Use::Publish, &key(BUYER), &[]));
    let half = Grant {
        order: base.order.clone(),
        purpose: "training".into(),
        recipient: key(BUYER),
        granted_by: vec![key(BUYER)],
    };
    assert!(!r.permits(&base, Use::Train, &key(BUYER), std::slice::from_ref(&half)));
    let both = Grant {
        granted_by: vec![key(BUYER), key(PROVIDER)],
        ..half
    };
    assert!(r.permits(&base, Use::Train, &key(BUYER), &[both]));
}

#[test]
fn repair_context_keeps_original_entries_and_only_adds_exact_authorized_evidence() {
    use crate::context::{self, Bytes};
    use serde_json::{Value, json};
    let t = terms(1);
    let mut book = crate::book::Book::new(t.clone(), NOW).unwrap();
    let first = submission(0, None, None, 10);
    let first_digest = crate::digest_of(&first).unwrap();
    book.apply(first, &key(WORKER), NOW).unwrap();
    let verify = verification(&first_digest, true);
    let verification_digest = crate::digest_of(&verify).unwrap();
    book.apply(verify, &key(REVIEWER), NOW + 1).unwrap();
    let review = review(
        &first_digest,
        &verification_digest,
        ReviewDecision::RequestRework,
        &["returns-42"],
    );
    let digest = crate::digest_of(&review).unwrap();
    book.apply(review.clone(), &key(BUYER), NOW + 2).unwrap();
    let raw = nostr::contracts::jcs(&serde_json::to_value(review).unwrap()).unwrap();
    let reference =
        json!({"digest":digest,"size":raw.len(),"media_type":"application/json","schema":REVIEW_V});
    let task = "73".repeat(32);
    let mut descriptor = json!({"v":"openagents.evidence.v1","requires":[],"content":reference,
        "source":{"kind":"document","identity":t.order,"version":digest,"observation":reference},
        "capture":{"complete":true,"omitted_bytes":0},"derived_from":[],
        "scope":{"task":task,"recipients":[t.worker],"classification":"private"}});
    let evidence_id = crate::digest_of(&descriptor).unwrap();
    descriptor["id"] = json!(evidence_id);
    let initial = json!({"v":"openagents.context.v1","requires":[],"task":task,"recipient":t.worker,
        "policy":{"digest":format!("sha256:{}","88".repeat(32)),"size":2,"media_type":"application/json"},
        "entries":[{"evidence":format!("sha256:{}","99".repeat(32)),"representation":{"digest":format!("sha256:{}","aa".repeat(32)),"size":3,"media_type":"application/json"},"mandatory":false}],
        "omissions":[],"coverage":"complete"});
    let mut proposed = initial.clone();
    proposed["entries"]
        .as_array_mut()
        .unwrap()
        .push(json!({"evidence":evidence_id,"representation":reference,"mandatory":true}));
    let evidence = BTreeMap::from([(evidence_id.clone(), descriptor.clone())]);
    let bytes = Bytes::from([(digest.clone(), raw)]);
    let check = |value: &Value| {
        context::admit_rework(
            &book,
            1,
            &digest,
            &initial,
            value,
            &evidence,
            &bytes,
            NOW + 3,
        )
    };
    check(&proposed).unwrap();
    let mut changed = proposed.clone();
    changed["entries"][0]["mandatory"] = json!(true);
    assert!(check(&changed).is_err());
    changed = proposed.clone();
    changed["entries"].as_array_mut().unwrap().remove(0);
    assert!(check(&changed).is_err());
    changed = proposed.clone();
    changed["recipient"] = json!(key(RESOLVER));
    assert!(check(&changed).is_err());
    changed = proposed.clone();
    changed["entries"][1]["representation"]["digest"] =
        json!(format!("sha256:{}", "ab".repeat(32)));
    assert!(check(&changed).is_err());
    changed = proposed.clone();
    changed["entries"][1]["mandatory"] = json!(false);
    assert!(check(&changed).is_err());
    assert!(
        context::admit_rework(
            &book,
            1,
            &digest,
            &initial,
            &proposed,
            &evidence,
            &bytes,
            t.due(1) + 1
        )
        .is_err()
    );
    let mut corrupt = bytes.clone();
    corrupt.get_mut(&digest).unwrap().push(0);
    assert!(
        context::admit_rework(
            &book,
            1,
            &digest,
            &initial,
            &proposed,
            &evidence,
            &corrupt,
            NOW + 3
        )
        .is_err()
    );
    book.apply(cancellation(BUYER), &key(BUYER), NOW + 4)
        .unwrap();
    assert!(
        context::admit_rework(
            &book,
            1,
            &digest,
            &initial,
            &proposed,
            &evidence,
            &bytes,
            NOW + 5
        )
        .is_err()
    );
}

#[test]
fn journal_replay_requires_original_bytes_terms_and_one_writer() {
    use serde_json::{Value, json};
    let t = terms(1);
    let mut net = Net::new(&t);
    let record = submission(0, None, None, 10);
    net.send(WORKER, record, NOW + 1).unwrap();
    let path = net.path(BUYER);
    assert!(
        Host::open(&path, secret(BUYER), t.clone(), NOW + 2)
            .unwrap_err()
            .contains("writer")
    );
    drop(net.hosts.remove(&BUYER));
    let journal = path.join("journal.json");
    let original = std::fs::read(&journal).unwrap();
    let mut changed: Value = serde_json::from_slice(&original).unwrap();
    changed["lines"][0]["record"]["limitations"] = json!(format!("sha256:{}", "ff".repeat(32)));
    std::fs::write(&journal, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(
        Host::open(&path, secret(BUYER), t.clone(), NOW + 2)
            .unwrap_err()
            .contains("signed bytes")
    );
    changed = serde_json::from_slice(&original).unwrap();
    changed["lines"][0]["event"] = Value::Null;
    std::fs::write(&journal, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(
        Host::open(&path, secret(BUYER), t.clone(), NOW + 2)
            .unwrap_err()
            .contains("no original")
    );
    std::fs::write(&journal, &original).unwrap();
    let mut other = t.clone();
    other.price_msat += 1;
    assert!(
        Host::open(&path, secret(BUYER), other, NOW + 2)
            .unwrap_err()
            .contains("terms")
    );
    let host = Host::open(&path, secret(BUYER), t.clone(), NOW + 2).unwrap();
    assert_eq!(host.book.entries().len(), 1);
    drop(host);
    std::fs::remove_file(&journal).unwrap();
    assert!(
        Host::open(&path, secret(BUYER), t, NOW + 2)
            .unwrap_err()
            .contains("history cannot restart")
    );
}

#[test]
fn an_uncertain_journal_write_cannot_publish_a_nondurable_duplicate() {
    let t = terms(0);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("worker");
    let mut host = Host::open(&path, secret(WORKER), t.clone(), NOW).unwrap();
    std::fs::create_dir(path.join("journal.json.tmp")).unwrap();
    let record = submission(0, None, None, 10);
    assert!(host.issue(&record, NOW + 1, [4; 32]).is_err());
    std::fs::remove_dir(path.join("journal.json.tmp")).unwrap();
    assert!(
        host.issue(&record, NOW + 1, [4; 32])
            .unwrap_err()
            .contains("reopening")
    );
    drop(host);
    let mut host = Host::open(&path, secret(WORKER), t, NOW + 2).unwrap();
    assert!(host.book.entries().is_empty());
    assert!(!host.issue(&record, NOW + 2, [5; 32]).unwrap().is_empty());
    assert_eq!(host.book.entries().len(), 1);
}

#[path = "tests/wire.rs"]
mod wire;
