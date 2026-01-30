# Zed Parity Roadmap (Core Editor + Styling) for Autopilot Desktop

Date: 2026-01-30

Scope
- Goal: reach **most core editor features** of Zed and **full styling parity**, using WGPUI (WGPU-backed) and adapting GPUI patterns where needed.
- This roadmap complements `docs/zed-gap-analysis.md` and focuses on *ordered delivery* and dependencies.
- Assumes we will **not** embed Zed’s GPUI crate directly (licensing + architectural mismatch); we adapt GPUI patterns into WGPUI.

Guiding constraints
- WGPUI is the UI runtime (no WebView).
- Existing OpenAgents primitives should be reused: `crates/editor`, `crates/vim`, WGPUI element/layout/action/keymap modules.
- Keep Autopilot UI intact; editor surfaces are additive and can coexist (agent panel + editor workspace).

---

## Phase 0 — UI + Framework Alignment (WGPUI as GPUI-like host)

Purpose: establish the **layout, action, and view patterns** that Zed relies on so the editor UI can be built consistently.

Deliverables
- **Adopt element-tree + layout-first UI** in Autopilot Desktop (replace manual bounds math).
  - Use `wgpui::element` + `wgpui::styled` + `wgpui::layout` in new editor surfaces.
- **Standardize action/keymap dispatch** across editor and panes.
  - Wire `wgpui::action` + `wgpui::keymap` into workspace root.
- **Unified focus model** for editor widgets and panels.
  - Leverage `wgpui::focus` and `wgpui::input` contexts.
- **UI test harness** for editor components (use `crates/wgpui/src/testing/*`).

Dependencies
- WGPUI element/layout APIs must be the primary layout mechanism.

Exit criteria
- A minimal workspace UI renders from a declarative element tree (no manual rect layout).
- Action dispatch + keymap context is used for basic commands (open file, focus panel).

---

## Phase 1 — Workspace + Project Skeleton

Purpose: deliver the Zed-style *workspace frame* that everything else plugs into.

Deliverables
- **Workspace model**: worktree(s), file tree, tabs, panes (dockable panels).
- **Project panel** with file tree, basic open/reveal.
- **Tabs + splits**: pane container with horizontal/vertical splits.
- **File open/save** pipeline (disk I/O, dirty state).

Where to build
- New or expanded crates: `crates/workspace`, `crates/project`, `crates/panel` (names flexible).
- UI in `crates/autopilot_ui` (new editor surfaces) or a new `crates/editor_ui`.

Exit criteria
- Open a repo, browse file tree, open file in editor tab, save edits.

---

## Phase 2 — Editor Core (Leverage `crates/editor`)

Purpose: turn the existing editor foundation into a Zed-class core editor.

Current assets
- `crates/editor` already provides:
  - Rope-based `TextBuffer` (ropey)
  - Multi-cursor and selection ranges
  - Undo/redo stack
  - `EditorView` WGPUI component with basic rendering and input handling
  - Simple Rust syntax highlighting (non Tree-sitter)

Phase deliverables
- **Expand `crates/editor` to Zed-like editing ops**:
  - Multi-cursor selection commands (select next/previous, select all matches)
  - Multi-buffer (editable excerpt) support
  - Structural selection hooks (for Tree-sitter later)
  - Search/replace engine (regex + plain)
  - Clipboard + kill-ring abstractions
- **EditorView 2.0**:
  - Performance tuning for large files
  - Accurate scrolling, line wrapping, soft wrap modes
  - Selection rendering, block selections, multi-cursor carets
  - Input method editor (IME) support (already partially present)

Dependencies
- Phase 1 workspace shells (tabs/panes) to host the editor.

Exit criteria
- Multi-cursor editing, search/replace, and multi-buffer editing work in UI.

---

## Phase 3 — Language Intelligence (Tree-sitter + LSP)

Purpose: provide the core “smart editor” behaviors that distinguish Zed.

Deliverables
- **Tree-sitter integration** for syntax highlighting + outline models.
  - Replace the current ad-hoc Rust highlighter in `crates/editor::syntax`.
- **LSP client** for completions, diagnostics, hover, code actions, rename, formatting.
- **Diagnostics UI** (gutter marks, inline hints, diagnostics list).

Dependencies
- Editor core with selection + buffer ops.
- Project/workspace knows language server lifecycle.

Exit criteria
- Syntax highlighting from Tree-sitter; LSP completions and diagnostics visible in editor.

---

## Phase 4 — Navigation + Search Surfaces

Purpose: reach the Zed-level navigation experience.

Deliverables
- File finder + tab switcher + command palette.
- Project search with editable results (multibuffer integration).
- Outline panel and symbol search (file + project).
- Go to definition / references with multibuffer view.

Dependencies
- Phase 3 (Tree-sitter + LSP) for symbol features.

Exit criteria
- Navigation coverage matches Zed docs: file finder, project search, outline panel, go to def/refs.

---

## Phase 5 — Styling & Theming Parity (Full)

Purpose: match Zed’s theming and UI customization surface.

Deliverables
- **Theme system**: light/dark pairs, theme selector, theme overrides.
- **Icon themes**: project panel + tabs + file icons.
- **UI customization settings**: fonts, ligatures, line height, cursor shapes, gutters, scrollbars, minimap, status bar, tab bar, panel docking sizes.

Dependencies
- Editor and workspace UI need to expose style hooks.

Exit criteria
- Themes and icon themes are switchable at runtime; UI settings match Zed coverage.

---

## Phase 6 — Core Panels (Terminal, Git, Tasks, Debugger)

Purpose: reach “most of Zed” beyond editor core.

Deliverables
- Terminal panel with tabs/splits and task runner integration.
- Git panel + staging + diff view + inline blame.
- DAP debugger panel and breakpoint UI.

Dependencies
- Workspace layout, command palette, and settings infrastructure.

Exit criteria
- Panels function with basic parity to Zed docs.

---

## Phase 7 — Optional Parity (Remote Dev + Collaboration)

Purpose: Zed parity for advanced workflows.

Deliverables
- SSH remote development with split UI/compute.
- Collaboration session support (if aligned with OpenAgents product direction).

Exit criteria
- Remote projects usable with LSP, tasks, and terminal.

---

## Suggested Ordering Summary

1. Framework alignment (Phase 0)
2. Workspace shell (Phase 1)
3. Editor core (Phase 2)
4. Language intelligence (Phase 3)
5. Navigation (Phase 4)
6. Styling parity (Phase 5) — can start in parallel once workspace/editor render
7. Terminal/Git/Tasks/Debugger (Phase 6)
8. Remote/Collab (Phase 7, optional)

---

## Notes on Existing OpenAgents Assets

- `crates/editor` and `crates/editor::EditorView` are a real foundation and should be expanded, not replaced.
- `crates/vim` provides a full editor-agnostic vim emulation layer ready for integration with `crates/editor`.
- WGPUI already contains action/keymap, focus, layout, and testing primitives; the main gap is **wiring them into the editor workspace**.

