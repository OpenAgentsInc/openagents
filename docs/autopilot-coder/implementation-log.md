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

## M13 Live Gemini Provider Gate / Issue #4771

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4771`

Status: closed for the scoped M13 live non-Codex provider leg.

Implemented and verified:

- Added migration `0173_provider_account_peer_provider_checks.sql` so
  provider-account, connection-attempt, grant, lease, and sanity-check tables
  accept `anthropic_claude` and `google_gemini` beside `chatgpt_codex`.
- Deployed `openagents-autopilot` Worker version
  `a10d2d08-fd81-4f50-ba01-e06ee90822ed`.
- Connected a production Google Gemini API-key BYOK account with public ref
  `provider-account_ref_m13_google_gemini_d2fc43560602`.
- Confirmed the provider pool projection returns that account as eligible,
  healthy, connected, and visible to registered-agent readers.
- Acquired a live required-provider lease, issued and resolved the grant, and
  released the lease as succeeded.
- Ran a Probe `gemini_api` backend call using the production-resolved grant
  payload shape and verified secret redaction.

Evidence:

- `docs/autopilot-coder/2026-06-12-m13-live-gemini-provider-gate-record.md`

Remaining boundary:

- This closes M13's live non-Codex leg only. M10, M14, market proof,
  settlement visibility, and Pack B hardening remain separate gates.

## P1 Market-Key Live Publisher Probe / Issues #4777 #4781 #4782 #4783

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4777`

Status: market-key signing blocker cleared; live negotiated labor job remains
open for independent-provider execution and settlement evidence.

Implemented and verified:

- Generated a dedicated market signing key, stored it only in ignored local
  secrets, and uploaded it to `openagents-autopilot` as
  `FORUM_WORK_REQUEST_MARKET_SECRET_KEY`.
- Deployed `openagents-autopilot` Worker version
  `f87df619-8678-40ad-872d-5ae35e953a80`.
- Posted a ref-only no-spend Forum work request for the #4773 A1
  parity-matrix slice.
- Verified `POST /api/forum/work-requests` returned `201` with work request
  `f3da4627-246c-444d-885a-0f779964a779`, relay ref
  `relay.public.market.0a2b94b3a5372b3a5cf8cbeb1325da9b`, and kind-5934 job
  event `d480e175984bb3afafa92162438c9b56a1399b5631f9f88110fea11673520327`.
- Queried the owned market relay by event id and confirmed the kind-5934 event
  is retrievable.
- Polled the work request eight times over two minutes; it remained `open`
  with zero offers and no accepted quote.
- The #4773-backed row was later expired after #4773 closed, so the live
  order book is correctly empty again and P1/P5 need a fresh currently-open
  target.

Evidence:

- `docs/labor/2026-06-12-p1-market-key-live-publisher-probe.md`

Remaining boundary:

- #4777/#4781/#4782/#4783 still require an independent contributor quote,
  requester acceptance, escrow reserve, provider execution, validator
  acceptance, release, payout ladder, and settlement visibility receipts.

## PB1 / Issue #4825: Provider Account Credential Boundary

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4825`

Status: implemented for the shared provider-account credential-boundary
projection contract.

Implemented:

- Added `provider-account-credential-boundary.ts` with a Pack B credential
  boundary projection over provider accounts, credential refs, auth grants,
  active lease refs, artifact refs, and receipt refs.
- The projection exposes `accountRef`, `credentialRef`, lease authority,
  typed blocker refs, reconnect action refs, cache invalidation refs, and
  safe joined evidence refs without exposing `providerSecretRef` or raw
  credential material.
- Revoked grants, expired grants, disconnected accounts, reauth-required
  health, deleted accounts, and missing credential refs now reduce to typed
  blockers before dependent work can claim lease authority.
- Added redaction tests for account/grant/lease/artifact/receipt joins and raw
  credential-shaped refs.
- Updated the `apps/openagents.com` invariant ledger with the Pack B
  credential-boundary projection rule.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/provider-account-credential-boundary.test.ts src/provider-account-lease-policy.test.ts src/provider-account-api-key.test.ts`

## PB2 / Issue #4826: Provider Account Effective Config Snapshots

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4826`

Status: implemented for the shared provider/account configuration snapshot
resolver.

Implemented:

- Added `provider-account-effective-config.ts` with explicit config precedence
  across `default`, `environment`, `organization`, `team`, `repository`,
  `user`, `device`, and `runtime` layers.
- Added safe effective config projections for provider, budget, approval,
  telemetry, retention, and routing decisions. The projection exposes config
  refs, caveat refs, source layers, value tags, blocker refs, and denial refs;
  it does not expose raw environment values or secrets.
- Missing required settings and invalid values now resolve to typed blockers
  instead of silent fallback.
- Added redaction tests for raw secret material in config refs, caveats, and
  values.
- Updated the `apps/openagents.com` invariant ledger with the Pack B effective
  config snapshot rule.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/provider-account-effective-config.test.ts`

## PB3 / Issue #4827: Provider Peer Security Review Gate

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4827`

Status: implemented for the typed provider-peer security-review gate and the
dated Pack B security review record.

Implemented:

- Added `provider-account-security-review.ts` with a typed security gate for
  provider-peer expansion, account connect, and lease-selection scopes.
- The gate requires ToS, credential-boundary, threat-model,
  telemetry/privacy, retention-policy, redaction-fixture, and
  revocation-fixture refs before broad provider-peer status can be approved.
- High-risk provider-account flows additionally require approval, denial,
  rollback, incident-boundary, and debug-boundary refs.
- Scoped exceptions remain possible for narrow existing slices, but the
  projection keeps blocker refs visible and cannot erase the missing review
  evidence.
- Added
  `docs/autopilot-coder/2026-06-11-provider-peer-security-review.md` as the
  dated Pack B security review record tied to #4771.
- Updated the `apps/openagents.com` invariant ledger with the Pack B
  provider-peer security review gate.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/provider-account-security-review.test.ts`

## PB4 / Issue #4828: Provider Account Telemetry Privacy Fixtures

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4828`

Status: implemented for the shared provider-account telemetry/privacy
projection and fixture contract.

Implemented:

- Added `provider-account-telemetry-privacy.ts` with a typed projection for
  account-health, rate-limit, low-credit, cooldown, reset-hint, reconnect,
  lease-utilization, and provider-routing telemetry.
- Aggregate telemetry exposes metric refs, counters, durations, statuses,
  provider ids, provider-account classes, caveat refs, source refs, redaction
  fixture refs, debug/support bundle refs, and freshness metadata only.
- `local_only` telemetry is ref-only outside the local/debug boundary, and
  `off` telemetry remains disabled.
- Account-health, rate-limit, low-credit, cooldown, reset-hint, and reconnect
  telemetry now require redaction fixture refs or produce typed blockers.
- Added
  `docs/autopilot-coder/2026-06-11-provider-account-telemetry-privacy-fixtures.md`
  to document telemetry modes, sharing policy, required fixtures, and
  freshness/staleness semantics.
- Updated the `apps/openagents.com` invariant ledger with the Pack B
  telemetry/privacy projection rule.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/provider-account-telemetry-privacy.test.ts`

## PB5 / Issue #4829: Provider Account Retention And Deletion Rules

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4829`

Status: implemented for the shared provider-account retention/deletion policy
projection and fixture contract.

Implemented:

- Added `provider-account-retention-policy.ts` with declared retention class,
  deletion behavior, and projection invalidation behavior for Pack B
  credentials, account leases, account-health telemetry, provider-routing
  decisions, policy snapshots, reconnect state, debug/support records,
  artifacts, and receipts.
- Credential revocation, account deletion, team deletion, and user deletion
  now produce dependent lease invalidation refs, typed dependent blockers,
  provider-account cache invalidation refs, and reconnect action refs where
  applicable.
- Retention expiry invalidates affected projection caches without inventing
  live lease blockers when the expired class is not active account authority.
- Tombstones, deletion receipts, retained audit refs, artifact refs, and
  receipt refs are projected as safe refs only and reject raw credentials, raw
  prompts, private repo data, raw provider responses, shell output, transcripts,
  local paths, and other private material.
- Added
  `docs/autopilot-coder/2026-06-11-provider-account-retention-deletion-rules.md`
  to document user, team, account, credential, and retention-expiry behavior.
- Updated the `apps/openagents.com` invariant ledger with the Pack B
  retention/deletion policy rule.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/provider-account-retention-policy.test.ts`

## PB6 / Issue #4830: Minimal Managed Policy Snapshots

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4830`

Status: implemented for the minimal provider-account and team-budget managed
policy snapshot contract.

Implemented:

- Added `provider-account-managed-policy.ts` with a typed resolver for
  provider-account lease/run, work-order, receipt, and team-budget decisions.
- The resolver turns organization, team, repository, user, device/local,
  provider, budget, retention, and telemetry policy refs into a stable
  `effectivePolicyRef`.
- Provider allowlists, provider disallow reason refs, approved-user gates,
  budget caveats, retention caveats, telemetry caveats, and run/lease/work
  order/receipt attachment refs are projected as public-safe evidence refs.
- Active policy can allow or deny with typed denial refs. Stale and unknown
  policy states produce typed public denial refs before normal allowlist or
  budget checks.
- Added
  `docs/autopilot-coder/2026-06-11-provider-account-managed-policy-snapshots.md`
  to document the minimal managed-policy boundary and non-goals.
- Updated the `apps/openagents.com` invariant ledger with the Pack B
  managed-policy snapshot rule.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/provider-account-managed-policy.test.ts`

## Pack B Parent / Issue #4824: Account, Credential, And Policy Hardening

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4824`

Status: implemented as the parent closeout for #4825-#4830.

Implemented:

- #4825 PB1 credential-boundary projections.
- #4826 PB2 effective configuration snapshots.
- #4827 PB3 provider-peer security review gates.
- #4828 PB4 telemetry/privacy fixtures.
- #4829 PB5 retention/deletion policy projections.
- #4830 PB6 managed-policy snapshots.
- Updated the original terminal-agent-systems operationalization roadmap with
  final Pack B implementation status and the current timing rule for #4771,
  #4786, #4768, and #4772.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/provider-account-managed-policy.test.ts src/provider-account-retention-policy.test.ts src/provider-account-telemetry-privacy.test.ts src/provider-account-security-review.test.ts src/provider-account-effective-config.test.ts src/provider-account-credential-boundary.test.ts`

## PC1 / Issue #4832: Repository And Worktree Identity Snapshots

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4832`

Status: implemented for the Pack C repository/worktree identity projection
contract.

Implemented:

- Added `pack-c-repo-worktree-identity.ts` with typed repository and worktree
  identity projections for Pack C delivery evidence.
- Repository snapshots carry repository refs, host, owner/name, visibility,
  trust tier, default branch, pinned commit refs, remote digest refs,
  data-scope refs, and caveat refs.
- Worktree snapshots carry workspace refs, worktree refs, branch refs,
  base/head commit refs, cleanliness, sandbox profile refs, and retention refs.
- Projections include `generatedAt`, `observedAt`, `staleAt`, `ageMs`,
  freshness, status, and typed blocker refs for incomplete identity.
- Added branch-ref parsing and unsafe material rejection for private remotes,
  local paths, shell fragments, credentials, raw prompts, and private repo
  content.
- Added
  `docs/autopilot-coder/2026-06-12-pack-c-repo-worktree-identity.md` and
  updated the original terminal-agent-systems roadmap and app invariant ledger.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/pack-c-repo-worktree-identity.test.ts`

## PC2 / Issue #4833: Change Capture And Diff Review Artifacts

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4833`

Status: implemented for the Pack C change-capture and diff-review artifact
projection contract.

Implemented:

- Added `pack-c-change-capture.ts` with digest-and-summary-only change capture
  projections for Pack C delivery evidence.
- Change captures carry repository/worktree refs, base/head refs, file summary
  refs, patch digest refs, verification refs, diagnostic refs, review caveat
  refs, authority receipt refs, visibility, public-safety state, freshness
  metadata, and typed blocker refs.
- Captures block on missing verification, missing patch digest, missing
  writeback authority, stale or blocked worktree identity, and unsafe public
  visibility.
- Added unsafe material rejection for raw patches, raw file contents, raw
  shell material, private repo data, local paths, provider payloads,
  credentials, wallet/payment material, and raw prompts.
- Added `docs/autopilot-coder/2026-06-12-pack-c-change-capture.md` and
  updated the original terminal-agent-systems roadmap and app invariant ledger.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/pack-c-change-capture.test.ts src/pack-c-repo-worktree-identity.test.ts`

## PC3 / Issue #4834: File, Shell, And Workspace Authority Boundary

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4834`

Status: implemented for the Pack C workspace/file/shell delivery-evidence
authority contract.

Implemented:

- Added `pack-c-workspace-authority.ts` with typed evidence projections for
  file, shell, verification, and workspace-bound operations.
- Workspace evidence now carries workspace refs, sandbox refs, operation kind,
  command intent refs, allowed command refs, allowed path refs, touched path
  refs, approval refs, timeout refs, cancellation refs, redaction class,
  redaction receipt refs, and typed blockers.
- Unsafe operations produce typed blockers for out-of-scope paths, missing
  approval, disallowed command intent, sandbox mismatch, timeout,
  cancellation, and missing public redaction.
- Added unsafe material rejection for raw shell logs, raw commands, raw
  prompts, local filesystem paths, private repo content, provider payloads,
  credentials, wallet/payment material, and customer-private data.
- Added `docs/autopilot-coder/2026-06-12-pack-c-workspace-authority.md` and
  updated the original terminal-agent-systems roadmap and app invariant ledger.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/pack-c-workspace-authority.test.ts src/pack-c-change-capture.test.ts src/pack-c-repo-worktree-identity.test.ts`

## PC4 / Issue #4835: Delivery Authority And PR Readiness Receipts

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4835`

Status: implemented for the Pack C delivery readiness and PR draft receipt
projection contract.

Implemented:

- Added `pack-c-delivery-readiness.ts` with typed delivery readiness
  projections over repository/worktree identity refs, change capture refs,
  verification refs, GitHub writeback authority refs, review refs, and
  human-merge caveat refs.
- PR draft readiness can be `ready`, `blocked`, or `scoped_exception`, with
  blockers for missing change capture, missing writeback authority, missing
  verification, missing review refs, missing human-merge caveats, stale or
  blocked identity/capture evidence, stale projection freshness, and unsafe
  public visibility.
- Market and agent delivery refs remain evidence-only and do not satisfy
  maintainer merge, acceptance, settlement, payout, or public-claim authority.
- Added unsafe material rejection for raw patches, raw file contents, raw
  shell logs, raw commands, raw prompts, private repo data, local paths,
  provider payloads, credentials, wallet/payment material, and
  customer-private data.
- Added `docs/autopilot-coder/2026-06-12-pack-c-delivery-readiness.md` and
  updated the original terminal-agent-systems roadmap and app invariant ledger.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/pack-c-delivery-readiness.test.ts src/pack-c-workspace-authority.test.ts src/pack-c-change-capture.test.ts src/pack-c-repo-worktree-identity.test.ts`

## Pack C Parent / Issue #4831: Repo Scope, Delivery, And Evidence

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4831`

Status: implemented as the parent closeout for #4832-#4835.

Implemented:

- #4832 PC1 repository/worktree identity snapshots.
- #4833 PC2 change capture and diff-review artifacts.
- #4834 PC3 file/shell/workspace authority evidence.
- #4835 PC4 delivery readiness and PR draft receipt projections.
- Updated the original terminal-agent-systems roadmap with final Pack C
  implementation status and the then-current timing rule for #4768, #4772,
  #4777, #4781, #4782, #4783, #4786, #4836, and #4837.
- Recorded that no additional Pack C child issues are needed now and that Pack
  D should wait until the MVP proof gates and public freshness/order-book
  hygiene issues are closed or explicitly scoped. #4836/#4837 were later
  closed, leaving #4768/#4772 as the broad Pack D timing gate.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/provider-account-retention-policy.test.ts src/pack-c-delivery-readiness.test.ts src/pack-c-workspace-authority.test.ts src/pack-c-change-capture.test.ts src/pack-c-repo-worktree-identity.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## Issue #4836: Product-Promises Freshness And Announcement Gate

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4836`

Status: implemented for the public product-promises projection.

Implemented:

- Added `generatedAt`, `registryVersion`, `maxStalenessSeconds`, and the
  shared `live_at_read` projection staleness contract to the top-level
  `/api/public/product-promises` document.
- Added a regression-tested announcement-readiness helper that returns
  `blocked` when a proposed announcement version does not match the registry
  version actually served by the endpoint.
- Updated OpenAPI descriptions so agents know the endpoint exposes the
  deployed registry version and freshness contract.
- Updated the original terminal-agent-systems roadmap to mark the
  product-promises freshness half of the public hygiene blocker complete.
  #4837 later closed the Forum order-book half, so Pack D now waits on
  #4768/#4772 MVP proof closure or explicit proof-slice scoping.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/product-promises.test.ts src/openagents-openapi-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## Issue #4837: Forum Work-Request Closed Objective Hygiene

Issue: `https://github.com/OpenAgentsInc/openagents/issues/4837`

Status: implemented and closed by `9730f6728`; this follow-up pass verified
the state and synchronized the roadmap/docs.

Implemented:

- `/api/forum/work-requests` now returns `generatedAt`,
  `maxStalenessSeconds`, and the shared `live_at_read` projection staleness
  contract, with rebuild causes for work-request creation, lifecycle,
  archival, and quote recording.
- `buildBacklogWorkRequestFiling` rejects closed GitHub issues before they can
  become open-market Forum work requests.
- The stale live work request backed by closed issue #4773 was expired by
  lifecycle receipt, and the detail route reports it as `expired`.
- Updated the original terminal-agent-systems roadmap and Autopilot docs index
  so #4837 is no longer treated as the remaining blocker. Pack D still waits
  on #4768/#4772 MVP proof closure or explicit proof-slice scoping.

Verification:

- `bun run --cwd apps/openagents.com/workers/api test src/backlog-faucet.test.ts src/forum-routes.test.ts src/openagents-openapi-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`

## Terminal-Agent Systems Roadmap Follow-Up Review

Issue context:

- Reviewed #4749, #4768, #4772, #4777, #4781, #4782, #4783, and #4786 with
  `gh issue view --comments`.
- Rechecked #4755-#4786 and #4813-#4837 issue states via `gh issue view`.

Status: docs-only synchronization; no new GitHub issues opened.

Implemented:

- Updated the original terminal-agent-systems roadmap so the current issue
  state matches GitHub: Pack A/B/C and #4836/#4837 are closed, while #4768,
  #4772, #4777, #4781, #4782, #4783, #4786, and W3 #4749 remain open.
- Clarified that Pack D should still not be filed. The remaining blockers are
  live overnight proof, the MVP exit decision, independent provider/settlement
  evidence, and W3 training/eval completion rather than missing issue
  decomposition.
- Updated the open-issue blocker record so the expired #4773-backed market row
  is not treated as active inventory.
- Added a current-status guard to the delegation plan so agents do not claim
  closed Pack A/B/C or #4836/#4837 work from the historical assignment map.

Verification:

- `git diff --check`

## Pylon Supervised Daily-Driver Correction

Issue context:

- Owner clarified that an overnight unattended run is not required for the
  daily-driver MVP because the owner will sit at Pylon and supervise Codex.
- Rechecked the TUI composer path, Codex SDK executor, local Codex CLI flags,
  and current open issues.

Status: docs-only correction plus GitHub issue filing; no product promise,
runtime behavior, or invariant changed.

Implemented:

- Updated
  `docs/autopilot-coder/2026-06-12-pylon-codex-day-to-day-readiness-audit.md`
  to split supervised local daily-driver readiness from unattended/market
  readiness.
- Corrected the MVP bar: #4768 overnight proof is not required for
  owner-watched Codex work inside Pylon.
- Identified the ASAP switch path as local `pylon dev` / composer-to-Codex
  work against the active repo, with explicit local supervised dangerous mode.
- Filed top-priority issues:
  - #4839: P0 composer/current-repo Codex mode.
  - #4840: P0 local-only dangerous Codex execution.
  - #4841: P0 dev doctor repo/instruction/account context.
  - #4842: P0 dev check/apply/reload loop.
  - #4843: P1 work-order commit pinning and adapter intent.

Verification:

- `git diff --check`

## Pylon Codex Composer SDK Backend

Issue context:

- #4839: make the Pylon composer run Codex in the current repo.
- Owner decision: use the official TypeScript SDK rather than a raw
  `codex exec` parser for this path.

Status: source implementation for the supervised daily-driver composer path;
local dangerous mode remains #4840.

Implemented:

- Added `apps/pylon/src/codex-composer.ts`, which lazy-loads
  `@openai/codex-sdk`, probes Codex readiness before starting a thread, opens
  `runStreamed()` in the selected working directory, and reports structured
  event, usage, command, and file-change counts.
- Replaced the TUI composer hard-code to OpenCode with a backend seam; the
  default dashboard and attach mode now receive a Codex backend from
  `apps/pylon/src/index.ts`.
- Documented the source behavior in `apps/pylon/README.md`,
  `apps/pylon/docs/codex-bridge.md`, and this daily-driver audit.

Verification:

- `bun test tests/codex-composer.test.ts tests/tui-render-harness.test.ts tests/tui-commands.test.ts tests/codex-agent.test.ts`
- `git diff --check`

## Pylon Local Supervised Codex Danger Mode

Issue context:

- #4840: add local-only dangerous Codex mode for supervised Pylon dev.
- Owner decision: keep using the official TypeScript SDK; model the CLI
  dangerous bypass as SDK `sandboxMode: "danger-full-access"` plus
  `approvalPolicy: "never"`.

Status: source implementation for the local dashboard composer only;
assignment/provider/headless paths remain bounded.

Implemented:

- Added a separate local dev config reader for
  `dev.codexExecutionMode: "local_supervised_danger"` while keeping
  assignment-safe `codex.sandboxMode` limited to `read-only` /
  `workspace-write`.
- Added composer execution-mode enforcement so `danger-full-access` is
  accepted only with `executionMode: "local_supervised_danger"`.
- Wired `--codex-danger` and local dev config into the dashboard composer,
  labeling the backend `Codex DANGER` and showing the active mode/sandbox in
  the feed status.
- Rejected `--codex-danger` on `pylon work`, `pylon assignment`,
  `pylon provider`, `pylon node`, and `pylon attach` with a typed blocker.
- Updated README, Codex bridge docs, and the daily-driver audit.

Verification:

- `bun test tests/codex-composer.test.ts tests/codex-agent.test.ts tests/codex-agent-executor.test.ts`
- `bun test tests/codex-composer.test.ts tests/codex-agent.test.ts tests/codex-agent-executor.test.ts tests/tui-render-harness.test.ts tests/tui-commands.test.ts`
- `bun src/index.ts work --codex-danger` (expected rejection with `blocker.codex.local_supervised_danger_public_path`)
- `git diff --check`

## Pylon Dev Doctor Context Projection

Issue context:

- #4841: add `pylon dev doctor --json` for repo, instruction, account, and
  execution-mode context.

Status: source implementation for the CLI/projection source; pane rendering
and check/reload loop were follow-up work in this entry. The visible pane was
implemented later under #4838.

Implemented:

- Added `apps/pylon/src/dev-doctor.ts`, a typed redacted projection collector
  for active repo provider/name, branch, commit, dirty count, instruction and
  config digest refs, Codex SDK/CLI/auth readiness, Claude/Fable readiness,
  active Codex execution mode/sandbox, and backend refs.
- Added `pylon dev doctor --json [--codex-danger]` to the CLI.
- Kept the projection public-safe: no raw keys, auth file paths, instruction
  text, changed filenames, or local absolute paths.
- Updated README, Codex bridge docs, and the daily-driver audit.

Verification:

- `bun test tests/dev-doctor.test.ts`
- `bun src/index.ts dev doctor --json`
- `bun src/index.ts dev doctor --json --codex-danger`
- `git diff --check`

## Terminal-Agent Systems Roadmap Historical Status Refresh

Issue context:

- Re-reviewed the current open issue tail: #4749, #4768, #4772, #4777,
  #4781, #4782, #4783, and #4786.
- Searched the terminal-agent systems roadmap for stale "ready to file",
  "can close", and Pack A/B/C pending-action language.

Status: docs-only synchronization; no new GitHub issues opened.

Implemented:

- Updated the original terminal-agent-systems roadmap so Pack A, Pack B, Pack
  C, and #4836/#4837 read as closed historical work rather than pending action.
- Preserved the current sequencing rule: do not file Pack D until #4768/#4772
  close or explicitly narrow, and until #4777/#4781/#4782/#4783 have concrete
  live market evidence beyond the already-open issue bodies.

Verification:

- `git diff --check`
- `bunx prettier --check docs/autopilot-coder/implementation-log.md docs/autopilot-coder/terminal-agent-systems/2026-06-11-terminal-agent-systems-operationalization-roadmap.md`

## Pylon Codex Day-To-Day Readiness Audit

Issue context:

- Owner asked whether day-to-day coding can switch immediately to Pylon, with
  Codex as the main workhorse and Fable available occasionally.
- Re-read the current open issue tail (#4749, #4768, #4772, #4777, #4781,
  #4782, #4783, #4786), the terminal-agent systems operationalization roadmap,
  the main Autopilot Coder docs, Pylon Codex/Claude bridge docs, release
  records, and the current Pylon CLI/work-requester implementation.

Status: docs-only readiness audit; no product promise or runtime invariant
changed.

Implemented:

- Added
  `docs/autopilot-coder/2026-06-12-pylon-codex-day-to-day-readiness-audit.md`.
- Recorded the current verdict: Pylon v0.3 source is minimally usable for
  controlled owner dogfood with Codex, but not yet ready as the owner's
  supported full daily-driver coding replacement.
- Identified the then-immediate blockers: v0.3 package publication,
  placeholder commit pinning in `pylon work submit`, missing explicit
  Codex/Fable adapter preference, delivery/PR ergonomics, and still-open
  M10/M14 live proof gates. The work-submit commit/adapter blockers are later
  addressed by #4843.
- Updated the Autopilot Coder README index with the new audit.

Verification:

- `git diff --check`

## Pylon Dev Mode Readiness Audit Addendum

Issue context:

- Owner asked to extend the Pylon/Codex day-to-day readiness audit with a
  suggestion for a mode that helps improve Pylon from inside Pylon.

Status: docs-only addendum; no product promise, route, runtime behavior, or
invariant changed.

Implemented:

- Added a proposed Pylon Dev Mode section to
  `docs/autopilot-coder/2026-06-12-pylon-codex-day-to-day-readiness-audit.md`.
- Defined the intended command/TUI surface, local-only dev task contract,
  redacted diagnostic bundle, Codex-main/Fable-review workflow, check/reload
  loop, guardrails, and minimal milestones DM1-DM5.
- Updated the Autopilot Coder README entry for the audit.

Verification:

- `git diff --check`

## Pylon Dev Check Apply Reload Loop

Issue context:

- #4842 asked for a local supervised post-Codex loop so Pylon can summarize
  changes, run focused checks, and perform an explicit reload action without
  committing, pushing, cleaning, or switching branches.

Status: source implementation for the supervised daily-driver dev loop.

Implemented:

- Added `apps/pylon/src/dev-loop.ts` with typed projections for:
  - `openagents.pylon.dev_check.v0.3`
  - `openagents.pylon.dev_apply.v0.3`
  - `openagents.pylon.dev_reload.v0.3`
  - `openagents.pylon.dev_codex_run.v0.3`
- Added `pylon dev check --json [--allow-dirty] [--command <argv>]`,
  `pylon dev apply --json [--allow-dirty]`, and `pylon dev reload --json`.
- Recorded safe latest-run metadata after Codex SDK composer runs.
- Exposed Dev check/apply/reload actions in the TUI command palette.
- Kept projections redacted to refs/counts/digests: no raw stdout/stderr,
  changed filenames, local absolute paths, prompts, or credentials.
- Updated README, Codex bridge docs, and the daily-driver audit.

Verification:

- `bun test tests/dev-loop.test.ts tests/dev-doctor.test.ts tests/tui-commands.test.ts tests/codex-composer.test.ts tests/codex-agent.test.ts`
- `bun src/index.ts dev check --json --allow-dirty --command "bun --version"`
- `bun src/index.ts dev apply --json --allow-dirty`
- `bun src/index.ts dev reload --json`
- `bun src/index.ts dev check --json --command "bun --version"` (expected
  dirty-prestate block and exit 1)

## Pylon Work Submit Commit Pinning And Adapter Intent

Issue context:

- #4843 asked for `pylon work submit` to stop emitting placeholder commits and
  to carry Codex/Fable/Claude requester intent through the Autopilot
  work-order lane.
- Owner decision: keep the local daily-driver Codex path SDK-first. This issue
  hardens the network work-order lane; it does not replace the local
  supervised `pylon dev` / composer path.

Status: source implementation for the work-order CLI/API path.

Implemented:

- Added required `--commit <40-char-sha>` handling to `pylon work submit`.
  The CLI rejects missing, non-SHA, all-zero, and all-one placeholder commits.
- Added a public GitHub commit preflight before posting
  `/api/autopilot/work`, so unresolvable commits fail before submission.
- Added `--adapter codex|claude_agent|fable`; Fable maps to the Claude Agent
  lane with `profile.claude_agent.fable`.
- Carried requester adapter/profile intent through work request validation,
  task records, Pylon assignment synthesis, and normalized coding assignment
  selection.
- Preserved the platform dual-capability default of Claude for intent-less
  orders; owner Codex-primary work should use `--adapter codex` or local Dev
  Mode.
- Rejected placeholder commit pins at the server work-request boundary and
  downstream normalized coding-assignment boundary.
- Updated Pylon README, Codex bridge docs, the daily-driver audit, and the docs
  index.

Verification:

- `bun test tests/work-requester.test.ts`
- `bun test apps/openagents.com/workers/api/src/autopilot-work-request.test.ts apps/openagents.com/workers/api/src/autopilot-work-pylon-assignment-synthesizer.test.ts apps/openagents.com/workers/api/src/autopilot-work-adapter-selection.test.ts apps/openagents.com/workers/api/src/autopilot-coding-assignment.test.ts`
- `bun test apps/openagents.com/workers/api/src/autopilot-work-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`
- `git diff --check`

Known unrelated check result:

- `bun run --cwd apps/pylon test` currently fails in
  `tests/assignment.test.ts` with `stale NIP-98 event` in the local assignment
  harness. The focused work-requester tests pass.

## Pylon Repo And AI Context Pane

Issue context:

- #4838 asked for the Pylon TUI to show active repository, instruction-layer,
  AI account/adapter readiness, selected adapter, current-job refs, and
  blockers without leaving the dashboard.

Status: source implementation for the supervised daily-driver context surface.

Implemented:

- Added `apps/pylon/src/context-projection.ts` with
  `openagents.pylon.context.v0.3`, built from the existing dev-doctor probes.
- Added `pylon context --json` as the public-safe status command used by the
  TUI and scripts.
- Added TUI context state plus a wide-dashboard `Repo & AI Context` pane beside
  `Telemetry & Wallet` on terminals wide enough to keep the logs readable.
- Added an `f6` context route and `Context: refresh repo & AI` command for
  narrow terminals.
- Rendered Codex DANGER/local execution posture, OpenAI/Codex source refs,
  Claude/Fable readiness, backend refs, current-job/workspace/verification
  refs, required capability refs, and blockers from typed data.
- Kept output ref-only and redacted: no raw secrets, account emails, auth
  paths, raw instruction text, changed filenames, or local absolute paths.
- Updated the Pylon README and daily-driver audit.

Verification:

- `bun test tests/context-projection.test.ts tests/tui-store.test.ts tests/tui-commands.test.ts tests/tui-render-harness.test.ts tests/dev-doctor.test.ts`
- `bun src/index.ts context --json`
- `git diff --check`

## Pylon Claude Composer Backend

Issue context:

- #4844 asked for the Pylon dashboard composer to run Claude/Fable in the
  current repo behind the same adapter-neutral composer seam as Codex.
- Owner decision: use the installed TypeScript Claude Agent SDK, not a raw CLI
  parser.

Status: source implementation for local supervised dashboard composer
selection. This does not implement Claude `bypassPermissions`; that remains
the #4845 permissive-mode issue.

Implemented:

- Added `apps/pylon/src/claude-composer.ts`, a local composer backend that
  streams `@anthropic-ai/claude-agent-sdk` `query()` messages, assistant text,
  tool-use summaries, usage totals, and result events.
- Added `dev.defaultAdapter: "codex" | "claude_agent"` parsing plus
  `--adapter codex|claude|claude_agent` launch override for the local TUI
  composer.
- Wired the selected composer backend through the existing TUI seam while
  keeping Codex as the default.
- Preflighted `probeClaudeAgentReadiness()` before launching SDK sessions.
- Ran Claude in the active repo cwd and kept raw SDK session ids local for
  resume; TUI/footer output uses hashed session refs only.
- Labeled Claude sessions as `Claude` / `Claude (<model>)`, so a configured
  Fable model is visible without creating a separate Fable adapter.
- Updated the context projection to treat configured Claude-primary mode as
  the selected primary adapter when readiness is green.
- Updated Pylon README, Claude bridge docs, the daily-driver audit, and tests.

Verification:

- `bun test tests/claude-composer.test.ts tests/codex-composer.test.ts tests/codex-agent.test.ts tests/context-projection.test.ts tests/dev-doctor.test.ts tests/tui-render-harness.test.ts tests/tui-commands.test.ts`
- `bun src/index.ts context --json`
- `git diff --check`

## #4845 CL2: local-only supervised permissive Claude mode (2026-06-12)

Status: source implementation merged to `main`. The retained supervised
Claude/Fable daily-driver proof remains #4847.

Implemented:

- `ClaudeComposerExecutionMode` (`local_bounded` | `local_supervised_danger`)
  and `permissionModeForClaudeComposerExecutionMode()` in
  `apps/pylon/src/claude-composer.ts`: the danger mode maps to SDK
  `permissionMode: "bypassPermissions"` with no tool allowlist, the
  permission-system equivalent of Codex `danger-full-access`.
- Explicit opt-in only: `pylon --claude-danger` or
  `"dev": { "claudeExecutionMode": "local_supervised_danger" }` read by the
  new `loadClaudeDevConfig()`; requesting `bypassPermissions` without the
  execution mode throws
  `blocker.claude.local_supervised_danger_requires_opt_in`.
- settingSources decision recorded: bounded mode keeps `settingSources: []`
  executor-style isolation; danger mode loads `settingSources: ["project"]`
  so the owner's own `CLAUDE.md`/`.claude` layers are active exactly when
  they have opted into watching an unrestricted session.
- `Claude DANGER` TUI label plus `mode | permissions` status line and footer.
- `pylon work`, `pylon assignment`, `pylon provider`, `pylon node`, and
  `pylon attach` reject `--claude-danger` with
  `blocker.claude.local_supervised_danger_public_path`; danger flags are
  per-lane, so `--codex-danger` can never select a permissive Claude session.
- `loadClaudeAgentConfig()` documented and tested as the assignment-safe
  surface that never reads a permissive mode.
- Root `INVARIANTS.md` records the per-lane local danger boundary.

Verification:

- `bun test tests/claude-composer.test.ts tests/claude-agent.test.ts tests/codex-composer.test.ts tests/dev-doctor.test.ts tests/context-projection.test.ts`
- `bun run smoke:default-start`

## #4846 CL3: dev doctor and context pane show Claude execution mode (2026-06-12)

Status: source implementation merged to `main`.

Implemented:

- Dev-doctor `claudeAgent` section gains `executionMode`, `permissionMode`,
  and `dangerPublicPathBlockerRef` (set while the permissive mode is active,
  naming the typed public-path blocker); `pylonConfig` gains
  `claudeDevOverlayRef` (`config.pylon.dev.claude_local_supervised_danger`).
- `pylon dev doctor --json` accepts `--claude-danger` alongside
  `--codex-danger`.
- The `Repo & AI Context` pane renders `Claude DANGER` with the same
  prominence as `Codex DANGER`, adds Claude `Mode:`/`Permissions:` lines,
  colors the pane text as an error when either lane is permissive, and
  treats the Claude overlay/mode as dev mode.
- Redaction law unchanged and reasserted by fixture tests covering bounded,
  danger-from-config, and danger-from-flag states.

Verification:

- `bun test tests/dev-doctor.test.ts tests/context-projection.test.ts tests/tui-commands.test.ts tests/tui-render-harness.test.ts tests/tui-store.test.ts tests/claude-composer.test.ts`
- `bun run smoke:default-start`
