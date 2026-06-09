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
