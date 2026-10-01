# Porting the Zeron desktop UI to Rust Native: audit

This audit estimates what it would take to port the whole desktop user
interface of Zeron, the app in the `zeronsh/zeron` repository, into the
OpenAgents desktop app, and to convert it to Rust Native along the way. It
inventories every Zeron screen and component, maps each GPUI concept to a
Rust Native equivalent, names the framework capabilities that Rust Native
lacks, and proposes phases, milestones, and issues. It now records the shell
and sidebar implementation in [#9993](https://github.com/OpenAgentsInc/openagents/issues/9993)
and revises the remaining estimates using this session's measured pace.

| Field | Value |
| --- | --- |
| Reference | `github.com/zeronsh/zeron`, installed checkout at `~/zeron`; formerly Comet |
| Revisions read | `50cf9e97` for the original inventory; `ed3b1aae` in `~/zeron` for the shell implementation |
| License | MIT, copyright 2026 Wing (`~/zeron/LICENSE`) |
| OpenAgents baseline for the implementation | `d9e2a1020a`, with newer pairing changes and version alignment integrated before final checks |
| Method | Original source inventory, followed by implementation, targeted tests, PNG captures, a live Linux window, and foreground timing measurements |
| Earlier review | [Zeron (zeronsh/comet) review](2026-09-28-zeron-comet-review.md), which covers the phone transcript, text engine, and command ledger |

Paths that start with `comet/` retain the inventory's old prefix and are
relative to `~/zeron`. All other
paths are relative to this repository. LOC counts include tests unless a
row says otherwise; Zeron keeps most unit tests in `mod tests` blocks at the
end of each file. Source inventory counts below are the original snapshot,
unless a row explicitly describes the implementation.

## Contents

- [Status on 2026-09-30](#status-on-2026-09-30)
- [Executive summary](#executive-summary)
- [Completed in this session](#completed-in-this-session)
- [Comet architecture overview](#comet-architecture-overview)
- [OpenAgents today](#openagents-today)
- [GPUI to Rust Native concept mapping](#gpui-to-rust-native-concept-mapping)
- [Component inventory and port difficulty](#component-inventory-and-port-difficulty)
- [Rust Native capabilities to add](#rust-native-capabilities-to-add)
- [What not to port, and what to adapt instead](#what-not-to-port-and-what-to-adapt-instead)
- [Licensing and attribution](#licensing-and-attribution)
- [Performance considerations](#performance-considerations)
- [Phased plan](#phased-plan)
- [Proposed issue breakdown](#proposed-issue-breakdown)
- [Open questions](#open-questions)

## Status on 2026-09-30

The rest of this audit is the September 29 snapshot: where it says the
transcript, composer, menus, settings, or review pane are missing or pending,
that was true then and is no longer. The application rows of the
[issue breakdown](#proposed-issue-breakdown) landed under tracker
[#10003](https://github.com/OpenAgentsInc/openagents/issues/10003), all now
closed. The [desktop README](../../crates/openagents-desktop/README.md#what-is-built)
lists what is built per platform, with its limits and verification records.

| Audit rows | Delivered by |
| --- | --- |
| CDP-00, CDP-10 (long-content measurement, transcript painter, selection) | #10005, #10010 |
| CDP-06, CDP-07 (editing contract, IME, clipboard) | #10004 |
| CDP-08, CDP-11, CDP-19 (commands, key map, overlays, menus, palette) | #10013 |
| CDP-12, CDP-25 (motion tokens, reduced motion, theme) | #10022, dark only by owner direction rather than light and dark |
| CDP-13 (images) | #10011 |
| CDP-14 (syntax spans) | #10010 and #10019 on the desktop, #10028 on the phones |
| CDP-15 (AccessKit) | #10024 |
| CDP-16, CDP-17 (real chat state, transcript and chat cards) | #10006, #10007, #10009 |
| CDP-18 (composer attachments, queue, questions) | #10011, #10016 |
| CDP-20 (native menu bar and notifications) | #10023 (macOS menu bar), #10026 (Linux), #10061 (macOS), #10062 (Windows) |
| CDP-21 (diff pane and **What changed**) | #10019 |
| CDP-22 (settings) | #10021 |
| CDP-23 (phone hosts) | #10028 |

The framework rows CDP-01 to CDP-05, CDP-09, and CDP-24 were not reconciled
here. Open question 2 is settled: the desktop and the phones share
`openagents-chat` and `openagents-chat-app`. Open question 4 is settled:
Windows runs chat without the Verse backdrop
([#10027](https://github.com/OpenAgentsInc/openagents/issues/10027)).

## Executive summary

**Feasibility.** A port is feasible, but it isn't a port of screens alone.
Zeron's UI crate is 157,280 lines of Rust in 128 files
(`comet/crates/ui/src`), about 122,500 of them outside test modules. It sits
on a forked GPUI that supplies a GPU renderer, flexbox layout (Taffy), text
shaping, IME, virtualized lists, overlays, focus, key bindings, animation,
drag and drop, and native menus. Rust Native already supplies semantic
views, transcript measurement, and a working desktop painter. The desktop
adapter now also has split panes, independent clipped scrolling, vector
icons, shortcuts, and retained painting. It can't draw `Transcript`, `Message`, `Tool`, or
`Composer` yet (`crates/rust-native-desktop/src/layout.rs`, `Scene::unsupported`).
The remaining framework work should follow the next measured product slice.

**Effort.** Estimates are remaining focused implementation hours for the
agent-assisted workflow used here, including targeted tests and fixtures.
The requested shell and sidebar, previously estimated at 5–7 engineer-weeks,
were implemented and checked in about one working hour, including the lag
fix. The issue was created at 01:57 UTC on September 30, still September 29
locally. This is a measured calibration point for a bounded UI slice; the
unimplemented transcript, IME, and accessibility work still needs its own
measurement. These ranges replace the original manual-engineering estimates.

| Remaining scope | Framework (phases 0–3) | Application (phases 4–5) | Total |
| --- | --- | --- | --- |
| Chat-first slice: real chat binding, transcript, composer, pickers, menus, settings subset, diff review, command palette, and phone-host mappings | 32–54 h | 21–36 h | **53–90 h** |
| Every inventoried Zeron UI surface, including the optional IDE surfaces | 36–62 h | 52–98 h | **88–160 h** |

At eight focused hours per day, these ranges are about 7–12 coding days for
the chat-first slice and 11–20 for all inventoried UI surfaces. They exclude
waiting for devices, owner feedback, signing, and store releases. The full
surface estimate has lower confidence: none of its editor, browser, or
capture integrations was measured in this session. Re-estimate after the
first functioning transcript and composer. The engine, harness, and sync
remain outside this UI estimate.

| Component group | Remaining estimate |
| --- | --- |
| Long-content renderer/layout measurement and needed spec updates | 2–4 h |
| GPU foreground renderer, if the long-content measurements require it | 6–10 h |
| General box layout and style extensions beyond the implemented split layout | 3–5 h |
| Text editing contract, desktop IME, and clipboard | 6–10 h |
| Focus scopes and command/keymap extensions beyond the implemented shortcuts | 2–3 h |
| General virtual lists and desktop transcript painting | 5–8 h |
| Overlays, menus, tooltips, and modals | 4–6 h |
| Animation, images, syntax spans, and accessibility | 4–8 h |
| Requested shell and sidebar with sample content | **Done; 0 h remaining** |
| Bind the shell to real chat state | 2–4 h |
| Transcript rows and chat cards | 4–6 h |
| Composer, queue, and attachments | 4–6 h |
| Pickers, command palette, native menu bar, and notifications | 2–4 h |
| Theme and motion tokens | 1–2 h |
| Diff review pane | 3–5 h |
| Settings subset | 2–4 h |
| Phone hosts: text editing, overlays, images, and highlight spans | 3–5 h |

**Recommended approach.**

1. Don't copy Zeron's UI one to one. Most of it is an IDE for driving
   vendor coding agents: a file editor, git history, a terminal, an embedded
   browser, and screen capture. OpenAgents' direction is chat first and
   IDIOT PROOF ([app wireframe](../product/2026-09-28-app-wireframe.md)):
   chat with OpenAgents, dispatch Coder, run the Gym and evals, and watch
   Verse. Port Zeron's chat shell, transcript, composer, menus, and diff
   review, and adapt them to OpenAgents' chat cards.
2. Keep Rust Native as the contract. Screens stay `rust-native.view.v2`
   semantic views produced by Rust application state, so the same screens
   can reach iOS and Android. Add the missing capabilities in the right
   layer: semantics in `crates/rust-native`, painting and platform input in
   `crates/rust-native-desktop`, and native widgets in the phone hosts.
3. Keep the implemented split layout and retained painter for the shell.
   The first lag report was resolved without replacing the renderer. Measure
   a streaming 3,000-row transcript before choosing further renderer work.
4. Use a 2–4-hour long-content spike to decide the next renderer and layout
   changes. Evaluate a GPUI backend only if the existing adapter cannot meet
   the measured budget. See [open questions](#open-questions).

**Order.** Shell and sidebar are implemented. Next measure long-content
painting and deliver a real transcript and composer, then the remaining
chat-first controls, then the
review surfaces and settings. Defer the IDE surfaces until a product need
exists.

## Completed in this session

[#9993](https://github.com/OpenAgentsInc/openagents/issues/9993) implements
the requested desktop shell and sidebar:

- Full-height sidebar, inset main pane and header, fixed sidebar header and
  footer, grouped sample chats and projects, selection, new sample chats,
  section disclosure, Grid navigation, and settings/computer navigation.
- Sidebar collapse, 224–400-point drag resizing, independent clipped body
  scrolling, scrollbars, keyboard focus, and Ctrl/Cmd+B and Ctrl/Cmd+N.
- Existing Rust Native nodes and typed intents, with desktop split layout
  in the adapter. No new wire schema, GPUI dependency, or product language.
- Existing pairing controls and the live Grid remain integrated. Leaving
  the pairing screen cancels its codes. Newer full-permission pairing and
  silent host-start behavior on `main` are preserved.
- Retained foreground pixels, drawing-operation damage comparison, partial
  texture uploads, hidden-operation culling, and faster rectangle spans.
  Screen-lock polling runs separately from the UI and host requests.

The sidebar data is bounded sample content. Chat persistence, a real
transcript, an editable composer, the right-hand review pane, overlays, and
general virtualization remain future work. This completes the requested
shell slice of CDP-16 and parts of C1, C2, C6, C9, C11, and C16; it does not
complete every capability in those rows.

See the [verification record](../desktop/verification/2026-09-29-shell-sidebar/verification.md)
for checks, captures, and timing measurements. The source design was
reimplemented from `~/zeron/crates/ui/src/shell.rs`; no Zeron code or assets
were vendored.

## Comet architecture overview

Zeron is a local-first controller for coding agents: Claude Code, Codex,
Cursor, Devin, Grok, Hermes, Pi, and Antigravity (`comet/README.md`). One
binary, `zeron`, runs headed or headless. Every device runs an engine that
stores each chat as a Loro CRDT document. Optional sync goes through
Cloudflare Durable Objects in `comet/edge/` (TypeScript).

### Workspace layout

| Crate | Files | LOC | Role |
| --- | --- | --- | --- |
| `comet/crates/ui` (`zeron-ui`) | 128 | 159,867 (including examples) | The GPUI desktop app |
| `comet/crates/engine` | 78 | 75,528 | Sessions, run journal, repositories, terminals, accounts |
| `comet/crates/harness` | 77 | 43,765 | Agent adapters (Claude Code, Codex, ACP agents, mock) |
| `comet/crates/client` | 29 | 13,366 | Engine-free viewer peer used by the phone |
| `comet/crates/doc` | 16 | 10,191 | Session and registry document schemas |
| `comet/crates/sync` | 22 | 10,199 | Room clients and SQLite store |
| `comet/crates/text` | 16 | 7,561 | Analytic text measurement (a pretext port); used only by `crates/mobile` |
| `comet/crates/mobile` | 14 | 6,377 | UniFFI core for the UIKit iOS app |
| `comet/crates/theme` | 5 | 4,738 | Theme schema, 31 built-in variants, VS Code theme compiler |
| `comet/crates/preview` | 15 | 4,577 | Dev-server preview discovery |
| `comet/crates/proto` | 10 | 4,360 | Wire types and shared view derivations |
| `comet/crates/rpc` | 7 | 3,676 | Typed RPC over WebSocket or in memory |
| `comet/crates/mcp` | 5 | 2,879 | MCP server that lets agents drive other chats |
| `comet/crates/update` | 2 | 2,533 | Release checker and installer |
| `comet/crates/markdown` | 3 | 1,753 | Incremental pulldown-cmark block parser with display-only mending |
| `comet/crates/syntax` | 2 | 1,690 | tree-sitter highlighting for 25 grammars |
| `comet/apps/zeron` | 6 | 1,533 | The binary: clap CLI, `headless`, `daemon`, `login`, `mcp`, `update` |

The port concerns `crates/ui`, `crates/theme`, `crates/markdown`, and
`crates/syntax`. The engine, harness, sync, and edge are out of scope: the
OpenAgents host, Coder, and NIP-HOST fill those roles.

### UI framework

Zeron uses **GPUI**, Zed's GPU UI framework, through its own fork:

- `gpui`, `gpui_platform`, and `gpui_tokio` come from `zeronsh/zui` at
  `18a89af`, which was extracted from `wingleeio/zed` at `e2ddcc6`
  (`comet/Cargo.toml`, lines 54–87). The fork adds `BackdropBlur`,
  per-edge and per-pixel `EdgeFade`, bounded GPU memory,
  `ImageSource::evict`, a transparent-window alpha fix, macOS 26 window blur,
  compositing above native macOS views (for the embedded browser), line-wrap
  fixes, and rounded image masks. It removes GPL tracing crates.
- `gpui-base` comes from `zeronsh/gpui-component` at `103f13d` (Apache-2.0).
  Zeron uses only its code editor (`gpui_base::input::{Editor, EditorState,
  Rope}`) in `comet/crates/ui/src/files/`, its `Checkbox`, and its scrollbar
  styles. Everything else is hand-built.
- Zeron avoids Zed's GPL crates (`editor`, `ui`, `theme`, `markdown`), as
  `comet/ARCHITECTURE.md` section 4 states.
- Async: tokio for the engine, bridged by `gpui_tokio`. The UI crate has
  184 `cx.spawn` calls, 11 `Tokio::spawn` calls, and 123
  `background_executor` references, 34 of them timers.

API density in `comet/crates/ui/src`, from fixed-string counts:

| GPUI API | Uses | Meaning for the port |
| --- | --- | --- |
| `div()` | 1,524 | Every box is a flex container with Tailwind-style styling |
| `cx.notify()` | 964 | Fine-grained invalidation per entity |
| `cx.new(` / `Entity<` | 284 / 208 | Many stateful views |
| `.tooltip` mentions | 196 (33 `.tooltip(` calls) | Hover help is everywhere |
| `on_mouse_down` | 120 | Custom pointer handling |
| `KeyBinding::new` | 93 (66 in the composer) | A real key-binding system |
| `FocusHandle` / `track_focus` | 83 / 50 | Focus graph with tab groups |
| `on_action` | 81 | Typed actions |
| `list(` / `ListState` | 73 / 40 | Variable-height virtualized lists in 7 views |
| `on_drop` / `on_drag(` | 47 / 33 | Typed drag payloads, reordering, file drops |
| `canvas(` | 44 | Custom painting in 19 files |
| `with_animation` | 32 (25 calls) | Animations in 9 files |
| `anchored(` / `deferred(` / `occlude()` | 14 / 17 / 39 | Popover layer |
| `TextRun` / `StyledText` | 49 / 11 | Rich text by run |
| `img(` / `svg()` | 16 / 2 | Images, and icons through one helper |
| `EntityInputHandler` | 3 | One hand-rolled text input with IME |
| Custom `impl Element` | 8 | `ContainedMenu`, `ResponsiveText`, `LinkRanges`, `Layered`, `Frosted`, `EdgeFaded`, `DockedComposer`, `ComposerTextElement` |

### Window, layout, and state

- **One window.** The only production `open_window` is `open_main_window`
  (`comet/crates/ui/src/lib.rs`, line 364). The other 24 calls are in tests.
  The window has a transparent title bar with inset traffic lights,
  `app_owns_titlebar_drag`, client-side decorations on Linux, a minimum size
  of 900×600, and a blurred, transparent, or opaque background chosen by the
  theme.
- **Shell.** `Shell` (`comet/crates/ui/src/shell.rs`, 16,132 lines) owns the
  application state, the sidebar, the transcript, the composer, terminal
  panels, and maps of right-pane surfaces. `render()` (about line 11954)
  draws a frosted root, then a row of sidebar, resizable seam, main card
  (transcript, composer dock, message rail, status strip, terminal), a
  second seam, and the right pane with its tab strip. The title bar is
  absolutely positioned above it, and overlays go on top.
- **Routes.** `Route::Chat` or `Route::Settings(SettingsSection)`, with nine
  visible settings pages. Right-pane surfaces are `Picker`, `File`,
  `Browser`, `Diff`, `Terminal`, `Subagent`, and `SideChat`.
- **State.** One `Entity<AppState>` (`comet/crates/ui/src/state.rs`, 5,118
  lines) holds connection, auth, devices, spaces, chats, sessions, the
  selected transcript, queue, uploads, and review comments. Fifteen
  `WATCH_*` RPC streams are pumped with `cx.spawn`, folded in with
  `update`, and followed by `cx.notify()`.
- **Actions and keys.** 5 `actions!` blocks: `shell` (12 actions plus
  `JumpSession(usize)`), `composer` (38 editing actions), `zeron` (12 menu
  actions), `terminal`, and `browser` (6). The user can rebind 13 shortcuts
  plus 9 jump slots (`ShortcutId` in `comet/crates/ui/src/settings.rs`,
  about line 933), persisted in `ui-settings.json`, with click-to-record and
  conflict detection (`comet/crates/ui/src/settings/shortcuts.rs`).
- **Menus and platform.** Native menus through `cx.set_menus`
  (`comet/crates/ui/src/app_menus.rs`): app, Edit, View (appearance), and
  Window. No tray item and no dock menu. Notifications use
  `NSUserNotification` or `notify-send` (`notify.rs`); sounds shell out to
  `afplay` or its Linux equivalents (`sound.rs`).

### Tests and fixtures

- `comet/crates/ui/src` has 209 `#[gpui::test]` and 1,089 `#[test]`
  functions. The heaviest GPUI test files are `shell/spaces.rs` (19),
  `shell/navigation_tests.rs` (14), and `files/preview.rs` (8).
- `comet/crates/ui/examples/` has 9 manual fixture binaries, including
  `command-palette-fixture.rs`, `sidebar-fixture.rs`, and
  `windows-render-fixture.rs`. There is no automated pixel-diff harness.
- Much of the logic is written as pure reducers that plain `#[test]`
  functions cover, such as `menu_step`, `match_rank`, and `classify_key` in
  `comet/crates/ui/src/popover.rs`, and the auto-grow math in `composer.rs`.
  That style ports well to Rust Native, where application state is already
  pure Rust.

### Assets

- Fonts: Geist and Geist Mono, 16 TTF files under
  `comet/crates/ui/assets/fonts/`, under the SIL Open Font License
  (`fonts/licenses/Geist-OFL.txt`).
- Icons: 115 SVG UI icons under `comet/crates/ui/assets/icons/`, plus 250
  file icons and 105 folder icons from the MIT-licensed Symbols set
  (`assets/file-icons/`, `LICENSE.symbols`).
- Sounds: 4 WAV chimes (`appshot`, `attention`, `done`, `request`).
- Themes: 31 built-in variants adapted from MIT-licensed VS Code themes
  (`comet/crates/theme/src/builtins.rs`, 1,338 lines; sources in
  `comet/THIRD_PARTY_NOTICES.md`).

## OpenAgents today

### Rust Native core

`crates/rust-native` (19 source files, 9,148 lines, about 2,351 of them
tests) defines schema `rust-native.view.v2` in `src/view.rs`.

- **Elements:** `Surface`, `Stack` (vertical, horizontal, wrap), `List`,
  `Text`, `Button` (with checkbox, pill, and circular icon forms),
  `Transcript`, `Message`, `Markdown`, `Tool`, `Working`, and `Composer`.
  There are 17 `Glyph` icons. Views are bounded at 512 KiB, 1,024 nodes,
  depth 16, and 64 KiB of text.
- **Style** (`src/style.rs`, 301 lines): foreground and background colors,
  four paddings, a `gap` step (`xs` to `lg`), weight, and alignment, composed
  as `StylePatch` layers. There is no size, radius, border, shadow, opacity,
  overflow, or position.
- **Text roles:** `Body`, `Heading`, `Code`, `Status`, `Markdown`, and
  `Terminal`. Adapters choose sizes; the desktop theme uses body 15,
  heading 22, status 13, and code 13 points.
- **Transcript layout** (`src/layout/`, about 5,500 lines): the design from
  the earlier review has landed. Rust measures rows with exact heights and
  offsets, `rows_in` is a binary search, frames are immutable, and each row
  has a `RowDisplay` of runs, rectangles, links, widgets, scrollers, and
  accessibility records (`src/layout/display.rs`). The Rust line breaker in
  `src/layout/shape.rs` is checked against CoreText. A C interface is in
  `src/layout/ffi.rs` and `include/rust_native_layout.h`.
- **Markdown:** `src/markdown.rs`, with incremental reparsing and
  display-only mending in `src/markdown/incremental.rs` and
  `src/markdown/mend.rs`.
- **Input:** only `Activation` (instance, revision, node) and
  `InputRequest`. Pointer, keyboard, focus, and IME belong to adapters, and
  the text-editing contract (phase RN4) is not implemented.
- **Missing:** a general layout engine, text editing, general virtual lists,
  overlays, drag and drop, images, SVG, animation, and a mounting runtime
  (RN3).

### Desktop adapter

The original `crates/rust-native-desktop` snapshot (8 files, 3,145 lines) uses winit 0.30.13,
wgpu 29.0.4, and swash 0.2.10:

- `src/layout.rs` places nodes in points and records button rectangles in
  focus order. It now supports a two-pane window with fixed headers and
  footers, independently scrolling clipped bodies, and a resize seam.
- `src/paint.rs` and `src/canvas.rs` paint antialiased rounded rectangles,
  strokes, check marks, and vector icons on the CPU. `paint::Retained`
  compares drawing operations, clears and repaints changed regions, and
  uploads those regions to the foreground texture over the GPU Grid.
- `src/window.rs` repaints only when the view, size, or hovered target
  changes. It handles clicks, Tab and Shift+Tab, Enter, Space, Escape, and
  Cmd or Ctrl+Q and W, plus application-defined command chords. Focus
  scrolling brings hidden sidebar controls into view. There is still no
  desktop IME or editable text control; the existing pairing clipboard is
  application-owned.
- `src/backdrop.rs` and `src/backdrop.wgsl` draw an application backdrop at
  half size, blurred and dimmed, under the views.
- `capture` and `capture_views` render PNGs without a window, for tests.

### Desktop app

The original `crates/openagents-desktop` snapshot (21 source files, 8,735 lines) is the companion
app from [#9970](https://github.com/OpenAgentsInc/openagents/issues/9970):

- Screens `DSK-01` (Connect a phone), `DSK-02` (Connected), `DSK-03`
  (Home), and `DSK-04` (A phone nearby wants to connect) are Rust Native
  views in `src/screens.rs` and `src/screens/nearby.rs`. `DSK-05`, the
  menu bar, is native `NSStatusItem` code in `src/menubar.rs` (649 lines).
  The screen specifications are in the
  [app wireframe](../product/2026-09-28-app-wireframe.md#desktop-app-screens).
- `src/shell.rs` implements `rust_native_desktop::App` and paints the QR
  code and OpenAgents mark on registered surfaces. `src/chrome.rs` now
  projects the shell and bounded sample sidebar using existing semantic
  nodes and local typed navigation state.
- The Verse backdrop (`src/backdrop.rs`,
  [#9982](https://github.com/OpenAgentsInc/openagents/issues/9982)) spectates
  the Grid and is drawn by the Verse renderer on the window's wgpu device.
  It measured 7.2% CPU with three players on an M5 Max.
- `src/update.rs` (1,441 lines) is a signed-manifest updater.
- The shell supplies its own dark neutral palette through `App::theme`.
  Pairing-only captures retain their existing theme.

### Phone hosts and themes

- iOS decodes view JSON into SwiftUI (`bins/coder-ios/host/App/NativeView.swift`,
  440 lines) and paints Rust-measured transcripts with CoreText
  (`NativeTranscriptPainter.swift`, 2,107 lines). `bins/openagents-ios`
  reuses these files.
- Android maps views to framework widgets
  (`bins/openagents-android/.../NativeRenderer.kt`, 388 lines) and paints
  transcripts in `TranscriptPainter.kt` (1,130 lines), with
  `TextSelection.kt`, `SelectionLayer.kt`, and `StreamFade.kt`.
- `crates/coder-ui` (98 lines of theme) holds only Coder's amber intensity
  ladder and two near-black colors. Fonts are Inter and JetBrains Mono,
  bundled in `crates/rust-native/fonts/`.
- The chat, Coder, Gym, and computer state the desktop would show already
  lives in Rust in `crates/openagents-mobile` (`chats.rs`,
  `conversation.rs`, `coder_tab.rs`, `gym.rs`, `computers_home.rs`, and
  others).

### Plans

`docs/coder/rust-native/build-order.md` lists phases RN0 (done), RN1
(partly done), RN2 (open), RN3 (phone readers done; mounting and web open),
RN4 (editing and long content, open), RN5 (reader screens done; control and
composer open), and RN6 (theme groups and ergonomics, later). The build
order says not to add a universal layout engine up front. These docs
predate `rust-native-desktop` and don't mention it.

## GPUI to Rust Native concept mapping

The rule for every row: the application produces semantic nodes and typed
intents in Rust; the core validates them; the adapter owns platform input,
layout of primitives, and painting.

| GPUI concept | Zeron use | Rust Native equivalent today | What to add, and where |
| --- | --- | --- | --- |
| Elements: `div()`, `Render`, `RenderOnce`, `IntoElement` | 1,524 `div()` | Semantic nodes; adapter-owned split layout and clipped pane bodies are implemented | Reuse the current split shell. Add shared semantics for general boxes, virtual lists, and overlays only when a screen needs them. |
| Styling and flex (Taffy under GPUI) | `.flex()`, `.w()`, `.rounded()`, `.border()`, `.shadow()`, `.overflow_hidden()` | `Style` with color, padding, gap, weight, align | Core: size constraints (fixed, min, max, fill, fraction), grow and shrink, radius, border, shadow, opacity, overflow, and absolute position. Desktop: a flex-subset solver, or Taffy. |
| Entities and models: `Entity<T>`, `cx.notify`, `subscribe`, `observe`, `EventEmitter` | 208 `Entity<`, 964 `notify` | The app rebuilds semantic views; retained drawing-operation damage is implemented in the adapter | Keep one-way state. Measure long transcripts before adding a broader retained node tree. |
| Actions: `actions!`, `on_action`, `key_context` | 5 blocks, 81 handlers | Typed intents resolved from `Activation` | Core: a `Command` registry with ids, labels, and default key chords, dispatched as intents. Used by menus, the palette, and keys. |
| Key bindings: `KeyBinding::new`, contexts, `clear_key_bindings` | 93 bindings, user rebinding | Tab, Enter, Space, Escape, quit, and `App::key_bindings` with revision-bound chord activation | Add contexts, user rebinding, persistence, and a shared command registry when needed. |
| Focus: `FocusHandle`, `track_focus`, `tab_index`, tab groups | 83 handles, 93 tab references | Focus order equals button order | Core: `focusable` and focus scopes on nodes, focus-restore rules, and `autofocus`. Desktop: focus rings, scope trapping in modals. |
| Text input: `EntityInputHandler`, marked text, `bounds_for_range` | The composer's hand-rolled input (38 actions, 66 bindings) | `Composer` with a draft string; no editing | Core: an `EditState` (text, selection, caret affinity, undo, grapheme and word motion). Desktop: winit `Ime` events, IME candidate position, and clipboard. Phones: map to `UITextView` and `EditText`. |
| Code editor: `gpui_base::input::Editor` with a rope | File preview and editing | None | Don't port (see [what not to port](#what-not-to-port-and-what-to-adapt-instead)). A read-only code view is enough. |
| Rich text: `StyledText`, `TextRun`, custom `LinkRanges` | Markdown, links, selection | `RowDisplay` runs, rectangles, and links for transcripts | Reuse the transcript display lists on desktop. Add hit testing, link hover, and cross-row selection in the desktop adapter. |
| Virtual lists: `list()`, `ListState`, `uniform_list`, `ScrollHandle` | 7 variable-height lists | Transcript frames with `rows_in` | Core: a general `VirtualList` element whose rows are keyed and measured by the adapter or by Rust. Desktop: scroll containers, scrollbars, anchors, and the stick-to-bottom spring. |
| Overlays: `anchored()`, `deferred()`, `occlude()` | About 10 `anchored_menu` variants, dialogs | None (the composer's long-press choices only) | Core: `Overlay` with an anchor node, placement, dismissal rules, and modality. Desktop: an overlay layer with occlusion. Phones: `UIMenu`, sheets, and `PopupMenu`. |
| Tooltips: `.tooltip()`, hover intent | 33 calls | None | Core: a `tooltip` property on nodes. Desktop: hover delay and placement. Phones: accessibility hint. |
| Drag and drop: `on_drag`, `on_drop`, `drag_over`, `ExternalPaths` | 47 drops, 33 drags, 4 payload types | None | Core: `Draggable { payload }` and `DropTarget` intents, and a `Reorder` list mode. Desktop: drag ghost, auto-scroll, winit `DroppedFile`. |
| Animation: `with_animation`, `Animation`, easing, `request_animation_frame` | 25 calls, `motion.rs` presets | `SurfaceLifecycle.frame_delta` only | Core: transition specs (property, duration, curve) and named motion tokens. Desktop: a frame clock that runs only while something animates, and a reduced-motion switch. |
| Images: `img()`, `ImageSource::evict` | 16 calls | None; the QR code is a painted `Surface` | Core: `Image { resource, fit, radius }`. Desktop: decode, a texture atlas, and eviction. Phones: `UIImage` and `Bitmap`. |
| SVG icons: `svg()` via `icons.rs` | 115 icons | 17 `Glyph` values | Core: grow the glyph set. Desktop: rasterize icons with `resvg` into the atlas at each scale. |
| Custom paint: `canvas()`, custom `Element` | 44 canvases, 8 elements | `Surface` with `paint_surface` | Keep `Surface` for rings, graphs, and spinners. Move it onto the GPU renderer. |
| Fonts: GPUI text system, bundled Geist | 16 faces | Bundled Inter and JetBrains Mono, `swash`, `ShapingMeasurer` | Keep the bundled faces. Add a glyph atlas and font fallback for emoji and CJK. |
| Blur and frost: `BackdropBlur`, `EdgeFade` (fork-only) | 24 `frosted(` calls, 31 `edge_faded(` | Backdrop blur for the whole window | Desktop: per-region blur and edge-fade masks in the GPU renderer, later. |
| Windows: `WindowOptions`, transparent title bar | One main window | One winit window with `Options` and `Look` | Desktop: transparent title bar, saved geometry, and a second window only if a screen needs one. |
| Native menus: `cx.set_menus` | App, Edit, View, Window | `NSStatusItem` menu in `crates/openagents-desktop/src/menubar.rs` | Desktop host: a menu bar driven by the core `Command` registry, with `muda` or the existing `objc2` code. |
| Async: `cx.spawn`, `gpui_tokio`, timers | 184 spawns | App worker threads and `Waker` (`crates/openagents-desktop/src/worker.rs`) | Keep work off the UI thread in application code. The adapter needs timers only for animation and hover delays. |
| Testing: `#[gpui::test]`, `TestAppContext` | 209 tests | `capture` and `capture_views`, pure state tests | Desktop: scripted input (click, type, key chord) against a headless window, and PNG snapshot comparison. |

## Component inventory and port difficulty

Difficulty: **trivial** (views over existing Rust Native), **moderate** (needs
one new capability), **hard** (needs several capabilities or tricky
behavior), **blocked** (needs a capability that doesn't exist and is large
or platform-specific). "Port" says whether the chat-first plan includes it.
All paths are under `comet/crates/ui/src/`.

### Shell and navigation

| Component | Path | LOC | Difficulty | Rust Native gap | Port |
| --- | --- | --- | --- | --- | --- |
| Shell root, routes, overlays, title bar, seams | `shell.rs` | 16,132 | Hard | Layout, split panes with drag resizing, overlays, transparent title bar, focus scopes | Yes, much smaller |
| Spaces sidebar: space filter, sessions list, add-space palette | `shell/spaces.rs` | 6,881 | Hard | Virtual list, drag reorder (10 drag sites), context menus, search field | Adapt as chats and projects |
| Sidebar sections | `shell/sidebar_sections.rs` | 1,065 | Moderate | Collapsible sections, virtual list | Adapt |
| Pins overlay | `shell/sidebar_pins.rs` | 222 | Trivial | None beyond state | Yes |
| Session navigation and title | `shell/tabs.rs` | 750 | Moderate | Key bindings | Yes |
| Command palette | `shell/command_palette.rs` | 644 | Moderate | Overlay, text field, keymap | Yes |
| Focus boundaries | `shell/navigation_focus.rs` | 171 | Moderate | Focus scopes | Yes |
| Side chats in the right pane | `shell/side_chats.rs` | 671 | Moderate | Second transcript instance, tabs | Later |
| Agent update island | `shell/harness_updates.rs` | 1,044 | Moderate | Animation (height tween) | Adapt as the update strip |
| Project actions menu and editor | `shell/actions_ui.rs`, `project_actions.rs` | 1,276 + 476 | Moderate | Overlay, text fields | No |
| Files explorer panel | `shell/files_panel.rs` | 605 | Hard | Tree, virtual list | No |
| Project icon fetch | `shell/project_icon.rs` | 408 | Moderate | Images | Later |
| Native menus | `app_menus.rs` | 487 | Moderate | Command registry, native menu host | Yes |
| Navigation tests | `shell/navigation_tests.rs`, `shell/files_panel_workspace_tests.rs` | 535 + 549 | n/a | Scripted-input harness | Rewrite |

### Chat surface

| Component | Path | LOC | Difficulty | Rust Native gap | Port |
| --- | --- | --- | --- | --- | --- |
| Transcript: block rows, tool groups, chips, stick-to-bottom spring, anchors | `transcript.rs` | 14,036 | Hard | Desktop transcript painter, scroll anchors, spring, row widgets, image rows | Yes |
| Markdown rendering: tables, code blocks, lists, quotes | `markdown/render.rs` | 3,039 | Moderate | Already modeled in `crates/rust-native/src/layout/rows.rs`; needs desktop painting and table scrollers | Yes |
| Link hit testing and presentation | `markdown/link_interaction.rs`, `link_presentation.rs`, `links.rs`, `link_destination.rs` | 1,054 + 394 + 230 + 93 | Moderate | Link hit rectangles exist; add hover, tooltip, context menu | Yes |
| Cross-row text selection | `markdown/selection.rs` | 454 | Hard | Selection model across virtualized rows, copy | Yes |
| Streaming fade-in veil | `markdown/veil.rs` | 508 | Moderate | Per-run opacity animation | Yes |
| Mermaid diagrams | `markdown/mermaid.rs` | 192 | Blocked | SVG rendering of generated diagrams | No |
| Message rail minimap | `rail.rs` | 805 | Moderate | Hover preview overlay, smooth scroll | Later |
| Composer: input, IME, selection, undo, mentions, slash commands, send and stop, question wizard | `composer.rs` | 13,996 | Hard | Text editing contract, IME, clipboard, overlays for completion, key bindings | Yes |
| Composer dock choreography | `composer_dock.rs`, `composer_dock/panel_handoff.rs` | 1,197 + 103 | Hard | Layout-measured animation | Simplify |
| Markdown-aware composer helpers | `composer_markdown.rs` | 977 | Moderate | Text editing contract | Later |
| Pending-message queue with drag reorder | `queue.rs` | 2,137 | Moderate | Drag reorder | Yes, as steer or queue |
| Attachments, uploads, image cache, lightbox | `attachments.rs` | 1,472 | Hard | Images, external file drop, paste of images | Yes, images only |
| Context-block badges | `badges.rs` | 252 | Trivial | Pill style | Yes |
| Notice chip | `notice.rs` | 136 | Trivial | None | Yes |
| Context and plan usage rings | `context_usage.rs`, `account_usage.rs` | 228 + 514 | Moderate | `Surface` ring paint, popover | No (see [what not to port](#what-not-to-port-and-what-to-adapt-instead)) |
| Loaders and spinners | `loaders.rs` | 406 | Moderate | Animation clock | Yes, as `Working` |
| New-thread background effects | `new_thread_background_effects.rs`, `new_thread_background_mask.rs`, `new_thread_background_image.rs` | 499 + 235 + 10 | Hard | Image effects, masks | No; Verse backdrop replaces it |
| Review comments on diff and file lines | `comments.rs`, `comment_ui.rs` | 422 + 316 | Moderate | Text field, inline cards | Later |

### Pickers, popovers, and motion

| Component | Path | LOC | Difficulty | Rust Native gap | Port |
| --- | --- | --- | --- | --- | --- |
| Repo, branch, harness and model, and traits pickers | `pickers.rs` | 7,780 | Hard | Overlays, filterable lists, nested menus, `uniform_list` | Adapt as agent and project pickers |
| Popover and menu primitives and reducers | `popover.rs` | 2,527 | Moderate | Overlay layer; reducers port as pure Rust | Yes |
| Hover intent for nested menus | `popover/hover_intent.rs` | 258 | Moderate | Pointer-path tracking | Yes |
| Menu contained in a dialog | `popover/contained.rs` | 258 | Moderate | Overlay placement inside a modal | Yes |
| Motion presets and curves | `motion.rs`, `motion/windows_pulse.rs` | 1,164 + 99 | Moderate | Animation model | Yes, as tokens |
| Edge fades | `edge_fade.rs` | 427 | Hard | Fork-only GPU primitive | Later |
| Frosted surfaces | `frost.rs`, `surface_chrome.rs` | 177 + 46 | Hard | Per-region backdrop blur | Later |
| UI icons and file icons | `icons.rs`, `file_icons.rs` | 286 + 513 | Moderate | SVG icon rendering | Yes, UI icons only |

### Review and workspace panes

| Component | Path | LOC | Difficulty | Rust Native gap | Port |
| --- | --- | --- | --- | --- | --- |
| Diff viewer: unified and split, three scopes, fold tween, time-sliced highlighting | `changes.rs` | 6,458 | Hard | Virtual list at line granularity, syntax spans, animation | Yes, read-only unified first |
| Pull request state | `change_requests.rs` | 655 | Trivial | Mostly model code | Later |
| Git history with lane graph and column drag | `history.rs` | 5,263 | Hard | Virtual list, custom graph paint, drag reorder | No |
| File tree with indent guides and drag out | `files/tree.rs` | 586 | Hard | Tree, virtual list, drag | No |
| File preview and editor host | `files/preview.rs`, `files/editor.rs`, `files/editor_adapter.rs`, `files/document.rs` | 4,806 + 240 + 348 + 592 | Blocked | A full code editor | No |
| Markdown preview with comments | `files/markdown_preview.rs`, `files/markdown_media.rs` | 1,996 + 67 | Moderate | Virtual list, images | No |
| Image preview and lightbox | `files/image_preview.rs`, `image_viewer.rs`, `image_media.rs` | 559 + 572 + 426 | Moderate | Images, zoom and pan gestures | Later, for attachments |
| File search | `files/search.rs` | 913 | Moderate | Filterable virtual list | No |
| File model, client, watcher, git status, sections | `files/mod.rs`, `model.rs`, `client.rs`, `watch.rs`, `git_status.rs`, `sections.rs` | 1,251 + 1,142 + 890 + 171 + 496 + 1,279 | Moderate | State only | No |
| Terminal: alacritty grid, tabs with drag reorder, height drag | `terminal/emulator.rs`, `view.rs`, `panel.rs` | 740 + 1,113 + 2,015 | Hard | Monospace grid paint, key encoding, drag reorder | No (see below) |
| Embedded browser: WKWebView child view, WebKitGTK offscreen | `browser/` | 3,109 | Blocked | Native child views composited under an owned renderer | No |
| Appshots: capture the frontmost window, global hotkey | `appshots.rs`, `appshots/` | 1,231 + 2,719 | Blocked | Screen capture and accessibility permissions per platform | No |

### Settings

`comet/crates/ui/src/settings/` is 12,830 lines across 13 files, plus
`settings.rs` (3,350 lines of persisted settings).

| Page | Path | LOC | Difficulty | Gap | Port |
| --- | --- | --- | --- | --- | --- |
| Settings store, keymap config, clamping | `settings.rs` | 3,350 | Moderate | None; pure Rust | Adapt the parts used |
| Shared page scaffolding | `settings/widgets.rs` | 1,678 | Moderate | Rows, toggles, segmented controls | Yes |
| Appearance: system, light, dark, accent, fonts | `settings/appearance.rs` | 3,453 | Moderate | Pickers, color swatches | Reduced: appearance and text size only |
| Accounts: provider cards, usage meters, sign-in dialog | `settings/accounts.rs` | 2,650 | Hard | Dialogs, text fields, progress meters | Adapt as "Codex and Claude sign-in" status from `DSK-02` |
| Harnesses per device | `settings/harnesses.rs` | 1,384 | Moderate | Toggles | No |
| Shortcuts: record, conflicts, reset | `settings/shortcuts.rs` | 1,131 | Moderate | Keymap, key capture | Yes |
| Devices: registry, rename, copy id | `settings/devices.rs` | 492 | Moderate | Text field | Adapt as phones and computers |
| Archived chats | `settings/archived.rs` | 470 | Trivial | List | Yes |
| Notifications | `settings/notifications.rs` | 409 | Trivial | Toggles | Yes |
| Appshots hotkey | `settings/appshots.rs` | 323 | Moderate | Key capture | No |
| Composer defaults | `settings/composer.rs` | 291 | Trivial | Toggles | Later |
| File editing preferences | `settings/files.rs` | 249 | Trivial | Toggles | No |
| Thread naming | `settings/thread_naming.rs` | 161 | Trivial | Picker | No |
| Completion preferences | `settings/completion.rs` | 139 | Trivial | Toggles | No |

### Application services

| Component | Path | LOC | Difficulty | Gap | Port |
| --- | --- | --- | --- | --- | --- |
| App state and engine subscriptions | `state.rs` | 5,118 | Moderate | None in the framework; replace with `crates/openagents-mobile` state | Replace |
| Theme and typography | `theme.rs`, `typography.rs`, `theme_library.rs`, `appearance.rs`, plus `comet/crates/theme` | 2,930 + 886 + 217 + 409 + 4,738 | Moderate | Token groups in the core | Adapt as one OpenAgents theme with light and dark |
| Notifications | `notify.rs` | 383 | Moderate | Platform notification host | Yes |
| Sounds | `sound.rs` | 617 | Trivial | Audio playback host | Later |
| In-app update strip | `app_update.rs` | 540 | Trivial | Reuse `crates/openagents-desktop/src/update.rs` | Yes |
| Workspace links and deep links | `workspace_links.rs`, `links.rs` | 253 + 230 | Moderate | URL-scheme host | Later |
| Syntax highlight cache | `syntax_cache.rs`, plus `comet/crates/syntax` | 199 + 1,690 | Moderate | Highlight spans in the core | Yes, for code blocks and diffs |

### Counts

| Difficulty | Components | Zeron LOC |
| --- | --- | --- |
| Trivial | 12 | about 4,100 |
| Moderate | 39 | about 61,800 |
| Hard | 17 | about 82,900 |
| Blocked | 4 | about 13,200 |
| Total | 72 | about 162,100 |

The total is higher than the UI crate's 157,280 lines because the theme and
syntax rows include `comet/crates/theme` and `comet/crates/syntax`. The
chat-first plan ports or adapts 41 of the 72 components. Their Zeron
sources total about 117,000 lines, but that figure counts whole files,
including their tests and the parts the plan drops, such as most of
`shell.rs`. OpenAgents' application state already lives in Rust, and the
chat-first product needs fewer surfaces. Implement them as small product
slices. The shell used existing semantic
nodes and a bounded adapter extension, so the original prediction of
25,000–35,000 view lines plus 20,000–25,000 framework lines is withdrawn.
Measure the resulting code after the transcript and composer slice instead
of using Zeron's source size as a proxy for effort.

## Rust Native capabilities to add

"Phones" says whether iOS and Android also gain from the capability. "Core"
is `crates/rust-native`; "desktop" is `crates/rust-native-desktop`; "hosts"
are `bins/*-ios/host` and `bins/*-android/host`.

| # | Capability | Core | Desktop adapter | Hosts | Phones | Estimate |
| --- | --- | --- | --- | --- | --- | --- |
| C1 | GPU renderer: instanced rounded rectangles, borders, shadows, clip stack, layers, glyph atlas from `swash`, damage tracking | None | Retained damage and partial uploads implemented; add GPU primitives and glyph atlas if long-content measurements require them | None | No | 6–10 h; damage and partial uploads done |
| C2 | Box layout: size constraints, grow and shrink, absolute position, overflow, split panes | New layout primitives and style fields; a flex-subset solver shared by any Rust-painting adapter | Split panes implemented; general solver remains | SwiftUI and Android map fields to their own layout | Partly: style fields | 2–3 h; split layout done |
| C3 | Style extensions: radius, border, shadow, opacity, size, theme token groups (RN6 brought forward) | `Style`, `StylePatch`, `StyleSheet` | Paint them | Map what the platform supports | Yes | 1–2 h |
| C4 | Text editing contract: `EditState`, selection, caret affinity, undo, word and grapheme motion, multi-line, auto-grow bounds | New `edit` module (RN4) | Caret, selection paint, key handling, winit IME preedit and commit, candidate window position | `UITextView` and `EditText` bridges | Yes | 6–10 h with C5 |
| C5 | Clipboard for text and images | Intent types | `arboard` or per-platform code | Platform pasteboard | Yes | Included in C4 |
| C6 | Virtual list and scroll containers: keyed rows, measured heights, anchors, scrollbars, stick-to-bottom spring | `VirtualList` element; generalize `layout::Frame` beyond transcripts | Clipped pane scrolling and scrollbars implemented; general virtualization, anchors, and momentum remain | Map to `UICollectionView` and `RecyclerView` | Yes | 2–3 h; pane scrolling done |
| C7 | Desktop transcript painter over `RowDisplay`, with link hover and cross-row selection | Selection model shared with phones | Paint runs, rectangles, widgets; hit testing | Android already has `TextSelection.kt` | Yes (shared selection model) | 3–5 h |
| C8 | Overlays: popover, menu, context menu, tooltip, modal dialog, anchored placement, dismissal, nested menus with hover intent | `Overlay` element and `tooltip` property | Overlay layer, occlusion, focus trap | `UIMenu`, sheets, `PopupMenu`, dialogs | Yes | 4–6 h |
| C9 | Commands, key bindings, focus scopes | `Command`, `Keymap`, focus fields | Command chords, focus rings, and focus scrolling implemented; contexts and modal scopes remain | iPad hardware keyboard commands | Partly | 2–3 h; shell chords and focus scrolling done |
| C10 | Animation: transitions and springs, motion tokens, reduced motion | Transition specs on nodes | Frame clock that runs only while animating | `withAnimation` and `ViewPropertyAnimator` | Yes | 0.5–1 h |
| C11 | Images and SVG icons | `Image` element and a larger glyph set | Vector drawing of the closed Glyph set implemented; image decoding and cache remain | `UIImage` and `Bitmap` | Yes | 0.5–1 h; vector glyphs done |
| C12 | Drag and drop: internal payloads, list reordering, external file drop | `Draggable`, `DropTarget`, `Reorder` | Drag ghost, auto-scroll, `DroppedFile` | iPad drag and drop later | Later | 2–4 h, later |
| C13 | Syntax highlight spans for code blocks and diffs | Spans on `Markdown` code and a `Diff` element | Paint colors only | Paint colors only | Yes | 1–2 h |
| C14 | Accessibility tree for painted desktop views | Existing labels and `Accessibility` records | AccessKit (`accesskit_winit`) | Already native | No | 2–4 h |
| C15 | Native menu bar and notifications driven by commands | Command ids and labels | None | Desktop host: `muda` or the `objc2` code in `crates/openagents-desktop/src/menubar.rs` | No | 1–2 h, application budget |
| C16 | Scripted-input test harness and PNG snapshots | Fixture views | PNG capture fixtures, coder-desk live inputs, and full/incremental pixel comparison; extend with each feature | None | No | Shell captures and pixel-parity checks done; broader checks in feature budgets |
| C17 | Per-region blur and edge fades | Style hints | GPU passes (the backdrop blur exists) | Material blur on iOS | Partly | 3–5 h, later |

The remaining framework budget is 32–54 focused hours including the spike.
C12 and C17 are deferred, and C15 belongs to the application budget. The
table records remaining work after the shell implementation; overlapping
capabilities are counted once in the summary. C4, C6, C8, C10, C11, and C13 help the phone apps directly: the
phone composer needs C4, long chats need C6, chat card menus need C8, and
code blocks need C13.

The earlier review's items 1 and 3 have already landed in
`crates/rust-native/src/layout/` and `crates/rust-native/src/markdown/`.
That is the main reason the transcript is "hard" rather than "blocked" on
desktop: the rows, heights, and display lists exist, and only a desktop
painter is missing.

## What not to port, and what to adapt instead

The OpenAgents desktop app's job is to chat with OpenAgents, dispatch Coder
to a project on that computer, show Gym and eval results, and show Verse.
The [IDIOT PROOF checklist](../product/2026-09-28-app-wireframe.md#idiot-proof-checklist)
requires one obvious primary action per screen (`CHK-01`), no jargon
(`CHK-02`), and no required setup (`CHK-05`). Zeron is built for developers
who run many coding agents at once, and much of it fails those checks.

| Zeron surface | Decision | Reason | Adapt instead |
| --- | --- | --- | --- |
| File editor, file tree, file search, markdown preview (`files/`, 15,336 lines) | Don't port | An IDE; a person who has never heard of agents doesn't need it | A read-only "What changed" card in chat that opens the diff pane |
| Git history lane graph (`history.rs`) | Don't port | Developer tooling | A commit summary line on Coder's result card |
| Terminal panel (`terminal/`, 3,884 lines) | Don't port now | `DSK-01.E03` lets the phone open a terminal on the computer; the desktop doesn't need its own grid | Revisit if Coder needs a visible log; show logs as a collapsed tool card first |
| Embedded browser (`browser/`) | Don't port | Needs native child views under an owned renderer on each platform | Open links in the system browser |
| Appshots (`appshots*`) | Don't port | Screen-recording permissions are setup a new player shouldn't face | Drag or paste an image into the composer |
| Eight vendor harness pickers and per-device harness settings (`pickers.rs`, `settings/harnesses.rs`) | Don't port | OpenAgents chooses defaults for the player (`CHK-07`) | One "Coder" agent; show Codex or Claude sign-in status as `DSK-02` does |
| Plan usage rings and account switching (`account_usage.rs`, `context_usage.rs`, `settings/accounts.rs`) | Don't port | Vendor-account management is jargon and setup | A plain "Coder can't reach Claude right now" card with one next step |
| 31 themes, VS Code theme import, surface preferences (`comet/crates/theme`, `settings/appearance.rs`) | Don't port | Choice without player value | One OpenAgents theme, light and dark, with a text-size setting |
| New-thread background effects (dither, ASCII, halftone) | Don't port | The Verse backdrop is the product's own backdrop | Keep `crates/openagents-desktop/src/backdrop.rs` behind the chat |
| Mermaid diagrams | Don't port | Needs an SVG renderer for generated content, rarely used in chat | Show the source as a code block |
| Spaces as device and folder pairs, space filter (`shell/spaces.rs`) | Adapt | "Workspace" is banned wording on the desktop app | "Projects" on "this Mac", and a flat chat list that sorts by attention |
| Session queue with steer (`queue.rs`) | Adapt | Useful, but "steer" is jargon | "Send now" or "Send after this" on a queued message |
| Question wizard in the composer | Adapt | Matches `CHK-11`, talk leads to a tap | Chat cards with one primary button |
| Command palette | Adapt | Good for returning users, invisible to new ones | Keep it keyboard-only; never the only path to an action |
| Diff pane (`changes.rs`) | Port, reduced | "What changed" review is core to trusting Coder | Unified, read-only diff first; split view later |

Also out of scope are Zeron's engine, harness adapters, Loro sync, edge
Durable Objects, and MCP server. OpenAgents' host, Coder, NIP-HOST, and
NIP-REACH cover those roles, and the earlier review ranks what to borrow
from them.

## Licensing and attribution

- **Zeron** is MIT, copyright 2026 Wing (`comet/LICENSE`). You may copy,
  modify, and redistribute it, including under this repository's Apache-2.0
  license, if you keep the copyright and permission notice with any
  substantial portion you copy.
- **Repository rule.** Reimplement designs and name the source in the
  commit message; don't vendor Zeron code (see the
  [earlier review](2026-09-28-zeron-comet-review.md)). If a port does copy a
  substantial portion, such as the popover reducers or the motion curves,
  add the MIT notice to that file's header and to a third-party notices
  file for the desktop crate.
- **GPUI** and the `zeronsh/zui` fork are Apache-2.0 (Zed). The recommended
  plan doesn't depend on them. If the GPUI spike is chosen, keep the Apache
  `NOTICE` obligations and never pull in Zed's GPL-3.0 crates (`editor`,
  `ui`, `theme`, `markdown`, `workspace`), as Zeron itself avoids them.
- **gpui-component** (`gpui-base`) is Apache-2.0. The plan doesn't use it.
- **Assets.** Geist fonts are SIL OFL 1.1; the port keeps Inter and
  JetBrains Mono instead. The 115 UI icons have no separate license file in
  `comet/crates/ui/assets/icons/`; treat them as part of Zeron's MIT code, or
  draw our own icons, which the `Glyph` set already requires. The Symbols
  file icons (MIT) and the 31 theme palettes (MIT sources listed in
  `comet/THIRD_PARTY_NOTICES.md`) aren't needed.
- **Upstream ideas.** Zeron credits pretext and mugen for text and
  virtualization (`comet/docs/research/mugen-pretext.md`) and streamdown for
  markdown mending. Check those projects' licenses before porting an idea
  that came from them.
- **Third-party crates** that the capabilities above suggest: `taffy`
  (MIT), `resvg` and `usvg` (Apache-2.0 or MIT), `arboard` (Apache-2.0 or
  MIT), `accesskit` (Apache-2.0 or MIT), `muda` (Apache-2.0 or MIT), and
  `tree-sitter-highlight` (MIT). Record each in `docs/dependencies.md` when
  it's added.

## Performance considerations

Zeron's own measurements set the bar. They are the authors' numbers on
their machines, not ours:

- Streaming UI CPU is about 12.9% of one core; an empty chat is about 0.6%
  (`comet/docs/performance-resource-usage.md`).
- A 1,000-line composer draft once cost 238% CPU through a repaint loop and
  now costs 0.2% (`comet/docs/performance-composer.md`).
- Opening a 21 MB chat takes 806 ms of background preparation and 12 ms on
  the UI thread; revisiting costs 9 ms (`comet/docs/whale-ui-responsiveness.md`).
- Peak footprint is 302–329 MiB, most of it Metal and IOAccelerator memory,
  not Rust heap (`comet/docs/performance-macos.md`). The fork bounds GPU
  scratch memory and evicts image atlas tiles for this reason.

What that means for Rust Native on desktop:

1. **The shell lag is measured and fixed.** Whole-window foreground painting
   cost 121 ms at 1200×840 and 208 ms at 1271×1428 in the unoptimized build;
   2× scale cost 461 ms. Retained painting reduced warmed hover painting
   to about 2.6 ms at 1× and 8.5 ms at 2× in the same build. The optimized
   build measures roughly 0.1–0.4 ms for hover, under 2 ms for scrolling,
   and 1–4 ms for sidebar resizing. These are CPU foreground measurements,
   excluding GPU rendering and display latency; they don't establish a
   3,000-row transcript budget.
2. **Paint only what changed.** Drawing-operation damage and partial uploads
   are implemented. Reuse them for the first transcript painter and measure
   whether visible-row culling is sufficient before adding GPU foreground
   rendering or keyed node mounting.
3. **Measure off the UI thread.** The transcript layout already produces
   immutable frames. Run it on a worker and publish frames, as the phone
   design does, so the UI thread only paints.
4. **Idle must be free.** The adapter already sleeps until the next tick.
   Keep animations on a clock that stops when nothing moves, and cap the
   Verse backdrop as `crates/openagents-desktop/src/backdrop.rs` does
   (30 fps active, 10 empty, 0 hidden).
5. **Bound GPU memory.** Size glyph and image atlases, evict unused tiles,
   and release blur scratch textures when idle, as the Zeron fork learned.
6. **Keep syntax highlighting paint-only.** Highlight in the background and
   apply colors to runs, so highlighting never changes layout, as Zeron and
   the phone transcript already do.
7. **Budget the view tree.** The 1,024-node and 512 KiB view bounds suit a
   screen but not a whole desktop shell with a long sidebar. Virtual lists
   must page rows by index instead of placing every row in the tree.

## Phased plan

Each phase ends with a snapshot fixture, a scripted-input test, and a note
in the desktop verification directory, `docs/desktop/verification/`.

### Phase 0: long-content measurement and needed contracts (2–4 h)

- Build a 3,300-row transcript fixture and a virtualized sidebar with 500
  chats. Start with the existing retained painter and record its costs.
- Add a small GPU foreground comparison only if it misses the budget.
  Decide whether broader box layout needs a solver or another bounded
  adapter extension.
- **Acceptance:** a written decision with measured frame time (p50 and p99),
  idle CPU, streaming CPU, memory, build time, and binary size for both
  the current adapter and any proposed replacement on the available Linux
  machine; confirm platform-specific behavior on an Apple silicon Mac.

### Phase 1: remaining renderer and layout work (9–15 h)

- Extend C1, C2, and C3 where the measured next slice needs them. Split
  panes, clipped bodies, damage, partial uploads, and shell captures exist.
- Keep `DSK-01` to `DSK-04` correct through any painter changes.
- **Acceptance:** the existing desktop snapshots match within a small pixel
  tolerance; scrolling the 3,300-row fixture (without transcript features)
  holds p99 frame time under 8.3 ms at 120 Hz; idle CPU is under 1%.

### Phase 2: input (8–13 h)

- C4, C5, and C9.
- A single-line and multi-line text field as Rust Native elements, with
  undo, selection, clipboard, and IME.
- **Acceptance:** typing Japanese with the macOS Japanese input method shows
  marked text and commits correctly; a 1,000-line draft stays under 1% CPU
  when idle; Tab order and focus restore pass scripted tests; shortcuts
  rebind and persist.

### Phase 3: lists, transcript, and overlays (13–22 h)

- C6, C7, C8, C10, C11, C13, and C14.
- **Acceptance:** the desktop paints `Transcript`, `Message`, `Tool`, and
  `Markdown`, which `Scene::unsupported` no longer records; the view follows
  the tail while streaming and stops following on a wheel-up; selection
  copies text across rows; menus open, dismiss on an outside click or
  Escape, and trap focus in dialogs.

### Phase 4: remaining chat-first desktop, milestone A (13–22 h)

- The shell and resizable sample sidebar are implemented. Bind them to
  real chats, add a transcript with OpenAgents cards, a composer with
  attachments, and agent and project
  pickers, command palette, native menu bar driven by commands (C15), one
  OpenAgents theme with motion tokens, and the Verse backdrop behind the chat. Application state comes from
  `crates/openagents-mobile`, not new desktop-only models.
- **Acceptance:** a first-time player opens the app, chats with OpenAgents,
  and dispatches Coder to a project on that Mac with no terminal step and
  no explanation, and every screen passes `CHK-01` to `CHK-13`; streaming
  CPU is at most 15% of one core; peak memory is at most 350 MiB.

### Phase 5: review, settings, and phones, milestone B (8–14 h)

- Read-only unified diff pane, "What changed" card, settings (appearance,
  text size, shortcuts, notifications, phones and computers, archived
  chats), notifications, and the update strip.
- Map the new text editing, overlay, image, and highlight contracts in the
  iOS and Android hosts.
- **Acceptance:** a Coder result opens its diff in the pane; settings
  changes persist across restarts; notifications open the right chat; the
  phone composer edits through `EditState` and chat card menus use native
  menus on both phones.

### Phase 6: optional surfaces (not scheduled)

- Split diff view, side chats, message rail, per-region frost and edge
  fades (C17), drag reorder in lists (C12), and, only with a product
  decision, the terminal panel or a read-only file view.

## Proposed issue breakdown

The shell and sidebar work is tracked by the single issue
[#9993](https://github.com/OpenAgentsInc/openagents/issues/9993). The other
rows remain planning items, not newly created issues. Estimates are
remaining focused hours and reuse the capabilities already implemented.

| ID | Title | Phase | Depends on | Estimate |
| --- | --- | --- | --- | --- |
| CDP-00 | Measure the current adapter with long-content fixtures; choose further rendering work | 0 | None | 1.5–3 h |
| CDP-01 | `rust-native-desktop`: GPU quad, border, shadow, and clip renderer | 1 | CDP-00 | 3–5 h |
| CDP-02 | `rust-native-desktop`: glyph atlas and GPU text from `swash` | 1 | CDP-01 | 3–5 h |
| CDP-03 | `rust-native`: size, grow, position, and overflow fields, and the box layout solver | 1 | CDP-00 | 2–3 h |
| CDP-04 | `rust-native`: radius, border, shadow, opacity, and theme token groups | 1 | CDP-03 | 1–2 h |
| CDP-05 | `rust-native-desktop`: scripted input driver and PNG snapshot comparison | 1 | Implemented shell fixtures; extend with each feature | Shell fixtures and parity checks done; remaining checks in feature budgets |
| CDP-06 | `rust-native`: text editing contract (`EditState`), RN4 part 1 | 2 | CDP-03 | 3–5 h |
| CDP-07 | `rust-native-desktop`: text field painting, IME, and clipboard | 2 | CDP-02, CDP-06 | 3–5 h |
| CDP-08 | `rust-native`: commands, key map, and focus scopes | 2 | CDP-03 | 2–3 h |
| CDP-09 | `rust-native`: general virtual list and scroll containers | 3 | CDP-03 | 2–3 h |
| CDP-10 | `rust-native-desktop`: transcript painter, link hover, and cross-row selection | 3 | CDP-02, CDP-09 | 3–5 h |
| CDP-11 | `rust-native`: overlays, menus, tooltips, and modal dialogs | 3 | CDP-08 | 4–6 h |
| CDP-12 | `rust-native`: transitions, motion tokens, and reduced motion | 3 | CDP-01 | 0.5–1 h |
| CDP-13 | `rust-native`: images and a larger icon set | 3 | CDP-01 | 0.5–1 h |
| CDP-14 | `rust-native`: syntax highlight spans for code blocks and diffs | 3 | None | 1–2 h |
| CDP-15 | `rust-native-desktop`: AccessKit accessibility tree | 3 | CDP-03 | 2–4 h |
| CDP-16 | Implemented shell and sample sidebar; next bind real chat state | 4 | Current adapter; real chat state | Shell/sidebar done in #9993; real chat binding: 2–4 h |
| CDP-17 | Desktop chat: transcript and OpenAgents chat cards | 4 | CDP-10 | 4–6 h |
| CDP-18 | Desktop composer: attachments, queued messages, and questions as cards | 4 | CDP-07, CDP-13 | 4–6 h |
| CDP-19 | Desktop agent and project pickers and the command palette | 4 | CDP-11 | 1–2 h |
| CDP-20 | Desktop native menu bar and notifications driven by commands | 4 | CDP-08 | 1–2 h |
| CDP-21 | Desktop diff pane and "What changed" card | 5 | CDP-09, CDP-14 | 3–5 h |
| CDP-22 | Desktop settings: appearance, shortcuts, notifications, phones and computers, archived | 5 | CDP-07, CDP-11 | 2–4 h |
| CDP-23 | Phone hosts: map `EditState`, overlays, images, and highlight spans | 5 | CDP-06, CDP-11, CDP-13, CDP-14 | 3–5 h |
| CDP-24 | Update the Rust Native spec, architecture, and build order for the desktop adapter | 0 | CDP-00 | 0.5–1 h |
| CDP-25 | Desktop theme: one OpenAgents theme, light and dark, with motion tokens | 4 | CDP-04, CDP-12 | 1–2 h |

CDP-24 matters when shared layout semantics change: `crates/rust-native/docs/spec.md` and
`docs/coder/rust-native/build-order.md` still say adapters lay out blocks
and that no universal layout engine should be added up front. The implemented
adapter-specific split does not change the wire contract. A general shared
box solver would change that boundary and must be recorded when it is added.

## Open questions

1. **Does long-content rendering need GPU foreground drawing?** The shell
   is responsive on the retained painter. GPUI would supply general
   layout, IME, lists, and menus, but a rewrite is no longer a prerequisite
   for porting screens. The costs are a large dependency tree pinned
   to a fork, Zed's release cadence, limited accessibility support compared
   with AccessKit, and a second window system beside the winit and wgpu stack that Verse and the
   deck already use. The short spike (CDP-00) should decide with numbers
   from long-content fixtures, starting with the current adapter.
2. **Does the desktop app share one state crate with the phones?** This
   audit assumes the desktop reads `crates/openagents-mobile`. If desktop
   state diverges, the Rust Native benefit of one set of screens shrinks.
3. **How should the sample shell bind to real chats?** The shell is now
   implemented, and **Phones and computers** keeps `DSK-03` reachable.
   The next slice needs real chat state and a transcript/composer flow;
   a wireframe update should document the implemented structure.
4. **Windows support.** Verse doesn't build on Windows, so the backdrop is
   absent there. Decide whether Windows is in scope for milestone A.
5. **Does Coder need a visible terminal on desktop?** The answer decides
   whether a terminal surface, provisionally 3–6 focused hours using the
   existing Rust terminal crates, joins the optional work.
