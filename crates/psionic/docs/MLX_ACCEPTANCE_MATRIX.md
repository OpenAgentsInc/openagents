# Psionic MLX Acceptance Matrix

> Status: canonical `PMLX-003` / `#3831` reference record, updated 2026-03-16
> after landing `PMLX-101` / `#3834`, `PMLX-102` / `#3835`,
> `PMLX-103` / `#3836`, `PMLX-104` / `#3837`, `PMLX-105` / `#3838`, and
> `PMLX-402` / `#3854`, `PMLX-403` / `#3855`, plus `PMLX-404` / `#3856` in
> `psionic-array` / `psionic-function-io`, plus `PMLX-501` / `#3859` and
> `PMLX-502` / `#3860` in `psionic-distributed`, and
> refreshing the machine-readable report to keep the export/tooling category
> honestly `implemented_early` now that the bounded extension-authoring slice
> has landed.

This document defines the closure categories for the Psionic MLX roadmap.

It is not a claim that the MLX lane is already green.

It is the contract that says which categories have to turn green before later
MLX adoption or compatibility claims become honest.

## Canonical Runner

Run the matrix from the repo root:

```bash
scripts/release/check-psionic-mlx-acceptance-matrix.sh
```

Write the machine-readable report:

```bash
scripts/release/check-psionic-mlx-acceptance-matrix.sh \
  --report /tmp/psionic-mlx-acceptance-matrix.json
```

Target one or more categories:

```bash
scripts/release/check-psionic-mlx-acceptance-matrix.sh --only array-runtime-surface
scripts/release/check-psionic-mlx-acceptance-matrix.sh --only nn-optimizer --only backend-closure
```

The report schema lives at
`crates/psionic/docs/mlx_acceptance_matrix_report.schema.json`.

## Current Posture

The initial machine-readable matrix posture is `tracking_only`.

That means:

- the acceptance contract is now canonical and runnable
- closure must now be discussed by category instead of one-off demos
- most categories are still honestly marked `planned`

## Matrix

| Category | Current status | What a green category would mean | Current repo truth | Governing issues | Boundary note |
| --- | --- | --- | --- | --- | --- |
| `array-runtime-surface` | `implemented_early` | Public lazy arrays exist with explicit evaluation, device and stream semantics, creation and view families, random or cast behavior, and host-materialization boundaries. | `psionic-array` now publishes a first user-facing lazy-array facade with runtime-backed device handles, honest unified-memory flags, explicit stream-dependency policy, graph-backed arithmetic, explicit `eval` / deferred `async_eval`, replay-stable eval receipts, scalar and filled-array creation helpers, `reshape` / `permute` / `transpose` / `slice` / `select` / `concat` / `broadcast_to` view families, explicit seeded or best-effort random-uniform and random-normal creation, logical dtype casts, `arange` / `linspace` / `eye` helpers, explicit host-owned typed buffer export, singleton `item()` extraction, deterministic tree flatten/map/unflatten utilities, and a bounded runtime resource report with explicit active/peak/cache counters plus cache-limit and reset controls above the reference eval substrate. | `PMLX-101` through `PMLX-106` | Treat this as supported early array/runtime closure only; it does not imply transform, `nn`, export, or distributed MLX closure. |
| `transform-compile` | `implemented_early` | Public transforms cover `grad`, `value_and_grad`, `vjp`, `jvp`, `vmap`, `checkpoint`, and compile-as-transform with typed refusals and symbolic boundaries. | `psionic-ir` now exposes a bounded public transform layer with stable `grad`, `value_and_grad`, `vjp`, `jvp`, `vmap`, `checkpoint`, and graph-registry-backed `custom_vjp` objects above `AutodiffGraph`, plus typed target validation, zero-cotangent materialization for disconnected reverse-mode targets, dense `f32` tangent propagation for primitive forward-mode graphs, per-lane reference vectorization over selected graph inputs, checkpoint replay that retains only the requested output then replays backward-plan primal bindings, graph-digest plus reverse-signature keyed transform-hook registration, and explicit cast/backend-extension refusal for the current `vmap` and checkpoint surfaces; `psionic-compiler` now exposes `compile_transform(...)` with explicit enable/disable posture, declared purity, cache reuse versus bypass versus invalidation control, cache-identity versus trace-family trace capture, plan-debug output, and a bounded `shapeless_trace_family` identity with explicit reshape/expand refusal where the current graph model cannot yet carry symbolic formulas. | `PMLX-201` through `PMLX-206` | Do not infer full MLX transform closure from the current reverse-plus-forward plus bounded-`vmap` plus checkpoint plus compile-transform slice; jacobian remains outside the current bounded surface, `shapeless_trace_family` is still narrower than a full symbolic-shape environment, and `custom_vjp` is still graph-scoped rather than a broad plugin-distribution story. |
| `nn-optimizer` | `implemented_early` | Public `Module`, state save/load, core layers, losses, initializers, optimizers, schedulers, and quantized-module behavior exist above Psionic-native train primitives. | `psionic-nn` now exposes a first public `Module` tree with explicit parameter versus buffer registration, trainable versus frozen posture, recursive parameter discovery with filtered trainable or frozen views, deterministic state-tree/state-dict behavior, bounded public `save_weights` / `load_weights` wrappers with strict-by-default plus explicit non-strict load posture, a bounded CPU-reference core layer surface spanning linear, embedding, layer norm, RMS norm, activations, dropout, conv1d, conv2d, pool1d, and pool2d families, bounded CPU-reference losses, initializers, and helpers including `mse_loss`, `l1_loss`, `binary_cross_entropy_loss`, `cross_entropy_loss`, `softmax_last_dim`, `log_softmax_last_dim`, `sigmoid`, `one_hot`, `init_tensor`, and `init_parameter`, plus a bounded public optimizer and scheduler shell with module-path keyed state, scheduler bindings, parameter-group scaling semantics, and multi-optimizer composition built above `psionic-train` optimizer and scheduler primitives; psionic-nn now also carries a first eval-oriented quantized-module shell with `Module::quantize(...)`, explicit keep-dense versus strict quantize reports, and `QuantizedLinear` plus `QuantizedEmbedding` wrappers over `int8_symmetric` block storage and dequantize-to-`f32` forward semantics. | `PMLX-301` through `PMLX-307` | Treat this as supported early `nn` closure only: the public surface now covers registration, freeze posture, module-state save/load semantics, core layer numerics, reusable CPU-reference losses and initializers, a path-keyed optimizer plus scheduler shell with parameter-group composition, and a first eval-oriented quantized wrapper slice for linear and embedding families; it does not yet imply broad quantized training closure, conv/norm quantized wrappers, or export-format quantization parity. |
| `export-serialization-tooling` | `implemented_early` | General array IO, native function export/import, bounded `.mlxfn` interop, memory controls, debug hooks, and extension tooling are public. | `psionic-array-io` now closes the first public array-IO slice with `npy` / `npz` / `safetensors` plus bounded dense GGUF save/load above `psionic-array`, `psionic-function-io` now closes both the first native `.psifn` function-artifact slice with export-safe graphs, optional compiler artifacts, trace-family identity, deployment bundle binding, and stable import/export receipts and a bounded `.mlxfn` import/export shell for one positional CPU trace over the current primitive and dtype subset, and `psionic-array` now exposes bounded public runtime memory reporting with active, peak, and cache counters plus explicit cache-limit and reset controls, plus a public backend-debug layer with lane-specific support matrices for `cpu` / `metal` / `cuda`, retained runtime log events, bounded runtime-observability snapshots, compiler-backed debug captures, optional lane-labeled JSON bundles, and a bounded extension-authoring layer with custom-op, custom-kernel, backend-plugin, and quantizer-plugin registration plus dispatch-resolution and declared-output validation above the extensible operator registry. | `PMLX-402` through `PMLX-406` | Do not collapse internal model IO, runtime diagnostics, or extension-authoring contracts into full MLX execution closure; this category is now implemented early, but custom-op runtime execution, vendor-native profiler capture, and broader plugin distribution remain intentionally bounded. |
| `distributed-semantics` | `implemented_early` | Framework-visible distributed groups, collectives, gradient helpers, tensor-parallel helpers, FSDP-style helpers, and launch or topology tooling are public. | `psionic-distributed` now exposes a bounded public distributed surface above current runtime mesh truth, including explicit mesh bootstrap, reusable global-group init, honest singleton fallback, ordered member/rank snapshots, explicit-plan subgroup split semantics, MLX-style singleton passthrough for `all_sum` / `all_gather` / `reduce_scatter`, explicit host-owned per-rank reference emulation for multi-rank collectives and `recv`, validation-only `send`, typed collective-support snapshots, a bounded launch/config planning shell with hostfile parsing, honest single-rank-per-node validation, cluster membership/address/backend readiness checks, sandbox contract preflight, per-rank bootstrap payloads and sandbox job plans, distributed reserved-environment synthesis, cluster execution evidence, stable plan digests, tree-aware gradient reduction helpers through `grouped_all_sum` / `grouped_all_reduce` and floating-point `average_gradients`, bounded MLX-style `AllToShardedLinear` and `ShardedToAllLinear` wrappers with deterministic row/column sharding, inspectable shard layouts, local shard-input splitting, and reference-emulated multi-rank reconstruction that requires explicit rank wrappers and shard inputs, plus a bounded `fsdp_apply_gradients` helper above distributed optimizer contracts with typed `zero_stage3` admission, mixed replicated/full-shard group handling, explicit remote-rank state and batch maps, optional global-norm clipping, shard-local optimizer updates, gathered parameter reconstruction, and stable apply receipts; it now also publishes explicit backend-family capability mapping for `ring`, `mpi`, `nccl`, and `jaccl`-class requests, with `backend=any` resolving honestly from current topology truth and `jaccl` refusing until a real RDMA-style transport profile exists. | `PMLX-501` through `PMLX-507` | Do not infer transport-backed multi-rank execution or full upstream distributed parity from this category alone; backend-family mapping is now explicit, but collectives remain reference-emulated on the current public Psionic surface. |
| `backend-closure` | `planned` | CPU, Metal, and CUDA honestly cover the declared MLX surface, the parity harness carries upstream MLX test families, and compatibility or binding shells remain bounded. | Backend-specific substrate and the frozen MLX version window contract exist, but backend closure and parity evidence do not. | `PMLX-601` through `PMLX-608` plus `PMLX-004` and `PMLX-005` | Do not claim bounded MLX closure from one backend lane, one demo, or one local checkout. |

## Why This Matters

This issue closes the second governance requirement in `ROADMAP_MLX.md`:

- one MLX acceptance matrix exists

Epic 0 governance is now complete because the version-window contract, parity
harness, and supported/convertible/unsupported compatibility matrix exist too.
