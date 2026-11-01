# 22. Kinds and tags

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A kind says what an event means and how relays keep it. Tags point at other
events, keys and addresses. This chapter reads the kind ranges, which NIP-16
and NIP-33 first defined and NIP-01 now holds, and the standard tags.

Part VI, The event model. Result: Kind ranges (regular, replaceable 0, 3 and
10000–19999, ephemeral 20000–29999, addressable 30000–39999 by `d` tag) and
the standard tags `e`, `p`, `a` and `d`.

## What this chapter teaches

- Ranges: regular (1000–9999 and the small kinds), replaceable, ephemeral and
  addressable.
- Replacement: the newest `created_at` wins; on a tie the lowest ID wins.
- Tags: `e` with relay hint and marker, `p`, `a` as `kind:pubkey:d`, `d`.
- Kind 0 metadata: content is a JSON object; NIP-24's extra fields are kept
  and not interpreted here.
- A tag is a reading, not evidence the referenced object exists.

## What exists today

- `coax/lib/nostr/tags.coax` reads `e`, `p`, `a` and `d`; `metadata.coax`
  reads kind 0 `name`, `about` and `picture`.
- Record:
  [2026-10-30-nostr-tags-metadata.md](../../docs/verification/2026-10-30-nostr-tags-metadata.md).

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)

## Done means

- Every range boundary (9999, 10000, 19999, 20000, 29999, 30000, 39999, 40000)
  classifies correctly.
- A malformed `a` tag refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-01](../nips/official/01.md)
- [NIP-16](../nips/official/16.md)
- [NIP-33](../nips/official/33.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
