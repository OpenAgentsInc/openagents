# OpenAgents Gym — docs

The **Gym** is the interactive experimentation surface and eval+reward factory
that trains [Khala](../khala/khala.md). Like OpenAI's original Gym (standard
environments + one interface so policies can be compared), it lets you configure a
Khala **policy** — coordinator candidate × provider fan-out × tool set ×
plugin/module composition × sampling × quantization/speculation — run it against a
registered **environment** (Terminal-Bench, `khala-code`, long-context QA, the M8
head-to-head, the **OpenCode coding-agent head-to-head**, ...), and score it on the
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
- GPT-OSS owner/internal latency playground is landed at `/gym/oss`:
  `#6167`. It is auth/owner-gated, capped at eight in-flight requests, streams
  against `openagents/khala-oss-20b`, and keeps `not_measured` distinct from
  fabricated zeroes.
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
  Remaining real-sweep work expands the executor backends and public surfaces.

## Contents

- [`ROADMAP.md`](ROADMAP.md) — **the one unified build roadmap** (epics → proposed
  GitHub issues with titles + bodies) covering QA, the Gym, Terminal-Bench/Harbor,
  ecosystem-tool landings, the dogfood lanes, and measurement — keyed to the North
  Star (tokens served/day). Start here for "what to build, in what order."
- [`openagents-gym.md`](openagents-gym.md) — the Gym spec (what the Gym *is*).
- [`2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md`](2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md)
  — Episode 243 considerations: the OpenCode head-to-head, BigPickle (de-TBD'd),
  the expanded lane set (Fireworks DeepSeek V4 Flash, GPT-OSS via Hydralisk,
  GLM 5.2 (Z.ai)), the real cost basis, and the train-and-use-Khala flywheel.
- [`2026-06-25-harbor-for-gym-terminalbench-and-benchmarks.md`](2026-06-25-harbor-for-gym-terminalbench-and-benchmarks.md)
  — audit: using Harbor (the official Terminal-Bench 2.0 harness + ATIF) as the
  Gym's executor/verifier for `terminal-bench` and other benchmarks; the
  Hydralisk/Psionic placement, the separate-verifier distinct-device seam, and the
  trajectory→training flywheel.
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
  + [`../opencode/`](../opencode/) — pointing OpenCode at Khala (the first Gym
  client surface) and the OpenCode-via-Khala planning memos.
- [`../inference/2026-06-25-khala-cost-model-and-analytics.md`](../inference/2026-06-25-khala-cost-model-and-analytics.md)
  — the real per-lane cost basis the Gym's cost-per-accepted-outcome consumes.
- [`../transcripts/243.md`](../transcripts/243.md) — Episode 243, "Khala in
  OpenCode" (the source for the considerations doc above).

> Status: implementation-linked spec, honest-scope. Not a product promise,
> served public capability, or public-claim copy.
