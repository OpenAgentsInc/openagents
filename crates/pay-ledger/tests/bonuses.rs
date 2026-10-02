use pay_ledger::*;
use proptest::prelude::*;

fn at(text: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(text)
        .unwrap()
        .timestamp()
}

fn start() -> i64 {
    at("2026-10-16T00:00:00Z")
}

fn rules(first: i64, match_bps: u16, cap: i64, until: &str) -> String {
    V1.replace("version = 1", "version = 2")
        .replace("2026-10-15", "2026-10-16")
        .replace(
            "first_paid_call_msat = 1_000_000",
            &format!("first_paid_call_msat = {first}"),
        )
        .replace(
            "launch_match_bps = 10000",
            &format!("launch_match_bps = {match_bps}"),
        )
        .replace(
            "launch_match_cap_msat_per_month = 50_000_000",
            &format!("launch_match_cap_msat_per_month = {cap}"),
        )
        .replace("2026-12-14T00:00:00Z", until)
}

fn configured(first: i64, match_bps: u16, cap: i64) -> Ledger {
    let mut ledger = Ledger::in_memory().unwrap();
    let text = rules(first, match_bps, cap, "2026-12-14T00:00:00Z");
    ledger.load_rule(&text, &digest(&text)).unwrap();
    ledger
}

fn call(key: &str, plugin: &str, author: &str, received: i64, fee: i64) -> SettlementInput {
    SettlementInput {
        key: key.into(),
        resource: format!("/plugins/{plugin}/invoke"),
        plugin_id: Some(plugin.into()),
        price_msat: received.max(fee),
        received_msat: received,
        rail: Rail::Lightning,
        payer_alias: None,
        settled_at: start(),
        split: Split::Plugin {
            author: author.into(),
            fee_msat: fee,
        },
    }
}

fn fund(ledger: &mut Ledger, key: &str, amount: i64) -> Recorded {
    let mut input = call(key, "unused", "unused", amount, 0);
    input.plugin_id = None;
    input.split = Split::OpenAgents;
    ledger.record_settlement(input).unwrap()
}

fn payee(ledger: &mut Ledger, party: &str) {
    ledger
        .register_payee(Payee {
            party: party.into(),
            destination_kind: "spark".into(),
            destination_value: "fixture-destination".into(),
            source: "account".into(),
            verified_at: start(),
        })
        .unwrap();
}

fn role(record: &Recorded, name: &str) -> i64 {
    record
        .shares
        .iter()
        .filter(|share| share.role == name)
        .map(|share| share.amount_msat)
        .sum()
}

fn conserved(ledger: &Ledger) {
    let totals = ledger.totals().unwrap();
    assert_eq!(
        totals.received_msat,
        totals.accrued_msat + totals.reserved_msat + totals.paid_msat
    );
    for record in ledger.since(0).unwrap() {
        assert_eq!(
            record
                .shares
                .iter()
                .map(|share| i128::from(share.amount_msat))
                .sum::<i128>(),
            i128::from(record.received_msat)
        );
    }
}

#[test]
fn launch_match_cap_follows_the_author_across_plugins_and_resets_each_utc_month() {
    let mut ledger = configured(0, 10_000, 150);
    let first = ledger
        .record_settlement(call("one", "plugin-a", "alice", 1_000, 100))
        .unwrap();
    let second = ledger
        .record_settlement(call("two", "plugin-b", "alice", 1_000, 100))
        .unwrap();
    let third = ledger
        .record_settlement(call("three", "plugin-a", "alice", 1_000, 100))
        .unwrap();
    let other = ledger
        .record_settlement(call("other", "plugin-c", "bob", 1_000, 100))
        .unwrap();
    assert_eq!(role(&first, "bonus"), 100);
    assert_eq!(role(&second, "bonus"), 50);
    assert_eq!(role(&third, "bonus"), 0);
    assert_eq!(role(&other, "bonus"), 100);
    let capped = second
        .bonuses
        .iter()
        .find(|bonus| bonus.kind == "launch_match")
        .unwrap();
    assert_eq!(capped.party, "alice");
    assert_eq!(capped.requested_msat, 100);
    assert_eq!(capped.amount_msat, 50);
    let mut november = call("november", "plugin-b", "alice", 1_000, 100);
    november.settled_at = at("2026-11-01T00:00:00Z");
    assert_eq!(
        role(&ledger.record_settlement(november).unwrap(), "bonus"),
        100
    );
    conserved(&ledger);
}

#[test]
fn launch_match_window_ends_at_the_exclusive_timestamp() {
    let mut ledger = Ledger::in_memory().unwrap();
    let end = "2026-11-01T00:00:00Z";
    let text = rules(0, 10_000, 1_000, end);
    ledger.load_rule(&text, &digest(&text)).unwrap();
    let mut before = call("before", "plugin", "alice", 1_000, 100);
    before.settled_at = at(end) - 1;
    let mut exact = call("exact", "plugin", "alice", 1_000, 100);
    exact.settled_at = at(end);
    assert_eq!(
        role(&ledger.record_settlement(before).unwrap(), "bonus"),
        100
    );
    assert_eq!(role(&ledger.record_settlement(exact).unwrap(), "bonus"), 0);
    conserved(&ledger);
}

#[test]
fn first_bonus_pools_partial_funding_without_rewriting_old_settlements() {
    let mut ledger = configured(100, 0, 0);
    let first_source = fund(&mut ledger, "fund-a", 70);
    let second_source = fund(&mut ledger, "fund-b", 80);
    let award = ledger
        .record_settlement(call("earned", "plugin", "alice", 10, 10))
        .unwrap();
    let bonus = award
        .bonuses
        .iter()
        .find(|bonus| bonus.kind == "first_paid_call")
        .unwrap();
    assert_eq!(bonus.party, "alice");
    assert_eq!(bonus.requested_msat, 100);
    assert_eq!(bonus.amount_msat, 100);
    assert_eq!(bonus.outcome, "awarded");
    assert_eq!(ledger.accrued(OPENAGENTS).unwrap(), 50);
    assert_eq!(ledger.accrued("alice").unwrap(), 110);
    assert_eq!(
        ledger
            .available_shares(OPENAGENTS)
            .unwrap()
            .iter()
            .map(|share| share.amount_msat)
            .sum::<i64>(),
        50
    );
    assert_eq!(ledger.since(0).unwrap()[..2], [first_source, second_source]);
    let claim = ledger
        .available_shares("alice")
        .unwrap()
        .into_iter()
        .find(|share| share.role == "first_paid_call")
        .unwrap();
    assert_eq!(claim.settlement, "earned");
    assert_eq!(claim.amount_msat, 100);
    conserved(&ledger);
}

#[test]
fn launch_match_uses_the_call_share_before_first_bonus_funding() {
    for (first, expected, outcome) in [(50, 50, "awarded"), (100, 0, "bonus_unfunded")] {
        let mut ledger = configured(first, 10_000, 1_000);
        let record = ledger
            .record_settlement(call("earned", "plugin", "alice", 250, 100))
            .unwrap();
        assert_eq!(role(&record, "bonus"), 100);
        assert_eq!(role(&record, "openagents"), 50);
        let bonus = record
            .bonuses
            .iter()
            .find(|bonus| bonus.kind == "first_paid_call")
            .unwrap();
        assert_eq!(bonus.amount_msat, expected);
        assert_eq!(bonus.outcome, outcome);
        assert_eq!(ledger.accrued(OPENAGENTS).unwrap(), 50 - expected);
        assert_eq!(ledger.accrued("alice").unwrap(), 200 + expected);
        conserved(&ledger);
    }
}

#[test]
fn an_unfunded_first_bonus_is_skipped_permanently() {
    let mut ledger = configured(100, 0, 0);
    let first = ledger
        .record_settlement(call("first", "plugin", "alice", 10, 10))
        .unwrap();
    let bonus = first
        .bonuses
        .iter()
        .find(|bonus| bonus.kind == "first_paid_call")
        .unwrap();
    assert_eq!(bonus.amount_msat, 0);
    assert_eq!(bonus.outcome, "bonus_unfunded");
    fund(&mut ledger, "later-funds", 200);
    let second = ledger
        .record_settlement(call("second", "plugin", "alice", 10, 10))
        .unwrap();
    assert!(
        second
            .bonuses
            .iter()
            .all(|bonus| bonus.kind != "first_paid_call")
    );
    assert_eq!(ledger.accrued(OPENAGENTS).unwrap(), 200);
    assert_eq!(ledger.accrued("alice").unwrap(), 20);
    conserved(&ledger);
}

#[test]
fn first_bonus_survives_restart_and_replay_without_new_funding() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite");
    let mut ledger = Ledger::open(&path).unwrap();
    let text = rules(100, 0, 0, "2026-12-14T00:00:00Z");
    ledger.load_rule(&text, &digest(&text)).unwrap();
    let source = fund(&mut ledger, "funds", 150);
    let awarded = ledger
        .record_settlement(call("earned", "plugin", "alice", 10, 10))
        .unwrap();
    let before = ledger.totals().unwrap();
    drop(ledger);
    let mut ledger = Ledger::open(&path).unwrap();
    assert_eq!(ledger.since(0).unwrap(), vec![source, awarded.clone()]);
    let mut replay = call("earned", "different", "bob", -1, -1);
    replay.settled_at = 0;
    assert_eq!(ledger.record_settlement(replay).unwrap(), awarded);
    assert_eq!(ledger.totals().unwrap(), before);
    assert_eq!(ledger.accrued(OPENAGENTS).unwrap(), 50);
    assert_eq!(ledger.accrued("alice").unwrap(), 110);
    conserved(&ledger);
}

#[test]
fn only_failed_openagents_payouts_make_their_funding_available() {
    for state in [
        PayoutState::Pending,
        PayoutState::Unknown,
        PayoutState::Succeeded,
        PayoutState::Failed,
    ] {
        let mut ledger = configured(100, 0, 0);
        fund(&mut ledger, "funds", 100);
        payee(&mut ledger, OPENAGENTS);
        let shares = ledger.available_shares(OPENAGENTS).unwrap();
        ledger
            .reserve_payout("treasury", OPENAGENTS, &shares, start())
            .unwrap();
        if state != PayoutState::Pending {
            ledger
                .set_payout_state("treasury", state, Some("wallet-reference"), start())
                .unwrap();
        }
        let record = ledger
            .record_settlement(call("earned", "plugin", "alice", 10, 10))
            .unwrap();
        let bonus = record
            .bonuses
            .iter()
            .find(|bonus| bonus.kind == "first_paid_call")
            .unwrap();
        let available = state == PayoutState::Failed;
        assert_eq!(bonus.amount_msat, if available { 100 } else { 0 });
        assert_eq!(
            bonus.outcome,
            if available {
                "awarded"
            } else {
                "bonus_unfunded"
            }
        );
        conserved(&ledger);
    }
}

#[test]
fn a_stale_openagents_share_cannot_pay_money_already_used_for_a_bonus() {
    let mut ledger = configured(100, 0, 0);
    let source = fund(&mut ledger, "funds", 200);
    let original = source
        .shares
        .into_iter()
        .find(|share| share.role == "openagents")
        .unwrap();
    ledger
        .record_settlement(call("earned", "plugin", "alice", 10, 10))
        .unwrap();
    payee(&mut ledger, OPENAGENTS);
    assert!(
        ledger
            .reserve_payout("stale", OPENAGENTS, &[original], start())
            .is_err()
    );
    let remaining = ledger.available_shares(OPENAGENTS).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].amount_msat, 100);
    assert_eq!(
        ledger
            .reserve_payout("remaining", OPENAGENTS, &remaining, start())
            .unwrap(),
        100
    );
    conserved(&ledger);
}

#[test]
fn a_first_bonus_can_be_paid_once_and_only_failed_payouts_release_it() {
    let mut ledger = configured(100, 0, 0);
    fund(&mut ledger, "funds", 100);
    ledger
        .record_settlement(call("earned", "plugin", "alice", 10, 10))
        .unwrap();
    payee(&mut ledger, "alice");
    let bonus = ledger
        .available_shares("alice")
        .unwrap()
        .into_iter()
        .find(|share| share.role == "first_paid_call")
        .unwrap();
    assert_eq!(
        ledger
            .reserve_payout("first", "alice", std::slice::from_ref(&bonus), start())
            .unwrap(),
        100
    );
    conserved(&ledger);
    ledger
        .set_payout_state("first", PayoutState::Unknown, None, start())
        .unwrap();
    assert!(
        ledger
            .reserve_payout("duplicate", "alice", std::slice::from_ref(&bonus), start())
            .is_err()
    );
    ledger
        .set_payout_state("first", PayoutState::Failed, None, start())
        .unwrap();
    conserved(&ledger);
    ledger
        .reserve_payout("retry", "alice", std::slice::from_ref(&bonus), start())
        .unwrap();
    ledger
        .set_payout_state(
            "retry",
            PayoutState::Succeeded,
            Some("wallet-reference"),
            start(),
        )
        .unwrap();
    assert!(
        ledger
            .reserve_payout("paid-again", "alice", &[bonus], start())
            .is_err()
    );
    assert_eq!(ledger.accrued("alice").unwrap(), 10);
    assert_eq!(ledger.totals().unwrap().paid_msat, 100);
    conserved(&ledger);
}

#[test]
fn only_the_first_paid_call_of_a_plugin_earns_its_first_bonus() {
    let mut ledger = configured(100, 0, 0);
    fund(&mut ledger, "funds", 200);
    let free = ledger
        .record_settlement(call("free", "plugin", "alice", 0, 0))
        .unwrap();
    assert!(
        free.bonuses
            .iter()
            .all(|bonus| bonus.kind != "first_paid_call")
    );
    let paid = ledger
        .record_settlement(call("paid", "plugin", "alice", 10, 0))
        .unwrap();
    assert_eq!(
        paid.bonuses
            .iter()
            .find(|bonus| bonus.kind == "first_paid_call")
            .unwrap()
            .amount_msat,
        100
    );
    let next_author = ledger
        .record_settlement(call("next-author", "plugin", "bob", 10, 0))
        .unwrap();
    assert!(
        next_author
            .bonuses
            .iter()
            .all(|bonus| bonus.kind != "first_paid_call")
    );
    assert_eq!(ledger.accrued("bob").unwrap(), 0);
    conserved(&ledger);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn bonuses_and_other_recipient_shares_never_exceed_receipts(
        funding in 0i64..2_000,
        calls in prop::collection::vec(
            (0u8..4, 0i64..1_000, 0i64..1_000, 0u8..4, any::<bool>()),
            1..20,
        ),
    ) {
        let mut ledger = configured(100, 5_000, 200);
        fund(&mut ledger, "initial-funding", funding);
        for party in ["alice", "bob", OPENAGENTS] {
            payee(&mut ledger, party);
        }
        for (index, (plugin, received, fee, outcome, hosted)) in calls.into_iter().enumerate() {
            let author = if plugin % 2 == 0 { "alice" } else { "bob" };
            let mut input = call(
                &format!("call-{index}"),
                &format!("plugin-{plugin}"),
                author,
                received,
                fee,
            );
            if hosted {
                input.plugin_id = None;
                input.split = Split::HostedResource { owner: author.into() };
            }
            ledger.record_settlement(input).unwrap();
            conserved(&ledger);
            let records = ledger.since(0).unwrap();
            let recipients: i128 = records.iter().map(|record| {
                let ordinary: i128 = record.shares.iter()
                    .filter(|share| matches!(share.role.as_str(), "author" | "resource" | "bonus"))
                    .map(|share| i128::from(share.amount_msat))
                    .sum();
                let first: i128 = record.bonuses.iter()
                    .filter(|bonus| bonus.kind == "first_paid_call")
                    .map(|bonus| i128::from(bonus.amount_msat))
                    .sum();
                ordinary + first
            }).sum();
            prop_assert!(recipients <= i128::from(ledger.totals().unwrap().received_msat));
            for (party_index, party) in ["alice", "bob", OPENAGENTS].into_iter().enumerate() {
                let shares = ledger.available_shares(party).unwrap();
                if shares.is_empty() {
                    continue;
                }
                let payout = format!("payout-{index}-{party_index}");
                ledger.reserve_payout(&payout, party, &shares, start()).unwrap();
                let state = match outcome {
                    0 => PayoutState::Pending,
                    1 => PayoutState::Unknown,
                    2 => PayoutState::Succeeded,
                    _ => PayoutState::Failed,
                };
                if state != PayoutState::Pending {
                    ledger.set_payout_state(&payout, state, Some("fixture-reference"), start()).unwrap();
                }
                conserved(&ledger);
            }
        }
    }
}
