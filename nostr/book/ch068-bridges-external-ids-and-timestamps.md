# 68. Bridges, external IDs and timestamps

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Some events point outside Nostr. This chapter covers the tags that say where.

Part X, Every NIP. Result: NIP-48 `proxy` tags for bridged events, NIP-73 `i`
and `k` tags for external content IDs (URLs, ISBNs, podcasts, geohashes), and
NIP-03 kind 1040 OpenTimestamps attestations.

## What this chapter teaches

- NIP-48: `["proxy", id, protocol]` for `activitypub`, `atproto`, `rss`,
  `web`.
- NIP-73: normalized identifiers per type, such as `isbn:` and
  `podcast:guid:`.
- NIP-03: kind 1040 with an `e` tag and base64 `.ots` content; the pinned text
  marks it `unrecommended` and vulnerable to one specific attack.
- Verifying an OpenTimestamps proof requires Bitcoin block headers; this
  chapter states what is and is not checked.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [4. SHA-256](ch004-sha-256.md)

## Done means

- NIP-73 examples normalize to their stated forms.
- An OTS proof that does not commit to the event ID refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-48](../nips/official/48.md)
- [NIP-73](../nips/official/73.md)
- [NIP-03](../nips/official/03.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
