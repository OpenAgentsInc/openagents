# MASTER ROADMAP — reliable synced fleet software; OpenCode-parity Desktop

- Date: 2026-07-10
- Updated: 2026-07-10 (owner desktop/mobile reliability reset)
- Revision: 24
- Status: canonical OpenAgents implementation roadmap
- Supersedes: [`docs/fable/MASTER_ROADMAP.md`](../fable/MASTER_ROADMAP.md)
- Issue source set: [`issues/README.md`](./issues/README.md)
- Triage receipt: [`2026-07-09-issue-triage.md`](./2026-07-09-issue-triage.md)
- Desktop parity audit:
  [`2026-07-10-opencode-khala-openagents-desktop-parity-audit.md`](./2026-07-10-opencode-khala-openagents-desktop-parity-audit.md)

## Owner decisions encoded here

1. **Reliable desktop/mobile fleet software is P0 now.** The immediate product
   exit is one trustworthy OpenAgents system on Desktop and mobile: the same
   authenticated identity, conversations, FleetRuns, work units, attempts,
   approvals, commands, outcomes, and receipts continue over Khala Sync. An
   owner can start, inspect, steer, approve, pause, resume, stop, and recover a
   fleet without guessing which device or local cache owns reality.
2. **Sarah is not the required front door.** Named-persona, relationship-first,
   avatar, opener, voice, video, and A/V quality work are paused. Existing
   Sarah routes and adapters remain compatibility and regression substrate;
   they do not define the new app information architecture or acceptance
   path. A future assistant may consume the same typed application actions
   only after the direct software flows are reliable.
3. **Desktop and mobile are the active product clients.** OpenAgents web remains
   a supported public/API surface and operational dependency, but new landing,
   Forum, portal, persona, and broad route-conversion work does not preempt the
   desktop/mobile reliability burn.
4. **All retained application UI uses Effect Native.** Web, mobile, desktop,
   and canvas share typed components and intents; platform frameworks are hosts
   or renderers.
5. **Khala Sync is the cross-device authority, not a chat transport.** It owns
   durable, owner-scoped projections and mutation outcomes for conversations,
   projects/sessions, fleets, attempts, approvals, commands, and receipts.
   Device-local stores are caches/offline queues with explicit freshness and
   conflict state; they never become a second authority.
6. **Sol owns the roadmap.** Fable remains strategic source material, but its
   master roadmap and pre-reset queue are historical.
7. **Mobile and desktop are greenfield.** The new mobile app uses Effect Native
   with a React Native/Expo host at `apps/openagents-mobile`; it is named
   `OpenAgents`, uses `com.openagents.app` on iOS and Android, and copies the
   pinned current Khala Code mobile icon. The new desktop app uses Effect Native
   with Electron at `apps/openagents-desktop`. The old mobile and Electrobun
   desktop clients are deprecated extraction sources, not conversion targets.
8. **Terra is an authorized execution lane under Sol.** Sol owns priority,
   dependency order, hot-contract integration, and roadmap reconciliation.
   Terra may claim and ship ready low-collision vertical slices without waiting
   for Sol to implement every leaf. Terra's first active home is #8574 Desktop
   parity; it does not take the #8640 burn or other P0 hot contracts without an
   explicit claim handoff. The operating contract is
   [`2026-07-10-terra-execution-lane.md`](./2026-07-10-terra-execution-lane.md).
9. **OpenAgents Desktop reaches practical OpenCode Desktop parity.** This is a
   core product exit, not optional polish and not a literal code/pixel clone.
   The benchmark is the current OpenCode desktop workbench: project/session
   navigation, streamed agent work, rich composer/context, file/editor/review/
   terminal loop, commands/keybindings, providers/models/MCP/permissions,
   settings, diagnostics, lifecycle, and distribution. OpenAgents keeps its
   existing Effect Native application grammar, hardened Electron boundary,
   Khala Sync continuity, and Pylon/Fleet authority. Every material desktop
   action has a stable typed intent and outcome shared with mobile where that
   action is appropriate. Multi-window depth and WSL may follow the first
   complete workbench; they do not dilute the required everyday parity scope.
10. **Reliability is measured through cross-device receipts.** Green unit tests
    or a single-device demo are necessary but insufficient. The P0 exit must
    prove restart, reconnect, offline catch-up, duplicate suppression, explicit
    conflict/refetch behavior, and one real desktop-to-mobile handoff while a
    mixed coding fleet is active.

## The product in one sentence

**OpenAgents is reliable software for doing coding work and managing fleets of
agents from Desktop and mobile without forking identity, state, authority, or
receipts.**

Desktop is the complete coding workbench and fleet cockpit. Mobile is a
purpose-built supervision and continuity client, not a miniature editor. Both
use the same typed state, control, policy, Sync, and receipt contracts.

Khala is the inference, routing, and Sync engine. Pylon and Agent Computers are
execution. Blueprint is legible memory and work state. Effect Native is the
shared application grammar. Receipts are completion truth.

## One authority loop, two active clients

```text
intent -> policy -> orchestration -> execution
  ^                                  |
  |                                  v
state <- Sync <- evidence <- durable outcome

       OpenAgents mobile  <---- Khala Sync ---->  OpenAgents Desktop
       supervision + handoff                    workbench + fleet cockpit
               \_____ same identity, authority, and receipts _____/
```

| Layer | Canonical responsibility |
| --- | --- |
| Identity | Authenticated owner, organization, device, account, and capability scopes |
| Comprehension | Khala inference, typed tools, semantic selectors, and Blueprint drafts |
| Control | Owner scope, policy, budget, approval posture, and typed intents |
| Orchestration | Fleet planning, routing, claims, Pylon, and harness selection |
| Execution | Codex/Claude/Grok workers on owner-local Pylons or Agent Computers |
| Evidence | Verification, exact or explicitly unmeasured usage, and closeout receipts |
| Continuity | Khala Sync, offline/catch-up protocol, Blueprint, provenance-bearing memory, and resumable work |

Desktop owns the deep workbench affordances: projects/sessions, rich agent
timeline and composer, files/editor, diff/review, terminal, commands, accounts,
settings, diagnostics, and release lifecycle. Mobile owns glanceable fleet and
work status, attention/approval queues, steering, stop/recovery, receipts, and
handoff. Device capabilities differ; durable identity, run state, command
outcomes, policy, and evidence never do. Web services may host authority and
public/API surfaces, but web product expansion is not on the active P0 path.

### Executive pause boundary — effective 2026-07-10

Pause new work on:

- Sarah-specific front-door, persona, relationship, role-program, and named-
  colleague UX;
- avatar, opener, pre-rendered clips, voice, ASR/VAD, realtime video, media
  admission, semantic media cache, and presentation-quality experiments;
- Liquid Glass or visual polish that is not required for accessibility,
  interaction correctness, platform support, or a reliability gate;
- new web marketing, Forum, portal, and route-conversion scope that does not
  unblock identity, Sync, Fleet authority, release, or incident recovery.

Do not delete proven contracts or break production consumers merely to express
the pause. Security, privacy, data-loss, production-outage, and compatibility
repairs remain allowed. Existing named-assistant endpoints may be used as a
temporary adapter when they are the only landed route, but new client state and
acceptance tests must target persona-neutral typed contracts. Reactivation
requires a later owner decision after the P0 reliability exit.

## Current implementation truth

The coding-fleet program starts from substantial working substrate. This list
records landed truth, including components whose further product work is now
paused; inclusion here is not active priority:

- typed `FleetRun`, work planner, plan DAG, and claim registry;
- one typed fleet intent vocabulary for worker selection, pause/resume/drain/
  stop, approvals, and steering;
- Codex, Claude, and Grok chat plus worker adapters;
- harness-conformance fixtures and honest usage/failure classes;
- a Pylon-owned mixed-kind supervisor and manager with zero duplicate claims;
- a canonical Pylon-home `orchestration.sqlite` runtime whose runs and claims
  survive reopen, plus typed interrupted-executor recovery;
- bounded durable work-source descriptors and a Pylon-owned planner that can
  reconstruct fixture, pinned-issue, pinned-backlog, and pinned-plan-DAG work
  after restart;
- a real-account capacity mapper that preserves Codex/Claude/Grok worker kind,
  advertised slots, readiness, and honest marginal-cost class without silently
  substituting another harness;
- a Pylon-owned named-account capacity authority over strict local registry,
  readiness, health, quota, usage, dispatch-breaker, and durable cross-run load
  state; Grok custody is restricted to one exact, isolated, Pylon-owned named
  home, while default homes, duplicate refs, global credentials, and unsafe
  config sources fail closed with fixed diagnostics;
- a Pylon-owned exact claimed-work runner that converts one durable claim into
  one exact named Codex/Claude Khala assignment, verifies the delegation,
  account hash, strict assignment ref, and no-spend closeout, suppresses
  duplicate dispatch, and reconciles restart state by inspection rather than
  rerunning;
- a direct exact named-Grok claimed-work adapter over the real Grok harness,
  with pinned checkout and argv verification, bounded execution, one canonical
  claim registry, durable refs-only receipts, restart reconciliation, no
  provider substitution, and honestly `not_measured` usage;
- production Cloud SQL/Postgres FleetRun intake authority with owner-scoped run
  and work-unit rows, canonical fingerprints/idempotency conflicts, pinned
  public-repository validation, active owner-linked Pylon intake leases, exact
  claim acceptance, and an owner-safe Sync draft projection;
- the actual authenticated Sarah `coding_fleet_start` tool and
  `POST|GET /api/sarah/fleet-runs` production adapter: human owner and
  relationship policy are server-derived, Postgres is mandatory, response
  envelopes are exact-schema decoded, hostile/private material is rejected,
  and refreshed OpenAuth cookies propagate back to the browser;
- a strict Pylon remote-intake seam that durably imports the exact server run,
  work, verifier, worker, and target pins into the one local registry before
  accepting the server claim, replays acceptance after restart, replaces only
  an exactly expired lease, and activates only through node `arm`/`status`;
- registered-Pylon bearer claim/accept routes over the Postgres authority plus
  a strict HTTPS-or-loopback client and serialized standing-node poller: owner
  and Pylon authority are server-derived, claim idempotency survives dropped
  responses and local import failure, malformed/private material fails closed,
  and a refs-only loopback status makes unattended intake observable;
- a Pylon-only standing activation seam that recovers stale work before it
  idempotently resumes and refills an existing durable run;
- one canonical owned standing-executor composition that opens one Pylon-home
  runtime/store, constructs planner, named capacity, liveness, exact runner,
  recovery, and refill against that same store, and closes the runtime on any
  construction/recovery/resume failure;
- a concrete assignment/process/heartbeat liveness adapter that distinguishes
  live, dead, and unknown recovery evidence without treating PID presence as
  sufficient;
- Khala Sync fleet projections and steering mutators;
- caller-owned Khala→Pylon assignments, exact token rows, private event
  archives, and closeout proofs;
- a headless Pylon node with account registry, presence, assignment polling,
  session execution, and a local coordinator, plus explicit owner-local
  `arm`/`status`/`disarm` control for one already-known durable FleetRun and
  fail-closed resume after node restart;
- Sarah's owned runtime, authenticated relationship, SSE bus, Blueprint Map,
  Actions, and Code/Receipts panels;
- Sarah owner-safe FleetRun, continuity/stall, and six-section coding-closeout
  schemas plus tested Effect Native supervision and receipt views;
- a same-origin exact-scope Sync cursor client, strict bounded entity reducer,
  serializable tombstone/version state, and exact-cursor reopen seam, with the
  Fleet view composed into Sarah only when an explicit owner projection exists;
- an exact-scope browser persistence and `/api/sync/connect` controller with
  serial delta application, abort/dispose, bounded reconnect, stale watchdog,
  cursor-gap catch-up, and first-class MustRefetch reasons, now instantiated by
  retained `/sarah` only when one strict `fleet_run` URL ref derives the scope;
- retained `/sarah` loading/live/reconnecting/MustRefetch/failed/stopped Fleet
  rendering plus exact run controls, approval decisions, local evidence/
  receipt navigation, and a bounded per-page idempotent command ledger; no
  scope preserves the Blueprint-first surface;
- a typed media-admission projection in which text is the floor, realtime
  queues expire to text after 30 seconds, LIVE requires admission and transport
  leases, and cost/recovery inputs remain explicit;
- browser media truth in which LIVE additionally requires a fresh decoded
  frame on a live track, projection is cadence-bounded, start/stop are bounded
  and generation-fenced, every successful mint has one coalesced authoritative
  server cleanup, and cleanup pending/unconfirmed blocks replacement without
  removing text or an exact-scope Fleet surface;
- a bounded per-conversation VAD coalescer plus exact prospect/conversation/
  turn streaming coordinator and OpenAI-SSE adapter with immediate role bytes,
  keepalives, one canonical producer/publish-and-record attempt, bounded replay,
  disconnect isolation, abort propagation, and honest overflow/record-timeout
  outcomes; the current renderer-wide bearer route remains byte-compatible and
  deliberately does not arm this trusted-context entrypoint.
- a production Pipecat 1.5.0 seam audit that rejects a Python foundation swap
  while selecting smart-turn, VAD-segmented owned ASR, qualified interruption,
  TTFA/TTFB, task-lifecycle, and behavioral-eval patterns for one bounded,
  process-isolated experiment under #8610.
- the greenfield OpenAgents mobile shell on the Effect Native/React Native seam,
  exact `OpenAgents` / `com.openagents.app` / pinned-icon identity oracles, and
  an owned `openagents-production` OTA channel. TestFlight build 113 closed the
  text-first Sarah compatibility slice. Build 114 at `23aba8270a` is App Store
  Connect `VALID` and adds a separate generic Khala chat mode, but it regressed
  the intended native glass chrome by omitting the local SwiftUI module. Build
  115 source at `ee78dc1a2e` restores that module; typecheck and 40 tests pass,
  archive/export completed, and App Store Connect reports `VALID`. Physical-
  device visual acceptance remains open.
  The app-owned five-thread catalog, Sarah adapter, and generic Khala mode do
  **not** provide authenticated Sync, Fleet/account authority, or cross-device
  receipts. Demo prices/video are presentation-only and paused; no StoreKit
  purchase is implemented;
- the Effect Native v30 host/lowering seam through `5202a2665a`: the Scope-
  bound render-rn host driver and internal `@expo/ui` lowering landed, and the
  then-current D-MB-02 app-local island was deleted. Build 115 later restored
  an application-local `openagents-liquid-glass` SwiftUI module to correct the
  build-114 visual regression. Treat that module as a bounded current host
  exception, not proof of final shared lowering; further visual migration is
  paused except where it blocks R0–R7 correctness or platform support;
- the greenfield OpenAgents Desktop line through `f4cb8ed18e`: the pinned
  electron-shadcn scaffold and Electron sandbox/isolation boundary now host a
  minimal Effect Native conversation workspace, a bounded host-owned five-
  thread store, a host-held model-gateway bridge with honest configuration
  failure, shared typed icons and glass backdrop/material lowering, and the
  first real workspace slice with user-selected root, bounded root listing, and
  bounded read-only file preview. A dedicated Settings surface reads bounded
  Codex readiness and drives Pylon's isolated device-auth flow. The current
  reconciled Terra receipt has 58 passing tests, green typecheck, and a passing
  real-Electron smoke; the smoke proves a system/error chat response and
  scripted awaiting-browser account state, not live model completion or real
  owner authentication. The pinned parity audit at `84648bd03c` scores the
  destination at 1 landed, 6 partial, 3 scaffold, and 10 absent capability
  areas, versus 8 landed, 10 partial, 1 scaffold, and 1 absent in the frozen
  Khala Code extraction source. That difference is now an explicit #8574
  product burn: authoritative Sync conversation, command/project/session
  infrastructure, rich composer, edit/review/terminal, provider/MCP/settings,
  server-authoritative Fleet/approval/receipt projection, diagnostics,
  packaging/signing/updates, identity acceptance, and legacy-client retirement
  remain open;
- product-promise source registry `2026-07-09.2` at `55452fa614`, the
  fix-forward #8644 provenance correction that binds the two owner-scoped
  FleetRun intake routes as intake evidence only without changing promise state
  or implying closeout, payment, settlement, or multi-earning availability.
- accepted-claim steering exchange at `e0b0fdc617`: registered owner/Pylon
  delivery, strict-prefix bounded pages, durable delivery reservation,
  content-bound body-free ordered outcomes, and lost-ACK replay now join a
  standing Pylon consumer with an atomic local watermark/outbox, exact
  work-claim/assignment targeting, restart-safe composition, and ACK
  backpressure;
- reconnect-honest command outcomes at `2a3fc0dfaf`/`08aac90250`: the client
  no longer manufactures effective run/approval/steer state, durable
  `fleet_command_outcome` rows distinguish delivery from completion, accepted-
  lease authorization is row-locked, schema coherence is strict, and each run
  is capped at 1,024 intents within Sarah's bounded cache;
- the follow-up/completion stack through `59538f71a2`: approval, private steer,
  and active stop are oldest-first and restart-safe; leases are generation+
  token fenced; control calls carry stable content-bound idempotency; terminal
  completions are body-free, authenticated, ordered, exact-replay idempotent,
  and reconnect-visible; failed/stale completions cannot claim effective
  state; and execution/steering request streams are byte-bounded even without
  `Content-Length`. When the unattended executor has no typed
  `approval_requested` event, approval remains honestly unavailable rather
  than receiving a fabricated binding.
- first-class server work-unit/attempt authority through `849856d189`: every
  normalized plan unit is durable before execution, every claim retry has a
  stable exact attempt identity, only v2 attempts with coherent verifier,
  artifact, proof, authority, closeout, economics, and exact-or-explicitly-
  unmeasured usage evidence can succeed, and server receipt time—not a remote
  worker clock—governs freshness. Historic v1 rows remain replayable but never
  manufacture success; migration 0056 repairs unsafe legacy projection refs to
  opaque digests, terminalizes stranded unsafe running attempts for safe retry,
  and leaves the immutable execution ledger as audit authority.
- exact worker-closeout evidence through `dd807e6d91`: Pylon submits explicit
  payment/settlement/payout policy, strict Worker intake rejects incoherent,
  excess, private, duplicate, or lossy evidence, and owner-only proof/status
  retains verifier, test, artifact, proof, authority, result, and closeout refs
  exactly. Legacy policy remains unknown rather than inferred, malformed legacy
  evidence is explicitly unavailable, OpenAPI matches runtime, and malformed
  remote evidence becomes typed blockers rather than exceptions.
- Sarah's strict Sync consumer through `fe7b523e13`: direct `fleet_work_unit`
  and `fleet_attempt` rows survive bootstrap, delta, tombstone, persistence,
  replay, and exact-cursor reconnect; nested evidence/economics/privacy fails
  closed; read-only collections use canonical work-unit/attempt keys. The owner
  projection intentionally still ignores these retained rows until its next
  serialized slice, so no assignment-backed UI claim is implied.
- exact approval authority through `05638b0320`: execution v2 admits a bounded
  `approval_requested` only for an existing active attempt; the owner-safe
  projection binds run, work unit, attempt, effective assignment/account edge,
  worker, request event, and tool class on the server receipt clock. Legacy
  worker-only approvals remain visible but non-actionable. Migration 0057 adds
  the event kind and indexed global ref lookup; sorted pre-locking prevents
  reverse-order cross-run deadlocks. Migrations 0054–0057 were later applied in
  staging and production as part of the #8639 closure;
- FC-3 closure at `1d84386cb5`: one integrated real-Postgres fixture proves a
  Sarah-started three-work-unit run, pause/resume, exact-attempt steer, exact
  blocked approval, coherent closeouts, privacy-safe reconnect, and durable
  command receipts. Staging `00047-ct5` and production `00069-h2k` carry the
  minimum-safe supervision stack, and #8639 is closed.

The landed runtime still has one live mixed-account proof gap, but the immediate
product gaps are the R0–R7 Desktop/mobile continuity and reliability gates:

- C1 is crossed. #8637, #8633, and #8639 are closed; the minimum-safe Sarah →
  standing Pylon → exact supervision/reconnect path is fixture-proven and
  deployed.
- #8640 Phase A is the next owner-account runtime proof, but an owner-gated
  reconnect does not block R0–R2 client work. It requires simultaneous useful
  Codex + Claude work through named isolated homes. Its recorded blockers are
  the clean API ambient-type gate, a credential-scanner correction that
  preserves the no-long-lived-SCM-credentials invariant, and owner
  reauthentication of one isolated Codex home without touching default
  `~/.codex`. Desktop Settings exposes the isolated Pylon device-auth flow.
- Grok is postponed by owner decision because the connected account is
  quota/payment exhausted. Its real accepted historical canary, HTTP-402
  state, adapters, and fixtures remain evidence/regression substrate; Grok is
  not a Phase A acceptance item.
- Agent Computer rootfs and in-VM exact-receipt source work has advanced under
  #8547, but its real brokered owner-account Firecracker turn remains open.
  Hybrid owner-local plus managed-cloud acceptance belongs to #8636 and never
  blocks the local cutover.
- The trusted-context voice coordinator/SSE adapter remains fixture-proven and
  paused. Model/system text is not scope authority.

P0 closes the live runtime receipt while projecting the existing Fleet system
into reliable Desktop/mobile software; it does not build another fleet system.

### Current P0 integration ledger

| Lane | Narrowest proven state on `main` | Next blocking proof |
| --- | --- | --- |
| #8637 FC-1 | **closed** at `0892d57b3b`: integrated operator conversation → durable authority → registered standing Pylon → bounded closeout is fixture-proven; timing is 1.8s acknowledgment / 6.1s first claim / 8.6s first capacity; staging `00046-jpn` and production `00068-5t8` are deployed and smoke-proven | none; live owner-account execution is #8640 |
| #8633 FC-2 | **closed** on the stack ending `d779c360c3`: production standing adapters, shared typed auto policy, restart-safe mixed claims, health rotation, durable execution outbox/server projection, and one integrated three-harness restart/usage fixture are proven | none; useful live account work is #8640, not reopened FC-2 residue |
| #8639 FC-3 | **closed** at `1d84386cb5`: exact run/work/attempt authority, durable control outcomes, approval/steer binding, full evidence canvas, privacy-safe reconnect, migrations 0054–0057, and the integrated C1 fixture are proven; production `00069-h2k` deployed the stack | none; the live owner-account rung is #8640 |
| #8640 FC-5 | C1 crossed; Grok historical canary accepted; a Claude canary failed closed before verification; current exit is owner-approved Codex+Claude Phase A | land the strict scanner/type-boundary repairs, reauthenticate one isolated Codex home, then run one clean simultaneous Codex+Claude receipt |

These rows are implementation receipts, not issue closure. The commit named in
one row does not imply deployment or live proof, and later commits inherit the
landed substrate only when their relevant verification stays green.

## P0 program — reliable Desktop/mobile fleet control

The active program composes **[#8566](https://github.com/OpenAgentsInc/openagents/issues/8566)**,
**[#8574](https://github.com/OpenAgentsInc/openagents/issues/8574)**,
**[#8597](https://github.com/OpenAgentsInc/openagents/issues/8597)**, and the
landed Fleet Command substrate under
**[#8638](https://github.com/OpenAgentsInc/openagents/issues/8638)**. Issue
#8640 supplies the real mixed-account burn. This roadmap priority is effective
immediately even where live issue labels or older issue prose still say
`P1 parallel` or require Sarah as the initiating surface; reconcile those
records before claiming the first new slice.

### Starting gap, stated plainly

- Desktop has a hardened Electron/Effect Native shell, local five-thread chat,
  partial project/file access, and provider readiness/device auth. It does not
  yet have authoritative Sync threads, a complete coding workbench, or a
  visible server-authoritative Fleet cockpit.
- Mobile has an Effect Native/React Native shell, a persisted local five-thread
  catalog, a production Sarah compatibility path, and a separate generic Khala
  chat mode. It does not yet have authenticated cross-device Sync, FleetRun
  authority, account/capacity state, durable controls, or receipt continuity.
- The backend has considerably stronger FleetRun, claim, command, attempt,
  outcome, and receipt authority than either app exposes. The shortest path is
  to project and mutate that existing authority through Khala Sync, not to
  invent new local fleet models or infer success from transcript text.

The evidence baseline is maintained in
[`docs/terra/CURRENT_STATE.md`](../terra/CURRENT_STATE.md),
[`docs/terra/MOBILE_PARITY.md`](../terra/MOBILE_PARITY.md), and the
[`desktop parity audit`](./2026-07-10-opencode-khala-openagents-desktop-parity-audit.md).

### Cross-device reliability gates

| Gate | Required scope | Exit evidence |
| --- | --- | --- |
| R0 — truthful green foundation | Desktop and mobile typecheck/tests/builds; isolated first-run state; capability manifests; explicit `unconfigured`, `offline`, `reconnecting`, `stale`, `must_refetch`, `failed`, and `ready` states; remove or finish fake/dormant Fleet affordances | Both clients are green from clean state and no fixture, local cache, transcript, or optimistic toast is presented as authority |
| R1 — shared identity and session | One authenticated owner/org identity; device registration; scoped provider/account readiness; stable conversation/project/session refs; safe token refresh/revocation; persona-neutral app/session bootstrap | Sign in on Desktop and mobile, see the same authorized account/session catalog, revoke either device, and prove no credential or private payload crosses renderer/public evidence boundaries |
| R2 — Khala Sync continuity | Canonical projections for conversations, projects/sessions, FleetRuns, work units, attempts, workers/accounts, approvals, commands, outcomes, and receipts; monotonic cursor/version; tombstones; bounded cache; mutation idempotency keys; explicit conflict/refetch semantics | Create or change durable state on one device, observe it on the other with matching refs/versions, restart both, and reconstruct the same current state without duplicate objects or invented completion |
| R3 — fleet operations | Start from pinned work; choose named worker/account/capacity; inspect plan/claim/assignment/attempt; steer, approve/reject, pause/resume/drain/stop; surface unavailable/quota/policy states; show exact or `not_measured` usage and verification/closeout | One real Codex+Claude run is started and managed with controls from both clients; every command has one durable accepted/rejected/failed outcome and zero duplicate claims or silent provider substitution |
| R4 — interruption and recovery | Offline mutation queue with bounded eligibility; foreground/background transitions; dropped acknowledgements; replay deduplication; out-of-order/duplicate events; stale leases; server restart; device restart; explicit merge/refetch rules | Fault-injection suite plus a real handoff prove no lost accepted intent, double execution, false LIVE/success state, or indefinite spinner; stale clients converge or fail closed |
| R5 — Desktop everyday workbench | Complete D0–D6 below: streamed sessions, composer/context, projects/files/editor, Git review, bounded terminal, commands/keybindings, runtimes/models/MCP/permissions, settings/diagnostics, Fleet cockpit, lifecycle/distribution | The practical OpenCode-parity workflow completes through the hardened Effect Native/Electron app and survives restart/reconnect while retaining authoritative Sync/Fleet state |
| R6 — mobile supervision | Activity/fleet home, recent work, attention queue, run/work/attempt detail, worker/account readiness, steer/approve/pause/resume/stop, outcome/receipt inspection, deep link/handoff, accessible loading/error/offline states | On a physical phone, supervise the R3 run end-to-end and hand deep work to Desktop; mobile never pretends to offer unsupported editor/terminal capability |
| R7 — release and dogfood | Signed/recoverable Desktop release lane; iOS and Android build/install proof; schema compatibility window; migration/rollback; public-safe diagnostics; telemetry for sync lag, command latency, reconnect, conflicts, and duplicate suppression | A sustained owner dogfood window includes real cross-device fleet work, upgrade/restart/offline faults, no P0/P1 data-loss or false-authority defect, and a signed owner-accepted receipt |

### Khala Sync laws for this program

- Server/Pylon authority decides claims, attempts, worker custody, approvals,
  command acceptance, and terminal outcomes. Khala Sync distributes typed
  projections and mutation results; it does not promote cache state into
  execution authority.
- Every mutation carries owner scope, stable target ref, expected version or
  explicit commutative semantics, idempotency key, and a durable typed outcome.
  A transport timeout is `unknown_pending_reconcile`, never success or an
  automatic unsafe replay.
- Reconnect uses ordered cursors plus bounded gap detection. A missing history
  window produces `must_refetch`; it is never papered over with transcript
  inference or last-write-wins on authority-bearing fields.
- Device-local optimistic state is visually distinct and reversible. Approval,
  stop, payment, claim, credential, and destructive actions are not displayed
  as committed before authoritative acknowledgment.
- Desktop and mobile consume one shared Effect Native domain/component/intent
  vocabulary. Platform hosts may expose different capabilities, but do not
  fork identifiers, enums, error classes, sync semantics, or outcome grammar.

## Proof status is six distinct rungs

Every roadmap claim uses the narrowest true state:

1. **code-landed** — source is on `main`;
2. **fixture-proven** — bounded deterministic tests/fixtures pass;
3. **deployed** — the intended artifact/config is verifiably present in the
   target environment;
4. **live-proven** — a real production path produced the named receipt;
5. **owner-accepted** — the owner reviewed the live behavior and accepted it;
6. **closed** — issue exit, residuals, docs, and duplicate-path deletion are
   reconciled.

No rung implies the next. In particular, Blueprint code and fixtures are not
described as live/accepted merely because the surface exists in source. #8640
must report the rung of every cutover criterion.

## Interim parallelism and the direct-software cutover

In this current Codex app runtime, the root agent plus up to three concurrently
active subagents are available—four active agents total. This is the surfaced
cap for this session, not a permanent Codex-wide limit. Keep the root as
coordinator/integration owner; give every mutating lane its own clean worktree
and bounded issue/file scope. Serialize shared schemas, migrations, generated
catalogs, lockfiles, central route tables, and other hot files through one
lane. Use separate Codex tabs beyond this session cap or for independently
steered long-lived contexts; tabs on one account share its quota budget.
Same-session subagents are Codex agents; they do not exercise the connected
Claude or Grok accounts. Closed #8633 proves the production three-harness path
in code and fixtures. C1 is crossed; the remaining fleet-runtime proof is the
owner-approved Codex+Claude Phase A receipt, while R0–R7 now define the product
cutover.

Terra supplies additional cross-session implementation throughput under
[`2026-07-10-terra-execution-lane.md`](./2026-07-10-terra-execution-lane.md).
It may pull ready claimed P1 leaves and push them directly to `main` while Sol
owns the serial P0 burn and roadmap integration. Terra's active #8574 Desktop
claim is the first such lane. This authorization does not erase claims or grant
implicit ownership of schemas, migrations, catalogs, lockfiles, route tables,
authority policy, or other hot contracts.

The coding cutover is staged:

| Gate | When | Operating decision |
| --- | --- | --- |
| C0 | complete | FC-1/FC-2 implementation and the minimum-safe FC-3 seam were built through this Codex app and coordinated lanes. |
| C1 | **crossed** — #8637, #8633, and #8639 are closed; the exact command/reconnect fixture is deployed through production `00069-h2k`. | Low-risk runtime canaries may run through an existing compatibility adapter; this app remains coordinator, independent verifier, and break-glass while failures are repaired in place. |
| C2 | #8640 Phase A clean simultaneous Codex+Claude receipt from one pinned integrated deployment | The mixed-account Fleet runtime is accepted as substrate. This does **not** make a Sarah route the product front door and does not by itself complete the R0–R7 client cutover. |
| C3 | #8547/#8636 exits integrated with one clean owner-local plus managed-cloud receipt | Direct software controls may choose owner-local or managed-cloud capacity through the same typed run contract. |

C2 remains the runtime-unblock point. R7 is now the owner-facing software
cutover. Neither waits for Sarah presentation work, public-route contraction,
or other paused scope. Historical C2 criteria and fallback rules are in
[`2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md`](./2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md).

## P0 substrate — Fleet Command

Epic: **[#8638](https://github.com/OpenAgentsInc/openagents/issues/8638)**.

### P0.1 — durable run authority (closed substrate)

**[#8637 FC-1](https://github.com/OpenAgentsInc/openagents/issues/8637)**
landed the durable run request through the then-current authenticated Sarah
adapter. That adapter remains supported substrate, not the required Desktop or
mobile information architecture.

Status: **closed** on `0892d57b3b`, deployed staging then production. The
[closure receipt](https://github.com/OpenAgentsInc/openagents/issues/8637#issuecomment-4930867072)
records every proof rung and assigns the real mixed-account/live-control work to
#8640/#8639 rather than holding this contract lane open.

Exit:

- owner-authenticated request through the landed `/sarah` compatibility route;
- pinned public repository/work plan and bounded verifier;
- one durable owner-scoped `runRef` with idempotency;
- Pylon can claim it without a supervising CLI process;
- typed `prospect | customer | operator | administrator` relationship mode
  selects policy-owned tool/retrieval/posture/UI behavior; the model cannot
  select or upgrade it, and operator coding posture contains no sales flow;
- acknowledgment plus durable `runRef` p95 <= 5 seconds and first capacity/
  claim state p95 <= 15 seconds, otherwise an explicit typed delay/blocker;
- no raw prompts, shell output, paths, or credentials in any client projection.

### P0.2 — run a real mixed local fleet

**[#8633 FC-2](https://github.com/OpenAgentsInc/openagents/issues/8633)**
wires the real Pylon supervisor across Codex, Claude, and Grok.

Status: **closed at the code/fixture boundary** on the implementation stack
ending `d779c360c3`. The integrated receipt proves one accepted fleet run,
three concrete harness ports, restart without duplicate claims or redispatch,
durable server terminal projection, Codex/Claude exact evidence, and Grok
`not_measured` evidence. The useful real-account burn is #8640, not an implied
FC-2 live rung.

Exit:

- durable Pylon-home FleetRun store;
- one real mixed account/capacity projection;
- one production runner path for all three harnesses;
- typed `auto` policy on live accounts;
- standing refill up to advertised capacity, no manual background shells;
- three simultaneous production-adapter streams in the integrated fixture,
  one per harness, zero double claims;
- authenticated execution events and terminal closeouts survive offline
  delivery and one restart without synthetic proof.

P0.2 now consumes the closed P0.1 contract. Any schema change remains serialized
through the narrow shared package.

### P0.3 — durable fleet controls (closed substrate)

**[#8639 FC-3](https://github.com/OpenAgentsInc/openagents/issues/8639)**
connects durable progress and fleet intents to the landed compatibility
surface. R2, R3, R4, and R6 own their neutral Desktop/mobile projection.

Status: **closed** at `1d84386cb5`, fixture-proven and deployed. The integrated
C1 receipt proves three named work units, exact attempt/worker authority,
pause/resume, one exact steer, one exact approval, coherent verification and
closeout, privacy-safe reconnect, and durable command outcomes. Migrations
0054–0057 are applied; production `00069-h2k` carries the stack. The real
owner-account Codex+Claude work remains #8640 rather than an FC-3 residual.

Exit:

- named work streams visible in an authorized client and Blueprint canvas;
- pause/resume/drain/stop, steer, and approval actions;
- evidence-backed plan→claim→assignment→verification→closeout edges;
- reconnect reconstructs current authoritative state;
- first executor progress/blocker p95 <= 30 seconds, progress freshness at
  least every 15 seconds, and 30 seconds without freshness becomes typed
  `stalled`/`reconnecting`, never indefinite live;
- the first coding closeout card passes the one-minute comprehension grammar:
  outcome, verification/verifier, safe artifact, account/cost truth,
  approval/authority, next action.

### P0.4 — live mixed-account runtime proof

**[#8640 FC-5](https://github.com/OpenAgentsInc/openagents/issues/8640)**
Phase A is the immediate acceptance run.

Required receipt:

- at least two simultaneous pinned real work units;
- Codex and Claude each complete useful work through explicitly named isolated
  accounts;
- an authenticated typed client/adapter starts and manages the run;
- one steer or approval round trip;
- exact usage or explicit `not_measured` per turn;
- zero duplicate claims, silent substitution, default provider homes, or
  manually launched assignment shells;
- verification and closeout visible through the authenticated typed surface;
- measured FC latency distribution and proof rung for every acceptance item.

This is the Fleet runtime-unblock milestone, not the user-facing product
cutover. After its clean receipt, the substrate is ready for R2/R3 integration;
R7 still requires reliable Desktop/mobile operation. Do not wait for cloud or
Grok quota to run it. Grok is postponed; its adapters and historical canary
remain regression evidence.

### Follow-on — add managed cloud without changing the product contract

**[#8547 FC-CLOUD-1](https://github.com/OpenAgentsInc/openagents/issues/8547)**
completes Codex in real Firecracker.

**[#8636 FC-4](https://github.com/OpenAgentsInc/openagents/issues/8636)**
adds per-work-unit `owner_local | managed_cloud | auto` routing and owns the
hybrid acceptance receipt.

#8636 closes when one owner-scoped run executes local and managed-cloud work
concurrently under one claim registry: at least one owner-local unit and one
managed Agent Computer unit both complete useful verified work, target
selection/fallback is typed and visible, and compute/model usage truth remains
separate. Claude/Grok cloud expansion follows only through separately accepted
capacity and the same contract; it is not part of #8640's local cutover exit.

Cloud is additive and follows the local R7 exit unless it becomes necessary to
close a demonstrated reliability defect. A cloud blocker never stalls R0–R7.

### P0 completed substrate — production inference

**[#8600 FC-BRAIN](https://github.com/OpenAgentsInc/openagents/issues/8600)**
moves the landed chat adapter through persona-neutral Khala inference with exact receipts, turn
coalescing, caps, and typed fallback.

This issue is closed: its persona-neutral gateway lane, exact receipts,
coalescing, caps, typed fallback, deployment, and live proof are retained
substrate. It is not an open lane or a prerequisite for the first owner-gated
local fleet dogfood slice.

## PAUSED — Sarah presentation, voice, video, and named front door

**[#8610](https://github.com/OpenAgentsInc/openagents/issues/8610)** and all
remaining opener/avatar/voice/video/media-quality work are paused by the
2026-07-10 owner decision. Existing artifacts, audits, recipes, contracts, and
closed receipts remain historical evidence. Do not spend active product or GPU
capacity on new A/V experiments, canaries, rendering quality, semantic media
cache, ASR/VAD, or persona polish.

Allowed work is limited to a production outage, security/privacy/data-loss
repair, removing active cost, or preserving a compatibility floor needed by an
already-supported consumer. Reopening requires an explicit owner decision
after R7; the next proposal must show why direct Desktop/mobile software is no
longer the higher-value reliability constraint.

The existing voice audit remains a historical decision record:
[`docs/sarah/2026-07-09-pipecat-voice-infra-audit.md`](../sarah/2026-07-09-pipecat-voice-infra-audit.md).

## Effect Native runtime work; presentation program paused

The 2026-07-09 glass/Sarah program is superseded as an active product lane by
the 2026-07-10 reliability reset. Keep the architecture it proved: app code
uses typed Effect Native components/intents, and host-specific lowering stays
behind renderer boundaries. Continue only work required for R0–R7 interaction
correctness, accessibility, lifecycle safety, Android/iOS parity, or removal of
an app-local duplicate architecture. Pause aesthetic glass refinement, named-
assistant placement, and presentation-only migrations.

The target hybrid contract is: `@expo/ui` is consumed strictly inside
`render-rn` as a lowering target (SwiftUI on iOS, Compose on Android); app code
sees only typed Effect Native catalog components; `surface: "glass"` is a
semantic contract (Liquid Glass on iOS 26+, honest material equivalents
elsewhere); owned lowerings replace `@expo/ui` component-by-component. Build
115's restored app-local SwiftUI module is the explicitly recorded current
exception, not a second application-state architecture.

| Lane | Issue | Purpose |
| --- | --- | --- |
| GL epic | #8646 | **PAUSED** presentation program; retain closed receipts |
| GL-1 | #8647 | CLOSED 2026-07-10 — host driver + v30 @expo/ui lowering + island deletion |
| GL-2 | #8648 | CLOSED 2026-07-09 — glass shell shipped (builds 107–112) |
| GL-3 | #8649 | CLOSED 2026-07-10 — build 113, production conversation, contract enforced |
| GL-4 | #8650 | **PAUSED except R0–R7 blockers** — owned-lowering migration |

Current narrow truth (2026-07-10): GL-1 closed at `5202a2665a` after the Scope-
bound render-rn host driver and catalog-v30 internal `@expo/ui` lowering landed,
the then-current D-MB-02 island was deleted, and the mechanical import oracle
passed. Build 114 subsequently shipped an opaque fallback because its binary
omitted the expected native module; build 115 source at `ee78dc1a2e` restores an
application-local `openagents-liquid-glass` module. Therefore the closed receipt
is historical architecture evidence, not a claim that the current tree has no
app-local native presentation module. #8646/#8650 remain paused except where a
narrow host/lowering defect demonstrably blocks R0–R7.

GL-3 is delivered and closed at `6647d998ad` / TestFlight build 113. The
mobile glass shell now mints/persists the production prospect session, sends
typed turns through the same `/sarah` contracts as web, renders the production
reply, survives restart, and shows bounded offline/reconnect states under an
enforced behavior contract. Its current text path renders the POST result while
SSE carries liveness/cards. Authenticated operator posture, authoritative Sync,
and Android proof move into R1/R2/R6; voice/avatar tiers remain paused. The
checked-in historical issue source and exit ordering live in
[`issues/glass-ui-and-sarah-mobile.md`](./issues/glass-ui-and-sarah-mobile.md).

## Active client programs — OpenCode-parity Desktop and synced mobile

Epic: **[#8566 APP-1](https://github.com/OpenAgentsInc/openagents/issues/8566)**.
Its Desktop/mobile reliability scope is P0 under R0–R7. Its web/presentation
scope is maintenance or paused unless it blocks those gates.

### OpenAgents web — maintain, do not expand during the reliability burn

Retained product routes:

- `/` — landing;
- `/sarah` and Sarah-owned API/event paths;
- `/forum` and required Forum routes;
- `/promises` — human-readable promise state and claim-integrity audit.

Explicit infrastructure exceptions include `/privacy`, `/terms`, auth
callbacks, public APIs, assets, health checks, machine-readable manifests, and
receipt endpoints. Promise-specific preserved routes include the stable
`/docs/product-promises` meaning/report path, registry, transition, audit and
readiness APIs, owner-gated transition authority, and every public-safe
receipt/verification/evidence route cited by a promise or service deliverable.
They do not become extra product destinations.

- **[#8634 APP-WEB](https://github.com/OpenAgentsInc/openagents/issues/8634):**
  one Effect Native host, exact route allowlist, promise/service-deliverable
  integrity preservation, redirect/410 plan, deletion of other public pages,
  and migration of private operator functions toward Desktop.
- **[#8635 APP-FORUM](https://github.com/OpenAgentsInc/openagents/issues/8635):**
  retain Forum behavior and deep links inside the Effect Native web app.
- **[#8595 APP-WEB-LANDING](https://github.com/OpenAgentsInc/openagents/issues/8595):**
  **paused product work**; the earlier Sarah + three-app copy/cutover scope no
  longer reflects the active owner priority.

Status: #8634's retained-route oracle and live `/`, `/sarah`, `/forum`,
`/promises`, promise-registry, and proof-replay smokes passed against production
revision `00068-5t8`; the exhaustive retirement inventory and cutover remain
open. #8595's `/landing-en` surface is code/fixture-proven at `0625e8b291`;
root promotion, owner copy/assets, rollback proof, and preview deletion remain
open.

Do not continue generic EN-4 route conversion or new web product scope during
R0–R7. Preserve security, auth, API, receipt, promise-integrity, and production
operations. A page scheduled for retirement is deleted, not lovingly ported.

### OpenAgents mobile

**[#8597 APP-MOBILE](https://github.com/OpenAgentsInc/openagents/issues/8597)**
builds a new OpenAgents iOS/Android app at `apps/openagents-mobile`.

Status: the initial greenfield shell, identity/icon oracles, Effect Native
React Native renderer seam, owned OTA feed, and iOS release lane are proven.
TestFlight build 113 closed the text-first Sarah compatibility slice. Build 114
at `23aba8270a` adds a separate public generic Khala conversation mode and is
App Store Connect `VALID`, but omitted the intended native glass module. Build
115 source at `ee78dc1a2e` restores that module; 40 tests/typecheck, archive/
export, and App Store Connect validity pass, while physical-device acceptance
remains open. Khala mode is stateless with respect to authenticated
account keys, credits, FleetRun, receipts, and cross-device Sync. The persisted
five-thread catalog remains app-local. Neither route satisfies R1–R6.

The bundled video and demo pricing are presentation-only and paused; there is
no StoreKit purchase.

#8597 retains an unreleased Fable claim whose published scope is only the
initial greenfield setup; later OTA/SwiftUI/TestFlight work exceeded that
recorded scope. Treat it as owned until the actor posts an explicit re-scope or
release—do not infer a stale claim without the protocol's evidence and process/
worktree audit. Authenticated Sync/Fleet continuation, Android proof, vendor
reconciliation needed for reliability, and the full issue exit remain open.

GL-1/#8647 landed the Scope-bound host driver and internal `@expo/ui` lowering.
The build-115 compatibility repair reintroduced an application-local SwiftUI
module after build 114's visible fallback regression. Preserve the working
release floor and record the exception honestly; further visual migration is
paused unless a narrow defect blocks R0–R7.

- The default target is a neutral activity/fleet home: recent work, sync
  health, attention/approvals, active runs, outcomes, and handoff.
- Fleet runs, approvals, command outcomes, receipts, conversations, and
  Blueprint continuity use authoritative Khala Sync projections.
- Account setup remains directly accessible for recovery/power use.
- Effect Native is the application model and React Native/Expo is the host.
- Mobile implements the R6 supervision subset. File editing, full Git review,
  and PTY are handed off to Desktop instead of approximated with unsafe or
  cramped mobile controls.
- The product name is `OpenAgents`; both the iOS bundle identifier and Android
  application ID are the owner-designated existing identifier
  `com.openagents.app`.
- The checked-in application icon is copied exactly from
  `clients/khala-mobile/assets/images/icon.png` (SHA-256
  `0a1865ac6d1efc792d365d9a37af9e6ffa3270fa7c8731f36129f35371bfc7ce`).
- `clients/khala-mobile` is deprecated and frozen as a parity, contract, native-
  module, and migration reference. It is not renamed, converted in place, or
  shipped as the destination app.

### OpenAgents Desktop

**[#8574 APP-DESKTOP](https://github.com/OpenAgentsInc/openagents/issues/8574)**
builds a new Electron application at `apps/openagents-desktop`.

**Priority contract:** this is a P0 product lane. D0–D6 and the shared R0–R7
contracts receive first available product capacity alongside the narrow #8640
runtime burn. Cloud, Sarah/persona work, A/V/presentation, broad web work,
multi-window depth, and WSL do not block ready parts of this program.

**Benchmark and architecture:** reach practical parity with the current
OpenCode Desktop workbench while retaining OpenAgents architecture. OpenCode's
affordances and current `packages/desktop` + `packages/app` behavior are the
benchmark; its code and renderer capability model are not the destination.
Effect Native remains the only application/component/intent grammar, Electron
remains a hardened host, Khala Sync owns cross-device continuity, Pylon/Source
Authority own Fleet execution and receipts, and direct typed software controls
are the primary surface. The detailed evidence and 20-area baseline are in the
[`desktop parity audit`](./2026-07-10-opencode-khala-openagents-desktop-parity-audit.md).

**Current rung:** Terra is the active #8574 execution lane. Through
`f4cb8ed18e`, the app has a hardened scaffold, minimal Effect Native chat,
host-owned gateway completion, a bounded persisted five-thread catalog, shared
icons/glass lowering, folder selection, bounded root listing/read-only preview,
and Codex readiness/device-auth Settings. The reconciled Terra receipt records
58 passing tests, green typecheck, and a real-Electron smoke after the Settings
slice. The smoke proves a system/error response plus scripted device auth, not
live model or owner-account success. The audit scores OpenAgents Desktop at 1 landed, 6
partial, 3 scaffold, and 10 absent areas; broad Khala Code parity work has not
yet crossed the greenfield boundary.

#### Required product shape

OpenAgents Desktop is one coherent application with three depths:

1. **Work and activity** — persistent conversations/sessions, active context,
   follow-ups, requests, approvals, outcomes, and next actions.
2. **Coding workbench** — projects/sessions, streamed agent timeline, rich
   composer, files/editor, Git diff/review/comments, terminal, commands,
   providers/models/MCP/permissions, settings, diagnostics, and desktop
   lifecycle.
3. **Fleet cockpit** — active FleetRun, worker/account/capacity state,
   assignments, approvals, steering, Inbox attention, Gym/proof, receipts, and
   closeout over the same server-authoritative records mobile sees.

Conversation remains the quiet default. The workbench and Fleet cockpit are
fast to open through projects, tabs, commands, and explicit active state; they
do not become permanent developer/proof chrome around every conversation.

#### Typed action and cross-device control contract

- Every material user-visible action has a stable typed intent and command ID:
  create/open/focus a project or session, select context, submit/follow up/
  interrupt, open files/review/terminal, choose a model/runtime, respond to a
  request, and steer/approve/pause/resume/stop Fleet work.
- Every typed action is registered centrally, never selected through ad hoc
  string or keyword routing. Policy determines whether it may execute
  immediately or requires approval. A future assistant may observe/propose/
  invoke the same registry, but no named assistant is required for R0–R7.
- Khala Sync carries stable conversation, project/session, active-context,
  request, FleetRun, attempt, outcome, and receipt projections so mobile,
  Desktop, and supported web adapters do not fork reality. Ephemeral cursor,
  focus, and selection state stays local unless a typed continuity use requires
  it.
- Direct Desktop and mobile controls call the same policy/runtime service and
  produce the same typed outcome. No automation bypasses filesystem grants,
  terminal boundaries, credentials, approvals, worker authority, or receipt
  gates.
- Desktop surfaces show pending, unavailable, rejected, reconnecting, and
  failed states explicitly; mobile receives the corresponding durable outcome
  instead of inferring success from transcript prose or pixels.

#### Parity burn

| Gate | Scope | Exit |
| --- | --- | --- |
| D0 — truthful green baseline | Keep shared contracts green; remove or finish dormant Review/Terminal/Inbox/Fleet names and stale docs; isolate smoke state and distinguish live, unconfigured, and fixture receipts | Typecheck, tests, bundle, isolated first-run Electron smoke, and route/capability manifest are green and agree |
| D1 — OpenAgents + Sync conversation runtime | Replace five local-only threads and request/response chat with authoritative thread/session identity; streamed text/reasoning/tools/plan/todo/questions/permissions/approvals/errors/usage; interrupt/resume/reconnect; rich composer, history, modes, attachments, model/agent/variant selection, and selected context | One real authenticated stream survives restart/reconnect and continues on mobile with matching identity, event cursor, and durable state |
| D2 — projects, sessions, commands | Project/session routes and home, search/archive, sortable/recoverable tabs, command registry/palette, conflict-safe keybindings, native menu, deep links, single-instance and route restore | Every global/session/workbench action uses the command registry or has an explicit bounded exception |
| D3 — coding workbench | Recursive lazy tree, capability grants, watcher/cache/search, edit/save/dirty/reload, file tabs and selected ranges, typed Git status/diff, review/comments/revert, interactive workspace-bounded PTY tabs with reconnect/teardown | Select a project, edit/save, review the diff, add context, run a bounded terminal, steer the work through a typed control, and resume after restart |
| D4 — runtime and settings | OpenAgents sign-in; provider account custody; runtime/model catalog and selection; MCP auth/enable state; enforced permissions; themes/fonts/shell/layout; locale/accessibility; notifications/sounds; diagnostics/recovery/support | Settings mutate real host/runtime state, unavailable actions explain why, and no credential/private payload reaches renderer logs or public evidence |
| D5 — authoritative Fleet cockpit | Compose active FleetRun/work-unit/attempt/account/worker/approval/command/receipt state from current Pylon/Sync authority; Inbox attention and proof views; Desktop and mobile controls share typed intents | A run opens with matching Desktop/mobile state and controls; steering on either client converges to the same durable command outcomes and receipts |
| D6 — desktop productization | Freeze independent identity; package with verified fuses; signing/notarization; update/release notes; public-safe debug export and crash/load/unresponsive recovery; clean install/update/rollback proof | Installable, updateable, recoverable app with an independent release lane; legacy Electrobun release/install paths are removed |

The first practical parity gate includes the everyday OpenCode capabilities in
D1–D5 plus the safe desktop lifecycle needed to use them. Persisted multi-window
geometry/restore, remote-server depth, and the complete Windows WSL lifecycle
remain explicit follow-ons unless they become necessary for a supported user
path before D6. “Parity” never means those deferred items silently disappear.

#### Non-negotiable implementation boundaries

- The local five-thread store and staged Fleet brief are temporary scaffolds,
  not Khala Sync/Fleet authority.
- Monaco/editor and terminal render through typed Effect Native foreign-host
  nodes. The renderer never receives general filesystem, arbitrary process,
  raw IPC, token, or credential authority.
- Pylon is an engine, not a separate public desktop product or a second local
  run universe.
- OpenCode and `clients/khala-code-desktop` are read-only reference/extraction
  sources. Port behavior, typed contracts, and test vectors; never import or
  convert their product UI in place.
- The scaffold's sandbox/context isolation, `nodeIntegration: false`,
  restrictive CSP, and deny-by-default permission/navigation/window-open
  posture remain mandatory as capabilities expand. Every new preload method is
  fixed, origin/sender-validated, schema-decoded, and least-authority.
- Component gaps go upstream through Effect Native. Starter
  Zod/oRPC/shadcn/TanStack application semantics do not return as a parallel
  desktop architecture.
- The reusable Electron host gap remains OpenAgentsInc/effect-native#69. The
  earlier Electrobun Phase 4 issues are historical, not destination proof.
- The full cross-platform app/protocol/data/update/OAuth identity freezes before
  the first packaged build. Packaged fuses, signing/notarization, clean-machine
  smoke, update, and rollback are required before distribution.
- The live #8574 claim is the coordination authority for each leaf and every
  shared Effect Native hot contract.

### Effect Native integration topology

The deploy guard now verifies the physical runtime boundary instead of treating
peer-report output as resolution authority: OpenAgents/Omega remains on Effect
`4.0.0-beta.70`, exactly the four vendored Effect Native packages resolve
`4.0.0-beta.94`, and the isolated Nostr line remains Effect 3. The mismatch is
no longer a deploy blocker; any package escaping its declared line fails the
guard.

## Deferred work after the reliability exit

The following directions remain dependency-held until P0 evidence pulls a
bounded next slice:

- **[#8642 BM-CORRECT](https://github.com/OpenAgentsInc/openagents/issues/8642):**
  inspect/correct/delete/export Blueprint facts through provenance-bearing
  revisions, scoped tombstones, authorized propagation, and receipts. It
  activates after R7 or immediately when the first real user requests a
  correction/deletion or a live privacy incident fires the tripwire.
- **[#8643 SARAH-ROLES](https://github.com/OpenAgentsInc/openagents/issues/8643):**
  **paused by owner decision**; do not generalize the relationship-mode seam or
  create a named colleague during R0–R7.

- named-assistant standing responsibilities using `agent_definition.v1`;
- Blueprint Map maturation into the company brain;
- proven role/template extraction;
- in-conversation payment;
- outbound sales and email automation;
- broader assurance, connector, and network programs.

When explicitly reactivated, each begins against the same typed authority and
action contracts used by Desktop/mobile. It does not begin as a fourth product
surface or a parallel state universe.

## Canonical open issue set

There are **16 open roadmap issue records** at this reset. Their previous labels
and prose do not all match the new priority. The disposition below is
authoritative for sequencing; reconcile live labels, issue bodies, and claims
before starting or continuing a slice. Closed #8639, #8647, #8648, and #8649
remain landed substrate and are not active burn items.

| Roadmap disposition | Issue | Purpose now |
| --- | --- | --- |
| **P0 active** | #8566 | Parent for the R0–R7 Effect Native Desktop/mobile reliability program |
| **P0 active** | #8574 | OpenCode-parity Desktop, authoritative Sync, Fleet cockpit, and D0–D6 |
| **P0 active** | #8597 | Purpose-built mobile continuity and fleet supervision, including iOS/Android proof |
| **P0 active substrate** | #8638 | Persona-neutral Fleet Command contracts projected into both clients |
| **P0 active proof** | #8640 | Real simultaneous Codex + Claude runtime burn, then R3/R7 client receipt |
| P1 follow-on | #8547 | Codex inside real Agent Computer after local cross-device reliability is green |
| P1 follow-on | #8636 | Hybrid local/cloud routing through the same proven run contract |
| Maintenance/deferred | #8634 | Web host/public-route work only when required for R0–R7 or production integrity |
| Maintenance/deferred | #8635 | Retained Forum maintenance; no active conversion expansion |
| **PAUSED** | #8595 | Landing/root copy and product cutover |
| **PAUSED** | #8610 | Sarah presentation, opener, avatar, voice, and video quality |
| **PAUSED** | #8643 | Sarah roles, named colleagues, and relationship-mode expansion |
| **PAUSED** | #8646 | Glass/Sarah-in-app presentation epic; retain closed architectural receipts |
| **PAUSED except R0–R7 blocker** | #8650 | Owned visual lowering migration |
| **PAUSED** | #8652 | Portal/product expansion during the core client reliability burn |
| P2 privacy tripwire | #8642 | Blueprint correction/deletion/provenance export; activate for real privacy/data-integrity need |

`roadmap:sol` remains the program label. “Paused” means no new feature or
quality scope is pulled; it does not authorize breaking production, abandoning
an active unsafe partial migration, or ignoring a security/privacy incident.

## Execution order

1. Reconcile #8566/#8574/#8597/#8638/#8640 issue bodies, labels, and active
   claims to this reset. Close R0 on both clients first: green clean-state
   builds/tests/smokes, honest capability manifests, and no fake authority.
2. Define one versioned persona-neutral identity/session/Sync contract from the
   existing Khala Sync and Fleet authority. Land R1/R2 vertical slices through
   both clients together: schema + migration, server projection/mutator,
   shared Effect Native domain/intent, Desktop view, mobile view, fault tests.
   Serialize shared schemas, migrations, catalogs, and generated clients.
3. In parallel, continue ready Desktop D1/D2 work and replace mobile's current
   route picker/local catalog as the default product shape with the neutral R6
   activity/fleet home. Preserve existing Sarah and generic Khala routes only
   as clearly bounded compatibility adapters while neutral contracts land.
4. Run #8640 Phase A at the first honest owner-account opportunity without
   blocking other R0–R2 work. Fix runtime defects in place. Then close R3 with
   one real simultaneous Codex+Claude run whose state and typed controls are
   exercised from both Desktop and mobile.
5. Close R4 before adding breadth: duplicate/out-of-order events, lost command
   acknowledgement, offline queue, stale lease, server/device restart, cursor
   gap, migration, and rollback. Convert every counterexample into a regression
   test and a user-visible bounded state.
6. Finish Desktop D3–D6/R5 and mobile R6: complete workbench, runtime/settings,
   Fleet cockpit, purposeful mobile supervision, accessibility, diagnostics,
   packaging, iOS/Android proof, and deep-link handoff. Reuse contracts and
   test vectors from deprecated Khala clients without converting them in place.
7. Close R7 with a sustained owner dogfood and release receipt. Only then pull
   #8547/#8636 cloud expansion or reactivate another product lane. #8642 may
   activate earlier only for a real privacy/data-integrity request or incident.
   All Sarah/persona, A/V, presentation, landing, portal, and optional visual-
   lowering work remains paused until an explicit owner reactivation.

## Implementation laws

1. **Integrate existing substrate before inventing.** New fleet schemas must
   justify why `FleetRun`, plan DAG, `KhalaFleetIntent`, Sync projections, and
   assignment refs are insufficient.
2. **One claim registry.** No local/cloud/harness path gets a private work-claim
   universe.
3. **Named account refs.** Automatic work never uses a default provider home.
4. **Authority remains distributed.** Desktop/mobile present and request;
   Worker/Pylon/Cloud/CRM/payment services retain their own typed authority.
5. **Exact/private evidence, bounded UI projections.** Raw work remains private;
   completion is independently verifiable.
6. **Effect Native for retained UI.** Shared domain/components/intents go
   upstream; Electron and React Native are capability hosts, not alternate app
   architectures. Deletion beats conversion for retired pages.
7. **Desktop parity is a product exit.** #8574 reaches the practical OpenCode
   workbench scope in D1–D5; a secure shell, placeholder panes, or Fleet-only
   cockpit does not satisfy Desktop completion.
8. **One action contract across clients.** Material actions use typed intents/
   command IDs and the same policy, capability, approval, idempotency, and
   outcome paths from Desktop or mobile. Future automation may consume that
   contract; model prose and local pixels are never authority.
9. **No product-surface regrowth.** New active product capability lands in
   Desktop or mobile; web receives only required public/API/operations work
   until an owner decision changes this focus.
10. **Constant motion with integration bias.** Owner/cloud blockers cause work
   to shift to another R0–R7 slice, never to paused presentation scope.
11. **Developer claims are explicit.** The live Sol GitHub issue set is the
   cross-session claim ledger; same-session coordination belongs to the root.
   Follow [`CLAIM_PROTOCOL.md`](./CLAIM_PROTOCOL.md), name hot files and hot
   contracts, and never steal a claim on elapsed time alone.
12. **Proof rungs never collapse.** Code-landed, fixture-proven, deployed,
    live-proven, owner-accepted, and closed remain distinct in issue bodies and
    reports.
13. **Challenges retain falsifiers.** Fable reviews from outside the queue; Sol
    records material dispositions, tripwires, and revisit conditions in
    [`CHALLENGE_LEDGER.md`](./CHALLENGE_LEDGER.md).
14. **Terra pulls ready leaves.** Terra may claim and ship a low-collision
    vertical slice without waiting for Sol to implement or restate it. Sol owns
    roadmap reconciliation and shared hot contracts; an active claim or P0
    boundary still requires explicit coordination.
15. **Mobile is purposeful, not compressed Desktop.** Mobile provides fleet
    supervision, attention, control, receipts, and handoff. Unsupported editor,
    Git, terminal, or credential capability is explicit and deep-links to
    Desktop rather than being approximated unsafely.
16. **A timeout is not an outcome.** Accepted, rejected, failed, and unknown-
    pending-reconcile are distinct durable command states. Retries are driven by
    idempotency and reconciliation evidence, never user-visible optimism.

## Completion reporting

Every issue closeout reports:

- landed commit and deployed version where applicable;
- exact tests, smokes, and live receipts;
- issue acceptance items met;
- authority/security boundaries exercised;
- legacy code or duplicate path deleted;
- residual could-not-prove list;
- next ready issue in this roadmap.

Every acceptance item names its current proof rung. A fixture or deployment is
never reported as live/owner acceptance.

## Reconciliation cadence

- Master roadmap and live issue bodies: after each material landing, owner
  priority change, issue disposition, or challenge decision.
- Execution/cutover/operating docs: when the critical path changes and at least
  weekly during active P0 burn.
- Subsystem, authority, and Effect Native architecture: on boundary change and
  at least monthly while actively cited.
- Dated analyses: immutable historical argument by default; append a response
  or mark superseded rather than silently changing the original analysis.

This file is reconciled in place after material landings. Do not grow a long
revision diary or restore the old 30-item phase queue.

---

## Fable reconciliation pass — 2026-07-10 (mid-day)

**Historical snapshot.** The facts below describe the mid-day state before the
owner's Desktop/mobile reliability reset. Any “in flight,” “P0,” or sequencing
language here is superseded by Revision 24 and the disposition table above.

Factual status updates since the last Sol reconciliation (Fable lane,
evidence on the named issues):

- **P0:** FC-3 #8639 CLOSED (overnight fleet burn; supervision surface
  landed). #8640 remains THE event: Sol root owns the typecheck gate;
  the owner-gated Codex reconnect now has a desktop UI path
  (`oa` → Settings → Connect Codex account, `f4cb8ed18e`). #8547's wall
  items 1–3 are done (in-VM codex execution + live-proven rootfs bake;
  a production double-billing bug in usage metering was found and fixed,
  now DEPLOYED); only its owner-gated live brokered turn remains.
- **Coordinated monolith deploy shipped** (staging+prod, smoke 8/8):
  FC-3 surface, the forum EN conversion (#8635 scopes 1–4+6 live; scope
  5 rides #8634), and the billing fix.
- **GL program:** GL-1/GL-2/GL-3 all CLOSED (see the GL section update);
  GL-4 + epic remain.
- **New lane:** #8652 PORTAL-1 (client portal on openagents.com —
  engagement view, content approvals, lead-gen KPIs; the Sell-in-Public
  revenue-loop front door, transcripts/247) — claimed and building; the
  canonical open set is now 16 issues:
  8547, 8566, 8574, 8595, 8597, 8610, 8634, 8635, 8636, 8638, 8640,
  8642, 8643, 8646, 8650, 8652.
- **Presentation (#8610):** openers-v2 (Hallo2 tier) received the OWNER
  VERDICT: "much better," opener-05 "close to shippable" — the Hallo2
  direction is approved. In flight: audio re-rolls to ≥8/10 + FLAIR as
  the permissive upscaler (the Hallo2 SR script is CodeFormer-derived,
  S-Lab non-commercial — research-only), and the web-/sarah pre-rendered
  clip tier (instant opener playback + canned-answer clips over the live
  WebRTC stream). Capacity truth for planning: pre-rendered tier ≈
  unlimited concurrency at ~$0; text/voice ≈ hundreds per instance;
  live MuseTalk video = 1 session per L4 GPU until a session router adds
  linear scaling.
- **Upstream effect-native:** v28 (markdown href), v29 (chat chrome +
  submit lifecycle), v30 (host driver + glass lowering) all shipped from
  consumer demand; a backlog-resolution pass over its 6 open issues is
  in flight.

*Editing note: this factual reconciliation was written by the **Fable** lane at
explicit owner request. Revision 24 preserves it as evidence but supersedes its
sequencing with the reliable Desktop/mobile program above.*
