# OpenAgents Inference — gateway, credits business, decentralized serving

This folder holds the thinking on the **OpenAgents inference business**: a single,
subscription-free, usage-based API where people and agents **buy inference credits** (USD
card or Bitcoin) and we serve them from the supply we control — our Google Cloud / Vertex
quota, partner passthrough, and our own decentralized **Pylon serving fabric**. The revenue
model is built in: every inference dollar can split three ways — OpenAgents margin, the
**serving node**, and the **referrer** (ongoing referral revshare on *all* inference by
anyone they sign up). All of it rides the revenue-loop spine (EPIC #5457: RL-1 referral
ledger, RL-2 escrow→Bitcoin payout, RL-3 asset-boundary/no-resale guards).

> Status: initial thinking docs, not specs. Honest-scope throughout. Aggregating hosted
> model inference behind one credits-based API is standard gateway practice (OpenRouter,
> Together, Fireworks, et al.).

## Contents

1. **[`2026-06-19-inference-gateway-business.md`](./2026-06-19-inference-gateway-business.md)**
   — the core thesis. One OpenAI/Anthropic-compatible API + credits; who it's for (Autopilot
   users with no subscription, BYO-tool users who run out, businesses that can't use personal
   subscriptions, agents); the supply we already hold (**verified Vertex quota**: Global Opus
   40M / Sonnet 50M / Haiku 60M tok-min, +US/EU); the product, economics, and **referral-on-
   all-inference**.

2. **[`2026-06-19-pricing-vs-factory.md`](./2026-06-19-pricing-vs-factory.md)**
   — comparison with Factory. Their structure (subscription tiers + rolling rate limits +
   Extra-Usage credits + per-model multipliers), and the key finding: **their multipliers
   compress the frontier** (Opus only 1.67× Sonnet vs ~5× real cost) — frontier is
   near-cost/loss-leader, margin lives in the cheap/open tier + the flat subscription. What we
   want instead: single usage-based credits, card-or-Bitcoin (BTC = discount), adopt the
   multiplier mechanic, drop the subscriptions.

3. **[`2026-06-19-pricing-model.md`](./2026-06-19-pricing-model.md)**
   — the worked pricing math. Cost basis → 40% margin → sell prices → multiplier table
   (cost-proportional default vs Factory-compressed) → base credit price ($0.01) → **~5%
   Bitcoin discount funded by real card-fee savings**. Plus the remaining inputs to finalize
   (the one true unknown is our real committed-use Vertex per-token cost — pull from the
   billing export).

4. **[`2026-06-19-decentralized-serving-shard-wan.md`](./2026-06-19-decentralized-serving-shard-wan.md)**
   — decentralized model serving on the Pylon network: every Pylon loads weights + serves;
   small models whole, large models **sharded across N Pylons via the shard-WAN pipeline**
   (per Psionic's `2026-06-19-shard-wan-pipeline-implementation-roadmap.md`). The third supply
   pillar for the gateway, with per-stage payout (serving nodes earn Bitcoin revshare against
   exact-parity receipts) and the Psionic boundary (Psionic = execution substrate + evidence;
   product layer = pricing/payout/referral/marketplace).

5. **[`2026-06-19-fireworks-provider.md`](./2026-06-19-fireworks-provider.md)**
   — Fireworks AI provider integration (**verified live 2026-06-19**). The passthrough lane
   for open-weight models Vertex lacks (DeepSeek/Kimi/GLM/Qwen/MiniMax/gpt-oss/Nemotron +
   embeddings/vision/image); OpenAI-compatible, key in `.secrets/fireworks.env`. Real
   serverless per-token pricing (our cheap-tier **cost basis**), billing dimensions, prompt
   caching, adaptive rate limits, and serving paths — what the adapter must handle.

6. **[`2026-06-19-agent-cloud-revshare-everywhere.md`](./2026-06-19-agent-cloud-revshare-everywhere.md)**
   — the capstone. Inference is one surface of **OpenAgents Cloud — the Agent Cloud, the
   one-stop shop for every agent need**: inference, fine-tuning, training, sandboxes, agentic
   compute, tasks, data. The same model (one credit balance, USD or Bitcoin, **revshare
   throughout** to the contributor + the referrer **on everything, forever**) generalizes
   across all of it, on the accepted-outcome → receipt → settle spine. Ties to root
   `docs/cloud/` + the "Let's Make Money" thesis.

7. **[`2026-06-19-leyten-compute-shard-audit.md`](./2026-06-19-leyten-compute-shard-audit.md)**
   — audit of leyten's `shard` (WAN-pipeline inference engine) + `c0mpute` (the two-sided
   compute marketplace that runs it), harvested for ideas/code to bring into OpenAgents. Two
   distinct repos with a one-way `c0mpute → shard` dependency — the **same engine/marketplace
   split we already draw** (Psionic execution vs product/Pylon surfaces). Top harvest items map
   onto the shard-WAN serving lane: Held-Karp topology solver, sealed activation-frame
   discipline, spec-decode acceptance + lazy-KV-crop + async pipelining, verifiable run
   receipts, and canary anti-cheat + graded reputation. **No Solana** — every settlement edge
   is mapped to **Bitcoin/Lightning + our exact-execution/replay verification + dereferenceable
   receipts**.

8. **[`khala.md`](./khala.md)** — **Khala** spec & roadmap: the
   orchestrating-model brand on top of this gateway (one OpenAI-compatible
   endpoint that behaves like one model, an agent network underneath). API
   surface, the coordinator ladder (heuristic → TRINITY → Conductor → full),
   verification classes + receipts, metering/pricing/settlement, distribution,
   and a phased roadmap keyed to the gateway EPIC. Ties to `docs/sakana/`
   (coordinator) and `docs/research/tmax/` (verification/stability lessons).

## The shape, in one line

Aggregate cheap, reliable inference supply (our Vertex quota + our Pylon fabric + passthrough)
→ sell it as simple usage credits (card or Bitcoin) priced by per-model multipliers over our
real cost → split each dollar to the serving node and the referrer → the buy-side that closes
the revenue loop.

## Implementation status

### Gateway API skeleton (#5476 of EPIC #5474) — landed, INERT by default

The OpenAI-compatible request surface lives on the `openagents.com` Worker
(`apps/openagents.com/workers/api`):

- **Route:** `POST /v1/chat/completions` (streaming + non-streaming, OpenAI Chat
  Completions shape). A clean spot is left for the parallel Anthropic Messages
  surface (`/v1/messages`) — see the `ANTHROPIC SEAM` marker in
  `src/inference/chat-completions-route.ts`.
- **Auth + balance gate:** per-account API-key auth (the OpenAgents agent bearer
  token via `authenticateProgrammaticAgent`) + a **read-only** credit-balance
  gate (`readAgentBalance(...).availableMsat`). Insufficient balance → `402`.
  This phase only *gates* on balance; the real per-model decrement is #5477.
- **Two seams the rest of the EPIC plugs into:**
  - **Provider-adapter interface + registry** (`src/inference/provider-adapter.ts`):
    a typed `InferenceProviderAdapter` and `InferenceProviderRegistry`. Each
    provider child issue registers ONE adapter by id — #5479 Fireworks, #5480
    Vertex Anthropic, #5481 partner passthrough. A stub/echo adapter
    (`src/inference/stub-echo-adapter.ts`) ships so the route works end-to-end.
    Model→adapter routing is the `ModelRouter` seam (cheapest-viable selection
    is #5482); #5476 ships a stub router that always selects the stub adapter.
  - **Metering hook** (`src/inference/metering-hook.ts`): the typed
    `MeteringHook` point where #5477 decrements credits from the provider
    `usage` object (receipt-first, never an estimate). Stubbed as a public-safe
    no-op/log for now.

#### Feature flag

The route is gated behind **`INFERENCE_GATEWAY_ENABLED`** (default **off**). When
unset or not an explicit truthy token (`1`/`true`/`yes`/`on`), the route returns
`404 inference_gateway_disabled`, so landing this code on the live Worker changes
nothing in production until the rest of the EPIC lands. The production
`wrangler.jsonc` sets it to `"false"` explicitly.

#### Provider keys (do NOT wire live keys here)

The deployed Worker reads provider keys as **Worker secrets** (`wrangler secret
put …`), separate from the local `~/work/.secrets/` files (e.g.
`fireworks.env`). No provider key is committed or hardcoded in this repo; the
adapter child issues (#5479/#5480/#5481) wire their secret reads when they land.

## References

- **[`speculative-decoding-article.md`](./speculative-decoding-article.md)** — primer on
  speculative decoding (draft+verify: a small fast model proposes K tokens, the target model
  verifies them in one parallel forward pass): why it works, the verify loop, draft-model
  selection, and the major variants (EAGLE 1/2/3, Medusa, lookahead, draft-attention) plus how
  production engines (SGLang / vLLM / TensorRT-LLM) deploy it. Relevant on two fronts: it's the
  latency-hiding technique the **decentralized shard-WAN serving** lane leans on (see
  `2026-06-19-decentralized-serving-shard-wan.md` — speculative decode hides WAN round-trips),
  and it's a low-batch / single-user latency win for any lane the gateway serves. Background
  reference, not an OpenAgents implementation claim.
