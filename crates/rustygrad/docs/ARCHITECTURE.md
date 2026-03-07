# Rustygrad Architecture

Rustygrad is structured as a layered engine subtree.

## Layering

1. `rustygrad-core`
   Public engine-facing scalar, tensor, dtype, device, and shape types.
2. `rustygrad-ir`
   Canonical graph and plan types suitable for deterministic inspection.
3. `rustygrad-compiler`
   Lowering, scheduling, and plan construction boundaries over IR.
4. `rustygrad-runtime`
   Runtime traits for devices, buffers, allocators, and execution.
5. Backend crates
   Backend-specific runtime implementations only.
6. `rustygrad-models`
   Model abstractions and metadata over core/runtime primitives.
7. `rustygrad-serve`
   Served compute product contracts and execution interfaces.
8. `rustygrad-provider`
   Capability envelopes, readiness, receipts, and provider adapter types.

## Dependency Direction

- `rustygrad-core` sits at the bottom.
- `rustygrad-ir` may depend on `rustygrad-core`.
- `rustygrad-compiler` may depend on `rustygrad-ir` and `rustygrad-core`.
- `rustygrad-runtime` may depend on `rustygrad-core`, `rustygrad-ir`, and
  `rustygrad-compiler`.
- backend crates may depend on runtime/core/IR/compiler as needed.
- `rustygrad-models` depends on reusable engine crates only.
- `rustygrad-serve` depends on models/runtime/core.
- `rustygrad-provider` depends on serve/runtime/core and remains the only
  OpenAgents-specific crate in the subtree.

## Boundaries

- No crate in `crates/rustygrad/` may path-depend on `apps/*`.
- No app-specific UX or product workflows live in reusable engine crates.
- Backend crates must not own provider policy, payout logic, or app orchestration.
- `rustygrad-provider` defines adapter-facing types but must not pull app code into
  the engine subtree.

## Review Checklist

- Is this logic in the lowest crate that can honestly own it?
- Does this change pull product-specific behavior into reusable crates?
- Is the backend model explicit and truthful?
- Can the type or plan be serialized or inspected deterministically when needed?
