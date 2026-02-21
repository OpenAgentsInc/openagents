# WGPUI iOS Feasibility Audit (Autopilot Desktop -> Autopilot iOS)

Date: 2026-02-20
Status: Research audit
Scope: Evaluate replacing SwiftUI in `apps/autopilot-ios` with the Rust/WGPUI stack used by desktop.

## TL;DR

- Reusing WGPUI on iOS is technically feasible, but not drop-in.
- `wgpui` and `autopilot_ui` both compile and link for `aarch64-apple-ios` in this repo.
- The current desktop app host (`apps/autopilot-desktop`) does not fully link for iOS as-is.
- Major product blockers are input model (no touch/gesture events), text-input lifecycle/IME details, and missing Swift<->Rust packaging glue.
- Recommendation: keep SwiftUI as shipping UI for now, add shared Rust core first, and run a narrow WGPUI-on-iOS spike before committing to full replacement.

## Current State (Code Reality)

Desktop side:

- Desktop host is Rust `winit + wgpu + wgpui` in `apps/autopilot-desktop/src/main.rs`.
- Shared desktop UI lives in `crates/autopilot_ui` and is compiled against `wgpui` with `desktop` feature (`crates/autopilot_ui/Cargo.toml:13`).
- WGPUI features are currently `web` and `desktop`; there is no iOS-specific feature or platform module (`crates/wgpui/Cargo.toml:14`, `crates/wgpui/Cargo.toml:22`, `crates/wgpui/src/platform.rs:33`, `crates/wgpui/src/platform.rs:372`).

iOS side:

- iOS app is native SwiftUI (`apps/autopilot-ios/Autopilot/Autopilot/ContentView.swift:3`).
- Runtime/Codex networking is Swift (`apps/autopilot-ios/Autopilot/Autopilot/CodexHandshakeViewModel.swift:6`) with Khala WS stream handling (`apps/autopilot-ios/Autopilot/Autopilot/CodexHandshakeViewModel.swift:513`).

Input model in WGPUI today:

- WGPUI input events are mouse/scroll/keyboard only (`crates/wgpui/src/input.rs:74` to `crates/wgpui/src/input.rs:102`).
- No touch/pinch/pan gesture event types exist in this input contract.
- Desktop host event loop only maps mouse wheel, mouse buttons, cursor movement, and keyboard (`apps/autopilot-desktop/src/main.rs:1327` to `apps/autopilot-desktop/src/main.rs:1469`).

Clipboard/platform assumptions:

- WGPUI clipboard helper supports macOS/Linux/Windows command paths only; unsupported platforms return error (`crates/wgpui/src/clipboard.rs:3` to `crates/wgpui/src/clipboard.rs:46`).
- There is no existing Rust<->Swift bridge setup (`uniffi`, `cbindgen`, `swift-bridge`, `xcframework`) in repo.

## Build/Target Validation Run

All commands run from `/Users/christopherdavid/code/openagents` on 2026-02-20.

Passed:

- `cargo check -p wgpui --no-default-features --features desktop --target aarch64-apple-ios-sim`
- `cargo check -p wgpui --no-default-features --features desktop --target aarch64-apple-ios`
- `cargo check -p autopilot_ui --target aarch64-apple-ios-sim`
- `cargo check -p autopilot_ui --target aarch64-apple-ios`
- `cargo check -p autopilot-desktop --target aarch64-apple-ios-sim`
- `cargo check -p autopilot-desktop --target aarch64-apple-ios`
- `cargo build -p wgpui --no-default-features --features desktop --target aarch64-apple-ios`
- `cargo build -p autopilot_ui --target aarch64-apple-ios`

Failed:

- `cargo build -p autopilot-desktop --target aarch64-apple-ios`

Observed failure mode:

- Linker error for arm64 with `Undefined symbols ... ___chkstk_darwin` (via `zstd_sys` object chain), plus many warnings about object files built for iOS 26.2 vs link target 10.0.

Interpretation:

- WGPUI and shared UI crate are linkable for iOS target.
- Full desktop binary is not currently linkable for iOS without toolchain/deployment-target/dependency cleanup.
- This failure is in the full desktop host/dependency graph, not in WGPUI core alone.

## Feasibility Assessment

What is viable today:

- Reusing Rust UI primitives and rendering stack on iOS is plausible.
- `wgpui` and `autopilot_ui` can already produce iOS-target binaries.

What blocks “replace SwiftUI now”:

- No touch-first input contract in WGPUI (`InputEvent` has no touch/gesture types).
- No iOS-specific host integration layer for lifecycle/IME/keyboard safe-area behavior.
- Existing Autopilot UI behaviors are desktop-biased (hover, cursor modes, hotbar/shortcuts, pane dragging).
- No Swift integration path exists yet (no generated bindings/framework pipeline).
- Large dependency surface if you pull the full desktop host into iOS app binary.

## Migration Options

Option A: Keep SwiftUI UI, share Rust core logic only.

- Move protocol/event normalization/dedupe/state-reducer logic into a new Rust core crate.
- Call it from Swift through a small FFI wrapper.
- Lowest UX risk, fastest path to consistency with desktop parsing/state logic.

Option B: Hybrid UI.

- Keep Swift shell/navigation/auth screens.
- Embed one Rust-rendered WGPUI surface (chat timeline) as a constrained module.
- Medium risk; good for proving rendering and input strategy.

Option C: Full WGPUI replacement of SwiftUI.

- Build iOS app UI entirely with WGPUI and a Rust host.
- Highest risk/effort; requires touch/IME/lifecycle parity work before acceptable product quality.

## Recommended Path

1. Short term: keep shipping SwiftUI in `apps/autopilot-ios`.
2. Immediately: create a shared Rust mobile-core crate for Codex event parsing, dedupe, message assembly, and handshake state transitions.
3. Spike: build a tiny iOS Rust host with WGPUI rendering one scrollable chat view and basic text input.
4. Add to WGPUI: touch/gesture event primitives and adapters.
5. Add packaging: reproducible `xcframework` (or equivalent) build path for Swift integration.
6. Decision gate: replace more UI only after the spike hits latency, battery, and interaction quality targets on real devices.

## Go/No-Go Criteria For Full Replacement

- Touch interactions feel native (tap, drag, scroll, long-press, selection).
- Text input/IME behavior is production-safe on real devices.
- Stream rendering latency is better or equal vs SwiftUI implementation.
- Binary size and startup time stay within acceptable thresholds.
- CI can build and test iOS Rust artifacts reproducibly.

## Bottom Line

A WGPUI-based iOS app is possible in this codebase, but the project is not currently in a “replace SwiftUI now” state. The correct next step is a hybrid spike plus shared Rust core extraction, then decide whether full replacement is justified by measurable gains.
