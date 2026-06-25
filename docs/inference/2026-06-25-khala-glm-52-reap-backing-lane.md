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

## Expected Behavior

- `openagents/khala` uses the GLM Hydralisk adapter first when the GLM lane is
  armed and registered.
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
- each public inference receipt carries matching model evidence and usage;
- the public Khala tokens-served counter moves by approximately the served usage.

Do not run the armed smoke while a decision-grade single-flight benchmark owns
the GLM proxy. The unarmed skip and catalog-only reads are safe; authenticated
completion calls are not.
