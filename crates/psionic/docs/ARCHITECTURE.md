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
   Runtime traits for devices, buffers, allocators, and execution.
5. `psionic-net`
   Peer identity, direct/NAT/relay session lifecycle, relay-backed rendezvous,
   durable trust/candidate state, logical-stream reservation, bounded HTTP
   service tunnels, and transport observations.
6. Backend crates
   Backend-specific runtime implementations only.
7. `psionic-models`
   Model abstractions and metadata over core/runtime primitives.
8. `psionic-serve`
   Served compute product contracts and execution interfaces.
9. `psionic-provider`
   Capability envelopes, readiness, receipts, and provider adapter types.

## Dependency Direction

- `psionic-core` sits at the bottom.
- `psionic-ir` may depend on `psionic-core`.
- `psionic-compiler` may depend on `psionic-ir` and `psionic-core`.
- `psionic-runtime` may depend on `psionic-core`, `psionic-ir`, and
  `psionic-compiler`.
- `psionic-net` may depend on reusable runtime-facing crates but owns no market
  authority or app behavior.
- `psionic-cluster` depends on `psionic-net` for transport/session truth and
  owns durable ordered-state, admission/revocation policy, compaction/catch-up,
  remote whole-request scheduling, replica-routed serving placement,
  public-network pipeline stage planning, layer-sharded handoff planning,
  tensor-collective planning, artifact residency/staging truth,
  sharded-manifest intake, and topology planning on top of it.
- backend crates may depend on runtime/core/IR/compiler as needed.
- `psionic-models` depends on reusable engine crates only.
- `psionic-serve` depends on models/runtime/core.
- `psionic-provider` depends on serve/runtime/core and remains the only
  OpenAgents-specific crate in the subtree.

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
