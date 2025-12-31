//! Benchmarks for NIP-01 validation functions

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use nostr::{EventTemplate, finalize_event, generate_secret_key};
use nostr_relay::{
    Filter, validate_close_message, validate_event_message, validate_event_structure,
    validate_filter, validate_req_message, validate_subscription_id,
};
use std::hint::black_box;
use std::time::{SystemTime, UNIX_EPOCH};

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn create_test_event() -> nostr::Event {
    let secret_key = generate_secret_key();
    let template = EventTemplate {
        kind: 1,
        tags: vec![],
        content: "test".to_string(),
        created_at: current_timestamp(),
    };
    finalize_event(&template, &secret_key).unwrap()
}

fn bench_validate_event_structure(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_event_structure");

    // Benchmark valid event with minimal content
    let minimal_event = create_test_event();
    group.bench_function("minimal_event", |b| {
        b.iter(|| validate_event_structure(black_box(&minimal_event)))
    });

    // Benchmark event with many tags
    let secret_key = generate_secret_key();
    for num_tags in [10, 100, 500, 1000] {
        let template = EventTemplate {
            kind: 1,
            tags: vec![vec!["t".to_string(), "test".to_string()]; num_tags],
            content: "test".to_string(),
            created_at: current_timestamp(),
        };
        let event = finalize_event(&template, &secret_key).unwrap();
        group.bench_with_input(
            BenchmarkId::new("many_tags", num_tags),
            &event,
            |b, event| b.iter(|| validate_event_structure(black_box(event))),
        );
    }

    // Benchmark event with long content
    for content_len in [1000, 10000, 32768, 65536] {
        let template = EventTemplate {
            kind: 1,
            tags: vec![],
            content: "a".repeat(content_len),
            created_at: current_timestamp(),
        };
        let event = finalize_event(&template, &secret_key).unwrap();
        group.throughput(Throughput::Bytes(content_len as u64));
        group.bench_with_input(
            BenchmarkId::new("long_content", content_len),
            &event,
            |b, event| b.iter(|| validate_event_structure(black_box(event))),
        );
    }

    group.finish();
}

fn bench_validate_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_filter");

    // Benchmark empty filter
    let empty_filter = Filter::new();
    group.bench_function("empty", |b| {
        b.iter(|| validate_filter(black_box(&empty_filter)))
    });

    // Benchmark filter with single id
    let mut filter_with_id = Filter::new();
    filter_with_id.ids = Some(vec!["abcdef0123456789".to_string()]);
    group.bench_function("single_id", |b| {
        b.iter(|| validate_filter(black_box(&filter_with_id)))
    });

    // Benchmark filter with many ids
    for num_ids in [10, 50, 100] {
        let mut filter = Filter::new();
        filter.ids = Some(vec!["abcdef0123456789".to_string(); num_ids]);
        group.bench_with_input(
            BenchmarkId::new("many_ids", num_ids),
            &filter,
            |b, filter| b.iter(|| validate_filter(black_box(filter))),
        );
    }

    // Benchmark filter with authors
    let mut filter_with_authors = Filter::new();
    filter_with_authors.authors = Some(vec!["abcdef0123456789".to_string()]);
    group.bench_function("single_author", |b| {
        b.iter(|| validate_filter(black_box(&filter_with_authors)))
    });

    // Benchmark filter with timestamp range
    let mut filter_with_time = Filter::new();
    let now = current_timestamp();
    filter_with_time.since = Some(now - 86400);
    filter_with_time.until = Some(now);
    group.bench_function("time_range", |b| {
        b.iter(|| validate_filter(black_box(&filter_with_time)))
    });

    // Benchmark complex filter
    let mut complex_filter = Filter::new();
    complex_filter.ids = Some(vec!["abcdef0123456789".to_string(); 10]);
    complex_filter.authors = Some(vec!["abcdef0123456789".to_string(); 5]);
    complex_filter.kinds = Some(vec![1, 2, 3, 4, 5]);
    complex_filter.since = Some(now - 86400);
    complex_filter.until = Some(now);
    complex_filter.limit = Some(100);
    group.bench_function("complex", |b| {
        b.iter(|| validate_filter(black_box(&complex_filter)))
    });

    group.finish();
}

fn bench_validate_subscription_id(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_subscription_id");

    // Benchmark short subscription IDs
    for len in [4, 8, 16, 32, 64] {
        let sub_id = "a".repeat(len);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::new("length", len), &sub_id, |b, sub_id| {
            b.iter(|| validate_subscription_id(black_box(sub_id)))
        });
    }

    // Benchmark alphanumeric subscription ID
    let alphanumeric = "sub-123-abc-xyz";
    group.bench_function("alphanumeric", |b| {
        b.iter(|| validate_subscription_id(black_box(alphanumeric)))
    });

    group.finish();
}

fn bench_validate_event_message(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_event_message");

    // Create valid EVENT message
    let event = create_test_event();
    let msg = serde_json::json!(["EVENT", event]);

    group.bench_function("valid", |b| {
        b.iter(|| validate_event_message(black_box(&msg)))
    });

    // Benchmark invalid message (wrong structure)
    let invalid_msg = serde_json::json!(["EVENT"]);
    group.bench_function("invalid_structure", |b| {
        b.iter(|| validate_event_message(black_box(&invalid_msg)))
    });

    group.finish();
}

fn bench_validate_req_message(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_req_message");

    // Simple REQ message
    let simple_req = serde_json::json!(["REQ", "sub-123", {"kinds": [1]}]);
    group.bench_function("simple", |b| {
        b.iter(|| validate_req_message(black_box(&simple_req)))
    });

    // REQ with multiple filters
    let multi_filter_req = serde_json::json!([
        "REQ",
        "sub-456",
        {"kinds": [1], "limit": 10},
        {"kinds": [3], "authors": ["abc123"]},
        {"kinds": [7], "since": 1000000}
    ]);
    group.bench_function("multiple_filters", |b| {
        b.iter(|| validate_req_message(black_box(&multi_filter_req)))
    });

    // REQ with complex filter
    let complex_req = serde_json::json!([
        "REQ",
        "sub-789",
        {
            "ids": ["abcdef0123456789"],
            "authors": ["abcdef0123456789", "fedcba9876543210"],
            "kinds": [1, 2, 3, 4, 5],
            "since": 1000000,
            "until": 2000000,
            "limit": 100
        }
    ]);
    group.bench_function("complex_filter", |b| {
        b.iter(|| validate_req_message(black_box(&complex_req)))
    });

    group.finish();
}

fn bench_validate_close_message(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_close_message");

    let close_msg = serde_json::json!(["CLOSE", "sub-123"]);
    group.bench_function("valid", |b| {
        b.iter(|| validate_close_message(black_box(&close_msg)))
    });

    let invalid_msg = serde_json::json!(["CLOSE"]);
    group.bench_function("invalid", |b| {
        b.iter(|| validate_close_message(black_box(&invalid_msg)))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_validate_event_structure,
    bench_validate_filter,
    bench_validate_subscription_id,
    bench_validate_event_message,
    bench_validate_req_message,
    bench_validate_close_message
);
criterion_main!(benches);
