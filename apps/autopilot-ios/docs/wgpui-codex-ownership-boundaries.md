# iOS Codex Ownership Boundaries (WGPUI Cutover)

This document defines what is product-authoritative vs host-adapter code for iOS Codex.

## Production UI Authority

- Production iOS Codex UI is the WGPUI surface rendered by `WgpuiBackgroundView`.
- App entrypoint routes directly to `WgpuiCodexRootView` (WGPUI path only).
- `ContentView` is retained only as a deprecated compatibility wrapper.

## Rust-Authoritative Lanes

- Control request lifecycle and receipt reconciliation: Rust client-core FFI.
- Khala WS session protocol orchestration and watermark semantics: Rust client-core FFI.
- Control success context extraction (thread/turn updates, interrupt semantics): Rust client-core FFI.
- Handshake/receipt payload decoding helpers used by iOS host: Rust client-core FFI.

## iOS Host-Adapter Lanes

- `WgpuiBackgroundBridge.swift`: FFI boundary only.
- `WgpuiBackgroundView.swift`: UIKit input focus/keyboard, lifecycle, and render-surface adaptation only.
- Host callbacks dispatch user intents to app runtime methods; they do not render separate Codex product UI.

## Transitional Notes

- `CodexHandshakeViewModel.swift`, `RuntimeCodexClient.swift`, and `RuntimeCodexModels.swift` remain in the production lane as runtime adapters while parity migration completes.
- Follow-on work should continue shrinking these Swift lanes by moving remaining state-machine/business logic into Rust client-core.
