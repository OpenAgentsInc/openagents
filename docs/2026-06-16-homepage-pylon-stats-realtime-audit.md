# Homepage pylon-stats: instant load + realtime audit (#5050)

**Date:** 2026-06-16
**Surface:** `openagents.com` homepage stats bar — PYLONS ONLINE / WORKING NOW /
SATS SETTLED · 24H / TRAINING CONTRIBUTORS.
**Symptom (owner report):** the stats render `…` for ~15 seconds before the real
counts appear, and they do not update live as pylons join/leave.

## TL;DR

The numbers were slow for two compounding reasons, both now fixed (phase 1):

1. **Server: `GET /api/public/pylon-stats` took ~5s on every call** because the
   settlement-totals computation was a **serial N+1**: it listed up to 1000
   payment-authority receipts and then, for each settlement receipt, did up to
   **3 sequential D1 reads** (`readPayoutIntentByRef` → `readReconciliationEventByRef`
   → `readPayoutAttemptByRef`). With the response marked `no-store`, this ran
   fresh on **every** request and for **every** visitor.
2. **Client: the bar showed `…` until that first ~5s fetch landed, then polled
   only every 15s** — so even after load, "online now" lagged up to 15s behind
   reality.

(The perceived ~15s = the ~5s server compute, plus a cold Worker/isolate, plus
the next value only arriving on the 15s poll.)

Follow-up on 2026-06-17 fixed a separate correctness bug in the same bar:

- `SATS SETTLED · 24H` only summed NIP-90 market receipts, so direct
  treasury/tips-buffer payments made during the launch-recognition work were not
  represented. The public stats payload now exposes `publicRealSatsSettled24h`,
  composed from real settled treasury outflows, NIP-90 settled receipts, and
  accepted-work settlement receipts while avoiding accepted-work/treasury
  double-counting.
- `TRAINING CONTRIBUTORS` still read hardcoded zeros. It now reads distinct live
  Tassadar run contributor refs from the training authority store instead of
  registrations or constants. The stricter accepted-work/qualified contributor
  gate remains separate.

Follow-up #5171 on 2026-06-17 fixed the remaining first-paint flash:

- The Worker now embeds the cached `/api/public/pylon-stats` JSON into the `/`
  and `/pylon` app-shell HTML as an application/json boot payload.
- `pylonStatsElement` reads that payload and seeds the slot-text values before
  its first client fetch. If the payload is absent, it keeps the old `…`
  fail-soft behavior and the 3s poll still refreshes from the public route.

## Root cause detail

### Server — `workers/api/src/public-pylon-stats.ts`
`publicPylonSettlementTotalsFromReceipts` used:

```
settlementReceipts.reduce(async (previous, receipt) => {
  const counted = await previous            // serializes the whole loop
  const intent  = await readPayoutIntentByRef(receipt.payoutIntentRef)
  const event   = await readReconciliationEventByRef(receipt.eventRef)
  const attempt = await readPayoutAttemptByRef(receipt.payoutAttemptRef)
  ...
})
```

`reduce(async …)` forces strictly serial iteration, and each iteration awaits up
to 3 D1 round-trips. For N settlement receipts that is ~3N sequential queries.
That is the dominant cost (the snapshot also lists up to 1000 NIP-90 market
receipts and queries pylon presence, but those are single scans).

### Server — `workers/api/src/public-pylon-stats-routes.ts`
The handler returned `noStoreJsonResponse(stats)` and recomputed the snapshot on
**every** request — no caching at all, so the N+1 ran per request and per
visitor.

### Client — `apps/web/src/scene/pylonStatsElement.ts`
- Renders `…` (LOADING) into each stat slot on mount.
- Fetches `GET /api/public/pylon-stats` once immediately, then on a `setInterval`
  of **15_000 ms**.
- So the `…` persists for the full first-fetch latency (~5s + cold start), and
  subsequent updates lag up to 15s.

There is **no realtime push** — no WebSocket/SSE/Durable-Object presence channel
for the homepage. Pylon presence is written to D1 by heartbeat; nothing streams
deltas to connected browsers.

## Fixes shipped (phase 1 — this change)

1. **Killed the N+1** (`public-pylon-stats.ts`): resolve every settlement
   receipt's intent/event/attempt **in parallel** (`Promise.all`), memoizing
   intent reads by ref (receipts share intents), then accumulate in sorted order
   with the **exact same** first-pass-per-intent semantics. ~3N sequential reads
   → effectively one parallel batch.
2. **In-isolate snapshot cache** (`public-pylon-stats-routes.ts`): cache the
   computed snapshot for `STATS_CACHE_TTL_MS = 4_000`. Repeated requests (the
   homepage poll + concurrent visitors) return instantly and at most ~4s stale —
   well inside the "online now" (minutes) and "sats 24h" windows. The response
   stays `no-store` so each poll still gets the latest *cached* value (not a
   frozen browser copy). The cache is bypassed when tests inject in-memory stores.
3. **Faster client poll** (`pylonStatsElement.ts`): 15_000 ms → **3_000 ms**.
   Cheap now that the endpoint is cached; gives "pylons join/leave" updates within
   ~3s.
4. **Correct multi-rail sats total** (`public-pylon-stats.ts` +
   `pylonStatsElement.ts`): added `publicRealSatsSettled24h` and made the
   homepage prefer it over the old NIP-90-only sum. Direct treasury outflows are
   shown as real settled sats, but are not upgraded into accepted-work totals.
5. **Correct training contributor count** (`public-pylon-stats.ts`): replaced
   hardcoded training zeros with the live run authority's distinct contributor
   refs for the current Tassadar run.

Net effect: warm requests are sub-200ms (cache hit), so the `…` is now a brief
flash rather than ~15s, and the bar refreshes ~5× faster.

## Fixes shipped (phase 2 — #5171)

1. **Boot snapshot (zero `…`).** The Worker injects the cached public stats
   snapshot into the initial app shell for `/` and `/pylon`; the client reads it
   before mounting the slot-text controllers. Result: real values paint with the
   page whenever the cached snapshot is available.
2. **No divergent computation.** The embedded payload is generated by the same
   `/api/public/pylon-stats` route contract that the client poll uses.
3. **Fail-soft fallback.** If the snapshot cannot be read, the app shell is
   served unchanged and the client falls back to `…` until the first fetch lands.

## Remaining realtime work

1. **Push instead of poll (true realtime).** Add a presence Durable Object that
   already sees every pylon heartbeat/registration and **broadcasts stat deltas
   over WebSocket/SSE** to connected homepages. Eliminates polling and updates the
   instant a pylon joins/leaves. The client falls back to the 3s poll if the
   socket drops.
2. **Background refresh (no cold-path wait).** Refresh the cached snapshot on a
   scheduled alarm / `ctx.waitUntil` so even a cache miss never blocks a request.

## Verification

- `bun test` in `workers/api`: `public-pylon-stats(.routes)` suites green,
  N+1 refactor preserves settlement-counting semantics, cache bypassed under
  test, multi-rail sats are counted, and training contributor refs come from the
  run authority store.
- Post-deploy: `GET /api/public/pylon-stats` warm latency should drop from ~5s to
  sub-200ms; homepage `…` becomes a brief flash; counts refresh every ~3s.
- #5171: `pylonStatsElement` boot-seeding tests cover embedded snapshot first
  paint and absent-payload fallback; Worker tests cover HTML injection, unchanged
  fallback, route scoping, and script-tag escaping.
