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
