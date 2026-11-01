# 32. Many relays and the outbox model

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Nostr has no central server. The outbox model says where to look for a
person's events and where to send events that mention them. This chapter
builds relay selection from NIP-65 lists.

Part VIII, A client. Result: NIP-65 relay lists (kind 10002 with `r` tags
marked `read` or `write`), reading from an author's write relays, writing to
your write relays and the tagged users' read relays.

## What this chapter teaches

- Kind 10002, replaceable, with `["r", url, "read"|"write"]` tags; no marker
  means both.
- Reading: fetch an author's events from their write relays.
- Writing: send to your write relays and to each tagged person's read relays.
- Bounds: a maximum number of relays per author and per request.
- Merging results from many relays with deduplication by ID.

## What exists today

- Nothing in Coax yet. Pylon planning includes publishing selected relays as a
  NIP-65 list (`psionic/lib/pylon/`).

## Dependencies

- [30. Subscriptions and EOSE](ch030-subscriptions-and-eose.md)
- [31. Publish and wait for OK](ch031-publish-and-wait-for-ok.md)

## Done means

- Selection for a fixture of 50 authors matches a hand-computed table.
- Events from several relays merge to one copy per ID.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-65](../nips/official/65.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
