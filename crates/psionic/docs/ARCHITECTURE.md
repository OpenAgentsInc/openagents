# Psionic Architecture

Psionic is structured as a layered engine subtree.

## Layering

1. `psionic-core`
   Public engine-facing scalar, tensor, dtype, device, and shape types.
2. `psionic-ir`
   Canonical graph and plan types suitable for deterministic inspection.
3. `psionic-compiler`
   Lowering, scheduling, and plan construction boundaries over IR.
4. `psionic-runtime`
   Runtime traits for devices, buffers, allocators, execution, low-level
   quantization dispatch, worker batching/parking decisions, and canonical
   execution-proof bundles that later validator and kernel layers can
   reference, including embeddings-first activation-fingerprint proof adapters
   and explicit optional-proof posture.
5. `psionic-sandbox`
   Bounded sandbox runtime detection, profile realization, execution adapters,
   execution evidence, and reusable background-job/file-transfer lifecycle
   contracts for sandbox compute lanes.
6. `psionic-net`
   Peer identity, direct/NAT/relay session lifecycle, relay-backed rendezvous,
   durable trust/candidate state, logical-stream reservation, bounded HTTP
   service tunnels, and transport observations.
7. `psionic-datastream`
   Resumable dataset/checkpoint manifests, chunk transport, and delivery
   receipts for staged artifact and training/eval data movement.
8. `psionic-collectives`
   Elastic device-mesh and benchmark-gated collective planning for training-
   class execution.
9. `psionic-train`
   Training-session truth for async checkpointing, live recovery, and
   elastic-membership posture built on clustered state and datastream
   manifests.
10. Backend crates
   Backend-specific runtime implementations only.
11. `psionic-models`
   Model abstractions and metadata over core/runtime primitives.
12. `psionic-serve`
   Served compute product contracts and execution interfaces.
13. `psionic-provider`
   Capability envelopes, readiness, receipts, and provider adapter types.

## Dependency Direction

- `psionic-core` sits at the bottom.
- `psionic-ir` may depend on `psionic-core`.
- `psionic-compiler` may depend on `psionic-ir` and `psionic-core`.
- `psionic-runtime` may depend on `psionic-core`, `psionic-ir`, and
  `psionic-compiler`.
- `psionic-sandbox` may depend on reusable engine crates only and owns no
  market authority or app behavior.
- `psionic-net` may depend on reusable runtime-facing crates but owns no market
  authority or app behavior.
- `psionic-datastream` may depend on reusable engine crates only and owns no
  market authority or app behavior.
- `psionic-collectives` depends on reusable runtime-facing crates only and owns
  no market authority or app behavior.
- `psionic-train` depends on `psionic-cluster`, `psionic-datastream`, and
  `psionic-runtime` for checkpoint, recovery, and elastic-membership truth, and
  owns no market authority or app behavior.
- `psionic-cluster` depends on `psionic-net` and `psionic-datastream` for
  transport/session truth plus staged artifact/data delivery contracts, and
  owns durable ordered-state, admission/revocation policy, compaction/catch-up,
  remote whole-request scheduling, replica-routed serving placement,
  public-network pipeline stage planning, layer-sharded handoff planning,
  tensor-collective planning, artifact residency/staging truth,
  sharded-manifest intake, clustered prefix/KV cache compatibility truth,
  streamed staging offers, and topology planning on top of it.
- backend crates may depend on runtime/core/IR/compiler as needed.
- `psionic-models` depends on reusable engine crates only.
- `psionic-serve` depends on models/runtime/core.
- `psionic-provider` depends on serve/runtime/core and remains the only
  OpenAgents-specific crate in the subtree.

## Proof Layers

- Base execution proof stays canonical in `psionic-runtime::ExecutionProofBundle`.
- Optional proof augmentations stay explicit through
  `ExecutionProofAugmentationPosture` instead of hidden metadata.
- The first augmentation is an embeddings-first activation-fingerprint adapter
  using quantized deterministic sampling with a benchmark helper for cost
  measurement.
- `psionic-provider` is responsible for attaching product-appropriate proof
  artifacts to receipts; it does not invent new proof schemas outside runtime.

## Boundaries

- No crate in `crates/psionic/` may path-depend on `apps/*`.
- No app-specific UX or product workflows live in reusable engine crates.
- Backend crates must not own provider policy, payout logic, or app orchestration.
- `psionic-provider` defines adapter-facing types but must not pull app code into
  the engine subtree.

## Review Checklist

- Is this logic in the lowest crate that can honestly own it?
- Does this change pull product-specific behavior into reusable crates?
- Is the backend model explicit and truthful?
- Can the type or plan be serialized or inspected deterministically when needed?
