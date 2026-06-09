# Exa Enrichment Effect Service Audit

Date: 2026-06-05

Status: audit plus staged implementation tracker. Issues #89 through #97 have
implemented the typed Exa config/client, ledger, planner, source refs, research
brief task-packet integration, operator API/admin review surface, and
operational budgets/cache/metrics policy, plus the canonical Ben OTEC smoke
runbook and route-level smoke coverage. This document still does not authorize
use of private social data, raw provider payload projection, or unreviewed
customer/public visibility.

## Executive Summary

OpenAgents product surface does not currently have an Exa service. The active Adjutant/Sites
implementation has durable assignments, preflight, task packets, launch,
callback lifecycle mapping, public activity projection, and customer-safe
progress, but the task packet still mostly reflects the order prompt and
operator notes. That is enough to build Ben's OTEC page from the literal order:

```text
Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter.
```

It is not enough to make the sitebuilder feel grounded in the customer's public
work, public repos, and public statements.

The right next step is an Effect-native Exa enrichment layer:

```text
explicit Adjutant assignment
  -> typed enrichment plan
  -> Exa search / contents calls
  -> bounded evidence ledger
  -> operator-reviewed research brief
  -> Adjutant task packet / launch selector
  -> sitebuilder run starts with sourced context
```

The critical policy boundary is that Exa enrichment must not become intent
routing. Customer text such as "OTEC", "ocean", "website", or "landing page"
must not decide that Adjutant owns the order, which account gets searched, or
which private context gets included. Adjutant assignment remains explicit. Exa
only enriches after a typed order, Site, assignment, or operator-selected public
source has already been selected.

For Ben's OTEC/SWAC launch, the desired outcome is:

- preserve the exact software order as the authoritative customer request;
- discover public context around the order subject, source repository, and
  explicit public identity/source refs;
- summarize only sourced evidence, with URLs and short excerpts;
- identify unknowns and claims that need operator review;
- feed the safe brief into the Adjutant task packet before launch.

## Source Material Reviewed

Workspace and repo guidance:

- `/Users/christopherdavid/work/AGENTS.md`
- `/Users/christopherdavid/work/INVARIANTS.md`
- `AGENTS.md`
- `INVARIANTS.md`

OpenAgents product surface Adjutant and Sites docs:

- `docs/sites-plan.md`
- `docs/2026-06-05-adjutant-sites-supervisor-audit.md`
- `docs/2026-06-04-stripe-effect-service-audit.md`
- `docs/2026-06-04-effect-foldkit-codebase-audit.md`
- `docs/2026-06-04-openagents-zero-tech-debt-caller-inventory.md`

OpenAgents product surface implementation surfaces:

- `workers/api/src/config.ts`
- `workers/api/src/stripe-billing.ts`
- `workers/api/src/customer-orders.ts`
- `workers/api/src/sites.ts`
- `workers/api/src/adjutant-assignments.ts`
- `workers/api/src/adjutant-task-packets.ts`
- `workers/api/src/operator-adjutant-routes.ts`
- `workers/api/src/adjutant-run-lifecycle.ts`
- `workers/api/src/adjutant-public-activity.ts`
- `workers/api/src/index.ts`
- `workers/api/migrations/0030_software_orders.sql`
- `workers/api/migrations/0032_autopilot_sites.sql`

Local Exa references:

- `docs/probe/02-opencode-architecture-and-lessons.md`
- `../projects/repos/cherry-studio/src/main/services/webSearch/providers/api/ExaProvider.ts`
- `../projects/repos/cherry-studio/src/main/services/webSearch/providers/mcp/ExaMcpProvider.ts`
- `../projects/repos/cherry-studio/src/main/services/webSearch/WebSearchService.ts`
- `../projects/repos/cherry-studio/src/shared/data/presets/web-search-providers.ts`
- `../projects/repos/cherry-studio/src/main/services/agents/services/claudecode/index.ts`
- `../projects/repos/cherry-studio/src/main/services/agents/services/cherryclaw/prompt.ts`

Effect guidance:

- `effect-solutions show services-and-layers config error-handling testing`

External Exa documentation checked on 2026-06-05:

- `https://exa.ai/docs/sdks/javascript-sdk`
- `https://exa.ai/docs/reference/search-api-guide-for-coding-agents`
- `https://exa.ai/docs/reference/contents-retrieval`
- `https://exa.ai/docs/reference/verticals/people-for-coding-agents`
- `https://exa.ai/docs/reference/exa-mcp`
- `https://exa.ai/docs/reference/rate-limits`
- `https://exa.ai/docs/websets/api/overview`

## Current OpenAgents product surface State

OpenAgents product surface has no `EXA_API_KEY`, no `exa-js` dependency, no Exa service, no Exa D1
ledger, and no Exa-backed enrichment step in Adjutant launch.

The current Adjutant implementation is already strong enough to host the new
boundary:

- `AdjutantAssignmentService` creates a cross-object receipt linking software
  orders, Sites, goals, runs, packet refs, commits, objective, status, and
  visibility.
- `operator-adjutant-routes.ts` exposes assignment, preflight, task packet,
  review, and launch routes.
- `buildAdjutantTaskPacket` writes safe Markdown using assignment/order/Site
  context and rejects secret-shaped material.
- `launchSelectorForAssignment` passes the task packet, commit, order, Site,
  goal, target URL, and preflight summary into the run selector.
- `adjutant-run-lifecycle.ts` maps runner lifecycle events into order, Site,
  assignment, and artifact receipt state.
- `customer-orders.ts` projects customer-safe Adjutant progress.
- `adjutant-public-activity.ts` projects public-safe activity only.
- `index.ts` parses `@adjutant` as a bounded command in the Adjutant project
  room and still refuses context-free Adjutant intent.

The current gap is not assignment or dispatch. The gap is that the assignment
does not have a durable, reviewable research brief before the sitebuilder starts.

## Local Exa Lessons

The only owned-workspace Exa mention outside reference repos is a Probe note
from the OpenCode audit: tool availability should depend on runtime, model,
policy, and extension state, including whether Exa-backed search tools are
available. That lesson matters for OpenAgents product surface because Exa should be a policy-gated
tool and service, not a globally assumed capability.

The Cherry Studio reference is the useful implementation precedent:

- Its direct Exa provider validates request and response shape, posts to
  `https://api.exa.ai/search`, sends `x-api-key`, asks for text content, and
  normalizes results into a provider-neutral `WebSearchResponse`.
- Its Exa MCP provider posts JSON-RPC `tools/call` requests to
  `https://mcp.exa.ai/mcp`, calls `web_search_exa`, accepts SSE or text
  responses, sets a 25 second timeout, and parses result chunks into title,
  URL, and text.
- Its provider preset layer records Exa direct API and Exa MCP as separate
  providers with capability metadata.
- Its web search service fans out across inputs with `Promise.allSettled`,
  keeps caller aborts distinct from partial provider failures, applies domain
  blacklist filtering, post-processes final results, and logs failures without
  failing successful independent inputs.
- Its Claude Code path injects hosted Exa MCP for structured web search while
  adding tool guidance that describes what the tool can and cannot do.

OpenAgents product surface should not copy that code. It should port the architectural lessons into
Effect services:

- typed provider config;
- typed requests and responses;
- explicit capability availability;
- timeout and abort handling;
- partial failure policy;
- source post-processing;
- no raw API keys in prompts, task packets, D1, logs, or public projections.

## Current Exa API Notes

New OpenAgents product surface code should target Exa's current `/search` and `/contents` shapes,
not older snippets.

Relevant Exa API facts from the checked docs:

- `/search` is `POST https://api.exa.ai/search` with `x-api-key`.
- JavaScript SDK support exists through `exa-js`, but OpenAgents product surface can use direct
  Worker `fetch` to keep schema decoding and errors under its own service
  boundary.
- `type: "auto"` is the default search mode. `fast` and `instant` are latency
  modes. `deep-lite`, `deep`, and `deep-reasoning` are more expensive synthesis
  modes.
- For agent workflows, `contents: { highlights: true }` is the recommended
  first retrieval mode because it gives relevant excerpts without pulling full
  page text into every result.
- On `/search`, content fields are nested under `contents`. On `/contents`,
  those same fields are top-level because URLs or IDs are already known.
- Freshness should use `contents.maxAgeHours`, with `0` forcing live crawl.
  Older `livecrawl` examples should not be copied into new code.
- `outputSchema` works with `/search` when the product needs structured
  synthesis, and Exa returns grounding separately. Do not duplicate citation or
  confidence fields inside the requested schema.
- People Search is still `/search` with `category: "people"`. It searches
  public professional profiles and may return structured `entities` metadata.
  It has parameter restrictions: no date filters or exclude domains, and all
  constraints should be encoded in the natural-language query.
- The legacy `/research/v1` API should not be the first implementation path for
  Adjutant enrichment. Use `/search` with deep modes and `outputSchema` for
  structured research-style output. Websets can be a later async scale-out path.
- Default published rate limits are 10 QPS for `/search`, 100 QPS for
  `/contents`, and 10 QPS for `/answer`.
- Hosted Exa MCP is available at `https://mcp.exa.ai/mcp`. It is useful for
  agent tool access, but production OpenAgents product surface enrichment should still have a direct
  service path so requests, costs, payload size, and projection safety are
  auditable.

## Product Model

The product feature is not "let the agent browse." The feature is "create a
sourced, bounded, operator-reviewable context brief before Adjutant starts a
sitebuilding run."

Recommended staged flow:

1. Customer submits or already has a `software_orders` row.
2. Operator explicitly creates or selects an Adjutant assignment.
3. Operator triggers enrichment, or launch requires an enrichment check for
   selected assignment kinds.
4. `AdjutantEnrichmentPlanner` builds a typed plan from explicit refs:
   software order prompt, Site slug/title, source repository, selected public
   account/source refs, and optional operator notes.
5. `ExaSearchService` executes bounded searches and optional contents calls.
6. `AdjutantResearchBriefService` stores a safe brief and source cards.
7. Operator reviews the brief for unsupported claims, bad sources, privacy
   concerns, and public suitability.
8. Task packet generation includes the approved brief or a reference to it.
9. Launch selector includes the approved brief ref and brief summary.
10. Runner starts with the order prompt plus sourced context, not with hidden
    ad hoc web browsing.

This preserves the existing explicit-assignment invariant while letting the
sitebuilder start with current, grounded information.

## Social And Public Identity Enrichment

The user example is important: a customer may say one short line about OTEC and
ocean infrastructure, while their public GitHub, public website, LinkedIn,
Twitter/X, or other public statements may reveal what they actually care about.

The safe implementation should be explicit and typed:

- Use the customer's connected GitHub identity and selected repository only
  when the user has connected GitHub or the order already names a public repo.
- Use public social/profile URLs only when they are stored as typed source refs
  or provided by an operator/customer for this assignment.
- Do not infer private account access from a name match.
- Do not scrape private social media, private repos, OAuth-only surfaces, DMs,
  emails, browser sessions, or provider-account grants through Exa.
- Do not store raw profile dumps in D1. Store bounded source cards, URLs,
  titles, short highlights, timestamps, and reviewer decisions.
- Do not show public/customer projections of private-source enrichment. Public
  projections may show only approved source titles/URLs and high-level
  summaries.

For Ben's OTEC assignment, the initial safe source set would be:

- the `software_orders.request` text;
- the `bensilone/openagents` repository ref already attached to the order;
- any public source URLs explicitly attached by the operator or customer;
- public web searches around OTEC, SWAC, floating datacenters, and the named
  repository only after the assignment exists.

The planner can generate richer queries, but it should be a typed planner with
policy tests, not keyword routing.

## Proposed Effect Service Topology

### Exa Config

Add `ExaConfig` as the only owner of Exa secrets and defaults.

Recommended fields:

- `apiKey: Redacted.Redacted<WorkerSecret>`
- `baseUrl: "https://api.exa.ai"`
- `defaultSearchType: "auto"`
- `defaultNumResults`
- `requestTimeoutMs`
- `maxHighlightCharacters`
- `maxTextCharacters`
- `freshnessMaxAgeHours`
- `enabled`

Use Worker secrets for `EXA_API_KEY`. Do not put it in `wrangler.jsonc` `vars`.

### Exa Client

Add a direct HTTP client service rather than route-local `fetch`.

Recommended methods:

```ts
search(input: ExaSearchInput): Effect.Effect<ExaSearchResponse, ExaError>
getContents(input: ExaContentsInput): Effect.Effect<ExaContentsResponse, ExaError>
```

Service rules:

- Use `Effect.fn` for service methods.
- Accept a test fetcher in the live/test layer.
- Use `AbortSignal.timeout` or an equivalent timeout wrapper.
- Send `x-api-key` only from `ExaConfig`.
- Decode requests and responses with Effect Schema.
- Classify 400, 401, 422, 429, 5xx, timeout, invalid JSON, and schema errors
  as tagged errors.
- Record Exa request IDs and cost fields when returned, but never log the key
  or raw provider payload.

### Search Planner

Add a policy-owned planner service:

```text
AdjutantEnrichmentPlanner
  input: assignment + order + Site + explicit public source refs
  output: ExaEnrichmentPlan
```

The planner output should be data:

- plan ID;
- subject summary;
- search tasks;
- contents tasks;
- people/profile tasks, when explicit public identity refs are present;
- privacy policy decisions;
- result limits;
- freshness settings;
- expected source categories;
- blocked source reasons.

The planner can use model-generated structured output later, but the first
implementation can be deterministic from typed refs. Either way, policy tests
must prove no loose keywords select private context or an Adjutant route.

### Research Ledger

Add a D1/R2-backed ledger:

- `exa_enrichment_runs`
- `exa_enrichment_queries`
- `exa_enrichment_sources`
- `adjutant_assignment_enrichments`

D1 should store small safe facts. If raw Exa payloads are needed for operator
debugging, put them behind private R2 keys with strict size limits and never
project them publicly.

### Adjutant Research Brief

Add a service that converts Exa results into a bounded brief:

- customer request;
- source cards;
- grounded themes;
- facts safe to use;
- claims needing review;
- unknowns;
- suggested site sections;
- source list;
- rejected/private/unsafe sources.

The brief should be included in task packets only after it passes safety checks
and size limits. The packet can include source URLs and short highlights. It
should not include raw large text, private profile data, or hidden policy.

## Where It Fits In Adjutant

The best integration point is before `buildAdjutantTaskPacket`.

Current:

```text
assignment -> task packet -> preflight -> launch
```

Recommended:

```text
assignment -> enrichment plan -> Exa calls -> research brief -> task packet
  -> preflight checks include approved/fresh brief -> launch
```

Preflight should add checks:

- `exa_config`
- `enrichment_plan`
- `enrichment_sources`
- `research_brief`
- `research_review`

Launch should not block every assignment forever if Exa is unavailable. Use
policy by assignment kind:

- `site_generation`: warning or required depending on product flag.
- `site_adjustment`: optional if the existing Site has an approved brief.
- `site_deployment`: not needed except to verify no stale claims changed since
  generation.
- `general_order_fulfillment`: operator decision.

For Ben OTEC, make the first launch require an approved brief because it is the
canonical public launch smoke.

## Security And Privacy Boundaries

Hard rules:

- Exa key lives in Worker secret config only.
- No Exa key in task packets, prompts, D1 rows, issue comments, logs, or public
  projections.
- No raw social/private account scraping.
- No provider-account grants, GitHub write grants, OAuth tokens, callback URLs,
  browser session data, emails, or private repo content in Exa queries.
- No customer/public projection of unapproved source cards.
- No D1 storage of large raw page text.
- No ad hoc keyword routing.

The customer and public products should eventually show that a Site was
"grounded in public sources" only after the operator marks source cards as
public-safe.

## Ordered Implementation Issues

Create and implement these issues in order:

1. [#89](https://github.com/OpenAgentsInc/openagents/issues/89)
   `Exa enrichment: add source authority, config, and direct API client service`
2. [#90](https://github.com/OpenAgentsInc/openagents/issues/90)
   `Exa enrichment: add typed response schemas, fixtures, and provider tests`
3. [#91](https://github.com/OpenAgentsInc/openagents/issues/91)
   `Exa enrichment: add D1 research ledger and safe source-card storage`
4. [#92](https://github.com/OpenAgentsInc/openagents/issues/92)
   `Exa enrichment: add Adjutant enrichment planner with policy tests`
5. [#93](https://github.com/OpenAgentsInc/openagents/issues/93)
   `Exa enrichment: support explicit public identity and repository source refs`
6. [#94](https://github.com/OpenAgentsInc/openagents/issues/94)
   `Exa enrichment: tie approved research briefs into Adjutant task packets`
7. [#95](https://github.com/OpenAgentsInc/openagents/issues/95)
   `Exa enrichment: add operator APIs and admin review UI for research briefs`
8. [#96](https://github.com/OpenAgentsInc/openagents/issues/96)
   `Exa enrichment: add budgets, rate limiting, observability, and cache policy`
9. [#97](https://github.com/OpenAgentsInc/openagents/issues/97)
   `Exa enrichment: add Ben OTEC end-to-end smoke and launch runbook`

Each issue should be closed only after the implementation comments include
files changed, tests run, production config status, and remaining follow-ups.

## Implementation Notes For The Issues

### 1. Exa enrichment: add source authority, config, and direct API client service

Deliverables:

- Add this audit as source authority for Exa enrichment.
- Add `EXA_API_KEY` to Worker config decoding as a redacted optional secret.
- Add `ExaConfig`, `ExaClient`, and tagged Exa errors.
- Use direct Worker `fetch` against `https://api.exa.ai`.
- Keep `exa-js` out of the first implementation unless direct fetch proves
  inadequate.

Acceptance:

- Missing `EXA_API_KEY` produces a typed disabled/unconfigured state, not a
  module-load crash.
- The live client sends `x-api-key` only from redacted config.
- Tests prove the key is not serialized in errors, logs, or service output.

### 2. Exa enrichment: add typed response schemas, fixtures, and provider tests

Deliverables:

- Add Effect Schema models for `/search` and `/contents` request/response
  shapes used by OpenAgents product surface.
- Cover results, highlights, text, summary, output, grounding, request ID,
  search type, and cost fields.
- Add fixtures for successful search, contents retrieval, people search
  entities, 400, 401, 422, 429, timeout, invalid JSON, and schema mismatch.

Acceptance:

- Response decoding is total for supported fields and defensive for optional
  provider fields.
- Older/incorrect parameter shapes such as top-level `text`, top-level
  `highlights`, and `livecrawl` are not emitted by OpenAgents product surface.

### 3. Exa enrichment: add D1 research ledger and safe source-card storage

Deliverables:

- Add migrations for enrichment runs, queries, source cards, and
  assignment-to-brief linkage.
- Store bounded source cards with title, URL, domain, published date, short
  highlights, selected text hash, Exa request ID, search type, and review
  status.
- Add payload size limits and secret-shaped material rejection.

Acceptance:

- No raw large Exa payload is written to D1.
- Source-card text is length bounded.
- Public/customer projections fail closed if source cards contain
  provider/secret/private-shaped material.

### 4. Exa enrichment: add Adjutant enrichment planner with policy tests

Deliverables:

- Add `AdjutantEnrichmentPlanner` as an Effect service.
- Build enrichment plans from explicit assignment/order/Site/source-ref inputs.
- Add planner output schemas and source policy decisions.
- Add tests proving the planner does not use prompt keywords to select
  Adjutant, a software order, a Site, or private account context.

Acceptance:

- `@adjutant Build the Ben OTEC website from the order` remains context-free
  and rejected unless explicit context is provided.
- Given an explicit `softwareOrderId` and Site, the planner can create web,
  repository, and optional public identity search tasks.

### 5. Exa enrichment: support explicit public identity and repository source refs

Deliverables:

- Add typed public source refs for GitHub repositories, GitHub profiles,
  personal sites, LinkedIn profiles, X/Twitter profiles, and generic public
  URLs.
- Add operator/customer-safe APIs to attach approved public source refs to an
  assignment or order.
- Add people-search support only when a public person/profile ref is explicit.

Acceptance:

- The service never searches private repos, OAuth-only profile data, email,
  DMs, or provider-account grants.
- Public source refs can be approved, rejected, or marked internal-only.
- Ben OTEC can use the explicit order repo and any explicit public refs without
  guessing from the name "Ben".

### 6. Exa enrichment: tie approved research briefs into Adjutant task packets

Deliverables:

- Add `AdjutantResearchBriefService`.
- Generate brief sections for grounded facts, suggested site sections, source
  cards, unknowns, and claims needing operator review.
- Update `buildAdjutantTaskPacket` to include approved brief content or a brief
  ref.
- Update launch selector metadata to include `researchBriefId`.

Acceptance:

- Task packets include concise sourced context and URLs.
- Packets still reject secret-shaped material.
- Launch preflight can warn or block when a required brief is missing, stale,
  or unreviewed.

### 7. Exa enrichment: add operator APIs and admin review UI for research briefs

Deliverables:

- Add operator routes for plan, run, read, approve, reject, and refresh
  enrichment.
- Add admin UI states for queued/running/succeeded/failed enrichment.
- Let operators approve source cards and brief sections before launch.

Acceptance:

- Only core operators can run and approve enrichment.
- The UI does not expose raw provider payloads or private rejected sources.
- Assignment review shows the current research brief status and next safe
  action.

Implemented in #95:

- Operator routes now exist under
  `/api/operator/adjutant/assignments/:assignmentId/enrichment` for read,
  `plan`, `run`, `refresh`, `source-refs`, source-ref review, source-card
  review, and brief review.
- `run` fails fast with `exa_unconfigured` when `EXA_API_KEY` is absent. When
  configured, it executes a bounded task subset, stores normalized source
  cards, records failed provider calls as failed queries, rolls up run status,
  and creates a needs-review brief.
- Assignment review now includes `review.enrichment` with latest run, queries,
  source-card summaries, public source refs, current brief, status, and next
  action.
- The admin review UI shows compact research status, run counts, source cards,
  source-card approve/reject actions, and brief approve/reject/stale actions.
  It does not show raw Exa payloads, request headers, API keys, or hidden text
  for rejected/internal source refs.

### 8. Exa enrichment: add budgets, rate limiting, observability, and cache policy

Deliverables:

- Add per-assignment and per-day Exa request budgets.
- Add backoff for 429 and retry policy for transient 5xx/timeout failures.
- Add cache policy using source URL/domain/query hashes and `maxAgeHours`.
- Add structured metrics for request count, latency, status, cost, source
  count, and brief approval state.

Acceptance:

- A failed or rate-limited Exa call cannot cause duplicate launches.
- Operators see a typed blocker or warning, not a generic failure.
- Cached enrichment can be reused only within the configured freshness window.

Implemented in #96:

- Worker config now includes `EXA_ASSIGNMENT_REQUEST_BUDGET`,
  `EXA_DAILY_REQUEST_BUDGET`, `EXA_CACHE_TTL_HOURS`,
  `EXA_RATE_LIMIT_BACKOFF_MS`, and `EXA_RETRY_LIMIT`, with typed validation and
  safe defaults.
- `adjutant-enrichment-operations.ts` adds an Effect service for durable
  budget reservations, normalized Exa cache entries, redacted metric events,
  cache keys, and retry decisions.
- Migration `0041_exa_enrichment_operations.sql` adds D1 tables for budget
  events, cache entries, and metric events. Cache entries store only bounded
  normalized source results, not raw provider payloads.
- The operator enrichment `run` path now rejects duplicate active enrichment
  runs, reserves budget before marking briefs stale or creating runs, reuses
  fresh compatible cache entries, retries 429/5xx/timeout/fetch failures within
  policy, records cache hit/miss and success/failure metrics, and records brief
  review-state metrics.
- Typed operator responses now include `exa_budget_exhausted`,
  `adjutant_enrichment_already_running`,
  `unsafe_enrichment_operations_payload`, and
  `enrichment_operations_storage_error`.
- Tests cover config decoding, budget exhaustion, 429/backoff retry decisions,
  retry limits, cache hit/miss/stale behavior, cache replacement archival,
  metric names/redaction, duplicate active enrichment guards, and route-level
  budget blockers.

### 9. Exa enrichment: add Ben OTEC end-to-end smoke and launch runbook

Deliverables:

- Add a runbook for assigning Ben's OTEC Site, running Exa enrichment,
  approving the brief, generating the task packet, preflighting, launching,
  saving the version, and deploying after review.
- Add test fixtures for the OTEC order, public repo/source cards, research
  brief, task packet, preflight, and launch selector.
- Add a manual production smoke checklist that does not print secrets.

Acceptance:

- The canonical OTEC flow proves that an initial one-line customer goal can be
  enriched with current public evidence before Adjutant starts the project.
- Customer/public projections remain clean and safe.
- The runbook names exact routes and commands for operators.

Implemented in #97:

- Added `docs/2026-06-05-ben-otec-exa-enrichment-runbook.md` with the full
  operator flow from assignment creation through explicit public source refs,
  enrichment planning/running, source-card review, brief approval, task packet
  generation, preflight, launch, Site version save, deploy, rollback, and
  failure handling.
- Added an `exa_enrichment` preflight check so operators see Exa
  configuration/latest-run status alongside `research_brief` and
  `research_review`.
- Added route-level OTEC smoke fixtures for an approved enrichment run and
  approved research brief with public-safe OTEC/SWAC source cards.
- Added tests proving the generated task packet includes the approved brief and
  source URLs, preflight reports `exa_enrichment`, `research_brief`, and
  `research_review` as `ok`, and the launch selector carries
  `researchBriefId` plus approved brief metadata.

## Open Questions

- Should `site_generation` require an approved Exa brief by default, or only
  for public launch candidates such as Ben OTEC?
- Should public source refs be attached at order time by customers, only by
  operators, or both?
- Should the first implementation use only direct Exa API calls, or also attach
  Exa MCP to runner sessions for optional in-run follow-up search?
- Should raw Exa payloads ever be stored in private R2 for audit, or should the
  system store only normalized source cards?
- What is the production Exa budget for the public beta?

## Bottom Line

Adjutant should start projects with a grounded brief, not with a blank web
search impulse. Exa is a good fit if OpenAgents product surface wraps it as an Effect service with
typed config, typed responses, explicit source refs, bounded storage, operator
review, and clear projection policy. The integration should make Ben's OTEC
site more specific and more honest without weakening the existing explicit
assignment and privacy boundaries.
