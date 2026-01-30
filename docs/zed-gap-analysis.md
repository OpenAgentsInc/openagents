# Zed vs Autopilot Desktop (WGPUI) Gap Analysis

Date: 2026-01-30

Scope
- Compare OpenAgents' Autopilot Desktop app in `apps/autopilot-desktop/` plus its WGPUI usage (`crates/autopilot_ui/`, `crates/wgpui/`) against Zed in `~/code/zed`.
- Focus on core editor capabilities and styling/theming parity.
- This is a feature gap report ("what Zed has that we don't") and not a roadmap or implementation plan.

---

## Baseline: OpenAgents Autopilot Desktop + WGPUI (current)

What exists today (code references in parentheses):

- Autopilot Desktop is a WGPUI + winit host that renders a single root component (`MinimalRoot`) and forwards winit input into WGPUI events (`apps/autopilot-desktop/src/main.rs`).
- The UI is a multi-pane, draggable window canvas with specialized panes (Chat, Threads, Events, Identity, Pylon, Wallet, Sell Compute, DVM History, NIP-90) and an internal hotbar (`crates/autopilot_ui/src/lib.rs`).
- Rendering is immediate-mode component painting with explicit bounds calculation; layout is not yet driven by the WGPUI layout engine in this UI surface (`crates/autopilot_ui/src/lib.rs`).
- WGPUI provides GPU rendering, text shaping, SDF primitives, and a Taffy-based layout engine, plus a component system (buttons, inputs, dropdowns, scroll views, virtual lists, markdown rendering) (`crates/wgpui/src/lib.rs`, `crates/wgpui/src/components/*`).
- WGPUI includes a basic multi-line editor component (`LiveEditor`) with undo/redo, selection, and a small Vim-mode subset intended for Markdown editing (used by Onyx) (`crates/wgpui/src/components/live_editor/mod.rs`).
- Theming is a small global theme struct with tokens and a single built-in theme (`MIDNIGHT`) plus app-specific palette helpers in Autopilot (`crates/wgpui/src/theme/mod.rs`, `crates/autopilot/src/app/ui/theme.rs`).

What this means for editor parity:
- Autopilot Desktop has rich agent UI but **no project model, file/buffer editing, or workspace/editor primitives**.
- WGPUI has the rendering primitives to build an editor, but the editor stack itself does not exist in Autopilot Desktop today.

---

## Baseline: Zed (current)

Zed is a full code editor with a mature GPUI-based UI framework, a project/workspace model, and a deep feature stack (docs + code references below).

GPUI (framework) highlights
- Entity/view model with retained state + immediate rendering via `Render` and element trees (`crates/gpui/README.md`, `crates/gpui/docs/contexts.md`).
- Action + key context system for keyboard-first workflows (`crates/gpui/docs/key_dispatch.md`).

Core editor + workspace features (docs)
- Project/workspace model, file finder, project search, tab switcher, outline panel, and symbol navigation (`docs/src/finding-navigating.md`, `docs/src/outline-panel.md`).
- Multi-buffer editing and multi-cursor workflows (`docs/src/multibuffers.md`).
- Language intelligence via Tree-sitter + LSP (completions, diagnostics, code actions, formatting, etc.) (`docs/src/configuring-languages.md`, `docs/src/completions.md`, `docs/src/diagnostics.md`).
- Integrated terminal with multiple tabs, splits, and full configuration (`docs/src/terminal.md`).
- Git panel, staging, diff views, blame, and merge conflict UI (`docs/src/git.md`).
- Tasks system and DAP-based debugger (`docs/src/tasks.md`, `docs/src/debugger.md`).
- Remote development via SSH/WSL (`docs/src/remote-development.md`).
- Fully configurable keybindings with modal editing (Vim/Helix) (`docs/src/key-bindings.md`, `docs/src/vim.md`).

Styling + theming features (docs)
- Theme selector, theme overrides, theme extensions (`docs/src/themes.md`).
- Icon themes (built-in + extension-based) (`docs/src/icon-themes.md`).
- Extensive appearance + UI customization (fonts, cursor styles, minimap, gutter, scrollbars, tab bar, status bar, panels, etc.) (`docs/src/appearance.md`, `docs/src/visual-customization.md`).

---

## Gap Analysis: What Zed Has That Autopilot Desktop + WGPUI Don’t

### 1) Workspace + Project Model
- Project/worktree abstraction, multi-root workspaces, file system watchers, and project panel (file tree) UI.
- Tabs + pane splitting model for editors and terminals.
- File open/save lifecycle, recent projects, and worktree trust.

(See Zed: `docs/src/finding-navigating.md`, `docs/src/visual-customization.md`, `docs/src/worktree-trust.md`)

### 2) Core Editor Engine
- Dedicated code editor buffer model with efficient rope/text storage and large-file handling.
- Multi-cursor editing and multi-buffer editing across files.
- Precise selection model, structural selection, cursor/selection history.
- Rich text operations (indent/outdent, multi-edit, region transforms).

(See Zed: `docs/src/multibuffers.md`, `docs/src/editing-code.md`)

Autopilot/WGPUI status
- Only a basic Markdown-oriented `LiveEditor` component (single buffer, single selection) exists; it is not integrated into Autopilot Desktop or wired to a project model.

### 3) Language Intelligence Stack
- Tree-sitter parsing for syntax highlighting, outline, and structural features.
- LSP management (auto-download, configuration, diagnostics, completions, code actions, rename, formatting, hover).
- Inline diagnostics (error lens), inlay hints, signature help, code lenses.

(See Zed: `docs/src/configuring-languages.md`, `docs/src/completions.md`, `docs/src/diagnostics.md`)

Autopilot/WGPUI status
- No Tree-sitter integration, no LSP client, no diagnostics pipeline.

### 4) Navigation + Search
- File finder, project search with editable results, go-to-definition, find references, symbol search (file + project), outline panel, tab switcher.

(See Zed: `docs/src/finding-navigating.md`, `docs/src/outline-panel.md`)

Autopilot/WGPUI status
- No project-aware navigation, no editor-local navigation primitives.

### 5) Git Integration
- Git panel with staged/unstaged status, commit UI, stash operations, merge conflict helpers.
- Project diff view with editable hunks and inline blame.

(See Zed: `docs/src/git.md`)

Autopilot/WGPUI status
- Autopilot UI can render diffs but has no Git panel, staging workflow, or repo-aware UI.

### 6) Terminal + Tasks + Debugger
- Integrated terminal emulator with tabs/splits and deep editor integration.
- Task runner with per-project and global tasks.
- Debugger (DAP) integration with breakpoints and debug panel.

(See Zed: `docs/src/terminal.md`, `docs/src/tasks.md`, `docs/src/debugger.md`)

Autopilot/WGPUI status
- No terminal panel, task runner UI, or debugger integration.

### 7) Command Palette + Action Surface
- Comprehensive action catalog and command palette for every action.
- Context-aware keybinding system with UI for editing keymaps.

(See Zed: `docs/src/finding-navigating.md`, `docs/src/key-bindings.md`)

Autopilot/WGPUI status
- WGPUI has an action/keymap system, but Autopilot Desktop does not expose a full command palette or user-facing keymap editor.

### 8) Styling & Theming (Full Parity Gap)
- Theme selector + live preview; light/dark theme pairs; theme overrides.
- Theme extension system (installable themes + importers).
- Icon themes for project panel + tabs.
- Fine-grained UI styling controls: fonts (UI/buffer/terminal), ligatures, cursor styles, minimap, gutter, line numbers, scrollbars, tab bar, status bar, panels, active pane styling.

(See Zed: `docs/src/themes.md`, `docs/src/icon-themes.md`, `docs/src/appearance.md`, `docs/src/visual-customization.md`)

Autopilot/WGPUI status
- Global theme struct + fixed palette values; no user theme selection, overrides, or icon theming.
- No equivalent UI settings surface for editor/panel styling.

### 9) Extensions + Ecosystem
- Zed extensions for languages, themes, icon themes, debuggers, and feature add-ons.

(See Zed: `docs/src/extensions.md`, `docs/src/extensions/languages.md`)

Autopilot/WGPUI status
- No extension system for editor features or themes.

### 10) Collaboration + Remote Development
- Real-time collaboration and shared sessions.
- Remote development via SSH/WSL with a split UI/compute model.

(See Zed: `docs/src/remote-development.md`, `docs/src/collaboration/overview.md`)

Autopilot/WGPUI status
- Autopilot Desktop is currently local-only for UI; remote dev isn't integrated into the editor surface.

### 11) GPUI Framework Parity (for WGPUI adaptation)
Zed’s GPUI includes a mature view + action + layout ecosystem used pervasively across the editor:
- View/entity model with `Render` and element trees as the dominant UI pattern.
- Key contexts and action dispatch baked into elements.
- UI testing utilities and app-level services integrated with the framework.

(See Zed GPUI: `crates/gpui/README.md`, `crates/gpui/docs/contexts.md`, `crates/gpui/docs/key_dispatch.md`)

WGPUI status
- WGPUI already has entities, action/keymap primitives, a Taffy layout engine, and components, but Autopilot Desktop’s UI surface does not yet use GPUI-style layout or view trees, and lacks the editor-centric elements Zed depends on.

---

## Core Editor + Styling Parity Targets (Zed → OpenAgents)

If the goal is “most of Zed’s core editor features + all styling” using WGPUI, the missing surfaces to implement include:

Core editor (must-have)
- Project/workspace model + project panel UI.
- Buffer/editor engine (multi-cursor, multibuffer, selections, undo/redo, large file handling).
- Tree-sitter integration for syntax highlighting and outline.
- LSP client stack for completions, diagnostics, code actions, rename, hover, formatting.
- Search/navigation surfaces (file finder, project search, go-to, outline panel, tab switcher).
- Tabs + pane splitting, editors + panels architecture.

Styling (must-have)
- Theme system with light/dark pairs, live selector, overrides, and theme extension format.
- Icon themes for project panel + tabs.
- UI customization: fonts (UI/buffer/terminal), ligatures, line height, cursor styles, gutters, minimap, scrollbars, tab bar/status bar, panel sizes/docking.

Other major surfaces (likely needed for "most" parity)
- Integrated terminal and tasks panel.
- Git panel, diff/staging UI, inline blame.
- Debugger panel (DAP).
- Settings/keymap editors and base keymaps (VS Code/JetBrains/Vim/Helix).

---

## Appendix: Source Pointers (OpenAgents)

- Autopilot Desktop host: `apps/autopilot-desktop/src/main.rs`
- Autopilot Desktop UI root/panes: `crates/autopilot_ui/src/lib.rs`
- WGPUI core: `crates/wgpui/src/lib.rs`
- WGPUI editor component (LiveEditor): `crates/wgpui/src/components/live_editor/mod.rs`
- WGPUI theme tokens: `crates/wgpui/src/theme/mod.rs`
- Autopilot palette: `crates/autopilot/src/app/ui/theme.rs`

## Appendix: Source Pointers (Zed)

- GPUI framework: `~/code/zed/crates/gpui/README.md`, `~/code/zed/crates/gpui/docs/contexts.md`, `~/code/zed/crates/gpui/docs/key_dispatch.md`
- Editor features: `~/code/zed/docs/src/editing-code.md`, `~/code/zed/docs/src/multibuffers.md`
- Language support: `~/code/zed/docs/src/configuring-languages.md`, `~/code/zed/docs/src/completions.md`, `~/code/zed/docs/src/diagnostics.md`
- Navigation: `~/code/zed/docs/src/finding-navigating.md`, `~/code/zed/docs/src/outline-panel.md`
- Git: `~/code/zed/docs/src/git.md`
- Terminal: `~/code/zed/docs/src/terminal.md`
- Tasks: `~/code/zed/docs/src/tasks.md`
- Debugger: `~/code/zed/docs/src/debugger.md`
- Themes + styling: `~/code/zed/docs/src/themes.md`, `~/code/zed/docs/src/icon-themes.md`, `~/code/zed/docs/src/appearance.md`, `~/code/zed/docs/src/visual-customization.md`
- Keybindings + Vim: `~/code/zed/docs/src/key-bindings.md`, `~/code/zed/docs/src/vim.md`
- Remote dev: `~/code/zed/docs/src/remote-development.md`
