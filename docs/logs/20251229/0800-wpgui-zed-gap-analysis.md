# WGPUI vs Zed GPUI Gap Analysis (2025-12-29 08:00)

## Scope
- WGPUI: `crates/wgpui` (OpenAgents)
- Zed GPUI: `/home/christopherdavid/code/zed/crates/gpui`

## Summary
Zed GPUI covers a much larger platform/runtime surface, richer element primitives (image/svg/canvas/surface), a full keybinding/action system, and a deeper text/layout/style pipeline. WGPUI focuses on quads + text rendering and a domain-specific component library, with minimal platform and input plumbing.

## Gaps (Zed has, WGPUI does not)

### Platform backends and OS integration
- Multi-backend native rendering and windowing: Metal, DirectX, Blade, Linux (Wayland/X11), headless. See `/home/christopherdavid/code/zed/crates/gpui/src/platform/*` and `/home/christopherdavid/code/zed/crates/gpui/src/platform/blade.rs`.
- Clipboard, drag/drop, screen capture, app menus, status items, window prompts, system settings, display management. See `/home/christopherdavid/code/zed/crates/gpui/src/platform/*`, `/home/christopherdavid/code/zed/crates/gpui/src/platform/app_menu.rs`, `/home/christopherdavid/code/zed/crates/gpui/src/platform/mac/status_item.rs`, `/home/christopherdavid/code/zed/crates/gpui/src/window/prompts.rs`.
- WGPUI has a stub desktop platform that does not render or handle OS integration. See `crates/wgpui/src/platform.rs`.

### Input, actions, and keymaps
- Action system + macros and keymap-driven key dispatch with key contexts. See `/home/christopherdavid/code/zed/crates/gpui/src/action.rs`, `/home/christopherdavid/code/zed/crates/gpui/src/keymap/*`, `/home/christopherdavid/code/zed/crates/gpui/src/key_dispatch.rs`, `/home/christopherdavid/code/zed/crates/gpui/docs/key_dispatch.md`.
- Rich input event model (click counts, pressure, touch, IME, modifiers changes). See `/home/christopherdavid/code/zed/crates/gpui/src/interactive.rs`.
- WGPUI input is limited to basic mouse/scroll/key events without keymap or action dispatch. See `crates/wgpui/src/input.rs`.

### Element primitives and retained-mode building blocks
- Zed elements: `img`, `svg`, `canvas`, `surface` (offscreen), `deferred`, `anchored`, `list`, `uniform_list`, `animation`. See `/home/christopherdavid/code/zed/crates/gpui/src/elements/*`.
- WGPUI exposes `Div`, `Text`, `Button`, `TextInput`, `ScrollView`, `VirtualList`, `Modal`, `Dropdown`, `Tabs` plus app-specific components; no image/svg/canvas/surface/deferred elements. See `crates/wgpui/src/components/*` and `crates/wgpui/src/scene.rs`.

### Vector graphics and images
- SVG renderer and path builder in Zed. See `/home/christopherdavid/code/zed/crates/gpui/src/svg_renderer.rs`, `/home/christopherdavid/code/zed/crates/gpui/src/path_builder.rs`, `/home/christopherdavid/code/zed/crates/gpui/src/elements/svg.rs`.
- Image cache and image element. See `/home/christopherdavid/code/zed/crates/gpui/src/elements/image_cache.rs`, `/home/christopherdavid/code/zed/crates/gpui/src/elements/img.rs`.
- WGPUI has image loading helpers but no render path or element. See `crates/wgpui/src/tools.rs`.

### Text system depth
- Zed text system includes line layout/wrapping, font features, font fallbacks, tab stops. See `/home/christopherdavid/code/zed/crates/gpui/src/text_system/*` and `/home/christopherdavid/code/zed/crates/gpui/src/tab_stop.rs`.
- ~~WGPUI text shaping is minimal; measurement is fixed-width heuristic and layout assumes a single line. See `crates/wgpui/src/text.rs`.~~ **DONE** (2025-12-29) - Text system upgraded with line wrapping, decoration runs, layout caching. See `crates/wgpui/src/text_system/*`.

### Styling and layout breadth
- Zed style supports large CSS-like surface: overflow, position, border radii, shadows, background fills, cursor, opacity, etc. See `/home/christopherdavid/code/zed/crates/gpui/src/style.rs` and `/home/christopherdavid/code/zed/crates/gpui/src/styled.rs`.
- WGPUI style is limited to layout + background/border/text fields. See `crates/wgpui/src/styled/style.rs` and `crates/wgpui/src/styled/refinement.rs`.

### App/runtime services and globals
- Zed has `Global` state, `AppContext` trait, async contexts, and window management via `App::open_window`. See `/home/christopherdavid/code/zed/crates/gpui/src/global.rs`, `/home/christopherdavid/code/zed/crates/gpui/src/app/*`, `/home/christopherdavid/code/zed/crates/gpui/src/window.rs`, `/home/christopherdavid/code/zed/crates/gpui/src/view.rs`.
- WGPUI `App` is entity-only and lacks window management or global registry. See `crates/wgpui/src/app/app_context.rs`.

### Debugging and profiling
- Runtime inspector and profiler in Zed. See `/home/christopherdavid/code/zed/crates/gpui/src/inspector.rs`, `/home/christopherdavid/code/zed/crates/gpui/src/profiler.rs`.
- WGPUI has no equivalent.

### Testing and headless platform
- Zed test support includes headless platform and test contexts. See `/home/christopherdavid/code/zed/crates/gpui/src/platform/test/*`, `/home/christopherdavid/code/zed/crates/gpui/src/test.rs`, `/home/christopherdavid/code/zed/crates/gpui/src/app/test_context.rs`.
- WGPUI has a custom testing framework but not a headless platform or `gpui::test` macro. See `crates/wgpui/src/testing/*`.

### Data structures and utilities
- Shared string/URI wrappers for memory and ownership control. See `/home/christopherdavid/code/zed/crates/gpui/src/shared_string.rs`, `/home/christopherdavid/code/zed/crates/gpui/src/shared_uri.rs`.
- Bounds tree and queue primitives. See `/home/christopherdavid/code/zed/crates/gpui/src/bounds_tree.rs`, `/home/christopherdavid/code/zed/crates/gpui/src/queue.rs`.
- WGPUI does not have equivalents.

## Immediate port candidates (component-level)
- `img`, `svg`, `canvas`, `surface`, `deferred`, `anchored`, `list`, `uniform_list`, `animation` from `/home/christopherdavid/code/zed/crates/gpui/src/elements/*`.
- `svg_renderer` + `path_builder` for vector drawing.
- `image_cache` and `asset_cache` for asset lifecycle.
- ~~`action` + `keymap` system for keyboard-first workflows.~~ **DONE** (2025-12-29)

---

## Implementation Status (2025-12-29)

### Completed: Action + Keymap System

The action/keymap system has been fully implemented. This closes the "Input, actions, and keymaps" gap identified above.

**Files created (14):**

| Module | Files |
|--------|-------|
| `crates/wgpui/src/action/` | `mod.rs`, `action.rs`, `keystroke.rs`, `binding.rs`, `registry.rs`, `dispatch.rs`, `macros.rs`, `standard.rs` |
| `crates/wgpui/src/keymap/` | `mod.rs`, `context.rs`, `keymap.rs`, `defaults.rs` |
| `crates/wgpui/src/` | `interactive.rs` |

**Files modified:**
- `lib.rs` - Module exports
- `input.rs` - Added `PartialEq`, `Eq`, `Hash` to `Key`, `Modifiers`, `NamedKey`
- `components/context.rs` - Extended `EventContext` with `KeyContext` and action dispatch

**Features implemented:**
- `Action` trait with type-safe dispatch and runtime type erasure
- `Keystroke` parsing from strings ("cmd-shift-s")
- `KeyBinding` connecting keystrokes to actions with optional context
- `Keymap` with precedence-based resolution (context depth > binding order)
- `KeyContext` stack for scoped bindings
- `ActionListeners` for per-component action handlers
- `Interactive` trait with fluent `.on_action()` and `.key_context()` API
- `actions!` and `action!` macros for concise action definitions
- 21 standard actions (MoveUp, Copy, Cancel, Save, etc.)
- Default keymap with common bindings

**Documentation:** `crates/wgpui/docs/action-keymap-system.md`

**Simplifications vs Zed:**
- Manual registration (no `inventory` crate auto-registration)
- Simple string context matching (no complex predicates like `"Editor && mode == vim"`)
- Single keystrokes only (no multi-key sequences like "ctrl-k ctrl-c")
- Bubble phase only (no capture phase)

---

## Recommended Next Steps

### Priority 1: Text System Upgrade
Replace fixed-width heuristic with proper text layout:
- Line wrapping
- Font fallbacks
- Tab stops

**Key files to study:**
- `/home/christopherdavid/code/zed/crates/gpui/src/text_system/*`
- `/home/christopherdavid/code/zed/crates/gpui/src/tab_stop.rs`

### Priority 2: SVG Renderer
Port `svg_renderer.rs` + `path_builder.rs`. Enables:
- Vector icons without rasterization
- Scalable graphics
- Custom shapes

**Key files to study:**
- `/home/christopherdavid/code/zed/crates/gpui/src/svg_renderer.rs`
- `/home/christopherdavid/code/zed/crates/gpui/src/path_builder.rs`
- `/home/christopherdavid/code/zed/crates/gpui/src/elements/svg.rs`

### Priority 3: Image Element + Cache
Port from Zed's `img.rs` and `image_cache.rs`. Enables:
- User avatars
- Icons
- Data visualizations

**Key files to study:**
- `/home/christopherdavid/code/zed/crates/gpui/src/elements/img.rs`
- `/home/christopherdavid/code/zed/crates/gpui/src/elements/image_cache.rs`

### Priority 4: Expand StyleRefinement
Add more CSS-like properties:
- Overflow handling
- Box shadows
- Opacity
- Cursor styles

### Priority 5: Inspector/Profiler
Add runtime debugging tools for development
