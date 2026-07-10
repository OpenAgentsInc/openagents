# MASTER ROADMAP — Sarah Fleet Command first; three OpenAgents apps

- Date: 2026-07-09
- Revision: 19
- Status: canonical OpenAgents implementation roadmap
- Supersedes: [`docs/fable/MASTER_ROADMAP.md`](../fable/MASTER_ROADMAP.md)
- Issue source set: [`issues/README.md`](./issues/README.md)
- Triage receipt: [`2026-07-09-issue-triage.md`](./2026-07-09-issue-triage.md)

## Owner decisions encoded here

1. **Sarah managing coding fleets is P0 now.** The immediate goal is multiple
   simultaneous work streams across the owner's Codex, Claude, and Grok
   accounts. Some capacity runs on desktop Pylons; managed cloud joins without
   blocking the local unblock.
2. **Presentation quality is parallel, not blocking.** Avatar, opener, video,
   voice, visual quality, and UI polish have a dedicated lane, but they do not
   remain the serial queue head.
3. **There are three product applications:** OpenAgents web, OpenAgents mobile,
   and OpenAgents Desktop. Sarah is the relationship surface. Former Khala Code
   product ideas become capabilities inside these applications.
4. **All retained application UI uses Effect Native.** Web, mobile, desktop,
   and canvas share typed components and intents; platform frameworks are hosts
   or renderers.
5. **The public web surface contracts sharply.** Retain `/`, `/sarah`,
   `/forum*`, and `/promises`. Preserve the complete product-promise and
   service-deliverable integrity chain: stable promise docs/report paths,
   registry/transition/audit/readiness APIs, owner-gated transition authority,
   and dereferenceable receipt/verification/evidence refs. Legal, auth, other
   API, asset, and health routes are explicit infrastructure exceptions. Other
   human-facing pages are deleted, redirected, or made non-public rather than
   converted.
6. **Sol owns the roadmap.** Fable remains strategic source material, but its
   master roadmap and pre-reset queue are historical.
7. **Mobile and desktop are greenfield.** The new mobile app uses Effect Native
   with a React Native/Expo host at `apps/openagents-mobile`; it is named
   `OpenAgents`, uses `com.openagents.app` on iOS and Android, and copies the
   pinned current Khala Code mobile icon. The new desktop app uses Effect Native
   with Electron at `apps/openagents-desktop`. The old mobile and Electrobun
   desktop clients are deprecated extraction sources, not conversion targets.

## The product in one sentence

**OpenAgents is Sarah: a persistent, inspectable relationship that can direct
and supervise real work across the owner's coding fleet now, then carry more
standing responsibilities over time.**

Khala is the inference, routing, and Sync engine. Pylon and Agent Computers are
execution. Blueprint is legible memory and work state. Effect Native is the
shared application grammar. Receipts are completion truth.

## One relationship loop, three applications

```text
relationship -> comprehension -> control -> orchestration
      ^                                      |
      |                                      v
continuity   <-    evidence    <-         execution

       OpenAgents web  |  OpenAgents mobile  |  OpenAgents Desktop
       relationship       relationship          relationship + cockpit
               \________ same typed state, authority, and receipts ________/
```

| Layer | Canonical responsibility |
| --- | --- |
| Relationship | Sarah's persistent authenticated/prospect relationship across text, voice, and UI |
| Comprehension | Khala inference, typed tools, semantic selectors, and Blueprint drafts |
| Control | Owner scope, policy, budget, approval posture, and typed intents |
| Orchestration | Fleet planning, routing, claims, Pylon, and harness selection |
| Execution | Codex/Claude/Grok workers on owner-local Pylons or Agent Computers |
| Evidence | Verification, exact or explicitly unmeasured usage, and closeout receipts |
| Continuity | Khala Sync, Blueprint, provenance-bearing memory, and the next conversation |

Web and mobile are relationship-first projections. Desktop adds the specialist
Fleet/code/terminal cockpit. They may emphasize different layers but never own
different run, authority, memory, or evidence realities. This one-page shape is
the acceptance artifact owned by #8566.

## Current implementation truth

The coding-fleet program starts from substantial working substrate:

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
  an owned `openagents-production` OTA channel. TestFlight 0.4.3 build 106,
  0.5.0 build 107, 0.5.1 build 108, and 0.5.2 build 109 reached `VALID`;
  build 107 is the pixel-proven typed glass shell/drawer/composer loop. Build
  108 added the typed surface selector and bundled Sarah presentation loop,
  then received JS-only owner chrome corrections through the OTA channel at
  `d5e524b142`. Build 109 baked those corrections in and replaced the Sarah
  loop with the owner-selected, container-level silent video at `65f8216cb9`.
  Build-110 source at `c17c3823ad` keeps marketing version 0.5.2 per owner
  direction and adds a typed composer-triggered full-screen Sarah reply video
  with audio; source tests/typecheck and simulator proof are recorded, but no
  build-110 App Store Connect/`VALID` receipt is recorded. Owner-device acceptance,
  Sarah/Sync continuity, Android, and the full #8597 exit remain open;
- the Effect Native vendor at `66d2f7544b` now includes upstream `2918c277`
  (v27): typed `IconButton`, `Toolbar`, semantic `surface: "glass"`, and Sheet
  detents. The Scope-bound host-driver registry, conversion/deletion of the
  app-local island, and real internal `@expo/ui` lowering are not landed;
- the greenfield OpenAgents Desktop scaffold at `7313b0934e`, pinned to the
  required electron-shadcn source, with the Electron sandbox/isolation boundary,
  an Effect Native renderer loop, and a real Electron click smoke proven;
  Sarah/Fleet/Pylon composition, packaging/signing/updates, identity acceptance,
  and legacy-client retirement remain open under #8574;
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

The immediate gaps are now narrower composition and live-proof gaps:

- #8637 is closed. The authenticated Sarah tool, Postgres authority, registered-
  Pylon claim/import/accept/activate handoff, operator-to-closeout fixture,
  prospect/owner-isolation proofs, and latency receipt are fixture-proven;
  migration 0052 is table-verified in both environments; and exact main commit
  `0892d57b3b` is deployed as staging revision `00046-jpn` and production
  revision `00068-5t8`. The real mixed-account canary remains #8640, not an
  unreported FC-1 residue.
- #8633 is closed at the implementation/fixture boundary on the stack ending
  `d779c360c3`: one production standing composition now owns mixed
  Codex/Claude/Grok capacity, typed auto fallback, restart-safe claims and
  health, exact-or-unmeasured closeout evidence, a durable local outbox, and an
  authenticated owner/Pylon-bound server execution projection. Migration 0053
  for that projection is applied and table-verified in staging and production,
  but the FC-2 application stack ending `d779c360c3` has not been deployed; the
  attempted pre-deploy gate stopped before any application release. It has not
  spent a live three-account run in this program; the pinned integrated deploy
  and real-account receipt remain #8639/#8640 rather than an implied FC-2 rung.
- Sarah's safe FleetRun projection, persisted exact-cursor live session, views,
  run controls, and approval decisions are code/fixture-proven in retained
  `/sarah`, including exact start-result-to-scope selection through
  `6cd9d09205`. The transport split, reconnect-honest command outcome, and
  restart-safe private follow-up/completion are closed through `59538f71a2`.
  First-class stable work-unit/exact-attempt server authority is closed through
  `849856d189`. Real Pylon v2 lifecycle/approval/evidence production, Sarah
  direct attempt consumption and full canvas, the integrated C1 reconnect/
  control/privacy fixture, migrations 0054/0055/0056 deployment, and a deployed
  owner-cookie canary remain.
- The trusted-context voice coordinator/SSE adapter is fixture-proven but stays
  unarmed until renderer-authenticated conversation/session metadata carries
  the server-minted conversation ref; model/system text is not scope authority.
- The #8637 Sarah→standing-Pylon fixture and deployment rung are satisfied.
  Minimum-safe #8639 live reconnect/steering still gates the first real canary,
  and no real Codex+Claude+Grok burn has satisfied C2.
- Agent Computer Codex still lacks the new live Firecracker proof.

P0 fixes those seams. It does not build another fleet system.

### Current P0 integration ledger

| Lane | Narrowest proven state on `main` | Next blocking proof |
| --- | --- | --- |
| #8637 FC-1 | **closed** at `0892d57b3b`: integrated operator conversation -> durable authority -> registered standing Pylon -> bounded closeout is fixture-proven; timing is 1.8s acknowledgment / 6.1s first claim / 8.6s first capacity; staging `00046-jpn` and production `00068-5t8` are deployed and smoke-proven | none in FC-1; real mixed-account execution is #8640 and owner-cookie reconnect/steer is #8639 |
| #8633 FC-2 | **closed** on the stack ending `d779c360c3`: production standing adapters, shared typed auto policy, restart-safe mixed claims, health rotation, durable execution outbox/server projection, and one integrated three-harness restart/usage fixture are proven; migration 0053 is applied in staging/production, while the application stack is not deployed | the pinned integrated deployment is a #8639/#8640 gate, not reopened FC-2 residue; useful live Codex+Claude+Grok work is #8640 |
| #8639 FC-3 | controls/reconnect through `6cd9d09205`, media continuity through `3d87cb609b`, reconnect-honest command outcomes through `08aac90250`, restart-safe private follow-up/completion through `59538f71a2`, and first-class server work-unit/exact-attempt authority through `849856d189` are code-landed and fixture-proven | real Pylon v2 lifecycle/approval/evidence production; Sarah direct attempt consumption/full canvas; integrated reconnect/control/privacy receipt; deploy migrations 0054/0055/0056 and application stack |
| #8640 FC-5 | acceptance contract only | C1 integrated fixture, then Phase A on one pinned deployment |

These rows are implementation receipts, not issue closure. The commit named in
one row does not imply deployment or live proof, and later commits inherit the
landed substrate only when their relevant verification stays green.

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

## Interim parallelism and the Codex→Sarah switch

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
in code and fixtures, but real mixed-account fanout begins only with the C1/C2
pinned live receipts below.

The coding cutover is staged:

| Gate | When | Operating decision |
| --- | --- | --- |
| C0 | Now | #8637 and #8633 are closed; finish minimum-safe #8639 through this Codex app, subagents, or explicitly partitioned tabs. Sarah is not yet the coding front door. |
| C1 | On one pinned integrated commit, closed #8637 durable Sarah run + #8633 standing real mixed executor + minimum-safe #8639 named progress, typed control, and reconnect pass one Sarah→Pylon fixture E2E. | Only then send the first low-risk pinned real issue through Sarah as a canary; retain this app as observer/break-glass. |
| C2 | #8640 Phase A clean local three-harness receipt from one pinned integrated deployment | Sarah/Khala/Pylon becomes the default entry point for new bounded owner coding work. This Codex app becomes control-plane development, independent review, and break-glass—not the routine dispatcher. |
| C3 | #8547/#8636 exits integrated + clean #8640 Phase B receipt | Sarah may choose owner-local or managed-cloud capacity through the same run contract. |

C2 is the requested coding-unblock point. It does not wait for cloud,
presentation perfection, public-route contraction, or the complete three-app
Effect Native conversion. Full criteria and fallback rules are in
[`2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md`](./2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md).

## P0 — Sarah Fleet Command

Epic: **[#8638](https://github.com/OpenAgentsInc/openagents/issues/8638)**.

### P0.1 — create the durable run from Sarah

**[#8637 FC-1](https://github.com/OpenAgentsInc/openagents/issues/8637)**
adds the authenticated Sarah fleet tool and durable run request.

Status: **closed** on `0892d57b3b`, deployed staging then production. The
[closure receipt](https://github.com/OpenAgentsInc/openagents/issues/8637#issuecomment-4930867072)
records every proof rung and assigns the real mixed-account/live-control work to
#8640/#8639 rather than holding this contract lane open.

Exit:

- owner-authenticated `/sarah` request only;
- pinned public repository/work plan and bounded verifier;
- one durable owner-scoped `runRef` with idempotency;
- Pylon can claim it without a supervising CLI process;
- typed `prospect | customer | operator | administrator` relationship mode
  selects policy-owned tool/retrieval/posture/UI behavior; the model cannot
  select or upgrade it, and operator coding posture contains no sales flow;
- acknowledgment plus durable `runRef` p95 <= 5 seconds and first capacity/
  claim state p95 <= 15 seconds, otherwise an explicit typed delay/blocker;
- no raw prompts, shell output, paths, or credentials in Sarah's projection.

### P0.2 — run a real mixed local fleet

**[#8633 FC-2](https://github.com/OpenAgentsInc/openagents/issues/8633)**
wires the real Pylon supervisor across Codex, Claude, and Grok.

Status: **closed at the code/fixture boundary** on the implementation stack
ending `d779c360c3`. The integrated receipt proves one accepted Sarah run,
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

### P0.3 — supervise through Sarah

**[#8639 FC-3](https://github.com/OpenAgentsInc/openagents/issues/8639)**
connects durable progress and existing fleet intents to Sarah.

Current critical gap: requested-versus-effective command truth and the private
follow-up/completion path are now closed through `59538f71a2`. Browser reconnect
sees body-free delivery/completion receipts, no client collection manufactures
effective state, and queued steer/active stop execute through a durable fenced
dispatcher with an authenticated terminal ACK.

The server half of evidence composition is now closed through `849856d189`:
stable plan work units, retry-preserving exact attempts, strict proven-terminal
evidence, owner-safe refs, and legacy replay/repair all project atomically.
The remaining serial block is production and consumption. Pylon must emit v2
lifecycle, actual verifier/artifact/proof/authority/closeout/economics/usage,
and a typed approval binding only when a real executor lifecycle event exists.
Sarah must consume the work-unit/attempt entities directly, remove assignment
fallback/synthetic proof, populate closeout receipts, expose named steer only
for an authorized exact attempt, and render the full plan→claim→assignment→
verification→closeout chain. The upgraded C1 fixture then proves pause/resume/
approval/steer/reconnect/privacy before deployment.

Exit:

- named work streams visible in conversation and Blueprint canvas;
- pause/resume/drain/stop, steer, and approval actions;
- evidence-backed plan→claim→assignment→verification→closeout edges;
- browser reconnect reconstructs current state;
- avatar failure leaves full text/fleet control available;
- first executor progress/blocker p95 <= 30 seconds, progress freshness at
  least every 15 seconds, and 30 seconds without freshness becomes typed
  `stalled`/`reconnecting`, never indefinite live;
- conversation and media have separate state machines; media LIVE requires a
  fresh lease and stale video cannot render a frozen LIVE badge;
- the first coding closeout card passes the one-minute comprehension grammar:
  outcome, verification/verifier, safe artifact, account/cost truth,
  approval/authority, next action.

### P0.4 — live local dogfood unblock

**[#8640 FC-5](https://github.com/OpenAgentsInc/openagents/issues/8640)**
Phase A is the immediate acceptance run.

Required receipt:

- at least three simultaneous pinned real work units;
- Codex, Claude, and Grok each complete useful work;
- Sarah starts and manages the run;
- one steer or approval round trip;
- exact usage or explicit `not_measured` per turn;
- zero duplicate claims, silent substitution, default provider homes, or
  manually launched assignment shells;
- verification and closeout visible through Sarah;
- measured FC latency distribution and proof rung for every acceptance item.

This is the coding-unblock and default-owner-local cutover milestone. After its
clean receipt, new bounded pinned backlog work starts through Sarah/Khala by
default; this Codex app remains break-glass and control-plane development. Do
not wait for cloud or presentation perfection to run it.

### P0.5 — add managed cloud without changing the product contract

**[#8547 FC-CLOUD-1](https://github.com/OpenAgentsInc/openagents/issues/8547)**
completes Codex in real Firecracker.

**[#8636 FC-4](https://github.com/OpenAgentsInc/openagents/issues/8636)**
adds per-work-unit `owner_local | managed_cloud | auto` routing, then Claude
and Grok cloud parity through the same contract.

**[#8640 FC-5](https://github.com/OpenAgentsInc/openagents/issues/8640)**
Phase B closes when one Sarah run executes local and managed-cloud work
concurrently under one claim registry: at least one owner-local unit and one
managed Agent Computer unit both complete useful verified work, target
selection/fallback is typed and visible, and compute/model usage truth remains
separate.

Cloud is additive. A cloud blocker never stalls the P0.4 local burn.

### P0 completed substrate — production inference

**[#8600 FC-BRAIN](https://github.com/OpenAgentsInc/openagents/issues/8600)**
moves Sarah through persona-neutral Khala inference with exact receipts, turn
coalescing, caps, and typed fallback.

This issue is closed: its persona-neutral gateway lane, exact receipts,
coalescing, caps, typed fallback, deployment, and live proof are retained
substrate. It is not an open lane or a prerequisite for the first owner-gated
local fleet dogfood slice.

## P1 parallel — Sarah presentation quality

**[#8610](https://github.com/OpenAgentsInc/openagents/issues/8610)** is the one
consolidated presentation lane. The former OAV/SQ children are closed into it.

Scope:

- perfect opener and pre-rendered semantic-cache takes;
- one selected real-time and one selected offline rendering recipe;
- audio/prosody, ASR, turn latency, fallback, motion, responsive UI,
  accessibility, and Blueprint readability;
- deploy simulator and text-only degradation;
- typed admission: text is the floor, pre-rendered media never delays input,
  realtime video is a leased `available | queued | text_only | unavailable`
  enhancement, and bounded queue expiry returns to text;
- separate conversation/media health with a fresh-frame lease for LIVE and an
  unrepresentable frozen-frame-LIVE state;
- cost/admission telemetry: marginal cost per active minute, utilization,
  queue time, abandonment, recovery, and fallback;
- paired within-owner crossover at the first canary window across text, audio,
  realtime video, and pre-rendered opener + text, measuring scoped-action time,
  verified-outcome time, interventions, state comprehension, repeat-use
  preference after receipts, and marginal cost.

This lane runs continuously on separate paths/capacity. It may fix a live
front-door outage immediately, but subjective or offline quality work does not
preempt P0 fleet integration.

No experiment enters without the production decision/threshold it can change
and the candidate it will remove afterward. Tiny-N results publish medians and
bounded raw trials, not false population confidence.

### Voice infrastructure disposition

The canonical decision record is
[`docs/sarah/2026-07-09-pipecat-voice-infra-audit.md`](../sarah/2026-07-09-pipecat-voice-infra-audit.md).
Pipecat 1.5.0 at audited upstream commit `5b75654` is a BSD-2-Clause pattern
source and one bounded ASR/VAD sidecar experiment, not Sarah's orchestration
foundation.

Preserve `apps/sarah` Effect orchestration, Effect Native state/intents, Khala
inference and receipt authority, hydralisk's owned TTS/avatar renderer, and the
text-first/media-admission floor. Extract smart-turn end-of-turn inference,
Silero VAD-segmented `faster-whisper` STT, qualified interruption, word
timestamps, per-stage TTFB/TTFA, tracked task cleanup, heartbeat/ICE recovery,
and behavioral-eval patterns. Do not add Daily/Pipecat Cloud coupling, replace
the brain/renderer, or create a parallel RTVI client-state universe.

Ordered slice:

1. Port smart-turn with the upstream ref, BSD notice, model digest, and a
   single-thread CPU executor.
2. Run `SARAH_ASR=browser|owned` as a separate process on the existing GPU host:
   Silero -> `faster-whisper` -> smart-turn -> exactly one final utterance into
   the existing `/sarah/api/avatar/speak` contract. Compare a pinned Pipecat
   sidecar with a native FastAPI port; browser ASR remains rollback/fallback.
3. Add VAD-qualified barge-in through hydralisk's existing `interrupt` verb,
   with a minimum-words/backchannel guard, behavior-contract row, and simulator
   oracle in the same implementation change.
4. Add stage latency/TTFA, captions/word timestamps, and scripted live scenarios,
   then explicitly accept the sidecar, select the native port, or retain the
   browser fallback.

Advancement gates are falsifiable: the existing voice-to-voice target remains
p50 <= 800 ms with stage p50/p95 published; VAD-qualified speech-start reaches
interrupt acknowledgement at p95 <= 500 ms and queued audible output stops at
p95 <= 750 ms; final-utterance delivery is exactly once; raw mic audio is
ephemeral by default; interim transcripts/metrics remain owner/session scoped;
and the renderer watchdog plus FPS/cadence simulator stay green under concurrent
ASR load. Failure selects a native port or fallback, never foundation adoption
by inertia. This P1 work cannot delay #8640 Phase A or the C2 coding cutover.

## P1 parallel — GL: native glass UI stdlib and Sarah-in-the-app

Owner decision (2026-07-09, adopting the hybrid recommendation of
`docs/fable/2026-07-09-swiftui-expo-ui-and-the-effect-native-stdlib.md`):
this program runs ALONGSIDE the Sarah Fleet Command burn, because the goal
is **consuming Sarah in the mobile app with native glass UI elements
ASAP**. Design target: the ChatGPT iOS app's Liquid Glass structure
(owner screenshots — glass pill buttons, circular icon buttons, nav
flyout drawer, floating composer).

The hybrid contract: `@expo/ui` is consumed strictly INSIDE `render-rn`
as a lowering target (SwiftUI on iOS, Compose on Android); app code sees
only typed Effect Native catalog components; `surface: "glass"` is a
semantic contract (Liquid Glass on iOS 26+, honest material equivalents
elsewhere); owned lowerings replace `@expo/ui` component-by-component as
effect-native#70 / EN-S lanes mature (convert-and-delete).

| Lane | Issue | Purpose |
| --- | --- | --- |
| GL epic | #8646 | Program epic: hybrid lowering, glass shell, Sarah in-app |
| GL-1 | #8647 | render-rn lowers typed EN glass components via @expo/ui |
| GL-2 | #8648 | openagents-mobile Home → ChatGPT-style glass shell |
| GL-3 | #8649 | Sarah conversation surface in the mobile app (text-first) |
| GL-4 | #8650 | Owned lowerings replace @expo/ui (migration half) |

Current narrow truth: GL-1 has the v27 typed catalog primitives on `main` at
`66d2f7544b`, but not its host-driver/island-conversion/real-lowering exit.
GL-2 has the pixel-proven build-107 shell; builds 108 and 109 subsequently
reached `VALID`, with the owner chrome corrections both OTA-proven on build 108
and baked into build 109 at `65f8216cb9`. Build-110 source at `c17c3823ad`
adds the typed composer-triggered full-screen Sarah reply video with audio, but
has no recorded ASC/`VALID` receipt. Both bundled Sarah videos are presentation
material, not GL-3 auth/SSE/reconnect proof. GL-3's text-first shared contract
can begin now. GL-4 remains held by the owned-lowering dependency.

GL-3 is the convergence point with the Sarah program: the mobile
conversation surface consumes the same `/sarah` APIs, SSE transcript, and
typed intent grammar as the web surface — no parallel state models. Its
voice/avatar tiers follow the #8610 capacity policy; text is the
availability floor and is never blocked. GL-3 may build its text-first seam in
parallel with GL-1/GL-2 and then compose the glass catalog; consuming Sarah
does not wait for every owned SwiftUI lowering. The checked-in issue source and
exit ordering live in
[`issues/glass-ui-and-sarah-mobile.md`](./issues/glass-ui-and-sarah-mobile.md).

## P1 — three OpenAgents applications

Epic: **[#8566 APP-1](https://github.com/OpenAgentsInc/openagents/issues/8566)**.

### OpenAgents web

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
  rewrite copy for Sarah + three apps, complete owner review, promote the
  existing EN catalog surface to `/`, and delete previews.

Status: #8634's retained-route oracle and live `/`, `/sarah`, `/forum`,
`/promises`, promise-registry, and proof-replay smokes passed against production
revision `00068-5t8`; the exhaustive retirement inventory and cutover remain
open. #8595's `/landing-en` surface is code/fixture-proven at `0625e8b291`;
root promotion, owner copy/assets, rollback proof, and preview deletion remain
open.

Do not continue generic EN-4 route conversion. A page scheduled for retirement
is deleted, not lovingly ported.

### OpenAgents mobile

**[#8597 APP-MOBILE](https://github.com/OpenAgentsInc/openagents/issues/8597)**
builds a new OpenAgents iOS/Android app at `apps/openagents-mobile`.

Status: the initial greenfield shell, identity/icon oracles, Effect Native
React Native renderer seam, owned OTA feed, and iOS release lane are proven.
TestFlight 0.4.3 build 106, 0.5.0 build 107, 0.5.1 build 108, and 0.5.2 build
109 reached `VALID`; build 107 is the simulator-pixel-proven glass shell
correction and still needs owner-device acceptance. Build 108 added the typed
surface selector/Sarah presentation loop and received JS-only owner chrome
corrections through the live OTA channel. Build 109 at `65f8216cb9` baked
those corrections in and container-stripped audio from the owner-selected
Sarah loop. Build-110 source at `c17c3823ad` stays on marketing version 0.5.2
and adds a typed composer-triggered full-screen Sarah reply video with audio;
its source tests/typecheck and simulator proof are recorded, but it is not
called TestFlight-deployed/valid without an ASC/`VALID` receipt.
#8597 retains an unreleased Fable claim whose published scope is only the
initial greenfield setup; later OTA/SwiftUI/TestFlight work exceeded that
recorded scope. Treat it as owned until the actor posts an explicit re-scope or
release—do not infer a stale claim without the protocol's evidence and process/
worktree audit. Sarah/Sync cross-device continuation, Android proof, vendor
reconciliation, and the full issue exit remain open.

The current SwiftUI Liquid Glass island is intentionally app-local/interim,
not catalog-native. D-MB-02/effect-native#70 host-driver conversion and vendor
reconciliation are GL-1/#8647 work, not evidence that the final shared lowering
already exists.

- Sarah is home.
- Fleet runs, approvals, receipts, and Blueprint continue over Khala Sync.
- Account setup remains directly accessible for recovery/power use.
- Effect Native is the application model and React Native/Expo is the host.
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

Status: exit slice 1 is code/fixture/live-smoke proven at `7313b0934e`: the
pinned electron-shadcn scaffold is in the Bun workspace, the retained surface
runs through Effect Native, Electron starts with context isolation, sandboxing,
and deny-by-default navigation/permission posture, and the typed ping loop was
clicked in a real Electron smoke. The active app-directory claim remains with
the Fable session recorded on #8574, but its published scope is only the
already-landed initial greenfield setup. Its last evidence does not by itself
make the claim stale; require an explicit release/re-scope or the full protocol
audit before another session mutates the lane. Owner identity acceptance,
Sarah/Fleet and Pylon composition, specialist hosts, packaging/fuses, signing/
notarization, updates, cross-device continuation, and legacy-client retirement
remain open.

The scaffold already removed the template updater/publisher wiring, disabled
`nodeIntegration`, enabled sandboxing/context isolation, and installed
deny-by-default permission/navigation/window-open handling. Packaged Electron
fuse verification, signing/notarization, and the real release/update path are
still unstarted; the click receipt is a local smoke, not the whole issue exit.

- Sarah is the relationship surface.
- Fleet is the specialist cockpit over the same run state.
- Monaco, terminal, and raw diagnostics remain typed specialist hosts.
- Pylon is an engine, not a separate public desktop product.
- Effect Native is the application model and Electron is the host. The old
  Electrobun shell is not the destination architecture.
- Scaffold from the required MIT-licensed
  [`LuanRoger/electron-shadcn`](https://github.com/LuanRoger/electron-shadcn)
  template, pinning the imported upstream commit. Retain its useful Electron
  Forge/Vite/fuse/test structure, but harden its current `nodeIntegration: true`
  default before product work: remove its upstream updater/publisher wiring,
  set `sandbox: true`, install a deny-by-default Electron boundary, verify
  packaged fuses, and replace starter Zod/oRPC/shadcn/TanStack application
  semantics with a mechanically asserted Effect Native/Effect Schema boundary.
- The reusable Electron host gap is OpenAgentsInc/effect-native#69. The earlier
  Electrobun Phase 4 issues are historical, not destination proof.
- `clients/khala-code-desktop` is deprecated and frozen as a parity, contract,
  service-extraction, and migration reference; it is never renamed or converted
  in place.
- The full cross-platform app/protocol/data/update/OAuth identity freezes before
  the first packaged build. The secure Electron boundary, signed/notarized
  release lane, and independent updates feed must be proven before distribution.

### Effect Native integration topology

The deploy guard now verifies the physical runtime boundary instead of treating
peer-report output as resolution authority: OpenAgents/Omega remains on Effect
`4.0.0-beta.70`, exactly the four vendored Effect Native packages resolve
`4.0.0-beta.94`, and the isolated Nostr line remains Effect 3. The mismatch is
no longer a deploy blocker; any package escaping its declared line fails the
guard.

## P2 — after the coding loop is real

The following directions remain dependency-held until P0 evidence pulls a
bounded next slice:

- **[#8642 BM-CORRECT](https://github.com/OpenAgentsInc/openagents/issues/8642):**
  inspect/correct/delete/export Blueprint facts through provenance-bearing
  revisions, scoped tombstones, authorized propagation, and receipts. It
  activates after #8640 Phase A or immediately when the first real user asks
  Sarah to correct/delete remembered information or a live privacy incident
  fires the tripwire.
- **[#8643 SARAH-ROLES](https://github.com/OpenAgentsInc/openagents/issues/8643):**
  generalize the FC relationship-mode seam into typed role programs and create
  a named colleague only when at least two authority/scope/responsibility/
  audience/metric dimensions diverge and repeated mode-switch tests show
  confusion or accountability loss.

- Sarah-held standing responsibilities using `agent_definition.v1`;
- Blueprint Map maturation into the company brain;
- proven role/template extraction;
- in-conversation payment;
- outbound sales and email automation;
- broader assurance, connector, and network programs.

When reactivated, each begins as a capability inside Sarah and one of the three
apps. It does not begin as a fourth product surface.

## Canonical open issue set

The issue reset plus this reconciliation leaves **19 open roadmap issues**: 17
P0/P1 program issues, including dependency-following GL-4, and two explicitly
dependency-held P2 lanes.

| Priority | Issue | Purpose |
| --- | --- | --- |
| P0 | #8638 | Sarah Fleet Command epic |
| P0 | #8639 | Sarah progress, canvas, approval, and steering |
| P0 | #8640 | Live multi-stream dogfood burn |
| P0 | #8547 | Codex inside real Agent Computer |
| P0 | #8636 | Hybrid local/cloud routing |
| P1 parallel | #8610 | Sarah presentation quality |
| P1 parallel | #8566 | Three-app Effect Native epic; greenfield mobile/desktop |
| P1 parallel | #8634 | Web host consolidation + public-page retirement |
| P1 parallel | #8635 | Retained Forum on Effect Native |
| P1 parallel | #8595 | Retained landing/root cutover |
| P1 parallel | #8597 | Greenfield OpenAgents mobile (`com.openagents.app`) |
| P1 parallel | #8574 | Greenfield Electron OpenAgents Desktop |
| P1 parallel | #8646 | GL epic: glass UI stdlib + Sarah in-app |
| P1 parallel | #8647 | GL-1 render-rn @expo/ui lowering seam |
| P1 parallel | #8648 | GL-2 mobile Home glass shell conversion |
| P1 parallel | #8649 | GL-3 Sarah conversation surface in mobile |
| P1 parallel | #8650 | GL-4 owned-lowering migration |
| P2 deferred | #8642 | Blueprint correction/deletion/provenance export + privacy tripwire |
| P2 deferred | #8643 | Typed role programs + evidence-based colleague split |

Every open issue carries `roadmap:sol`. P0 fleet issues carry `priority:P0`;
parallel presentation/app lanes carry `priority:P1-parallel`; dependency-held
future lanes carry `priority:P2-deferred` and do not enter the active burn until
their milestone or tripwire fires.

## Execution order

1. Finish #8639 against the now-stable owner-safe run projection while retaining
   text/fleet control under media failure; do not infer authority in the UI.
2. Run #8640 Phase A at the first honest opportunity. Fix fleet substrate bugs
   in place until the receipt is clean, then flip routine bounded owner coding
   to Sarah/Khala/Pylon by default.
3. Run #8547 and #8636 on dedicated cloud capacity; never block Phase A.
4. Keep #8610 active in parallel without taking the fleet integration hot
   paths; closed #8600 remains production inference substrate.
5. Run #8646–#8650 on the mobile/Effect Native capacity already owning that
   surface. Finish GL-1's Scope-bound host driver, convert/delete the D-MB-02
   app-local island, and prove actual internal `@expo/ui` lowering. Treat GL-2
   as integration/owner-acceptance residue after the landed build-107/108 shell,
   and start GL-3's shared text-first Sarah API/state contract now. GL-4 remains
   the later convert-and-delete lane. These are parallel P1 work and do not take
   #8639 hot contracts.
6. Run #8634 route inventory/retirement and #8635 Forum work in parallel with
   app conversion; landing/mobile/desktop slices continue from their landed
   substrate and preserve the active #8597/#8574 claim ownership.
7. Keep #8642/#8643 dependency-held until Phase A, except that #8642's first
   real correction/deletion request or privacy-incident receipt activates it
   immediately.

## Implementation laws

1. **Integrate existing substrate before inventing.** New fleet schemas must
   justify why `FleetRun`, plan DAG, `KhalaFleetIntent`, Sync projections, and
   assignment refs are insufficient.
2. **One claim registry.** No local/cloud/harness path gets a private work-claim
   universe.
3. **Named account refs.** Automatic work never uses a default provider home.
4. **Authority remains distributed.** Sarah presents; Worker/Pylon/Cloud/CRM/
   payment services retain their own typed authority.
5. **Exact/private evidence, bounded UI projections.** Raw work remains private;
   completion is independently verifiable.
6. **Effect Native for retained UI.** Deletion beats conversion for retired
   pages; component gaps go upstream.
7. **No product-surface regrowth.** New capability lands in web, mobile, or
   desktop unless an owner decision changes the three-app rule.
8. **Constant motion with integration bias.** Owner/cloud blockers cause work
   to shift to another P0 slice, not to presentation scope by default.
9. **Developer claims are explicit.** The live Sol GitHub issue set is the
   cross-session claim ledger; same-session coordination belongs to the root.
   Follow [`CLAIM_PROTOCOL.md`](./CLAIM_PROTOCOL.md), name hot files and hot
   contracts, and never steal a claim on elapsed time alone.
10. **Proof rungs never collapse.** Code-landed, fixture-proven, deployed,
    live-proven, owner-accepted, and closed remain distinct in issue bodies and
    reports.
11. **Challenges retain falsifiers.** Fable reviews from outside the queue; Sol
    records material dispositions, tripwires, and revisit conditions in
    [`CHALLENGE_LEDGER.md`](./CHALLENGE_LEDGER.md).

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
