# Rust Migration Invariant Gates

Status: active
Last updated: 2026-02-22

These invariants gate Rust migration work and release decisions.

Transition note (2026-02-25):

1. `docs/adr/ADR-0009-spacetime-sync-canonical-transport.md` is accepted and is canonical sync doctrine.
2. Khala is an implemented legacy lane during migration and must not be treated as long-term sync authority.
3. Replacement execution details are tracked in `docs/plans/spacetimedb-full-integration.md`.

## INV-01: Proto-first contracts

Cross-process contracts are defined in `proto/` and generated into Rust.

## INV-02: HTTP-only authority mutations

Authority mutations happen through authenticated HTTP APIs only.

## INV-03: Spacetime WS canonical live transport

Spacetime live subscriptions are the canonical transport for retained OpenAgents sync delivery. No SSE/poll live authority lanes are allowed.

Migration allowance:
- Khala WS may remain enabled only as an explicitly bounded legacy fallback lane until replacement cutover gates pass.
- New sync feature work must target Spacetime transport semantics, not Khala expansion.

Bounded exception:
- SSE is allowed only as an HTTP presentation adapter over existing codex/sync authority outputs (see `docs/adr/ADR-0008-bounded-vercel-sse-compatibility-lane.md`).
- SSE is prohibited as an alternate authority source or as a live-sync authority transport.

## INV-04: Control-plane authority isolation

Control-plane canonical state is isolated from runtime authority state.

## INV-05: Runtime authority isolation

Runtime canonical event state is written only by runtime authority paths.

## INV-06: Sync transport is delivery, not authority

Spacetime/legacy Khala lanes may write sync/replay metadata only, never domain authority events.

## INV-07: Replay/idempotency contract

Client apply path is idempotent with replay/resume support and ordered stream keys.

Migration mapping:
- legacy Khala key: `(topic, seq)`
- Spacetime key: `(stream_id, seq)`

## INV-08: Service deploy isolation

Control, runtime, and sync transport services are independently deployable in production.

## INV-09: Migration discipline

Runtime deploys must run migration job validation and drift checks.

## INV-10: Legacy removal ordering

Legacy infra removal occurs only after parity gates and rollback evidence.

## INV-11: iOS WGPUI-only product UI

iOS product UI surfaces (Codex/chat/thread/admin flows) must run through Rust/WGPUI.

- Swift/SwiftUI is limited to host/bootstrap and OS capability bridges only.
- No new product UI/state logic is allowed in SwiftUI view code.
- Existing SwiftUI product UI is migration debt and must be removed as WGPUI parity lands.

## INV-12: No GitHub workflow automation in-repo

The repository must not contain `.github/` workflow automation.

- Do not add GitHub Actions workflow files under `.github/workflows/`.
- Execute verification/deploy gates via canonical local scripts and runbooks.
