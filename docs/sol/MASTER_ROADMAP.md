# MASTER ROADMAP — Sarah Fleet Command first; three OpenAgents apps

- Date: 2026-07-09
- Revision: 4
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
- a real-account capacity mapper that preserves Codex/Claude worker kind,
  advertised slots, readiness, and honest marginal-cost class without silently
  substituting an unsupported harness;
- a Pylon-only standing activation seam that recovers stale work before it
  idempotently resumes and refills an existing durable run;
- Khala Sync fleet projections and steering mutators;
- caller-owned Khala→Pylon assignments, exact token rows, private event
  archives, and closeout proofs;
- a headless Pylon node with account registry, presence, assignment polling,
  session execution, and a local coordinator;
- Sarah's owned runtime, authenticated relationship, SSE bus, Blueprint Map,
  Actions, and Code/Receipts panels;
- Sarah owner-safe FleetRun, continuity/stall, and six-section coding-closeout
  schemas plus a tested Effect Native receipt card;
- a bounded per-conversation VAD coalescer that prevents same-conversation
  parallel model calls; its live SSE fanout boundary is not yet wired.

The immediate gaps are now narrower composition and live-proof gaps:

- Sarah still lacks a merged production-durable, owner-scoped FleetRun creation
  authority that a standing Pylon can claim. A local JSON-only FC-1 proposal is
  not sufficient for this boundary.
- `pylon node` cannot yet reconstruct a real plan and executor from a Sarah run:
  the durable work-source descriptor, concrete Codex/Claude/Grok runner
  composition, explicit owner-local arming intake, and ref-to-process liveness
  adapter are not all Pylon-owned and wired.
- Grok still uses a separate spawn path rather than the same production
  supervisor path as Codex and Claude.
- Sarah's safe FleetRun projection and receipt card are code/fixture-proven but
  not yet fed by an authenticated cursor-resumable live client; named controls,
  approval/steering, reconnect, and exactly-once intent receipts remain.
- The VAD coalescer needs a bounded multi-controller SSE fanout/replay boundary
  before it can preserve immediate first byte and one `publishAndRecord` while
  sharing a model turn.
- No integrated Sarah→standing-Pylon fixture has satisfied C1, and no real
  Codex+Claude+Grok burn has satisfied C2.
- Agent Computer Codex still lacks the new live Firecracker proof.

P0 fixes those seams. It does not build another fleet system.

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
Claude or Grok accounts. Real mixed-harness fanout begins when #8633 is
integrated.

The coding cutover is staged:

| Gate | When | Operating decision |
| --- | --- | --- |
| C0 | Now | Build #8637/#8633/#8639 through this Codex app, subagents, or explicitly partitioned tabs. Sarah is not yet the coding front door. |
| C1 | On one pinned integrated commit, #8637 durable Sarah run + #8633 standing real mixed executor + minimum-safe #8639 named progress, typed control, and reconnect pass one Sarah→Pylon fixture E2E. | Only then send the first low-risk pinned real issue through Sarah as a canary; retain this app as observer/break-glass. |
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

Exit:

- durable Pylon-home FleetRun store;
- one real mixed account/capacity projection;
- one production runner path for all three harnesses;
- typed `auto` policy on live accounts;
- standing refill up to advertised capacity, no manual background shells;
- three simultaneous real local streams, one per harness, zero double claims.

P0.1 and P0.2 may proceed in parallel against their shared existing schemas.
Any schema change is serialized through the narrow shared package.

### P0.3 — supervise through Sarah

**[#8639 FC-3](https://github.com/OpenAgentsInc/openagents/issues/8639)**
connects durable progress and existing fleet intents to Sarah.

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

### P0 parallel — production inference

**[#8600 FC-BRAIN](https://github.com/OpenAgentsInc/openagents/issues/8600)**
moves Sarah through persona-neutral Khala inference with exact receipts, turn
coalescing, caps, and typed fallback.

This is a production-hardening priority but not a prerequisite for the first
owner-gated local fleet dogfood slice.

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

Do not continue generic EN-4 route conversion. A page scheduled for retirement
is deleted, not lovingly ported.

### OpenAgents mobile

**[#8597 APP-MOBILE](https://github.com/OpenAgentsInc/openagents/issues/8597)**
builds a new OpenAgents iOS/Android app at `apps/openagents-mobile`.

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

### Effect Native integration blocker

At this source snapshot `check:effect-topology` still expects Effect
`4.0.0-beta.70`, while the four vendored Effect Native packages require
`4.0.0-beta.94`. Resolve that guard/runtime mismatch under #8566 before any
clean-deploy claim.

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

The issue reset plus this Fable/Sol reconciliation leaves **17 open roadmap
issues**: 15 active P0/P1 lanes and two explicitly dependency-held P2 lanes.

| Priority | Issue | Purpose |
| --- | --- | --- |
| P0 | #8638 | Sarah Fleet Command epic |
| P0 | #8637 | Sarah fleet tool + durable run contract |
| P0 | #8633 | Real mixed-harness standing Pylon executor |
| P0 | #8639 | Sarah progress, canvas, approval, and steering |
| P0 | #8640 | Live multi-stream dogfood burn |
| P0 | #8547 | Codex inside real Agent Computer |
| P0 | #8636 | Hybrid local/cloud routing |
| P0 parallel | #8600 | Khala inference hardening |
| P1 parallel | #8610 | Sarah presentation quality |
| P1 | #8566 | Three-app Effect Native epic; greenfield mobile/desktop |
| P1 | #8634 | Web host consolidation + public-page retirement |
| P1 | #8635 | Retained Forum on Effect Native |
| P1 | #8595 | Retained landing/root cutover |
| P1 | #8597 | Greenfield OpenAgents mobile (`com.openagents.app`) |
| P1 | #8574 | Greenfield Electron OpenAgents Desktop |
| P2 deferred | #8642 | Blueprint correction/deletion/provenance export + privacy tripwire |
| P2 deferred | #8643 | Typed role programs + evidence-based colleague split |

Every open issue carries `roadmap:sol`. P0 fleet issues carry `priority:P0`;
parallel presentation/app lanes carry `priority:P1-parallel`; dependency-held
future lanes carry `priority:P2-deferred` and do not enter the active burn until
their milestone or tripwire fires.

## Execution order

1. Start #8637 and #8633 immediately on disjoint paths.
2. Start #8639 as soon as the stable run/projection seam from #8637 exists;
   fixture UI/projection work may begin earlier.
3. Run #8640 Phase A at the first honest opportunity. Fix fleet substrate bugs
   in place until the receipt is clean, then flip routine bounded owner coding
   to Sarah/Khala/Pylon by default.
4. Run #8547 and #8636 on dedicated cloud capacity; never block Phase A.
5. Keep #8600 and #8610 active in parallel without taking the fleet integration
   hot paths.
6. Run #8634 route inventory/retirement and #8635 Forum work in parallel with
   app conversion; landing/mobile/desktop slices follow their substrate.
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
