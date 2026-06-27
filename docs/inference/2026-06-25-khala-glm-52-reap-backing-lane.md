# Khala GLM-5.2 REAP Hydralisk Backing Lane

Date: 2026-06-25

## Summary

Khala now has a private Hydralisk GLM-5.2 REAP 504B backing lane in the
gateway mix. The public API surface remains the single model selector:

```text
openagents/khala
```

When the lane is armed, the router tries the private GLM-5.2 REAP worker first,
then the existing Hydralisk GPT-OSS workers, then Vertex Gemini as the final
degradation lane. Raw supply-lane IDs are not advertised in the public catalog.

For Gym/Terminal-Bench replication work, the Worker may reference the lane
through closed public-safe serving profile refs such as
`glm-reap-504b-g4-tp4-minp-rp105`,
`glm-reap-504b-g4-tp4-mtp2-rp105`, and
`glm-reap-504b-g4-dual-tp4-minp-rp105`. Those refs disclose topology,
context-window, speculation, quantization, sampler guardrails, and source
attribution needed for a decision-grade benchmark comparison, but they do not
disclose private Hydralisk URLs, bearer tokens, raw task prompts, responses, or
operator-only placement notes.

## Operator Contract

The GLM lane is fail-closed and only arms when all transport and evidence fields
are present:

```text
HYDRALISK_GLM_52_REAP_504B_ENABLED=ready
HYDRALISK_GLM_52_REAP_504B_BASE_URL=<private OpenAI-compatible base URL>
HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN=<secret bearer token>
HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF=<public-safe preflight evidence ref>
HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF=<public-safe smoke/receipt evidence ref>
```

These legacy variables now resolve to a pool of one with replica id `primary`.
That keeps the first production smoke path stable while letting Khala broaden
capacity without exposing any new public model selector.

As of 2026-06-26 (#6262) the production Khala GLM pool was armed to the full
10-replica G4 fleet from the live roster, keyed by stable roster replica ids
(e.g. `g4-4g-b-20260625154532` ... `g4-8g-b-20260624214500`). Each replica's
origin URL and bearer are uploaded as Worker secrets
(`HYDRALISK_GLM_52_REAP_504B_<REPLICA_ID>_BASE_URL` / `_BEARER_TOKEN`, hyphens
become underscores, uppercased) and `HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS`
lists all ten. When all replicas are healthy, one inflight slot per replica
lets up to ten overlapping Khala requests spread across distinct G4 hosts
instead of overflowing to the GPT-OSS/Gemini fallback. No replica was
benchmark-reserved at arming time: the roster collection live-probed all ten
endpoints (`/health`, `/v1/models`, and a tiny `/v1/chat/completions`) at HTTP
200 before arming.

Current live state (2026-06-27): serving capacity has recovered, but durability
acceptance is still blocked. The public `/v1/gateway/glm-fleet/readiness`
projection reports `8` ready replicas, `0` reclaimed replicas, and
`warmOrReadyMaxInflight:9`; the operator summary should not emit
`recover_reclaimed_replicas` while reclaimed count is zero. Do not describe
#6311 as complete until the forced STOP recovery evidence, capacity-floor owner
decision evidence, multi-region reserve/prebake auto-replace evidence, and
quota request tracking blockers are satisfied.

For two or more replicas, set a comma-separated pool and use the named variable
form. Replica ids must be lower-case alphanumeric or hyphenated; hyphens become
underscores in env names:

```text
HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS=primary,second

HYDRALISK_GLM_52_REAP_504B_PRIMARY_ENABLED=ready
HYDRALISK_GLM_52_REAP_504B_PRIMARY_BASE_URL=<Worker secret value>
HYDRALISK_GLM_52_REAP_504B_PRIMARY_BEARER_TOKEN=<Worker secret value>
HYDRALISK_GLM_52_REAP_504B_PRIMARY_PREFLIGHT_REF=<public-safe ref>
HYDRALISK_GLM_52_REAP_504B_PRIMARY_RECEIPT_REF=<public-safe ref>
HYDRALISK_GLM_52_REAP_504B_PRIMARY_PROFILE_REF=<public-safe profile ref>
HYDRALISK_GLM_52_REAP_504B_PRIMARY_COST_PROFILE_REF=<public-safe cost profile ref>
HYDRALISK_GLM_52_REAP_504B_PRIMARY_MAX_INFLIGHT=1
HYDRALISK_GLM_52_REAP_504B_PRIMARY_BENCHMARK_RESERVED=true

HYDRALISK_GLM_52_REAP_504B_SECOND_ENABLED=ready
HYDRALISK_GLM_52_REAP_504B_SECOND_BASE_URL=<Worker secret value>
HYDRALISK_GLM_52_REAP_504B_SECOND_BEARER_TOKEN=<Worker secret value>
HYDRALISK_GLM_52_REAP_504B_SECOND_PREFLIGHT_REF=<public-safe ref>
HYDRALISK_GLM_52_REAP_504B_SECOND_RECEIPT_REF=<public-safe ref>
HYDRALISK_GLM_52_REAP_504B_SECOND_PROFILE_REF=<public-safe profile ref>
HYDRALISK_GLM_52_REAP_504B_SECOND_COST_PROFILE_REF=<public-safe cost profile ref>
HYDRALISK_GLM_52_REAP_504B_SECOND_MAX_INFLIGHT=1
HYDRALISK_GLM_52_REAP_504B_SECOND_DRAINING=false
```

The Worker registers one private GLM adapter id for the whole pool:
`hydralisk-vllm-glm-5p2-reap-504b`. Each replica carries public-safe
`profileRef`, evidence refs, cost-profile ref, `maxInflight`, and lifecycle
flags internally; raw URLs and bearer values stay in Worker secrets. Missing or
partial replica fields fail closed for that replica, so a partially armed
`second` endpoint is not used just because `primary` is healthy.

Only the variable names and evidence-reference names belong in tracked files.
Endpoint values, bearer tokens, and private topology stay in Worker secrets or
operator-only notes.

## Worker Pool Heartbeat

The Worker has an inert-by-default GLM pool heartbeat for owner visibility and
route hints:

```text
HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_ENABLED=true
HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_CADENCE_MINUTES=4
HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_WARM_COMPLETION_ENABLED=false
HYDRALISK_GLM_52_REAP_504B_BENCHMARK_OWNERSHIP_ACTIVE=false
```

When enabled, each cadence tick reads the same replica config as the GLM pool.
Replicas marked `benchmarkReserved` or `draining` are recorded as skipped and
receive no HTTP probe. Eligible replicas get control-plane probes to `/health`
and `/v1/models`. A tiny `/v1/chat/completions` warm probe is sent only when
`HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_WARM_COMPLETION_ENABLED=true` and
`HYDRALISK_GLM_52_REAP_504B_BENCHMARK_OWNERSHIP_ACTIVE=false`.

Heartbeat rows land in the canonical token-usage ledger with public-safe refs,
selected replica ref/id, `keepWarmStatus`, `watchdogStatus`,
`warmCompletionStatus`, wall-clock milliseconds, and exact token counts when a
warm completion ran. The in-process routing oracle consumes the latest heartbeat
state it has seen, so warm healthy replicas rank ahead of cold ones while
unhealthy replicas are avoided.

## Pool Routing Contract

Each GLM replica is treated as one interactive singleflight slot unless its
public-safe state says otherwise. The pool adapter builds a typed
`GlmReplicaRoutingState` for each replica before every request:

- health: `healthy`, `degraded`, or `unhealthy`;
- warm timestamp, queue depth, inflight count, and configured `maxInflight`;
- last 429 timestamp, observed TTFT, observed tokens/second, and region when
  the control plane exposes them;
- coarse capacity class (`spot`, `on_demand`, or `unknown`);
- heartbeat-derived warm state (`warm`, `unknown`, or `cold`) when the Worker
  heartbeat has observed the replica;
- `benchmarkReserved` and `draining` lifecycle flags.

The selector only sends product traffic to replicas that are healthy, not
draining, not benchmark-reserved, and below `maxInflight`. If the request has a
cache-affinity value and the affinity oracle maps it to an idle healthy replica,
that replica wins so long conversations and repeated codebase work stay warm.
If the affinity target is busy, unhealthy, draining, reserved, or missing, the
selector falls back to the best warmed idle replica and records a public-safe
fallback reason.

The adapter keeps an in-process inflight counter around every dispatched call.
With two `maxInflight=1` replicas, two overlapping product requests are sent to
different endpoints rather than both hitting the same singleflight proxy. If no
replica is eligible, the pool enters a typed saturation path instead of stacking
more work into either busy four-GPU process.

The saturation policy is explicit:

- `overflow_immediately`: interactive streaming requests fail the GLM adapter
  immediately with retryable `glm_pool_saturated`, so Khala can continue to the
  next supply lane without adding a second request to a busy singleflight
  proxy.
- `queue_then_overflow`: non-stream Khala chat work waits one short bounded edge
  queue window, defaults to 250 ms, and then retries replica selection once. If
  a replica became idle, the request is served there; if not, Khala overflows to
  the next lane with the same typed `glm_pool_saturated` reason.
- `queue_then_429`: operator-controlled strict backpressure for situations
  where we would rather tell the caller to retry than spend the request on a
  non-GLM lane. The response is an OpenAI-compatible `429` with `Retry-After: 1`.

The edge queue is intentionally tiny and hard-capped at 1,000 ms. It is a short
race for a just-finishing request, not a long in-Worker job queue. Longer batch
or detached work belongs in the async lane so Cloudflare request deadlines and
user-visible latency do not become hidden backlog.

Successful Khala responses include the selected replica in the existing
`openagents.routing` block:

```json
{
  "selected_replica_id": "second",
  "selected_replica_ref": "replica.hydralisk.glm_52_reap_504b.second",
  "queue_wait_ms": 0,
  "glm_saturation_policy": "queue_then_overflow",
  "replica_fallback_reason": "inflight_full",
  "replica_health_score": 1,
  "replica_region": "us-central1-a"
}
```

Those fields are refs, coarse scores, and neutral reasons only. They never
include endpoint URLs, private IPs, bearer tokens, prompts, or responses.

When GLM is saturated and Khala successfully overflows to another adapter, the
receipt keeps that public-safe reason attached:

```json
{
  "fallback_reason": "glm_pool_saturated",
  "queue_wait_ms": 250,
  "glm_saturation_policy": "queue_then_overflow",
  "replica_busy_reason": "inflight_full",
  "replica_fallback_reason": "inflight_full"
}
```

That makes operator and owner dashboards honest about why a request did not use
GLM while still keeping the private Hydralisk topology out of customer-visible
data.

## Owner Analytics Contract

Completed Khala requests now persist public-safe GLM routing metrics into the
canonical token usage ledger. The owner-only endpoint:

```text
GET /api/admin/inference-analytics?window=today|7d|30d|all
```

can group traffic by supply lane, adapter id, backing model, request class,
demand labels, and selected GLM replica ref. It also returns a `glmReplicas`
summary with latest inflight count, max inflight, queue depth, warm state,
capacity class (`spot`, `on_demand`, or `unknown`), busy/fallback/saturation
counts, TTFT, wall time, queue wait, and perceived tokens/sec. This is the
private owner/operator surface for answering: which replica served, what fell
back, what saturated, and what cost was recorded on the token row.

Owned hourly economics are deliberately separated from marginal token cost. The
analytics response now maps GLM `replicaCostProfileRef` values to typed
`OwnedInferenceCostProfile` rows and amortizes the selected window's hourly burn
across active serving wall-clock, idle time, served tokens, demand labels, and
accepted outcomes when a caller records `acceptedOutcomes` in public-safe
metadata.

For the default 4-GPU Spot profile
`cost_profile.hydralisk.glm_52_reap_504b.g4_4g.spot.2026_06_25`, the current
owner estimate is `$2,696/month` or `$3.693151/hour` using a 730-hour month. A
12-hour `today` window therefore shows `$44.317812` of GLM burn even when no
token rows exist; if one hour of that window served traffic, roughly `$3.693151`
is active demand burn and the remaining `$40.624661` is idle burn. The same
schema also returns DWS-flex and on-demand scenarios for the same machine shape,
so owners can compare Spot vs durable capacity without changing dashboards.

The response stays honest about what is still missing. `ownedHourly.costCoverage`
is `partial` until Hydralisk host lifecycle telemetry writes exact uptime,
keep-warm probes, watchdog status, benchmark reservation windows, and storage
overhead into an owner ledger. Those fields remain `not_measured` with blocker
refs instead of silently treating uncovered owned-infra cost as free.

## Expected Behavior

- `openagents/khala` uses the GLM Hydralisk adapter first when the GLM lane is
  armed and registered.
- Product traffic avoids GLM replicas marked `benchmarkReserved` or `draining`.
- Same-session/cache-affinity traffic prefers the warm replica when that replica
  is healthy and idle, then falls back to another warmed idle replica when it is
  not.
- Overlapping requests spread across distinct idle singleflight replicas when
  capacity exists.
- Saturated GLM pools do not dispatch into busy endpoints; they use the typed
  overflow, short-queue, or strict-429 policy above.
- If the GLM worker is not armed, the router skips it and continues through the
  rest of the Khala plan.
- The GLM backing lane counts as Hydralisk supply for disclosures, usage
  receipts, and billing context.
- Owner analytics can see GLM replica refs, capacity class, latest inflight,
  queue depth, warm state, fallback/saturation counts, TTFT, wall time, queue
  wait, and TPS without exposing private endpoints.
- Owner analytics can see derived GLM hourly burn, monthly burn, window burn,
  idle burn, active serving hours, internal/external/unlabeled demand burn,
  effective cost per served token, and cost per accepted outcome when accepted
  outcomes are recorded. When the Worker heartbeat is enabled, owners also see
  keep-warm and watchdog status per replica. Benchmark-reserved burn, storage
  overhead, and exact host lifecycle remain explicit `not_measured` gaps until
  those host-side ledgers land.
- Direct customer selection remains closed; public model listing still exposes
  Khala, not internal supply workers.

## Gym Benchmark Matrix

The OpenAgents Gym benchmark harness can now compare the GLM pool against the
same provider field Khala actually uses: Fireworks DeepSeek V4 Flash, Hydralisk
GPT-OSS 120B/20B, Vertex Gemini, Vertex Anthropic, and future Pylon/Psionic
lanes. The GLM target is a public-safe candidate profile:

```text
hydralisk.glm_52_reap_504b.pool.vllm.tp4x2.v1
```

It carries only `replicaPoolRef`, `replicaCount`, route role, cost-profile ref,
and evidence refs. It does not disclose private endpoint URLs, bearer tokens,
raw provider payloads, prompts, responses, raw price, or margin.

The fixture suite remains synthetic and produces `decisionGrade:false`. A real
comparison uses `KHALA_GLM_PROVIDER_OBSERVED_SWEEP_CONFIG`, which anchors the
sequence shape to the public-safe 2026-06-25 Khala token-ledger mix, then passes
through `preflightRealBenchmarkSweep` with owner approval, a positive budget cap,
and a maximum billable-sample cap. Only that owner-armed path can produce
decision-grade routing advice saying whether GLM should be first, fallback, or
reserved for a request class.

## Verification

The code path is covered by API tests for:

- Khala router ordering.
- Direct internal adapter selection.
- Fail-closed arming behavior.
- Catalog exclusion for the raw supply lane.
- Chat-completions routing, disclosure, and metering context.

Run the deployment gate from `apps/openagents.com` before production rollout:

```sh
bun run check:deploy
```

The operator end-to-end smoke for issue #6259 is:

```sh
cd apps/openagents.com
OPENAGENTS_AGENT_TOKEN=oa_agent_... \
HYDRALISK_GLM_52_REAP_504B_ENABLED=ready \
HYDRALISK_GLM_52_REAP_504B_BASE_URL=<Worker secret value> \
HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN=<Worker secret value> \
HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF=<public-safe ref> \
HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF=<public-safe ref> \
bun run smoke:khala:glm-reap -- --approve-live-spend
```

For the #6265 pool smoke, keep the benchmark-owned first lane reserved and make
the second lane the expected eligible product replica:

```sh
cd apps/openagents.com
OPENAGENTS_AGENT_TOKEN=oa_agent_... \
HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS=primary,second \
HYDRALISK_GLM_52_REAP_504B_PRIMARY_ENABLED=ready \
HYDRALISK_GLM_52_REAP_504B_PRIMARY_BASE_URL=<Worker secret value> \
HYDRALISK_GLM_52_REAP_504B_PRIMARY_BEARER_TOKEN=<Worker secret value> \
HYDRALISK_GLM_52_REAP_504B_PRIMARY_PREFLIGHT_REF=<public-safe ref> \
HYDRALISK_GLM_52_REAP_504B_PRIMARY_RECEIPT_REF=<public-safe ref> \
HYDRALISK_GLM_52_REAP_504B_PRIMARY_BENCHMARK_RESERVED=true \
HYDRALISK_GLM_52_REAP_504B_SECOND_ENABLED=ready \
HYDRALISK_GLM_52_REAP_504B_SECOND_BASE_URL=<Worker secret value> \
HYDRALISK_GLM_52_REAP_504B_SECOND_BEARER_TOKEN=<Worker secret value> \
HYDRALISK_GLM_52_REAP_504B_SECOND_PREFLIGHT_REF=<public-safe ref> \
HYDRALISK_GLM_52_REAP_504B_SECOND_RECEIPT_REF=<public-safe ref> \
HYDRALISK_GLM_52_REAP_504B_SECOND_PROFILE_REF=<public-safe profile ref> \
bun run smoke:khala:glm-reap -- \
  --approve-live-spend \
  --expected-replica-id second
```

The pool smoke can also target a single named replica by setting
`HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS=second` and supplying the matching
`HYDRALISK_GLM_52_REAP_504B_SECOND_*` fields above. Do not commit the endpoint
values or bearer tokens; only the env variable names and public-safe evidence
refs belong in tracked files or issue comments.

Default target paths are the public canonical aliases:

- `GET /api/v1/models`
- `POST /api/v1/chat/completions`
- `GET /api/public/khala-tokens-served`

The smoke exits `0` with `skipped: true` when those GLM arming variables are not
present, so CI and unarmed operator shells do not accidentally call another
backing lane. When armed, it verifies:

- the public catalog lists `openagents/khala` but not raw GLM ids such as
  `openagents/glm-5.2-reap-504b`;
- non-streaming and streaming `openagents/khala` calls disclose
  `supply_lane: hydralisk`, `worker: hydralisk-vllm-glm-5p2-reap-504b`, and
  `served_model: openagents/glm-5.2-reap-504b`;
- pool-mode completions disclose a public-safe
  `openagents.routing.selected_replica_ref`, and the smoke rejects a response
  that routes to a replica marked `benchmarkReserved` or `draining`;
- billable tokens dereference a public inference receipt with matching model
  evidence and usage; with `--operator-exempt-zero-debit`, operator-exempt
  tokens may instead disclose a public-safe
  `receipt.inference.operator_credit.*` ref and the smoke records that as an
  `operator_exempt_zero_debit` skip without treating it as a paid receipt;
- the public Khala tokens-served counter moves by approximately the served usage.

Do not run the armed smoke while a decision-grade single-flight benchmark owns
the GLM proxy. The unarmed skip and catalog-only reads are safe; authenticated
completion calls are not.
