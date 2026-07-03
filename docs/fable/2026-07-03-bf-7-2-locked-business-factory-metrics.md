# BF-7.2 Locked Business Factory Metrics

Date: 2026-07-03
Status: definitions and query contract for
[`ROADMAP_BIZ.md`](./ROADMAP_BIZ.md) BF-7.2 / issue #8106. This document
does not flip promise state, broaden product copy, or publish a dashboard.

The rule is simple: a metric can appear on a dashboard only when the backing
query returns an auditable row with `measurement_state = 'measured'`. If the
ledger needed for a metric does not exist, the query must return
`measurement_state = 'not_measured'` with a caveat ref instead of emitting a
zero, estimate, or manually-entered value.

## Privacy Boundary

Metric rows may contain only:

- opaque refs such as `business_engagement.*`, `contract.*`, `workroom.*`, or
  public receipt refs;
- coarse work kind, source kind, stage, and time-window labels;
- aggregate counts, minutes, basis points, and rates.

Metric rows must not contain names, emails, phone numbers, raw client prompts,
raw provider payloads, wallet material, customer documents, local paths, or
client-identifying free text. If a source table carries sensitive data, the
metric query must use its opaque ref columns only.

## Metric Contract

Every query row uses the same shape:

| Field | Meaning |
| --- | --- |
| `metric_ref` | Stable metric id, e.g. `business_factory.throughput.accepted_outcomes.v1`. |
| `grain` | `window`, `work_kind`, or `engagement`. |
| `work_kind` | Work class when grouped; otherwise `NULL`. |
| `engagement_ref` | Opaque engagement ref when grouped; otherwise `NULL`. |
| `window_start`, `window_end` | Inclusive lower bound and exclusive upper bound used by the query. |
| `numerator`, `denominator`, `value` | Raw count/minute/rate components. Rates are basis points unless the unit says otherwise. |
| `unit` | `outcomes`, `minutes`, or `basis_points`. |
| `measurement_state` | `measured` or `not_measured`. No unaudited metric may report as measured. |
| `evidence_refs_json` | JSON array of source-table/query refs. |
| `caveat_refs_json` | JSON array of public-safe caveats. Empty for fully audited rows. |

## Locked Definitions

### Throughput

`business_factory.throughput.accepted_outcomes.v1`

Accepted outcome count in the window, grouped by `work_kind`.

Formula:

```text
count(omni_accepted_outcome_contracts)
where acceptance_state = 'accepted'
  and archived_at is null
  and updated_at in [window_start, window_end)
```

Unit: `outcomes`. A zero count is measured because the contract ledger is the
source of truth.

### Cycle Time

`business_factory.cycle_time.accepted_minutes.v1`

Average minutes from accepted-outcome contract creation to accepted closeout,
grouped by `work_kind`.

Formula:

```text
avg(updated_at - created_at in minutes)
where acceptance_state = 'accepted'
  and archived_at is null
  and updated_at in [window_start, window_end)
```

Unit: `minutes`. If there are no accepted outcomes in the window, the row is
`not_measured`; it is not zero minutes.

### Pass Rate

`business_factory.pass_rate.terminal_outcomes_bps.v1`

Accepted outcomes divided by terminal reviewed outcomes, grouped by
`work_kind`.

Formula:

```text
accepted_count / terminal_count * 10000
where terminal states are accepted, rejected, revision_requested, unavailable
```

Unit: `basis_points`. If there are no terminal outcomes in the window, the row
is `not_measured`; it is not a 0% or 100% pass rate.

### Review Minutes

`business_factory.review_minutes.v1`

Ledgered human review minutes recorded on accepted-outcome economics rows,
grouped by `work_kind`.

Formula:

```text
sum(omni_accepted_outcome_economics.review_minutes)
where archived_at is null
  and updated_at in [window_start, window_end)
```

Unit: `minutes`. The economics row is the audit receipt; no review-minute
dashboard may use issue comments, free-form notes, or estimates.

### Operator Minutes Per Engagement

`business_engagement.operator_minutes.review_ledger_floor.v1`

Current auditable lower bound for operator minutes per engagement. It sums
ledgered review minutes by opaque engagement ref. Until a broader
operator-labor event ledger exists, this metric must carry
`caveat.business_metrics.operator_minutes_review_only_until_labor_ledger`.

Formula:

```text
sum(omni_accepted_outcome_economics.review_minutes)
joined to omni_accepted_outcome_contracts
grouped by coalesce(customer_ref, subject_ref)
```

Unit: `minutes`. This is dashboard-eligible only with the caveat shown next to
it. Any future broader operator-minute metric must add a first-class labor
ledger and update this document and the query pack in the same PR.

## Instrumented Query Pack

The SQL source of truth is
[`apps/openagents.com/workers/api/src/business-factory-metrics.sql`](../../apps/openagents.com/workers/api/src/business-factory-metrics.sql).
The Worker helper
[`business-factory-metrics.ts`](../../apps/openagents.com/workers/api/src/business-factory-metrics.ts)
runs the same queries against D1, and
[`business-factory-metrics.test.ts`](../../apps/openagents.com/workers/api/src/business-factory-metrics.test.ts)
asserts measured rows, `not_measured` rows, and public-safe refs.

