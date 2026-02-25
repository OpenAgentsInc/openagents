# Rust Codex Unified Plan

Status: active strategy
Last updated: 2026-02-21

## Objective

Unify desktop and web Codex administration around one runtime authority path and one retained Spacetime replay/subscription transport path.

## Core Decisions

1. Runtime owns Codex worker authority state.
2. Control service owns auth/session/device state and sync-token issuance.
3. Spacetime is canonical replay/live delivery for retained Codex projections.
4. Proto contracts under `proto/openagents/codex/v1/*` remain the wire authority.

## Surface Behavior

1. Desktop is the primary operator surface.
2. Web and iOS consume the same Codex projection streams and command APIs.
3. Client apply path is idempotent by `(stream_id, seq)` (with legacy compatibility parsing where required).

## Required Workstreams

1. Command-path parity across desktop/web/iOS.
2. Shared client-core transport/resume logic.
3. Replay correctness and stale-cursor recovery on all surfaces.
4. Observability and incident runbooks for WS auth/reconnect/resume.

## Verification

```bash
./scripts/local-ci.sh changed
./scripts/local-ci.sh workspace-compile
./scripts/run-cross-surface-contract-harness.sh
cargo test --manifest-path apps/runtime/Cargo.toml server::tests::spacetime_sync_metrics_expose_stream_delivery_totals -- --nocapture
cargo test --manifest-path apps/runtime/Cargo.toml server::tests::retired_khala_routes_return_not_found -- --nocapture
```
