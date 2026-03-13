# Psionic

Psionic is the Rust-native compute engine program for OpenAgents.

It is intentionally scoped as a workspace subtree under `crates/psionic/` so the
engine can evolve without bleeding product-specific behavior into shared crates.

## What Psionic is

- A tensor, IR, compiler, and runtime stack built in Rust.
- A place to land productized inference and embeddings execution.
- A backend family that can map cleanly into provider capabilities and receipts.
- A foundation for CPU first, then Metal and AMD backends.

## What Psionic is not

- Not a literal line-by-line port of Tinygrad.
- Not an app surface or provider UX layer.
- Not a shortcut around `docs/OWNERSHIP.md`.
- Not a promise that text generation, KV cache, or AMD kernels are already built.

## Crate Map

- `psionic-core`: foundational tensor, shape, dtype, and device types.
- `psionic-ir`: canonical graph and execution-plan representation.
- `psionic-compiler`: lowering and scheduling boundaries over IR.
- `psionic-runtime`: runtime traits for devices, allocation, and execution.
- `psionic-net`: peer identity, direct/NAT/relay session establishment, durable trust and candidate history, relay-backed rendezvous, policy-gated HTTP service tunnels, and transport observations.
- `psionic-cluster`: durable ordered-state, admission policy, catch-up, scheduling, and topology substrate over `psionic-net`.
- `psionic-models`: reusable model definitions and metadata.
- `psionic-serve`: request/response and execution interfaces for served products.
- `psionic-provider`: capability, readiness, and receipt-facing types.
- `psionic-backend-cpu`: CPU reference backend.
- `psionic-backend-metal`: Metal backend with a first embeddings product path.
- `psionic-backend-amd-kfd`: AMD KFD discovery/readiness backend.
- `psionic-backend-amd-userspace`: AMD userspace discovery/readiness backend.
- `psionic-apple-fm`: Apple Foundation Models bridge contracts, HTTP client, and types for the Swift sidecar.

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
`psionic.embeddings` path, wider-network `psionic-net` session establishment
for direct/NAT/relay connectivity, durable ordered-state persistence, policy-
driven wider-network candidate admission, truthful remote whole-request
scheduling, and restart-safe catch-up in `psionic-cluster`, plus explicit AMD
discovery/readiness groundwork. AMD execution support is still future work.

## Docs

- **[docs/FM_BRIDGE_CONSIDERATIONS.md](docs/FM_BRIDGE_CONSIDERATIONS.md)** — Apple Foundation Models bridge: architecture, binary discovery, build, run, test, shipping, and user requirements in full detail.
- **[docs/ROADMAP_FM.md](docs/ROADMAP_FM.md)** — Apple FM lane roadmap and API coverage.
- Other planning and reference docs live under `crates/psionic/docs/`.
