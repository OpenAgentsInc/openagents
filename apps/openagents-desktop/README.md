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

Runtime Gateway protocol v6 now carries the authoritative conversation path:
confirmed catalog/thread/current-timeline queries, canonical create/append
enqueues, and exact thread/message/run start or interrupt commands. Enqueue is
reported only as `pending_reconcile` or `unknown_pending_reconcile`. The
Effect Native shell streams bounded canonical text, reasoning, tool/plan,
usage, interruption, connection, and terminal items from confirmed Sync; raw
provider events and process authority remain host-only. Renderer remount reads
the same host-owned run, while host restart reconstructs it from the durable
runtime/agent-run log.

At boot the shell selects confirmed Sync mode only when the catalog is live;
otherwise it retains explicit local-only mode for that renderer lifetime. The
catalogs never merge. The canonical runtime turn is the sole execution
authority and is transactionally mirrored to `agent_run`/`agent_run_event`
under the same thread/run refs. Exact semantic retries reconcile without a
second dispatch; conflicting identity reuse fails closed.

Protocol v6 also carries a separate owner-local Codex history catalog/page
capability. Active and archived (including zstd) rollouts are indexed in the
history worker without an age ceiling. The Effect Native history workspace
keeps top-level conversations left, a bounded selected-agent timeline center,
and a semantic agent/item inspector right. Source-order rows are typed,
credential-redacted, paged, and loss-accounted; raw JSONL, paths, credentials,
and provider authority never cross preload or enter Khala Sync.

Desktop main now also opens the shared `khala-sync-client` SQLite store beneath
its private `userData` root and persists one installation identity. After
opening it also creates a separate immutable device-local identity and local-
authority tables. Desktop remains usable without OpenAuth; Runtime Gateway v6
projects only `local_only | account_linked | local_unavailable`. Verified
account linking adds personal Sync, while disconnect/denial retains local rows.
After
native-session verification, main composes the shared production HTTP/
WebSocket session, subscribes only the server-derived owner's personal scope,
re-reads rotated access custody, and closes the session before the store. No
database path, handle, identity ref, row, queue, or credential crosses preload.

Desktop main now also owns one versioned native-session record encrypted with
Electron `safeStorage` beneath its private `userData` root. The host refuses
unavailable OS encryption and Linux `basic_text`, writes only an opaque atomic
encrypted blob, purges invalid records, and projects at most signed-out,
credential-present-unverified, or unavailable through the Runtime Gateway.
No owner ref, access token, or refresh token crosses preload.

At startup, a recovered encrypted record is now validated through the existing
native-session GET. Main rewrites valid OpenAuth rotation before projecting
`session_ready`, purges 401/403 or server-derived owner mismatch, and retains
custody but reports unavailable on transient or malformed-response failure.
Verified session readiness remains distinct from an authoritative conversation
projection.

Electron main now also composes the frozen Desktop loopback public-client
policy end to end. It binds one temporary `127.0.0.1` listener, generates and
validates cryptographic state + S256 PKCE, opens the exact authorize request,
exchanges the callback code, verifies the server owner, and writes encrypted
custody. Sign-out requires proof that both credential classes were revoked
before clear. The Runtime Gateway exposes only argument-free entry/exit
commands and bounded outcomes. Effect Native Settings now queries an
explicit tokenless bootstrap phase and dispatches typed, argument-free sign-in
or sign-out intents with honest loading/authenticating/ready/unavailable copy.
Session-ready remains distinct from an authoritative conversation projection.

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
