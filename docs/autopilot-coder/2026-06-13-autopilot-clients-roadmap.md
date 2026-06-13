# Autopilot Clients Roadmap — Web, Desktop, Mobile

Date: 2026-06-13
Status: implementation roadmap. Sequences the buildout of the three Autopilot
Coder client surfaces and the shared spine that unifies them, from first
local handshake through TUI parity and cloud integration. Planning doc; each
milestone's rungs are tracked as issues. No runtime invariant changes here.

## The three clients + one spine

- **Web** — Foldkit/Effect/Tailwind (`apps/openagents.com/apps/web`), exists.
- **Desktop** — Autopilot Desktop, Bun/Electrobun + Foldkit
  (`apps/autopilot-desktop`); audit
  `2026-06-13-autopilot-desktop-app-audit.md`.
- **Mobile** — Autopilot Remote Control, React Native/Expo (`clients/mobile`);
  audit `2026-06-13-autopilot-remote-control-mobile-app-audit.md`.

**Spine:** `packages/pylon-control-protocol` (Effect Schema + typed client +
cursor/dedup/decision logic). All three clients speak the same control + bridge
vocabulary (`openagents.pylon.control.v0.3` today; the system-#39 bridge verbs
next). Desktop reaches the local node over loopback; mobile reaches it over the
bridge (and, dev-only, over the host network from a simulator); web over the
existing API. One protocol, implemented and tested once.

Authority and projection rules come from the remote session bridge audit
(system #39) and the mobile/web companion audit; this roadmap is the *sequencing*
of building against them.

## Milestones at a glance

| Milestone | Theme | Headline outcome |
| --------- | ----- | ---------------- |
| **M0** | Shared spine | `packages/pylon-control-protocol` + shared fixtures, tested once |
| **M1** | Scaffolds + first handshake | Desktop *and* mobile-in-emulator both show the **same live Autopilot Coder session** running on this Mac |
| **M2** | The bridge (#39) | Proper pairing/scoped-credential/cursor-resumable transport for mobile + remote desktop |
| **M3** | Actions + TUI parity | Each client does what the TUI does today: spawn, approve, steer, cancel, accounts/quota, artifacts |
| **M4** | Cloud integration | Clients deploy/observe OpenAgents Cloud sessions (BYO-key / credits, quota/failover) |
| **M5** | Cross-client polish | Perfect interop: dedup across clients, notifications, theming parity, distribution, conformance suite |

---

## M0 — Shared spine (foundation, do first)

Everything else imports this. Build and test it once.

- **CL-0** Extract `packages/pylon-control-protocol`: Effect Schema for the
  control surface (`openagents.pylon.control.v0.3`: `session.spawn/list/events/
  cancel`, node snapshot, event frames) plus the planned bridge verbs/events
  (`bridge.pair/revoke/clients.list`, `session.subscribe/snapshot/history`,
  `turn.steer/interrupt`, `decision.resolve`, `artifact.read`, and the event
  names). Include the typed client, the cursor/sequence/dedup logic, and the
  decision exactly-once state. Repo: openagents. **Foundational.**
- **CL-1** Shared node-fixture set (health, `/command` results,
  `/sessions/:ref/events` frames, decision request/resolve/cancel) used by
  vitest across web/desktop/mobile. Repo: openagents. *Parallel after CL-0 shape.*
- **CL-2** `packages/autopilot-ui`: shared Foldkit components (session rows,
  decision cards, event timeline, status/staleness/lag chips) for web + desktop.
  Repo: openagents. *Parallel after CL-0.*

## M1 — Scaffolds + first handshake (the "get the picture" demo)

Goal: a concrete, local, on-this-Mac demo where **both new clients render the
same live Autopilot Coder session** from the shared protocol — no bridge yet,
dev-only transport.

How the local handshake works (no bridge required for the demo):

- Run a local Pylon node (`pylon node`) — control server on `127.0.0.1:4716`
  with its `control-token`.
- **Desktop** (same machine) connects over **loopback** directly.
- **Mobile in a simulator/emulator** reaches the *host* machine: iOS Simulator
  shares the host network (`localhost`/`127.0.0.1` → host), Android emulator
  uses `10.0.2.2`. So the emulator can hit the host's control server with the
  dev bearer token — enough to prove the protocol before the bridge exists.
- Start a real bounded Autopilot Coder session (e.g. the multi-session runner or
  `session.spawn`) and watch it appear, with a **live event timeline**, in both
  clients at once.

Rungs:

- **CL-3** Desktop scaffold: `apps/autopilot-desktop` Electrobun app
  (`electrobun.config.ts`, Bun main wiring the Pylon control client over
  loopback, Foldkit webview via Vite → `views://`), added to workspace globs.
  Repo: openagents. *Parallel.*
- **CL-4** Mobile scaffold: `clients/mobile` vanilla Expo app (SDK 55 / RN 0.81 /
  React 19 / React Navigation v7; MMKV, edge-to-edge, EAS, Maestro patterns
  borrowed from Ignite) consuming `packages/pylon-control-protocol`. Repo:
  openagents. *Parallel.*
- **CL-5** Desktop P0: connect to local node, render session list + live
  session-detail timeline from the shared protocol. Repo: openagents. *After
  CL-0, CL-3.*
- **CL-6** Mobile P0 (dev transport): connect from the simulator to the host
  control server (localhost / `10.0.2.2`, dev token in `expo-secure-store`),
  render session list + live timeline. Explicitly dev-only; real transport is
  M2. Repo: openagents. *After CL-0, CL-4.*
- **CL-7** **Handshake demo + doc:** a scripted local demo — start a bounded
  Autopilot Coder session; show it live in desktop and the mobile emulator
  simultaneously (same session ref, same event timeline; bonus: a decision
  request appears on both). Capture a short runbook + screenshots. Repo:
  openagents. *After CL-5, CL-6.* **This is the M1 milestone gate.**

## M2 — The remote session bridge (system #39)

Replace the dev-only loopback/host-network access with the real, secure
transport so mobile (and remote desktop) work off the local machine.

- **CL-8** Pylon Tailnet-reachable control binding with explicit refusal of
  unauthenticated non-loopback traffic. Repo: openagents/apps/pylon. *Parallel.*
- **CL-9** Bridge pairing + scoped credentials: short-lived one-time QR/bootstrap
  secret exchanged for a capability-scoped credential (issuer/audience/expiry/
  client id/device class/pairing id/`jti`/projection level); node stores hash +
  revocation, client stores the credential. + `bridge.clients.list`/`revoke`.
  Repo: openagents/apps/pylon. *After CL-0.*
- **CL-10** Cursor-resumable streams: event ids + sequences, lossless/best-effort
  tiers, duplicate-safe replay, lag caveats, bounded backpressure; atomic
  subscribe that replays pending decisions first. Repo: openagents/apps/pylon.
  *After CL-0.*
- **CL-11** Decision relay: server-originated decision requests, single-use
  exactly-once binding, cancellation/resolution broadcast (reuses the
  decision-queue spine #4765). Repo: openagents/apps/pylon. *After CL-10.*
- **CL-12** Action receipts + typed failures for every remote verb (duplicate,
  expired, cancelled, revoked, stale, unauthorized, unsupported, overloaded).
  Repo: openagents/apps/pylon. *After CL-11.*
- **CL-13** Pairing UX in the Pylon TUI/CLI (render the QR + copyable bootstrap).
  Repo: openagents/apps/pylon. *After CL-9.*
- **CL-14** Wire desktop + mobile clients onto the bridge transport (replacing
  the M1 dev transport), behind the same client interface. Repo: openagents.
  *After CL-9..CL-12, CL-5, CL-6.*

## M3 — Actions + parity with the TUI

Bring each client to functional parity with what the Pylon TUI / control surface
does today. Parity dimensions (map of TUI capability → client work):

| TUI capability today | Client target |
| -------------------- | ------------- |
| session spawn/list/events/cancel | CL-15 spawn/cancel + list/detail (all clients) |
| approvals / decision queue | CL-16 approve/deny/answer, exactly-once, read-only enforced |
| steer / interrupt / pause / resume | CL-17 bounded steering + interrupt + pause/resume as separate verbs |
| accounts list/usage, quota | CL-18 account/quota panel (incl. quota-block/failover state from #4884) |
| dev-loop check / verify status | CL-19 verify/dev-check status + required-artifact view |
| node status / go-online/offline / heartbeat | CL-20 node + provider online/offline status surface |
| artifacts / closeout / receipts | CL-21 artifact + receipt review (refs/projection-gated) |
| assignments poll/accept/progress | CL-22 work/assignment view (read-first) |
| wallet/balance/earnings (read) | CL-23 earnings/balance read-only panel (no spend authority) |

- Each rung implements its capability across the **shared protocol** first, then
  web/desktop/mobile UI. Read surfaces (CL-18..CL-23) are largely parallel; the
  effectful ones (CL-15..CL-17) depend on M2 (CL-11/CL-12) and honor read-only
  capability gating. Repo: openagents.
- **CL-24** Parity conformance checklist: a doc + test asserting each client can
  do (or explicitly, honestly cannot yet do) each TUI capability — the gate for
  claiming "parity with the TUI."

## M4 — Cloud integration

Per the cloud commercial plan
(`2026-06-13-cloud-remote-execution-commercial-plan.md`): clients become the
operator console for OpenAgents Cloud.

- **CL-25** Cloud coordinator client in the shared package (deploy/list/observe
  cloud sessions). Repo: openagents (+ cloud contracts). *After cloud C-5/C-14.*
- **CL-26** "Deploy to Cloud" from desktop + mobile (BYO-key vs OpenAgents
  credits selection), reusing the M3 spawn surface. Repo: openagents.
- **CL-27** Cloud quota/cost/failover view: surface compute metering, credit
  balance, and account-quota failover (#4884) across clients. Repo: openagents.
- **CL-28** Unified multi-origin session list: local + remote(bridge) + cloud in
  one view, with origin badges and per-origin authority. Repo: openagents.

## M5 — Cross-client polish ("all three working perfectly together")

- **CL-29** Cross-client decision consistency: if two clients answer the same
  prompt, exactly-once wins and the others show "resolved elsewhere"; cancellation
  propagates. Repo: openagents (+ pylon).
- **CL-30** Notifications: `expo-notifications` (mobile) + desktop OS
  notifications + web, all projections of the #4765 spine, refs-only payloads,
  quiet hours; notification-open routes to the right node/session/decision.
- **CL-31** Theming/UX parity: shared design tokens so web/desktop (Foldkit) and
  mobile (RN) feel consistent; status/staleness/lag chips identical in meaning.
- **CL-32** Distribution: desktop code-sign/notarize + BSDIFF auto-update feed;
  mobile EAS build + App Store submission ($4.99). Repo: openagents.
- **CL-33** **Cross-client conformance suite**: one fixture-driven test matrix
  (the CL-1 fixtures) every client runs, proving identical protocol behavior —
  cursor resume, dedup, exactly-once decisions, read-only gating, projection
  levels. The objective definition of "working perfectly with each other."

## Dependency spine (critical path)

```
CL-0 ─┬─ CL-1/CL-2 (parallel)
      ├─ CL-3 (desktop scaffold) ─ CL-5 ─┐
      ├─ CL-4 (mobile scaffold)   ─ CL-6 ─┴─ CL-7  ◀ M1 demo gate
      ├─ CL-9 ─ CL-10 ─ CL-11 ─ CL-12 ─┐
      └─ CL-8 ─────────────────────────┴─ CL-14    ◀ M2 (real transport)
                                          │
                 M3 actions/parity (CL-15..CL-24, mostly parallel after M2)
                                          │
                 M4 cloud (CL-25..CL-28, after cloud C-5/C-14)
                                          │
                 M5 polish (CL-29..CL-33)
```

Parallelizable now (fan-out friendly): CL-0 first, then CL-1/CL-2/CL-3/CL-4 and
the M2 pylon rungs CL-8/CL-9/CL-10 can run concurrently; M3 read surfaces
(CL-18..CL-23) are broadly parallel once M2 lands.

## How this connects to the existing plans

- The desktop and mobile audits define *what* each client is; this roadmap
  defines *the order* and the shared spine that makes them one system.
- M2 here is the same "remote session bridge (system #39)" the audits depend on.
- M4 consumes the cloud commercial plan's coordinator + the quota-aware routing
  (#4884) already shipped.
- M0's `packages/pylon-control-protocol` is the single most important rung — it
  is what makes "one implementation, three clients" real and is why the mobile
  app is React Native rather than Swift.

## Open decisions

1. **First-demo session content for CL-7** — simplest compelling demo is a
   bounded `session.spawn` whose live timeline + a single approval prompt render
   on both clients. (Recommended.)
2. **Workspace integration of `clients/mobile`** — path/workspace dependency on
   `packages/pylon-control-protocol` vs a locally published package; keep Metro
   scoped to `clients/mobile`.
3. **When to claim "TUI parity"** — gate on CL-24's conformance checklist, not
   vibes.
4. **Desktop pricing/channel** — still open (mobile is $4.99 App Store; desktop
   TBD per its audit).
