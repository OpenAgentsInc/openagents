# OpenAgents Public Launch Dashboard

Date: 2026-06-08
Issue: #571

## Scope

`GET /api/public/launch-dashboard` is the machine-checkable red/yellow/green
truth surface for the launch promises inventoried from `source-conversation.md` in
`docs/2026-06-08-pylon-agentic-revenue-gap-audit.md`.

The dashboard contains one row for each numbered source-transcript promise.

## Row Contract

Each row includes:

- `promiseId`
- `promiseText`
- `status`: `red`, `yellow`, or `green`
- `evidenceRefs`
- `blockerRefs`
- `safeCopy`
- `unsafeCopy`

Green means the row has public endpoint evidence or receipt refs and is safe for
public launch copy. Yellow means partial, planned, or manually gated. Red blocks
public launch copy.

## Guards

- Every source promise must be represented exactly once.
- Red and yellow rows require blocker refs.
- Every row requires evidence refs.
- Stale endpoint data forces stale-sensitive rows to red or yellow.
- Public projections must not expose bearer tokens, wallet material, raw
  invoices, preimages, payment hashes, provider secrets, private customer data,
  or payout targets.

## Verification

Focused gate:

```bash
bun run --cwd workers/api test -- \
  src/public-launch-dashboard.test.ts \
  src/openagents-capability-manifest-routes.test.ts \
  src/openagents-openapi-routes.test.ts \
  src/openagents-agent-onboarding-routes.test.ts
```

The dashboard route is also included in the public capability manifest, OpenAPI,
and canonical `AGENTS.md` sheet.
