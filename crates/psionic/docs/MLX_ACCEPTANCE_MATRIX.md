# Psionic MLX Acceptance Matrix

> Status: canonical `PMLX-003` / `#3831` reference record, updated 2026-03-16
> after landing `PMLX-101` / `#3834`, `PMLX-102` / `#3835`,
> `PMLX-103` / `#3836`, `PMLX-104` / `#3837`, and `PMLX-105` / `#3838` in
> `psionic-array` and
> refreshing the machine-readable report to keep the array/runtime category
> `partial`.

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
| `array-runtime-surface` | `implemented_early` | Public lazy arrays exist with explicit evaluation, device and stream semantics, creation and view families, random or cast behavior, and host-materialization boundaries. | `psionic-array` now publishes a first user-facing lazy-array facade with runtime-backed device handles, honest unified-memory flags, explicit stream-dependency policy, graph-backed arithmetic, explicit `eval` / deferred `async_eval`, replay-stable eval receipts, scalar and filled-array creation helpers, `reshape` / `permute` / `transpose` / `slice` / `select` / `concat` / `broadcast_to` view families, explicit seeded or best-effort random-uniform and random-normal creation, logical dtype casts, `arange` / `linspace` / `eye` helpers, explicit host-owned typed buffer export, singleton `item()` extraction, and deterministic tree flatten/map/unflatten utilities. | `PMLX-101` through `PMLX-106` | Treat this as supported early array/runtime closure only; it does not imply transform, `nn`, export, or distributed MLX closure. |
| `transform-compile` | `partial` | Public transforms cover `grad`, `value_and_grad`, `vjp`, `jvp`, `vmap`, `checkpoint`, and compile-as-transform with typed refusals and symbolic boundaries. | `psionic-ir` now exposes a first public transform layer with stable `grad`, `value_and_grad`, `vjp`, `jvp`, `vmap`, `checkpoint`, and graph-registry-backed `custom_vjp` objects above `AutodiffGraph`, plus typed target validation, zero-cotangent materialization for disconnected reverse-mode targets, dense `f32` tangent propagation for primitive forward-mode graphs, per-lane reference vectorization over selected graph inputs, checkpoint replay that retains only the requested output then replays backward-plan primal bindings, graph-digest plus reverse-signature keyed transform-hook registration, and explicit cast/backend-extension refusal for the current `vmap` and checkpoint surfaces, but jacobian and compile-as-transform remain open. | `PMLX-201` through `PMLX-206` | Do not infer full MLX transform closure from the current reverse-plus-forward plus bounded-`vmap` plus checkpoint slice; jacobian and compile contracts remain open, and `custom_vjp` is still graph-scoped rather than a broad plugin-distribution story. |
| `nn-optimizer` | `planned` | Public `Module`, state save/load, core layers, losses, initializers, optimizers, schedulers, and quantized-module behavior exist above Psionic-native train primitives. | Psionic already owns module, optimizer, and checkpoint substrate, but not the MLX-class `nn` and optimizer shell. | `PMLX-301` through `PMLX-307` | Do not claim MLX `nn` closure from `psionic-nn` or `psionic-train` alone. |
| `export-serialization-tooling` | `planned` | General array IO, native function export/import, bounded `.mlxfn` interop, memory controls, debug hooks, and extension tooling are public. | Model IO, graph export, compiler artifacts, and runtime diagnostics exist, but not the public MLX tooling shell. | `PMLX-401` through `PMLX-406` | Do not collapse internal model IO or runtime diagnostics into MLX export closure. |
| `distributed-semantics` | `planned` | Framework-visible distributed groups, collectives, gradient helpers, tensor-parallel helpers, FSDP-style helpers, and launch or topology tooling are public. | Collectives, cluster, and distributed optimizer substrate exist, but not the MLX framework-distributed API. | `PMLX-501` through `PMLX-507` | Do not infer MLX distributed closure from lower-level collectives alone. |
| `backend-closure` | `planned` | CPU, Metal, and CUDA honestly cover the declared MLX surface, the parity harness carries upstream MLX test families, and compatibility or binding shells remain bounded. | Backend-specific substrate and the frozen MLX version window contract exist, but backend closure and parity evidence do not. | `PMLX-601` through `PMLX-608` plus `PMLX-004` and `PMLX-005` | Do not claim bounded MLX closure from one backend lane, one demo, or one local checkout. |

## Why This Matters

This issue closes the second governance requirement in `ROADMAP_MLX.md`:

- one MLX acceptance matrix exists

Epic 0 governance is now complete because the version-window contract, parity
harness, and supported/convertible/unsupported compatibility matrix exist too.
