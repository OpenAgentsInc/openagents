# Autopilot Admin Test-User Trigger (Full Trace Lifecycle)

This runbook documents the headless admin trigger for Autopilot chat runs using a **fixed test user**.

## Purpose

Use this when an agent/operator needs to:

- trigger a real `/api/autopilot/send`-equivalent run without browser interaction
- avoid impersonating arbitrary users
- fetch a complete thread trace bundle for analysis

## Security Model

- Admin endpoints require `Authorization: Bearer <OA_AUTOPILOT_ADMIN_SECRET>`.
- Worker then mints an internal E2E JWT and authenticates as a fixed subject:
  - `user_autopilot_admin_test`
- This keeps access deterministic and avoids user impersonation.
- Fallback behavior: if `OA_AUTOPILOT_ADMIN_SECRET` is unset, Worker falls back to `OA_DSE_ADMIN_SECRET`.

## Required Secrets

Set these on the Worker:

- `OA_AUTOPILOT_ADMIN_SECRET` (recommended; dedicated admin trigger secret)
- `OA_E2E_JWT_PRIVATE_JWK` (required to mint fixed test-user JWT)

If needed, sync from `.env.production`:

```bash
cd apps/web
npm run wrangler:secrets
```

## Admin Endpoints

All endpoints are under `/api/autopilot/admin/*`.

### 1) Send

`POST /api/autopilot/admin/send`

Body:

- `text` (required)
- `threadId` (optional; defaults to fixed test-user owned thread)
- `resetThread` (optional boolean)

Response fields include:

- `testUserId`
- `threadId`
- `runId`
- `userMessageId`
- `assistantMessageId`

### 2) Reset

`POST /api/autopilot/admin/reset`

Body:

- `threadId` (optional; defaults to fixed test-user owned thread)

### 3) Snapshot

`GET /api/autopilot/admin/snapshot?threadId=...`

Useful for polling run completion and quick stream inspection.

### 4) Trace

`GET /api/autopilot/admin/trace?threadId=...`

Returns `getThreadTraceBundle` output (`messages`, `parts`, `runs`, `receipts`, `featureRequests`, optional DSE rows).

## End-to-End Runner Script

Script: `apps/web/scripts/autopilot-admin-trace.ts`

NPM command:

```bash
cd apps/web
OA_AUTOPILOT_ADMIN_SECRET="<secret>" npm run trace:admin -- \
  --base-url https://openagents.com \
  --text "Admin trace check: summarize your current capabilities in one sentence."
```

Behavior:

1. Calls `/api/autopilot/admin/send`
2. Polls `/api/autopilot/admin/snapshot` until run terminal status
3. Fetches `/api/autopilot/admin/trace`
4. Writes JSON artifact to:
   - `apps/web/output/autopilot-admin-trace/<timestamp>-<runId>.json`

The artifact includes:

- run ids and status
- worker request ids (`send`, polling `snapshot`s, and `trace`) for log correlation
- part type counts
- DSE signature ids seen (if any)
- finish/model metadata presence
- full trace bundle payload

## Notes

- This path exercises the same runtime stream pipeline used by normal chat sends.
- Convex direct mutations alone are not equivalent; they do not execute Worker inference/DSE stages.
