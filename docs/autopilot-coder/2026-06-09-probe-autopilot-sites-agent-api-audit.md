# Probe, Autopilot, And Autopilot Sites Agent API Audit

Date: 2026-06-09

Status: audit and implementation map. This document does not create a new
runtime API, launch jobs, change payment policy, grant deployment authority,
or publish new public claims by itself.

## Scope

This audit maps the current OpenAgents systems against the desired next product
shape:

```text
user tells any capable agent "do this on Autopilot"
-> that agent discovers/calls the OpenAgents Autopilot API
-> OpenAgents creates a free, quoted, or paid work order
-> OpenAgents asks for only the missing access, payment, or consent
-> OpenAgents plans and fans out Probe/Pylon/runner assignments
-> assignments run on approved infrastructure such as SHC, Pylons, user boxes,
   local Codex, hosted Gemini, cloud sandboxes, TEEs, Maple AI, or later
   privacy-enhanced lanes
-> workers return redacted evidence, diffs, Sites, previews, tests, and receipts
-> OpenAgents gates acceptance, payment, settlement, and public/forum reporting
```

The audit covers:

- Probe as the small coding-worker runtime.
- Autopilot as the customer-facing cloud coding agent and order system.
- Autopilot Sites as the hosted-site generation and revision product.
- The new agent-first API and payment/fanout direction.
- The requirement to fold existing Autopilot queue work into the new path and
  report public-safe status on the Forum.

## Desired End State

Autopilot should become the delegated coding-work endpoint for the OpenAgents
network. A user should be able to tell their own agent, IDE agent, terminal
agent, Pylon, or another agent-capable client:

```text
Do this on Autopilot.
Delegate this to Autopilot.
Have OpenAgents handle this.
```

The relevant agent should know, from OpenAgents discovery docs and capability
manifests, how to submit the task to `openagents.com` without the user
manually navigating the UI. The happy path is:

1. The caller sends a typed work request to the Autopilot API.
2. OpenAgents recognizes the caller, owner, repo, Pylon, or payment context if
   available.
3. If access is missing, OpenAgents returns a structured access request:
   GitHub/repo grant, Pylon enrollment, secret-broker approval, privacy-tier
   confirmation, operator approval, or customer review.
4. If payment is required, OpenAgents returns an MDK checkout or L402 challenge
   with stable pricing. Agent clients should be able to pay and retry.
5. Once authorized and funded, OpenAgents chooses the lowest-friction allowed
   placement: the user's local Pylon/Codex path first when available, then
   OpenAgents SHC or cloud capacity, then paid privacy or premium lanes when
   requested.
6. OpenAgents launches one or more Probe-shaped workers, receives evidence,
   gates acceptance, posts public-safe Forum progress where policy allows, and
   settles eligible worker/provider/referral payments only after the relevant
   acceptance gates clear.

The product should be Pylon-first but not Pylon-only. Pylon should be the
lowest-friction path when the user has it installed because it already gives
OpenAgents presence, heartbeat, capability refs, local compute, wallet
readiness, and future local secret boundaries. If the request comes from
outside Pylon, the same API should still work: use existing owner-scoped agent
auth when present, return access-required prompts when missing, and fall back
to paid MDK/L402 entry when the user wants OpenAgents to supply the compute.

This is the commercial loop: customers and agents pay OpenAgents through MDK
checkout or L402 for coding work; OpenAgents uses local user compute,
OpenAgents-owned hosted capacity or paid network capacity to perform
the work; OpenAgents pays Pylons, providers, referrers, or contributors only
from accepted-work and settlement ledgers, not from buyer checkout state alone.

## Source Set Reviewed

- `docs/refactor/README.md`
- `docs/refactor/2026-06-09-bun-effect-monorepo.md`
- `docs/refactor/2026-06-09-openagents-com-staging-homepage-audit.md`
- `docs/transcripts/228.md`
- `docs/transcripts/229.md`
- `apps/openagents.com/AGENTS.md`
- `apps/openagents.com/INVARIANTS.md`
- `apps/openagents.com/docs/2026-06-04-programmatic-autopilot-work-runbook-audit.md`
- `apps/openagents.com/docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md`
- `apps/openagents.com/docs/2026-06-05-openai-sites-parity-implementation-audit.md`
- `apps/openagents.com/docs/2026-06-05-openagents-agent-surface-gap-analysis.md`
- `apps/openagents.com/docs/2026-06-06-episode-228-launch-claim-ledger.md`
- `apps/openagents.com/docs/2026-06-08-openagents-agent-sheet-route-coverage.md`
- `apps/openagents.com/docs/2026-06-08-pylon-agentic-revenue-gap-audit.md`
- `apps/openagents.com/docs/2026-06-02-mdk-l402-agent-checkout-audit.md`
- `apps/openagents.com/docs/2026-06-08-probe-gepa-settlement-readiness-gate.md`
- `apps/openagents.com/docs/2026-06-08-probe-gepa-paid-mode-campaign-ladder.md`
- `apps/openagents.com/docs/2026-06-08-provider-capacity-marketplace-gate.md`
- `apps/openagents.com/docs/2026-06-08-data-trace-marketplace-gate.md`
- `apps/openagents.com/docs/2026-06-08-signature-marketplace-revenue-gate.md`
- `packages/probe/README.md`
- `packages/probe/docs/probe-openagents-run-assignment.md`
- `packages/probe/docs/probe-fleet-telemetry.md`
- `packages/probe/docs/probe-blueprint-backend-capability-routing.md`
- `packages/probe/docs/probe-gepa-candidate-execution.md`
- `packages/probe/docs/2026-06-08-probe-gepa-live-network-system-audit.md`
- `apps/pylon/README.md`
- `apps/pylon/docs/2026-06-09-probe-to-pylon-port-audit.md`
- `apps/pylon/docs/live-worker-loop-smoke.md`
- `apps/pylon/docs/presence-registration-heartbeat.md`
- `apps/pylon/docs/mdk-wallet-readiness-ledger.md`
- `apps/pylon/docs/launch-gates-no-overclaim.md`
- Current Worker, Pylon, and Probe source files named below.

## Transcript Product Commitments

### Episode 228

Episode 228 describes Autopilot as the public cloud coding wedge:

- users go to `openagents.com`, log in with GitHub, choose or type a repo, and
  describe coding work;
- the first small slice is free or public-beta funded;
- ambitious work should move into paid checkout;
- public work and traces can be visible because the free beta is meant to
  generate public proof and useful learning data;
- private repos and private execution are future or gated support;
- accepted useful work can eventually feed revenue share, but settled payout
  claims are not proven by the transcript.

The repo already encodes this as an explicit launch-claim ledger in
`apps/openagents.com/docs/2026-06-06-episode-228-launch-claim-ledger.md`.
That ledger is the correct public-copy posture: beta launch and limited free
work are verified or measured; private repo support is planned; revenue share
is modeled; accepted-work payout settlement and best-agent superlatives are
prohibited until matching receipts exist.

### Episode 229

Episode 229 extends the same Autopilot wedge into Sites:

- some orders are whole websites rather than code edits;
- Autopilot Sites creates versions, lets the customer submit follow-up
  revisions, and can publish public Sites at `sites.openagents.com/<slug>`;
- the demo flow included order queue, revision feedback, and public Site
  discovery/referral;
- the referral loop is intended to let humans or agents discover a Site and
  later compensate the Site owner if referred users become paying customers.

The current code and docs support the operator-supervised version of this flow,
not the fully self-serve parity claim. Site projects, versions, deployments,
builder sessions, agent Site contracts, Site feedback, and commerce/referral
contracts exist. Automatic production deploy and broad self-serve creation
remain gated.

## Current Repo Boundaries

The 2026-06-09 monorepo reset matters for routing the implementation work:

| Area | Current home | Audit finding |
| --- | --- | --- |
| Product surface and Worker API | `apps/openagents.com/` | Active implementation home for order intake, Autopilot, Sites, Forum, Pylon APIs, payment gates, and public projections. |
| Probe runtime | `packages/probe/` | Imported Probe runtime/evidence package still exists, but current launch runtime work has also been ported into Pylon. |
| Pylon contributor app/runtime bundle | `apps/pylon/` | Active package for users' machines and Pylon-hosted runtime commands. It now carries the former Probe runtime as `@openagentsinc/pylon-runtime`. |
| Forum extraction target | `apps/forum/` | Minimal target app exists. Live Forum routes remain inside `apps/openagents.com` for now. |
| Refactor docs | `docs/refactor/` | Correct place for cross-system reset and migration audits like this one. |

The current invariant ledgers require the following boundaries:

- Probe evidence does not authorize deployment, spend, provider mutation, or
  public claim promotion.
- Public UI does not own payout, settlement, runtime promotion, or accepted
  outcome authority.
- MDK checkout, buyer payment, payout eligibility, and Bitcoin settlement are
  separate states.
- Agent-facing routing and task selection must stay typed or semantic; do not
  add ad hoc keyword routing.
- Product-promise and loose mismatch reports are Forum-first, while GitHub
  issues are reserved for concrete reproducible bugs.

## What Is Built

### 1. Agent-Facing Discovery And Identity

Built.

Evidence:

- `workers/api/src/agent-registration.ts`
- `workers/api/src/agent-scoped-grant-routes.ts`
- `workers/api/src/agent-home-routes.ts`
- `workers/api/src/openagents-capability-manifest.ts`
- `workers/api/src/openagents-openapi.ts`
- `apps/openagents.com/docs/2026-06-08-openagents-agent-sheet-route-coverage.md`

Current behavior:

- Agents can self-register and receive `oa_agent_...` credentials.
- `/api/agents/home`, `/AGENTS.md`, `/.well-known/openagents.json`, and
  `/api/openapi.json` expose live/planned/gated capabilities.
- Signed-in owners can grant scoped authority to registered agents for
  `customer_orders` and `agent_sites`.
- Capability manifest and OpenAPI now list customer order, Site action, Forum,
  Pylon, payment-preview, and public proof routes.

Gap:

- This is discovery and scoped access, not broad autonomous execution authority.
- Broad scoped API keys remain planned or gated.

### 2. Customer Order API For Coding Work

Built for basic agent-first order intake; partial for the new batch fanout
vision.

Evidence:

- `workers/api/migrations/0030_software_orders.sql`
- `workers/api/migrations/0104_customer_order_agent_idempotency.sql`
- `workers/api/src/customer-orders.ts`
- `workers/api/src/customer-order-agent-auth.ts`
- `workers/api/src/onboarding/routes.ts`
- `workers/api/src/customer-order-routes.test.ts`
- `workers/api/src/openagents-openapi.ts`

Current behavior:

- `POST /api/customer-orders` creates a public software workstream from request
  text.
- Agent writes are supported with owner-bound `customer_orders.write` grants
  and required `Idempotency-Key`.
- `GET /api/customer-orders`, `GET /api/customer-orders/active`, and
  `GET /api/customer-orders/{orderId}` return customer-safe projections.
- Site revisions, Site feedback, and fulfillment artifacts are exposed to
  owners or owner-granted agents.
- Orders carry `free_slice_cents`, `quote_cents`,
  `compute_payment_acknowledged_at`, and `provider_account_required`.

Gap:

- There is no single "delegate to Autopilot" API contract that another agent
  can call and then poll/stream until the work is done.
- The API accepts one request string, not a typed batch of tasks with
  repository refs, privacy tier, placement constraints, acceptance criteria,
  budget, payment mode, forum-reporting policy, and per-task idempotency.
- Creating an order does not automatically plan, lease, or launch network
  Probe assignments.
- MDK checkout is not wired as the purchase path for software orders.
- L402-style "pay this quoted amount, retry with proof, then launch" behavior
  is not yet exposed for coding work orders.
- Existing order queue records are not yet folded into a Probe-native task
  queue or Forum-reporting queue.

### 3. Autopilot Goal, Run, And Operator Fulfillment Substrate

Partially built.

Evidence:

- `workers/api/migrations/0010_omni_agent_runs_and_deployments.sql`
- `workers/api/migrations/0027_agent_goals.sql`
- `workers/api/src/agent-goal-routes.ts`
- `workers/api/src/adjutant-assignments.ts`
- `workers/api/src/adjutant-run-lifecycle.ts`
- `workers/api/src/operator-order-triage-routes.ts`
- `apps/openagents.com/docs/2026-06-04-programmatic-autopilot-work-runbook-audit.md`

Current behavior:

- Agent goals, goal events, runs, artifacts, and sync projections exist.
- Programmatic Autopilot administration has been exercised against SHC-backed
  runs.
- Operator triage can classify first-batch orders and create Adjutant
  assignments.
- The first-batch prepare path intentionally says "Generate a task packet and
  preflight plan, but do not launch without explicit review."
- Follow-up Site feedback can create Adjutant adjustment assignments.

Gap:

- Autopilot fulfillment remains operator/preflight centered.
- The platform has no single "submit batch and automatically fan out probes"
  orchestration service.
- Runner dispatch history is SHC-shaped and still being refactored toward
  backend-neutral runner contracts.
- Provider reconnect, runner preflight, callback schema unification, and
  execution closeout are better than before but still not a fully automated
  commercial work pipeline.

### 4. Autopilot Sites Control Plane

Built for operator-supervised public Sites; partial for fully self-serve,
agent-generated Sites.

Evidence:

- `workers/api/migrations/0032_autopilot_sites.sql`
- `workers/api/migrations/0056_site_compatibility_checks.sql`
- `workers/api/migrations/0057_site_build_validations.sql`
- `workers/api/migrations/0058_site_revision_feedback.sql`
- `workers/api/migrations/0082_site_builder_sessions.sql`
- `workers/api/migrations/0084_site_builder_saved_versions.sql`
- `workers/api/migrations/0085_site_deployment_attempts.sql`
- `workers/api/src/sites.ts`
- `workers/api/src/site-runtime.ts`
- `workers/api/src/site-runtime-routes.ts`
- `workers/api/src/agent-site-routes.ts`
- `workers/api/src/sites-builder-sessions.ts`
- `workers/api/src/sites-builder-saved-versions.ts`
- `workers/api/src/site-library.ts`
- `apps/openagents.com/docs/2026-06-05-openai-sites-parity-implementation-audit.md`

Current behavior:

- `site_projects`, `site_versions`, `site_deployments`, storage bindings,
  environment values, access grants, and events exist.
- `AutopilotSitesService` creates order-backed Site projects, saves versions,
  deploys versions, records events, updates access, and enforces public launch
  checklist constraints.
- Public runtime can serve active static R2 deployments and dispatch Workers
  for Platforms deployments.
- Site builder sessions, messages, event streams, file snapshots, file reads,
  and exports exist for signed-in product flows.
- Scoped agent Site contracts can create order-backed Site projects, create
  builder sessions, queue preview records/events, save reviewable versions when
  evidence gates are complete, and create deploy-review requests.

Gap:

- Production deployment remains owner/operator gated.
- Agents can request deploy review but cannot autonomously publish production
  Sites.
- The `@Sites`-style direct prompt-to-hosted-project product action is not a
  complete self-serve loop.
- Existing-project clone/build/adapt/import is not a complete automatic
  pipeline.
- Hosted secret/environment management and private/internal Site auth are not
  broad productized customer flows.

### 5. Site Commerce, Referral, And MDK/L402 Payment Primitives

Built as contracts and narrow live lanes; partial for the new Autopilot work
checkout economy.

Evidence:

- `workers/api/migrations/0065_site_commerce_catalog.sql`
- `workers/api/migrations/0066_site_commerce_revenue_share_linkage.sql`
- `workers/api/migrations/0067_site_referral_sources.sql`
- `workers/api/migrations/0068_site_referral_attributions.sql`
- `workers/api/migrations/0114_buyer_payment_ledger.sql`
- `workers/api/migrations/0115_site_payment_catalog.sql`
- `workers/api/migrations/0124_site_mdk_checkout_intents.sql`
- `workers/api/migrations/0127_site_mdk_account_bindings.sql`
- `workers/api/src/site-commerce-routes.ts`
- `workers/api/src/site-payment-manifest.ts`
- `workers/api/src/site-payment-proof.ts`
- `workers/api/src/site-payment-to-payout-bridge.ts`
- `workers/api/src/site-referral-routes.ts`
- `apps/openagents.com/docs/2026-06-02-mdk-l402-agent-checkout-audit.md`

Current behavior:

- Generated Sites can expose payment discovery, checkout-intent contracts,
  L402 challenge/redemption contracts, payment proof reads, commerce review
  decisions, and MDK account-binding reads.
- Site referral capture and attribution persistence exist.
- Site payment-to-payout bridge policy exists and rejects client-success,
  missing accepted work, missing payout target approval, stale wallet
  readiness, spend-cap violations, missing movement evidence, and duplicate
  buyer receipts.
- Forum paid actions and hosted search/proposal recovery show that L402-style
  buyer-side paid API recovery can be modeled in this Worker.

Gap:

- Customer software-order checkout is not yet MDK-backed.
- There is no production `POST /api/autopilot/work` or equivalent paid endpoint
  where an unauthenticated or underfunded agent gets an HTTP 402 challenge,
  pays with an agent wallet, and retries with `Authorization: L402
  <token>:<preimage>`.
- A buyer paying for an Autopilot coding task is not yet connected to
  assignment dispatch, accepted-work payout eligibility, and settlement.
- Dynamic pricing for coding work has not been made deterministic across quote,
  invoice/challenge creation, retry verification, and assignment launch.
- Referral capture is attribution only; it does not create Bitcoin payout
  eligibility.
- Hosted MDK direct programmatic payouts remain separately gated, and MDK is
  not accepted-work payout authority by itself.

### 6. Probe Runtime Contracts

Built as runtime/evidence foundation; partial for live generalized coding
worker execution.

Evidence:

- `packages/probe/README.md`
- `packages/probe/packages/runtime/src/cli.ts`
- `packages/probe/packages/runtime/src/index.ts`
- `packages/probe/docs/probe-openagents-run-assignment.md`
- `packages/probe/docs/probe-fleet-telemetry.md`
- `packages/probe/docs/probe-blueprint-backend-capability-routing.md`
- `packages/probe/docs/probe-gepa-candidate-execution.md`

Current behavior:

- Probe has assignment contracts, OpenAgents grant resolution, per-run auth
  materialization, runner identity, fleet telemetry, token-usage telemetry,
  Apple FM backend contracts, Gemini support, Blueprint signature lookup,
  tool-menu planning, action-submission boundaries, retained benchmark
  fixtures, and GEPA candidate execution seams.
- Probe emits redacted refs and evidence shapes intended for OpenAgents
  workrooms and public-safe projections.
- Probe does not locally iterate raw provider tokens; OpenAgents remains
  account selection and lease authority.

Gap:

- The current live product path is not "Probe fleet executes arbitrary coding
  orders end-to-end."
- The live benchmark smoke proved plumbing and retained/failed evidence, not a
  production Probe coding-worker lane.
- Probe does not own acceptance, payout, settlement, candidate promotion, or
  Forum posting authority.

### 7. Pylon As The User-Box Runtime Package

Built as a release-candidate worker shell and runtime bundle; partial for paid
network-wide task execution.

Evidence:

- `apps/pylon/README.md`
- `apps/pylon/docs/2026-06-09-probe-to-pylon-port-audit.md`
- `apps/pylon/docs/presence-registration-heartbeat.md`
- `apps/pylon/docs/mdk-wallet-readiness-ledger.md`
- `apps/pylon/docs/live-worker-loop-smoke.md`
- `apps/pylon/src/assignment.ts`
- `apps/pylon/src/live-worker-loop-smoke.ts`

Current behavior:

- Pylon `0.3.0-rc1` bundles the former Probe runtime as
  `@openagentsinc/pylon-runtime`.
- `pylon presence register`, heartbeat, wallet readiness, payout-target
  admission, assignment poll, and no-spend assignment run commands exist.
- The live worker-loop smoke passed against `https://openagents.com` on
  2026-06-09 with registration, heartbeat, wallet-readiness projection,
  assignment creation, assignment read, accept, progress, artifacts, and
  operator closeout.
- Paid leases are blocked unless wallet send readiness is explicitly proven.

Gap:

- `0.3.0-rc1` is not a stable public earning release.
- The live worker loop proves no-spend event path, not paid work settlement or
  broad availability across arbitrary user machines.
- A Pylon can carry Probe runtime capability refs, but OpenAgents does not yet
  use a general scheduler to choose Pylons for customer coding orders.
- Pylon is not yet treated as the preferred Autopilot execution context for
  users who have a local machine, local Codex, local secrets, or local-only
  placement policy available.

### 8. Pylon API And Assignment Lifecycle

Built for registration, heartbeat, wallet readiness, assignment leasing, and
controlled dispatch.

Evidence:

- `workers/api/migrations/0123_pylon_agent_api.sql`
- `workers/api/migrations/0134_pylon_api_assignment_leases.sql`
- `workers/api/migrations/0135_pylon_api_version_heartbeat_state.sql`
- `workers/api/src/pylon-api.ts`
- `workers/api/src/pylon-api-routes.ts`
- `workers/api/src/public-pylon-stats.ts`
- `workers/api/src/probe-gepa-settlement-readiness.ts`
- `workers/api/src/probe-gepa-paid-mode-ladder.ts`

Current behavior:

- Pylons can register, heartbeat, post wallet readiness, poll assignments,
  accept assignments, submit progress, submit artifact/proof metadata, post
  payment receipts, and post settlement status.
- Operator assignment creation passes a controlled dispatch gate requiring
  explicit payment mode, campaign/policy refs, no-duplicate refs, no Forum
  auto-publish, unpaused state, wallet readiness, online freshness, and
  capability refs.
- Public Pylon stats distinguish recently seen, online-now, wallet-ready-now,
  assignment-ready-now, sellable, and blocked/unavailable states.

Gap:

- The current Pylon assignment API is not yet the default Autopilot order
  scheduler.
- Payment modes and settlement readiness are modeled, but paid campaigns are
  not generalized.
- The API does not yet assign arbitrary coding work from a customer order batch
  to Pylon-hosted Probe runtimes.
- Heartbeat, wallet-readiness, and capability facts are not yet fed into a
  product placement decision that says "use this user's Pylon first, otherwise
  use OpenAgents/cloud capacity, otherwise ask for payment or access."

### 9. Forum Reporting Surface

Built for Forum product and specific public-safe agent/reporting flows; partial
for automatic Autopilot order reporting.

Evidence:

- `workers/api/migrations/0101_forum_foundation.sql`
- `workers/api/migrations/0118_forum_artanis_seed.sql`
- `workers/api/src/forum-routes.ts`
- `workers/api/src/forum/launch-gates.ts`
- `workers/api/src/probe-gepa-forum-summary.ts`
- `workers/api/src/artanis-probe-gepa-benchmark-summary.ts`
- `apps/openagents.com/docs/forum/README.md`
- `apps/openagents.com/docs/2026-06-05-openagents-agent-surface-gap-analysis.md`

Current behavior:

- Forum reads, topic/reply creation, watches, bookmarks, reports, moderation,
  notifications, paid actions, receipt lookup, and launch status exist.
- Registered agents can post in open forums and use public-safe paid actions
  where route-specific contracts allow.
- Probe GEPA public summaries can be generated from public-safe refs, with
  Artanis posting kept behind operator authority.
- Product Promise reporting is Forum-first by repo contract.

Gap:

- Customer software orders are not automatically reported into a Forum topic.
- Autopilot queue status and Probe closeout summaries are not automatically
  posted as public-safe Forum updates.
- Existing queue records still need a one-time foldover/reporting job with
  redaction, owner visibility, and no overclaiming.

### 10. Privacy And Placement Controls

Mostly modeled; not productized end-to-end.

Built or partially built:

- Provider-account secret boundaries and Probe auth materialization.
- Site environment secret refs.
- Pylon public-safe projections and wallet material redaction.
- Data trace, provider capacity, signature marketplace, generated Site
  checkout, and public launch copy gates.
- Runner backend docs for SHC primary, Cloudflare Containers backup, and hosted
  model lanes
  reference lanes.

Not built:

- A customer-facing or agent-facing `privacyTier` selector that compiles into
  placement constraints.
- A scheduler that can enforce "local only", "SHC only", "TEE only", "cloud
  allowed", "no third-party provider", "secret management required", or
  "public trace allowed" across all task placements.
- Maple AI or equivalent privacy provider integration.
- TEE-backed execution receipts.
- General secret-brokered paid coding tasks where customer secrets are exposed
  only to an approved worker lane.

## Specific Gap Analysis: Current Monorepo Ready State Vs End State

The monorepo is ready for a supervised, operator-assisted version of the
Autopilot idea. It is not yet ready for the desired "tell any agent to do this
on Autopilot and the system handles the rest" product. The missing work is not
one feature; it is the orchestration layer that joins already-built identity,
orders, Pylon presence, Probe runtime contracts, Sites, payment primitives, and
Forum reporting.

| Capability | Ready in the monorepo today | Desired end state | Specific gap |
| --- | --- | --- | --- |
| Universal Autopilot invocation | Agent registration, `/api/agents/home`, capability manifest, OpenAPI, `POST /api/customer-orders`, agent Site routes | Any capable agent can call one Autopilot endpoint for "do this" and get a durable work order, status URL, event stream, access request, or payment request | No `POST /api/autopilot/work` contract, no single status/event model, no agent-readable paid retry contract for coding work |
| Agent/client discovery | `/.well-known/openagents.json`, `/AGENTS.md`, `/api/openapi.json`, capability manifest | External agents discover the Autopilot delegation flow, auth modes, L402 payment flow, Pylon hints, polling/events, and error recovery | Discovery surfaces list pieces, but not one canonical delegated-work capability with examples and state machine |
| Owner and agent authority | Registered agent credentials and owner-granted `customer_orders` / `agent_sites` scopes | Autopilot asks only for missing authority: repo read/write, branch/PR, secret broker, local Pylon, private placement, operator approval | No unified `access_required` response shape, no grant flow per missing capability, no private-repo/secret broker launch path |
| Work request schema | `customer_orders` store request text, quotes, free-slice fields, provider-account flags | Typed work request with tasks, repo refs, acceptance criteria, placement policy, privacy tier, budget, payment mode, idempotency, and forum reporting | Existing order text is too unstructured to drive safe automatic planning and placement |
| Order-to-assignment planning | Agent goals, Adjutant assignments, first-batch triage, SHC-oriented runbooks | Internal planner turns each task into typed assignment intents without launching until access/payment/placement gates clear | No persistent planner decisions, no typed task table, no quote/access/blocked/ready state per task |
| Pylon-first execution | Pylon registration, heartbeat, wallet readiness, assignment poll/accept/progress/artifacts, no-spend smoke | If the user has Pylon or local Codex available, Autopilot prefers that local path before OpenAgents or cloud capacity | Heartbeat/capability facts are not yet consumed by Autopilot placement; no local Codex capability model; no customer-order-to-Pylon assignment synthesis |
| Wider Probe/Pylon network execution | Probe runtime contracts, Pylon runtime package, assignment leasing APIs | Autopilot fans work out to approved SHC, requester Pylon, Pylon network, hosted Gemini, cloud, TEE, or Maple AI lanes according to policy | No generalized scheduler, lease offer model, or backend-neutral runner adapter for coding tasks |
| Probe coding-worker loop | Probe can materialize grants, run bounded runtime pieces, emit redacted evidence shapes and telemetry | Probe executes normalized coding assignments, produces diffs/tests/previews/closeouts, scrubs per-run material, and submits results to the order state machine | Probe plumbing exists, but arbitrary customer coding tasks are not executed end-to-end through it |
| Buyer payment intake | Site MDK/L402 contracts, buyer payment ledger, Forum paid actions, payment proof, site checkout intents | Coding work can return deterministic MDK checkout or HTTP 402 L402 challenge, accept paid retry, and start funded work | No coding-order quote service, no `withPayment`/L402 route for Autopilot work, no stable quote verification across retry |
| Worker/provider settlement | Pylon settlement gates, small-sats evidence, paid-mode campaign ladder docs | OpenAgents pays Pylons/providers/referrers only after accepted work, receipts, spend caps, duplicate checks, and settlement authority pass | Buyer payment is not linked to accepted-work ledger; paid Pylon work remains gated; hosted direct payout still disabled |
| Autopilot Sites delegation | Site projects, versions, deployments, events, runtime store, feedback, scoped agent Site create/session/preview/save/deploy-review | A delegated Autopilot task can create or revise a Site, return previews, request customer review, and promote only through gated deployment authority | Site flow is operator-supervised; autonomous production deploy remains request-only; existing-project import/self-serve gaps remain |
| Forum reporting and queue foldover | Forum routes, agent posting, paid actions, public-safe Probe GEPA summaries, Product Promise reporting norm | Public-safe Autopilot orders get Forum topics and lifecycle updates; existing queue work is dry-run folded and reported where allowed | No order-to-Forum bridge, no foldover job, no redaction/idempotency gate for customer order progress updates |
| Privacy and placement policy | Redaction patterns, secret refs, provider-account boundaries, Pylon public-safe projections, launch gates | Request can declare `public_beta`, `openagents_shc`, `customer_local_pylon`, `local_only`, `tee`, `maple_ai`, `cloud_allowed`, or secret-broker requirements | No unified privacy tier selector, placement compiler, TEE/Maple integration, or secret-brokered paid coding task path |
| API observability and recovery | Runs, artifacts, callbacks, Pylon events, Forum status pieces | Caller can poll or stream queued/running/needs-input/delivered/accepted/blocked/settled states and recover idempotently | No unified Autopilot work event stream, no client retry/recovery contract, no single closeout projection |
| Public claim safety | Launch claim ledgers, invariants, Forum-first product mismatch reporting | Public UI can say exactly what is live: free slice, paid task intake, Pylon-local execution, settlement, privacy lanes, Sites support | Claims must remain gated until the new orchestrator has evidence; no public "magic Autopilot" claim is currently supportable |

The ready pieces should be reused rather than replaced. The key build is a
narrow Autopilot orchestration spine:

```text
autopilot_work_request
-> access/payment decision
-> typed task records
-> placement decision
-> Probe/Pylon assignment leases
-> evidence and closeout ingestion
-> acceptance/review
-> settlement eligibility
-> Forum/public-safe reporting
```

## Target Autopilot Invocation Contract That Is Missing

The current `/api/customer-orders` endpoint is the right seed, but the product
needs a typed invocation contract. A caller should be able to use one endpoint
for "do this on Autopilot" and receive one of four durable outcomes:

- `accepted_free_slice`: OpenAgents can start a bounded free/public-beta task.
- `access_required`: the user must grant repo, secret, Pylon, privacy, or
  operator access.
- `payment_required`: OpenAgents returns an MDK checkout intent or L402
  challenge.
- `queued_or_running`: OpenAgents has accepted funding/access and created
  task/assignment records.

A likely first version should be a new schema rather than overloading the
single request string:

```json
{
  "schema": "openagents.autopilot_work_request.v1",
  "clientRequestRef": "client.example.20260609.001",
  "intent": "delegate_to_autopilot",
  "mode": "free_slice_or_paid_quote_or_l402",
  "caller": {
    "kind": "registered_agent",
    "agentId": "oa_agent_example",
    "pylonId": "optional-pylon-id",
    "agentWallet": "optional-mdk-agent-wallet-ref"
  },
  "tasks": [
    {
      "taskRef": "task.repo.docs.001",
      "kind": "code_change",
      "repository": {
        "provider": "github",
        "fullName": "owner/repo",
        "branch": "main",
        "visibility": "public"
      },
      "objective": "Add the requested API docs.",
      "acceptanceCriteriaRefs": [
        "acceptance.customer.docs_updated",
        "acceptance.tests.pass"
      ],
      "accessRequests": [
        {
          "kind": "github_repo_write",
          "reason": "Required only if OpenAgents must open a branch or PR."
        }
      ],
      "forumReporting": {
        "mode": "public_safe_summary",
        "targetForumRef": "forum.product-promises"
      }
    }
  ],
  "placementPolicy": {
    "privacyTier": "public_beta",
    "preferredRunnerKinds": ["requester_pylon", "openagents_shc"],
    "allowedRunnerKinds": [
      "requester_pylon",
      "shc",
      "pylon_network",
      "cloud_sandbox",
      "hosted_gemini",
      "tee",
      "maple_ai"
    ],
    "disallowedRunnerKinds": [],
    "requiresSecretBroker": false,
    "localOnlyAllowed": false,
    "publicTraceAllowed": true
  },
  "paymentPolicy": {
    "buyerPaymentMode": "free_slice_or_mdk_checkout_or_l402",
    "maxSpendCents": 0,
    "quotedAmountCents": null,
    "quoteRef": null,
    "settlementMode": "no_worker_payout_until_accepted_work"
  }
}
```

This schema should compile into durable records rather than prompt text:

- one customer-visible order;
- one or more task records;
- one caller/access state record;
- one placement policy record;
- one payment/quote/L402 state;
- zero or more Probe/Pylon/runner assignments;
- one Forum reporting policy;
- one acceptance state machine;
- one closeout bundle per assignment.

The HTTP behavior should be explicit:

- Authenticated and funded requests return `202 Accepted` with an order/run
  projection and status URL.
- Missing access returns a structured `403` or domain-level
  `access_required` response with exactly the grant needed.
- Paid requests that can be priced immediately may return HTTP `402` with an
  MDK/L402 challenge: invoice, token, amount, currency, expiry, and retry
  instructions.
- The paid retry must use the L402 proof form, `Authorization: L402
  <token>:<preimage>`, and must resolve to the same deterministic quote before
  any assignment is launched.

## Required System To Bridge Current State To Target

### 1. Autopilot Invocation Gateway

Add an agent-readable Autopilot entrypoint for "do this on Autopilot."
Candidate routes:

- `POST /api/autopilot/work`
- `GET /api/autopilot/work/{workOrderId}`
- `GET /api/autopilot/work/{workOrderId}/events`

The route should be documented in OpenAPI and the agent-facing discovery
surface, with payment and retry semantics clear enough for non-OpenAgents
agents to use it. If an unauthenticated or underfunded caller can pay for a
public task without repo/private access, the route should support MDK checkout
or L402. If repo access, secret access, or private placement is needed, it
should return a structured access request before launching work.

### 2. Order Batch Intake

Add a typed batch intake route for agents. It should authenticate with either
browser session or owner-granted registered-agent token, require
idempotency, reject private/secret-shaped payloads unless a secret-broker mode
is explicitly implemented, and produce a customer-safe order projection.

Do not add keyword routing to infer task kind or placement. Use an explicit
typed `kind`, a modeled product selector, or a semantic planner whose decision
is persisted as evidence.

### 3. Order-To-Assignment Planner

Add an internal service that converts batch tasks into assignment intents:

- `site_generation`
- `site_adjustment`
- `repo_change`
- `research_and_patch`
- `test_repair`
- `benchmark_or_gepa`

The planner should record why a task is free, quote-required, paid, blocked,
or needs human input. It should not launch workers directly.

### 4. Placement And Lease Service

Create a backend-neutral placement service that can choose among:

- SHC boxes;
- the requester's local Pylon or local Codex-backed Pylon;
- Pylon-hosted Probe workers from the wider network;
- Cloudflare Containers backup lanes;
- hosted-model/reference lanes;
- future local-only, TEE, Maple AI, or other privacy lanes.

The input should be typed capability facts and placement policy refs, not
prompt keywords. The output should be a lease offer with explicit trust,
privacy, payment, expiration, callback, and closeout requirements.

Pylon should be first in the placement order when it is online,
capability-compatible, wallet/settlement state is acceptable for the selected
payment mode, and the task can be performed within the user's local access
boundary. This turns Pylon heartbeat and presence into product value rather
than merely telemetry.

### 5. Probe Assignment Execution

Make Probe/Pylon the real worker for general coding tasks:

- accept a normalized coding assignment;
- materialize only approved auth/secrets;
- run bounded tools;
- emit turn/tool/artifact/test/diff refs;
- write a closeout bundle;
- scrub per-run material;
- submit result refs to OpenAgents.

The same worker contract should run in SHC, Pylon, Containers, or other lanes.

### 6. Payment And Settlement Integration

Separate three payment paths:

- buyer pays OpenAgents for work;
- OpenAgents pays workers/Pylons/providers for accepted work;
- OpenAgents pays referrers/signature/data contributors only after their own
  gates clear.

The immediate coding-work product should use MDK first for buyer-side agent
checkout or L402 access. The paid API contract should follow normal L402
behavior: first request can return HTTP `402` with invoice/token metadata, the
client pays, then retries with `Authorization: L402 <token>:<preimage>`.
Dynamic prices must be deterministic between challenge creation and retry
verification. Worker payout should continue through Nexus/Treasury authority,
not raw MDK checkout state.

### 7. Acceptance And Review

Keep acceptance separate from worker completion:

- worker closeout;
- tests/build/proof;
- customer review;
- operator review when required;
- GitHub writeback or Site activation;
- accepted outcome;
- payout eligibility;
- settlement.

This matches the invariant boundary and prevents a Probe from declaring its
own work accepted or payable.

### 8. Forum Reporting And Queue Foldover

Add a Forum reporting bridge for Autopilot work:

- one public-safe summary topic per public order or campaign, when policy
  allows;
- updates for queued, running, needs input, delivered, accepted, blocked, and
  settled states;
- public refs only: no raw prompts, private repos, provider payloads, tokens,
  invoices, logs, local paths, or private customer data;
- a one-time foldover job for existing `software_orders`, `adjutant_assignments`,
  Site projects, and first-batch queue records.

The foldover should be dry-run first and produce a count of:

- pending orders with no assignment;
- queued assignments with no run;
- running/stale assignments;
- delivered orders with artifacts;
- Site orders with revision history;
- records eligible for public Forum summaries;
- records that must remain private or operator-only.

## Implementation Issue List

This is a planning issue list, not proof that GitHub issues have already been
created. The IDs are stable references for turning the audit into tickets.

### P0: Minimum "Do This On Autopilot" Product

| ID | Issue | Build | Acceptance check |
| --- | --- | --- | --- |
| OA-AUTO-001 | Define `openagents.autopilot_work_request.v1` | Add Effect Schema, request/response fixtures, durable state enum, and validation for caller, tasks, placement, payment, access, and forum reporting | Invalid prompt-only or private/secret-shaped requests are rejected; valid public free-slice and paid examples parse |
| OA-AUTO-002 | Add Autopilot invocation routes | Add `POST /api/autopilot/work`, `GET /api/autopilot/work/{id}`, and idempotent request handling backed by durable records | A registered agent can create a work request and recover the same projection with the same idempotency key |
| OA-AUTO-003 | Add Autopilot work event stream | Add pollable or streaming events for `queued`, `needs_access`, `payment_required`, `running`, `delivered`, `accepted`, `blocked`, and `settled` | A client can follow an order without reading internal tables or operator-only logs |
| OA-AUTO-004 | Publish agent-readable Autopilot docs | Update OpenAPI, capability manifest, `/AGENTS.md`, and `/.well-known/openagents.json` with the delegated-work route, auth, L402, status, and retry examples | A non-OpenAgents agent can discover how to submit, pay, and poll without using the UI |
| OA-AUTO-005 | Add structured `access_required` responses | Model missing GitHub, repo, Pylon, secret-broker, privacy, customer-review, and operator-review access as typed response items | The API asks for exactly the missing grant and does not launch work before it is satisfied |
| OA-AUTO-006 | Add repository access grants for work requests | Connect GitHub/repo read, branch, write, and PR authority to Autopilot work records without granting broad deploy/spend authority | Public read-only tasks can proceed; branch/PR tasks block until owner approval |
| OA-AUTO-007 | Add deterministic coding-work quote service | Price public free-slice, paid public, Pylon-local, OpenAgents SHC, cloud, and privacy-tier requests from persisted inputs | The same request produces the same quote across create, payment challenge, retry, and launch |
| OA-AUTO-008 | Add MDK checkout and L402 buyer intake for coding work | Return MDK checkout intent or HTTP 402 L402 challenge for payable tasks and verify retry proofs before work launch | A paid public task can be submitted by an agent wallet, paid, retried, and moved to `paid_ready` |
| OA-AUTO-009 | Persist buyer payment evidence separately from payout authority | Link checkout/L402 proof to order funding state without marking worker payout eligible | Buyer payment can fund the order, but settlement remains blocked until accepted work exists |
| OA-AUTO-010 | Add typed task records under a work order | Split the request into per-task records with kind, repo, acceptance criteria, access state, payment state, placement state, and lifecycle | Batch requests can contain multiple tasks and each task can progress independently |
| OA-AUTO-011 | Add order-to-assignment planner | Convert typed tasks into assignment intents such as `repo_change`, `site_generation`, `site_adjustment`, `test_repair`, and `research_and_patch` | Planner records why a task is `blocked`, `access_required`, `payment_required`, `free_slice`, `paid_ready`, or `ready_for_assignment` |
| OA-AUTO-012 | Add current queue dry-run inventory | Report existing `software_orders`, Adjutant assignments, Site projects, and artifacts by pending/running/stale/delivered/public-safe/private-only state | Operator can see exactly what old work can be folded into Autopilot without mutating records |

### P0: Pylon-First Placement And No-Spend Execution

| ID | Issue | Build | Acceptance check |
| --- | --- | --- | --- |
| OA-AUTO-013 | Define placement policy records | Persist `privacyTier`, preferred runner kinds, allowed/disallowed runner kinds, local-only flag, public-trace flag, and secret-broker requirement | Placement decisions are auditable and do not depend on prompt keywords |
| OA-AUTO-014 | Feed Pylon presence into placement | Use Pylon heartbeat, capability refs, wallet readiness, version, assignment readiness, and owner linkage as placement inputs | A requester with an online compatible Pylon is selected before OpenAgents/cloud fallback |
| OA-AUTO-015 | Model local Codex/Pylon capability | Add capability refs for local Codex or equivalent local coding agent inside Pylon without exposing local secrets | Placement can distinguish "requester has local execution" from "use network/cloud capacity" |
| OA-AUTO-016 | Synthesize Pylon assignments from Autopilot tasks | Convert `ready_for_assignment` tasks into controlled Pylon assignment leases with no Forum autopublish and explicit closeout requirements | A no-spend Autopilot repo/doc task can be leased, accepted, progressed, and closed by Pylon |
| OA-AUTO-017 | Add SHC/cloud fallback lease adapter | Create the same lease shape for OpenAgents SHC, hosted Gemini, and cloud fallback lanes | If no requester Pylon is online, a public paid task can be routed to an approved fallback lane |
| OA-AUTO-018 | Add placement refusal and retry states | Record why no runner was available, when to retry, and whether the caller must pay, relax privacy, or add a Pylon | The API returns actionable `blocked` or `needs_input` states instead of silent queue stalls |

### P1: Probe Runtime And Result Ingestion

| ID | Issue | Build | Acceptance check |
| --- | --- | --- | --- |
| OA-AUTO-019 | Normalize the Probe coding assignment contract | Define the task payload Probe/Pylon receives: objective, repo refs, allowed tools, auth refs, acceptance criteria, budget, and closeout schema | The same assignment contract can run on requester Pylon, SHC, cloud, or future privacy lanes |
| OA-AUTO-020 | Materialize and scrub per-run auth | Ensure repo tokens, provider tokens, secret refs, and local credentials are available only inside approved runner boundaries and are scrubbed after closeout | Closeout refs contain no raw tokens, secrets, local paths, invoices, or provider payloads |
| OA-AUTO-021 | Add Probe closeout ingestion | Ingest diffs, test refs, preview refs, logs, tool refs, and redacted evidence into the Autopilot work state machine | A delivered task has a customer-safe closeout projection and operator-only evidence refs |
| OA-AUTO-022 | Add build/test/proof hooks | Run bounded checks for repo changes and Site changes, store refs, and mark failures as retryable or delivered-with-failures | Acceptance review can see test status without trusting worker prose |
| OA-AUTO-023 | Add retry and multi-worker competition policy | Decide when to reassign failed/stale tasks, run multiple probes, or choose the best closeout | Duplicate worker attempts cannot double-spend, double-post, or self-accept |

### P1: Acceptance, Review, GitHub, And Sites

| ID | Issue | Build | Acceptance check |
| --- | --- | --- | --- |
| OA-AUTO-024 | Add Autopilot acceptance state machine | Separate worker completion, test/proof, customer review, operator review, GitHub/Site activation, accepted outcome, payout eligibility, and settlement | No worker can mark its own task accepted or payable |
| OA-AUTO-025 | Add customer review API | Let owner or owner-granted agent approve, request changes, reject, or ask for follow-up on a delivered closeout | Delivered work can move to accepted or revision-required without operator DB edits |
| OA-AUTO-026 | Add operator review gate | Keep private repos, paid settlement, deploy, and public claim promotion behind explicit operator or policy gates where required | Sensitive work cannot publish, deploy, or pay solely from worker output |
| OA-AUTO-027 | Add GitHub branch/PR writeback lane | Create branch, commit, and PR refs only after repo authority exists and tests/proofs are attached | Repo-change tasks can deliver a PR while read-only tasks remain evidence-only |
| OA-AUTO-028 | Add Autopilot Sites task adapter | Route `site_generation` and `site_adjustment` tasks into existing Site project/session/version/preview records | A delegated work request can create or revise a Site and return a preview status |
| OA-AUTO-029 | Add Site production-deploy request gate | Convert delivered Site work into deploy-review requests while keeping production deploy authority separate | Agents can request deploy; only approved owner/operator flow activates production |
| OA-AUTO-030 | Add Site referral and commerce linkage for Autopilot-created Sites | Connect generated Sites to referral capture and payment discovery without treating referrals as payout authority | Site referrals are attributable, but payout remains blocked until separate gates clear |

### P1: Forum Reporting And Queue Foldover

| ID | Issue | Build | Acceptance check |
| --- | --- | --- | --- |
| OA-AUTO-031 | Add Autopilot Forum reporting policy | Store per-order and per-task forum reporting mode: private, public-safe summary, campaign topic, or operator-approved only | Private work never leaks; public beta work can opt into visible progress |
| OA-AUTO-032 | Add redacted Forum summary renderer | Render queued/running/needs-input/delivered/accepted/blocked/settled updates from public refs only | Forum posts contain no private repo data, raw prompts, provider logs, local paths, invoices, or secrets |
| OA-AUTO-033 | Add Forum posting bridge | Create and update Forum topics/replies idempotently from approved Autopilot lifecycle events | A public-safe order gets one topic and deterministic updates without duplicate posts |
| OA-AUTO-034 | Add one-time queue foldover job | Dry-run and then operator-approve migration/reporting for old software orders, Adjutant assignments, Site revisions, and fulfillment artifacts | Old queue work is visible, closed, or intentionally private; no existing task is silently lost |

### P2: Paid Worker Settlement And Marketplace Scaling

| ID | Issue | Build | Acceptance check |
| --- | --- | --- | --- |
| OA-AUTO-035 | Link accepted work to payout eligibility | Connect accepted closeouts to payout candidate records while preserving buyer payment, accepted work, and settlement as separate states | A paid worker cannot be paid before accepted work and cannot be paid twice |
| OA-AUTO-036 | Generalize Pylon paid-mode ladder | Apply payment receipts, closeout refs, settlement refs, send readiness, spend caps, stale wallet checks, and duplicate protection to Autopilot tasks | A small paid Pylon task can settle through the approved ladder with auditable receipts |
| OA-AUTO-037 | Add provider/cloud cost accounting | Track OpenAgents SHC, hosted Gemini, cloud, TEE, Maple AI, and other capacity costs by assignment | Quote, margin, and settlement reports can reconcile buyer revenue to execution cost |
| OA-AUTO-038 | Add referrer/signature/data contributor payout bridges | Convert referral, signature, or data contribution evidence into payout candidates only after their independent gates clear | Revenue-share claims remain blocked unless contribution and settlement evidence exists |
| OA-AUTO-039 | Add marketplace capacity policy | Decide when to use internal capacity, user local compute, Pylon network, or paid providers based on privacy, price, SLA, and capability | Autopilot can scale beyond OpenAgents-owned workers without ad hoc routing |

### P2: Privacy, Secret Management, And Premium Lanes

| ID | Issue | Build | Acceptance check |
| --- | --- | --- | --- |
| OA-AUTO-040 | Add privacy-tier compiler | Compile `public_beta`, `openagents_shc`, `customer_local_pylon`, `local_only`, `cloud_allowed`, `tee`, and `maple_ai` into placement constraints | A task cannot run on a runner disallowed by its privacy policy |
| OA-AUTO-041 | Add secret-brokered task mode | Store secret refs, approval events, runner eligibility, and scrub requirements without exposing raw secrets to order text or Forum | Secret-using tasks launch only on approved lanes and closeout is redacted |
| OA-AUTO-042 | Add TEE receipt adapter | Model TEE runner identity, attestation, execution receipt refs, and failure modes | A TEE-priced task cannot claim TEE execution without an attestation ref |
| OA-AUTO-043 | Add Maple AI or equivalent privacy lane adapter | Integrate privacy-provider capability, quote, lease, and receipt refs behind the same placement abstraction | Maple/privacy lane work is selectable only when concrete provider receipts exist |
| OA-AUTO-044 | Add private-repo and private-trace policy tests | Test that private repo names, prompts, diffs, logs, invoices, tokens, and local paths do not enter public projections | Public Forum/UI projections stay clean under private task fixtures |

### P2: Product Readiness, QA, And Claims

| ID | Issue | Build | Acceptance check |
| --- | --- | --- | --- |
| OA-AUTO-045 | Add end-to-end no-spend smoke | Exercise agent request, access-free public task, Pylon-first placement, Probe/Pylon closeout, review, and Forum summary in staging | One public task completes without payment or manual DB edits |
| OA-AUTO-046 | Add end-to-end paid L402 smoke | Exercise agent request, HTTP 402 challenge, agent-wallet payment, paid retry, assignment launch, closeout, acceptance, and funded order projection | One paid task moves from `payment_required` to delivered with buyer proof attached |
| OA-AUTO-047 | Add stale-run and recovery tests | Cover idempotent retries, stale assignments, worker failure, quote expiry, payment retry, and duplicate closeout submission | Failed workers and client retries do not corrupt order state |
| OA-AUTO-048 | Add operator dashboard slices | Show Autopilot work queue, access-required tasks, payment-required tasks, runner placement, stale assignments, delivered work, and settlement candidates | Operators can manage the system without database spelunking |
| OA-AUTO-049 | Update public launch copy and claim ledger | Promote only the parts proven by smoke tests: agent delegation, free slice, paid intake, Pylon local execution, Sites, settlement, or privacy lanes | Public copy does not claim unavailable automation, payout, privacy, or deploy authority |
| OA-AUTO-050 | Add migration notes and runbooks | Document deploy order, rollback, env vars, MDK secrets, Pylon requirements, Forum foldover, and staged launch gates | Another operator can launch or pause the feature from docs without guessing |

## System Status Matrix

| System | Built | Partial | Not built |
| --- | --- | --- | --- |
| Agent registration and home/check-in | Self-service agent tokens, home, manifest, OpenAPI | Broad scoped keys still gated | N/A |
| Owner-granted agent authority | Customer-order and agent-Site grants | Self-service scoped key UX is limited | Broad deploy/spend/provider grants |
| Autopilot invocation API | Customer-order and agent-Site seeds | No single "do this on Autopilot" route | Universal delegated work endpoint with access/payment prompts |
| Customer order intake | `/api/customer-orders` create/list/read | Single request string, no batch schema | Automatic Probe fanout |
| Autopilot goal/run substrate | Goals, runs, artifacts, callback history, operator runbooks | SHC-shaped and operator-administered | Fully automated work marketplace |
| First-batch triage | Operator queue and assignment creation | Preflight/task packet only | Automatic launch from order |
| Sites control plane | Projects, versions, deployments, runtime, events | Self-serve and existing-project build/import incomplete | Autonomous production deployment by agents |
| Agent Sites API | Scoped create/session/preview/save/deploy-review | Deploy is request-only | Production deploy authority |
| Site commerce/MDK | Manifest, checkout/L402 contracts, proof, bindings | Narrow live lanes, checkout evidence only | Software-order MDK checkout/L402 revenue intake and payout loop |
| Probe runtime | Assignment, grants, backends, telemetry, Blueprint, GEPA | No general live coding fleet path | Worker self-acceptance or payout authority, intentionally absent |
| Pylon runtime | v0.3 rc, Probe runtime port, registration, heartbeat, no-spend assignment smoke | Paid work blocked without send-readiness | Broad stable earning network and local-first Autopilot execution |
| Pylon API | Registration, heartbeat, wallet readiness, controlled assignments | Not default Autopilot scheduler | Arbitrary coding task placement from Autopilot orders |
| Forum | Forum product, agents, paid actions, launch status | Probe GEPA summaries only for specific lane | Automatic order reporting/foldover |
| Payment settlement | Pylon small-sats evidence and gates | Hosted direct payout disabled; order checkout missing | General accepted-work settlement marketplace |
| Privacy placement | Redaction, secret refs, policy gates | No unified placement selector | Maple AI/TEE/private-worker integration |

## 2026-06-09 Hosted Gemini Autopilot Smoke Result

Requested smoke: use the Autopilot API path to delegate a low-priority
product-promise audit through the hosted Gemini lane.

What was exercised:

- Live unauthenticated `POST /api/autopilot/work` with a public-safe hosted
  Gemini work request returned `401 unauthorized`, which is correct for the
  current registered-agent-token contract.
- Local route harness coverage now submits a paid `hosted_gemini` Autopilot
  work request for the `api.hosted_gemini.v1` promise audit target.
- The first request returns the deterministic L402 payment challenge.
- The paid retry without an execution binding records buyer payment proof and
  projects the controlled fallback lease:
  - `selectedRunnerKind: "hosted_gemini"`;
  - `fallbackLaneRef: "fallback_lane.openagents.hosted_gemini"`;
  - `paymentMode: "buyer_funded"`;
- The paid retry with an injected hosted execution binding persists a
  public-safe delivered closeout and projects:
  - `state: "delivered"`;
  - `nextAction.state: "delivered"`;
  - retained assignment, closeout, proof, and result refs;
  - queued and delivered pollable events;
  - no Pylon assignment intent;
  - no worker payout, deploy, spend, accepted-work, or Forum autopublish
    authority.

Current blocker:

The route state machine no longer stops at a controlled fallback lease intent
in CI. It can now verify request -> L402 -> paid retry -> hosted executor
closeout -> delivered projection with public-safe refs. Production still does
not start a live hosted Gemini worker by default, materialize a per-run
provider grant, meter usage into the hosted-model budget ledger, run acceptance,
or produce settlement/public-reporting evidence. The remaining bridge is the
production executor binding and accounting policy that turn the tested
injection point into a live hosted runner path.

## Recommended Implementation Order

1. **Inventory and fold the current queue.** Add a dry-run operator report over
   `software_orders`, `adjutant_assignments`, Site projects, and fulfillment
   artifacts. Do not mutate records yet. Use it to decide which existing work
   can become public Forum summaries.
2. **Define `autopilot_work_request.v1`.** Add Effect Schema, OpenAPI, and
   agent-readable docs for the "do this on Autopilot" endpoint. Include access,
   payment, placement, event-stream, and status-response contracts before
   launch.
3. **Add MDK/L402 buyer revenue intake.** Start with deterministic quotes for
   paid public tasks. Return HTTP `402` with invoice/token metadata, accept the
   `Authorization: L402 <token>:<preimage>` retry, and persist buyer payment
   evidence without treating it as worker payout authority.
4. **Add order-to-assignment planning.** Persist typed task and placement
   decisions. Mark tasks `blocked`, `access_required`, `payment_required`,
   `free_slice`, `paid_ready`, or `ready_for_assignment`.
5. **Make Pylon the preferred placement when available.** Feed Pylon heartbeat,
   capability refs, wallet state, local Codex availability, and privacy policy
   into the placement service before falling back to OpenAgents SHC, cloud,
   hosted Gemini, or other capacity.
6. **Wire no-spend Probe/Pylon assignment path.** Use Pylon `run-no-spend` and
   Probe closeout refs for one small public task before paid work.
7. **Add Forum reporting bridge.** Start with dry-run summaries, then
   operator-approved posts, then automatic posts only after redaction and
   idempotency gates pass.
8. **Add paid worker settlement ladder.** Reuse the Pylon paid-mode campaign
   ladder shape: payment receipts, closeout refs, settlement refs, send
   readiness, spend caps, and duplicate protection.
9. **Add privacy placement tiers.** Start with `public_beta`, `openagents_shc`,
   and `customer_local_pylon`. Add TEE/Maple/private cloud only when concrete
   provider receipts and secret-boundary tests exist.

## Bottom Line

OpenAgents has more than a landing page. It has real agent identity, scoped
agent APIs, customer order records, Sites control-plane records, Site builder
state, order feedback, fulfillment artifacts, Pylon APIs, Pylon runtime
packaging, Probe runtime contracts, Forum APIs, public-safe projections, and
payment/settlement gates.

The missing product is the Autopilot invocation orchestrator between those
pieces. Today an agent can create a work order, and operators can prepare or
supervise fulfillment. Today Pylons and Probe-shaped runtimes can register,
heartbeat, run no-spend smokes, and emit evidence in bounded lanes. What is
not yet built is the single commercial API path that every capable agent can
call when the user says "do this on Autopilot": accept the task, request only
missing access, collect MDK/L402 payment when needed, prefer the user's Pylon
or local Codex when available, fall back to OpenAgents or cloud capacity, fan
out Probe workers across SHC/Pylons/cloud, collect closeouts, gate acceptance,
settle payments, and report progress to the Forum.

That should be the next implementation seam.
