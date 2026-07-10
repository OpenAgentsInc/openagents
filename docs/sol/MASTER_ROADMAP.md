# MASTER ROADMAP — Sarah Fleet Command; OpenCode-parity Desktop; three apps

- Date: 2026-07-10 (Fable reconciliation pass — see editing notes at bottom)
- Updated: 2026-07-10 (owner OpenCode-parity Desktop directive)
- Revision: 23
- Status: canonical OpenAgents implementation roadmap
- Supersedes: [`docs/fable/MASTER_ROADMAP.md`](../fable/MASTER_ROADMAP.md)
- Issue source set: [`issues/README.md`](./issues/README.md)
- Triage receipt: [`2026-07-09-issue-triage.md`](./2026-07-09-issue-triage.md)
- Desktop parity audit:
  [`2026-07-10-opencode-khala-openagents-desktop-parity-audit.md`](./2026-07-10-opencode-khala-openagents-desktop-parity-audit.md)

## Owner decisions encoded here

1. **Sarah managing coding fleets is P0 now.** The immediate cutover goal is
   simultaneous useful work across the owner's named isolated Codex and Claude
   accounts. Grok is postponed by the 2026-07-10 owner decision because its
   connected account is quota/payment exhausted; the existing Grok adapter and
   receipts remain regression substrate but do not block the cutover. Some
   capacity runs on desktop Pylons; managed cloud joins without blocking the
   local unblock.
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
   Khala Sync continuity, Pylon/Fleet authority, and Sarah-first relationship.
   Every material desktop action becomes a typed intent that Sarah can observe,
   propose, or execute under the same approval/policy boundary as a direct user
   action. Multi-window depth and WSL may follow the first complete workbench;
   they do not dilute the required everyday parity scope.

## The product in one sentence

**OpenAgents is Sarah: a persistent, inspectable relationship that can direct
and supervise real work across the owner's coding fleet now, then carry more
standing responsibilities over time.**

On Desktop, Sarah is also the steering and continuity surface for a complete
coding workbench. The editor, review, terminal, project/session system, account
controls, and Fleet cockpit are not separate products: they expose typed state
and actions to the same relationship, policy, Sync, and receipt loop.

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

Web and mobile are relationship-first projections. Desktop adds the complete
coding workbench and specialist Fleet cockpit: projects/sessions, rich agent
timeline and composer, files/editor, diff/review, terminal, commands, accounts,
settings, diagnostics, and release lifecycle. Sarah can steer those surfaces
through typed intents and Khala Sync, subject to the same approvals and host
capability boundaries as direct interaction. The three apps may emphasize
different layers but never own different conversation, run, authority, memory,
or evidence realities. This one-page shape is the acceptance artifact owned by
#8566.

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
  an owned `openagents-production` OTA channel. The owner accepted and closed
  GL-2 after build 111; build 112 fixed the Minerals-sheet lifecycle. TestFlight
  build 113 at `6647d998ad` is `VALID` and closes GL-3's text-first Sarah slice:
  production prospect/turn contracts, persistent relationship state, typed
  reply rendering, and bounded offline/reconnect behavior are proven. Pure
  transcript-stream/Sync continuation, authenticated operator posture,
  voice/avatar tiers, Android, and the full #8597 exit remain open. `7d77150514`
  adds an app-owned persisted five-thread catalog with fresh relationship mint
  and exact selected-thread restoration; it is local continuity, not Sync.
  Demo prices are presentation-only—no StoreKit purchase is implemented;
- the Effect Native vendor at `66d2f7544b` now includes upstream `2918c277`
  (v27): typed `IconButton`, `Toolbar`, semantic `surface: "glass"`, and Sheet
  detents. The Scope-bound host-driver registry, conversion/deletion of the
  app-local island, and real internal `@expo/ui` lowering are not landed;
- the greenfield OpenAgents Desktop line through `f4cb8ed18e`: the pinned
  electron-shadcn scaffold and Electron sandbox/isolation boundary now host a
  minimal Effect Native conversation workspace, a bounded host-owned five-
  thread store, a host-held model-gateway bridge with honest configuration
  failure, shared typed icons and glass backdrop/material lowering, and the
  first real workspace slice with user-selected root, bounded root listing, and
  bounded read-only file preview. A dedicated Settings surface reads bounded
  Codex readiness and drives Pylon's isolated device-auth flow. The current
  package has 60 passing tests, a passing bundle, and a passing real-Electron
  smoke; the smoke proves a system/error chat response and scripted
  awaiting-browser account state, not live model completion or real owner
  authentication. The current typecheck remains red on the shared Effect Native
  `Compose` icon mismatch. The pinned parity audit at `84648bd03c` scores the
  destination at 1 landed, 6 partial, 3 scaffold, and 10 absent capability
  areas, versus 8 landed, 10 partial, 1 scaffold, and 1 absent in the frozen
  Khala Code extraction source. That difference is now an explicit #8574
  product burn: Sync-backed Sarah conversation, command/project/session
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

The immediate gaps are now live-burn and additive-cloud gaps:

- C1 is crossed. #8637, #8633, and #8639 are closed; the minimum-safe Sarah →
  standing Pylon → exact supervision/reconnect path is fixture-proven and
  deployed.
- #8640 Phase A is the serial owner-local event. It now requires simultaneous
  useful Codex + Claude work through named isolated homes. The current blockers
  are the clean API ambient-type gate, a credential-scanner correction that
  preserves the no-long-lived-SCM-credentials invariant, and owner
  reauthentication of one isolated Codex home without touching default
  `~/.codex`. Terra's Desktop Settings path now exposes the proven isolated
  Pylon device-auth flow, but the real browser completion remains owner-gated.
- Grok is postponed by owner decision because the connected account is
  quota/payment exhausted. Its real accepted historical canary, HTTP-402
  state, adapters, and fixtures remain evidence/regression substrate; Grok is
  not a Phase A acceptance item.
- Agent Computer rootfs and in-VM exact-receipt source work has advanced under
  #8547, but its real brokered owner-account Firecracker turn remains open.
  Hybrid owner-local plus managed-cloud acceptance belongs to #8636 and never
  blocks the local cutover.
- The trusted-context voice coordinator/SSE adapter remains fixture-proven but
  unarmed until renderer-authenticated conversation/session metadata carries
  the server-minted conversation ref; model/system text is not scope authority.

P0 now closes the live receipt rather than building another fleet system.

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
in code and fixtures. C1 is now crossed; the remaining live cutover proof is the
owner-approved Codex+Claude Phase A receipt below.

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
| C1 | **crossed** — #8637, #8633, and #8639 are closed; the exact command/reconnect fixture is deployed through production `00069-h2k`. | Low-risk Sarah canaries may run; this app remains coordinator, independent verifier, and break-glass while failed canaries are repaired in place. |
| C2 | #8640 Phase A clean simultaneous Codex+Claude receipt from one pinned integrated deployment | Sarah/Khala/Pylon becomes the default entry point for new bounded owner coding work. This Codex app becomes control-plane development, independent review, and break-glass—not the routine dispatcher. |
| C3 | #8547/#8636 exits integrated with one clean owner-local plus managed-cloud receipt | Sarah may choose owner-local or managed-cloud capacity through the same run contract. |

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

Status: **closed** at `1d84386cb5`, fixture-proven and deployed. The integrated
C1 receipt proves three named work units, exact attempt/worker authority,
pause/resume, one exact steer, one exact approval, coherent verification and
closeout, privacy-safe reconnect, and durable command outcomes. Migrations
0054–0057 are applied; production `00069-h2k` carries the stack. The real
owner-account Codex+Claude work remains #8640 rather than an FC-3 residual.

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

- at least two simultaneous pinned real work units;
- Codex and Claude each complete useful work through explicitly named isolated
  accounts;
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
not wait for cloud, Grok quota, or presentation perfection to run it. Grok is
postponed; its adapters and historical canary remain regression evidence.

### P0.5 — add managed cloud without changing the product contract

**[#8547 FC-CLOUD-1](https://github.com/OpenAgentsInc/openagents/issues/8547)**
completes Codex in real Firecracker.

**[#8636 FC-4](https://github.com/OpenAgentsInc/openagents/issues/8636)**
adds per-work-unit `owner_local | managed_cloud | auto` routing and owns the
hybrid acceptance receipt.

#8636 closes when one Sarah run executes local and managed-cloud work
concurrently under one claim registry: at least one owner-local unit and one
managed Agent Computer unit both complete useful verified work, target
selection/fallback is typed and visible, and compute/model usage truth remains
separate. Claude/Grok cloud expansion follows only through separately accepted
capacity and the same contract; it is not part of #8640's local cutover exit.

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
| GL-1 | #8647 | CLOSED 2026-07-10 — host driver + v30 @expo/ui lowering + island deletion |
| GL-2 | #8648 | CLOSED 2026-07-09 — glass shell shipped (builds 107–112) |
| GL-3 | #8649 | CLOSED 2026-07-10 — build 113, production conversation, contract enforced |
| GL-4 | #8650 | Owned lowerings replace @expo/ui (migration half) |

Current narrow truth (2026-07-10): GL-1 is CLOSED at `5202a2665a` — the
Scope-bound render-rn host driver landed upstream (83f1bde), catalog v30
lowers the glass set through render-rn-internal `@expo/ui` to real SwiftUI
(pixel-proven on iOS 26.5), the D-MB-02 app-local island is fully deleted,
and a mechanical oracle proves app code never imports `@expo/ui`. The
vendored divergence is cataloged in VENDORING.md. GL-2 and GL-3 closed
earlier (builds 107–113). GL-4 (owned lowerings replacing `@expo/ui`)
is the program's only open lane besides the epic; native fingerprint
changed with GL-1, so the next coordinated TestFlight build carries it.

GL-3 is delivered and closed at `6647d998ad` / TestFlight build 113. The
mobile glass shell now mints/persists the production prospect session, sends
typed turns through the same `/sarah` contracts as web, renders the production
reply, survives restart, and shows bounded offline/reconnect states under an
enforced behavior contract. Its current text path renders the POST result while
SSE carries liveness/cards; pure transcript-stream unification, authenticated
operator posture, voice/avatar tiers, and Android proof remain follow-on work
under their owning lanes rather than GL-3 closure blockers. GL-4 remains held
by the owned-lowering dependency. The checked-in issue source and exit ordering
live in
[`issues/glass-ui-and-sarah-mobile.md`](./issues/glass-ui-and-sarah-mobile.md).

## P1 — three OpenAgents applications; OpenCode-parity Desktop

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
The owner accepted and closed the GL-2 shell at build 111; build 112 then fixed
the Minerals-sheet lifecycle. TestFlight build 113 at `6647d998ad` is `VALID`
and closes GL-3's text-first Sarah slice: the glass shell uses production
`/sarah` prospect/turn contracts, persists the relationship across restart,
renders typed replies, and exposes bounded offline/reconnect state. The bundled
video and demo pricing remain presentation-only; there is no StoreKit purchase.
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

**Priority contract:** this is a product-critical P1 lane, not presentation
polish. It runs in parallel with the current #8640 owner-local acceptance burn;
after C2, D1–D5 below receive the first available serial product capacity until
the practical parity gate is crossed. Cloud, avatar/voice perfection, real owner
Codex reconnect, multi-window depth, and WSL do not block the ready parts of
this program.

**Benchmark and architecture:** reach practical parity with the current
OpenCode Desktop workbench while retaining OpenAgents architecture. OpenCode's
affordances and current `packages/desktop` + `packages/app` behavior are the
benchmark; its code and renderer capability model are not the destination.
Effect Native remains the only application/component/intent grammar, Electron
remains a hardened host, Khala Sync owns cross-device continuity, Pylon/Source
Authority own Fleet execution and receipts, and Sarah remains the relationship
and steering surface. The detailed evidence and 20-area baseline are in the
[`desktop parity audit`](./2026-07-10-opencode-khala-openagents-desktop-parity-audit.md).

**Current rung:** Terra is the active #8574 execution lane. Through
`f4cb8ed18e`, the app has a hardened scaffold, minimal Effect Native chat,
host-owned gateway completion, a bounded persisted five-thread catalog, shared
icons/glass lowering, folder selection, bounded root listing/read-only preview,
and Codex readiness/device-auth Settings. Sixty tests, bundle, and real-Electron
smoke pass; current typecheck is red on the shared `Compose` icon contract. The
smoke proves a system/error response plus scripted device auth, not live model
or owner-account success. The audit scores OpenAgents Desktop at 1 landed, 6
partial, 3 scaffold, and 10 absent areas; broad Khala Code parity work has not
yet crossed the greenfield boundary.

#### Required product shape

OpenAgents Desktop is one coherent application with three depths:

1. **Sarah relationship** — persistent conversations, active context,
   follow-ups, requests, approvals, and next actions.
2. **Coding workbench** — projects/sessions, streamed agent timeline, rich
   composer, files/editor, Git diff/review/comments, terminal, commands,
   providers/models/MCP/permissions, settings, diagnostics, and desktop
   lifecycle.
3. **Fleet cockpit** — active FleetRun, worker/account/capacity state,
   assignments, approvals, steering, Inbox attention, Gym/proof, receipts, and
   closeout over the same server-authoritative records Sarah sees.

Conversation remains the quiet default. The workbench and Fleet cockpit are
fast to open through projects, tabs, commands, and explicit active state; they
do not become permanent developer/proof chrome around every conversation.

#### Sarah-steerability contract

- Every material user-visible action has a stable typed intent and command ID:
  create/open/focus a project or session, select context, submit/follow up/
  interrupt, open files/review/terminal, choose a model/runtime, respond to a
  request, and steer/approve/pause/resume/stop Fleet work.
- Every typed action is Sarah-addressable through a semantic action registry,
  never ad hoc string or keyword routing. Policy determines whether Sarah may
  observe, propose, execute immediately, or pause for approval; no action is
  hidden from Sarah merely because it originated in direct UI. The same owner
  scope, capability, approval, and budget checks still apply.
- Khala Sync carries stable conversation, project/session, active-context,
  request, FleetRun, attempt, outcome, and receipt projections so Sarah, mobile,
  web, and Desktop do not fork reality. Ephemeral cursor/focus/selection state
  stays local unless a typed continuity use requires it.
- Direct manipulation and Sarah steering call the same host/runtime service and
  produce the same typed outcome. Sarah never bypasses filesystem grants,
  terminal boundaries, credentials, approvals, worker authority, or receipt
  gates.
- Desktop surfaces show pending, unavailable, rejected, reconnecting, and
  failed states explicitly; Sarah receives the corresponding typed result
  instead of inferring success from prose or pixels.

#### Parity burn

| Gate | Scope | Exit |
| --- | --- | --- |
| D0 — truthful green baseline | Fix the shared icon/typecheck drift; remove or finish dormant Review/Terminal/Inbox/Fleet names and stale docs; isolate smoke state and distinguish live, unconfigured, and fixture receipts | Typecheck, tests, bundle, isolated first-run Electron smoke, and route/capability manifest are green and agree |
| D1 — Sarah + Sync conversation runtime | Replace five local-only threads and request/response chat with authoritative thread/session identity; streamed text/reasoning/tools/plan/todo/questions/permissions/approvals/errors/usage; interrupt/resume/reconnect; rich composer, history, modes, attachments, model/agent/variant selection, and selected context | One real authenticated stream survives restart/reconnect, is steerable by Sarah, and continues on another authorized OpenAgents surface |
| D2 — projects, sessions, commands | Project/session routes and home, search/archive, sortable/recoverable tabs, command registry/palette, conflict-safe keybindings, native menu, deep links, single-instance and route restore | Every global/session/workbench action uses the command registry or has an explicit bounded exception |
| D3 — coding workbench | Recursive lazy tree, capability grants, watcher/cache/search, edit/save/dirty/reload, file tabs and selected ranges, typed Git status/diff, review/comments/revert, interactive workspace-bounded PTY tabs with reconnect/teardown | Select a project, edit/save, review the diff, add context, run a bounded terminal, ask Sarah to steer the work, and resume after restart |
| D4 — runtime and settings | OpenAgents sign-in; provider account custody; runtime/model catalog and selection; MCP auth/enable state; enforced permissions; themes/fonts/shell/layout; locale/accessibility; notifications/sounds; diagnostics/recovery/support | Settings mutate real host/runtime state, unavailable actions explain why, and no credential/private payload reaches renderer logs or public evidence |
| D5 — authoritative Fleet cockpit | Compose active FleetRun/work-unit/attempt/account/worker/approval/command/receipt state from current Pylon/Sync authority; Inbox attention and proof views; Sarah and direct controls share typed intents | A Sarah-started run opens with matching Desktop state/controls, Desktop steering is reflected by Sarah, and both show the same durable outcomes and receipts |
| D6 — desktop productization | Freeze independent identity; package with verified fuses; signing/notarization; update/release notes; public-safe debug export and crash/load/unresponsive recovery; clean install/update/rollback proof | Installable, updateable, recoverable app with an independent release lane; legacy Electrobun release/install paths are removed |

The first practical parity gate includes the everyday OpenCode capabilities in
D1–D5 plus the safe desktop lifecycle needed to use them. Persisted multi-window
geometry/restore, remote-server depth, and the complete Windows WSL lifecycle
remain explicit follow-ons unless they become necessary for a supported user
path before D6. “Parity” never means those deferred items silently disappear.

#### Non-negotiable implementation boundaries

- The local five-thread store and staged Fleet brief are temporary scaffolds,
  not Sarah/Sync/Fleet authority.
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

The issue reset plus this reconciliation leaves **16 open roadmap issues**: 14
P0/P1 program issues, including dependency-following GL-4, and two explicitly
dependency-held P2 lanes. #8639, #8648, and #8649 are closed and remain listed
in their parent sections as landed substrate, not active burn items.

| Priority | Issue | Purpose |
| --- | --- | --- |
| P0 | #8638 | Sarah Fleet Command epic |
| P0 | #8640 | Live simultaneous Codex + Claude dogfood burn |
| P0 | #8547 | Codex inside real Agent Computer |
| P0 | #8636 | Hybrid local/cloud routing |
| P1 parallel | #8610 | Sarah presentation quality |
| P1 parallel | #8566 | Three-app Effect Native epic; greenfield mobile/desktop |
| P1 parallel | #8634 | Web host consolidation + public-page retirement |
| P1 parallel | #8635 | Retained Forum on Effect Native |
| P1 parallel | #8595 | Retained landing/root cutover |
| P1 parallel | #8597 | Greenfield OpenAgents mobile (`com.openagents.app`) |
| P1 parallel | #8574 | Effect Native/Khala Sync OpenAgents Desktop with practical OpenCode workbench parity and Sarah steering |
| P1 parallel | #8646 | GL epic: glass UI stdlib + Sarah in-app |
| P1 parallel | #8647 | GL-1 render-rn @expo/ui lowering seam |
| P1 parallel | #8650 | GL-4 owned-lowering migration |
| P2 deferred | #8642 | Blueprint correction/deletion/provenance export + privacy tripwire |
| P2 deferred | #8643 | Typed role programs + evidence-based colleague split |

Every open issue carries `roadmap:sol`. P0 fleet issues carry `priority:P0`;
parallel presentation/app/Desktop-parity lanes carry `priority:P1-parallel`;
dependency-held future lanes carry `priority:P2-deferred` and do not enter the
active burn until their milestone or tripwire fires.

## Execution order

1. Run #8640 Phase A at the first honest opportunity. Land the strict API type-
   boundary and credential-scanner corrections, reauthenticate one isolated
   named Codex home without touching default `~/.codex`, then complete one
   clean simultaneous Codex+Claude receipt. Fix discovered fleet bugs in place
   until the receipt is clean, then flip routine bounded owner coding to
   Sarah/Khala/Pylon by default.
2. Run #8547 and #8636 on dedicated cloud capacity; never block Phase A.
3. Keep #8610 active in parallel without taking the fleet integration hot
   paths; closed #8600 remains production inference substrate.
4. Continue GL-1's Scope-bound host driver, convert/delete the D-MB-02 app-local
   island, and prove actual internal `@expo/ui` lowering; GL-2 and GL-3 are
   closed, and GL-4 remains the later convert-and-delete lane.
5. Treat #8574 as the product-critical OpenCode-parity Desktop program. Keep D0
   and ready low-collision D1–D6 leaves moving in parallel now; a real isolated
   Codex reconnect is evidence when the owner is available, not a queue-wide
   blocker. After #8640 Phase A crosses C2, give D1–D5 the first available
   serial product capacity until the practical parity gate is crossed:
   authoritative Sarah/Sync stream, projects/sessions/commands, complete file/
   editor/review/PTY loop, runtime/settings, then the server-authoritative Fleet
   cockpit. Run D6 productization as soon as identity inputs and feature
   stability permit. Terra may pull another ready P1 leaf after claim release
   without waiting for Sol to restate the work. Sol retains P0 and shared-hot-
   contract integration unless the claim records a handoff.
6. Run #8634 route inventory/retirement and finish #8635 Forum cutover residue
   in parallel with app conversion; preserve active claim ownership.
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
7. **Desktop parity is a product exit.** #8574 reaches the practical OpenCode
   workbench scope in D1–D5; a secure shell, placeholder panes, or Fleet-only
   cockpit does not satisfy Desktop completion.
8. **Sarah steering shares direct-action contracts.** Material Desktop actions
   use typed intents/command IDs and the same policy, capability, approval, and
   outcome paths whether initiated by Sarah or direct UI. Khala Sync carries
   durable continuity; model prose and local pixels are never authority.
9. **No product-surface regrowth.** New capability lands in web, mobile, or
   desktop unless an owner decision changes the three-app rule.
10. **Constant motion with integration bias.** Owner/cloud blockers cause work
   to shift to another P0 slice, not to presentation scope by default.
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

*Editing note: this Sol-owned document was updated by the **Fable** lane
at explicit owner request ("ensure master roadmap in docs/sol/ is fully
up to date", 2026-07-10). Factual reconciliation only — no sequencing
changes; Sol re-sequences at its next pass.*
