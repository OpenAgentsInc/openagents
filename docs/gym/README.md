# OpenAgents Gym — docs

The **Gym** is the interactive experimentation surface and eval+reward factory
that trains [Khala](../khala/khala.md). Like OpenAI's original Gym (standard
environments + one interface so policies can be compared), it lets you configure a
Khala **policy** — coordinator candidate × provider fan-out × tool set ×
plugin/module composition × sampling × quantization/speculation — run it against a
registered **environment** (Terminal-Bench, `khala-code`, long-context QA, the M8
head-to-head, the **OpenCode coding-agent head-to-head**, throughput/concurrency,
...), and score it on the
**executed verification verdict + cost-per-accepted-outcome**. The public `/gym`
web route is the Phase 0
fixture-only knobs-and-dials surface in Foldkit + `@openagentsinc/three-effect`;
the owner-gated `/gym/oss` surface is the GPT-OSS 20B latency playground for
hammering the hourly Hydralisk L4 lane without exposing a public load generator.
Paid-run planning now rides the existing metering/settlement spine so funded,
owner-armed accounts can pay to run decision-grade benchmarks once the real lane
executor is supplied.

It is **not** a new inference engine or metric vocabulary — it compiles down to
the already-landed Khala benchmark harness, coordinator/`ModelRouter`,
provider-adapter registry, verification-class registry, and the
`openagents.khala.telemetry.v1` schema.

## Status

- Phase 0 public fixture Gym is landed and intentionally spend-free:
  `#6164`, `#6165`, `#6166`, and the closeout epic `#6163`.
- Public `/gym` now also carries the Terminal-Bench 2.0 GLM-REAP comparison
  visualizer from `#6257`: a Foldkit page with a
  `@openagentsinc/three-effect` run field, profile lanes, verifier placement,
  accepted/failing/not-started state, cost/latency/throughput mirror data, and
  explicit caveats. It is fixture-only, `decisionGrade:false`, and defers full
  Autopilot Verse/world integration.
- GPT-OSS owner/internal latency playground is landed at `/gym/oss`:
  `#6167`. It is auth/owner-gated, capped at eight in-flight requests, streams
  against `openagents/khala-oss-20b`, and keeps `not_measured` distinct from
  fabricated zeroes.
- **Epic F3 landed:** `throughput-concurrency` is registered as a typed Gym
  environment, and `throughput.ts` builds repeatable per-lane TTFT/TPS/ITL,
  aggregate-throughput, speculation-acceptance, and degradation-point reports
  from `/gym/oss`-style reconciled samples without coercing `not_measured` to 0.
- **Phase 1 D1 landed:** the **OpenCode coding-agent head-to-head** now has typed
  benchmark lanes, a fixture-only BigPickle rung, an OpenCode config/usage runner,
  and a deterministic `decisionGrade:false` Khala-vs-BigPickle fixture report.
- **Phase 1 D2 landed:** `GYM_ENVIRONMENT_REGISTRY` now registers
  `terminal-bench`, `khala-code`, `long-context-codebase-qa`, and
  `m8-head-to-head` with task-set, verifier, acceptance-contract, default-shape,
  and default-tool bindings. The fixture seam runs all four with their grader
  bound; graderless environments are rejected before execution.
- **Phase 2 D3 landed:** `paid-run.ts` prepares owner-armed real Gym sweeps with
  quote, 402 balance gate, real-sweep preflight, narrowly declared real executors
  for otherwise fixture-only competitor lanes, `MeteringHook` contexts, and a
  public-safe report receipt.
- **Phase 3 D4 landed:** `flywheel.ts` converts Gym reports into
  GEPA/TRINITY/Conductor reward bundles, emits internal `openagents-gym` Khala
  token attribution for the served-tokens counter, and gates shadow candidates
  plus approval-backed `runtime_promotion` readiness on decision-grade
  cost-per-accepted-outcome improvement.
- **Phase 4 D5 landed:** `leaderboard.ts` ranks only decision-grade,
  public-safe reports and models owner-armed module-author splits from
  composition evidence while keeping payout, settlement, listing, and public
  marketplace authority disabled.
- **Epic E1/E2/E3/E4 landed:** `harbor-dispatch.ts` formalizes the Worker/Gym →
  Hydralisk → Harbor seam for Terminal-Bench against `openagents/khala` and
  closed GLM-REAP replication profiles: typed job spec, injected Hydralisk
  harness, public-safe
  `hydralisk.evals.terminal_bench.summary.v1` ingest, ATIF artifact ref, and a
  test that the Worker imports no Harbor runtime code. Dispatch receipts now
  must include distinct-device verifier evidence: `environment_mode = separate`,
  distinct agent/verifier host+device refs, `no-network` verifier, explicit
  artifact handoff, and reward read from the verifier artifact.
  `harbor-reward.ts` then maps Harbor rewards to cost-per-accepted-outcome using
  served-token cost basis, emits a training-ready public-safe ATIF trajectory
  bundle, and blocks readiness when GPU contention is not cleared.
- **Epic E4 landed (#6256):** `terminal-bench-comparison.ts` builds
  `openagents.gym.terminal_bench_comparison_report.v1`, comparing GLM-REAP
  profile refs against the external 69.1% Terminal-Bench 2.0 target as an
  external claim, not an OpenAgents result. Decision-grade replication requires
  the official 89-task denominator, owner approval, public-safe summary,
  distinct-device verifier evidence, served-token cost basis, and cleared
  GPU-contention evidence; pilot/attempted-only denominators remain visible but
  cannot satisfy the replication claim.
- **Epic E5 landed (#6257):** public `/gym` visualizes the Terminal-Bench replay
  using `@openagentsinc/three-effect`, with an accessible text/table mirror and
  no raw prompts, completions, private endpoint material, bearer material, or
  hidden tokens. Full Autopilot Verse integration remains deferred.
- **Epic E5b landed (#6261, #6271):** active owner-armed Harbor runs have a live
  `openagents.gym.run_progress.v1` status path, D1-backed Worker ingest, and
  `/gym` follow-along view over the same three-effect visual language. Operators
  run `bun run gym:harbor-progress-push -- --result path/to/result.json ...`
  beside Harbor; the pusher sends only counts, public-safe refs, token counts,
  and freshness to `POST /api/operator/gym/run-progress`. Raw trajectories,
  prompts, completions, logs, pane recordings, verifier stdout, bearer material,
  private endpoints, and task ids stay out of the Worker and public projection.
- **Issue #6272 landed:** a bounded Harbor Terminal-Bench 2.0 smoke ran through
  `openagents/khala`, moved the public Khala token counter, and published running
  plus completed snapshots to `/api/public/gym/run-progress` / `/gym`. Evidence:
  [`2026-06-25-khala-terminal-bench-through-openagents-run.md`](2026-06-25-khala-terminal-bench-through-openagents-run.md).
- **Epic E6 landed (#6258):** `terminal-bench-khala-orchestration.ts` compares
  decision-grade Khala Terminal-Bench policy reports against the raw Z.ai
  GLM-5.2 REAP baseline and emits explicit `beats_on_solve_rate`,
  `beats_on_cost_per_accepted_outcome`, `no_win`, `blocked`, or `not_measured`
  outcomes. The paired flywheel projection is evidence-only: no public claim,
  runtime promotion, payout, settlement, or provider mutation authority is
  granted by the comparison.
- **Epic F1/F2 landed:** `token_usage_events` now carries typed owner-gated
  demand attribution (`internal`, `external`, or `unlabeled` plus source/client
  labels), `GET /api/admin/inference-analytics` exposes the split plus
  demand-client/day adoption, and public `/stats` shows the tokens-served per-day
  curve while the public Khala tokens-served counter stays total-only. Remaining
  real-sweep work expands live executor wiring and throughput measurement.

## Contents

- [`ROADMAP.md`](ROADMAP.md) — **the one unified build roadmap** (epics → proposed
  GitHub issues with titles + bodies) covering QA, the Gym, Terminal-Bench/Harbor,
  ecosystem-tool landings, the dogfood lanes, and measurement — keyed to the North
  Star (tokens served/day). Start here for "what to build, in what order."
- [`openagents-gym.md`](openagents-gym.md) — the Gym spec (what the Gym _is_).
- [`2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md`](2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md)
  — Episode 243 considerations: the OpenCode head-to-head, BigPickle (de-TBD'd),
  the expanded lane set (Fireworks DeepSeek V4 Flash, GPT-OSS via Hydralisk,
  GLM 5.2 (Z.ai)), the real cost basis, and the train-and-use-Khala flywheel.
- [`2026-06-25-harbor-for-gym-terminalbench-and-benchmarks.md`](2026-06-25-harbor-for-gym-terminalbench-and-benchmarks.md)
  — audit: using Harbor (the official Terminal-Bench 2.0 harness + ATIF) as the
  Gym's executor/verifier for `terminal-bench` and other benchmarks; the
  Hydralisk/Psionic placement, the separate-verifier distinct-device seam, and the
  trajectory→training flywheel.
- [`2026-06-25-khala-terminal-bench-through-openagents-run.md`](2026-06-25-khala-terminal-bench-through-openagents-run.md)
  — evidence note for #6272: a bounded Harbor Terminal-Bench 2.0 run through
  `openagents/khala`, token-counter verification, D1 migration repair, and live
  `/gym` progress projection.
- [`2026-06-30-mutalisk-khala-code-gym-integration-audit.md`](2026-06-30-mutalisk-khala-code-gym-integration-audit.md)
  — audit: how Mutalisk should plug into the Gym as the offline GEPA optimizer
  lane for Khala Code fleet delegation, what is already contract-ready, and what
  backend/UI seam is still required before product testing is meaningful.
- [`2026-06-24-openagents-gym-issues-6164-6166-audit.md`](2026-06-24-openagents-gym-issues-6164-6166-audit.md)
  — the Phase 0 / `/gym/oss` issue-run audit (#6163–#6167).

## Related (in this repo)

- [`../khala/khala.md`](../khala/khala.md) — the Khala model the Gym trains.
- [`../khala/2026-06-23-khala-benchmark-harness-book-p1-5.md`](../khala/2026-06-23-khala-benchmark-harness-book-p1-5.md)
  — the typed benchmark matrix/runner/report the Gym compiles to (book P1-5 / #6088).
- [`../khala/2026-06-23-khala-blueprint-program-and-plugin-extensibility.md`](../khala/2026-06-23-khala-blueprint-program-and-plugin-extensibility.md)
  — typed programs + the plugin/marketplace layer the Gym composes (and its
  no-public-marketplace boundary).
- [`../khala/khala-in-the-world.md`](../khala/khala-in-the-world.md) — the
  Verse fan-out/verdict/cost visual language the Gym run scene reuses.
- [`../khala/2026-06-23-khala-head-to-head-m8-status.md`](../khala/2026-06-23-khala-head-to-head-m8-status.md)
  — the M8 head-to-head, a first Gym environment.
- [`../khala/khala-buildout-roadmap.md`](../khala/khala-buildout-roadmap.md)
  — the M0–M8 buildout the coordinator candidates come from.
- [`../inference/2026-06-25-khala-inference-gtm-push.md`](../inference/2026-06-25-khala-inference-gtm-push.md)
  — the GTM push (Pillar 3 "run it through the gym — the benchmark ladder").
- [`../inference/2026-06-25-opencode-khala-runbook-and-audit.md`](../inference/2026-06-25-opencode-khala-runbook-and-audit.md)
  - [`../opencode/`](../opencode/) — pointing OpenCode at Khala (the first Gym
    client surface) and the OpenCode-via-Khala planning memos.
- [`../inference/2026-06-25-khala-cost-model-and-analytics.md`](../inference/2026-06-25-khala-cost-model-and-analytics.md)
  — the real per-lane cost basis the Gym's cost-per-accepted-outcome consumes.
- [`../transcripts/243.md`](../transcripts/243.md) — Episode 243, "Khala in
  OpenCode" (the source for the considerations doc above).

> Status: implementation-linked spec, honest-scope. Not a product promise,
> served public capability, or public-claim copy.
