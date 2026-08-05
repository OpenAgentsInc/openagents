# @openagentsinc/mkt-swp-pair

Asset/direction selection from live Offerings, plus the rate and fee panel,
for the MKT-SWP swap widget (openagents#9316, SWAP-1): the headless
pair-selection state machine the SWAP-0 shell mounts.

## What lives here

- `asset.ts` — asset identity. `asset_id`
  (`swp:1:bip122:<ref>:btc:{chain,lightning}`) is identity; tickers and
  display names are labels only and never used for matching, grouping,
  pricing, or replay identity. Ordered directions, swap-type derivation.
- `amount.ts` — exact integer-satoshi arithmetic: canonical MKT-SWP §3.2
  wire strings, bigint-only parsing/formatting for BTC and sats display
  with a persisted decimal-separator preference. A source-scan test
  asserts no floating-point path exists in the amount code.
- `corpus.ts` — the live-Offering fold. Reachable directions are a
  function of the discovered `39601` corpus, never a hardcoded pair list.
  `max="0"` disables a side (§3.2); unreachable directions carry a typed
  reason (`no_offering`, `side_disabled`, `provider_paused`,
  `offerings_stale`), and the empty corpus is a first-class typed state.
- `selection.ts` — the pure reducer the SWAP-0 shell mounts: side-labelled
  selection where choosing the counterparty's asset swaps the sides
  (teardown §3.1), authoritative-side amount entry, MAX bounded by the
  Offering maximum, the BTC/sats toggle as the *only* path that changes
  denomination (no Boltz-style auto-switch), and the primary-action gate
  that states the single most proximate refusal.
- `quote.ts` — the held Quote's amount/fee terms re-derived with exact
  bigint arithmetic (mirroring the engine's `verify_amount_equation`);
  a mismatch surfaces `swp_amount_equation_mismatch`.
- `price-feed.ts` — exact price-feed pinning (§3.4): provenance rendering
  and refusal of substituted hosts, mirrors, fallback endpoints, altered
  pointers, values, or digests; staleness per the pinned max age.
- `view.ts` — render-ready views: pair selector with per-option
  reachability disclosed before selection, pre-typed limits, the primary
  action, and the fee panel framed as the §3.3 **fill promise** (a
  provider may not reduce the output after Order because its route or
  miner fee changed).
- `messages.ts` — amount refusals and §17 identifiers render through
  `@openagentsinc/swap-i18n`; pair/panel-specific messages keep stable
  local `swap.pair.*` keys shaped for migration into the SWAP-8 catalog.

Everything here is UX pre-checking over public heads and held Quote terms.
The MKT-SWP engine behind the SWAP-0 boundary owns verify-before-fund
truth; nothing in this package authorises funding. Quote selection across
providers is SWAP-3; destination entry is SWAP-2.

## Checks

```
pnpm --dir packages/mkt-swp-pair run check
```
