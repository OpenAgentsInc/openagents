# 53. Long-form, drafts and wiki

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Long documents are addressable events edited in place. This chapter covers
articles, encrypted drafts and wiki pages.

Part X, Every NIP. Result: Kind 30023 articles and 30024 drafts in Markdown,
NIP-37 kind 31234 draft wraps encrypted with NIP-44, and NIP-54 kind 30818
wiki articles with `d`-tag normalization.

## What this chapter teaches

- NIP-23: `d`, `title`, `summary`, `image`, `published_at`; Markdown content.
- NIP-37: kind 31234 with `k` and an encrypted draft body; kind 10013 lists
  private relays; checkpoints as kind 1234.
- NIP-54: kind 30818, `d` normalized (lowercase, non-letters to `-`), merge
  requests kind 818, redirects kind 30819.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)

## Done means

- NIP-54's normalization examples give the stated `d` values.
- A NIP-37 draft decrypts only for its author.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-23](../nips/official/23.md)
- [NIP-37](../nips/official/37.md)
- [NIP-54](../nips/official/54.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
