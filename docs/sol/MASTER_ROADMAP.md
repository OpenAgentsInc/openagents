# MASTER ROADMAP — Sarah Fleet Command first; three OpenAgents apps

- Date: 2026-07-09
- Revision: 2
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

## Current implementation truth

The coding-fleet program starts from substantial working substrate:

- typed `FleetRun`, work planner, plan DAG, and claim registry;
- one typed fleet intent vocabulary for worker selection, pause/resume/drain/
  stop, approvals, and steering;
- Codex, Claude, and Grok chat plus worker adapters;
- harness-conformance fixtures and honest usage/failure classes;
- a fixture-proven mixed-kind supervisor with zero duplicate claims;
- Khala Sync fleet projections and steering mutators;
- caller-owned Khala→Pylon assignments, exact token rows, private event
  archives, and closeout proofs;
- a headless Pylon node with account registry, presence, assignment polling,
  session execution, and a local coordinator;
- Sarah's owned runtime, authenticated relationship, SSE bus, Blueprint Map,
  Actions, and Code/Receipts panels.

The immediate gaps are composition gaps:

- Sarah cannot create a FleetRun.
- The production FleetRun manager is process-local/in-memory and reachable only
  through desktop tooling.
- Real capacity inspection still narrows to one provider at a time and drops
  the mixed-pool worker kind on its account mapping.
- Grok uses a separate spawn path rather than the same production supervisor.
- The standing remote assignment worker is serial rather than a refillable
  parallel run executor.
- Sarah does not yet receive durable work-unit progress or emit the existing
  typed fleet controls.
- Agent Computer Codex still lacks the new live Firecracker proof.

P0 fixes those seams. It does not build another fleet system.

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
- avatar failure leaves full text/fleet control available.

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
- verification and closeout visible through Sarah.

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
- deploy simulator and text-only degradation.

This lane runs continuously on separate paths/capacity. It may fix a live
front-door outage immediately, but subjective or offline quality work does not
preempt P0 fleet integration.

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

The following directions remain, but have no active issue until P0 evidence
pulls a bounded next slice:

- Sarah-held standing responsibilities using `agent_definition.v1`;
- Blueprint Map maturation into the company brain;
- proven role/template extraction;
- in-conversation payment;
- outbound sales and email automation;
- broader assurance, connector, and network programs.

When reactivated, each begins as a capability inside Sarah and one of the three
apps. It does not begin as a fourth product surface.

## Canonical open issue set

The issue reset leaves **15 open roadmap issues**:

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

Every open issue carries `roadmap:sol`. P0 fleet issues carry `priority:P0`;
parallel presentation/app lanes carry `priority:P1-parallel`.

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

## Completion reporting

Every issue closeout reports:

- landed commit and deployed version where applicable;
- exact tests, smokes, and live receipts;
- issue acceptance items met;
- authority/security boundaries exercised;
- legacy code or duplicate path deleted;
- residual could-not-prove list;
- next ready issue in this roadmap.

This file is reconciled in place after material landings. Do not grow a long
revision diary or restore the old 30-item phase queue.
