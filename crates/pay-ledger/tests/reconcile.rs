//! Reconciliation against fake wallets: every drift kind is detected and
//! cleared, extra wallet payments are told apart from top-ups, and an
//! `unknown` payout is settled only from a record that proves it.

use pay_ledger::reconcile::{
    Direction, Kind, Severity, Snapshot, State, Status, WalletPayment, WalletView, reconcile,
    resolve_unknown,
};
use pay_ledger::{Ledger, Payee, PayoutState, Rail, SettlementInput, Split};

const START: i64 = 1_792_022_400;
const SPARK: &str = "spark1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq";
const LUD16: &str = "bob@example.com";
/// What each settlement receives: a 600,000 msat author fee plus 5,000.
const RECEIVED: i64 = 605_000;

fn settle(ledger: &mut Ledger, key: &str, author: &str, at: i64) {
    ledger
        .record_settlement(SettlementInput {
            key: key.into(),
            resource: "/v1/plugins/demo/invoke".into(),
            plugin_id: Some(format!("plugin-{author}")),
            release_id: None,
            price_msat: RECEIVED,
            received_msat: RECEIVED,
            rail: Rail::Lightning,
            payer_alias: None,
            settled_at: at,
            split: Split::Plugin {
                author: author.into(),
                fee_msat: 600_000,
            },
        })
        .unwrap();
}

fn payee(ledger: &mut Ledger, party: &str, kind: &str, value: &str) {
    ledger
        .register_payee(Payee {
            party: party.into(),
            destination_kind: kind.into(),
            destination_value: value.into(),
            source: "release".into(),
            verified_at: START,
        })
        .unwrap();
}

/// Reserve all of `party`'s shares as payout `id` with wallet reference
/// `reference`, then leave it in `state`. Returns the msat that went out.
fn payout(
    ledger: &mut Ledger,
    id: &str,
    party: &str,
    reference: &str,
    state: PayoutState,
    at: i64,
) -> i64 {
    let shares = ledger.available_shares(party).unwrap();
    let amount = ledger.reserve_payout(id, party, &shares, at).unwrap();
    let sent = amount / 1000 * 1000;
    ledger.begin_send(id, reference, None, sent, at).unwrap();
    if state != PayoutState::Sending {
        let fee = (state == PayoutState::Sent).then_some(3_000);
        ledger.finish_payout(id, state, fee, None, at).unwrap();
    }
    sent
}

fn record(reference: &str, direction: Direction, status: Status, msat: i64) -> WalletPayment {
    WalletPayment {
        reference: reference.into(),
        direction,
        status,
        amount_msat: Some(msat),
        fee_msat: if direction == Direction::Outbound {
            3_000
        } else {
            0
        },
        at: START,
    }
}

fn hash(n: u8) -> String {
    format!("{n:02x}").repeat(32)
}

/// A ledger with two settlements for alice (paid out over Spark) and one
/// for bob (paid out over Lightning), and wallets that match it exactly:
/// the receiver received both settlements, paid bob's invoice, and topped
/// the Spark wallet up, and the Spark wallet paid alice.
struct World {
    ledger: Ledger,
    snapshot: Snapshot,
    alice_sent: i64,
}

fn world() -> World {
    let mut ledger = Ledger::in_memory().unwrap();
    settle(&mut ledger, &hash(1), "alice", START);
    settle(&mut ledger, &hash(2), "alice", START + 1);
    settle(&mut ledger, &hash(3), "bob", START + 2);
    payee(&mut ledger, "alice", "spark", SPARK);
    payee(&mut ledger, "bob", "lud16", LUD16);
    let alice_sent = payout(
        &mut ledger,
        "p-alice",
        "alice",
        "p-alice",
        PayoutState::Sent,
        START + 60,
    );
    let bob_sent = payout(
        &mut ledger,
        "p-bob",
        "bob",
        &hash(9),
        PayoutState::Sent,
        START + 60,
    );
    let top_up = 2_000_000;
    let receiver = WalletView {
        payments: vec![
            record(&hash(1), Direction::Inbound, Status::Succeeded, RECEIVED),
            record(&hash(2), Direction::Inbound, Status::Succeeded, RECEIVED),
            record(&hash(3), Direction::Inbound, Status::Succeeded, RECEIVED),
            record(&hash(9), Direction::Outbound, Status::Succeeded, bob_sent),
            record(&hash(7), Direction::Outbound, Status::Succeeded, top_up),
        ],
        complete: true,
        balance_msat: Some(10_000_000),
    };
    let spark = WalletView {
        payments: vec![
            record("top-up", Direction::Inbound, Status::Succeeded, top_up),
            record(
                "p-alice",
                Direction::Outbound,
                Status::Succeeded,
                alice_sent,
            ),
        ],
        complete: true,
        balance_msat: Some(top_up - alice_sent),
    };
    World {
        ledger,
        snapshot: Snapshot {
            receiver: Some(receiver),
            spark: Some(spark),
        },
        alice_sent,
    }
}

fn kinds(report: &pay_ledger::reconcile::Report) -> Vec<(Kind, Severity)> {
    report
        .findings
        .iter()
        .map(|f| (f.kind, f.severity))
        .collect()
}

fn receiver(world: &mut World) -> &mut WalletView {
    world.snapshot.receiver.as_mut().unwrap()
}

fn spark(world: &mut World) -> &mut WalletView {
    world.snapshot.spark.as_mut().unwrap()
}

#[test]
fn matching_wallets_reconcile_ok() {
    let w = world();
    let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
    assert_eq!(kinds(&report), vec![]);
    assert_eq!(report.state, State::Ok);
    assert_eq!(report.figures.lightning_settlements, 3);
    assert_eq!(report.figures.received_msat, 3 * RECEIVED);
    assert_eq!(report.figures.payouts_sent, 2);
    assert_eq!(report.receiver, "listed");
    assert!(report.text().starts_with("Reconciliation "));
    assert!(report.text().contains(": ok (0 drift, 0 notices)"));
}

#[test]
fn a_settlement_without_its_inbound_payment_drifts_until_it_appears() {
    let mut w = world();
    let saved = receiver(&mut w).payments.remove(0);
    let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
    assert_eq!(
        kinds(&report),
        vec![(Kind::SettlementMissing, Severity::Drift)]
    );
    assert_eq!(report.state, State::Drift);
    assert_eq!(
        report.findings[0].reference.as_deref(),
        Some(hash(1).as_str())
    );

    // Pending is still missing.
    let mut pending = saved.clone();
    pending.status = Status::Pending;
    receiver(&mut w).payments.push(pending);
    let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
    assert_eq!(report.state, State::Drift);
    assert!(report.findings[0].detail.contains("still pending"));

    receiver(&mut w).payments.pop();
    receiver(&mut w).payments.push(saved);
    let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
    assert_eq!(report.state, State::Ok);
}

#[test]
fn a_settlement_amount_mismatch_drifts_and_clears() {
    let mut w = world();
    receiver(&mut w).payments[1].amount_msat = Some(RECEIVED - 1_000);
    let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
    assert_eq!(
        kinds(&report),
        vec![(Kind::SettlementAmount, Severity::Drift)]
    );
    assert_eq!(report.findings[0].wallet_msat, Some(RECEIVED - 1_000));
    // An overpayment is capped at the price in the ledger: not drift.
    receiver(&mut w).payments[1].amount_msat = Some(RECEIVED + 1_000);
    let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
    assert_eq!(report.state, State::Ok);
}

#[test]
fn a_sent_payout_without_its_outbound_record_drifts_and_clears() {
    for (wallet, index) in [("spark", 1), ("receiver", 3)] {
        let mut w = world();
        let view = if wallet == "spark" {
            spark(&mut w)
        } else {
            receiver(&mut w)
        };
        let saved = view.payments.remove(index);
        let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
        assert_eq!(
            kinds(&report),
            vec![(Kind::PayoutMissing, Severity::Drift)],
            "{wallet}"
        );
        assert_eq!(report.findings[0].wallet, Some(wallet));

        let mut wrong = saved.clone();
        wrong.amount_msat = Some(1_000);
        let view = if wallet == "spark" {
            spark(&mut w)
        } else {
            receiver(&mut w)
        };
        view.payments.push(wrong);
        let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
        assert_eq!(
            kinds(&report),
            vec![(Kind::PayoutAmount, Severity::Drift)],
            "{wallet}"
        );

        let view = if wallet == "spark" {
            spark(&mut w)
        } else {
            receiver(&mut w)
        };
        view.payments.pop();
        view.payments.push(saved);
        let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
        assert_eq!(report.state, State::Ok, "{wallet}");
    }
}

#[test]
fn extra_outbound_payments_drift_and_extra_inbound_is_a_notice() {
    let mut w = world();
    receiver(&mut w).payments.push(record(
        &hash(20),
        Direction::Outbound,
        Status::Succeeded,
        50_000,
    ));
    spark(&mut w).payments.push(record(
        "stray",
        Direction::Outbound,
        Status::Succeeded,
        40_000,
    ));
    receiver(&mut w).payments.push(record(
        &hash(21),
        Direction::Inbound,
        Status::Succeeded,
        21_000,
    ));
    // Failed and pending outbound records move nothing.
    receiver(&mut w).payments.push(record(
        &hash(22),
        Direction::Outbound,
        Status::Failed,
        9_000,
    ));
    let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
    assert_eq!(
        kinds(&report),
        vec![
            (Kind::ExtraOutbound, Severity::Drift),
            (Kind::ExtraOutbound, Severity::Drift),
            (Kind::ExtraInbound, Severity::Notice),
        ]
    );
    // A Spark inbound payment of the same amount is the top-up it paid.
    spark(&mut w).payments.push(record(
        "top-up-2",
        Direction::Inbound,
        Status::Succeeded,
        50_000,
    ));
    spark(&mut w).payments.retain(|p| p.reference != "stray");
    let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
    assert_eq!(kinds(&report), vec![(Kind::ExtraInbound, Severity::Notice)]);
    assert_eq!(report.state, State::Ok);
}

#[test]
fn a_wallet_that_only_looked_up_references_flags_no_extras() {
    let mut w = world();
    receiver(&mut w).complete = false;
    receiver(&mut w).payments.push(record(
        &hash(20),
        Direction::Outbound,
        Status::Succeeded,
        50_000,
    ));
    let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
    assert_eq!(report.receiver, "looked_up");
    assert_eq!(report.state, State::Ok);
}

#[test]
fn an_unreadable_wallet_is_unknown_not_ok() {
    let mut w = world();
    w.snapshot.spark = None;
    let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
    assert_eq!(report.state, State::Unknown);
    assert_eq!(report.spark, "unreachable");
    // The receiver's top-up cannot be matched without the Spark wallet.
    assert_eq!(
        kinds(&report),
        vec![
            (Kind::ExtraOutbound, Severity::Notice),
            (Kind::Unchecked, Severity::Notice)
        ]
    );
    // Drift still shows while the other wallet is unread.
    receiver(&mut w).payments.remove(0);
    let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
    assert_eq!(report.state, State::Drift);
}

#[test]
fn holdings_below_what_is_owed_drift_and_clear() {
    let mut w = world();
    let mut ledger = w.ledger;
    settle(&mut ledger, &hash(4), "carol", START + 100);
    w.ledger = ledger;
    receiver(&mut w).payments.push(record(
        &hash(4),
        Direction::Inbound,
        Status::Succeeded,
        RECEIVED,
    ));
    let owed = reconcile(&w.ledger, &w.snapshot, START + 3_600)
        .unwrap()
        .figures
        .owed_msat;
    assert!(owed > 0);
    let spark_balance = spark(&mut w).balance_msat.unwrap();
    receiver(&mut w).balance_msat = Some(owed - spark_balance - 1);
    let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
    assert_eq!(kinds(&report), vec![(Kind::HoldingsShort, Severity::Drift)]);
    receiver(&mut w).balance_msat = Some(owed - spark_balance);
    let report = reconcile(&w.ledger, &w.snapshot, START + 3_600).unwrap();
    assert_eq!(report.state, State::Ok);
}

/// A world plus carol, whose Spark payout is `unknown` with reference
/// `p-carol`.
fn with_unknown() -> (World, i64) {
    let mut w = world();
    settle(&mut w.ledger, &hash(5), "carol", START + 100);
    payee(&mut w.ledger, "carol", "spark", SPARK);
    let sent = payout(
        &mut w.ledger,
        "p-carol",
        "carol",
        "p-carol",
        PayoutState::Unknown,
        START + 200,
    );
    receiver(&mut w).payments.push(record(
        &hash(5),
        Direction::Inbound,
        Status::Succeeded,
        RECEIVED,
    ));
    (w, sent)
}

#[test]
fn an_unknown_payout_is_a_notice_then_drift_until_resolved_by_its_record() {
    let (mut w, sent) = with_unknown();
    let young = reconcile(&w.ledger, &w.snapshot, START + 300).unwrap();
    assert_eq!(kinds(&young), vec![(Kind::PayoutUnknown, Severity::Notice)]);
    assert_eq!(young.state, State::Ok);
    let late = START + 200 + pay_ledger::reconcile::UNKNOWN_GRACE_SECS;
    let old = reconcile(&w.ledger, &w.snapshot, late).unwrap();
    assert_eq!(kinds(&old), vec![(Kind::PayoutUnknown, Severity::Drift)]);

    // Absent, pending, and wrong-amount records prove nothing.
    assert_eq!(
        resolve_unknown(&mut w.ledger, &w.snapshot, late).unwrap(),
        vec![]
    );
    spark(&mut w).payments.push(record(
        "p-carol",
        Direction::Outbound,
        Status::Pending,
        sent,
    ));
    assert_eq!(
        resolve_unknown(&mut w.ledger, &w.snapshot, late).unwrap(),
        vec![]
    );
    spark(&mut w).payments.pop();
    spark(&mut w).payments.push(record(
        "p-carol",
        Direction::Outbound,
        Status::Succeeded,
        sent - 1_000,
    ));
    assert_eq!(
        resolve_unknown(&mut w.ledger, &w.snapshot, late).unwrap(),
        vec![]
    );
    assert_eq!(
        w.ledger.payout("p-carol").unwrap().unwrap().state,
        PayoutState::Unknown
    );

    // The succeeded record with the amount that went out settles it.
    spark(&mut w).payments.pop();
    spark(&mut w).payments.push(record(
        "p-carol",
        Direction::Outbound,
        Status::Succeeded,
        sent,
    ));
    let resolved = resolve_unknown(&mut w.ledger, &w.snapshot, late).unwrap();
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].state, "sent");
    let p = w.ledger.payout("p-carol").unwrap().unwrap();
    assert_eq!(p.state, PayoutState::Sent);
    assert_eq!(p.fee_msat, Some(3_000));
    // The Spark wallet's payment needs its top-up to balance.
    spark(&mut w).payments.push(record(
        "top-up-carol",
        Direction::Inbound,
        Status::Succeeded,
        sent,
    ));
    let report = reconcile(&w.ledger, &w.snapshot, late).unwrap();
    assert_eq!(report.state, State::Ok, "{}", report.text());
    // Resolving again changes nothing.
    assert_eq!(
        resolve_unknown(&mut w.ledger, &w.snapshot, late).unwrap(),
        vec![]
    );
    assert!(w.alice_sent > 0);
}

#[test]
fn an_unknown_payout_the_wallet_failed_returns_its_shares() {
    let (mut w, sent) = with_unknown();
    assert_eq!(w.ledger.accrued("carol").unwrap(), 0);
    spark(&mut w)
        .payments
        .push(record("p-carol", Direction::Outbound, Status::Failed, sent));
    let resolved = resolve_unknown(&mut w.ledger, &w.snapshot, START + 300).unwrap();
    assert_eq!(resolved[0].state, "failed");
    let reserved = w.ledger.payout("p-carol").unwrap().unwrap().amount_msat;
    assert_eq!(w.ledger.accrued("carol").unwrap(), reserved);
    let report = reconcile(&w.ledger, &w.snapshot, START + 300).unwrap();
    assert_eq!(report.state, State::Ok);
}

#[test]
fn a_failed_payout_the_wallet_sent_is_drift() {
    let mut w = world();
    settle(&mut w.ledger, &hash(6), "carol", START + 100);
    payee(&mut w.ledger, "carol", "spark", SPARK);
    let sent = payout(
        &mut w.ledger,
        "p-carol",
        "carol",
        "p-carol",
        PayoutState::Failed,
        START + 200,
    );
    receiver(&mut w).payments.push(record(
        &hash(6),
        Direction::Inbound,
        Status::Succeeded,
        RECEIVED,
    ));
    spark(&mut w).payments.push(record(
        "p-carol",
        Direction::Outbound,
        Status::Succeeded,
        sent,
    ));
    spark(&mut w).payments.push(record(
        "top-up-carol",
        Direction::Inbound,
        Status::Succeeded,
        sent,
    ));
    let report = reconcile(&w.ledger, &w.snapshot, START + 300).unwrap();
    assert_eq!(kinds(&report), vec![(Kind::FailedButSent, Severity::Drift)]);
}
