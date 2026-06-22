# Khala Buildout Roadmap — to the Head-to-Head

*Roadmap — 2026-06-22. The single sequenced buildout that takes Khala from the
existing gateway skeleton to a learned, verified, Bitcoin-settled inference model
that you **consume through Autopilot** and **watch serve in the Verse** — ending
in a concrete north-star: running our own version of the Fugu-Ultra-vs-frontier
head-to-head.*

Consolidates the per-area roadmaps: [`khala.md`](khala.md) (spec/§11),
[`khala-in-the-world.md`](khala-in-the-world.md) (visualization),
`docs/sakana/psionic-coordinator-roadmap.md` (coordinator primitives),
`docs/sakana/tassadar-run-integration.md` (verification classes),
`docs/research/tmax/synthesis.md` (recipe/stability), and the revenue-loop spine
(EPIC #5457).

## North-star goal (the end of the flow)

Run our own version of the head-to-head that's circulating publicly — **Sakana
Fugu Ultra vs. a frontier model**, same prompt, measured side by side:

> Prompt: *"build a really high quality single html file crossy road game with
> three.js"*
>
> **Sakana Fugu Ultra** (reported): ~89k tokens, ~$7.32, 22 min. Issues: inverted
> turn direction, wonky camera, no SFX, not identical to Crossy Road.
> **Claude Opus 4.8 (Ultracode)** (reported): ~940k tokens, ~$37.85, 79 min.
> Issues: stuck twice in a retry loop, wrong character position after restart,
> hard from the start (Fugu correctly ramped difficulty).
> Reported verdict: Opus won on functionality/quality/design; Fugu won on
> speed/cost/efficiency.
> *(Source: external X/Twitter comparison; treat the numbers as reported claims,
> not our measurements.)*

**Our version is the same task, run through Khala, with four things no one else
in that comparison has:**

1. **Run it through Autopilot → Khala.** A real user (or agent) submits the
   prompt to the Khala endpoint via the Autopilot cockpit; Khala's coordinator
   composes the pool (plan / write / verify) instead of one model one-shotting.
2. **Verified, not vibes.** "High quality crossy-road game" becomes a **rubric**
   (playable, correct controls, camera, difficulty ramps with progress, runs in
   one HTML file) graded by a verification class — `test_passed` (headless
   checks) and ideally a replay/parity check. The win condition is an **accepted
   outcome**, with a receipt.
3. **Settled in Bitcoin.** Whoever served the accepted outcome (Pylon worker,
   coding agent, validator) is paid, with a public receipt — the head-to-head is
   also a money loop.
4. **Watched in the Verse, and the artifact lives there too.** The whole serving
   flow renders in the world (crackling energy to the Pylons doing the work, a
   gateway portal if any external model is called, the coding-agent avatar
   building the game) — and the built three.js game is itself **playable in our
   three-effect world** (a three.js game inside our three.js Verse).

**We report the same axes plus ours:** tokens, $, wall-clock — **and**
cost-per-accepted-outcome, accepted-outcomes-per-kWh, in-world-vs-gateway split,
and verified-rate. The thesis we're proving: *composition + verification + a paid
open pool* can match frontier quality at lower cost, and — uniquely — you can
**watch and audit every piece**.

This is the demo the entire buildout below is sequenced to reach.

## Workstreams (what has to exist)

- **A — Khala serving.** The OpenAI-compatible endpoint + the coordinator ladder
  (v0 heuristic → v1 TRINITY logit router → v2 Conductor NL planner → v3 full).
  (`khala.md` §5, §11.)
- **B — Verification & economics.** Verification classes wired into the receipt
  (`test_passed` / `seeded_replication` / `exact_trace_replay`), metering,
  Bitcoin settlement to worker + validator, referral (revenue-loop RL-1/2/3).
  (`khala.md` §6–§7, §10.)
- **C — Supply & training.** Pylon serving workers in the pool (capability →
  dispatch → exact-parity receipt → payout); Psionic coordinator-training
  primitives P1–P5; TMAX-style data/stability recipe for the worker side.
  (psionic-coordinator-roadmap; tmax/synthesis.)
- **D — Verse visualization.** The crackling-energy serving view: arcs to Pylons,
  the gateway portal, coding-agent avatars, verify glow, settlement beams, HUD —
  on the Effect-centric multiplayer engine. (`khala-in-the-world.md`.)
- **E — Autopilot consumption.** The cockpit that submits prompts to Khala,
  shows the live Verse view of the serve, and renders/plays the resulting
  artifact.

## Parallel execution plan (delegate this way)

Run the buildout as parallel lanes with one named subagent per lane. Each
subagent owns its slice end-to-end: read the cited docs first, keep changes
inside the owning packages, write receipts/tests for its claims, and post a
handoff note naming blockers, public refs, and exact verification commands. A
lane may use fixtures or inert adapters while its upstream dependency is not
ready, but it must label those results as scaffold evidence, not product proof.

### Agent Nexus — Gateway and Khala serving

**Owns:** M0 and the serving side of M7/M8.
**Primary files:** `apps/openagents.com/workers/api/src/inference/*`,
`docs/inference/khala.md`, `docs/inference/README.md`.
**Read first:** `2026-06-19-gateway-gemini-live-verification.md`,
`2026-06-19-fireworks-provider.md`, `2026-06-19-inference-gateway-business.md`.

**Instructions:**
- Register `openagents/khala-mini`, `openagents/khala-pro`, and
  `openagents/khala-code` behind `/v1/models` without breaking existing model
  ids.
- Grow `ModelRouter` into a stable `Coordinator` interface. Keep v0 heuristic
  routing simple and swappable; learned routing must drop in later without API
  shape changes.
- Turn real provider adapters on behind flags, and return the non-breaking
  `openagents` response block from real provider `usage`.
- Coordinate with Agent Ledger before claiming paid success. Free or owner-grant
  usage can prove route shape, but the paid-credits business requires a
  dereferenceable card/credit/inference receipt.

**First useful output:** an SDK smoke that calls `openagents/khala-mini` and
returns route, worker, usage, `cost_msat`, `price_msat`, and `settled:false`.

### Agent Ledger — Payments, credits, referral, settlement

**Owns:** M3, the paid side of M0, and the #5512/#5520/#5521 gate.
**Primary files:** billing, metering, referral, settlement, asset-boundary, and
no-resale code under `apps/openagents.com/workers/api/src/`.
**Read first:** `2026-06-19-pricing-model.md`,
`2026-06-19-pricing-vs-factory.md`, `2026-06-19-agent-cloud-revshare-everywhere.md`,
issues #5508, #5512, #5520, #5521, #5524.

**Instructions:**
- Keep the staging-before-production rule absolute: no prod live keys or real
  payout arming until #5520 has a full Stripe TEST receipt chain.
- Produce public-safe readback for every money claim: checkout receipt, USD→msat
  bridge/grant receipt, inference charge receipt, referral accrual, and payout
  settlement receipt.
- Preserve the asset boundary: USD-funded credits are inference-spendable, never
  Bitcoin-withdrawable; Bitcoin settlement goes only through the approved
  payout path.
- Do not print secrets or raw payment payloads. Owner-held Stripe/Spark/MDK
  arming remains a `NEEDS-OWNER` handoff, not an agent workaround.

**First useful output:** a staging report proving card/test-credit → funded
balance → metered Khala spend → dereferenceable receipt, with referral accrual
still inert/test unless owner-armed.

### Agent Verifier — Accepted-outcome rubric and receipts

**Owns:** M2 and the verifier contract M6/M7 train against.
**Primary files:** new rubric/harness code near the inference or accepted-outcome
surfaces; product-promise evidence refs as needed.
**Read first:** `docs/sakana/tassadar-run-integration.md`,
`docs/sakana/coordinator-as-verified-work.md`, `docs/research/tmax/synthesis.md`,
issue #6010.

**Instructions:**
- Define the crossy-road rubric as a deterministic, machine-checkable acceptance
  test: single HTML file, loads headless, controls are correct, camera is sane,
  difficulty ramps, restart places the character correctly.
- Keep producer and verifier separate. The model/coding agent may produce the
  artifact; the independent harness decides `test_passed|failed`.
- Return structured verdict details in the receipt so Agent Psion can consume a
  scalar reward and Agent Demo can publish an honest benchmark.
- Add negative fixtures: broken controls, broken camera/restart, missing single
  file, and a non-running HTML result must all fail.

**First useful output:** `khala-code` fixture verification where one known-good
  HTML artifact passes and at least one deliberately broken artifact fails.

### Agent Psion — Learned coordinator and Psionic primitives

**Owns:** M6 and the TRINITY/Conductor training substrate.
**Primary repos/files:** `psionic/` for implementation; this repo only for
OpenAgents integration docs/receipts.
**Read first:** `docs/sakana/trinity.md`, `docs/sakana/conductor.md`,
`docs/sakana/psionic-coordinator-roadmap.md`,
`docs/sakana/adapting-sakana-coordination.md`, issue #6014.

**Instructions:**
- Build in the roadmap order: P4 offline scalar reward harness, P3 sep-CMA-ES
  plus random-search baseline, P1 hidden-state extraction, P2 coordinator head
  and optional SVF, P5 typed worker-pool binding.
- Start offline. Do not wait on live Pylon serving or paid settlement to validate
  optimizer mechanics.
- The verifier role binds to Agent Verifier/Tassadar verdicts, not prompted
  self-judgment.
- Ship learned policies as candidate artifacts in shadow. Promotion requires a
  clean verified-work-per-sat win and a gated `runtime_promotion`.

**First useful output:** an offline `evaluate_coordinator(params) -> scalar`
  harness plus sep-CMA-ES/random-search comparison over a deterministic fixture
  batch.

### Agent Pylon — Fabric supply and worker pool

**Owns:** M4 and the Pylon/Psionic supply adapter.
**Primary files:** `apps/pylon/`, Psionic serving receipts, gateway adapter seam.
**Read first:** `2026-06-19-decentralized-serving-shard-wan.md`,
`2026-06-19-leyten-compute-shard-audit.md`, issue #6012 and DE-4 #5527.

**Instructions:**
- Start with whole-small-model serving on one admitted Pylon. Leave shard-WAN
  large-model serving as a later, receipt-gated lane.
- Build a fabric adapter behind `InferenceProviderAdapter`: ask-plan → execute →
  consume exact-parity receipt.
- Require capability, heartbeat/readiness, wallet/payout readiness, and exact
  parity before payout. No parity, no pay.
- Expose enough worker and receipt metadata for Agent Verse to render in-world
  vs gateway split honestly.

**First useful output:** one Khala request served by a Pylon in a trusted/small
  lane with a public-safe serve receipt, even if settlement stays owner-armed.

### Agent Verse — World visualization

**Owns:** M5 and all Verse rendering of serving events.
**Primary files:** `apps/openagents-world`, `packages/world-contract`,
`packages/world-client`, `@openagentsinc/three-effect`, Autopilot desktop world
visualization files.
**Read first:** `khala-in-the-world.md`,
`docs/sakana/tassadar-fugu-exploration.md`,
`docs/game/2026-06-22-effect-typescript-world-backend-replacement-audit.md`,
issue #6013.

**Instructions:**
- Build the visualization against evidence-bound motion: no animation without a
  Khala receipt or source ref.
- Add `gateway_station` and service-only `upsert_gateway_station`; model request
  activity as a `world_event` payload first, not a broad new authority surface.
- Implement `createCracklingArc` and `createGatewayPortal`; reuse existing Pylon
  stations, agent avatars, payment beams, HUD meters, and source-ref inspectors.
- Use fixtures from Agent Nexus/Pylon while live events are immature, but mark
  fixture-only demos as inert visual proof.

**First useful output:** a local or fixture-driven scene where one Khala receipt
  renders as either nexus→Pylon arc or nexus→gateway portal, clickable back to
  its receipt ref.

**Active claim — 2026-06-22:** this session claims the Agent Verse lane for M5
under parent issue #6013. The claim is deliberately scoped to evidence-bound
world projection and rendering. It does not claim live Khala serving, live Pylon
payout, Bitcoin settlement, or learned coordination; those stay with Agent
Nexus, Agent Pylon, Agent Ledger, and Agent Psion until their receipt-backed
gates land.

**Brief operator plan:**
1. **#6013-A — Contract and bridge shape.** Add the minimal `gateway_station`
   row, service-only `upsert_gateway_station`, and Khala inference
   `world_event` payload shape in `packages/world-contract` and
   `apps/openagents-world`, with source-ref/public-safety tests. This is the
   first implementation slice because it prevents the renderer from inventing
   routes, workers, providers, or receipt refs.
2. **#6013-B — Render primitives.** Implement `createCracklingArc` and
   `createGatewayPortal` in the shared three-effect surface, driven only by
   fixture rows or receipt-backed source refs.
3. **#6013-C — Desktop projection.** Map fixture Khala receipt events through
   `packages/world-client` into the Autopilot Desktop Verse visualization,
   reusing Pylon stations, agent avatars, payment beams, HUD meters, and
   source-ref inspectors.
4. **#6013-D — Live receipt integration.** Replace inert fixture events with
   M1/M4 Khala receipt streams when available, keeping fixture-only demos labeled
   as scaffold evidence and keeping live product claims gated on dereferenceable
   receipts.

**Starting issue:** start on parent #6013 with subissue #6013-A ("Contract and
bridge shape"). Create real GitHub child issues only when multiple agents need
separate queues; until then the #6013 checklist plus these anchors is enough.

### Agent Cockpit — Autopilot consumption and artifact handoff

**Owns:** M1 and the user-facing run path into M8.
**Primary files:** Autopilot client/cockpit surfaces and artifact preview paths.
**Read first:** `khala.md`, `khala-buildout-roadmap.md`, issue #6009.

**Instructions:**
- Add a Khala provider/model selection path that submits to `openagents/khala-*`
  and displays the `openagents` receipt block without exposing private internals.
- Preserve BYO-provider behavior; Khala is an available first-party route, not a
  forced replacement.
- Save/render the returned single-file HTML artifact and hand its refs to Agent
  Verifier for acceptance and Agent Verse for world playback.
- Keep UI claims scoped: show receipt, route, cost, verification, and settlement
  state; do not claim learned coordination until Agent Psion has promoted a
  candidate.

**First useful output:** the crossy-road prompt runs through the cockpit against
  `khala-code`, returns an HTML artifact, displays receipt metadata, and hands
  the artifact to the verifier.

### Agent Demo — Benchmark, metrics, and publication pack

**Owns:** M8 and the comparison evidence package.
**Primary files:** benchmark scripts/docs, public evidence pack, product-promise
refs if a claim is upgraded.
**Read first:** this roadmap, `khala.md`, `khala-in-the-world.md`, issue #6016,
and DE-10 #5533.

**Instructions:**
- Define the head-to-head runbook before the learned coordinator exists so every
  lane knows the measurement target.
- Report tokens, dollars, wall-clock, verification result, cost per accepted
  outcome, accepted outcomes per kWh where measured, in-world vs gateway split,
  and settlement refs.
- Treat external Fugu/Opus numbers as reported claims unless independently
  reproduced; label them separately from OpenAgents measurements.
- Publish wins and losses honestly. A failed Khala run with good receipts is
  still useful evidence; do not turn benchmark copy into product proof without
  receipts and owner sign-off.

**First useful output:** a dry-run evidence template that can ingest one Khala
  run, one frontier baseline run, verifier output, and settlement/Verse refs.

### Parallel merge order

```
Nexus M0 ─┬─► Cockpit M1 ─► Verifier M2 ─► Ledger M3 ─┐
          │                                            ├─► Demo M8
          └─────────────► Verse M5 (fixtures first) ───┤
Pylon M4 ───────────────────────────────┬──────────────┤
Verifier M2 + Pylon M4 ─► Psion M6 ─────┴─► Conductor M7
```

**Work that can start immediately:** Agent Nexus on M0, Agent Ledger on the
staging/paid receipt gate, Agent Verifier on the rubric, Agent Verse on
fixture-driven rendering, Agent Psion on offline P4/P3, Agent Pylon on
whole-small-model adapter shape, Agent Cockpit on receipt display against a mock
Khala response, and Agent Demo on the runbook/template.

**Work that must wait for live evidence:** settlement claims wait on Agent
Ledger; real Pylon-payout claims wait on Agent Pylon + Agent Ledger; learned
router promotion waits on Agent Psion + Agent Verifier + Agent Pylon; published
head-to-head claims wait on Agent Demo with receipt-backed measurements.

## Sequenced milestones

Each milestone converges the workstreams toward the demo; later ones depend on
earlier. EPIC anchors in parentheses where they exist.

**M0 — Khala serves, metered, with a receipt.** *(A, B)*
Flip the gateway stub router to real cheapest-viable (#5482), name
`openagents/khala-mini`, turn on a real adapter (Fireworks #5479 / Vertex #5480),
real per-model decrement (#5477). *Done when:* an OpenAI SDK call to
`khala-mini` returns a metered completion with an `openagents` receipt block.

**M1 — Autopilot calls Khala.** *(E)*
Autopilot submits a prompt to the Khala endpoint and shows the completion +
receipt (route, workers, cost). *Done when:* the crossy-road prompt runs through
Autopilot end-to-end against `khala-code` and returns a single HTML file.

**M2 — Verified coding outcomes.** *(B)*
`khala-code` runs a verification command / headless checks → `test_passed`;
define the crossy-road **rubric** (playable, controls, camera, difficulty ramp,
single file) as the acceptance test. *Done when:* a run reports `verified: true`
against the rubric, with a receipt.

**M3 — Bitcoin settlement on accepted outcomes.** *(B)*
Wire verified-lane settlement (RL-2) to pay the worker + validator; accepted-
outcome pricing for `khala-code`. *Done when:* a verified crossy-road build
settles sats to a contributor with a public receipt.

**M4 — Pylon workers in the pool.** *(C)*
Pylon whole-small-model serving as a worker (capability → dispatch → exact-parity
receipt → payout) via the fabric supply adapter; some of the work now stays
in-world and paid. *Done when:* a Khala request is served by a Pylon with a
parity receipt and a Bitcoin payout.

**M5 — The Verse serving view (P0/P1 of khala-in-the-world).** *(D)*
Project inference events from the activity timeline; render crackling arcs to
assigned Pylons, the gateway portal for external calls, and a coding-agent avatar
for the build — under the evidence-bound motion contract. Build the two new
three-effect primitives (`createCracklingArc`, `createGatewayPortal`) and the
`gateway_station` row. *Done when:* submitting the crossy-road prompt in
Autopilot shows the serve happening live in the world.

**M6 — Learned coordinator (TRINITY lane), shadow-deployed.** *(A, C)*
Psionic primitives P1–P5; train the logit router by sep-CMA-ES on the
verification verdict; ship as a shadow candidate vs the heuristic router; promote
on cost-per-accepted-outcome (a gated `runtime_promotion`). *Done when:* the
learned router beats heuristic on cost-per-accepted-outcome in shadow.

**M7 — Conductor lane (compose to win the benchmark).** *(A, C, D)*
GRPO-trained NL planner (DPPO + FP32 head, TMAX recipe) that decomposes the
crossy-road task — plan (frontier via gateway) → implement (best coding worker) →
verify (rubric/replay) → refine — and the Verse shows the multi-worker fan-out.
*Done when:* `openagents/khala` solves the crossy-road task by composition, beating
single-model cost at comparable quality.

**M8 — The head-to-head demo (north-star).** *(all)*
Run the crossy-road prompt through `openagents/khala` vs a frontier baseline,
side by side: report tokens, $, time, **cost-per-accepted-outcome**,
**accepted-outcomes-per-kWh**, in-world-vs-gateway split, verified-rate; show the
live Verse serve; **play the resulting three.js game inside our three-effect
world**; and show the Bitcoin settlement receipts. *Done when:* we can publish our
own head-to-head with a verified, paid, watchable result.

## Critical path & honest gaps

```
M0 ─► M1 ─► M2 ─► M3 ─┐
            └─► M4 ───┼─► M5 ──┐
                      └─► M6 ──┴─► M7 ─► M8 (head-to-head)
```

- The **request surface, auth, balance gate, adapter registry, payment beams,
  Pylon scene, agent avatars, and multiplayer engine already exist** — M0/M1/M5
  are mostly wiring + two new render primitives.
- The genuinely new capability is the **learned coordinator** (M6/M7) — Psionic
  P1–P5 (no CMA-ES / hidden-state extraction / SVF today) — and **verified coding
  outcomes for arbitrary tasks** (M2: rubric/verification-command discovery).
- Until M6, the demo runs on the **heuristic router** (still a valid head-to-head;
  it just isn't the learned-composition story yet).
- The crossy-road rubric and headless verification harness are net-new and
  gate M2 onward — define them early; they double as a reusable
  accepted-outcome template for the Tassadar coding lane.

## Why this is the right north-star

It exercises every load-bearing claim at once: **composition over scale**
(Khala beats a single model by routing), **verification not trust** (the game is
an accepted outcome with a receipt, not a vibe), **paid open economy** (workers
and validators settle in Bitcoin), and **legibility** (you watch the serve and
play the artifact in our own world). Beating — or honestly losing to — a frontier
model on a concrete, fun task, with every piece auditable and paid, is the most
compelling possible proof that the whole system works.
