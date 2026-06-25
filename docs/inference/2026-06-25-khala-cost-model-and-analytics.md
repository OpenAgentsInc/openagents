# Khala inference cost model, free-tier quota, and provider-lane analytics

Date: 2026-06-25. Issue #6232. Status: **shipped** (cost model + raised quota +
owner-gated analytics). Internal — provider ids and cost are NOT public claim
copy. Companion to `2026-06-19-pricing-model.md` and
`2026-06-19-fireworks-provider.md`.

This doc answers four questions the owner asked: (1) what does Khala inference
cost us and what is the burn rate, (2) what should the free-tier quota be, (3)
which providers/lanes has Khala actually gone to and is that data stored +
queryable, and (4) how do we measure this going forward.

## 1. The real backing provider (measured, not assumed)

`openagents/khala` is the single public model. Its routing plan (Khala-first,
`model-router.ts: KHALA_HYDRALISK_ADAPTER_PLAN` / the Fireworks-DeepSeek backing
variant) is, in order:

1. Hydralisk GPT-OSS-120B (owned vLLM)
2. Hydralisk GPT-OSS-20B (owned vLLM)
3. Vertex Gemini Flash (graceful-degradation overflow)

…with the Fireworks DeepSeek-V4-Flash lane in front when the
`KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH` backing is selected. So "what served"
is a runtime fact, not a config guess. The ledger's `provider` column (= the
served adapter id) is the ground truth.

**Production ledger (`token_usage_events`, queried 2026-06-25):**

| provider | model | rows | input tok | output tok | served | stored cost |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `fireworks` | `accounts/fireworks/models/deepseek-v4-flash` | 560 | 321,065 | 810,787 | 1,131,852 | NULL |
| `google_gemini` | `gemini-2.5-flash` | 1 | 6 | 1 | 7 | NULL |

The ~1.13M tokens served so far were served by **Fireworks DeepSeek V4 Flash** —
NOT GPT-OSS-120B and NOT Gemini Flash. The owner's "Flash"/Gemini intuition maps
to the *final overflow lane*, which carried ~7 tokens. **DeepSeek V4 Flash is the
real cost basis to reason about today.**

## 2. Cost model — $ per 1M tokens on the real lane

Fireworks DeepSeek V4 Flash, verified cost (`2026-06-19-fireworks-provider.md`):

| dimension | our cost $/Mtok | Khala sell $/Mtok (catalog `oa_price`) |
| --- | ---: | ---: |
| input | **0.14** | 0.196 |
| cached input | 0.028 | — |
| output | **0.28** | 0.392 |

- The catalog `oa_multiplier 0.03` / sell `$0.196 in / $0.392 out` is the
  **customer price**; our **cost** is the Fireworks rate above. Sell ÷ cost =
  **1.4×** → exactly the 40% margin band. (The catalog row for `openagents/khala`
  is modeled on the GPT-OSS-120B cost basis, $0.15/$0.60; the *actually served*
  lane, DeepSeek V4 Flash at $0.14/$0.28, is even cheaper, so margin is intact.)
- **Blended cost at the observed mix** (28% input / 72% output):
  **~$0.24 / Mtok**. Output dominates because Khala traffic runs ~2.5 output : 1
  input.

### Burn rate so far

The ~1.13M tokens served to date (321,065 in + 810,787 out) cost us:

- COST = 0.321065 × $0.14 + 0.810787 × $0.28 = **$0.272**
- (If those had been paid at the sell price: $0.381 charged → $0.109 margin,
  ~29%.)

**Total real inference burn to date: ~$0.27.** This is the whole live-load-test
spend. Khala inference is, at current volume, financially negligible.

### Projection table (cost/day per fully-maxed free user)

Using blended cost ~$0.24/Mtok (realistic) and the all-output worst case
($0.28/Mtok):

| daily quota | realistic $/user/day | worst-case $/user/day |
| ---: | ---: | ---: |
| 200,000 (old) | $0.048 | $0.056 |
| 1,000,000 | $0.240 | $0.280 |
| **2,500,000 (chosen)** | **$0.601** | **$0.700** |
| 5,000,000 | $1.201 | $1.400 |

### Projection table (chosen 2.5M quota × N daily free users)

| daily free users | all maxing quota (realistic) | @20% avg utilization |
| ---: | ---: | ---: |
| 10 | $6.01/day (~$180/mo) | $1.20/day (~$36/mo) |
| 100 | $60.07/day (~$1.8k/mo) | $12.01/day (~$360/mo) |
| 1000 | $600.72/day (~$18k/mo) | $120.14/day (~$3.6k/mo) |

"All maxing the full 2.5M every day" is a deliberate worst case; a real "try it"
session uses a fraction. Even so, 100 fully-maxed free users is ~$60/day — a
bounded, known number the owner can watch via the analytics endpoint (§4).

## 3. The raised free-tier quota (decision + rationale)

Old: `FREE_TIER_MAX_REQUESTS_PER_DAY = 200`, `FREE_TIER_MAX_TOKENS_PER_DAY =
200_000` ("rookie numbers").

**New (shipped):**

- `FREE_TIER_MAX_TOKENS_PER_DAY = 2_500_000` (12.5×)
- `FREE_TIER_MAX_REQUESTS_PER_DAY = 2_000` (10×)

Rationale: 2.5M tokens/day is a genuinely generous "try it" session (dozens of
real coding turns or a long agent loop) while costing at most ~$0.60/user/day on
the real lane — fully bounded by the cost model. The request cap rises in
proportion so the token cap, not the request cap, is the binding limit. Premium
models are still never free; over-quota still falls through to the normal balance
/ 402 path; minting is still per-IP-hash rate-limited.

**Env-overridable without a deploy.** `resolveFreeTierQuota(env)` reads
`FREE_TIER_MAX_TOKENS_PER_DAY` and `FREE_TIER_MAX_REQUESTS_PER_DAY` (positive-int
strings; bad/absent values fall back to the compiled defaults) and threads the
result through the balance-gate bypass, the zero-debit metering wrapper, the
`POST /api/keys/free` mint response, and the public `/v1/models` catalog
projection — so all four stay in lockstep. To tune live:

```sh
# from apps/openagents.com/workers/api
npx wrangler secret put FREE_TIER_MAX_TOKENS_PER_DAY    # e.g. 5000000
npx wrangler secret put FREE_TIER_MAX_REQUESTS_PER_DAY  # e.g. 4000
```

(or set as `vars` in `wrangler.jsonc` for a tracked default).

## 4. Provider / lane analytics over the token ledger

### Is the data stored + queryable today?

`token_usage_events` (migration 0137) already stores `provider`, `model`,
`producer_system`, `source_route`, `input_tokens`, `output_tokens`,
`total_tokens`, `observed_at`, and — importantly — `cost_amount` / `currency`
columns, with indexes on `(provider, model, observed_at)` and
`(producer_system, source_route, observed_at)`. So **"which providers + how many
tokens + when" was fully answerable** from day one.

**The one gap: cost was never recorded.** Every one of the 561 rows had
`cost_amount = NULL` — the served-tokens recorder wrote tokens but not cost. So
"what did it cost" could only be *derived*, not read.

### What I added to storage

`served-tokens-recorder.ts` now prices each served completion against the
**served model on its real lane** (via `priceRequest`, cost-only/before-margin)
and writes `cost_amount` (USD) + `currency` on every new ledger row. From now on
the ledger answers "what did it cost" directly. Historical NULL-cost rows are
reported honestly via a `costCoverage` ratio (see below) rather than silently
counted as $0.

### The analytics read

`TokenUsageLedger.readInferenceAnalytics({ window })` (aggregate-only, no
per-user/prompt material) returns token + cost rollups grouped by **provider**,
**model**, **source-route/producer-system**, and **day**, plus window-wide
totals. Windows: `today | 7d | 30d | all`.

### The endpoint (owner-gated)

`GET /api/admin/inference-analytics?window=7d` — **admin/owner browser session
only** (`requireAdminSession`; `401` anonymous, `403` non-admin). It is internal
cost/provider data, deliberately NOT on a public route and NOT in the public
OpenAPI surface (allowlisted as intentionally-undocumented, like
`/api/stats/token-usage/aggregate`).

Sample response shape:

```json
{
  "schemaVersion": "openagents.inference_analytics.v1",
  "window": "7d",
  "generatedAt": "2026-06-25T...Z",
  "byProvider": [
    { "key": "fireworks", "label": "fireworks",
      "inputTokens": 321065, "outputTokens": 810787, "totalTokens": 1131852,
      "usageEvents": 560, "costUsd": 0.272 }
  ],
  "byModel":  [ { "key": "accounts/fireworks/models/deepseek-v4-flash", ... } ],
  "byRoute":  [ { "key": "omega:omega_hosted_gemini", ... } ],
  "byDay":    [ { "day": "2026-06-25", "totalTokens": 1131852, "costUsd": 0.272, ... } ],
  "totals": {
    "inputTokens": 321065, "outputTokens": 810787, "totalTokens": 1131852,
    "usageEvents": 560, "costUsd": 0.272, "costCoverage": 0.0
  }
}
```

`costCoverage` is the fraction of rows in the window that carry a stored
`cost_amount`. Before this change it is ~0 (all historical rows are NULL-cost);
as new rows land it climbs toward 1. When `< 1`, `costUsd` understates true cost
and you should fall back to the per-Mtok rates in §2 for the uncovered rows.

## 5. How to measure this going forward

- **Live cost/provider read (owner):** sign in as an OpenAgents admin in the
  browser, then:
  ```sh
  curl -s 'https://openagents.com/api/admin/inference-analytics?window=7d' \
    -H "Cookie: <your authenticated openagents.com session cookie>" | jq .
  ```
  Use `window=today` for the day, `30d`/`all` for trend. Watch `byProvider`
  (which lane is carrying load + its cost), `byDay.costUsd` (daily burn), and
  `totals.costCoverage` (data completeness).
- **Public served-tokens counter** (`/api/public/khala-tokens-served` +
  `/history`) remains the public-safe token total; the analytics endpoint is the
  internal cost/provider companion.
- **Re-confirm the cost basis** whenever the served lane changes: re-run the
  prod ledger group-by from §1; if a new `provider`/`model` appears, add/confirm
  its row in `pricing.ts` so `cost_amount` keeps pricing the real lane.
- **Watch the quota's blast radius:** `totals.costUsd` at the new quota tells you
  the real burn; if free-user volume grows, the §3 projection table sets the
  expectation and the env overrides let the owner dial the quota up or down
  without a deploy.

## 6. Files

- Cost basis / pricing: `workers/api/src/inference/pricing.ts`
- Cost recorded per row: `workers/api/src/inference/served-tokens-recorder.ts`
- Quota constants + env override: `workers/api/src/inference/inference-free-tier-key.ts`
- Catalog quota projection: `workers/api/src/inference/model-catalog.ts`,
  `workers/api/src/inference/models-routes.ts`
- Analytics read: `workers/api/src/token-usage-ledger.ts`
  (`readInferenceAnalytics`), schema in
  `packages/sync-schema/src/token-usage-ledger.ts` (`InferenceAnalytics*`)
- Endpoint: `workers/api/src/token-usage-ledger-routes.ts`
  (`handleInferenceAnalyticsApi`), wired in `index.ts` at
  `/api/admin/inference-analytics`
- Tests: `inference-analytics.test.ts`, `token-usage-ledger-routes.test.ts`,
  `inference/served-tokens-recorder.test.ts`, `inference/inference-free-tier-key.test.ts`
