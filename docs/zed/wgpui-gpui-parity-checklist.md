# WGPUI ↔ GPUI Parity Checklist (Mapped to OpenAgents Modules)

Date: 2026-01-30

Purpose
- Provide a practical checklist for matching Zed’s GPUI patterns and editor surfaces using WGPUI.
- Map each capability to OpenAgents modules/crates so ownership is clear.
- Status legend: ✅ present, 🟡 partial, ❌ missing, ⚠️ present but not wired into Autopilot Desktop.

Reference
- Zed GPUI: `~/code/zed/crates/gpui/*`
- Zed UI/editor stack: `~/code/zed/crates/ui`, `~/code/zed/crates/editor`, `~/code/zed/crates/workspace`
- OpenAgents WGPUI: `crates/wgpui/*`
- OpenAgents editor foundation: `crates/editor/*`

---

## 1) Core Framework: App/Context/Entity

- ✅ **Entity + Context model**
  - OpenAgents: `crates/wgpui/src/app/*` (`App`, `Context`, `Entity`)
  - Zed GPUI: entity + context model (`crates/gpui/docs/contexts.md`)

- 🟡 **Async/test contexts parity**
  - OpenAgents: `crates/wgpui/src/async/*`, `crates/wgpui/src/testing/*`
  - Zed GPUI provides async/test contexts with richer ergonomics.
  - Gap: WGPUI test context API parity and async context fallibility behavior.

## 2) Element Tree + Layout

- ✅ **Element + Render/RenderOnce**
  - OpenAgents: `crates/wgpui/src/element/*`
  - Zed GPUI: `Render` + element tree.

- ✅ **Layout engine (Taffy)**
  - OpenAgents: `crates/wgpui/src/layout.rs`, `layout_helpers.rs`

- ⚠️ **Autopilot Desktop does not use layout engine**
  - Current Autopilot UI uses manual bounds and pane math: `crates/autopilot_ui/src/lib.rs`.

- 🟡 **Styled helpers parity**
  - OpenAgents: `crates/wgpui/src/styled/*` (`div`, `h_flex`, `v_flex`)
  - Zed’s `ui` crate has a richer fluent API (min_w_0, overflow helpers, conditional styling).

## 3) Action + Keymap System

- ✅ **Action model**
  - OpenAgents: `crates/wgpui/src/action/*`

- ✅ **Keymap + context stack**
  - OpenAgents: `crates/wgpui/src/keymap/*`

- 🟡 **Element-level action dispatch parity**
  - OpenAgents supports actions but lacks a Zed-style ubiquitous `on_action` pattern in editor UI.
  - Needs consistent wiring in workspace/editor root.

## 4) Input + Focus + Hit Testing

- ✅ **Input events**: `crates/wgpui/src/input.rs`
- ✅ **Focus system**: `crates/wgpui/src/focus/*`
- ✅ **Hit testing**: `crates/wgpui/src/hit_test.rs`
- ⚠️ **Autopilot Desktop** only translates winit input to WGPUI events; focus contexts are not systematically used.

## 5) Text System + Rendering

- ✅ **Text shaping + atlas**: `crates/wgpui/src/text_system/*`, `text.rs`
- ✅ **Markdown rendering**: `crates/wgpui/src/markdown/*`
- 🟡 **Editor-specific text layout**
  - OpenAgents: `crates/editor/src/view.rs` implements line layout + caret/selection.
  - Needs advanced line wrapping, shaped-line caching, and performance tuning to match Zed.

## 6) Accessibility

- ✅ **Accessibility model exists**: `crates/wgpui/src/accessibility.rs`
- 🟡 **Integration depth**
  - Zed uses accessibility across the UI; WGPUI has the types but not a full pipeline.

## 7) Window/Platform + Clipboard

- ✅ **Platform + window abstractions**: `crates/wgpui/src/platform.rs`, `crates/wgpui/src/window/*`
- ✅ **Clipboard helpers**: `crates/wgpui/src/clipboard.rs`
- ⚠️ **Autopilot Desktop** still manages winit directly (not via a unified WGPUI platform shell).

## 8) Styling + Theming System

- ✅ **Theme tokens**: `crates/wgpui/src/theme/*`
- 🟡 **Theming surface**
  - OpenAgents has static tokens; Zed has theme selector, overrides, and extensions.
- ❌ **Icon themes + theme extensions**
  - No icon theme pipeline in OpenAgents (no file icon registry or theme import).

## 9) UI Components / Widgets

- ✅ **Buttons, inputs, dropdowns, scroll views, markdown views**
  - OpenAgents: `crates/wgpui/src/components/*`
- 🟡 **Virtual lists** exist but editor-sized lists (project tree, outline, completions) are not implemented.
- ❌ **Workspace-specific widgets**: project panel, tabs, splits, terminal panel, git panel.

## 10) Editor Core (OpenAgents-specific)

- ✅ **Text buffer + cursor model**
  - `crates/editor/src/buffer.rs`, `caret.rs`, `editor.rs`
- ✅ **WGPUI editor view**
  - `crates/editor/src/view.rs`
- 🟡 **Syntax highlighting**
  - Current: simple Rust scanner (`crates/editor/src/syntax.rs`)
  - Missing: Tree-sitter, multi-language, semantic styling
- 🟡 **Multi-cursor + selection ops** exist but need feature depth (select next/prev, multi-buffer editing).
- ❌ **Project/workspace integration**
  - Editor not wired into Autopilot Desktop or a workspace/pane system.

## 11) Vim/Modal Editing

- ✅ **Vim engine**: `crates/vim/*` (editor-agnostic)
- ❌ **Integration** with `crates/editor` + keymap contexts.

## 12) LSP + Diagnostics + Code Intelligence

- ❌ **LSP client**
- ❌ **Diagnostics UI** (gutter, inline diagnostics, diagnostics list)
- ❌ **Code actions, rename, hover, completions**

## 13) Search + Navigation Surfaces

- ❌ **File finder / project search / symbol search**
- ❌ **Outline panel + tab switcher**
- ❌ **Go-to definition / references**

## 14) Git, Terminal, Tasks, Debugger

- ❌ **Terminal panel + task runner**
- ❌ **Git panel + staging/diff UI**
- ❌ **DAP debugger panel**

---

## Autopilot Desktop Wiring Gaps (Immediate Opportunities)

- ⚠️ **EditorView is not embedded** in Autopilot Desktop UI.
- ⚠️ **Layout engine is unused**; current UI uses manual bounds in `crates/autopilot_ui/src/lib.rs`.
- ⚠️ **Action/keymap system is present** but not the primary command surface.

---

## Suggested Parity Workstreams

1. **Framework wiring** (element/layout/action/focus) → Autopilot Desktop
2. **Workspace shell + project panel** → new workspace crate + UI
3. **Editor core expansion** → `crates/editor` + `EditorView`
4. **Tree-sitter + LSP** → language intelligence
5. **Styling parity** → themes, icons, UI customization
6. **Terminal/Git/Debugger** → supporting panels

