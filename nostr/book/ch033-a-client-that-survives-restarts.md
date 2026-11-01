# 33. A client that survives restarts

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

A real client goes offline, restarts and reconnects. This chapter builds the
client-side store and outbox so that none of those loses or duplicates an
event.

Part VIII, A client. Result: A durable client store of verified originals, an
offline outbox that sends each event once after a restart, and deletions that
are never resurrected on reconnect.

## What this chapter teaches

- Store verified original bytes with their ID; never re-serialize.
- The outbox: queued, sent, acknowledged per relay; replay after restart
  without duplicate effects.
- Deletions (NIP-09) recorded as tombstones and honored on every later fetch.
- Partitions: two relays that disagree are both kept, with the disagreement
  visible.

## What exists today

- The relay's store slices (chapter 35) exist; no client store exists.

## Dependencies

- [32. Many relays and the outbox
  model](ch032-many-relays-and-the-outbox-model.md)
- [31. Publish and wait for OK](ch031-publish-and-wait-for-ok.md)

## Done means

- A crash at each step of a publish leaves exactly one copy delivered after
  restart.
- A deleted event does not return after reconnecting to a relay that still has
  it.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-01](../nips/official/01.md)
- [NIP-09](../nips/official/09.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
