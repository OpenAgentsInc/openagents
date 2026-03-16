#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release/check-psionic-framework-core-acceptance.sh [options]

Runs the canonical framework-core acceptance matrix for PLIB-107 / #3709.

Options:
  --only <category>   Run one named category (repeatable)
  --report <path>     Write a machine-readable JSON acceptance report
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
report_path=""
report_schema="crates/psionic/docs/framework_core_acceptance_report.schema.json"
report_work_dir="$(mktemp -d)"
category_meta_tsv="${report_work_dir}/category_meta.tsv"
hook_results_tsv="${report_work_dir}/hook_results.tsv"
overall_result="passed"
trap 'rm -rf "${report_work_dir}"' EXIT

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
    --report)
      if [[ $# -lt 2 ]]; then
        echo "missing output path after --report" >&2
        exit 1
      fi
      report_path="$2"
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

write_tsv_line() {
  local destination="$1"
  shift
  {
    local first=1
    for field in "$@"; do
      if [[ "${first}" -eq 0 ]]; then
        printf '\t'
      fi
      printf '%s' "${field}"
      first=0
    done
    printf '\n'
  } >> "${destination}"
}

category_status() {
  case "$1" in
    tensor-semantics|autodiff-optimizer|model-state-io|compiler-realize|memory-cache|replay-identity|local-multi-device)
      echo "implemented_early"
      ;;
    *)
      echo "unknown category: $1" >&2
      exit 1
      ;;
  esac
}

category_summary() {
  case "$1" in
    tensor-semantics)
      echo "Tensor identity, layout/view semantics, dtype promotion, quantized storage posture, and cross-library refusal boundaries stay explicit and deterministic."
      ;;
    autodiff-optimizer)
      echo "Reverse-mode autodiff, detach and no-grad posture, reusable optimizer families, and distributed optimizer contracts stay machine-checkable outside one training loop."
      ;;
    model-state-io)
      echo "Portable model bundles, tokenizer bindings, and tensor-role manifests roundtrip through stable formats instead of lane-local loaders."
      ;;
    compiler-realize)
      echo "The IR and compiler publish schema-backed lowering, transform-safety and meta-execution contracts, deterministic compiler artifacts, and fixture-backed replay."
      ;;
    memory-cache)
      echo "Compile-time memory planning plus runtime admission, KV cache, and prefix-cache truth stay explicit instead of hiding behind backend heuristics."
      ;;
    replay-identity)
      echo "Compiled and trained paths emit machine-legible replay and provenance identity instead of relying on anecdotal environment memory."
      ;;
    local-multi-device)
      echo "Same-host same-backend execution already has a real tensor-sharded runner, explicit sharding contracts, and topology-sensitive compiler identity."
      ;;
    *)
      echo "unknown category: $1" >&2
      exit 1
      ;;
  esac
}

category_boundary_note() {
  case "$1" in
    tensor-semantics)
      echo "The shared semantics are explicit, but this remains short of a blanket claim that every backend executes every promoted dtype or advanced storage family."
      ;;
    autodiff-optimizer)
      echo "Backend-extension gradients still refuse explicitly and later operator families remain outside the current acceptance runner."
      ;;
    model-state-io)
      echo "State-dict and optimizer-state compatibility still belong to the next semantics layer rather than this framework-core acceptance pass."
      ;;
    compiler-realize)
      echo "The runner proves broad contract coverage, but later semantics work still has to widen operator, export, and compatibility breadth above this compact core."
      ;;
    memory-cache)
      echo "Compile-time and runtime cache truth are explicit here, but product throughput claims still need separate serving evidence."
      ;;
    replay-identity)
      echo "Framework replay closure here does not substitute for product routing or request-receipt acceptance."
      ;;
    local-multi-device)
      echo "Local serving topology remains intentionally unsupported until a served lane adopts the lower-level same-host multi-device runner."
      ;;
    *)
      echo "unknown category: $1" >&2
      exit 1
      ;;
  esac
}

category_hooks() {
  case "$1" in
    tensor-semantics)
      cat <<'EOF'
tensor-spec-device-dtype|psionic-core|lib||tests::tensor_spec_retains_device_and_dtype
broadcast-shape-rules|psionic-core|lib||tests::shape_broadcast_merges_trailing_singleton_axes
dtype-promotion-table|psionic-core|lib||tests::dtype_promotion_prefers_widest_supported_representation
quantized-dtype-contracts|psionic-core|lib||tests::dtype_contracts_mark_current_quantized_and_dense_surface
derived-view-aliases|psionic-core|lib||tests::derived_views_remain_alias_preserving_transforms
layout-alias-relation|psionic-core|lib||tests::layout_alias_relation_tracks_dense_and_broadcast_views
refusal-builder|psionic-core|lib||tests::psionic_refusal_builder_keeps_code_scope_and_subject
expand-zero-strides|psionic-core|lib||tests::layout_expand_uses_zero_strides
permute-shape-strides|psionic-core|lib||tests::layout_permute_updates_shape_and_strides
cpu-buffer-view-contracts|psionic-backend-cpu|lib||tests::cpu_buffer_views_preserve_storage_identity_and_view_semantics
cpu-allocator-reuse|psionic-backend-cpu|lib||tests::cpu_allocator_pool_reuses_dense_storage_identity
graph-refusal-adapters|psionic-ir|lib||tests::graph_error_refusal_taxonomy_maps_layout_capability_and_serialization_boundaries
autodiff-refusal-adapters|psionic-ir|lib||autodiff::tests::autodiff_refusal_taxonomy_maps_unsupported_gradient_family
binary-broadcast-expands|psionic-ir|lib||tests::binary_ops_broadcast_inputs_through_explicit_expand_views
cpu-broadcast-execution|psionic-backend-cpu|lib||tests::cpu_backend_executes_broadcast_add_over_index_views
runtime-refusal-adapters|psionic-runtime|lib||tests::runtime_refusal_taxonomy_maps_capability_and_serialization_boundaries
local-topology-refusal|psionic-runtime|lib||local_multi_device::tests::local_sharding_contract_refusal_taxonomy_surfaces_topology_mismatch
sandbox-policy-refusal|psionic-sandbox|lib||execution::tests::policy_rejection_receipt_maps_into_refusal_taxonomy
EOF
      ;;
    autodiff-optimizer)
      cat <<'EOF'
autodiff-matmul-chain|psionic-ir|lib||autodiff::tests::reverse_mode_autodiff_materializes_matmul_chain_gradients
autodiff-shared-paths-detach|psionic-ir|lib||autodiff::tests::reverse_mode_autodiff_accumulates_shared_paths_and_honors_detach
autodiff-select-concat-reshape|psionic-ir|lib||autodiff::tests::reverse_mode_autodiff_covers_select_concat_and_reshape_primitives
autodiff-axis-seed|psionic-ir|lib||autodiff::tests::reverse_mode_autodiff_accepts_non_scalar_axis_seed
autodiff-training-no-grad|psionic-ir|lib||autodiff::tests::autodiff_context_makes_training_and_no_grad_behavior_explicit
autodiff-support-matrix|psionic-ir|lib||autodiff::tests::autodiff_support_matrix_marks_primitives_and_backend_extensions_explicitly
autodiff-backend-extension-refusal|psionic-ir|lib||autodiff::tests::unsupported_gradient_backend_extensions_refuse_per_op_label
autodiff-typed-refusal|psionic-ir|lib||autodiff::tests::unsupported_gradient_ops_refuse_through_typed_error
trainer-autodiff-integration|psionic-train|lib||core_loop::tests::autodiff_gradients_compose_with_fixed_budget_training_core
trainer-fixed-budget-step|psionic-train|lib||core_loop::tests::fixed_budget_training_loop_applies_updates_and_tracks_telemetry
optimizer-small-model|psionic-train|lib||optimizer::tests::reusable_optimizer_surface_advances_small_model_with_sgd_and_adam
optimizer-family-surface|psionic-train|lib||optimizer::tests::reusable_optimizer_surface_supports_all_declared_optimizer_families
optimizer-state-refusal|psionic-train|lib||optimizer::tests::reusable_optimizer_surface_refuses_state_kind_mismatch
distributed-optimizer-contract|psionic-train|lib||distributed_optimizer::tests::distributed_optimizer_contract_surfaces_precision_and_memory_truth
distributed-optimizer-refusal|psionic-train|lib||distributed_optimizer::tests::distributed_optimizer_contract_refuses_incomplete_shard_coverage
EOF
      ;;
    model-state-io)
      cat <<'EOF'
portable-model-bundle-roundtrip|psionic-train|lib||model_io::tests::portable_model_bundle_roundtrips_through_safetensors_manifest
gguf-import-tokenizer-inventory|psionic-train|lib||model_io::tests::gguf_import_surfaces_tokenizer_binding_and_tensor_inventory
EOF
      ;;
    compiler-realize)
      cat <<'EOF'
builtin-operator-registry|psionic-ir|lib||tests::builtin_operator_registry_exposes_kernel_composite_and_meta_surfaces
extensible-operator-registry-seed|psionic-ir|lib||tests::extensible_operator_registry_seeds_builtin_dispatch_contracts
extensible-operator-registry-custom-dispatch|psionic-ir|lib||tests::extensible_operator_registry_accepts_custom_schema_and_backend_dispatch
extensible-operator-registry-refusal|psionic-ir|lib||tests::extensible_operator_registry_refuses_shadowing_duplicates_and_missing_output
transform-safety-report|psionic-ir|lib||tests::transform_safety_report_tracks_alias_roots_and_export_barriers
functionalize-alias-metadata|psionic-ir|lib||tests::functionalize_export_safe_graph_preserves_alias_metadata
functionalize-opaque-refusal|psionic-ir|lib||tests::functionalize_export_safe_policy_refuses_opaque_backend_extensions
custom-meta-tensor-family|psionic-ir|lib||tests::custom_meta_tensor_contract_accepts_non_dense_and_storage_aware_families
meta-family-capability-refusal|psionic-ir|lib||tests::meta_capability_profile_refuses_unsupported_non_dense_family_contract
operator-registry-plan-validation|psionic-ir|lib||tests::operator_registry_validates_execution_plan_specs
meta-executor-graph|psionic-ir|lib||tests::meta_executor_runs_graph_without_real_tensor_data
meta-executor-capability-refusal|psionic-ir|lib||tests::meta_executor_refuses_missing_backend_kernel_capability
compile-graph-step-list|psionic-compiler|lib||tests::compile_graph_lists_expected_steps
compile-plan-meta-execution|psionic-compiler|lib||tests::compile_graph_plan_can_run_through_meta_execution_without_tensor_data
compile-graph-deterministic-digest|psionic-compiler|lib||tests::compile_graph_preserves_deterministic_digest
compiler-artifacts-surface|psionic-compiler|lib||tests::compile_graph_artifacts_surface_schedule_fusion_and_memory_contracts
compiler-cache-identity|psionic-compiler|lib||tests::compile_graph_artifacts_cache_identity_tracks_topology_and_contract_changes
compiler-plan-cache-evidence|psionic-compiler|lib||tests::compiler_plan_cache_emits_cold_compile_then_warm_reuse_evidence
replay-fixture-matmul-add|psionic-compiler|test|process_replay|matmul_add_replay_fixture_matches
replay-fixture-attention-topology|psionic-compiler|test|process_replay|attention_backend_extension_tensor_sharded_replay_fixture_matches
EOF
      ;;
    memory-cache)
      cat <<'EOF'
compiler-artifacts-memory-plan|psionic-compiler|lib||tests::compile_graph_artifacts_surface_schedule_fusion_and_memory_contracts
compiler-cache-identity-memory|psionic-compiler|lib||tests::compile_graph_artifacts_cache_identity_tracks_topology_and_contract_changes
compiler-plan-cache-cold-warm|psionic-compiler|lib||tests::compiler_plan_cache_emits_cold_compile_then_warm_reuse_evidence
runtime-admission-eviction|psionic-runtime|lib||tests::model_admission_can_evict_oldest_idle_model_to_fit_budget
runtime-admission-refusal|psionic-runtime|lib||tests::model_admission_refuses_when_only_active_models_block_the_budget
prefix-cache-identity|psionic-runtime|lib||tests::prefix_cache_identity_and_policy_serialize_stably
kv-cache-state-growth|psionic-runtime|lib||tests::kv_cache_state_and_growth_serialize_stably
EOF
      ;;
    replay-identity)
      cat <<'EOF'
replay-truth-receipt|psionic-train|lib||replay_truth::tests::replay_truth_receipt_is_machine_legible_and_verifiable
replay-truth-verification|psionic-train|lib||replay_truth::tests::replay_truth_verification_detects_seed_tool_and_order_drift
replay-fixture-matmul-add|psionic-compiler|test|process_replay|matmul_add_replay_fixture_matches
replay-fixture-attention-topology|psionic-compiler|test|process_replay|attention_backend_extension_tensor_sharded_replay_fixture_matches
EOF
      ;;
    local-multi-device)
      cat <<'EOF'
decoder-family-sharding-contract|psionic-models|lib||sharding::tests::gguf_decoder_family_tensor_parallel_contract_is_declarative_and_inspectable
local-runner-tensor-sharded|psionic-runtime|lib||local_multi_device::tests::local_multi_device_plan_runner_executes_tensor_sharded_workload_without_cluster_truth
local-sharding-mismatch-refusal|psionic-runtime|lib||local_multi_device::tests::local_sharding_contract_refuses_backend_memory_and_device_count_mismatches
local-topology-refusal|psionic-runtime|lib||local_multi_device::tests::local_sharding_contract_refusal_taxonomy_surfaces_topology_mismatch
compiler-topology-digest|psionic-compiler|lib||tests::compile_graph_with_topology_changes_digest_when_sharding_changes
EOF
      ;;
    *)
      echo "unknown category: $1" >&2
      exit 1
      ;;
  esac
}

run_filtered_test() {
  local category="$1"
  local hook_name="$2"
  local package="$3"
  local target_kind="$4"
  local target_name="$5"
  local filter="$6"
  local -a cmd
  case "${target_kind}" in
    lib)
      cmd=(cargo test -p "${package}" --lib "${filter}" -- --exact --nocapture)
      ;;
    test)
      cmd=(cargo test -p "${package}" --test "${target_name}" "${filter}" -- --exact --nocapture)
      ;;
    *)
      echo "unknown target kind: ${target_kind}" >&2
      exit 1
      ;;
  esac

  local command_str
  printf -v command_str '%q ' "${cmd[@]}"
  command_str="${command_str% }"

  echo
  echo "==> [${category}/${hook_name}] ${command_str}"
  if "${cmd[@]}"; then
    write_tsv_line "${hook_results_tsv}" "${category}" "${hook_name}" "${package}" "${target_kind}" "${target_name}" "${filter}" "${command_str}" "passed"
    return 0
  fi

  overall_result="failed"
  write_tsv_line "${hook_results_tsv}" "${category}" "${hook_name}" "${package}" "${target_kind}" "${target_name}" "${filter}" "${command_str}" "failed"
  return 1
}

run_note() {
  echo
  echo "==> $1"
}

emit_report() {
  [[ -z "${report_path}" ]] && return 0

  mkdir -p "$(dirname "${report_path}")"
  python3 - "${report_path}" "${report_schema}" "${overall_result}" "${repo_root}" "${category_meta_tsv}" "${hook_results_tsv}" "${selected_categories[@]}" <<'PY'
import csv
import json
import pathlib
import subprocess
import sys

report_path = pathlib.Path(sys.argv[1])
schema_path = sys.argv[2]
overall_result = sys.argv[3]
repo_root = pathlib.Path(sys.argv[4])
category_meta_path = pathlib.Path(sys.argv[5])
hook_results_path = pathlib.Path(sys.argv[6])
selected_categories = sys.argv[7:]

git_head = subprocess.check_output(
    ["git", "-C", str(repo_root), "rev-parse", "HEAD"],
    text=True,
).strip()

category_meta = {}
with category_meta_path.open(newline="") as handle:
    reader = csv.reader(handle, delimiter="\t")
    for row in reader:
        if not row:
            continue
        name, status, summary, boundary = row
        category_meta[name] = {
            "name": name,
            "matrix_status": status,
            "summary": summary,
            "boundary_note": boundary,
            "hooks": [],
        }

with hook_results_path.open(newline="") as handle:
    reader = csv.reader(handle, delimiter="\t")
    for row in reader:
        if not row:
            continue
        category, hook_name, package, target_kind, target_name, filter_name, command, result = row
        category_meta[category]["hooks"].append(
            {
                "name": hook_name,
                "package": package,
                "target_kind": target_kind,
                "target_name": target_name or None,
                "filter": filter_name,
                "command": command,
                "result": result,
            }
        )

report = {
    "schema_version": 1,
    "schema_path": schema_path,
    "runner": "scripts/release/check-psionic-framework-core-acceptance.sh",
    "git_head": git_head,
    "overall_result": overall_result,
    "selected_categories": selected_categories,
    "categories": [category_meta[name] for name in selected_categories],
}

with report_path.open("w", encoding="utf-8") as handle:
    json.dump(report, handle, indent=2, sort_keys=False)
    handle.write("\n")
PY
}

run_category() {
  local category="$1"
  local matrix_status
  local summary
  local boundary_note
  matrix_status="$(category_status "${category}")"
  summary="$(category_summary "${category}")"
  boundary_note="$(category_boundary_note "${category}")"
  write_tsv_line "${category_meta_tsv}" "${category}" "${matrix_status}" "${summary}" "${boundary_note}"

  local category_failed=0
  while IFS='|' read -r hook_name package target_kind target_name filter; do
    [[ -z "${hook_name}" ]] && continue
    if ! run_filtered_test "${category}" "${hook_name}" "${package}" "${target_kind}" "${target_name}" "${filter}"; then
      category_failed=1
    fi
  done < <(category_hooks "${category}")

  case "$1" in
    tensor-semantics)
      ;;
    autodiff-optimizer)
      ;;
    model-state-io)
      ;;
    compiler-realize)
      ;;
    memory-cache)
      ;;
    replay-identity)
      ;;
    local-multi-device)
      ;;
    *)
      echo "unknown category: $1" >&2
      exit 1
      ;;
  esac

  run_note "matrix-status: ${matrix_status}"
  run_note "summary: ${summary}"
  run_note "boundary: ${boundary_note}"

  return "${category_failed}"
}

echo "Running Psionic framework-core acceptance matrix from ${repo_root}"
echo "Selected categories: ${selected_categories[*]}"

for category in "${selected_categories[@]}"; do
  run_note "category: ${category}"
  run_category "${category}" || true
done

emit_report

echo
echo "Psionic framework-core acceptance hooks completed."
echo "Overall result: ${overall_result}"
if [[ -n "${report_path}" ]]; then
  echo "Acceptance report: ${report_path}"
  echo "Acceptance schema: ${report_schema}"
fi
echo "No framework-core categories are currently marked partial in the canonical matrix."
echo "Implemented-early boundaries remain intentional."
echo "Do not treat this runner as proof of sweeping Tinygrad-class closure without broader operator-family coverage."

if [[ "${overall_result}" != "passed" ]]; then
  exit 1
fi
