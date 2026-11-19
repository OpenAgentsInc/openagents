# Zeron Components, Skin & App Integrations Ported to OpenAgents

> Reference catalog of all Zeron-derived UI components, skin tokens, desktop/web/mobile integrations, and verification suites currently ported into the OpenAgents repository (written in **Coax** with thin C/Objective-C/JS hosts).

For documentation of the upstream `zeronsh/zeron` Rust/Luau repository itself, see [`README.md`](./README.md), [`features.md`](./features.md), [`architecture.md`](./architecture.md), and [`audit.md`](./audit.md).

---

## 1. Executive Summary

OpenAgents ports Zeron's visual design system (**Zeron Dark**), chat UI primitives, and multi-session desktop workspace into pure **Coax** modules rendered onto a shared GPU primitive/atlas pipeline (Metal/CoreText on Apple, Vulkan/Wayland on Linux, WebGPU in the browser). Rather than embedding Zeron's Rust `zeron-engine` or Luau UI runtime, OpenAgents re-implements the UI components in Coax so that the same compiled layout, text editing, Markdown, and vertex-generation code runs across:

- **`apps/openagents-desktop`** — macOS and Linux native multi-session AI agent desk (Claude Code, Psionic, Lab/Elephant router, Codex).
- **`apps/openagents-web`** — `openagents.com/chat` and `agents` pages rendered via WebGPU (`chat.wgsl` + `coax/web/gpui.js`).
- **`apps/openagents-ios`** — canvas-only text editor adapter reusing `components/zeron/editor.coax`.
- **`apps/metaverse-engine-desktop`** & **`apps/shared/app`** — shared GPUI primitives and tool-chip rendering.
- **`components/skin/`** — `zeron_skin()` serves as the canonical base `UiSkin` from which every other OpenAgents skin (`arcade`, `everglade`, `metaverse`, `terminal-pixels`) derives.

**Codebase footprint (ported Zeron surface):**

| Area | Files | Lines | Inline `#test` Cases |
| --- | ---: | ---: | ---: |
| `components/zeron/*.coax` (11 modules + 4 check suites) | 15 | 4,799 | 69 |
| `components/skin/zeron.coax` | 1 | 414 | 1 |
| `apps/openagents-desktop/zeron/*.coax` | 12 | 4,294 | 60 |
| `apps/openagents-desktop/zeron-face.c` | 1 | 221 | — |
| `apps/openagents-desktop/tests/*zeron*` | 3 | 425 | — |
| **Total** | **32** | **10,153** | **130** |

---

## 2. Attribution & Bundled Assets (`components/zeron/`)

`components/zeron/NOTICE` documents the upstream licenses and attributions for all ported visual assets and design inspirations:

- **Zeron by Wing** (MIT) — visual design, Zeron Dark palette, composer/transcript/picker/desk layout.
- **Geist & Geist Mono** (SIL Open Font License 1.1) — bundled in `components/zeron/fonts/`:
  - `Geist.ttf`, `Geist-Medium.ttf`, `Geist-SemiBold.ttf`, `GeistMono.ttf`, `Geist-OFL.txt`.
- **Icons** (MIT / ISC — Solar Linear, Tabler, Lucide) — 37 SVG icons bundled in `components/zeron/icons/` (arrows, chevrons, paperclip, send/stop, harness marks, status indicators, etc.) plus `components/zeron/icons/archive/`.
- **GPUI** (Apache-2.0) — inspiration for the quad/glyph/atlas GPU rendering primitives.

---

## 3. Core Zeron Component Library (`components/zeron/`)

All reusable Zeron-styled UI components live in `components/zeron/`. They are pure Coax modules with no platform I/O, outputting GPU primitives/vertices against an injected `Atlas` and `UiSkin`.

### 3.1 Production Modules

| Module | Lines | `#test` | Key Types & Responsibilities |
| --- | ---: | ---: | --- |
| [`gpui.coax`](../../components/zeron/gpui.coax) | 33 | 0 | Thin compatibility shim re-exporting `components/raster/gpui.coax` (atlas slots, quad/glyph primitives, vertex emission). |
| [`markdown.coax`](../../components/zeron/markdown.coax) | 37 | 1 | Thin compatibility shim re-exporting `components/kit/markdown.coax` for Zeron transcript Markdown layout & styling. |
| [`chrome.coax`](../../components/zeron/chrome.coax) | 127 | 1 | Shared page chrome primitives: buttons, text inputs, notices, and section headers used by web chat and desktop surfaces. |
| [`tools.coax`](../../components/zeron/tools.coax) | 214 | 3 | Tool-call chip and collapsible tool-group rendering (tool name, status badge, summary/detail expansion) shared with `apps/shared/app/view/page.coax`. |
| [`chat-lines.coax`](../../components/zeron/chat-lines.coax) | 223 | 4 | Unified chat state line-protocol reader (`Z`, `C`, `S`, `U`, `T` record lines) shared between `apps/openagents-web/chat.coax` and `chat/chat-hud.coax`. |
| [`chat-lines-profile.coax`](../../components/zeron/chat-lines-profile.coax) | 259 | 0 | Profiling and throughput harness for `chat-lines.coax` parsing. |
| [`editor.coax`](../../components/zeron/editor.coax) | 224 | 13 | **`UiEditor`** — reusable single-line (`no_wrap`) and multi-line canvas text editor with caret/anchor selection, undo/redo history stacks, word-boundary navigation, placeholder text, and IME marked/provisional composition support. |
| [`attachments.coax`](../../components/zeron/attachments.coax) | 273 | 3 | **`AtFile`**, **`Attachments`** — staged file attachment list and inline removable chips (paperclip picker, drag-and-drop, clipboard paste). |
| [`picker.coax`](../../components/zeron/picker.coax) | 846 | 5 | Popover menus attached to composer chips: compact **model picker** (reasoning-effort slider above the model list, plus mode tabs on empty chats) and **access-mode menu**. |
| [`composer.coax`](../../components/zeron/composer.coax) | 1,051 | 18 | **`Composer`** — centered new-chat form and docked in-chat composer wrapping `editor.coax`, inline attachment chips, model/access chips, Send-to-Stop state transition, clipboard paste, and keyboard/IME handling. |
| [`transcript.coax`](../../components/zeron/transcript.coax) | 1,125 | 9 | **`Transcript`**, **`SelPos`** — 736 px centered conversation column rendering user message bubbles, streaming assistant Markdown (headings, lists, blockquotes, fenced code blocks, and tables with per-column minimums and clipping), tool groups, text selection, scrolling, and model/credential footers. |

### 3.2 Component Verification & Golden Frame Suites

Four dedicated test suites in `components/zeron/` pin pixel/vertex determinism and skin independence:

| Test File | Lines | `#test` | What It Verifies |
| --- | ---: | ---: | --- |
| [`composer-frames-checks.coax`](../../components/zeron/composer-frames-checks.coax) | 136 | 5 | Focused composer golden vertex-buffer hashes (without unrelated transcript/menu fixtures); also loaded by `coax/native/gpu-editor-check.coax`. |
| [`frames-checks.coax`](../../components/zeron/frames-checks.coax) | 72 | 2 | Pinned full-frame transcript and menu rendering checks. |
| [`frame-hash-checks.coax`](../../components/zeron/frame-hash-checks.coax) | 30 | 1 | Vertex-buffer hash stability across deterministic atlas inputs. |
| [`skins-checks.coax`](../../components/zeron/skins-checks.coax) | 149 | 4 | Multi-skin parity: verifies Zeron components render cleanly across skins registered in `components/skin/all.coax` and checks distinct palette colors. |

---

## 4. The Zeron Skin & Multi-Skin Architecture (`components/skin/zeron.coax`)

`components/skin/zeron.coax` (414 lines, 1 `#test`) defines the **Zeron Dark** design system as a value of the shared `UiSkin` struct via `zeron_skin() -> UiSkin`, plus the supporting `Theme` struct.

### 4.1 Token Groups Defined in `zeron_skin()`

- **Palette & Surfaces:** Canvas, elevated surfaces, borders/hairlines, foreground text hierarchy (primary, secondary, muted), accent and semantic status colors (running, error, approval/warning, tool badges).
- **Typography:** Font family/weight slots mapped onto the bundled Geist & Geist Mono faces (`Geist`, `Geist-Medium`, `Geist-SemiBold`, `GeistMono`), line heights, and Markdown heading/body/code measures.
- **Radii, Spacing & Control Metrics:** Panel/card/bubble/control corner radii (`radius_control`, knob/button heights, padding, focus-ring strokes).
- **Component Token Sub-structs:**
  - `UiTranscript` — 736 px column geometry, bubble/Markdown spacing, table hairlines, code block backgrounds.
  - `UiComposer` — card elevation, chip spacing, send/stop button metrics.
  - `UiMenus` — popover elevation, item row heights, slider track/thumb tokens.
  - `UiDesk` — sidebar width bounds (208–400 px, default 256 px), tab bar metrics, folder/session row styling.

### 4.2 Role in the Layered `components/` Architecture

As documented in [`components/README.md`](../../components/README.md), OpenAgents structures UI into layered, backend-agnostic Coax packages:

1. **`components/core/`** — input events, focus/activation, accessibility & `coax drive` element trees (`elements.coax`).
2. **`components/layout/`** — box/stack/wrap measurement and geometry.
3. **`components/paint/`** & **`components/raster/`** — GPU primitive lists, `Atlas` (`atlas.coax`), and host font/glyph tables (`host-tables.coax`).
4. **`components/kit/`** — reusable controls (`controls.coax`), Markdown (`markdown.coax`), and HUD widgets.
5. **`components/skin/`** — `skin.coax` (`UiSkin` schema), `zeron.coax` (the canonical base skin), derived skins (`arcade.coax`, `everglade.coax`, `metaverse.coax`, `terminal-pixels.coax`), and `all.coax` (registry & `uiskin_pixels`).
6. **`components/backend/`** — GPU shader (`gpu-shader.coax` / `chat.wgsl`) and renderer bridges.
7. **`components/zeron/`** — high-level chat/composer/transcript/picker components consuming `UiSkin`.

Every non-Zeron skin in `components/skin/` starts from `s := zeron_skin()` and overrides only the colors, faces, or radii it changes, making `zeron.coax` the single structural baseline for the entire component system.

---

## 5. Desktop App Integration (`apps/openagents-desktop/`)

The flagship consumer of the ported Zeron UI is **`apps/openagents-desktop`**, a native macOS (AppKit/Metal/CoreText) and Linux (Wayland/Vulkan/PipeWire) multi-session agent workspace.

### 5.1 Coax Application Modules (`apps/openagents-desktop/zeron/`)

| Module | Lines | `#test` | Key Structs & Responsibilities |
| --- | ---: | ---: | --- |
| [`claude.coax`](../../apps/openagents-desktop/zeron/claude.coax) | 409 | 5 | **`SUi`** — projects shared `coax/lib/agent/session/session.coax` records into `Transcript` rows (user bubbles, assistant Markdown, tool groups, costs/timings). |
| [`modes.coax`](../../apps/openagents-desktop/zeron/modes.coax) | 116 | 3 | Picker marks, tints, and option tables for the 4 harnesses backed by `coax/lib/agent/engine/engine.coax`: **Claude Code**, **Psionic** (`gpt-oss-20b` Responses API), **Lab** (Elephant router), and **Codex** (`codex app-server`). |
| [`dictation.coax`](../../apps/openagents-desktop/zeron/dictation.coax) | 126 | 10 | **`ZrDictationGuard`** — ticket-guarded speech-to-text draft insertion into `Composer`: replaces only the exact snapshot/selection it started from, cancels on any edit/navigation, inserts as a single undoable edit, and never auto-submits. |
| [`dictation-ui.coax`](../../apps/openagents-desktop/zeron/dictation-ui.coax) | 45 | 4 | Dictation button/indicator visual states and microphone interaction rules. |
| [`dictation-labels.coax`](../../apps/openagents-desktop/zeron/dictation-labels.coax) | 22 | 2 | Status label strings for dictation states. |
| [`dictation-linux.coax`](../../apps/openagents-desktop/zeron/dictation-linux.coax) | 87 | 5 | Linux dictation controller (`#authority`): PipeWire `pw-record` argument builder (`48 kHz`, `f32`, `20ms`), sample-count timestamping with drift recovery, and shared model path resolution. |
| [`linux.coax`](../../apps/openagents-desktop/zeron/linux.coax) | 7 | 0 | Linux desktop compilation entry shim. |
| [`screen.coax`](../../apps/openagents-desktop/zeron/screen.coax) | 1,486 | 13 | **`ZeronState`** — complete single-conversation Zeron-fork screen: empty new-chat form -> docked composer + 736 px transcript, model/access pickers, file attachments, dictation, approval prompts, `--demo` fixture mode, and atlas/frame emission. |
| [`room.coax`](../../apps/openagents-desktop/zeron/room.coax) | 272 | 4 | **`Room`**, **`RmMsg`**, **`RmAsk`**, **`RmNotes`** — inter-agent messaging log and in-flight `ask_agent` / `tell_agent` coordination across desk sessions, with self/cycle detection (`RM_ERR_SELF`, `RM_ERR_CYCLE`), optional `max_hops`, and `STOP` cancellation. |
| [`desk.coax`](../../apps/openagents-desktop/zeron/desk.coax) | 1,362 | 9 | **`Desk`**, **`DskChat`**, **`DskHit`**, **`DskChord`**, **`DskOut`** — multi-session workspace managing up to 64 chats (`DSK_MAX`), folder-grouped collapsible/resizable sidebar (208–400 px, default 256 px), top tabs, keyboard shortcuts (`Cmd+S`, `Cmd+1..9`, `Cmd+N`, `Cmd+W`, `Shift+Up/Down`), vertex shifting (`dsk_shift`), and `dsk_elements` for VoiceOver/`coax drive`. |
| [`store.coax`](../../apps/openagents-desktop/zeron/store.coax) | 123 | 2 | Local transcript persistence (`sessions/<id>.jsonl`) via `coax/lib/agent/session/log.coax` (`st_lines`, `st_load`), with backwards-compatible reading of pre-shared logs. |
| [`settings.coax`](../../apps/openagents-desktop/zeron/settings.coax) | 239 | 3 | **`Settings`** — `settings.json` load/save (`ds_load`, `ds_restore_cwd`, `ds_restore`, `ds_restore_tabs`, `ds_save`) for window frame, sidebar state, default harness/access, and per-session resume IDs (Claude `--resume`, Codex `thread/resume`). |

### 5.2 Native Host Bridge (`zeron-face.c` & Platform Hosts)

- [`apps/openagents-desktop/zeron-face.c`](../../apps/openagents-desktop/zeron-face.c) (221 lines) — C FFI wrapper implementing `apps/openagents-desktop/face.h` (`zr_new`, `zr_quit`, `zr_new_chat`, `zr_dictation_draft`, `zr_dictation_sent`, `zr_dictation_parts`, etc.). Allocates `coax_t_ZeronState` and invokes the compiler-generated `coax_default_t_ZeronState()` exactly once before any attachment or live-owner operation.
- **macOS host:** `apps/shared/host/apple/ct-atlas.m` rasterizes the Geist fonts and SVG icons into the GPU atlas and drives frames through `zeron-face.c`.
- **Linux host:** `apps/openagents-desktop/linux/chat.c` manages `Desk`, `Settings`, and per-session `ZeronState` instances (`zr_new()`), wiring Wayland input, dictation (`zr_dictation_key`), and rendering.

### 5.3 Parity Tests, Lifecycle Verification, Goldens & Fixtures

- **Native default-initialization & lifecycle harness (`apps/openagents-desktop/tests/`):**
  - [`zeron_source_extract.py`](../../apps/openagents-desktop/tests/zeron_source_extract.py) (91 lines) — hash-bound extractor that projects complete authored Coax declarations (starting from `settings.coax`) into a deterministic `subset.coax` + `extraction.json` manifest.
  - [`check_zeron_defaults.py`](../../apps/openagents-desktop/tests/check_zeron_defaults.py) (198 lines) — compiles the extracted `ZeronState` / `lab_attach` / `coaxial_zeron_quit` subset and runs the native C check binary.
  - [`zeron-defaults.c`](../../apps/openagents-desktop/tests/zeron-defaults.c) (136 lines) — verifies guarded allocation, single invocation of `coax_default_t_ZeronState`, allocation-failure null safety, active-call refusal without mutating live descriptors/bytes, and never-started quit safety.
  - `dictation-insertion-native.coax` & `dictation-screen-negative.coax` — native dictation insertion and negative screen-guard tests.
- **Visual & Accessibility Tree Goldens (`apps/openagents-desktop/goldens/`):** 24 `.png` + `.tree` golden pairs (plus a `linux/` subdirectory) covering:
  - Approvals (`approval-allow`, `approval-deny`, `approval-waiting`)
  - Chat layouts (`chat-1320x880`, `chat-900x600`, `chat-sonnet`, `type-and-paste`)
  - Onboarding & error states (`choose-folder`, `claude-missing`, `claude-signed-out`)
  - Focus rings (`focus-button`, `focus-composer`)
  - Multi-agent Room (`room-ask`, `room-codex-asks`, `room-tab`, `room-target`)
  - Sidebar, settings & store round-trips (`sidebar-900x600`, `sidebar-open`, `sidebar-shortcuts`, `settings-quit`, `settings-reopen`, `settings-shortcuts`, `store-quit`, `store-reopen`)
- **Replay Fixtures (`apps/openagents-desktop/zeron/fixtures/`):** `asks`, `claude-tools.jsonl`, `fixture.py`, `psionic-turns.sse`, `psionic.py`, `room`, `signed-out`, `stream`.

---

## 6. Other Consumers Across the Repository

Beyond `apps/openagents-desktop`, Zeron components and skins are reused across web, mobile, engine chrome, and compiler/component test suites:

| Consumer | Imports / Symbols Used | Purpose |
| --- | --- | --- |
| [`apps/openagents-web/chat.coax`](../../apps/openagents-web/chat.coax) | `components/zeron/picker.coax`, `chrome.coax`, `chat-lines.coax`, `components/skin/all.coax` | `openagents.com/chat` web app rendered on WebGPU (`chat.wgsl` + `coax/web/gpui.js`) with Zeron's bubbles, Markdown, tool groups, composer, model picker, tabs/sidebar, and `#demo` mode. |
| [`apps/openagents-web/chat/chat-frames.coax`](../../apps/openagents-web/chat/chat-frames.coax) | Loads `../chat.coax` | Vertex-buffer hash pinning for web `/chat` demo frames across component refactors. |
| [`apps/openagents-web/chat/chat-hud.coax`](../../apps/openagents-web/chat/chat-hud.coax) | `components/zeron/chat-lines.coax` | Legacy HUD chat backend sharing the same `Z`/`C`/`S`/`U`/`T` line reader. |
| [`apps/openagents-web/agents.coax`](../../apps/openagents-web/agents.coax) | `components/skin/zeron.coax` (`zeron_skin()`) | Web agent-approval and credential management UI styled with Zeron Dark tokens. |
| [`apps/openagents-ios/editor.coax`](../../apps/openagents-ios/editor.coax) & `host.c` | `components/zeron/editor.coax` (`UiEditor`, `ue_close`) | Canvas-only active text editor adapter (`ChEditor`) for single-line/multiline fields and IME transactions on compiled pages / Wayland hosts. |
| [`apps/shared/app/view/page.coax`](../../apps/shared/app/view/page.coax) | `components/zeron/tools.coax` | Shared app view rendering Zeron tool chips. |
| [`apps/metaverse-engine-desktop/chrome.coax`](../../apps/metaverse-engine-desktop/chrome.coax) | `components/zeron/gpui.coax` | Setup and status chrome (`EditorChrome`) for the Metaverse Engine desktop app. |
| `components/raster/atlas.coax` & `host-tables.coax` | `components/zeron/gpui.coax`, `components/skin/zeron.coax` | Shared rasterizer atlas and host font/metric tables. |
| `components/kit/*-checks.coax`, `components/app/pane-checks.coax`, `components/gallery/*` | `components/skin/all.coax`, `zeron_skin()` | Reference skin for controls, HUD, Markdown, pane, and skin-parity checks. |
| `coax/native/gpu-editor-check.coax` | `components/zeron/editor.coax`, `composer-frames-checks.coax` | Native compiler/runtime verification of `UiEditor` and composer frame hashes. |

---

## 7. Ported Surface vs. Upstream `zeronsh/zeron`

| Capability | Upstream `zeronsh/zeron` | Ported in OpenAgents |
| --- | --- | --- |
| **Visual design & Zeron Dark theme** | `apps/zgx/ui/ds/themes/zeron-dark.json` | `components/skin/zeron.coax` (`zeron_skin()`) + Geist fonts + 37 SVG icons |
| **UI runtime & language** | Rust `zgx` GPU host + hot-reloaded Luau UI (~296k LOC) | Pure Coax compiled to C/WASM + Metal/Vulkan/WebGPU hosts |
| **Chat composer, Markdown, tool chips, picker** | Luau components in `apps/zgx/ui/` | `components/zeron/{editor,attachments,composer,transcript,picker,tools,markdown,chrome}.coax` |
| **Multi-session desk (sidebar + tabs)** | Workspace/session tree in `apps/zgx` + `crates/engine` | `apps/openagents-desktop/zeron/{desk,screen,settings,store}.coax` |
| **Inter-agent coordination** | Multi-harness orchestration in `crates/engine` | `apps/openagents-desktop/zeron/room.coax` (`ask_agent` / `tell_agent`) |
| **Voice / Dictation** | Real-time voice sessions & `VoiceOrbView` | Push-to-dictate draft insertion (`dictation*.coax`, PipeWire on Linux, Apple speech on macOS) |
| **Persistence & Sync** | Local SQLite/Loro CRDT + Cloudflare DO multi-device mesh + Vault | Local `settings.json` + JSONL session logs (`store.coax`), web chat backed by `oa-api` |
| **Embedded terminals, previews, git worktrees** | Ghostty (macOS), WebKitGTK/WebView2, `crates/engine` worktrees | Not ported into `components/zeron/` |

---

## 8. Running Tests & Checks

```bash
# Zeron skin and core component tests
coax/bin/coax test components/skin/zeron.coax
coax/bin/coax test components/zeron/editor.coax
coax/bin/coax test components/zeron/composer.coax
coax/bin/coax test components/zeron/transcript.coax
coax/bin/coax test components/zeron/picker.coax
coax/bin/coax test components/zeron/composer-frames-checks.coax
coax/bin/coax test components/zeron/skins-checks.coax

# Desktop Zeron integration & Linux dictation authority tests
coax/bin/coax test apps/openagents-desktop/zeron/dictation-linux.coax --library
python3 apps/openagents-desktop/tests/check_zeron_defaults.py

# Web chat frame-hash regression checks
coax/bin/coax test apps/openagents-web/chat/chat-frames.coax --only frames
```
