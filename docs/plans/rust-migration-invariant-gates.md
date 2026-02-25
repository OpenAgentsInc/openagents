# Rust Migration Invariant Gates

Status: active
Last updated: 2026-02-26

These invariants gate Rust migration work and release decisions.

Transport enforcement note (2026-02-26):

1. `docs/adr/ADR-0010-spacetime-only-sync-transport-hard-mandate.md` is the active sync doctrine.
2. Retained runtime/client sync paths must be Spacetime-only; no legacy websocket/topic lanes.
3. Any temporary legacy exception requires explicit owner + expiry + deletion issue per ADR-0010.

## INV-01: Proto-first contracts

Cross-process contracts are defined in `proto/` and generated into Rust.

## INV-02: HTTP-only authority mutations

Authority mutations happen through authenticated HTTP APIs only.

## INV-03: Spacetime-Only Live Transport

Spacetime live subscriptions are the only permitted retained live sync transport for runtime/client
delivery.

Hard rules:
- Legacy websocket pathing, Phoenix frame semantics, and topic poll/fanout compatibility lanes are prohibited in retained paths.
- SSE is allowed only as an HTTP presentation adapter over existing authority outputs (see `docs/adr/ADR-0008-bounded-vercel-sse-compatibility-lane.md`).
- SSE is prohibited as an alternate live-sync authority transport.
- No new compatibility alias may be introduced for sync transport without an approved ADR-0010 exception.

Exception gate:
- Must include owner, expiry, blast radius, rollback path, and linked deletion issue.
- Exception automatically fails compliance after expiry.

## INV-04: Control-plane authority isolation

Control-plane canonical state is isolated from runtime authority state.

## INV-05: Runtime authority isolation

Runtime canonical event state is written only by runtime authority paths.

## INV-06: Sync transport is delivery, not authority

Spacetime/legacy Spacetime lanes may write sync/replay metadata only, never domain authority events.

## INV-07: Replay/idempotency contract

Client apply path is idempotent with replay/resume support and ordered stream keys.

Migration mapping:
- legacy Spacetime key: `(topic, seq)`
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
