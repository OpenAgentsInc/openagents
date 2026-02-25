# Rust Migration Invariant Gates

Status: active
Last updated: 2026-02-22

These invariants gate Rust migration work and release decisions.

Transition note (2026-02-25):

1. Current invariants reflect the implemented Khala sync lane.
2. Active replacement plans target SpacetimeDB as canonical sync transport (`docs/plans/spacetimedb-full-integration.md`).
3. `INV-03`/`INV-06` will be superseded by equivalent Spacetime transport invariants when the replacement ADR lands.

## INV-01: Proto-first contracts

Cross-process contracts are defined in `proto/` and generated into Rust.

## INV-02: HTTP-only authority mutations

Authority mutations happen through authenticated HTTP APIs only.

## INV-03: Khala WS-only live transport

Khala live subscriptions use WebSocket only. No SSE/poll live authority lanes are allowed.

Bounded exception:
- SSE is allowed only as an HTTP presentation adapter over existing codex/Khala authority outputs (see `docs/adr/ADR-0008-bounded-vercel-sse-compatibility-lane.md`).
- SSE is prohibited as an alternate authority source or as a Khala live-sync transport.

## INV-04: Control-plane authority isolation

Control-plane canonical state is isolated from runtime authority state.

## INV-05: Runtime authority isolation

Runtime canonical event state is written only by runtime authority paths.

## INV-06: Khala is delivery, not authority

Khala writes only sync/replay metadata, never domain authority events.

## INV-07: Replay/idempotency contract

Client apply path is idempotent by `(topic, seq)` with replay/resume support.

## INV-08: Service deploy isolation

Control, runtime, and Khala are independently deployable in production.

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
