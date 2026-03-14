#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-framework-core-acceptance.sh [options]

Runs the canonical framework-core acceptance matrix for #3609.

Options:
  --only <category>   Run one named category (repeatable)
  --help              Show this help

Categories:
  tensor-semantics
  autodiff-optimizer
  model-state-io
  compiler-realize
  memory-cache
  replay-identity
  local-multi-device
EOF
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

declare -a selected_categories=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --only)
      if [[ $# -lt 2 ]]; then
        echo "missing category name after --only" >&2
        exit 1
      fi
      selected_categories+=("$2")
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

if [[ "${#selected_categories[@]}" -eq 0 ]]; then
  selected_categories=(
    tensor-semantics
    autodiff-optimizer
    model-state-io
    compiler-realize
    memory-cache
    replay-identity
    local-multi-device
  )
fi

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

run_category() {
  case "$1" in
    tensor-semantics)
      run_filtered_test psionic-core lib tests::tensor_spec_retains_device_and_dtype
      run_filtered_test psionic-core lib tests::layout_expand_uses_zero_strides
      run_filtered_test psionic-core lib tests::layout_permute_updates_shape_and_strides
      ;;
    autodiff-optimizer)
      run_filtered_test psionic-train lib core_loop::tests::fixed_budget_training_loop_applies_updates_and_tracks_telemetry
      run_filtered_test psionic-train lib distributed_optimizer::tests::distributed_optimizer_contract_surfaces_precision_and_memory_truth
      run_filtered_test psionic-train lib distributed_optimizer::tests::distributed_optimizer_contract_refuses_incomplete_shard_coverage
      run_note "open gap: #3602 and #3603 remain open; current hooks validate explicit-gradient training core and train-owned optimizer contracts, not general reverse-mode autodiff or reusable optimizer primitives"
      ;;
    model-state-io)
      run_filtered_test psionic-train lib model_io::tests::portable_model_bundle_roundtrips_through_safetensors_manifest
      run_filtered_test psionic-train lib model_io::tests::gguf_import_surfaces_tokenizer_binding_and_tensor_inventory
      ;;
    compiler-realize)
      run_filtered_test psionic-compiler lib tests::compile_graph_lists_expected_steps
      run_filtered_test psionic-compiler lib tests::compile_graph_preserves_deterministic_digest
      run_filtered_test psionic-compiler test matmul_add_replay_fixture_matches process_replay
      run_filtered_test psionic-compiler test attention_backend_extension_tensor_sharded_replay_fixture_matches process_replay
      ;;
    memory-cache)
      run_filtered_test psionic-runtime lib tests::model_admission_can_evict_oldest_idle_model_to_fit_budget
      run_filtered_test psionic-runtime lib tests::model_admission_refuses_when_only_active_models_block_the_budget
      run_filtered_test psionic-runtime lib tests::prefix_cache_identity_and_policy_serialize_stably
      run_filtered_test psionic-runtime lib tests::kv_cache_state_and_growth_serialize_stably
      ;;
    replay-identity)
      run_filtered_test psionic-train lib replay_truth::tests::replay_truth_receipt_is_machine_legible_and_verifiable
      run_filtered_test psionic-train lib replay_truth::tests::replay_truth_verification_detects_seed_tool_and_order_drift
      run_filtered_test psionic-compiler test matmul_add_replay_fixture_matches process_replay
      ;;
    local-multi-device)
      run_filtered_test psionic-runtime lib tests::backend_selection_can_surface_multi_device_topology_truth
      run_filtered_test psionic-compiler lib tests::compile_graph_with_topology_changes_digest_when_sharding_changes
      run_note "open gap: #3608 remains open; current hooks validate multi-device topology identity and topology-sensitive compile digests, not a shipped same-host multi-device plan runner"
      run_note "adjacent refusal contract: TOPOLOGY_ACCEPTANCE_MATRIX.md still keeps local tensor/pipeline/layer/replica topologies unsupported until the local runner issue closes"
      ;;
    *)
      echo "unknown category: $1" >&2
      exit 1
      ;;
  esac
}

echo "Running Psionic framework-core acceptance matrix from ${repo_root}"
echo "Selected categories: ${selected_categories[*]}"

for category in "${selected_categories[@]}"; do
  run_note "category: ${category}"
  run_category "${category}"
done

echo
echo "Psionic framework-core acceptance hooks completed."
echo "Open framework-core gap categories remain explicit:"
echo "- autodiff-optimizer"
echo "- local-multi-device"
echo "Do not treat this runner as proof of Tinygrad-class framework-core closure while #3602, #3603, and #3608 remain open."
