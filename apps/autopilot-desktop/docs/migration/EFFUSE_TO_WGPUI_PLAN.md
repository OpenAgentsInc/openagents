# Effuse -> WGPUI Migration Plan (Autopilot Desktop)

## Summary

Autopilot Desktop currently ships a Tauri + TypeScript/Effuse UI with a
signature-driven UITree runtime and a large catalog of HTML/CSS components.
WGPUI is already the GPU UI stack for `crates/autopilot` and includes a mature
component system plus a full desktop and wasm platform layer. This plan
migrates Autopilot Desktop from Effuse to WGPUI as a **native Rust app** while
preserving the existing UI contract unless explicitly superseded.

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
  - Signature-driven UITree runtime in `apps/autopilot-desktop/src/effuse/ui/`
- UITree + UiPatch contract is defined and enforced (ADR-0022).
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
- `crates/autopilot_ui_contract/`
  Canonical UI contract types: `UITree`, `UiPatch`, validation, patch apply,
  action envelope.
- `crates/autopilot_ui_runtime/` (optional)
  If we keep UITree as the UI intermediate, this is the reducer + registry
  glue that turns UITree into WGPUI components.
- `crates/autopilot_desktop_shell/`
  Desktop-only: window management, menus, file dialogs, OS integration.

### What Gets Removed (end-state)

- `apps/autopilot-desktop/` (Tauri + TS/Effuse) -> deleted
- `apps/autopilot-desktop/src/effuse/**` -> deleted
- `apps/autopilot-desktop/src/components/**` -> deleted
- Tauri IPC layer -> deleted (or temporary compatibility bridge only)

## Contract Decision

Preserve UITree + UiPatch as the backend-to-UI contract **unless superseded**.
For full Rustiness:

- Move canonical UITree types + patching logic into `crates/autopilot_ui_contract`.
- Treat the contract as a Rust API (not IPC) for native desktop.
- If a new contract is needed, write a superseding ADR and keep a compatibility
  adapter during deprecation.

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
- Event dispatch uses actions and context, not a serialized UITree.
- UITree/UiPatch remains as a **log + replay** artifact, not the primary
  in-memory view state, unless we later decide otherwise via ADR.

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

### Phase 2: Canonicalize UITree + Patching in Rust

Move or implement in `crates/autopilot_ui_contract`:
- `UITree` / `UIElement`
- `UiPatch` / patch apply
- Catalog validation (if still needed)
- Action envelopes: `UIAction`, `ActionResult` (confirm/success/error)

Then `autopilot_app` emits:
- `AppEvent::UiPatch(UiPatch)`
- `AppEvent::UiTreeSnapshot(UITree)` (optional for resync)

Gate: reducer unit tests apply patch sequences -> expected UITree.

### Phase 3: WGPUI UI Runtime

Adopt the Zed/GPUI component model:

- Implement a view layer based on `Render` / `RenderOnce` with entities as the
  primary state holders.
- Introduce a typed `AppViewModel` (or entity graph) and render directly to
  WGPUI elements each frame (immediate-mode view construction).
- Keep UITree/UiPatch as **recorded output** (log + replay), not the primary
  rendering input. If we still need external UI automation, add a reducer that
  emits UITree snapshots from the typed view model.

Gate: Status dashboard + conversation + tool-call cards working in WGPUI.

### Phase 4: Parity & Consolidation with `crates/autopilot` UI

- Move shared desktop surfaces into `crates/wgpui` or a shared UI crate.
- Desktop binary becomes a thin host + theming + routing layer.

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
- Contract drift from ADR-0022 (UITree + UiPatch).
  - Mitigation: keep contract stable; if change required, add ADR + compatibility layer.
- Duplicate UI logic between Autopilot Desktop and `crates/autopilot`.
  - Mitigation: consolidate shared components in `crates/wgpui` and shared state in `crates/autopilot_app`.

## Acceptance Criteria

- WGPUI UI renders all primary Autopilot Desktop flows with functional parity.
- UI updates continue to be driven by `UiPatch` (or a superseding contract with ADR).
- End-to-end flows verified:
  - Connect workspace, start session, send message, view tool calls/diffs.
  - Adjutant UI patches render correctly in the WGPUI canvas.
- Legacy Effuse UI removed or gated behind a feature flag.

## Next Steps

1. Create the new native WGPUI desktop binary.
2. Define the backend extraction boundary (`crates/autopilot_app`).
3. Establish UITree contract crate (`crates/autopilot_ui_contract`).
4. Start porting the first WGPUI surface (status + session list).

## Work Log

- 2026-01-27: Rewrote plan for full Rustiness (native WGPUI app, new crate layout, no Tauri), removed time estimates, and defined phased migration + deletion gates.
- 2026-01-27: Phase 0 bootstrap started: added `apps/autopilot-desktop-wgpu` native WGPUI binary with a minimal render loop + text root, and registered it in the workspace.
- 2026-01-27: Verified `cargo build -p autopilot-desktop-wgpu`.
- 2026-01-27: Updated `.cargo/config.toml` so `cargo autopilot` runs the new native WGPUI desktop binary.
- 2026-01-27: Phase 1 started: added `crates/autopilot_app` with core app/event types, workspace/session handles, and broadcast-based event streaming; added a unit test for initial workspace events; registered the crate in the workspace.
- 2026-01-27: Verified `cargo build -p autopilot_app`.
- 2026-01-27: Phase 2 started: added `crates/autopilot_ui_contract` with UITree/UiElement/UiPatch types, dynamic value + visibility expressions, JSON patch parsing, and patch apply helpers plus unit tests; registered the crate in the workspace.
- 2026-01-27: Verified `cargo build -p autopilot_ui_contract`.
- 2026-01-27: Reviewed Zed GPUI architecture (entities + Render/RenderOnce + contexts) and updated Phase 3 to follow that immediate-mode component model; clarified UITree/UiPatch as log + replay artifacts.
- 2026-01-27: Phase 3 started: wired `apps/autopilot-desktop-wgpu` to `crates/autopilot_app`, added an immediate-mode `AppViewModel` + `DesktopRoot` component, and bridged app events into the Winit user-event loop for rendering.
- 2026-01-27: Verified `cargo build -p autopilot-desktop-wgpu` after Phase 3 wiring.
