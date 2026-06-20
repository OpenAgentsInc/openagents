# AGENTS — Autopilot Desktop (Electrobun)

The desktop GUI companion to the Pylon TUI: an Electrobun + Bun app whose
webview renders a projection of the local Pylon node and dispatches scoped
control actions over loopback. Same-machine sibling of the mobile client; shares
the `@openagentsinc/autopilot-control-protocol` contract.

## UI: Foldkit over everything — NO hand-DOM (owner mandate, 2026-06-13)

**The webview UI MUST be built with Foldkit + the shared
`@openagentsinc/autopilot-ui` components.** This is the same stack as
`apps/openagents.com/apps/web` (Foldkit TEA: `Model`/`Message`/`update`/`view`,
`foldkit/html`, Effect, Tailwind tokens). The point is one component library and
one look-and-feel across web + desktop.

- **Do NOT hand-roll DOM** (`document.createElement`, `innerHTML`,
  `addEventListener` wiring) in the webview. Render through `foldkit/html` `h.*`
  and the shared `autopilot-ui` components (`SessionList`, `DecisionCard`,
  `EventTimeline`, node-status / accounts / verify / artifacts / decision-actions
  / steer-controls / assignments / cloud-quota, plus the dark `tokens`).
- If a needed component doesn't exist in `@openagentsinc/autopilot-ui`, **add it
  there** (so web reuses it too) rather than building a one-off in the desktop.
- State changes flow through the Foldkit `update`; side effects (RPC to the Bun
  main process) are Foldkit `Command`s; inbound node-state/notification messages
  from Bun arrive as `Message`s via a subscription.
- The **Bun main process** (`src/bun/`) stays plain TS: it owns the control
  token, loopback control client, node-home discovery, polling, and the typed
  RPC bridge. Foldkit is the **webview** layer only.

Reference: `docs/autopilot-coder/2026-06-13-autopilot-desktop-app-audit.md` and
the web app under `apps/openagents.com/apps/web` (its `entry.ts` / `view.ts` show
the `Runtime.makeProgram` + `view()` pattern).

## Default surface: the Verse (owner directive, 2026-06-20)

**The app launches to the Verse: the chat/world surface centered on Pylons,
Tassadar, and one chat bar.** This is the `chat` pane (`PaneId`), with
`model.verseEnabled` default-on and the launch build resolving the Verse bundle
on unless an explicit fallback/debug kill switch is set. Fresh first paint must
not show the shell target tabs, `Claude Code`, or `Codex`.

- **The fallback shell is KEPT, just demoted.** The `shell` pane, its
  programmatic-control path, and its tests remain available for advanced/debug
  work. It is reachable through the explicit fallback control, Escape from the
  full UI, and command-palette navigation. Do not make it the real app entry
  again without a new owner directive.
- **All advanced panes are KEPT and reachable.** The full multi-pane UI
  (network/chat/code/supervise/explore/settings + nav + Cmd-K palette) still
  mounts and works, but the Verse first paint does not show the sidebar or
  status HUD. Code, Supervise, Composer, Sessions, Swarm, repo/worktree, deploy,
  Claude, and Codex affordances belong behind Cmd-K/Advanced and then the normal
  advanced nav, not first paint. The black-screen guard still mounts EVERY pane
  — keep it green.
- **HUD H3 (#5501) — the managed pane layer.** Panes are available as a MANAGED
  layer of draggable + resizable floating windows over the base surface, the
  durable OpenAgents pane-as-data pattern (audit
  `docs/launch/2026-06-19-previous-hud-systems-audit.md` §4.1/§4.6). A pane is
  plain data (`{id, kind, rect, z}`) in `src/ui/pane-manager.ts`; the pure
  `reducePaneLayer` is the only mutator. Cascade + clamp keep panes from
  stacking exactly or being dragged off-screen. Parity: `paneLayerText(layer)`.
  Tests: `tests/pane-layer.test.ts`.
- **The chat bar is the first input.** The launch copy should say
  Verse/Pylon/Tassadar. The existing coding-session chat path is still present
  until the Tassadar default chat path lands, but the first visible surface must
  not invite the user to shell out to Codex/Claude/cloud-code.
- **Programmatic shell control + parity stay intact.** Drive the fallback shell
  over the existing RPC path: Bun→webview `shellControl` (`shared/rpc.ts`,
  routed in `main.ts`) pushes the same inbound messages the UI dispatches
  (`ChangedShellInput` / `SubmittedShell`). Read what the owner sees with
  `shellTranscriptText(model)`. Proof: `bun run proof:shell-control`. Tests:
  `tests/zero-base-shell.test.ts`, `tests/shell-turn.test.ts`.
- **Quiet notifications.** Native OS notifications are OFF unless
  `OA_DESKTOP_OS_NOTIFICATIONS=1` (see `src/bun/index.ts`). The in-app
  notification center still accumulates as a passive projection.

## Visualizations: `three-effect` (Three.js) first (owner mandate, 2026-06-14)

**When you build UI, reach for `@openagentsinc/three-effect` first, and do as
much as you can with Three.js — rich, cool visualizations over plain DOM/HTML
widgets.** Foldkit (above) still owns app structure/TEA and the shared
`autopilot-ui` components; `three-effect` owns the *visual/3D* layer (scenes,
canvases, particles, text-in-scene), and it has Foldkit bindings
(`@openagentsinc/three-effect/foldkit`) so the two compose.

- **Look at the `three-effect` primitives before hand-rolling anything visual.**
  It already ships text primitives (`createTextGeometry`), `htmlOverlayPrimitives`,
  geometry/shader/material/instance/particle/motion/camera/controls primitives,
  and a ready `trainingRun.ts` scene module (sibling clone:
  `/Users/christopherdavid/work/three-effect`, package `packages/core/src` +
  `packages/foldkit/src`; examples under `examples/training-run`,
  `examples/moksha`, `examples/bezier-nodes`).
- **Crisp in-scene text may need porting.** `createTextGeometry` is extruded
  font geometry, not high-quality screen-space/SDF text. For good 3D text, port
  the text helper from drei (`@react-three/drei`'s `<Text>`, backed by
  `troika-three-text`) into `three-effect` as a reusable primitive (an Effect/
  Foldkit-bound wrapper over `troika-three-text`), rather than wiring troika
  ad hoc in the desktop. Keep it in `three-effect` so web reuses it.
- The Training pane (`oa-training-run`) is the canonical place to lean into this.
- If a needed visual primitive is missing, **add it to `three-effect`** (so web
  reuses it too) rather than building a one-off here — same rule as `autopilot-ui`.
  Per the workspace contract, extend `three-effect` first instead of rebuilding
  parallel Three primitives.
- Proof replay is included in this rule. The desktop app may fetch and gate
  `@openagentsinc/proof-replay` bundles and may render Foldkit controls,
  source inspectors, lists, and accessibility mirrors, but replay stages,
  avatars, payment zaps, camera grammar, particles, and world motion must come
  from `@openagentsinc/three-effect` and the visual taxonomy exercised in the
  `openagents.com` `/animations` route. Do not add another app-local DOM or
  canvas proof replay renderer in desktop; add the primitive to the sibling
  `three-effect` repo first and then consume it from web and desktop.

## Boundaries

- Node/runtime authority lives in Pylon (`apps/pylon`); the desktop renders and
  relays, it does not reimplement control logic.
- Secrets (control token, bridge/cloud credentials) stay in the Bun main
  process; the webview only ever receives public-safe projections.
- Honor the control server's loopback bind and danger-mode refusals.

## Build / test

- `bun test` (in `apps/autopilot-desktop`) is the gate for the Bun-side logic and
  any pure view-model helpers. The repo's desktop `tsc` config carries
  pre-existing nodenext type-resolution noise against the protocol package; rely
  on `bun test` + a `bun build` bundle check, not a clean `tsc`.
- For Training pane changes, run `bun run verify:training` in
  `apps/autopilot-desktop` (or `bun run verify:autopilot-desktop:training` from
  the repo root). It runs the focused Foldkit tests, CSS/build bundle checks,
  and the Chrome-backed `oa-training-run` canvas-pixel smoke. Set `CHROME_PATH`
  if Chrome, Chromium, or Edge is not installed in a common path.
- `electrobun dev` runs the app; `electrobun build` produces the distributable.
