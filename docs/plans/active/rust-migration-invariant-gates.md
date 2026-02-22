# Rust Migration Invariant Gates

Status: active
Last updated: 2026-02-21

These invariants gate Rust migration work and release decisions.

## INV-01: Proto-first contracts

Cross-process contracts are defined in `proto/` and generated into Rust.

## INV-02: HTTP-only authority mutations

Authority mutations happen through authenticated HTTP APIs only.

## INV-03: Khala WS-only live transport

Khala live subscriptions use WebSocket only. No new SSE/poll live lanes.

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
