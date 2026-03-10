mod support;

use std::{fs, path::PathBuf, time::Instant};

use psionic_cluster::{
    ClusterRecoveryDisposition, ClusterReplicaLifecyclePolicy, ClusterServingPolicy,
    plan_replicated_serving, schedule_layer_sharded_execution, schedule_remote_whole_request,
    schedule_tensor_sharded_execution,
};
use serde_json::json;
use support::{
    ClusterValidationFixture, recovery_policy, sample_cluster_id, sample_recovery_log,
    stale_rejoin_request,
};

#[test]
#[ignore = "release benchmark gate"]
fn whole_request_scheduler_release_gate() {
    let fixture = ClusterValidationFixture::new();
    let state = fixture.state();
    let request = fixture.whole_request();
    run_gate(
        "whole_request_scheduler",
        env_usize("PSIONIC_CLUSTER_BENCH_WHOLE_REQUEST_ITERATIONS", 10_000),
        env_u128("PSIONIC_CLUSTER_BENCH_WHOLE_REQUEST_MAX_MS", 2_500),
        || {
            let schedule = schedule_remote_whole_request(&state, &request);
            assert!(
                schedule.is_ok(),
                "whole-request scheduler should remain valid"
            );
        },
    );
}

#[test]
#[ignore = "release benchmark gate"]
fn recovery_catchup_release_gate() {
    let log = sample_recovery_log();
    let policy = recovery_policy();
    let request = stale_rejoin_request(&sample_cluster_id());
    run_gate(
        "recovery_catchup",
        env_usize("PSIONIC_CLUSTER_BENCH_RECOVERY_ITERATIONS", 5_000),
        env_u128("PSIONIC_CLUSTER_BENCH_RECOVERY_MAX_MS", 2_500),
        || {
            let response = log.catchup_response(&request, &policy);
            assert!(response.is_ok(), "catchup should remain valid");
            assert_eq!(
                response.ok().map(|value| value.disposition),
                Some(ClusterRecoveryDisposition::CatchUp)
            );
        },
    );
}

#[test]
#[ignore = "release benchmark gate"]
fn replicated_serving_release_gate() {
    let fixture = ClusterValidationFixture::new();
    let state = fixture.state();
    let load_snapshot = fixture.load_snapshot_with_slow_nodes(&["worker-a"]);
    let replica_snapshot = fixture.replica_snapshot_with_warm_nodes(&["worker-a", "worker-b"]);
    let serving_request = fixture.serving_request("bench-replica");
    let scheduling_request = fixture.whole_request();
    run_gate(
        "replicated_serving",
        env_usize("PSIONIC_CLUSTER_BENCH_REPLICATED_ITERATIONS", 5_000),
        env_u128("PSIONIC_CLUSTER_BENCH_REPLICATED_MAX_MS", 4_000),
        || {
            let decision = plan_replicated_serving(
                &state,
                &load_snapshot,
                &replica_snapshot,
                &ClusterReplicaLifecyclePolicy::replicated_lane(),
                &ClusterServingPolicy::direct_caller_latency_first(),
                &serving_request,
                &scheduling_request,
            );
            assert!(decision.is_ok(), "replicated serving should remain valid");
        },
    );
}

#[test]
#[ignore = "release benchmark gate"]
fn layer_sharded_release_gate() {
    let fixture = ClusterValidationFixture::new();
    let state = fixture.state();
    let request = fixture.layer_request();
    let policy = fixture.layer_policy();
    run_gate(
        "layer_sharded_planner",
        env_usize("PSIONIC_CLUSTER_BENCH_LAYER_ITERATIONS", 2_000),
        env_u128("PSIONIC_CLUSTER_BENCH_LAYER_MAX_MS", 4_000),
        || {
            let schedule = schedule_layer_sharded_execution(&state, &request, &policy);
            assert!(
                schedule.is_ok(),
                "layer-sharded planner should remain valid"
            );
        },
    );
}

#[test]
#[ignore = "release benchmark gate"]
fn tensor_sharded_release_gate() {
    let fixture = ClusterValidationFixture::new();
    let state = fixture.state();
    let request = fixture.tensor_request();
    let policy = fixture.tensor_policy();
    run_gate(
        "tensor_sharded_planner",
        env_usize("PSIONIC_CLUSTER_BENCH_TENSOR_ITERATIONS", 2_000),
        env_u128("PSIONIC_CLUSTER_BENCH_TENSOR_MAX_MS", 4_000),
        || {
            let schedule = schedule_tensor_sharded_execution(&state, &request, &policy);
            assert!(
                schedule.is_ok(),
                "tensor-sharded planner should remain valid"
            );
        },
    );
}

fn run_gate(name: &str, iterations: usize, max_total_ms: u128, mut step: impl FnMut()) {
    let start = Instant::now();
    for _ in 0..iterations {
        step();
    }
    let elapsed = start.elapsed();
    let total_ms = elapsed.as_millis();
    let avg_us = elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64;
    let summary = json!({
        "benchmark": name,
        "iterations": iterations,
        "total_ms": total_ms,
        "avg_us": avg_us,
        "max_total_ms": max_total_ms,
    });
    println!("{summary}");
    maybe_write_summary(name, &summary);
    assert!(
        total_ms <= max_total_ms,
        "benchmark gate `{name}` exceeded budget: total_ms={total_ms} max_total_ms={max_total_ms}"
    );
}

fn maybe_write_summary(name: &str, summary: &serde_json::Value) {
    let Ok(dir) = std::env::var("PSIONIC_CLUSTER_BENCH_JSON_OUT") else {
        return;
    };
    let output_dir = PathBuf::from(dir);
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(&output_dir)?;
        let path = output_dir.join(format!("{name}.json"));
        fs::write(path, serde_json::to_vec_pretty(summary)?)?;
        Ok(())
    })();
    assert!(
        result.is_ok(),
        "failed to write benchmark summary: {result:?}"
    );
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_u128(name: &str, default: u128) -> u128 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}
