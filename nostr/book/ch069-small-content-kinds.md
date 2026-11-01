# 69. Small content kinds

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

These NIPs each define a small content type. This chapter reads and writes
them with the same tag readers.

Part X, Every NIP. Result: NIP-35 torrents (2003), NIP-64 chess games in PGN
(64), NIP-CC geocaches (37516) and logs (7516, 7517), and NIP-F4 podcasts.

## What this chapter teaches

- NIP-35: kind 2003 with `x` info hash, `file`, `tracker`, and `i` prefixes
  such as `imdb:`.
- NIP-64: kind 64 with PGN content; a client validates the PGN before display.
- NIP-CC: kind 37516 listings, 7516 found logs, 7517 verification events.
- NIP-F4: show and episode events and their lists.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Done means

- Each NIP's example event parses and round-trips.
- A kind 64 event with invalid PGN is shown as invalid.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-35](../nips/official/35.md)
- [NIP-64](../nips/official/64.md)
- [NIP-CC](../nips/official/CC.md)
- [NIP-F4](../nips/official/F4.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
