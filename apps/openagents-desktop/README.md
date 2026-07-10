# OpenAgents Desktop

Greenfield OpenAgents Desktop application (#8574, epic #8566): **Effect
Native owns the application, component, state, and typed-intent model;
Electron is the desktop host.** This is not a rename of
`clients/khala-code-desktop` — that Electrobun app is frozen legacy
reference material and nothing here imports it.

Current public, test-backed promises are summarized in
[`GUARANTEES.md`](./GUARANTEES.md). Agents should use that document rather than
infer guarantees from roadmap material or screenshots.

The binding target process/data/authority design is
[`docs/sol/2026-07-10-openagents-desktop-product-architecture.md`](../../docs/sol/2026-07-10-openagents-desktop-product-architecture.md).
It keeps the signed Effect Native renderer tokenless, places OpenAgents
identity/Khala Sync/Pylon/workspace authority behind one host-owned Runtime
Gateway, and requires the first real streamed Desktop conversation to continue
on mobile before broad workbench parity. That target is roadmap intent; only
`GUARANTEES.md` and its oracles describe behavior enforced today.

This package now includes a neutral desktop chat workspace: a hardened
Electron app whose renderer is 100% Effect Native (the shared vendored catalog
at `apps/openagents.com/packages/effect-native-*`). It projects recent local
Codex chats read-only, renders assistant and owner transcript roles, clears the
composer after a submitted turn, provides New Chat and a closed command
palette, supports a user-selected workspace with bounded read/edit/save plus
typed read-only Git status/diff, and opens an explicit Fleet deployment brief
without pretending that local UI has authority to create a FleetRun.

## Run it

From the monorepo root:

```bash
bun install
bun run dev:openagents-desktop   # builds dist/ and launches Electron
```

Or from this directory: `bun run dev`.

What you should see: a neutral chat workspace with a chat rail, an owner
composer, and **Open Fleet** in the titlebar. A submitted message renders the
owner turn plus a typed assistant response and clears the composer. **New
chat** resets the local conversation. **Open Fleet** exposes a local deployment
brief; **Dispatch to Pylon** sends only the bounded objective through a
schema-checked, host-owned loopback control capability. The Pylon control token
never enters the renderer. An accepted intent is not a FleetRun receipt:
repository pins, verifier, named account, and authority-backed closeout remain
the Pylon contract.

## Verify it

```bash
bun test apps/openagents-desktop   # from repo root; or `bun run test` here
bun run smoke                      # launches Electron, opens the Fleet deck,
                                   # submits a chat turn, verifies both
                                   # roles + clear-on-submit, exits 0/1
bun run typecheck
```

Tests cover: pure `state -> View` component trees, pure transitions, the
intent loop through the real registry, theme parity with the shared surface,
the mechanical Electron/EN boundary oracle, and a real bundle build.

## Architecture

- `src/main.ts` — Electron main process (plain TS). Hardened per #8574:
  `contextIsolation: true`, `nodeIntegration: false`, `sandbox: true`,
  `webviewTag: false`, deny-by-default permissions/navigation/window-open,
  restrictive CSP, no updater/publisher/devtools-installer.
- `src/preload.cts` — the only bridge: a closed set of schema-checked chat,
  workspace, bounded save/Git, Fleet-brief, and public-safe Codex-account
  capabilities over fixed IPC channels. No raw token, Node capability,
  arbitrary command/channel, generic event subscription, or `MessagePort`
  reaches the renderer.
- `src/fleet-control.ts` — main-process adapter for the existing local Pylon
  `intent.submit` command. It resolves the loopback control token locally and
  returns only `accepted | rejected | unavailable` status.
- `src/renderer/` — the application, 100% Effect Native:
  - `shell.ts` — typed state, `defineIntent` intents, pure transitions,
    pure `state -> View` over the shared catalog.
  - `theme.ts` — the one Protoss-blue theme via `@effect-native/tokens`,
    token-identical to the shared OpenAgents theme values.
  - `boot.ts` — `SubscriptionRef` + `makeViewProgramFromState` +
    `makeIntentRegistry` + `makeDomRenderer().mount(...)`, the same
    consumer pattern shared by the OpenAgents Effect Native surfaces.
- `scripts/build.ts` — Bun bundles main (ESM), preload (CJS, sandboxed),
  and renderer into `dist/`.

Target evolution preserves this boundary rather than widening the preload one
feature at a time. The renderer consumes one closed schema-decoded projection/
intent/event surface; a host-owned Runtime Gateway composes existing Khala
Sync, Pylon, workspace, and execution services. Lightweight R1/R2/D1 adapters
may start in main for delivery speed, while filesystem watch, PTY, engine
supervision, extension, and other heavy services move behind one utility
process before D3/D4 breadth. The renderer never receives bearer/provider/
Pylon credentials, a loopback URL, raw runtime events, general IPC, or a raw
`MessagePort`.

The first Runtime Gateway slice is now enforced: a versioned closed bootstrap,
command-outcome, and lifecycle-event protocol crosses preload; main validates
the top-level bundled renderer; the Electron smoke exercises a truthful
bootstrap. Khala Sync and durable conversation streaming remain explicitly
`unavailable` until their later leaves land.

Desktop main now also opens the shared `khala-sync-client` SQLite store beneath
its private `userData` root, persists one installation identity, and closes the
store on quit. The gateway reports that local persistence is ready while
keeping network Sync unavailable until native OpenAgents sign-in lands. No
database path, handle, identity ref, row, queue, or credential crosses preload.

**One catalog, many hosts.** The transcript-message and composer
compositions are deliberately structured around the shared Effect Native chat
contract, and `src/renderer/shell.test.ts` asserts the typed shape. New
component needs go to `docs/effect-native/DEMAND_REGISTER.md` (row
D-DESK-01 tracks the reusable Electron host, upstream
OpenAgentsInc/effect-native#69) — never app-local primitives.

## What this exit is NOT yet

Honest residue, tracked on #8574:

- The local assistant response and Pylon brief dispatch do not yet create or
  project a server-authoritative `coding_fleet_start` FleetRun. That bridge,
  live FleetRun projection/controls, and Khala Sync remain (scopes 2, 3, 6, 8).
- No Forge packaging, fuses verification, signing/notarization, or updates
  feed (scope 7) — blocked on the owner identity freeze (scope 1); the
  interim dev identity uses an `OpenAgentsDesktopDev` userData dir and no
  deep-link scheme.
- The Electron host adapter is app-local boot code until the reusable
  `@effect-native` Electron host lands upstream (effect-native#69).

Template attribution and the adopted/removed/deferred ledger:
[`UPSTREAM.md`](./UPSTREAM.md).
