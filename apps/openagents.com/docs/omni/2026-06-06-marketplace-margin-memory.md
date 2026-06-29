# Marketplace Margin Memory

Status: implemented for issue #375 / `OPENAGENTS-LATE-015`.

## Purpose

Marketplace memory should help reviewed signatures and packages rank by
business usefulness without becoming routing, payout, settlement, public rank,
or module promotion authority. This contract records evidence-only economics,
quality, review burden, repeat demand, refund exposure, and settlement state
for reviewed capabilities.

Implementation:

- `workers/api/src/marketplace-margin-memory.ts`
- `workers/api/src/marketplace-margin-memory.test.ts`

## Ranking Inputs

The memory record tracks:

- accepted, rejected, refunded, and retry counts;
- accepted outcome refs;
- rejected and refunded outcome refs;
- retry evidence refs;
- revenue evidence refs;
- gross margin evidence refs;
- accepted revenue cents;
- accepted gross profit cents;
- provider payable cents;
- settled provider cents;
- total buyer count;
- repeat buyer count;
- repeat buyer signal refs;
- review burden refs and score;
- settlement state refs;
- program signature, module, tool, package, source, provider, reviewer, route,
  and work-class refs; and
- caveat/evidence refs.

The projection calculates:

- acceptance rate in basis points;
- gross margin in basis points;
- refund rate in basis points;
- repeat buyer rate in basis points;
- review burden label;
- settlement label; and
- a bounded ranking score in basis points.

The score is an input for review and marketplace decisions. It does not mutate
public ranking by itself.

## Economic Boundaries

The contract preserves distinctions between:

- modeled marketplace value;
- accepted outcomes;
- accepted revenue;
- accepted gross profit;
- provider payable value;
- settled provider value;
- refunds; and
- settlement state.

Accepted revenue requires revenue evidence and accepted outcome refs. Accepted
gross profit requires gross margin evidence, revenue evidence, and accepted
outcome refs. Provider payable value requires a payable, partially settled, or
settled state. Settled provider value requires partially settled or settled
state and cannot exceed payable value. A fully settled state requires
settlement refs and fully settled provider payable value.

## Settlement States

Supported settlement states:

- `modeled`;
- `accepted`;
- `payable`;
- `partially_settled`;
- `settled`;
- `refunded`;
- `disputed`; and
- `unknown`.

Settlement labels are projected as friendly labels, not inferred claims.

## Authority Boundaries

Marketplace margin memory remains evidence-only. It cannot:

- mutate public ranking;
- promote modules;
- mutate payouts;
- mutate routing; or
- mutate settlement.

Public rank candidate flags can become true only when reviewed or promoted
records have accepted outcome refs and evidence refs, but even then the
projection does not change ranking state.

## Projection Audiences

Supported audiences are:

- `public`;
- `customer`;
- `team`; and
- `operator`.

Public and customer projections redact private provider, reviewer, revenue,
settlement, and source refs. Team projections retain more review metadata but
still hide private provider and settlement refs. Operator projections can
retain the full safe ref set.

All projections reject private customer data, raw source archives, tokens,
provider payloads, wallet/payment material, payout targets, private repo refs,
raw runner logs, and raw timestamps.

## Tests

Coverage includes:

- ranking inputs and calculated rates;
- gross margin and ranking score calculations;
- refund and repeat buyer rate handling;
- review burden labels;
- settlement labels and settlement state constraints;
- revenue, gross margin, count, and evidence requirements;
- audience redaction; and
- hard false public rank, module promotion, payout, routing, and settlement
  mutation authority.
