# OpenAgents Desktop

Greenfield OpenAgents Desktop application (#8574, epic #8566): **Effect
Native owns the application, component, state, and typed-intent model;
Electron is the desktop host.** This is not a rename of
`clients/khala-code-desktop` — that Electrobun app is frozen legacy
reference material and nothing here imports it.

This package is the issue's *initial greenfield setup* exit: the smallest
real, testable desktop app — a hardened Electron shell whose renderer is
100% Effect Native (the shared vendored catalog at
`apps/openagents.com/packages/effect-native-*`, catalog v26), with one
typed intent loop proven end-to-end inside the real Electron renderer.

## Run it

From the monorepo root:

```bash
bun install
bun run dev:openagents-desktop   # builds dist/ and launches Electron
```

Or from this directory: `bun run dev`.

What you should see: a Protoss-blue OpenAgents shell — titlebar row
(OpenAgents · DESKTOP · READY · host badge · loop-proof badge), a
Transcript of typed system notes, and a composer. Pressing **Ping loop**
(or adding a note) drives the full typed loop: DOM event → `IntentRef` →
intent registry (Effect Schema decode) → handler → `SubscriptionRef` state
→ `viewStream` re-render.

## Verify it

```bash
bun test apps/openagents-desktop   # from repo root; or `bun run test` here
bun run smoke                      # launches Electron, clicks the EN-rendered
                                   # Ping button, asserts the re-render, exits 0/1
bun run typecheck
```

Tests cover: pure `state -> View` component trees, pure transitions, the
intent loop through the real registry, theme parity with the Sarah surface,
the mechanical Electron/EN boundary oracle, and a real bundle build.

## Architecture

- `src/main.ts` — Electron main process (plain TS). Hardened per #8574:
  `contextIsolation: true`, `nodeIntegration: false`, `sandbox: true`,
  `webviewTag: false`, deny-by-default permissions/navigation/window-open,
  restrictive CSP, no updater/publisher/devtools-installer.
- `src/preload.cts` — the only bridge: a frozen static identity object via
  `contextBridge`. No ipcRenderer, no MessagePort, no Node authority. The
  renderer decodes it with Effect Schema.
- `src/renderer/` — the application, 100% Effect Native:
  - `shell.ts` — typed state, `defineIntent` intents, pure transitions,
    pure `state -> View` over the shared catalog.
  - `theme.ts` — the one Protoss-blue theme via `@effect-native/tokens`,
    token-identical to `apps/sarah/src/ui/theme.ts`.
  - `boot.ts` — `SubscriptionRef` + `makeViewProgramFromState` +
    `makeIntentRegistry` + `makeDomRenderer().mount(...)`, the same
    consumer pattern as the Sarah surface (`apps/sarah/src/ui/main.ts`).
- `scripts/build.ts` — Bun bundles main (ESM), preload (CJS, sandboxed),
  and renderer into `dist/`.

**One catalog, many hosts.** The transcript-message and composer
compositions are deliberately structured identically to the Sarah EN web
surface, and `src/renderer/shell.test.ts` asserts the shared shape. New
component needs go to `docs/effect-native/DEMAND_REGISTER.md` (row
D-DESK-01 tracks the reusable Electron host, upstream
OpenAgentsInc/effect-native#69) — never app-local primitives.

## What this exit is NOT yet

Honest residue, tracked on #8574:

- No Sarah conversation, FleetRun state, Pylon services, or Khala Sync yet
  (scopes 2, 3, 6, 8).
- No Forge packaging, fuses verification, signing/notarization, or updates
  feed (scope 7) — blocked on the owner identity freeze (scope 1); the
  interim dev identity uses an `OpenAgentsDesktopDev` userData dir and no
  deep-link scheme.
- The Electron host adapter is app-local boot code until the reusable
  `@effect-native` Electron host lands upstream (effect-native#69).

Template attribution and the adopted/removed/deferred ledger:
[`UPSTREAM.md`](./UPSTREAM.md).
