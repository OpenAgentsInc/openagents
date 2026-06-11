# Pylon Capacity Funnel And Dark-Capacity Reasons

Date: 2026-06-06

Status: implemented contract note for GitHub issues #322 / `OPENAGENTS-075` and
#362 / `OPENAGENTS-LATE-002`.

## Purpose

The capacity funnel shows where Pylon/provider capacity sits before it becomes
accepted and, later, paid or settled work. It also records why capacity is
"dark" instead of silently treating registered machines as useful supply.

The pure projection contract lives in
`workers/api/src/pylon-capacity-funnel.ts`. The live public route lives in
`workers/api/src/pylon-capacity-funnel-live-routes.ts`.

## Funnel Stages

The v1 stage model is:

- `registered`;
- `benchmarked`;
- `eligible`;
- `assigned`;
- `running`;
- `artifact_producing`;
- `accepted`;
- `paid`;
- `settled`;
- `dark`.

The model keeps capacity position separate from accepted-work economics and
settled provider payout. A capacity record can be accepted without being paid,
and paid without being settlement-proof.

## Dark-Capacity Reasons

Dark capacity requires explicit reason refs. The current examples include:

- no work assigned;
- missing payout target;
- not benchmarked;
- not eligible;
- blocked by trust policy;
- low connectivity;
- insufficient liquidity;
- failed run;
- not accepted.

Reason refs are public-safe labels, not raw host identifiers, payout targets,
hardware telemetry, or wallet material.

## Projection Rules

Public projection can show public-safe provider/node identity, stage, caveats,
evidence refs, work class refs, and dark-capacity reason refs. It hides private
provider/node refs, reward refs, and settlement refs.

Operator projection can see safe private provider/node refs and safe reward or
settlement refs. It still rejects raw host identifiers, private hardware
telemetry, wallet/payment secrets, provider tokens, raw runner logs, private
repo material, customer-private data, and raw timestamps.

## Provider Job Lifecycle

The live public route projects assigned, running, artifact-producing, and
accepted stages from durable `pylon_provider_job_lifecycle` rows, then falls
back to the public-safe assignment state when a lifecycle row is missing or
lagging. Assignment creation and assignment-state updates write the assignment
row and matching lifecycle row in one D1 `db.batch`; the fallback prevents the
public count route from showing zero accepted work if a lifecycle projection
lags behind the accepted-work assignment row.

Lifecycle stages are:

- `offered`;
- `accepted`;
- `running`;
- `artifact_submitted`;
- `closeout_submitted`;
- `accepted_work`.

The public funnel remains count-only. Lifecycle rows are keyed by Pylon and
assignment refs, but the public route still returns aggregate stage and dark
reason counts without exposing device identifiers, wallet material, raw
artifacts, or provider-private details.

## Retained History

Issue #4660 adds retained live-funnel snapshots. The Worker cron refreshes the
current hourly bucket and current daily bucket from the live public aggregate.
The public history route is:

- `GET /api/public/pylon-capacity-funnel/history`

The history route returns count-only hourly and daily series with the same
read-only accounting authority boundary as the live route. It does not expose
device identifiers, owner linkage, wallet material, raw assignment details,
artifacts, or provider-private details.

Retention policy:

- hourly snapshots: 14 days;
- daily snapshots: 180 days.

Closeout evidence recorded 2026-06-11:

- the live history route retained hourly and daily buckets for both
  2026-06-10 and 2026-06-11;
- provider lifecycle accounting from #4659 was already deployed;
- transition receipt `promise_transition_cd1c3145-eccd-4985-b48a-99f8b1b20fbe`
  proposed `pylon.no_dark_capacity_accounting.v1` from yellow to green before
  the registry edit;
- registry version `2026-06-10.27` marks the promise green while preserving
  the boundary that capacity presence is not accepted work, payment,
  settlement, or withdrawal evidence.

## Aggregates

`aggregatePylonCapacityFunnel` returns count-only funnel data:

- total count;
- count by stage;
- count by dark-capacity reason;
- individual stage counters for registered, benchmarked, eligible, assigned,
  running, artifact-producing, accepted, paid, settled, and dark;
- `settledClaimAllowedCount`, which only counts records with settlement
  evidence.

The aggregate is meant for dashboards and public proof bundles without leaking
private node or provider details.

## Accounting Projection

Issue #362 adds `accountPylonCapacityFunnel`, a stricter accounting projection
for investor/operator reporting. It keeps the same stage model but adds:

- read-only capacity-accounting authority flags;
- dark-capacity reason summaries with capacity refs, caveat refs, evidence
  refs, and work class refs;
- fresh, stale, and unknown freshness counts;
- stale capacity refs;
- paid-but-not-settled count;
- visible settlement-claim count;
- settled-without-visible-receipt count;
- public-safe claim-boundary caveats.

This matters because a capacity record can be in the `settled` stage while the
public or customer projection cannot see the settlement receipt. The accounting
projection therefore separates:

- stage count;
- paid-but-not-settled count;
- settlement receipts visible to the current audience;
- settlement refs that may exist but are hidden from the current audience.

The accounting projection does not:

- spend from a wallet;
- dispatch a provider payout;
- mutate provider eligibility;
- mutate payout targets;
- mutate settlement;
- assign capacity;
- upgrade a public claim.

## Settlement Boundary

Capacity accounting is not wallet accounting. A `paid` stage means the capacity
has reward evidence. A `settled` stage means there is settlement evidence in the
underlying record. A public settlement claim should only be made when the
projection exposes a public-safe settlement receipt. Otherwise the record is
counted as settled without a visible receipt for that audience.

## Tests

`workers/api/src/pylon-capacity-funnel.test.ts` covers:

- public/operator projection splits;
- private provider/node redaction;
- friendly time labels and raw timestamp omission;
- funnel and dark-reason aggregation;
- capacity accounting projection, detailed dark-reason summaries, stale
  capacity counts, paid-but-not-settled counts, visible settlement receipt
  counts, and no-mutation authority;
- required refs for benchmarked/eligible/running/paid/dark stages;
- rejection of raw host, private hardware, wallet, payment, provider, runner,
  customer, and payout-destination material.

`workers/api/src/pylon-capacity-funnel-live-routes.test.ts` and
`workers/api/src/pylon-api-routes.test.ts` cover:

- lifecycle-backed live funnel stages;
- unchanged public route shape with counts only;
- assignment/lifecycle atomic D1 batch writes and mid-batch failure behavior;
- retained hourly/daily history snapshots, retention pruning, and the public
  history route's count-only output.
