# Rust Client Core Integration (iOS)

Status: Active (`OA-RUST-107`)

## Purpose

`apps/autopilot-ios` consumes shared Rust client-core artifacts from `crates/openagents-client-core` for auth normalization and Khala frame parsing paths. Swift remains host/UI/platform glue only.

## Canonical Packaging Commands

Build deterministic versioned artifacts:

```bash
apps/autopilot-ios/scripts/build-rust-client-core.sh --clean
```

Verify deterministic reproducibility (two clean rebuilds, checksum/manifest parity):

```bash
apps/autopilot-ios/scripts/verify-rust-client-core-reproducibility.sh
```

## Artifact Model

Build output root:

- `apps/autopilot-ios/Autopilot/RustCore/`

Versioned artifact directory:

- `apps/autopilot-ios/Autopilot/RustCore/v<crate-version>-<git-sha>/`

Generated outputs:

- `OpenAgentsClientCore.xcframework/`
- `Headers/openagents_client_core.h`
- `Headers/module.modulemap`
- `ffi-contract.json`
- `manifest.json`
- `manifest.sha256`
- `libopenagents_client_core_sim.a`

Pointer files:

- `LATEST_VERSION`
- `current` symlink -> active version directory

## FFI Contract (Authoritative)

Contract version:

- `oa_client_core_ffi_contract_version` (currently `1`)

Required symbols:

- `oa_client_core_ffi_contract_version`
- `oa_client_core_normalize_email`
- `oa_client_core_normalize_verification_code`
- `oa_client_core_normalize_message_text`
- `oa_client_core_extract_desktop_handshake_ack_id`
- `oa_client_core_parse_khala_frame`
- `oa_client_core_free_string`

Memory ownership rules:

- Input C strings remain caller-owned.
- Returned pointers are Rust-owned and must be released with `oa_client_core_free_string`.
- Null return indicates invalid input or parse/normalization failure.

Threading model:

- Bridge functions are re-entrant and safe to call from app-managed threads.
- iOS host handles task lifecycle/cancellation and UI threading.

Error mapping:

- FFI boundary maps recoverable failures to null pointer returns.
- Swift host maps null to domain fallback/error handling where needed.

## Swift Host Boundary

`RustClientCoreBridge.swift` is the host boundary.

It must:

- Resolve all required symbols.
- Validate `oa_client_core_ffi_contract_version` equals the expected contract version.
- Refuse bridge availability when symbol set/contract version is incompatible.

It must not:

- Re-implement canonical protocol parsing or auth normalization semantics that live in Rust.

## CI and Verification

Local CI lane:

- `./scripts/local-ci.sh ios-rust-core`

Lane checks:

- deterministic package build
- reproducibility verifier
- Rust FFI contract tests (`cargo test -p openagents-client-core`)

Manual iOS smoke:

- Build/run `Autopilot` simulator target and verify auth + worker stream message flow against runtime.
- Real-device handshake procedure: `apps/autopilot-ios/docs/real-device-codex-handshake-runbook.md`
