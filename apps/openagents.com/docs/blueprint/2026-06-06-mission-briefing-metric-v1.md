# Blueprint Mission Briefing Metric v1

Date: 2026-06-06

Status: implemented for issue #276.

## Purpose

The Mission Briefing metric records whether a returning customer, operator,
team member, or agent can understand current workroom state in under two
minutes.

This is the feedback loop for improving continuation briefings without
projecting private notes, raw logs, raw email material, credentials, or
provider-account internals into customer/public surfaces.

## Record Shape

`BlueprintMissionBriefingMetricRecord` ties feedback to:

- briefing ref;
- workroom ref;
- optional Program Run ref;
- receipt refs;
- scorecard refs;
- reviewer kind;
- elapsed-time bucket;
- comprehension result;
- missing-context refs;
- follow-up action;
- public-safe feedback summary ref;
- optional private feedback-note ref.

Elapsed-time buckets are:

- `under_30s`;
- `under_1m`;
- `under_2m`;
- `over_2m`;
- `not_understood`.

## Projections

`projectBlueprintMissionBriefingMetric` creates public/customer/team/operator
views. Public, customer, and team projections hide private feedback-note refs,
provider-account details, source-authority internals, raw emails, credentials,
and raw timestamps. Operator projections may retain redacted operator-safe note
refs but still reject raw secrets and raw logs.

## Aggregates

`aggregateBlueprintMissionBriefingMetrics` returns safe counts:

- total records;
- under-two-minute count and percent;
- understood, partial, and not-understood counts;
- understood percent;
- counts by reviewer kind;
- counts by elapsed bucket;
- counts by follow-up action;
- improvement-needed flag.

The aggregate is intentionally count-only so it can appear in roadmap,
operator, or future product dashboards without leaking reviewer notes.

## Boundaries

The metric loop does not:

- persist records by itself;
- mutate workrooms;
- approve or reject work;
- trigger retries;
- send emails;
- deploy Sites;
- affect payment or payout eligibility.

Persistence, dashboards, and release-gate consumption can build on this
evidence-only model in later issues.
