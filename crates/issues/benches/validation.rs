//! Benchmarks for input validation functions

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use issues::validation::{validate_agent, validate_description, validate_title};
use std::hint::black_box;

fn bench_validate_title(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_title");

    // Benchmark valid titles of different lengths
    for len in [10, 50, 100, 200] {
        let title = "a".repeat(len);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::new("valid", len), &title, |b, title| {
            b.iter(|| validate_title(black_box(title)))
        });
    }

    // Benchmark titles with leading whitespace (rejected)
    let title_with_whitespace = "  Valid title";
    group.bench_function("leading_whitespace", |b| {
        b.iter(|| validate_title(black_box(title_with_whitespace)))
    });

    // Benchmark titles with trailing whitespace (rejected)
    let title_with_trailing = "Valid title  ";
    group.bench_function("trailing_whitespace", |b| {
        b.iter(|| validate_title(black_box(title_with_trailing)))
    });

    // Benchmark empty title (rejected)
    group.bench_function("empty", |b| b.iter(|| validate_title(black_box(""))));

    // Benchmark too long title (rejected)
    let too_long = "a".repeat(201);
    group.bench_function("too_long", |b| {
        b.iter(|| validate_title(black_box(&too_long)))
    });

    // Benchmark unicode title
    let unicode_title = "修复认证错误🔧";
    group.bench_function("unicode", |b| {
        b.iter(|| validate_title(black_box(unicode_title)))
    });

    group.finish();
}

fn bench_validate_description(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_description");

    // Benchmark None description
    group.bench_function("none", |b| b.iter(|| validate_description(black_box(None))));

    // Benchmark valid descriptions of different lengths
    for len in [100, 1000, 5000, 10000] {
        let desc = "a".repeat(len);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::new("valid", len), &desc, |b, desc| {
            b.iter(|| validate_description(black_box(Some(desc.as_str()))))
        });
    }

    // Benchmark too long description (rejected)
    let too_long = "a".repeat(10001);
    group.throughput(Throughput::Bytes(10001));
    group.bench_function("too_long", |b| {
        b.iter(|| validate_description(black_box(Some(too_long.as_str()))))
    });

    // Benchmark description with newlines
    let with_newlines = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
    group.bench_function("with_newlines", |b| {
        b.iter(|| validate_description(black_box(Some(with_newlines))))
    });

    group.finish();
}

fn bench_validate_agent(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_agent");

    // Benchmark valid agents
    group.bench_function("codex", |b| b.iter(|| validate_agent(black_box("codex"))));

    group.bench_function("codex", |b| b.iter(|| validate_agent(black_box("codex"))));

    // Benchmark invalid agents
    group.bench_function("invalid_gpt4", |b| {
        b.iter(|| validate_agent(black_box("gpt4")))
    });

    group.bench_function("invalid_empty", |b| {
        b.iter(|| validate_agent(black_box("")))
    });

    group.bench_function("invalid_case", |b| {
        b.iter(|| validate_agent(black_box("Codex")))
    });

    group.finish();
}

fn bench_combined_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("combined_validation");

    // Simulate validating a full issue creation
    group.bench_function("typical_issue", |b| {
        b.iter(|| {
            let title = validate_title(black_box("Fix authentication bug"));
            let description = validate_description(black_box(Some(
                "This issue tracks the work to fix authentication problems in the login flow.",
            )));
            let agent = validate_agent(black_box("codex"));
            (title, description, agent)
        })
    });

    // Simulate validating issue with max-length fields
    let long_title = "a".repeat(200);
    let long_description = "a".repeat(10000);
    group.bench_function("max_length_issue", |b| {
        b.iter(|| {
            let title = validate_title(black_box(&long_title));
            let description = validate_description(black_box(Some(&long_description)));
            let agent = validate_agent(black_box("codex"));
            (title, description, agent)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_validate_title,
    bench_validate_description,
    bench_validate_agent,
    bench_combined_validation
);
criterion_main!(benches);
