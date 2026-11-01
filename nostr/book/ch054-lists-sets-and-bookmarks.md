# 54. Lists, sets and bookmarks

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Lists hold a person's mutes, bookmarks and groupings. Some items are public;
some are encrypted to the list owner. This chapter builds both.

Part X, Every NIP. Result: NIP-51 standard lists (10000 mutes, 10001 pins,
10003 bookmarks and others) and sets (30000 follow sets and others) with
public tags and private items encrypted to self, and NIP-B0 kind 39701 web
bookmarks.

## What this chapter teaches

- Standard lists are replaceable (10000–10102); sets are addressable
  (30000–30267) with a `d` tag.
- Private items: a JSON tag array encrypted with NIP-44 to the owner's own
  key.
- NIP-B0: kind 39701 with `d` as the URL without scheme, plus `published_at`,
  `title` and `t`.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)

## Done means

- Private items round-trip only with the owner's key.
- A NIP-04-encrypted private section is read and re-encrypted with NIP-44 on
  the next write.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-51](../nips/official/51.md)
- [NIP-B0](../nips/official/B0.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
