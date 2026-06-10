# NIP-90 Market Public Receipts

Date: 2026-06-10

Issue: `OpenAgentsInc/openagents#4640`

Registry version during implementation: `2026-06-10.4`

## Public Surfaces

- `GET /api/public/nip90-market/receipts/{receiptRef}`
  - Returns a public-safe receipt projection for settled NIP-90 market jobs.
  - Pending, blocked, failed, malformed, and unsafe rows return `404`.
- `GET /api/public/pylon-stats`
  - Includes `nip90MarketSettlementStats` with `compute`, `data`, and `labor`
    stream counters.
  - Each stream reports jobs settled in the last 24 hours, jobs settled total,
    sats settled in the last 24 hours, sats settled total, and counted receipt
    refs.

## Projection Contract

The public receipt projection includes:

- receipt ref
- settled state
- stream kind: `compute`, `data`, or `labor`
- amount in sats
- settled timestamp
- public job, request event, and result event refs
- public caveat and source refs

The projection structurally excludes raw invoices, preimages, payment hashes,
mnemonics, private keys, wallet material, provider credentials, customer data,
and counterparty destination details. Current buy-mode rows project as the
`compute` stream; the data and labor stream counters are present for the next
plan steps.

## Counting Rules

- Only `state = "settled"` rows with a public receipt ref count.
- Rows whose amount is not an exact sats value are not projected into public
  sats counters.
- Pending, blocked, failed, duplicate, or unsafe rows do not count.
- `GET /api/public/pylon-stats` keeps these counters separate from the legacy
  Nexus/Pylon accepted-work settlement totals.

## Verification

```bash
cd apps/openagents.com/workers/api
bunx vitest run src/public-pylon-stats.test.ts src/public-nip90-market-receipt-routes.test.ts
```

The focused tests cover public lookup, no-store response headers, mutation
rejection, pending/unsafe projection exclusion, per-stream counters, 24-hour
windows, and private-material scan assertions.
