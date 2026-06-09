# Adjutant Sites Supervisor Audit

Date: 2026-06-05

Status: audit and implementation plan only. This document does not create an
agent, change runtime policy, launch a run, expose a public route, or alter
customer/order/site visibility by itself.

## Executive Summary

OpenAgents product surface now has the pieces for a real customer software-ordering system and an
Autopilot Sites lifecycle, but the handoff from a submitted order to a durable,
supervised site-building run is still mostly an operator action.

The right next implementation shape is an internal supervisor codenamed
**Adjutant**, exposed in product UI as **Autopilot**:

```text
agent_adjutant
  -> OpenAgents Core-owned Autopilot supervisor
  -> specializes in customer software-order fulfillment and Sites delivery
  -> creates or continues durable goals
  -> launches site-building Autopilot runs
  -> keeps order, site, version, deployment, and observer projections current
  -> publishes only sanitized public/customer activity
```

Adjutant should not be a separate implementation substrate. It should be a
specialized version of Autopilot, like Artanis is already a specialized public
Autopilot campaign agent, but constrained around customer orders, site builds,
site adjustments, release review, and public/customer/team visibility.

The most important design rule: **the internal Adjutant supervisor is
explicitly assigned by core team members or typed operator actions.** Do not
infer assignment from customer prompt keywords such as "website", "site",
"landing page", or "make adjustments". Customer text can become the objective
only after an explicit order/site assignment route has selected Autopilot.

## Source Material Reviewed

Docs and task packets:

- `docs/sites.md`
- `docs/sites-plan.md`
- `docs/autopilot-tasks/AGENTS.md`
- `docs/autopilot-tasks/2026-06-04-programmatic-autopilot-operator-runbook.md`
- `docs/autopilot-tasks/2026-06-04-r10-pylon-campaign-continuation.md`
- `docs/autopilot-tasks/2026-06-04-cloudflare-containers-runner-backup-implementation.md`
- `docs/autopilot-tasks/done/2026-06-04-customer-software-ordering-flywheel.md`
- `docs/2026-06-04-programmatic-autopilot-work-runbook-audit.md`
- `docs/2026-06-04-cloudflare-containers-runner-backup-audit.md`
- `docs/2026-06-04-openai-codex-goal-implementation-audit.md`
- `AGENTS.md`
- `INVARIANTS.md`

Implementation surfaces:

- `workers/api/migrations/0023_team_projects.sql`
- `workers/api/migrations/0024_project_agent_metadata.sql`
- `workers/api/migrations/0027_agent_goals.sql`
- `workers/api/migrations/0028_agent_goal_events.sql`
- `workers/api/migrations/0030_software_orders.sql`
- `workers/api/migrations/0032_autopilot_sites.sql`
- `workers/api/src/customer-orders.ts`
- `workers/api/src/sites.ts`
- `workers/api/src/operator-sites-routes.ts`
- `workers/api/src/agent-goal-runtime.ts`
- `workers/api/src/agent-goal-public-projection.ts`
- `workers/api/src/agent-goal-routes.ts`
- `workers/api/src/omni-handlers.ts`
- `workers/api/src/omni/dispatch-service.ts`
- `workers/api/src/team-chat.ts`
- `apps/web/src/route.ts`
- `apps/web/src/page/loggedOut/update.ts`
- `apps/web/src/page/loggedOut/page/publicAgent.ts`
- `apps/web/src/page/loggedIn/team-chat/transitions.ts`
- `apps/web/src/page/loggedIn/page/admin.ts`
- `apps/web/src/page/loggedIn/page/order.ts`

## How OpenAgents product surface Is Launching Software Orders Today

The current customer ordering wedge is intentionally simple:

1. A non-core GitHub-authenticated customer chooses or enters a repository.
2. The customer writes the requested outcome in plain language.
3. The customer acknowledges public beta work, data-use, and OpenAgents-paid
   compute terms.
4. OpenAgents product surface creates or reads an active `software_orders` row.
5. The customer lands on `/order` and sees order status, quote/free-slice
   information, and eventual Site URL only.
6. OpenAgents Core members keep the full operator workroom, team/project chat,
   run streaming, files, diagnostics, provider state, callback retry, and
   continuation controls.

The D1 order table already captures the current beta contract:

- `software_orders.status` supports submitted, scoping, free-slice, quote,
  agent-queued, agent-running, delivered, needs-input, declined, and unavailable
  states.
- `software_orders.visibility` is currently `public` only.
- `software_orders.current_run_id` can point at an Autopilot run.
- The customer projection joins a linked `site_projects` row and active
  deployment URL, but exposes only `site.status` and `site.activeUrl`.

The customer-order docs are clear that customers should not see runner streams,
provider-account refs, SHC state, callback payloads, private diagnostics, raw
shell logs, private prompts, or internal delivery mechanics.

## How OpenAgents product surface Launches Autopilot Runs Today

The current Autopilot launch contract is packet-driven and preflight-gated:

1. Write a task packet under `docs/autopilot-tasks/`.
2. Commit and push the packet and prerequisite specs.
3. Run operator preflight/checklist for target user/team/project/run.
4. Resolve provider reconnect, GitHub writeback, SHC health, callback config,
   migration state, and callback lag before dispatch.
5. Create or reuse a durable `agent_goals` row.
6. Launch a queued `agent_runs` row through the SHC dispatch path.
7. Monitor Cloudflare/D1 state first.
8. Retry callbacks by run ID when needed.
9. Continue the same durable goal instead of launching unrelated duplicate
   runs.

The team-chat UI has a bounded `@autopilot` parser. It accepts:

- leading `@autopilot <prompt>`;
- a standalone `@autopilot` line with surrounding prompt text;
- trailing `<prompt> @autopilot`.

That parser is intentionally not a loose keyword matcher. Adjutant should follow
the same discipline: explicit command or explicit operator action first,
deterministic parsing second.

## How Autopilot Sites Works Today

The Sites implementation now has a real lifecycle authority:

- `site_projects` links to `software_orders`, owner user, team/project, slug,
  prompt, status, access mode, visibility, source repo, active version, and
  active deployment.
- `site_versions` records source kind, source archive refs, artifact manifest,
  build log refs, build status, worker module refs, static manifest, binding
  names, metadata, creating user, creating run, and saved/rejected timestamps.
- `site_deployments` records runtime kind, script/dispatch metadata, URL,
  status, actor, timestamps, and rollback/disable state.
- `site_environment_values`, `site_access_grants`, `site_storage_bindings`,
  and `site_events` cover governance and audit.

The operator API can:

- create a Site from a software order;
- read/list Sites;
- update access and visibility;
- request generation;
- save versions;
- deploy versions;
- manage environment values;
- grant access;
- list events;
- disable and rollback deployments.

The public runtime can serve `sites.openagents.com/<slug>` from either:

- `openagents_static_r2`; or
- `workers_for_platforms` dispatch metadata.

Important current gap:

`AutopilotSitesService.requestGeneration` builds a clean generation packet,
marks the Site `generating`, and records a `site_generation.requested` event.
It does **not** yet create or continue an `agent_goals` row, dispatch a real
Autopilot run, store a task packet path/commit SHA, update the software order
run pointer, or attach generated runner artifacts back into the Sites version
lifecycle.

That is the natural home for Adjutant.

## What Artanis Already Proves

Artanis is the closest working pattern for Adjutant.

Existing Artanis shape:

- `project_artanis` is seeded under `team_openagents_core`.
- `project_artanis` metadata carries `agent_artanis`, name, status, runtime,
  backend, repository, and focus.
- `/artanis` maps to `PublicAgentRoute({ agentRef: 'artanis' })`.
- `/agents/artanis` is the canonical public-agent route.
- Public routes load `agent_artanis` current public goal from
  `/api/public/agents/agent_artanis/current-goal`.
- `agent_goals.visibility = 'public'` gates public projection.
- `agent_goal_events` are projected through
  `AgentPublicProjectionService`.
- Public event projection sanitizes refs and rejects provider/secret-shaped
  material.
- The Artanis task packet declares explicit launch fields:
  `agentId`, `projectId`, `teamId`, `visibility`, `taskSpecPath`,
  repository, base ref, and product goal.

Artanis is not a general customer fulfillment agent. Its current public page is
Pylon-specific and the logged-out public agent view still loads Pylon stats for
public agent pages. The Adjutant design should reuse the durable goal and safe
projection machinery, but it needs a different public page and a different
activity vocabulary.

## Adjutant Definition

Adjutant is an OpenAgents Core-owned Autopilot supervisor for customer order
fulfillment, with Sites as the first product lane.

Naming contract:

- `Adjutant`, `agent_adjutant`, `project_adjutant`, `adjutant_assignments`, and
  `adjutant.*` event names are internal implementation identifiers.
- User-facing product surfaces should call this surface `Autopilot`, including
  public agent pages, customer order progress, operator review panels, share
  pages, generated summaries, and GitHub-facing task/PR copy.
- URLs and API routes may continue to use `/adjutant` as compatibility and
  implementation paths, but the rendered UI should not present `Adjutant` as a
  product name.
- This naming contract supersedes older implementation-plan issue headings that
  used `Adjutant` as shorthand for the customer-visible Autopilot surface.

Canonical identity:

```text
teamId: team_openagents_core
projectId: project_adjutant
agentId: agent_adjutant
public route: /adjutant
canonical public route: /agents/adjutant
primary product lane: Autopilot Sites
default repository: OpenAgentsInc/openagents
default baseRef: main
```

Role:

- Convert approved customer software orders into supervised Autopilot work.
- Build and adjust Sites through the Sites lifecycle.
- Maintain one durable goal per active order/site fulfillment thread unless an
  operator explicitly splits the work.
- Launch or continue Autopilot runs after preflight passes.
- Keep `software_orders`, `site_projects`, `site_versions`,
  `site_deployments`, `site_events`, `agent_goals`, and `agent_runs`
  synchronized enough for observers.
- Publish public/customer/team progress without exposing private delivery
  mechanics.
- Escalate blockers through typed statuses instead of pretending work is
  complete.

Non-role:

- Adjutant does not replace Artanis.
- Adjutant does not become a loose natural-language router.
- Adjutant does not bypass the operator preflight/checklist path.
- Adjutant does not directly expose raw runner streams to customers.
- Adjutant does not deploy public Sites without the Sites launch checklist.
- Adjutant does not store secrets, provider grants, callback tokens, hidden
  steering, private prompts, or shell logs in public/customer projections.

## Why Adjutant Should Be A Specialized Autopilot Agent

The repo already has the right primitives:

- durable goals;
- goal tool context and hidden steering;
- team/project agent metadata;
- public goal projection;
- public route support;
- team chat launch;
- operator preflight/checklist;
- run continuation;
- callback retry;
- software order rows;
- Sites lifecycle rows;
- public/customer/operator visibility split.

Adjutant should therefore be implemented as:

```text
agent profile + project metadata
  -> typed order/site assignment service
  -> durable goal creation/continuation
  -> Autopilot run dispatch using existing run machinery
  -> Sites event and version/deployment integration
  -> public/customer/team/operator projections
```

This keeps the system inside OpenAgents product surface's current Autopilot architecture instead of
creating a separate agent runtime.

## Visibility Model

Adjutant needs four different projections.

### Operator-Private

Audience: OpenAgents Core admins and authorized operators.

Can show:

- full order and Site lifecycle;
- customer identity and repo metadata;
- preflight/checklist state;
- provider reconnect requirement;
- GitHub writeback readiness;
- run IDs and internal statuses;
- version/build/deployment governance;
- safe build log links;
- disable/rollback controls;
- callback retry and continuation actions.

Must not show secret values. Secret refs are acceptable only in operator-safe
contexts.

### Team-Visible

Audience: OpenAgents Core team/project workrooms.

Can show:

- Adjutant goal objective;
- team/project chat messages;
- run summary;
- current run status;
- sanitized events;
- commit/PR/artifact/deployment links that are safe for the team.

Should not show provider grants, callback tokens, raw runner payloads, or raw
private prompt material.

### Customer-Visible

Audience: owning customer.

Can show:

- order status;
- Site status;
- active URL;
- expected next action;
- whether customer input is needed;
- quote/free-slice/payment state when backed by real services.

Must not show:

- raw runner logs;
- provider refs;
- SHC/GCloud/Container details;
- callback state;
- private build logs;
- Cloudflare deployment internals;
- hidden prompts;
- shell output.

### Public

Audience: anonymous visitors.

Can show:

- Adjutant's public objective;
- selected public order/site milestones;
- public Site URLs;
- sanitized activity summaries;
- safe commit/PR/artifact/receipt refs;
- aggregate progress counts.

Must not show:

- private customer data;
- private repositories;
- non-public order details;
- raw logs or payloads;
- provider/credential-shaped material;
- internal dispatch mechanics.

## Proposed Product Flow

### Order To Site

1. Customer submits a software order.
2. Core operator opens admin order queue.
3. Operator clicks `Assign to Adjutant` or tags a project room with a bounded
   `@adjutant` command that includes an explicit `softwareOrderId` or `siteId`.
4. OpenAgents product surface creates or finds a `site_projects` row when the order is a Site order.
5. OpenAgents product surface creates or finds an Adjutant durable goal scoped to
   `team_openagents_core`, `project_adjutant`, and `agent_adjutant`.
6. OpenAgents product surface writes an Adjutant assignment receipt linking:
   - `softwareOrderId`;
   - `siteId`;
   - `goalId`;
   - `runId`, when launched;
   - `taskSpecPath`;
   - `commitSha`;
   - visibility policy.
7. Operator preflight runs.
8. If preflight passes, OpenAgents product surface launches the run.
9. If preflight blocks, OpenAgents product surface records a typed blocker and shows the next safe
   action.

### Site Generation

1. Adjutant receives a Sites generation packet.
2. The run creates source, assets, tests, build output, and version manifest.
3. Runner callbacks and receipts map into `agent_goal_events` and `site_events`.
4. The Sites service saves a reviewable `site_versions` row.
5. Operator reviews source/build/audience/secrets/URL.
6. Operator deploys the saved version after checklist completion.
7. Customer sees Site status and active URL.
8. Public Adjutant page can show the milestone if the order/site is public.

### Adjustments

1. Customer or operator requests an adjustment.
2. Request is recorded as a typed order/site adjustment, not a free-floating
   chat instruction.
3. Adjutant continues the same durable goal when the adjustment is within the
   original fulfillment thread.
4. A new run is launched or a running SHC job receives a continuation.
5. New artifacts become a new saved version.
6. Operator deploys or rejects the version.

## Required Data Model Additions

The existing tables are close, but Adjutant needs explicit assignment
authority.

Recommended table:

```text
adjutant_assignments
```

Fields:

- `id`
- `software_order_id`
- `site_id`
- `goal_id`
- `current_run_id`
- `team_id`
- `project_id`
- `agent_id`
- `assigned_by_user_id`
- `assignment_kind`
- `status`
- `visibility`
- `task_spec_path`
- `commit_sha`
- `objective`
- `created_at`
- `updated_at`
- `completed_at`
- `blocked_at`
- `archived_at`

Suggested `assignment_kind` values:

- `site_generation`
- `site_adjustment`
- `site_review`
- `site_deployment`
- `general_order_fulfillment`

Suggested `status` values:

- `draft`
- `preflight_pending`
- `blocked`
- `queued`
- `running`
- `review_needed`
- `deployed`
- `delivered`
- `complete`
- `canceled`

Why this table matters:

- `site_projects` should remain Site lifecycle authority.
- `software_orders` should remain customer order authority.
- `agent_goals` should remain durable goal authority.
- `agent_runs` should remain run authority.
- Adjutant needs the cross-object assignment receipt that explains why those
  rows are connected.

## API Surface

Operator-only:

```text
POST /api/operator/adjutant/assignments
GET  /api/operator/adjutant/assignments
GET  /api/operator/adjutant/assignments/:assignmentId
POST /api/operator/adjutant/assignments/:assignmentId/preflight
POST /api/operator/adjutant/assignments/:assignmentId/launch
POST /api/operator/adjutant/assignments/:assignmentId/continue
POST /api/operator/adjutant/assignments/:assignmentId/cancel
POST /api/operator/adjutant/orders/:softwareOrderId/assign
POST /api/operator/adjutant/sites/:siteId/assign
POST /api/operator/adjutant/sites/:siteId/adjustments
```

Customer-safe:

```text
GET /api/customer-orders/:orderId/adjutant
GET /api/customer-orders/:orderId/site
```

Public:

```text
GET /api/public/agents/agent_adjutant/current-goal
GET /api/public/adjutant/activity
```

The public route can initially reuse
`/api/public/agents/:agentId/current-goal`, but Adjutant will eventually need
an activity endpoint that joins public-safe order/site milestones.

## Public Route

Adjutant should have:

```text
https://openagents.com/adjutant
https://openagents.com/agents/adjutant
```

The current public agent page is too Artanis/Pylon-specific. It should either:

1. become a generic public-agent page with agent-specific modules; or
2. add a dedicated Adjutant public page.

Adjutant page sections:

- current public Adjutant goal;
- current public fulfillment run, when safe;
- public order/site milestones;
- deployed public Site links;
- sanitized activity;
- aggregate counts such as active Sites, delivered Sites, review-needed Sites.

Do not show raw workroom streams or private runner details.

## Command And Tag Model

Support explicit core-team operation through:

```text
@adjutant <instruction>
```

Bounded forms should mirror `@autopilot`:

- leading `@adjutant <prompt>`;
- standalone `@adjutant` line with surrounding prompt text;
- trailing `<prompt> @adjutant`.

But unlike ordinary team chat, Adjutant launch should require one of:

- explicit `softwareOrderId`;
- explicit `siteId`;
- explicit selected order/site from UI state;
- explicit task packet path.

Do not parse customer prompt text to decide which order or Site to include.

## Assignment Context

Every Adjutant run should receive typed context:

```json
{
  "schemaVersion": "openagents.adjutant_assignment.v1",
  "assignmentId": "...",
  "softwareOrderId": "...",
  "siteId": "...",
  "goalId": "...",
  "agentId": "agent_adjutant",
  "teamId": "team_openagents_core",
  "projectId": "project_adjutant",
  "visibility": "public",
  "site": {
    "slug": "otec",
    "targetUrl": "https://sites.openagents.com/otec",
    "status": "generating"
  },
  "customerSafeRules": [
    "Do not expose raw runner logs.",
    "Do not expose provider refs.",
    "Do not expose private prompts."
  ],
  "outputContract": [
    "Produce reviewable site source.",
    "Produce build artifacts.",
    "Produce a Sites version manifest."
  ]
}
```

This context should be part of the runner assignment or task packet metadata,
not improvised in untracked chat.

## Event Vocabulary

Adjutant should publish typed events in addition to generic goal events.

Suggested event types:

- `adjutant.assignment_created`
- `adjutant.preflight_passed`
- `adjutant.preflight_blocked`
- `adjutant.run_queued`
- `adjutant.run_started`
- `adjutant.run_failed`
- `adjutant.run_completed`
- `adjutant.customer_input_needed`
- `adjutant.review_requested`
- `adjutant.version_saved`
- `adjutant.deployment_activated`
- `adjutant.deployment_disabled`
- `adjutant.adjustment_requested`
- `adjutant.adjustment_completed`

Mapping rules:

- Public projection may show status, summaries, safe links, and public Site
  URLs.
- Customer projection may show order/site status and next action.
- Team projection may include richer run summary.
- Operator projection may include full lifecycle and controls.
- No projection may include secrets or provider/callback payloads.

## Supervisor Policy

Adjutant should enforce these policies:

- Use the current operator preflight/checklist before launch.
- Do not launch without provider-account and GitHub-write readiness for the
  target operator account.
- Continue the same durable goal for related adjustments.
- Create a new goal only for a materially new order/site objective.
- Treat public deployments as checklist-gated.
- Treat generated dynamic Worker code as untrusted until reviewed.
- Mark Site status `needs_review` after environment changes on an active Site.
- Record typed blockers when provider, runner, repo, callback, build, or
  deployment readiness fails.
- Never widen customer or public access as a side effect of generation.
- Never infer Site assignment from unstructured customer text.

## Current Gaps

1. There is no `project_adjutant` or `agent_adjutant` seed.
2. There is no `/adjutant` route.
3. Public agent routing has a hardcoded Artanis mapping and the public agent
   page loads Pylon stats by default.
4. There is no Adjutant assignment table or service.
5. `requestGeneration` creates a packet but does not dispatch a run.
6. Sites generation events are not linked to durable goals.
7. `software_orders.current_run_id` is not consistently updated from Sites
   generation and adjustment activity.
8. There is no customer-safe Adjutant/order progress projection.
9. There is no public Adjutant activity projection over Site milestones.
10. The admin Sites UI has a generate button, but save/deploy/rollback/disable
    are still limited from the overview screen.
11. There is no typed `@adjutant` command.
12. There is no automatic task-packet writer or packet-path validator for
    order/site assignments.
13. Runner dispatch is still SHC-primary and SHC-shaped; Cloudflare Containers
    remains a queued backup-runner implementation.
14. Generated artifacts are not automatically converted into saved Site
    versions through a trusted receipt path.
15. Customer adjustment requests are not modeled as first-class continuation
    inputs.
16. Email/customer notification remains a follow-up unless routed through the
    approved email ledger.

## Ordered Implementation Issues

Create and implement these issues in order.

### 1. Adjutant: add source authority, identity, and project metadata

Deliverables:

- Add this audit as the source authority.
- Add a D1 migration seeding `project_adjutant` under
  `team_openagents_core`.
- Add project metadata for `agent_adjutant` with name `Autopilot`, runtime
  `Autopilot`, backend `SHC`, repository `openagents`, and focus
  `Sites`.
- Add tests that preflight can resolve the Adjutant team/project/agent
  metadata.

Acceptance:

- `project_adjutant` exists and is active.
- `agent_adjutant` metadata is readable by existing operator preflight.

Implementation note, June 5, 2026:

- Issue #66 added migration `0033_adjutant_project_metadata.sql`, seeding
  `project_adjutant` under `team_openagents_core` with complete
  `agent_adjutant` metadata.
- The team repository metadata test now covers Adjutant's exact agent metadata
  shape used by the operator preflight `team_project_agent` check, with the
  public display name set to `Autopilot`.

### 2. Adjutant: add generic public-agent routing for `/adjutant`

Deliverables:

- Map `/adjutant` to `PublicAgentRoute({ agentRef: 'adjutant' })`.
- Map `adjutant` to `agent_adjutant` without hardcoding only Artanis.
- Keep `/artanis` behavior intact.
- Stop loading Pylon stats for every public agent page; make Pylon stats an
  Artanis-specific module.
- Add public route tests for `/adjutant` and `/agents/adjutant`.

Acceptance:

- Anonymous users can open `/adjutant`.
- The route does not request auth bootstrap.
- The page shows Autopilot public goal fallback if no public goal exists.
- Artanis public page still shows Pylon stats.

Implementation note, June 5, 2026:

- Issue #67 added `/adjutant` as a short public route beside the existing
  `/agents/adjutant` canonical route.
- Public agent startup now maps Adjutant to `agent_adjutant` and loads Pylon
  stats only for Artanis.
- The public-agent page now has an Autopilot fallback objective while
  preserving the Artanis Pylon campaign module.

### 3. Adjutant: add assignment ledger and service

Deliverables:

- Add `adjutant_assignments` migration.
- Add Schema-first domain/service module.
- Link software order, Site, goal, current run, team, project, agent, task
  spec path, commit SHA, objective, assignment kind, status, and visibility.
- Add tests for create/read/update, unique active assignment per order/site
  where appropriate, and no secret-shaped payloads.

Acceptance:

- Assignments can be created from a `softwareOrderId` or `siteId`.
- Assignment rows do not store secrets, provider grants, callback tokens, or
  raw runner payloads.

Implementation note, June 5, 2026:

- Issue #68 added `adjutant_assignments` as the cross-object assignment
  ledger linking software orders, Sites, goals, runs, team/project scope,
  agent identity, task packets, commits, objective, status, and visibility.
- `AdjutantAssignmentService` can create assignments from a software order or
  Site, infer the order from a Site, update lifecycle pointers, list active
  assignments, block duplicate active work, and reject secret-shaped payloads.

### 4. Adjutant: add operator assignment APIs

Deliverables:

- `POST /api/operator/adjutant/orders/:softwareOrderId/assign`.
- `POST /api/operator/adjutant/sites/:siteId/assign`.
- `GET /api/operator/adjutant/assignments`.
- `GET /api/operator/adjutant/assignments/:assignmentId`.
- Admin-only authorization.
- Typed errors for missing order/site, duplicate active assignment, invalid
  visibility, and unsafe payload.

Acceptance:

- Core operators can assign an order or Site to Adjutant.
- Non-core users cannot call the operator APIs.
- Assignment creation records a safe event.

Implementation note, June 5, 2026:

- Issue #69 added an admin-only operator Adjutant API for assigning software
  orders or Sites, plus assignment list and detail reads.
- Assignment creation now writes a sanitized `adjutant.assignment_created`
  event to `adjutant_assignment_events`, keeping Adjutant lifecycle events
  separate from Site and goal authority until those objects are linked.
- Route errors are typed for unauthorized, forbidden, missing order/Site,
  duplicate active assignment, invalid visibility, unsafe payload, validation,
  and storage failures.

### 5. Adjutant: add durable goal creation and continuation policy

Deliverables:

- Create or reuse an `agent_goals` row scoped to
  `agent_adjutant/project_adjutant/team_openagents_core`.
- Link goal ID into `adjutant_assignments`.
- Preserve goal identity for adjustments to the same order/site.
- Add typed blocker behavior when a stopped run has no goal.

Acceptance:

- Assigning a Site creates or reuses a durable Adjutant goal.
- Related adjustments continue the same goal unless an operator explicitly
  splits the work.

Implementation note, June 5, 2026:

- Issue #70 made `AdjutantAssignmentService` create or reuse the current
  `agent_adjutant` goal scoped to `team_openagents_core/project_adjutant`
  when an assignment is created, then link that goal ID into
  `adjutant_assignments`.
- Explicit goal IDs are verified before assignment insertion, completed
  current goals are replaced through the existing `agent_goals` authority, and
  non-complete current goals are reused for related Site follow-up work.
- Assignment updates now reject run-linked rows that would leave a stopped or
  continued run without a durable goal, returning a typed `run_goal_required`
  blocker through the operator API.

### 6. Adjutant: add assignment preflight and launch checklist

Deliverables:

- `POST /api/operator/adjutant/assignments/:assignmentId/preflight`.
- Reuse the current operator Autopilot checklist checks.
- Include Site/order-specific checks: software order exists, Site exists when
  required, access mode, public launch checklist requirement, source repo, task
  packet path, commit SHA, and active deployment state.
- Return next safe action.

Acceptance:

- Provider reconnect, GitHub writeback, SHC health, callback config, migration
  state, and target project/agent state block launch before dispatch.
- Public deployment work cannot skip Sites launch checklist requirements.

Implementation note, June 5, 2026:

- Issue #71 added
  `POST /api/operator/adjutant/assignments/:assignmentId/preflight`.
- Adjutant preflight now reuses the existing operator Autopilot preflight
  payload for database migration, team/project/agent, provider reconnect,
  GitHub writeback, SHC health, and callback configuration checks.
- The Adjutant layer adds assignment-specific checks for durable goal linkage,
  assignment state, software order, Site, source repository, task packet path,
  commit SHA, Sites launch checklist, and active deployment state.
- Preflight updates assignments to `preflight_pending` when launch is safe, or
  `blocked` when any inherited or Adjutant-specific check blocks launch, and
  returns the next safe action.

### 7. Adjutant: generate tracked task packets for Site work

Deliverables:

- Add a task-packet template for Adjutant Site generation and adjustment work.
- Write packets under `docs/autopilot-tasks/` or a future generated packet
  lane, with safe content only.
- Record `taskSpecPath` and commit SHA on the assignment before launch.
- Validate that the packet exists at the pushed commit SHA.

Acceptance:

- Adjutant launches do not depend on hidden chat context.
- Packets include order/site IDs, target URL, output contract, safety rules,
  and acceptance criteria.
- Packets exclude secrets and private delivery mechanics.

Implementation note, June 5, 2026:

- Issue #72 added a tracked Adjutant Site task packet template under
  `docs/autopilot-tasks/` and a pure packet builder for assignment-specific
  Markdown.
- The operator API now supports
  `POST /api/operator/adjutant/assignments/:assignmentId/task-packet` to
  generate safe packet Markdown, validate the `docs/autopilot-tasks/*.md` path
  and pushed commit SHA, confirm the packet exists in
  `OpenAgentsInc/openagents` at that SHA through GitHub raw content, and
  record `taskSpecPath` plus `commitSha` on the assignment.
- Generated packets include assignment, order, Site, goal, target URL, output
  contract, safety rules, and acceptance criteria, while rejecting
  secret-shaped content.

### 8. Adjutant: launch Autopilot runs from assignments

Deliverables:

- `POST /api/operator/adjutant/assignments/:assignmentId/launch`.
- Build a typed runner selector from assignment, goal, Site, order, packet, and
  operator preflight result.
- Queue `agent_runs` through the existing Omni launch machinery.
- Update `adjutant_assignments.current_run_id`,
  `software_orders.current_run_id`, and relevant Site event actor/run refs.
- Notify sync scopes.

Acceptance:

- Launch creates one queued run linked to the Adjutant goal and assignment.
- Dispatch failures record typed assignment and goal events.
- Provider/GitHub readiness failures return typed pre-dispatch blockers.

Implementation note, June 5, 2026:

- Issue #73 added
  `POST /api/operator/adjutant/assignments/:assignmentId/launch`.
- Launch reruns operator Autopilot preflight plus Adjutant source checks before
  dispatch. Blocked preflight returns `adjutant_launch_blocked`, records an
  `adjutant.launch_blocked` assignment event, and does not call the runner.
- Passing launches build an Omni selector from the assignment, durable goal,
  software order, Site, task packet path, packet commit SHA, target URL, and
  preflight summary, then delegate queued run creation and SHC dispatch to the
  existing Omni launch machinery.
- Successful launch records `adjutant_assignments.current_run_id`, moves the
  assignment to `queued`, updates `software_orders.current_run_id` and
  `software_orders.status = agent_queued`, and records a Site event with
  `actor_run_id`.
- Omni pre-dispatch failures such as provider or GitHub write readiness return
  their typed blocker responses and record `adjutant.launch_blocked`.
- Dispatch failures after run creation record `adjutant.dispatch_failed`, link
  the assignment to the created run, mark the assignment `blocked`, and mark
  the software order `unavailable`. Goal dispatch failure accounting remains
  owned by Omni's existing run-event to goal-event path.

### 9. Adjutant: map runner callbacks into order and Site lifecycle

Deliverables:

- Add callback/event mapping from runner events to Adjutant assignment events.
- Map relevant events into `site_events`.
- Update `software_orders.status` to `agent_queued`, `agent_running`,
  `needs_customer_input`, `delivered`, or `unavailable` when appropriate.
- Preserve callback retry by run ID.

Acceptance:

- Customers see high-level order status change as Adjutant runs.
- Operators see detailed assignment/site events.
- Public projection remains sanitized.

Implementation note, June 5, 2026:

- Issue #74 added `adjutant-run-lifecycle`, a typed mapper that runs inside
  Omni runner callback ingestion after run events and goal runtime accounting
  are accepted.
- The mapper finds the Adjutant assignment by `current_run_id`, then maps
  queued/running/waiting/completed/failed callback states to assignment events,
  `site_events`, and customer-visible `software_orders.status` values:
  `agent_queued`, `agent_running`, `needs_customer_input`, `delivered`, or
  `unavailable`.
- Assignment status now tracks callback lifecycle as `queued`, `running`,
  `review_needed`, `delivered`, or `blocked`.
- Lifecycle event payloads are sanitized summaries containing run ID, stage,
  task packet path, packet commit SHA, event count, and the latest event's
  type/status/source/sequence only. Raw runner payloads and logs are not copied
  into Site or assignment lifecycle projections.
- Callback retries remain keyed by run ID. The mapper updates state
  idempotently and skips duplicate assignment/Site lifecycle events for the
  same run and lifecycle event type.

### 10. Adjutant: save generated artifacts as Site versions

Deliverables:

- Define a runner artifact receipt contract for Site source archive, static
  manifest, build log, worker module, and metadata.
- Store source/build artifacts in R2.
- Create `site_versions` with `sourceKind = 'autopilot_generated'`.
- Link version rows to `created_by_run_id`.
- Redact build logs.

Acceptance:

- A completed generation run can produce a saved reviewable Site version.
- Versions cannot deploy unless build status is `saved`.
- Unsafe metadata or secret-shaped material is rejected.

Implementation note, June 5, 2026:

- Issue #75 added the `openagents.adjutant.site_artifact_receipt.v1` runner
  receipt contract. Receipts carry Site ID, build status, static asset
  manifest, optional source archive, build log, worker module text or pre-stored
  worker module R2 key, source commit SHA, storage binding names, build command,
  and metadata.
- Omni callback lifecycle mapping now extracts receipts from runner event
  payloads at `adjutantSiteArtifactReceipt`, `siteArtifactReceipt`, nested
  `payload.*`, or the payload root when the receipt schema version is present.
- When an Adjutant-linked run callback carries a valid receipt, the lifecycle
  mapper saves a `site_versions` row through `AutopilotSitesService.saveVersion`
  with `sourceKind = autopilot_generated` and `created_by_run_id` set to the
  runner ID.
- `saveVersion` now accepts optional `workerModuleText`, rejects
  secret-shaped module contents, stores it in R2 as `worker.mjs`, and links the
  resulting key to the saved version unless a pre-stored `workerModuleR2Key` is
  provided.
- Source archives, static manifests, build logs, worker modules, and metadata
  continue through the existing Sites safety checks. Build logs are redacted
  before R2 storage.
- Callback retries do not create duplicate Site versions because receipt saves
  are skipped when a version already exists for the same Site and
  `created_by_run_id`.

### 11. Adjutant: add operator review and deployment workflow

Deliverables:

- Add operator detail UI for Adjutant assignment and linked Site.
- Show order, Site, goal, current run, versions, deployments, events, and next
  action.
- Enable save/deploy/rollback/disable where required inputs exist.
- Keep launch checklist explicit for public deploy/access widening.

Acceptance:

- Core operators can review and deploy Adjutant-generated Site versions without
  leaving the operator UI.
- Disable and rollback receipts are visible.

Implementation note, June 5, 2026:

- Issue #76 extended `GET /api/operator/adjutant/assignments/:assignmentId`
  with an operator review payload for the linked order, Site, goal, current run,
  generated versions, deployments, assignment events, Site events, and next
  action.
- The review payload intentionally exposes event type, summary, run ref, and
  timestamp rather than raw event payload JSON.
- The admin UI now loads Adjutant assignments, lets operators open an
  assignment review panel, and shows saved generated versions, active/previous
  deployments, Site and assignment event receipts, linked order/Site/goal/run
  state, and the next operator action.
- Operators can deploy the latest saved generated version, disable the active
  deployment, or roll back to a previous deployment without leaving the admin
  page.
- Public Site deployments send the explicit launch checklist payload for source,
  build, audience, secrets, and URL review; the Sites service remains the
  backend authority that enforces the checklist.

### 12. Adjutant: add customer-safe progress projection

Deliverables:

- Add customer-safe Adjutant projection for `/order` and future
  `/orders/:orderId`.
- Include order status, Site status, active URL, review-needed/input-needed
  state, and next action.
- Exclude raw runner/build/internal dispatch data.

Acceptance:

- Customers can tell whether Autopilot is queued, running, reviewing, deployed,
  or waiting for input.
- Customers cannot access operator-only details.

Implementation note, June 5, 2026:

- Issue #77 added a customer-safe `adjutant` progress projection to customer
  order responses. The projection includes the high-level Autopilot stage, order
  status, Site status, active URL, review/input-needed booleans, and next
  action.
- `/api/customer-orders/active` returns the projection for `/order`, and
  `/api/customer-orders/:orderId` now reads a specific customer-owned order for
  `/orders/:orderId` without exposing other users' orders.
- The customer order response no longer includes raw run ids or agent-started
  timestamps; those remain operator/internal data.
- The `/order` UI now renders the Autopilot stage and next action alongside
  customer-safe Site and active URL state.

### 13. Adjutant: add public activity projection

Deliverables:

- Add public-safe Autopilot activity service over public goals and selected
  public order/site milestones.
- Include deployed public Site URLs and safe refs only.
- Add provider-secret-material checks like the Artanis public projection.

Acceptance:

- `/adjutant` can show public Autopilot work without leaking private order,
  provider, callback, prompt, or shell data.
- Private/team-only assignments do not appear publicly.

Implementation note, June 5, 2026:

- Issue #78 added `/api/public/adjutant/activity`, a public-safe Adjutant
  activity projection over public Adjutant assignments plus public order/Site
  deployment state.
- The projection returns safe milestones and deployed public Site rows only:
  public refs, public Site titles/slugs, public statuses, timestamps, and active
  public Site URLs.
- The service excludes private/team assignments and non-public Sites before
  projection, does not select request/prompt/payload/callback/provider fields,
  and scans the final serialized response for provider-secret material before
  returning it.
- `/adjutant` now loads that activity endpoint and renders deployed public
  Sites plus recent public milestones alongside the public goal feed.

### 14. Adjutant: add typed `@adjutant` team command

Deliverables:

- Add bounded `@adjutant` parsing to team/project chat.
- Require explicit selected order/site/task packet context.
- Post an `adjutant_intent` or typed team message kind.
- Do not change the existing `@autopilot` parser behavior.

Acceptance:

- Core team members can tag Adjutant from a project room.
- The command cannot launch without explicit order/site assignment context.

Implementation note:

- Issue #79 added `adjutant_intent` as a durable `team_chat_messages.kind`
  value, including migration `0036_team_chat_adjutant_intent.sql`.
- Team/project chat now recognizes exact `@adjutant` command forms separately
  from `@autopilot`; `@autopilot` continues to be the only team-chat command
  that launches an Autopilot run directly.
- Accepted Adjutant intent rows require the Adjutant project room plus bounded
  `softwareOrderId`, `siteId`, or `taskSpecPath` context from explicit request
  fields or typed key/value command tokens. The context is stored in
  `metadata_json.adjutantIntent`; natural-language prompt wording is not used
  to infer order or Site selection.
- The browser keeps generic project workrooms disabled while allowing the
  `adjutant` project room as the narrow command surface for core-team Site
  fulfillment supervision.

### 15. Adjutant: add adjustment continuations

Deliverables:

- Model Site adjustment requests as typed records linked to assignments.
- Add operator/customer-safe adjustment request APIs.
- Continue the same goal/run where policy permits.
- Save adjustment output as a new Site version.

Acceptance:

- Adjustments do not create duplicate unrelated goals.
- Customer-visible status reflects adjustment progress.
- Operator can approve/reject/deploy the new version.

Implementation note:

- Issue #80 added the typed `adjutant_adjustment_requests` ledger, the
  operator assignment adjustment route, and customer-safe adjustment progress on
  the order projection. Active assignment runs receive a follow-up turn; closed
  runs launch a new run with the existing Adjutant goal ID so related
  adjustments stay on the same durable goal.
- The runner receipt lifecycle now links the latest active adjustment to the
  saved Site version and moves the Site back to `needs_review` for operator
  review/deploy handling through the existing Sites routes.

### 16. Adjutant: integrate notifications through approved email ledger

Deliverables:

- Add notification-needed events for review-ready, deployed, input-needed, and
  unavailable states.
- Send email only through the existing typed email boundary.
- Store email ledger refs on assignment/order events.

Acceptance:

- Customers can be notified without raw Gmail/provider shortcuts.
- Email events are auditable and idempotent.

Implementation note:

- Issue #81 routes Adjutant customer notifications through `EmailService` and
  the existing `email_messages` / `email_deliveries` ledger. Lifecycle callbacks
  emit `adjutant.notification.review_ready`,
  `adjutant.notification.input_needed`, and `adjutant.notification.unavailable`
  events; operator deploys emit `adjutant.notification.deployed`.
- `adjutant_assignment_events` and `site_events` now carry nullable
  `email_message_id` references so customer/order-visible Site milestones can be
  audited back to the typed email ledger without adding Gmail/provider
  shortcuts or raw provider payloads to public events.
- Notification sends use stable Adjutant idempotency keys. Callback retries and
  deploy retries record at most one notification event per lifecycle stage and
  reuse already accepted email ledger rows instead of sending duplicate provider
  requests.

### 17. Adjutant: add runner backend resilience

Deliverables:

- After the runner gateway/Cloudflare Containers work lands, allow Adjutant
  preflight to report SHC primary, Cloudflare Container backup, and GCloud
  reference lane readiness.
- Keep low-to-medium trust Site generation eligible for backup/burst lanes.
- Keep sensitive workloads gated to approved backends.

Acceptance:

- Adjutant can explain runner capacity/blockers before launch.
- Automatic failover is disabled until staging smoke and policy approval exist.

Implementation note:

- Issue #82 adds a typed runner backend policy config and a
  `runner_backends` operator preflight check. Adjutant inherits this check
  through the existing operator Autopilot preflight and launch path.
- The check reports SHC primary, Cloudflare Container backup/burst, and GCloud
  reference/sensitive lane readiness. Low-to-medium trust Site work is eligible
  for the Container backup lane only when the lane is enabled, configured,
  smoke-tested, and policy-approved. Sensitive work is never eligible for the
  Container backup lane and requires an approved GCloud reference/sensitive
  lane before that lane reports ready.
- Automatic failover remains effectively disabled unless the explicit failover
  flag is set and the Container lane is enabled, configured, smoke-tested, and
  policy-approved. If failover is requested without those prerequisites,
  preflight reports a blocker instead of silently changing dispatch behavior.

### 18. Adjutant: add billing, usage, and receipts

Deliverables:

- Attribute generation, build, hosting, storage, and adjustment usage to the
  order/Site/assignment.
- Reuse Stripe and billing ledger services when paid Sites launch.
- Add public/customer/team-safe receipts.

Acceptance:

- Operators can distinguish generation cost from hosting/storage cost.
- Public beta work remains free until pricing policy changes.
- Paid Sites use customer-facing credits, not raw provider cost pass-through.

Implementation note:

- Issue #83 added the `adjutant_usage_receipts` ledger. Receipts attach to the
  Adjutant assignment and optionally to the software order, Site, adjustment,
  run, and future billing ledger entry.
- Receipt categories are explicit: `generation`, `build`, `hosting`,
  `storage`, and `adjustment`. Operator assignment review now returns
  `usageReceipts` plus `usageSummary`, so generation work is separated from
  hosting and artifact storage.
- Public/customer-safe order responses now include public receipt summaries
  without raw runner IDs or provider-cost internals. Team/operator views can
  see team-safe details such as version, deployment, assignment, and adjustment
  references.
- Public beta receipt writes use `billingMode: public_beta_free`,
  `creditsChargedCents: 0`, and stable idempotency keys. The receipt service
  rejects public-beta credit charges. Future paid Sites must use
  `billingMode: paid_credits` and link an existing `billing_ledger_entries`
  debit, keeping Stripe and credit accounting in the approved billing ledger
  instead of passing through raw provider cost.
- Generation receipts are recorded after accepted Adjutant launch, adjustment
  receipts after accepted adjustment continuation/new-run dispatch, build and
  storage receipts from runner artifact callbacks, and hosting receipts after
  active Site deployment.

### 19. Adjutant: add end-to-end verification and launch runbook

Deliverables:

- Add focused tests for identity, assignment, preflight, launch, event mapping,
  artifact save, customer projection, public projection, and operator UI.
- Add a runbook packet for launching the first Adjutant Site fulfillment.
- Include Ben OTEC as the canonical first smoke when production data permits.

Acceptance:

- One operator can assign Ben's OTEC Site to Adjutant, preflight, launch,
  monitor, save, review, deploy, and show customer/public progress.
- The runbook lists exact commands and safe production links.

Implementation note, June 5, 2026:

- Issue #84 added the active first-launch runbook packet at
  `docs/autopilot-tasks/2026-06-05-adjutant-site-fulfillment-runbook.md`.
- The runbook makes Ben's OTEC/SWAC floating datacenter Site the canonical
  first Adjutant smoke when the production software order exists, with exact
  local verification commands, operator API payloads, safe production links,
  public/customer projection checks, deployment checklist payloads, and
  closeout fields.
- Existing focused API coverage now forms the Adjutant end-to-end verification
  matrix:
  `adjutant-assignments.test.ts` covers identity, assignment, and durable goal
  policy; `operator-adjutant-routes.test.ts` covers assignment listing,
  task-packet validation, preflight, launch, adjustment, review, receipts, and
  deployment action payloads; `adjutant-run-lifecycle.test.ts` covers runner
  event mapping, order/Site status movement, artifact receipt save, Site
  version creation, and usage receipts; `customer-order-routes.test.ts` covers
  customer-safe Adjutant progress and public usage receipts;
  `adjutant-public-activity.test.ts` covers sanitized public projection; and
  `operator-sites-routes.test.ts` plus `sites.test.ts` cover Site creation,
  version save, deployment, disable, rollback, and checklist gating.
- Browser coverage now includes the public Adjutant routes in `route.test.ts`,
  `main.test.ts`, and `docs-blog-route.test.ts`; customer order progress and
  usage summary rendering plus operator Adjutant review usage receipts,
  versions, deployments, Site events, and checklist deployment controls are
  covered in `apps/web/src/page/loggedIn/view.scene.test.ts`.
- The production launch remains gated: no real Ben OTEC launch should occur
  until preflight passes on deployed `main`, the task packet is committed and
  pushed, the operator review confirms the saved version, and customer/public
  projections are verified without exposing private runner material.

## First Adjutant Launch Candidate

The first concrete Adjutant run should be Ben's OTEC/SWAC floating datacenter
Site because it already appears in the Sites plan as the launch wedge.

Target:

```text
softwareOrderId: Ben OTEC order from production
site slug: otec
target URL: https://sites.openagents.com/otec
agentId: agent_adjutant
projectId: project_adjutant
teamId: team_openagents_core
visibility: public after review
```

The run should not launch until:

- `project_adjutant` and `agent_adjutant` exist;
- the assignment ledger exists;
- the order is explicitly assigned;
- the task packet is committed and pushed;
- preflight passes;
- public projection safety tests pass;
- Sites launch checklist can gate deployment.

## Non-Goals For The First Adjutant Release

- Automatic conversion of every customer order into an Adjutant assignment.
- Keyword-based Site routing.
- Customer access to raw workroom streams.
- Public raw build logs.
- Broad dynamic Worker execution without review.
- Custom domains.
- Live paid Sites packages unless billing policy and Stripe production config
  are complete.
- Automatic Cloudflare Containers failover before the runner gateway task is
  implemented and smoked.
- Bitcoin or revenue-share payouts.

## Open Questions

1. Should Adjutant be public by default, or should public visibility be
   assignment-specific?
2. Should `project_adjutant` be a single project for all customer order
   fulfillment, or should each major customer/order get a child project later?
3. Should customer-visible adjustment requests live on `/order`, a future
   `/orders/:orderId`, or a Site-specific customer page?
4. Should Adjutant publish aggregate public counts before the first public Site
   fulfillment is complete?
5. Should Adjutant-generated task packets live directly in
   `docs/autopilot-tasks/` or in a generated subdirectory with a stricter
   cleanup/archive policy?

## Bottom Line

Autopilot should be the Sites fulfillment surface that turns OpenAgents product surface's public
software-order wedge into repeatable, observable delivery. The system does not
need a new runtime to get there. Internally, it needs a durable Adjutant
identity, an assignment ledger, explicit operator assignment APIs,
preflight-gated launch, artifact-to-version wiring, and safe
customer/public/team projections.

Artanis proves the public-agent pattern. Sites proves the hosting lifecycle.
The customer-order work proves the intake split. The internal Adjutant
supervisor is the bridge that binds those three pieces into one supervised
Autopilot fulfillment loop.
