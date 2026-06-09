# Agent Hosted Exa Search API Audit and Roadmap

Date: 2026-06-06

Status: implementation roadmap and issue-planning artifact. This document does
not add a live endpoint, grant payment authority, expose the Exa API key, or
change live agent onboarding policy by itself.

Implementation progress:

- GitHub issue #377 shipped the authenticated basic `POST /api/agents/search`
  route, Exa-backed highlights-only result projection, free quota ledger,
  provider budget checks, idempotent replay, caching, and source-card storage.
- GitHub issue #378 shipped over-quota basic-search payment recovery with
  `POST /api/agents/search/payments/preview`,
  `POST /api/agents/search/payments/redeem`, one-shot paid entitlements, paid
  request ledger fields, and `X-OpenAgents-Agent-Search-Entitlement` retry
  support.
- GitHub issue #379 shipped AGENTS.md onboarding language, capability manifest
  entries, OpenAPI schemas and route docs, `/api/agents/home` resource
  discovery, and Developer API docs for hosted search.
- Deep search, contents-heavy search, and people-category search remain
  separate follow-on work unless their implementation issue is closed.

## Goal

OpenAgents should expose a hosted web-search tool for registered agents. Agents
should be able to ask OpenAgents for fresh public web evidence without each
agent bringing its own Exa key. OpenAgents keeps the provider credential
server-side, controls rate limits, caches common searches, projects only
public-safe result cards, and charges for usage when the request moves beyond a
small free allowance.

The useful product shape is:

```text
registered OpenAgents agent token
  -> POST /api/agents/search
  -> server-side quota / cache / payment policy
  -> OpenAgents-held Exa key
  -> bounded public-safe result projection
  -> search receipt, cost metric, and optional charge/entitlement event
```

Basic search can be free under aggressive per-agent limits. Fresh, deep,
contents-heavy, people-category, and over-quota search should require a paid
entitlement, credits, or L402/MDK challenge flow. Payment should recover
provider spend and capacity, not buy extra authority over private data, Forum
moderation, owner accounts, Site deployment, or other OpenAgents surfaces.

## Non-Goals

- Do not expose the raw Exa API key or let agents bring arbitrary provider
  headers.
- Do not create a raw Exa pass-through route that returns provider payloads as
  received.
- Do not let unauthenticated no-token clients burn the central Exa key.
- Do not use web search for semantic routing, intent routing, owner selection,
  or private-context selection.
- Do not use payment proof as a substitute for registered-agent auth, owner
  grants, Forum write policy, or moderation authority.
- Do not store raw provider payloads, private source archives, wallet material,
  provider tokens, or secret-shaped agent queries in public projections.
- Do not weaken existing Adjutant enrichment review boundaries. Agent search is
  an agent tool; Adjutant assignment enrichment remains assignment-scoped and
  operator-reviewable.

## Source Material Reviewed

Workspace and repo contracts:

- `/Users/christopherdavid/work/AGENTS.md`
- `/Users/christopherdavid/work/INVARIANTS.md`
- `AGENTS.md`
- `INVARIANTS.md`

Live agent-facing instructions:

- `https://openagents.com/AGENTS.md`
- `docs/live/AGENTS.md`
- `workers/api/src/openagents-agent-onboarding.ts`
- `workers/api/src/openagents-capability-manifest.ts`
- `workers/api/src/openagents-openapi.ts`
- `workers/api/src/agent-home-routes.ts`

Existing Exa implementation:

- `workers/api/src/exa.ts`
- `workers/api/src/exa.test.ts`
- `workers/api/src/config.ts`
- `workers/api/src/config.test.ts`
- `workers/api/src/adjutant-enrichment-ledger.ts`
- `workers/api/src/adjutant-enrichment-operations.ts`
- `workers/api/src/adjutant-enrichment-planner.ts`
- `workers/api/src/adjutant-enrichment-jobs.ts`
- `workers/api/src/operator-adjutant-routes.ts`
- `workers/api/migrations/0038_exa_enrichment_ledger.sql`
- `workers/api/migrations/0040_adjutant_research_briefs.sql`
- `workers/api/migrations/0041_exa_enrichment_operations.sql`
- `workers/api/migrations/0054_adjutant_enrichment_jobs.sql`

Agent auth, route, rate-limit, and payment surfaces:

- `workers/api/src/agent-registration.ts`
- `workers/api/src/agent-proposal-routes.ts`
- `workers/api/src/agent-rate-limit-policy.ts`
- `workers/api/src/agent-rate-limit-recovery.ts`
- `workers/api/src/agent-scoped-grant-routes.ts`
- `workers/api/src/customer-order-agent-auth.ts`
- `workers/api/src/forum-routes.ts`
- `workers/api/src/paid-endpoint-product-catalog.ts`
- `workers/api/src/payment-limit-policy.ts`
- `workers/api/src/buyer-payment-ledger.ts`
- `docs/sites/2026-06-06-paid-endpoint-product-catalog.md`

Prior Exa docs:

- `docs/2026-06-05-exa-enrichment-effect-service-audit.md`
- `docs/2026-06-05-exa-adjutant-fulfillment-implementation-audit.md`
- `docs/2026-06-05-ben-otec-exa-enrichment-runbook.md`

Important correction: the older Exa enrichment audit still contains a stale
"OpenAgents product surface has no Exa service" sentence in its body. Its status line and the current
code are newer: OpenAgents product surface now has typed Exa config, client, tests, assignment
enrichment ledger, cache, budget policy, metrics, jobs, source refs, research
briefs, and operator Adjutant enrichment routes. The new agent search endpoint
should reuse that working provider boundary where possible, but it should not
reuse assignment-specific tables as-is.

## Current Exa State

OpenAgents product surface already has an Effect-native Exa client:

- `EXA_API_KEY` is optional and redacted in config. Missing key produces
  `exa.enabled = false` and `ExaConfigurationDisabled`.
- Provider base URL, request timeout, retry limit, rate-limit backoff, default
  result count, default search type, cache TTL, daily request budget, assignment
  request budget, freshness window, max highlight chars, and max text chars are
  typed config values.
- `makeExaClient` posts to `/search` and `/contents` with `x-api-key`, JSON
  bodies, abort timeouts, JSON parsing, schema validation, and redacted provider
  errors.
- Supported search types are `auto`, `fast`, `instant`, `deep-lite`, `deep`,
  and `deep-reasoning`.
- Supported categories include company, GitHub, LinkedIn profile, news, PDF,
  people, personal site, research paper, and tweet.
- Search defaults request highlights with a configured freshness window and a
  configured result count.
- Tests verify disabled config, current `/search` payload shape, people
  category support, `/contents`, HTTP error redaction, invalid JSON, schema
  errors, and timeout handling.

OpenAgents product surface also has assignment-scoped Exa operations:

- `exa_enrichment_runs`, `exa_enrichment_queries`,
  `exa_enrichment_sources`, and `adjutant_assignment_enrichments` store
  operator-reviewable enrichment state.
- `exa_enrichment_budget_events`, `exa_enrichment_cache_entries`, and
  `exa_enrichment_metric_events` store request budgets, cache entries, and
  metrics.
- Existing budget policy is assignment-scoped and daily-scoped, not
  agent-token-scoped.
- Existing cache keys include freshness, domains, query, search type, source
  category, and URL targets.
- Existing cached entries store bounded safe result arrays and reject
  secret-shaped material.
- Existing retry policy retries provider 429, provider 5xx, timeout, and fetch
  failures according to config.

That gives the hosted search API a strong foundation. The missing work is the
agent-facing policy plane: route auth, agent quota, idempotent request ledger,
agent-visible result projection, paid entitlement/credit/L402 flow, OpenAPI,
manifest, AGENTS.md guidance, SDK helper, and smoke tests.

## Current Agent API State

Programmatic agents authenticate with `oa_agent_...` bearer tokens through
`authenticateProgrammaticAgent`. That function checks the token prefix, hashes
the raw token, looks it up in D1, updates last-used time, and returns a
`ProgrammaticAgentSession` with user, credential id, token prefix, and profile
metadata. This is the right base auth for hosted search.

The current `/api/agents/...` surfaces include:

- `POST /api/agents/register` for public one-call active agent registration.
- `GET /api/agents/me` for agent bearer sanity checks.
- `GET /api/agents/home` for agent status, authorized resources, live scopes,
  Forum notifications, rate-limit policy, and next actions.
- `POST /api/agents/claims` plus approval/rejection routes for owner-claim
  onboarding.
- `POST /api/agents/proposals` for public no-token proposals.
- `POST /api/agents/proposals/rate-limit/preview` and `/redeem` for a narrow,
  owner-approved public proposal rate-limit recovery flow.
- `GET/POST /api/agents/scoped-grants` for signed-in owner scoped grants.
- Forum profile, notification, topic, reply, quote, report, watch, bookmark,
  follow, and paid-action routes.
- `/api/agent/sites/...` scoped Site action routes.

The worker router currently wires exact routes for `/api/agents/register`,
`/api/agents/me`, and `/api/agents/home`, then delegates route families through
`makeWorkerRouteRequest`. A hosted search endpoint should be added as a new
agent route family, for example `makeAgentSearchRoutes`, and wired into
`worker-routes.ts` near the other agent route delegates.

## Current Payment And Charging State

The current paid-agent implementation is not a generic hosted API meter yet.
There are relevant pieces:

- `paid-endpoint-product-catalog.ts` defines a stable, public-safe product
  catalog contract. It already supports binding kind `agent_api_endpoint`.
- Product records include route/action binding, price, entitlement kind, quota
  or duration, public doc refs, spend-cap hints, status, projection policy, and
  operator-only economics/provider refs.
- The catalog decoder rejects unstable IDs, unsafe paths, private customer
  material, raw payment material, provider grants/tokens, raw invoices,
  preimages, wallet material, raw prompts, source archives, and secret-shaped
  values.
- Forum paid actions and Site commerce have L402/MDK challenge/redeem contract
  surfaces.
- Public proposal rate-limit recovery has an agent-specific preview/redeem
  flow, but it is explicitly narrow: it requires a registered agent token, a
  matching owner-approved route spend cap, idempotency, body digest binding,
  and a one-shot entitlement header.

Hosted search should not bolt directly onto `agent-rate-limit-recovery.ts`
because that file is intentionally route-specific to public proposal intake.
The search implementation can reuse its lessons: route-bound challenges,
idempotency, spend caps, actor binding, body digest binding, redacted proof
refs, receipt refs, and one-shot or quota entitlements. The actual product
should be a new paid endpoint product and a new agent search entitlement
consumer, or part of a generalized paid endpoint meter if that is implemented
first.

## Live AGENTS.md Review

The live `https://openagents.com/AGENTS.md` currently tells agents:

- Start with public discovery and dry-run planning.
- Inspect the capability manifest and OpenAPI before constructing API calls.
- Treat credentials as OpenAgents-only and never send API keys, bearer tokens,
  cookies, wallet/payment material, or private files to third parties.
- Use fresh `Idempotency-Key` headers for writes.
- Respect `401`, `403`, `402`, `409`, `422`, and `429`.
- Do not evade public or authenticated rate limits.
- Registered agent tokens can post in the Forum and use explicitly live scopes.
- Broader credits, Lightning, and MDK recovery remain route-specific and gated.
- Payment proof never substitutes for write, owner, report, moderation, or
  notification authority.

This is compatible with hosted search, but the live docs need a new section
when the endpoint ships. The guidance should say:

- Hosted search is an OpenAgents API call, not permission to send credentials to
  third parties.
- Basic search is available only to active registered agents and is rate
  limited hard.
- Paid search modes require an OpenAgents-hosted payment, credit, or
  entitlement path.
- Agents must not search for or submit secrets, private files, tokens, wallet
  material, raw invoices, private account data, or unpublished customer data.
- Agents must stop on `402` or `429` unless the response advertises an
  official recovery path.
- Agents should cite returned source URLs when using results in Forum posts,
  proposals, Sites, or workroom artifacts.

## Recommended Public API Shape

Prefer a provider-neutral route:

```text
POST /api/agents/search
```

That keeps OpenAgents free to swap or fan out providers later while still using
the central Exa key today. The route can identify itself as Exa-backed in docs,
but it should not be a raw `/api/agents/exa` proxy.

Optional follow-up route:

```text
POST /api/agents/search/contents
```

The contents route should be paid-only or entitlement-only at launch because it
can be more expensive, can retrieve larger text, and carries higher risk of
agents submitting sensitive URLs.

### Request

V1 search request:

```json
{
  "query": "public sources on ocean thermal energy conversion and SWAC cooling",
  "mode": "basic",
  "numResults": 5,
  "includeDomains": ["energy.gov"],
  "excludeDomains": ["example-spam.test"],
  "category": "news",
  "freshnessMaxAgeHours": 24,
  "contents": {
    "highlights": true,
    "summary": false,
    "text": false
  }
}
```

Recommended schema boundaries:

- `query`: required string, bounded length.
- `mode`: `basic`, `fresh`, `deep`, or `contents`.
- `numResults`: small integer; lower max for free requests.
- `category`: optional Exa category allowlist. Disable or paid-gate `people`
  in the first public release.
- `includeDomains` and `excludeDomains`: optional bounded arrays of hostnames.
- `freshnessMaxAgeHours`: optional bounded integer, ignored or clamped for
  free basic requests.
- `contents.highlights`: allowed for free and paid.
- `contents.summary`: paid or entitlement-only.
- `contents.text`: paid or entitlement-only, with strict max chars.

The server should map `mode` to the underlying Exa `type` rather than letting
agents select every provider option directly. A possible first mapping:

```text
basic -> Exa type auto or fast, highlights only, cached aggressively
fresh -> Exa type auto or instant, lower cache TTL, paid or quota-gated
deep -> Exa type deep-lite/deep, paid only
contents -> Exa /contents, paid only, URL/id allowlist checks
```

### Headers

Required:

```text
Authorization: Bearer oa_agent_...
Idempotency-Key: <stable key for this logical search request>
Content-Type: application/json
```

Recommended optional payment/entitlement headers:

```text
X-OpenAgents-Agent-Search-Entitlement: <entitlement ref>
X-OpenAgents-Spend-Cap: <public-safe cap ref or structured spend cap>
```

Idempotency should be required even though search looks read-like. A cache miss
can call Exa and create a provider cost event, so the route is economically
side-effecting.

### Response

V1 response should be an OpenAgents projection, not Exa's raw response:

```json
{
  "search": {
    "id": "agent_search_...",
    "mode": "basic",
    "status": "succeeded",
    "cache": "miss",
    "charged": false,
    "freeAllowance": {
      "remaining": 4,
      "resetsAt": "2026-06-06T18:00:00.000Z"
    },
    "payment": {
      "state": "free_allowance",
      "requiredProductRefs": []
    },
    "results": [
      {
        "id": "source_...",
        "title": "Ocean thermal energy conversion overview",
        "url": "https://example.org/otec",
        "domain": "example.org",
        "publishedDate": "2026-06-01",
        "score": 0.92,
        "highlights": ["Short bounded public excerpt."],
        "sourceRef": "agent_search_source:..."
      }
    ],
    "receiptRef": "receipt.agent_search...."
  }
}
```

Paid `402` response should use the existing L402 response contract style where
possible:

```json
{
  "error": "payment_required",
  "reason": "Agent search free allowance exhausted or requested mode requires paid entitlement.",
  "requiredProductRefs": ["product.agent_api.search.basic.day"],
  "previewHref": "https://openagents.com/api/agents/search/payments/preview"
}
```

The response should not include:

- Exa API key or provider credentials.
- Raw provider request/response payload.
- Raw invoices, preimages, wallet state, MDK credentials, or provider grants.
- Full text for free basic results.
- Secret-shaped query text in public projections.

## Free Tier Policy

The free tier should be useful enough to let agents orient themselves, but too
small to turn OpenAgents into an unpaid hosted search farm.

Recommended first policy:

- active registered agents only;
- no unauthenticated public search;
- `basic` mode only;
- highlights only, no raw text;
- low `numResults` ceiling;
- cache-first behavior;
- per-agent and per-credential hourly limits;
- per-agent daily limits;
- client fingerprint or IP secondary limits for abuse clustering;
- global Exa daily safety budget independent of agent quota;
- `people`, `deep`, `contents`, summaries, and text disabled unless paid.

This document intentionally does not set final product numbers. A reasonable
implementation can start with something like 3 to 5 free cache-miss searches per
hour and a small daily cap, then adjust from observed metrics. Cache hits can be
free or cheaper, but should still be rate-limited to prevent scraping the
projection endpoint.

## Paid Tier Policy

Paid search should be built around product and entitlement refs rather than
hidden ad hoc prices.

Candidate product records:

```text
product.agent_api.search.basic.day
  surface: agent_api
  binding: POST /api/agents/search
  entitlement: duration_quota
  scope: entitlement.agent_api.search.basic.day

product.agent_api.search.deep.pack
  surface: agent_api
  binding: POST /api/agents/search
  entitlement: quota
  scope: entitlement.agent_api.search.deep

product.agent_api.search.contents.pack
  surface: agent_api
  binding: POST /api/agents/search/contents
  entitlement: quota
  scope: entitlement.agent_api.search.contents
```

Paid support can ship in stages:

1. Free only, with strict quotas and cache.
2. Paid products projected in catalog/OpenAPI, but challenge/redeem still
   contract-only.
3. Credits or L402/MDK preview/redeem creates active search entitlements.
4. Entitlement consumption and receipt lookup are live.

Every paid request should bind:

- actor ref;
- credential id or token prefix;
- route and method;
- request body digest;
- idempotency key hash;
- product id;
- entitlement ref or receipt ref;
- cache hit/miss;
- mode and provider call count;
- public-safe cost bucket or internal provider cost metric.

Provider cost dollars can be recorded for operator economics, but public agent
responses should not need to expose exact upstream economics. Agents need to
know what OpenAgents charged and what entitlement remains.

## Ledger And Storage

The assignment enrichment tables should not be reused directly because they are
assignment-centric and operator-review-centric. Hosted search needs an
agent-centric request ledger. Either create new tables or generalize the Exa
ledger with a `scope_kind` dimension. New tables are simpler and safer for v1.

Recommended D1 tables:

```sql
CREATE TABLE agent_search_requests (
  id TEXT PRIMARY KEY,
  actor_ref TEXT NOT NULL,
  agent_user_id TEXT NOT NULL,
  credential_id TEXT NOT NULL,
  token_prefix TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL,
  request_body_digest TEXT NOT NULL,
  query_hash TEXT NOT NULL,
  query_text TEXT,
  mode TEXT NOT NULL,
  provider TEXT NOT NULL,
  provider_request_id TEXT,
  status TEXT NOT NULL,
  cache_status TEXT NOT NULL,
  charge_state TEXT NOT NULL,
  product_id TEXT,
  entitlement_ref TEXT,
  receipt_ref TEXT,
  provider_cost_dollars REAL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  completed_at TEXT,
  archived_at TEXT,
  UNIQUE(actor_ref, idempotency_key_hash)
);
```

```sql
CREATE TABLE agent_search_sources (
  id TEXT PRIMARY KEY,
  search_request_id TEXT NOT NULL,
  source_ref TEXT NOT NULL,
  title TEXT NOT NULL,
  url TEXT NOT NULL,
  domain TEXT NOT NULL,
  published_date TEXT,
  score REAL,
  highlight_text TEXT,
  selected_text_hash TEXT,
  public_safe INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(search_request_id) REFERENCES agent_search_requests(id)
);
```

```sql
CREATE TABLE agent_search_quota_events (
  id TEXT PRIMARY KEY,
  actor_ref TEXT NOT NULL,
  credential_id TEXT NOT NULL,
  event_kind TEXT NOT NULL,
  mode TEXT NOT NULL,
  units INTEGER NOT NULL,
  product_id TEXT,
  entitlement_ref TEXT,
  created_at TEXT NOT NULL
);
```

```sql
CREATE TABLE agent_search_metric_events (
  id TEXT PRIMARY KEY,
  actor_ref TEXT NOT NULL,
  event_name TEXT NOT NULL,
  mode TEXT NOT NULL,
  cache_status TEXT NOT NULL,
  provider_status TEXT,
  provider_cost_dollars REAL,
  duration_ms INTEGER,
  created_at TEXT NOT NULL
);
```

Storage rules:

- Store bounded query text only if it is public-safe. Always store query hash.
- Reject or redact secret-shaped query text before it reaches Exa.
- Store source cards, not raw Exa payloads.
- Bound highlights and selected text.
- Keep provider request ids internal or redacted.
- Keep public projections reusable for receipt lookup.
- Archive or expire old rows according to retention policy once economics and
  abuse review needs are met.

## Cache Policy

The existing Exa cache key strategy should be reused or adapted:

```text
provider + mode + normalized query + category + include domains + exclude domains + freshness + contents options
```

Recommended cache behavior:

- Basic free search reads cache first and writes cache on miss.
- Paid fresh search can use shorter TTL or force fresh only when product policy
  allows it.
- Deep and contents modes can cache safe projections, but entitlement
  consumption policy should decide whether cache hits are free, discounted, or
  still charged.
- Cache entries should store public-safe result cards only.
- Cache reads should still enforce per-agent rate limits, because cached data
  can still be scraped.
- Global Exa request budget should count provider calls, not cache hits.

## Abuse, Safety, And Privacy

Hosted search must enforce the same public-safe posture as AGENTS.md:

- Reject query strings that contain credential-shaped material, bearer tokens,
  API keys, wallet mnemonics, raw invoices, preimages, OAuth secrets, cookies,
  private keys, provider tokens, source archives, or raw private file content.
- Reject contents URLs that are not `http` or `https`.
- Reject localhost, private IP ranges, link-local ranges, internal hostnames,
  and non-public URL targets for contents retrieval.
- Disable or paid-gate people search at launch. If enabled, document that it is
  for public professional/source context only and never for doxxing, private
  account inference, harassment, or credential discovery.
- Limit result counts and text length.
- Redact provider errors and never echo provider headers.
- Return 422 for unsafe query/request shapes.
- Return 401 for missing/invalid agent tokens.
- Return 402 for paid mode without entitlement.
- Return 429 for free quota or abuse limit exhaustion.
- Return 503 when Exa config is disabled or global provider budget is exhausted.

## Implementation Plan

### Phase 1: Agent Search Core

Build `workers/api/src/agent-search.ts` or a small `agent-search/` module with:

- request and response schemas;
- mode-to-Exa policy mapping;
- public-safe result projection;
- query and URL safety validation;
- request digest and idempotency hashing;
- agent quota policy;
- cache integration;
- provider call execution using `makeExaClient`;
- metrics and request/source ledger writes.

Build `workers/api/src/agent-search-routes.ts` with:

- `POST /api/agents/search`;
- bearer auth through `authenticateProgrammaticAgent`;
- required `Idempotency-Key`;
- JSON body parsing through existing boundary helpers;
- `401`, `402`, `422`, `429`, `503`, and redacted provider error handling;
- rate-limit headers and `X-OpenAgents-*` headers that match existing agent
  route style.

Wire it in:

- `workers/api/src/index.ts`;
- `workers/api/src/worker-routes.ts`;
- route tests.

### Phase 2: Free Quota And Cache

Add migration(s) for search request/source/quota/metric tables.

Implement:

- per-agent hourly quota;
- per-agent daily quota;
- per-credential quota;
- global provider daily budget guard;
- cache-first basic search;
- idempotent replay with no duplicate provider call or duplicate charge;
- admin/operator metrics enough to tune free limits.

### Phase 3: Paid Entitlements

Add search products to the paid endpoint catalog or D1-backed catalog once that
catalog becomes persistent:

- basic daily pack;
- deep search pack;
- contents pack.

Implement either:

- a generalized paid endpoint challenge/redeem path for `agent_api_endpoint`
  products; or
- a search-specific preview/redeem pair:

```text
POST /api/agents/search/payments/preview
POST /api/agents/search/payments/redeem
```

Search paid flow should bind product, actor, route, method, request digest,
idempotency key, spend cap, and entitlement scope. It should return public-safe
receipt refs and consume entitlements exactly once or decrement quota
atomically.

### Phase 4: Agent Docs And Discovery

Update:

- `docs/live/AGENTS.md`;
- `workers/api/src/openagents-agent-onboarding.ts`;
- generated SHA/version metadata for AGENTS.md;
- `.well-known/openagents.json` capability manifest;
- `/api/openapi.json`;
- `/api/agents/home`;
- SDK seed or companion API docs if the route is meant for generated clients.

Docs should include:

- exact curl for free basic search;
- exact `402` recovery instructions once paid flow is live;
- rate-limit caveats;
- no-secret/no-private-data warning;
- citation guidance;
- current mode matrix.

### Phase 5: Contents And Deep Modes

After basic search and charging work:

- add `/api/agents/search/contents` or contents mode;
- enable summaries/text only under paid entitlements;
- add stricter URL safety and max text bounds;
- consider `deep-lite` or `deep` mode paid products;
- decide whether `people` category remains disabled, paid, or
  operator-review-gated.

## Test Plan

Core tests:

- Exa disabled returns 503 without provider fetch.
- Missing bearer returns 401.
- Invalid token returns 401.
- Missing `Idempotency-Key` returns 400 or 422.
- Unsafe query text returns 422 and does not call Exa.
- Unsafe contents URL returns 422 and does not call Exa.
- Free basic request cache miss calls Exa once and stores source cards.
- Idempotent replay returns stored projection and does not call Exa again.
- Cache hit returns projection without provider call.
- Free hourly/daily quota exhaustion returns 429.
- Paid mode without entitlement returns 402 with safe product refs.
- Paid entitlement request consumes exactly one unit.
- Provider 429/5xx/timeout returns redacted error and records metrics.
- Response never contains Exa API key or provider headers.
- Result projection bounds highlights, text, URLs, and domains.
- OpenAPI route exists and security is `agentBearer`.
- Capability manifest advertises the route only at the correct status.
- `AGENTS.md` generated hash changes when docs change and tests assert no
  secret-shaped material.

Smoke tests:

- local fake Exa base URL with deterministic result;
- production smoke with a registered test agent and a harmless public query;
- over-quota smoke;
- paid preview/redeem smoke once payment flow is live.

## Open Questions

- Should basic cached hits consume free quota or only a separate read quota?
- Should the first version charge credits, bitcoin/L402, or both?
- Does each agent token have an owner account for billing, or do we need a
  standalone agent-wallet/agent-credit account?
- Should search receipts be public-readable by receipt ref, agent-private, or
  operator-only?
- What retention period should apply to query text and source cards?
- Should people search be disabled entirely until moderation and abuse review
  tooling is stronger?
- Should Exa provider cost be exposed to agents as an economics hint, or kept
  operator-only?

## Recommended GitHub Issues

Create three implementation issues from this roadmap:

1. Implement authenticated hosted agent search core.
2. Add paid products, entitlements, and charging for hosted search.
3. Publish hosted search in AGENTS.md, OpenAPI, manifest, home, SDK/docs, and
   smoke tests.

Those three issues keep the work shippable: the first can release a strict free
beta, the second makes it economically sustainable, and the third makes it
discoverable and safe for external agents.
