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
> As of #6244, the same pattern is promoted into a typed
> `throughput-concurrency` Gym environment and a pure Worker-side
> `openagents.gym.throughput_concurrency_report.v1` artifact that reports
> per-lane TTFT/TPS/ITL, aggregate throughput, speculation acceptance, and the
> first concurrency point where quota or latency degrades while preserving
> `not_measured` as distinct from measured `0`.
> The Gym is the interactive experimentation surface and **eval+reward factory**
> that sits *on top of the
> already-landed Khala benchmark harness*
> (`apps/openagents.com/workers/api/src/inference/benchmark/`, book P1-5 / #6088),
> the coordinator/`ModelRouter` seam (#5482), the provider-adapter registry
> (#5479/#5480/#5481 + Pylon + Tassadar), the verification-class registry, the
> Blueprint/program + plugin layer
> ([`../khala/2026-06-23-khala-blueprint-program-and-plugin-extensibility.md`](../khala/2026-06-23-khala-blueprint-program-and-plugin-extensibility.md)),
> and the merged telemetry schema (`openagents.khala.telemetry.v1`). It specifies
> what the Gym *is*, the `/gym` web surface, its typed config, its economics, and
> a phased roadmap keyed to those existing seams. **It claims nothing is shipped
> beyond what those docs already document.** This is not a product promise, a
> served capability, or public-claim copy; nothing here widens a promise registry
> entry or asserts a public benchmark marketplace.
>
> **Episode 243 delta (2026-06-25):** see
> [`2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md`](2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md)
> for the current Phase-1 direction — the **OpenCode coding-agent head-to-head** as
> the first real environment, **BigPickle** (OpenCode's default free model) de-TBD'd
> as ladder rung 1, the expanded backing-lane set, the real per-lane cost basis the
> cost-per-accepted-outcome now consumes, and the **train-and-use-Khala** flywheel
> (the Gym's own runner/eval inference routed through Khala = the next dog-food lane).

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
a Khala request in the world ([`../khala/khala-in-the-world.md`](../khala/khala-in-the-world.md)).

### The knobs and dials (each maps to one typed config field)

1. **Environment picker** — choose the benchmark/task env: `terminal-bench`,
   `khala-code` (crossy-road rubric / artifact-gen), `long-context-codebase-qa`,
   the **M8 head-to-head** demo ([`../khala/2026-06-23-khala-head-to-head-m8-status.md`](../khala/2026-06-23-khala-head-to-head-m8-status.md)),
   the **OpenCode coding-agent head-to-head** (the first *client-surface*
   environment — run the same coding task through OpenCode against each model
   endpoint; FIRST Phase-1 target per Episode 243, see
   [`2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md`](2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md)),
   or a custom registered env. Picking an env fixes its verifier + acceptance
   contract — you cannot run an env without its grader.
2. **Provider fan-out** — multi-select the lanes (`fireworks` —
   DeepSeek V4 Flash, the primary backing lane today — plus `vertex-anthropic`,
   `vertex-gemini`, `partner-passthrough`, the OpenRouter-free lane, and the
   own-infra `gpt-oss-20b` / `gpt-oss-120b` (Hydralisk) and `glm-52` (Z.ai
   GLM 5.2, REAP-pruned) lanes that Episode 243 added to the Khala mix; `pylon-whole-small`,
   `psionic-shard-wan` labeled future), and a **fan-out mode**: `single` · `race`
   (first viable) · `best-of-N` · `verifier-pick` (run N, keep the one that
   verifies). Concurrency is a dial. The single `LANE_AVAILABILITY` table stays
   the source of truth; `available` lanes can enter owner-armed real sweeps,
   `fixture_only` lanes can be exercised by the deterministic no-spend seam, and
   future lanes are selectable axes but never measured (honest skipped run, never
   a fake zero). The Gym is also where a **new or tuned lane is exercised before
   it joins the Khala mix** (e.g. the GLM-REAP MTP2 speculative-decoding win),
   scored on outcome, not just raw tok/s.
   For the head-to-head ladder, competitor model endpoints are typed lanes too:
   `bigpickle` (OpenCode's default free model) → other open/free → paid frontier.
3. **Tool set** — select the tools/tool-schemas exposed to the policy (and MCP
   tool toggles). Tool order/serialization stays canonical so the stable
   prompt-prefix hash holds (book P0-2, `prompt-prefix-cache.ts`).
4. **Plugins / modules (marketplace lane)** — compose admitted capability units
   behind Khala **program signatures** (starter-plugin-catalog deterministic
   units today; FUTURE Tier-E conformance-tested modules behind ABI tokens),
   discovered **semantically** via the signature lookup — never string-matched.
   **Boundary (in force):** this is *not* a public plugin marketplace and not
   arbitrary external admission; the
   [extensibility doc's](../khala/2026-06-23-khala-blueprint-program-and-plugin-extensibility.md)
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
  and client surfaces with their verifiers and acceptance contracts; wire the
  full policy axis (provider fan-out modes, tool sets, coordinator candidate
  selection). **Landed D1:** the OpenCode client-surface fixture registers typed
  endpoint lanes, provisions public-safe `opencode.json`, records provider usage
  without estimation, and produces a `decisionGrade:false` Khala-vs-BigPickle
  report scored on cost-per-accepted-outcome, verified-rate, and tool-call
  success. **Landed D2:** `GYM_ENVIRONMENT_REGISTRY` binds `terminal-bench`,
  `khala-code`, `long-context-codebase-qa`, and `m8-head-to-head` to a task set,
  verifier, acceptance contract, default realistic/public-safe shape, and default
  tool set. `compileGymExperiment` carries that grader binding forward and
  refuses unregistered or graderless environments before any fixture run starts.
- **Phase 2 — paid runs (owner-armed real seam).** The quote → balance-gate →
  `preflightRealBenchmarkSweep` → real seam → report-receipt path. *Success:* a
  funded account pays to run a real, billable sweep over realistic traffic and
  gets a `decisionGrade:true` report receipt; revenue splits land on the spine.
  **Landed D3:** `paid-run.ts` compiles real-seam experiments as pure plans,
  quotes the executable cells, returns a `402` balance gate before spend, requires
  real-sweep preflight evidence and owner approval, arms only explicitly covered
  real lane executors, emits `MeteringHook` contexts, and builds the public-safe
  receipt.
- **Phase 3 — Gym → training loop.** Gym reports feed GEPA candidate feedback +
  TRINITY/Conductor training in Psionic; winners return as shadow candidates and
  re-enter the Gym for the head-to-head. *Success:* a coordinator candidate
  trained on Gym-produced reward beats the heuristic in shadow on
  cost-per-accepted-outcome, then is promoted via an approval-gated
  `runtime_promotion`. **Landed D4:** `flywheel.ts` projects Gym reports into
  GEPA/TRINITY/Conductor reward bundles, emits Khala served-token recorder inputs
  with `openagents-gym` internal attribution, and gates shadow plus
  approval-backed runtime-promotion readiness on decision-grade
  cost-per-accepted-outcome improvement.
- **Phase 4 — plugin/module composition + leaderboard.** Compose admitted
  modules behind program signatures into Gym policies; public-safe leaderboard
  projection over decision-grade reports; (FUTURE/gated) per-trace revenue split
  to component authors. *Success:* a composed-module policy is benchmarked and
  metered, with the author split modeled on evidence — boundary intact (no public
  plugin marketplace). **Landed D5:** `leaderboard.ts` ranks only
  `decisionGrade:true` public-safe reports and models owner-armed module-author
  splits from composition evidence while keeping payout, settlement, listing, and
  public marketplace authority disabled.
- **Harbor backend seam — Terminal-Bench on Hydralisk.** The Worker-side
  dispatch/ingest contract for the first real Terminal-Bench backend is landed:
  `harbor-dispatch.ts` builds a typed
  `openagents.gym.harbor_terminal_bench_job_spec.v1` for Hydralisk to run
  `harbor run -d terminal-bench/terminal-bench-2 --agent terminus-2 --model
  openagents/khala` or a closed GLM-REAP replication profile, then ingests the
  public-safe `hydralisk.evals.terminal_bench.summary.v1` summary plus an ATIF
  trace ref.
  The Worker imports no Harbor runtime code; raw Harbor artifacts and private
  endpoint material stay on Hydralisk. The job spec now carries `profileRef`,
  model/source attribution, topology, context-window, speculation, and sampler
  guardrails so 4xTP, 8xTP, dual-4x, MTP-2, 65K, and 250K GLM-REAP sweeps are
  comparable without widening public claims. The dispatch receipt also carries
  verified distinct-device
  verifier placement evidence: Harbor `environment_mode = separate`, distinct
  agent/verifier host+device refs, `no-network` verifier, explicit artifact
  handoff, and reward read from a verifier artifact. `harbor-reward.ts` maps the
  summary into a Gym reward report with cost-per-accepted-outcome from
  served-token cost basis and emits a training-ready public-safe ATIF trajectory
  bundle when GPU contention is cleared. `terminal-bench-comparison.ts` then
  projects those reward reports plus throughput measurements into
  `openagents.gym.terminal_bench_comparison_report.v1`, comparing profile refs
  against the external 69.1% Terminal-Bench 2.0 target without treating that
  target as an OpenAgents result. Decision-grade replication requires the
  official 89-task denominator, owner approval, public-safe summary,
  distinct-device verifier evidence, served-token cost basis, and cleared
  GPU-contention evidence; attempted-only or pilot denominators stay visible but
  cannot satisfy the claim.

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
   verifier + acceptance contract + default shapes): OpenCode head-to-head,
   terminal-bench, khala-code, long-context-qa, and m8 are landed for the fixture
   seam. Selection is typed/semantic, never string-matched, and a graderless env
   is rejected before it can run.
4. Add GymRoute() + the public /gym page in Foldkit; the live-run visualization
   in @openagentsinc/three-effect, reusing the Verse fan-out/verdict/cost visual
   language. Knobs/dials bind to the typed config fields above.
5. Paid runs: `prepareGymPaidRun` compiles without spending, quotes executable
   cells from `LANE_AVAILABILITY` and samples, returns a readAgentBalance/402
   gate when unfunded, requires `preflightRealBenchmarkSweep` with owner approval
   and realistic traffic evidence, arms a real seam only for startable plans,
   calls `MeteringHook` receipt-first, and returns a public-safe report receipt;
   splits on the revenue-loop spine (RL-1/RL-2, RL-3 author split FUTURE/gated).
6. Keep the fixture seam the default (free, in-CI, decisionGrade:false). Make it
   impossible for an un-armed environment to issue a billable request.
7. Tests: schema round-trip, compiler -> expected matrix cells, fixture run is
   deterministic, report public-safety tripwire, balance gate refuses unfunded,
   real seam refuses unarmed. No live spend in CI.
8. Training flywheel: project decision-grade reports into typed GEPA/TRINITY/
   Conductor reward bundles, produce Khala internal served-token attribution for
   Gym runner/eval calls, return improved candidates to the head-to-head as
   shadow candidates, and require an explicit approval ref before any
   `runtime_promotion` can be marked ready.
9. Leaderboard/splits: project a public-safe leaderboard from decision-grade
   reports only, excluding fixture/synthetic or unsafe reports; model module
   author shares only from owner-armed composition evidence and keep payout,
   settlement, listing, and public marketplace authority disabled.
10. Harbor dispatch seam: for `terminal-bench`, build a typed Worker-side job
    spec for the Hydralisk Harbor harness, scope the first run to
    `openagents/khala`, then compare closed GLM-REAP profile refs for the
    `glm-52` replication lane without leaking private endpoint material. Request
    only public-safe summary + ATIF artifacts, ingest
    `hydralisk.evals.terminal_bench.summary.v1`, and assert no Harbor runtime
    package is imported into the Worker bundle.
11. Harbor verifier placement: require dispatch evidence proving
    `environment_mode = separate`, distinct agent/verifier devices, `no-network`
    verifier execution, explicit artifact handoff, and reward read from the
    verifier artifact before ingest can mark the placement verified.
12. Harbor reward/cost/training projection: map solved Terminal-Bench tasks to
    accepted outcomes, properly attempted tasks to attempted verifications, real
    served-token cost basis to cost-per-accepted-outcome, and public-safe ATIF
    refs into GEPA/TRINITY/Conductor-ready training trajectory bundles. Block
    decision-grade/training readiness when GPU contention is not cleared.
13. Terminal-Bench comparison report: compare closed profile refs against the
    external 69.1% target as a target row, not an OpenAgents result; preserve
    `not_measured` versus measured `0`; require the official 89-task denominator
    before `replicationClaimSatisfied` can be true.
```

## 10. Open questions

- Environment adapter contract beyond the fixture seam: the minimal descriptor is
  landed, but real dispatch adapters still need per-surface artifact ingest.
- Terminal-Bench specifically: the typed env, Worker-side Hydralisk Harbor
  dispatch/summary-ingest seam, and distinct-device verifier placement evidence
  are landed. Reward/cost mapping and training trajectory projection from Harbor
  summary/ATIF artifacts are also landed, along with the GLM-REAP profile
  comparison report. Remaining work is wiring the live owner-armed executor path
  and public/logged-in surfaces around the projections.
- Pricing tiers: free fixture vs metered self-serve vs decision-grade certified;
  the first quote path is landed from matrix shape and configured lane prices,
  while tier packaging and certified-report pricing remain open.
- Public leaderboard: the first projection is landed for public-safe,
  `decisionGrade:true` reports only; open work is the logged-in/public surface
  that dereferences those rows and its refresh cadence.
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
