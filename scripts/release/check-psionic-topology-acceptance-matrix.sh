#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-topology-acceptance-matrix.sh [options]

Runs the executable Psionic serving-topology acceptance matrix for PSI-255.

Options:
  --local-only           Run only local-serving suites
  --cluster-only         Run only clustered-serving suites
  --only <suite>         Run one named suite (repeatable)
  --include-benchmarks   Include ignored cluster benchmark gates
  --help                 Show this help

Suites:
  local-baseline
  routing-cache
  pd-modes
  contracts
  whole-request
  replicated
  pipeline
  layer
  tensor
  unsupported
  benchmarks
EOF
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

declare -a selected_suites=()
include_benchmarks=0
mode="all"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --local-only)
      mode="local"
      shift
      ;;
    --cluster-only)
      mode="cluster"
      shift
      ;;
    --only)
      if [[ $# -lt 2 ]]; then
        echo "missing suite name after --only" >&2
        exit 1
      fi
      selected_suites+=("$2")
      shift 2
      ;;
    --include-benchmarks)
      include_benchmarks=1
      shift
      ;;
    --help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

run_cmd() {
  echo
  echo "==> $*"
  "$@"
}

run_filtered_test() {
  local package="$1"
  local target_kind="$2"
  local filter="$3"
  shift 3
  case "${target_kind}" in
    lib)
      run_cmd cargo test -p "${package}" --lib "${filter}" -- --nocapture "$@"
      ;;
    test)
      local target_name="$1"
      shift
      run_cmd cargo test -p "${package}" --test "${target_name}" "${filter}" -- --nocapture "$@"
      ;;
    *)
      echo "unknown target kind: ${target_kind}" >&2
      exit 1
      ;;
  esac
}

run_note() {
  echo
  echo "==> ${1}"
}

run_suite() {
  case "$1" in
    local-baseline)
      run_filtered_test psionic-serve lib cpu_reference_continuous_batch_scheduler_mixes_prefill_and_decode
      run_filtered_test psionic-serve lib cpu_reference_text_generation_reports_cold_then_warm_provenance
      run_filtered_test psionic-serve lib paged_kv_cache_tracks_growth_refill_and_refusal
      run_filtered_test psionic-serve lib shared_prefix_store_reports_hit_miss_and_rebuilt
      run_filtered_test psionic-serve lib shared_prefix_store_reports_tenant_and_sampler_boundary_refusals
      run_filtered_test psionic-serve lib cpu_reference_text_generation_reports_prefix_hits_and_bypasses
      ;;
    routing-cache)
      run_filtered_test psionic-router lib router_prefers_safe_cache_match_over_cold_route
      run_filtered_test psionic-router lib router_never_uses_unsafe_cache_match_across_tenants
      run_filtered_test psionic-router lib router_uses_power_of_two_to_pick_less_loaded_warm_route
      run_filtered_test psionic-serve lib generic_server_prefix_cache_headers_are_machine_checkable
      run_filtered_test psionic-serve lib generic_server_route_headers_are_machine_checkable
      ;;
    pd-modes)
      run_filtered_test psionic-serve lib generic_server_grammar_fallback_is_machine_checkable
      run_filtered_test psionic-runtime lib communication_eligibility_can_be_derived_from_capability_profile
      run_filtered_test psionic-runtime lib lane_communication_eligibility_refuses_undeclared_lane_even_when_profile_exists
      run_filtered_test psionic-cluster lib whole_request_scheduler_refuses_unsupported_prefill_decode_mode_explicitly
      ;;
    contracts)
      run_filtered_test psionic-runtime lib sharded_model_manifest_validates_replicated_layer_and_tensor_topologies
      run_filtered_test psionic-provider lib capability_envelope_can_publish_declared_cluster_capability_profile_without_execution
      ;;
    whole-request)
      run_filtered_test psionic-cluster test scheduling_validation_covers_staging_and_degraded_candidate cluster_validation_matrix
      run_filtered_test psionic-cluster lib whole_request_scheduler_refuses_disallowed_artifact_staging_explicitly
      run_filtered_test psionic-provider lib text_generation_receipt_preserves_cluster_execution_from_provenance
      ;;
    replicated)
      run_filtered_test psionic-cluster lib replicated_serving_builds_replicated_topology_and_selects_best_warm_replica
      run_filtered_test psionic-cluster lib replicated_serving_refuses_when_lane_lacks_enough_warm_replicas
      run_filtered_test psionic-provider lib capability_envelope_overrides_surface_for_replicated_cluster_execution
      run_filtered_test psionic-provider lib text_generation_receipt_surfaces_replicated_cluster_execution_truth
      ;;
    pipeline)
      run_filtered_test psionic-cluster lib pipeline_sharded_scheduler_builds_public_network_plan
      run_filtered_test psionic-cluster test pipeline_validation_covers_public_network_stage_truth cluster_validation_matrix
      run_filtered_test psionic-provider lib text_generation_capability_envelope_publishes_pipeline_sharded_profile_from_cluster_request
      run_filtered_test psionic-provider lib text_generation_receipt_surfaces_pipeline_sharded_cluster_execution_truth
      ;;
    layer)
      run_filtered_test psionic-cluster lib layer_sharded_scheduler_builds_two_shard_cuda_plan
      run_filtered_test psionic-cluster lib layer_sharded_scheduler_allows_bounded_degraded_handoff_with_explicit_reason
      run_filtered_test psionic-runtime lib delivered_execution_context_surfaces_layer_sharded_handoffs
      run_filtered_test psionic-provider lib text_generation_receipt_surfaces_layer_sharded_cluster_execution_truth
      ;;
    tensor)
      run_filtered_test psionic-cluster lib tensor_sharded_scheduler_builds_two_shard_cuda_plan
      run_filtered_test psionic-cluster lib tensor_sharded_scheduler_refuses_unsuitable_mesh_transport
      run_filtered_test psionic-runtime lib delivered_execution_context_surfaces_tensor_sharded_collectives
      run_filtered_test psionic-provider lib sandbox_capability_envelope_publishes_tensor_sharded_profile_from_cluster_request
      run_filtered_test psionic-provider lib text_generation_receipt_surfaces_tensor_sharded_cluster_execution_truth
      ;;
    unsupported)
      run_filtered_test psionic-cluster test scheduling_validation_refuses_metal_cluster_dispatch_explicitly cluster_validation_matrix
      run_filtered_test psionic-cluster lib whole_request_scheduler_refuses_metal_cluster_dispatch_explicitly
      run_filtered_test psionic-cluster lib tensor_sharded_scheduler_refuses_metal_backend_explicitly
      run_filtered_test psionic-cluster lib layer_sharded_scheduler_refuses_non_cuda_backend
      run_note "expert-parallel serving remains unsupported: no ClusterExecutionLane::ExpertParallel and no ExecutionTopologyKind::ExpertParallel exist in the current serving contracts"
      run_note "local tensor/pipeline/layer/replica topologies remain unsupported: the local serving surface is still one runtime owner with explicit local PD, not a shipped multi-device local planner"
      ;;
    benchmarks)
      PSIONIC_CLUSTER_BENCH_WHOLE_REQUEST_ITERATIONS=50 \
      PSIONIC_CLUSTER_BENCH_RECOVERY_ITERATIONS=25 \
      PSIONIC_CLUSTER_BENCH_REPLICATED_ITERATIONS=25 \
      PSIONIC_CLUSTER_BENCH_PIPELINE_ITERATIONS=25 \
      PSIONIC_CLUSTER_BENCH_LAYER_ITERATIONS=25 \
      PSIONIC_CLUSTER_BENCH_TENSOR_ITERATIONS=25 \
      run_cmd cargo test -p psionic-cluster --test cluster_benchmark_gates -- --ignored --nocapture
      ;;
    *)
      echo "unknown suite: $1" >&2
      exit 1
      ;;
  esac
}

if [[ "${#selected_suites[@]}" -eq 0 ]]; then
  case "${mode}" in
    local)
      selected_suites=(
        local-baseline
        routing-cache
        pd-modes
        unsupported
      )
      ;;
    cluster)
      selected_suites=(
        pd-modes
        contracts
        whole-request
        replicated
        pipeline
        layer
        tensor
        unsupported
      )
      ;;
    all)
      selected_suites=(
        local-baseline
        routing-cache
        pd-modes
        contracts
        whole-request
        replicated
        pipeline
        layer
        tensor
        unsupported
      )
      ;;
  esac
fi

if [[ "${include_benchmarks}" -eq 1 ]]; then
  selected_suites+=(benchmarks)
fi

echo "Running Psionic topology acceptance matrix from ${repo_root}"
echo "Selected suites: ${selected_suites[*]}"

for suite in "${selected_suites[@]}"; do
  run_note "suite: ${suite}"
  run_suite "${suite}"
done

echo
echo "Psionic topology acceptance matrix passed."
