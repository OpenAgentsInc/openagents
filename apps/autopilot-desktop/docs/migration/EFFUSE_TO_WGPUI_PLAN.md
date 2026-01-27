# Effuse -> WGPUI Migration Plan (Autopilot Desktop)

## Summary

Autopilot Desktop currently ships a Tauri + TypeScript/Effuse UI with a
large catalog of HTML/CSS components.
WGPUI is already the GPU UI stack for `crates/autopilot` and includes a mature
component system plus a full desktop and wasm platform layer. This plan
migrates Autopilot Desktop from Effuse to WGPUI as a **native Rust app** while
dropping the legacy UI contract entirely.

## North Star (Full Rustiness)

- One UI stack: WGPUI everywhere.
- One runtime: the same Rust app core drives both CLI and desktop.
- No WebView, no TS UI, no CSS catalog in the steady state.
- No Bun server in steady state (allow a short-lived compatibility bridge).

## Current State (grounded in repo)

Effuse + Autopilot Desktop
- UI entrypoint mounts `StatusDashboardComponent` via Effuse in
  `apps/autopilot-desktop/src/main.ts`.
- Effuse runtime lives in `apps/autopilot-desktop/src/effuse/` and includes:
  - Component system (`StateCell`, `html`` templates)
  - Hypermedia actions (`data-ez`)
- UI catalog + component registry is in
  `apps/autopilot-desktop/src/components/catalog.ts`.
- Large set of HTML/CSS components in
  `apps/autopilot-desktop/src/components/ai-elements/`.
- Backend emits UI events over `ui-event` (see
  `apps/autopilot-desktop/src-tauri/src/agent/ui.rs` and
  `apps/autopilot-desktop/src-tauri/src/contracts/ipc.rs`).

WGPUI elsewhere in repo
- `crates/wgpui/` provides GPU UI primitives, layout, text rendering, a
  component system, and a web + desktop platform layer.
- WGPUI is already used in `crates/autopilot` for the Autopilot UI, with
  extensive atoms/molecules/organisms/sections and a full rendering pipeline.
- Component contract is formalized in ADR-0019.

## Decision: Native WGPUI App (Option B)

We are committing to a **native WGPUI app (winit)** with backend logic extracted
into reusable Rust libraries. This means:

- No Tauri WebView or Tauri packaging in the final app.
- Direct Rust calls instead of Tauri IPC for the core UI/backend flow.
- Full conversion of Autopilot Desktop UI to WGPUI.

## Target Architecture

### Binaries

- `apps/autopilot-desktop-wgpu/`
  Native WGPUI desktop app (new). Runs WGPUI root component.
- `crates/autopilot/` (existing)
  Remains as the CLI + existing UI, but will share core runtime crates with
  the desktop app.

### New/Refactored Crates

- `crates/autopilot_app/`
  App core used by both CLI + desktop: session lifecycle, workspace wiring,
  tool execution orchestration, event stream.
- `crates/autopilot_ui/`
  Shared WGPUI surfaces (immediate-mode views + components).
- `crates/autopilot_desktop_shell/`
  Desktop-only: window management, menus, file dialogs, OS integration.

### What Gets Removed (end-state)

- `apps/autopilot-desktop/` (Tauri + TS/Effuse) -> deleted
- `apps/autopilot-desktop/src/effuse/**` -> deleted
- `apps/autopilot-desktop/src/components/**` -> deleted
- Tauri IPC layer -> deleted (or temporary compatibility bridge only)

## Contract Decision

We are **not** supporting the legacy UI protocol at all. The desktop UI
is rendered from typed view model state and WGPUI’s immediate-mode element
tree. If we ever need structured logs or replays, they are derived from
`AppEvent` + `UserAction` streams, not a legacy UI protocol.

## Component Model (Zed/GPUI-Inspired)

We will mirror the GPUI patterns from Zed:

- **Application + contexts**: a root `App` owns entities; `Context<T>` is passed
  to view methods for state access and notifications (see `crates/gpui/docs/contexts.md` in Zed).
- **Views are entities**: any view is an `Entity<T>` implementing `Render`,
  and GPUI calls `render()` each frame to build an element tree (see
  `crates/gpui/src/element.rs` in Zed).
- **Components are `RenderOnce`**: reusable, immediate-mode components are
  pure data objects that expand to elements (Zed’s `RenderOnce`).
- **Elements are low-level**: imperative rendering and custom layout live
  in element implementations for performance-sensitive surfaces.

In OpenAgents, we align WGPUI with this pattern:
- WGPUI views will be entities implementing `Render`.
- Shared UI state lives in entities; UI is reconstructed each frame.
- Event dispatch uses actions and context; no legacy UI protocol is retained.

## Zed Layout System Adoption Plan (Full App)

The current desktop UI still uses manual bounds math for layout, which breaks as
soon as font sizes, DPI, or content length change (as seen in the sidebar overlap).
Zed avoids this by building a **layout tree** (flex rows/columns + sizing rules)
and letting the layout engine compute bounds each frame. We will adopt the same
approach across the *entire* desktop UI.

### Goals

- All layout is expressed declaratively (flex rows/columns, gaps, fixed sizes).
- No hand-positioned y-offsets for rows or sections.
- Text size changes or DPI scaling never cause overlap.
- UI can be reflowed by theme/token changes alone.

### Option A: Use Zed’s GPUI crate directly

This is the “exact code” path:

- Add Zed’s `gpui` as a workspace dependency (either via git subtree or path).
- Render Autopilot UI as GPUI `Render`/`RenderOnce` views.
- Use GPUI’s `div()` + `Styled` flex helpers and element tree for layout.
- Bind WGPU window + renderer from GPUI’s platform layer.

**Pros**
- Proven layout engine + element tree used in production.
- Eliminates manual layout math immediately.

**Cons**
- License/compliance check (Zed is AGPL; ensure compatibility).
- Large dependency surface + platform abstractions may conflict with current WGPUI host.
- Requires bridging or replacing WGPUI components.

### Option B: Adapt WGPUI to the GPUI layout model (preferred if licensing conflicts)

WGPUI already includes the same primitives:

- `crates/wgpui/src/layout.rs` → Taffy layout engine
- `crates/wgpui/src/element/*` → element tree + layout requests
- `crates/wgpui/src/styled/*` → flex helpers (`div().flex().flex_col()`)

This path keeps our WGPU host and uses the Zed/GPUI layout model as the
blueprint. The work is to align the WGPUI API and Autopilot UI with GPUI
patterns rather than hand positioning.

#### Zed Layout API Parity Map

Mirror the ergonomics of Zed's `ui` crate:

- `h_flex()` / `v_flex()` helpers like `crates/ui/src/components/stack.rs` in Zed.
- `StyledExt` helpers like `crates/ui/src/traits/styled_ext.rs` (flex_1, min_w_0,
  overflow_* helpers, elevation helpers, debug backgrounds).
- `when()` style conditional chaining (Zed uses it heavily for readability).
- `rems()` / spacing tokens to avoid raw pixel math (Zed uses dynamic spacing
  and rem-based sizing in `ui`).
- `group()` and `inspectable` style tags for debugging layout trees.

In practice, this means adding a small "WGPUI UI prelude" (or
`autopilot_ui::prelude`) that exposes the same fluent builder API so Autopilot
layouts can be ported 1:1 from Zed-style code.

#### Layout Engine Alignment (Taffy)

Zed's layout tree is driven by Taffy. WGPUI already uses Taffy in
`LayoutEngine`, but our Autopilot desktop still uses manual bounds math.
The plan is to move all layout into the element tree:

- Every container that currently uses manual `Bounds` math becomes a `Div`
  (or a small WGPUI element) with a layout style.
- Panel headers, rows, and sidebar sections become flex children with fixed
  heights or `flex_grow` where appropriate.
- Scrollable regions use a single scroll container (no per-row y offsets).
- The root view becomes one flex row: left panel, center panel, right panel.
- The center panel is a flex column: header, thread body (flex_1), composer.

No manual `y += row_height` math remains in the steady state.

#### View + Entity Model Alignment

Zed uses `Entity<T>` + `Render` / `RenderOnce` to rebuild the element tree each
frame (see `crates/gpui/src/element.rs` in Zed). WGPUI already has
`Render` / `RenderOnce`, but Autopilot UI is still treating views like manual
paintable widgets. We will:

- Make `DesktopRoot` a `Render` view and build the tree every frame.
- Keep view state in entities and use `Context<T>` to mutate and notify.
- Use component-level `RenderOnce` for reusable pieces (badges, headers,
  status rows, tool call cards, etc).
- Adopt Zed-style action dispatch: events dispatch `Action` types through the
  context instead of directly calling handlers.

#### Autopilot UI Conversion Steps

1. Add Zed-style layout helpers to WGPUI or `autopilot_ui`:
   - `h_flex` / `v_flex`
   - `flex_1`, `min_w_0`, `overflow_x_hidden`, `overflow_y_auto`
   - `gap_*`, `px_*`, `py_*`, `rounded_*`, `border_*`, `shadow_*`
2. Replace the manual `Layout` struct in `crates/autopilot_ui` with a pure
   element tree (flex rows/columns) using those helpers.
3. Convert left sidebar to a `VirtualList` or scrollable flex column.
4. Convert status sidebar into a flex column with `gap` sections, no manual
   y-positioning.
5. Convert the thread view to a flex column with a scrollable body and a
   pinned composer.
6. Convert tool cards to flex layouts, eliminate hard-coded widths.
7. Convert command bar to a flex row with fixed-height cells and `justify_between`.
8. Remove all manual `Bounds` math and any render-time `y` offsets.

#### Testability Alignment (from TESTABILITY.md)

The layout rewrite should be testable by design:

- Use WGPUI's test harness (`crates/wgpui/src/testing`) to build UI in a
  deterministic `TestContext`.
- Add "layout snapshot" tests that assert element bounds for key surfaces
  (left list, right status, thread body, composer).
 - Keep AppEvent + UserAction logs for replay so UI regressions can be
  reproduced without manual interaction.
- Ensure services are mocked via `autopilot_app` service traits so UI tests
  remain deterministic and offline.

#### Acceptance Gates

- No manual layout math remains in desktop UI code.
- Resizing or changing DPI never causes overlap (right sidebar stays readable).
- All major panels are flex-based and use Taffy layout each frame.
- UI is renderable in WGPUI test context with stable snapshots.


## Packaging Without Tauri: Shipping the Bun Server

If we drop Tauri packaging but still want the JS/Bun server, we can ship it as
an interim sidecar or replace it outright. Viable paths:

1) Bundle Bun + JS assets as a sidecar (fastest)
   - Ship the `bun` binary and `apps/autopilot-desktop/ai-server/` with the app.
   - On first run, copy to app data dir and spawn `bun run server.ts`.
   - Manage ports, logs, lifecycle (start/stop with app), and codesigning.

2) Compile the server to a standalone binary (Bun build-to-binary flow)
   - Ship a native executable instead of runtime JS assets.
   - Fewer runtime deps, still out-of-process.
   - Requires verifying Bun build support and platform constraints.

3) Replace with Rust service
   - Move server logic into a Rust crate and embed it or ship as a Rust sidecar.
   - Best long-term alignment, more upfront work.

4) Install as a system service
   - Launch via launchd/systemd and connect over localhost.
   - Good for persistence, heavier install/permissions.

Assumption for this plan: default to option (1) during migration, and revisit
option (2) or (3) once the WGPUI app is stable.

## Migration Strategy (Phased)

### Phase 0: Native WGPUI Bootstrap

Deliverable: `cargo run -p autopilot-desktop-wgpu` opens a window and renders
an initial WGPUI root.

Checklist:
- Add new binary crate `apps/autopilot-desktop-wgpu/`.
- Minimal app loop: window + renderer + WGPUI mount.
- Render a simple WGPUI root (placeholder view or borrowed Autopilot UI).

Gate: stable render, window resize, text rendering.

### Phase 1: Extract App Core into Rust Library

Goal: Desktop no longer calls Tauri commands; it calls Rust functions.

Create `crates/autopilot_app` with:
- `App::new(config) -> App`
- `App::open_workspace(path) -> WorkspaceHandle`
- `WorkspaceHandle::start_session(...)`
- `WorkspaceHandle::events() -> impl Stream<Item = AppEvent>`
- `WorkspaceHandle::dispatch(UserAction)`

Gate: CLI and desktop both compile against `autopilot_app` APIs.

### Phase 2: Typed View Model + App Events

Define the canonical app-to-UI contract as **typed Rust state + events**:

- `AppEvent` captures lifecycle + tool execution + UI-relevant changes.
- The desktop UI derives a typed `AppViewModel` directly from events.
- No legacy UI reducer exists; UI state is owned by the view model.

Gate: view model reducer tests cover core workflows (open workspace, start
session, send message, tool calls).

### Phase 2b: Testability Spine (Zed Parity)

Establish the test harnesses and services before layout conversion so the UI
rewrite is safe and replayable:

- Define service traits + `Services::test_defaults()` in `autopilot_app`
  (Clock, IdGen, Fs, Proc, Git, Http, Model, Store, UiSink).
- Add headless scenario runner for `autopilot_app` (UserAction -> AppEvent).
- Implement event recorder + replay loader (JSONL).
- Add view model reducer tests + invariants for contract stability.
- Add WGPUI layout snapshot tests using `crates/wgpui/src/testing` and the
  component registry to assert bounds for key panels.

Gate: contract tests + headless scenarios + layout snapshots run in CI without
network access.

### Phase 3: WGPUI UI Runtime

Adopt the Zed/GPUI component model:

- Implement a view layer based on `Render` / `RenderOnce` with entities as the
  primary state holders.
- Introduce a typed `AppViewModel` (or entity graph) and render directly to
  WGPUI elements each frame (immediate-mode view construction).
- Keep AppEvent + UserAction streams as the recorded output (log + replay), not
  a legacy UI protocol.

Gate: Status dashboard + conversation + tool-call cards working in WGPUI.

### Phase 3b: Zed-Style Layout Conversion

- Introduce Zed-style layout helpers (`h_flex`, `v_flex`, `flex_1`, `min_w_0`,
  `overflow_*`, `gap_*`, `rems`) in WGPUI or `crates/autopilot_ui`.
- Replace all manual bounds math in the desktop UI with flex layout trees.
- Convert left list, center thread, and right status panels to flex-based
  containers with scrollable regions.

Gate: No manual layout math remains in `crates/autopilot_ui`, and resizing/DPI
changes do not cause overlap.

### Phase 4: Core Surfaces (Immediate-Mode Scaffolding)

- Build out the first typed surfaces in WGPUI (status + session list + event log).
- Keep the UI state in the `AppViewModel` entity graph and render directly each frame.
- Use simple layout + panel primitives, without trying to match Effuse parity.

Gate: desktop renders status, session list, and event log from live `autopilot_app` events.

### Phase 4b: Shared UI Crate + Thin Host

- Move shared desktop surfaces into a shared UI crate (`crates/autopilot_ui`).
- Desktop binary becomes a thin host + theming + routing layer.

Gate: desktop builds while consuming shared UI components from `crates/autopilot_ui`.

### Phase 4c: Parity & Consolidation with `crates/autopilot` UI

Priorities:
- Thread view + composer
- Tool call rendering (diff, terminal, file edits)
- Plan/canvas view (start read-only)
- Full-auto controls + run state
- Session list + search

Gate: end-to-end flow (open repo -> start session -> send msg -> see tool calls -> apply diffs).

### Phase 5: Remove Tauri/Effuse

- Delete `apps/autopilot-desktop/` (Tauri + TS/Effuse).
- Delete Effuse runtime + TS UI components.
- Delete Tauri IPC commands used only for UI.
- Update docs + ADR references.

Gate: CI proves no references remain; native binary packaging works.

## Component Mapping (Initial Pass)

Effuse catalog -> WGPUI target
- `canvas`, `node`, `edge` -> new WGPUI Canvas/Node/Edge components
- `stack`, `row`, `panel` -> WGPUI layout containers (`Div`/Element + Taffy)
- `text`, `heading` -> WGPUI `Text` with size/weight styles
- `code_block` -> WGPUI `MarkdownView` or `CodePane`
- `button` -> WGPUI `Button`
- `input`, `textarea` -> WGPUI `TextInput` (single/multiline)
- `select` -> WGPUI `Dropdown` / `ModelSelector`
- `conversation` -> WGPUI `ThreadView` + `ThreadEntry`
- `tool_call` -> WGPUI `ToolCallCard` + tool-specific components
- `diff` -> WGPUI `DiffToolCall` / `CodePane`
- `plan` -> WGPUI `TrajectoryView` or new Plan component

## Risks & Mitigations

- Feature parity gaps (text input, selection, accessibility).
  - Mitigation: prioritize UX-critical components; reuse WGPUI components already built.
- GPUI license or dependency constraints (if Option A is chosen).
  - Mitigation: default to Option B (WGPUI adaptation) unless license is cleared.
- Event schema drift for `AppEvent` / `UserAction`.
  - Mitigation: version events, validate replays, and gate changes via ADR.
- Duplicate UI logic between Autopilot Desktop and `crates/autopilot`.
  - Mitigation: consolidate shared components in `crates/wgpui` and shared state in `crates/autopilot_app`.

## Acceptance Criteria

- WGPUI UI renders all primary Autopilot Desktop flows with functional parity.
- UI updates are driven by typed view model state derived from `AppEvent`s.
- All desktop layout is expressed via flex layout trees (no manual bounds math).
- End-to-end flows verified:
  - Connect workspace, start session, send message, view tool calls/diffs.
  - Adjutant events render correctly in the WGPUI canvas.
- Legacy Effuse UI removed or gated behind a feature flag.
- Testability gates met (from `TESTABILITY.md`):
  - Contract correctness (view model reducer + invariants + replay load).
  - App core determinism (headless scenarios offline).
  - UI mapping regression (replay -> UI runtime without panics).

## Next Steps

1. Create the new native WGPUI desktop binary.
2. Define the backend extraction boundary (`crates/autopilot_app`).
3. Establish typed view model reducers + AppEvent schema tests.
4. Start porting the first WGPUI surface (status + session list).

## Work Log

- 2026-01-27: Rewrote plan for full Rustiness (native WGPUI app, new crate layout, no Tauri), removed time estimates, and defined phased migration + deletion gates.
- 2026-01-27: Phase 0 bootstrap started: added `apps/autopilot-desktop-wgpu` native WGPUI binary with a minimal render loop + text root, and registered it in the workspace.
- 2026-01-27: Verified `cargo build -p autopilot-desktop-wgpu`.
- 2026-01-27: Updated `.cargo/config.toml` so `cargo autopilot` runs the new native WGPUI desktop binary.
- 2026-01-27: Phase 1 started: added `crates/autopilot_app` with core app/event types, workspace/session handles, and broadcast-based event streaming; added a unit test for initial workspace events; registered the crate in the workspace.
- 2026-01-27: Verified `cargo build -p autopilot_app`.
- 2026-01-27: Reviewed Zed GPUI architecture (entities + Render/RenderOnce + contexts) and updated Phase 3 to follow that immediate-mode component model.
- 2026-01-27: Dropped legacy UI protocol compatibility from the migration plan; moved to typed view model + AppEvent/UserAction replay.
- 2026-01-27: Phase 3 started: wired `apps/autopilot-desktop-wgpu` to `crates/autopilot_app`, added an immediate-mode `AppViewModel` + `DesktopRoot` component, and bridged app events into the Winit user-event loop for rendering.
- 2026-01-27: Verified `cargo build -p autopilot-desktop-wgpu` after Phase 3 wiring.
- 2026-01-27: Phase 4 completed (core surfaces): added session list + event log panels driven by the typed `AppViewModel`, with two-column layout and immediate-mode rendering.
- 2026-01-27: Verified `cargo build -p autopilot-desktop-wgpu` after Phase 4 UI scaffolding.
- 2026-01-27: Phase 4b completed: created `crates/autopilot_ui`, moved the desktop root view + view model into it, and updated the WGPUI host to consume shared UI components.
- 2026-01-27: Verified `cargo build -p autopilot-desktop-wgpu` after moving shared UI into `crates/autopilot_ui`.
- 2026-01-27: Phase 4c completed: added ThreadView + MessageEditor conversation panel with tool call cards (read/search/terminal/diff/edit), added plan/trajectory sidebar view, session search bar, and thread controls (mode/model/run state) in `crates/autopilot_ui`, plus scroll routing + input handling via Winit in `apps/autopilot-desktop-wgpu`.
- 2026-01-27: Wired UI send actions to `autopilot_app` via an action channel so Enter/send dispatches `UserAction::Message` back into the app core.
- 2026-01-27: Verified `cargo build -p autopilot-desktop-wgpu` after Phase 4c UI + input wiring.
- 2026-01-27: Reviewed Zed GPUI layout approach (element tree + Taffy, `h_flex`/`v_flex`, `StyledExt`) and expanded the migration plan with a Zed-style layout adoption path, including WGPUI parity helpers and layout conversion gates.
- 2026-01-27: Re-read `apps/autopilot-desktop/docs/migration/TESTABILITY.md` and aligned the plan with testability requirements (service traits, deterministic UI tests, log/replay).
- 2026-01-27: Added a dedicated Phase 2b testability spine (headless scenarios, replay, layout snapshots) and expanded acceptance gates for contract correctness + determinism.
- 2026-01-27: Removed legacy UI protocol references from the plan and testability alignment; moved to typed view model + AppEvent/UserAction replay.
- 2026-01-27: Phase 3b started: replaced manual panel layout math with Taffy-powered flex layout in `crates/autopilot_ui`.
- 2026-01-27: Phase 3b continued: converted session list + status section stacking to Taffy-based layout so rows are computed structurally instead of manual y offsets.
- 2026-01-27: Phase 3b continued: added Zed-style flex helpers (`h_flex`/`v_flex`, flex/overflow conveniences) to WGPUI styled API.
- 2026-01-27: Phase 4 continued: switched desktop host layout + input to logical sizes (scale-factor aware) so UI scales correctly on high-DPI displays.
