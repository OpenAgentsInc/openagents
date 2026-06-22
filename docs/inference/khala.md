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
    "settled": false
  }
}
```

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

Keyed to the existing gateway EPIC where possible.

- **Phase 0 — MVP gateway (mostly landed).** `GET /v1/models` + `POST /v1/chat/completions`, auth + balance gate, adapter registry, stub router/metering (#5476). **Khala work:** name `openagents/khala-mini`; flip the stub router to real cheapest-viable (#5482); turn on a first real adapter. *Success:* a real OpenAI SDK call to `khala-mini` returns a metered completion with a receipt.
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

---

> Khala is one model endpoint that is actually an agent network underneath. The
> difference from every other router is that ours is wired into verified work,
> Pylon contributors, and Bitcoin settlement from day one. One endpoint outside.
> Many agents inside. Receipts underneath.

[1]: https://sakana.ai/fugu-beta/ "Sakana Fugu: A Multi-Agent Orchestration System as a Model"
[2]: https://openrouter.ai/docs/quickstart "OpenRouter Quickstart Guide"
[3]: https://openrouter.ai/docs/guides/community/for-providers "OpenRouter Provider Integration"
