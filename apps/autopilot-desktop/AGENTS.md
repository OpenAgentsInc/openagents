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

## Default surface: the zero-base shell (owner directive, 2026-06-19)

**The app launches to a dead-simple shell: a black screen with NOTHING on it
except a single text bar at the bottom** (and the clean conversation above it
once there is a response). This is the `shell` pane (`PaneId`), the default set
by `initial-state.ts` (`pane: "shell"`, NO warm-up commands — the screen stays
quiet and black).

- **All the old UI is KEPT, just hidden.** The full multi-pane UI
  (network/chat/code/supervise/explore/settings + nav + Cmd-K palette) still
  mounts and works; it just does not render by default. It is reachable only via
  an explicit open: Cmd-K (the palette overlays the shell) or the small "open
  panes" affordance (`OpenedPanes` → lands on chat). `ClosedPanes` returns to the
  black shell. Settings specifically is behind that explicit open, never on the
  default screen. The black-screen guard (`tests/black-screen-guard.test.ts`)
  still mounts EVERY pane — keep it green.
- **The text bar is the one surface.** Typing + submitting shows a clean
  conversation (you → answer) with NO session refs / program-step / verdict /
  node-state jargon. The response path is `RespondToShellInput` in `commands.ts`
  (→ `RespondedShell`). **HUD H5 (#5503) wired a REAL model:** the command calls
  the Bun `shellTurn` RPC verb (`src/bun/shell-turn.ts`), which posts to the live
  OpenAgents inference gateway (`POST /v1/chat/completions`, Gemini 3.5 Flash on
  the free per-agent allowance) using the desktop's configured agent token
  (`OPENAGENTS_AGENT_TOKEN`, kept in the Bun host — never crosses to the
  webview). No token / a gateway failure returns an HONEST plain-language
  message, never a fabricated answer. The reducer + view are unchanged; the
  deterministic `shellLoopbackReply` is kept as the offline/test fallback and is
  what the proof injects so parity stays deterministic.
- **Programmatic control + parity.** Drive the shell over the existing RPC path:
  the Bun→webview `shellControl` message (`shared/rpc.ts`, routed in `main.ts`)
  pushes the SAME inbound messages the UI dispatches (`ChangedShellInput` /
  `SubmittedShell`). Read what the owner sees with `shellTranscriptText(model)`
  (pure projection of the rendered conversation). Proof:
  `bun run proof:shell-control`. Tests: `tests/zero-base-shell.test.ts`,
  `tests/shell-turn.test.ts`.
- **Quiet by default.** Native OS notifications are OFF unless
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
