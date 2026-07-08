# OpenCode Gym Benchmark Ladder — Khala vs the Field

> **Status:** planning doc, 2026-06-24. Defines the three-rung benchmark ladder
> that the Gym runs on a recurring basis. Every rung uses the same typed harness
> (matrix, runner, lane-seam, report schema). Numbers are only publishable from
> owner-armed real seams over traffic-sourced fixtures (`decisionGrade: true`).
> Synthetic/illustrative runs are training, not publication.

## Audience

This doc is for **OpenCode users evaluating Khala** as their coding agent model,
and for the **Gym maintainers** wiring new competitor lanes. Every comparison
described below runs on the exact OpenCode surface: a real coding task, same
prompt, same verifier, recorded tokens + wall-clock + tool-call-completion +
verification outcome.

## The Ladder (three rungs)

| Rung | Opponent | Purpose | Bar to clear |
|------|----------|---------|--------------|
| 1 | **Big Pickle** | Baseline — the default free model in OpenCode | Beat on cost-per-accepted-outcome AND verified-rate on the same coding task |
| 2 | **Free/open models** | The field users compare to when not paying | Equal or better on verified-rate at equal or lower cost, or clearly better on cost-per-accepted-outcome |
| 3 | **Paid frontier models** | The upper bound — what users pay for | Measure the gap; track closing over time. No requirement to beat — honesty demands showing the gap |

### Rung 1 — Khala vs Big Pickle

**Big Pickle** is the main free model shipped with OpenCode. This is the
first and most important rung: if Khala cannot beat Big Pickle on the
OpenCode surface, it has no reason to exist as an OpenCode provider.

**Competitor lane:** `bigpickle` — registered as `fixture_only` for the no-spend
Gym fixture. The fixture selector is `opencode/bigpickle`; an owner-armed real
sweep must resolve and record the exact upstream OpenCode model id + API version
+ date before a decision-grade report.

**What we measure (all on the same coding task, same prompt, same verifier):**

| Metric | Target |
|--------|--------|
| Verified task completion rate | Khala >= Big Pickle |
| Cost-per-accepted-outcome | Khala < Big Pickle |
| Tool-call completion rate | Khala >= Big Pickle |
| P50/P90 wall-clock per task | Khala <= Big Pickle |

**Decision-grade requires:**
- Minimum 30 samples per cell (the existing harness default)
- Shapes sourced from observed OpenCode traffic or QA-runner tasks (not synthetic)
- `decisionGrade: true` with `ownerApprovalRef` + `budgetCapMsat` + `trafficEvidence`
- Report lists exact model ids, API versions, date of run
- Public-safety tripwire passes (no prompts/completions/keys leaked)

### Rung 2 — Khala vs Free/Open Models

The models an OpenCode user reaches for when they do not want to pay or
want to try before buying. This is the comparison group that matters for
OpenCode's "try any model" workflow.

**Competitor lanes** (exact model ids TBD — record at run time):

| Lane | Likely Candidate | Notes |
|------|-----------------|-------|
| `gemini-free` | Gemini 3.5 Flash (free tier) | Google AI Studio / Gemini API free quota |
| `llama-free` | Llama 4 Scout / Instruct via Together or Groq free tier | Whichever is the default free Llama in OpenCode's provider list |
| `qwen-free` | Qwen3 / QwQ via free API | If listed as a free option in OpenCode |
| `mistral-free` | Mistral Small / Codestral free tier | If OpenCode lists a Mistral free option |

**Honesty constraints:**
- Free-tier competitor calls are labeled `provenance: 'free_tier'` with
  `costBasisMsat: 0` — they are free in direct spend but consume quota.
  Never imply "free forever" or "free at scale."
- If a competitor lane has rate limits that prevent statistically meaningful
  sampling, note that in the report and label the comparison `rate_limited`.
- If a competitor model changes between Gym runs (e.g., Gemini Flash free
  tier gets a new checkpoint), record the exact serving model id and flag
  the change in the report.

**What qualifies as "winning" this rung:**
- Khala achieves equivalent or better verified-rate on the same OpenCode task.
- Khala cost-per-accepted-outcome is lower than every free-tier competitor
  (trivially true if Khala is free and the others are free — in that case,
  verified-rate and tool-call completion are the only axes).
- If a free-tier competitor is genuinely better on verification, the report
  says so — no spin.

### Rung 3 — Khala vs Paid Frontier Models

The models OpenCode users currently pay for. This rung measures the gap and
tracks it over successive Gym runs. The goal is **not** to beat these models
today — the goal is to honestly measure how far away Khala is and to make
that gap shrink.

**Competitor lanes:**

| Lane | Model | API |
|------|-------|-----|
| `openai-gpt` | GPT-5 / GPT-4.1 / o-series (latest coding-optimized) | OpenAI API |
| `anthropic-claude` | Claude Sonnet 4 / Opus (latest coding-optimized) | Anthropic API |
| `google-gemini` | Gemini 2.5 Pro (latest) | Gemini API |

**What we measure (same task, same verifier, same prompt):**

| Metric | Purpose |
|--------|---------|
| Verified task completion rate | The primary quality axis |
| Cost-per-accepted-outcome | The value axis — even if Khala is less capable, the cost ratio may still win |
| P50/P90 wall-clock | Speed comparison |
| Tool-call completion rate | Coding-agent-specific quality |
| Gap (frontier rate − Khala rate) | Tracked over time — the number we want to shrink |

**Spend gates:**
- Each competitor lane requires its own `budgetCapMsat` + `ownerConfirmed`
  flag in the preflight. These calls incur real API spend.
- The total sweep budget must be approved by the owner per run.
- `maxBillableSamples` per cell prevents runaway sampling.

**Report honesty rules:**
- Never claim "beats frontier" based on a single task or a single run.
- Always report the competitor's exact model id + API version + date.
- If a frontier model fails due to rate limits, tool-call errors, or API
  unavailability, note that — do not silently skip.
- Gap shrinkage claims require a minimum of 3 runs over time showing
  monotonic or net improvement.

## Honesty Gates (applied to every rung)

| Gate | What it prevents | Enforced by |
|------|------------------|-------------|
| `decisionGrade: false` | Publishing synthetic/illustrative numbers as measurements | `checkReportPublicSafety` + report-level `decisionGrade` field |
| `owner_confirmation_missing` | Running real spend without approval | `preflightRealBenchmarkSweep` — the same gate used for supply-lane comparisons |
| `budget_cap_exceeded` | Unbounded spend on competitor API calls | Per-lane `budgetCapMsat` in preflight config |
| `real_traffic_evidence_missing` | Benchmarks on unrealistic prompts | `trafficEvidence` array with `RealTrafficShapeEvidence` entries |
| `public_safety_tripwire` | Leaking prompts/completions/keys in the report | `checkReportPublicSafety` returns `Fail` if `decisionGrade` is true and any field triggers a safety rule |
| `competitor_model_id_required` | Comparing against "GPT" without specifying which version | Report must list exact model id string, API version, date for every competitor cell |
| `provenance_separation` | Mixing free-tier competitor results with paid-lane results | Each competitor cell carries `provenance`: `free_tier`, `paid_api`, or `internal` |

## Decision-Grade Criteria (what qualifies for publication)

A Gym benchmark result is **decision-grade** and may be referenced in public
comparisons only when ALL of the following are true:

1. **`decisionGrade: true`** in the report header — set by the runner only
   when the preflight arming passed all gates.
2. **Traffic-sourced fixtures** — shapes are derived from observed production
   traffic (OpenCode sessions, QA-runner traces, internal dogfood). Synthetic
   shapes are disqualified.
3. **Minimum 30 samples per cell** — enough for stable percentiles.
4. **All competitor model ids recorded** — exact strings, API versions, dates.
5. **Public-safety tripwire passed** — no prompts, completions, or keys in
   the published artifact.
6. **`illustrativeNotice` is empty or absent** — if `decisionGrade` is true,
   there should be no `illustrativeNotice` (that field is for synthetic runs).

Any result that fails any of these is labeled **illustrative** and may not be
used in external comparisons or claims.

## Implementation Sequence

1. **Shipped (#6246): register OpenCode endpoint lanes** in the typed matrix:
   `khala`, `bigpickle`, `gemini-free`, `openai-gpt`, `claude`, plus own/open
   `gpt-oss-20b`, `gpt-oss-120b`, and `glm-52`. `bigpickle`, `gemini-free`,
   `openai-gpt`, and `claude` are `fixture_only` until a real executor is wired.
2. **Shipped (#6246): add the OpenCode client runner** in
   `workers/api/src/inference/benchmark/opencode-client-runner.ts`. It provisions
   public-safe `opencode.json`, extracts provider `usage` without token
   estimation, records wall-clock/tool-call success/verifier verdict, and feeds
   the existing report path.
3. **Next: add owner-armed real executors** for free-tier and paid competitor
   lanes, each with its own budget cap and `makeRealLaneSeam` arm gate.
4. **Source the first realistic fixture set** from QA-runner traces or
   internal dogfood sessions. Package as `RealTrafficShapeEvidence[]`.
5. **Extend the shipped runner with real competitor executors** that:
   - Read the fixture set and competitor lane list from config
   - For each owner-armed cell, call the competitor API with the same prompt
   - Records usage (provider `usage` block, never estimated), wall-clock
     (TTFT, total, generation), and verification outcome
   - Produce the same public-safe `BenchmarkReport` with competitor
     attribution
6. **Run an owner-armed real sweep** — first on rung 1 only (Khala vs Big
   Pickle), then expand to rungs 2 and 3.
7. **Publish the first decision-grade report** to a well-known path
   (e.g., `docs/opencode/gym-rung1-report-YYYY-MM-DD.md`) with full
   competitor model ids, dates, and honesty caveats.

## OpenCode-Facing Summary Table

| Rung | Opponent | Khala must... |
|------|----------|---------------|
| 1 | Big Pickle | Beat on verified-rate + cost-per-accepted-outcome |
| 2 | Free/open (Gemini Flash, Llama, Qwen, Mistral) | Match or beat on verified-rate; win on tool-call completion |
| 3 | Paid frontier (GPT, Claude, Gemini Pro) | Honest measurement of the gap; track shrinkage over time |

Every rung is re-run on every significant Khala model update. The Gym
publishes a timed, dereferenceable leaderboard at a recurring cadence
(e.g., weekly or per model release).
