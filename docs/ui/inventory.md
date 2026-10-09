# UI inventory: every system, every platform, at a glance

Status: audit, 2026-10-09. This is a read-only survey of the code at `origin/main`. It describes what exists today; it does not set policy.
The design-language plan for the web is in
[`docs/web/apps-sdk-ui-adoption-plan.md`](../web/apps-sdk-ui-adoption-plan.md).
The shared Rust UI contract is in [`crates/rust-native`](../../crates/rust-native/README.md).

## Headline

- **The repo has no GPUI, wgpui, egui, iced, Slint, Tauri, Dioxus, Leptos or Yew,** and no React or Effect Native code. Every UI is Rust, apart from thin Swift and Kotlin hosts on the phones.
- **Three component families ship today:**
  1. **`openagents-ui`**: server-rendered Maud HTML and CSS in the Apps SDK UI language. It is used only by the website and is the most complete component set.
  2. **Rust Native** (`rust-native`): one serializable `Element` tree. Adapters draw it on four targets:
     - desktop (`rust-native-desktop`)
     - iOS (SwiftUI/UIKit host)
     - Android (Views host)
     - web HTML (`rust-native-web`, used for `/components`)
  3. **Terminal ratatui** (`coder-terminal`, `coder-new`): the shipped `coder` CLI draws with ratatui directly, not through Rust Native.
- **Colors:** there is one token table, `oa-tokens`. Every family reads it except the desktop base theme and parts of the mobile hosts, which still hard-code colors.
- **Two catalogs that do not overlap:**
  - `/ui` shows family 1, the website components, in Light and Dark.
  - `/components` shows the terminal `coder-new` presentation, rebuilt as Rust Native views and rendered as HTML, in Noir only.
  - Neither catalog shows the desktop or mobile apps.

## 1. UI systems

| System | Crate(s) | Renders with | Used by |
| --- | --- | --- | --- |
| Web components (new design language) | `crates/openagents-ui` | Maud HTML plus one CSS bundle (`static/components/*.css`). Alpine CSP and HTMX for behaviour. Light and Dark from `oa-tokens` through `build.rs`. | `openagents-web` (every `UiPage` page and `/ui`) |
| Shared semantic views | `crates/rust-native` | Data only. `Element` = Surface, Stack, List, Text/RichText, Field, Choice, Dialog, Button, Transcript, Message, Markdown, Tool, Working, Composer (`src/view.rs:62`). Also handles Markdown (pulldown-cmark), syntax, transcript layout and shaping. | desktop, mobile, deck, verse, `/components`, coder-computers |
| Desktop adapter | `crates/rust-native-desktop` | `winit` 0.30 window and AccessKit. Hand-written layout. CPU software paint (`canvas.rs`, `paint.rs`) with `swash` glyphs and `resvg` SVG, presented through `wgpu` 29 (which also draws the backdrop shader). | `openagents-desktop`, `openagents-deck`, `verse` |
| Web adapter | `crates/rust-native-web` | Escaped HTML plus keyed browser mounting | `/components` (`openagents-web`), `coder-components-web`, `coder-browser-web` |
| Mobile hosts | `bins/openagents-ios/host`, `bins/coder-ios/host`, `bins/openagents-android/host`, `bins/coder-android/host` | The Rust tree arrives as JSON over FFI/JNI. iOS decodes it in `NativeView.swift`, `NativeChat.swift` and `NativeTranscriptPainter.swift`, which the OpenAgents app shares from coder-ios. Android uses `NativeRenderer.kt`, `TranscriptPainter.kt` and `InputBar.kt` on platform Views (no Compose). Verse draws with wgpu on Metal or Vulkan. | OpenAgents iOS and Android, Coder iOS and Android |
| App view builders | `crates/openagents-chat-app` (chat views, theme seam `visual.rs`), `crates/openagents-mobile` (app screens, `appearance.rs`), `crates/coder-mobile` | Build Rust Native trees | desktop, mobile |
| Coder presentation library | `crates/coder-ui` (about 15.5k lines) | Rust Native views of the `coder-new` screens, the `/components` fixtures (`src/catalog*.rs`) and `gui_theme.rs` (oa-tokens to native colors) | `/components`, `coder-new`, `coder-terminal` |
| Terminal | `crates/coder-terminal` (ladder, framed composer, markdown, spinner, components), `crates/coder-new` (shipped as `coder`), `crates/openagents-terminal` | ratatui 0.30 and crossterm 0.29. `coder-terminal/src/native.rs` is a Rust Native terminal adapter, but `coder-new` does not draw through it. | `coder` CLI, `openagents-terminal`, `terminal-mux` |
| Terminal-cell to SVG/HTML | `crates/coder-demo-ui` | ratatui `TestBackend` cells exported as SVG (`svg.rs`) or HTML (`html.rs`) | `coder-new --snapshot`, `/components` demo |
| Terminal window | `crates/terminal-app` | wgpu and winit with `terminal-gfx` (swash glyphs) over `coder-vt` | OpenAgents Terminal (GPU) |
| Chat Wasm helper | `crates/coder-chat-web` | A small Wasm module for the draft, composer and scroll behaviour on HTMX chat pages. It is not a component set. | `/chat/*` (`--chat-build`) |
| Tokens | `crates/oa-tokens` | Dependency-free Rust data: `apps_sdk` (Coder Light = upstream light), `noir`, `product`, and `palette` (`Palette::LIGHT`/`NOIR`). It generates nothing itself. | `openagents-ui`, `coder-ui`, `openagents-chat-app`, `openagents-mobile` |
| Effect Native / three-effect | none | Not in this repo. Effect Native appears only as the idea Rust Native carries forward (`docs/coder/rust-native/architecture.md`). | none |

## 2. What each shipping surface uses

| Surface | UI system today | New design language? |
| --- | --- | --- |
| openagents.com scrolling pages: home, chat, demo, docs, download, stats, live, profile, settings, environments, projects, tasks, account, auth, purchases, pilot, connect, efficiency | `openagents-ui` through `UiPage` (`crates/openagents-web/src/ui_page.rs`) | **Yes.** UI-13 removed the legacy CSS. They link only `/static/ui.css` (budget 290 KB, enforced by tests). Light and Dark. |
| openagents.com `/ui` | `openagents-ui::catalog` | Yes |
| openagents.com `/components` | `coder-ui` + `rust-native-web` + `coder-components-web` Wasm, with `components.css`/`native.css` | **No.** Coder Noir only, through `--noir-*` (`src/palette.rs`). |
| openagents.com `/everglade`, `/druid`, `/grid`, `/bunny` | Full-screen canvas plus `static/legacy-demo.css` (`layout::fullscreen`) | **No** (kept on purpose) |
| openagents.com `/studios/blue-rush` | Its own brand stylesheet (`static/bluerush/`) | No (separate brand, on purpose) |
| Desktop app (`crates/openagents-desktop`) | Rust Native + `rust-native-desktop` | **Partly.** The theme seam (`openagents-chat-app/src/visual.rs`, #11028) gives Light from `oa_tokens::Palette::LIGHT`, but Dark comes from constants inside `visual.rs`. `rust-native-desktop/src/theme.rs` does not use oa-tokens. Hard-coded colors remain in `screens.rs`, `shell.rs:1957-2018`, `route_chat.rs`, `slides.rs`, `grid.rs` and `route_future.rs`. |
| OpenAgents iOS (TestFlight 1.0.0) | Rust Native trees, SwiftUI/UIKit host | **Partly.** The palette arrives from Rust (`appearance.rs`, through oa-tokens). `GymViews.swift`, `WalletTab.swift` and `NativeChat.swift` (about 20 `Color(`) hard-code colors. |
| OpenAgents Android | Rust Native trees, Kotlin Views host | **Partly.** As on iOS. `Theme.kt` holds a hex fallback; `GymViews.kt` and `WalletScreen.kt` hard-code colors. |
| Coder iOS/Android (Verse) | `coder-mobile` Rust Native plus a wgpu Verse surface | Noir. Verse keeps its own art. |
| `coder` CLI (`coder-new`) | ratatui | Noir only, by decision (no light mode in terminal UIs) |
| OpenAgents Terminal (`terminal-app`) | wgpu + `terminal-gfx` glyphs | Noir only |

## 3. Component matrix

Legend:
- **ui**: `crates/openagents-ui/src`
- **cw**: `/components` (`coder-ui` fixtures)
- **rn**: a shared `rust_native::Element` (a cell may name the element alone)
- **D**: `crates/openagents-desktop/src`
- **iOS**: `bins/*-ios/host/App`
- **And**: the Android host
- **T**: `crates/coder-terminal/src` and `crates/coder-new/src`
- **–**: missing

| Component | Web `/ui` | Web `/components` | Desktop | iOS | Android | Terminal |
| --- | --- | --- | --- | --- | --- | --- |
| Button | ui `actions/button.rs` | rn Button | rn Button (`screens.rs:63`, `chrome.rs:496`) | rn to `NativeView.swift` | rn to `NativeRenderer.kt` | action rows only (`coder-new/src/ui/plugins.rs:932`) |
| Input / field | ui `forms/input.rs`, `textarea.rs`, `field.rs` | composer input | divergent: `rust-native-desktop/src/composer/field.rs`; rn `Field` **not drawn** | `SecretInputField.swift` | `DraftEditor.kt` | `editor.rs` |
| Composer | ui `shell/composer.rs` | `composer.input`, `composer.rails` | rn Composer (`rust-native-desktop/src/composer.rs`) | `NativeChat.swift` | `InputBar.kt` | `composer.rs`, `coder-new/src/ui.rs:913` |
| Message / thread | ui `shell/thread.rs` | `conversation.*` | rn Transcript/Message (`rust-native-desktop/src/transcript.rs`) | `NativeTranscriptPainter.swift` | `TranscriptPainter.kt` | `components/turn.rs` |
| Sidebar / nav item | ui `shell/layout.rs` (Sidebar, NavItem) | `agents.rail` | `chrome.rs` | native tabs (`AppTabs.swift`) | native tabs (`MainActivity.kt`) | `components/rail.rs` |
| Chat list | ui `shell/layout.rs` ChatList, `chat_row.rs` | `sessions.row` | `chrome.rs` from `openagents-chat-app/src/chat_list.rs` | rn list (`chat_list.rs`) | rn list | `coder-new/src/ui/resume.rs` |
| Menu | ui `overlays/menu.rs` | `pickers.slash`, `models.picker` | `appmenu.rs`, `menubar.rs`, palette in `chat.rs` | `UIMenu`/`contextMenu` | `PopupMenu` | `components/overlay.rs` |
| Dialog | ui `overlays/dialog.rs` | `approvals.disclosure` | – (rn Dialog not drawn; hand-made confirms) | `.sheet` (rn Dialog unused) | `Dialog(` | `coder-new/src/approval.rs` |
| Avatar | ui `actions/avatar.rs` | – | `style.button_avatar` (`chrome.rs:678`) | `AccountScreens.swift` | `AccountScreens.kt` | – |
| Badge | ui `actions/badge.rs` | – | – | – | ad hoc (`GymViews.kt`) | – |
| Alert | ui `actions/alert.rs` | `status.notice` | update strip only (`strip.rs`) | `.alert` | `AlertDialog` | notices only |
| Loading | ui `actions/indicator.rs`, `shimmer_text.rs` | `*.running` | rn Working | `ProgressView` | `ProgressBar` | `spinner.rs`, `progress.rs` |
| Tool call | ui `content/activity.rs` (ToolCall, ToolGroup) | `tools.*`, `edit.*`, `run.*` | rn Tool | rn Tool, painter | rn Tool, painter | `coder-demo-ui/src/tools.rs`, `components/run.rs` |
| Plugin card | ui `content/plugin_card.rs` | `plugin.status`, `plugins.row` | – (`route_plugin.rs` is a Map scene) | `GymViews.swift` (cards from `gym.rs`) | `GymViews.kt` | `coder-new/src/ui/plugins.rs`, `components/card.rs` |
| Task row | ui `shell/status.rs` TaskRow | `sessions.row` | `screens.rs` task status | rn (`coder_list.rs`) | rn | `components/run.rs` RunRow |
| Status chip | ui `shell/status.rs` (ChatStatus, TaskStatus) | `plugin.status` | `chrome.rs:865`, `chat.rs:4473` | Capsule (ad hoc) | `rounded()` (ad hoc) | `status_color` (`ui/plugins.rs:204`) |
| Code block | ui `content/code.rs` | `conversation.markdown` | rn Markdown + `rust-native/src/syntax.rs` | rn painter | rn painter | `markdown.rs` + `coder-ui/src/components/syntax.rs` |
| Markdown | ui `content/markdown.rs` | `conversation.markdown`, `conversation.tables` | rn Markdown (`rust-native-desktop/src/rich.rs`) | rn (parsed in Rust) | rn | `coder-terminal/src/markdown.rs` |
| Forms (select, switch, radio, slider, date, tags) | ui `forms/*` | `settings.*` fields | partial (settings) | native | native | settings overlays |

How code and tokens are shared:

| Family | Shares code across platforms | Tokens |
| --- | --- | --- |
| `openagents-ui` | Web only. Its components are not reused elsewhere. | `oa-tokens`, generated into CSS (Light and Dark) |
| Rust Native (desktop, mobile, `/components`) | Shared: the view tree, Markdown, syntax and transcript layout, intents. Each platform has its own painters, composer editors, menus, sheets and alerts. | Partly. Light comes from `oa-tokens`; Dark is a copy in `visual.rs`. The desktop base `theme.rs` and several mobile host files hard-code colors. |
| Terminal | Shares `coder-terminal`/`coder-ui` with `/components` through fixtures | `oa-tokens::noir` through `coder-ui/src/coder_noir.rs` (re-export, not a copy) |

So the same component is written up to five times, for example the composer: `shell/composer.rs`, `rust-native-desktop/src/composer.rs`, `NativeChat.swift`, `InputBar.kt` and `coder-terminal/src/composer.rs`. Only the Rust Native targets share its *meaning* (`Element::Composer`).

## 4. `/ui` vs `/components`

| | `/ui` | `/components` |
| --- | --- | --- |
| Shows | Every `openagents-ui` component: about 85 specimens in Foundations, Actions, Forms, Content, Overlays and Shell | `coder-new`'s terminal presentation (composer, conversation, rail, pickers, tools, plugins, settings, approvals) as Rust Native views. It has named variants (narrow, overflow, streaming) and width, height and scroll controls. |
| Source | `crates/openagents-ui/src/catalog/` | `crates/coder-ui/src/catalog*.rs`, served by `crates/openagents-web/src/components.rs` |
| Rendering | Server HTML in the real site shell | Server HTML via `rust-native-web`, made interactive by Wasm from `coder-components-web` (`--components-build`, `Dockerfile.components`) |
| Theme | Light and Dark | Noir only |
| Who uses it | Web development. Its specimens are the components the site ships. | The record of the Coder terminal UI on the web (spec `docs/coder/rust-native/coder-components.md`). No product page consumes it. |
| Linked from nav | No | No |
| Freshness | Current: the live site is built from these components | Current with `coder-new`, but it is a separate world. It shows nothing from desktop or mobile and predates Coder Light. |

**Overlap:** conceptual only (composer, message, tool call, markdown, plugin card, menu). The two catalogs share no code and no theme.

**Recommendation:** make `/ui` the one place to see everything. Keep `/components` as the interactive terminal-component lab, linked from `/ui`'s Terminal tab.

| `/ui` tab | Content | Feasible now? |
| --- | --- | --- |
| Web | The live specimens already there | Yes |
| Desktop | PNGs from `openagents-desktop --capture DIR`, which already exists. It renders headlessly against a fake host and fails on unsupported elements; 7 PNGs are committed in `crates/openagents-desktop/screenshots/`. Add a per-component capture through `rust_native_desktop::capture_views`. | Yes |
| Slides | `openagents-deck --deck NAME --capture DIR` PNGs | Yes |
| Terminal | `coder-new --snapshot` (110×36 SVG on stdout, `--models` for the picker; demo only in dev builds), plus a link to `/components` | Yes |
| iOS | `OPENAGENTS_UITEST_SHOTS=DIR` PNGs from `bins/openagents-ios/host/UITests/SharedContractsUITests.swift` (`--chat-fixture 1`, `--rust-native-fixture`). Needs a Mac simulator run, so it cannot run in Cloud Build. | Manual or CI-on-Mac |
| Android | Only `coder-android` has screenshot tests (`MobileAcceptanceTest.kt`); `openagents-android` has none | Needs a test first |

Implementation: a script writes the captures to `crates/openagents-ui/static/captures/<platform>/<component>-<theme>.png` together with a manifest. `/ui` then renders a row per component, showing the live web specimen next to each platform's capture, with "missing" where none exists. That row view is the matrix above, kept current by the capture run.

## 5. Converging on cross-platform components

**Share:**
- Tokens: one table, `oa-tokens`. Nothing else may hold a color value.
- Component specs: name, states, variants, roles and intents, as the `rust_native::Element` vocabulary.
- Behaviour: Markdown, syntax, transcript layout, composer editing rules and validation, all already in Rust.

**Stay per-platform:**
- Painting and widgets: Maud/CSS, the desktop painter, SwiftUI/UIKit, Android Views, ratatui.
- Platform chrome: tabs, sheets, system alerts, menus.
- Terminal stays Noir only.

| Phase | Goal | Steps |
| --- | --- | --- |
| 0. See it (this doc, then `/ui` tabs) | One page that shows everything | Capture script plus manifest. Add platform tabs and per-component rows to `/ui`, and link `/components`. |
| 1. One token source | No hard-coded colors | (a) Have `visual.rs` Dark read `oa_tokens::Palette::NOIR`. (b) Have `rust-native-desktop/src/theme.rs` take an `oa_tokens::Palette`. (c) Replace the literals in desktop `screens.rs`, `shell.rs`, `slides.rs`, `grid.rs` and `route_*`, and in `GymViews`, `WalletTab`/`WalletScreen` and `NativeChat.swift` with palette roles. (d) Add a test that fails on color literals outside `oa-tokens` and the art crates. |
| 2. One component spec list | The names in the matrix are the contract | Add a `docs/ui/components.md` table that maps each `openagents-ui` component to its `rust_native::Element` (or "platform-native"). Fill the gaps in the Rust Native vocabulary where `openagents-ui` already has the component: Badge, Avatar, Alert, StatusChip, PluginCard, TaskRow and Menu. |
| 3. Close adapter gaps | Each spec element is drawn on every Rust Native target | Draw `Field`, `Dialog` and `Choice` on desktop (they currently fall back to labels). Use `Dialog` on mobile instead of ad hoc sheets where the content is product content. Build an Android screenshot test for `openagents-android`. |
| 4. Web on the same spec (optional) | Spec parity, not shared code | Keep `openagents-ui` as the web painter, since it is the best-developed. Add a test that every spec element has a `/ui` specimen. A `rust-native-web` path for product pages is not recommended: it would duplicate `openagents-ui`. |

**First concrete steps:**
1. Point `openagents-chat-app/src/visual.rs` Dark at `oa_tokens::Palette::NOIR` and add a test that Light and Dark both come from `oa-tokens`.
2. Have `rust-native-desktop::Theme` take an `oa_tokens::Palette` and delete its own color constants.
3. Write a `scripts/ui-captures.sh` that runs `openagents-desktop --capture`, `openagents-deck --capture` and `coder-new --snapshot`, then add a "Platforms" section to `/ui` that shows those captures.
4. Add Badge and StatusChip to `rust_native::Element` (both have an `openagents-ui` reference), then draw them on desktop and both mobile hosts.
