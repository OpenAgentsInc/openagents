# 50. Notes, threads and subjects

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Text notes and replies are the most common events. This chapter reads and
writes threads correctly.

Part X, Every NIP. Result: Kind 1 notes, NIP-10 marked `e` tags (`root`,
`reply`, `mention`) with the deprecated positional form, `p` tags for
participants, NIP-14 `subject` and NIP-36 `content-warning`.

## What this chapter teaches

- Marked `e` tags: `["e", id, relay, "root"|"reply", pubkey]`.
- Positional `e` tags, deprecated, read for old events only.
- Reply `p` tags: the parent's author plus the parent's `p` tags.
- NIP-14 `subject` and NIP-36 `["content-warning", reason]`.

## What exists today

- `coax/lib/nostr/tags.coax` reads `root`, `reply` and `mention` markers.
  Thread building, NIP-14 and NIP-36 are not implemented.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Done means

- Thread reconstruction on a fixture matches a hand-built tree.
- A positional-only event is read with the documented rule.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-10](../nips/official/10.md)
- [NIP-14](../nips/official/14.md)
- [NIP-36](../nips/official/36.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
