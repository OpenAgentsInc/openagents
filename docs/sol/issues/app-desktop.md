# APP-DESKTOP: greenfield OpenAgents Desktop — Electron + Effect Native

## Outcome

Build a new **OpenAgents Desktop** application from scratch at
`apps/openagents-desktop`. Sarah is the relationship surface and Fleet is the
specialist cockpit for deep coding work. Effect Native owns the application,
component, state, and typed-intent model; Electron is the desktop host.

This is not a rename or in-place rewrite of `clients/khala-code-desktop`. That
Electrobun application is deprecated and frozen as a parity, behavior-contract,
service-extraction, and migration reference until greenfield cutover proof.
Public claim authority is planned `openagents.desktop_app.v1`; the legacy
`khala_code.desktop_codex_wrapper.v1` and
`autopilot.desktop_gui_client.v1` records are withdrawn but remain
dereferenceable history.
The reusable Effect Native Electron host gap is tracked upstream by
OpenAgentsInc/effect-native#69; the earlier Electrobun Phase 4 issues #20/#21
are historical, not destination proof.

## Current status and Terra ownership

Through `f4cb8ed18e`, the greenfield app is beyond the initial ping scaffold:

- Electron isolation/sandbox/navigation/permission boundaries remain proven;
- the default UI is a minimal Effect Native conversation workspace;
- the host owns a bounded five-thread local store and a host-held OpenAgents
  gateway bridge, returning an honest error when configuration is unavailable;
- shared typed icons plus a typed backdrop/glass-material DOM lowering replace
  app-local visual vocabulary;
- a user-selected project root, bounded root listing, and bounded read-only
  file preview begin the local workspace slice; and
- a dedicated Settings screen reads bounded Codex account readiness and starts
  Pylon's isolated device-auth flow through fixed renderer-argument-free IPC,
  never default `~/.codex`; and
- typecheck, 58 tests, and the real Electron smoke pass for the current slice.

Terra is the active execution lane for ready #8574 leaves under
[`../2026-07-10-terra-execution-lane.md`](../2026-07-10-terra-execution-lane.md).
The live issue claim remains the coordination authority. The Settings smoke
uses an explicitly labeled scripted device-auth fixture because headless
Electron cannot finish the browser flow; real account readiness remains an
owner proof gate. Bounded edit/save plus Git status/diff/review can proceed
while that proof waits; terminal follows through a bounded host seam.
Sync-backed conversation authority, server-authoritative Fleet/approval/
receipt projection, packaging identity, signed updates, and legacy-client
retirement remain open and must not be implied by the local baseline.

## Legacy substrate to extract, not inherit

- Codex, Claude, and Grok harness/control capabilities.
- Pylon account status, fleet intents, sessions, approvals, assignments, and
  exact closeout projections.
- Khala Sync state, Monaco, terminal, and local diagnostic host requirements.
- Applicable UX contracts, release-signing knowledge, and update invariants.

No new app code imports the legacy app package, renderer, Electrobun APIs, or
shell state. Reusable platform-neutral contracts/services move into shared
packages with independent tests first.

## Required starting template

Scaffold the app from the MIT-licensed
[`LuanRoger/electron-shadcn`](https://github.com/LuanRoger/electron-shadcn)
template. The reviewed local mirror is `~/work/projects/repos/electron-shadcn`;
the reviewed 2026-07-09 baseline is upstream commit
`a02e7bbfe0c196db22b76f40ec23b5c265d24215`. Record the actual imported commit
and attribution in the new app and in an `UPSTREAM.md` or `NOTICE` manifest.

Use its Electron Forge + Vite main/preload/renderer layout, fuse hardening,
packaging structure, Vitest, and Playwright scaffolding as the bootstrap. It is
a template, not the application architecture: Effect Native replaces its
starter renderer/component tree, Effect Schema replaces Zod at IPC boundaries,
and Effect services replace starter process/state logic. Remove unused shadcn,
TanStack Router/Query, oRPC, updater, and demo dependencies rather than carrying
parallel architectures. Before the first launch or package, remove the
template's `updateElectronApp` call and Forge publisher target for
`LuanRoger/electron-shadcn`, ensure React DevTools cannot install in production,
remove the template `package-lock.json`, and integrate the scaffold into the Bun
workspace/lockfile.

The template is not safe unchanged for this product. The reviewed baseline has
`contextIsolation: true` but also `nodeIntegration: true` and no explicit
renderer sandbox. The first scaffold commit must set `nodeIntegration: false`,
set `sandbox: true`, expose only a minimal `contextBridge` preload API, remove
the generic MessagePort/oRPC starter bridge, and prove those settings with tests
before any provider/account capability is added.

## Scope

1. Scaffold `@openagentsinc/openagents-desktop` at
   `apps/openagents-desktop` from the pinned electron-shadcn template, with an
   independent product identity. Before the first packaged build, freeze the
   exact macOS bundle ID/product name/executable, Windows AppUserModelId and
   installer identity, Linux desktop/app ID, deep-link scheme, Electron
   `userData` path/session partition, update product/feed/channel, GitHub tag
   namespace, and OAuth redirect/client ownership in `NEEDS_OWNER.md`. Do not
   reuse `com.openagents.khala.code.desktop`, `khala-code://`, `.khala-code`,
   `desktop/khala-code-desktop`, or `khala-code-desktop-v*`.
2. Make Sarah conversation and active Blueprint/FleetRun state first-class,
   using the same work-unit, account, approval, control, and receipt projections
   as `/sarah` and mobile.
3. Build the Fleet pane as a typed specialist projection, not a second
   orchestration authority. Keep Monaco, terminal, and raw local diagnostics
   behind typed Effect Native foreign-host nodes.
4. Enforce a narrow Electron boundary: `contextIsolation: true`,
   `nodeIntegration: false`, `sandbox: true`, `webviewTag: false`,
   `webSecurity: true`, restrictive CSP, deny-by-default permission/navigation/
   window-open handlers, allowlisted external protocols, IPC sender/frame-origin
   validation, verified packaged fuses, and no raw `ipcRenderer`, MessagePort,
   Node/Electron built-ins, filesystem/process authority, provider credentials,
   or raw private events in the renderer.
5. Add a mechanical Effect Native boundary: depend on the Electron host tracked
   by OpenAgentsInc/effect-native#69 (or record its typed interim gap), boot the
   product renderer through Effect Native, and fail a source/dependency oracle
   if shadcn/Zod/oRPC/TanStack starter application semantics or direct product UI
   return outside explicitly approved host/renderer adapters.
6. Consume typed Pylon engine/control services and remove stdout parsing or
   duplicate orchestration state only after replacement proof.
7. Establish a new signed/notarized Electron build, clean-machine first-run
   smoke, rollback, and updates-feed path. Electrobun release tooling and
   `desktop/khala-code-desktop` feeds are historical inputs, not a destination.
8. Port applicable behavior contracts into the new app registry and prove
   cross-device Sarah/Fleet continuation before retiring the legacy app.
9. Maintain a capability-disposition ledger: every Khala Code idea is marked
   `fold into Sarah`, `retain as an OpenAgents specialist capability`, or
   `extract as an engine consumed by the Sarah-first apps`. Only superseded
   legacy implementations may be retired after their successor disposition is
   explicit and proven; there is no silent idea loss and no surviving Khala
   Code product surface.
10. After extraction, parity, and release receipts exist, remove the deprecated
   Electrobun package from active workspace, install, release, and update paths.

## Non-goals

- Desktop is not a second product home or authority plane.
- Do not convert or rename the Electrobun app.
- Do not block P0 Sarah Fleet Command on the full desktop build.
- Do not retire useful CLI/TUI diagnostics until their replacement is proven.

## Exit

A Sarah-started FleetRun opens in the greenfield Electron app with matching
state and controls; a desktop-started run is accurately summarized by Sarah.
The retained UI mechanically boots through Effect Native with no surviving
starter application architecture. The frozen cross-platform identity, secure
Electron boundary, verified packaged fuses, and signed/notarized independent
update lane are proven, and the legacy Khala Code Electrobun app is unable to
release and is no longer an installable product surface.
