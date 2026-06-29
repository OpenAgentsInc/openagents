# vertex-fleet run note — energy.flexible_load_proof.v1

**Date:** 2026-06-20  
**Worker:** claude-sonnet-4-6 (Vertex AI)  
**Branch:** vertex-fleet/energy.flexible_load_proof.v1

## Blocker advanced

`blocker.product_promises.energy_market_ingestion_missing`

## What was built

**ERCOT LMP ingestion adapter** — the smallest genuine piece for live
energy-market ingestion.

Files added:

| Path | Purpose |
|---|---|
| `apps/openagents.com/workers/api/src/ercot-lmp-ingestion.ts` | Typed adapter: validates pre-parsed ERCOT public API v2 LMP responses (using Effect Schema, no raw `JSON.parse`), filters by settlement point, computes average $/MWh over the window, applies authority/caveat/missing-data-flag logic, returns `ErcotLmpWindowRecord` compatible with `ArtanisPylonPowerMarketWindowRecord`. |
| `apps/openagents.com/workers/api/src/ercot-lmp-ingestion.test.ts` | 9 tests: happy path average, authority flags, cross-point filtering, sparse-window flag, unknown-point flag, malformed schema rejection, missing-settlement-point rejection, hub enum coverage, example record shape. All pass. |

### What this provides

- A typed contract between the ERCOT v2 public-reports API and our
  internal Artanis/Pylon economics pipeline.  
- Authority boundary: `read_only_market_data_ingestion` — no grid
  dispatch, no market-data mutation, no wallet spend.  
- An `exampleErcotLmpWindowRecord()` fixture that can seed the modeled
  operator proof report while live ingestion is not yet wired to a cron.

## What remains for green

The promise stays **planned**. Still genuinely missing:

1. **Live cron/worker** — a scheduled Cloudflare Worker (or cron trigger)
   that calls the ERCOT API, passes the body through `json-boundary.ts`,
   and invokes `normalizeErcotLmpResponse` to populate a real
   `ArtanisPylonPowerMarketWindowRecord` with `claimState: 'measured'`.
2. **Work-class flex profiles from real telemetry** — schemas exist
   (`pylon-flexible-load-profiles.ts`); real per-workload measurements
   needed.
3. **Real flexible-load event history** — schemas exist
   (`pylon-flexible-load-events.ts`); the training-marathon curtailment
   drill receipt is the intended first real event.
4. **Owner sign-off** — required for any green flip per the promise rules.
