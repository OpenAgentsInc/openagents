# 64. App data and handlers

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Apps store their own data and advertise what they handle. This chapter covers
those four NIPs.

Part X, Every NIP. Result: NIP-78 kind 30078 application data, NIP-89 handler
recommendations (31989) and announcements (31990), NIP-5A static websites
(35128 and 15128 manifests), and NIP-CD slash commands (31992, 10992).

## What this chapter teaches

- NIP-78: kind 30078 with any `d`; content defined by the app.
- NIP-89: kind 31990 lists kinds an app handles with URL templates; kind 31989
  recommends a handler; the `client` tag.
- NIP-5A: site manifests mapping paths to SHA-256 hashes on Blossom.
- NIP-CD: slash command definitions with typed arguments, installation lists,
  and the executor's response rules.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Done means

- A NIP-5A file whose hash differs from the manifest is refused.
- Handler lookup for a kind returns announcements from the user's follows
  first.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-78](../nips/official/78.md)
- [NIP-89](../nips/official/89.md)
- [NIP-5A](../nips/official/5A.md)
- [NIP-CD](../nips/official/CD.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
