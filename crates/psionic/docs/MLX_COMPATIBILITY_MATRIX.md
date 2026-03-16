# Psionic MLX Compatibility Matrix

> Status: canonical `PMLX-005` / `#3833` reference record, updated 2026-03-16
> after landing `PMLX-101` / `#3834`, `PMLX-102` / `#3835`,
> `PMLX-103` / `#3836`, `PMLX-104` / `#3837`, `PMLX-105` / `#3838`, and
> `PMLX-402` / `#3854`, and refreshing the native function-export bridge row
> to `supported`.

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
| `seeded_transform_compile_export_parity_anchors` | `supported` | Seeded parity anchors now exist for autograd, `vmap`, `custom_vjp`, compile, and export/import. | none | Seeded family anchors are evidence, not blanket MLX-class API closure. |
| `graph_first_function_export_bridge` | `supported` | `psionic-function-io` now exposes one supported native graph-first function export bridge through digest-bound `.psifn` artifacts that bind export-safe graphs to optional compiler artifacts, trace-family identity, deployment contracts, and stable import/export receipts. | `PMLX-403` | This supported bridge is Psionic-native `.psifn`, not current `.mlxfn` support. |
| `portable_model_io_bridge` | `convertible` | Portable model IO through safetensors manifests and GGUF import can support later bounded MLX migration paths. | none | Portable model IO is not the same thing as native MLX weight or module-state compatibility. |
| `module_state_tree_bridge` | `supported` | A public module tree with explicit trainable versus frozen posture, deterministic module state-tree and state-dict contracts, and bounded public `save_weights` / `load_weights` behavior now exists in `psionic-nn`. | none | This supported module-state bridge is still bounded to Psionic-native module naming and receipts; it does not imply general array file IO or broad external MLX artifact compatibility. |
| `public_mlx_array_api` | `supported` | `psionic-array` now exposes a first public lazy-array facade with runtime-backed device handles, honest unified-memory flags, explicit stream-dependency policy, graph-backed arithmetic, explicit `eval` / deferred `async_eval`, replay-stable eval receipts, explicit-only materialization boundaries, scalar and filled-array creation helpers, bounded `reshape` / `permute` / `transpose` / `slice` / `select` / `concat` / `broadcast_to` families, explicit seeded or best-effort random creation, logical dtype casts, `arange` / `linspace` / `eye` helpers, explicit host-owned typed buffer export, singleton `item()` extraction, and deterministic tree flatten/map/unflatten utilities; the companion `psionic-array-io` crate now adds stable `npy` / `npz` / `safetensors` plus bounded dense GGUF save/load with explicit receipt inventory and GGUF quantization-to-dense import disclosure. | none | This is a bounded supported early array surface; it does not imply MLX transform, `nn`, native function export, or distributed support. |
| `public_mlx_transform_api` | `supported` | `psionic-ir` now exposes a bounded public transform layer with `grad`, `value_and_grad`, `vjp`, `jvp`, bounded `vmap`, checkpoint replay, and graph-scoped `custom_vjp` hooks above `AutodiffGraph`, while `psionic-compiler` now exposes compile-as-transform with explicit purity, cache, concrete-plan identity, trace-family identity, and debug controls plus honest reshape/expand refusal on the current shapeless boundary. | none | This is a bounded supported public transform surface, not a claim of jacobian support, full symbolic-shape closure, or broad higher-order transform completeness. |
| `public_mlx_nn_optimizer_api` | `supported` | `psionic-nn` now exposes a bounded supported public `nn` surface with a `Module` tree, bounded public `save_weights` / `load_weights`, a CPU-reference core layer surface, CPU-reference losses and initializers, a public optimizer plus scheduler shell, and a first eval-oriented quantized module API through `Module::quantize(...)` plus `QuantizedLinear` and `QuantizedEmbedding`. | none | This is a bounded supported early public `nn` surface, not a claim of broad quantized training closure, quantized conv/norm wrapper breadth, or export-format quantization parity. |
| `mlxfn_interop` | `unsupported` | There is no `.mlxfn` import or export support in Psionic today. | `PMLX-403` | Native graph-first export substrate does not imply `.mlxfn` compatibility. |
| `mlx_naming_facade_and_bindings` | `unsupported` | There is no MLX naming facade or Python/C/Swift binding layer in Psionic today. | `PMLX-606`, `PMLX-607`, `PMLX-608` | Adoption-facing names and bindings are explicitly late work and must not be implied early. |
| `public_mlx_distributed_api` | `unsupported` | There is no public MLX-class distributed group and helper API in Psionic today. | `PMLX-501` through `PMLX-507` | Current collectives and cluster internals are not themselves a supported MLX distributed surface. |
| `mlx_package_ecosystem` | `unsupported` | There is no supported MLX-lm, multimodal, audio, serving, recipe, or benchmark ecosystem layer in Psionic today. | `PMLX-701` through `PMLX-709` | Ecosystem workflows are intentionally later and must not be implied by the current governance slice. |

## Why This Matters

This closes the last Epic 0 governance requirement in `ROADMAP_MLX.md`:

- compatibility language is explicit and bounded
- one supported, convertible, and unsupported matrix now exists

That means the MLX governance slice is now complete.
