# Codex Control Core Crate

Status: Active
Date: 2026-02-21

## Purpose

`crates/openagents-codex-control` is the host-agnostic core for mobile->desktop Codex control semantics.

It owns:
1. Worker handshake parsing (`ios/handshake`, `desktop/handshake_ack`).
2. iOS message parsing (`ios/user_message`).
3. Structured control request parsing/validation (`worker.request` + wrapped event forms).
4. Method allowlist typing (`thread/start`, `thread/resume`, `turn/start`, `turn/interrupt`, `thread/list`, `thread/read`).
5. Session/thread target resolution helpers.
6. Success/error receipt builders (`worker.response`, `worker.error`).
7. Replay/idempotency primitives (`RequestReplayState`, dedupe keys).

## Host Adapters

Desktop adapter:
- `apps/autopilot-desktop/src/runtime_codex_proto.rs`
- Re-exports core parsing/receipt helpers and composes with Khala frame parsing from `openagents-client-core`.

iOS/thin shared adapter:
- `crates/openagents-client-core/src/codex_control.rs`
- Provides normalized extraction helpers for host bridges.
- FFI export for mobile bridge: `oa_client_core_extract_control_request`.

## Verification

Current validation lane for this extraction:
1. `cargo test -p openagents-codex-control`
2. `cargo test -p openagents-client-core codex_worker`
3. `cargo test -p autopilot-desktop runtime_codex_proto`

These cover core contract parsing + desktop integration regression checks for handshake/user-message behavior.
