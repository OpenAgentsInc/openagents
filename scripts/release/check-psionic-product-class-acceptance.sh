#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-product-class-acceptance.sh [options]

Runs the canonical product-class acceptance split for PSI-256.

Options:
  --local-portability-only           Run only the local-portability category
  --throughput-serving-only          Run only the high-throughput-serving category
  --structured-agent-only            Run only the structured-agent category
  --apple-silicon                    Include Apple Silicon local-portability hooks
  --linux-nvidia                     Include Linux NVIDIA local-portability hooks
  --include-local-throughput-benchmark
                                     Include the exact local GPT-OSS benchmark script
  --local-throughput-json-out DIR    Write local throughput benchmark receipts to DIR
  --help                             Show this help
EOF
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

declare -a selected_categories=()
include_apple_silicon=0
include_linux_nvidia=0
include_local_throughput_benchmark=0
local_throughput_json_out=""
mode="all"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --local-portability-only)
      mode="local"
      shift
      ;;
    --throughput-serving-only)
      mode="throughput"
      shift
      ;;
    --structured-agent-only)
      mode="agent"
      shift
      ;;
    --apple-silicon)
      include_apple_silicon=1
      shift
      ;;
    --linux-nvidia)
      include_linux_nvidia=1
      shift
      ;;
    --include-local-throughput-benchmark)
      include_local_throughput_benchmark=1
      shift
      ;;
    --local-throughput-json-out)
      if [[ $# -lt 2 ]]; then
        echo "missing directory after --local-throughput-json-out" >&2
        exit 1
      fi
      local_throughput_json_out="$2"
      shift 2
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
  echo "==> $1"
}

run_local_portability() {
  run_filtered_test psionic-serve test model_backed_embeddings_flow_returns_response_capability_and_receipt model_backed_embeddings
  run_filtered_test psionic-serve test model_backed_text_generation_flow_returns_response_capability_and_receipt model_backed_text_generation
  run_filtered_test psionic-serve lib local_serving_truth_headers_include_optional_hybrid_layers
  run_filtered_test psionic-serve lib ollama_http_subject_normalizes_live_http_responses
  run_filtered_test psionic-provider lib metal_gpt_oss_text_generation_fallback_capability_reports_explicit_refusal_validation
  run_filtered_test psionic-provider lib metal_gpt_oss_text_generation_failed_receipt_reports_explicit_refusal_validation

  if [[ "${include_apple_silicon}" -eq 1 ]]; then
    run_filtered_test psionic-serve test metal_model_backed_embeddings_flow_returns_response_capability_and_receipt_or_explicit_unavailability metal_model_backed_embeddings
    run_filtered_test psionic-serve test metal_model_backed_embeddings_match_cpu_baseline_within_tolerance_on_ready_hardware metal_embeddings_parity
    run_filtered_test psionic-serve test metal_model_backed_text_generation_returns_response_capability_and_receipt_or_explicit_unavailability metal_model_backed_text_generation
    run_filtered_test psionic-serve test metal_text_generation_matches_cpu_baseline_within_budget_and_seeded_sampling metal_text_generation_parity
    run_filtered_test psionic-serve lib metal_gpt_oss_service_matches_cpu_reference_on_synthetic_fixture
  else
    run_note "skipping Apple Silicon local-portability hooks; rerun with --apple-silicon before claiming Apple-native portability closure"
  fi

  if [[ "${include_linux_nvidia}" -eq 1 ]]; then
    run_filtered_test psionic-serve test cuda_model_backed_embeddings_flow_returns_response_capability_and_receipt_or_explicit_unavailability cuda_model_backed_embeddings
    run_filtered_test psionic-serve test cuda_model_backed_embeddings_match_cpu_baseline_within_tolerance_or_report_explicit_fallback cuda_embeddings_parity
  else
    run_note "skipping Linux NVIDIA local-portability hooks; rerun with --linux-nvidia before claiming CUDA local portability closure"
  fi
}

run_throughput_serving() {
  run_filtered_test psionic-serve lib cpu_reference_continuous_batch_scheduler_mixes_prefill_and_decode
  run_filtered_test psionic-serve lib cpu_reference_text_generation_reports_prefix_hits_and_bypasses
  run_filtered_test psionic-serve lib generic_server_prefix_cache_headers_are_machine_checkable
  run_cmd scripts/release/check-psionic-topology-acceptance-matrix.sh --cluster-only --include-benchmarks

  if [[ "${include_local_throughput_benchmark}" -eq 1 ]]; then
    benchmark_cmd=(
      crates/psionic/scripts/benchmark-gpt-oss-vs-llama.sh
    )
    if [[ -n "${local_throughput_json_out}" ]]; then
      benchmark_cmd+=(--json-out "${local_throughput_json_out}")
    fi
    run_cmd "${benchmark_cmd[@]}"
  else
    run_note "skipping exact local GPT-OSS benchmark hook; rerun with --include-local-throughput-benchmark before claiming exact local tok/s closure"
  fi
}

run_structured_agent() {
  run_filtered_test psionic-serve lib generic_server_grammar_fallback_is_machine_checkable
  run_filtered_test psionic-serve lib generic_server_json_schema_fallback_is_machine_checkable
  run_filtered_test psionic-serve lib generic_server_required_tool_call_is_machine_checkable
  run_filtered_test psionic-serve lib generic_server_tool_call_validation_refuses_invalid_arguments
  run_filtered_test psionic-serve lib generic_server_streaming_tool_calls_preserve_machine_envelope
  run_filtered_test psionic-serve lib generic_server_router_tool_loop_boundary_executes_multi_step_flow
  run_filtered_test psionic-router lib tool_loop_executes_router_owned_multi_step_flow
  run_filtered_test psionic-router lib tool_loop_refuses_hidden_tool_results
  run_filtered_test psionic-serve lib generic_responses_conversation_state_replays_and_updates
  run_filtered_test psionic-serve lib generic_responses_file_backed_state_survives_server_restart
  run_filtered_test psionic-serve lib generic_responses_refuse_unknown_state_references
  run_filtered_test psionic-serve lib generic_server_refuses_unsupported_json_schema_features
  run_filtered_test psionic-serve lib generic_server_refuses_reasoning_request_for_unsupported_family
  run_filtered_test psionic-serve lib generic_server_route_headers_are_machine_checkable
}

case "${mode}" in
  local)
    selected_categories=(local-portability)
    ;;
  throughput)
    selected_categories=(throughput-serving)
    ;;
  agent)
    selected_categories=(structured-agent)
    ;;
  all)
    selected_categories=(
      local-portability
      throughput-serving
      structured-agent
    )
    ;;
esac

echo "Running Psionic product-class acceptance matrices from ${repo_root}"
echo "Selected categories: ${selected_categories[*]}"

for category in "${selected_categories[@]}"; do
  run_note "category: ${category}"
  case "${category}" in
    local-portability)
      run_local_portability
      ;;
    throughput-serving)
      run_throughput_serving
      ;;
    structured-agent)
      run_structured_agent
      ;;
    *)
      echo "unknown category: ${category}" >&2
      exit 1
      ;;
  esac
done

echo
echo "Psionic product-class acceptance matrices passed."
