//! Performance benchmarks for GitAfter notification operations

use bip39::Mnemonic;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use gitafter::nostr::cache::EventCache;
use nostr::EventTemplate;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;
use wallet::core::identity::UnifiedIdentity;

/// Create a test cache with populated events
fn create_test_cache_with_events(num_events: usize) -> (EventCache, TempDir, UnifiedIdentity) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("bench-notifications.db");
    let cache = EventCache::new(db_path).unwrap();

    let mnemonic = Mnemonic::parse(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .unwrap();
    let identity = UnifiedIdentity::from_mnemonic(mnemonic).unwrap();
    let pubkey = identity.nostr_public_key();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Create events and notifications
    for i in 0..num_events {
        let event_template = EventTemplate {
            kind: 1,
            content: format!("Review comment {}", i),
            tags: vec![vec!["p".to_string(), pubkey.to_string()]],
            created_at: now + i as u64,
        };

        let event = identity.sign_event(event_template).unwrap();
        let event_id = event.id.clone();

        cache.insert_event(&event).unwrap();

        cache
            .create_notification(
                &pubkey.to_string(),
                &event_id,
                1,
                "pr_review",
                &format!("Review {}", i),
                None,
            )
            .unwrap();
    }

    (cache, temp_dir, identity)
}

fn bench_create_notification(c: &mut Criterion) {
    let mut group = c.benchmark_group("create_notification");

    // Single notification creation (cold cache)
    group.bench_function("single_cold", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let db_path = temp_dir.path().join("bench-notifications.db");
                let cache = EventCache::new(db_path).unwrap();

                let mnemonic = Mnemonic::parse(
                    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
                )
                .unwrap();
                let identity = UnifiedIdentity::from_mnemonic(mnemonic).unwrap();
                let pubkey = identity.nostr_public_key();

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                let event_template = EventTemplate {
                    kind: 1,
                    content: "Review comment".to_string(),
                    tags: vec![vec!["p".to_string(), pubkey.to_string()]],
                    created_at: now,
                };

                let event = identity.sign_event(event_template).unwrap();
                cache.insert_event(&event).unwrap();

                (cache, event.id, pubkey.to_string(), temp_dir)
            },
            |(cache, event_id, pubkey, _temp_dir)| {
                black_box(
                    cache
                        .create_notification(
                            &pubkey,
                            &event_id,
                            1,
                            "pr_review",
                            "New review",
                            None,
                        )
                        .unwrap(),
                );
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Bulk notification creation
    for batch_size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("bulk", batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let db_path = temp_dir.path().join("bench-notifications.db");
                        let cache = EventCache::new(db_path).unwrap();

                        let mnemonic = Mnemonic::parse(
                            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
                        )
                        .unwrap();
                        let identity = UnifiedIdentity::from_mnemonic(mnemonic).unwrap();
                        let pubkey = identity.nostr_public_key();

                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs();

                        let mut event_ids = Vec::new();
                        for i in 0..batch_size {
                            let event_template = EventTemplate {
                                kind: 1,
                                content: format!("Review comment {}", i),
                                tags: vec![vec!["p".to_string(), pubkey.to_string()]],
                                created_at: now + i as u64,
                            };

                            let event = identity.sign_event(event_template).unwrap();
                            event_ids.push(event.id.clone());
                            cache.insert_event(&event).unwrap();
                        }

                        (cache, event_ids, pubkey.to_string(), temp_dir)
                    },
                    |(cache, event_ids, pubkey, _temp_dir)| {
                        for (i, event_id) in event_ids.iter().enumerate() {
                            black_box(
                                cache
                                    .create_notification(
                                        &pubkey,
                                        event_id,
                                        1,
                                        "pr_review",
                                        &format!("Review {}", i),
                                        None,
                                    )
                                    .unwrap(),
                            );
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_get_notifications(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_notifications");

    for num_notifications in [10, 100, 1000, 10000].iter() {
        let (cache, _temp_dir, identity) = create_test_cache_with_events(*num_notifications);
        let pubkey = identity.nostr_public_key();

        // Get recent notifications
        group.bench_with_input(
            BenchmarkId::new("recent_10", num_notifications),
            &cache,
            |b, cache| {
                b.iter(|| {
                    black_box(cache.get_notifications(&pubkey.to_string(), 10).unwrap());
                });
            },
        );

        // Get more notifications
        group.bench_with_input(
            BenchmarkId::new("recent_100", num_notifications),
            &cache,
            |b, cache| {
                b.iter(|| {
                    black_box(cache.get_notifications(&pubkey.to_string(), 100).unwrap());
                });
            },
        );
    }

    group.finish();
}

fn bench_get_unread_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_unread_count");

    for num_notifications in [10, 100, 1000, 10000].iter() {
        let (cache, _temp_dir, identity) = create_test_cache_with_events(*num_notifications);
        let pubkey = identity.nostr_public_key();

        group.bench_with_input(
            BenchmarkId::new("count", num_notifications),
            &cache,
            |b, cache| {
                b.iter(|| {
                    black_box(cache.get_unread_count(&pubkey.to_string()).unwrap());
                });
            },
        );
    }

    group.finish();
}

fn bench_mark_notification_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("mark_notification_read");

    // Single mark as read
    group.bench_function("single", |b| {
        b.iter_batched(
            || {
                let (cache, temp_dir, identity) = create_test_cache_with_events(1);
                let pubkey = identity.nostr_public_key();
                let notifications = cache.get_notifications(&pubkey.to_string(), 1).unwrap();
                let notification_id = notifications[0].id.clone();
                (cache, notification_id, temp_dir)
            },
            |(cache, notification_id, _temp_dir)| {
                black_box(cache.mark_notification_read(&notification_id).unwrap());
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_mark_all_notifications_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("mark_all_notifications_read");

    for num_notifications in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("bulk", num_notifications),
            num_notifications,
            |b, &num_notifications| {
                b.iter_batched(
                    || {
                        let (cache, temp_dir, identity) =
                            create_test_cache_with_events(num_notifications);
                        let pubkey = identity.nostr_public_key();
                        (cache, pubkey.to_string(), temp_dir)
                    },
                    |(cache, pubkey, _temp_dir)| {
                        black_box(cache.mark_all_notifications_read(&pubkey).unwrap());
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_create_notification,
    bench_get_notifications,
    bench_get_unread_count,
    bench_mark_notification_read,
    bench_mark_all_notifications_read
);
criterion_main!(benches);
