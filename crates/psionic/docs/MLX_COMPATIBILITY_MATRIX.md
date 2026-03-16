# Psionic MLX Compatibility Matrix

> Status: canonical `PMLX-005` / `#3833` reference record, updated 2026-03-16
> after landing `PMLX-101` / `#3834`, `PMLX-102` / `#3835`,
> `PMLX-103` / `#3836`, `PMLX-104` / `#3837`, and `PMLX-105` / `#3838` in
> `psionic-array` and refreshing the public-array row while keeping it
> `convertible`.

This document is the bounded adoption matrix for the Psionic MLX roadmap.

It answers a different question than the acceptance matrix or parity harness:

- not "what categories must eventually close?"
- not "which upstream families do we currently seed with pass/refusal/unsupported?"

It answers:

> what can Psionic honestly call supported today, what is only convertible
> through bounded bridges, and what remains intentionally unsupported?

## Canonical Runner

Run the matrix from the repo root:

```bash
scripts/release/check-psionic-mlx-compatibility-matrix.sh
```

Write the machine-readable report:

```bash
scripts/release/check-psionic-mlx-compatibility-matrix.sh \
  --report /tmp/psionic-mlx-compatibility-matrix.json
```

Target one or more surfaces:

```bash
scripts/release/check-psionic-mlx-compatibility-matrix.sh --only governance_contracts
scripts/release/check-psionic-mlx-compatibility-matrix.sh --only graph_first_function_export_bridge --only public_mlx_array_api
```

The report schema lives at
`crates/psionic/docs/mlx_compatibility_matrix_report.schema.json`.

## Frozen Oracle Window

The matrix is tied to the same bounded MLX oracle window as the other governance
docs:

- `ml-explore/mlx`
- `v0.31.0` through `v0.31.1`

## Matrix

| Surface | Current posture | Evidence / current truth | Blocking issues | Boundary note |
| --- | --- | --- | --- | --- |
| `governance_contracts` | `supported` | `MlxCompatibilityScopeReport`, `MlxAcceptanceMatrixReport`, and `MlxParityHarnessReport` are all repo-owned and runnable today. | none | Governance support is not framework API closure. |
| `seeded_transform_compile_export_parity_anchors` | `supported` | Seeded parity anchors now exist for autograd, compile, export/import, and explicit higher-order-transform refusal. | none | Seeded family anchors are evidence, not blanket MLX-class API closure. |
| `graph_first_function_export_bridge` | `convertible` | Psionic-native exportable-graph and deployment-artifact contracts exist and can support later bounded MLX function compatibility work. | `PMLX-402`, `PMLX-403` | This is a native bridge substrate, not current `.mlxfn` support. |
| `portable_model_io_bridge` | `convertible` | Portable model IO through safetensors manifests and GGUF import can support later bounded MLX migration paths. | `PMLX-302`, `PMLX-401` | Portable model IO is not the same thing as native MLX weight or module-state compatibility. |
| `module_state_tree_bridge` | `convertible` | Deterministic module state-tree and state-dict contracts already exist in `psionic-nn`. | `PMLX-301`, `PMLX-302` | Bridge substrate does not mean public MLX `save_weights` / `load_weights` support exists. |
| `public_mlx_array_api` | `supported` | `psionic-array` now exposes a first public lazy-array facade with runtime-backed device handles, honest unified-memory flags, explicit stream-dependency policy, graph-backed arithmetic, explicit `eval` / deferred `async_eval`, replay-stable eval receipts, explicit-only materialization boundaries, scalar and filled-array creation helpers, bounded `reshape` / `permute` / `transpose` / `slice` / `select` / `concat` / `broadcast_to` families, explicit seeded or best-effort random creation, logical dtype casts, `arange` / `linspace` / `eye` helpers, explicit host-owned typed buffer export, singleton `item()` extraction, and deterministic tree flatten/map/unflatten utilities. | none | This is a bounded supported early array surface; it does not imply MLX transform, `nn`, export, or distributed support. |
| `public_mlx_transform_api` | `convertible` | `psionic-ir` now exposes a first public reverse-mode transform layer with `grad`, `value_and_grad`, and `vjp` objects above `AutodiffGraph`, but forward-mode, higher-order transforms, and compile-as-transform are still incomplete. | `PMLX-202` through `PMLX-206` | The first public reverse-mode slice is not yet the same thing as full supported MLX transform closure. |
| `public_mlx_nn_optimizer_api` | `unsupported` | No public MLX-class `nn`, loss, initializer, optimizer, or scheduler API exists today. | `PMLX-301` through `PMLX-307` | Current `psionic-nn` and `psionic-train` primitives are not themselves a supported MLX public `nn` surface. |
| `mlxfn_interop` | `unsupported` | There is no `.mlxfn` import or export support in Psionic today. | `PMLX-402`, `PMLX-403` | Native graph-first export substrate does not imply `.mlxfn` compatibility. |
| `mlx_naming_facade_and_bindings` | `unsupported` | There is no MLX naming facade or Python/C/Swift binding layer in Psionic today. | `PMLX-606`, `PMLX-607`, `PMLX-608` | Adoption-facing names and bindings are explicitly late work and must not be implied early. |
| `public_mlx_distributed_api` | `unsupported` | There is no public MLX-class distributed group and helper API in Psionic today. | `PMLX-501` through `PMLX-507` | Current collectives and cluster internals are not themselves a supported MLX distributed surface. |
| `mlx_package_ecosystem` | `unsupported` | There is no supported MLX-lm, multimodal, audio, serving, recipe, or benchmark ecosystem layer in Psionic today. | `PMLX-701` through `PMLX-709` | Ecosystem workflows are intentionally later and must not be implied by the current governance slice. |

## Why This Matters

This closes the last Epic 0 governance requirement in `ROADMAP_MLX.md`:

- compatibility language is explicit and bounded
- one supported, convertible, and unsupported matrix now exists

That means the MLX governance slice is now complete.
