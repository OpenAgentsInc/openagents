# Khala — Inference Model Spec & Roadmap

> **Status:** design spec, honest-scope. Khala is the orchestrating-model brand
> on top of the **already-landed** OpenAgents inference gateway skeleton
> (EPIC #5474 / #5476, INERT behind `INFERENCE_GATEWAY_ENABLED`). This doc
> specifies what Khala *is*, its API, its coordinator, its verification and
> economics, and a phased roadmap keyed to the existing gateway seams. It does
> not claim anything is shipped beyond what the gateway README documents.

Khala is a single OpenAI-compatible inference endpoint that **behaves like one
model but is an agent network underneath** — it routes and orchestrates a pool
of models, tools, validators, and (eventually) Pylon workers, settling verified
work in Bitcoin. The name follows our world: the **Khala** is the psionic link
that joins many minds into one (the same role Tassadar and Artanis play).

This is a generic industry pattern — Sakana's Fugu is the clearest public
statement of "a multi-agent system delivered as one model" ([Sakana AI][1]);
OpenRouter and LiteLLM occupy nearby points. Khala's differentiator is what no
closed router has: **verified work, Pylon contributors, Bitcoin settlement, and
Tassadar modules**, wired in from day one.

## 1. Where Khala sits

Khala is **not** a new codebase. It is:

- a set of **virtual model IDs** (`openagents/khala-*`) exposed through the
  existing gateway route `POST /v1/chat/completions` on
  `apps/openagents.com/workers/api`;
- the **`ModelRouter` seam** (cheapest-viable selection, #5482) evolved into the
  **Khala coordinator** (heuristic → learned);
- the existing **`InferenceProviderAdapter` / `InferenceProviderRegistry`**
  (`src/inference/provider-adapter.ts`) as the **worker pool** (Fireworks #5479,
  Vertex Anthropic #5480, partner passthrough #5481, + Pylon shard-WAN serving,
  + Tassadar modules);
- the existing **`MeteringHook`** (#5477) extended to write **receipts** and, for
  verified lanes, trigger **settlement**.

So shipping Khala is mostly: name the virtual models, grow the router into a
coordinator, and add the verification + receipt + settlement metadata. The
request surface, auth, balance gate, and adapter registry already exist.

Related context: the credits business and supply pillars
([`README`](README.md), [`2026-06-19-inference-gateway-business.md`](2026-06-19-inference-gateway-business.md)),
pricing ([`2026-06-19-pricing-model.md`](2026-06-19-pricing-model.md)),
decentralized serving ([`2026-06-19-decentralized-serving-shard-wan.md`](2026-06-19-decentralized-serving-shard-wan.md)),
the coordinator design (`docs/sakana/`), the verification recipe lessons
(`docs/research/tmax/`), and the Verse visualization of inference serving
([`khala-in-the-world.md`](khala-in-the-world.md)).

## 2. Design principles

1. **One endpoint outside, many agents inside.** External callers see one model;
   internally Khala assembles workers per request.
2. **Verified, not trusted.** Correctness is established by independent
   verification (test command / replay / verification-class challenge), never by
   the producing model self-grading. (See §6; the empirical case is TMAX's
   reward-hacking finding — `docs/research/tmax/synthesis.md` §3.)
3. **Receipts underneath, not chain-of-thought.** Expose routing class,
   verification class, cost, and a receipt id — never the internal CoT.
4. **Cost per accepted outcome is the north-star** metric; per-token billing is
   the compatibility default, accepted-outcome pricing is the goal for
   coding/agentic lanes.
5. **The coordinator interface is swappable.** Heuristic v0 must be replaceable
   by a learned coordinator without changing the API or the adapters.
6. **Bitcoin-native economics.** Every call meters provider cost and OpenAgents
   price in msat; verified-work lanes settle to the worker and validator.

## 3. Public API surface

OpenAI-compatible, on the existing Worker routes:

- `GET /v1/models` — lists Khala virtual models.
- `POST /v1/chat/completions` — OpenAI Chat Completions shape (streaming +
  non-streaming). `/v1/messages` (Anthropic) is a left seam.
- **Auth:** per-account API-key / agent bearer token
  (`authenticateProgrammaticAgent`); **balance gate** (`readAgentBalance`),
  `402` on insufficient credits.

### Model tiers

| Model ID | Lane | Behaviour |
|---|---|---|
| `openagents/khala-mini` | cheap default | cheapest-viable router over the pool; good for agents; priced above blended cost |
| `openagents/khala-pro` | stronger | escalates to stronger models + a verifier pass; higher price |
| `openagents/khala-code` | coding | coding-optimized; runs tests / verification commands; returns a receipt |
| `openagents/khala` *(later)* | full | learned coordinator over frontier APIs + Pylons + verified Tassadar modules |
| `openagents/tassadar` *(later)* | executor | Tassadar executor-class modules where exact replay applies |

### Request

```json
{
  "model": "openagents/khala-mini",
  "messages": [{ "role": "user", "content": "Fix this TypeScript error..." }],
  "temperature": 0.2,
  "stream": false
}
```

### Response

A normal OpenAI chat completion plus a **non-breaking `openagents` block**:

```json
{
  "id": "chatcmpl_...",
  "model": "openagents/khala-mini",
  "choices": [...],
  "usage": {...},
  "openagents": {
    "receipt": "oa_receipt_...",
    "route": "coding|cheap|long_context|default",
    "workers": ["gemini-flash", "validator"],
    "verification": "none|seeded|test_passed|exact_trace_replay",
    "cost_msat": 123,
    "price_msat": 170,
    "settled": false,
    "telemetry": {
      "schemaVersion": "openagents.khala.telemetry.v1",
      "requestClass": "interactive_stream",
      "promptTokens": 400,
      "completionTokens": 12,
      "totalTokens": 412,
      "ttftMs": 200,
      "totalWallClockMs": 500,
      "verificationClass": "test_passed",
      "executedVerdict": "passed",
      "scalarReward": 1,
      "detailRef": "/api/public/inference/receipts/receipt.inference.charge.chatcmpl_..."
    }
  }
}
```

### Request-telemetry scorecard (book P0-1 / Open Questions #1–2)

The `openagents` block carries a non-breaking `telemetry` summary — the canonical,
public-safe **Khala request-lifecycle telemetry** the inference-engineering book's
P0-1 asks us to record *before* optimizing serving. The full typed schema is
`apps/openagents.com/workers/api/src/inference/khala-telemetry.ts` (Effect Schema,
`openagents.khala.telemetry.v1`); the book P0-1 cross-reference is
[`2026-06-23-khala-telemetry-scorecard-book-p0-1.md`](2026-06-23-khala-telemetry-scorecard-book-p0-1.md).

**Honest measured-vs-`not_measured` discipline.** Every numeric is either a real
measured number or the explicit sentinel `not_measured`. A field is *never*
fabricated and *never* a misleading `0`. `not_measured` ("no measurement exists")
and a measured `0` are different products — the same discipline as the M8 metric
table (`docs/inference/2026-06-23-khala-head-to-head-m8-status.md`).

**Block-vs-receipt split (Open Question #2 — RESOLVED).** The *immediate* block
stays small: request class, prompt/completion/total tokens, TTFT, total
wall-clock, verification class + executed verdict + scalar reward, and a
`detailRef` pointer. The *full* P0-1 record — the time split (provider / gateway
overhead / verifier / settlement), inter-token latency / perceived TPS, queue and
batch wait, region, the cache-affinity key **hash** (never the raw key), fallback
reason, cached input tokens, cost basis / price / margin **bucket** (never the raw
margin) / settlement state / blocker refs — is the dereferenceable depth behind
`/api/public/inference/receipts/<ref>`. Depth lives off the hot path.

**What is measured NOW vs honestly `not_measured`.**

| field | now | source |
| --- | --- | --- |
| prompt / completion / total tokens | measured | provider `usage` (receipt-first) |
| total wall-clock | measured | gateway edge (request accept → completion) |
| TTFT | measured on the **true-streaming** path | first content delta − request accept |
| inter-token latency / perceived TPS | measured on the **true-streaming** path | derived from completion tokens + generation wall-clock |
| request class | measured | stream → `interactive_stream`; detached batch job → `batch`; else `async_job` (book P0-3, #6086) |
| route / provider / served model | measured | coordinator + adapter |
| verification class / executed verdict / scalar reward | measured | reuse of the existing `khala-code` verifier verdict (no parallel grader) |
| cached input tokens | measured when the provider reports a cached dimension (book P0-2, #6084); else `not_measured` | provider `usage.cachedPromptTokens` (Fireworks `prompt_tokens_details.cached_tokens`, Anthropic `cache_read_input_tokens`, Gemini `cachedContentTokenCount`) → block + record `cachedInputTokens` |
| unaccounted tokens (`total − (prompt + completion)`) | measured when all three counts are measured (book P0-2, #6084) | reconciliation in the record builder; surfaces the real billed reasoning/thinking/tool-use dimension behind the live total-vs-sum gap |
| provider / gateway / verifier / settlement time split | `not_measured` (no split instrumentation yet) | future per-stage timers |
| queue / batch wait | chat path: `not_measured`. **Async batch lane (book P0-3, #6086): measured** — `queueWaitMs` = `0` (a batch job never blocks the edge), `batchWaitMs` = the real enqueue→consumer-start wait (or `not_measured` when timing is unavailable) | batch-job consumer stamps `enqueued_at`/`started_at`; surfaced on the closeout receipt + `GET /v1/inference/batches/:jobId` |
| cache-affinity hash | measured for Khala requests with an account/session/codebase key (book P0-2, #6084); else `null` with a `cache_affinity_key_not_resolved` `blockerRef` | one-way `hashCacheAffinityKey(account/session/codebase)` |
| region / fallback reason | `not_measured` / `null` (not wired on the gateway yet) | future region wiring |
| cost basis / price / margin bucket | `not_measured` on this hot path (the immediate block omits raw economics) | metering hook (receipt-first) feeds the full record |

**Privacy (INVARIANTS).** The cache-affinity key is recorded only as a one-way
FNV-1a hash; no raw account/session/codebase key, prompt, completion,
chain-of-thought, amount, destination, or payment material is ever a telemetry
field — only token counts, durations, neutral classifiers, public refs, and the
coarse margin bucket.

**How it feeds M8 + the coordinator reward.** This closes the M8 "tokens / cost /
verification telemetry are `not_measured`" gap by making the request lifecycle a
typed, dereferenceable artifact: M8 manifests can read measured tokens + latency +
verdict instead of treating them as afterthoughts, and the learned coordinator's
reward inputs (accepted outcome per sat and per second) read from a stable schema
rather than ad-hoc fields.

### Prefix caching as a product feature (book P0-2 / #6084)

The inference-engineering book's §5.3 makes prompt **order** a performance/cost
lever: prefix caching reuses the shared input "from the start of the sequence
until the first non-repeated token", so a single novel token at the front voids
the whole shared prefix. Khala coding traffic (khala-code / Autopilot) repeats
long *stable* content on every call — the Khala identity prompt, the acceptance
contract, tool schemas, stable policy — ahead of a small *volatile* user turn.
P0-2 turns "prompt order is an accident" into a deliberate, tested layout so the
shared prefix stays cacheable and the provider prompt cache (e.g. Fireworks'
on-by-default cache, cached input billed at a discount) actually hits.

The six deliverables, all in `apps/openagents.com/workers/api/src/inference/`:

1. **Stable prompt layout** — `assembleStablePromptLayout`
   (`prompt-prefix-cache.ts`) partitions the outgoing messages into a STABLE
   prefix (acceptance contract → identity → tool schemas → stable policy) followed
   by VOLATILE/user content last. The gateway tags its own injected blocks with a
   `StableBlockKind`, so ordering is deterministic and structural — a
   classification of *our own* injected blocks, not a keyword match on user intent
   (honors the workspace semantic-routing rule). The `khala-identity.ts` injection
   stays in the stable prefix as the primary identity mechanism.
2. **Deterministic ordering + hashing** — tool schemas and the acceptance
   contract are serialized canonically (`canonicalJson` / `serializeToolSchemas`,
   sorted object keys, stable tool order) so the same logical inputs always
   produce byte-identical prefix text and the same `stablePrefixHash`.
3. **Cache-affinity keys** — `deriveCacheAffinityKey({account, session?,
   codebase?})` composes the dimensions into one raw key; it is recorded ONLY as
   the one-way `hashCacheAffinityKey` digest (`cacheAffinityKeyHash` in telemetry/
   receipt). The raw account/session/codebase key never leaves the gateway.
4. **Provider session affinity** — `sessionAffinityParams` sets Fireworks
   `x-session-affinity` and the OpenAI-style `user` to the SAME opaque value, a
   hash of the affinity key, so a session's follow-ups pin to one cache-warm
   replica and the upstream provider sees only a correlation token, never the raw
   identifiers.
5. **Cached input tokens + total reconciliation** — `cachedInputTokens` flows
   from provider usage into the telemetry block + record. The live discrepancy
   where `totalTokens` (679) ≠ prompt (347) + completion (20) is reconciled in the
   telemetry record builder: the provider's total is recorded receipt-first
   (authoritative — never recomputed as prompt+completion, which would under-count
   billed reasoning/thinking/tool-use), and the gap is disclosed honestly as
   `unaccountedTokens` (679 − 367 = 312) rather than dropped or treated as a
   miscount.
6. **Cache-aware routing** — `decideCacheAwareRouting` (`cache-aware-routing.ts`)
   reorders — never widens — the coordinator's viable lane plan so a same-session/
   codebase/account follow-up tries the cache-WARM lane first, via a typed
   `CacheWarmthOracle` keyed by the affinity HASH (no ad-hoc string routing). A
   warm hint is promoted only when the lane is still in the plan AND healthy AND
   allowed by the privacy/region pin policy; otherwise the cheapest-viable plan is
   preserved. The seam is inert by default (no oracle wired → plan unchanged), so
   existing behavior is unaffected until the Worker wires the oracle.

Privacy (INVARIANTS): every cross-boundary projection of the affinity key is the
one-way FNV-1a digest; the raw key, prompt, and volatile content never enter the
stable prefix hash, the telemetry, the receipt, or the provider header.

## 4. Request flow

```text
client
  → openagents.com/v1/chat/completions
  → auth + balance gate (402 if insufficient)
  → normalize OpenAI request
  → Khala coordinator chooses route (worker[s] + optional role/verify)
  → call worker model(s) via provider adapters
  → optional verifier / tests / replay (verification class)
  → aggregate final answer
  → meter cost (receipt-first, from provider usage)
  → write receipt (+ settle verified lanes)
  → return OpenAI-compatible response
```

Components, mapped to existing seams: gateway route (#5476) · auth+balance
(landed) · normalizer · **coordinator = `ModelRouter`** (#5482 → learned) ·
**provider adapters/registry** (#5479/#5480/#5481 + Pylon + Tassadar) · verifier
(verification-class registry) · **meter = `MeteringHook`** (#5477) · receipt +
settlement (revenue-loop spine, EPIC #5457).

### Streaming / async split (book P0-3 / #6086)

Long synchronous inference through the Cloudflare edge is the wrong shape: the
edge gives up on an origin that produces no bytes for ~100s and returns a `524`
(the local 524 postmortem,
[`2026-06-22-long-running-inference-response-strategies.md`](2026-06-22-long-running-inference-response-strategies.md),
independently rediscovered the book's Ch.7 production lesson). The request lane is
therefore chosen by the request *shape*, and the chosen lane is recorded as the
telemetry `requestClass`:

| Request shape | Transport | `requestClass` | Wait telemetry |
|---|---|---|---|
| **Interactive** (a human/agent waits on it) | **Streaming SSE** — `stream:true`, true pass-through (#6035); first byte ~1s, every chunk resets the edge idle-timer so a 3-min generation never 524s | `interactive_stream` | `queueWaitMs` ~`not_measured`/0 (no edge queue); no batch wait |
| **Detached / minutes-long / agentic** | **Async batch job** — submit → `202 {jobId, receiptRef}` → a **Queue consumer** runs it OFF the request path → a dereferenceable **batch closeout receipt** | `batch` | `queueWaitMs` = measured `0` (never blocks the edge); `batchWaitMs` = real enqueue→consumer-start wait (or `not_measured`) |
| **Live multi-subscriber UI** (cockpit, Verse projection) | **Durable Object + hibernatable WebSocket** | (world transport, not the chat gateway) | — |

**Terminal receipt at the end of BOTH lanes.** The interactive stream attaches
the terminal `openagents` disclosure block (built by the same
`khalaReceiptForResult`) as the final `chat.completion.chunk` at stream close
(EOF), settling metering receipt-first from the terminal usage frame. The async
lane attaches the canonical `openagents.khala.telemetry.v1` *record* to the batch
closeout receipt at `/api/public/inference/batch-job-receipts/<ref>`, with
`requestClass: batch` and the batch/queue wait. Both classes are distinguishable,
and both end with an auditable receipt.

**Async lane internals (Khala, #6028 / EPIC #6017).** Submit
(`handleBatchJobsSubmit`) prices + charges up front, persists a `pending`
`inference_batch_jobs` row stamped with `enqueued_at` (the START of the batch
wait), and hands the executable items to the queue producer. The Queue consumer
(`executeBatchJob`, dispatched from the Worker `queue` handler when
`INFERENCE_BATCH_JOBS_ENABLED` is armed) loads the row, stamps `started_at` (the
END of the batch wait), runs each item against the **same provider-adapter
registry** the interactive route uses, meters each through the **same
`MeteringHook`** (per-item idempotency key → no double-charge on redelivery), and
drives the row to `completed`/`failed`. Job state and the batch wait are queryable
at `GET /v1/inference/batches/:jobId`; the closeout receipt becomes
dereferenceable once the job is `completed`. Idempotent on the job id: a
redelivered queue message is a safe no-op.

**Durable-connection scope (deliverable 4).** The chat gateway uses Durable
Objects in exactly one bounded place: the durable-stream proxy (#6056 /
`durable-inference-proxy.ts`) tees a single client's paid token stream into a
per-`requestId` offset log so a *reconnect* can replay the suffix — it is a
single-subscriber resumability aid, not multi-subscriber fan-out, and metering
still fires exactly once on the real upstream EOF (replays are free). DO +
hibernatable-WebSocket **multi-subscriber** transport is reserved for the live
Verse world (`apps/openagents-world` Region DO + `packages/world-client`), where a
live "watch the energy flow" projection genuinely needs many subscribers on one
long-lived generation. Plain request/response inference does **not** spin up a DO
or WebSocket — that would be over-applying the heaviest transport to the lightest
shape.

## 5. The coordinator (the missing middle)

The coordinator is the only genuinely new capability. It evolves on a fixed
interface so the API and adapters never change:

- **v0 — heuristic router** (the current `ModelRouter`, #5482): cheap/simple →
  small/open or Gemini Flash; coding → best coding backend; long context →
  Gemini/Claude; verifier pass → cheap checker or deterministic test; failure →
  fallback to stronger model.
- **v1 — TRINITY-style logit router:** a tiny hidden-state head picks
  `(worker, role)` (Thinker/Worker/Verifier), trained by evolution against the
  verification verdict. (Psionic primitives P1–P5 in
  `docs/sakana/psionic-coordinator-roadmap.md`.)
- **v2 — Conductor-style planner:** a 7B coordinator emits a natural-language
  workflow — subtasks, worker ids, access-list topology — trained by GRPO. Adopt
  TMAX's stability recipe for this lane: **DPPO + FP32 LM head**, filter
  zero-std samples (`docs/research/tmax/synthesis.md` §5).
- **v3 — full Khala:** frontier APIs + open Pylons + verified Tassadar modules +
  Bitcoin-settled work, coordinator selecting the cheapest composition that still
  **Verifies**.

**Training signal:** the coordinator's reward is the **verification verdict**
(proven, per-contribution, live), not a benchmark grader. *Train on the verdict,
monetize on settlement* — see `docs/sakana/coordinator-as-verified-work.md` and
`docs/sakana/tassadar-run-integration.md`. Ship learned coordinators as **shadow
candidates** against the heuristic router and promote only on a clean win
(verified-work-per-sat), via Psionic's promoted/candidate contract.

## 6. Verification & receipts

Khala exposes a **verification class** per response, drawn from the run's
verification-class registry (`docs/sakana/tassadar-run-integration.md`):

| Khala `verification` | Meaning | When |
|---|---|---|
| `none` | unverified single-model answer | cheap chat, `khala-mini` default |
| `seeded` | re-run under `seeded_replication` (fixed seed/temp) | stochastic LLM outcomes |
| `test_passed` | a deterministic test / verification command passed | `khala-code` |
| `exact_trace_replay` | independent device re-executed; digests matched | executor / kernel-parity work |

Why independent verification, not self-grading: TMAX (App. D.6) showed an RL'd
agent will **tamper with its own checker** if it can reach it. Khala's verifier
role binds to the replay/verification machinery on a **distinct** device — the
producer cannot grade itself. Confidence is therefore a **priceable product**:
`none` < `seeded` < `test_passed` < `exact_trace_replay` are different products
at different prices.

Receipts (the `openagents` block) carry the receipt id, route, workers,
verification class, cost/price in msat, and settled state — enough for a stranger
to re-check without exposing CoT.

**Quantization disclosure (book P1-7 / #6090).** A model served at a reduced
precision (FP8 / MXFP8 / NVFP4 / INT4 / …) is **not the same product** as the
unqualified model id. Receipts therefore carry typed quantization metadata —
precision, serving backend/engine + version, the quantized **scope** (which
tensors), and whether the lane passed the quantization eval gate — with honest
`unquantized` / `not_measured` sentinels where unknown (never a fabricated full
precision). A quantized lane may carry an **unqualified** public alias only when
its precision/backend are disclosed in the receipt **and** it has passed an
executed eval comparison vs the original precision (accepted-outcome rate held,
or cost-per-accepted-outcome improved). Otherwise it must use a **qualified**
alias that names the precision (e.g. `openagents/khala-code-fp8`). Policy:
weights-only / FP8 before aggressive KV-cache or attention quantization. See
`docs/inference/2026-06-23-khala-quantization-eval-gate-book-p1-7.md`.

## 7. Metering, pricing, settlement

- **Metering** (`MeteringHook`, #5477): per call record user/agent, model,
  provider, prompt/completion tokens, `cost_msat`, `price_msat`, route, latency,
  status, receipt id — receipt-first from the provider `usage`, never an
  estimate.
- **Pricing:** per-token / per-credit first (cost basis × per-model multiplier
  over real cost; see `2026-06-19-pricing-model.md`), card or **Bitcoin (BTC =
  discount)**. Add **accepted-outcome pricing** for `khala-code`/agentic lanes as
  verification matures.
- **Settlement:** every inference dollar can split three ways — OpenAgents
  margin, **serving node**, **referrer** (referral revshare on all inference they
  sign up), on the revenue-loop spine (EPIC #5457). Verified-work lanes
  additionally settle Bitcoin to the **worker and validator**.

## 8. Distribution

1. **Host it ourselves first** — `openagents.com/v1/chat/completions`; prove
   usage, pricing, uptime, receipts.
2. **OpenAI-compatible** — OpenRouter, Open WebUI, LiteLLM, LangChain,
   Cursor-like tools, and agents work with minimal changes.
3. **OpenRouter, two ways, in order:** (a) use OpenRouter as a *backend
   provider* inside Khala for quick coverage ([OpenRouter][2]); (b) become an
   OpenRouter *provider* so users can select `openagents/khala-mini` inside
   OpenRouter ([OpenRouter][3]) — `khala-mini` → `khala-pro` → `khala` /
   `tassadar`.

## 9. Safety & policy

- **No chain-of-thought exposure**; expose receipts, routing class, verification
  class, and cost.
- **Opt-out pool control** — callers can restrict which workers/providers
  participate (compliance / data-residency / export-control), the same control
  Fugu offers, plus our admission standard: earned/mined Bitcoin welcome,
  shitcoin-spam not.
- **Legible and steerable** — every outcome dereferences to a receipt; learned
  coordinators ship as shadow candidates under the promoted/candidate contract
  before taking live routing authority.

## 10. How Khala connects to the rest of the system

Khala is not a standalone product — it is the **inference surface of OpenAgents
Cloud**, sitting in the product layer over Psionic execution, drawing supply from
Pylon, grading on Tassadar verification, settling on the revenue-loop spine, and
operable by Artanis under bounded authority.

```text
                 Artanis (bounded autonomous operator)
                        │  proposes (approval-gated)
                        ▼
  caller → Khala API ── coordinator ──► worker pool ──► verifier ──► receipt ──► settle
            (product layer)   │            │              │            │          │
                              │   Vertex / Fireworks / passthrough     │     revenue-loop
                              │   + Pylon fabric (Psionic)        Tassadar      spine (#5457)
                              │                                  verification    RL-1/2/3
                         trained in Psionic                        classes
```

### OpenAgents Cloud — Khala is one of eight primitives

Per [`2026-06-19-agent-cloud-revshare-everywhere.md`](2026-06-19-agent-cloud-revshare-everywhere.md),
OpenAgents Cloud is the one-stop "Agent Cloud" spanning **inference, fine-tuning,
training, sandboxes, agentic compute, tasks, data** (and future primitives) on
**one credit balance (USD or Bitcoin), one referral relationship, revshare
everywhere**. Khala is the *inference* primitive — the first lit by the revenue
spine; the others plug into the identical `accepted-outcome → receipt → settle`
machinery as they mature. Every Khala dollar fans three ways: OpenAgents margin +
the serving node + the referrer.

### The revenue-loop spine (EPIC #5457) — five attachment points

Khala attaches to the spine at exactly five points (the gateway README's
`MeteringHook`/balance-gate seams are where these land):

1. **Credit-balance read** — `readAgentBalance(...).availableMsat`; `402` on
   insufficient. Gates the request before any worker call.
2. **Credit-balance decrement (receipt-first)** — `MeteringHook` (#5477) writes a
   `billing_ledger_entries` row from the provider `usage`, never an estimate.
   That metering receipt is the single source of truth for both billing and
   referral.
3. **Referral attribution — RL-1** (`site-referral-payout-ledger.ts`,
   `site_referral_payout_ledger_entries`): the metering receipt becomes the
   `qualifyingEventRef`; a row accrues at the default 5%
   (`SITE_REFERRAL_PAYOUT_PERCENT_BPS`) and runs `eligible → approved →
   dispatched → settled`. **Refer once, earn forever** applies to *all* of a
   referred account's inference — and, cross-category, to everything else they
   ever run on the Cloud.
4. **Serving-node payout — RL-2** (escrow → Bitcoin): when a Pylon served the
   work, its cut is held in `held_msat`, firms up on a **born-verified**
   (exact-parity) receipt, and settles over Spark/reliable-tips.
5. **No-resale authorization — RL-3** (`inference-resale-authorization.ts`):
   Khala *is* the explicitly-**allowed** `api_inference_gateway_resale` case —
   but only with the full reference chain present (provider grant, route policy,
   metering receipt, pricing policy, ToS boundary, dispatch, assignment,
   settlement). Subscription-capacity resale stays unconditionally **forbidden**.
   `validateAssetBoundary` (`asset-bitcoin-boundary.ts`) keeps credit-spend →
   credit-revshare and Bitcoin-service → Bitcoin-revshare from mixing. This is
   the compliance backbone for selling Khala at all.

### Supply: Pylon as a worker in Khala's pool

Khala's pool has three supply lanes: **Vertex quota + partner passthrough + the
Pylon fabric** (`docs/inference/2026-06-19-decentralized-serving-shard-wan.md`).
Pylon is the "open Pylons" of the v3 pool: a contributor node bundling Psionic
(inference/embeddings/training) and a self-custodial Lightning wallet that can
earn the moment it comes online. A Pylon becomes a Khala worker by:

- **registering capability** (NIP-89 refs like
  `capability.public.pylon.nip90.text_inference.v0.3` + heartbeat readiness),
- **accepting dispatch** (whole small models now; large models sharded across N
  Pylons via the shard-WAN pipeline later),
- producing an **exact-parity receipt** (`psionic.serve.*_run_receipt.v1`,
  naming nodes/layer-ranges/token hashes + the greedy-parity verdict) that makes
  its output trustable — **no parity, no pay**,
- and being **paid in Bitcoin** to its admitted Lightning target via RL-2/RL-3.

The new seam is the **fabric supply adapter** (gateway ↔ Psionic:
ask-plan → execute → consume-receipt), behind the existing
`InferenceProviderAdapter` interface.

### Substrate: Psionic

The boundary is strict: **Psionic owns execution + evidence and never holds
money; Khala (product layer) owns pricing, payout, referral, and routing.**
Psionic is also where the learned coordinator is *trained* (primitives P1–P5,
`docs/sakana/psionic-coordinator-roadmap.md`) — so the Khala coordinator is
trained in Psionic and served through Khala. Psionic's non-negotiable payment
gate is **exact-greedy parity**, which is exactly the trust property RL-2 firms
up against.

### Verification: Tassadar

Khala's `verification` field draws from the **Tassadar verification-class
registry**, and the verified worker pool + the run's flywheel (verified work →
better model → lower cost → more demand) are what make composition beat scale
over time. The learned coordinator's terminal reward *is* the Tassadar verdict
(§5; `docs/sakana/tassadar-run-integration.md`).

### Operation: Artanis under bounded authority

Artanis is the autonomous Cloud Mind (`apps/openagents.com/workers/api/src/artanis-*.ts`):
a once-a-minute cron tick with **read-only authority by default**
(`ARTANIS_LOOP_READ_ONLY_AUTHORITY` — eight `no*` booleans), risky action kinds
(`wallet_spend`, `provider_mutation`, `runtime_promotion`, …) behind approval
gates, and bounded treasury spend via standing grants (per-payout / per-day
caps). Three connection modes (mostly **new** work):

- **Run on Khala** — point Artanis's own mind (`artanis-mind.ts`, currently
  Gemini via Cloudflare AI Gateway) at the Khala endpoint.
- **Operate Khala under bounds** — Artanis proposes routing/quota changes as
  approval-gated `artanis-work-routing.ts` records (work class `inference`,
  target the gateway); it never mutates routing directly — `provider_mutation`
  and `runtime_promotion` are gated kinds.
- **Fund verified-work settlement** — within its bounded treasury envelope.

The deep alignment: **"a learned coordinator ships as a shadow candidate, gated
before it takes live routing authority" (§5) is the same governance Artanis
already enforces.** Promoting a learned Khala router is a `runtime_promotion` —
an approval-gated action under the autonomous-loop contract and Psionic's
promoted/candidate contract. The safety story and the operator model are one
system.

## 11. Roadmap

Keyed to the existing gateway EPIC where possible. For the cross-workstream
buildout sequence (M0–M8) that converges serving, verification, supply,
visualization, and Autopilot consumption onto the head-to-head north-star demo,
see [`khala-buildout-roadmap.md`](khala-buildout-roadmap.md).

- **Phase 0 — MVP gateway (M0 / #6008, code landed).** `GET /v1/models` + `POST /v1/chat/completions`, auth + balance gate, cheapest-viable router (#5482), real adapters (Fireworks/Vertex/passthrough), and receipt-first metering already exist. **Khala work landed:** `openagents/khala-mini` is a first-class priced catalog alias backed by the Vertex Gemini lane (#6018), and Khala responses carry the non-breaking `openagents` disclosure block (`requested_model`, `served_model`, `worker`, `lane`, `verification:'none'`). Khala stays paid (not free-tier). *Remaining (owner-gated):* enable `INFERENCE_GATEWAY_ENABLED` + wire provider secrets in prod, then run the live OpenAI-SDK smoke that returns a metered completion + receipt — the final acceptance proof.
- **Phase 1 — real supply + metering.** Fireworks (#5479, live), Vertex Anthropic (#5480), passthrough (#5481); real per-model decrement (#5477); streaming. **Khala work:** `khala-pro` (escalate + verifier pass), `khala-code` (run tests / verification command → `test_passed`). *Success:* `khala-code` returns receipts with a verification class on real coding tasks.
- **Phase 2 — verification classes + settlement.** Wire `seeded_replication` and `exact_trace_replay` into the `verification` field; settle Bitcoin to worker + validator on verified lanes; accepted-outcome pricing for `khala-code`. *Success:* a verified coding outcome settles sats to a contributor with a public receipt.
- **Phase 3 — learned coordinator (TRINITY lane).** Build Psionic primitives P1–P5; train the logit router by sep-CMA-ES on the verification verdict; ship as a **shadow candidate** against the heuristic router; promote on verified-work-per-sat. *Success:* learned router beats heuristic on cost-per-accepted-outcome in shadow.
- **Phase 4 — Conductor lane + Tassadar modules.** GRPO-trained NL planner (DPPO + FP32 head) for `khala`/agentic tasks; add Pylon shard-WAN serving and verified Tassadar modules to the pool; apply as an OpenRouter provider. *Success:* `openagents/khala` available in OpenRouter, composing open + frontier + module workers.
- **Phase 5 — full Khala.** Frontier APIs + open Pylons + verified Tassadar modules + Bitcoin-settled work under one learned coordinator; `openagents/tassadar` exposed where exact replay applies. *Success:* frontier-grade outcomes at composed cost, every contribution verified and paid.

## 12. Build spec (for a coding agent)

```text
Evolve the OpenAI-compatible inference gateway at /v1/chat/completions into Khala.

Requirements:
1. Accept OpenAI Chat Completions shape: model, messages, temperature, max_tokens, stream.
2. Bearer auth via existing OpenAgents agent/user tokens; keep the 402 balance gate.
3. GET /v1/models returns at least openagents/khala-mini (+ khala-pro, khala-code).
4. Provider adapters behind the existing registry:
   - Fireworks / Vertex / passthrough (real)
   - internal mock provider for tests
5. Grow ModelRouter into a swappable Coordinator interface:
   - v0 heuristic: coding→coding backend; cheap/simple→Gemini Flash; unknown→default; fail→fallback once
   - keep the interface stable so a learned coordinator drops in later
6. Return OpenAI-compatible responses exactly enough for standard SDKs.
7. Extend MeteringHook to record an inference_receipt row:
   user_id, agent_id, model, provider, prompt_tokens, completion_tokens,
   cost_msat, price_msat, route, latency, status, verification_class, receipt_id.
8. Add the non-breaking `openagents` response block (receipt, route, workers,
   verification, cost_msat, price_msat, settled).
9. Tests with the OpenAI SDK pointed at baseURL=https://openagents.com/v1.
10. Do NOT implement learned routing yet; keep the coordinator interface swappable.
```

## 13. Open questions

- Real committed-use Vertex per-token cost (the one true pricing unknown — pull
  from the billing export; see `2026-06-19-pricing-model.md`).
- Where the verifier pass for `khala-pro` runs (cheap checker model vs
  deterministic) and its sampling rate vs cost.
- Settlement latency on verified lanes (verdict → sat) and how it surfaces in the
  `settled` field on a streaming response.
- How `khala-code` defines "accepted outcome" for arbitrary repos (verification
  command discovery) — ties to the Tassadar coding-environment work.
- ~~Which quantization modes may share a public model alias?~~ **Resolved (P1-7 /
  #6090):** an unqualified public alias may be served by a quantized lane only
  when its precision/backend are disclosed in the receipt **and** the lane passed
  the quantization eval gate; otherwise the lane uses a qualified alias that names
  the precision. Honestly-unknown precision (`not_measured`) may not share an
  unqualified alias. See §6 + the eval-gate doc.

---

> Khala is one model endpoint that is actually an agent network underneath. The
> difference from every other router is that ours is wired into verified work,
> Pylon contributors, and Bitcoin settlement from day one. One endpoint outside.
> Many agents inside. Receipts underneath.

[1]: https://sakana.ai/fugu-beta/ "Sakana Fugu: A Multi-Agent Orchestration System as a Model"
[2]: https://openrouter.ai/docs/quickstart "OpenRouter Quickstart Guide"
[3]: https://openrouter.ai/docs/guides/community/for-providers "OpenRouter Provider Integration"
