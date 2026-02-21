# WGPUI on iOS — Reference for Coding Agents

WGPUI (the repo’s GPU UI crate) renders the iOS background: dots grid, puffs, and a login card. This doc tells future agents where that code lives and how to change it.

## Where the code lives

| Layer | Location | Notes |
|-------|----------|--------|
| **Rust — WGPUI iOS platform** | `crates/wgpui/src/platform.rs` | `#[cfg(feature = "ios")] pub mod ios`: `IosBackgroundState`, C FFI, `render()` |
| **Rust — iOS feature** | `crates/wgpui/Cargo.toml` | `[features] ios = ["pollster"]` |
| **Rust — client-core re-export** | `crates/openagents-client-core/src/lib.rs` | `#[cfg(target_os = "ios")] pub use wgpui::IosBackgroundState` so the static lib keeps wgpui symbols |
| **Rust — client-core dep** | `crates/openagents-client-core/Cargo.toml` | `[target.'cfg(target_os = "ios")'.dependencies] wgpui = { ..., features = ["ios"] }` |
| **Swift — bridge** | `apps/autopilot-ios/Autopilot/Autopilot/WgpuiBackgroundBridge.swift` | Loads `wgpui_ios_background_*` (main exe or `Autopilot.debug.dylib`), exposes create/render/resize/destroy |
| **Swift — view** | `apps/autopilot-ios/Autopilot/Autopilot/WgpuiBackgroundView.swift` | `CAMetalLayer` UIView, `CADisplayLink` → render each frame |
| **Swift — root UI** | `apps/autopilot-ios/Autopilot/Autopilot/ContentView.swift` | Uses `WgpuiBackgroundView` as background (with or without Codex UI on top) |

## C FFI (Rust → Swift)

In `crates/wgpui/src/platform.rs` (ios module):

- `wgpui_ios_background_create(layer_ptr, width, height, scale)` → opaque pointer to `IosBackgroundState`
- `wgpui_ios_background_render(state)` → render one frame
- `wgpui_ios_background_resize(state, width, height)`
- `wgpui_ios_background_destroy(state)`

Swift calls these via the bridge; the layer pointer is the view’s `CAMetalLayer`. No input events are passed from Swift to Rust yet (background is display-only).

## What gets rendered (order)

Inside `IosBackgroundState::render()` in `platform.rs`:

1. **Black fullscreen quad**
2. **Puffs** — `PuffsBackground` (Arwes-style), grid-aligned, floating up; driven by `update_with_delta(Entered, delta)` each frame
3. **Dots grid** — `DotsGrid`, circle shape, theme muted, 32px spacing
4. **Login card** — centered card: “Log in” title, email `TextInput`, “Log in” `Button` (visual only until input is wired)

To change background content (add/remove/reorder), edit `render()` in `crates/wgpui/src/platform.rs` (ios module).

## WGPUI components available

From `crates/wgpui`: `Button`, `ButtonVariant`, `Text`, `TextInput`, `Div`, `Quad` (scene), `PaintContext`, `DotsGrid`, `PuffsBackground`, `theme::*`. Puffs support `.grid_distance(Some(d))` to spawn on grid columns (same spacing as dots). Scene API: `draw_quad(Quad::new(bounds).with_background(...).with_corner_radius(...))`, then component `.paint(bounds, &mut paint)`.

## Theme (Rust)

Use `crate::theme` in platform.rs: e.g. `theme::bg::ELEVATED`, `theme::text::PRIMARY`, `theme::text::MUTED`, `theme::border::DEFAULT`, `theme::font_size::*`, `theme::spacing::*`.

## After changing Rust (wgpui or client-core)

1. Rebuild the iOS artifact so the XCFramework/static lib contains the new code:
   ```bash
   ./apps/autopilot-ios/scripts/build-rust-client-core.sh --clean
   ```
2. Build and run the app in Xcode again.

Swift bridge and view rarely need changes unless you add new FFI entrypoints or change how the layer is provided.

## References

- **Building the app / XCFramework:** `BUILDING.md`
- **Rust client core:** `rust-client-core-integration.md`
- **Cutover log:** `../../../docs/autopilot/wgpui-ios-cutover-log.md`
- **WGPUI audit:** `docs/audit/WGPUI_IOS_FEASIBILITY_AUDIT_2026-02-20.md`
