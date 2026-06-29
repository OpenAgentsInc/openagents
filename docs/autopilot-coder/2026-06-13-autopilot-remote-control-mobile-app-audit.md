# Autopilot Remote Control — Mobile App Audit

> **⚠️ BUILD/SHIP POLICY (2026-06-13): EAS is OUT.** Native builds compile
> locally on our Mac (`expo prebuild`→`xcodebuild`/`fastlane`), TestFlight upload
> is Apple-native (`xcrun altool`), JS ships OTA via our own
> `updates.openagents.com`. Any "EAS/Expo cloud" phrasing below is historical.
> Canonical runbook: `clients/khala-ios/AutopilotRemoteControl/TESTFLIGHT.md`.

Date: 2026-06-13 (framework decision revised 2026-06-13: React Native/Expo,
superseding the earlier Swift direction — see "Framework Decision" below).
Status: planning/architecture audit. Defines a React Native (Expo) mobile app
that connects to a running Pylon node to observe and steer Autopilot Coder
sessions remotely. Changes no runtime invariant by itself; the Pylon-side
dependencies it names are tracked as their own issues.

## Overview

**Autopilot Remote Control** is a React Native (Expo) mobile app — iOS-first,
Android-capable from the same codebase — that pairs with a running Pylon node
and lets an operator watch and steer Autopilot Coder coding sessions from their
phone: see active/blocked/waiting sessions, read public-safe progress, answer
approval prompts, send a bounded instruction, spawn or cancel a bounded session,
and get notified when the agent needs attention.

It is the consumer, open-source sibling of the private owner-only `control`
repo. `control` is a Tailnet-first operator console for *our* fleet; this app is
a shippable, paid, single-user companion for *any* Pylon user, open source in
the `openagents` monorepo.

It also has a **same-machine sibling**: Autopilot Desktop
(`2026-06-13-autopilot-desktop-app-audit.md`), a Bun/Electrobun + Foldkit GUI
that talks to the *local* Pylon node over loopback. Because this mobile app is
now React Native/TypeScript, it **imports the shared
`packages/autopilot-control-protocol` (Effect Schema) directly** — the same package
the desktop Bun main process and the web companion use. One contract, one
implementation, tested once. The only differences across surfaces are transport
(mobile = remote bridge; desktop = loopback) and UI (mobile = React Native;
desktop/web = Foldkit/DOM).

Key product constraints (owner direction, 2026-06-13):

- **React Native (Expo), TypeScript** — reverses the earlier "Swift, no React
  Native" constraint. Rationale in "Framework Decision" below.
- **Open source** in the `openagents` repo.
- **$4.99 paid download** on the App Store (source is free; the convenience
  binary is paid). Android/Play is possible from the same codebase later.

## Framework Decision (React Native/Expo, not Swift)

Reversed from the initial Swift direction after weighing technical merit with
build cost held equal:

- **The crux is one protocol implementation, not two.** The hard, evolving,
  correctness-critical part of this app is the bridge/control protocol (cursor
  resume, dedup, exactly-once decisions, capability gating, backpressure). In
  React Native it is the shared `packages/autopilot-control-protocol` Effect
  package, imported and run on-device, tested once with the same vitest fixtures
  as web/desktop. In Swift it would be a hand-mirrored Codable + client kept in
  sync by hand against a versioned protocol — the highest ongoing drift/bug
  risk in a remote-control-of-an-agent app.
- **The UI is not shared either way.** Foldkit renders to the DOM, which RN does
  not have; RN uses its own components, Swift uses SwiftUI. So RN's win is
  specifically the **non-UI layer** (protocol, client, cursor/decision state),
  which is exactly the part that drifts. RN does not reuse the Foldkit UI.
- **Thin-client profile.** This is a status/control client, not graphics- or
  latency-critical, so RN's usual costs (JS runtime, binary size, bridge) barely
  bite while the shared-correctness upside is maximal.
- **Flip trigger back to Swift:** if first-class iOS-native surfaces become core
  v1 features — **Live Activities / Dynamic Island** ("decision required" on the
  lock screen), interactive **widgets**, **App Intents/Siri**, or an **Apple
  Watch** companion — Swift's platform depth would outweigh the shared-protocol
  win. For the v1–v3 scope here, those are nice-to-haves, so RN wins.

### Base: vanilla Expo, not the Ignite boilerplate

Use a lean custom **Expo** app, not Infinite Red's Ignite boilerplate
(`projects/repos/ignite`):

- Our value lives in the shared Effect protocol package + a streaming client —
  **neither base provides that**, so Ignite adds no leverage where the work is.
- Ignite's headline pieces are redundant or counter to our architecture: its
  **apisauce REST client** is the opposite of our Effect client + SSE/WS
  transport; its component library, generators, and i18n are product-app breadth
  a thin streaming supervisor does not need. We would spend effort *pruning*
  Ignite to fit.
- Ignite scaffolds a standalone repo via its CLI; a hand-rolled Expo app slots
  into our monorepo cleanly and consumes the shared workspace package directly.
- **Mine Ignite for parts, not the frame:** match its current version baseline
  (Expo SDK 55 / RN 0.81 / React 19 / React Navigation v7), and borrow MMKV for
  persistence (node list, stream cursors, cached projections), edge-to-edge +
  keyboard-controller, its EAS build config and Maestro E2E patterns, and its
  theming structure. Skip apisauce; the transport is the Effect client.
- **Flip trigger to Ignite:** if this grows into a broad, screen-heavy,
  REST-backed product app with a large design system and multiple teams,
  Ignite's opinionated structure starts paying off. Not the v1–v3 scope.

## Goals / Non-Goals

Goals:

- Connect to one or more Pylon nodes over the network (Tailnet-first).
- Render the Pylon control surface as a fast, status-oriented React Native UI.
- Relay approvals/decisions and bounded instructions back to the session.
- Stream live session events into a timeline (WS or SSE; see transport note).
- Notify (push/local) when a session is waiting on a decision or finishes.

Non-goals (hard boundaries, from the remote session bridge audit, system #39):

- Not a remote terminal. No raw shell, no raw provider payloads, no secrets, no
  local paths, no private repo content unless a private-channel policy is
  explicitly granted for that exact run.
- Not a second runtime. The local Pylon node remains the authority; the app is a
  projection + scoped-action layer.
- Cannot introduce new tools or widen filesystem/shell/provider/wallet authority.
- Not a wallet. No spend authority from the app by default.

## Repo Placement And Structure

A React Native/Expo app is TypeScript, so it can consume the shared workspace
protocol package — but it uses Metro (not Vite) and a large native-module dep
tree, so it should not sit inside the Bun/Effect deploy-topology guard's globs.
Place it in a `clients/khala-ios/` area that depends on the shared package without
becoming part of the Worker/Foldkit build:

```
openagents/
  clients/
    mobile/
      AutopilotRemoteControl/        # Expo app (Metro)
        app.config.ts  eas.json  metro.config.js  package.json
        app/                          # screens (React Navigation v7)
          nodes/ sessions/ session-detail/ decisions/ spawn/ settings/
        src/
          control/                    # adapters over @openagents/autopilot-control-protocol
          stream/                     # WS/SSE client, cursor resume, dedup
          stores/                     # node/event/decision stores (MMKV-backed)
          pairing/                    # QR parse + bootstrap exchange
          notifications/              # expo-notifications registration
        .maestro/                     # E2E flows (borrowed from Ignite patterns)
        README.md                     # build + provenance + pricing note
```

- `clients/` is the area for client apps that are not part of the Bun/Effect
  Worker build (it already houses the desktop-adjacent and mobile clients).
- **It imports `packages/autopilot-control-protocol`** (Effect Schema + client). To
  consume the workspace package without dragging Metro into the root install,
  reference it via a path/workspace dependency or a thin build step that
  publishes the package locally; keep the RN/Metro install scoped to
  `clients/khala-ios/`.
- The shared package stays on the tracked Effect v4 line; the RN app pins its
  own Expo SDK / RN versions independently.

## Connectivity And Pairing Model

Pylon's control server (`apps/pylon/src/node/control-server.ts`) binds
**loopback** (`127.0.0.1:4716`) by default and authenticates with a bearer token
written to `<pylon-home>/control-token`. The schema tag is
`openagents.pylon.control.v0.3`. A phone cannot reach loopback, so connectivity
uses a private-network-first path:

- **Tailnet-first.** The user runs Pylon with the control server bound to a
  Tailnet-reachable interface and connects the app to `http://<tailnet-ip>:4716`
  with the node's bearer token. This keeps the surface private to the user's own
  tailnet with no public exposure.
- **Pairing UX.** The Pylon TUI/CLI should present the base URL + token as a
  scannable QR (and copyable text); the app pairs by scanning it. The app stores
  the token in secure storage (`expo-secure-store`), never in plain storage or logs.
- **Multiple nodes.** The app keeps a list of paired nodes (label + base URL +
  secure-store-backed token ref) and shows per-node reachability.
- **Scoped credential exchange.** The app-ready bridge should replace direct
  long-lived operator-token storage with a short-lived QR/bootstrap secret that
  is exchanged for a capability-scoped pairing credential.
- **Active-client receipts.** The node should expose connected client id,
  device class, projection level, last heartbeat, expiry, and revocation state
  so the local operator can audit and revoke phone access.

The app must surface, honestly, when a node is unreachable, when a projection is
stale (`last update` + staleness, per the companion audit), and what authority
the current pairing holds.

## Pylon Control API Consumed

From the live control server today:

- `GET /health` → `{ ok, schema }` (no auth) — reachability probe.
- `GET /events` (SSE, bearer) — node snapshot + heartbeat stream.
- `POST /command` (bearer) with a typed `ControlCommand`:
  - `session.spawn` — bounded session (adapter, accountRef/home, worktree/repoRef,
    objective, verify, timeout). The server already rejects danger modes.
  - `session.list` — current sessions + state.
  - `session.events` — event history for a session.
  - `session.cancel` — cancel a session.
- `GET /sessions/:sessionRef/events` (SSE, bearer) — live per-session event
  stream → the app's timeline.

Transport note: React Native's `fetch` does not stream cleanly, so prefer a
**WebSocket** transport for the read stream (the bridge plan already supports WS
or SSE-read + POST-write), or `react-native-sse` if staying on SSE. Either way
the cursor-resume/dedup logic is the shared package's, not transport-specific.

The app is a typed client over exactly these; it introduces no new authority the
server does not already expose.

For P0, the app can consume the current endpoints in read-only mode while all
effectful controls stay disabled. For P1 and later, effectful controls should
flow through the remote session bridge rather than exposing the node's
all-purpose bearer token to a mobile client. The app should therefore isolate
the live endpoint bindings behind a `PylonControlClient` protocol so the read
models can ship before the scoped action protocol is complete.

## Bridge Protocol Contract For Actions

The app-ready bridge should present a smaller remote-control contract on top of
the node runtime:

- `bridge.pair.exchange` — exchange QR/bootstrap material for a scoped pairing
  credential.
- `bridge.revoke` and `bridge.clients.list` — revoke or inspect paired clients.
- `session.list`, `session.subscribe`, `session.snapshot`, and
  `session.history` — read the current projection.
- `turn.steer` — send a bounded instruction when the active turn accepts
  steering.
- `turn.interrupt` — request interruption of a specific active turn.
- `session.cancel`, `session.pause`, and `session.resume` — separate lifecycle
  controls.
- `decision.resolve` — approve, deny, or answer one pending decision request.
- `artifact.read` — fetch an artifact by ref and projection level.

Every action request should include a client request id, idempotency key,
pairing ref, session/run ref where applicable, capability ref, and current
stream cursor. Every action response should include a receipt ref and a typed
result so the app can distinguish success, duplicate, expired, cancelled,
revoked, stale, and overloaded states.

The app should treat server-originated decision requests as first-class objects,
not timeline text. A decision request needs a request id, action ref, effect
summary, expiry, allowed verbs, capability ref, and public-safe evidence refs.
Responses bind to that request id exactly once. Free-form user text is a
bounded instruction only when sent through `turn.steer`; it is never an
approval.

## Authority / Capability Model

Adopt the remote session bridge authority model (system #39). Capabilities are
separate and individually policy-gated:

- observe public-safe progress
- observe private session detail (only with an explicit per-run private-channel
  grant)
- answer approval prompts
- send a bounded user message
- request cancellation
- pause / resume
- download artifacts

Rules the app must honor:

- Remote approval answers bind to a pending approval id + session ref +
  capability ref + expiry, and are single-use and idempotent. Free-form chat is
  never an approval.
- Pairings expire and are locally revocable; the node shows connected clients.
- On disconnect, unresolved remote-only prompts return to local handling.
- The app renders missing/stale/private-only data as scoped caveats, never as
  empty success.

## Screens (React Native)

- **Nodes** — paired nodes, reachability, add via QR scan, revoke.
- **Sessions** — list with state chips (queued/running/completed/failed/
  cancelled/waiting), objective ref, last public-safe progress, staleness.
- **Session detail** — live event timeline, routing/account ref,
  artifacts/closeout refs, cancel/pause/resume actions per capability.
- **Decisions** — approval/answer queue; one-tap approve/deny bound to the
  pending approval id.
- **Spawn** — start a bounded session (adapter, objective, verify, timeout);
  never danger mode.
- **Settings** — notification prefs (push/local/quiet hours), per-node authority
  display, sign-out/revoke.

React Native (React Navigation v7); status-oriented, not a terminal emulator.

## Client Architecture

The protocol/client layer is **the shared `packages/autopilot-control-protocol`
Effect package** (schemas, request builders, the cursor/decision state logic),
imported by this app, the desktop, and the web companion. On top of it the RN
app adds device-specific TypeScript modules:

- `nodeStore`: persisted node labels, base URLs, secure-store token refs, last
  reachability, per-node display prefs (MMKV-backed).
- `pairingClient`: QR parsing, bootstrap exchange, capability display,
  revocation, and active-client refresh.
- `controlClient`: thin RN adapter over the shared package's typed requests
  (health, session list/history, subscribe, actions, artifact reads).
- `streamClient`: WS or `fetch`/`react-native-sse` reader with incremental frame
  parsing, heartbeat/liveness timers, cursor resume, duplicate filtering, and
  backoff (reuses the shared package's cursor/dedup logic).
- `eventStore` / `decisionStore`: normalized snapshots, timelines, stream
  cursor, lag caveats, staleness; pending decisions with expiry timers and
  exactly-once response state — built on the shared decision types.
- `reachabilityMonitor`: network-path changes, foreground/background refresh,
  reconnect scheduling.
- `notificationRegistrar`: `expo-notifications` registration, device-token sync,
  quiet hours, notification-open routing.

Keep UI state derived from stores and capability refs. Controls are disabled
because the pairing lacks a capability or the request is stale — never because a
view made an independent authority guess.

## Event Stream And Offline Behavior

P0 should use a WebSocket (or `react-native-sse`) read stream and typed POST for
writes. Each stream stores the last accepted event id and sequence. On
reconnect, the app sends the cursor; the server either replays missing lossless
events or sends a fresh snapshot with a lag caveat. The app deduplicates by
event id and treats out-of-order or duplicate action receipts as already
handled. (This dedup/cursor logic lives in the shared package.)

The app should maintain a small local cache per node:

- Latest node snapshot.
- Session list projection.
- Per-session timeline window.
- Pending decisions.
- Last stream cursor and last successful refresh time.
- Current capability and revocation status.

Foreground behavior: reconnect streams, refresh snapshots, replay pending
receipt checks, and mark any stale views with exact timestamps. Background
behavior: close long-lived streams when iOS requires it, rely on APNs/local
notifications for attention, and refresh the relevant node/session when the
notification opens the app.

Offline action handling should be strict. Decision responses should send only
while the request is still fresh; otherwise the app should discard the queued
response and show an expired state. Bounded instructions may be queued only when
the session still accepts steering and the capability remains fresh. Spawn,
cancel, interrupt, and private artifact reads should not be silently queued
across long offline windows.

## Notifications

- `expo-notifications` (APNs on iOS, FCM on Android) for "waiting on decision"
  and "session finished/failed" when backgrounded; local notifications as
  fallback. Delivery is a projection of the decision-queue / notification spine
  (#4765) — the app subscribes, it does not invent new event types. Payloads
  carry refs only, never private content.

## Pricing, Licensing, Distribution

- **$4.99 paid App Store download.** Source is open in `openagents`; the binary
  is a paid convenience. (Open-source + paid binary is an established model.)
  The app `README.md` states the source is free under the repo license and the
  App Store price covers the built/signed/distributed binary.
- No in-app purchases for v1. No subscription. The app does not meter or bill —
  it is a client; any paid compute/inference (the OpenAgents Cloud plan) is
  billed server-side, not here.
- App Store review notes: it is a developer utility that connects to the user's
  own Pylon node via a URL + token the user supplies; no account required to
  use, no data collection beyond the secure-store node token on-device.
- Built and submitted via Expo/EAS. iOS App Store first; the same Expo codebase
  can target Android/Play later (pricing parity TBD).

## Security And Privacy

- Credentials stored via `expo-secure-store` (iOS Keychain / Android Keystore);
  never logged, never in MMKV/plain storage, never in analytics.
- For scoped pairing, the QR/bootstrap secret should be short-lived and
  one-time-use. The exchanged credential should include node ref, pairing ref,
  client id, device class, capability refs, projection level, issuer/audience,
  expiry, and nonce or `jti`.
- Non-loopback node listeners must reject unauthenticated control traffic.
- TLS where the transport offers it; Tailnet provides the private network.
- The app honors the control server's loopback/danger-mode refusals; it cannot
  request danger-full-access.
- No raw shell/prompt/secret/path rendering; only public-safe projections unless
  a private-channel grant exists for that run.
- Read-only mode is a real authority mode. A read-only pairing must be unable
  to approve, interrupt, cancel, spawn, steer, or fetch private artifacts even
  if the UI has stale controls.
- No telemetry by default. Any future telemetry must be refs/aggregates only.

## Pylon-Side Dependencies (what this app needs that may not exist yet)

The app is a thin client; several capabilities live on the Pylon/bridge side:

1. **Remote-reachable control binding.** Today the control server binds loopback
   by default. A documented Tailnet-bind path is needed, with explicit refusal
   for unauthenticated non-loopback control traffic.
2. **Pairing + revocation + capability scoping (system #39).** The control
   server today is single bearer-token, all-or-nothing. The remote session
   bridge layer (pairing receipt, per-capability policy refs, revocation
   receipt, public/private projection levels, active-client list) is **not yet
   implemented**.
3. **Approval/decision relay over the control API.** Remote approval must reuse
   the decision-queue spine (#4765), bind single-use answers, replay pending
   prompts on subscribe, and broadcast cancellation/resolution events.
4. **Cursor-resumable event streams.** Node and per-session streams need event
   ids, sequences, lossless/best-effort tiers, duplicate-safe replay, lag
   caveats, and bounded backpressure behavior.
5. **Action receipts and typed failures.** Every remote action needs an
   idempotent receipt and typed failure states for duplicate, expired,
   cancelled, revoked, stale, unauthorized, unsupported, and overloaded cases.
6. **Push fan-out.** A notification path the app can register against, with
   refs-only payloads and quiet-hours policy.

No public claim should say "remote terminal control shipped" until there is a
pairing receipt, revocation receipt, remote-approval test, and public/private
projection test (per the system #39 decision).

## Testing

- Protocol/client: tested **once** in `packages/autopilot-control-protocol` with
  vitest (schema round-trips against `openagents.pylon.control.v0.3`, frame
  parsing, cursor/dedup) — shared with web/desktop, not re-tested per platform.
- App: RN component/store tests for pairing, session list/detail, and the
  approval flow; Maestro E2E flows (patterns borrowed from Ignite).
- Contract: a shared fixture set mirroring `/health`, `/command` results, and
  `/sessions/:ref/events` frames so all surfaces test against the same node
  fixtures without a live node.
- Negative: expired/revoked pairing rejected; danger-mode spawn rejected by the
  server surfaced as a clear error; stale projection shown as a caveat.
- Stream: reconnect from last event id/sequence, replay missing events,
  deduplicate repeats, show lag caveats after retention gaps, and preserve
  lossless decision/completion events under best-effort drops.
- Actions: approval exactly once, duplicate approval ignored with typed result,
  late approval rejected after expiry, externally resolved prompt disabled, and
  read-only pairing blocked from all effectful verbs.
- Offline: queued bounded instruction sent only while capability and session
  freshness remain valid; queued decision response discarded after expiry.
- Security: QR/bootstrap one-time use, secure-store-only credential storage,
  unauthenticated non-loopback requests rejected, and refs-only notification
  payloads.
- UI: stale/offline/read-only banners, disabled controls with reasons,
  notification-open routing to the correct node/session/decision, and mobile
  layout for long objectives and effect summaries.

## Phased Plan

- **P0 — Read-only companion.** Pair (URL+token+QR), node reachability, session
  list + live detail timeline (SSE). No actions yet; read-only mode may ship
  against the control API as it exists today while presenting stale/offline and
  authority state honestly.
- **P1 — Scoped actions.** Approve/deny decisions, cancel, send bounded
  instruction — gated on the bridge capability/pairing work, action receipts,
  and cursor-resumable decision replay (system #39).
- **P2 — Spawn + notifications.** Bounded `session.spawn` from the app; push
  (expo-notifications) for waiting/finished; no spawn across stale or read-only
  pairings.
- **P3 — Multi-node + polish.** Multiple paired nodes, per-node authority
  display, theming, App Store submission at $4.99.

## Recommended Issues

0. Extract `packages/autopilot-control-protocol` (Effect Schema + client +
   cursor/decision logic) shared by web, desktop, and this app — the foundation
   the RN decision depends on. (openagents) *[shared with the desktop audit]*
1. `clients/khala-ios` scaffold: vanilla **Expo** app (Expo SDK 55 / RN 0.81 /
   React 19 / React Navigation v7), MMKV + edge-to-edge + EAS + Maestro patterns
   borrowed from Ignite, consuming `packages/autopilot-control-protocol`. (openagents)
2. P0 read-only companion: pairing (QR/URL/token in `expo-secure-store`), node
   health, session list, WS/SSE session-detail timeline. (openagents)
3. Pylon: documented Tailnet-bind path for the control server + a TUI/CLI
   pairing QR (URL + token). (openagents/apps/pylon)
4. Pylon remote session bridge (system #39): pairing receipt, per-capability
   policy refs, revocation receipt, public/private projection levels, remote
   approval single-use binding + tests. (openagents/apps/pylon)
5. Pylon streams: event ids, stream sequences, cursor resume, duplicate-safe
   replay, lag caveats, and lossless/best-effort delivery tiers.
6. App P1 scoped actions wired to #4 and #5
   (approve/deny/cancel/interrupt/instruct).
7. App P2 spawn + push notifications against the decision/notification spine
   (#4765).
8. App README + App Store metadata: open-source provenance, $4.99 pricing note,
   privacy (on-device token only), review notes.

## Open Decisions

1. **Framework — RESOLVED: React Native (Expo), vanilla (not Ignite).** See
   "Framework Decision". Flip to Swift only if iOS-native surfaces (Live
   Activities, widgets, Watch, App Intents) become core v1 features; flip to
   Ignite only if this becomes a broad REST-backed product app.
2. **Connectivity beyond Tailnet?** v1 is Tailnet-first (matches `control`). A
   public relay (e.g., via the OpenAgents Cloud coordinator) is a later option,
   not v1.
3. **Bundle id / Apple+Google dev accounts / EAS signing** — owner action.
4. **One app for both fleets?** Keep this consumer app separate from the private
   `control` operator app; they share the bounded control contract but differ in
   audience, authority, and distribution.
5. **Free read-only tier vs fully paid?** Owner chose $4.99 paid download for
   v1; revisit a free read-only tier only if App Store positioning calls for it.
6. **Android/Play?** The Expo codebase can target Android later; iOS App Store
   is v1. Pricing parity and Play listing are deferred.
7. **iOS-native attention surfaces?** Live Activities / Dynamic Island for
   "decision required" are compelling but limited under RN; treat as a later
   native-module spike, and the trigger to reconsider Swift if they become core.
