//! Live M2 conformance. Set `NOSTR_RELAY_TEST_DATABASE_URL`, or run
//! `scripts/test-postgres.sh`, to exercise an isolated real Postgres cluster.

use std::{collections::BTreeMap, time::Duration};

use nostr_relay::{
    domain::{Event, Filter, Tag},
    store::{AdmissionOutcome, AdmissionRejection, NotificationListener, Store, StoreError},
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use tokio::{sync::watch, time::timeout};
use tokio_postgres::NoTls;

const NOW: u64 = 10_000;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn m2_store_contract_against_postgres() {
    let Ok(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL") else {
        eprintln!("skipped: set NOSTR_RELAY_TEST_DATABASE_URL or run scripts/test-postgres.sh");
        return;
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!(
            "skipped: the live suite changes roles and migration metadata; use scripts/test-postgres.sh"
        );
        return;
    }

    let (initial_store, report) = Store::connect_with_report(&database_url).await.unwrap();
    assert_eq!(report.applied_versions, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    drop(initial_store);

    let (mut store, report) = Store::connect_with_report(&database_url).await.unwrap();
    assert_eq!(report.applied_versions, Vec::<i64>::new());
    assert!(store.is_current());

    let (_second_store, report) = Store::connect_with_report(&database_url).await.unwrap();
    assert!(
        report.applied_versions.is_empty(),
        "migrations are idempotent"
    );
    let _verified_store = Store::connect_verified(&database_url).await.unwrap();
    if database_url.contains("dbname=nostr_relay_test") {
        least_privilege_runtime(&database_url).await;
    }

    nostr_effect_import_is_idempotent(&database_url, &mut store).await;

    let mut listener = NotificationListener::connect(&database_url, 64)
        .await
        .unwrap();

    let regular = signed_event(
        1,
        100,
        1,
        vec![
            Tag::new(vec!["e".into(), "indexed-value".into(), "ignored".into()]),
            Tag::new(vec!["alt".into(), "not-indexed".into()]),
        ],
        "hello searchable world",
    );
    let stored = store.admit(&regular, NOW).await.unwrap();
    let AdmissionOutcome::Stored { ingest_seq } = stored else {
        panic!("regular event was not stored: {stored:?}");
    };
    assert_eq!(
        timeout(Duration::from_secs(2), listener.recv())
            .await
            .unwrap(),
        Some(ingest_seq),
        "NOTIFY is delivered only after commit"
    );
    assert_eq!(
        store.admit(&regular, NOW).await.unwrap(),
        AdmissionOutcome::Duplicate
    );
    assert_eq!(
        store
            .event_by_id(&regular.id, NOW)
            .await
            .unwrap()
            .unwrap()
            .event,
        regular
    );

    let tag_filter = Filter {
        tags: BTreeMap::from([("e".to_owned(), vec!["indexed-value".to_owned()])]),
        ..Filter::default()
    };
    assert_eq!(
        store
            .query_filter(&tag_filter, NOW, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    let ignored_filter = Filter {
        tags: BTreeMap::from([("e".to_owned(), vec!["ignored".to_owned()])]),
        ..Filter::default()
    };
    assert!(
        store
            .query_filter(&ignored_filter, NOW, 10)
            .await
            .unwrap()
            .is_empty()
    );
    let (cancel, cancelled) = watch::channel(false);
    cancel.send(true).unwrap();
    assert!(matches!(
        store
            .query_filter_cancellable(&Filter::default(), NOW, 10, i64::MAX, cancelled)
            .await,
        Err(StoreError::QueryCancelled)
    ));

    let ephemeral = signed_event(1, 110, 20_000, Vec::new(), "never stored");
    assert_eq!(
        store.admit(&ephemeral, NOW).await.unwrap(),
        AdmissionOutcome::Ephemeral
    );
    assert!(
        store
            .event_by_id(&ephemeral.id, NOW)
            .await
            .unwrap()
            .is_none()
    );

    let expiring = signed_event(
        1,
        120,
        1,
        vec![Tag::new(vec!["expiration".into(), "200".into()])],
        "temporary",
    );
    store.admit(&expiring, 150).await.unwrap();
    assert!(
        store
            .event_by_id(&expiring.id, 199)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .event_by_id(&expiring.id, 200)
            .await
            .unwrap()
            .is_none()
    );

    let expiring_target = signed_event(12, 121, 1, Vec::new(), "remains deleted");
    let expiring_deletion = signed_event(
        12,
        122,
        5,
        vec![
            Tag::new(vec!["e".into(), expiring_target.id.clone()]),
            Tag::new(vec!["expiration".into(), "200".into()]),
        ],
        "expiring deletion publication",
    );
    store.admit(&expiring_deletion, 150).await.unwrap();
    assert!(store.delete_expired(200).await.unwrap() >= 2);
    assert!(
        store
            .event_by_id(&expiring_deletion.id, 199)
            .await
            .unwrap()
            .is_none(),
        "the deletion publication was physically removed"
    );
    assert_eq!(
        store.admit(&expiring_target, NOW).await.unwrap(),
        AdmissionOutcome::Rejected(AdmissionRejection::Deleted),
        "the durable tombstone outlives its expired source event"
    );

    replacement_race(&database_url, &mut store).await;
    deletion_before_event(&database_url, &mut store).await;
    concurrent_deletion_race(&database_url, &mut store).await;
    concurrent_media_quota(&database_url, &mut store).await;
    search_equivalence(&mut store).await;
    policy_and_fts(&database_url, &mut store).await;

    let high_water = store.latest_ingest_seq().await.unwrap();
    let catch_up = store.events_after(0, high_water, NOW, 100).await.unwrap();
    assert!(!catch_up.is_empty());
    assert!(
        catch_up
            .windows(2)
            .all(|pair| pair[0].ingest_seq < pair[1].ingest_seq)
    );
    assert!(listener.is_current());
    migration_hash_drift_fails_closed(&database_url).await;
}

async fn nostr_effect_import_is_idempotent(database_url: &str, store: &mut Store) {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    client
        .batch_execute(
            r#"
CREATE TABLE events (
    id text PRIMARY KEY,
    pubkey text NOT NULL,
    created_at bigint NOT NULL,
    kind integer NOT NULL,
    tags jsonb NOT NULL,
    content text NOT NULL,
    sig text NOT NULL,
    d_tag text
)
"#,
        )
        .await
        .unwrap();
    let event = signed_event(99, 99, 1, vec![], "legacy event");
    let group_event = signed_event(
        98,
        98,
        9,
        vec![Tag::new(vec!["h".into(), "legacy-group".into()])],
        "legacy group event",
    );
    let expired = signed_event(
        97,
        50,
        1,
        vec![Tag::new(vec!["expiration".into(), "60".into()])],
        "expired legacy event",
    );
    let insert = client
        .prepare(
            r#"
INSERT INTO events (id, pubkey, created_at, kind, tags, content, sig, d_tag)
VALUES ($1, $2, $3, $4, $5::text::jsonb, $6, $7, NULL)
"#,
        )
        .await
        .unwrap();
    for source in [&event, &group_event, &expired] {
        let tags = serde_json::to_string(&source.tags).unwrap();
        let created_at = source.created_at as i64;
        let kind = i32::from(source.kind);
        client
            .execute(
                &insert,
                &[
                    &source.id,
                    &source.pubkey,
                    &created_at,
                    &kind,
                    &tags,
                    &source.content,
                    &source.sig,
                ],
            )
            .await
            .unwrap();
    }

    let report = store.import_nostr_effect_events(NOW, None).await.unwrap();
    assert_eq!(report.scanned, 3);
    assert_eq!(report.stored, 2);
    assert_eq!(report.expired, 1);
    assert_eq!(report.rejected, 0);
    assert_eq!(
        store
            .event_by_id(&event.id, NOW)
            .await
            .unwrap()
            .unwrap()
            .event,
        event
    );
    assert_eq!(
        store
            .event_by_id(&group_event.id, NOW)
            .await
            .unwrap()
            .unwrap()
            .event,
        group_event,
        "legacy group history bypasses only newer group-derived admission"
    );
    assert!(store.event_by_id(&expired.id, NOW).await.unwrap().is_none());
    assert!(
        store
            .import_nostr_effect_events(NOW, None)
            .await
            .unwrap()
            .is_empty(),
        "ledger makes the compatibility import idempotent"
    );
    let retry = store
        .retry_rejected_nostr_effect_events(NOW, None)
        .await
        .unwrap();
    assert_eq!(retry.scanned, 0);
    assert_eq!(retry.rejected, 0);
}

async fn replacement_race(database_url: &str, store: &mut Store) {
    let first = signed_event(
        2,
        300,
        30_000,
        vec![Tag::new(vec!["d".into(), "race".into()])],
        "candidate one",
    );
    let second = signed_event(
        2,
        300,
        30_000,
        vec![Tag::new(vec!["d".into(), "race".into()])],
        "candidate two",
    );
    let (lower, higher) = if first.id < second.id {
        (first, second)
    } else {
        (second, first)
    };
    let mut other = Store::connect(database_url).await.unwrap();
    let (left, right) = tokio::join!(store.admit(&higher, NOW), other.admit(&lower, NOW));
    left.unwrap();
    right.unwrap();

    let filter = Filter {
        kinds: Some(vec![30_000]),
        tags: BTreeMap::from([("d".to_owned(), vec!["race".to_owned()])]),
        ..Filter::default()
    };
    let events = store.query_filter(&filter, NOW, 10).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event.id, lower.id, "lowest ID wins timestamp tie");
    assert!(store.event_by_id(&higher.id, NOW).await.unwrap().is_none());
}

async fn deletion_before_event(_database_url: &str, store: &mut Store) {
    let target = signed_event(3, 400, 1, Vec::new(), "arrives too late");
    let deletion = signed_event(
        3,
        500,
        5,
        vec![Tag::new(vec!["e".into(), target.id.clone()])],
        "delete target",
    );
    store.admit(&deletion, NOW).await.unwrap();
    assert_eq!(
        store.admit(&target, NOW).await.unwrap(),
        AdmissionOutcome::Rejected(AdmissionRejection::Deleted)
    );

    let old = signed_event(
        3,
        600,
        30_001,
        vec![Tag::new(vec!["d".into(), "deleted-address".into()])],
        "old address version",
    );
    store.admit(&old, NOW).await.unwrap();
    let address = format!("30001:{}:deleted-address", old.pubkey);
    let address_deletion = signed_event(
        3,
        700,
        5,
        vec![Tag::new(vec!["a".into(), address])],
        "delete address",
    );
    store.admit(&address_deletion, NOW).await.unwrap();
    assert!(store.event_by_id(&old.id, NOW).await.unwrap().is_none());

    let newer = signed_event(
        3,
        701,
        30_001,
        vec![Tag::new(vec!["d".into(), "deleted-address".into()])],
        "new address version",
    );
    assert!(matches!(
        store.admit(&newer, NOW).await.unwrap(),
        AdmissionOutcome::Stored { .. }
    ));
}

async fn concurrent_deletion_race(database_url: &str, store: &mut Store) {
    let target = signed_event(4, 800, 1, Vec::new(), "racing target");
    let deletion = signed_event(
        4,
        801,
        5,
        vec![Tag::new(vec!["e".into(), target.id.clone()])],
        "racing deletion",
    );
    let mut other = Store::connect(database_url).await.unwrap();
    let (target_result, deletion_result) =
        tokio::join!(store.admit(&target, NOW), other.admit(&deletion, NOW));
    target_result.unwrap();
    assert!(matches!(
        deletion_result.unwrap(),
        AdmissionOutcome::Stored { .. }
    ));
    assert!(store.event_by_id(&target.id, NOW).await.unwrap().is_none());
}

async fn concurrent_media_quota(database_url: &str, store: &mut Store) {
    let mut other = Store::connect(database_url).await.unwrap();
    let pubkey = "f".repeat(64);
    let first_authorization = "a".repeat(64);
    let first_hash = "b".repeat(64);
    let second_authorization = "c".repeat(64);
    let second_hash = "d".repeat(64);
    let first = store.register_media(
        &first_authorization,
        &pubkey,
        &first_hash,
        600,
        "application/octet-stream",
        NOW,
        1_000,
    );
    let second = other.register_media(
        &second_authorization,
        &pubkey,
        &second_hash,
        600,
        "application/octet-stream",
        NOW,
        1_000,
    );
    let (first, second) = tokio::join!(first, second);
    let outcomes = [first, second];
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| matches!(result, Err(StoreError::Media(reason)) if reason.contains("quota")))
            .count(),
        1,
        "different hashes for one owner must serialize quota accounting"
    );
}

async fn policy_and_fts(database_url: &str, store: &mut Store) {
    let (admin, connection) = tokio_postgres::connect(database_url, NoTls).await.unwrap();
    let driver = tokio::spawn(connection);

    let blocked = signed_event(5, 900, 1, Vec::new(), "blocked author");
    let statement = admin
        .prepare("INSERT INTO relay_blocked_pubkey (pubkey, reason) VALUES ($1, $2)")
        .await
        .unwrap();
    admin
        .execute(&statement, &[&blocked.pubkey, &"fixture block"])
        .await
        .unwrap();
    assert_eq!(
        store.admit(&blocked, NOW).await.unwrap(),
        AdmissionOutcome::Rejected(AdmissionRejection::BlockedPubkey(
            "fixture block".to_owned()
        ))
    );

    let blocked_kind = signed_event(7, 901, 42, Vec::new(), "blocked kind");
    let statement = admin
        .prepare("INSERT INTO relay_blocked_kind (kind, reason) VALUES ($1, $2)")
        .await
        .unwrap();
    admin
        .execute(&statement, &[&42_i32, &"fixture kind block"])
        .await
        .unwrap();
    assert_eq!(
        store.admit(&blocked_kind, NOW).await.unwrap(),
        AdmissionOutcome::Rejected(AdmissionRejection::BlockedKind(
            "fixture kind block".to_owned()
        ))
    );

    let kind_not_allowed = signed_event(7, 902, 7, Vec::new(), "kind allowlist");
    let statement = admin
        .prepare("INSERT INTO relay_allowed_kind (kind, reason) VALUES ($1, $2)")
        .await
        .unwrap();
    admin
        .execute(&statement, &[&1_i32, &"fixture allow"])
        .await
        .unwrap();
    assert_eq!(
        store.admit(&kind_not_allowed, NOW).await.unwrap(),
        AdmissionOutcome::Rejected(AdmissionRejection::KindNotAllowed)
    );
    let statement = admin
        .prepare("DELETE FROM relay_allowed_kind")
        .await
        .unwrap();
    admin.execute(&statement, &[]).await.unwrap();

    let allowed_author = signed_event(7, 903, 1, Vec::new(), "allowed author");
    let disallowed_author = signed_event(8, 903, 1, Vec::new(), "other author");
    let statement = admin
        .prepare("INSERT INTO relay_allowed_pubkey (pubkey, reason) VALUES ($1, $2)")
        .await
        .unwrap();
    admin
        .execute(&statement, &[&allowed_author.pubkey, &"fixture allow"])
        .await
        .unwrap();
    assert_eq!(
        store.admit(&disallowed_author, NOW).await.unwrap(),
        AdmissionOutcome::Rejected(AdmissionRejection::PubkeyNotAllowed)
    );
    assert!(matches!(
        store.admit(&allowed_author, NOW).await.unwrap(),
        AdmissionOutcome::Stored { .. }
    ));
    let statement = admin
        .prepare("DELETE FROM relay_allowed_pubkey")
        .await
        .unwrap();
    admin.execute(&statement, &[]).await.unwrap();

    let member = signed_event(9, 904, 1, Vec::new(), "closed relay member");
    let statement = admin
        .prepare("UPDATE relay_policy SET closed_membership = $1 WHERE singleton = TRUE")
        .await
        .unwrap();
    admin.execute(&statement, &[&true]).await.unwrap();
    assert_eq!(
        store.admit(&member, NOW).await.unwrap(),
        AdmissionOutcome::Rejected(AdmissionRejection::NotMember)
    );
    let statement = admin
        .prepare("INSERT INTO relay_member_pubkey (pubkey, note) VALUES ($1, $2)")
        .await
        .unwrap();
    admin
        .execute(&statement, &[&member.pubkey, &"fixture member"])
        .await
        .unwrap();
    assert!(matches!(
        store.admit(&member, NOW).await.unwrap(),
        AdmissionOutcome::Stored { .. }
    ));
    let statement = admin
        .prepare("UPDATE relay_policy SET closed_membership = $1 WHERE singleton = TRUE")
        .await
        .unwrap();
    admin.execute(&statement, &[&false]).await.unwrap();

    let oversized = signed_event(10, 905, 1, Vec::new(), "12345");
    let statement = admin
        .prepare("UPDATE relay_policy SET max_content_bytes = $1 WHERE singleton = TRUE")
        .await
        .unwrap();
    admin.execute(&statement, &[&4_i64]).await.unwrap();
    assert_eq!(
        store.admit(&oversized, NOW).await.unwrap(),
        AdmissionOutcome::Rejected(AdmissionRejection::ContentTooLarge {
            actual_bytes: 5,
            max_bytes: 4,
        })
    );
    admin.execute(&statement, &[&131_072_i64]).await.unwrap();

    let overtagged = signed_event(
        10,
        906,
        1,
        vec![Tag::new(vec!["e".into(), "tag-limit".into()])],
        "tag bound",
    );
    let statement = admin
        .prepare("UPDATE relay_policy SET max_tags = $1 WHERE singleton = TRUE")
        .await
        .unwrap();
    admin.execute(&statement, &[&0_i32]).await.unwrap();
    assert_eq!(
        store.admit(&overtagged, NOW).await.unwrap(),
        AdmissionOutcome::Rejected(AdmissionRejection::TooManyTags { actual: 1, max: 0 })
    );
    admin.execute(&statement, &[&256_i32]).await.unwrap();

    let future = signed_event(10, NOW + 1, 1, Vec::new(), "future bound");
    let statement = admin
        .prepare("UPDATE relay_policy SET max_future_seconds = $1 WHERE singleton = TRUE")
        .await
        .unwrap();
    admin.execute(&statement, &[&0_i64]).await.unwrap();
    assert_eq!(
        store.admit(&future, NOW).await.unwrap(),
        AdmissionOutcome::Rejected(AdmissionRejection::TimestampTooFarInFuture {
            created_at: NOW + 1,
            latest_allowed: NOW,
        })
    );
    admin.execute(&statement, &[&900_i64]).await.unwrap();

    let old = signed_event(10, NOW - 2, 1, Vec::new(), "past bound");
    let statement = admin
        .prepare("UPDATE relay_policy SET max_past_seconds = $1 WHERE singleton = TRUE")
        .await
        .unwrap();
    admin.execute(&statement, &[&1_i64]).await.unwrap();
    assert_eq!(
        store.admit(&old, NOW).await.unwrap(),
        AdmissionOutcome::Rejected(AdmissionRejection::TimestampTooOld {
            created_at: NOW - 2,
            earliest_allowed: NOW - 1,
        })
    );
    admin.execute(&statement, &[&0_i64]).await.unwrap();

    let fts = admin
        .prepare(
            "SELECT count(*) FROM nostr_event WHERE search_vector @@ plainto_tsquery('simple', $1)",
        )
        .await
        .unwrap();
    let count = admin
        .query_one(&fts, &[&"searchable"])
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(count, 1);

    drop(admin);
    driver.await.unwrap().unwrap();
}

/// Judge the shared NIP-50 corpus through the replay and COUNT SQL and
/// require the verdicts to equal `search_matches`, the rule live delivery
/// applies. `crates/nostr/tests/search_equivalence.rs` holds the pure
/// crate's half of the same fixture.
async fn search_equivalence(store: &mut Store) {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/nip50/search-equivalence.json"
    ))
    .unwrap();
    let corpus = fixture["corpus"].as_array().unwrap();
    let searches = fixture["searches"].as_array().unwrap();

    let mut events = Vec::with_capacity(corpus.len());
    for (index, entry) in corpus.iter().enumerate() {
        let kind = u16::try_from(entry["kind"].as_u64().unwrap()).unwrap();
        let content = entry["content"].as_str().unwrap();
        let mut tags = Vec::new();
        if (30_000..40_000).contains(&kind) {
            tags.push(Tag::new(vec!["d".into(), format!("search-{index}")]));
        }
        if kind == 1_059 {
            tags.push(Tag::new(vec!["p".into(), "2".repeat(64)]));
        }
        let created_at = 5_000 + u64::try_from(index).unwrap();
        let event = signed_event(40, created_at, kind, tags, content);
        // The corpus carries every search-excluded kind; historical
        // admission stores them without the Block ingest validators so the
        // query predicate, not admission, decides whether they match.
        let outcome = store.admit_historical(&event, NOW).await.unwrap();
        assert!(
            matches!(outcome, AdmissionOutcome::Stored { .. }),
            "corpus entry {index} ({kind}, {content:?}) was not stored: {outcome:?}"
        );
        events.push(event);
    }

    for case in searches {
        let search = case["search"].as_str().unwrap();
        let filter = Filter {
            search: Some(search.to_owned()),
            ..Filter::default()
        };
        let expected = case["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|index| usize::try_from(index.as_u64().unwrap()).unwrap())
            .collect::<Vec<_>>();
        let replayed = store.query_filter(&filter, NOW, 1_000).await;
        if !case["valid"].as_bool().unwrap() {
            assert!(
                matches!(replayed, Err(StoreError::Domain(_))),
                "replay accepted invalid search {search:?}: {replayed:?}"
            );
            continue;
        }
        let replayed = replayed.unwrap();
        for stored in &replayed {
            assert!(
                filter.matches(&stored.event),
                "replay returned an event live delivery rejects for search {search:?}: kind {} content {:?}",
                stored.event.kind,
                stored.event.content
            );
        }
        let from_corpus = events
            .iter()
            .enumerate()
            .filter(|(_, event)| replayed.iter().any(|stored| stored.event.id == event.id))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(
            from_corpus, expected,
            "replay membership for search {search:?}"
        );
        let live = events
            .iter()
            .enumerate()
            .filter(|(_, event)| filter.matches(event))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(live, expected, "live membership for search {search:?}");
        let counted = store
            .count_filters(std::slice::from_ref(&filter), NOW, 1_000, &[])
            .await
            .unwrap();
        assert_eq!(
            counted,
            Some(replayed.len()),
            "COUNT disagrees with replay for search {search:?}"
        );
    }
}

async fn least_privilege_runtime(database_url: &str) {
    let (admin, connection) = tokio_postgres::connect(database_url, NoTls).await.unwrap();
    let driver = tokio::spawn(connection);
    for sql in [
        "CREATE ROLE nostr_relay_runtime_m2_test LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION",
        "GRANT USAGE ON SCHEMA public TO nostr_relay_runtime_m2_test",
        "GRANT SELECT ON schema_migrations TO nostr_relay_runtime_m2_test",
        "GRANT SELECT, INSERT, DELETE ON nostr_event TO nostr_relay_runtime_m2_test",
        "GRANT SELECT, INSERT ON nostr_indexed_tag TO nostr_relay_runtime_m2_test",
        "GRANT SELECT, INSERT, UPDATE ON replaceable_head TO nostr_relay_runtime_m2_test",
        "GRANT SELECT, INSERT, UPDATE ON deletion_tombstone TO nostr_relay_runtime_m2_test",
        "GRANT SELECT ON relay_policy, relay_allowed_pubkey, relay_allowed_kind, relay_member_pubkey, relay_blocked_pubkey, relay_blocked_kind TO nostr_relay_runtime_m2_test",
        "GRANT USAGE, SELECT ON SEQUENCE nostr_event_ingest_seq_seq TO nostr_relay_runtime_m2_test",
    ] {
        let statement = admin.prepare(sql).await.unwrap();
        admin.execute(&statement, &[]).await.unwrap();
    }
    drop(admin);
    driver.await.unwrap().unwrap();

    let runtime_url = format!("{database_url} user=nostr_relay_runtime_m2_test");
    let mut runtime = Store::connect_verified(&runtime_url).await.unwrap();
    let event = signed_event(6, 50, 7, Vec::new(), "least privilege write");
    assert!(matches!(
        runtime.admit(&event, NOW).await.unwrap(),
        AdmissionOutcome::Stored { .. }
    ));
    assert!(runtime.event_by_id(&event.id, NOW).await.unwrap().is_some());
    let private = signed_event(
        6,
        51,
        39_604,
        vec![Tag::new(vec!["d".into(), "6".repeat(64)])],
        "least privilege immutable write",
    );
    assert!(matches!(
        runtime.admit(&private, NOW).await.unwrap(),
        AdmissionOutcome::Stored { .. }
    ));
    assert!(
        runtime
            .event_by_id(&private.id, NOW)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        runtime.admit(&private, NOW).await.unwrap(),
        AdmissionOutcome::Duplicate
    );
}

async fn migration_hash_drift_fails_closed(database_url: &str) {
    let (admin, connection) = tokio_postgres::connect(database_url, NoTls).await.unwrap();
    let driver = tokio::spawn(connection);
    let statement = admin
        .prepare("UPDATE schema_migrations SET sha256 = $1 WHERE version = $2")
        .await
        .unwrap();
    admin
        .execute(&statement, &[&"0".repeat(64), &1_i64])
        .await
        .unwrap();
    drop(admin);
    driver.await.unwrap().unwrap();

    let error = match Store::connect_verified(database_url).await {
        Ok(_) => panic!("changed migration hash was accepted"),
        Err(error) => error,
    };
    assert!(matches!(error, StoreError::MigrationDrift(_)));
}

fn signed_event(
    secret_byte: u8,
    created_at: u64,
    kind: u16,
    tags: Vec<Tag>,
    content: &str,
) -> Event {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32]).unwrap();
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let pubkey = keypair.x_only_public_key().0.to_string();
    let content = content.to_owned();
    let mut event = Event {
        id: "0".repeat(64),
        pubkey,
        created_at,
        kind,
        tags,
        content,
        sig: "0".repeat(128),
    };
    let id_bytes = event.computed_id_bytes().unwrap();
    event.id = event.computed_id().unwrap();
    event.sig = secp
        .sign_schnorr_no_aux_rand(&id_bytes, &keypair)
        .to_string();
    event
}
