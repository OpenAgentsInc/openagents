# OpenAgents Gym — Spec & Roadmap

> **Status:** Phase 0 backend core, public fixture UI, and illustrative
> report/scene viewer landed and closed (#6163, #6164, #6165, #6166).
> `GymExperiment`,
> `compileGymExperiment`, and
> `runGymFixtureExperiment` live in
> `apps/openagents.com/workers/api/src/inference/gym/` and compile into the
> existing Khala benchmark matrix/runner/report path without real spend. The
> public `/gym` route lives in
> `apps/openagents.com/apps/web/src/page/loggedOut/page/gym.ts` with typed
> fixture knobs, a locked no-spend economics panel, a deterministic Three.js
> fixture scene, and a public-safe report viewer that keeps `decisionGrade:
> false`, drops `not_measured` samples from metric math, and renders null
> cost-per-accepted-outcome as an explicit finding.
> The owner/internal `/gym/oss` route is also landed (#6167) as the GPT-OSS 20B
> latency playground: it is logged-in owner-gated, capped at eight in-flight
> requests, streams against the neutral `openagents/khala-oss-20b` lane, reads
> `openagents.khala.telemetry.v1`, and charts TTFT/TPS/ITL/wall-clock without a
> per-call balance gate because that Hydralisk L4 lane is billed hourly.
> The Gym is the interactive experimentation surface and **eval+reward factory**
> that sits *on top of the
> already-landed Khala benchmark harness*
> (`apps/openagents.com/workers/api/src/inference/benchmark/`, book P1-5 / #6088),
> the coordinator/`ModelRouter` seam (#5482), the provider-adapter registry
> (#5479/#5480/#5481 + Pylon + Tassadar), the verification-class registry, the
> Blueprint/program + plugin layer
> ([`../inference/2026-06-23-khala-blueprint-program-and-plugin-extensibility.md`](../inference/2026-06-23-khala-blueprint-program-and-plugin-extensibility.md)),
> and the merged telemetry schema (`openagents.khala.telemetry.v1`). It specifies
> what the Gym *is*, the `/gym` web surface, its typed config, its economics, and
> a phased roadmap keyed to those existing seams. **It claims nothing is shipped
> beyond what those docs already document.** This is not a product promise, a
> served capability, or public-claim copy; nothing here widens a promise registry
> entry or asserts a public benchmark marketplace.

## 0. In one paragraph

OpenAI shipped **Gym** early: a standard set of *environments* plus a common
interface (`reset`/`step`/observation/action/reward) so anyone could develop and
compare RL policies against the same tasks. **OpenAgents Gym is the analogous
thing for Khala** — a standard set of **environments** (benchmarks/tasks each
paired with a verifier and acceptance contract), a typed interface to run a
**policy** (a Khala configuration: coordinator candidate × provider fan-out × tool
set × plugin/module composition × sampling × quantization/speculation × prompt
layout) against them, and a **reward** that is the *executed verification verdict
+ cost-per-accepted-outcome* — the same signal the learned coordinator trains on.
The `/gym` web route is the human surface: a wall of knobs and dials over that
typed config, rendered in our existing Foldkit + `three-effect` UI, where you can
fan out to multiple inference providers, swap tool sets, compose plugins/modules,
point at Terminal-Bench or any other environment, watch a run light up, and read a
dereferenceable report. Because every run rides the existing metering/settlement
spine, **people can pay to run benchmarks** — submit an environment + policy, get a
priced quote, spend credits (or Bitcoin), and receive a public-safe report
receipt. The Gym is therefore both the *lab where we train Khala* and a
*benchmark-as-a-service* product over the same machinery.

## 1. Where the Gym sits (it is not a new engine)

The Gym is **not** a new inference codebase and **not** a new metric vocabulary.
It is a typed experimentation layer that *compiles down to* surfaces that already
exist:

| Gym concept | Existing seam it compiles to |
|---|---|
| Environment (task set + verifier + acceptance contract) | the benchmark **matrix** workload axis (`benchmark/matrix.ts`) + the verification-class registry (`khala.md` §6) + the executed acceptance contract ([`../inference/2026-06-22-verified-work-must-execute-the-artifact.md`](../inference/2026-06-22-verified-work-must-execute-the-artifact.md)) |
| Policy under test | a Khala configuration: a **coordinator candidate** (`ModelRouter` → TRINITY → Conductor) over a **provider fan-out** (provider-adapter registry) with a **tool set** and **plugin/module composition** (program-signature layer) |
| Run | `expandMatrix` → `runner.ts` over a `BenchmarkLaneSeam` (`lane-seam.ts`) producing `KhalaTelemetryRecord`s |
| Reward / score | `buildBenchmarkReport` (`report.ts`): latency percentiles, **cost-per-accepted-outcome**, verification rate, cache-hit rate |
| Spend / billing | the balance gate (`readAgentBalance`, `402`) + `MeteringHook` (#5477) + the revenue-loop spine (EPIC #5457) |
| Public output | the public-safe report (already enforced by `checkReportPublicSafety`) + an optional leaderboard projection |

So building the Gym is mostly: (a) a **typed `GymExperiment` config** that is a
thin, human-authored superset of the existing `BenchmarkMatrixConfig`; (b) a
**compiler** from that config to matrix cells + a coordinator/policy selection;
(c) the **`/gym` web surface** (knobs/dials + live viz + report viewer); and (d)
the **paid-run path** that arms the real lane seam behind the balance gate and an
owner-gated preflight. The runner, telemetry, report math, public-safety
tripwire, provider adapters, and settlement spine already exist.

## 2. Why "Gym" — the analogy, made precise

| OpenAI Gym | OpenAgents Gym |
|---|---|
| `Environment` with `reset()`/`step(action)` | `GymEnvironment` = task set + verifier + acceptance contract + sequence shapes (ISL/OSL/cacheable-prefix/concurrency) |
| `observation` | the request (prompt-prefix layout, tool schemas, context) |
| `action` | the policy's choice: which workers/providers, which role plan (Thinker/Worker/Verifier), which tools/modules |
| `reward` | the **executed verification verdict** + cost-per-accepted-outcome — never a self-grade, never a benchmark grader the policy can reach (the TMAX reward-hacking lesson, `khala.md` §6) |
| comparing RL algorithms on a fixed env | comparing **coordinator candidates / provider mixes / tool sets / plugin compositions** on a fixed environment, scored on outcome |
| leaderboard | optional public-safe Gym leaderboard projection over decision-grade reports |

The crucial discipline carried from the benchmark harness (book P1-5): **"faster"
is meaningless until you say faster at *what*, on *which lane*, under *which
traffic shape*, judged on *which outcome*.** The Gym makes those four axes the
explicit knobs of an experiment, and refuses to call a result decision-grade until
an owner-armed real seam ran over **realistic** traffic.

## 3. The `/gym` web surface

A new logged-out **explainer + fixture demo** route, registered the same way
`/khala` is (`KhalaRoute()` in `apps/openagents.com/apps/web/src/route.ts`).
Phase 0 adds `GymRoute()` and
`apps/openagents.com/apps/web/src/page/loggedOut/page/gym.ts` for the public
fixture surface. It has no auth requirement, no provider calls, and no spend.
The logged-in paid-run benchmark surface remains future work gated by auth,
balance, and owner approval.

The sibling `/gym/oss` route is a narrower owner/internal surface for the live
GPT-OSS 20B hourly lane. It is not the Phase 0 public fixture demo and it is not
a paid-lane benchmark runner. It exists to run real streaming samples against
`openagents/khala-oss-20b`, reconcile server telemetry with client timing, and
show P50/P90/P99/mean TTFT, perceived TPS, inter-token latency, wall-clock,
completion tokens, aggregate throughput, and the 1->2->4->8 concurrency ramp.
It stays auth/owner-gated and hard-capped at eight in-flight requests so the
hourly lane can be exercised without exposing an unauthenticated load generator.

**UI stack (owner mandate).** Structure in **Foldkit**; visualization in
**`@openagentsinc/three-effect`** first (the same stack `khala-in-the-world.md`
and the `scene/` elements already use — `pylonBezierNetworkElement.ts`,
`tassadarProofReplayElement.ts`, etc.). The knobs/dials are Foldkit structure; the
live run is a `three-effect` scene reusing the Verse visual language — **fan-out
energy arcs from the Gym nexus to each selected provider/worker, verdict beams
back on `test_passed`, a cost meter filling in msat** — so a Gym run *looks like*
a Khala request in the world ([`../inference/khala-in-the-world.md`](../inference/khala-in-the-world.md)).

### The knobs and dials (each maps to one typed config field)

1. **Environment picker** — choose the benchmark/task env: `terminal-bench`,
   `khala-code` (crossy-road rubric / artifact-gen), `long-context-codebase-qa`,
   the **M8 head-to-head** demo ([`../inference/2026-06-23-khala-head-to-head-m8-status.md`](../inference/2026-06-23-khala-head-to-head-m8-status.md)),
   or a custom registered env. Picking an env fixes its verifier + acceptance
   contract — you cannot run an env without its grader.
2. **Provider fan-out** — multi-select the lanes (`vertex-anthropic`,
   `vertex-gemini`, `fireworks`, `partner-passthrough` real today;
   `pylon-whole-small`, `psionic-shard-wan` labeled future), and a **fan-out
   mode**: `single` · `race` (first viable) · `best-of-N` · `verifier-pick`
   (run N, keep the one that verifies). Concurrency is a dial. The single
   `LANE_AVAILABILITY` table stays the source of truth; future lanes are
   selectable axes but never measured (honest skipped run, never a fake zero).
3. **Tool set** — select the tools/tool-schemas exposed to the policy (and MCP
   tool toggles). Tool order/serialization stays canonical so the stable
   prompt-prefix hash holds (book P0-2, `prompt-prefix-cache.ts`).
4. **Plugins / modules (marketplace lane)** — compose admitted capability units
   behind Khala **program signatures** (starter-plugin-catalog deterministic
   units today; FUTURE Tier-E conformance-tested modules behind ABI tokens),
   discovered **semantically** via the signature lookup — never string-matched.
   **Boundary (in force):** this is *not* a public plugin marketplace and not
   arbitrary external admission; the
   [extensibility doc's](../inference/2026-06-23-khala-blueprint-program-and-plugin-extensibility.md)
   no-marketplace boundary holds.
5. **Coordinator / policy** — pick the candidate under test: `heuristic-v0`
   (`ModelRouter`), `trinity-v1` (logit router), `conductor-v2` (NL planner), or
   a named Psionic candidate artifact; choose **shadow** vs (owner-gated) live;
   set the role plan.
6. **Sampling + serving** — temperature / reasoning effort, max tokens, transport
   (streaming vs async batch, book P0-3), and **quantization / speculation**
   selection (precision + engine, disclosed per the P1-7 / P1-8 gates — a
   quantized lane is a different product, `khala.md` §6).
7. **Sequence shapes + samples** — ISL/OSL/cacheable-prefix/concurrency, each
   tagged `realistic` or `synthetic`, and samples-per-cell (≥5 to read
   percentiles, book §4.5.2).
8. **Economics** — spend cap (msat), max billable samples, **fixture vs
   owner-armed real seam**, and the owner approval ref required to arm real spend.

### The output panel

- a live `three-effect` run scene (arcs/verdicts/cost meter);
- the dereferenceable **report**: P50/P90/P99 + mean for TTFT / wall-clock /
  perceived TPS / ITL, **cost-per-accepted-outcome**, verification rate, cache-hit
  rate — over *measured* samples only (`not_measured` dropped, never coerced);
- the honesty header: `decisionGrade` true only for an owner-armed real seam over
  realistic traffic with no synthetic-only group; otherwise the `illustrativeNotice`;
- the **report receipt** (`detailRef`) for a paid run.

## 4. Typed config (the Gym is config, not keywords)

The Gym's experiment is a typed Effect Schema, a human-authored superset of the
existing `BenchmarkMatrixConfig`, that **compiles** to matrix cells + a policy
selection. No ad-hoc string/keyword routing anywhere — env, lane, tool, plugin,
and coordinator selection are all typed enums / semantic selectors (workspace
semantic-routing rule).

```text
GymExperiment {
  environment: GymEnvironmentRef            // terminal-bench | khala-code | long-context | m8 | custom
  policy: {
    coordinator: CoordinatorCandidateRef    // heuristic-v0 | trinity-v1 | conductor-v2 | psionic:<id>
    fanout: { lanes: Lane[]; mode: 'single'|'race'|'best-of-n'|'verifier-pick'; concurrency: int }
    tools: ToolSetRef
    modules: ProgramSignatureComposition     // admitted plugins/modules behind signatures (semantic discovery)
    sampling: { temperature, reasoningEffort, maxTokens, transport }
    serving: { quantization?: QuantSpec; speculation?: SpecSpec }
  }
  shapes: SequenceShape[]                    // each tagged realistic|synthetic
  samplesPerCell: int                        // >= 5
  budget: { spendCapMsat, maxBillableSamples, seam: 'fixture'|'real', ownerApprovalRef? }
}
```

`compileGymExperiment(exp) -> { matrixConfig, policySelection }` reuses
`expandMatrix`; `runner.ts` executes; `buildBenchmarkReport` scores. The reward a
training consumer reads (`scalarReward` + cost-per-accepted-outcome) comes
straight from the canonical telemetry record — **no parallel grader**.

## 5. The Gym trains Khala (the reason it exists)

A DSPy/Blueprint Khala request is a *signature → composed module → executed
reward* program (the extensibility doc). The Gym is the surface that **produces
the eval + reward artifacts** that improve those programs:

```text
Gym experiment  →  runner  →  telemetry records  →  report (reward: verdict + cost/outcome)
       │                                                        │
       │                                                        ▼
       │                                 GEPA candidate feedback (psionic.probe_gepa_candidate_manifest.v1)
       │                                 TRINITY sep-CMA-ES   ·   Conductor GRPO   (trained in Psionic)
       ▼                                                        │
  shadow candidate  ◄──────────── promote on cost-per-accepted-outcome (runtime_promotion, approval-gated)
       │
       └─► back into the Gym for the head-to-head (M8) — the flywheel closes
```

- The reward is the **executed verification verdict** + **cost-per-accepted-
  outcome** — the exact inputs the learned coordinator needs (accepted outcome per
  sat and per second), now produced by a first-class lab instead of ad-hoc scripts.
- Winning candidates ship as **shadow candidates** under Psionic's
  promoted/candidate contract; promoting one to live routing authority is a
  `runtime_promotion` — approval-gated under the Artanis autonomous-loop contract
  (`khala.md` §10). The Gym never silently promotes a policy.
- The boundary with Psionic is unchanged: **Psionic owns training + evidence and
  holds no money; the Gym (product layer) owns the experiment surface, pricing,
  and the report receipt.**

## 6. Economics — pay to run benchmarks

A Gym run is just another metered product on the revenue-loop spine. The
**benchmark-as-a-service** path:

1. **Quote** — `compileGymExperiment` + `LANE_AVAILABILITY` + sample counts price
   the run up front (provider cost basis × policy fan-out × samples), surfaced as
   an msat quote before anything spends.
2. **Gate** — `readAgentBalance` / `402` on insufficient credits; real spend
   requires `seam:'real'` **and** an owner-gated `preflightRealBenchmarkSweep`
   (positive budget cap, max billable cap, realistic-traffic evidence) — the same
   gate the benchmark harness already enforces. A fixture run is always free and
   always `decisionGrade:false`.
3. **Charge (receipt-first)** — `MeteringHook` writes the ledger from real
   provider usage, never an estimate; per-sample idempotency.
4. **Report receipt** — the public-safe report dereferences at a receipt ref; a
   decision-grade report may opt into a public **leaderboard** projection.
5. **Split** — every Gym dollar fans the standard three ways: OpenAgents margin +
   serving node (RL-2, when a Pylon served) + referrer (RL-1, refer-once-earn-
   forever). **FUTURE/gated:** when composed plugin/modules did work, the per-trace
   decomposition splits to the component authors (the extensibility doc's revived
   60/20/20-on-evidence), behind a real settlement loop + owner arming.

Pricing tiers worth modeling (open question): free fixture/illustrative runs;
metered self-serve real runs; and a higher "decision-grade certified" run that
guarantees realistic traffic + owner-armed seam + a citable public report.

## 7. Safety, privacy, invariants

- **No chain-of-thought exposure.** The Gym shows routing class, verification
  class, cost, and receipts — never internal CoT (same as Khala).
- **Public-safe reports only.** Reuse `checkReportPublicSafety`; no prompt,
  completion, account ref, raw cache key, raw price, or margin ever leaves — only
  token counts, durations, neutral classifiers, coarse region/bucket, and a
  one-way affinity **hash** (book P0-2 privacy rule).
- **No fabricated numbers.** `not_measured` ≠ `0`; a not-yet-available lane is an
  honest skipped run; a zero-accepted-outcome group is a `null` cost-per-outcome
  finding, not a fake cheap result.
- **Typed selection, never keyword routing.** Env/lane/tool/plugin/coordinator
  selection are typed enums or semantic signature lookups.
- **Real spend is owner-gated and balance-gated.** The UI must make it impossible
  for an un-armed environment to issue a billable request (the
  `RealLaneNotArmedError` gate is authoritative).
- **Promotion is approval-gated.** A Gym-winning learned coordinator becomes live
  only via a `runtime_promotion` under the autonomous-loop + promoted/candidate
  contract.
- **Schema reuse, never fork.** The Gym reuses `openagents.khala.telemetry.v1`
  and the benchmark report types; it does not invent a parallel metric vocabulary.

## 8. Roadmap (keyed to existing seams)

- **Phase 0 — explainer + fixture demo (no spend).** `GymRoute()` + the public
  `/gym` page (Foldkit + `three-effect`), the typed `GymExperiment` schema, and
  `compileGymExperiment` over the existing fixture lane seam. Output: a live
  fixture run scene + an illustrative report. *Success:* a visitor configures
  knobs, runs the bundled decision suite through the fixture seam, and reads a
  labeled non-decision-grade report — entirely in-CI, no spend. **Landed and
  closed:** #6163, #6164, #6165, #6166.
- **Owner/internal GPT-OSS latency playground.** `/gym/oss` is the focused
  Hydralisk GPT-OSS 20B load/latency surface requested in #6167: prompt presets
  plus custom prompt, sample count, concurrency dial, optional ramp, live
  streaming execution, owner gate, hard in-flight cap, telemetry reconciliation,
  `three-effect` throughput scene, result cards, aggregate table, and ramp chart.
  This is not a public no-spend fixture route; it is an owner-gated operational
  surface for the hourly lane. **Landed and closed:** #6167.
- **Phase 1 — environments + policy matrix.** Register the first environments
  (Terminal-Bench adapter, `khala-code`, long-context QA, M8) with their verifiers
  and acceptance contracts; wire the full policy axis (provider fan-out modes,
  tool sets, coordinator candidate selection). *Success:* a fixture run compares
  two coordinator candidates × two provider mixes on one env, scored on
  cost-per-accepted-outcome.
- **Phase 2 — paid runs (owner-armed real seam).** The quote → balance-gate →
  `preflightRealBenchmarkSweep` → real seam → report-receipt path. *Success:* a
  funded account pays to run a real, billable sweep over realistic traffic and
  gets a `decisionGrade:true` report receipt; revenue splits land on the spine.
- **Phase 3 — Gym → training loop.** Gym reports feed GEPA candidate feedback +
  TRINITY/Conductor training in Psionic; winners return as shadow candidates and
  re-enter the Gym for the head-to-head. *Success:* a coordinator candidate
  trained on Gym-produced reward beats the heuristic in shadow on
  cost-per-accepted-outcome, then is promoted via an approval-gated
  `runtime_promotion`.
- **Phase 4 — plugin/module composition + leaderboard.** Compose admitted
  modules behind program signatures into Gym policies; public-safe leaderboard
  projection over decision-grade reports; (FUTURE/gated) per-trace revenue split
  to component authors. *Success:* a composed-module policy is benchmarked and
  metered, with the author split modeled on evidence — boundary intact (no public
  plugin marketplace).

## 9. Build spec (for a coding agent)

```text
Add OpenAgents Gym: a typed experimentation surface + /gym web route over the
EXISTING Khala benchmark harness. Do NOT build a new inference engine, a new
metric vocabulary, or a new settlement path.

1. Define GymExperiment (Effect Schema) as a human-authored superset of
   BenchmarkMatrixConfig: environment ref, policy (coordinator candidate, provider
   fan-out + mode + concurrency, tool set, module/signature composition, sampling,
   serving quant/spec), sequence shapes (realistic|synthetic), samplesPerCell,
   budget (spend cap, max billable, seam fixture|real, ownerApprovalRef).
2. compileGymExperiment(exp) -> { matrixConfig, policySelection }; reuse
   expandMatrix / runner.ts / buildBenchmarkReport. Reuse the
   openagents.khala.telemetry.v1 schema and checkReportPublicSafety. No forks.
3. Register environments behind a typed GymEnvironment registry (task set +
   verifier + acceptance contract + default shapes): terminal-bench, khala-code,
   long-context-qa, m8. Selection is typed/semantic, never string-matched.
4. Add GymRoute() + the public /gym page in Foldkit; the live-run visualization
   in @openagentsinc/three-effect, reusing the Verse fan-out/verdict/cost visual
   language. Knobs/dials bind to the typed config fields above.
5. Paid runs: quote -> readAgentBalance/402 gate -> preflightRealBenchmarkSweep
   (owner-gated, real seam only) -> MeteringHook receipt-first -> report receipt;
   splits on the revenue-loop spine (RL-1/RL-2, RL-3 author split FUTURE/gated).
6. Keep the fixture seam the default (free, in-CI, decisionGrade:false). Make it
   impossible for an un-armed environment to issue a billable request.
7. Tests: schema round-trip, compiler -> expected matrix cells, fixture run is
   deterministic, report public-safety tripwire, balance gate refuses unfunded,
   real seam refuses unarmed. No live spend in CI.
```

## 10. Open questions

- Environment adapter contract: what is the minimal typed interface a new
  `GymEnvironment` must satisfy (task source, verifier binding, acceptance
  contract, default realistic shapes, public-safety of its task content)?
- Terminal-Bench specifically: which retained fixtures (`apps/pylon/docs/probe-port/`
  Terminal-Bench material) seed the first env, and where does the executor run
  (Pylon/Psionic) so the verifier is on a **distinct** device from the producer?
- Pricing tiers: free fixture vs metered self-serve vs decision-grade certified;
  how the quote is computed before real provider usage is known.
- Public leaderboard: which report fields are safe to rank publicly, and how to
  keep `decisionGrade:false` runs out of any ranked surface.
- Where the Gym's logged-in run surface lives relative to the operator dashboard,
  and how Artanis may *propose* (approval-gated) Gym sweeps as `inference`-class
  work.

---

> OpenAI Gym gave RL a standard set of environments and one interface so policies
> could be compared honestly. OpenAgents Gym does that for Khala: pick an
> environment, configure a policy — providers, tools, plugins, coordinator — run
> it, watch it light up, read a public-safe report, and pay for the real ones.
> The reward is verified work per sat. It is the lab where we train Khala, and a
> benchmark-as-a-service product over the same machinery.
