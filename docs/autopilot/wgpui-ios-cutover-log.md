# WGPUI iOS cutover — implementation log

**Date:** 2026-02-21
**Scope:** Replace iOS homescreen background with WGPUI-rendered dots grid (same as desktop). Cutover is WGPUI-only for the background; SwiftUI remains for tabs and content.

## What was done

### 1. WGPUI iOS platform (`crates/wgpui`)

- **Feature:** `ios = ["pollster"]` in `Cargo.toml` (no `raw-window-handle`).
- **`platform.rs`:** New `#[cfg(feature = "ios")] pub mod ios`:
  - Uses `wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer_ptr)` and `instance.create_surface_unsafe(...)` so no WindowHandle/Send/Sync is required.
  - **`IosBackgroundState`:** Created from a `CAMetalLayer` pointer; holds device, queue, surface, config, renderer, text_system, size, scale.
  - **`IosBackgroundState::new(layer_ptr, width, height, scale)`:** Async init with `pollster::block_on`, Metal backend, same dots grid as desktop (black quad + `DotsGrid`, `DotShape::Circle`, `theme::text::MUTED`, opacity 0.12, distance 32, size 1.5).
  - **`render()`:** Builds scene (black + dots grid), prepare/render, present.
  - **`resize(width, height)`:** Updates config and size.
  - **C FFI** (Rust 2024 `#[unsafe(no_mangle)]`):
    `wgpui_ios_background_create`, `wgpui_ios_background_render`, `wgpui_ios_background_resize`, `wgpui_ios_background_destroy`.
- **`lib.rs`:** `#[cfg(feature = "ios")] pub use platform::ios::IosBackgroundState;`

### 2. iOS client core dependency

- **`crates/openagents-client-core/Cargo.toml`:**
  `[target.'cfg(target_os = "ios")'.dependencies]`
  `wgpui = { path = "../wgpui", default-features = false, features = ["ios"] }`
  So the iOS build of `openagents-client-core` pulls in wgpui with the `ios` feature; the static lib contains the four FFI symbols.

### 3. iOS app integration

- **`WgpuiBackgroundBridge.swift`:** Loads `wgpui_ios_background_*` via `dlsym(nil, …)` (same pattern as `RustClientCoreBridge`). Exposes `isAvailable`, `create`, `render`, `resize`, `destroy`.
- **`WgpuiBackgroundView.swift`:**
  - `WgpuiBackgroundUIView`: custom `UIView` with `layerClass = CAMetalLayer.self`; in `layoutSubviews` creates WGPUI state from layer pointer and starts `CADisplayLink`; each tick calls render; `deinit` invalidates display link and destroys state.
  - `WgpuiBackgroundView`: SwiftUI view that uses the bridge when available, else `Color.black`.
- **`ContentView.swift`:** Root background changed from `OATheme.background.ignoresSafeArea()` to `WgpuiBackgroundView()`.

## Build / run

- Rebuild Rust iOS artifact:
  `apps/autopilot-ios/scripts/build-rust-client-core.sh`
  (unchanged; only checks `oa_client_core_*` symbols; wgpui symbols are in the same static lib.)
- Then build and run the app in Xcode (or `xcodebuild` + `xcrun simctl`) so the app links the new artifact. The homescreen shows the WGPUI dots grid when symbols are present, otherwise solid black.

## References

- Desktop grid reference: `crates/autopilot_ui/src/lib.rs` (e.g. `DotsGrid`, `GRID_DOT_DISTANCE = 32`).
- Audit: `docs/audit/WGPUI_IOS_FEASIBILITY_AUDIT_2026-02-20.md`.
