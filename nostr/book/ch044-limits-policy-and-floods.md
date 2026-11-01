# 44. Limits, policy and floods

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A public relay is attacked. This chapter collects every limit and policy in
one place and drives each past its bound.

Part IX, A relay. Result: Per-connection and global limits, rate limits with
`rate-limited:` reasons, storage caps, slowloris timeouts and a write policy
that is declared, not implicit.

## What this chapter teaches

- Connection, subscription, filter, frame and storage caps.
- Rate limits per key and per connection.
- Slow clients: handshake and idle timeouts.
- Write policy: who may write which kinds, declared in one table.

## What exists today

- `coax/tests/nostr/relay_flood.py` drives 216 connections, 512 subscriptions,
  oversized frames and 2,100 events until "relay storage is full" at 2,048.
- Record:
  [2026-10-30-coax-relay-r1-tailnet-flood.md](../../docs/verification/2026-10-30-coax-relay-r1-tailnet-flood.md).

## Dependencies

- [34. Admit an event](ch034-admit-an-event.md)
- [36. Match filters with indexes](ch036-match-filters-with-indexes.md)

## Done means

- Every limit has a flood case that receives its refusal.
- Rate limits are configurable and shown in NIP-11.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-01](../nips/official/01.md)
- [NIP-11](../nips/official/11.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
