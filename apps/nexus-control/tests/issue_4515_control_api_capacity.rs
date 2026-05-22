//! Local reproduction for issue #4515 — Nexus control-API capacity exhaustion.
//!
//! The relay returns `503 embedded Nexus control API capacity exhausted` once
//! its 256-permit authority-slot budget fills. That is a symptom. The cause is
//! in `nexus-control`: every mutating handler serializes on one
//! `RwLock<ControlStore>`, and the admission path, under that lock, persists
//! the *entire* kernel state (`persist_compute_authority_state`: clone ~30
//! collections, `serde_json::to_vec_pretty`, `fs::write` + `fs::rename`) and
//! recomputes a snapshot over *all* receipts. Per-mutation cost is therefore
//! O(accumulated state), and because the lock is a blocking `std::sync::RwLock`
//! on a 4-worker runtime, concurrent mutations fully serialize.
//!
//! This test drives admissions through the real `nexus-control` router and
//! prints two measurements:
//!   Phase A — per-admission latency as kernel state accumulates,
//!   Phase B — throughput under 4-way concurrency vs. sequential.
//!
//! It is `#[ignore]`d because it is a measurement reproduction, not a fast
//! pass/fail unit test. Run it explicitly:
//!
//! ```text
//! cargo test -p nexus-control --test issue_4515_control_api_capacity \
//!     -- --ignored --nocapture
//! ```
//!
//! `ISSUE_4515_ADMISSIONS` overrides the admission count (default 800).

// This is a measurement reproduction: printing a report to stdout and using
// test assertions in a `Result`-returning test are intentional here, though
// the workspace lints restrict both in normal code.
#![allow(clippy::print_stdout, clippy::panic_in_result_fn)]

use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

type TestError = Box<dyn std::error::Error + Send + Sync>;
type TestResult<T = ()> = Result<T, TestError>;

const ADMISSION_PATH: &str = "/api/training/nodes/admission";

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

/// Build a `nexus-control` router backed by a real on-disk kernel state file
/// under `state_dir`, with treasury and auto-dispatch disabled.
fn build_test_router(state_dir: &Path) -> TestResult<Router> {
    let mut config = nexus_control::ServiceConfig::from_env()?;
    config.kernel_state_path = Some(state_dir.join("kernel-state.json"));
    config.receipt_log_path = Some(state_dir.join("receipt-log.jsonl"));
    config.training_trn_identity_path = state_dir.join("trn-identity.mnemonic");
    config.treasury.enabled = false;
    config.cs336_homework_auto_dispatch_enabled = false;
    Ok(nexus_control::build_router(config))
}

/// Send one training-node admission; return its HTTP status and latency.
///
/// The request omits a capability envelope, so the kernel records the
/// admission and then refuses it (`training_node_capability_missing`). The
/// persistence path under test is identical either way: a receipt is written,
/// a snapshot is recomputed over every receipt, and the full kernel state is
/// rewritten to disk.
async fn admit(app: &Router, tag: &str, idx: usize) -> TestResult<(StatusCode, Duration)> {
    let body = serde_json::json!({
        "idempotency_key": format!("issue4515-{tag}-{idx:06}"),
        "requested_at_ms": now_unix_ms(),
        "node_pubkey_hex": format!("issue4515-{tag}-node-{idx:06}"),
        "release_id": "openagents.pylon@issue4515",
        "build_digest": "issue4515-build-digest",
        "role_claims": ["worker"],
        "node_label": format!("issue #4515 reproduction node {idx}"),
        "allowed_networks": ["mainnet"],
    });
    let request = Request::builder()
        .method("POST")
        .uri(ADMISSION_PATH)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body)?))?;
    let started = Instant::now();
    let response = app.clone().oneshot(request).await?;
    let elapsed = started.elapsed();
    let status = response.status();
    to_bytes(response.into_body(), usize::MAX).await?;
    Ok((status, elapsed))
}

fn avg(samples: &[Duration]) -> Duration {
    if samples.is_empty() {
        return Duration::ZERO;
    }
    samples.iter().sum::<Duration>() / samples.len() as u32
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "issue #4515 measurement reproduction; run with --ignored --nocapture"]
async fn issue_4515_control_api_mutation_latency_scales_with_state() -> TestResult {
    let total: usize = std::env::var("ISSUE_4515_ADMISSIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(800);
    let window = (total / 10).max(1);
    let burst = (window * 2).max(2);

    let state_dir = tempfile::tempdir()?;
    let app = build_test_router(state_dir.path())?;

    println!();
    println!("issue #4515 reproduction — nexus-control mutation latency vs accumulated state");
    println!("  worker threads: 4 (matches embedded control API default)");
    println!("  admissions:     {total} (override with ISSUE_4515_ADMISSIONS)");
    println!();

    // ---- Phase A: per-admission latency as kernel state accumulates ----
    println!("Phase A — sequential admissions, latency per {window}-admission window:");
    println!(
        "  {:>9}  {:>13}  {:>13}  {:>13}",
        "admitted", "avg", "min", "max"
    );
    let mut window_samples: Vec<Duration> = Vec::with_capacity(window);
    let mut window_avgs: Vec<Duration> = Vec::new();
    for idx in 0..total {
        let (status, elapsed) = admit(&app, "seq", idx).await?;
        assert_eq!(status, StatusCode::OK, "admission {idx} returned {status}");
        window_samples.push(elapsed);
        if window_samples.len() == window {
            let a = avg(&window_samples);
            let mn = window_samples.iter().copied().min().unwrap_or_default();
            let mx = window_samples.iter().copied().max().unwrap_or_default();
            println!("  {:>9}  {a:>13?}  {mn:>13?}  {mx:>13?}", idx + 1);
            window_avgs.push(a);
            window_samples.clear();
        }
    }
    let first_window_avg = window_avgs.first().copied().unwrap_or_default();
    let last_window_avg = window_avgs.last().copied().unwrap_or_default();
    let growth =
        last_window_avg.as_secs_f64() / first_window_avg.as_secs_f64().max(f64::MIN_POSITIVE);
    let state_bytes = std::fs::metadata(state_dir.path().join("kernel-state.json"))
        .map(|meta| meta.len())
        .unwrap_or_default();
    println!();
    println!("  first window avg: {first_window_avg:?}");
    println!("  last  window avg: {last_window_avg:?}");
    println!("  growth factor:    {growth:.1}x");
    println!("  kernel-state.json after {total} admissions: {state_bytes} bytes");
    println!();

    // ---- Phase B: 4-way concurrency yields no throughput gain because the
    //      single RwLock serializes every mutation ----
    println!("Phase B — sequential vs 4-way concurrent throughput at depth {total}:");
    let seq_started = Instant::now();
    for idx in 0..burst {
        let (status, _) = admit(&app, "seqB", idx).await?;
        assert_eq!(status, StatusCode::OK);
    }
    let seq_total = seq_started.elapsed();

    let conc_started = Instant::now();
    let mut handles = Vec::with_capacity(burst);
    for idx in 0..burst {
        let app = app.clone();
        handles.push(tokio::spawn(
            async move { admit(&app, "concB", idx).await },
        ));
    }
    let mut conc_latencies = Vec::with_capacity(burst);
    for handle in handles {
        let (status, elapsed) = handle.await??;
        assert_eq!(status, StatusCode::OK);
        conc_latencies.push(elapsed);
    }
    let conc_total = conc_started.elapsed();
    conc_latencies.sort_unstable();
    let p50 = conc_latencies
        .get(conc_latencies.len() / 2)
        .copied()
        .unwrap_or_default();
    let p99_idx = (conc_latencies.len() * 99 / 100).min(conc_latencies.len().saturating_sub(1));
    let p99 = conc_latencies.get(p99_idx).copied().unwrap_or_default();
    let speedup = seq_total.as_secs_f64() / conc_total.as_secs_f64().max(f64::MIN_POSITIVE);
    let conc_throughput = burst as f64 / conc_total.as_secs_f64().max(f64::MIN_POSITIVE);
    let conc_latency_ms = avg(&conc_latencies).as_secs_f64() * 1000.0;

    println!("  {burst} admissions sequential : {seq_total:?}");
    println!("  {burst} admissions concurrent : {conc_total:?} (4 workers)");
    println!("  concurrency speedup        : {speedup:.2}x (1.0x == fully serialized)");
    println!("  concurrent latency p50/p99 : {p50:?} / {p99:?}");
    println!();
    println!("Conclusion:");
    println!("  Per-admission latency grew {growth:.1}x across this run because every");
    println!("  mutation persists the full kernel state (clone + JSON + fsync) under one");
    println!("  RwLock. 4-way concurrency gave only {speedup:.2}x throughput, so the control");
    println!("  API is serialized: a ceiling of ~{conc_throughput:.0} mutations/sec, falling");
    println!("  as kernel state grows. Each request also holds its relay authority-slot");
    println!("  permit for its full ~{conc_latency_ms:.0}ms. Sustained fleet load above the");
    println!("  ceiling drives the relay's in-flight count to its 256-permit limit -> 503");
    println!("  'embedded Nexus control API capacity exhausted'. The relay semaphore is");
    println!("  the symptom, not the cause.");
    println!();

    assert!(
        last_window_avg > first_window_avg,
        "expected per-admission latency to grow with accumulated kernel state \
         (first window {first_window_avg:?}, last window {last_window_avg:?})"
    );
    assert!(
        speedup < 2.0,
        "expected ~serialized throughput (speedup near 1x); got {speedup:.2}x"
    );
    assert!(state_bytes > 0, "kernel state was not persisted to disk");

    Ok(())
}
