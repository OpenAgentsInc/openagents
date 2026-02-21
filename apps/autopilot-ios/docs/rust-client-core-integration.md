# Rust Client Core Integration (iOS)

Status: Active (OA-RUST-055)

## Scope

iOS now routes core command/sync normalization and Khala frame decoding through the shared Rust client core (`crates/openagents-client-core`) via C-ABI bridge symbols.

## What moved to Rust core

From Swift host logic to shared Rust core:
- auth email normalization
- verification-code normalization
- outbound message text normalization
- desktop handshake-ack extraction from worker event payloads
- Phoenix/Khala frame parsing for websocket stream frames

Rust bridge symbols consumed by iOS:
- `oa_client_core_normalize_email`
- `oa_client_core_normalize_verification_code`
- `oa_client_core_normalize_message_text`
- `oa_client_core_extract_desktop_handshake_ack_id`
- `oa_client_core_parse_khala_frame`
- `oa_client_core_free_string`

## Swift host boundary after migration slice

Swift remains responsible for:
- UI state and rendering (`SwiftUI`)
- iOS lifecycle/background behavior
- URLSession transport + websocket socket lifecycle
- secure local token/device persistence and user UX

Swift no longer owns the above normalization/protocol semantics when Rust symbols are present.

## Build artifact generation

Generate iOS/simulator Rust artifacts:

```bash
apps/autopilot-ios/scripts/build-rust-client-core.sh
```

This builds `openagents-client-core` for iOS + simulator and emits:
- `apps/autopilot-ios/Autopilot/RustCore/OpenAgentsClientCore.xcframework`

## Runtime behavior

`RustClientCoreBridge.swift` dynamically resolves Rust symbols in-process.
- If symbols are present: iOS uses Rust core paths.
- If symbols are absent: iOS falls back to existing Swift parsing/normalization logic (safe fallback for local dev while wiring full artifact embedding).

## Verification

Rust-side verification:
- `cargo test -p openagents-client-core`
- `cargo test -p autopilot-desktop runtime_codex_proto`

iOS flow verification:
- run real-device handshake flow in `apps/autopilot-ios/docs/real-device-codex-handshake-runbook.md`
- confirm auth code flow, message send, and handshake ack detection still function
