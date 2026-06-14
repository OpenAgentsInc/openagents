# Cloud + Desktop + Mobile Coding Sessions — Full-Flow Audit

**Date:** 2026-06-14
**Author:** Autopilot Coder audit lane
**Scope:** How OpenAgents Cloud (`cloud/`), the Pylon node + new Electrobun
Autopilot Desktop app (`openagents/apps/autopilot-desktop`), and the mobile
control surfaces fit together to deliver one goal:

> **Run coding sessions not only on my desktop but also on OpenAgents Cloud
> infra (SHC or Google), and administer them from a phone so I can code on the
> go.**

This audit reconciles four sub-system audits already in this folder
(`2026-06-13-cloud-remote-execution-commercial-plan.md`,
`2026-06-13-autopilot-desktop-app-audit.md`,
`2026-06-13-autopilot-desktop-reality-vs-claim-status.md`,
`2026-06-13-autopilot-remote-control-mobile-app-audit.md`,
`2026-06-13-autopilot-clients-roadmap.md`) plus the private `cloud/` repo
(`oa-node`, `oa-workroomd`, `oa-codex-control`, CND-0xx bootstrap docs) into a
single end-to-end picture, then names exactly what is missing to make the
"code on the go" loop real.

---

## 0. TL;DR

**The two halves of this loop both exist, but they are not connected to each
other.**

- **Local execution is real.** Pylon runs bounded Codex/Claude coding sessions
  on your machine over a loopback control API (`127.0.0.1:4716`,
  `openagents.pylon.control.v0.3`). The new Electrobun **Autopilot Desktop**
  app is a thin, typed GUI client of that same control API and can spawn,
  list, cancel, and approve sessions today.

- **Cloud execution is real, but on a separate spine.** The private `cloud/`
  repo can run real Codex coding sessions on **SHC** (`oa-shc-katy-01` at
  `23.182.128.195`) and on **Google (GCE)** via `oa-node` + `oa-workroomd` +
  `oa-codex-control`, driven by an HTTP control API
  (`POST /v1/codex-runs`, contract `openagents.codex_workroom_assignment.v1`).
  This is wired to **Vortex**, not to Pylon or the desktop app.

- **Mobile is the weakest leg.** The shipped iOS `control/` app is a
  read-only Tailnet device-roster viewer (`GET /v1/snapshot`). It cannot see
  sessions, approve decisions, or spawn work. The real remote-control vision
  (React Native/Expo + bridge pairing + cursor-resumable streams) is
  audited and designed but **not scaffolded**.

**The missing middle is one unified control surface.** Today there are three
control planes that don't share a transport:

1. Pylon loopback control (`/command`, desktop + TUI clients) — local only.
2. `cloud/` `oa-codex-control` HTTP API (`/v1/codex-runs`) — Vortex client only.
3. iOS `control/` Tailnet snapshot (`/v1/snapshot`) — read-only roster only.

To "code on the go," these must collapse into **one control protocol** that a
phone, the desktop, and the web can all speak, with sessions placed on **local
Pylon, SHC, or Google** behind a single placement decision.

---

## 1. The Three Surfaces — What Actually Exists

### 1.1 Local desktop (Pylon + Electrobun Autopilot Desktop)

**Pylon** (`openagents/apps/pylon`) — published as `@openagentsinc/pylon`
(`0.2.5` stable, `0.3.0-rc2` rc). Two run modes:
- Interactive TUI dashboard (`pylon`).
- Headless node (`pylon node`) — HTTP control server on `127.0.0.1:4716`.

Pylon is the **execution authority** for local sessions: Codex adapter and
Claude Agent adapter (`@openagentsinc/claude-agent-bridge`), bounded workspace
fork, approval/decision queue, wallet state, coordinator loop, NIP-90 provider.

Control contract `openagents.pylon.control.v0.3`:
```
GET  /health                       -> { ok, schema }
GET  /events                       -> SSE node snapshot + heartbeat
POST /command                      -> session.spawn | session.list |
                                      session.events | session.cancel |
                                      approvals.resolve | coordinator.pause/resume |
                                      deploy.cloud | deploy.status | wallet.status |
                                      intent.submit | intent.list
GET  /sessions/:ref/events         -> SSE cursor-resumable per-session timeline
```

**Autopilot Desktop** (`openagents/apps/autopilot-desktop`) — new Electrobun
app (Bun main process + system-WebView Foldkit UI, ~10.5 kLOC, electrobun
`1.18.1`). Status after the 2026-06-13 fixes (CL-44…CL-59):
- Auto-discovers the local node home (`.pylon-local` / `.pylon-tailnet`,
  walking up from cwd) and reads `control-token`. The earlier "offline · 0
  sessions" bug (token-path mismatch) is **fixed** (CL-45 / #4958).
- Real sidebar + pane router; Foldkit-only webview rendering shared
  `@openagentsinc/autopilot-ui` components (CL-53 / #4966).
- Polls every 2s, renders sessions/decisions/accounts/wallet/deploy.
- Dispatches `spawnSession`, `cancelSession`, `resolveApproval`,
  `setCoordinatorPaused`, `submitIntent`, `deployCloud` as typed RPC →
  Pylon `/command`. Bearer token stays in the Bun process; webview only sees
  public-safe projections.

**Key architectural fact:** the desktop app is a *pass-through client*. It does
**not** execute, does **not** supervise the node process (P2), and does **not**
yet reach any remote or cloud node. `deployCloud` exists as a verb but routes
to Pylon's deploy gate, not to the `cloud/` coordinator.

### 1.2 OpenAgents Cloud (`cloud/`) — SHC and Google

Private Rust repo. Three binaries:

- **`oa-node`** — managed node daemon (register, capability detect, lifecycle,
  update/rollback, quarantine, settlement modes, Forge/Psionic/Probe
  attachment, broker redaction). Runs as `systemd`/`launchd` service.
- **`oa-workroomd`** — per-workroom sidecar. Owns the **Codex workroom
  runner**: assignment intake → session-scoped `CODEX_HOME` auth
  materialization → `codex exec` / `opencode run` → event normalization →
  content-addressed artifact closeout → resource-usage receipt → scrub.
- **`oa-codex-control`** — narrow HTTP control API that fronts the runner for
  async control from Vortex. Bound `0.0.0.0:8787`, bearer auth.

Control surface (`oa-codex-control`):
```
POST /v1/codex-runs                 -> queue async Codex run (202)
POST /v1/codex-runs/{id}/turns      -> continuation turn
POST /v1/codex-runs/{id}/cancel     -> cancel
GET  /v1/codex-runs/{id}            -> status
GET  /v1/codex-runs/{id}/events     -> events after cursor
GET  /v1/codex-runs/{id}/stream     -> SSE snapshot + heartbeat
POST /v1/training-runs              -> training/benchmark run
POST /v1/artanis/bootstrap         -> Artanis bootstrap run
```

Contracts: `openagents.codex_workroom_assignment.v1`,
`openagents.codex_workroom_event.v1`, `openagents.codex_auth_grant.v1`,
`openagents.resource_usage_receipt.v1`,
`openagents.compute_quota_routing.v1`.

**SHC** = self-hosted cloud. The live MVP node is **`oa-shc-katy-01`**
(`23.182.128.195`, 16 vCPU / 62 GiB / KVM present, Ubuntu 24.04, Codex CLI
`0.135.0`, Firecracker `1.15.1` bootable). Codex runs under
`danger_full_access` inside the no-wallet VM boundary because the bubblewrap
`workspace-write` sandbox fails at the loopback layer on this host
(CND-041 / CND-055).

**Google** = GCE capacity class. `oa-gcp-shc-katy-01` is deployed and
smoke-tested (CND-033…CND-035) as the **measured baseline / fallback
reference**. The commercial plan's C-5 shipped ephemeral per-session GCE VMs
(SSH metadata, firewall, labels, lifecycle, cleanup). Routing between SHC and
Google is governed by `openagents.compute_quota_routing.v1` (classes
`micro|standard|compute|gpu-standard|gpu-high`; caps 4 sessions/owner,
20/org; cost-plus-10% billing) — schema complete, **receipt comparison
(CND-042) not yet done**, so there is no live cost-driven placement yet.

**Commercial plan status** (`cloud-remote-execution-commercial-plan.md`):
foundation wave **C-0…C-8 + C-11 merged** (#4886–#4894, #4897). Remaining
**C-9, C-10, C-12–C-15** ready to parallelize: static-SSH remote-verify
prototype, Model-2 inference gateway, tenant isolation + spend caps,
settlement bridge, **C-14 = wire the `openagents-cloud` provider backend in
the Pylon client to the `cloud/` coordinator**, and C-15 isolation benchmarks.

### 1.3 Mobile / remote administration

- **Shipped:** iOS `control/` (Swift/SwiftUI, 6 files, vendored
  `Wgpui.xcframework`). Polls `GET /v1/snapshot` over Tailnet every 10s and
  renders a Tailnet device roster + optional workspace context. Bearer token in
  plaintext `AppStorage`. **No session list, no approvals, no spawn/cancel, no
  notifications.**
- **Designed, not built:** React Native/Expo app at
  `openagents/clients/mobile/AutopilotRemoteControl/` (directory empty). Spec
  in `2026-06-13-autopilot-remote-control-mobile-app-audit.md`: bridge pairing
  (QR/bootstrap → scoped `PairingCredentialClaims`), cursor-resumable streams,
  decision relay, `expo-notifications` (APNs), projection levels. Build/ship
  policy (2026-06-13): **off Expo/EAS cloud** — local `expo prebuild` →
  `xcodebuild`/`fastlane` → `xcrun altool`/ASC, JS OTA over self-hosted
  `updates.openagents.com`.
- **Web team dashboard:** `/autopilot`, `/autopilot/work`, `/decisions`,
  `/clients-preview` in `apps/openagents.com/apps/web` — currently
  fixtures-only, no live Pylon path, no team RBAC yet.

Shared spine for all clients: `@openagentsinc/autopilot-control-protocol`
(Effect Schema) defining `SessionSummary`, `SessionEvent`, `DecisionRecord`,
`bridge.*` verbs, `ProjectionLevel`, cursor dedup/resume logic.

---

## 2. How They Relate Today (and Where the Seams Break)

```
                         ┌─────────────────────────────────────────────┐
                         │            SESSION CONTROL PLANES            │
                         │            (three, not unified)              │
                         └─────────────────────────────────────────────┘

  Desktop (Electrobun) ──RPC──> Pylon /command ──> LOCAL session (Codex/Claude)
  Pylon TUI ───────────RPC────> 127.0.0.1:4716         [REAL, works today]

  Vortex (web) ────HTTP───────> oa-codex-control ──> oa-workroomd Codex runner
                                /v1/codex-runs         on SHC or GCE
                                @ 23.182.128.195:8787  [REAL, works today]

  iOS control/ ────HTTP───────> desktop node /v1/snapshot
                                (read-only roster)     [REAL but trivial]

  Expo mobile (designed) ──bridge──> Pylon bridge/* ──> sessions (any lane)
                                                         [NOT BUILT]
```

**The three breaks:**

1. **Desktop ↔ Cloud break.** The desktop app speaks `pylon.control.v0.3`
   only, to a *local* node. It has no client for `oa-codex-control`
   (`/v1/codex-runs`) and no `openagents-cloud` provider path. So a coding
   session you start in the desktop app can **only** run on your local machine.

2. **Pylon ↔ Cloud break.** The Pylon `openagents-cloud` execution-provider
   abstraction (commercial plan C-1/C-14) is specified but **not wired to the
   `cloud/` coordinator**. The cloud runner today is reachable only from
   Vortex's HTTP callbacks (`OA_VORTEX_CODEX_VM_CONTROL_URL`). Pylon cannot
   place a job on SHC or GCE.

3. **Mobile ↔ everything break.** The shipped mobile app sees a device roster,
   not sessions. The bridge transport (system #39 — pairing, capability
   scoping, cursor-resumable streams, decision relay) is **partially built in
   Pylon and not consumed by any phone client**. There is no APNs path.

**The one piece that already spans both spines** is the normalized payload
`openagents.autopilot_coding_assignment.v1` (Stack-B work-order spine), which
was *designed* to be lane-agnostic across Pylon, SHC, cloud, and hosted lanes.
But the work-order spine itself "has never executed real coding" — only a
validation-class smoke (#4633) — and is not what the desktop app drives.

---

## 3. The Target End-to-End Flow ("Code on the Go")

The flow you want, decomposed into the legs each surface must own:

```
 PHONE                    CONTROL PLANE                 EXECUTION LANE
 (Expo)                   (unified bridge)              (placement-selected)

 1. Open app  ───pair──>  bridge.pair.exchange  ──>  scoped credential
                          (QR from desktop/web)       (capabilities + expiry)

 2. New session ──spawn─> session.spawn {              placement selector picks:
    {objective,            objective, verify,           - LOCAL Pylon (free, own machine)
     verify, lane?}        lane: auto|local|            - SHC oa-shc-katy-01
                           cloud-shc|cloud-gcp}         - GCE ephemeral VM
                                                        |
 3. Watch     ──stream──> session.subscribe ─────────> normalized event stream
    progress              (cursor-resumable)            (codex_workroom_event.v1
                                                         ↔ pylon SessionEvent)

 4. Approve   ──relay───> decision.resolve ──────────> exactly-once to runner
    a decision            (single-use, replayable)      (approval queue)

 5. Get push  <──APNs───  decision_required /          notification fan-out
                          session_finished              (refs-only payload)

 6. Accept /  ──review──> work review or artifact.read  artifact closeout
    download              (public-safe projection)      (sha256/<digest>)
```

For this to work, **three contracts must converge**:

- **Transport:** one bridge that a phone can reach over Tailnet/relay, that
  brokers to either Pylon (local) or `oa-codex-control` (cloud). Either Pylon
  becomes the single front door (it proxies to cloud) or a new thin coordinator
  fronts both.
- **Event model:** `openagents.codex_workroom_event.v1` (cloud) and the Pylon
  `SessionEvent` shape must map to one normalized stream the mobile app renders
  identically regardless of lane.
- **Placement:** a `lane` selector on spawn (`auto|local|cloud-shc|cloud-gcp`)
  honoring the existing placement policy (own-Pylon-first-and-free; repo trust
  tiers: regulated→SHC-only, private→own/verified, public→any) and the cloud
  quota-routing contract.

---

## 4. Gap Analysis — What's Missing Per Leg

### Leg A — Desktop can start a cloud session
**Have:** Desktop RPC `spawnSession{adapter,objective,verify}` → Pylon
`/command`. Pylon `openagents-cloud` provider spec (C-1).
**Missing:**
- A `lane` parameter on `session.spawn` (`auto|local|cloud-shc|cloud-gcp`).
- Pylon `openagents-cloud` provider backend wired to `oa-codex-control`
  (**C-14**) — translate a Pylon spawn into `POST /v1/codex-runs` with a
  resolved `codex_auth_grant.v1`, then mirror `codex_workroom_event.v1` back
  into the Pylon session event stream.
- Desktop UI affordance to choose the lane and show "running on
  oa-shc-katy-01 / GCE" provenance.

### Leg B — Pylon ↔ Cloud coordinator transport
**Have:** `oa-codex-control` async API + job registry + Vortex callbacks. SHC
node live; GCE deploy tested.
**Missing:**
- A **coordinator/placement** layer Pylon can call that is not Vortex-specific.
  Today routing is hard-wired via `OA_VORTEX_CODEX_VM_CONTROL_URL` +
  `OA_VORTEX_CODEX_VM_RUNNER_ID`. Need a placement endpoint that accepts a
  lane-agnostic assignment and returns a runner binding (SHC vs GCE) per
  `compute_quota_routing.v1`.
- **Auth-grant brokering for Pylon-originated sessions.** The cloud runner
  resolves Codex/ChatGPT grants from Vortex
  (`OA_VORTEX_GRANT_RESOLVE_URL`). A Pylon/desktop-originated cloud session
  needs the same grant-resolution path (BYO key per commercial-plan Model 1, or
  OpenAgents credits via the Model-2 egress gateway **C-10**, not yet built).
- **CND-042 receipt comparison** so SHC↔GCE placement is cost-driven, not
  manual.

### Leg C — Mobile can administer sessions
**Have:** shared `autopilot-control-protocol` package; Pylon bridge verbs
partially built; design spec complete.
**Missing (all of it, in build terms):**
- Scaffold the Expo app at `clients/mobile/AutopilotRemoteControl/`
  (clients-roadmap M1; fast-track #4919 for TestFlight via local build).
- Finish Pylon **bridge transport (system #39)**: `bridge.pair.exchange`,
  `bridge.revoke`, `bridge.clients.list`, capability-scoped credentials,
  cursor-resumable `session.subscribe`/`history`, `decision.resolve` relay,
  `artifact.read`.
- **Remote-reachable bind** for the control server (Tailnet/LAN, not just
  loopback) — clients-roadmap M2; today desktop discovers `.pylon-tailnet` but
  the bridge auth is single all-or-nothing bearer.
- **APNs push** (`expo-notifications`) for `decision_required` /
  `session_finished|failed` with refs-only payloads.
- Mobile lane selection + provenance (same `lane` param as Leg A).

### Leg D — Administration parity & safety
**Missing:**
- Decision-queue spine that is exactly-once across desktop, web, and mobile
  (unified roadmap M5–M8 / #4763–#4766) so an approval from the phone is
  idempotent with the desktop.
- Per-session spend caps + kill switch for cloud lanes (commercial-plan C-12),
  tenant identity (WorkOS) for any non-owner use.
- Settlement: cloud `resource_usage_receipt.v1` → customer credit/invoice
  ledger in the public monorepo (C-13) — only needed once cloud minutes are
  billed; for owner dogfthat can stay `internal-accounting`.

---

## 5. Recommended Build Path (Owner-Dogfood First)

Sequenced so you reach "code on the go on cloud infra" with the **fewest new
parts**, dogfooding before any multi-tenant/billing work. This deliberately
makes Pylon the **single front door** (the desktop and phone keep speaking
`pylon.control.v0.3`; Pylon brokers to cloud) rather than teaching every client
to speak `oa-codex-control` directly.

**Phase 1 — Pylon becomes a cloud client (unblocks desktop cloud sessions).**
1. Implement the Pylon `openagents-cloud` provider backend (commercial-plan
   **C-14**): on `session.spawn{lane:"cloud-shc"|"cloud-gcp"}`, Pylon resolves
   a `codex_auth_grant.v1`, calls `oa-codex-control POST /v1/codex-runs`, and
   maps `codex_workroom_event.v1` → Pylon `SessionEvent` so the existing
   `/sessions/:ref/events` stream is lane-transparent.
2. Add a thin placement endpoint in `cloud/` (or extend `oa-codex-control`)
   that accepts a lane-agnostic assignment and binds SHC vs GCE per
   `compute_quota_routing.v1`. Until CND-042 lands, default SHC, GCE on
   explicit request.
3. Add the `lane` parameter end-to-end (control protocol → desktop RPC → UI),
   defaulting to `auto` = own-Pylon-first-and-free, cloud on overflow/opt-in.

   *Exit proof:* spawn a real repo-edit session from Autopilot Desktop that
   runs on `oa-shc-katy-01`, streams events back into the desktop timeline, and
   produces a content-addressed artifact + `resource_usage_receipt.v1`.

**Phase 2 — Remote-reachable bridge (unblocks the phone).**
4. Finish Pylon bridge transport / system #39: pairing, capability scoping,
   cursor-resumable subscribe, decision relay, revoke/clients.list, over
   Tailnet/LAN bind (clients-roadmap M2).
5. Scaffold the Expo app and ship a **read-only** build to TestFlight via the
   local build path (#4919): pair → list sessions → stream one session's
   events (works identically for local and cloud sessions because Phase 1 made
   the stream lane-transparent).

   *Exit proof:* on the phone, watch the Phase-1 SHC session stream live.

**Phase 3 — Mobile write actions + push.**
6. Add `decision.resolve`, `session.spawn{lane}`, `session.cancel`,
   `turn.steer` to the mobile app behind capability gates.
7. Wire `expo-notifications`/APNs for `decision_required` and terminal events.
8. Make the decision queue exactly-once across desktop/web/mobile (M5–M8).

   *Exit proof (the actual goal):* from the phone, away from the desk, spawn a
   coding session that runs on SHC (or GCE), get a push when it needs a
   decision, approve it, and accept the artifact — desktop shows the same
   session state throughout.

**Phase 4 — Hardening for non-owner / billed use (only when opening up).**
9. Model-2 inference gateway (C-10), tenant identity + spend caps + kill switch
   (C-12), settlement bridge cloud→public ledger (C-13), CND-042 cost-driven
   SHC↔GCE routing, microVM isolation benchmarks (C-15).

---

## 6. Key Decisions Needed From the Owner

1. **Front door:** make **Pylon the single control front door** (recommended —
   one protocol for all clients, Pylon brokers to cloud) vs. teaching the
   desktop/phone to speak `oa-codex-control` directly (more parts, two
   protocols to maintain on every client). This audit assumes Pylon-as-front-door.
2. **SHC vs Google default for owner sessions:** SHC (`oa-shc-katy-01`) is the
   live, real-account Codex lane and is free-to-us; GCE is metered. Recommend
   **SHC default, GCE on explicit request** until CND-042 receipt comparison
   makes routing cost-driven.
3. **Auth for cloud sessions:** owner dogfood can use the existing Vortex grant
   resolver (ChatGPT/Codex subscription-backed, `internal-accounting`,
   no-wallet). Confirm we reuse that path for Pylon-originated cloud sessions
   rather than building Model-2 (C-10) first.
4. **Mobile app identity:** the new Expo remote-control app is open-source and
   single-user (any Pylon user), distinct from the owner-only Swift `control/`
   app. Confirm we invest in the Expo app and let `control/` stay the
   owner-only Tailnet/WGPU console (or retire it).

---

## 7. Pointers (canonical sources for each claim)

- Local desktop: `apps/autopilot-desktop/{AGENTS.md,README.md,src/bun,src/ui}`;
  `docs/autopilot-coder/2026-06-13-autopilot-desktop-app-audit.md`;
  `2026-06-13-autopilot-desktop-reality-vs-claim-status.md`.
- Pylon control: `apps/pylon` (`pylon node`, control `v0.3`).
- Cloud spine: `cloud/{README.md,docs/ARCHITECTURE.md,docs/ISSUES.md}`;
  `cloud/docs/oa-workroomd/CODEX_WORKROOM_RUNNER.md`;
  `cloud/docs/control/CODEX_CONTROL_API.md`;
  `cloud/docs/contracts/openagents.codex_workroom_assignment.v1.md`,
  `…compute_quota_routing.v1.md`, `…resource_usage_receipt.v1.md`;
  `cloud/docs/bootstrap/CND-041-shc-katy-01-bootstrap.md`,
  `CND-055-artanis-pylon-bootstrap.md`.
- Commercial plan + cloud milestones (C-0…C-15):
  `docs/autopilot-coder/2026-06-13-cloud-remote-execution-commercial-plan.md`.
- Mobile + web: `docs/autopilot-coder/2026-06-13-autopilot-remote-control-mobile-app-audit.md`,
  `2026-06-13-autopilot-web-team-sync-audit.md`,
  `2026-06-13-autopilot-clients-roadmap.md`; shipped app `control/` (sibling
  repo); shared protocol `packages/autopilot-control-protocol`.
- Unification context (two Autopilot stacks):
  `docs/autopilot-coder/2026-06-11-autopilot-unified-audit-roadmap.md`.

---

## 8. Bottom Line

You already own both engines: a working **local** coding-session runtime
(Pylon + Electrobun desktop) and a working **cloud** coding-session runtime
(`cloud/` on SHC and Google). What is missing is the **drivetrain** that
connects them and the **steering wheel in your pocket**:

1. **C-14** — wire Pylon's `openagents-cloud` provider to `oa-codex-control`
   so a desktop/phone spawn can land on SHC or GCE (lane-transparent events).
2. **System #39 bridge** — make Pylon remotely reachable with scoped pairing
   and cursor-resumable streams.
3. **Expo remote-control app** — scaffold it, ship read-only to TestFlight,
   then add approve/spawn/cancel + APNs push.

Do those three, in that order, and "spawn a coding session from my phone, run
it on OpenAgents Cloud, approve its decisions on the go, accept the result"
becomes a real, demonstrable loop — with the desktop and cloud you've already
built doing the actual work.
