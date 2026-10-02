use pay_ledger::*;
use proptest::prelude::*;

const START: i64 = 1_792_022_400; // 2026-10-15T00:00:00Z
const NEXT: i64 = START + 86_400;
fn input(key: &str, received: i64, fee: i64) -> SettlementInput {
    SettlementInput {
        key: key.into(),
        resource: "/plugins/demo/invoke".into(),
        plugin_id: Some("demo".into()),
        release_id: Some("release-1".into()),
        price_msat: received.max(fee),
        received_msat: received,
        rail: Rail::Lightning,
        payer_alias: None,
        settled_at: START,
        split: Split::Plugin {
            author: "alice".into(),
            fee_msat: fee,
        },
    }
}
fn v2(bps: u16) -> String {
    V1.replace("version = 1", "version = 2")
        .replace("2026-10-15", "2026-10-16")
        .replace(
            "resource_owner_bps = 9000",
            &format!("resource_owner_bps = {bps}"),
        )
}
fn payee(ledger: &mut Ledger, party: &str) {
    ledger
        .register_payee(Payee {
            party: party.into(),
            destination_kind: "spark".into(),
            destination_value: "test-destination".into(),
            source: "account".into(),
            verified_at: START,
        })
        .unwrap();
}
fn role(record: &Recorded, name: &str) -> i64 {
    record
        .shares
        .iter()
        .filter(|s| s.role == name)
        .map(|s| s.amount_msat)
        .sum()
}
fn conserved(ledger: &Ledger) {
    let t = ledger.totals().unwrap();
    assert_eq!(
        t.paid_msat + t.accrued_msat + t.reserved_msat,
        t.received_msat
    );
}

proptest! {
    #[test]
    fn invariant_1_replays_write_nothing(received in 0i64..1_000_000_000, fee in 0i64..1_000_000_000) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.sqlite");
        let mut ledger = Ledger::open(&path).unwrap();
        let first = ledger.record_settlement(input("hash", received, fee)).unwrap();
        let before = ledger.totals().unwrap();
        let observer = rusqlite::Connection::open(&path).unwrap();
        let version: i64 = observer.query_row("PRAGMA data_version", [], |r| r.get(0)).unwrap();
        let mut replay = input("hash", -1, -1);
        replay.settled_at = 0;
        prop_assert_eq!(ledger.record_settlement(replay).unwrap(), first.clone());
        prop_assert_eq!(ledger.totals().unwrap(), before);
        prop_assert_eq!(ledger.since(0).unwrap(), vec![first]);
        let after: i64 = observer.query_row("PRAGMA data_version", [], |r| r.get(0)).unwrap();
        prop_assert_eq!(version, after);
        prop_assert_eq!(ledger.record_settlement(input("next", 0, 0)).unwrap().seq, 2);
    }

    #[test]
    fn invariant_2_shares_sum_to_net_receipts(received in 0i64..=i64::MAX, fee in 0i64..=i64::MAX, hosted in any::<bool>()) {
        let mut ledger = Ledger::in_memory().unwrap();
        let mut i = input("hash", received, fee);
        if hosted { i.split = Split::HostedResource { owner: "alice".into() }; }
        let r = ledger.record_settlement(i).unwrap();
        prop_assert_eq!(r.shares.iter().map(|s| s.amount_msat as i128).sum::<i128>(), received as i128);
        prop_assert!(r.shares.iter().all(|s| s.amount_msat >= 0));
        prop_assert_eq!(role(&r,"provider"), 0);
        prop_assert_eq!(role(&r,"lsp_fee"), 0);
        if hosted {
            let owner = (received as i128 * 9000 / 10_000) as i64;
            prop_assert_eq!(role(&r,"bonus"), 0);
            prop_assert_eq!(role(&r,"resource"), owner);
            prop_assert_eq!(role(&r,"openagents"), received - owner);
        } else {
            prop_assert_eq!(role(&r,"author"), received.min(fee));
            prop_assert_eq!(r.short, received < fee);
            prop_assert_eq!(r.lsp_fee_msat, received.max(fee) - received);
        }
    }

    #[test]
    fn invariant_3_rules_are_effective_by_time_and_never_rewrite(received in 0i64..1_000_000_000, bps in 0u16..=10_000) {
        let mut ledger = Ledger::in_memory().unwrap();
        let mut old = input("old", received, 0);
        old.split = Split::HostedResource { owner: "alice".into() };
        let first = ledger.record_settlement(old.clone()).unwrap();
        let text = v2(bps);
        ledger.load_rule(&text, &digest(&text)).unwrap();
        prop_assert_eq!(ledger.record_settlement(old.clone()).unwrap(), first.clone());
        old.key = "late-old".into();
        old.settled_at = NEXT - 1;
        let late = ledger.record_settlement(old.clone()).unwrap();
        prop_assert_eq!(late.rule_version, 1);
        prop_assert_eq!(role(&late,"resource"), role(&first,"resource"));
        old.key = "new".into();
        old.settled_at = NEXT;
        let new = ledger.record_settlement(old).unwrap();
        prop_assert_eq!(new.rule_version, 2);
        prop_assert_eq!(role(&new,"resource"), (received as i128 * bps as i128 / 10_000) as i64);
        prop_assert_eq!(&ledger.since(0).unwrap()[0], &first);
    }

    #[test]
    fn invariant_4_only_failed_payouts_release_shares(amount in 1i64..1_000_000_000, fail in any::<bool>()) {
        let mut ledger = Ledger::in_memory().unwrap();
        let r = ledger.record_settlement(input("hash", amount, amount)).unwrap();
        payee(&mut ledger, "alice");
        let shares: Vec<_> = r.shares.into_iter().filter(|s| s.role == "author").collect();
        ledger.reserve_payout("first", "alice", &shares, START).unwrap();
        prop_assert!(ledger.reserve_payout("duplicate", "alice", &shares, START).is_err());
        ledger.set_payout_state("first", PayoutState::Unknown, None, START).unwrap();
        prop_assert!(ledger.reserve_payout("unknown-retry", "alice", &shares, START).is_err());
        ledger.set_payout_state("first", if fail { PayoutState::Failed } else { PayoutState::Succeeded }, Some("wallet-ref"), START).unwrap();
        prop_assert_eq!(ledger.reserve_payout("retry", "alice", &shares, START).is_ok(), fail);
        prop_assert!(ledger.set_payout_state("first", PayoutState::Pending, None, START).is_err());
        conserved(&ledger);
    }

    #[test]
    fn invariant_5_payouts_and_claims_never_exceed_receipts(calls in prop::collection::vec((1i64..1_000_000, 0i64..1_000_000, 0u8..3), 1..20)) {
        let mut ledger = Ledger::in_memory().unwrap();
        payee(&mut ledger, "alice");
        payee(&mut ledger, OPENAGENTS);
        for (index, (received, fee, outcome)) in calls.into_iter().enumerate() {
            let r = ledger.record_settlement(input(&format!("hash-{index}"), received, fee)).unwrap();
            conserved(&ledger);
            for (n, share) in r.shares.into_iter().filter(|s| s.amount_msat > 0).enumerate() {
                let id = format!("payout-{index}-{n}");
                ledger.reserve_payout(&id, &share.party, std::slice::from_ref(&share), START).unwrap();
                conserved(&ledger);
                let state = match outcome { 0 => PayoutState::Failed, 1 => PayoutState::Unknown, _ => PayoutState::Succeeded };
                ledger.set_payout_state(&id, state, Some("wallet-ref"), START).unwrap();
                conserved(&ledger);
                let t = ledger.totals().unwrap();
                prop_assert!(t.paid_msat + t.accrued_msat <= t.received_msat);
            }
        }
    }
}

#[test]
fn persistence_queries_and_balance_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite");
    let mut ledger = Ledger::open(&path).unwrap();
    let first = ledger.record_settlement(input("hash", 101, 80)).unwrap();
    let mut i = input("debit:123", 200, 100);
    i.rail = Rail::Balance;
    let second = ledger.record_settlement(i).unwrap();
    assert_eq!(ledger.accrued("alice").unwrap(), 301);
    assert_eq!(ledger.accrued("missing").unwrap(), 0);
    assert_eq!(ledger.accrued(OPENAGENTS).unwrap(), 0);
    assert_eq!(
        ledger.per_plugin().unwrap()["demo"],
        PluginTotals {
            settlements: 2,
            received_msat: 301,
            author_msat: 180,
            openagents_msat: 0,
            bonus_msat: 121
        }
    );
    assert_eq!(ledger.since(first.seq).unwrap(), vec![second.clone()]);
    drop(ledger);
    let ledger = Ledger::open(&path).unwrap();
    assert_eq!(ledger.since(0).unwrap(), vec![first, second]);
    conserved(&ledger);
}

#[test]
fn invalid_rules_inputs_and_payouts_are_atomic() {
    let mut ledger = Ledger::in_memory().unwrap();
    assert!(ledger.load_rule(V1, "incorrect").is_err());
    let changed = V1.replace("9000", "8000");
    assert!(ledger.load_rule(&changed, &digest(&changed)).is_err());
    let invalid = v2(10_001);
    assert!(ledger.load_rule(&invalid, &digest(&invalid)).is_err());
    let mut i = input("early", 10, 10);
    i.settled_at = START - 1;
    assert!(matches!(ledger.record_settlement(i), Err(Error::NoRule)));
    assert!(
        ledger
            .record_settlement(input("debit:wrong-rail", 10, 10))
            .is_err()
    );
    assert!(ledger.record_settlement(input("negative", -1, 0)).is_err());
    assert!(ledger.since(0).unwrap().is_empty());
    let r = ledger.record_settlement(input("hash", 10, 10)).unwrap();
    payee(&mut ledger, "alice");
    let share = r
        .shares
        .iter()
        .find(|s| s.role == "author")
        .unwrap()
        .clone();
    assert!(
        ledger
            .reserve_payout(
                "duplicates",
                "alice",
                &[share.clone(), share.clone()],
                START
            )
            .is_err()
    );
    assert_eq!(ledger.accrued("alice").unwrap(), 10);
    let mut inflated = share.clone();
    inflated.amount_msat += 1;
    assert!(
        ledger
            .reserve_payout("inflated", "alice", &[inflated], START)
            .is_err()
    );
    ledger
        .reserve_payout("valid", "alice", &[share], START)
        .unwrap();
    assert!(
        ledger
            .set_payout_state("valid", PayoutState::Succeeded, None, START)
            .is_err()
    );
    conserved(&ledger);
}

#[test]
fn a_settlement_keeps_its_release_and_calls_are_recorded_paid_or_free() {
    let mut ledger = Ledger::in_memory().unwrap();
    let recorded = ledger
        .record_settlement(input("hash-r", 6_000, 1_000))
        .unwrap();
    assert_eq!(recorded.release_id.as_deref(), Some("release-1"));
    let call = |paid: bool, outcome: &str| CallRecord {
        at: START,
        route: "invoke".into(),
        resource: "plugin".into(),
        plugin_id: Some("demo".into()),
        release_id: Some("release-1".into()),
        outcome: outcome.into(),
        paid,
        price_msat: Some(6_000),
    };
    let first = ledger.record_call(&call(false, "challenged")).unwrap();
    ledger.record_call(&call(true, "executed")).unwrap();
    let calls = ledger.calls_since(0).unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, first);
    assert!(!calls[0].1.paid && calls[1].1.paid);
    assert_eq!(ledger.calls_since(first).unwrap().len(), 1);
}
