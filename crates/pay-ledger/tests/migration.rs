use std::{
    path::Path,
    sync::{Arc, Barrier},
    thread,
};

use pay_ledger::{Ledger, Rail, Recorded, SettlementInput, Split};

const START: i64 = 1_792_022_400;

fn input(key: &str, release: Option<&str>) -> SettlementInput {
    SettlementInput {
        key: key.into(),
        resource: "/v1/plugins/demo/invoke".into(),
        plugin_id: Some("demo".into()),
        release_id: release.map(str::to_owned),
        price_msat: 10_000,
        received_msat: 10_000,
        rail: Rail::Lightning,
        payer_alias: Some("caller".into()),
        settled_at: START,
        split: Split::Plugin {
            author: "alice".into(),
            fee_msat: 3_000,
        },
    }
}

fn old_schema(path: &Path) -> Recorded {
    let mut ledger = Ledger::open(path).unwrap();
    let recorded = ledger.record_settlement(input("existing", None)).unwrap();
    assert_eq!(recorded.release_id, None);
    assert!(!recorded.shares.is_empty());
    drop(ledger);
    let connection = rusqlite::Connection::open(path).unwrap();
    connection
        .execute_batch("ALTER TABLE settlement DROP COLUMN release_id;")
        .unwrap();
    assert_eq!(release_columns(&connection), 0);
    recorded
}

fn release_columns(connection: &rusqlite::Connection) -> i64 {
    connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('settlement') WHERE name='release_id'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn old_ledger_keeps_existing_settlements_and_accepts_release_attribution() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite");
    let original = old_schema(&path);
    let mut ledger = Ledger::open(&path).unwrap();
    assert_eq!(ledger.since(0).unwrap(), vec![original.clone()]);
    let claims = ledger.available_shares("alice").unwrap();
    let totals = ledger.totals().unwrap();
    // A retried old payment cannot acquire release metadata from its new payload.
    let replay = ledger
        .record_settlement(input("existing", Some("different-release")))
        .unwrap();
    assert_eq!(replay, original);
    assert_eq!(ledger.totals().unwrap(), totals);
    assert_eq!(ledger.available_shares("alice").unwrap(), claims);
    let new = ledger
        .record_settlement(input("new", Some("release-2")))
        .unwrap();
    assert_eq!(new.seq, original.seq + 1);
    assert_eq!(new.release_id.as_deref(), Some("release-2"));
    let expected = vec![original, new.clone()];
    assert_eq!(ledger.since(0).unwrap(), expected);
    let totals = ledger.totals().unwrap();
    drop(ledger);
    for _ in 0..2 {
        let mut ledger = Ledger::open(&path).unwrap();
        assert_eq!(ledger.since(0).unwrap(), expected);
        assert_eq!(ledger.totals().unwrap(), totals);
        assert_eq!(ledger.record_settlement(input("new", None)).unwrap(), new);
    }
    let connection = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(release_columns(&connection), 1);
}

#[test]
fn concurrent_opens_migrate_the_existing_ledger_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite");
    let original = old_schema(&path);
    let barrier = Arc::new(Barrier::new(4));
    let workers = (0..4)
        .map(|index| {
            let path = path.clone();
            let original = original.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let mut ledger = Ledger::open(path).unwrap();
                assert_eq!(ledger.since(0).unwrap()[0], original);
                assert_eq!(
                    ledger.record_settlement(input("existing", None)).unwrap(),
                    original
                );
                let release = format!("release-{index}");
                let recorded = ledger
                    .record_settlement(input(&format!("new-{index}"), Some(&release)))
                    .unwrap();
                assert_eq!(recorded.release_id.as_deref(), Some(release.as_str()));
                recorded
            })
        })
        .collect::<Vec<_>>();
    let mut added = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    added.sort_by_key(|record| record.seq);
    assert_eq!(
        added.iter().map(|record| record.seq).collect::<Vec<_>>(),
        vec![2, 3, 4, 5]
    );
    let ledger = Ledger::open(&path).unwrap();
    let mut expected = vec![original];
    expected.extend(added);
    assert_eq!(ledger.since(0).unwrap(), expected);
    assert_eq!(ledger.totals().unwrap().settlements, 5);
    let connection = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(release_columns(&connection), 1);
}
