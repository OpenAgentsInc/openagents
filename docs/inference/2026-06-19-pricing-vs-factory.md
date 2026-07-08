# OpenAgents inference pricing vs Factory — comparison + the model we want

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-19. Status: **initial thinking doc.** Companion to
`2026-06-19-inference-gateway-business.md`. Purpose: study Factory's pricing (which we
like the *mechanics* of), contrast it with the single, subscription-free, usage-based,
credits-or-Bitcoin model we actually want, and try to establish where Factory's **margin**
lives so we can deliberately build ours in.

## 1. What Factory does (as published)

**Plans (subscriptions) + rolling rate limits:**
- Pro $20/mo, Plus $100/mo (~5× Pro usage), Max $200/mo (~10× Pro usage). Teams/Enterprise = contact sales.
- "Standard Usage" is rate-limited across **three rolling windows simultaneously** (5-hour, 7-day, 30-day) — you need headroom in all three to send a request.
- **Droid Core** — a pool of cheap open-weight models with their *own* separate rate limits; "free" to keep working after Standard Usage is exhausted.
- **Extra Usage** — prepaid USD credits ($10 min, never expire) that kick in after Standard Usage runs out.
- Consumption order: Standard → (Droid Core rate limits for Core models) → Extra Usage.

**The charging unit — per-model multipliers** (the part we want to adopt). A request
consumes `credits = base_work_unit × model_multiplier × (reasoning effort)`. Examples
from Factory's model list:

| Tier | Examples (multiplier) |
| --- | --- |
| Premium frontier | Opus 4.8 **2×**, GPT-5.5 **2×**, Opus 4.8 *Fast* **4×**, Fable 5 **4×**, "Pro/Fast" variants **5×–12×** |
| Mid | Sonnet 4.6 **1.2×**, GPT-5.4 **1×**, Gemini 3.1 Pro **0.8×**, GPT-5.3-Codex **0.7×** |
| Cheap / open (Droid Core) | Haiku 4.5 **0.4×**, GPT-5.4-mini **0.3×**, DeepSeek V4 Pro **0.7×**, Kimi **0.25–0.4×**, MiniMax **0.12×**, Gemini 3 Flash **0.2×** |

## 2. Where Factory's margin lives (the key finding)

**The multipliers do NOT linearly track raw provider token cost — they compress the
high end and fatten the low end.** That's the tell.

- In Factory, Opus is **2×** and Sonnet is **1.2×** → Opus/Sonnet ≈ **1.67×**.
- Raw Anthropic list pricing has Opus roughly **~5× Sonnet** per token (Opus is far
  pricier than Sonnet; Sonnet far pricier than Haiku).
- So Factory charges *relatively less* for Opus than raw cost implies and *relatively
  more* for cheap models. Conclusion: **the multiplier is a normalized "work unit," not
  a token passthrough, and the margin is concentrated in the cheap/open models.**

Implications:
1. **Frontier models (Opus/GPT-5.5 Pro) are near-cost or loss-leaders** — the 2× headline
   keeps them attractive; Factory makes little/negative margin there.
2. **Cheap + open models (Droid Core 0.12×–0.7×, Haiku 0.4×) carry the fat margin** — they
   cost Factory almost nothing (open weights they may even self-host) yet still bill
   0.12–0.4 credits. Making Droid Core "free / own rate limit" *steers usage there*, which
   is exactly where the margin is highest.
3. **The subscription is the real margin engine.** Plans are flat $20/$100/$200 with rolling
   rate limits and **no rollover** of Standard Usage. Unused capacity is pure margin; the
   rate limits cap downside; Extra Usage (prepaid, non-expiring) is upside. The multiplier
   system mostly governs *how fast you burn the bucket*, not the headline price.

**What we can't pin without more data:** the absolute margin needs (a) Factory's
credit→USD rate (how many dollars per base work unit in Extra Usage), and (b) their real
per-model token cost. Those two + the multiplier give exact margin. We should derive
*our* numbers from our own Vertex billing (§4), not reverse-engineer theirs.

## 3. What we want (deliberately different)

**No subscriptions. No tiers. No three-window rate-limit puzzle. One pricing model:**
- **Pure usage-based credits.** You hold a balance; each request decrements it. No monthly
  plan, no "Standard vs Extra," no per-window caps to reason about.
- **Two ways to fund, one of them discounted:**
  - **Credit card → USD credits** (Stripe top-up).
  - **Bitcoin/Lightning → credits at a discount** (pay in sats, get a better rate). The
    BTC discount is both a real cost saving (no card fees/chargebacks, instant settlement)
    and the on-brand incentive that pulls the agent economy onto Bitcoin.
- **Factory-style per-model multipliers as the charging unit** — adopt the mechanic: a base
  credit unit, a published multiplier per model (Opus/Sonnet/Haiku/Gemini/open), effort
  tiers fold in. Simple, legible, and it lets us tune margin per model.
- Credits **never expire** (match Factory's good behavior); spend caps + alerts instead of
  rate-limit tiers.

Why this is better for us: it removes the subscription friction the "32k views → ~0 users"
problem exposed (no one wants a plan to try it), it's the honest usage-based deal, and the
BTC discount differentiates + advances the Bitcoin thesis. The multiplier mechanic gives us
Factory's margin control without the plan machinery.

## 4. How we bake in our margin (proposal)

Set **one base credit price** (USD per base work unit) and a **multiplier table**, such that
for every model: `price_to_customer = base_credit_$ × multiplier ≥ our_marginal_cost ×
(1 + target_margin)`.

- **Our cost is unusually low on the supply we already hold.** We have first-party Vertex
  quota (project `openagentsgemini`): Opus 40M, Sonnet 50M, Haiku 60M tokens/min on Global
  (see the gateway doc). Inference served from that quota costs us our Vertex/committed-use
  rate, not retail — so our achievable margin on Claude is structurally better than a pure
  passthrough reseller's.
- **Mirror Factory's curve but anchor to OUR cost:** keep frontier multipliers attractive
  (Opus ~2×, Sonnet ~1.2×, Haiku ~0.4×) and put healthy margin on the cheap/open tier we
  route to our own network/self-host — same "margin lives in the cheap models" play, but
  the cheap tier is *our agent network* (Pylon nodes), so that margin can fan back to
  contributors (revenue-loop spine).
- **BTC discount** comes off the top (e.g. pay-in-sats = N% lower effective base credit
  price); size N from the real card-fee + treasury benefit so the discount is funded by
  genuine cost savings, not given away from margin.

**To establish exact margin (do this before publishing prices):**
1. Pull our **real per-token cost** from Vertex billing for Opus/Sonnet/Haiku (committed-use
   vs on-demand) + partner-passthrough rates for non-Vertex models.
2. Pick `base_credit_$` and the target margin (e.g. 30–50%).
3. Solve the multiplier table so each model clears cost + margin; sanity-check our headline
   $/Mtok against Factory's implied $/Mtok and against buying direct, so we're cheaper than
   "get your own subscription" for the no-subscription segment but still margined.
4. Set the BTC discount = realized card-fee + settlement savings (so it's free to us).

## 5. Summary table — them vs us

| Dimension | Factory | OpenAgents (intended) |
| --- | --- | --- |
| Structure | Subscription tiers + rolling rate limits + Extra Usage credits | **Single usage-based credits, no tiers** |
| Funding | USD (plan + prepaid credits) | **USD card OR Bitcoin (BTC = discount)** |
| Charging unit | Per-model multiplier on Standard/Extra usage | **Per-model multiplier on a credit balance** (adopt) |
| Where margin lives | Flat plans + cheap-model pool (Droid Core) | **Multiplier-vs-our-cost; cheap tier = our network (margin fans to contributors)** |
| Cheap/fallback tier | Droid Core open models, own rate limit | Our agent network + open models (paid, not "free") |
| Expiry | Standard no-rollover; Extra never expires | **Credits never expire; no rollover concept (it's all one balance)** |
| Rate limiting | 3 rolling windows | Spend caps + alerts, not tiered throttling |

## 6. Open items
- Get real Vertex per-token cost (billing export) to set the base credit price + margin.
- Decide `base_credit_$`, target margin %, and the BTC discount %.
- Decide whether effort tiers (Low/High/Max) carry their own sub-multiplier like Factory.
