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

## The shape, in one line

Aggregate cheap, reliable inference supply (our Vertex quota + our Pylon fabric + passthrough)
→ sell it as simple usage credits (card or Bitcoin) priced by per-model multipliers over our
real cost → split each dollar to the serving node and the referrer → the buy-side that closes
the revenue loop.
