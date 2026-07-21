# RLM-07 — Dense-Recall Evaluation And Honesty Gate Over OpenAgents Transcripts

**Date:** 2026-07-21
**Lane:** evidence and analysis. This document makes no public product claim. It
flips no promise state, admits no issue, and changes no runtime behavior. It
reports retrieval-quality evidence and keeps that evidence separate from runtime
conformance.
**Issue:** RLM-07 (#9143). Parent #9136. Semantic consumer #9141.
**Authorities:** `AGENTS.md`, `INVARIANTS.md`, and live issue state stay the
factual authorities. The SDK conformance and evaluation contracts come from
`@openagentsinc/rlm` 0.2.0-rc.1 (OpenAgentsInc/ai#11, #13, and
`docs/rlm/PAPER-AUDIT.md` in that repository).

## 1. Purpose and boundary

This evaluation measures dense-history usefulness and cost on OpenAgents
transcript shapes. It runs before OpenAgents enables automatic semantic
escalation, depth above one, or any long-context quality claim. The evaluation
is hermetic. It uses scripted models and deterministic fixtures, so a clean
checkout reproduces every number with no network and no spend.

The evaluation package is `@openagentsinc/rlm-recall-eval`
(`packages/rlm-recall-eval`). The raw aggregate artifact is
`docs/rlm/rlm-recall-eval-hermetic-aggregate.json`. That artifact is the
source of the numbers below.

## 2. What the evaluation consumes from the SDK

The evaluation consumes the published `@openagentsinc/rlm` contracts and forks
no evaluation module into an OpenAgents package. It reuses:

- the engine entry points `makeRlm`, the inline corpus source Layer, and the
  deterministic and semantic request path,
- the corpus builder `buildInlineCorpusInput` and the citation machinery,
- the program Schema, the budget and evidence defaults, and the terminal result
  Schema (`RlmCompleted`, `RlmPartial`, `RlmRefused`, `RlmHonesty`, and
  `RlmTokenUsage`),
- the scripted-model plan interface (`RlmModelPlan`) for a deterministic root
  and leaf reader.

The evaluation adds OpenAgents-specific transcript generators, tier runners,
scoring, a versioned price-catalog cost scorer, and the two product-admission
gates.

## 3. Method

### 3.1 Transcript families and sizes

The generators build corpora that mirror the desktop history corpus shape in
`apps/openagents-desktop/src/desktop-history-corpus-source.ts`. They use
chronological turns, the `openagents.desktop.history_cursor.v1` source-address
scheme, and `turnId#sequence` entry refs. Generation is pure and deterministic.

The evaluation follows the paper work classes from `PAPER-AUDIT.md` section 5:

- `constant` (O(1)): the answer is one final decision near the end. Work does
  not grow with history length.
- `linear` (O(n)): three planted keyfacts sit at early, middle, and late
  positions, so bounded-window recall meets each placement.
- `pair` (O(n^2)): two planted turns share a tag and carry conflicting values.
  The answer requires both turns.

The representative history sizes are 8, 64, 256, 1024, and 4096 turns. These
sizes stay under the SDK inline corpus byte ceiling of 4 MiB, so every run
executes. The total is 200 hermetic runs.

### 3.2 Tiers compared

Every question runs through eight tiers. Tier D and every Tier S variant run
through the published engine. The direct, bounded-window, and provider-compaction
baselines are deliberate non-engine strategies. All tiers hold the same fixed
reader capability, so the contrast is retrieval strategy and not model capability.

- `direct`: the whole corpus goes into one model call. A corpus larger than the
  per-call prompt headroom refuses before the call.
- `tier_d`: deterministic grep. No model call. No spend.
- `semantic_depth0`: a symbolic environment with zero subcalls.
- `semantic_modelmap`: a one-shot ModelMap fan-out. Depth stays the same.
- `semantic_depth1`: a recursive RlmMap at depth one.
- `semantic_depth2`: a separately admitted higher depth.
- `bounded_window`: only the last 32 turns are visible.
- `provider_compaction`: the last 16 turns stay verbatim and older turns are
  compacted with their precise values dropped.

### 3.3 Scoring

The evaluation scores answer correctness, citation exactness and coverage,
abstention and partial honesty, modeled latency, model-call distribution, token
distribution, and cost. It reports p50, p75, p90, p95, and p99 and stratifies by
outcome, because the paper appendices show that means hide the sharp tails and
the divergence between successful and failed trajectories.

Two scoring rules keep the outcome honest:

- A tier that surfaces evidence but does not combine it into an answer scores as
  partial on a pair task, not success. Only a leaf or reduce model call counts
  as synthesis.
- Honest abstention never scores as a wrong answer. A window miss or a per-call
  limit produces a refused outcome, not an incorrect one.

### 3.4 Versioned price catalog and cost honesty

Cost uses a pinned snapshot of the OpenAgents price catalog,
`openagents.price-catalog.2026-07-21`. The source of truth is
`apps/openagents.com/workers/api/src/inference/pricing.ts` (`MODEL_PRICING_TABLE`).
The snapshot carries the cost-basis provenance from that table. The Vertex Claude
and Gemini rows are the published list rate and carry the billing placeholder
label. The Fireworks open-model rows are measured.

Unknown usage stays unknown. A model call without exact token counts is excluded
from cost aggregates and labeled, never priced as zero. An unknown model returns
an unknown cost, never a fabricated one.

The default scored model is `gemini-3.5-flash`, the desktop free-tier lane. Its
cost basis is the list placeholder, so every cost figure below carries that
label and is a planning estimate, not a settled rate.

### 3.5 Fixture, version, and model refs

- SDK: `@openagentsinc/rlm` 0.2.0-rc.1.
- Strategy profile: `openagents.rlm-recall-eval.scripted.v1`.
- Price catalog: `openagents.price-catalog.2026-07-21`.
- Scored model: `gemini-3.5-flash` (cost basis list_placeholder).
- Config: families constant, linear, pair. Sizes 8, 64, 256, 1024, 4096.

## 4. Headline results

The table reports the 200 hermetic runs. Success, partial, incorrect, and
refused are counts over 25 runs per tier. Citation coverage is the p50 fraction
of expected refs cited. Calls p95 is the p95 of model calls plus subcalls. Cost
p95 is the p95 of known cost in United States dollars.

| tier                | success | partial | incorrect | refused | cite.cov.p50 | calls.p95 | cost.p95 (USD) |
| ------------------- | ------- | ------- | --------- | ------- | ------------ | --------- | -------------- |
| direct              | 20      | 0       | 0         | 5       | 0.00         | 1         | 0.00122250     |
| tier_d              | 20      | 5       | 0         | 0       | 1.00         | 0         | 0.00000000     |
| semantic_depth0     | 20      | 5       | 0         | 0       | 1.00         | 1         | 0.00002617     |
| semantic_modelmap   | 25      | 0       | 0         | 0       | 1.00         | 3         | 0.00006810     |
| semantic_depth1     | 25      | 0       | 0         | 0       | 1.00         | 3         | 0.00005865     |
| semantic_depth2     | 25      | 0       | 0         | 0       | 1.00         | 3         | 0.00005865     |
| bounded_window      | 12      | 0       | 1         | 12      | 0.00         | 1         | 0.00003998     |
| provider_compaction | 10      | 0       | 0         | 15      | 0.00         | 1         | 0.00428317     |

The tier comparison shows four findings.

1. Deterministic Tier D answers every constant and linear single-value question
   at zero model cost with an exact, digest-anchored citation. It surfaces both
   spans of a pair task but does not combine them, so a pair task scores partial.
   This is the free default the desktop tier policy runs first.
2. The symbolic environment at depth zero matches Tier D quality on single-value
   questions with one cheap model call. It stays partial on pair tasks, because
   it commits evidence without synthesis.
3. Only the synthesizing tiers (semantic_modelmap, semantic_depth1,
   semantic_depth2) answer the pair task in full. They keep exact citations. The
   cost is a few one-hundred-thousandths of a dollar per run at the scored model.
4. The non-engine baselines lose recall. The direct baseline refuses when the
   corpus passes the per-call prompt headroom, and it never emits an exact
   citation. The bounded-window baseline abstains on distant facts. The
   provider-compaction baseline drops precise facts in the compacted region and
   still carries the largest prompt cost tail, because the verbatim tail and the
   summary both ship to the model.

## 5. Distributions

The distributions confirm the paper point that depth and fan-out are not free
and that tails matter.

- Model calls plus subcalls: tier_d p95 is 0. semantic_depth0 p95 is 1.
  semantic_modelmap p95 is 3. semantic_depth1 and semantic_depth2 p50 is 2 and
  p95 is 3.
- Modeled latency in scripted call units (not wall-clock): tier_d is 0.
  semantic_depth0 is 1000. semantic_modelmap is 3000. The synthesizing tiers pay
  latency for every leaf call.
- Cost is known for all 25 runs of every tier in the main matrix and carries the
  list placeholder basis. No run in the main matrix has unknown usage.

## 6. Product-admission gates

Both gates define explicit pass or fail criteria. Both stay disabled. The
evaluation flips no switch. Enabling either gate is a separate product admission,
not an engine default. The desktop Tier S consumer keeps semantic recall
host-admitted and depth clamped to one regardless of this result.

### 6.1 Automatic Tier S escalation

The escalation gate passes only when a synthesizing semantic tier clearly answers
the family Tier D cannot synthesize, with exact citations, without adding wrong
answers, and with known cost. The criteria are:

- Tier D is insufficient on the pair family (pair success is zero and pair
  partial is above zero).
- The synthesizing semantic tier reaches a pair success rate of at least 1.
- The synthesizing semantic tier keeps citation coverage and exactness at 1.
- The synthesizing semantic tier adds no wrong answers.
- The synthesizing semantic tier cost is known.

On the current evidence every criterion passes, so the gate result is
`wouldPass=true`. The admission stays `admitted=false`. Turning on automatic
escalation is a separate product decision under the desktop admission policy.

### 6.2 Depth above one

The depth gate passes only when depth two strictly improves quality over depth
one without adding wrong answers or raising the tail cost. The criteria are:

- Depth two has a strictly higher success rate than depth one.
- Depth two adds no wrong answers relative to depth one.
- Depth two tail cost at p95 is not worse than depth one.

On the current evidence depth two ties depth one. The strict-improvement
criterion fails, so the gate result is `wouldPass=false`. Depth above one stays
disabled. This matches the paper finding that depth is not a monotonic
improvement.

## 7. Honesty findings

A separate honesty probe runs the synthesizing semantic tier with model usage
reporting turned off. It runs 9 times. Every run keeps usage unknown and is
excluded from cost, never priced as zero. The probe confirms the cost aggregate
honors the unknown-usage rule.

The retrieval-only tiers (tier_d and semantic_depth0) score partial on pair
tasks. They do not overclaim a synthesized answer they did not produce. The
bounded-window and provider-compaction baselines abstain when the fact is out of
reach, so a miss is a refused outcome and not a confident wrong answer. The one
incorrect bounded-window result is a case where the window held one of the two
conflicting values and the reader returned an incomplete answer to a
report-both question. That case is scored as incorrect, not partial, because the
reader asserted an incomplete answer rather than abstaining.

## 8. Limitations

- Scale. The SDK inline corpus has a 4 MiB byte ceiling, so the executed sizes
  stop at 4096 turns. The million-token and 10M-token out-of-core scales in the
  paper are not executed here. They need the SDK out-of-core corpus handle and a
  streaming source Layer. This evaluation documents that as a gap and does not
  model those scales as if they ran.
- Recursion depth. In the pinned SDK 0.2.0-rc.1 the recursive child runner is
  single-level. Depth two therefore executes the same work as depth one, and the
  depth-two rows are labeled as modeled rather than truly nested. The depth gate
  correctly stays failed for that reason.
- Scripted models. The reader capability is fixed and deterministic. The
  evaluation measures retrieval strategy under a constant reader, not model
  reasoning quality. A live run under a real provider is a separate lane.
- Price catalog. The scored model is a list-placeholder rate. Every cost figure
  is a planning estimate and re-solves when the real committed-use rate lands in
  the worker price catalog.
- Latency. Latency is a modeled sequential figure in scripted call units. It is
  not wall-clock. Structured concurrency in the engine reduces real latency
  below the sequential model.

## 9. Reproducibility

From a clean checkout with dependencies installed:

- Run the hermetic suite:
  `pnpm --filter @openagentsinc/rlm-recall-eval test`.
- Regenerate the raw aggregate artifact:
  `pnpm --filter @openagentsinc/rlm-recall-eval run eval:hermetic`.
  This writes `docs/rlm/rlm-recall-eval-hermetic-aggregate.json`.

The aggregate carries no timestamp and no machine fact, so two runs produce a
byte-identical artifact. A test asserts that property.

The live-provider path is a separate owner-triggered command,
`pnpm --filter @openagentsinc/rlm-recall-eval run eval:live`. It refuses to run
without the `OPENAGENTS_RLM_EVAL_LIVE=1` flag, an admitted account ref, explicit
call and spend caps, and a bound live model module. Live output is written
separately and marked `meta.kind=live`, so live and hermetic results can never be
confused.

## 10. Quality evidence versus runtime conformance

This document is retrieval-quality evidence. It is not runtime conformance. The
runtime conformance for the RLM engine lives in the SDK conformance suite
(OpenAgentsInc/ai#11). The desktop runtime boundary lives in the Tier S consumer
(`apps/openagents-desktop/src/history-recall-semantic.ts`) and its invariants.
This evaluation neither admits an issue nor authorizes a rollout. Any automatic
escalation or higher-depth rollout stays disabled until its stated gate passes
under a separate product admission.
