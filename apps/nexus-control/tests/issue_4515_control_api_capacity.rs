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
//! This test drives requests through the real `nexus-control` router and
//! prints four measurements:
//!   Phase A — per-admission latency as kernel state accumulates,
//!   Phase B — admission throughput under 4-way concurrency vs. sequential,
//!   Phase C — validator-challenge claim latency, idle vs. under admission
//!             load (the validator backlog drains through the same lock),
//!   Phase D — artifact-resolver and signed-access read-handler latency,
//!             idle vs. under admission load.
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

/// Send one validator-challenge claim; return its status, body, and latency.
///
/// The claim targets a node that was never admitted, so the handler acquires
/// the global `store` write lock, finds no such node, and returns a client
/// error. Claim/finalize are `store.write()` mutations like admission, so the
/// claim's latency is dominated by the time spent waiting to *acquire* that
/// lock — which is what Phase C measures under contention.
async fn claim(app: &Router, idx: usize) -> TestResult<(StatusCode, String, Duration)> {
    let body = serde_json::json!({
        "idempotency_key": format!("issue4515-claim-{idx:06}"),
        "requested_at_ms": now_unix_ms(),
        "node_pubkey_hex": format!("issue4515-validator-{idx:06}"),
    });
    let request = Request::builder()
        .method("POST")
        .uri("/api/training/validator-challenges/claim")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body)?))?;
    let started = Instant::now();
    let response = app.clone().oneshot(request).await?;
    let elapsed = started.elapsed();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let body = String::from_utf8_lossy(&bytes).into_owned();
    Ok((status, body, elapsed))
}

/// Probe the artifact resolver for a missing artifact. The handler acquires
/// `store.read()`, fails the lookup, and returns — so the latency is the time
/// spent acquiring that read lock. (Read path: `get_kernel_compute_training_
/// artifact_resolver`.)
async fn resolve_probe(app: &Router, idx: usize) -> TestResult<(StatusCode, String, Duration)> {
    let uri = format!("/v1/kernel/compute/training/artifacts/issue4515-missing-{idx:06}");
    let request = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())?;
    let started = Instant::now();
    let response = app.clone().oneshot(request).await?;
    let elapsed = started.elapsed();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await?;
    Ok((status, String::from_utf8_lossy(&bytes).into_owned(), elapsed))
}

/// Probe signed-access for a missing artifact. The handler acquires
/// `store.read()` and resolves the artifact *before* it reaches signed-URL
/// signing, so a missing artifact fails right after the read lock — the
/// latency is again the read-lock acquire. (Read path:
/// `post_kernel_compute_training_artifact_signed_access`.)
async fn signed_access_probe(
    app: &Router,
    idx: usize,
) -> TestResult<(StatusCode, String, Duration)> {
    let uri = format!(
        "/v1/kernel/compute/training/artifacts/issue4515-missing-{idx:06}/signed-access"
    );
    let body = serde_json::json!({ "mode": "read" });
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body)?))?;
    let started = Instant::now();
    let response = app.clone().oneshot(request).await?;
    let elapsed = started.elapsed();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await?;
    Ok((status, String::from_utf8_lossy(&bytes).into_owned(), elapsed))
}

fn avg(samples: &[Duration]) -> Duration {
    if samples.is_empty() {
        return Duration::ZERO;
    }
    samples.iter().sum::<Duration>() / samples.len() as u32
}

fn pctl99(sorted: &[Duration]) -> Duration {
    sorted
        .get((sorted.len() * 99 / 100).min(sorted.len().saturating_sub(1)))
        .copied()
        .unwrap_or_default()
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

    // ---- Phase C: validator-challenge requests stall on the same lock ----
    // Validator claim/retry/finalize are `store.write()` mutations too. A claim
    // for a never-admitted node still acquires the global write lock before it
    // fails, so its latency measures time waiting for that lock. Measured idle
    // and then under a concurrent admission flood.
    println!("Phase C — validator-challenge claim latency, idle vs under admission load:");
    let claim_count = 60usize;

    let mut idle_claims = Vec::with_capacity(claim_count);
    let mut claim_status = StatusCode::OK;
    let mut claim_body = String::new();
    for idx in 0..claim_count {
        let (status, body, elapsed) = claim(&app, idx).await?;
        claim_status = status;
        claim_body = body;
        idle_claims.push(elapsed);
    }

    let flood = burst * 3;
    let mut flood_handles = Vec::with_capacity(flood);
    for idx in 0..flood {
        let app = app.clone();
        flood_handles.push(tokio::spawn(
            async move { admit(&app, "floodC", idx).await },
        ));
    }
    let mut loaded_claims = Vec::with_capacity(claim_count);
    for idx in 0..claim_count {
        let (_status, _body, elapsed) = claim(&app, claim_count + idx).await?;
        loaded_claims.push(elapsed);
    }
    for handle in flood_handles {
        let (status, _) = handle.await??;
        assert_eq!(status, StatusCode::OK);
    }

    idle_claims.sort_unstable();
    loaded_claims.sort_unstable();
    let idle_claim_avg = avg(&idle_claims);
    let loaded_claim_avg = avg(&loaded_claims);
    let idle_p99 = idle_claims
        .get((idle_claims.len() * 99 / 100).min(idle_claims.len().saturating_sub(1)))
        .copied()
        .unwrap_or_default();
    let loaded_p99 = loaded_claims
        .get((loaded_claims.len() * 99 / 100).min(loaded_claims.len().saturating_sub(1)))
        .copied()
        .unwrap_or_default();
    let claim_slowdown =
        loaded_claim_avg.as_secs_f64() / idle_claim_avg.as_secs_f64().max(f64::MIN_POSITIVE);
    let claim_reached = if claim_body.contains("training_node_not_found") {
        "reached handler (training_node_not_found)"
    } else {
        "UNEXPECTED body"
    };
    println!("  claim handler:              HTTP {claim_status} — {claim_reached}");
    println!("  idle       claim avg/p99 :  {idle_claim_avg:?} / {idle_p99:?}");
    println!("  under-load claim avg/p99 :  {loaded_claim_avg:?} / {loaded_p99:?}");
    println!("  claim slowdown under load:  {claim_slowdown:.0}x");
    println!();

    // ---- Phase D: read handlers (artifact resolver, signed access) ----
    // The resolver and signed-access endpoints are read paths: each takes
    // state.store.read() before its (fast) work. A request for a missing
    // artifact still acquires that read lock, so its latency measures how long
    // a reader waits while writers hold the lock. These map to the production
    // "Artifact resolver latency" and "Signed access latency" health metrics.
    println!("Phase D — read-handler latency, idle vs under admission load:");
    let probe_count = 30usize;

    let mut resolver_idle = Vec::with_capacity(probe_count);
    let mut signed_idle = Vec::with_capacity(probe_count);
    let mut resolver_status = StatusCode::OK;
    let mut signed_status = StatusCode::OK;
    let mut resolver_body = String::new();
    let mut signed_body = String::new();
    for idx in 0..probe_count {
        let (rs, rb, rl) = resolve_probe(&app, idx).await?;
        resolver_status = rs;
        resolver_body = rb;
        resolver_idle.push(rl);
        let (ss, sb, sl) = signed_access_probe(&app, idx).await?;
        signed_status = ss;
        signed_body = sb;
        signed_idle.push(sl);
    }

    let read_flood = burst * 2;
    let mut read_flood_handles = Vec::with_capacity(read_flood);
    for idx in 0..read_flood {
        let app = app.clone();
        read_flood_handles.push(tokio::spawn(
            async move { admit(&app, "floodD", idx).await },
        ));
    }
    let mut resolver_loaded = Vec::with_capacity(probe_count);
    let mut signed_loaded = Vec::with_capacity(probe_count);
    for idx in 0..probe_count {
        let (.., rl) = resolve_probe(&app, probe_count + idx).await?;
        resolver_loaded.push(rl);
        let (.., sl) = signed_access_probe(&app, probe_count + idx).await?;
        signed_loaded.push(sl);
    }
    for handle in read_flood_handles {
        let (status, _) = handle.await??;
        assert_eq!(status, StatusCode::OK);
    }

    resolver_idle.sort_unstable();
    resolver_loaded.sort_unstable();
    signed_idle.sort_unstable();
    signed_loaded.sort_unstable();
    let resolver_idle_avg = avg(&resolver_idle);
    let resolver_loaded_avg = avg(&resolver_loaded);
    let signed_idle_avg = avg(&signed_idle);
    let signed_loaded_avg = avg(&signed_loaded);
    let resolver_loaded_p99 = pctl99(&resolver_loaded);
    let signed_loaded_p99 = pctl99(&signed_loaded);
    let resolver_slowdown =
        resolver_loaded_avg.as_secs_f64() / resolver_idle_avg.as_secs_f64().max(f64::MIN_POSITIVE);
    let signed_slowdown =
        signed_loaded_avg.as_secs_f64() / signed_idle_avg.as_secs_f64().max(f64::MIN_POSITIVE);
    println!("  resolver      : HTTP {resolver_status}");
    println!(
        "    idle {resolver_idle_avg:?}  ->  under load {resolver_loaded_avg:?} \
         / p99 {resolver_loaded_p99:?}  ({resolver_slowdown:.0}x)"
    );
    println!("  signed-access : HTTP {signed_status}");
    println!(
        "    idle {signed_idle_avg:?}  ->  under load {signed_loaded_avg:?} \
         / p99 {signed_loaded_p99:?}  ({signed_slowdown:.0}x)"
    );
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
    println!("  Validator claim/finalize are the same store.write() mutations: under");
    println!("  admission load, validator claim latency rose {claim_slowdown:.0}x here, so the");
    println!("  validator backlog cannot drain while the control plane is saturated.");
    println!("  The artifact resolver and signed-access endpoints are read paths; under");
    println!("  admission load they stalled {resolver_slowdown:.0}x / {signed_slowdown:.0}x waiting for the same");
    println!("  lock — the mechanism behind the resolver/signed-access latency alerts.");
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
    assert!(
        claim_body.contains("training_node_not_found"),
        "validator claim did not reach the handler; body: {claim_body}"
    );
    assert!(
        claim_status.is_client_error(),
        "expected a client error from the unadmitted-node claim; got {claim_status}"
    );
    assert!(
        loaded_claim_avg > idle_claim_avg * 2,
        "expected validator claims to stall under admission load \
         (idle avg {idle_claim_avg:?}, under-load avg {loaded_claim_avg:?})"
    );
    assert!(
        resolver_status.is_client_error() && signed_status.is_client_error(),
        "expected resolver/signed-access probes to reach the handler \
         (resolver {resolver_status} body {resolver_body}; \
          signed-access {signed_status} body {signed_body})"
    );
    assert!(
        resolver_loaded_avg > resolver_idle_avg * 2,
        "expected the artifact resolver to stall under admission load \
         (idle {resolver_idle_avg:?}, under-load {resolver_loaded_avg:?})"
    );
    assert!(
        signed_loaded_avg > signed_idle_avg * 2,
        "expected signed-access to stall under admission load \
         (idle {signed_idle_avg:?}, under-load {signed_loaded_avg:?})"
    );

    Ok(())
}
