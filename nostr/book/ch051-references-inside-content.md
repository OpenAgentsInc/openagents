# 51. References inside content

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Content can mention people and events inline. This chapter parses those
references for display without trusting them.

Part X, Every NIP. Result: NIP-27 `nostr:` references in content, the
deprecated NIP-08 `#[index]` mentions, and NIP-30 `:shortcode:` custom emoji
with `emoji` tags and kind 30030 sets.

## What this chapter teaches

- NIP-27: find `nostr:npub…`, `nostr:nevent…` and others in content; optional
  matching tags.
- NIP-08: `#[0]` refers to the first tag; read only.
- NIP-30: `["emoji", shortcode, url]`, shortcodes of letters, digits and
  underscores; kind 30030 emoji sets.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [16. NIP-19 entities and nostr:
  URIs](ch016-nip-19-entities-and-nostr-uris.md)
- [50. Notes, threads and subjects](ch050-notes-threads-and-subjects.md)

## Done means

- Every reference in the NIP-27 examples is found with its byte range.
- A shortcode with an invalid character is not replaced.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-27](../nips/official/27.md)
- [NIP-08](../nips/official/08.md)
- [NIP-30](../nips/official/30.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
