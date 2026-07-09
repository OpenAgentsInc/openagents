# MH-7 / EN-5 Effect Native Fleet Cockpit Receipt

Issue: OpenAgentsInc/openagents#8586 (MH-7, rides EN-5 #8574)

Surface: a new dev-only page `en-cockpit.html` in the Khala Code desktop
webview (`clients/khala-code-desktop`). Reachable via `vite dev`
(`bun run dev:hmr` → `http://localhost:5173/en-cockpit.html`). It is NOT in the
packaged Electrobun build (`build:ui` emits only `index.html`).

## Scope

- One real fleet cockpit screen rendered through the REAL Effect Native DOM
  renderer, mirroring EN-1's `/stage1` discipline: prove the render + typed
  intent path before flipping anything, coexisting with the shipping shell and
  replacing no existing screen.
- The cockpit renders: an account/capacity chip strip, per-harness readiness
  rows, a worker/run list, pause/resume/drain/stop run controls,
  worker-selection pills, and approval allow/deny controls.
- It binds to the EXISTING live desktop fleet data shape
  (`KhalaCodeDesktopFleetStatus`, produced by `fleet-run-supervisor.ts` /
  `pylon-service.ts` and surfaced over RPC) through a pure read-only adapter
  (`cockpit-projection.ts`). No new backend logic was added — this is a
  rendering proof.
- Every control dispatches a typed Effect Native `IntentRef` that the mount
  layer converts into a shared `@openagentsinc/khala-fleet-intents`
  `KhalaFleetIntent` value (the ONE vocabulary that is both the UI intent and
  the Khala Sync mutator), validated with `decodeKhalaFleetIntent`.

## Vendored Effect Native packages

The desktop client now depends on the SAME app-local Effect Native snapshot
EN-1 vendored (`@effect-native/{core,render-dom,tokens}`, workspace members
under `apps/openagents.com/packages/`, copied from OpenAgentsInc/effect-native
commit `6dda1d443321d815eff342058b8f53c26615b721`). No new snapshot was copied —
the desktop reuses EN-1's exact vendored packages as `workspace:*` deps.

## Mount decision: DOM renderer, not `platform-desktop`

Mounted via the Effect Native DOM renderer (`@effect-native/render-dom`), the
same choice `/stage1` made — NOT `@effect-native/platform-desktop`'s
`runMainDesktop` / `DesktopBridge`. Two honest reasons:

1. `platform-desktop` is not part of the effect-native snapshot EN-1 vendored
   into this monorepo (only core / tokens / render-dom / render-rn are), so
   using it would require re-vendoring a newer snapshot.
2. `DesktopBridge` abstracts native menu / window / deep-link / single-instance
   concerns over its own request/event schema; Electrobun already owns those via
   `Electroview`, and the cockpit needs none of them — it needs a DOM container
   and a typed intent sink. The Electrobun webview IS a live DOM host, so the DOM
   renderer is the natural, clean fit; forcing the cockpit through
   `DesktopBridge` would be a bad fit.

## Catalog pieces used vs skipped

- Used: the base EN primitive set present in the vendored snapshot — `Stack`,
  `Text`, `Card`, `Button`, `List`, `Spacer`, plus the typed intent algebra
  (`defineIntent`, `IntentRef`, `makeIntentRegistry`, `makeViewProgramFromState`,
  `makeDomRenderer`).
- Skipped (not in the vendored snapshot NOR the local effect-native checkout at
  the time of this work — both still catalog `effect-native/v5` with the 12 base
  component tags): `Table`, `Chip`, `Badge`, `Meter`, `StatTile`, `Tabs`,
  `SplitPane`, `NavRail`, command palette, toast/status-banner, and
  `GraphFigure` (#37, needs `render-canvas`). The chip strip, harness rows, and
  run/approval lists are therefore composed from `Card` + `Text` + `Button`
  exactly the way `/stage1` composed its stat/plan cards. When the Phase 4
  data-display + chip catalog lands in a re-vendored snapshot, these composites
  should be swapped for the first-class components. The fleet graph figure is
  intentionally omitted (no `render-canvas` in the snapshot).

## Theme

The Protoss-blue `khalaTheme` is not in the vendored (or local) tokens snapshot,
so — exactly like `/stage1`'s route-local theme — a local Protoss-blue theme is
defined via `defineTheme` (`src/ui/en-cockpit/theme.ts`). Replace with the shared
`khalaTheme` when it lands in a re-vendored snapshot.

## Verification

- `bun test clients/khala-code-desktop/tests/en-cockpit.test.ts` — 6 tests:
  projection mapping, typed EN view tree (catalog v5), real DOM render through
  `@effect-native/render-dom`, pause dispatch → `fleet_run_control` intent,
  approval-allow dispatch → `approval_decision` intent, and the deterministic
  converter shape.
- Full desktop suite `bun test tests/*.test.ts` — 992 pass / 0 fail.
- `bun run build:ui` — green; `en-cockpit.html` is correctly excluded from
  `dist/` (dev-only, not shipped).
- Architecture scan (`scan:architecture`) — this change adds ZERO new findings
  (the dev-entry boot uses `Effect.runFork` of a scoped program, keeping the
  mounted surface alive for the page lifetime rather than a scanned
  `Effect.runPromise`).

## Pre-existing breakage flagged (NOT caused by this change, separate lane)

`clients/khala-code-desktop` typecheck and the architecture scan are already red
on `origin/main` from the grok-harness worker-kind lane, in files this change
does not touch:

- typecheck (5, all grok worker-kind widening `"codex"|"claude"|"grok"` not
  assignable to `"codex"|"claude"`): `src/bun/fleet-run-supervisor-rpc-adapter.ts`
  (unused `narrowToDelegateWorkerKind`, env exactOptional),
  `src/bun/khala-fleet-tools.ts` (2), `packages/khala-tools/src/fleet-delegate-program.ts`.
- scan (2 `date-now-in-logic` drifts): `scripts/khala-code-tui.ts:325`,
  `src/bun/khala-fleet-tools.ts:4622`.

These belong to the grok-harness / MH-8 lane and were left intact per multi-agent
git hygiene (do not edit another lane's active surfaces). All cockpit files
(`src/ui/en-cockpit/**`, `tests/en-cockpit.test.ts`) typecheck clean and add no
scan findings.

## Follow-ups

- Re-vendor a newer effect-native snapshot once the Phase 4 data-display / chip /
  tabs catalog + shared `khalaTheme` land, then swap the `Card`-composed chip
  strip / rows for first-class components and add the `GraphFigure` fleet graph.
- Wire the cockpit's `onFleetIntent` sink into the real fleet supervisor RPC /
  Khala Sync mutator path (this proof records + logs the typed intent only).
- EN-5 #8574 full desktop shell conversion remains its own larger scope; MH-7
  #8586 is scoped to this cockpit screen.
