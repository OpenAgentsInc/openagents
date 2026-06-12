# Autopilot Coder Implementation Log

This log records the sequential implementation of the Autopilot coder issue
backlog.

## OA-AUTO-001: Work Request Contract

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4575`

Status: implemented.

Implemented:

- Added `openagents.autopilot_work_request.v1` schemas in
  `apps/openagents.com/workers/api/src/autopilot-work-request.ts`.
- Added a durable work-state enum, caller/task/repository/access/forum/
  placement/payment policy schemas, response fixtures, and decode-time
  validation.
- Added conformance fixtures for a valid public free-slice request and a paid
  L402 request.
- Rejected prompt-only payloads, empty task batches, private repositories,
  unsafe secret/payment/wallet/raw-prompt material, and public traces on
  private/premium privacy tiers.
- Added focused regression tests in
  `apps/openagents.com/workers/api/src/autopilot-work-request.test.ts`.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-request.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-002: Autopilot Invocation Routes

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4576`

Status: implemented.

Implemented:

- Added D1 migration `0140_autopilot_work_orders.sql` for durable Autopilot
  work-order records keyed by owner and idempotency hash.
- Added `apps/openagents.com/workers/api/src/autopilot-work-routes.ts` with:
  - `POST /api/autopilot/work`
  - `GET /api/autopilot/work/{workOrderRef}`
  - registered-agent bearer authentication through the existing customer-order
    grant path;
  - required `Idempotency-Key` handling for create;
  - idempotent replay returning the original projection;
  - D1-backed store and in-memory-testable route factory.
- Wired the route family into the Worker dispatcher.
- Added focused tests in
  `apps/openagents.com/workers/api/src/autopilot-work-routes.test.ts`.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-request.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-003: Autopilot Work Event Stream

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4577`

Status: implemented.

Implemented:

- Added public-safe Autopilot work event projections for queued work and the
  current terminal gate/state:
  - `needs_access`
  - `payment_required`
  - `running`
  - `delivered`
  - `blocked`
- Added the pollable route:
  - `GET /api/autopilot/work/{workOrderRef}/events`
- Added cursor recovery using `?after=` or `Last-Event-ID`.
- Added server-sent event formatting when the caller sends
  `Accept: text/event-stream` or `?stream=sse`.
- Kept the event payload customer-safe: no raw prompt body, private repo data,
  operator logs, invoices, secrets, or worker payout authority are exposed.
- Added focused tests for JSON polling, cursor/SSE recovery, and read-scope
  authorization.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-request.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-004: Agent-Readable Autopilot Docs

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4578`

Status: implemented.

Implemented:

- Added Autopilot delegated-work discovery to the live `/AGENTS.md` guidance
  and synced the public asset copy.
- Updated the exported `/AGENTS.md` sha256 used by the capability manifest.
- Added generated onboarding guidance and a delegated Autopilot coding-agent
  example prompt.
- Added `submit_autopilot_work`, `autopilot_work_status`, and
  `autopilot_work_events` entries to `/.well-known/openagents.json`.
- Added OpenAPI schemas and operations for:
  - `POST /api/autopilot/work`
  - `GET /api/autopilot/work/{workOrderRef}`
  - `GET /api/autopilot/work/{workOrderRef}/events`
- Documented idempotency, status recovery, SSE/event cursor recovery, MDK/L402
  payment handling, and the boundary that buyer payment is not deploy,
  accepted-work, payout, or settlement authority.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/openagents-capability-manifest-routes.test.ts src/openagents-openapi-routes.test.ts src/openagents-agent-onboarding-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-005: Structured Access-Required Responses

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4579`

Status: implemented.

Implemented:

- Added missing access request kinds for:
  - `customer_review`
  - `github_account_link`
  - `repository_selection`
- Added typed `accessRequirements` to Autopilot work projections with:
  - `accessRequestRef`
  - `taskRef`
  - `kind`
  - `grantAction`
  - `reasonRef`
  - `ownerActionRef`
  - `status: "missing"`
  - `requiredBeforeLaunch: true`
- Mapped missing GitHub, repository, Pylon, secret broker, privacy,
  customer-review, and operator-review requirements to exact owner/operator
  actions.
- Preserved the gate: requests with missing access stay in `access_required`,
  emit no buyer payment challenge, and do not move into queued/running work.
- Tightened the unsafe-value scanner so typed access-kind enum values such as
  `secret_broker` are allowed while arbitrary secret-shaped strings remain
  rejected.
- Added focused regression coverage for the full missing-access projection.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts src/autopilot-work-request.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-006: Repository Access Grants For Work Requests

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4580`

Status: implemented.

Implemented:

- Added explicit access kinds for branch write and pull request authority:
  - `github_branch_write`
  - `github_pull_request`
- Added `repositoryAuthorities` to Autopilot work projections so callers can
  see read/write/PR authority state per task without internal tables.
- Treated public GitHub `github_repo_read` requests as satisfied by public
  repository visibility, allowing public read-only tasks to proceed.
- Kept branch/write/PR work blocked as `access_required` until owner approval
  is represented by explicit access grants.
- Preserved authority boundaries in the projection:
  - `deployAuthority: false`
  - `spendAuthority: false`
- Updated event tests so `needs_access` events are generated only for explicit
  write-gated work, not public read-only work.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts src/autopilot-work-request.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-007: Deterministic Coding-Work Quote Service

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4581`

Status: implemented.

Implemented:

- Added a pure deterministic quote service in
  `apps/openagents.com/workers/api/src/autopilot-work-quote.ts`.
- Priced persisted request inputs across:
  - public free-slice work;
  - paid public work;
  - requester/Pylon-local runner preferences;
  - OpenAgents SHC and cloud runner preferences;
  - privacy-tier, private-trace, and secret-broker requirements.
- Preserved upstream persisted quotes when `quoteRef` and `quotedAmountCents`
  are present.
- Added `quote` to Autopilot work projections so create, idempotent replay,
  status recovery, and payment challenge refs derive from the same stored
  request.
- Kept quote/payment separate from worker payout, acceptance, settlement, and
  deploy authority.
- Added focused quote service tests and route-level quote stability coverage.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-quote.test.ts src/autopilot-work-routes.test.ts src/autopilot-work-request.test.ts src/openagents-openapi-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-008: MDK Checkout And L402 Buyer Intake

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4582`

Status: implemented.

Implemented:

- Added D1 migration `0141_autopilot_work_payment_proofs.sql` to persist
  public-safe buyer payment proof refs on Autopilot work orders.
- Added the durable `paid_ready` state.
- Added payment challenge projections for payable work, including:
  - L402 `WWW-Authenticate` challenge headers;
  - MDK checkout intent/url refs;
  - deterministic quote refs and amount cents.
- Changed unpaid payable create/replay responses to return HTTP 402 while
  preserving the durable work order and quote.
- Verified idempotent paid retries:
  - L402 proof via `X-OpenAgents-L402`;
  - MDK checkout proof via `X-OpenAgents-MDK-Checkout-Proof`.
- Promoted paid retries to `paid_ready` without treating buyer payment as
  worker payout, acceptance, settlement, deploy, or public-claim authority.
- Added focused route coverage for unpaid challenge, paid retry, and detail
  recovery.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts src/autopilot-work-request.test.ts src/autopilot-work-quote.test.ts src/l402-payment-headers.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-009: Buyer Funding Versus Worker Payout Authority

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4583`

Status: implemented.

Implemented:

- Added a `funding` projection to Autopilot work-order responses.
- Represented buyer funding separately from worker payout authority with:
  - `buyerFundingState`
  - `buyerPaymentProofRef`
  - `fundedAmountCents`
  - `quoteRef`
  - `settlementBlockedReasonRef`
  - `settlementEligible: false`
  - `workerPayoutEligible: false`
- Kept free/no-worker-payout work in `not_required` funding state.
- Kept unpaid payable work in `payment_required` funding state.
- Promoted paid L402/MDK retries to `funded` while still blocking settlement
  on `settlement.accepted_work_required`.
- Updated OpenAPI summary text so agent-readable docs expose the funding
  projection.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts src/autopilot-work-request.test.ts src/autopilot-work-quote.test.ts src/l402-payment-headers.test.ts src/openagents-openapi-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-010: Typed Task Records Under A Work Order

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4584`

Status: implemented.

Implemented:

- Added typed `tasks` records to Autopilot work-order projections.
- Each task record now exposes:
  - `taskRef`
  - `kind`
  - `repository`
  - `acceptanceCriteriaRefs`
  - task-scoped `accessRequirements`
  - `accessState`
  - `paymentState`
  - `placementState`
  - `lifecycleState`
- Kept top-level `taskRefs` and aggregate access/payment fields for backward
  compatibility.
- Derived per-task states independently from the persisted work request and
  order funding state, so a batch can contain ready tasks and access-blocked
  tasks at the same time.
- Updated OpenAPI summary text so agent-readable docs advertise typed task
  records.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts src/autopilot-work-request.test.ts src/autopilot-work-quote.test.ts src/l402-payment-headers.test.ts src/openagents-openapi-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-011: Order-To-Assignment Planner

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4585`

Status: implemented.

Implemented:

- Added `autopilot-work-assignment-planner.ts` as a pure planner for typed
  Autopilot task records.
- Converted task kinds into assignment kinds:
  - `repo_change`
  - `site_generation`
  - `site_adjustment`
  - `test_repair`
  - `research_and_patch`
- Added `assignmentIntents` to work-order projections with planner state,
  reason refs, task ref, repository ref, access/payment/placement state, and
  explicit authority-denial fields.
- Recorded planner reasons for:
  - `access_required`
  - `payment_required`
  - `blocked`
  - `free_slice`
  - `paid_ready`
  - `ready_for_assignment`
- Preserved the boundary that assignment intent is not deploy, spend, accepted
  work, payout, or settlement authority.
- Updated OpenAPI summary text so agent-readable docs expose assignment
  intents.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-assignment-planner.test.ts src/autopilot-work-routes.test.ts src/autopilot-work-request.test.ts src/autopilot-work-quote.test.ts src/l402-payment-headers.test.ts src/openagents-openapi-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-012: Current Queue Dry-Run Inventory

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4586`

Status: implemented.

Implemented:

- Added an operator-only dry-run inventory endpoint:
  - `GET /api/operator/orders/triage/autopilot-foldover-inventory`
- The report reads existing records without mutation and returns:
  - `dryRun: true`
  - `mutatesRecords: false`
  - per-source inventory items
  - aggregate counts by source kind, lifecycle state, and public/private
    safety state
- Covered the existing foldover sources:
  - `software_orders`
  - `adjutant_assignments`
  - `site_projects`
  - `site_builder_artifacts`
- Classified records as `pending`, `running`, `stale`, or `delivered`.
- Classified records as `public_safe` or `private_only`.
- Added route coverage proving the endpoint reports old work and does not
  mutate source records.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/operator-order-triage-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-013: Placement Policy Records

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4587`

Status: implemented.

Implemented:

- Added D1 migration `0142_autopilot_work_placement_policies.sql`.
- Persisted normalized `placement_policy_json` on Autopilot work orders.
- Added an auditable `placementPolicy` record to work-order projections with:
  - `privacyTier`
  - `preferredRunnerKinds`
  - `allowedRunnerKinds`
  - `disallowedRunnerKinds`
  - `localOnlyAllowed`
  - `publicTraceAllowed`
  - `requiresSecretBroker`
  - `reasonRefs`
- Explicitly exposed:
  - `auditable: true`
  - `promptKeywordRouting: false`
- Updated OpenAPI summary text so agent-readable docs expose placement policy
  records.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts src/autopilot-work-request.test.ts src/autopilot-work-assignment-planner.test.ts src/autopilot-work-quote.test.ts src/l402-payment-headers.test.ts src/openagents-openapi-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-014: Pylon Presence Placement Input

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4588`

Status: implemented.

Implemented:

- Added `autopilot-work-placement-selector.ts`.
- The selector evaluates Pylon registration facts:
  - owner linkage
  - heartbeat timestamp and status
  - assignment-readiness capability ref
  - wallet readiness
  - client version compatibility
  - latest resource mode
- Added `placementDecision` to Autopilot work-order projections.
- The placement decision selects an online compatible requester Pylon before
  OpenAgents/cloud fallback.
- When no compatible requester Pylon is available, the decision records the
  fallback runner kind or a no-runner blocker.
- Added optional Autopilot route dependency for Pylon registrations so current
  deployments keep fallback behavior until the Pylon store is wired.
- Updated OpenAPI summary text so agent-readable docs expose Pylon-aware
  placement decisions.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-placement-selector.test.ts src/autopilot-work-routes.test.ts src/autopilot-work-request.test.ts src/autopilot-work-assignment-planner.test.ts src/autopilot-work-quote.test.ts src/l402-payment-headers.test.ts src/openagents-openapi-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-015: Local Codex/Pylon Capability Model

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4589`

Status: implemented.

Implemented:

- Added public-safe local coding-agent capability refs:
  - `capability.pylon.local_codex`
  - `capability.pylon.local_coding_agent`
- Updated the Pylon placement selector so requester-local execution requires:
  - owner-linked Pylon;
  - fresh online heartbeat;
  - compatible client version;
  - wallet readiness;
  - assignment readiness;
  - local coding-agent readiness.
- Added `localExecutionReady` to Pylon placement candidate projections.
- Preserved secret boundaries: local capability is represented only by public
  refs, never by local paths, tokens, provider credentials, or raw Codex state.
- Added tests proving placement can distinguish requester-local execution from
  OpenAgents/network/cloud fallback.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-placement-selector.test.ts src/autopilot-work-routes.test.ts src/autopilot-work-request.test.ts src/autopilot-work-assignment-planner.test.ts src/autopilot-work-quote.test.ts src/l402-payment-headers.test.ts src/openagents-openapi-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-016: Pylon Assignment Synthesis

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4590`

Status: implemented.

Implemented:

- Added `autopilot-work-pylon-assignment-synthesizer.ts`.
- Converted ready Autopilot assignment intents into controlled Pylon assignment
  intents when placement selects a requester Pylon.
- Added `pylonAssignmentIntents` to work-order projections.
- Synthesized no-spend Pylon assignment fields:
  - `paymentMode: "unpaid_smoke"`
  - `forumAutoPublishAllowed: false`
  - `requiredCapabilityRefs`
  - `spendCapRefs`
  - `closeoutPathRefs`
  - `resultExpectationRefs`
  - `rollbackRefs`
- Preserved the boundaries that Pylon closeout is not accepted-work,
  settlement, payout, deploy, spend, or Forum autopublish authority.
- Updated OpenAPI summary text so agent-readable docs expose controlled Pylon
  assignment intents.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-pylon-assignment-synthesizer.test.ts src/autopilot-work-placement-selector.test.ts src/autopilot-work-routes.test.ts src/autopilot-work-request.test.ts src/autopilot-work-assignment-planner.test.ts src/autopilot-work-quote.test.ts src/l402-payment-headers.test.ts src/openagents-openapi-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-017: SHC And Cloud Fallback Lease Adapter

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4591`

Status: implemented.

Implemented:

- Added `autopilot-work-fallback-lease-adapter.ts`.
- Converted ready Autopilot assignment intents into controlled fallback lease
  intents when placement selects an approved fallback runner.
- Covered the initial fallback lanes:
  - `openagents_shc`
  - `shc`
  - `cloud_sandbox`
  - `hosted_gemini`
- Added `fallbackLeaseIntents` to work-order projections.
- Preserved the same closeout and safety shape as Pylon assignment intents:
  - `forumAutoPublishAllowed: false`
  - explicit closeout path refs
  - public-safe result expectation refs
  - rollback refs that block deploy without owner acceptance
  - spend caps separated from worker payout authority
- Modeled funded fallback execution as `paymentMode: "buyer_funded"` without
  granting worker payout, deploy, spend, or accepted-work authority.
- Kept unpaid fallback work as `paymentMode: "unpaid_smoke"` for future smoke
  paths.
- Updated OpenAPI summary text so agent-readable docs expose controlled
  SHC/cloud fallback lease intents.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-fallback-lease-adapter.test.ts src/autopilot-work-pylon-assignment-synthesizer.test.ts src/autopilot-work-placement-selector.test.ts src/autopilot-work-routes.test.ts src/autopilot-work-request.test.ts src/autopilot-work-assignment-planner.test.ts src/autopilot-work-quote.test.ts src/l402-payment-headers.test.ts src/openagents-openapi-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-018: Placement Refusal And Retry States

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4592`

Status: implemented.

Implemented:

- Added placement availability fields to placement decisions:
  - `availabilityState`
  - `callerActionRefs`
  - `refusalReasonRefs`
  - `retryAfterSeconds`
- Added retry guidance for owner-linked Pylons that are otherwise eligible but
  have stale heartbeat state.
- Added needs-input guidance for local-only or Pylon-only policies with no
  eligible requester Pylon.
- Added work-order `nextAction` so the API can surface:
  - `payment_required` when the caller must pay first;
  - `needs_input` when the caller must add or restart a Pylon or relax privacy
    or runner policy;
  - `retry_later` when a retry window is appropriate;
  - `ready` when placement and funding are not blocking assignment.
- Preserved the separation between buyer payment, placement, worker payout,
  accepted-work, deploy, spend, and public-claim authority.
- Updated OpenAPI summary text so agent-readable docs expose placement refusal
  and retry state.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-placement-selector.test.ts src/autopilot-work-routes.test.ts src/autopilot-work-fallback-lease-adapter.test.ts src/autopilot-work-pylon-assignment-synthesizer.test.ts src/autopilot-work-request.test.ts src/autopilot-work-assignment-planner.test.ts src/autopilot-work-quote.test.ts src/l402-payment-headers.test.ts src/openagents-openapi-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## 2026-06-09: Hosted Gemini Execution Closeout Bridge

Status: implemented in the Worker route harness.

Implemented:

- Added a durable `execution_closeout_json` column for Autopilot work orders.
- Added a route dependency hook for ready hosted Gemini work execution.
- Preserved the production default that no hosted work is executed unless a
  real executor binding is installed.
- Added public-safe execution closeout refs to the work-order projection:
  assignment refs, closeout refs, proof refs, result refs, and runner kind.
- Moved delivered work to `state: "delivered"` with
  `nextAction.state: "delivered"` and queued/delivered event projection.
- Ensured delivered tasks no longer synthesize fresh assignment or fallback
  lease intents.
- Preserved authority boundaries:
  - accepted-work authority remains false;
  - worker payout authority remains false;
  - deploy and spend authority remain false;
  - Forum autopublish remains false;
  - settlement still requires a later accepted-work path.
- Updated the hosted Gemini product-promise and launch-dashboard copy to say
  the route harness is verified while the public paid product remains red.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts src/autopilot-work-assignment-planner.test.ts src/autopilot-work-fallback-lease-adapter.test.ts src/autopilot-work-request.test.ts src/autopilot-work-quote.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-019: Production Pylon Placement Store Wiring

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4614`

Status: implemented.

Implemented:

- Added a production Pylon API store dependency to Autopilot work routes.
- Wired the live Worker route factory to
  `makeD1PylonApiStore(openAgentsDatabase(env))`.
- Kept the route-level test injection for selector fixtures, but changed the
  production default path from an empty Pylon list to
  `listRegistrations(1000)`.
- Preserved the existing placement selector policy over production records:
  owner linkage, active status, wallet readiness, fresh heartbeat, compatible
  client version, assignment readiness, and local coding-agent capability.
- Added route coverage proving an online compatible requester Pylon is selected
  through the production-store dependency without the test-only
  `pylonRegistrations` injection.
- Preserved public-safe projection shape: no local paths, tokens, wallet
  material, provider payloads, or raw machine state.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts src/autopilot-work-placement-selector.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-020: Durable Autopilot-To-Pylon Assignment Leases

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4611`

Status: implemented.

Implemented:

- Added an idempotent Autopilot dispatch transition that turns ready
  requester-Pylon assignment intents into durable `pylon_api_assignments`
  records.
- Reused the existing Pylon assignment lease store and public-safe assignment
  projection instead of creating a parallel worker queue.
- Marked Autopilot work orders `queued_or_running` only after at least one
  Pylon assignment lease exists.
- Updated assignment planning so queued/running work does not keep emitting
  fresh ready-for-assignment intents.
- Linked generated Pylon assignments to the Autopilot work order and task refs
  through public-safe `taskRefs`, acceptance criteria refs, and deterministic
  assignment refs.
- Preserved no-spend mode and authority separation:
  - `paymentMode: "unpaid_smoke"`;
  - Forum autopublish disabled;
  - no deploy authority;
  - no accepted-work authority;
  - no payout or settlement authority.
- Added route coverage proving idempotent Autopilot retries do not create
  duplicate Pylon assignments.
- Added route coverage proving the existing Pylon API can poll and accept the
  Autopilot-created lease.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts src/autopilot-work-assignment-planner.test.ts src/pylon-api-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-021: Normalized Autopilot Coding Assignment Payload

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4612`

Status: implemented.

Implemented:

- Added `openagents.autopilot_coding_assignment.v1` in
  `apps/openagents.com/workers/api/src/autopilot-coding-assignment.ts`.
- Defined the shared requester-Pylon, fallback, SHC/cloud, hosted Gemini, and
  future privacy-lane assignment payload with:
  - ref-only objective;
  - task kind;
  - public GitHub repository refs;
  - branch/write/PR/deploy/read/spend authority refs;
  - allowed tool kinds;
  - auth refs;
  - acceptance criteria refs;
  - budget, quote, payment challenge, spend-cap, timeout, and settlement mode;
  - trace policy that explicitly forbids raw prompts, provider payloads,
    runner logs, and source archives;
  - closeout schema for diffs, test/blocker evidence, result refs, and
    no-self-acceptance.
- Added mapping from current Autopilot task records plus requester-Pylon and
  fallback assignment intents into the normalized assignment payload.
- Kept the payload public-safe: private repositories are rejected until a
  private/secret-broker lane is modeled, and unsafe local paths, raw prompts,
  provider payloads, payment material, wallet material, source archives, and
  secrets are rejected before decode or projection.
- Added tests for requester-Pylon mapping, fallback mapping, unsafe fixture
  rejection, and private repository rejection.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-coding-assignment.test.ts`
- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts src/autopilot-work-assignment-planner.test.ts src/pylon-api-routes.test.ts src/autopilot-coding-assignment.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-022: Real No-Spend Pylon Worker Loop For Public Autopilot Tasks

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4610`

Status: implemented.

Implemented:

- Added durable storage and projection support for the normalized Autopilot
  coding assignment payload on `pylon_api_assignments`.
- Embedded the normalized assignment payload in no-spend requester-Pylon
  Autopilot assignments so Pylon clients can consume the same public-safe
  contract that fallback lanes use.
- Extended the Pylon assignment API with a worker closeout event separate from
  operator accepted-work closeout:
  - worker closeout can submit artifact, proof, build, test, result, blocker,
    and closeout refs;
  - worker closeout moves the assignment to `closeout_submitted`;
  - worker closeout does not grant accepted-work, payout, settlement, deploy,
    spend, or Forum autopublish authority.
- Updated Pylon assignment polling to consume current OpenAgents assignment
  projections, normalize the embedded coding assignment payload, and preserve
  legacy local lease polling for harnesses.
- Added bearer-token client support for registered-agent Pylon loops while
  preserving the signed-header path for local harnesses.
- Added bounded no-spend Pylon runtime execution that:
  - polls a public assignment;
  - accepts it after local admission checks;
  - submits progress;
  - submits artifact/proof refs;
  - submits a public-safe worker closeout;
  - records local closeout state without raw logs, local paths, wallet
    material, provider payloads, or source archives.
- Added proof-trace and visibility handling for `worker_closeout` events.
- Kept the product boundary explicit: worker closeout is still not customer
  acceptance, paid work verification, settlement eligibility, or deploy
  authority.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts src/pylon-api-routes.test.ts src/autopilot-coding-assignment.test.ts src/nexus-pylon-visibility-routes.test.ts src/artanis-pylon-proof-trace.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`
- `bun test --cwd apps/pylon tests/assignment.test.ts tests/live-worker-loop-smoke.test.ts`

## OA-AUTO-023: Real Autopilot Worker Closeout And Artifact Ingestion

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4613`

Status: implemented.

Implemented:

- Extended Autopilot execution closeout records to carry public-safe worker
  artifact, blocker, build, preview, proof, result, summary, and test refs.
- Added a Pylon worker-closeout ingestion helper that:
  - infers the Autopilot `workOrderRef` and `taskRef` from the Pylon assignment
    and normalized coding assignment payload;
  - verifies the assignment owner matches the Autopilot agent user;
  - validates that all projected closeout refs are public-safe;
  - records the closeout through the Autopilot work store as `delivered`.
- Wired the Pylon `worker_closeout` route to the Autopilot closeout ingester in
  production.
- Kept worker closeout separate from owner/customer acceptance, accepted-work
  closeout, settlement, payout, deploy, spend, and Forum publication authority.
- Added route coverage proving a Pylon worker closeout moves the Autopilot work
  order to delivered and emits delivered events.
- Added a redaction regression proving unsafe local-path-shaped closeout refs
  are rejected before Autopilot delivery persistence.
- Updated the Pylon runtime closeout contract to submit preview and summary
  refs in addition to artifact/proof/build/test/result/blocker refs.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts src/pylon-api-routes.test.ts src/autopilot-coding-assignment.test.ts src/nexus-pylon-visibility-routes.test.ts src/artanis-pylon-proof-trace.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`
- `bun test --cwd apps/pylon tests/assignment.test.ts tests/live-worker-loop-smoke.test.ts`

## OA-AUTO-024: Customer Review And Revision API

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4615`

Status: implemented.

Implemented:

- Added persisted Autopilot review states:
  - `accepted`
  - `rejected`
  - `revision_required`
- Added `POST /api/autopilot/work/{workOrderRef}/review` for owner-granted
  registered agents with `customer_orders.write` scope.
- Added public-safe review request handling for:
  - `accept`
  - `reject`
  - `request_changes`
- Persisted review decisions with actor agent refs, idempotency key hash,
  decision refs, rejection refs, and revision request refs.
- Added D1 migration `0147_autopilot_work_review_decisions.sql` for durable
  review decision storage.
- Kept review authority separate from worker closeout, buyer payment,
  worker payout, settlement, deploy, spend, and Forum publication.
- Updated work projections, task lifecycle projections, next actions, and event
  streams for accepted, rejected, and revision-required states.
- Added route coverage for accept, reject, request-changes, idempotent replay,
  before-delivery conflict, unsafe review refs, missing write scope, and
  non-owner denial.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-025: No-Spend Autopilot Coder End-To-End Smoke

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4616`

Status: implemented.

Implemented:

- Added `apps/openagents.com/workers/api` script
  `smoke:autopilot-coder:no-spend`.
- Added a named smoke test that drives the no-spend path through route calls:
  - public Autopilot work submission;
  - requester Pylon placement;
  - durable Pylon assignment creation;
  - Pylon assignment acceptance;
  - worker closeout submission;
  - Autopilot delivered detail/event recovery;
  - owner-granted review acceptance.
- Added retained projection redaction scanning for private paths,
  wallet/payment material, provider payloads, raw prompts/logs/source archives,
  secret material, and forbidden hosted-infrastructure wording.
- Documented the smoke command and expected no-spend authority boundaries in
  `docs/autopilot-coder/no-spend-e2e-smoke.md`.

Verification:

- `bun run --cwd apps/openagents.com/workers/api smoke:autopilot-coder:no-spend`

## OA-AUTO-026: Signed L402 Payment Issuance And Verification Gate

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4617`

Status: implemented for the signed L402 route boundary; live external payment
movement remains a separate verifier/smoke gap.

Implemented:

- Replaced Autopilot L402 proof-ref-only funding with signed credential
  verification against the stored work order.
- Derived the Autopilot L402 challenge from durable work-order state:
  - quote amount and currency;
  - challenge ref;
  - request-body digest;
  - owner, agent, and work-order scope refs;
  - endpoint and product refs;
  - 15-minute expiry.
- Added private `x-openagents-l402-credential` issuance on `402` responses
  when the MDK route signing boundary is configured.
- Added an explicit payment verifier hook that fails closed when missing or
  rejected; a signed credential alone no longer funds payable work.
- Kept MDK checkout proof headers payment-required until checkout creation and
  reconciliation are wired.
- Tightened payment persistence so only unpaid `payment_required` work can move
  to `paid_ready`.
- Wired the production Autopilot route to the existing MDK route signing
  boundary while leaving live proof verification unconfigured/fail-closed.
- Added route coverage for unpaid, malformed, unverified, expired, mismatched,
  verified, and idempotent replay L402 retries.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## OA-AUTO-027: Paid Autopilot Coder End-To-End Smoke

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4618`

Status: implemented as a CI-safe paid route smoke; live external payment
movement remains part of the real paid-work epic.

Implemented:

- Added `apps/openagents.com/workers/api` script
  `smoke:autopilot-coder:paid`.
- Added a named smoke test that drives the payable public path through route
  calls and the Pylon API:
  - payable Autopilot work submission;
  - signed L402 `402` challenge;
  - verifier-approved paid retry with the same idempotency key;
  - funded work projection;
  - requester Pylon assignment recovery;
  - Pylon assignment acceptance;
  - worker closeout submission;
  - Autopilot delivered detail/event recovery;
  - owner-granted review acceptance.
- Projected funded Pylon coding assignments as
  `payable_pending_settlement`, while the normalized coding assignment budget
  still exposes `buyer_funded` without granting worker payout authority.
- Kept settlement, payout, deploy, spend, and Forum publication authority
  blocked after buyer funding, worker closeout, and customer acceptance.
- Added retained projection redaction scanning for private paths,
  wallet/payment material, provider payloads, raw prompts/logs/source archives,
  secret material, and forbidden hosted-infrastructure wording.
- Documented the smoke command and its live-payment limitation in
  `docs/autopilot-coder/paid-e2e-smoke.md`.

Verification:

- `bun run --cwd apps/openagents.com/workers/api smoke:autopilot-coder:paid`

## Epic #4619: Real No-Spend Pylon Execution Path Closeout

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4619`

Status: complete for the CI-safe no-spend Pylon execution contract; staging/live
deployment remains a separate environment run.

Implemented across child issues:

- Production Pylon placement input.
- Durable Pylon assignment leases.
- Normalized coding assignment payloads.
- Pylon accept/progress/artifact/proof/closeout flow.
- Autopilot delivery ingestion from Pylon worker closeouts.
- Owner-granted review decisions.
- A documented no-spend Autopilot Coder smoke.

Closeout clarification:

- The no-spend smoke does not pass test-only `pylonRegistrations` and does not
  use an injected hosted executor.
- It reads Pylon presence from the Pylon API store, creates a durable
  assignment lease, accepts/closes through the Pylon assignment API, ingests
  delivered refs back into Autopilot, and scans retained projections for
  private material.
- The Pylon runtime regression covers local assignment normalization,
  acceptance, proof/progress/closeout receipts, cancellation handling, and
  public-safe closeout refs.

Verification:

- `bun run --cwd apps/openagents.com/workers/api smoke:autopilot-coder:no-spend`
- `bun test --cwd apps/pylon tests/assignment.test.ts tests/live-worker-loop-smoke.test.ts`

## Epic #4620: Paid Work Payment Verification Path

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4620`

Status: complete for the repo-level paid route verification contract; staging
or live external payment movement remains a deployment/integration smoke gap.

Implemented:

- Persist L402 buyer-payment challenges on Autopilot `402` responses when the
  buyer-payment ledger store is configured.
- Added `verifyAutopilotL402PaymentProofFromBuyerLedger`, which accepts paid
  retries only when the buyer-payment ledger has:
  - a redeemed record for the signed challenge;
  - the same public-safe proof ref supplied by the paying agent;
  - an issued receipt;
  - an active entitlement covering the signed L402 scopes;
  - matching product, challenge, and amount refs;
  - a matched reconciliation event.
- Wired the production Autopilot route to the D1 buyer-payment ledger verifier.
- Upgraded the paid Autopilot Coder smoke to use the ledger verifier instead
  of a proof-ref allowlist.
- Kept buyer payment separate from worker payout, accepted-work, settlement,
  deploy, spend, and Forum publication authority.
- Updated the paid L402 boundary, paid smoke docs, and current audit.

Verification:

- `bun run --cwd apps/openagents.com/workers/api smoke:autopilot-coder:paid`
- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-routes.test.ts src/buyer-payment-ledger.test.ts src/l402-credential-service.test.ts src/l402-payment-headers.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## MVP M3 / Issue #4761: Own-Pylon Free Lane Policy

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4761`

Status: complete for the product-policy projection and route-level guarantee.

Implemented:

- Added a typed Autopilot work pricing policy with lane-to-meter mappings:
  `requester_pylon` own-job work has no buyer debit and `openagents_shc`
  fallback work uses USD-credit metering.
- Surfaced policy-derived placement reason refs in work-order projections:
  `placement.reason.placed_on_your_pylon_free` and
  `placement.reason.your_pylon_unavailable_hosted_metered`.
- Added `pricingPolicy.activeLane` to Autopilot work-order projections so web
  and Pylon clients can render lane/meter state from typed data.
- Bound route tests to the free-lane guarantee: requester-Pylon work creates a
  no-spend assignment, has no payment challenge, and writes zero buyer debit
  rows; SHC fallback projects the metered lane.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-work-placement-selector.test.ts src/autopilot-work-routes.test.ts`

## MVP M4 / Issue #4762: Cloud Pylon Deployment Path

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4762`

Status: complete for the supported headless install/auth/supervision path and
the code affordance that lets a cloud node pick up owner no-spend assignments.

Implemented:

- Added `apps/pylon/scripts/install-cloud-node.sh`, a Linux/systemd installer
  for the Pylon v0.3 source RC. It creates a dedicated service user, installs
  the repo, bootstraps Pylon state, writes a root-owned env file, installs a
  restart-on-boot service, and redacts secrets in dry-run output.
- Added an opt-in headless assignment worker loop for `pylon node`, enabled by
  `PYLON_ASSIGNMENT_WORKER=1`, so a cloud Pylon can continuously poll and run
  eligible no-spend owner assignments.
- Added `apps/pylon/docs/cloud-node-deployment.md` covering owner identity,
  BYOK credential posture, Claude Agent capability verification, loopback-only
  attach, restart/upgrade, compromise response, and the public-safe evidence
  bundle for the 24h unattended run.
- Linked the cloud deployment path from `apps/pylon/README.md`.

Verification:

- `bash -n apps/pylon/scripts/install-cloud-node.sh`
- `OPENAGENTS_AGENT_TOKEN=dummy-token ANTHROPIC_API_KEY=dummy-key PYLON_REF=pylon.cloud.test PYLON_DISPLAY_NAME='Test Cloud Pylon' apps/pylon/scripts/install-cloud-node.sh --dry-run`
- `cd apps/pylon && bun test tests/control-protocol.test.ts tests/assignment.test.ts`

Note: direct `bunx tsc -p tsconfig.json --noEmit` in `apps/pylon` remains
blocked by preexisting repo-wide node16/import-extension and TUI type issues;
the focused runtime tests and script syntax/dry-run checks pass.

## RK1 / Issue #4805: Shared Agent Runtime Kernel Schema

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4805`

Status: complete for the schema-only kernel contract.

Implemented:

- Added `packages/agent-runtime-schema` as a schema-only Effect Schema package
  with no provider SDK or Vercel AI SDK dependency.
- Exported the RK1 run, event, part, tool invocation, external invocation,
  artifact ref, usage record, visibility, redaction policy, adapter kind, loop
  kind, and lifecycle contract.
- Added reusable fixture event logs for fixture, native model, external agent,
  and hosted loops.
- Added redaction and lifecycle helpers so public event logs reject raw prompts,
  raw shell logs, provider payloads, secrets, and local paths before
  persistence or projection.
- Covered every filed event tag. The filed RK1 tag list contains 32 tags even
  though the prose says 31; the implementation preserves and tests all listed
  tags.

Verification:

- `bun run --cwd packages/agent-runtime-schema test`
- `bun run --cwd packages/agent-runtime-schema typecheck`

## RK2 / Issue #4806: Existing Pylon Loops Behind AgentRuntimeAdapter

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4806`

Status: complete for the no-behavior-change Pylon adapter wrap.

Implemented:

- Added `apps/pylon/src/agent-runtime-adapter.ts` with the shared
  `AgentRuntimeAdapter` contract: `kind`, `canRun`, `start`, and `cancel`.
- Wrapped the existing Claude and Codex assignment executors without changing
  their inputs, execution behavior, or closeout wire records.
- Added OpenCode normalization through the existing `runOpencodeStream` helper.
- Added deterministic `test_fixture` and reserved `hermes` adapters.
- Added a replay reducer that rebuilds projection state from runtime events
  alone.
- Added typed `tool.denied` event construction and cancellation coverage that
  emits `run.cancelled`.

Verification:

- `bun run --cwd apps/pylon test tests/agent-runtime-adapter.test.ts tests/claude-agent-executor.test.ts tests/codex-agent-executor.test.ts`
- `bun run --cwd packages/agent-runtime-schema test`
- `bun run --cwd apps/pylon smoke:claude-agent-task`
- `bun run --cwd apps/pylon smoke:codex-agent-task`

Note: direct `bunx tsc -p apps/pylon/tsconfig.json --noEmit` remains blocked
by preexisting repo-wide import-extension, implicit-unknown, and TUI typing
issues. The new adapter source does not appear in the remaining typecheck
diagnostics after its imports were made NodeNext-clean.

## RK3 / Issue #4807: Native OpenAgents Effect Model Loop

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4807`

Status: complete for the deterministic native adapter slice.

Implemented:

- Added `apps/pylon/src/openagents-native-runtime.ts` with the
  `openagents_native` adapter behind the same RK2 `AgentRuntimeAdapter`
  contract.
- Isolated the experimental Effect AI dependency surface behind local Effect
  service/layer facades: `OpenAgentsNativeLanguageModel` and
  `OpenAgentsNativeToolkit`.
- Added a Schema-recognized native task payload and Schema-typed fixture
  summary tool input/output.
- Emitted `model.*`, `tool.*`, `run.completed`, `run.interrupted`,
  `run.failed`, and `run.cancelled` kernel events end to end.
- Added deterministic test language-model/toolkit layers.
- Kept tool selection typed by `allowedToolRefs`; no prompt keyword inference
  is used for adapter or tool routing.

Verification:

- `bun run --cwd apps/pylon test tests/openagents-native-runtime.test.ts tests/agent-runtime-adapter.test.ts tests/claude-agent-executor.test.ts tests/codex-agent-executor.test.ts`
- `bun run --cwd packages/agent-runtime-schema test`

Typecheck note:

- `bunx tsc -p apps/pylon/tsconfig.json --noEmit` remains blocked by the same
  preexisting broad Pylon issues noted in RK2. The new native runtime and
  adapter files do not appear in the remaining typecheck diagnostics.

## MVP M5 / Issue #4763: Card-On-File And Auto Top-Up

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4763`

Status: complete for the Stripe/D1 card-on-file policy surface and the
server-side auto-top-up trigger.

Implemented:

- Added migration `0170_billing_auto_top_up.sql` for Stripe saved payment
  method metadata, auto-top-up policy rows, auto-top-up events, and the
  `stripe_auto_top_up` ledger source.
- Extended the billing summary with saved-card status, policy state, monthly
  cap usage, and recent auto-top-up events. The D1 ledger remains the balance
  authority.
- Added Stripe SetupIntent creation and succeeded-SetupIntent save handling.
  OpenAgents stores Stripe customer/payment-method IDs plus brand/last4/expiry
  only; raw card data remains with Stripe.
- Added the auto-top-up policy API and an off-session PaymentIntent trigger
  that writes exactly one idempotent positive ledger row on success.
- Recorded missing-card, declined-card, skipped, and monthly-cap events. A
  declined or missing-card path pauses the policy; a cap-reached path leaves
  the out-of-credits suspend/cancel/notify behavior intact.
- Updated the logged-in Billing page to show card status, policy values,
  cap usage, event history, and actions for card setup, enable/disable, and
  manual top-up check.
- Updated billing docs and the OpenAPI anti-staleness route allowlist.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test -- src/billing.test.ts src/billing-routes.test.ts src/openagents-openapi-routes.test.ts src/omni-services.test.ts`
- `cd apps/openagents.com && bunx tsc -p workers/api/tsconfig.json --noEmit`
- `cd apps/openagents.com && bunx tsc -p apps/web/tsconfig.json --noEmit`

## RK4 / Issue #4808: Worker Kernel Event Ingestion And Projection

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4808`

Status: complete for the Worker ingestion/projection module.

Implemented:

- Added `workers/api/src/agent-runtime-kernel.ts` with schema-decoded
  `AgentRuntimeEvent` ingestion and an append-only repository contract.
- Added duplicate and non-append sequence rejection before persistence.
- Added public projection rebuild from the event log with `generatedAt`,
  declared staleness metadata, artifact refs, blocker refs, terminal state,
  event count, and latest event id.
- Documented and projected the visibility split: events remain stored with
  their declared visibility/redaction class, while public projections read only
  public-visible events.
- Kept acceptance, payout, and public-claim authority explicitly false in
  projections.
- Added `apps/openagents.com/INVARIANTS.md` coverage for the kernel ingestion
  and projection boundary.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/agent-runtime-kernel.test.ts src/public-projection-staleness.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`
- `bun run --cwd packages/agent-runtime-schema test`

OpenAPI note:

- RK4 did not add an HTTP route in this slice, so there was no new served
  OpenAPI path to register. The ingestion/projection module is route-ready and
  remains adapter-agnostic.

## RK5 / Issue #4809: Projection-Backed Status Surfaces And Failure Smokes

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4809`

Status: complete for the shared workroom/TUI projection-presenter slice.

Implemented:

- Added a shared `projectAgentRuntimeSurfaceStatus` presenter in
  `packages/agent-runtime-schema` so web workroom and Pylon TUI status rows
  derive from the same kernel projection fields.
- Added Worker `projectAgentRuntimeWorkroomStatus` over the RK4 public run
  projection, carrying `generatedAt`, staleness, event count, artifact refs,
  blocker refs, review refs, and verification refs without raw adapter logs.
- Added Pylon TUI store state for agent-runtime status rows populated only from
  projections, not adapter transcripts.
- Added RK5 smokes for cancellation, `tool.denied`, budget stop, and adapter
  failure. Each smoke drives real adapter event streams, rebuilds the Worker
  public projection, feeds that exact projection into the TUI store, and
  asserts identical public-safe status rows.
- Verified the decision-queue-ready tool denial event carries invocation id,
  tool ref, status, and blocker refs without starting or completing the tool.

Verification:

- `bun run --cwd apps/pylon test tests/agent-runtime-surface-smokes.test.ts tests/tui-store.test.ts tests/openagents-native-runtime.test.ts`
- `bun run --cwd apps/openagents.com/workers/api test src/agent-runtime-kernel.test.ts src/public-projection-staleness.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`
- `bun run --cwd packages/agent-runtime-schema test`

## Pack A Operationalization / Issues #4813-#4823

Issue parent: `https://github.com/OpenAgentsInc/openagents/issues/4813`

Status: filed and documented as the proof/supervision acceptance overlay for
the active #4786 Autopilot MVP sprint.

Implemented:

- Added Pack A as a tracked issue set:
  - #4813 parent issue.
  - #4814 task supervisor.
  - #4815 schedule and continuation receipts.
  - #4816 notification and attention coordinator.
  - #4817 mobile and web companion projection.
  - #4818 smoke receipt authority.
  - #4819 artifact and receipt ledger.
  - #4820 structured event-log replay discipline.
  - #4821 usage budget and cost-stop projections.
  - #4822 permission and approval contract.
  - #4823 accessibility and non-interactive contract.
- Added the Pack A tracking table and timing guidance to
  `docs/autopilot-coder/terminal-agent-systems/2026-06-11-terminal-agent-systems-operationalization-roadmap.md`.
- Added a #4786 epic comment clarifying timing: Pack A is an acceptance
  overlay, not a replacement ladder. It should not pause unrelated M/A/P work,
  but it does gate proof/claim closure for M9, M10, M14, and the proof side of
  A1 where those issues cite Pack A receipts.
- Updated the terminal-agent README and this top-level Autopilot Coder README
  so the Pack A setup is discoverable from the folder index.

Verification:

- `gh issue list --repo OpenAgentsInc/openagents --state open --label pack-a`
- `gh issue view 4813 --repo OpenAgentsInc/openagents`
- `gh issue view 4786 --repo OpenAgentsInc/openagents --comments`

Timing rule:

- Product-surface rungs can close on their scoped acceptance while Pack A
  issues carry remaining operational hardening.
- MVP-gating proof or door-open claims should wait for their relevant Pack A
  receipts.
- Closed rungs should not be reopened only to hold operational debt; cross-link
  the relevant Pack A issue and clarify the claim boundary instead.

## Pack A Chronos / Issues #4814, #4815, #4820: Runtime Supervision Events

Issue refs:

- `https://github.com/OpenAgentsInc/openagents/issues/4814`
- `https://github.com/OpenAgentsInc/openagents/issues/4815`
- `https://github.com/OpenAgentsInc/openagents/issues/4820`

Status: complete for the Wave 1 shared event/replay foundation. Full
overnight SHC/own-Pylon/cloud-Pylon proof remains Gate-owned under #4768 and
#4772.

Implemented:

- Added `autopilot-pack-a-runtime-supervision.ts` with a typed
  `PackARuntimeEvent` boundary for Pack A task and schedule subjects.
- Added a `TaskSupervisor` boundary over an append-only repository contract for
  created, started, output, artifact, usage, waiting, completion, failure,
  cancellation, and notification events.
- Added schedule event helpers for created, fired, skipped, failed, cancelled,
  and continuation-queued receipts, including no-double-fire protection by
  occurrence ref.
- Added replay reducers for task and schedule projections carrying
  `generatedAt`, the shared staleness contract, refs-only output/artifact/usage
  evidence, blocker refs, receipt refs, and explicit non-authority flags.
- Updated the `apps/openagents.com` invariant ledger with the Pack A
  supervision append/replay/redaction contract.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/autopilot-pack-a-runtime-supervision.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck` is currently
  blocked by unrelated pre-existing errors in
  `src/autopilot-decision-routes.test.ts` and
  `src/autopilot-rate-limit-rotation-smoke.test.ts`; no reported error
  references the Chronos files.

Deferred to Gate:

- Cross-lane M10 overnight proof across SHC and own-Pylon/cloud-Pylon.
- M14 door-open closeout and parent #4786/#4813 proof comments.

## Pack B Operationalization / Issues #4824-#4830

Issue parent: `https://github.com/OpenAgentsInc/openagents/issues/4824`

Status: filed and documented as the account, credential, and policy hardening
overlay for provider/account expansion.

Readiness assessment:

- Reviewed
  `docs/autopilot-coder/terminal-agent-systems/2026-06-11-terminal-agent-systems-operationalization-roadmap.md`.
- Checked the current open issue set with `gh issue list`. The remaining open
  Autopilot set is #4768, #4771, #4772, #4777, #4781, #4782, #4783, and #4786;
  #4749 remains open as a separate W3 evaluation issue.
- M8 (#4766) and M9 (#4767) are now closed, while M13 provider peers (#4771)
  remains open. That matches the roadmap trigger for Pack B: credential,
  policy, telemetry, security, and retention guardrails should be tracked
  before broad provider-peer claims close.

Implemented:

- Added Pack B as a tracked issue set:
  - #4824 parent issue.
  - #4825 authentication and credential storage boundary.
  - #4826 resolved settings/configuration snapshots.
  - #4827 security review gate for provider peers and account leases.
  - #4828 telemetry/privacy fixtures for account health and provider routing.
  - #4829 retention/deletion rules for credential, lease, telemetry, and
    policy records.
  - #4830 minimal managed policy snapshots for team and approved-user gates.
- Updated the terminal-agent operationalization roadmap with the Pack B
  readiness assessment, tracking table, and timing rule.
- Updated the terminal-agent README, the top-level Autopilot Coder README, and
  the open-issue delegation plan so Pack B ownership and discoverability are
  explicit.

Timing rule:

- Pack B should run in parallel with remaining #4786 Gate work.
- Pack B should not reopen closed M8/M9 issues just to hold operational debt.
- Pack B should gate broad provider-peer closure in #4771 and future work that
  relies on provider credentials, account telemetry, managed policy state,
  retention guarantees, or provider security review.
- #4768 and #4772 should cite Pack B only if their proof evidence depends on
  those provider/account/policy surfaces.
