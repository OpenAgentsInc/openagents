# 56. Proof of work

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

A relay can ask for work before it stores an event. This chapter mines and
checks NIP-13 proof of work.

Part X, Every NIP. Result: NIP-13 difficulty as the number of leading zero
bits of the event ID, the `nonce` tag with its target, and relay admission by
`pow:` reason.

## What this chapter teaches

- Difficulty: count leading zero bits of the 32-byte ID.
- `["nonce", counter, target]`; a stated target lower than the work is honored
  as the claimed difficulty.
- Mining loop over the nonce with `created_at` updates.
- Relay policy: refuse with `pow: difficulty X is less than Y`.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)

## Done means

- The NIP-13 example ID has difficulty 21 as stated.
- An event whose target is below the relay minimum refuses even if its ID has
  more work.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-13](../nips/official/13.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
