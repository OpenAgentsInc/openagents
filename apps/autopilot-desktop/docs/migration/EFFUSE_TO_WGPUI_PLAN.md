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

Choose one:

Option 3A (fastest parity)
- Implement `UITreeRenderer` mapping UIElement types to WGPUI components.

Option 3B (cleanest long-term)
- Map patches into a typed `AppViewModel` and render directly.
- Keep UITree only as a backend log/debug artifact.

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
