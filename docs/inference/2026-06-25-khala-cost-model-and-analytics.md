# Khala inference cost model, free-tier quota, and provider-lane analytics

Date: 2026-06-25. Issues #6232, #6266, and #6267. Status: **shipped** (cost
model + raised quota + owner-gated analytics, extended with GLM pool visibility
and owned-GPU hourly amortization).
Internal — provider ids and cost are NOT public claim copy. Companion to
`2026-06-19-pricing-model.md` and `2026-06-19-fireworks-provider.md`.

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

| provider        | model                                         | rows | input tok | output tok |    served | stored cost |
| --------------- | --------------------------------------------- | ---: | --------: | ---------: | --------: | ----------: |
| `fireworks`     | `accounts/fireworks/models/deepseek-v4-flash` |  560 |   321,065 |    810,787 | 1,131,852 |        NULL |
| `google_gemini` | `gemini-2.5-flash`                            |    1 |         6 |          1 |         7 |        NULL |

The ~1.13M tokens served so far were served by **Fireworks DeepSeek V4 Flash** —
NOT GPT-OSS-120B and NOT Gemini Flash. The owner's "Flash"/Gemini intuition maps
to the _final overflow lane_, which carried ~7 tokens. **DeepSeek V4 Flash is the
real cost basis to reason about today.**

## 2. Cost model — $ per 1M tokens on the real lane

Fireworks DeepSeek V4 Flash, verified cost (`2026-06-19-fireworks-provider.md`):

| dimension    | our cost $/Mtok | Khala sell $/Mtok (catalog `oa_price`) |
| ------------ | --------------: | -------------------------------------: |
| input        |        **0.14** |                                  0.196 |
| cached input |           0.028 |                                      — |
| output       |        **0.28** |                                  0.392 |

- The catalog `oa_multiplier 0.03` / sell `$0.196 in / $0.392 out` is the
  **customer price**; our **cost** is the Fireworks rate above. Sell ÷ cost =
  **1.4×** → exactly the 40% margin band. (The catalog row for `openagents/khala`
  is modeled on the GPT-OSS-120B cost basis, $0.15/$0.60; the _actually served_
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

|            daily quota | realistic $/user/day | worst-case $/user/day |
| ---------------------: | -------------------: | --------------------: |
|          200,000 (old) |               $0.048 |                $0.056 |
|              1,000,000 |               $0.240 |                $0.280 |
| **2,500,000 (chosen)** |           **$0.601** |            **$0.700** |
|              5,000,000 |               $1.201 |                $1.400 |

### Projection table (chosen 2.5M quota × N daily free users)

| daily free users | all maxing quota (realistic) |    @20% avg utilization |
| ---------------: | ---------------------------: | ----------------------: |
|               10 |         $6.01/day (~$180/mo) |     $1.20/day (~$36/mo) |
|              100 |       $60.07/day (~$1.8k/mo) |   $12.01/day (~$360/mo) |
|             1000 |       $600.72/day (~$18k/mo) | $120.14/day (~$3.6k/mo) |

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
`(producer_system, source_route, observed_at)`. Migration 0232 adds typed demand
attribution columns: `demand_kind`, `demand_source`, and `demand_client`.

**The one gap: cost was never recorded.** Every one of the 561 rows had
`cost_amount = NULL` — the served-tokens recorder wrote tokens but not cost. So
"what did it cost" could only be _derived_, not read.

### What I added to storage

`served-tokens-recorder.ts` now prices each served completion against the
**served model on its real lane** (via `priceRequest`, cost-only/before-margin)
and writes `cost_amount` (USD) + `currency` on every new ledger row. From now on
the ledger answers "what did it cost" directly. Historical NULL-cost rows are
reported honestly via a `costCoverage` ratio (see below) rather than silently
counted as $0.

For GLM pool traffic, the recorder also writes public-safe routing and
performance metadata into `safe_metadata_json`: supply lane, request class,
selected replica ref/id, replica inflight/max-inflight/queue-depth, warm state,
capacity class (`spot` / `on_demand` / `unknown`), queue wait, batch wait, TTFT,
total wall time, perceived tokens/sec, and bounded fallback/saturation reasons.
It deliberately does **not** write raw prompts, responses, private endpoint
URLs, IPs, bearer tokens, or provider payloads.

The same recorder also promotes request attribution into the typed demand
columns. Internal dogfood callers send `x-openagents-demand-kind: internal` plus
a bounded source/client label such as `openagents-gym` or `qa-runner`. External
callers may send `external`. Missing or partial attribution is deliberately
`unlabeled`, never guessed as external.

### The analytics read

`TokenUsageLedger.readInferenceAnalytics({ window })` (aggregate-only, no
per-user/prompt material) returns token + cost rollups grouped by **provider**,
**supply lane**, **adapter id**, **model**, **source-route/producer-system**,
**GLM replica ref**, **request class**, **demand kind**, **demand source**,
**demand client**, **day**, and **demand client by day**, plus window-wide
totals. Windows: `today | 7d | 30d | all`.

It also returns operational summaries:

- `operational`: busy/429 count, fallback rate, GLM saturation count, queue wait, batch wait,
  TTFT, total wall time, and perceived tokens/sec (`p50` / `p90` / `p99`).
- `glmReplicas`: one row per selected GLM replica ref with token/cost rollups,
  latest public-safe inflight/max-inflight/queue-depth, warm state, capacity
  class, fallback/busy/saturation counts, and per-replica latency/throughput.
- `ownedHourly`: the owned-GPU hourly accounting slot. It maps public-safe GLM
  cost-profile refs to typed `OwnedInferenceCostProfile` rows, assumes each
  selected replica was up for the selected analytics window, and amortizes that
  hourly burn across active serving wall-clock, idle time, served tokens, and
  accepted outcomes when an accepted-outcome count is present. Because host
  lifecycle, storage, and benchmark-reserved burn are not yet first-class
  ledgers, `costCoverage` is `partial` and those gaps appear as blocker refs
  rather than being hidden as $0. Keep-warm and watchdog status come from the
  Worker GLM pool heartbeat when it is enabled.

### GLM heartbeat visibility

The Worker heartbeat is disabled by default and controlled separately from model
serving:

```text
HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_ENABLED=true
HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_CADENCE_MINUTES=4
HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_WARM_COMPLETION_ENABLED=false
HYDRALISK_GLM_52_REAP_504B_BENCHMARK_OWNERSHIP_ACTIVE=false
```

Each enabled tick writes one public-safe ledger row per configured replica.
Rows for replicas marked `benchmarkReserved` or `draining` are skipped without
calling the endpoint. Eligible replicas get `/health` and `/v1/models` checks,
and only send a tiny warm completion when the warm-completion flag is true and
the benchmark ownership flag is false. Owner analytics read
`keepWarmStatus`, `watchdogStatus`, `warmCompletionStatus`, warm state, wall
time, and any exact warm-completion token counts from those rows.

### Owned GLM hourly profiles

These are owner estimates for the current Google Cloud GLM lane, normalized to
730 hours/month. Storage overhead is deliberately `not_measured` until disk and
image retention are metered separately.

| profile                         | machine           | GPUs | spot $/mo | DWS flex $/mo | on-demand $/mo | spot $/h | DWS flex $/h | on-demand $/h |
| ------------------------------- | ----------------- | ---: | --------: | ------------: | -------------: | -------: | -----------: | ------------: |
| GLM-5.2 REAP 504B 4-GPU profile | `g4-standard-192` |    4 |    $2,696 |        $6,570 |        $13,140 |    $3.69 |        $9.00 |        $18.00 |
| GLM-5.2 REAP 504B 8-GPU profile | `g4-standard-384` |    8 |    $5,392 |       $13,140 |        $26,280 |    $7.39 |       $18.00 |        $36.00 |

Plain language: the 8-GPU host roughly doubles the hourly cost of the 4-GPU
host. It may improve capacity, concurrency, KV/cache headroom, and some per-user
latency depending on the serving topology, but it is not automatically "twice as
fast" for one request. The analytics model therefore compares cost scenarios
from the same schema rather than assuming bigger is better.

For one 4-GPU Spot replica that is kept up all day, idle burn is about
`$3.693151 * 24 = $88.64/day` even if nobody sends a request. For a 12-hour
`today` window, the owner endpoint now answers that as `$44.32` of window burn.
If that 12-hour window served 30,000 tokens and produced 2 accepted outcomes,
the derived values are:

- effective cost per served token: `$44.317812 / 30,000 = $0.001477/token`
- effective cost per accepted outcome: `$44.317812 / 2 = $22.158906/outcome`
- active demand burn for a one-hour request window: `$3.693151`
- idle burn for the remaining 11 hours: `$40.624661`

At higher sustained utilization the effective $/token drops quickly. A 4-GPU
Spot host held up for 24 hours costs about `$88.64/day`; that is `$88.64/Mtok`
at 1M tokens/day, `$8.86/Mtok` at 10M tokens/day, and `$0.89/Mtok` at 100M
tokens/day. That is why owner analytics must show both provider-token marginal
cost and owned-GPU idle burn: low utilization makes an otherwise cheap owned lane
look expensive per token.

### Gym benchmark shape from this ledger

Issue #6268 wires the Gym benchmark matrix to compare GLM against Fireworks,
GPT-OSS 120B/20B, and Vertex/Gemini on the same workload cells. The default
suite stays synthetic and illustrative, but the owner-armed observed template
uses this public-safe aggregate ledger mix:

```text
shape: observed-khala-fireworks-current-mix
evidence: evidence.openagents.token_usage_events.fireworks_mix.2026_06_25
observed requests: 560
avg input tokens: 573
avg output tokens: 1448
cacheable prefix: 0 (historical cached-input count was not measured)
request class: interactive_stream
```

That shape is enough to avoid benchmarking only toy prompts, but not enough to
claim GLM should lead Khala. Decision-grade routing advice still requires an
owner-approved real sweep with a budget cap and max billable-sample cap, then the
report scores cost-per-accepted-outcome and verified rate on the same prompts.

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
    {
      "key": "fireworks",
      "label": "fireworks",
      "inputTokens": 321065,
      "outputTokens": 810787,
      "totalTokens": 1131852,
      "usageEvents": 560,
      "costUsd": 0.272,
      "costCoverage": 1
    }
  ],
  "bySupplyLane": [
    { "key": "fireworks", "totalTokens": 1131852, "costCoverage": 1 }
  ],
  "byAdapter": [
    { "key": "fireworks", "totalTokens": 1131852, "costCoverage": 1 }
  ],
  "byModel":  [ { "key": "accounts/fireworks/models/deepseek-v4-flash", ... } ],
  "byRoute":  [ { "key": "omega:omega_hosted_gemini", ... } ],
  "byGlmReplica": [
    {
      "key": "replica.hydralisk.glm_52_reap_504b.second",
      "label": "second",
      "totalTokens": 30000,
      "costCoverage": 1
    }
  ],
  "byRequestClass": [ { "key": "interactive_stream", "totalTokens": 30000 } ],
  "byDemandKind":   [ { "key": "internal", "totalTokens": 700000, ... } ],
  "byDemandSource": [ { "key": "internal:openagents-gym", "totalTokens": 700000, ... } ],
  "byDemandClient": [ { "key": "internal:gym-opencode-runner", "totalTokens": 700000, ... } ],
  "byDay":    [ { "day": "2026-06-25", "totalTokens": 1131852, "costUsd": 0.272, ... } ],
  "byDemandClientDay": [
    {
      "day": "2026-06-25",
      "key": "internal:gym-opencode-runner",
      "label": "internal / gym-opencode-runner",
      "totalTokens": 700000,
      "costUsd": 0.18
    }
  ],
  "operational": {
    "busyEvents": 0,
    "fallbackEvents": 1,
    "fallbackRate": 0.333333,
    "saturationEvents": 1,
    "queueWaitMs": { "sampleCount": 2, "p50Ms": 0, "p90Ms": 125 },
    "batchWaitMs": { "sampleCount": 0, "p50Ms": "not_measured" },
    "ttftMs": { "sampleCount": 1, "p50Ms": 320, "p90Ms": 320 },
    "totalWallClockMs": { "sampleCount": 3, "p50Ms": 1200, "p90Ms": 2400 },
    "perceivedTokensPerSecond": {
      "sampleCount": 1,
      "p50TokensPerSecond": 40000
    }
  },
  "glmReplicas": [
    {
      "key": "replica.hydralisk.glm_52_reap_504b.second",
      "label": "second",
      "capacityClass": "spot",
      "warmState": "warm",
      "latestInflight": 1,
      "maxInflight": 1,
      "latestQueueDepth": 0,
      "busyEvents": 0,
      "fallbackEvents": 0,
      "saturationEvents": 0,
      "keepWarmStatus": "not_measured",
      "watchdogStatus": "not_measured",
      "uptimeHours": 168,
      "idleHours": 167.999333,
      "effectiveCostPerServedTokenUsd": 0.020682
    }
  ],
  "ownedHourly": {
    "costCoverage": "partial",
    "hourlyBurnUsd": 3.693151,
    "monthlyBurnUsd": 2696,
    "windowBurnUsd": 620.449368,
    "activeDemandBurnUsd": 0.002462,
    "idleBurnUsd": 620.446906,
    "uptimeHours": 168,
    "activeServingHours": 0.000667,
    "idleHours": 167.999333,
    "internalDemandBurnUsd": 0.002462,
    "externalDemandBurnUsd": 0,
    "unlabeledDemandBurnUsd": 0,
    "acceptedOutcomes": "not_measured",
    "costPerAcceptedOutcomeUsd": "not_measured",
    "effectiveCostPerServedTokenUsd": 0.020682,
    "profiles": [
      {
        "profileRef": "cost_profile.hydralisk.glm_52_reap_504b.g4_4g.spot.2026_06_25",
        "machineShape": "g4-standard-192",
        "gpuCount": 4,
        "provisioningModel": "spot",
        "monthlyComputeUsd": 2696,
        "hourlyComputeUsd": 3.693151,
        "monthlyStorageOverheadUsd": "not_measured",
        "hourlyStorageOverheadUsd": "not_measured"
      }
    ],
    "scenarios": [
      { "provisioningModel": "spot", "windowBurnUsd": 620.449368 },
      { "provisioningModel": "dws_flex", "windowBurnUsd": 1512 },
      { "provisioningModel": "on_demand", "windowBurnUsd": 3024 }
    ],
    "demand": [
      {
        "key": "internal:openagents-gym:gym-opencode-runner",
        "activeDemandBurnUsd": 0.002462,
        "totalTokens": 30000
      }
    ],
    "blockerRefs": [
      "blocker.inference_analytics.accepted_outcomes_not_measured",
      "blocker.inference_analytics.glm_benchmark_reserved_burn_not_measured",
      "blocker.inference_analytics.glm_keepwarm_burn_not_measured",
      "blocker.inference_analytics.glm_storage_overhead_not_measured",
      "blocker.inference_analytics.owned_hourly_host_lifecycle_derived_window_assumption"
    ]
  },
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
`ownedHourly.costCoverage` is separate from token-row `costCoverage`: `partial`
means the compute profile and selected analytics window are known, but host
lifecycle, storage, keep-warm, and benchmark reservation splits are still
derived or uncovered.

## 5. How to measure this going forward

- **Live cost/provider read (owner):** sign in as an OpenAgents admin in the
  browser, then:
  ```sh
  curl -s 'https://openagents.com/api/admin/inference-analytics?window=7d' \
    -H "Cookie: <your authenticated openagents.com session cookie>" | jq .
  ```
  Use `window=today` for the day, `30d`/`all` for trend. Watch `byProvider`
  / `bySupplyLane` / `byAdapter` (which lane is carrying load + its cost),
  `byGlmReplica` and `glmReplicas` (which owned replica is serving, saturated,
  warm, idle, or uncovered), `operational` (TTFT, queue wait, TPS, fallback
  rate), `byDemandKind` / `byDemandSource` / `byDemandClient` (internal dogfood
  vs external vs unlabeled tool traffic), `byDemandClientDay` (which tools are
  moving the curve over time), `byDay.costUsd` (daily marginal token burn),
  `totals.costCoverage` (token-cost completeness), and `ownedHourly` (owned GPU
  hourly/idle burn, scenario comparison, cost per served token, and cost per
  accepted outcome when measured).
- **Public served-tokens counter** (`/api/public/khala-tokens-served` +
  `/history`) remains the public-safe token total; it never exposes demand
  labels or implies internal dogfood is external traction. The analytics
  endpoint is the internal cost/provider/demand companion.
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
  `token-usage-ledger.test.ts`, `inference/served-tokens-recorder.test.ts`,
  `inference/inference-free-tier-key.test.ts`
