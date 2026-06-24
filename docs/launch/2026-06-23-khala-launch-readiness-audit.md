# Khala Launch-Readiness Audit And Issue Order

Date: 2026-06-23

Scope: Khala/OpenAgents launch readiness after the 2026-06-23 hardening run
through OpenAgents `#6106` and Psionic `#1141`.

This is a coordination audit, not a production authority. The production
contracts remain the code, receipts, product-promises registry, invariant
ledgers, and owner-armed runtime configuration.

## Executive Verdict

Khala is close to a credible paid public endpoint at the protocol and receipt
layer, but it is not yet a fully launchable public production product without
owner/compute/payment arming and a final live readiness smoke.

The strongest parts are already landed:

- OpenAI-compatible model/catalog/chat surfaces and paid-gateway policy.
- Public `/khala`, `/terms`, and `/privacy` pages.
- Request lifecycle telemetry with honest `not_measured` sentinels.
- Prefix-cache, streaming/async, verifier, benchmark, quantization, and
  speculation vocabulary.
- Fixture-backed Pylon capability evidence and Psionic M8 comparison harness.

The launch blockers are not mostly copy or route shape now. They are the final
authority flips and evidence:

- A live gateway-readiness proof against production arming:
  `INFERENCE_GATEWAY_ENABLED`, provider secrets, lane arming, model catalog,
  balance gate, metered completion, and dereferenceable receipt.
- Real model-serving authority for any lane represented as production-ready,
  especially Pylon/OpenAgents-network lanes.
- Billing/credits and MPP/Stripe production profile proof.
- Settlement caps and live payout flips for accepted outcome / contributor
  economics.
- A real decision-grade benchmark sweep, not fixture-only benchmark math.
- Verse visualization wiring for Khala interaction and immediate in-world
  effects.
- SLA/observability and incident runbooks for launch operations.

Recommended launch posture today: keep Khala public pages live as an explainer
and API preview, run owner-gated dogfood against funded accounts, and do not
market a broad paid production launch until the P0 critical path below is green.

## Evidence Map

Primary docs:

- `docs/khala/khala.md`
- `docs/khala/khala-buildout-roadmap.md`
- `docs/inference/inference-engineering-book/IMPLEMENTATION_LOG.md`
- `docs/inference/inference-engineering-book/khala-investigation-notes.md`
- `docs/khala/2026-06-23-khala-telemetry-scorecard-book-p0-1.md`
- `docs/khala/2026-06-23-khala-benchmark-harness-book-p1-5.md`
- `docs/khala/2026-06-23-khala-quantization-eval-gate-book-p1-7.md`
- `docs/khala/2026-06-23-khala-speculation-telemetry-book-p1-8.md`
- `docs/khala/2026-06-23-khala-head-to-head-m8-status.md`
- `docs/game/2026-06-22-talk-to-khala-from-verse-audit.md`

Primary code surfaces:

- `apps/openagents.com/workers/api/src/inference`
- `apps/openagents.com/apps/web/src/page/loggedOut/page/khala.ts`
- `apps/openagents.com/apps/web/src/route.ts`
- `apps/openagents.com/workers/api/src/worker-routes.ts`
- `apps/pylon/src/serving-capability.ts`
- `apps/pylon/src/serving-benchmark.ts`
- `apps/pylon/src/serving-receipt.ts`

2026-06-23 follow-up: the legacy unauthenticated Concierge onboarding turn
route no longer injects caller-provided `verticalOverlay` prose into the system
prompt. It now accepts an explicit bounded `vertical` enum or normalizes old
storage values into server-owned Autopilot Concierge guidance. The browser
`/autopilot` flow now also sends only the bounded `vertical` enum to that
transport instead of carrying client-side prompt guidance. This hardens one
legacy #6148 path, but it does not close the remaining launch gates around the
authenticated/metered `/v1` migration, old-route keep/deprecate/wrap policy,
structured output handling, production deploy smoke, or owner sign off.

Psionic coordinator context:

- `OpenAgentsInc/psionic#1141`
- `crates/psionic-train/src/coordinator_m8_head_to_head.rs`
- `docs/KHALA_M6_M7_COORDINATOR_PLAN.md`

Local caution: the normal `/Users/christopherdavid/work/psionic` checkout was
observed behind `origin/main`; use a fresh fast-forward before continuing
Psionic implementation work.

## Done Or Live Enough To Build On

### Public Pages

Status: landed in OpenAgents `main`.

Relevant files:

- `apps/openagents.com/apps/web/src/page/loggedOut/page/khala.ts`
- `apps/openagents.com/apps/web/src/page/terms.ts`
- `apps/openagents.com/apps/web/src/page/privacy.ts`
- `apps/openagents.com/apps/web/src/route.ts`
- `apps/openagents.com/workers/api/src/worker-routes.ts`

Notes:

- `/khala` presents the public API story, the two current model ids
  `openagents/khala-mini` and `openagents/khala-code`, API-key registration, SSE
  streaming, receipts, and credit framing.
- `/terms` and `/privacy` were restored and updated for current operations.
- Earlier session smoke reported `/khala`, `/terms`, and `/privacy` returning
  200 unauthenticated after deployment.

Open item:

- The website agent has claimed the persistent `/landing` to `/khala` scene
  transition separately. Do not edit those web route/view files from this lane.

### Gateway Shape And Readiness Projection

Status: code exists; production launch depends on runtime arming and live smoke.

Relevant files:

- `apps/openagents.com/workers/api/src/inference/model-catalog.ts`
- `apps/openagents.com/workers/api/src/inference/model-router.ts`
- `apps/openagents.com/workers/api/src/inference/models-routes.ts`
- `apps/openagents.com/workers/api/src/inference/chat-completions-routes.ts`
- `apps/openagents.com/workers/api/src/inference/gateway-readiness.ts`
- `apps/openagents.com/workers/api/src/inference/gateway-readiness-routes.ts`

Done:

- Public model catalog derives model price/policy from the same pricing table as
  metering.
- Routing is bounded by model id/family and typed lane plans, not prompt intent
  matching.
- `GET /v1/gateway/readiness` projects the same model catalog and lane arming
  used by the gateway surfaces into a public-safe readiness fact.

Launch gate:

- Run a production arming smoke that proves the readiness endpoint reports at
  least degraded-with-servable-models, then execute a metered SDK call against
  `openagents/khala-mini` and dereference the receipt.

### Telemetry, Receipts, And Measurement Discipline

Status: strong foundation; some fields still honestly `not_measured`.

Relevant files:

- `apps/openagents.com/workers/api/src/inference/khala-telemetry.ts`
- `apps/openagents.com/workers/api/src/inference/chat-completions-routes.ts`
- `apps/openagents.com/workers/api/src/inference/batch-job-closeout-receipts.ts`
- `docs/khala/2026-06-23-khala-telemetry-scorecard-book-p0-1.md`

Done:

- Canonical `openagents.khala.telemetry.v1` record exists.
- Immediate response block stays small; dereferenceable receipt carries depth.
- Numeric fields distinguish measured values from `not_measured`.
- Streaming/async request classes and batch wait fields are modeled.

Launch gap:

- Provider/gateway/verifier/settlement timing split, region, fallback reason,
  and some economics remain unwired or `not_measured` on hot paths. This is
  acceptable for dogfood if disclosed, but should be closed before a broad
  performance or margin claim.

### Executed Verifier And M8 Measurement

Status: verifier truth improved; public north-star remains not fully green.

Relevant files/docs:

- `apps/openagents.com/workers/api/src/inference/acceptance-runner`
- `apps/openagents.com/workers/api/src/inference/khala-code-verifier.ts`
- `docs/khala/2026-06-23-khala-head-to-head-m8-status.md`
- `scripts/khala-demo/artifacts/khala-crossy-road-northstar-passing.v1.html`

Done:

- The old static `verified:true` problem was corrected by executing artifacts
  through the headless acceptance runner.
- A bare north-star prompt fails honestly.
- A contract-augmented artifact passes 6/6 executed acceptance checks.

Launch gap:

- The gateway pre-screen still rejects CDN-loaded three.js even when the
  standalone executed runner accepts the artifact. That means the production
  `khala-code` path still needs verifier-contract injection / pre-screen
  correction before it can claim the bare product path produces accepted
  artifacts.

### Benchmark, Quantization, And Speculation

Status: fixture-proven machinery, not decision-grade serving evidence.

Relevant files:

- `apps/openagents.com/workers/api/src/inference/benchmark`
- `apps/openagents.com/workers/api/src/inference/khala-quantization.ts`
- `apps/openagents.com/workers/api/src/inference/khala-quantization-guard.ts`
- `apps/openagents.com/workers/api/src/inference/khala-quantization-eval-gate.ts`
- `apps/openagents.com/workers/api/src/inference/khala-speculation.ts`

Done:

- Typed benchmark matrix and report shape exist.
- Real benchmark lane is explicitly owner-armed.
- Quantization metadata, same-model guard, and eval-gate logic exist.
- Speculation telemetry and dynamic-disablement policy exist.

Launch gap:

- No decision-grade real traffic benchmark, real quantized-vs-original sweep, or
  real speculative decode lane has been run/armed. Keep these as infrastructure
  readiness, not product performance claims.

### Pylon Serving Capability Evidence

Status: schema/self-benchmark/receipt shape exists; real GPU serving remains
compute/owner-gated.

Relevant files:

- `apps/pylon/src/serving-capability.ts`
- `apps/pylon/src/serving-benchmark.ts`
- `apps/pylon/src/serving-receipt.ts`
- `apps/pylon/docs/serving-capability-evidence.md`

Done:

- Pylon capability evidence names engine, memory, bandwidth, interconnect,
  residency, cold-start posture, quantization, self-benchmark receipt refs, and
  blockers.
- Real-GPU adapter is not silently active.

Launch gap:

- No live vLLM/SGLang/TensorRT-LLM Pylon worker evidence is wired into the
  production Khala routing/payout path. Keep OpenAgents-network/Pylon supply out
  of public production claims until canary/replay/parity receipts exist.

### Psionic Coordinator And M8 Harness

Status: fixture/offline harness is landed; paid live composition is not armed.

Relevant file in Psionic:

- `crates/psionic-train/src/coordinator_m8_head_to_head.rs`

Done:

- M8 head-to-head harness compares composed vs single-model arms with accepted
  rate, cost-per-accepted-outcome, and verified-work-per-sat.
- `ComposeToWinCheaper` requires cheaper composition at comparable quality.

Launch gap:

- The live paid lane needs a real Pylon pool, armed verdict source, spend caps,
  and a paid shadow/composition win. Current evidence is fixture/offline unless
  separately armed and recorded.

### Verse Visualization

Status: call path and rendering primitives exist; the immediate in-world Khala
interaction is not finished.

Relevant doc:

- `docs/game/2026-06-22-talk-to-khala-from-verse-audit.md`

Done:

- Desktop call path via `khalaTurn` can stream a Khala response.
- Owner desktop token/credits path is usable for MVP dogfood.
- Public activity/world projection already has M5-style inference visualization
  material.

Missing:

- In-world textbox/HUD input.
- Submit wiring from that textbox to `khalaTurn`.
- Live streamed token rendering in-world.
- Immediate local crackling-arc effect from the receipt, rather than waiting for
  public timeline polling.

## Critical Path Issue Order

### P0-1: Production Gateway Readiness Smoke

Goal: prove the live production endpoint can serve paid Khala requests with a
real receipt.

Owner: OpenAgents web/Worker agent.

Scope:

- `apps/openagents.com/workers/api/src/inference/gateway-readiness*`
- `apps/openagents.com/workers/api/src/inference/models-routes.ts`
- `apps/openagents.com/workers/api/src/inference/chat-completions-routes.ts`
- docs/runbook under `docs/launch/`

Acceptance:

- `GET /v1/gateway/readiness` returns servable model count > 0 in production.
- `GET /v1/models` lists the intended Khala ids.
- An authenticated SDK request to `openagents/khala-mini` completes.
- The response includes an `openagents` block and dereferenceable receipt.
- No raw secrets are logged or pasted into docs.

### P0-2: Credits/Billing/MPP Production Proof

Goal: prove public money-in to Khala spend is safe before broad launch.

Owner: billing/payment agent.

Scope:

- `apps/openagents.com/workers/api/src/inference/mpp`
- `apps/openagents.com/workers/api/src/inference/card-credit-*`
- `apps/openagents.com/workers/api/src/inference/usd-credit-bridge.ts`
- product-promise evidence docs

Acceptance:

- Stripe/MPP live profile or approved staging-to-prod path is documented.
- Card/test-credit to funded balance to metered inference receipt is proven.
- USD credit balance remains inference-spendable, not Bitcoin-withdrawable.
- Product promise state is updated only when receipt-first evidence exists.

### P0-3: Khala-Code Executed Verifier Product Path

Goal: make the production `khala-code` path align with the executed acceptance
runner.

Owner: verifier/gateway agent.

Scope:

- `apps/openagents.com/workers/api/src/inference/khala-code-verifier.ts`
- `apps/openagents.com/workers/api/src/inference/acceptance-runner`
- `docs/khala/2026-06-23-khala-head-to-head-m8-status.md`

Acceptance:

- The gateway can verify the same class of artifact that the headless runner
  accepts, including a pinned CDN three.js dependency when policy allows it.
- The verifier state contract is injected or otherwise supplied for
  `khala-code` runs.
- Static pre-screen and executed verdict remain separate receipt fields.
- `verified:true` is impossible without executed verifier evidence.

### P0-4: Settlement Caps And Accepted-Outcome Payout Gate

Goal: keep paid contributor economics safe while enabling the first accepted
outcome settlement.

Owner: settlement/payment agent.

Scope:

- `apps/openagents.com/workers/api/src/inference/khala-accepted-outcome-settlement.ts`
- `apps/openagents.com/workers/api/src/inference/khala-verified-work-settlement.ts`
- payout target admission and product-promise evidence docs

Acceptance:

- Real payout path has owner gate, allowlist, per-run/per-day caps, idempotency,
  and rollback/disable instructions.
- Public receipt distinguishes credited, pending, rejected, simulated, and
  settled states.
- No contributor earning claim turns green before real settlement receipt.

### P0-5: Public Copy And Promise Gate Review

Goal: ensure public pages and product promises do not outrun the evidence.

Owner: product/docs agent.

Scope:

- `apps/openagents.com/apps/web/src/page/loggedOut/page/khala.ts`
- `docs/promises`
- `/terms` and `/privacy` review notes

Acceptance:

- `/khala` copy matches actual armed model ids, pricing, registration, and
  receipt guarantees.
- Legal copy has owner/legal review before broad launch.
- Product promise states stay red/yellow unless exact evidence refs support
  green.

### P1-1: Decision-Grade Benchmark Sweep

Goal: replace fixture benchmark numbers with real traffic measurements.

Owner: benchmark/inference agent.

Scope:

- `apps/openagents.com/workers/api/src/inference/benchmark`
- `docs/khala/2026-06-23-khala-benchmark-harness-book-p1-5.md`

Acceptance:

- Realistic traffic shapes are sourced from actual Khala usage.
- `RealLaneExecutor` is owner-armed explicitly.
- Report is marked decision-grade only when real traffic and real lanes were
  used.

### P1-2: Pylon Real Serving Adapter

Goal: turn Pylon serving capability evidence into a canaryable supply lane.

Owner: Pylon/serving agent.

Scope:

- `apps/pylon/src/serving-*`
- `apps/openagents.com/workers/api/src/inference/openagents-network-adapter.ts`
- `apps/openagents.com/workers/api/src/inference/khala-pylon-admission.ts`

Acceptance:

- A real Pylon worker publishes capability evidence and self-benchmark receipt.
- Canary/replay/parity receipt passes before routing paid traffic.
- Routing and payout remain gated until admission succeeds.

### P1-3: Psionic M8 Real Evaluation Path

Goal: connect the fixture M8 harness to a real owner-armed comparison.

Owner: Psionic agent.

Scope:

- Psionic `crates/psionic-train/src/coordinator_m8_head_to_head.rs`
- Psionic coordinator docs
- OpenAgents M8 docs as readback only

Acceptance:

- Local Psionic checkout is fast-forwarded first.
- Real evaluation remains capped and owner-armed.
- Results report back to OpenAgents docs without modifying unrelated Worker
  implementation.

### P1-4: Verse Khala Interaction MVP

Goal: make Khala visible and usable from the Verse desktop experience.

Owner: desktop/Verse agent.

Scope:

- `apps/autopilot-desktop`
- `docs/game/2026-06-22-talk-to-khala-from-verse-audit.md`
- `three-effect` only if new render primitives are needed

Acceptance:

- In-world textbox/HUD submits to `khalaTurn`.
- Streamed tokens render in-world.
- Receipt triggers immediate local crackling effect.
- Public timeline projection remains evidence-bound.

### P2: Observability, SLA, And Operator Runbooks

Goal: launch with operational control rather than just code paths.

Owner: ops/docs agent.

Scope:

- `docs/launch`
- gateway readiness routes
- public smoke/runbook scripts

Acceptance:

- Runbook covers deploy, rollback, disabling gateway, disabling paid settlement,
  checking model catalog, checking receipts, and checking provider lane health.
- Live smoke commands avoid printing secrets.
- Incident handling identifies who can turn off spend or payout.

## Parallel Delegation Map

Use separate worktrees from clean `origin/main`. Update the root delegation
audit's active-claims table before editing files.

| Lane | Repo | Files | Avoid |
| --- | --- | --- | --- |
| Gateway readiness smoke | `openagents` | inference gateway routes/docs | web route/view files claimed by website agent |
| Billing/MPP proof | `openagents` | `workers/api/src/inference/mpp`, credit bridge docs | Pylon, Psionic, web scene files |
| Verifier product path | `openagents` | `khala-code-verifier`, `acceptance-runner` | benchmark/Pylon/Verse files unless coordinated |
| Settlement caps | `openagents` | accepted-outcome and verified-work settlement files | Stripe profile changes unless same owner |
| Benchmark real sweep | `openagents` | `workers/api/src/inference/benchmark` | gateway page/view files |
| Pylon real serving | `openagents` | `apps/pylon`, OpenAgents-network adapter/admission | benchmark runner unless coordinated |
| Psionic real M8 | `psionic` | coordinator M8 files/docs | OpenAgents implementation files |
| Verse interaction MVP | `openagents` | `apps/autopilot-desktop`, `three-effect` if needed | web `/landing` and `/khala` route implementation |

## Final Go / No-Go Checklist

Do not call Khala broadly launched until all P0 items are green:

- [ ] Production readiness endpoint reports servable model count > 0.
- [ ] `/v1/models` and `/v1/chat/completions` smoke with a funded account.
- [ ] Dereferenceable receipt proves a metered Khala completion.
- [ ] Credits/money-in path has a receipt-first proof.
- [ ] `khala-code` verifier path cannot produce false `verified:true`.
- [ ] Settlement/payout caps are owner-armed and fail-closed.
- [ ] `/khala` copy and product-promise state match the armed runtime.
- [ ] Disable/rollback runbook exists for gateway, provider lanes, and payout.

When those are true, the launch claim can be narrow and defensible:

> Khala is an OpenAI-compatible OpenAgents inference endpoint with receipt-backed
> routing, metering, and verification evidence. Some advanced serving modes
> remain gated until real compute and owner-armed benchmarks prove them.
