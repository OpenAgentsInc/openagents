//! FRLM Benchmarks for Paper Data
//!
//! These benchmarks generate data for Tables 1-4 in the FRLM paper.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use frlm::policy::{BudgetPolicy, Quorum, VerificationTier};
use frlm::types::{Fragment, SubQueryResult, Venue};
use frlm::verification::Verifier;
use rand::Rng;
use std::time::Instant;

// ============================================================================
// Test Fixtures
// ============================================================================

/// Generate a fragment of specified size
fn make_fragment(id: usize, size_bytes: usize) -> Fragment {
    let content = "a".repeat(size_bytes);
    Fragment::new(format!("frag-{}", id), content)
}

/// Generate N fragments of specified size
fn make_fragments(count: usize, size_bytes: usize) -> Vec<Fragment> {
    (0..count).map(|i| make_fragment(i, size_bytes)).collect()
}

/// Generate a successful result with specified content
fn make_result(query_id: &str, content: &str, duration_ms: u64, cost_sats: u64) -> SubQueryResult {
    SubQueryResult::success(query_id, content, Venue::Swarm, duration_ms).with_cost(cost_sats)
}

/// Generate N agreeing results (for redundancy tests)
fn make_agreeing_results(count: usize, content: &str) -> Vec<SubQueryResult> {
    (0..count)
        .map(|i| make_result(&format!("q-{}", i), content, 100, 10))
        .collect()
}

/// Generate N disagreeing results (for detection tests)
fn make_disagreeing_results(count: usize) -> Vec<SubQueryResult> {
    let answers = [
        "the sky is blue",
        "water is wet",
        "fire is hot",
        "ice is cold",
        "grass is green",
        "snow is white",
        "coal is black",
        "gold is yellow",
        "silver is gray",
        "copper is orange",
    ];
    (0..count)
        .map(|i| make_result(&format!("q-{}", i), answers[i % answers.len()], 100, 10))
        .collect()
}

/// Generate results with a percentage of bad/adversarial results
fn make_mixed_results(total: usize, bad_fraction: f32, good_content: &str) -> Vec<SubQueryResult> {
    let bad_count = (total as f32 * bad_fraction).ceil() as usize;
    let good_count = total - bad_count;

    let mut results = Vec::with_capacity(total);

    // Good results (agreeing)
    for i in 0..good_count {
        results.push(make_result(&format!("q-{}", i), good_content, 100, 10));
    }

    // Bad results (garbage/adversarial)
    let garbage = ["GARBAGE", "INVALID", "ATTACK", "SPAM", "NOISE"];
    for i in 0..bad_count {
        results.push(make_result(
            &format!("q-bad-{}", i),
            garbage[i % garbage.len()],
            50,
            5,
        ));
    }

    results
}

// ============================================================================
// Table 1: Latency Benchmarks
// ============================================================================

/// Benchmark verification latency across different fanout sizes
fn bench_latency_by_fanout(c: &mut Criterion) {
    let mut group = c.benchmark_group("table1_latency_fanout");

    for fanout in [5, 10, 20, 50] {
        let results = make_agreeing_results(fanout, "the consensus answer is 42");
        let tier = VerificationTier::None;

        group.bench_with_input(BenchmarkId::new("none", fanout), &fanout, |b, _| {
            b.iter(|| Verifier::verify(black_box(&results), black_box(&tier)))
        });
    }

    group.finish();
}

/// Benchmark verification latency across different verification tiers
fn bench_latency_by_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("table1_latency_verification");

    let results = make_agreeing_results(10, "the consensus answer is 42");

    // None tier
    group.bench_function("none", |b| {
        let tier = VerificationTier::None;
        b.iter(|| Verifier::verify(black_box(&results), black_box(&tier)))
    });

    // 2-of-3 redundancy
    let results_3 = make_agreeing_results(3, "the consensus answer is 42");
    group.bench_function("redundancy_2of3", |b| {
        let tier = VerificationTier::redundancy_2_of_3();
        b.iter(|| Verifier::verify(black_box(&results_3), black_box(&tier)))
    });

    // 3-of-5 redundancy
    let results_5 = make_agreeing_results(5, "the consensus answer is 42");
    group.bench_function("redundancy_3of5", |b| {
        let tier = VerificationTier::redundancy_3_of_5();
        b.iter(|| Verifier::verify(black_box(&results_5), black_box(&tier)))
    });

    // Objective (schema validation)
    let json_results: Vec<_> = (0..10)
        .map(|i| make_result(&format!("q-{}", i), r#"{"name": "Alice", "age": 30}"#, 100, 10))
        .collect();
    group.bench_function("objective_schema", |b| {
        let tier =
            VerificationTier::objective(Some(r#"{"type": "object", "required": ["name"]}"#.to_string()));
        b.iter(|| Verifier::verify(black_box(&json_results), black_box(&tier)))
    });

    group.finish();
}

// ============================================================================
// Table 2: Cost Benchmarks
// ============================================================================

/// Benchmark budget estimation across different fragment sizes
fn bench_cost_estimation(c: &mut Criterion) {
    let mut group = c.benchmark_group("table2_cost");

    let policy = BudgetPolicy::default();

    for size in [100, 1000, 10000] {
        group.bench_with_input(BenchmarkId::new("estimate", size), &size, |b, &size| {
            b.iter(|| policy.estimate_cost(black_box(size)))
        });
    }

    group.finish();
}

/// Measure cost distribution across fragment sizes
fn bench_cost_per_fragment_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("table2_cost_per_size");

    for size in [100, 1000, 10000] {
        let fragments = make_fragments(10, size);

        group.bench_with_input(BenchmarkId::new("fragments", size), &size, |b, _| {
            let policy = BudgetPolicy::default();
            b.iter(|| {
                let total: u64 = fragments
                    .iter()
                    .map(|f| policy.estimate_cost(black_box(f.size_bytes())))
                    .sum();
                total
            })
        });
    }

    group.finish();
}

// ============================================================================
// Table 3: Success Rate Benchmarks
// ============================================================================

/// Benchmark quorum checking performance
fn bench_quorum_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("table3_quorum");

    for (received, total) in [(8, 10), (9, 10), (10, 10), (45, 50), (50, 50)] {
        let label = format!("{}_of_{}", received, total);

        // All quorum
        group.bench_with_input(BenchmarkId::new("all", &label), &(received, total), |b, &(r, t)| {
            let q = Quorum::All;
            b.iter(|| q.is_met(black_box(r), black_box(t)))
        });

        // Fraction quorum (80%)
        group.bench_with_input(
            BenchmarkId::new("fraction_80", &label),
            &(received, total),
            |b, &(r, t)| {
                let q = Quorum::Fraction(0.8);
                b.iter(|| q.is_met(black_box(r), black_box(t)))
            },
        );

        // MinCount quorum
        group.bench_with_input(
            BenchmarkId::new("min_count", &label),
            &(received, total),
            |b, &(r, t)| {
                let q = Quorum::MinCount(t * 8 / 10); // 80% as min count
                b.iter(|| q.is_met(black_box(r), black_box(t)))
            },
        );
    }

    group.finish();
}

// ============================================================================
// Table 4: Detection Rate Benchmarks
// ============================================================================

/// Benchmark adversarial detection under redundancy verification
fn bench_detection_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("table4_detection");

    let good_content = "the correct consensus answer is forty-two";

    // Test with different bad result fractions
    for bad_pct in [1, 5, 10, 20] {
        let bad_fraction = bad_pct as f32 / 100.0;
        let results = make_mixed_results(10, bad_fraction, good_content);

        group.bench_with_input(
            BenchmarkId::new("redundancy_2of3", bad_pct),
            &bad_pct,
            |b, _| {
                let tier = VerificationTier::redundancy(10, 6); // 6 of 10 needed
                b.iter(|| Verifier::verify(black_box(&results), black_box(&tier)))
            },
        );
    }

    group.finish();
}

/// Benchmark similarity calculation (core of redundancy verification)
fn bench_similarity(c: &mut Criterion) {
    let mut group = c.benchmark_group("table4_similarity");

    // Short strings (character-based)
    let short_a = "the answer is 42";
    let short_b = "the answer is 43";
    group.bench_function("short_strings", |b| {
        b.iter(|| {
            // Access through verification - similarity is private
            let results = vec![
                make_result("q-1", short_a, 100, 10),
                make_result("q-2", short_b, 100, 10),
            ];
            let tier = VerificationTier::redundancy(2, 2);
            Verifier::verify(black_box(&results), black_box(&tier))
        })
    });

    // Long strings (word-based Jaccard)
    let long_a = "the ".repeat(500) + "answer is forty two";
    let long_b = "the ".repeat(500) + "answer is forty three";
    group.bench_function("long_strings", |b| {
        b.iter(|| {
            let results = vec![
                make_result("q-1", &long_a, 100, 10),
                make_result("q-2", &long_b, 100, 10),
            ];
            let tier = VerificationTier::redundancy(2, 2);
            Verifier::verify(black_box(&results), black_box(&tier))
        })
    });

    group.finish();
}

// ============================================================================
// Aggregate Statistics Collection
// ============================================================================

/// Run multiple iterations and collect statistics for paper tables
fn collect_statistics() {
    println!("\n=== FRLM Benchmark Statistics for Paper ===\n");

    // Table 1: Latency
    println!("## Table 1: End-to-end Latency\n");
    println!("| Fanout | Verification | p50 (µs) | p95 (µs) |");
    println!("|--------|--------------|----------|----------|");

    for fanout in [5, 10, 20, 50] {
        let results = make_agreeing_results(fanout, "the consensus answer is 42");

        // None tier
        let mut times: Vec<u128> = (0..1000)
            .map(|_| {
                let start = Instant::now();
                let _ = Verifier::verify(&results, &VerificationTier::None);
                start.elapsed().as_nanos()
            })
            .collect();
        times.sort();
        let p50 = times[500] / 1000;
        let p95 = times[950] / 1000;
        println!("| {} | None | {} | {} |", fanout, p50, p95);
    }

    // Redundancy tiers
    for (n, m, name) in [(3, 2, "2-of-3"), (5, 3, "3-of-5")] {
        let results = make_agreeing_results(n, "the consensus answer is 42");
        let tier = VerificationTier::redundancy(n, m);

        let mut times: Vec<u128> = (0..1000)
            .map(|_| {
                let start = Instant::now();
                let _ = Verifier::verify(&results, &tier);
                start.elapsed().as_nanos()
            })
            .collect();
        times.sort();
        let p50 = times[500] / 1000;
        let p95 = times[950] / 1000;
        println!("| {} | {} | {} | {} |", n, name, p50, p95);
    }

    // Table 2: Cost
    println!("\n## Table 2: Cost per Task\n");
    println!("| Fragment Size | Fanout | Est. Cost (sats) | Sats/Result |");
    println!("|---------------|--------|------------------|-------------|");

    let policy = BudgetPolicy::default();
    for size in [100, 1000, 10000] {
        for fanout in [10, 20] {
            let total_cost: u64 = (0..fanout).map(|_| policy.estimate_cost(size)).sum();
            let per_result = total_cost / fanout as u64;
            println!(
                "| {} | {} | {} | {} |",
                size, fanout, total_cost, per_result
            );
        }
    }

    // Table 3: Success Rate
    println!("\n## Table 3: Success Rate\n");
    println!("| Quorum Policy | Received | Total | Success |");
    println!("|---------------|----------|-------|---------|");

    for (received, total) in [(8, 10), (9, 10), (10, 10)] {
        let q_all = Quorum::All;
        let q_frac = Quorum::Fraction(0.8);
        let q_min = Quorum::MinCount(8);

        println!(
            "| All | {} | {} | {} |",
            received,
            total,
            if q_all.is_met(received, total) {
                "Yes"
            } else {
                "No"
            }
        );
        println!(
            "| Fraction(0.8) | {} | {} | {} |",
            received,
            total,
            if q_frac.is_met(received, total) {
                "Yes"
            } else {
                "No"
            }
        );
        println!(
            "| MinCount(8) | {} | {} | {} |",
            received,
            total,
            if q_min.is_met(received, total) {
                "Yes"
            } else {
                "No"
            }
        );
    }

    // Table 4: Detection Rate
    println!("\n## Table 4: Detection Rate\n");
    println!("| Bad % | Tier | Detection | Agreement |");
    println!("|-------|------|-----------|-----------|");

    let good_content = "the correct consensus answer is forty-two";
    for bad_pct in [0, 5, 10, 20, 30] {
        let bad_fraction = bad_pct as f32 / 100.0;
        let results = make_mixed_results(10, bad_fraction, good_content);
        let tier = VerificationTier::redundancy(10, 6);

        let verify_result = Verifier::verify(&results, &tier).unwrap();
        let detection = if verify_result.passed { "Passed" } else { "Blocked" };
        let agreement = verify_result
            .agreement
            .map(|a| format!("{:.0}%", a * 100.0))
            .unwrap_or_else(|| "N/A".to_string());

        println!("| {}% | 6-of-10 | {} | {} |", bad_pct, detection, agreement);
    }

    println!("\n=== End Statistics ===\n");
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    table1,
    bench_latency_by_fanout,
    bench_latency_by_verification
);
criterion_group!(table2, bench_cost_estimation, bench_cost_per_fragment_size);
criterion_group!(table3, bench_quorum_check);
criterion_group!(table4, bench_detection_rate, bench_similarity);

criterion_main!(table1, table2, table3, table4);

// Run statistics collection when called directly
#[cfg(test)]
mod stats {
    use super::*;

    #[test]
    fn print_paper_statistics() {
        collect_statistics();
    }
}
