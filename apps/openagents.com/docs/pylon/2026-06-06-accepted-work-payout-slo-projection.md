# Accepted-Work Payout SLO Projection

Issue #355 / `OPENAGENTS-L-007` adds read-only accepted-work payout SLO
projections.

The implementation lives in
`workers/api/src/pylon-accepted-work-payout-slo.ts`.

## Purpose

The projection lets OpenAgents product surface show payout progress and attention states for
accepted Pylon work without implying that OpenAgents product surface can dispatch payouts, spend
from a wallet, mutate payout targets, change provider eligibility, charge a
buyer, or mark settlement.

It separates:

- dispatch requested;
- dispatch recorded;
- confirmation observed;
- verification complete;
- settled;
- failed;
- skipped;
- blocked;
- stale; and
- attention required.

## SLO Facts

Each record can expose:

- dispatch latency;
- confirmation latency;
- freshness state;
- failed attempt count;
- skipped attempt count;
- blocker refs;
- caveat refs;
- SLO breach refs;
- evidence refs; and
- source refs.

Raw timestamps are accepted only as input for calculating friendly latency and
freshness displays. They are not emitted in the projection.

## Authority Boundary

`PYLON_ACCEPTED_WORK_PAYOUT_SLO_READ_ONLY_AUTHORITY` denies:

- buyer charge mutation;
- live wallet spend;
- payout dispatch;
- payout target mutation;
- provider eligibility mutation; and
- settlement mutation.

The SLO projection is observational only. It cannot decide that a payout is
owed, dispatch a payout, retry a payout, settle a payout, or alter wallet or
provider state.

## Redaction

Public, customer, team, and agent projections redact private provider,
dispatch, confirmation, verification, settlement, workroom, source, blocker,
caveat, freshness, and SLO refs according to audience.

All projections reject wallet material, raw bitcoin payment material, invoices,
preimages, raw payout targets, private channel state, provider secrets,
credentials, private repo refs, customer data, and raw timestamps in refs.

## Tests

`workers/api/src/pylon-accepted-work-payout-slo.test.ts` covers:

- fixture decoding;
- settled SLO projection;
- read-only authority;
- dispatch and confirmation latency labels;
- public redaction;
- accepted-work, payout-progress, and settlement claim separation;
- evidence requirements for state labels;
- stale/failed/blocked attention state;
- negative count and negative latency rejection; and
- unsafe payment, wallet, provider, customer, and timestamp material rejection.
