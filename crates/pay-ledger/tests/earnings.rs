//! Private statements use exact payable claims and pin reserved destinations.

use pay_ledger::{Error, Ledger, Payee, PayoutState, Rail, SettlementInput, Split};

const AT: i64 = 1_792_022_400;

fn settle(ledger: &mut Ledger, key: &str, party: &str, fee: i64) {
    ledger
        .record_settlement(SettlementInput {
            key: key.into(),
            resource: "/v1/plugins/demo/invoke?private=secret#secret".into(),
            plugin_id: Some("demo".into()),
            release_id: Some("public-release".into()),
            price_msat: fee + 5_000,
            received_msat: fee + 5_000,
            rail: Rail::Lightning,
            payer_alias: Some("private-payer".into()),
            settled_at: AT,
            split: Split::Plugin {
                author: party.into(),
                fee_msat: fee,
            },
        })
        .unwrap();
}

#[test]
fn destination_cas_preserves_payouts_and_signed_priority() {
    let mut ledger = Ledger::in_memory().unwrap();
    settle(&mut ledger, "private-hash", "alice", 10_555);
    ledger
        .change_account_payout("alice", 0, "old@example.com", AT)
        .unwrap();
    let shares = ledger.available_shares("alice").unwrap();
    ledger
        .reserve_payout("attempt", "alice", &shares, AT)
        .unwrap();
    ledger
        .change_account_payout("alice", 1, "new@example.com", AT + 1)
        .unwrap();
    assert_eq!(
        ledger.payout("attempt").unwrap().unwrap().destination,
        "lud16:old@example.com"
    );
    assert_eq!(
        ledger.payee("alice").unwrap().unwrap().destination_value,
        "new@example.com"
    );
    assert!(matches!(
        ledger.change_account_payout("alice", 1, "stale@example.com", AT + 2),
        Err(Error::Conflict(_))
    ));
    ledger
        .register_payee(Payee {
            party: "alice".into(),
            destination_kind: "lud16".into(),
            destination_value: "signed@example.com".into(),
            source: "release".into(),
            verified_at: AT + 3,
        })
        .unwrap();
    ledger
        .change_account_payout("alice", 2, "fallback@example.com", AT + 4)
        .unwrap();
    assert_eq!(
        ledger.payee("alice").unwrap().unwrap().destination_value,
        "signed@example.com"
    );
    assert!(
        ledger
            .change_account_payout("alice", 3, &format!("02{}", "ab".repeat(32)), AT + 5)
            .is_err()
    );
}

#[test]
fn settlement_reads_keep_unknown_liabilities_and_only_sent_claims_are_consumed() {
    let mut ledger = Ledger::in_memory().unwrap();
    settle(&mut ledger, "scope", "alice", 10_555);
    settle(&mut ledger, "other", "bob", 700_000);
    let original = ledger.settlement("scope").unwrap().unwrap();
    assert_eq!(original.key, "scope");
    assert!(ledger.settlement("missing").unwrap().is_none());
    assert!(ledger.settlement("").is_err());
    ledger
        .change_account_payout("alice", 0, "alice@example.com", AT)
        .unwrap();
    let shares = ledger.available_shares("alice").unwrap();
    ledger
        .reserve_payout("attempt", "alice", &shares, AT)
        .unwrap();
    ledger
        .set_payout_state(
            "attempt",
            PayoutState::Unknown,
            Some("stable-reference"),
            AT + 1,
        )
        .unwrap();
    let owed = ledger.settlement_liabilities("scope").unwrap();
    assert!(
        owed.iter()
            .any(|s| s.role == "author" && s.amount_msat == 10_555)
    );
    assert!(owed.iter().all(|s| s.settlement == "scope"));
    ledger
        .set_payout_state("attempt", PayoutState::Sent, None, AT + 2)
        .unwrap();
    assert!(
        !ledger
            .settlement_liabilities("scope")
            .unwrap()
            .iter()
            .any(|s| s.party == "alice")
    );
    assert_eq!(ledger.settlement("scope").unwrap().unwrap(), original);
    assert!(!ledger.settlement_liabilities("other").unwrap().is_empty());
}

#[test]
fn statement_conserves_exact_claims_and_redacts_payer_data() {
    let mut ledger = Ledger::in_memory().unwrap();
    settle(&mut ledger, "private-hash", "alice", 10_555);
    settle(&mut ledger, "other-private-hash", "bob", 700_000);
    ledger
        .change_account_payout("alice", 0, "alice@example.com", AT)
        .unwrap();
    let shares = ledger.available_shares("alice").unwrap();
    let earned = ledger
        .reserve_payout("attempt", "alice", &shares, AT)
        .unwrap();
    for state in [
        PayoutState::Planned,
        PayoutState::Sending,
        PayoutState::Unknown,
    ] {
        if state == PayoutState::Sending {
            ledger
                .begin_send(
                    "attempt",
                    "wallet-ref",
                    Some("private-invoice"),
                    earned / 1000 * 1000,
                    AT + 1,
                )
                .unwrap();
        }
        if state == PayoutState::Unknown {
            ledger
                .finish_payout("attempt", state, None, Some("private-wallet-error"), AT + 2)
                .unwrap();
        }
        let s = ledger.earnings_statement("alice", 0, 0, 1).unwrap();
        assert_eq!(s.figures.earned_msat, earned);
        assert_eq!(s.figures.accrued_msat, 0);
        assert_eq!(s.figures.reserved_msat, earned);
        assert_eq!(s.payouts[0].state, state);
        let text = serde_json::to_string(&s).unwrap();
        for private in [
            "private-hash",
            "other-private-hash",
            "private-payer",
            "private-invoice",
            "private-wallet-error",
            "private=secret",
            "bob",
        ] {
            assert!(!text.contains(private), "{private}: {text}");
        }
        assert_eq!(s.earnings[0].resource, "/v1/plugins/demo/invoke");
    }
    ledger
        .finish_payout("attempt", PayoutState::Sent, Some(7), None, AT + 3)
        .unwrap();
    let s = ledger.earnings_statement("alice", 0, 0, 200).unwrap();
    assert_eq!(
        s.figures.earned_msat,
        s.figures.accrued_msat + s.figures.reserved_msat + s.figures.consumed_msat
    );
    assert_eq!(s.figures.sent_msat, earned / 1000 * 1000);
    assert_eq!(s.figures.rounding_msat, earned % 1000);
    assert_eq!(s.payouts[0].fee_msat, Some(7));
    assert!(ledger.earnings_payout("bob", "attempt").unwrap().is_none());
    assert!(ledger.earnings_statement("alice", 0, 0, 201).is_err());
}

#[test]
fn failed_attempt_returns_claims_and_pages_do_not_skip_roles() {
    let mut ledger = Ledger::in_memory().unwrap();
    settle(&mut ledger, "h1", "alice", 10_000);
    settle(&mut ledger, "h2", "alice", 20_000);
    ledger
        .change_account_payout("alice", 0, "alice@example.com", AT)
        .unwrap();
    let shares = ledger.available_shares("alice").unwrap();
    ledger
        .reserve_payout("failed", "alice", &shares, AT)
        .unwrap();
    ledger
        .finish_payout("failed", PayoutState::Failed, None, Some("refused"), AT + 1)
        .unwrap();
    let first = ledger.earnings_statement("alice", 0, 0, 1).unwrap();
    assert_eq!(first.figures.reserved_msat, 0);
    assert_eq!(first.figures.earned_msat, first.figures.accrued_msat);
    assert!(
        first.earnings[0]
            .obligations
            .iter()
            .all(|o| o.state == "accrued" && o.payout.is_none())
    );
    let second = ledger
        .earnings_statement("alice", first.next_earning.unwrap(), 0, 1)
        .unwrap();
    assert_eq!(second.earnings.len(), 1);
    assert_ne!(first.earnings[0].sequence, second.earnings[0].sequence);
    assert!(second.next_earning.is_none());
}

#[test]
fn resolution_uses_the_current_persisted_fallback_after_source_gathering() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fallback.sqlite");
    let mut worker = Ledger::open(&path).unwrap();
    worker
        .change_account_payout("alice", 0, "old@example.com", AT)
        .unwrap();
    let found = worker
        .resolve_payee("alice", AT + 1, || {
            Ledger::open(&path)
                .unwrap()
                .change_account_payout("alice", 1, "new@example.com", AT + 1)
                .unwrap();
            // A source adapter can have captured an older account setting before
            // its relay reads. The persisted setting is the authoritative fallback.
            pay_ledger::payee::Sources {
                account_payout: Some("old@example.com".into()),
                ..Default::default()
            }
        })
        .unwrap()
        .unwrap();
    assert_eq!(found.destination_value, "new@example.com");
    assert_eq!(worker.account_payout("alice").unwrap().unwrap().version, 2);
}
