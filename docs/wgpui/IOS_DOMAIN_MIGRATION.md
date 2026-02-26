# iOS Domain Extraction Notes

This note captures the split introduced for issue `#2317`.

## What moved

- Mission/Codex app-domain model types and filter semantics were extracted to:
  - `crates/openagents-ios-domain`

## What stayed in `wgpui`

- `crates/wgpui/src/platform/ios.rs` is now bridge-focused:
  - surface/device/renderer lifecycle
  - frame rendering
  - FFI bridge entry points
  - minimal bridge state + consume flags

It no longer contains the previous in-adapter mission/Codex orchestration logic.

## Migration guidance

- iOS host/app code should own domain state and orchestration.
- Use `openagents-ios-domain` models for mission/Codex payload semantics.
- Pass explicit, precomputed display state into bridge calls instead of letting
  WGPUI own domain mutation rules.
