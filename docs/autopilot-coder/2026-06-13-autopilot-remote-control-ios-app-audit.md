# Autopilot Remote Control — iOS App Audit

Date: 2026-06-13
Status: planning/architecture audit. Defines a new native iOS app that connects
to a running Pylon node to observe and steer Autopilot Coder sessions remotely.
Changes no runtime invariant by itself; the Pylon-side dependencies it names are
tracked as their own issues.

## Overview

**Autopilot Remote Control** is a native iOS app that pairs with a running
Pylon node and lets an operator watch and steer Autopilot Coder coding sessions
from their phone: see active/blocked/waiting sessions, read public-safe
progress, answer approval prompts, send a bounded instruction, spawn or cancel a
bounded session, and get notified when the agent needs attention.

It is the consumer, open-source sibling of the private owner-only `control`
repo. `control` is a Tailnet-first operator console for *our* fleet; this app is
a shippable, paid, single-user companion for *any* Pylon user, open source in
the `openagents` monorepo.

Key product constraints (owner direction, 2026-06-13):

- **Swift, native UI only** — SwiftUI, no embedded web views / no React Native.
- **Open source** in the `openagents` repo.
- **$4.99 paid download** on the App Store (source is free; the convenience
  binary is paid).

## Goals / Non-Goals

Goals:

- Connect to one or more Pylon nodes over the network (Tailnet-first).
- Render the Pylon control surface as a fast, status-oriented native UI.
- Relay approvals/decisions and bounded instructions back to the session.
- Stream live session events (SSE) into a native timeline.
- Notify (APNs/local) when a session is waiting on a decision or finishes.

Non-goals (hard boundaries, from the remote session bridge audit, system #39):

- Not a remote terminal. No raw shell, no raw provider payloads, no secrets, no
  local paths, no private repo content unless a private-channel policy is
  explicitly granted for that exact run.
- Not a second runtime. The local Pylon node remains the authority; the app is a
  projection + scoped-action layer.
- Cannot introduce new tools or widen filesystem/shell/provider/wallet authority.
- Not a wallet. No spend authority from the app by default.

## Repo Placement And Structure

The `openagents` repo is a Bun/Effect monorepo and its AGENTS.md forbids
reintroducing a Cargo/Tauri workspace. A Swift app is native and must stay out
of the Bun workspace globs so it never enters the Effect-v4 deploy-topology
guard. Proposed layout:

```
openagents/
  clients/
    ios/
      AutopilotRemoteControl/
        AutopilotRemoteControl.xcodeproj/
        AutopilotRemoteControl/            # SwiftUI sources
          App/                              # @main App, scene, routing
          Pairing/                          # connect/pair/revoke flows
          Sessions/                         # list, detail, event timeline
          Decisions/                        # approval/answer queue
          Networking/                       # PylonControlClient (URLSession)
          Models/                           # Codable mirrors of control schema
          Notifications/                    # APNs/local registration
        AutopilotRemoteControlTests/
        README.md                           # build + provenance + pricing note
```

- `clients/` is a new top-level area for native client apps, deliberately
  outside `apps/` (Bun packages) and excluded from `package.json` workspaces and
  `tsconfig` so it does not affect the Worker/Foldkit/Effect build.
- The Xcode project lives inside the app folder, mirroring how the `control`
  repo keeps `Control/Control.xcodeproj` at its root.
- No new Bun dependency, no change to the deploy-topology guard.

## Connectivity And Pairing Model

Pylon's control server (`apps/pylon/src/node/control-server.ts`) binds
**loopback** (`127.0.0.1:4716`) by default and authenticates with a bearer token
written to `<pylon-home>/control-token`. The schema tag is
`openagents.pylon.control.v0.3`. A phone cannot reach loopback, so connectivity
mirrors the `control` repo's proven path:

- **Tailnet-first.** The user runs Pylon with the control server bound to a
  Tailnet-reachable interface and connects the app to `http://<tailnet-ip>:4716`
  with the node's bearer token. This keeps the surface private to the user's own
  tailnet with no public exposure.
- **Pairing UX.** The Pylon TUI/CLI should present the base URL + token as a
  scannable QR (and copyable text); the app pairs by scanning it. The app stores
  the token in the iOS Keychain, never in plists or logs.
- **Multiple nodes.** The app keeps a list of paired nodes (label + base URL +
  Keychain-backed token ref) and shows per-node reachability.

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
  stream → the app's native timeline.

The app is a typed client over exactly these; it introduces no new authority the
server does not already expose.

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

## Native UI (SwiftUI) Screens

- **Nodes** — paired nodes, reachability, add via QR scan, revoke.
- **Sessions** — list with state chips (queued/running/completed/failed/
  cancelled/waiting), objective ref, last public-safe progress, staleness.
- **Session detail** — live event timeline (SSE), routing/account ref,
  artifacts/closeout refs, cancel/pause/resume actions per capability.
- **Decisions** — approval/answer queue; one-tap approve/deny bound to the
  pending approval id.
- **Spawn** — start a bounded session (adapter, objective, verify, timeout);
  never danger mode.
- **Settings** — notification prefs (push/local/quiet hours), per-node authority
  display, sign-out/revoke.

All native SwiftUI; status-oriented, not a terminal emulator.

## Notifications

- APNs for "waiting on decision" and "session finished/failed" when the app is
  backgrounded; local notifications as fallback. Delivery is a projection of the
  decision-queue / notification spine (#4765) — the app subscribes, it does not
  invent new event types. Payloads carry refs only, never private content.

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
  use, no data collection beyond the Keychain-stored node token on-device.

## Security And Privacy

- Bearer token stored in the iOS Keychain; never logged, never in plists, never
  in analytics.
- TLS where the transport offers it; Tailnet provides the private network.
- The app honors the control server's loopback/danger-mode refusals; it cannot
  request danger-full-access.
- No raw shell/prompt/secret/path rendering; only public-safe projections unless
  a private-channel grant exists for that run.
- No telemetry by default. Any future telemetry must be refs/aggregates only.

## Pylon-Side Dependencies (what this app needs that may not exist yet)

The app is a thin client; several capabilities live on the Pylon/bridge side:

1. **Remote-reachable control binding.** Today the control server binds loopback
   by default. A documented Tailnet-bind path (host + token + QR) is needed.
2. **Pairing + revocation + capability scoping (system #39).** The control
   server today is single bearer-token, all-or-nothing. The remote session
   bridge layer (pairing receipt, per-capability policy refs, revocation
   receipt, public/private projection levels) is **not yet implemented**.
3. **Approval/decision relay over the control API.** Remote approval must reuse
   the decision-queue spine (#4765) and bind single-use answers.
4. **Push fan-out.** A notification path the app can register against.

No public claim should say "remote terminal control shipped" until there is a
pairing receipt, revocation receipt, remote-approval test, and public/private
projection test (per the system #39 decision).

## Testing

- App: unit tests for the `PylonControlClient` (Codable round-trips against the
  `openagents.pylon.control.v0.3` schema, SSE frame parsing, auth header), and
  UI tests for pairing, session list/detail, and the approval flow.
- Contract: a fixture set mirroring `/health`, `/command` results, and
  `/sessions/:ref/events` frames so the client is tested without a live node.
- Negative: expired/revoked pairing rejected; danger-mode spawn rejected by the
  server surfaced as a clear error; stale projection shown as a caveat.

## Phased Plan

- **P0 — Read-only companion.** Pair (URL+token+QR), node reachability, session
  list + live detail timeline (SSE). No actions yet. Ships against the control
  API as it exists today.
- **P1 — Scoped actions.** Approve/deny decisions, cancel, send bounded
  instruction — gated on the bridge capability/pairing work (system #39).
- **P2 — Spawn + notifications.** Bounded `session.spawn` from the app; APNs for
  waiting/finished.
- **P3 — Multi-node + polish.** Multiple paired nodes, per-node authority
  display, theming, App Store submission at $4.99.

## Recommended Issues

1. `clients/ios` scaffold: SwiftUI app + Xcode project + `PylonControlClient` +
   Codable models for `openagents.pylon.control.v0.3`, excluded from the Bun
   workspace. (openagents)
2. P0 read-only companion: pairing (QR/URL/token in Keychain), node health,
   session list, SSE session-detail timeline. (openagents)
3. Pylon: documented Tailnet-bind path for the control server + a TUI/CLI
   pairing QR (URL + token). (openagents/apps/pylon)
4. Pylon remote session bridge (system #39): pairing receipt, per-capability
   policy refs, revocation receipt, public/private projection levels, remote
   approval single-use binding + tests. (openagents/apps/pylon)
5. App P1 scoped actions wired to #4 (approve/deny/cancel/instruct).
6. App P2 spawn + APNs notifications against the decision/notification spine
   (#4765).
7. App README + App Store metadata: open-source provenance, $4.99 pricing note,
   privacy (on-device token only), review notes.

## Open Decisions

1. **Connectivity beyond Tailnet?** v1 is Tailnet-first (matches `control`). A
   public relay (e.g., via the OpenAgents Cloud coordinator) is a later option,
   not v1.
2. **Bundle id / App Store account / signing** — owner action.
3. **One app for both fleets?** Keep this consumer app separate from the private
   `control` operator app; they share the bounded control contract but differ in
   audience, authority, and distribution.
4. **Free read-only tier vs fully paid?** Owner chose $4.99 paid download for
   v1; revisit a free read-only tier only if App Store positioning calls for it.
