# Psionic MLX Parity Harness

> Status: canonical `PMLX-004` / `#3832` reference record, updated 2026-03-16
> after landing `PMLX-101` / `#3834`, `PMLX-102` / `#3835`,
> `PMLX-103` / `#3836`, `PMLX-104` / `#3837`, `PMLX-105` / `#3838`, and
> `PMLX-106` / `#3839` in `psionic-array`, `PMLX-402` / `#3854` in
> `psionic-function-io`, `PMLX-403` / `#3855` in `psionic-function-io`, plus
> `PMLX-404` / `#3856` in `psionic-array`, `PMLX-501` / `#3859`, and
> `PMLX-502` / `#3860` in `psionic-distributed`, plus `PMLX-201` / `#3840`,
> `PMLX-202` / `#3841`, and `PMLX-203` / `#3842` in `psionic-ir`, and
> refreshing the seeded basic-array family notes after `PMLX-601` / `#3866`
> without overclaiming full parity.

This document defines the seeded upstream MLX test families that Psionic uses
to ground later MLX parity claims.

It is intentionally bounded.

The current harness is not "all upstream MLX tests in Rust."

It is the first repo-owned runner and report that say:

- which upstream MLX test families are being mirrored
- which ones currently map to a seeded Psionic `pass`
- which ones currently map to an explicit `refusal`
- which ones remain honestly `unsupported`

## Canonical Runner

Run the harness from the repo root:

```bash
scripts/release/check-psionic-mlx-parity-harness.sh
```

Write the machine-readable report:

```bash
scripts/release/check-psionic-mlx-parity-harness.sh \
  --report /tmp/psionic-mlx-parity-harness.json
```

Target one or more families:

```bash
scripts/release/check-psionic-mlx-parity-harness.sh --only autograd
scripts/release/check-psionic-mlx-parity-harness.sh --only compile --only export_import
```

The report schema lives at
`crates/psionic/docs/mlx_parity_harness_report.schema.json`.

## Frozen Oracle Window

The seeded harness is explicitly tied to the frozen MLX oracle window from
`MLX_COMPATIBILITY_SCOPE.md`:

- `ml-explore/mlx`
- `v0.31.0` through `v0.31.1`

## Seeded Families

| Family | Current outcome | Upstream sources | Current Psionic anchor | Boundary note |
| --- | --- | --- | --- | --- |
| `array_core` | `unsupported` | `tests/array_tests.cpp`, `python/tests/test_array.py`, `python/tests/test_constants.py`, `python/tests/test_bf16.py`, `python/tests/test_double.py` | `cargo test -p psionic-array tests::public_lazy_array_surface_builds_graph_backed_arithmetic -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_creation_and_view_families_materialize -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_random_cast_and_common_creation_families_stay_seeded -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_host_interop_and_item_access_stay_explicit -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_tree_utilities_preserve_structure_and_refuse_bad_unflatten -- --exact --nocapture`; `cargo test -p psionic-array tests::mlx_cpu_reference_coverage_report_tracks_seeded_supported_and_refused_cases -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_metal_eval_refuses_ops_outside_bounded_surface_when_hardware_available -- --exact --nocapture` | The upstream array-core family is still not a parity-harness pass, but `psionic-array` now also has a bounded CPU-reference coverage report over flatten, expand_dims, squeeze, slice, select, concat, and transpose cases plus a bounded Metal lane that refuses reshape-backed view materialization explicitly outside the current dense execution slice. |
| `ops_numeric` | `unsupported` | `tests/ops_tests.cpp`, `tests/creations_tests.cpp`, `tests/arg_reduce_tests.cpp`, `tests/einsum_tests.cpp`, `tests/random_tests.cpp`, `python/tests/test_ops.py`, `python/tests/test_reduce.py`, `python/tests/test_einsum.py`, `python/tests/test_random.py` | `cargo test -p psionic-array tests::public_lazy_array_surface_builds_graph_backed_arithmetic -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_creation_and_view_families_materialize -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_random_cast_and_common_creation_families_stay_seeded -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_host_interop_and_item_access_stay_explicit -- --exact --nocapture`; `cargo test -p psionic-array tests::mlx_cpu_reference_coverage_report_tracks_seeded_supported_and_refused_cases -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_metal_eval_executes_bounded_dense_surface_when_hardware_available -- --exact --nocapture` | The numeric-op and creation families are still not a parity-harness pass, but `psionic-array` now also has a bounded CPU-reference coverage report over dense add/mul/matmul, axis-aware sum, seeded random-plus-cast, helper creation families, and typed refusal for incompatible dense matmul shapes, plus a bounded actual Metal dense `add`/`matmul` execution path on the selected runtime device. |
| `device_eval_memory` | `unsupported` | `tests/device_tests.cpp`, `tests/eval_tests.cpp`, `tests/allocator_tests.cpp`, `tests/gpu_tests.cpp`, `tests/scheduler_tests.cpp`, `python/tests/test_device.py`, `python/tests/test_eval.py`, `python/tests/test_memory.py` | `cargo test -p psionic-array tests::public_lazy_array_device_handles_preserve_unified_memory_truth -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_streams_report_dependency_policy_honestly -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_runtime_resource_report_tracks_active_peak_and_cache_counters -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_cache_limit_controls_clamp_and_reset_runtime_resources -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_backend_debug_support_and_snapshot_track_seeded_lanes -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_backend_debug_capture_emits_receipt_logs_and_artifact -- --exact --nocapture`; `cargo test -p psionic-array tests::mlx_cpu_reference_coverage_report_tracks_seeded_supported_and_refused_cases -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_metal_eval_executes_bounded_dense_surface_when_hardware_available -- --exact --nocapture` | The device/eval/memory family is still not a parity-harness pass, but `psionic-array` now also has a bounded CPU-reference coverage report over explicit eval, async-eval wait, cache counters, backend-debug capture receipts, and typed refusal for unsupported capture formats, plus availability-aware Metal eval receipts that preserve selected device and explicit stream identity on the bounded dense surface. |
| `autograd` | `pass` | `tests/autograd_tests.cpp`, `python/tests/test_autograd.py` | `cargo test -p psionic-ir autodiff::tests::public_reverse_mode_transforms_expose_grad_value_and_grad_and_vjp -- --exact --nocapture`; `cargo test -p psionic-ir autodiff::tests::public_forward_mode_jvp_exposes_value_and_tangent -- --exact --nocapture`; `cargo test -p psionic-ir autodiff::tests::reverse_mode_autodiff_materializes_matmul_chain_gradients -- --exact --nocapture` | This is a seeded reverse-plus-forward pass, not proof that the full public MLX transform API is complete. |
| `vmap` | `pass` | `tests/vmap_tests.cpp`, `python/tests/test_vmap.py` | `cargo test -p psionic-ir autodiff::tests::public_vmap_transform_batches_reference_graph_outputs -- --exact --nocapture`; `cargo test -p psionic-ir tests::program_transform_capability_matrix_tracks_seeded_transform_and_future_cases -- --exact --nocapture` | This is a bounded public `vmap` pass, not proof that jacobian or compile-as-transform are complete. |
| `custom_vjp` | `pass` | `tests/custom_vjp_tests.cpp` | `cargo test -p psionic-ir autodiff::tests::public_custom_vjp_transform_uses_registered_rule -- --exact --nocapture`; `cargo test -p psionic-ir autodiff::tests::custom_vjp_registry_and_transform_refuse_missing_and_duplicate_rules -- --exact --nocapture` | This is a bounded graph-scoped `custom_vjp` pass, not proof that jacobian or compile-as-transform are complete. |
| `compile` | `pass` | `tests/compile_tests.cpp`, `python/tests/test_compile.py`, `python/tests/test_graph.py` | `cargo test -p psionic-compiler tests::compiler_hygiene_parity_matrix_tracks_seeded_supported_and_refusal_cases -- --exact --nocapture`; `cargo test -p psionic-compiler tests::compile_transform_emits_cold_then_warm_cache_hits_with_trace_and_debug -- --exact --nocapture`; `cargo test -p psionic-compiler tests::compile_transform_cache_controls_make_bypass_and_invalidation_explicit -- --exact --nocapture`; `cargo test -p psionic-compiler tests::compile_transform_shapeless_trace_family_identity_groups_same_rank_graphs -- --exact --nocapture`; `cargo test -p psionic-compiler tests::compile_transform_shapeless_trace_family_refuses_reshape_without_formula -- --exact --nocapture` | This bounded compile-transform pass now includes a narrow shapeless trace-family identity, but it is still not a full symbolic-shape, dynamic-guard, or broad shape-polymorphic compile claim. |
| `export_import` | `pass` | `tests/export_import_tests.cpp`, `tests/load_tests.cpp`, `python/tests/test_export_import.py`, `python/tests/test_load.py` | `cargo test -p psionic-ir tests::exportable_graph_contract_tracks_entry_signature_and_refuses_opaque_graphs -- --exact --nocapture`; `cargo test -p psionic-train model_io::tests::portable_model_bundle_roundtrips_through_safetensors_manifest -- --exact --nocapture`; `cargo test -p psionic-array-io -- --nocapture`; `cargo test -p psionic-function-io -- --nocapture` | This is a bounded export/import seed that now includes the current `.mlxfn` subset, not full MLX tooling closure. |
| `nn_optimizers_quantized` | `pass` | `python/tests/test_nn.py`, `python/tests/test_losses.py`, `python/tests/test_init.py`, `python/tests/test_optimizers.py`, `python/tests/test_quantized.py`, `python/tests/test_tree.py` | `cargo test -p psionic-nn layers::tests::linear_forward_applies_affine_projection -- --exact --nocapture`; `cargo test -p psionic-nn layers::tests::embedding_lookup_preserves_index_shape_and_bounds -- --exact --nocapture`; `cargo test -p psionic-nn layers::tests::conv2d_and_pool2d_match_reference_windows -- --exact --nocapture`; `cargo test -p psionic-nn training::tests::classification_losses_and_helpers_match_reference -- --exact --nocapture`; `cargo test -p psionic-nn optimizers::tests::module_optimizer_updates_trainable_parameters_and_ignores_frozen_gradients -- --exact --nocapture`; `cargo test -p psionic-nn optimizers::tests::module_optimizer_scheduler_and_parameter_semantics_scale_effective_rates -- --exact --nocapture`; `cargo test -p psionic-nn optimizers::tests::multi_optimizer_composes_disjoint_groups_and_refuses_overlap_or_unassigned_paths -- --exact --nocapture`; `cargo test -p psionic-nn quantized::tests::module_quantize_reports_quantized_and_dense_paths_and_freezes_eval_copy -- --exact --nocapture`; `cargo test -p psionic-nn quantized::tests::quantized_linear_forward_tracks_dense_reference -- --exact --nocapture`; `cargo test -p psionic-nn quantized::tests::quantized_embedding_lookup_tracks_dense_reference -- --exact --nocapture`; `cargo test -p psionic-nn quantized::tests::quantized_linear_roundtrips_through_module_state_load -- --exact --nocapture` | `psionic-nn` now exposes a first bounded quantized `nn` slice on the MLX public surface, including `Module::quantize(...)` with explicit keep-dense versus strict posture, eval-only frozen quantized modules, and `QuantizedLinear` plus `QuantizedEmbedding` wrappers backed by `int8_symmetric` block storage and CPU-reference dequantize-to-`f32` forward checks. |
| `distributed` | `unsupported` | `python/tests/ring_test_distributed.py`, `python/tests/mpi_test_distributed.py`, `python/tests/nccl_test_distributed.py`, `python/tests/mlx_distributed_tests.py` | `cargo test -p psionic-distributed tests::init_bootstraps_one_group_and_reuses_it_as_the_global_group -- --exact --nocapture`; `cargo test -p psionic-distributed tests::named_backend_families_map_onto_current_topology_profiles -- --exact --nocapture`; `cargo test -p psionic-distributed tests::named_backend_families_refuse_incompatible_topologies -- --exact --nocapture`; `cargo test -p psionic-distributed tests::split_uses_one_explicit_plan_and_reassigns_rank_by_key -- --exact --nocapture`; `cargo test -p psionic-distributed tests::all_sum_respects_singleton_passthrough_and_multi_rank_reference_inputs -- --exact --nocapture`; `cargo test -p psionic-distributed tests::all_gather_handles_vector_and_scalar_payloads -- --exact --nocapture`; `cargo test -p psionic-distributed tests::reduce_scatter_sums_then_slices_local_chunk -- --exact --nocapture`; `cargo test -p psionic-distributed tests::send_and_recv_cover_validation_and_reference_payload_paths -- --exact --nocapture`; `cargo test -p psionic-distributed tests::parse_hostfile_accepts_comments_slots_and_addresses -- --exact --nocapture`; `cargo test -p psionic-distributed tests::plan_launch_emits_cluster_evidence_rank_assignments_and_reserved_environment -- --exact --nocapture`; `cargo test -p psionic-distributed tests::plan_launch_maps_named_backend_families_and_refuses_missing_profiles -- --exact --nocapture`; `cargo test -p psionic-distributed tests::plan_launch_refuses_configs_that_would_fail_the_sandbox_contract -- --exact --nocapture`; `cargo test -p psionic-distributed tests::grouped_all_sum_preserves_tree_structure_and_reduces_grouped_leaves -- --exact --nocapture`; `cargo test -p psionic-distributed tests::average_gradients_divides_grouped_reduction_by_world_size -- --exact --nocapture`; `cargo test -p psionic-distributed tests::all_to_sharded_linear_slices_output_rows_into_local_wrappers -- --exact --nocapture`; `cargo test -p psionic-distributed tests::sharded_to_all_linear_reconstructs_full_output_from_rank_wrappers -- --exact --nocapture`; `cargo test -p psionic-distributed tests::tensor_parallel_wrappers_refuse_missing_rank_state_and_tiny_axes -- --exact --nocapture`; `cargo test -p psionic-distributed tests::fsdp_apply_gradients_updates_mixed_full_shard_and_replicated_groups -- --exact --nocapture`; `cargo test -p psionic-distributed tests::fsdp_apply_gradients_applies_global_norm_clip_before_local_update -- --exact --nocapture`; `cargo test -p psionic-distributed tests::fsdp_apply_gradients_refuses_invalid_remote_maps_and_unsupported_layouts -- --exact --nocapture` | `psionic-distributed` now exposes a bounded public group, collective-helper, launch/config planning, tree-aware gradient-reduction, tensor-parallel linear-wrapper, and FSDP-style update-helper surface with singleton passthrough, reference-emulated multi-rank collectives, typed send/recv validation, hostfile parsing, cluster/sandbox-backed per-rank job plans, grouped small-leaf all-reduce, floating-point `average_gradients`, bounded `AllToShardedLinear` / `ShardedToAllLinear`, bounded `fsdp_apply_gradients` receipts and refusals, and explicit backend-family capability mapping plus refusal coverage for `ring`, `mpi`, `nccl`, and `jaccl`-class requests, but the seeded upstream distributed family still lacks transport-backed multi-rank execution needed for a parity pass. |

## Why This Matters

This issue closes the third governance requirement in `ROADMAP_MLX.md`:

- one parity harness entrypoint exists

Epic 0 governance is now complete because the version-window contract,
acceptance matrix, and supported/convertible/unsupported compatibility matrix
exist too.
