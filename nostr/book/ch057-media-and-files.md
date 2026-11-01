# 57. Media and files

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Nostr events point at media stored elsewhere. This chapter reads the metadata
that describes it and checks the hashes it states.

Part X, Every NIP. Result: `imeta` tags (NIP-92), kind 1063 file metadata
(NIP-94), picture (20) and video (21, 22, 34235, 34236) events, voice messages
(1222, 1244), Blossom servers (kind 10063) and the deprecated NIP-96.

## What this chapter teaches

- NIP-92 `imeta`: `url`, `m`, `x` (SHA-256), `dim`, `blurhash`, `alt`,
  `fallback`.
- NIP-94: kind 1063 with `url`, `m`, `x`, `ox`, `size`.
- NIP-68 and NIP-71: picture-first and video events with `imeta`.
- NIP-A0: kinds 1222 and 1244 voice messages.
- NIP-B7 Blossom: content addressed by SHA-256, server lists in kind 10063,
  uploads with NIP-98-style auth.
- NIP-96 is `unrecommended` in favor of Blossom; read only.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [49. HTTP auth](ch049-http-auth.md)

## Done means

- Downloaded media whose SHA-256 differs from `x` is refused.
- Every NIP-92 example parses to its stated fields.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-92](../nips/official/92.md)
- [NIP-94](../nips/official/94.md)
- [NIP-68](../nips/official/68.md)
- [NIP-71](../nips/official/71.md)
- [NIP-A0](../nips/official/A0.md)
- [NIP-B7](../nips/official/B7.md)
- [NIP-96](../nips/official/96.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
