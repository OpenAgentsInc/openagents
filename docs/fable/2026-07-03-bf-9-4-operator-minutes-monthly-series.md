# BF-9.4 Operator-Minutes Monthly Series

Date: 2026-07-03
Status: BF-9.4 instrumentation landed; no promise state flip; no public copy
change
Source issue: OpenAgentsInc/openagents#8118
Roadmap refs: [`ROADMAP_BIZ.md`](./ROADMAP_BIZ.md) BF-9.4 and
[`ROADMAP_AFTER.md`](./ROADMAP_AFTER.md) agency-trap kill criterion

BF-9.4 makes the services-engine falsifier measurable:

```text
operator minutes per accepted engagement must fall as accepted engagement
count rises.
```

The first auditable implementation is the review-ledger floor:

`business_engagement.operator_minutes_per_engagement.monthly_review_ledger_floor.v1`

It is emitted by the BF-7.2 query pack in
`apps/openagents.com/workers/api/src/business-factory-metrics.ts` and mirrored
in `business-factory-metrics.sql`.

## Contract

Each monthly row carries:

- `grain = "window"`;
- `window_start` and `window_end` as calendar-month bounds;
- `numerator` as summed ledgered review minutes from
  `omni_accepted_outcome_economics`;
- `denominator` as distinct opaque accepted engagement refs;
- `value` as minutes per accepted engagement;
- `unit = "minutes"`;
- `measurement_state = "measured"` only when the denominator is non-zero.

Empty months return `measurement_state = "not_measured"` and
`caveat.business_metrics.no_accepted_engagements_in_month`. They never report a
fake zero. Measured rows still carry
`caveat.business_metrics.operator_minutes_review_only_until_labor_ledger`
because this is a lower bound until a first-class operator-labor event ledger
exists.

## Review Rule

The monthly review asks two questions:

1. Did accepted engagement count rise?
2. Did operator minutes per accepted engagement fall?

If the answer to the first is yes and the second is no, the review must create
a blocker before expanding service volume. That blocker belongs in the BF-9.2
weekly pipeline review and the next monthly review packet. It must not be
papered over with owner memory, manual estimates, or client-identifying notes.

## Privacy Boundary

Rows may expose only opaque engagement refs through aggregate counts and the
public-safe metric ref. They must not include names, emails, raw client prompts,
private documents, local paths, provider payloads, payment material, or wallet
material.
