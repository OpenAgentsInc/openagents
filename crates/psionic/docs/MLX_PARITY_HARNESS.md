# Psionic MLX Parity Harness

> Status: canonical `PMLX-004` / `#3832` reference record, updated 2026-03-16
> after landing `PMLX-101` / `#3834`, `PMLX-102` / `#3835`,
> `PMLX-103` / `#3836`, `PMLX-104` / `#3837`, `PMLX-105` / `#3838`, and
> `PMLX-106` / `#3839` in `psionic-array`, plus `PMLX-201` / `#3840` in
> `psionic-ir`, and refreshing the array/autograd anchor notes without
> overclaiming parity.

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
| `array_core` | `unsupported` | `tests/array_tests.cpp`, `python/tests/test_array.py`, `python/tests/test_constants.py`, `python/tests/test_bf16.py`, `python/tests/test_double.py` | `cargo test -p psionic-array tests::public_lazy_array_surface_builds_graph_backed_arithmetic -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_creation_and_view_families_materialize -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_random_cast_and_common_creation_families_stay_seeded -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_host_interop_and_item_access_stay_explicit -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_tree_utilities_preserve_structure_and_refuse_bad_unflatten -- --exact --nocapture` | A first public lazy-array facade with standard creation, deterministic random, cast, host-interop, `item()`, and tree families now exists, but there is still no seeded upstream MLX array-core parity pass. |
| `ops_numeric` | `unsupported` | `tests/ops_tests.cpp`, `tests/creations_tests.cpp`, `tests/arg_reduce_tests.cpp`, `tests/einsum_tests.cpp`, `tests/random_tests.cpp`, `python/tests/test_ops.py`, `python/tests/test_reduce.py`, `python/tests/test_einsum.py`, `python/tests/test_random.py` | `cargo test -p psionic-array tests::public_lazy_array_surface_builds_graph_backed_arithmetic -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_creation_and_view_families_materialize -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_random_cast_and_common_creation_families_stay_seeded -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_host_interop_and_item_access_stay_explicit -- --exact --nocapture` | Graph-backed arithmetic, common creation, deterministic random, logical dtype-cast coverage, and explicit host or singleton export boundaries now exist in `psionic-array`, but they are not yet a seeded upstream MLX numeric parity family. |
| `device_eval_memory` | `unsupported` | `tests/device_tests.cpp`, `tests/eval_tests.cpp`, `tests/allocator_tests.cpp`, `tests/gpu_tests.cpp`, `tests/scheduler_tests.cpp`, `python/tests/test_device.py`, `python/tests/test_eval.py`, `python/tests/test_memory.py` | `cargo test -p psionic-array tests::public_lazy_array_device_handles_preserve_unified_memory_truth -- --exact --nocapture`; `cargo test -p psionic-array tests::public_lazy_array_streams_report_dependency_policy_honestly -- --exact --nocapture` | Public device and stream handles now exist in `psionic-array`, but there is still no MLX-class allocator, scheduler, or runtime-memory parity. |
| `autograd` | `pass` | `tests/autograd_tests.cpp`, `python/tests/test_autograd.py` | `cargo test -p psionic-ir autodiff::tests::public_reverse_mode_transforms_expose_grad_value_and_grad_and_vjp -- --exact --nocapture`; `cargo test -p psionic-ir autodiff::tests::reverse_mode_autodiff_materializes_matmul_chain_gradients -- --exact --nocapture` | This is a seeded reverse-mode pass, not proof that the full public MLX transform API is complete. |
| `vmap_custom_vjp` | `refusal` | `tests/vmap_tests.cpp`, `tests/custom_vjp_tests.cpp`, `python/tests/test_vmap.py` | `cargo test -p psionic-ir tests::program_transform_capability_matrix_tracks_seeded_transform_and_future_cases -- --exact --nocapture` | A typed refusal is honest progress, but not a ported transform family. |
| `compile` | `pass` | `tests/compile_tests.cpp`, `python/tests/test_compile.py`, `python/tests/test_graph.py` | `cargo test -p psionic-compiler tests::compiler_hygiene_parity_matrix_tracks_seeded_supported_and_refusal_cases -- --exact --nocapture` | This is a seeded compile-family pass, not proof that the public MLX compile surface exists. |
| `export_import` | `pass` | `tests/export_import_tests.cpp`, `tests/load_tests.cpp`, `python/tests/test_export_import.py`, `python/tests/test_load.py` | `cargo test -p psionic-ir tests::exportable_graph_contract_tracks_entry_signature_and_refuses_opaque_graphs -- --exact --nocapture`; `cargo test -p psionic-train model_io::tests::portable_model_bundle_roundtrips_through_safetensors_manifest -- --exact --nocapture` | This is a bounded export/import seed, not full MLX tooling closure. |
| `nn_optimizers_quantized` | `unsupported` | `python/tests/test_nn.py`, `python/tests/test_losses.py`, `python/tests/test_init.py`, `python/tests/test_optimizers.py`, `python/tests/test_quantized.py`, `python/tests/test_tree.py` | none yet | Current `psionic-nn` and `psionic-train` internals do not yet equal an MLX-class public `nn` family. |
| `distributed` | `unsupported` | `python/tests/ring_test_distributed.py`, `python/tests/mpi_test_distributed.py`, `python/tests/nccl_test_distributed.py`, `python/tests/mlx_distributed_tests.py` | none yet | Collectives and cluster substrate are not yet an MLX public distributed API. |

## Why This Matters

This issue closes the third governance requirement in `ROADMAP_MLX.md`:

- one parity harness entrypoint exists

Epic 0 governance is now complete because the version-window contract,
acceptance matrix, and supported/convertible/unsupported compatibility matrix
exist too.
