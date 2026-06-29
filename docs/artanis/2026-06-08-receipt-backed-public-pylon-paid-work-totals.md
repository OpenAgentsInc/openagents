# Receipt-Backed Public Pylon Paid-Work Totals

Issue: `OpenAgentsInc/openagents#554`

## Contract

`GET /api/public/pylon-stats` exposes accepted-work bitcoin totals only when
the totals are backed by public Nexus/Pylon settlement receipts. The settlement
gate is `gate.public.pylon.accepted_work_settlement_receipts.v1`.

The public fields are:

- `nexusAcceptedWorkPayoutSatsPaidTotal`
- `nexusAcceptedWorkPayoutSatsPaid24h`
- `nexusAcceptedWorkPayoutReceiptRefs`
- `nexusAcceptedWorkSettlementGate`

## Counted Evidence

A receipt counts toward public paid-work totals only when all of the following
are true:

- the receipt kind is `settlement_recorded`;
- the linked payout intent has accepted-work refs;
- the public Nexus/Pylon receipt projection reports `realBitcoinMoved: true`;
- the settlement projection is terminal and settled;
- the payout movement allows the terminal settlement claim;
- the payout amount is bitcoin millisatoshi and exactly divisible into sats.

Duplicate receipt retries for the same payout intent count once, using the
first qualifying settlement receipt in created-at order.

## Blocked Evidence

These do not count:

- simulation receipts;
- payment or verification receipts without settlement evidence;
- rejected reconciliation events;
- receipts with missing accepted-work refs;
- unsupported denominations;
- legacy aggregate sats that do not include public settlement receipt refs.

## Zero Versus Unavailable

`0` means the receipt store was readable and no qualifying settled accepted-work
receipts were present. `null` means receipt-backed totals were unavailable.
Public surfaces must render the settlement gate state instead of inferring from
missing numbers.

## Public Surfaces

The Artanis public report mirrors the gate and receipt refs in
`pylonSummary.acceptedWorkSettlementGate` and
`pylonSummary.acceptedWorkSettlementReceiptRefs`. The logged-out public Agent
view displays exact receipt refs when the paid-work totals gate is ready and
the gate label when it is blocked or unavailable.

## Verification

Run:

```sh
bun run --cwd workers/api test -- src/public-pylon-stats.test.ts src/artanis-public-report.test.ts src/artanis-nexus-pylon-adapters.test.ts
bun run --cwd apps/web test -- src/page/loggedOut/page/login.scene.test.ts src/docs-blog-route.test.ts
bun run --cwd workers/api test -- src/openagents-openapi-routes.test.ts src/openagents-capability-manifest-routes.test.ts src/redaction-regression.test.ts
bun run typecheck:api
bun run typecheck:web
```
