# Coding On Autopilot Situational Awareness

Date: 2026-06-06

Status: implemented contract note for GitHub issue #319 / `OPENAGENTS-072`.

## Purpose

Coding on Autopilot needs the same under-two-minute state-understanding loop
that continuation briefings use, but with Coding-specific refs attached:
missions, artifacts, decision actions, account failover, and repo trust.

The implementation lives in
`workers/api/src/coding-autopilot-situational-awareness.ts`.

## Relationship To Mission Briefing Metrics

This does not create a second comprehension metric. It reuses the existing
Blueprint Mission Briefing metric model from issue #276 and projects it through
the Coding on Autopilot lens.

Each situational-awareness record wraps a
`BlueprintMissionBriefingMetricRecord` and adds safe refs for:

- Coding mission records;
- Coding artifacts;
- Decision Queue actions;
- account failover state;
- repo trust state.

The projected record exposes the briefing metric's comprehension result,
missing-context refs, follow-up action, reviewer kind, elapsed-time bucket, and
under-two-minute target result.

## Projection Rules

Public, customer, team, and operator projections use the same redaction stance
as the underlying briefing metric:

- no raw timestamps in user-facing projections;
- no raw runner logs;
- no provider tokens, provider accounts, cookies, or OAuth material;
- no private repo URLs;
- no customer emails or raw email bodies;
- no payment preimages, invoices, wallet material, or secrets.

The public projection filters private artifact, decision-action, repo-trust,
route, source-authority, and workroom refs. Customer and team projections can
see safe operational refs where the surrounding product surface authorizes
them, while provider-account and source-authority internals remain out of those
views. Operator projections still reject raw secrets and raw logs.

## Aggregates

`aggregateCodingAutopilotSituationalAwarenessRecords` delegates the count model
to `aggregateBlueprintMissionBriefingMetrics`, then carries forward safe unique
Coding refs for mission, artifact, decision-action, account-failover, and
repo-trust visibility.

This keeps dashboards and public proof surfaces count-first while preserving
the refs needed to audit why a reviewer did or did not understand a mission
state quickly.

## Tests

`workers/api/src/coding-autopilot-situational-awareness.test.ts` covers:

- Coding refs projected with the base briefing metric;
- understood/not-understood and under-two-minute state;
- missing-context and follow-up action projection;
- reviewer-kind projection;
- public/customer/operator redaction splits;
- safe aggregate refs and counts;
- rejection of provider-account, raw-runner, and raw-timestamp refs.
