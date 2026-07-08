# OpenAgents inference pricing model — credits, multipliers, margin, BTC discount

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-19. Status: **worked pricing model + the math to finalize it.** Companion
to `2026-06-19-inference-gateway-business.md` and `2026-06-19-pricing-vs-factory.md`.
This doc does the exercise: cost basis → base credit price + target margin → multiplier
table → BTC discount.

> **Cost-basis caveat (read first):** the public Cloud Billing Catalog does not expose the
> Vertex *partner-model* (Anthropic) per-token SKUs (scanned 8,240 Vertex AI SKUs, none
> matched Claude token pricing), so the cost numbers below are the **published
> Vertex/Anthropic Claude list rates** — the right *structure*, but our **actual** cost is
> our committed-use / quota rate, which is lower. **Confirm real per-token cost from the
> billing export (BigQuery) before publishing prices.** The math below is built so you drop
> in the real numbers and the table re-solves.

## 1. The charging mechanic (adopted from Factory)

A request costs: **`credits = tokens × model_rate`**, where `model_rate` is published as a
per-model **multiplier** on a base unit. Customer holds one credit balance (USD card or
Bitcoin). No subscriptions, no tiers, no rolling rate-limit windows — just usage.

We express prices two equivalent ways: as **$/Mtok sell price** (what we charge) and as a
**multiplier** relative to a baseline model (Sonnet = 1.0×), so the published table reads
like Factory's.

## 2. Cost basis (published Vertex/Anthropic list — CONFIRM vs billing)

| Model | Input $/Mtok | Output $/Mtok |
| --- | ---: | ---: |
| Opus (4.x) | 15.00 | 75.00 |
| Sonnet (4.x) | 3.00 | 15.00 |
| Haiku (4.5) | 1.00 | 5.00 |
| Fable | *unknown — premium; Factory prices it 4×* | |
| Open models — **Fireworks (real cost, verified 2026-06-19)** | 0.07–1.74 | 0.28–4.40 |

Open-tier cost is now **real**, not a guess — from the Fireworks serverless price list (see
`2026-06-19-fireworks-provider.md`): e.g. **gpt-oss-120b $0.15 in / $0.60 out**, MiniMax/DeepSeek-Flash
~$0.14–0.30 in, GLM/DeepSeek-Pro/Kimi ~$1.4–1.74 in / $3.5–4.4 out, all per Mtok; batch −50%;
prompt-cache hits ~−50% input. This is our marginal cost for the open lane (our own
Pylon-fabric serving is cheaper still where it runs).

For a single legible multiplier we blend input:output at a **coding-typical 4:1** mix:
`blended = (4·in + 1·out) / 5`.

| Model | Blended cost $/Mtok |
| --- | ---: |
| Opus | 27.0 |
| Sonnet | 5.4 |
| Haiku | 1.8 |

## 3. Apply target margin → sell price

Sell = `cost × (1 + margin)`. Using the **40% midpoint** of the 30–50% target:

| Model | Sell in $/Mtok | Sell out $/Mtok | Blended sell $/Mtok |
| --- | ---: | ---: | ---: |
| Opus | 21.0 | 105.0 | 37.8 |
| Sonnet | 4.2 | 21.0 | 7.56 |
| Haiku | 1.4 | 7.0 | 2.52 |

## 4. Multiplier table (Sonnet = 1.0×)

Two strategies — this is the real decision:

| Model | **Cost-proportional** (our default, guaranteed margin) | **Factory's** (compressed) |
| --- | ---: | ---: |
| Opus | **5.0×** | 2× |
| Sonnet | **1.0×** | 1.2× |
| Haiku | **0.33×** | 0.4× |

**The key finding:** cost-proportional pricing makes Opus **5× Sonnet** (matching real token
cost), but Factory charges Opus only **1.67× Sonnet** — Factory *subsidizes the frontier*
and recovers margin from its flat subscription + the cheap-model pool. **We have no
subscription to backstop a frontier subsidy**, so:

- **Default = cost-proportional multipliers** (Opus ~5×) → every model clears cost+margin on
  its own. Safe.
- **Compress toward Factory only where our real Vertex cost beats list.** Our committed-use /
  quota Opus rate is below the $15/$75 list; to the extent it is, we can lower the Opus
  multiplier (toward 2–3×) and still be margin-positive — undercutting "get your own Opus
  subscription" while staying profitable. **This is exactly what the billing-export cost
  number decides.** Quantify our real Opus cost, then set the Opus multiplier to the lowest
  value that still clears margin.
- **Cheap tier carries reliable margin.** Haiku, and especially our **own-network/open
  models** (cost ~$0.10–0.50/Mtok), can be priced at a healthy multiplier with fat margin —
  and that margin is what fans to serving nodes + referrers.

## 5. Base credit price

Pick a clean unit: **1 credit = $0.01**. Then a model's credits-per-Mtok = `sell $/Mtok ÷
0.01`. E.g. Sonnet output (sell $21/Mtok) = 2,100 credits/Mtok; Haiku input (sell
$1.40/Mtok) = 140 credits/Mtok. (Internally we meter in $; "credits" is the legible UI unit.
Minimum top-up $10 like Factory; credits never expire.)

## 6. Bitcoin discount (funded by real savings, not margin)

Pay-in-Bitcoin gets a better effective rate — and it's **free to us** because it's funded by
costs we *don't* pay on the BTC rail:
- Card processing: **~2.9% + $0.30** per top-up (Stripe).
- Chargeback / fraud reserve on cards: **~1–2%**.
- Faster settlement + treasury benefit of holding sats: qualitative.

So a **~5% Bitcoin discount** (off the effective credit price) is roughly cost-neutral to us
(it's the card fees + fraud reserve we avoid), while being a strong on-brand pull onto the
Bitcoin rail. Size N precisely once real card-fee blended rate is known; keep it ≤ realized
savings so margin is untouched.

## 7. Worked example (end to end, illustrative)

A coding task: 200k input + 50k output tokens on **Sonnet**.
- Cost (list): 0.2·$3 + 0.05·$15 = $0.60 + $0.75 = **$1.35**.
- Sell @40%: **$1.89** (≈ 189 credits). Card pays $1.89; **Bitcoin pays ~$1.80** (5% off).
- If served from a network node, the node earns its revshare cut of the margin; if the
  account was referred, the referrer earns their ongoing cut of **every** such request
  (referral-on-all-inference). OpenAgents keeps the residual margin.

Same task on **Opus**, cost-proportional (5×): cost $0.20·15 + 0.05·75 = $3 + $3.75 = $6.75;
sell @40% ≈ **$9.45**. On Factory's compressed 2×, the customer-facing price would be far
lower relative to Sonnet — only viable for us if our real Opus cost is well under list.

## 8. To finalize (the remaining inputs)
1. **Real per-token cost** from the billing export (committed-use Opus/Sonnet/Haiku + open-model infra cost). This is the one true unknown and it sets how far we compress frontier multipliers.
2. **Target margin** (recommend 40%; can go 30% on frontier to win the no-subscription segment, 50%+ on cheap tier).
3. **Base credit $** (recommend $0.01/credit) and **min top-up** ($10).
4. **BTC discount %** (recommend ~5%, = realized card-fee + fraud savings).
5. **Effort sub-multipliers** (Low/High/Max) if we mirror Factory's reasoning tiers.
6. **Split policy:** of each request's margin, what % to serving node vs referrer vs OpenAgents (feeds RL-1/RL-2 payout).
