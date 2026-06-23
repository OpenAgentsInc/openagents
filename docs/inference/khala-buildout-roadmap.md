# Khala Buildout Roadmap — to the Head-to-Head

_Roadmap — 2026-06-22. The single sequenced buildout that takes Khala from the
existing gateway skeleton to a learned, verified, Bitcoin-settled inference model
that you **consume through Autopilot** and **watch serve in the Verse** — ending
in a concrete north-star: running our own version of the Fugu-Ultra-vs-frontier
head-to-head._

Consolidates the per-area roadmaps: [`khala.md`](khala.md) (spec/§11),
[`khala-in-the-world.md`](khala-in-the-world.md) (visualization),
`docs/sakana/psionic-coordinator-roadmap.md` (coordinator primitives),
`docs/sakana/tassadar-run-integration.md` (verification classes),
`docs/research/tmax/synthesis.md` (recipe/stability), and the revenue-loop spine
(EPIC #5457).

> **Direction (architecture):** the learned coordinator (M6/M7) is not the
> whole story — the longer-run direction is that Khala's inference runs as
> **typed, GEPA-optimizable Blueprint/DSPy programs**, and that the Blueprint
> program layer is **extensible via independently authored capability units**
> (Tassadar-style plugins/modules) composed into Khala programs, metered per
> use, and (FUTURE) paid in Bitcoin with a revenue split. See
> [`2026-06-23-khala-blueprint-program-and-plugin-extensibility.md`](2026-06-23-khala-blueprint-program-and-plugin-extensibility.md)
> for the current-vs-future split and the surfaces it maps to. Nothing there is
> a product promise or a public plugin-marketplace claim.

## North-star goal (the end of the flow)

Run our own version of the head-to-head that's circulating publicly — **Sakana
Fugu Ultra vs. a frontier model**, same prompt, measured side by side:

> Prompt: _"build a really high quality single html file crossy road game with
> three.js"_
>
> **Sakana Fugu Ultra** (reported): ~89k tokens, ~$7.32, 22 min. Issues: inverted
> turn direction, wonky camera, no SFX, not identical to Crossy Road.
> **Claude Opus 4.8 (Ultracode)** (reported): ~940k tokens, ~$37.85, 79 min.
> Issues: stuck twice in a retry loop, wrong character position after restart,
> hard from the start (Fugu correctly ramped difficulty).
> Reported verdict: Opus won on functionality/quality/design; Fugu won on
> speed/cost/efficiency.
> _(Source: external X/Twitter comparison; treat the numbers as reported claims,
> not our measurements.)_

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
and verified-rate. The thesis we're proving: _composition + verification + a paid
open pool_ can match frontier quality at lower cost, and — uniquely — you can
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

Multi-agent hygiene: never stash, reset, checkout, restore, or otherwise move
another agent's uncommitted work out of the way. If the active checkout is dirty
with concurrent lane work, create a fresh worktree from clean `origin/main` for
your scoped edit, test, commit, or push, then leave the original dirty checkout
intact and name any blocker in the handoff.

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

**Active claim — 2026-06-22:** this session claims the Agent Verifier lane for
M2 under parent issue #6010. The claim is scoped to the accepted-outcome rubric,
independent headless verification harness, fixture corpus, and receipt verdict
shape. It does not claim M0/M1 gateway or cockpit wiring, live Bitcoin
settlement, Pylon payout, Verse rendering, or learned coordinator promotion.

**Brief operator plan:**

1. **#6010-A — Rubric contract.** Specify the crossy-road acceptance schema:
   single HTML artifact, load/run result, control-direction probes, camera sanity,
   difficulty-ramp evidence, restart-position evidence, verdict enum, failure
   reasons, and public-safe receipt fields. Keep the verifier role independent
   of the producer so Agent Psion trains on mechanical verdicts, not model
   self-judgment.
2. **#6010-B — Fixture corpus.** Add one known-good single-file HTML fixture and
   negative fixtures for inverted controls, broken camera/restart, multi-file or
   missing artifact, and non-running HTML. These fixtures define the first stable
   regression surface before any live `khala-code` artifact is trusted.
3. **#6010-C — Headless harness.** Build the deterministic browser runner with
   bounded timeouts, console/error capture, input simulation, screenshot or state
   probes where useful, and a structured `test_passed|failed` verdict.
4. **#6010-D — Receipt bridge.** Thread the verdict details into the Khala
   receipt shape as scaffold evidence first, then into the real request flow once
   Agent Nexus/Cockpit provide M0/M1 artifacts. Keep fixture-only evidence labeled
   as scaffold, not product proof.
5. **#6010-E — Reward handoff.** Publish the scalar/verdict mapping that Agent
   Psion can consume for M6/M7 and the evidence fields Agent Demo needs for the
   head-to-head pack.

**Code-landed progress — 2026-06-22 (#6010):**

- **#6010-A — Rubric contract:** `khala-code` now has a deterministic
  crossy-road rubric contract (`single_html_file`, `loads_and_runs_headless`,
  `direction_controls`, `sane_follow_camera`,
  `difficulty_ramps_with_progress`, `restart_resets_character`) with public-safe
  failure reasons and receipt refs.
- **#6010-B — Fixture corpus:** the verifier test corpus includes one
  known-good single-file HTML fixture and negative fixtures for broken controls,
  broken restart, external/multi-file dependency, and missing difficulty ramp.
- **#6010-C — Headless harness:** `bun run khala-code:verify -- <artifact.html>`
  runs the Playwright Chromium headless probe, simulates Arrow/WASD input,
  checks restart reset through the artifact probe hook, and emits the same
  verifier verdict shape.
- **#6010-D — Receipt bridge:** `POST /v1/chat/completions` now recognizes
  `openagents/khala-code`, routes it through the priced open/code lane, and
  attaches `verification: unverified|failed|test_passed`, `executed`, `verified`,
  rubric checks, verifier receipt, verifier command ref, charge receipt URL when
  metered, and worker provenance in the `openagents` block. A fresh hot-path
  artifact that only passes the cheap pre-screen stays `unverified` until the
  out-of-Worker acceptance runner posts an executed verdict callback.
- **#6010-E — Reward handoff:** the verdict carries `scalar_reward` plus a
  public-safe `accepted_outcome.khala_code.crossy_road...` handoff ref for Psion
  and later accepted-outcome pricing.

**Issue organization used:** keep #6010 as the tracking issue for the verifier
lane until the next agent needs parallel ownership. Future split candidates are
`#6010-F` for arbitrary repo verification-command discovery and `#6010-G` for
live accepted-outcome pricing/settlement once M6/M7 are ready.

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

### Agent Verse — World visualization ✅ DONE (M5 / #6013, live-proven 2026-06-22)

> **Complete & live.** A real owner-enabled Khala receipt (`chatcmpl_5577d36f…`)
> flows gateway → public `khala_inference_served` event → D1 `gateway_station`+
> `world_event` rows → live region snapshot → desktop Verse render
> (`gatewayLinkOk:true`) → receipt/source-ref inspector. `openagents-world`
> deployed `5778bf03`; `createCracklingArc`/`createGatewayPortal` shipped in
> `@openagentsinc/three-effect`. Remaining is richness (P1 coding-agent avatars +
> verify glow, P2 fan-out + HUD, real-Pylon worker avatars once M4 lands), not
> existence. The lane plan below is retained as historical record.

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

**Progress — 2026-06-22:** #6013-A is implemented in the shared contract and
world service: `gateway_station`, service-only `upsert_gateway_station`, typed
Khala inference `world_event` payloads, bridge mapping from public Khala receipt
shapes, and read-model mirroring all have focused tests. #6013-B and #6013-C
are also implemented: `@openagentsinc/three-effect` now exposes the
receipt-driven crackling-arc and gateway-portal primitives, and Autopilot
Desktop projects Khala inference rows into the Verse scene with receipt-source
inspectability. #6013-D is partially implemented: the public activity timeline
now has a receipt-backed `khala_inference_served` event kind, Worker source
wiring from paid inference charge receipts, and a world bridge mapper that turns
those public events into `gateway_station` + `world_event` rows without exposing
private prompts, providers, or amounts. The world worker also has a scheduled
public activity timeline poller for that source, cursor persistence, manual poll
route, and fresh-socket D1 snapshot hydration so projected rows can reach new
Verse clients after the poll. Remaining #6013 work stays under #6013-D: deploy
and run the live bridge path, prove cursor/source-ref replay against production,
and capture an owner-enabled Khala smoke showing a real receipt flowing from
gateway → timeline → world → desktop.

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

**Active claim — 2026-06-22:** this session claims the Agent Demo lane for M8
under parent issue #6016. The claim is scoped to the benchmark runbook, evidence
manifest, metric reducers, and publication-pack skeleton for the head-to-head.
It does not claim M0/M1 gateway or cockpit wiring, the already-claimed Agent
Verifier or Agent Verse lanes, live Bitcoin settlement, Pylon payout, or learned
coordinator promotion; those remain upstream receipt sources that Agent Demo
ingests only after they become dereferenceable.

**Brief operator plan:**

1. **#6016-A — Evidence schema and runbook.** Define the dry-run manifest for
   one Khala run, one frontier baseline, verifier verdicts, settlement refs,
   Verse refs, artifact refs, and external reported-claim citations. Separate
   observed OpenAgents measurements from reported Fugu/Opus numbers.
2. **#6016-B — Metric reducers.** Add calculators for tokens, dollars,
   wall-clock, cost per accepted outcome, verified rate, in-world vs gateway
   split, and AO/kWh when measured telemetry exists. Missing energy telemetry
   must stay `not_measured`, not estimated into a claim.
3. **#6016-C — Fixture evidence pack.** Build a fixture-only example that
   exercises the template with inert Khala, verifier, settlement, and Verse refs
   so every downstream lane can see the expected evidence shape before live
   receipts land.
4. **#6016-D — Publication skeleton.** Prepare the comparison doc structure:
   setup, methodology, raw inputs, accepted-outcome verdict, payment/settlement
   refs, Verse playback refs, artifact playback refs, and honest losses.
5. **#6016-E — Live promotion gate.** Replace fixture refs only when M3/M5/M7
   provide dereferenceable evidence. Any world-first, AO/kWh, or public product
   claim upgrade remains blocked on DE-10 acceptance and owner sign-off.

**Starting issue:** start on #6016 with subissue anchor #6016-A ("Evidence schema
and runbook"). Create real GitHub child issues only when separate agents need
independent queues; until then the #6016 checklist plus these anchors is enough.

**Progress — 2026-06-22:** #6016-A and #6016-B have a repo-owned scaffold in
`docs/inference/khala-head-to-head-demo.md`,
`docs/inference/fixtures/khala-head-to-head-dry-run.v1.json`, and
`scripts/khala-demo/reduce-head-to-head.mjs`. The reducer validates public-safe
evidence shape, computes the M8 metrics, and emits a closure audit that keeps
#6016 blocked while evidence is fixture-only. #6016-C has a first inert fixture
pack. `scripts/khala-demo/render-publication.mjs` adds the #6016-D publication
draft renderer so fixture and future live packs share one visible scoreboard.
#6016-E now has a reducer-backed `livePromotionAudit` that blocks closure when
live manifests still contain fixture refs or miss live Khala/frontier runs,
settlement, Verse playback, playable artifact, energy telemetry, or publication
refs. #6016-D/E still need live publication refs and M3/M5/M7 receipts.

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

## Next-wave delegation — 2026-06-22 (post first wave)

The first wave landed: **M0** code (catalog alias + `openagents` disclosure block,
owner-gated for live), **M2** done (khala-code rubric + Playwright verifier),
**M5** partial (world-contract gateway projection + desktop Verse projection), and
**M8** self-gating scaffold (reducer/closure-audit/publication, blocked on live
evidence). Audit: `docs/launch/2026-06-22-khala-cloud-buildout-audit.md`.

The single highest-leverage step is **NEEDS-OWNER**, not an agent lane:

> **NEEDS-OWNER (unblocks M0-live, M1, M8):** wire one provider secret
> (Fireworks is verified-live and cheapest) and set `INFERENCE_GATEWAY_ENABLED=on`
> on a **staging/preview** Worker, then run the M0 live SDK smoke. Prod after
> staging proves out. No agent can flip prod secrets/flags.

### Payments: Bitcoin-only, and pay the guinea-pig Pylon first

Owner direction (2026-06-22): **all payment work is Bitcoin-only for now**, with
**Spark as the primary payout method** (over Lightning). **No Stripe.** (MDK is
checkout-only and not relevant to payouts; card/credit funding is explicitly out
of scope this wave.) Bitcoin/Spark lets us move real money in testing without
prod card secrets or fragile env wiring, so M3 is **agent-testable today** rather
than gated on a card-payment chain.

There is a designated **test payout target — the "guinea-pig" Pylon**: a live
online Pylon in an Autopilot instance whose **Spark receive address** is in
`/Users/christopherdavid/work/.secrets/khala-test-payout.env`
(`KHALA_TEST_PAYOUT_SPARK_ADDRESS`, gitignored, shared across all agent
sessions — do not commit it into tracked files). **Rule for every lane:** in
payment/serving tests, **pay this node first**, route test work/assignments to
it, and use its Pylon as the primary serving worker, so the first end-to-end
"verified work → Bitcoin to a real contributor" loop lands on a known node we
control.

Below is the next wave as **5 parallel lanes** that do not block on that owner
step (they build against the stub adapter / fixtures / staging) and **do not
collide**. Every lane: work on a **fresh worktree off clean `origin/main`**
(never touch another agent's dirty checkout — CLAUDE.md rule); read the cited
docs first; keep edits inside your owned paths; post a claim + status comment on
your issue; label fixture/inert results as scaffold, never product proof.

### Shared-seam ownership (collision rules)

A few files are touched by more than one lane. To avoid stepping on each other:

| Shared seam | Sole owner (edits) | Everyone else |
|---|---|---|
| `src/inference/provider-adapter.ts` (registry/interface) + adapter registration | **Lane B (Supply)** | read-only; request a registration via Lane B |
| `src/inference/chat-completions-routes.ts` `openagents` block | **Lane A (Cockpit)** may extend *consumption*; **Lane B/E** add fields only via PR review | coordinate field additions in the issue first |
| `src/inference/metering-hook.ts` + billing/referral/settlement | **Lane E (Ledger)** | read-only |
| `packages/world-contract` + `apps/openagents-world` bridge | **Lane D (Verse)** | read-only |
| `@openagentsinc/three-effect` (separate repo) | **Lane D (Verse)** | n/a |
| `psionic` repo | **Lane C (Psion)** | n/a (different repo) |

If two lanes must both change one file, the **owner** lands the change and the
other rebases — never edit the same file from two worktrees in flight.

### Lane A — Cockpit (M1, #6009)

**Goal:** Autopilot submits a prompt to `openagents/khala-*` and renders the
`openagents` receipt (route, worker, cost, verification). **Owns:**
`apps/autopilot-desktop` cockpit + the web client call path. **Reads:**
`docs/inference/khala.md` §3, `chat-completions-routes.ts` (read-only).
**Not blocked by owner:** build against a local/staging gateway or the stub
adapter; gate the "live" badge on a real receipt. **First output:** the
crossy-road prompt round-trips through the cockpit and shows the receipt block.
**Collision rule:** consume the `openagents` block; do not change its shape
without Lane B/E sign-off.

### Lane B — Supply / Pylon (M4, #6012)

**Goal:** the **fabric supply adapter** (gateway↔Psionic: ask-plan → execute →
consume exact-parity receipt), whole-small-model serving first; register it
behind the existing adapter registry. **Owns:** the new adapter file +
`provider-adapter.ts` registration + the Psionic serving seam.
**Reads:** `docs/inference/2026-06-19-decentralized-serving-shard-wan.md`,
`apps/pylon`. **Not blocked by owner:** prove the adapter against a local Psionic
serve + parity receipt; defer shard-WAN. **Use the guinea-pig Pylon as the
primary test worker** — dispatch test assignments to the node at
`KHALA_TEST_PAYOUT_SPARK_ADDRESS` (`.secrets/khala-test-payout.env`) so its
serve → parity receipt → Bitcoin payout (with Lane E) is the first proven loop.
**First output:** a Khala request served by that Pylon adapter returns an
exact-parity receipt in tests. **Collision rule:** sole owner of the adapter
registry; other lanes request registrations.

### Lane C — Psion (M6, #6014) — the long pole, start now

**Goal:** Psionic primitives **P1–P5** (hidden-state extraction → coordinator
head + SVF → sep-CMA-ES optimizer → scalar terminal-reward adapter → typed
worker-pool binding), then a first shadow run rewarded by the M2 verdict.
**Owns:** the `psionic` repo (fully isolated — different repo, zero collision).
**Reads:** `docs/sakana/psionic-coordinator-roadmap.md`,
`docs/sakana/coordinator-as-verified-work.md`, `docs/research/tmax/synthesis.md`.
**First output:** P1 hidden-state extraction with a reproducibility test, then the
ES optimizer on a trivial head. **Note:** needs real ML-training compute later;
begin the primitives now so they are ready when M0–M5 are green.

### Lane D — Verse (M5, #6013)

**Goal:** finish the serving visualization — the `three-effect` primitives,
world contract row shape, public activity timeline mapper, scheduled bridge
poller, D1 snapshot hydration, and desktop projection path are now landed. The
remaining work is the **live proof**: deploy/run the bridge path against the
real public timeline and capture a dereferenceable owner-enabled Khala receipt in
the desktop Verse scene. **Owns:** `@openagentsinc/three-effect` (separate
repo), `apps/openagents-world` bridge inference-event path, the desktop Verse
scene. **Reads:** `docs/inference/khala-in-the-world.md`. **Issue organization:**
continue under parent #6013 with #6013-D/live-proof; create child issues only if
another agent needs a separate queue.

### Lane E — Ledger (M3, #6011) — **Bitcoin-only**

**Goal:** the end-to-end **Bitcoin** money loop — metered Khala spend →
verified-work payout **settled over Spark** (our primary payout method; Lightning
as the rail) to a real contributor, with a dereferenceable receipt. **No Stripe /
no card funding this wave** (MDK is checkout-only, not used here); use the Spark
payout path the tip / treasury tests already exercise. **Owns:** `metering-hook.ts`
+ the Bitcoin
settlement/referral path. **Reads:** the revenue-loop spine (#5457, RL-1/RL-2/
RL-3), the existing tip/treasury runbooks, `docs/inference/2026-06-19-pricing-model.md`
(for the sats pricing basis). **First output, and the milestone proof:** route a
small verified-work payout to the **guinea-pig Pylon's Spark address**
(`.secrets/khala-test-payout.env`) and show the settled receipt
(`realBitcoinMoved:true`) — **get that node paid first.** Then parallelize
payouts to additional workers. Keep amounts small and treasury-bounded; this is
real money. **Collision rule:** sole owner of the metering/settlement path.

### What still waits (do NOT fake)

- **M7 (Conductor)** waits on Lane C (M6) — no parallel start; it is the next
  thing after the learned substrate exists.
- **M8 live run** waits on the owner gateway-enable + Lanes A/E (a real metered,
  settled completion) and ideally M6/M7. The M8 harness already refuses to close
  on fixture evidence (`closureAudit.canClose:false`); feed it real refs, don't
  add more scaffold.

## Sequenced milestones

Each milestone converges the workstreams toward the demo; later ones depend on
earlier. EPIC anchors in parentheses where they exist.

**M0 — Khala serves, metered, with a receipt.** _(A, B)_
Flip the gateway stub router to real cheapest-viable (#5482), name
`openagents/khala-mini`, turn on a real adapter (Fireworks #5479 / Vertex #5480),
real per-model decrement (#5477). _Done when:_ an OpenAI SDK call to
`khala-mini` returns a metered completion with an `openagents` receipt block.

**M1 — Autopilot calls Khala.** _(E)_
Autopilot submits a prompt to the Khala endpoint and shows the completion +
receipt (route, workers, cost). _Done when:_ the crossy-road prompt runs through
Autopilot end-to-end against `khala-code` and returns a single HTML file.

**M2 — Verified coding outcomes.** _(B)_
`khala-code` runs a verification command / headless checks → `test_passed`;
define the crossy-road **rubric** (playable, controls, camera, difficulty ramp,
single file) as the acceptance test. _Done when:_ a run reports `verified: true`
against the rubric, with a receipt.

**M3 — Bitcoin settlement on accepted outcomes.** _(B)_
Wire verified-lane settlement (RL-2) to pay the worker + validator; accepted-
outcome pricing for `khala-code`. _Done when:_ a verified crossy-road build
settles sats to a contributor with a public receipt.

**M4 — Pylon workers in the pool.** _(C)_
Pylon whole-small-model serving as a worker (capability → dispatch → exact-parity
receipt → payout) via the fabric supply adapter; some of the work now stays
in-world and paid. _Done when:_ a Khala request is served by a Pylon with a
parity receipt and a Bitcoin payout.

**M5 — The Verse serving view (P0/P1 of khala-in-the-world).** _(D)_
Project inference events from the activity timeline; render crackling arcs to
assigned Pylons, the gateway portal for external calls, and a coding-agent avatar
for the build — under the evidence-bound motion contract. Build the two new
three-effect primitives (`createCracklingArc`, `createGatewayPortal`) and the
`gateway_station` row. _Done when:_ submitting the crossy-road prompt in
Autopilot shows the serve happening live in the world.

**M6 — Learned coordinator (TRINITY lane), shadow-deployed.** _(A, C)_
Psionic primitives P1–P5; train the logit router by sep-CMA-ES on the
verification verdict; ship as a shadow candidate vs the heuristic router; promote
on cost-per-accepted-outcome (a gated `runtime_promotion`). _Done when:_ the
learned router beats heuristic on cost-per-accepted-outcome in shadow.

**M7 — Conductor lane (compose to win the benchmark).** _(A, C, D)_
GRPO-trained NL planner (DPPO + FP32 head, TMAX recipe) that decomposes the
crossy-road task — plan (frontier via gateway) → implement (best coding worker) →
verify (rubric/replay) → refine — and the Verse shows the multi-worker fan-out.
_Done when:_ `openagents/khala` solves the crossy-road task by composition, beating
single-model cost at comparable quality.

**M8 — The head-to-head demo (north-star).** _(all)_
Run the crossy-road prompt through `openagents/khala` vs a frontier baseline,
side by side: report tokens, $, time, **cost-per-accepted-outcome**,
**accepted-outcomes-per-kWh**, in-world-vs-gateway split, verified-rate; show the
live Verse serve; **play the resulting three.js game inside our three-effect
world**; and show the Bitcoin settlement receipts. _Done when:_ we can publish our
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
