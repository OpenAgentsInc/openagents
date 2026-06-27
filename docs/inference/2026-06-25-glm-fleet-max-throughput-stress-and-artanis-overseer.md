# GLM-5.2-REAP fleet: max-throughput continuous stress, reliability hardening, and the Artanis fleet overseer

**Date:** 2026-06-25
**Author:** automation (owner-directed)
**Status:** DIRECTION / PLAN. Not a claim of live capability.

> **Honest-scope header (direction vs live).** Everything below the "Current live
> state" section is **proposed design**, not shipped behavior, unless a sentence
> explicitly says "live" / "today" / "DONE". The GLM fleet, the Khala gateway,
> the GLM pool heartbeat, and Artanis all exist and run; the **continuous stress
> harness, the external-wins admission scheduler, the throughput engine-flag
> changes, and the Artanis fleet overseer described here do not yet exist** and
> are filed as new work. No public-claim copy here: this is an operator/eng plan.
> Throughput numbers cited are micro-benchmark snapshots or book figures, not
> decision-grade fleet measurements. The GLM-5.2 model is attributed to **Z.ai
> (Cerebras REAP method, REAP-pruned by 0xSero)**, served on OpenAgents' own
> Hydralisk GCP G4 infra — not a serving vendor's model.

---

## 0. Current live state (updated 2026-06-27)

- **GLM-5.2-REAP-504B is the live PRIMARY** backing model for the single public
  model id `openagents/khala` (`KHALA_BACKING_MODEL=hydralisk-glm-5.2-reap-504b`,
  GLM ordered first).
- The configured GLM pool has **10 total G4 replicas**, but the current public
  readiness state is degraded: `2` ready replicas, `8` reclaimed replicas,
  `warmOrReadyMaxInflight:2`, and durable acceptance `blocked`. The ten-replica
  throughput target in this plan is a recovery/optimization goal, not the
  current serving ceiling.
- Each 4× replica is intentionally **single-flight** (`--max-num-seqs 2`,
  singleflight 429 gate in the pool adapter). So **aggregate available GLM
  capacity today is bounded by the ready replica count** until reclaimed hosts
  recover; per-replica batching is still OFF.
- Fallback chain (proven live, HTTP 200 not 5xx with GLM dead):
  **GLM → GPT-OSS-120B → GPT-OSS-20B → Vertex-Gemini.** No OpenRouter lane
  exists in the repo (#6313).
- The pool already has live per-replica health + routing: `glm-pool-heartbeat.ts`
  produces `glmPoolHeartbeatRoutingStateOracle(replicaId)`; the pool adapter
  (`hydralisk-adapter.ts`) keeps an in-memory `inflight` map, ranks eligible
  replicas (warm > queueDepth > inflight > ttft > tps), and overflows on
  saturation.
- **Real external users are arriving** (OpenCode → Khala) and **must succeed**.
  The first external user already hit broken tool-calling (#6310).
- **Artanis** already runs autonomous loops on the Worker `scheduled` handler:
  the live side-effecting `runArtanisAdminTick` (no-spend executor dispatch,
  bounded per day) and the evidence-only `runArtanisScheduledTick`, both fenced
  by `artanis-approval-gates.ts` (money/destructive is gated by construction).
  Artanis does **not** today read GPU/replica/throughput/demand — its sensors
  are product/labor/forum/Pylon-registration state plus a gateway
  `khala_readiness` signal. The GPU fleet sensor (`glm-pool-heartbeat.ts`) runs
  as its own separate scheduled tick.

---

## 1. Goal + the external-wins invariant

### Goal

Drive the GLM fleet to **maximum sustained aggregate throughput (tok/s)** and
**harden reliability** (no empty fallbacks, no silent provider_errors, fast
recovery from Spot reclaim), and make all of it **autonomously operated by
Artanis** — continuously, not as one-off runs.

### The hard invariant: real external requests always win

> **EXTERNAL-WINS.** A real inbound external request is *guaranteed-served*
> capacity. Internal stress/saturation load is *best-effort and instantly
> yielding*: it never displaces, delays, or degrades a real external request,
> and it is cancellable mid-flight the instant external demand needs the slot.

Concretely the invariant has three enforceable clauses:

1. **Admission priority.** When the fleet has headroom, internal stress is
   admitted. When external demand rises toward saturation, internal-stress
   admission is refused *before* any external request is refused. External
   requests are admitted as long as there is any servable lane.
2. **Preemptibility.** An in-flight internal-stress request is cancellable. When
   external pressure arrives and no replica slot is free, the scheduler aborts
   internal-stress in-flight work (returning its slot) rather than queueing or
   overflowing the external request to a weaker lane.
3. **Accounting honesty.** Internal stress carries a distinct, typed demand tag
   so it is never confused with external market demand in `token_usage_events`,
   goodput metrics, or GTM claims. Served internal-stress tokens still count in
   the public all-demand tokens-served counter. Preempted/cancelled stress
   requests are recorded as such, not as failures of the external SLO.

The live proof has an additional fail-closed acceptance rule: scheduler
preemption is not enough by itself. The external response must remain on the GLM
primary lane with `fallbackReason:null`. A response that carries
`scheduler_preemption` but then serves through Fireworks/OpenRouter/Gemini after
`fallback_reason: empty_assistant_content` is a useful diagnostic, not an
external-wins pass.

### Priority / admission model

Every request carries a typed **demand class** with a **priority** and a
**preemptible** flag:

| Demand class | priority | preemptible | served-counter | source of truth |
|---|---|---|---|---|
| external (`external`, `unlabeled`) | guaranteed | no | yes (real) | end-user / OpenCode |
| own-capacity coding (`own_capacity`) | high | no | yes (real) | Khala→Pylon→Codex |
| internal dogfood (`internal`) | normal | no | yes (real, internal) | internal accounts |
| **internal stress (NEW: `internal_stress`)** | **best-effort** | **YES** | **yes (real, internal stress)** | stress harness |
| keep-warm heartbeat (`own_capacity` / `glm-pool-heartbeat`) | minimal | yes | yes when tokens are served | cron |

Today the demand-kind enum is `external | internal | own_capacity | unlabeled`
(`sync-schema/src/token-usage-ledger.ts`). The internal-account allowlist forces
listed accounts to `internal`, which would flatten stress into ordinary internal
dogfood. **The plan adds a dedicated `internal_stress` demand kind** (or an
explicit `preemptible: boolean` attribution field) so the admission check
branches on a typed value, not a free-form `demand_source` string.

### How the gateway/scheduler enforces it

The enforcement point is **`chat-completions-routes.ts`, immediately after
request attribution is resolved** (the one place that has the authenticated
account, the final demand kind, and has not yet touched a provider) — co-located
with the existing fair-share gate, *before* the `dispatchWithOverflow` calls:

- **External-pressure read.** Expose live fleet headroom — `sum(maxInflight −
  inflightCount)` across non-reserved, healthy replicas — from the pool adapter
  (extend `GlmReplicaRoutingStateOracle` / the in-memory `inflight` map into a
  read surface). Today selection metadata already emits per-request
  `replicaInflightCount` / `replicaMaxInflight` / `replicaQueueDepth`; the
  scheduler needs the *aggregate live* view.
- **Admission decision.** If `demandKind === 'internal_stress'` and headroom is
  below a reserved external buffer (e.g. keep ≥ K slots free for external), the
  stress request is **deferred/refused (429 with a back-off hint)** instead of
  admitted. External requests are never refused on this path.
- **Mid-flight preemption (net-new plumbing).** Thread an `AbortSignal`/priority
  field through `DispatchDeps → AdapterOperation → hydralisk-adapter` `fetch`.
  When an external request finds no free slot, the scheduler aborts the
  lowest-priority in-flight `internal_stress` request on a healthy replica,
  returning the slot to the external request rather than overflowing it to
  GPT-OSS. The stress request is re-queued by the harness, not failed.

This makes the invariant **structural**: stress traffic can only ever consume
*slack*, and it yields that slack the instant external demand needs it.

**Implementation note (2026-06-27).** The production route now wires the
`routeAdmission` snapshot from the same Hydralisk pool runtime that owns the GLM
adapter's in-memory `inflight` map. Earlier slices had the route tests,
preemption registry, and abort signal plumbing, but a registry without a live
admission snapshot cannot trigger the scheduler's external-demand preemption
branch. This is still not live saturation evidence; it is the wiring prerequisite
that makes the later #6317 stress proof meaningful.

**Acceptance note (2026-06-27).** The bounded proof evaluator is
`openagents.khala.glm_external_wins_proof.v0_1`. It accepts only when the probe
has scheduler-preemption evidence, external HTTP success, `servedLane:
"glm_primary"`, and `fallbackReason:null`. The observed live shape where GLM
returned empty assistant content and the external response fell through to a
weaker fallback is blocked as `fallback_after_preemption`,
`served_lane_not_glm_primary`, and `empty_glm_content_after_preemption`.

---

## 2. Continuous stress-test system (the saturation harness)

> Distinct from the one-off #6312 benchmark. **#6312 produces a published "max
> GLM tok/s" number on demand. This harness is the continuous saturation engine
> that keeps the fleet at its ceiling and is the instrument #6312's number is
> read from.**

### Shape

A **continuous load generator** that:

- **Saturates all 10 replicas** with a mix of realistic + synthetic workloads:
  realistic OpenCode-class sessions (bursty, multi-turn, tool-calling,
  long-context codebase prefixes) plus synthetic fixed-shape prompts for clean
  tok/s measurement.
- **Ramps concurrency** (1 → N per replica and 1 → M whole-pool) to find the
  ceiling, then **holds at the saturation knee** to keep replicas warm and
  exercised, not just probed once.
- **Measures continuously:** aggregate + per-replica completion tok/s, TTFT,
  inter-token latency at P50/P90/P99, **goodput** (useful tokens delivered
  within the interactive ITL SLA, not just raw tokens), error rate, singleflight-
  429/overflow rate, and in-cloud vs WAN deltas.
- **Tags every request `internal_stress`** so it is included in the public
  all-demand counter while remaining distinguishable from external-market
  demand metrics, and is preemptible per §1.
- **Auto-backs-off the instant external demand rises** — the harness reads the
  same live-headroom signal the admission check uses; when external pressure
  climbs, it lowers its target concurrency (and its in-flight requests are
  abortable by the scheduler regardless). It ramps back up when external demand
  falls. This is the closed loop that makes "always saturated, never in the way."

**Implementation note (2026-06-27).** The fail-closed harness prep now emits
canonical `x-openagents-client` attribution for stress/real-sweep dispatches and
the report schema carries public-safe overall plus per-replica TTFT/ITL
P50/P90/P99/mean/sample-count rollups, ok/deferred/preempted/failed counts, and
goodput/TPS. This is measurement plumbing only; the live continuous stress run
still waits on #6318 external-wins proof and #6320 throughput-rollout proof.

### Why continuous (not one-off)

- **Keep-warm by traffic.** Spot replicas that go idle cool down; continuous
  stress keeps decode paths and prefix caches warm so a real external request
  hits a hot replica, not a cold start.
- **Standing ceiling.** The fleet changes constantly (Spot reclaims, region
  swaps, flag changes). A continuous harness re-measures the ceiling as the
  fleet mutates, so #6312's published number stays current instead of stale.
- **Early-warning load.** Sustained synthetic load surfaces saturation/overflow
  regressions and reliability faults (empty fallbacks, provider_errors) *before*
  a real user does — this is exactly the gap #6310 exposed (failures left no
  trace because only successes were recorded).

### Reuse, not a throwaway script

Build on the existing dispatch/profile seam (`harbor-dispatch.ts` profiles,
the benchmark-matrix work P1-5 / #6088) and the live routing-state oracle so the
harness shares the gateway's view of the pool. Publish the aggregate ceiling
through the same telemetry schema (`openagents.khala.telemetry.v1`) the
scorecard already uses.

---

## 3. Reliability hardening

### Failure modes already seen (and what hardens each)

| Failure mode (observed) | Hardening |
|---|---|
| **provider_error on tool calls** (#6310): GLM tool-call path errors outright on some tool requests | Verify GLM-5.2-REAP vLLM `--tool-call-parser` × `--reasoning-parser` × chat-template interaction; prove a tool call round-trips a well-formed `tool_calls` response N× consecutively; circuit-break the GLM lane on a tool-parse-error spike rather than returning provider_error to the user |
| **Empty GPT-OSS-20B fallback** (#6310): GLM unavailable → GPT-OSS-20B returns `content:""`, `tool_calls:null` | A fallback lane that returns empty content must be treated as a **failed** lane (retryable) so dispatch overflows past it, never a silent black hole; fix or drop the GPT-OSS-20B tier; OpenRouter GLM-class tier (#6313) gives a healthier fallback |
| **Hallucinated tool names** (#6310): GLM emits `search` (not in schema) | Model-fit decision: keep GLM primary for chat but route tool-bearing/coding turns to a stronger tool-caller if GLM can't adhere even when healthy (#6310 option B); measured by a tool-adherence benchmark across lanes |
| **Spot reclaim** (#6311): 10/10 Spot; 8 hosts lack the STOP-watchdog → cold-and-stuck | Keep-warm + STOP-watchdog on **every** replica; multi-region auto-replace from a prebaked image (hydralisk #99); a documented non-Spot floor *or* an explicit owner-confirmed all-Spot decision |
| **Replica saturation → fallback** (#6310): GLM cap → singleflight 429 → weaker lane | Raise per-replica concurrency (§4 #1) so the pool absorbs more before overflowing; concurrency-aware routing + a real queue with honest status instead of overflowing to a weaker lane prematurely |

### The hardening program

1. **Per-replica health-checking + circuit-breaking.** The heartbeat already
   probes `/health` + `/v1/models`. Add a **circuit breaker**: a replica that
   spikes provider_errors / empty completions / tool-parse failures is
   *quarantined* (removed from eligibility, like the existing `draining` flag)
   and auto-re-admitted on health recovery. A lane-wide breaker overflows the
   whole GLM lane only when a quorum of replicas is unhealthy.
2. **Keep-warm on all replicas.** §2's continuous stress keeps replicas warm by
   traffic; the STOP-watchdog/keep-warm control plane (#6311) keeps them *alive*
   after Spot STOP. Both are needed: warmth (no cold prefill) and liveness (no
   cold-and-stuck host).
3. **Graceful degradation / SLO-based shedding.** Define SLOs (e.g. external
   TTFT P90, ITL P90, error rate). On SLO breach, shed in priority order:
   internal_stress first (already preemptible), then keep-warm, then reduce
   batch ambition — never shed external. This is goodput-preservation under
   saturation, straight from the book (admission + queue + degradation).
4. **Retry / hedging policy.** For external requests only, allow a bounded
   **hedge** (a second attempt to a different warm replica) when the first
   replica's TTFT exceeds a P99 threshold — bounded so it doesn't amplify load.
   Internal_stress never hedges.
5. **Fallback-chain correctness.** Empty content = lane failure (overflow past
   it). Wire the OpenRouter GLM-class tier (#6313) as a real fallback so the
   degraded path is still GLM-family, not a weaker model. Disclose the served
   lane + `fallback_reason` in the receipt.
6. **Failure telemetry (the #6310 gap).** Today only successful completions are
   traced. Add gateway **failure telemetry**: counts + redacted shapes of
   provider_error, empty-content, fallback, invalid-tool, and singleflight-429
   rates, so the next external breakage is visible without a user screenshot.

---

## 4. Max tok/s via the inference-engineering book

The book's central finding for our stack: our replicas are single-flight
(`--max-num-seqs 2`), so **continuous batching — the dominant aggregate-throughput
lever — is currently switched OFF by configuration**. The biggest untapped wins
are at the **vLLM-on-Hydralisk flag layer**; Khala's gateway/observability/routing
scaffolding is comparatively mature (the implementation log shows it *instrumented
and routed around* these levers but has **not turned them on at the engine**).

### Top 5 highest-leverage techniques (ranked, each with expected gain + how to measure)

1. **Raise per-replica concurrency — `--max-num-seqs` > 2 (unlock continuous
   batching).** *(vLLM flag, Hydralisk)* Aggregate today ≈ replica_count ×
   per-replica decode with per-replica concurrency pinned at ~1. Continuous
   batching amortizes the memory-bandwidth-bound decode weight-read across the
   batch, so single-flight → batched can **multiply per-replica throughput
   without adding GPUs** (book: throughput rises with batch size, lines
   6617–6619). **Tradeoff (explicit):** per-user interactive decode TPS/ITL
   degrades as the batch fills. Mitigate: tune `--max-num-seqs` to the largest
   value still meeting the interactive ITL SLO; split latency-critical vs bulk
   lanes with different concurrency targets. **Measure:** §2 harness — aggregate
   tok/s and ITL P90 at `--max-num-seqs` ∈ {2, 4, 8, 16}, pick the knee where
   ITL P90 still meets SLO. **Expected:** the single biggest aggregate gain
   available; targets a multiple, not a percent.
2. **Engine-side prefix caching + chunked prefill** (`--enable-prefix-caching`,
   `--enable-chunked-prefill`). *(vLLM flags, Hydralisk)* Coding/agent traffic
   repeats long system prompts, tool schemas, and codebase context — the book's
   strongest prefix-cache case ("skip prefill on thousands of tokens", lines
   4790). The gateway side (stable layout, cache-affinity routing) is already
   DONE (P0-2 / #6084); the engine side on our self-hosted replicas is the open
   complement. Chunked prefill is the necessary partner once #1 raises
   concurrency so a long-context prefill doesn't stall in-flight decode (lines
   4972–4976). **Tradeoff:** minimal (prefix caching is lossless). **Measure:**
   TTFT P50/P90 on repeated-prefix sessions vs cold; cache-hit rate.
3. **Speculative decoding for the low-batch interactive coding lane**
   (`--speculative-config`, n-gram/EAGLE/MTP). *(vLLM flag, Hydralisk)* Uniquely
   valuable *because* we are single-flight: low batch = maximum spare compute,
   exactly where speculation pays (lines 4558–4561), and code is the book's
   strong-fit workload. 2–3× per-user TPS when acceptance is high. **Tradeoff
   (explicit):** directly competes with #1 — raising batch size consumes the
   spare compute speculation needs, and at high batch speculation becomes a loss
   and must be dynamically disabled. **Decision is per-lane:** speculate on
   low-batch interactive; continuous-batch the bulk/stress lanes. Policy
   scaffolding is in progress (P1-8 / #6091). **Measure:** per-user TPS + draft
   acceptance rate at low batch; auto-disable threshold vs batch size.
4. **Quantization to free KV memory → larger feasible batch** (FP8 / KV-cache
   quant), eval-gated. *(vLLM flag, Hydralisk; gated by the Khala eval P1-7 /
   #6090)* A throughput *enabler*: KV-cache quant opens headroom so
   `--max-num-seqs` (#1) goes higher before OOM (lines 4396–4401). **Tradeoff:**
   quality risk — must pass the cost-per-accepted-outcome gate; do weights/FP8
   before aggressive KV/attention quant. **Measure:** max feasible
   `--max-num-seqs` before OOM at each quant level × Terminal-Bench solve-rate
   delta (ties to #6253).
5. **Concurrency-aware routing + admission control + a real queue at the Khala
   gateway across the 10-replica pool.** *(gateway + pool change)* Once replicas
   batch (#1), the gateway must route by sequence length and KV-cache affinity
   (lines 6748–6760) and queue overflow with honest status instead of blocking
   the Cloudflare edge (the 524 failure mode) or overflowing to a weaker lane
   too early. This converts raw per-replica throughput into **goodput** (useful
   tokens within SLO) and is also where the §1 external-wins admission lives.
   First slices exist (cache-affinity routing P0-2; provider-health overflow
   P2-10). **Measure:** goodput (in-SLO tokens) vs raw tok/s at saturation;
   singleflight-429 / premature-overflow rate.

### Honest tension

#1 (batch up) and #3 (speculate) pull in opposite directions on the same
spare-compute resource. #2 and #4 are enablers that make #1 safe and larger. #5
turns engine wins into SLO-respecting goodput and hosts the external-wins gate.
The plan sequences them: instrument (§2 harness) → #2 (lossless) + #4 (headroom,
gated) → sweep #1 to the ITL-SLO knee → enable #3 only on the low-batch
interactive lane with auto-disable → #5 to convert it all to goodput.

### Not-yet-warranted (study only)

**Disaggregated prefill/decode + NVIDIA Dynamo.** The book gates this to ≥100M–1B
tokens/day, model ≥100B params, prefill-heavy long-input traffic; below that,
"better off scaling replicas horizontally" (lines 5263–5272). Single-flight
RTX PRO 6000 replicas are below the threshold (matches our P2-9 conclusion).
Trigger to revisit: post-prefix-cache receipts show high-volume long-context
coding where prefill dominates.

---

## 5. Artanis as the autonomous fleet overseer

> Design the overseer as an **extension of the existing Artanis administrator
> tick + approval gates**, not a new system. Artanis already runs a bounded,
> live, side-effecting loop fenced by typed schemas and approval gates; the
> fleet overseer is one more responsibility on that loop, reading the GLM pool
> heartbeat as its sensor.

### The control loop

A new `artanis-fleet-overseer-tick.ts` exporting
`runArtanisFleetOverseerTick(db, deps)` + a `…Scheduled` Effect wrapper, mirroring
`runArtanisAdminTick` / `runArtanisAdminTickScheduled` exactly (env-gated by a new
`ARTANIS_FLEET_OVERSEER_ENABLED`, self-bounded cadence, every outcome a D1 row in
a new `artanis_fleet_overseer_decisions` table, blocked-on-schema-invalid). It is
registered as one new `observedEffect('ArtanisFleet.tick', …)` in the Worker
`scheduled` `Promise.all` beside `ArtanisAdmin.tick`.

Each tick:

1. **Watch (assembleContext).** Read the **existing** GPU sensor —
   `glmPoolHeartbeatRoutingStateOracle` (per-replica health/warm/draining, and
   the new live inflight/headroom read surface from §1) — plus aggregate
   throughput/goodput from telemetry and **external demand** from the token-usage
   ledger (recent `external` request rate). Artanis does not build a new fleet
   collector; the GLM heartbeat stays the sensor, Artanis becomes the orchestrator.
2. **Decide (bounded mind action).** Reuse `artanisMindComplete` with a bounded
   `S.Union` action vocabulary, validated by a typed schema (schema-invalid →
   `blocked` row carrying the raw proposal — same safety as the admin tick).
3. **Act (priority-fenced).** Emit autonomous actions or pending approval gates
   per the authority split below.
4. **Report.** Persist the decision + a public-safe health/throughput signal,
   fed into the existing `artanis-health.ts` snapshot (a new fleet
   `ArtanisHealthSignalKind`), so stale/blocked fleet health structurally blocks
   overclaiming, consistent with the existing health invariants.

### What Artanis may do autonomously vs owner-gated

Mapped onto `artanis-approval-gates.ts` (`ArtanisRiskyActionKind`). The line is
the existing one: **money or destructive ⇒ gated**; **internal no-spend exercise
of owned capacity ⇒ autonomous**.

**Autonomous (no spend, non-destructive — NOT in the risky enum):**

- **Start / scale / back-off the internal stress load** (§2). Pure internal
  no-spend exercise of owned capacity, directly analogous to the admin tick's
  `unpaid_smoke` `dispatch_executor_trace`. This is the core automation: Artanis
  keeps the fleet saturated and instantly throttles the harness when external
  demand rises (the same external-wins signal the admission check uses).
- **Re-admit a recovered replica / warm an already-owned idle replica** —
  reversible, free.
- **Emit health/throughput/goodput reports and forum/operator-console updates**
  (public-safe, via the existing report surfaces).

**Owner-gated (money or destructive — pending `ArtanisApprovalGateRecord`,
effective only with operator approval + receipt):**

- **Request paid scale-out** (provision new paid capacity) → reuse
  `kind: 'provider_call'` or `'deployment'` (both rollback-required). Artanis
  emits a *pending* gate and stops; execution waits for
  `artanisApprovalGateEffective`.
- **Quarantine a replica** (destructive to availability) → add a new
  `'fleet_mutation'` risky kind (rollback-required) to `ArtanisRiskyActionKind`,
  `ARTANIS_RISKY_ACTION_KINDS`, and `rollbackRequiredKinds`, with the matching
  test and an `INVARIANTS.md` update in the same change (adding a risky kind is a
  policy change). A *reversible, auto-recovering* circuit-breaker quarantine
  (drain + re-add on health recovery, with a rollback receipt) can later move to
  autonomous under a standing-cap-style bounded grant (mirror `artanis-spend.ts`).
- **Any wallet spend / settlement / payout** — already gated; the overseer never
  widens this.

This keeps the invariant that **intelligence never upgrades authority**: the
overseer's mind proposes, typed schemas validate, and approval gates hold for
anything that costs money or removes capacity.

### Why this is the right foundation

- The **GPU fleet sensor already exists** (`glm-pool-heartbeat.ts`) — the
  overseer reads it rather than re-collecting.
- The **bounded autonomous loop already exists** (`runArtanisAdminTick`) — the
  overseer copies its safety shape (per-day bound, every-outcome-a-row,
  schema-invalid → blocked).
- The **gate model already enforces money/destructive** — the overseer adds at
  most one new risky kind (`fleet_mutation`) and otherwise reuses existing kinds.
- The **health snapshot already gates overclaiming** — fleet health folds in.

---

## 6. Mapping onto existing issues — covered vs NEW

### Already covered (reference, do NOT duplicate)

- **#6316** — MASTER: maximize GLM-5.2-REAP usage in Khala. The new issues are
  filed under this master (or a fleet sub-master).
- **#6312** — one-off max-tok/s benchmark (the published number). The NEW
  continuous harness *feeds* this; it does not replace it.
- **#6311** — durable non-Spot fleet + keep-warm + STOP-watchdog on all
  replicas. The NEW reliability program *cross-refs* this for liveness; the
  program adds circuit-breaking, hedging, SLO-shedding, empty-fallback fix.
- **#6310** — OpenCode tool-calling broken (provider_error, empty fallback,
  hallucinated tools, no failure telemetry). The reliability program and the
  failure-telemetry work *cross-ref* this; it is the canonical bug record.
- **#6313** — real OpenRouter fallback lane. Referenced by the reliability
  program (healthier fallback tier); not duplicated.
- **#6314** — GLM-vs-DeepSeek precedence documentation. Unrelated to throughput;
  referenced only.
- **#6315 / #6259** — Khala→GLM smoke + receipt-ref fix. Referenced for
  verification continuity.
- **#6253** — Terminal-Bench solve-rate + inference-method comparison (quality,
  shared harness). The throughput work shares the benchmark seam; #6253 is
  solve-rate, this is tok/s — complementary.

### NEW work (genuinely uncovered)

1. **Continuous max-capacity stress/saturation harness** (§2) — distinct from
   the #6312 one-off; it is the continuous instrument #6312 reads from.
2. **External-wins admission/priority scheduler** (§1) — internal stress is
   preemptible and yields to external demand; new `internal_stress` demand tag.
3. **Reliability hardening program** (§3) — per-replica circuit-breaker, empty-
   fallback-as-failure, SLO-shedding, bounded hedging, failure telemetry.
4. **Inference-engineering throughput optimizations** (§4) — the engine-flag
   levers (continuous batching / chunked prefill / prefix caching / speculative
   / quant), each with expected gain + how to measure.
5. **Artanis fleet-overseer automation** (§5) — the autonomous control loop on
   the `artanis-administrator-tick` foundation, approval-gated.

---

## 7. Sequencing

1. **Instrument** — continuous stress harness (§2) + live-headroom read surface.
   You cannot tune what you cannot measure continuously.
2. **Protect** — external-wins admission + preemptibility (§1) BEFORE turning up
   load, so internal saturation can never hurt external users.
3. **Harden** — empty-fallback-as-failure, circuit-breaker, failure telemetry
   (§3) so reliability faults surface under synthetic load, not from users.
4. **Optimize** — engine flags (§4): #2 (lossless) + #4 (gated) → sweep #1 to
   the ITL-SLO knee → #3 on the interactive lane with auto-disable → #5 goodput.
5. **Automate** — Artanis fleet overseer (§5) ties it together: keep saturated,
   yield to external, heal/quarantine (gated), report.

---

## Refs

- `docs/inference/2026-06-25-glm-5.2-reap-504b-serving-audit.md`
- `docs/inference/inference-engineering-book/` (book reading notes, implementation log)
- `~/work/inference-engineering-fulltext.txt`
- `apps/openagents.com/workers/api/src/inference/model-router.ts`,
  `model-serving-policy.ts`, `chat-completions-routes.ts`,
  `glm-pool-heartbeat.ts`, `hydralisk-adapter.ts`
- `apps/openagents.com/packages/sync-schema/src/token-usage-ledger.ts`
  (`TokenUsageDemandKind`)
- `apps/openagents.com/workers/api/src/artanis-administrator-tick.ts`,
  `artanis-approval-gates.ts`, `artanis-scheduled-runner.ts`, `artanis-health.ts`,
  `artanis-spend.ts`, `artanis-mind.ts`
- Issues: #6316, #6312, #6311, #6310, #6313, #6314, #6315, #6259, #6253
