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

## Pool Routing Contract

Each GLM replica is treated as one interactive singleflight slot unless its
public-safe state says otherwise. The pool adapter builds a typed
`GlmReplicaRoutingState` for each replica before every request:

- health: `healthy`, `degraded`, or `unhealthy`;
- warm timestamp, queue depth, inflight count, and configured `maxInflight`;
- last 429 timestamp, observed TTFT, observed tokens/second, and region when
  the control plane exposes them;
- coarse capacity class (`spot`, `on_demand`, or `unknown`);
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
- Direct customer selection remains closed; public model listing still exposes
  Khala, not internal supply workers.

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
- each public inference receipt carries matching model evidence and usage;
- the public Khala tokens-served counter moves by approximately the served usage.

Do not run the armed smoke while a decision-grade single-flight benchmark owns
the GLM proxy. The unarmed skip and catalog-only reads are safe; authenticated
completion calls are not.
