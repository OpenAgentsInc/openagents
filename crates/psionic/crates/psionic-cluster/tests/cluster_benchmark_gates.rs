#![allow(clippy::print_stdout)]

mod support;

use std::{fs, path::PathBuf, time::Instant};

use psionic_cluster::{
    ClusterBenchmarkContext, ClusterBenchmarkId, ClusterBenchmarkReceipt,
    ClusterRecoveryBenchmarkContext, ClusterRecoveryDisposition, ClusterReplicaLifecyclePolicy,
    ClusterServingPolicy, ClusterTopologyBenchmarkContext, plan_replicated_serving,
    schedule_layer_sharded_execution, schedule_pipeline_sharded_execution,
    schedule_remote_whole_request, schedule_tensor_sharded_execution,
};
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
        ClusterBenchmarkId::WholeRequestScheduler,
        ClusterBenchmarkContext::Topology(ClusterTopologyBenchmarkContext::from_state(
            fixture.cluster_id.clone(),
            &state,
        )),
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
    let expected_response = log.catchup_response(&request, &policy);
    assert!(
        expected_response.is_ok(),
        "recovery benchmark context should remain valid: {expected_response:?}"
    );
    let expected_response = match expected_response {
        Ok(value) => value,
        Err(_) => return,
    };
    let recovery_context = ClusterRecoveryBenchmarkContext::from_response(&expected_response);
    assert!(
        recovery_context.is_ok(),
        "recovery benchmark context should derive a stable digest: {recovery_context:?}"
    );
    let recovery_context = match recovery_context {
        Ok(value) => value,
        Err(_) => return,
    };
    run_gate(
        ClusterBenchmarkId::RecoveryCatchup,
        ClusterBenchmarkContext::Recovery(recovery_context),
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
        ClusterBenchmarkId::ReplicatedServing,
        ClusterBenchmarkContext::Topology(ClusterTopologyBenchmarkContext::from_state(
            fixture.cluster_id.clone(),
            &state,
        )),
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
fn pipeline_sharded_release_gate() {
    let fixture = ClusterValidationFixture::new();
    let state = fixture.public_pipeline_state();
    let request = fixture.pipeline_request();
    let policy = fixture.pipeline_policy();
    run_gate(
        ClusterBenchmarkId::PipelineShardedPlanner,
        ClusterBenchmarkContext::Topology(ClusterTopologyBenchmarkContext::from_state(
            fixture.cluster_id.clone(),
            &state,
        )),
        env_usize("PSIONIC_CLUSTER_BENCH_PIPELINE_ITERATIONS", 2_000),
        env_u128("PSIONIC_CLUSTER_BENCH_PIPELINE_MAX_MS", 5_000),
        || {
            let schedule = schedule_pipeline_sharded_execution(&state, &request, &policy);
            assert!(
                schedule.is_ok(),
                "pipeline-sharded planner should remain valid"
            );
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
        ClusterBenchmarkId::LayerShardedPlanner,
        ClusterBenchmarkContext::Topology(ClusterTopologyBenchmarkContext::from_state(
            fixture.cluster_id.clone(),
            &state,
        )),
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
        ClusterBenchmarkId::TensorShardedPlanner,
        ClusterBenchmarkContext::Topology(ClusterTopologyBenchmarkContext::from_state(
            fixture.cluster_id.clone(),
            &state,
        )),
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

fn run_gate(
    benchmark_id: ClusterBenchmarkId,
    context: ClusterBenchmarkContext,
    iterations: usize,
    max_total_ms: u128,
    mut step: impl FnMut(),
) {
    let start = Instant::now();
    for _ in 0..iterations {
        step();
    }
    let elapsed = start.elapsed();
    let receipt =
        ClusterBenchmarkReceipt::measured(benchmark_id, context, iterations, max_total_ms, elapsed);
    let encoded = serde_json::to_string(&receipt);
    assert!(
        encoded.is_ok(),
        "benchmark receipt should encode: {encoded:?}"
    );
    let encoded = match encoded {
        Ok(value) => value,
        Err(_) => return,
    };
    println!("{encoded}");
    maybe_write_receipt(&receipt);
    assert!(
        receipt.outcome == psionic_cluster::ClusterBenchmarkOutcome::Passed,
        "benchmark gate `{}` exceeded budget: total_duration_ns={} max_total_duration_ms={}",
        receipt.benchmark_id.as_str(),
        receipt.total_duration_ns,
        receipt.max_total_duration_ms
    );
}

fn maybe_write_receipt(receipt: &ClusterBenchmarkReceipt) {
    let Ok(dir) = std::env::var("PSIONIC_CLUSTER_BENCH_JSON_OUT") else {
        return;
    };
    let output_dir = PathBuf::from(dir);
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(&output_dir)?;
        let path = output_dir.join(format!("{}.json", receipt.benchmark_id.as_str()));
        fs::write(path, serde_json::to_vec_pretty(receipt)?)?;
        Ok(())
    })();
    assert!(
        result.is_ok(),
        "failed to write benchmark receipt: {result:?}"
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
