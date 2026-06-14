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
- `electrobun dev` runs the app; `electrobun build` produces the distributable.
