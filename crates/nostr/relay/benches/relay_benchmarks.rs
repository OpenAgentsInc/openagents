//! Performance benchmarks for nostr-relay
//!
//! Run with: cargo bench --package nostr-relay --features full
//!
//! Benchmarks cover:
//! - Event insertion (single and batch)
//! - Event queries with various filters
//! - Filter matching
//! - Subscription management
//! - Broadcast operations

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use nostr::{EventTemplate, finalize_event, generate_secret_key};
use nostr_relay::{Database, DatabaseConfig, Filter};
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

// =============================================================================
// Helper Functions
// =============================================================================

fn setup_db() -> (Database, TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("bench.db");

    let config = DatabaseConfig {
        path: db_path,
        max_reader_connections: 10,
        max_metadata_connections: 5,
    };

    let db = Database::new(config).unwrap();
    (db, temp_dir)
}

fn create_test_event(kind: u16, content: &str) -> nostr::Event {
    let secret_key = generate_secret_key();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let template = EventTemplate {
        kind,
        tags: vec![],
        content: content.to_string(),
        created_at: now,
    };

    finalize_event(&template, &secret_key).unwrap()
}

fn create_event_with_tags(kind: u16, tags: Vec<Vec<String>>) -> nostr::Event {
    let secret_key = generate_secret_key();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let template = EventTemplate {
        kind,
        tags,
        content: "Tagged event".to_string(),
        created_at: now,
    };

    finalize_event(&template, &secret_key).unwrap()
}

fn populate_db(db: &Database, count: usize) -> Vec<nostr::Event> {
    let mut events = Vec::new();

    for i in 0..count {
        let event = create_test_event(1, &format!("Event {}", i));
        db.store_event(&event).unwrap();
        events.push(event);
    }

    events
}

// =============================================================================
// Event Insertion Benchmarks
// =============================================================================

fn bench_single_event_insert(c: &mut Criterion) {
    let (db, _temp_dir) = setup_db();

    c.bench_function("insert_single_event", |b| {
        b.iter(|| {
            let event = create_test_event(1, "Benchmark event");
            let _: () = db.store_event(&event).unwrap();
            black_box(());
        });
    });
}

fn bench_batch_event_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_insert");

    for batch_size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let (db, _temp_dir) = setup_db();
                    for i in 0..size {
                        let event = create_test_event(1, &format!("Batch event {}", i));
                        let _: () = db.store_event(&event).unwrap();
                        black_box(());
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_concurrent_inserts(c: &mut Criterion) {
    let (db, _temp_dir) = setup_db();
    let db = std::sync::Arc::new(db);

    c.bench_function("concurrent_inserts_10_threads", |b| {
        b.iter(|| {
            let mut handles = vec![];

            for thread_id in 0..10 {
                let db_clone = db.clone();
                let handle = std::thread::spawn(move || {
                    for i in 0..10 {
                        let event =
                            create_test_event(1, &format!("Thread {} event {}", thread_id, i));
                        let _: () = db_clone.store_event(&event).unwrap();
                        black_box(());
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
}

// =============================================================================
// Query Benchmarks
// =============================================================================

fn bench_query_by_id(c: &mut Criterion) {
    let (db, _temp_dir) = setup_db();
    let events = populate_db(&db, 1000);
    let event_id = &events[500].id;

    c.bench_function("query_by_id", |b| {
        b.iter(|| {
            let mut filter = Filter::new();
            filter.ids = Some(vec![event_id.clone()]);
            filter.limit = Some(1);
            black_box(db.query_events(&filter).unwrap());
        });
    });
}

fn bench_query_by_kind(c: &mut Criterion) {
    let (db, _temp_dir) = setup_db();
    populate_db(&db, 1000);

    c.bench_function("query_by_kind", |b| {
        b.iter(|| {
            let mut filter = Filter::new();
            filter.kinds = Some(vec![1]);
            filter.limit = Some(100);
            black_box(db.query_events(&filter).unwrap());
        });
    });
}

fn bench_query_by_author(c: &mut Criterion) {
    let (db, _temp_dir) = setup_db();
    let events = populate_db(&db, 1000);
    let author = &events[0].pubkey;

    c.bench_function("query_by_author", |b| {
        b.iter(|| {
            let mut filter = Filter::new();
            filter.authors = Some(vec![author.clone()]);
            filter.limit = Some(100);
            black_box(db.query_events(&filter).unwrap());
        });
    });
}

fn bench_query_time_range(c: &mut Criterion) {
    let (db, _temp_dir) = setup_db();
    populate_db(&db, 1000);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    c.bench_function("query_time_range", |b| {
        b.iter(|| {
            let mut filter = Filter::new();
            filter.since = Some(now - 3600);
            filter.until = Some(now);
            filter.limit = Some(100);
            black_box(db.query_events(&filter).unwrap());
        });
    });
}

fn bench_query_with_limit(c: &mut Criterion) {
    let (db, _temp_dir) = setup_db();
    populate_db(&db, 10000);

    let mut group = c.benchmark_group("query_with_limit");

    for limit in [10, 50, 100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(limit), limit, |b, &lim| {
            b.iter(|| {
                let mut filter = Filter::new();
                filter.kinds = Some(vec![1]);
                filter.limit = Some(lim);
                black_box(db.query_events(&filter).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_query_with_tags(c: &mut Criterion) {
    let (db, _temp_dir) = setup_db();

    // Populate with tagged events
    let event_id_ref = "a".repeat(64);
    for _ in 0..1000 {
        let tags = vec![
            vec!["e".to_string(), event_id_ref.clone()],
            vec!["p".to_string(), "b".repeat(64)],
        ];
        let event = create_event_with_tags(1, tags);
        db.store_event(&event).unwrap();
    }

    c.bench_function("query_by_tag", |b| {
        b.iter(|| {
            let mut filter = Filter::new();
            let mut tags = std::collections::HashMap::new();
            tags.insert("#e".to_string(), vec![event_id_ref.clone()]);
            filter.tags = Some(tags);
            filter.limit = Some(100);
            black_box(db.query_events(&filter).unwrap());
        });
    });
}

// =============================================================================
// Filter Matching Benchmarks
// =============================================================================

fn bench_filter_match_simple(c: &mut Criterion) {
    let event = create_test_event(1, "Test event");
    let mut filter = Filter::new();
    filter.kinds = Some(vec![1]);

    c.bench_function("filter_match_simple", |b| {
        b.iter(|| {
            black_box(filter.matches(&event));
        });
    });
}

fn bench_filter_match_complex(c: &mut Criterion) {
    let tags = vec![
        vec!["e".to_string(), "a".repeat(64)],
        vec!["p".to_string(), "b".repeat(64)],
        vec!["t".to_string(), "nostr".to_string()],
    ];
    let event = create_event_with_tags(1, tags);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut filter = Filter::new();
    filter.kinds = Some(vec![1, 2, 3]);
    filter.authors = Some(vec![event.pubkey.clone()]);
    filter.since = Some(now - 3600);
    filter.until = Some(now + 3600);
    let mut tags = std::collections::HashMap::new();
    tags.insert("#e".to_string(), vec!["a".repeat(64)]);
    filter.tags = Some(tags);

    c.bench_function("filter_match_complex", |b| {
        b.iter(|| {
            black_box(filter.matches(&event));
        });
    });
}

fn bench_multiple_filter_matching(c: &mut Criterion) {
    let event = create_test_event(1, "Test event");

    let filters: Vec<Filter> = (0..10)
        .map(|i| {
            let mut filter = Filter::new();
            filter.kinds = Some(vec![i % 5]);
            filter
        })
        .collect();

    c.bench_function("match_10_filters", |b| {
        b.iter(|| {
            for filter in &filters {
                black_box(filter.matches(&event));
            }
        });
    });
}

// =============================================================================
// Database Size Scaling Benchmarks
// =============================================================================

fn bench_query_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_scaling");

    for db_size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(db_size), db_size, |b, &size| {
            let (db, _temp_dir) = setup_db();
            populate_db(&db, size);

            b.iter(|| {
                let mut filter = Filter::new();
                filter.kinds = Some(vec![1]);
                filter.limit = Some(10);
                black_box(db.query_events(&filter).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_insert_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_scaling");

    for db_size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(db_size), db_size, |b, &size| {
            b.iter_batched(
                || {
                    let (db, temp_dir) = setup_db();
                    populate_db(&db, size);
                    (db, temp_dir)
                },
                |(db, _temp_dir)| {
                    let event = create_test_event(1, "New event");
                    let _: () = db.store_event(&event).unwrap();
                    black_box(());
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// =============================================================================
// Replaceable Event Benchmarks
// =============================================================================

fn bench_replaceable_event_update(c: &mut Criterion) {
    let (db, _temp_dir) = setup_db();
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 0, // Metadata (replaceable)
        tags: vec![],
        content: "Initial metadata".to_string(),
        created_at: 1000,
    };
    let event = finalize_event(&template, &secret_key).unwrap();
    db.store_event(&event).unwrap();

    c.bench_function("update_replaceable_event", |b| {
        let mut counter = 2000u64;
        b.iter(|| {
            let template = EventTemplate {
                kind: 0,
                tags: vec![],
                content: format!("Updated metadata {}", counter),
                created_at: counter,
            };
            let event = finalize_event(&template, &secret_key).unwrap();
            let _: () = db.store_event(&event).unwrap();
            black_box(());
            counter += 1;
        });
    });
}

// =============================================================================
// Memory and Resource Benchmarks
// =============================================================================

fn bench_event_serialization(c: &mut Criterion) {
    let event = create_test_event(1, "Serialization benchmark");

    c.bench_function("event_to_json", |b| {
        b.iter(|| {
            black_box(serde_json::to_string(&event).unwrap());
        });
    });
}

fn bench_event_deserialization(c: &mut Criterion) {
    let event = create_test_event(1, "Deserialization benchmark");
    let json = serde_json::to_string(&event).unwrap();

    c.bench_function("event_from_json", |b| {
        b.iter(|| {
            black_box(serde_json::from_str::<nostr::Event>(&json).unwrap());
        });
    });
}

// =============================================================================
// Benchmark Groups
// =============================================================================

criterion_group!(
    insertion_benches,
    bench_single_event_insert,
    bench_batch_event_insert,
    bench_concurrent_inserts,
);

criterion_group!(
    query_benches,
    bench_query_by_id,
    bench_query_by_kind,
    bench_query_by_author,
    bench_query_time_range,
    bench_query_with_limit,
    bench_query_with_tags,
);

criterion_group!(
    filter_benches,
    bench_filter_match_simple,
    bench_filter_match_complex,
    bench_multiple_filter_matching,
);

criterion_group!(scaling_benches, bench_query_scaling, bench_insert_scaling,);

criterion_group!(
    special_benches,
    bench_replaceable_event_update,
    bench_event_serialization,
    bench_event_deserialization,
);

criterion_main!(
    insertion_benches,
    query_benches,
    filter_benches,
    scaling_benches,
    special_benches,
);
