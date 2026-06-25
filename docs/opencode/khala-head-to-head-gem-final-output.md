Now I have a thorough picture. Here is the planning memo.

---

## Planning Memo: Head-to-Head Model Comparison Gym

### 0. Etymology — "BigPickle" and "gym"

The owner clarified that "gem" was the wrong hearing: the intended word is **gym** (G-Y-M), as in a benchmark/evaluation gym for head-to-head model runs. "BigPickle" remains a likely garbled voice transcription until separately confirmed. The earlier clarification (present in the `oa-wt-cost` worktree at `docs/inference/2026-06-25-khala-inference-gtm-push.md:279-283`) reads:

> Read charitably, these point at (a) a named **coding/inference benchmark** to run Khala against, and (b) a named **competitor tool/model** to beat — possibly a benchmark "gem" (harness/leaderboard) and a competitor codenamed something mis-transcribed. **Do not invent specifics.** The action item is to confirm the...

Update: **do not ask the owner what "gem" means anymore.** Treat it as **gym**: a reusable benchmark/evaluation gym that can run the same OpenCode task ladder against Khala and pluggable opponent endpoints. The remaining naming question is only what "BigPickle" refers to before wiring a specific comparator.

### 1. Benchmark Dimensions (what the matrix already measures)

The existing harness (`apps/openagents.com/workers/api/src/inference/benchmark/matrix.ts`) provides a typed cross-product grid:

| Axis | Values | Notes |
|---|---|---|
| **Lane** | `vertex-anthropic`, `vertex-gemini`, `fireworks`, `partner-passthrough`, `pylon-whole-small` (future), `psionic-shard-wan` (future) | LANE_AVAILABILITY table gates real vs future |
| **Engine** | `provider-native`, `vllm`, `sglang`, `tensorrt-llm` | Paired with lanes, never blind-crossed |
| **Workload** | `chat`, `khala-code-artifact-gen`, `verifier-run`, `long-context-codebase-question` | Each optimizes different metrics |
| **Sequence Shape** | ISL, OSL, cacheable prefix, concurrency, provenance (`realistic`/`synthetic`) | Must match production traffic for decision-grade |
| **Transport** | `streaming` / `batch` | TTFT only meaningful for streaming |
| **Sampling** | Temperature + reasoning effort | Production values, not benchmark-flattering |
| **Samples per cell** | Configurable (≥1) | Book §4.5.2: enough for percentiles |

**What the gym adds** on top of this matrix: a client-side surface (models compared via **OpenCode**, Aider, or direct API calls) rather than only comparing provider lanes behind the gateway. The existing matrix compares *supply lanes* (Fireworks vs Vertex). The gym compares *model endpoints* (Khala vs BigPickle/GPT/Claude/Gemini free tiers).

### 2. Fixtures — Prompt/Workload Corpus

The gym needs two layers of fixtures:

**A. Quick/taste fixtures** (no cost, deterministic, for harness proof):
- The existing `SAMPLE_DECISION_SUITE_CONFIG` in `fixtures.ts` with synthetic shapes (short-chat 350/220, code-artifact 2400/1800, long-codebase-32k 32000/600).
- OpenCode specific: the `docs/inference/2026-06-25-opencode-khala-runbook-and-audit.md` smoke task: "read docs/faq/khala-inference-quickstart.md and reply with base=...; model=..."
- Crossy-road north-star prompt: "build a really high quality single html file crossy road game with three.js"

**B. Owner-armed realistic fixtures** (decision-grade, need real spend approval):
- Shapes sourced from **observed Khala traffic** (once internal dogfood from Pillar 1 generates it)
- Concrete coding tasks from the OpenCode/Aider ecosystem — tasks that exercise tool-calling, edit/run loops, and file reads
- The existing `RealTrafficShapeEvidence` schema in `real-sweep-plan.ts` provides the evidence contract for this

**Discipline:** fixture runs stay labeled `synthetic` / `illustrative`. Only traffic-sourced shapes with public-safe evidence refs qualify for `decisionGrade: true`.

### 3. Spend Gates (existing, ready to use)

The `real-sweep-plan.ts` preflight system is already gated:

```
preflightRealBenchmarkSweep(config, {
  ownerConfirmed: true,
  ownerApprovalRef: "issue/PR ref",
  budgetCapMsat: <positive number>,
  maxBillableSamples: <positive number>,
  trafficEvidence: [RealTrafficShapeEvidence, ...],
})
```

Blockers (preflight refuses to arm):
- `owner_confirmation_missing`
- `owner_approval_ref_missing`
- `budget_cap_missing`
- `billable_sample_cap_missing` / `billable_sample_cap_exceeded`
- `real_traffic_evidence_missing` / `real_traffic_evidence_invalid`
- `no_available_cells`

`makeRealLaneSeam` throws `RealLaneNotArmedError` when unarmed. The gym must enforce the same gate for live API calls to competitors.

**For the gym specifically:** calling competitor APIs (OpenAI, Google Gemini free tier, Claude API, etc.) is real spend too. Each competitor lane needs its own budget cap, arm flag, and token accounting. The same `RealLaneSeam` architecture extends here by adding competitor lanes as new `BenchmarkLane` values — e.g., `openai-gpt`, `gemini-free`, `claude`, `bigpickle` (once confirmed).

### 4. Token Accounting

The existing telemetry schema (`buildKhalaTelemetryRecord`) already captures:
- `promptTokens`, `completionTokens`, `totalTokens`
- `cachedInputTokens`
- `costBasisMsat` (provider cost)
- `settlementState`

**For the gym:** each competitor call must return its own `usage` block. The runner records:
- Tokens from the provider's `usage` response (never estimated)
- Wall-clock from the benchmark runner's clock (TTFT, total wall-clock, generation wall-clock)
- Cost from the provider's stated pricing OR `not_measured` when unbilled (free tier)
- Verification outcome from the independent verifier (not from any model's self-claim)

**Honesty rule:** free-tier competitor calls have `costBasisMsat: 0` but are labeled with `provenance: 'free_tier'`. They are not "free" in the engineering sense (they consume quota/rate-limits), but the direct spend is zero.

### 5. Outputs — What the Gym Produces

The existing `BenchmarkReport` schema (`report.ts`) is the canonical output:

```
BenchmarkReport {
  schemaVersion: "openagents.khala.benchmark-report.v1"
  configId, seamId, decisionGrade, illustrativeNotice
  cellsExpanded, cellsExecuted, cellsSkipped
  groups: [{
    lane, workload, laneAvailability, syntheticOnly
    executedSamples, skippedSamples
    ttftMs: { p50, p90, p99, mean, sampleCount }
    totalWallClockMs: { ... }
    perceivedTps: { ... }
    interTokenLatencyMs: { ... }
    cacheHitRate
    verificationRate, acceptedOutcomes, attemptedVerifications
    costPerAcceptedOutcomeMsat, totalCostBasisMsat
  }]
  speculationAcceptance: [{ workload, model, temperature, route, mode, acceptanceRate, ... }]
}
```

**Gym-specific additions** (extend the report or produce a companion artifact):
- **Client attribution**: which client/CLI drove each run (OpenCode v1.17.9, Aider, direct curl)
- **Tool-call completion rate**: did the model actually call tools, and did calls complete? This is the single highest-risk dimension for OpenCode.
- **Verification outcome per run** (already supported via `BenchmarkLaneSample.executedVerdict`)
- **Cost-per-accepted-outcome** at the model level, not just lane level (already supported but needs competitor cost basis)

### 6. How to Compare Khala Against BigPickle/Free Models Honestly

**The ladder (from GTM push §3):**

1. **Khala vs BigPickle** (baseline — once confirmed what this is)
2. **Khala vs free/open models** (Gemini Flash free tier, Llama, Qwen, Mistral free API endpoints)
3. **Khala vs paid frontier models** (GPT-5, Claude Sonnet 4, Gemini 2.5 Pro)

**Honesty principles:**

| Principle | Enforced By |
|---|---|
| Same prompt, same task, same verifier | The matrix runner drives identical cells for every lane |
| Every competitor lane is typed, gated, and labeled | New `BenchmarkLane` values + `LANE_AVAILABILITY` table |
| Free-tier competitor calls are labeled `free_tier`, not free-forever | `provenance` on shape, `canSpend` on seam |
| Cost-per-accepted-outcome is null when no accepted outcome | `report.ts` line 209-214: `acceptedOutcomes === 0 ? null` |
| Fixture numbers are never published as measurements | `decisionGrade: false` + `illustrativeNotice` |
| Competitor exact model id, API version, and date are recorded | Extend `BenchmarkLaneSample` or run metadata |
| No fabricating competitor latency/cost from docs | Competitor calls go through the same real seam with the same measurement |
| Public safety: no prompts/completions/keys in reports | `checkReportPublicSafety` tripwire |
| External reported claims (e.g., Fugu numbers from X) are separated | The head-to-head demo manifest already has `externalReportedClaims` |

**The OpenCode integration step** (runbook step 5): the gym adds OpenCode as the first *client surface* — not comparing supply lanes but comparing entire coding agent experiences. For each model under test (Khala, BigPickle, Gemini-free, etc.), run the same OpenCode task against that model's endpoint, record tokens/wall-clock/tool-call-completion/verification-outcome, and produce the same report schema.

### 7. Implementation Sequence

1. **Confirm "BigPickle" with the owner** — "gym" is already clarified as the benchmark/evaluation gym; do not wire a BigPickle-specific lane until the comparator is confirmed
2. **Add competitor lanes** as new `BenchmarkLane` values (e.g., `bigpickle`, `gemini-free`, `openai-gpt`) with proper `LANE_AVAILABILITY` entries
3. **Extend the runner** to accept competitor API credentials via environment variables (matching the existing `makeRealLaneSeam` gating pattern)
4. **Add OpenCode as a client runner** — a module that provisions the `opencode.json`, runs a task, and extracts usage/wall-clock/verification from the output
5. **Define the first realistic fixture set** from the QA runner or internal dogfood traffic
6. **Run an owner-armed real sweep** with budget cap and approval ref, comparing Khala vs the confirmed competitor(s) on the OpenCode task
7. **Publish the first decision-grade report**, clearly labeled with competitor model ids, dates, and all honesty caveats

### 8. Key Open Questions for the Owner

- What exactly is "BigPickle"? A benchmark suite, a competitor product, a model name?
- What should the first gym artifact be named in code and reports (`khala-gym`, `opencode-gym`, or a broader benchmark-suite name)?
- Budget: what msat cap should the first armed sweep carry?
- Is the first comparison on the OpenCode coding-agent surface (tool calling) or direct chat-completion quality (rubric/verifier)?
