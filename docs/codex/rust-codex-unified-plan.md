# Rust Codex Unified Plan

Status: active strategy
Last updated: 2026-02-21

## Objective

Unify desktop, web, and iOS Codex administration around one runtime authority path and one Khala WS replay/subscription path.

## Core Decisions

1. Runtime owns Codex worker authority state.
2. Control service owns auth/session/device state and sync-token issuance.
3. Khala is WS-only replay/live delivery for Codex projections.
4. Proto contracts under `proto/openagents/codex/v1/*` remain the wire authority.

## Surface Behavior

1. Desktop is the primary operator surface.
2. Web and iOS consume the same Codex projection streams and command APIs.
3. Client apply path is idempotent by `(topic, seq)`.

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
cargo test --manifest-path apps/runtime/Cargo.toml server::tests::khala_topic_messages -- --nocapture
```
