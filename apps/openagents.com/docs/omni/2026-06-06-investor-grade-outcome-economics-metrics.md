# Investor-Grade Outcome Economics Metrics

Date: 2026-06-06

Status: implemented for issue #361.

## Purpose

Investor-grade outcome economics turns accepted workroom economics into a
read-only projection that can be grouped by work class.

The projection is for operator, team, agent, customer, and public review. It
does not charge a buyer, settle a provider payout, dispatch bitcoin, mutate a
refund, or upgrade a public claim.

## Contract

`workers/api/src/omni-investor-outcome-economics-metrics.ts` defines:

- `OmniInvestorOutcomeEconomicsMetricRecord`;
- revenue, provider-settlement, and refund claim states;
- read-only authority flags;
- aggregate metrics by work kind;
- audience-aware projections;
- validation for unsafe refs, raw timestamps, private payment material, wallet
  material, private provider material, private customer material, and false
  authority.

The model keeps each accepted outcome row separate from the underlying
`omni_accepted_outcome_economics` ledger. This avoids rewriting the v1
economics table while letting later investor bundles consume a richer summary.

## Metrics

Each projected aggregate includes:

- accepted outcome count;
- accepted revenue;
- accepted gross profit;
- gross margin basis points;
- runner, provider, retry, review, grading, artifact, and refund exposure cost
  components;
- review minutes;
- provider payable amount;
- provider settled amount;
- refund exposure and refunded amount;
- revenue, provider-settlement, and refund claim labels;
- public-safe evidence, caveat, blocker, source, economics, revenue, review,
  grading, retry, refund, and settlement refs.

Accepted gross profit is derived from accepted revenue minus runner cost,
provider payable cost, retry cost, review cost, grading cost, artifact cost,
and refund exposure. Provider-settled amounts are tracked separately so a
payable or dispatched provider amount cannot be presented as settled.

## Claim Boundaries

Revenue, provider, and refund states are intentionally separate.

- Accepted revenue can be claimed only after the revenue state is accepted or
  later.
- Provider payable can be claimed before settlement, but provider settlement
  can be claimed only when the state is `settled`, a positive settled amount is
  present, and settlement refs remain visible to the audience.
- Refund claims require refund refs and a partial, refunded, or settled refund
  state.
- Modeled-only rows remain marked as modeled and cannot become accepted
  revenue claims.

The projection uses friendly display time only. Raw ISO timestamps remain in
records and do not appear in the public projection.

## Audience Redaction

Public, customer, and agent projections remove private accepted-outcome,
economics, evidence, provider, settlement, refund, customer, source, caveat,
blocker, and workroom refs. Team projections can retain more operational refs
but still hide private provider, settlement, refund, and workroom material.
Operator/private projections can include safe internal refs, but still reject
secrets, raw payment material, wallet material, raw logs, private repository
refs, and raw timestamps.

## Tests

`workers/api/src/omni-investor-outcome-economics-metrics.test.ts` covers:

- work-class aggregation and margin math;
- public redaction while retaining public settlement evidence;
- accepted, payable, settled, modeled, and refund claim separation;
- false authority rejection;
- invalid provider settlement/refund overclaim rejection;
- unsafe ref and raw timestamp rejection.
