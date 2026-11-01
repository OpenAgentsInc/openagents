# 35. Store events

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A relay must keep what it acknowledged. This chapter builds the store:
in-memory decisions, an append-only file, replacement rules and recovery after
a crash.

Part IX, A relay. Result: Verified originals with one copy per ID, replaceable
and addressable replacement with the same-second lowest-ID rule, an fsync
before `OK`, and crash recovery that cuts only a partial last write.

## What this chapter teaches

- Admission of verified events and deduplication by ID.
- Replacement for kinds 0, 3, 10000–19999 and 30000–39999 by `d` tag.
- Durable append with fsync; a new file's directory is synced.
- Recovery: cut a partial last record; refuse to start on any other damage.
- What is not solved: rollback to an older copy of the whole file (#966b).

## What exists today

- The `store_*.coax` files listed above.
- Records:
  [2026-10-29-nostr-store-r1.md](../../docs/verification/2026-10-29-nostr-store-r1.md),
  [2026-10-30-nostr-store-recovery.md](../../docs/verification/2026-10-30-nostr-store-recovery.md).

## Dependencies

- [34. Admit an event](ch034-admit-an-event.md)
- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Done means

- A SIGKILL at any write leaves a store that restarts with every acknowledged
  event.
- Replacement order matches the NIP-01 rules on tied timestamps.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-01](../nips/official/01.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
