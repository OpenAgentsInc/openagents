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
   local Codex, cloud sandboxes, GCloud credits, TEEs, Maple AI, or later
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
OpenAgents-owned capacity, cloud credits, or paid network capacity to perform
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
- ambitious work should move into paid credits or checkout;
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
  later credit the Site owner if referred users become paying customers.

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
- Runner backend docs for SHC primary, Cloudflare Containers backup, and GCloud
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
      "gcloud_credit",
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
- GCloud/reference lanes;
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
   GCloud credits, or other capacity.
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
