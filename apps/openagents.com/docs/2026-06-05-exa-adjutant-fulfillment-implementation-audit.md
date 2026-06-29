# Exa Autopilot Fulfillment Implementation Audit

Date: 2026-06-05

Status: implementation audit and launch-readiness assessment. This document
does not change runtime policy, run Exa, dispatch the internal Adjutant
supervisor, approve a research brief, create a Site, launch Autopilot, deploy a
Site, or alter customer/order visibility by itself.

## Executive Summary

OpenAgents product surface has real Exa infrastructure in production. Exa is not just documented or
stubbed:

- `EXA_API_KEY` is decoded through the Worker config boundary and is now
  configured as a Cloudflare Worker secret for `openagents-autopilot`.
- The Worker has a typed Effect Exa client for `/search` and `/contents`.
- The internal Adjutant supervisor has a typed enrichment planner that turns an
  assignment, software order, Site, repository context, operator notes, and
  approved public source refs into bounded Exa tasks.
- The internal Adjutant supervisor has D1-backed enrichment ledgers for runs,
  queries, source cards, assignment/run links, budget events, cache entries,
  metric events, and research briefs.
- Operator APIs can plan, run, refresh, read, and review enrichment under
  `/api/operator/adjutant/assignments/:assignmentId/enrichment`.
- The admin UI has a Research panel that can run or refresh Exa-backed
  enrichment, show run/source counts, approve/reject source cards, and approve
  or reject research briefs.
- Task packet generation includes the latest approved research brief when one
  exists.
- Task packet freshness is recorded against the approved research brief used
  by the packet; newer approved research marks existing packets stale, and an
  operator can explicitly keep a packet current with a bounded customer-safe
  reason.
- Autopilot launch includes the approved research brief ID and summary in the
  launch selector when one exists.

Naming contract: `Adjutant` is the internal supervisor codename used in file
names, route paths, payload tags, and database identifiers. Product UI,
customer-safe projections, share pages, operator chrome, and public status copy
should call the capability `Autopilot`.

The important limitation is orchestration:

```text
Exa is wired into the operator-controlled Autopilot flow.
Exa is not automatically wired into customer order fulfillment yet.
```

Today the correct fulfillment sequence is:

```text
create/read Autopilot assignment
-> run Exa enrichment from operator/admin controls
-> review source cards and approve the research brief
-> generate the task packet
-> confirm task packet freshness or explicitly keep current
-> preflight
-> launch Autopilot
```

The missing product behavior is:

```text
customer order submitted
-> Autopilot assignment created
-> Exa enrichment automatically planned/run/review-queued
-> approved research context required or explicitly bypassed before launch
-> Autopilot starts with that context attached
```

That gap is orchestration and launch policy, not provider plumbing. The Exa
provider client, D1 persistence, operator APIs, review model, task packet
bridge, and launch selector bridge already exist.

## Source Scope

Reference audit used for structure:

- `docs/2026-06-05-openai-sites-parity-implementation-audit.md`

OpenAgents product surface evidence reviewed:

- `workers/api/src/config.ts`
- `workers/api/src/exa.ts`
- `workers/api/src/adjutant-enrichment-planner.ts`
- `workers/api/src/adjutant-enrichment-ledger.ts`
- `workers/api/src/adjutant-enrichment-operations.ts`
- `workers/api/src/adjutant-public-source-refs.ts`
- `workers/api/src/adjutant-research-briefs.ts`
- `workers/api/src/adjutant-task-packet-freshness.ts`
- `workers/api/src/adjutant-task-packets.ts`
- `workers/api/src/operator-adjutant-routes.ts`
- `workers/api/src/customer-orders.ts`
- `workers/api/src/onboarding/routes.ts`
- `workers/api/migrations/0038_exa_enrichment_ledger.sql`
- `workers/api/migrations/0039_adjutant_public_source_refs.sql`
- `workers/api/migrations/0040_adjutant_research_briefs.sql`
- `workers/api/migrations/0041_exa_enrichment_operations.sql`
- `workers/api/migrations/0055_adjutant_task_packet_freshness.sql`
- `apps/web/src/page/loggedIn/page/admin.ts`
- `apps/web/src/page/loggedIn/admin/transitions.ts`
- `docs/2026-06-05-ben-otec-exa-enrichment-runbook.md`
- `docs/2026-06-05-adjutant-sites-supervisor-audit.md`
- Exa-focused tests under `workers/api/src/*.test.ts` and admin scene tests.

Operational evidence from the deployment turn:

- `EXA_API_KEY` was uploaded as a Cloudflare Worker secret.
- Remote D1 migrations reported no pending migrations after applying the Exa
  ledger, public-source-ref, research-brief, and operations migrations.
- A live Exa smoke request for OTEC/SWAC/floating-datacenter context returned
  HTTP 200 with one result.
- `openagents-autopilot` was deployed successfully as Worker version
  `6c8037ac-83e3-4003-a5ba-2f483e1d39b6`.

No secret values or raw Exa payloads are recorded in this audit.

## Exa Capability Inventory

Current OpenAgents product surface Exa capabilities:

1. Decode Exa config from Cloudflare Worker environment.
2. Treat `EXA_API_KEY` as a redacted optional secret.
3. Default provider base URL to `https://api.exa.ai`.
4. Support `/search` and `/contents`.
5. Support Exa search types `auto`, `fast`, `instant`, `deep-lite`, `deep`,
   and `deep-reasoning`.
6. Support Exa categories including `github`, `people`, `personal site`,
   `linkedin profile`, `tweet`, `news`, `pdf`, `research paper`, and
   `company`.
7. Default search contents to highlights plus freshness max age.
8. Bound request timeout, retry count, rate-limit backoff, result count,
   highlight size, and text size.
9. Produce typed provider errors for unconfigured Exa, fetch failures, HTTP
   failures, invalid JSON, schema errors, and timeouts.
10. Redact provider/secret-shaped material from error summaries.
11. Build enrichment plans from assignment/order/Site context.
12. Include topic web search for the customer request or assignment objective.
13. Include GitHub repository search when a public repository is linked to the
    order or Site.
14. Include explicit approved public source refs as `/contents` and contextual
    search tasks.
15. Block unapproved, rejected, or internal-only source refs from enrichment.
16. Record enrichment runs, queries, and source cards in D1.
17. Link enrichment runs to Autopilot assignments with status and
    `required_for_launch`.
18. Record research briefs derived from public-safe approved source cards.
19. Store cache entries for bounded source results.
20. Reserve per-assignment and daily request budgets before running provider
    work.
21. Record cache hit/miss, success/failure, latency, result count, source-card
    count, cost, and error metrics.
22. Surface enrichment status in the admin Autopilot review UI.
23. Attach approved research briefs to task packets.
24. Attach approved research brief metadata to Autopilot launch selectors.
25. Derive an effective research policy for every Adjutant assignment from
    typed assignment kind, with durable operator overrides and explicit
    `research_bypassed_by_operator` records.
26. Surface research policy in operator assignment detail and preflight-safe
    responses without exposing private operator notes or raw Exa payloads.
27. Queue durable Adjutant enrichment jobs through
    `/api/operator/adjutant/assignments/:assignmentId/enrichment/enqueue`.
28. Prevent duplicate active enrichment jobs per assignment and create a queued
    Exa run/link as the durable execution receipt.
29. Consume `openagents-adjutant-enrichment-jobs` queue messages and execute
    the same policy-safe Exa task path used by manual enrichment.
30. Surface latest enrichment job status in assignment enrichment detail
    responses.
31. Record task packet freshness in D1 with task spec path, commit SHA,
    approved research brief ID, approval timestamp, source-card count, and
    freshness status.
32. Mark existing task packets stale when an operator approves newer research
    after packet generation.
33. Surface task packet freshness in operator assignment detail and preflight.
34. Allow an operator to keep a stale task packet current with bounded
    operator reason and customer-safe summary, while rejecting secret-shaped
    payloads.
35. Block research-required preflight and launch until approved research or an
    explicit operator bypass receipt exists.
36. Distinguish missing, queued/running, failed/unavailable, review-needed, and
    approved research states with redacted next actions.
37. Record durable `adjutant.preflight_blocked` and
    `adjutant.preflight_ready` events.

Missing or incomplete Exa capabilities:

1. No automatic enqueue on customer order submission.
2. No automatic enqueue on Autopilot assignment creation.
3. No automatic enqueue before task packet generation.
4. No automatic enqueue before launch.
5. No customer-visible explanation of pending/complete research.
6. No social account ingestion surface for customer-approved profile context.
7. No explicit OAuth/connect flow for X, LinkedIn, GitHub profile, personal
   site, or other public identity sources.
8. No customer-visible enrichment job projection yet.
9. No automatic committed-packet regeneration after stale detection; the
    operator still regenerates and pushes the replacement task packet.

## Current Implementation Map

### Worker Config

Current state: implemented and deployed.

Evidence:

- `decodeExaConfig` reads `EXA_API_KEY`, `EXA_BASE_URL`,
  `EXA_ASSIGNMENT_REQUEST_BUDGET`, `EXA_CACHE_TTL_HOURS`,
  `EXA_DAILY_REQUEST_BUDGET`, `EXA_DEFAULT_NUM_RESULTS`,
  `EXA_FRESHNESS_MAX_AGE_HOURS`, `EXA_MAX_HIGHLIGHT_CHARACTERS`,
  `EXA_MAX_TEXT_CHARACTERS`, `EXA_RATE_LIMIT_BACKOFF_MS`,
  `EXA_REQUEST_TIMEOUT_MS`, and `EXA_RETRY_LIMIT`.
- `enabled` is true only when the redacted API key exists.
- Defaults are conservative: base URL `https://api.exa.ai`, assignment budget
  12, daily budget 200, cache TTL 24 hours, freshness 24 hours, default
  result count 8, timeout 25 seconds, retry limit 2.

Assessment:

- This is production-grade enough for operator-run enrichment.
- It correctly treats the API key as a secret, not a `wrangler.jsonc` var.
- The remaining config gap is product policy: different assignment kinds may
  need different freshness, required-source, or launch-gate behavior.

### Exa Effect Client

Current state: implemented and test-covered.

Evidence:

- `workers/api/src/exa.ts` defines typed request and response schemas for
  search and contents calls.
- The client sends `POST` requests with `accept: application/json`,
  `content-type: application/json`, and `x-api-key`.
- Provider requests go to `${config.baseUrl}/search` or
  `${config.baseUrl}/contents`.
- `AbortSignal.timeout` is used when available.
- Provider HTTP errors summarize a redacted, bounded payload rather than
  logging raw provider bodies.

Assessment:

- The Exa client is a real Effect service boundary with injectable fetch.
- It is suitable for unit tests and live Worker use.
- It does not yet expose Exa through a broader agent tool registry; it is
  currently consumed directly by operator Adjutant routes.

### Enrichment Planner

Current state: implemented.

Evidence:

- `AdjutantEnrichmentPlanner` requires explicit assignment context:
  `softwareOrderId`, `siteId`, or `taskSpecPath`.
- The planner builds:
  - a topic web search task from the order request or assignment objective;
  - a GitHub repository search task when public repository context exists;
  - source-ref contents tasks for approved public refs;
  - source-ref search tasks for approved public refs.
- Private order repositories are excluded from repository planning.
- Source refs with `proposed`, `rejected`, or `internal_only` status are
  blocked and carried as explicit blocked-source policy decisions.
- People-style source refs can map to Exa's people category.

Assessment:

- The planner is deliberately bounded and context-driven.
- It avoids ad hoc keyword routing for selecting tools; the operator route
  explicitly chooses enrichment and the planner builds typed tasks from
  assignment data.
- The planner can support Ben-style OTEC enrichment and public profile/source
  enrichment, but only after those public source refs are provided and approved.

### D1 Enrichment Ledger

Current state: implemented and migrated.

Evidence:

- `exa_enrichment_runs` records assignment/order/Site linkage, plan ID,
  subject, status, request budget/counts, cache hits, source counts, approved
  source counts, cost, errors, and timestamps.
- `exa_enrichment_queries` records query text/hash, source category, search
  type, freshness, status, result count, latency, cost, and errors.
- `exa_enrichment_sources` records source cards with category, review status,
  title, URL, domain, published date, highlight, Exa request ID, search type,
  public-safe flag, and approval/rejection fields.
- `adjutant_assignment_enrichments` links assignments to enrichment runs and
  research briefs, including `required_for_launch`.

Assessment:

- The ledger is sufficient for operator review, launch context, and public-safe
  source selection.
- The `required_for_launch` flag exists, but current preflight treats missing
  or unapproved research as a warning. That makes it metadata today, not a
  hard launch gate.

### Budget, Cache, Metrics, And Retry

Current state: implemented and migrated.

Evidence:

- `exa_enrichment_budget_events` reserves request units per assignment/day.
- `reserveBudget` blocks requests that exceed assignment or daily budgets.
- `exa_enrichment_cache_entries` stores bounded source-result arrays with TTL.
- Existing active cache entries are archived before writing fresh cache.
- Cache payloads and metric payloads are scanned for secret-shaped material.
- `exa_enrichment_metric_events` records event name, status, error code, search
  type, source category, result counts, source-card counts, latency, cost, and
  cache status.
- Retry policy handles rate limit, 5xx, timeout, and fetch failures.

Assessment:

- The operational controls are strong enough for live operator use.
- The missing production control is asynchronous isolation: an enrichment run
  should eventually be a queued job/workflow rather than a synchronous admin
  POST when the task budget grows.

### Public Source Refs

Current state: implemented.

Evidence:

- Source ref kinds include GitHub repository, GitHub profile, personal site,
  LinkedIn profile, X profile, and generic URL.
- Source ref statuses include proposed, approved, rejected, internal-only, and
  public-safe.
- Private GitHub repositories are not accepted as public Exa source refs.

Assessment:

- This is the right shape for social/profile enrichment.
- It does not yet connect to social account providers or discover public refs
  automatically from customer OAuth/account connections.
- Operators can attach/approve refs; customers do not yet have a self-serve
  "use this profile/site/repo for grounding" path.

### Research Briefs

Current state: implemented.

Evidence:

- `adjutant_research_briefs` persists assignment ID, enrichment run ID, status,
  summary, grounded facts, suggested sections, unknowns, claims needing review,
  approved source cards, creator, approval/rejection timestamps, and update
  timestamps.
- Brief creation only includes source cards that are public-safe and approved
  or public-safe.
- The default section builder adds an OTEC/SWAC context section when the
  customer request includes OTEC.
- Briefs begin as `needs_review` unless explicitly created with another
  status.

Assessment:

- Research briefs are the main bridge from Exa data to Autopilot execution.
- The system intentionally requires human/operator review before those briefs
  become launch context.
- That is a good launch safety posture, but it means "Exa enrichment happened"
  is not the same as "Autopilot will use it." The brief must be approved.

### Operator APIs

Current state: implemented.

Evidence:

- `POST /api/operator/adjutant/assignments/:assignmentId/enrichment/plan`
  returns a typed plan and whether Exa is configured.
- `POST /api/operator/adjutant/assignments/:assignmentId/enrichment/run` runs
  selected Exa tasks.
- `POST /api/operator/adjutant/assignments/:assignmentId/enrichment/refresh`
  marks the latest brief stale before rerunning enrichment.
- `GET /api/operator/adjutant/assignments/:assignmentId/enrichment` reads the
  enrichment review state.
- Source-card and research-brief review routes approve/reject the material.

Assessment:

- This is a complete operator control plane for Exa-backed enrichment.
- It is not a background automaton. Every run/refresh is currently initiated
  through an operator/admin action.

### Admin UI

Current state: implemented enough for operator beta.

Evidence:

- The admin Autopilot review panel shows a Research section.
- It displays the enrichment status and `exa_configured` or `exa_missing`.
- It shows latest run ID, request budget/count, source count, and approved
  source count.
- It enables "Run research" or "Refresh research" when Exa is configured and
  no enrichment run is queued/running.
- Browser commands POST to `/api/operator/adjutant/assignments/:id/enrichment/run`
  or `/refresh`.
- The UI exposes source-card and brief review actions.

Assessment:

- Operators can use Exa today from the UI.
- Customers cannot use it directly from the order surface.
- The UI language is intentionally "Research" rather than exposing Exa as a
  product implementation detail.

### Preflight

Current state: hard gate for research-required assignments.

Evidence:

- Preflight checks include `research_policy`, `research_required_gate`,
  `exa_enrichment`, `research_brief`, `research_review`, and
  `task_packet_freshness`.
- When Exa is missing and the assignment requires research with no approved
  brief, preflight blocks and says Exa must be configured or a bounded bypass
  recorded.
- When no enrichment run exists for a research-required assignment, preflight
  blocks and instructs the operator to queue enrichment, review source cards,
  and approve a brief.
- When queued/running enrichment exists, preflight blocks with the safe job or
  run ID/status and a waiting next action.
- When enrichment failed, was rejected, or is unavailable, preflight blocks with
  redacted job/run state and a refresh next action.
- When a research brief is not approved for a research-required assignment,
  `research_required_gate` blocks until the brief is approved or an explicit
  bypass receipt exists.
- Optional and not-applicable assignment kinds do not block solely on research.
- The preflight route records durable `adjutant.preflight_blocked` or
  `adjutant.preflight_ready` events.

Assessment:

- This is strict enough for the launch-policy claim: research-required
  assignments cannot launch without approved research or explicit bypass.
- It still does not automatically start the enrichment job; orchestration
  remains separate work.

### Task Packet Bridge

Current state: implemented with freshness tracking.

Evidence:

- `generateTaskPacket` reads the latest approved research brief for the
  assignment.
- `buildAdjutantTaskPacket` rejects non-approved research briefs.
- Approved research is rendered into an "Approved Research Brief" section with
  summary, grounded facts, suggested sections, unknowns, claims needing review,
  and approved sources.
- The task packet remains a tracked Markdown file under
  `docs/autopilot-tasks`.
- `adjutant_task_packet_freshness` records the task spec path, commit SHA,
  approved research brief ID, approval timestamp, source-card count, and
  `current`, `stale`, or `kept_current` status.
- Approving a research brief marks any older generated task packet stale.
- Operator assignment detail and preflight expose task packet freshness without
  raw Exa payloads, provider-account material, or private operator notes.
- Operators can call
  `/api/operator/adjutant/assignments/:assignmentId/task-packet/keep-current`
  with a bounded reason and customer-safe summary when a stale packet is still
  acceptable.

Assessment:

- When research is approved before task packet generation, Autopilot receives
  that research in the task packet.
- If the task packet was generated before enrichment/approval, the packet is
  marked stale and the operator must regenerate it or explicitly keep it
  current with a bounded reason.
- There is no automatic committed-file regeneration workflow yet.

### Launch Selector Bridge

Current state: implemented.

Evidence:

- `launchAssignment` reads the latest approved research brief.
- `launchSelectorForAssignment` includes `Research brief ID` in the
  `dispatchGoal`.
- The launch selector includes a `researchBrief` object with ID, approved
  timestamp, source count, status, and summary when an approved brief exists.
- Launch events and Site events include the research brief ID when present.

Assessment:

- The Autopilot mission can receive approved Exa context at launch.
- Launch does not run Exa itself.
- Launch does not require an approved research brief today unless future
  policy turns the current warning into a blocker for selected assignment
  kinds.

### Customer Order Path

Current state: not automatically connected to Exa.

Evidence:

- `/api/customer-orders/active` requires a user session and calls
  `CustomerOrderStore.readOrCreateActiveOrder`.
- `/api/customer-orders/:orderId` reads an order by ID for the current user.
- The customer order projection shows order status, repository, Site status,
  Autopilot progress, and usage receipts.
- These routes do not call `planEnrichment`, `runEnrichment`, or any Exa
  service.
- They do not automatically create an Autopilot assignment.
- They do not automatically launch Autopilot.

Assessment:

- Customer order fulfillment and Exa enrichment are adjacent but not yet
  orchestrated together.
- An operator can connect them by creating/reading an assignment and running
  enrichment manually.
- The self-running fulfillment path still needs an assignment/enrichment
  orchestrator.

## Current Operator Flow

The current Exa-backed Autopilot sequence is:

1. Customer/order/Site context exists.
2. Operator creates or reads an Autopilot assignment.
3. Operator optionally attaches approved public source refs such as a GitHub
   repository, GitHub profile, personal site, LinkedIn profile, X profile, or
   generic URL.
4. Operator runs an enrichment plan or enrichment run.
5. The planner builds topic, repository, and source-ref tasks.
6. The route reserves budget.
7. The route creates an enrichment run and links it to the assignment.
8. The route executes selected Exa tasks sequentially.
9. Each task records a query and source cards, using cache when possible.
10. The run is marked succeeded, partial failure, or failed.
11. A research brief is created when the run is not failed.
12. The research brief starts in `needs_review`.
13. Operator reviews source cards and the research brief.
14. Operator approves the research brief.
15. Operator generates the task packet.
16. The task packet includes approved research.
17. Operator runs preflight.
18. Operator launches Autopilot.
19. The launch selector includes approved research metadata.

This is enough for a supervised beta. It is not enough for a claim that every
customer order is automatically enriched before fulfillment.

## Ben OTEC Status

Ben's OTEC/SWAC/floating-datacenter order is the canonical target for this
flow.

What exists:

- The planner can build a topic query from the order request.
- The planner can include public GitHub repository context if the order or Site
  has a public repository attached.
- The planner can include approved social/profile/personal-site source refs if
  an operator attaches them.
- The research brief generator has a specific OTEC/SWAC suggested-section
  default when the customer request includes OTEC.
- A live Exa smoke for the OTEC/SWAC/floating-datacenter topic succeeded.

What does not exist yet:

- Automatic lookup of Ben's GitHub repositories, profile pages, posts, or other
  social context from account connections.
- Customer consent and source-ref approval for using those accounts as
  grounding sources.
- Automatic enrichment at the moment the OTEC order is submitted.
- Automatic task packet regeneration after the brief is approved.

Launch verdict for Ben:

```text
Go for operator-supervised Exa enrichment before the OTEC Site launch.
No-go for claiming the OTEC order automatically enriches itself end to end.
```

## Status Matrix

| Capability                         | Current state           | Fulfillment status | Primary gap                                      |
| ---------------------------------- | ----------------------- | ------------------ | ------------------------------------------------ |
| Exa secret configured              | Deployed Worker secret  | Ready              | None, avoid leaking value                        |
| Live Exa provider call             | Smoke passed            | Ready              | Add scheduled/live smoke with redacted output    |
| Typed Exa client                   | Implemented             | Ready              | Not exposed as general agent tool                |
| Enrichment planner                 | Implemented             | Ready              | Needs product policy per assignment kind         |
| Public source refs                 | Implemented             | Operator-ready     | No customer/social connect flow                  |
| D1 run/query/source ledger         | Implemented             | Ready              | None for operator beta                           |
| Budget/cache/metrics               | Implemented             | Ready              | Async job isolation for larger runs              |
| Research briefs                    | Implemented             | Ready              | Requires operator approval                       |
| Admin Research panel               | Implemented             | Operator-ready     | Not customer-facing                              |
| Preflight Exa checks               | Hard gate by policy     | Ready              | Customer projection still missing                |
| Task packet research inclusion     | Implemented             | Conditional        | Requires approved brief before packet generation |
| Launch selector research inclusion | Implemented             | Conditional        | Launch does not trigger or require research      |
| Customer order auto-enrichment     | Missing                 | Not wired          | Need orchestration                               |
| Assignment auto-enrichment         | Missing                 | Not wired          | Need job/workflow                                |
| Social/profile enrichment          | Source-ref shape exists | Partial            | Need consent, discovery, and account refs        |

## Required Work To Fully Wire Exa Into Fulfillment

### 1. Fulfillment Enrichment Orchestrator

Add a service that owns the transition from order/assignment to enrichment:

- on software order submission or assignment creation, decide whether research
  is required;
- build an enrichment plan;
- enqueue or run an enrichment job;
- link the run to the assignment;
- notify the operator/admin surface when review is needed;
- avoid duplicate active enrichment runs.

This service should be typed and explicit. It should not infer fulfillment
intent from keyword matching.

### 2. Assignment Policy For Research Requirements

Define a policy such as:

- `research_required`;
- `research_optional`;
- `research_not_applicable`;
- `research_bypassed_by_operator`.

Policy should account for assignment kind:

- `general_order_fulfillment`;
- `site_generation`;
- `site_adjustment`;
- `site_review`;
- `site_deployment`.

The preflight checker should turn missing research from warning to blocker
only for assignment kinds that require research.

### 3. Queue Or Workflow Execution

Move enrichment execution off the synchronous operator POST for automatic
fulfillment:

- queue one enrichment job per assignment;
- keep budget reservation and duplicate-run checks;
- store job state and errors in D1;
- emit assignment events;
- keep manual operator run/refresh available as a control-plane action.

### 4. Customer And Operator Consent For Profile Sources

Add a source-ref intake flow:

- customer/operator proposes public profile or repository refs;
- customer grants or confirms use of public social/profile context;
- operator approves refs as public-safe;
- private repositories and private social data stay excluded unless a separate
  private-data policy is modeled.

This is the missing path for "Ben said one line about OTEC; look up more about
what he has already said or worked on." The current code can use approved
public refs, but it does not discover or authorize them automatically.

### 5. Regenerate Or Mark Task Packet After Approved Brief

When a brief becomes approved:

- detect whether the assignment already has a task packet; implemented;
- mark it stale when the approved brief is newer than the recorded packet
  context; implemented;
- allow an operator to keep it current with a bounded reason; implemented;
- regenerate the committed packet automatically; not implemented;
- update assignment `taskSpecPath` and `commitSha` only after the new packet is
  committed and validated; implemented for manual regeneration;
- record an assignment event for generation, stale marking, and keep-current;
  implemented.

The remaining gap is automatic regeneration and commit/writeback of a
replacement packet after stale detection.

### 6. Launch Gate Or Explicit Bypass

For research-required assignments:

- block launch when no approved brief exists;
- allow an explicit operator bypass only with a bounded reason;
- record that bypass in assignment events and preflight output;
- include bypass status in launch selector metadata.

This turns current warning-level Exa checks into real fulfillment policy.

### 7. Customer-Safe Research Projection

Add a customer-safe order projection:

- research status: pending, reviewing, approved, bypassed, unavailable;
- source count and approved source count;
- no raw Exa queries;
- no provider payloads;
- no private profile data;
- no secret-shaped content.

### 8. Operational Smokes

Add a recurring or operator-run smoke that verifies:

- `EXA_API_KEY` is configured;
- Exa `/search` responds to a bounded harmless query;
- no key or raw provider payload is printed;
- D1 migrations are present;
- cache/budget/metric writes are healthy.

## Immediate Recommendation

For the next launch window, describe the product as:

```text
Autopilot can use operator-reviewed Exa research to ground Sites/order
fulfillment before Autopilot launch.
```

Do not describe it as:

```text
Every customer order is automatically enriched from Exa and social context
before fulfillment starts.
```

The correct near-term OTEC path is:

1. Confirm the OTEC software order and Site assignment.
2. Attach any approved public Ben/source refs that should ground the work.
3. Run Exa research from the admin Autopilot Research panel.
4. Review source cards and approve the research brief.
5. Generate or regenerate the task packet.
6. Run preflight.
7. Launch Autopilot.
8. Verify the task packet and launch selector both carry the approved research
   brief ID.

## Completion Standard For Fully Wired Fulfillment

Exa is fully wired into Autopilot order fulfillment only when all of the
following are true:

- Customer order submission or assignment creation can automatically schedule
  enrichment when policy requires it.
- Enrichment executes asynchronously with budget, cache, metrics, duplicate-run
  prevention, and typed failure state.
- Social/profile/public-source refs are customer-confirmed or operator-approved
  before use.
- Research-required assignment kinds cannot launch without an approved brief
  or explicit operator bypass.
- Approved research automatically updates or invalidates task packets.
- Launch selectors and task packets consistently carry the approved brief.
- Customer/public projections expose only safe research status, not raw Exa
  provider payloads, private data, or secrets.
- Tests cover order-to-assignment-to-enrichment-to-brief-to-packet-to-launch.

Until then, the honest status is:

```text
Exa is production-configured and operator-wired into Autopilot.
Automatic Exa-first customer order fulfillment remains to be implemented.
```
