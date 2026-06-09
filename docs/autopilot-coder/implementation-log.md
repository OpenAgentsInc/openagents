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
  - `gcloud_credit`
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
