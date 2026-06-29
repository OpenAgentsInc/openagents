# Pylon Wallet Liquidity Readiness

Issue #351 / `OPENAGENTS-L-003` adds the first read-only Pylon wallet liquidity
readiness model.

The implementation lives in
`workers/api/src/pylon-wallet-liquidity-readiness.ts`.

## Purpose

The model separates wallet and channel liquidity facts that are often blurred
together in product copy:

- spendable onchain funds;
- anchor reserve;
- outbound liquidity;
- inbound liquidity;
- total channel balance;
- send readiness; and
- receive readiness.

It does not store wallet secrets, raw channel monitor state, raw private
channel data, raw invoices, preimages, payout targets, or credentials.

## Evidence States

Each liquidity bucket has an evidence state:

- `modeled`;
- `reported`;
- `verified`;
- `stale`;
- `blocked`;
- `unknown`.

Reported or verified buckets require safe amount refs and evidence refs.
Blocked buckets require blocker refs. Stale buckets require caveat refs.

Every record must include all required buckets:

- `spendable_onchain`;
- `anchor_reserve`;
- `outbound_liquidity`;
- `inbound_liquidity`;
- `total_channel_balance`.

## Send And Receive Readiness

Directional readiness is:

- `ready`;
- `degraded`;
- `not_ready`;
- `blocked`;
- `unknown`.

Blocked send or receive readiness requires blocker refs. Degraded or not-ready
liquidity requires warning or caveat refs. The initial warning vocabulary
covers no inbound liquidity, no outbound liquidity, insufficient anchor
reserve, stale sync, channel unavailable, and no approved payout target.

## Authority Boundary

`PYLON_WALLET_LIQUIDITY_READ_ONLY_AUTHORITY` explicitly denies:

- wallet mutation;
- channel mutation;
- liquidity provision mutation;
- live wallet spend;
- payout dispatch;
- payout target mutation;
- settlement mutation.

The model can support operator diagnosis and later eligibility projections, but
it cannot spend, rebalance, open channels, register payout targets, dispatch
payouts, or claim settlement.

## Redaction

Public, customer, team, and agent projections hide private provider refs,
wallet refs, amount refs, channel posture refs, payout-target admission refs,
sync refs, warning refs, evidence refs, and source refs according to audience.

Operator projections can include safe private refs, but they still reject raw
wallet material, raw channel state, raw liquidity telemetry, host identifiers,
payment material, payout targets, provider secrets, customer data, and raw
timestamps.

## Tests

`workers/api/src/pylon-wallet-liquidity-readiness.test.ts` covers:

- fixture decoding;
- read-only authority;
- send/receive readiness labels;
- required bucket coverage;
- evidence requirements for reported, verified, stale, and blocked buckets;
- public redaction of private provider, wallet, amount, channel, target, sync,
  and evidence refs; and
- rejection of raw wallet, channel, payment, payout, provider, and telemetry
  material.
