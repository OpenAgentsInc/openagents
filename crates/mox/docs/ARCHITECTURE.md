# Mox Architecture

Mox is structured as a layered engine subtree.

## Layering

1. `mox-core`
   Public engine-facing scalar, tensor, dtype, device, and shape types.
2. `mox-ir`
   Canonical graph and plan types suitable for deterministic inspection.
3. `mox-compiler`
   Lowering, scheduling, and plan construction boundaries over IR.
4. `mox-runtime`
   Runtime traits for devices, buffers, allocators, and execution.
5. Backend crates
   Backend-specific runtime implementations only.
6. `mox-models`
   Model abstractions and metadata over core/runtime primitives.
7. `mox-serve`
   Served compute product contracts and execution interfaces.
8. `mox-provider`
   Capability envelopes, readiness, receipts, and provider adapter types.

## Dependency Direction

- `mox-core` sits at the bottom.
- `mox-ir` may depend on `mox-core`.
- `mox-compiler` may depend on `mox-ir` and `mox-core`.
- `mox-runtime` may depend on `mox-core`, `mox-ir`, and
  `mox-compiler`.
- backend crates may depend on runtime/core/IR/compiler as needed.
- `mox-models` depends on reusable engine crates only.
- `mox-serve` depends on models/runtime/core.
- `mox-provider` depends on serve/runtime/core and remains the only
  OpenAgents-specific crate in the subtree.

## Boundaries

- No crate in `crates/mox/` may path-depend on `apps/*`.
- No app-specific UX or product workflows live in reusable engine crates.
- Backend crates must not own provider policy, payout logic, or app orchestration.
- `mox-provider` defines adapter-facing types but must not pull app code into
  the engine subtree.

## Review Checklist

- Is this logic in the lowest crate that can honestly own it?
- Does this change pull product-specific behavior into reusable crates?
- Is the backend model explicit and truthful?
- Can the type or plan be serialized or inspected deterministically when needed?
