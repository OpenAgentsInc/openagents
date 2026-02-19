//! Benchmarks for nostr-client relay connection and message performance
//!
//! Run with: cargo bench -p nostr-client --bench relay_performance

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use nostr::{EventTemplate, finalize_event, generate_secret_key};
use std::hint::black_box;

/// Generate a test event
fn create_test_event() -> nostr::Event {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1, // Text note
        content: "Test event for benchmarking".to_string(),
        tags: vec![],
        created_at: 1234567890,
    };
    finalize_event(&template, &secret_key).unwrap()
}

/// Generate test events in bulk
fn create_test_events(count: usize) -> Vec<nostr::Event> {
    let secret_key = generate_secret_key();
    (0..count)
        .map(|i| {
            let template = EventTemplate {
                kind: 1, // Text note
                content: format!("Test event {}", i),
                tags: vec![],
                created_at: 1234567890 + i as u64,
            };
            finalize_event(&template, &secret_key).unwrap()
        })
        .collect()
}

/// Benchmark event serialization (what gets sent to relay)
fn bench_event_serialization(c: &mut Criterion) {
    let event = create_test_event();

    c.bench_function("event_serialization", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&event)).unwrap();
            black_box(json)
        });
    });
}

/// Benchmark event deserialization (what comes from relay)
fn bench_event_deserialization(c: &mut Criterion) {
    let event = create_test_event();
    let json = serde_json::to_string(&event).unwrap();

    c.bench_function("event_deserialization", |b| {
        b.iter(|| {
            let event: nostr::Event = serde_json::from_str(black_box(&json)).unwrap();
            black_box(event)
        });
    });
}

/// Benchmark relay message serialization
fn bench_relay_message_serialization(c: &mut Criterion) {
    let event = create_test_event();

    c.bench_function("relay_message_event_serialization", |b| {
        b.iter(|| {
            // EVENT message format: ["EVENT", <subscription_id>, <event>]
            let msg = serde_json::json!(["EVENT", "test-sub", &event]);
            let json = serde_json::to_string(black_box(&msg)).unwrap();
            black_box(json)
        });
    });
}

/// Benchmark relay message deserialization
fn bench_relay_message_deserialization(c: &mut Criterion) {
    let event = create_test_event();
    let msg = serde_json::json!(["EVENT", "test-sub", &event]);
    let json = serde_json::to_string(&msg).unwrap();

    c.bench_function("relay_message_event_deserialization", |b| {
        b.iter(|| {
            let value: serde_json::Value = serde_json::from_str(black_box(&json)).unwrap();
            black_box(value)
        });
    });
}

/// Benchmark bulk event serialization
fn bench_bulk_event_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_event_serialization");

    for count in [10, 100, 1000].iter() {
        let events = create_test_events(*count);
        let bytes = events
            .iter()
            .map(|e| serde_json::to_string(e).unwrap().len())
            .sum::<usize>();
        group.throughput(Throughput::Bytes(bytes as u64));

        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                for event in &events {
                    let json = serde_json::to_string(black_box(event)).unwrap();
                    black_box(json);
                }
            });
        });
    }

    group.finish();
}

/// Benchmark bulk event deserialization
fn bench_bulk_event_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_event_deserialization");

    for count in [10, 100, 1000].iter() {
        let events = create_test_events(*count);
        let json_events: Vec<String> = events
            .iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect();
        let bytes = json_events.iter().map(|s| s.len()).sum::<usize>();
        group.throughput(Throughput::Bytes(bytes as u64));

        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                for json in &json_events {
                    let event: nostr::Event = serde_json::from_str(black_box(json)).unwrap();
                    black_box(event);
                }
            });
        });
    }

    group.finish();
}

/// Benchmark filter creation and serialization
fn bench_filter_serialization(c: &mut Criterion) {
    c.bench_function("filter_serialization", |b| {
        b.iter(|| {
            let filter = serde_json::json!({
                "kinds": [1, 6, 7],
                "authors": ["pubkey1", "pubkey2"],
                "since": 1234567890,
                "limit": 100
            });
            let json = serde_json::to_string(black_box(&filter)).unwrap();
            black_box(json)
        });
    });
}

/// Benchmark REQ message creation
fn bench_req_message_creation(c: &mut Criterion) {
    c.bench_function("req_message_creation", |b| {
        b.iter(|| {
            let filter = serde_json::json!({
                "kinds": [1],
                "limit": 100
            });
            let msg = serde_json::json!(["REQ", "sub-id", filter]);
            let json = serde_json::to_string(black_box(&msg)).unwrap();
            black_box(json)
        });
    });
}

/// Benchmark CLOSE message creation
fn bench_close_message_creation(c: &mut Criterion) {
    c.bench_function("close_message_creation", |b| {
        b.iter(|| {
            let msg = serde_json::json!(["CLOSE", "sub-id"]);
            let json = serde_json::to_string(black_box(&msg)).unwrap();
            black_box(json)
        });
    });
}

/// Benchmark event with tags serialization
fn bench_event_with_tags_serialization(c: &mut Criterion) {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1, // Text note
        content: "Test event with tags".to_string(),
        tags: vec![
            vec![
                "e".to_string(),
                "event-id-1".to_string(),
                "wss://relay1.example.com".to_string(),
            ],
            vec![
                "e".to_string(),
                "event-id-2".to_string(),
                "wss://relay2.example.com".to_string(),
                "reply".to_string(),
            ],
            vec!["p".to_string(), "pubkey-1".to_string()],
            vec!["p".to_string(), "pubkey-2".to_string()],
            vec!["t".to_string(), "bitcoin".to_string()],
            vec!["t".to_string(), "nostr".to_string()],
        ],
        created_at: 1234567890,
    };
    let event = finalize_event(&template, &secret_key).unwrap();

    c.bench_function("event_with_tags_serialization", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&event)).unwrap();
            black_box(json)
        });
    });
}

/// Benchmark event with tags deserialization
fn bench_event_with_tags_deserialization(c: &mut Criterion) {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1, // Text note
        content: "Test event with tags".to_string(),
        tags: vec![
            vec![
                "e".to_string(),
                "event-id-1".to_string(),
                "wss://relay1.example.com".to_string(),
            ],
            vec![
                "e".to_string(),
                "event-id-2".to_string(),
                "wss://relay2.example.com".to_string(),
                "reply".to_string(),
            ],
            vec!["p".to_string(), "pubkey-1".to_string()],
            vec!["p".to_string(), "pubkey-2".to_string()],
            vec!["t".to_string(), "bitcoin".to_string()],
            vec!["t".to_string(), "nostr".to_string()],
        ],
        created_at: 1234567890,
    };
    let event = finalize_event(&template, &secret_key).unwrap();
    let json = serde_json::to_string(&event).unwrap();

    c.bench_function("event_with_tags_deserialization", |b| {
        b.iter(|| {
            let event: nostr::Event = serde_json::from_str(black_box(&json)).unwrap();
            black_box(event)
        });
    });
}

/// Benchmark various message sizes
fn bench_message_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_sizes");

    for size in [100, 1000, 10000].iter() {
        let content = "a".repeat(*size);
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            kind: 1, // Text note
            content,
            tags: vec![],
            created_at: 1234567890,
        };
        let event = finalize_event(&template, &secret_key).unwrap();
        let json = serde_json::to_string(&event).unwrap();

        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let serialized = serde_json::to_string(black_box(&event)).unwrap();
                black_box(serialized)
            });
        });
    }

    group.finish();
}

/// Benchmark OK message parsing
fn bench_ok_message_parsing(c: &mut Criterion) {
    let ok_msg = r#"["OK", "event-id", true, ""]"#;

    c.bench_function("ok_message_parsing", |b| {
        b.iter(|| {
            let value: serde_json::Value = serde_json::from_str(black_box(ok_msg)).unwrap();
            black_box(value)
        });
    });
}

/// Benchmark EOSE message parsing
fn bench_eose_message_parsing(c: &mut Criterion) {
    let eose_msg = r#"["EOSE", "sub-id"]"#;

    c.bench_function("eose_message_parsing", |b| {
        b.iter(|| {
            let value: serde_json::Value = serde_json::from_str(black_box(eose_msg)).unwrap();
            black_box(value)
        });
    });
}

/// Benchmark NOTICE message parsing
fn bench_notice_message_parsing(c: &mut Criterion) {
    let notice_msg = r#"["NOTICE", "This is a notice message from the relay"]"#;

    c.bench_function("notice_message_parsing", |b| {
        b.iter(|| {
            let value: serde_json::Value = serde_json::from_str(black_box(notice_msg)).unwrap();
            black_box(value)
        });
    });
}

criterion_group!(
    benches,
    bench_event_serialization,
    bench_event_deserialization,
    bench_relay_message_serialization,
    bench_relay_message_deserialization,
    bench_bulk_event_serialization,
    bench_bulk_event_deserialization,
    bench_filter_serialization,
    bench_req_message_creation,
    bench_close_message_creation,
    bench_event_with_tags_serialization,
    bench_event_with_tags_deserialization,
    bench_message_sizes,
    bench_ok_message_parsing,
    bench_eose_message_parsing,
    bench_notice_message_parsing,
);
criterion_main!(benches);
