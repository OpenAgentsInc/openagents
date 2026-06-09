# Artanis Health And Staleness Monitor

Date: 2026-06-06

Issue: #394 / `ARTANIS-009`

Status: implemented as a schema/projection contract in
`workers/api/src/artanis-health.ts`, and surfaced publicly through
`GET /api/public/artanis/report`.

## Purpose

Artanis needs a health layer that can say whether the public proof surface is
fresh enough to trust. Stale Model Lab reports, lagged Forum publication,
pending approvals, missing loop ticks, stale OpenAgents product surface public Pylon stats, or unavailable
runner backends should become visible blockers and recovery actions. They must
not become hidden failures or overbroad public claims.

## Signal Set

The v1 health snapshot covers:

- loop freshness
- last tick
- blocker reason
- pending approvals
- Forum publication lag
- Pylon stats freshness
- OpenAgents product surface public Pylon stats freshness
- Model Lab report freshness
- runner/backend availability

Every snapshot must include all of these signal kinds. Missing signal coverage
is itself invalid because it would let `/artanis` overclaim by omission.

## States

Signals can be:

- `fresh`
- `available`
- `degraded`
- `stale`
- `missing`
- `blocked`
- `unavailable`
- `unknown`

Snapshot-level state can be:

- `healthy`
- `degraded`
- `stale`
- `blocked`
- `unavailable`

Fresh and available signals cannot carry blockers. Stale, blocked, missing,
unavailable, degraded, or unknown signals require recovery action refs or
blocker refs. A snapshot with any attention signal must set
`overclaimBlocked` and include overclaim blocker refs.

## Public Boundary

Public `/artanis` and Forum projections receive:

- overall state
- stale/blocked signal counts
- pending approval count
- public blocker refs
- public status refs
- public recovery action refs
- public source refs
- friendly display times

They do not receive:

- operator detail refs
- operator recovery refs
- pending approval refs
- runner backend refs
- private evidence refs
- provider, runner, wallet, payment, customer, private repo, raw prompt, raw
  log, raw timestamp, or secret material

The operator projection can expose operator detail refs and recovery refs by
reference, but it remains a health projection. It does not execute recovery,
approve risky actions, dispatch jobs, publish Forum posts, or settle payments.

## `/artanis` Report Integration

`workers/api/src/artanis-public-report.ts` now includes a compact
`healthSummary` in the public Artanis report. The `/artanis` page renders that
summary as a Health metric with the current state and attention count.

The public report also folds health blocker refs and caveats into the existing
public blocker/caveat lists so stale health blocks public overclaiming.

The heavier `/autopilot` operator console is intentionally left for #405 /
`ARTANIS-019`; this issue provides the typed operator projection and public
report integration needed by that UI.

## Tests

Coverage lives in:

- `workers/api/src/artanis-health.test.ts`
- `workers/api/src/artanis-public-report.test.ts`
- `apps/web/src/docs-blog-route.test.ts`

The tests prove:

- all required health signals are present;
- stale/blocked health blocks overclaiming and creates recovery action refs;
- healthy snapshots cannot carry overclaim blockers;
- public projections redact operator and private refs;
- unsafe provider, runner, wallet, payment, customer, private repo, raw prompt,
  raw log, raw timestamp, and secret material is rejected;
- `/artanis` renders the public Health metric.
