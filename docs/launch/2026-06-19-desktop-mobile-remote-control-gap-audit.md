# Desktop ↔ Mobile Remote Control Gap Audit

Date: 2026-06-19
Scope: `apps/autopilot-desktop/` (Electrobun desktop) vs
`clients/khala-ios/AutopilotRemoteControl/` (Expo mobile), against the shared
`@openagentsinc/autopilot-control-protocol`.
Goal: enumerate every node-steering capability the desktop has, map what the
mobile app can do today, and sequence the work to let mobile steer all the
relevant desktop capabilities on a paired Pylon node.

This is a doc-only audit. No code changed.

---

## 1. Surfaces and how they talk to a node

Both clients are projections of a local Pylon Autopilot Coder node. Node and
runtime authority lives in Pylon (`apps/pylon`); the clients render and relay,
they do not reimplement control logic.

There are **two distinct wire paths**, and this is the single most important
fact in this audit:

- **`/command` path (dev-token "type"-tagged JSON).** A flat
  `POST /command` with `{ type: "<verb>", ... }`. This is what the **desktop
  uses for everything**, and what **mobile uses for most reads and several
  writes** when it holds a dev token. Verbs are strings like `session.list`,
  `intent.submit`, `coordinator.pause`, `deploy.cloud`, `accounts.list`,
  `apple_fm.session.start`. These verbs are **not** the bridge protocol verbs;
  they are the node's local command surface.
- **Bridge path (capability-scoped, `@openagentsinc/autopilot-control-protocol`).**
  A pairing-credential transport (`bridge.pair.exchange` → scoped credential →
  `bridge.transport.*`) with its own verb set: `session.list`,
  `session.subscribe`, `session.snapshot`, `session.history`, `turn.steer`,
  `turn.interrupt`, `session.cancel`, `session.pause`, `session.resume`,
  `decision.resolve`, `artifact.read`, plus a typed event stream. Mobile uses
  this for a **narrow** capability set today (`observe_public` list/history,
  `decision.resolve`, `cancel`). The desktop does **not** use the bridge verbs
  at all.

The seam consequence: the shared protocol package is the *intended* unified
spine, but the **desktop's actual steering surface is the `/command` verb set**,
most of which has **no representation in the bridge protocol**. So "expose X to
mobile" usually means one of:

1. Mobile already can call the `/command` verb on the dev-token path → pure
   mobile-client + UI work.
2. The `/command` verb exists but mobile never wired it → mobile-client work,
   possibly a small protocol type addition for the request/response shape.
3. The action only exists as a desktop-local `/command` verb and we want it on
   the capability-scoped bridge → **protocol + node (BFF) change** to add the
   verb + a capability class, then mobile work.

Token-store, pairing, and the canonical dark palette are shared via the
protocol package; theming parity is already solved.

---

## 2. Desktop capability inventory

The desktop's Bun main process (`src/bun/pylon-control.ts`, `src/bun/index.ts`)
owns the control token, the loopback control client, polling, and the typed RPC
bridge to the Foldkit webview. Commands flow from the webview (`src/ui/commands.ts`)
through IPC handlers in `index.ts` to `pylon-control.ts`.

### 2.1 Write / steer actions (change node state)

| # | Capability | Desktop fn | `/command` verb | Notes |
|---|---|---|---|---|
| W1 | Spawn session | `spawnSession()` | `session.spawn` | adapter/objective/verify/lane/timeout, optional repoRef/worktreePath/accountRef |
| W2 | Cancel session | `cancelSession()` | `session.cancel` | |
| W3 | Resolve approval (approve/deny) | `resolveApproval()` | `approvals.resolve` | decision approve/deny |
| W4 | Submit intent ("Ask Autopilot") | `submitIntent()` | `intent.submit` | `submittedByClientRef: "desktop"` |
| W5 | Pause/resume coordinator | `setCoordinatorPaused()` | `coordinator.pause` / `coordinator.resume` | |
| W6 | Deploy to cloud | `deployToCloud()` | `deploy.cloud` | cloudrun/workers, gated by `OA_DEPLOY_ENABLE=1` |
| W7 | Spawn batch / swarm | `SpawnBatchSession` cmd → `spawnSession()` | `session.spawn` (×N) | desktop orchestrates bounded concurrency; no distinct verb |
| W8 | Composer turn (single / continuation) | `SpawnComposerTurn` cmd → `spawnSession()` | `session.spawn` | managed worktree + reply-continuation |
| W9 | Chat turn | `SpawnChatTurn` cmd → `spawnSession()` | `session.spawn` | chat pane wraps spawn |
| W10 | Start Apple FM session | `startAppleFmSession()` | `apple_fm.session.start` | bounded local AI |
| W11 | Composer Apple FM turn | `startComposerAppleFmSession()` | `apple_fm.session.start` | |
| W12 | Resolve managed worktree | `resolveManagedWorktreeRepoRef()` | (local `git ls-remote`) | no node verb; local resolution before spawn |
| W13 | Queue training closeout | `QueueTrainingCloseout` → `submitIntent()` | `intent.submit` | wraps intent |
| W14 | Add managed account | `addManagedAccount()` | (local node-config write) | CS-A1; not a control verb |
| W15 | Remove managed account | `removeManagedAccount()` | (local node-config write) | |
| W16 | Set account priority | `setManagedAccountPriority()` | (local node-config write) | |
| W17 | Plan training window | `planTrainingRunWindow()` | training API (admin token) | admin-gated, disabled by default |
| W18 | Activate training window | `activateTrainingWindow()` | training API (admin) | |
| W19 | Reconcile training window | `reconcileTrainingWindow()` | training API (admin) | |
| W20 | Claim training lease | `claimTrainingWindowLease()` | training API | |
| W21 | Request training bootstrap grant | `requestTrainingBootstrapGrant()` | training API (admin) | |
| W22 | Admit real-gradient evidence | `admitTrainingRealGradientEvidence()` | training API (admin) | |
| W23 | Build training evidence packet | `buildTrainingEvidencePacket()` | local file I/O + training API | |
| W24 | Choose identity (first-run) | `chooseIdentityHandler()` | (local seed/home selection) | AO-3 |
| W25 | Persist preferences | `savePreferences()` | (localStorage) | theme/adapter/lane/notifications |
| W26 | Surface promise gap (forum) | `surfacePromiseGapReport()` | forum API | feedback, not node control |

### 2.2 Read / projection actions (observe node state)

The desktop polls a comprehensive node snapshot every ~2s via
`fetchNodeState()`, which fans out to these `/command` reads:

| # | Read | `/command` verb |
|---|---|---|
| R1 | Sessions list | `session.list` |
| R2 | Session events tail | `session.events` |
| R3 | Session artifact stats | `session.artifact` |
| R4 | Accounts readiness | `accounts.list` |
| R5 | Deploy status | `deploy.status` |
| R6 | Intent list (asks + ship-status) | `intent.list` |
| R7 | Approvals (pending decisions) | `approvals.list` |
| R8 | Wallet status (MDK) | `wallet.status` |
| R9 | Assignments (open leases) | `assignments.poll` |
| R10 | Coordinator paused flag | `coordinator.status` |
| R11 | Apple FM readiness | `apple_fm.status` |

Plus desktop-only / derived projections with no node verb: built-in agent
readiness, install readiness, onboarding status, identity-choice state,
training-operator readiness, training runs/dashboard/gates/evidence-packet
summaries, public activity timeline, network stats, promise-surfacing
readiness, proof-replay bundle, node launch/supervisor status, control-token
probe. These are largely first-run/onboarding and public-stats surfaces, not
core node steering.

---

## 3. Mobile capability inventory (today)

Canonical control client: `src/control/control-client.ts` (note:
`controlClient.ts` is a stale stub and is **not** used). Screens live under
`app/`; shared polling + action dispatch is `src/connection/ConnectionContext.tsx`.

Screens: **Nodes** (hub: discovery, session list, asks, deploy, approvals,
wallet, accounts, assignments, coordinator toggle), **Sessions** (filterable
list), **Session detail** (event timeline polled every 4s, artifact summary,
cancel button), **Decisions** (approve/deny queue), **Spawn** (adapter,
objective, verify, lane), **Settings** (manual connect code; theme/updates are
read-only).

What mobile can do, mapped to the desktop write set:

| Desktop write | Mobile? | Mobile call / verb |
|---|---|---|
| W1 Spawn session | YES | `spawnSession()` → `session.spawn` (Spawn screen) |
| W2 Cancel session | YES | `cancelSession()` → `session.cancel` (+ `cancelSessionViaBridge`) |
| W3 Resolve approval | YES | `resolveApproval()` → `approvals.resolve` (+ `resolveDecisionViaBridge`) |
| W4 Submit intent | YES | `submitIntent()` → `intent.submit` (`submittedByClientRef: "mobile"`) |
| W5 Pause/resume coordinator | YES | `setCoordinatorPaused()` → `coordinator.pause`/`resume` |
| W6 Deploy to cloud | YES | `deployToCloud()` → `deploy.cloud` (cloudrun/main/production hard-coded on Nodes) |
| W7 Swarm / batch | NO | no batch orchestration UI or call |
| W8 Composer turn (continuation/worktree) | NO | spawn is one-shot; no managed-worktree continuation |
| W9 Chat turn | NO | only one-shot `intent.submit`; no turn-cycling / `turn.steer` |
| W10/W11 Apple FM | NO | no `apple_fm.*` calls |
| W12 Resolve managed worktree | NO | not present |
| W13 Queue training closeout | NO | not present |
| W14–W16 Account management | PARTIAL | reads accounts (`accounts.list`); cannot add/remove/set-priority |
| W17–W23 Training admin/lease/evidence | NO | not present |
| W24 Choose identity | N/A | mobile pairs to an existing node; no first-run identity creation |
| W25 Persist preferences | PARTIAL | connection only; theme/notif/updates read-only |
| W26 Surface promise gap | NO | not present |

Reads mobile has: R1 sessions, R2 session events, R3 artifact stats, R4
accounts, R5 deploy status, R6 intents, R7 approvals, R8 wallet, R9
assignments, R10 coordinator status. **Mobile does NOT poll R11 apple_fm.status.**

Two honest-scope notes on mobile reads:

- **Live streaming is built but not wired.** `src/control/session-stream.ts`,
  `session-subscription.ts`, and `src/stream/controlStreamClient.ts` implement
  the cursor/resume/dedup logic against the protocol's `session.subscribe` /
  event batch shapes, but **no screen consumes them** — every mobile screen
  polls (`session.list` on a timer, `session.events` every 4s). The plumbing for
  real-time exists; it is dark.
- **The `src/parity/` view-models are aspirational.** `accounts`, `artifacts`,
  `node-status`, `verify`, `earnings` view-models define richer projections
  (quota usage, artifact/receipt rows with digests, multi-node + provider
  health, verify command/required-artifact tones, earnings history) than the
  mobile UI currently renders. They encode the *intended* parity target, not
  shipped UI.

---

## 4. Capability matrix (desktop write capability × mobile state × seam)

State legend: **HAVE** = mobile ships it; **PARTIAL** = present but degraded /
read-only; **MISSING** = not on mobile. Seam legend: **client** = pure
mobile-client/UI work on an existing verb; **client+type** = mobile work plus a
small protocol type addition; **protocol+BFF** = needs a new bridge verb +
capability class and node support.

| Capability | Mobile | Lives in | Seam to expose |
|---|---|---|---|
| See live sessions | HAVE (polled) | shared (`session.list`) + bridge | client (upgrade poll→`session.subscribe`) |
| Approve / deny decisions | HAVE | shared (`approvals.resolve`) + bridge `decision.resolve` | client (cosmetic) |
| Pause / resume coordinator | HAVE | `/command` only (`coordinator.*`) | client (cosmetic) |
| Cancel session | HAVE | shared (`session.cancel`) + bridge | client (cosmetic) |
| Submit intent / ask | HAVE | `/command` (`intent.submit`) | client (cosmetic) |
| Spawn session | HAVE | `/command` (`session.spawn`) | client (cosmetic) |
| Deploy to cloud | HAVE (params fixed) | `/command` (`deploy.cloud`) | client (expose target/ref/env picker) |
| View session events timeline | HAVE (polled) | shared (`session.events`/bridge `history`) | client (upgrade to stream) |
| View artifacts/diffs | PARTIAL (stats only) | `/command` `session.artifact` (refs); bridge `artifact.read` | client+type (render diff via `artifact.read`) |
| Read accounts | HAVE | `/command` (`accounts.list`) | — |
| Manage accounts (add/remove/priority) | MISSING | desktop-local node-config | protocol+BFF (no remote verb exists) |
| Chat / multi-turn steer | MISSING | bridge `turn.steer` (desktop doesn't use it either) | protocol+BFF wiring + client |
| Swarm / batch spawn | MISSING | desktop-orchestrated `session.spawn`×N | client (orchestrate ×N) — no new verb |
| Composer continuation + worktree | MISSING | `/command` `session.spawn` + local git | client+type (worktree resolve is desktop-local) |
| Apple FM session | MISSING | `/command` (`apple_fm.*`) | client+type |
| Apple FM readiness read | MISSING | `/command` (`apple_fm.status`) | client (cosmetic) |
| Training admin/lease/evidence | MISSING | training API (admin) | out of scope (admin tool, not steering) |
| Surface promise gap | MISSING | forum API | client (low value for remote control) |
| Choose identity (first-run) | N/A | desktop-local | N/A (mobile pairs to existing node) |

### Counts

- **Core node-steering write capabilities** (excluding training-admin,
  identity, preferences, promise-gap — i.e. the things "steer the node" means):
  W1–W12 + account-management = **~15 distinct capabilities**.
- **Mobile HAVE (full):** 6 — spawn, cancel, approve/deny, submit intent,
  pause/resume, deploy.
- **Mobile PARTIAL:** 3 — artifacts (stats only), accounts (read-only),
  settings/preferences (connection only); plus live-view is polled not streamed.
- **Mobile MISSING (in-scope for "steer everything"):** swarm/batch, chat /
  `turn.steer`, composer continuation, Apple FM session, Apple FM readiness
  read, account management.
- **Shared-protocol-backed:** sessions list/events, cancel, approvals/decision
  resolve, artifact.read, the subscribe/stream/cursor machinery — **the
  read+decision+cancel core is genuinely shared and conformance-tested.**
- **Desktop-only `/command` verbs with no bridge representation:**
  `intent.submit`, `coordinator.pause/resume`, `coordinator.status`,
  `deploy.cloud`/`deploy.status`, `accounts.list`, `wallet.status`,
  `assignments.poll`, `apple_fm.*`, `session.spawn` (spawn exists as a
  desktop-local `ControlCommand`, not a bridge verb). Mobile reaches these only
  because it can ride the same dev-token `/command` path — **not** through the
  capability-scoped bridge.

---

## 5. Honest scope: what is genuinely missing vs cosmetic

**Genuinely missing (real capability gaps):**

1. **Chat / multi-turn steering.** Both clients lack `turn.steer`. Desktop's
   "chat" is just repeated `session.spawn`; mobile's "ask" is one-shot
   `intent.submit`. Real conversational steering of a running session is
   unbuilt everywhere. This is the biggest true protocol gap.
2. **Artifact/diff content.** Mobile shows artifact *stats* only. Viewing the
   actual diff/file content needs the bridge `artifact.read` verb (defined in
   protocol, used by neither client yet) or a `/command` content read.
3. **Live updates.** Mobile polls; the streaming stack is dark. Latency and
   battery cost are real, and "remote control" implies promptness.
4. **Account management, Apple FM, swarm/batch.** Present on desktop, absent on
   mobile.

**Cosmetic / low-effort once prioritized:**

- Deploy on mobile works but hard-codes cloudrun/main/production — exposing a
  picker is UI-only.
- Apple FM readiness read is one more poll line.
- Promise-gap surfacing and training-admin are not "remote control of a node"
  in the steering sense; treat as out of scope.

**Cleanup worth flagging:** the stale `src/control/controlClient.ts` stub on
mobile should be removed to avoid confusion with the canonical
`control-client.ts`.

---

## 6. Security seam reminder

The reason most desktop verbs reach mobile "for free" today is that mobile can
ride the **dev-token `/command` path** — a long-lived token on the wire. The
capability-scoped **bridge** (`observe_public`, `answer_decision`, `cancel`,
`send_instruction`, `pause_resume`, `read_artifact`) is the intended secure
remote path and currently only covers list/history/decision/cancel. Any plan to
"let mobile steer everything" should prefer moving each capability onto the
bridge with an explicit capability class rather than widening dev-token
exposure over the network. That is the difference between a convenient demo and
a shippable remote control.

---

## 7. Prioritized, sequenced gap-closure plan

Ordered by "what makes mobile a real remote control for steering the node
first." Each item flags the seam.

### Wave 1 — Make the existing HAVE set trustworthy and bridge-backed

The steering core already works on mobile via dev-token. Wave 1 hardens it.

- **G1. Promote spawn / intent / coordinator / deploy onto the bridge.**
  Add bridge verbs + capability classes for `session.spawn`, `intent.submit`,
  `coordinator.pause/resume`, `deploy.cloud` so remote mobile control does not
  depend on a dev token on the wire. Seam: **protocol+BFF** (new verbs +
  capabilities in the protocol package and Pylon), then thin mobile rewiring.
  Highest-leverage because it converts six "works in demo" actions into six
  "works over the network safely" actions.
- **G2. Replace polling with `session.subscribe` streaming on mobile.**
  Wire the already-built `session-subscription` / `controlStreamClient` into the
  Sessions and Session-detail screens. Seam: **client** (plumbing exists; the
  protocol verb exists). Removes 4s lag and the timer fan-out.

### Wave 2 — Close the steering-feature gaps

- **G3. Artifact / diff viewer.** Use bridge `artifact.read` to fetch and render
  diffs/file content in Session detail, not just stats. Seam: **client+type**
  (verb exists in protocol; mobile needs the call + a diff renderer; confirm
  `artifact.read` is served by the node). Promotes the PARTIAL artifacts row to
  HAVE.
- **G4. Multi-turn chat / `turn.steer`.** Wire `turn.steer` + `turn.interrupt`
  end to end so an operator can converse with / redirect a running session from
  the phone. Seam: **protocol+BFF** (verbs exist in protocol but neither node
  command path nor either client implements them — needs node support first),
  then a chat screen on mobile. This is the single most valuable *new*
  capability and the biggest lift.
- **G5. Deploy parameter picker.** Expose target/ref/env on the mobile deploy
  card. Seam: **client** (cosmetic).

### Wave 3 — Parity completeness

- **G6. Account management (add/remove/priority).** Needs a remote-capable verb;
  today it is desktop-local node-config. Seam: **protocol+BFF**. Lower priority
  for steering, higher for operations.
- **G7. Swarm / batch spawn.** Orchestrate `session.spawn`×N from mobile with a
  bounded-concurrency UI. Seam: **client** (no new verb; reuse spawn).
- **G8. Apple FM session + readiness.** Add `apple_fm.session.start` /
  `apple_fm.status`. Seam: **client+type**.
- **G9. Render the `src/parity/` view-models** (quota usage, multi-node +
  provider health, verify command/required-artifact tones, earnings history).
  Seam: **client** (view-models already exist; build the screens).
- **G10. Remove the stale `controlClient.ts` stub.** Seam: **client** (cleanup).

Out of scope for "remote control of a node": training admin (W17–W23, an
admin tool), first-run identity (W24, desktop-only by design), promise-gap
surfacing (W26, forum feedback).

---

## 8. Recommended first wave

To make mobile a real remote control fastest, ship in this order:

1. **G2 — Live streaming (client-only, plumbing already built).** Immediate,
   high-perceived-value, no protocol change. Do this first.
2. **G1 — Promote the six working steer actions onto the capability-scoped
   bridge (protocol+BFF).** Converts the demo-grade dev-token reach into a
   shippable secure remote control. This is the load-bearing protocol work and
   should start in parallel with G2 since it touches the protocol package +
   Pylon, not mobile UI.
3. **G3 — Artifact/diff viewer (client+type).** First "see what the agent
   actually did" capability beyond stats; mostly client work on an existing
   verb.

That trio gives an operator: real-time visibility, safe approve/deny +
pause/resume + cancel + spawn + intent + deploy over the network, and the
ability to inspect the diff — i.e. genuinely steering the node from the phone.
**G4 (chat / `turn.steer`)** is the marquee follow-on but requires node support
that does not exist on either client yet, so it should be scoped as its own
protocol+BFF project rather than bundled into the first wave.
