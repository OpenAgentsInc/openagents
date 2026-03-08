# Rustygrad

Rustygrad is the Rust-native compute engine program for OpenAgents.

It is intentionally scoped as a workspace subtree under `crates/rustygrad/` so the
engine can evolve without bleeding product-specific behavior into shared crates.

## What Rustygrad is

- A tensor, IR, compiler, and runtime stack built in Rust.
- A place to land productized inference and embeddings execution.
- A backend family that can map cleanly into provider capabilities and receipts.
- A foundation for CPU first, then Metal and AMD backends.

## What Rustygrad is not

- Not a literal line-by-line port of Tinygrad.
- Not an app surface or provider UX layer.
- Not a shortcut around `docs/OWNERSHIP.md`.
- Not a promise that text generation, KV cache, or AMD kernels are already built.

## Crate Map

- `rustygrad-core`: foundational tensor, shape, dtype, and device types.
- `rustygrad-ir`: canonical graph and execution-plan representation.
- `rustygrad-compiler`: lowering and scheduling boundaries over IR.
- `rustygrad-runtime`: runtime traits for devices, allocation, and execution.
- `rustygrad-models`: reusable model definitions and metadata.
- `rustygrad-serve`: request/response and execution interfaces for served products.
- `rustygrad-provider`: capability, readiness, and receipt-facing types.
- `rustygrad-backend-cpu`: CPU reference backend.
- `rustygrad-backend-metal`: Metal backend with a first embeddings product path.
- `rustygrad-backend-amd-kfd`: AMD KFD discovery/readiness backend.
- `rustygrad-backend-amd-userspace`: AMD userspace discovery/readiness backend.

## Design Principles

- Keep the compiler and runtime visible and inspectable.
- Keep crate ownership narrow and documented.
- Preserve a strict boundary between reusable engine crates and OpenAgents provider
  integration.
- Model backend families explicitly; AMD KFD and AMD userspace are separate
  backends, not one hidden toggle.
- Keep inference and embeddings first-class in architecture from the start.

## Current Phase

This subtree now has a tested CPU product baseline, a first Metal-backed
`rustygrad.embeddings` path, and explicit AMD discovery/readiness groundwork.
AMD execution support is still future work.
