# 40. Count events

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Some clients need a number, not the events. This chapter adds NIP-45 counts to
the relay.

Part IX, A relay. Result: NIP-45 `COUNT` requests and responses, `approximate`
counts, and HyperLogLog with 256 registers whose offset comes from the filter.

## What this chapter teaches

- `["COUNT", id, filter...]` answered by `["COUNT", id, {"count": n}]`.
- `approximate: true` when the relay estimates.
- HyperLogLog: 256 registers, the register index and offset chosen as the NIP
  specifies so different relays' sketches merge.
- `CLOSED` when counting is refused.

## What exists today

- The Coax relay reports `COUNT` as unsupported. The production relay
  advertises NIP-45.

## Dependencies

- [36. Match filters with indexes](ch036-match-filters-with-indexes.md)

## Done means

- Exact counts equal the length of the scanned result.
- HLL sketches built on two relays merge to the sketch of the union.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-45](../nips/official/45.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
