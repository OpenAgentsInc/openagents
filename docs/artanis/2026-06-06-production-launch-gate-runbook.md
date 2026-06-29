# Artanis Production Launch Gate And Runbook

Issue #414 / `ARTANIS-028` adds the final production launch gate before
Artanis can be described as continuously running autonomously.

The implementation lives in:

- `workers/api/src/artanis-production-launch-gate.ts`
- `workers/api/src/artanis-production-launch-gate.test.ts`
- `workers/api/src/artanis-public-report.ts`
- `apps/web/src/page/loggedOut/page/publicAgent.ts`

`GET /api/public/artanis/report` now includes `productionLaunchGate`, and
`/artanis` renders a compact Production gate card. As of OpenAgents product surface #511 and #512,
the example public projection has retained production-equivalent Probe
GEPA/Pylon smoke evidence and a separate bounded scheduled-runner proof.

## Current Gate Result

Do not say Artanis is an unbounded autonomous production administrator.

Do say:

- Artanis has a public evidence surface and operator-gated launch path.
- Artanis has bounded continuous status operation for GEPA/Pylon reporting
  evidence.
- Pylon v0.2 launch communication is prepared, while general release claims
  remain gated.

The gate blocks these public claims until every required production gate has
passed:

- "Artanis is continuously running autonomously."
- "Artanis is a fully autonomous production administrator."
- "Pylon v0.2 is shipped."

## Required Prerequisites

| Area | Status | Evidence |
| --- | --- | --- |
| Persistence | Passed | #403, `workers/api/src/artanis-persistence.test.ts` |
| Scheduled runner | Passed for bounded status projection | #404 implemented the runner; #512 proves bounded no-spend/no-dispatch scheduled status operation. |
| Operator console | Passed | #405, `/autopilot`, `GET /api/operator/artanis/console` |
| Approval gates | Passed | #393/#405, approve/reject endpoints |
| Forum delivery | Passed | #406 delivery bridge and #418 delivery verification refs |
| Forum listener | Passed | #407 read-only listener/triage and #418 listener verification refs |
| Nexus/Pylon adapter | Passed for monitoring/fake dispatch | #408 and `GET /api/public/pylon-stats`; live dispatch remains separately gated. |
| Marketplace intake | Passed | #410 operator intake and triage APIs |
| Continual-learning templates | Passed | #411 benchmark/GEPA/DSPy/LoRA template ledger |
| Payment/reward boundary | Passed | #412 simulation-only reward smoke and no wallet spend authority |
| Public report projection | Passed | #392/#413/#414, `/api/public/artanis/report` and `/artanis` |
| Production E2E smoke | Passed for retained GEPA/Pylon smoke | #511 retains the Probe GEPA/Pylon production-equivalent smoke. |
| Rollback runbook | Passed | This document and the typed gate contract |

## Production Verification Checklist

Start with the read-only verifier from #416:

```bash
bun run artanis:readiness -- \
  --scheduled-runner disabled \
  --latest-pylon-release-tag "pylon-v0.1.23"
```

Pass a read-only D1 table list through `--d1-tables` after a controlled
migration/deploy window. Pass `--production-smoke-ref` only after the
production-equivalent smoke evidence is retained. This verifier does not
deploy, apply migrations, post to Forum, or enable the scheduler.

The retained production-equivalent smoke evidence contract from #417 lives in
`workers/api/src/artanis-retained-launch-smoke.ts` and is documented in
`docs/artanis/2026-06-06-retained-production-launch-smoke.md`. A retained smoke
can feed the `production_e2e_smoke` launch-gate check. The newer Probe
GEPA/Pylon smoke contract from #511 lives in
`workers/api/src/artanis-gepa-production-smoke.ts` and is documented in
`docs/artanis/2026-06-08-probe-gepa-pylon-production-equivalent-smoke.md`.

The bounded scheduled-runner proof from #512 lives in
`workers/api/src/artanis-gepa-scheduled-runner-proof.ts` and is documented in
`docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md`. That proof permits
bounded public-safe status operation only; it does not grant assignment
dispatch, automatic Forum publishing, provider mutation, model training,
runtime promotion, settlement mutation, or wallet spend.

The Forum delivery/listener verification contract from #418 lives in
`workers/api/src/artanis-forum-verification.ts` and is documented in
`docs/artanis/2026-06-06-forum-delivery-listener-verification.md`. Retain a
verification record after a controlled delivery/listener pass so the launch
gate can distinguish real Forum evidence from merely having the delivery and
listener code in the repo.

Before enabling the scheduler, an operator must verify:

- `/artanis` renders the public report, production gate, Forum refs, Pylon
  stats, and no autonomy overclaim.
- `GET /api/public/artanis/report` returns `productionLaunchGate`,
  `healthSummary`, `autonomousLoop`, and public-safe blocker refs.
- `/forum/t/88888888-4001-4001-8001-888888888888` is readable as the Artanis
  status topic.
- a #418 verification record exists for the canonical status topic and Pylon
  release work-log topic, including delivered post or blocker evidence plus a
  listener no-op/read, reply-draft, operator-question, work-routing, or blocker
  result.
- `/autopilot` lets an admin inspect Artanis goals and pause them.
- `GET /api/operator/artanis/console` returns runtime, loop, health, approval
  gates, work routing, Forum queue, and staleness summaries for admins.
- Artanis approval gate approve/reject endpoints require admin auth and
  idempotency.
- `GET /api/public/pylon-stats` loads and distinguishes unavailable stats from
  zero stats.
- Health and staleness projections expose blockers before public copy can
  overclaim.

## Check Commands

Public report and gate:

```bash
curl -fsS https://openagents.com/api/public/artanis/report \
  | jq '{runtimeState, autonomousLoop, healthSummary, pylonSummary, productionLaunchGate}'
```

OpenAgents product surface public Pylon stats:

```bash
curl -fsS https://openagents.com/api/public/pylon-stats \
  | jq '{status, minimumClientVersion, pylonsOnlineNow, pylonsRegisteredTotal, pylonsWalletReadyNow, pylonsAssignmentReadyNow, asOfLabel, caveatRefs, sourceRefs}'
```

Artanis status topic:

```bash
curl -fsS https://openagents.com/api/forum/topics/88888888-4001-4001-8001-888888888888 \
  | jq '{topic: {id: .topic.topicId, title: .topic.title}, postCount: (.posts | length)}'
```

Operator console, using a local environment variable rather than a literal
token:

```bash
curl -fsS https://openagents.com/api/operator/artanis/console \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  | jq '{runtime, loop, health, approvalGates}'
```

Recent retained Artanis records:

```bash
bunx wrangler d1 execute openagents-autopilot --remote \
  --command "SELECT record_ref, record_type, updated_at FROM artanis_records ORDER BY updated_at DESC LIMIT 20;"
```

## Enable And Disable

The Worker cron trigger exists in `workers/api/wrangler.jsonc`, but Artanis
execution remains disabled unless the deployed Worker environment resolves:

```text
ARTANIS_SCHEDULED_RUNNER_ENABLED=true
```

Enable only during an operator-controlled window after the gate is ready. This
uses Wrangler's deployment `--var` path and `--keep-vars` so the flag becomes a
runtime Worker variable without deleting dashboard-managed variables:

```bash
bun run --cwd workers/api build:web
bunx wrangler deploy --config workers/api/wrangler.jsonc \
  --keep-vars \
  --var ARTANIS_SCHEDULED_RUNNER_ENABLED:true
```

Disable immediately when the launch window ends, when a blocker appears, or
when rollback begins:

```bash
bun run --cwd workers/api build:web
bunx wrangler deploy --config workers/api/wrangler.jsonc \
  --keep-vars \
  --var ARTANIS_SCHEDULED_RUNNER_ENABLED:false
```

If Cloudflare environment variables are managed only through the dashboard,
confirm the same `ARTANIS_SCHEDULED_RUNNER_ENABLED` value there before deploy.
Do not store secrets or wallet material in `wrangler.jsonc`.

## Pause, Revoke, Recover

Pause a specific Artanis goal:

```bash
curl -fsS -X POST https://openagents.com/api/operator/autopilot/goals/GOAL_ID/pause \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "Idempotency-Key: artanis-pause-GOAL_ID"
```

Reject or revoke a pending approval gate:

```bash
curl -fsS -X POST https://openagents.com/api/operator/artanis/approval-gates/GATE_REF/reject \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "Idempotency-Key: artanis-reject-GATE_REF"
```

Recover from duplicate or failed ticks:

1. Disable the scheduler.
2. Inspect recent retained rows with the D1 command above.
3. Check `/api/operator/artanis/console` for the latest tick, blockers, and
   pending approval gates.
4. Pause the owning goal if the same blocker recurs.
5. Re-run the focused tests locally before re-enabling.

```bash
bun run --cwd workers/api test -- \
  src/artanis-production-launch-gate.test.ts \
  src/artanis-scheduled-runner.test.ts \
  src/artanis-launch-smoke.test.ts \
  src/artanis-public-report.test.ts
```

## Rollback Steps

Publication mistake:

- Disable the scheduler.
- Reject the relevant publication approval gate.
- Post a correction in the Artanis status topic.
- Keep the mistaken post only as historical evidence; do not hide the mistake
  by rewriting the public proof trail.

Dispatch mistake:

- Disable the scheduler.
- Pause the owning goal.
- Reject the dispatch approval gate.
- Leave live dispatch authority blocked until a new operator-reviewed proposal
  is recorded.

Payment or reward mistake:

- Disable the scheduler.
- Verify public reward receipts.
- Remove or correct copy that confuses content rewards, accepted-work payouts,
  settlement, or wallet spend.
- Publish a correction distinguishing simulations from settled payments.

Public claim mistake:

- Revert copy that says Artanis is continuously autonomous before the gate is
  ready.
- Keep `productionLaunchGate.state` blocked.
- Cite this runbook and the gate projection as the authority boundary.

## Tests

Focused checks:

```bash
bun run --cwd workers/api test -- \
  src/artanis-production-launch-gate.test.ts \
  src/artanis-public-report.test.ts

bun run --cwd apps/web test -- src/docs-blog-route.test.ts
```

Broader pre-launch checks should include:

```bash
bun run --cwd workers/api test -- \
  src/artanis-production-launch-gate.test.ts \
  src/artanis-scheduled-runner.test.ts \
  src/artanis-launch-smoke.test.ts \
  src/artanis-public-report.test.ts \
  src/artanis-operator-console-routes.test.ts

bun run --cwd apps/web test -- src/docs-blog-route.test.ts src/main.test.ts
bun run typecheck
bun run check:architecture
```
