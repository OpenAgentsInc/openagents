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
Electron app whose application model is 100% Effect Native (the shared
vendored catalog at `apps/openagents.com/packages/effect-native-*`) and whose
renderer root and lifecycle are owned by React 19. The first cut retains the
proven direct catalog lowering inside that shared React surface while native
React component lowerings move across the renderer boundary. The renderer uses the T3 Code toolchain shape—
Vite and Tailwind CSS 4—without adopting a second router, store, schema system,
theme, icon set, or JSX application tree. Base UI remains the next renderer-
private primitive seam rather than an unused dependency in this cut.
It projects recent local
Codex chats read-only, renders assistant and owner transcript roles, clears the
composer after a submitted turn, provides New Chat and a closed command
palette, supports a user-selected workspace with bounded read/edit/save plus
typed read-only Git status/diff, and opens an explicit Fleet deployment brief
without pretending that local UI has authority to create a FleetRun.

## Run it

From the monorepo root:

```bash
pnpm install
pnpm run dev:openagents-desktop   # builds dist/ and launches Electron
```

Or from this directory: `pnpm run dev`.

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
pnpm --dir apps/openagents-desktop run verify
```

`verify` is the canonical clean-tree gate: typecheck, the complete package test
sweep, bundle build, and real headless Electron smoke/reload. The smoke uses a
checked-in privacy-safe Codex history fixture and scripted account-connect
child, never ambient `~/.codex` history or a default provider home. Set
`OPENAGENTS_DESKTOP_CODEX_SESSIONS` only for a separately identified real-
history acceptance run; that evidence is not the deterministic CI gate.

Automated smoke, startup-benchmark, live-proof, and MVP-proof runs keep their
Electron window hidden and hide the macOS Dock tile, including during
second-instance/deep-link steps, so they never take over the operator's screen.
Use `pnpm --dir apps/openagents-desktop run smoke:headed` only when a manual
proof explicitly needs the native window to be visible.

Tests cover: pure `state -> View` component trees, pure transitions, the
intent loop through the real registry, theme parity with the shared surface,
the mechanical Electron/EN boundary oracle, source-coupled service topology
and ambient-authority denial, replaceable exactly-once host lifecycle,
public-safe operation correlation through Sync causality, and a real bundle
build plus Electron teardown receipt.

## Architecture

- `src/main.ts` — Electron main process (plain TS). Hardened per #8574:
  `contextIsolation: true`, `nodeIntegration: false`, `sandbox: true`,
  `webviewTag: false`, deny-by-default permissions/navigation/window-open,
  restrictive CSP, no updater/publisher/devtools-installer.
- `src/desktop-host-lifecycle.ts` — the production-used replaceable owner for
  process, WorkContext/session, and window resources; app teardown drains it
  once in dependency-safe order.
- `src/desktop-operation-context.ts` — strict ref-only operation/session/run
  correlation and the bounded host journal used by the built acceptance path.
- `src/preload.cts` — the only bridge: a closed set of schema-checked chat,
  workspace, bounded save/Git, Fleet-brief, and public-safe Codex-account
  capabilities over fixed IPC channels. No raw token, Node capability,
  arbitrary command/channel, generic event subscription, or `MessagePort`
  reaches the renderer.
- `src/fleet-control.ts` — main-process adapter for the existing local Pylon
  `intent.submit` command. It resolves the loopback control token locally and
  returns only `accepted | rejected | unavailable` status.
- `src/product-spec-workroom.ts` — host-owned ProductSpec plan/run authority.
  In addition to revisioned intent edits, it exposes a distinct
  evidence-attachment proposal path backed by `@openagentsinc/product-spec`:
  an agent may propose exact Markdown, but only a fixed owner-confirmation IPC
  can apply it. Confirmation rechecks the reviewed document digest, preserves
  `spec_revision` and the canonical intent digest, changes the exact document
  digest, and retains any prior run and receipts as `revision_mismatch` history.
  There is deliberately no app-server confirmation tool, so private or
  unreviewed receipt material cannot auto-publish through the agent path.
  This remains internal MVP tooling: Desktop exposes no ProductSpec or
  AssuranceSpec navigation item, route, or user-facing screen.
- `src/renderer/` — the application is Effect Native; React is its DOM target:
  - `shell.ts` — typed state, `defineIntent` intents, pure transitions,
    pure `state -> View` over the shared catalog.
  - `theme.ts` — the one Protoss-blue theme via `@effect-native/tokens`,
    token-identical to the shared OpenAgents theme values.
  - `boot.ts` — `SubscriptionRef` + `makeViewProgramFromState` +
    `makeIntentRegistry` + the React-owned Effect Native DOM surface.
- `scripts/build.ts` — Vite Plus bundles main (ESM), preload (CJS, sandboxed),
  and workers; Vite + the React and Tailwind plugins emit the fixed signed
  renderer assets `index.html`, `boot.js`, and `app.css`.

The deliberate omissions from T3's stack are architectural: Desktop does not
need TanStack Router because navigation is already typed application state,
and it does not need Zustand or Effect Atom React because `SubscriptionRef`
and the Effect Native `ViewProgram` remain the single state/projection owner.
Base UI, Lexical, LegendList, Pierre, and xterm are deferred until their
corresponding catalog component or typed Host driver actually uses them. The system/SF Pro
font contract, closed OpenAI Apps SDK icon catalog, khala theme, boot frame,
and tokenless Electron boundary remain unchanged.

Target evolution preserves this boundary rather than widening the preload one
feature at a time. The renderer consumes one closed schema-decoded projection/
intent/event surface; a host-owned Runtime Gateway composes existing Khala
Sync, Pylon, workspace, and execution services. Lightweight R1/R2/D1 adapters
may start in main for delivery speed, while filesystem watch, PTY, engine
supervision, extension, and other heavy services move behind one utility
process before D3/D4 breadth. The renderer never receives bearer/provider/
Pylon credentials, a loopback URL, raw runtime events, general IPC, or a raw
`MessagePort`.

Runtime Gateway protocol v8 now carries the authoritative conversation path:
confirmed catalog/thread/current-timeline queries, canonical create/append
enqueues, exact thread/message/run start or interrupt commands, and typed
cursor-aware subscribe/resume/unsubscribe over the existing decoded event
channel. The same confirmed thread snapshot carries matching graph refs and a
bounded set of canonical live-agent graph post-images. Exact reconnect resumes
from the durable cursor; a proven gap sends one authoritative-refetch snapshot,
while a non-live scope exposes no cached graph authority. Enqueue is
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

Lifecycle convergence is also authority-backed: delayed Sync responses are
discarded when their subscription generation is superseded; runtime provider
events must match the durable next sequence and current turn state; and a
stale hosted worker is terminalized as interrupted without re-running the
provider. The real Desktop and Expo SQLite adapters reconstruct the same
partial timeline and terminal after close/reopen. The built Runtime Gateway v8
smoke covers correlation and teardown; the physical-mobile network-gap receipt
remains a separate #8689/#8677 close gate.

The live host registry is capped, generation-fenced, and backpressure-
coalesced. Authenticated Sync replacement/sign-out resets all current
subscriptions; Runtime Gateway disposal closes the registry. Renderer
conversation polling removal remains the final CUT-10 consumer step.

Protocol v8 also carries a separate owner-local Codex history catalog/page
capability. Active and archived (including zstd) rollouts are indexed in the
history worker without an age ceiling. The Effect Native history workspace
keeps top-level conversations left, a bounded selected-agent timeline center,
and a semantic agent/item inspector right. Source-order rows are typed,
credential-redacted, paged, and loss-accounted; raw JSONL, paths, credentials,
and provider authority never cross preload or enter Khala Sync.

Desktop main now also opens the shared `khala-sync-client` SQLite store beneath
its private `userData` root and persists one installation identity. After
opening it also creates a separate immutable device-local identity and local-
authority tables. Desktop remains usable without OpenAuth; Runtime Gateway v8
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

### Unattended macOS verification

Never diagnose this custody with the macOS `security` CLI. A
`security find-generic-password` probe can open a blocking login-Keychain
password dialog, and repeated probes create repeated dialogs while the owner is
away. Automated checks must not inspect or decrypt `OpenAgents Safe Storage`.

Use the existing isolated proof mode for signed-out/local-only packaged checks:

```sh
tmp_profile="$(mktemp -d "${TMPDIR%/}/openagents-proof.XXXXXX")"
OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF=1 \
OPENAGENTS_DESKTOP_USER_DATA="$tmp_profile" \
apps/openagents-desktop/out/OpenAgents-darwin-arm64/OpenAgents.app/Contents/MacOS/OpenAgents
```

The user-data path must remain below the OS temporary directory. This mode
enables Chromium's mock keychain and disables native session custody, so it is
not authenticated-Sync evidence. Authenticated checks instead launch the
signed app with its existing normal profile and inspect only public-safe
session/IPC state and visible UI; they never extract credential material.

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
