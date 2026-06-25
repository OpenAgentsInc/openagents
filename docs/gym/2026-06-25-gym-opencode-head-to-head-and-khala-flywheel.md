# Gym — Episode 243 considerations: the OpenCode head-to-head, BigPickle, the expanded lane set, and the train-and-use-Khala flywheel

Updated: 2026-06-25

> **Status:** considerations + roadmap delta, honest-scope. This folds Episode 243
> ([`../transcripts/243.md`](../transcripts/243.md), "Khala in OpenCode") and the
> Khala work shipped 2026-06-24/25 into the Gym plan. It updates the Gym spec
> ([`openagents-gym.md`](openagents-gym.md)) and README rather than replacing them.
> Nothing here is a product promise, a served public capability, or public-claim
> copy. The product-promise registry governs claims; this is internal direction.
> Anything not already landed in the Phase 0 fixture Gym (#6163–#6167) is labeled
> **direction**, not a live claim.

## 0. Why this doc

In Episode 243 the owner named the Gym as the **next thing to build** after the
Khala inference launch: *"one of the things we actually end up building from this
will be a gym, so we can do better head-to-heads of different models — starting
with OpenCode, then expanding that out to other things."* And: *"I'm excited to
build the gym. And I'm hoping we can build the gym using the Khala coding agent."*

The episode also resolved two garbled-voice terms that had been sitting as TBDs,
expanded the set of provider lanes Khala routes across, grounded
cost-per-accepted-outcome in a real cost basis, and named the Gym's tightest
flywheel (it both **trains** Khala and **runs on** Khala). This doc captures those
deltas so the Gym spec stays honest and the next build steps are concrete.

## 1. Resolved terminology (no more TBDs)

- **"do the gym," not "do a gem."** The benchmark/eval surface is the **Gym** — the
  same Gym this folder specs. Multiple transcripts and one OpenCode-via-Khala memo
  had carried "gem" as a garbled transcription; the owner confirmed *"I said
  'do gym,' like, you know, do the gym."* Treat any lingering "gem" in derived
  docs as a transcription error to fix.
- **"BigPickle" = the main free model in OpenCode.** Confirmed by the owner:
  *"'Big Pickle,' with a space, is the main free model of OpenCode."* It is **not**
  an external benchmark or a mystery competitor — it is the default open/free model
  an OpenCode user reaches for without configuring a paid provider. That makes it
  **ladder rung 1**: the immediate baseline Khala must beat on
  cost-per-accepted-outcome and verified-rate, on the exact tool (OpenCode) we are
  courting. The GTM push's benchmark ladder
  ([`../inference/2026-06-25-khala-inference-gtm-push.md`](../inference/2026-06-25-khala-inference-gtm-push.md)
  §"Run it through the gym") can now de-TBD this rung.

## 2. The benchmark ladder, run through the Gym

The Gym is the harness for the ladder named in the GTM push — Khala benched against
a climbing field, all on identical prompts/tasks and our axes (tokens, $, wall-clock,
cost-per-accepted-outcome, verified-rate):

1. **BigPickle vs Khala** — OpenCode's default free model. The first concrete rung.
2. **Other open/free models vs Khala** — the free endpoints a developer would
   otherwise point OpenCode at (Gemini Flash free tier, Llama/Qwen/Mistral free
   APIs, OpenRouter free models).
3. **Paid frontier models vs Khala** — Claude/GPT/Gemini-class as the upper bar, to
   see the gap to frontier and track it closing.

The progression is deliberate: win the **free** field first (where
cost-per-accepted-outcome and verified-rate are the whole game), then measure the
climb toward paid frontier. Published numbers come **only** from an owner-armed real
seam over realistic traffic (`decisionGrade: true`); fixture/synthetic runs stay
labeled illustrative and are never published as measurements.

## 3. New dimension: the Gym compares **client surfaces**, not only supply lanes

The shipped benchmark matrix compares **supply lanes** (Fireworks vs Vertex behind
the gateway). Episode 243 makes the first real Gym environment a **client surface**:
run the *same coding task through OpenCode* against each model endpoint (Khala,
BigPickle, a free Gemini, a paid frontier model) and score the *whole coding-agent
experience* — including whether the model actually drives tools to a verified
result. This is a genuinely new axis for the Gym and the right first environment,
because coding is the wedge and OpenCode is the cleanest "point your tool at us"
landing (config-driven, OpenAI-compatible, AI-SDK provider).

The OpenCode-via-Khala planning memo
([`../opencode/khala-head-to-head-gym-final-output.md`](../opencode/khala-head-to-head-gym-final-output.md))
already sketched the implementation: add competitor endpoints as new typed
`BenchmarkLane` values (`bigpickle`, `gemini-free`, `openai-gpt`, `claude`), add an
**OpenCode client runner** that provisions `opencode.json`, runs a task, and
extracts tokens/wall-clock/tool-call-completion/verification from the output, and
reuse the existing report schema + public-safety tripwire. The runbook for pointing
OpenCode at Khala is
[`../inference/2026-06-25-opencode-khala-runbook-and-audit.md`](../inference/2026-06-25-opencode-khala-runbook-and-audit.md);
the broader OpenCode lane material is in [`../opencode/`](../opencode/).

**Tool-calling compatibility — the blocker is fixed.** The first production blocker
was that Khala's OpenAI-compatible route rejected OpenCode's message content arrays
(`type: "parts"`) and dropped tool-call deltas on the way out (`finish_reason:
tool_calls` with no tool-calls array). That was the real reason OpenCode runs failed
under multi-step planning. It is now fixed across the **Hydralisk and Fireworks**
adapters (request mapping, non-streaming responses, and SSE deltas all preserve
OpenAI-style tool calls), and ten concurrent OpenCode→Khala sessions ran. A Gym
OpenCode environment must keep a **tool-call-completion rate** as a first-class
metric — it is the single highest-risk dimension for any new lane, and every adapter
that speaks OpenAI-ish chat must preserve the field once or it regresses lane by lane.

## 4. The expanded lane set the Gym fans across — and validates before promotion

Khala now routes across a wider mix than the spec's original lane list. As of
Episode 243 the live/near-live set includes:

- **Fireworks DeepSeek V4 Flash** — the actual primary backing lane today (see
  [`../inference/2026-06-24-khala-deepseek-v4-flash-provider-backing.md`](../inference/2026-06-24-khala-deepseek-v4-flash-provider-backing.md)).
- **Gemini Flash 3.5** (Google, on free credits) — overflow/secondary.
- **OpenRouter free model** — part of the current free ensemble.
- **GPT-OSS 20B / 120B on our own Google Cloud infra (Hydralisk)** — the
  Python/NVIDIA inference engine; the `/gym/oss` playground already exercises the
  20B lane.
- **GLM 5.2 (Z.ai) (REAP) on RTX Pro** — newest entrant; the model is from the lab
  **Z.ai** (z.ai), served on our own RTX Pro infra; *"add it to the mix that
  goes into Khala… exercise it in the gym."*

The Gym is explicitly the place where a **new or tuned lane gets exercised before it
joins the Khala mix**. Episode 243's GLM-REAP work is the canonical example: a
serving-shape tune (two independent 4× replicas beat 8× tensor-parallel) plus a
speculative-decoding win (MTP2 profile with min-P omitted, since vLLM rejects min-P
under speculative decoding) moved it from ~35 → ~48 tok/s. That is exactly the kind
of lane-level result the Gym should measure on the *outcome* axis, not just raw
tok/s — "faster" is meaningless until you say faster at *what*, on *which lane*,
under *which traffic*, judged on *which outcome*. Concretely:

- Add `gpt-oss-20b` / `gpt-oss-120b` (Hydralisk) and `glm-52` as typed
  lanes in `LANE_AVAILABILITY`, labeled honestly (live vs future), never fabricated
  zeroes.
- Reuse the speculation-acceptance fields already in the report schema to record
  spec-decoding wins (MTP2 acceptance rate) per lane.

## 5. Cost-per-accepted-outcome now has a real cost basis

The Gym's headline metric (cost-per-accepted-outcome) needs a real per-lane cost
basis. As of 2026-06-25 we have it: the served-tokens recorder now **prices each
completion against the served model's real lane and writes `cost_amount`** to the
ledger, and there is an owner-gated `GET /api/admin/inference-analytics` that breaks
tokens + cost down by provider/model/route/day. The cost model
([`../inference/2026-06-25-khala-cost-model-and-analytics.md`](../inference/2026-06-25-khala-cost-model-and-analytics.md))
puts the real lane (Fireworks DeepSeek V4 Flash) at ~$0.14 in / $0.28 out per Mtok,
~$0.24/Mtok blended at the observed 2.5:1 output:input mix. Gym quotes and
cost-per-accepted-outcome should consume that real basis (and the analytics endpoint
for historicals), not estimates — with the honesty rule intact: a free-tier
competitor call is `costBasisMsat: 0` but labeled `provenance: 'free_tier'` (zero
*direct* spend, not free of quota/rate-limit cost), and a zero-accepted-outcome group
is a `null` cost-per-outcome finding, never a fake-cheap result.

The free tier was also raised to **2.5M tokens/day, 2,000 req/day** (env-tunable),
which sets the headroom for free self-serve fixture Gym runs vs the metered/owner-armed
real runs.

## 6. The tightest flywheel: the Gym both trains Khala and runs on Khala

Episode 243 names the Gym as a **dog-food lane**, not just a quality lab: *"The gym
trains Khala and uses Khala… training that consumes the product it improves is the
tightest possible flywheel."* Two directions, kept honest:

- **Trains Khala** (already the spec's Phase 3): Gym reports are the eval+reward
  artifacts (executed verification verdict + cost-per-accepted-outcome) that feed
  GEPA candidate feedback + TRINITY/Conductor training in Psionic; winners return as
  shadow candidates and re-enter the Gym head-to-head; promotion is approval-gated.
- **Runs on Khala** (new dog-food lane to build): the Gym's *own* agent and eval
  inference — the OpenCode client runner, any judging/verification model calls,
  scene narration — should default to Khala where model fit allows, so every Gym run
  also moves the tokens-served-per-day counter (the North Star). **Direction, not a
  live claim:** the gym↔Khala wiring is not a single shipped seam today; treat it as
  the next dog-food lane to build, alongside QA-runner→Khala and Autopilot/Raynor→Khala.

This ties directly to the "improves, does not depreciate" claim from Episode 242:
the Gym is where that improvement is measured and fed back.

## 7. Build the Gym using the Khala coding agent (dog-food the build itself)

The owner wants the Gym **built with OpenCode→Khala** ("build the gym using the
Khala coding agent"). The Phase-1 build (competitor lanes + OpenCode client runner +
first environments) is itself good dog-food traffic: route the implementing coding
sessions through Khala/OpenCode, which (a) exercises the exact tool-calling/edit/run
workload, (b) adds real served tokens, and (c) surfaces compatibility bugs early —
exactly how the tool-call-array blocker was found and fixed. Keep the honesty split:
internal dog-food tokens are real served tokens, but analytics must distinguish
internal vs external demand so we never imply external traction we do not have.

## 8. Throughput & concurrency belong in the Gym too

Episode 243 stress-tested raw serving throughput (smoke tests reached ~9,500
tokens/sec; ten concurrent OpenCode→Khala sessions ran). The owner-gated `/gym/oss`
playground already measures TTFT/TPS/ITL/wall-clock with a 1→2→4→8 concurrency ramp
and a hard in-flight cap. Direction: generalize a **throughput/concurrency
environment** so "how many tokens/sec, and at what concurrency before quota/latency
degrades" is a first-class, repeatable Gym measurement (not just an ad-hoc smoke),
reusing `/gym/oss`'s telemetry-reconciliation (`not_measured` ≠ `0`) discipline.
This also gives the cost model real concurrency/latency curves per lane.

## 9. Concrete next steps (Phase 1, keyed to existing seams)

Mostly as the OpenCode-via-Khala memo sketched, now de-TBD'd:

1. **Shipped D1 (#6246): competitor lanes** are typed `BenchmarkLane` values with
   honest `LANE_AVAILABILITY` entries: `khala`, `bigpickle`, `gemini-free`,
   `openai-gpt`, `claude`; plus the own/open lanes `gpt-oss-20b`/`gpt-oss-120b`
   (Hydralisk) and `glm-52`. `fixture_only` keeps BigPickle in no-spend fixture
   reports without pretending a real/billable executor exists.
2. **Shipped D1 (#6246): OpenCode client runner** provisions public-safe
   `opencode.json` for a model endpoint, extracts tokens from provider `usage`
   (never estimated), records wall-clock, **tool-call success**, and the
   independent verification verdict, then feeds the existing
   matrix→runner→report path.
3. **First realistic fixtures** — sourced from QA-runner / internal dog-food traffic
   (Pillar 1) so a head-to-head can reach `decisionGrade: true`; keep synthetic
   taste fixtures (the crossy-road artifact prompt, the quickstart smoke) labeled
   illustrative.
4. **Real cost basis** — wire cost-per-accepted-outcome to the now-stored per-lane
   `cost_amount` / the admin analytics endpoint and the cost-model doc.
5. **Owner-armed sweep** — **D3 landed the paid-run planning path:** quote, 402
   balance gate, `preflightRealBenchmarkSweep` (budget cap, billable cap,
   real-traffic evidence, approval ref), explicitly covered real executors for
   otherwise fixture-only competitor lanes, `MeteringHook` contexts, and a
   public-safe report receipt. The first live decision-grade
   Khala-vs-BigPickle report on the OpenCode surface remains owner-armed.
6. **Gym↔Khala dog-food wiring** — **D4 landed the typed flywheel contract:**
   Gym reports become GEPA/TRINITY/Conductor reward bundles, Gym runner/eval
   Khala calls carry internal `openagents-gym` served-token attribution, winners
   return as shadow candidates for head-to-head re-entry, and
   `runtime_promotion` is marked ready only with an explicit approval ref.

## 10. Honest-scope boundaries (unchanged)

- Direction vs shipped: the Phase 0 fixture Gym (#6163–#6166), owner-gated
  `/gym/oss` playground (#6167), Phase 1 OpenCode/environment registry work
  (D1/D2), Phase 2 paid-run planning gate (D3), and Phase 3 typed reward/
  dogfood/promotion flywheel contract (D4), and Phase 4 public-safe leaderboard
  plus owner-armed author-split projection (D5) are landed. Live published
  decision-grade benchmark numbers still require an owner-armed real run over
  realistic traffic.
- No published numbers without an owner-armed real seam over realistic traffic;
  fixture/synthetic runs are `decisionGrade: false` + carry the `illustrativeNotice`.
- No fabricated numbers: `not_measured` ≠ `0`; an unavailable lane is an honest
  skipped run; a zero-accepted-outcome group is a `null` cost-per-outcome finding.
- Typed/semantic selection only — env/lane/tool/plugin/coordinator are typed enums or
  semantic signature lookups, never string/keyword routing.
- Real spend is owner-gated and balance-gated; promotion of a Gym-winning coordinator
  is an approval-gated `runtime_promotion`. Schema reuse, never a parallel metric
  vocabulary.
- The product-promise registry governs public claims; this doc widens nothing.

## 11. Open questions

- First-surface choice for the first armed sweep: OpenCode tool-calling coding tasks
  (whole-agent experience) vs direct chat-completion quality (rubric/verifier)?
- Budget cap (msat) for the first Khala-vs-BigPickle armed sweep.
- Minimal typed `GymEnvironment` contract for client-surface, task-set,
  artifact-acceptance, retrieval-QA, and recorded-head-to-head environments is
  landed in `GYM_ENVIRONMENT_REGISTRY`; real dispatch adapters still differ by
  surface and remain owner-armed work.
- How the throughput/concurrency environment (§8) relates to `/gym/oss` (promote it
  from an owner playground to a typed Gym environment, or keep it separate?).
- Internal-vs-external demand tagging in the analytics so Gym/dog-food tokens never
  imply external traction.

## 12. Cross-ref: QA-runner verified traces are the realistic-traffic + verifier source the Gym needs (added 2026-06-25)

> Marker: this section was added with the QA audit
> ([`../qa/2026-06-25-qa-agent-khala-dogfood-and-qa-on-every-push.md`](../qa/2026-06-25-qa-agent-khala-dogfood-and-qa-on-every-push.md)).
> Direction, not a live claim.

The Phase-1 "first realistic fixtures" step (§9.3) already names QA-runner /
internal dog-food traffic as the source that lets a head-to-head reach
`decisionGrade: true`. Worth stating plainly: **`qa-runner` is the closest thing we
have to the realistic-traffic + acceptance-verdict source the Gym needs.** It does
real browser work, emits an **honest executed pass/fail + a CONFIRMED/REFUTED/
INCONCLUSIVE verify verdict** (#6192) and a **committed e2e test**, and publishes a
redacted shareable `/trace/{uuid}` with the video inline — i.e. exactly the
"executed verification verdict + cost-per-accepted-outcome over realistic traffic"
the Gym refuses to call decision-grade without. Routing `qa-runner → Khala`
(dogfood lane #1; see the QA audit) therefore feeds the Gym's two needs at once:
realistic traffic and an independent verifier. As a next step, a **QA-runner Gym
environment** (a typed `GymEnvironment` whose tasks are QA scenarios and whose
reward is the real-browser verify verdict) could seed the Gym's `verified-rate` and
cost-per-accepted-outcome axes from live QA runs rather than synthetic fixtures —
consistent with §6's flywheel (the Gym both trains Khala and runs on Khala).
