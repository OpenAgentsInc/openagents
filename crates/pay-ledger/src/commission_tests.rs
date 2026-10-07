use super::*;
use crate::{Payee, PayoutState, Rail, SettlementInput, Split};
fn admission(l: &Ledger) -> Admission {
    Admission {
        schema: SCHEMA.into(),
        id: "a".repeat(64),
        ledger_origin: l.origin().unwrap(),
        payment_hash: "b".repeat(64),
        request_hash: "c".repeat(64),
        authorization: "d".repeat(64),
        buyer_account: "buyer".into(),
        buyer_workspace: "ws".into(),
        operator_account: "operator".into(),
        operator_workspace: "merchant-workspace".into(),
        customer: "buyer".into(),
        referrer: "source".into(),
        party: "referrer:source".into(),
        agreement: "sha256:agreement".into(),
        terms: "sha256:terms".into(),
        contract: "synthetic bilateral contract".into(),
        offer_digest: "offer".into(),
        invoice: "synthetic retained invoice".into(),
        receiver: "receiver".into(),
        payer: "payer".into(),
        plugin: "plugin".into(),
        release: "release".into(),
        author: "author".into(),
        author_fee_msat: 1000,
        price_msat: 10_000,
        numerator: 1,
        denominator: 2,
        exact_rounding: false,
        hold_secs: 60,
        minimum_msat: 1000,
        destinations: vec!["spark".into()],
        costs: [
            "model", "compute", "payment", "delivery", "support", "other",
        ]
        .into_iter()
        .map(|category| Cost {
            category: category.into(),
            amount_msat: Some(0),
            provenance: "operator-declared".into(),
            evidence: "synthetic explicitly declared zero".into(),
        })
        .collect(),
        cost_policy: "policy".into(),
        admitted_at: 1_800_000_000,
    }
}
fn settled(l: &mut Ledger, a: &Admission) {
    // The native launch window has ended; this small first-call bonus remains unfunded.
    l.record_settlement(SettlementInput {
        key: a.payment_hash.clone(),
        resource: "plugin".into(),
        plugin_id: Some(a.plugin.clone()),
        release_id: Some(a.release.clone()),
        price_msat: a.price_msat as i64,
        received_msat: a.price_msat as i64,
        rail: Rail::Lightning,
        payer_alias: None,
        settled_at: 1_900_000_000,
        split: Split::Plugin {
            author: a.author.clone(),
            fee_msat: a.author_fee_msat as i64,
        },
    })
    .unwrap();
}
#[test]
fn replay_hold_restart_conservation_and_remainder() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite");
    let mut l = Ledger::open(&path).unwrap();
    let a = admission(&l);
    l.admit_commission(&a).unwrap();
    settled(&mut l, &a);
    let original = l.settlement(&a.payment_hash).unwrap().unwrap();
    let held = l
        .observe_commission(&a.id, &"e".repeat(64), Some(true), 1_900_000_010)
        .unwrap();
    assert_eq!(held.state, "held");
    assert!(held.held_msat > 0);
    let c = l
        .observe_commission(&a.id, &"e".repeat(64), Some(true), 1_900_000_061)
        .unwrap();
    assert_eq!(c.state, "earned");
    assert_eq!(c.retained_remainder_msat, c.earned_msat % 1000);
    let sum: i64 = l
        .settlement_liabilities(&a.payment_hash)
        .unwrap()
        .iter()
        .map(|s| s.amount_msat)
        .sum();
    assert_eq!(
        sum + l.commission_held_liability().unwrap(),
        original.received_msat
    );
    drop(l);
    let mut l = Ledger::open(&path).unwrap();
    assert_eq!(
        l.observe_commission(&a.id, &"e".repeat(64), Some(true), 1_900_000_100)
            .unwrap()
            .earned_msat,
        c.earned_msat
    );
    assert!(
        l.observe_commission(&a.id, &"f".repeat(64), Some(true), 1_900_000_100)
            .is_err()
    );
    let mut other = a.clone();
    other.id = "f".repeat(64);
    assert!(l.admit_commission(&other).is_err());
    assert_eq!(l.settlement(&a.payment_hash).unwrap().unwrap(), original);
}
#[test]
fn unknown_costs_and_failed_delivery_earn_nothing() {
    let mut l = Ledger::in_memory().unwrap();
    let mut a = admission(&l);
    a.costs[0].amount_msat = None;
    l.admit_commission(&a).unwrap();
    settled(&mut l, &a);
    let held = l
        .observe_commission(&a.id, &"e".repeat(64), Some(true), 1_900_001_000)
        .unwrap();
    assert_eq!(held.earned_msat, 0);
    assert_eq!(held.state, "held");
    assert_eq!(held.cost_msat, None);
    assert!(l.available_shares(&a.party).unwrap().is_empty());
    let failed = l
        .observe_commission(&a.id, &"f".repeat(64), Some(false), 1_900_001_001)
        .unwrap();
    assert_eq!(failed.state, "ineligible");
    assert_eq!(failed.earned_msat, 0);
    assert_eq!(l.commission_held_liability().unwrap(), 0);
}
#[test]
fn reversals_once_partial_full_and_paid_loss() {
    let mut l = Ledger::in_memory().unwrap();
    let a = admission(&l);
    l.admit_commission(&a).unwrap();
    settled(&mut l, &a);
    let original = l.settlement(&a.payment_hash).unwrap().unwrap();
    let c = l
        .observe_commission(&a.id, &"e".repeat(64), Some(true), 1_900_001_000)
        .unwrap();
    let r = l
        .reverse_commission(&a.id, &"1".repeat(64), &"2".repeat(64), 2000, 1_900_001_001)
        .unwrap();
    assert_eq!(r.commission_reversed_msat, c.earned_msat / 5);
    assert_eq!(r.loss_msat, 0);
    assert_eq!(
        l.reverse_commission(&a.id, &"1".repeat(64), &"2".repeat(64), 2000, 1_900_001_001)
            .unwrap()
            .reversed_msat,
        2000
    );
    assert!(
        l.reverse_commission(&a.id, &"1".repeat(64), &"2".repeat(64), 2001, 1_900_001_001)
            .is_err()
    );
    l.register_payee(Payee {
        party: a.party.clone(),
        destination_kind: "spark".into(),
        destination_value: "synthetic".into(),
        source: "fixture".into(),
        verified_at: 1,
    })
    .unwrap();
    let claims = l.available_shares(&a.party).unwrap();
    assert!(!claims.is_empty());
    l.reserve_payout("paid", &a.party, &claims, 1_900_001_002)
        .unwrap();
    l.begin_send(
        "paid",
        "transfer",
        None,
        claims[0].amount_msat,
        1_900_001_002,
    )
    .unwrap();
    l.finish_payout("paid", PayoutState::Sent, Some(0), None, 1_900_001_003)
        .unwrap();
    let r = l
        .reverse_commission(&a.id, &"3".repeat(64), &"4".repeat(64), 8000, 1_900_001_004)
        .unwrap();
    assert_eq!(r.reversed_msat, 10_000);
    assert_eq!(r.commission_reversed_msat, c.earned_msat);
    assert!(r.loss_msat > 0);
    assert!(l.commission_payouts_held().unwrap());
    assert_eq!(l.settlement(&a.payment_hash).unwrap().unwrap(), original);
    assert!(
        l.reverse_commission(&a.id, &"5".repeat(64), &"6".repeat(64), 1, 1_900_001_005)
            .is_err()
    );
}
#[test]
fn original_admission_required_and_precision_is_checked() {
    let mut l = Ledger::in_memory().unwrap();
    let mut a = admission(&l);
    a.numerator = 2;
    a.denominator = 1;
    assert!(l.admit_commission(&a).is_err());
    a.numerator = 1;
    a.denominator = 2;
    a.minimum_msat = 500;
    assert!(l.admit_commission(&a).is_err());
    a.minimum_msat = 1000;
    settled(&mut l, &a);
    assert!(l.admit_commission(&a).is_err());
}

fn ready(l: &mut Ledger) -> Admission {
    let a = admission(l);
    l.admit_commission(&a).unwrap();
    settled(l, &a);
    l.observe_commission(&a.id, &"e".repeat(64), Some(true), 1_900_001_000)
        .unwrap();
    a
}
fn reserve(l: &mut Ledger, a: &Admission, state: PayoutState) {
    l.register_payee(Payee {
        party: a.party.clone(),
        destination_kind: "spark".into(),
        destination_value: "synthetic".into(),
        source: "fixture".into(),
        verified_at: 1,
    })
    .unwrap();
    let claims = l.available_shares(&a.party).unwrap();
    l.reserve_payout("reserved", &a.party, &claims, 1_900_001_001)
        .unwrap();
    l.begin_send(
        "reserved",
        "original-reference",
        None,
        claims[0].amount_msat,
        1_900_001_001,
    )
    .unwrap();
    l.finish_payout("reserved", state, Some(0), None, 1_900_001_002)
        .unwrap();
}
fn reverse(l: &mut Ledger, a: &Admission, amount: i64) -> Report {
    l.reverse_commission(
        &a.id,
        &"1".repeat(64),
        &"2".repeat(64),
        amount,
        1_900_001_003,
    )
    .unwrap()
}
fn conserved(l: &Ledger, a: &Admission, refunded: i64) {
    let t = l.totals().unwrap();
    let r = l.commission_report(&a.id).unwrap();
    assert_eq!(
        t.accrued_msat + t.reserved_msat + t.paid_msat + l.commission_held_liability().unwrap(),
        a.price_msat as i64 - refunded + r.loss_msat
    );
}
#[test]
fn partial_refund_keeps_reserved_cash_and_failed_recovery_releases_only_its_uncertainty() {
    let mut l = Ledger::in_memory().unwrap();
    let a = ready(&mut l);
    reserve(&mut l, &a, PayoutState::Unknown);
    let r = reverse(&mut l, &a, 2000);
    assert_eq!(
        (
            r.earned_msat,
            r.commission_reversed_msat,
            r.reserved_msat,
            r.retained_remainder_msat,
            r.loss_msat,
            r.paid_or_reserved_reversal_loss_msat
        ),
        (4500, 900, 4000, 0, 0, 400)
    );
    conserved(&l, &a, 2000);
    assert!(l.commission_payouts_held().unwrap());
    let p = l.payout("reserved").unwrap().unwrap();
    l.finish_payout(
        "reserved",
        PayoutState::Failed,
        None,
        Some("native verified failed"),
        1_900_001_004,
    )
    .unwrap();
    let r = l.commission_report(&a.id).unwrap();
    assert_eq!(
        (
            r.available_msat,
            r.retained_remainder_msat,
            r.loss_msat,
            r.paid_or_reserved_reversal_loss_msat
        ),
        (3000, 600, 0, 0)
    );
    assert!(!l.commission_payouts_held().unwrap());
    conserved(&l, &a, 2000);
    assert_eq!(
        l.payout("reserved").unwrap().unwrap().wallet_reference,
        p.wallet_reference
    );
}
#[test]
fn full_refund_does_not_double_count_paid_referrer_loss_or_release_unknown_cash() {
    for state in [PayoutState::Sent, PayoutState::Unknown] {
        let mut l = Ledger::in_memory().unwrap();
        let a = ready(&mut l);
        reserve(&mut l, &a, state);
        let r = reverse(&mut l, &a, 10_000);
        assert_eq!(
            (
                r.commission_reversed_msat,
                r.loss_msat,
                r.paid_or_reserved_reversal_loss_msat,
                r.retained_remainder_msat
            ),
            (4500, 5000, 4000, 0)
        );
        conserved(&l, &a, 10_000);
        assert_eq!(
            l.settlement(&a.payment_hash)
                .unwrap()
                .unwrap()
                .shares
                .iter()
                .find(|s| s.role == "author")
                .unwrap()
                .amount_msat,
            1000
        );
        if state == PayoutState::Unknown {
            l.finish_payout("reserved", PayoutState::Failed, None, None, 1_900_001_004)
                .unwrap();
            let r = l.commission_report(&a.id).unwrap();
            assert_eq!(
                (r.loss_msat, r.paid_or_reserved_reversal_loss_msat),
                (1000, 0)
            );
            conserved(&l, &a, 10_000);
        }
    }
}
#[test]
fn refund_before_cost_qualification_retains_original_base_and_exact_cost_provenance() {
    for amount in [2000, 10_000] {
        let mut l = Ledger::in_memory().unwrap();
        let mut a = admission(&l);
        a.costs[0].amount_msat = None;
        l.admit_commission(&a).unwrap();
        settled(&mut l, &a);
        l.observe_commission(&a.id, &"e".repeat(64), Some(true), 1_900_001_000)
            .unwrap();
        let r = reverse(&mut l, &a, amount);
        assert_eq!(r.held_msat, (9000 - amount).max(0));
        conserved(&l, &a, amount);
        let mut costs = a.costs.clone();
        costs[0].amount_msat = Some(0);
        l.qualify_commission_costs(&a.id, &"5".repeat(64), &costs)
            .unwrap();
        let r = l
            .observe_commission(&a.id, &"e".repeat(64), Some(true), 1_900_001_004)
            .unwrap();
        assert_eq!(r.earned_msat, 4500);
        assert_eq!(r.commission_reversed_msat, 4500 * amount / 10_000);
        conserved(&l, &a, amount);
        costs[1].amount_msat = Some(1);
        assert!(
            l.qualify_commission_costs(&a.id, &"5".repeat(64), &costs)
                .is_err()
        );
    }
}
#[test]
fn reserve_enforces_original_minimum_and_destination_even_without_worker() {
    let mut l = Ledger::in_memory().unwrap();
    let mut a = admission(&l);
    a.minimum_msat = 5000;
    l.admit_commission(&a).unwrap();
    settled(&mut l, &a);
    l.observe_commission(&a.id, &"e".repeat(64), Some(true), 1_900_001_000)
        .unwrap();
    l.register_payee(Payee {
        party: a.party.clone(),
        destination_kind: "spark".into(),
        destination_value: "synthetic".into(),
        source: "fixture".into(),
        verified_at: 1,
    })
    .unwrap();
    let claims = l.available_shares(&a.party).unwrap();
    assert!(l.reserve_payout("too-small", &a.party, &claims, 1).is_err());
    assert!(l.payout("too-small").unwrap().is_none());
}

#[test]
fn pending_original_collection_holds_new_payouts_before_observation_across_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite");
    let mut l = Ledger::open(&path).unwrap();
    let a = admission(&l);
    l.admit_commission(&a).unwrap();
    assert!(!l.commission_payouts_held().unwrap());
    settled(&mut l, &a);
    assert!(l.commission_payouts_held().unwrap());
    l.register_payee(Payee {
        party: "openagents".into(),
        destination_kind: "spark".into(),
        destination_value: "synthetic".into(),
        source: "fixture".into(),
        verified_at: 1,
    })
    .unwrap();
    let original = l.settlement(&a.payment_hash).unwrap().unwrap();
    let items = l.available_shares("openagents").unwrap();
    assert!(
        l.reserve_payout("raced", "openagents", &items, 1_900_000_001)
            .is_err()
    );
    drop(l);
    let mut l = Ledger::open(&path).unwrap();
    assert!(l.commission_payouts_held().unwrap());
    l.observe_commission(&a.id, &"e".repeat(64), Some(true), 1_900_001_000)
        .unwrap();
    assert!(!l.commission_payouts_held().unwrap());
    assert_eq!(l.settlement(&a.payment_hash).unwrap().unwrap(), original);
    assert_eq!(l.totals().unwrap().settlements, 1);
}
