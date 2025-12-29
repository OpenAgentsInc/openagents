# Exchange Layer: Relay-Backed Order Fetch

Date: 2025-12-29
Owner: Codex (GPT-5)
Scope: neobank exchange relay order discovery

## Summary
Implemented relay-backed order fetching for the exchange client. Exchange relay now opens a short-lived subscription against connected relays, ingests NIP-69 order events, updates the local cache with the latest order per order_id, and returns filtered results that exclude expired orders.

## Changes
- Added a relay fetch pass in `ExchangeRelay::fetch_orders` to pull recent orders from connected relays before returning cached results.
- Added a bounded fetch window (800ms) and limit (500) to avoid unbounded subscription reads.
- Added per-order dedupe by `order_id` + `created_at` to honor replaceable semantics.
- Applied an expiration filter (`expires_at > now`) to relay results for parity with exchange client filtering.

## Files
- `crates/neobank/src/relay.rs`

## Notes
- This is a minimal relay query loop that uses a short-lived subscription and then unsubscribes; it does not wait for EOSE (not exposed at the pool layer yet).
- When no relays are configured, behavior stays cache-only (unchanged).

## Follow-ups
- Add pool-level EOSE awareness so relay fetch can complete deterministically without timeouts.
- Add RFQ (5969/6969) publish + subscribe over relays to complete RFQ market flow.
- Consider caching/refresh policy (staleness window, incremental updates) to avoid over-subscribing on frequent fetch calls.
