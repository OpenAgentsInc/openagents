//! Statistics collection for paper tables.
//!
//! Run with: cargo test -p frlm bench_stats --release -- --nocapture

use crate::policy::{BudgetPolicy, Quorum, VerificationTier};
use crate::types::{Fragment, SubQueryResult, Venue};
use crate::verification::Verifier;
use std::time::Instant;

/// Generate a successful result
fn make_result(query_id: &str, content: &str, duration_ms: u64, cost_sats: u64) -> SubQueryResult {
    SubQueryResult::success(query_id, content, Venue::Swarm, duration_ms).with_cost(cost_sats)
}

/// Generate N agreeing results
fn make_agreeing_results(count: usize, content: &str) -> Vec<SubQueryResult> {
    (0..count)
        .map(|i| make_result(&format!("q-{}", i), content, 100, 10))
        .collect()
}

/// Generate results with a percentage of bad results
fn make_mixed_results(total: usize, bad_fraction: f32, good_content: &str) -> Vec<SubQueryResult> {
    let bad_count = (total as f32 * bad_fraction).ceil() as usize;
    let good_count = total - bad_count;

    let mut results = Vec::with_capacity(total);

    for i in 0..good_count {
        results.push(make_result(&format!("q-{}", i), good_content, 100, 10));
    }

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

/// Collect p50 and p95 latency in microseconds
fn measure_latency<F: Fn()>(iterations: usize, f: F) -> (u64, u64) {
    let mut times: Vec<u128> = (0..iterations)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed().as_nanos()
        })
        .collect();
    times.sort();
    let p50 = (times[iterations / 2] / 1000) as u64;
    let p95 = (times[iterations * 95 / 100] / 1000) as u64;
    (p50, p95)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_paper_statistics() {
        println!("\n");
        println!("================================================================================");
        println!("                    FRLM BENCHMARK STATISTICS FOR PAPER                         ");
        println!("================================================================================");

        // =========================================================================
        // Table 1: End-to-end Latency
        // =========================================================================
        println!("\n## Table 1: End-to-end Latency vs Baseline\n");
        println!("| Fanout | Verification | p50 (µs) | p95 (µs) |");
        println!("|--------|--------------|----------|----------|");

        // None tier across fanout sizes
        for fanout in [5, 10, 20, 50] {
            let results = make_agreeing_results(fanout, "the consensus answer is 42");
            let tier = VerificationTier::None;

            let (p50, p95) = measure_latency(1000, || {
                let _ = Verifier::verify(&results, &tier);
            });
            println!("| {:>6} | None         | {:>8} | {:>8} |", fanout, p50, p95);
        }

        // Redundancy tiers
        for (n, m, name) in [(3, 2, "2-of-3"), (5, 3, "3-of-5"), (10, 6, "6-of-10")] {
            let results = make_agreeing_results(n, "the consensus answer is 42");
            let tier = VerificationTier::redundancy(n, m);

            let (p50, p95) = measure_latency(1000, || {
                let _ = Verifier::verify(&results, &tier);
            });
            println!("| {:>6} | {:12} | {:>8} | {:>8} |", n, name, p50, p95);
        }

        // Objective tier
        let json_results: Vec<_> = (0..10)
            .map(|i| make_result(&format!("q-{}", i), r#"{"name": "Alice", "age": 30}"#, 100, 10))
            .collect();
        let tier = VerificationTier::objective(Some(r#"{"type": "object", "required": ["name"]}"#.to_string()));
        let (p50, p95) = measure_latency(1000, || {
            let _ = Verifier::verify(&json_results, &tier);
        });
        println!("| {:>6} | Objective    | {:>8} | {:>8} |", 10, p50, p95);

        // =========================================================================
        // Table 2: Cost per Solved Task
        // =========================================================================
        println!("\n## Table 2: Cost per Solved Task\n");
        println!("| Fragment Size | Fanout | Est. Cost (sats) | Sats/Result |");
        println!("|---------------|--------|------------------|-------------|");

        let policy = BudgetPolicy::default();
        for size in [100, 1000, 10000] {
            for fanout in [10, 20, 50] {
                let total_cost: u64 = (0..fanout).map(|_| policy.estimate_cost(size)).sum();
                let per_result = total_cost / fanout as u64;
                let size_str = if size >= 1000 {
                    format!("{}KB", size / 1000)
                } else {
                    format!("{}B", size)
                };
                println!(
                    "| {:>13} | {:>6} | {:>16} | {:>11} |",
                    size_str, fanout, total_cost, per_result
                );
            }
        }

        // =========================================================================
        // Table 3: Task Success Rate
        // =========================================================================
        println!("\n## Table 3: Task Success Rate\n");
        println!("| Workers | Success% | Quorum Policy | Met? |");
        println!("|---------|----------|---------------|------|");

        for (received, total, pct) in [(8, 10, 80), (9, 10, 90), (10, 10, 100), (40, 50, 80), (48, 50, 96)] {
            let q_all = Quorum::All;
            let q_frac = Quorum::Fraction(0.8);
            let min_needed = (total as f32 * 0.8).ceil() as usize;
            let q_min = Quorum::MinCount(min_needed);

            println!(
                "| {:>2}/{:<2} | {:>7}% | All           | {:>4} |",
                received, total, pct,
                if q_all.is_met(received, total) { "Yes" } else { "No" }
            );
            println!(
                "| {:>2}/{:<2} | {:>7}% | Fraction(0.8) | {:>4} |",
                received, total, pct,
                if q_frac.is_met(received, total) { "Yes" } else { "No" }
            );
            println!(
                "| {:>2}/{:<2} | {:>7}% | MinCount({})  | {:>4} |",
                received, total, pct, min_needed,
                if q_min.is_met(received, total) { "Yes" } else { "No" }
            );
        }

        // =========================================================================
        // Table 4: Fraud/Low-Quality Detection Rate
        // =========================================================================
        println!("\n## Table 4: Fraud/Low-Quality Detection Rate\n");
        println!("| Bad Results | Verification | Detection | Agreement |");
        println!("|-------------|--------------|-----------|-----------|");

        let good_content = "the correct consensus answer is forty-two which is the meaning of life";
        for bad_pct in [0, 5, 10, 20, 30, 40] {
            let bad_fraction = bad_pct as f32 / 100.0;
            let results = make_mixed_results(10, bad_fraction, good_content);
            let tier = VerificationTier::redundancy(10, 6);

            let verify_result = Verifier::verify(&results, &tier).unwrap();
            let detection = if verify_result.passed { "Accepted" } else { "Rejected" };
            let agreement = verify_result
                .agreement
                .map(|a| format!("{:.0}%", a * 100.0))
                .unwrap_or_else(|| "N/A".to_string());

            println!(
                "| {:>10}% | 6-of-10      | {:>9} | {:>9} |",
                bad_pct, detection, agreement
            );
        }

        println!("\n================================================================================");
        println!("                              END STATISTICS                                    ");
        println!("================================================================================\n");
    }
}
