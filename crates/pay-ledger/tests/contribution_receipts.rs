//! Exact release payout attribution through an immutable local ledger reader.
use pay_ledger::{Ledger, Payee, PayoutState, Rail, SettlementInput, Split};

const NOW: i64 = 1_792_022_400;

fn settlement(ledger: &mut Ledger, key: &str, release: &str, party: &str) {
    ledger
        .record_settlement(SettlementInput {
            key: key.into(),
            resource: format!("resource:{key}"),
            plugin_id: Some("example-plugin".into()),
            release_id: Some(release.into()),
            price_msat: 2_000,
            received_msat: 2_000,
            rail: Rail::Lightning,
            payer_alias: Some("private payer".into()),
            settled_at: NOW,
            split: Split::Plugin {
                author: party.into(),
                fee_msat: 1_000,
            },
        })
        .unwrap();
}

fn reserve(ledger: &mut Ledger, payout: &str, key: &str) {
    let share = ledger
        .available_shares("alice")
        .unwrap()
        .into_iter()
        .find(|share| share.settlement == key && share.role == "author")
        .unwrap();
    ledger
        .reserve_payout(payout, "alice", &[share], NOW + 1)
        .unwrap();
}

#[test]
fn exact_share_requires_its_own_sent_payout_and_reads_do_not_mutate() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("ledger.sqlite");
    let mut writer = Ledger::open(&path).unwrap();
    writer
        .register_payee(Payee {
            party: "alice".into(),
            destination_kind: "spark".into(),
            destination_value: "fake-destination".into(),
            source: "fixture".into(),
            verified_at: NOW,
        })
        .unwrap();
    settlement(&mut writer, "old-call", "old-release", "alice");
    settlement(&mut writer, "current-call", "current-release", "alice");
    settlement(&mut writer, "other-party", "current-release", "bob");
    reserve(&mut writer, "old-payout", "old-call");
    writer
        .begin_send("old-payout", "old-wallet-reference", None, 1_000, NOW + 2)
        .unwrap();
    writer
        .finish_payout("old-payout", PayoutState::Sent, Some(0), None, NOW + 3)
        .unwrap();
    let current = writer
        .contribution_receipts("example-plugin", "current-release", "alice")
        .unwrap();
    assert_eq!(current.len(), 1);
    assert!(current[0].payout_id.is_none());
    assert!(!current[0].is_settled());
    reserve(&mut writer, "failed-payout", "current-call");
    writer
        .finish_payout(
            "failed-payout",
            PayoutState::Failed,
            None,
            Some("fixture refusal"),
            NOW + 2,
        )
        .unwrap();
    reserve(&mut writer, "current-payout", "current-call");
    writer
        .begin_send(
            "current-payout",
            "current-wallet-reference",
            None,
            1_000,
            NOW + 3,
        )
        .unwrap();
    writer
        .finish_payout(
            "current-payout",
            PayoutState::Unknown,
            None,
            Some("lost acknowledgement"),
            NOW + 4,
        )
        .unwrap();
    let uncertain = writer
        .contribution_receipts("example-plugin", "current-release", "alice")
        .unwrap();
    assert_eq!(uncertain.len(), 2);
    assert!(uncertain.iter().all(|receipt| !receipt.is_settled()));
    writer
        .finish_payout("current-payout", PayoutState::Sent, Some(0), None, NOW + 5)
        .unwrap();
    drop(writer);
    let before = std::fs::read(&path).unwrap();
    let mut reader = Ledger::open_read_only(&path).unwrap();
    let receipts = reader
        .contribution_receipts("example-plugin", "current-release", "alice")
        .unwrap();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts.iter().filter(|r| r.is_settled()).count(), 1);
    let paid = receipts.iter().find(|r| r.is_settled()).unwrap();
    assert_eq!(paid.settlement, "current-call");
    assert_eq!(paid.share_msat, 1_000);
    assert_eq!(paid.sent_msat, Some(1_000));
    assert_eq!(
        paid.wallet_reference.as_deref(),
        Some("current-wallet-reference")
    );
    assert!(
        receipts
            .iter()
            .all(|r| r.party == "alice" && r.release_id == "current-release")
    );
    assert!(
        reader
            .contribution_receipts("another-plugin", "current-release", "alice")
            .unwrap()
            .is_empty()
    );
    assert!(
        reader
            .contribution_receipts("example-plugin", "absent-release", "alice")
            .unwrap()
            .is_empty()
    );
    assert!(
        reader
            .contribution_receipts("example-plugin", "current-release", "unrelated-party")
            .unwrap()
            .is_empty()
    );
    assert!(
        reader
            .set_payout_state("current-payout", PayoutState::Failed, None, NOW + 6)
            .is_err()
    );
    assert!(
        reader
            .register_payee(Payee {
                party: "blocked-write".into(),
                destination_kind: "spark".into(),
                destination_value: "fake-destination".into(),
                source: "fixture".into(),
                verified_at: NOW,
            })
            .is_err()
    );
    drop(reader);
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn read_only_open_never_creates_or_upgrades_an_incompatible_database() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing.sqlite");
    assert!(Ledger::open_read_only(&missing).is_err());
    assert!(!missing.exists());
    let path = temp.path().join("legacy.sqlite");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE retained_marker(value TEXT); INSERT INTO retained_marker VALUES('unchanged');").unwrap();
    drop(connection);
    let before = std::fs::read(&path).unwrap();
    let reader = Ledger::open_read_only(&path).unwrap();
    assert!(
        reader
            .contribution_receipts("plugin", "release", "alice")
            .is_err()
    );
    drop(reader);
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn a_sent_label_without_a_recorded_amount_does_not_prove_settlement() {
    let mut ledger = Ledger::in_memory().unwrap();
    ledger
        .register_payee(Payee {
            party: "alice".into(),
            destination_kind: "spark".into(),
            destination_value: "fake-destination".into(),
            source: "fixture".into(),
            verified_at: NOW,
        })
        .unwrap();
    settlement(&mut ledger, "legacy-call", "legacy-release", "alice");
    reserve(&mut ledger, "legacy-payout", "legacy-call");
    ledger
        .set_payout_state(
            "legacy-payout",
            PayoutState::Sent,
            Some("legacy-reference"),
            NOW + 2,
        )
        .unwrap();
    let receipts = ledger
        .contribution_receipts("example-plugin", "legacy-release", "alice")
        .unwrap();
    assert_eq!(receipts[0].payout_state, Some(PayoutState::Sent));
    assert_eq!(receipts[0].sent_msat, None);
    assert!(!receipts[0].is_settled());
}
