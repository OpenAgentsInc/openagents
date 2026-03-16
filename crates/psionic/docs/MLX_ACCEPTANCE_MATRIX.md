# Psionic MLX Acceptance Matrix

> Status: canonical `PMLX-003` / `#3831` reference record, updated 2026-03-16
> after adding the first MLX-lane acceptance doc, machine-readable report, JSON
> schema, and repo runner.

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
| `array-runtime-surface` | `planned` | Public lazy arrays exist with explicit evaluation, device and stream semantics, creation and view families, random or cast behavior, and host-materialization boundaries. | Lower-layer tensor, IR, runtime, and refusal substrate exist, but Psionic does not yet publish a user-facing MLX-class array facade. | `PMLX-101` through `PMLX-106` | Do not claim MLX array closure from internals alone. |
| `transform-compile` | `planned` | Public transforms cover `grad`, `value_and_grad`, `vjp`, `jvp`, `vmap`, `checkpoint`, and compile-as-transform with typed refusals and symbolic boundaries. | Autodiff and compiler substrate exist, but the coherent MLX-class transform API does not. | `PMLX-201` through `PMLX-206` | Do not infer MLX transform closure from private autodiff helpers or compiler internals. |
| `nn-optimizer` | `planned` | Public `Module`, state save/load, core layers, losses, initializers, optimizers, schedulers, and quantized-module behavior exist above Psionic-native train primitives. | Psionic already owns module, optimizer, and checkpoint substrate, but not the MLX-class `nn` and optimizer shell. | `PMLX-301` through `PMLX-307` | Do not claim MLX `nn` closure from `psionic-nn` or `psionic-train` alone. |
| `export-serialization-tooling` | `planned` | General array IO, native function export/import, bounded `.mlxfn` interop, memory controls, debug hooks, and extension tooling are public. | Model IO, graph export, compiler artifacts, and runtime diagnostics exist, but not the public MLX tooling shell. | `PMLX-401` through `PMLX-406` | Do not collapse internal model IO or runtime diagnostics into MLX export closure. |
| `distributed-semantics` | `planned` | Framework-visible distributed groups, collectives, gradient helpers, tensor-parallel helpers, FSDP-style helpers, and launch or topology tooling are public. | Collectives, cluster, and distributed optimizer substrate exist, but not the MLX framework-distributed API. | `PMLX-501` through `PMLX-507` | Do not infer MLX distributed closure from lower-level collectives alone. |
| `backend-closure` | `planned` | CPU, Metal, and CUDA honestly cover the declared MLX surface, the parity harness carries upstream MLX test families, and compatibility or binding shells remain bounded. | Backend-specific substrate and the frozen MLX version window contract exist, but backend closure and parity evidence do not. | `PMLX-601` through `PMLX-608` plus `PMLX-004` and `PMLX-005` | Do not claim bounded MLX closure from one backend lane, one demo, or one local checkout. |

## Why This Matters

This issue closes the second governance requirement in `ROADMAP_MLX.md`:

- one MLX acceptance matrix exists

The remaining Epic 0 work still needs:

- one supported, convertible, and unsupported compatibility matrix
