# 31. Publish and wait for OK

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Sending an event is not the same as storing it. This chapter builds publishing
that tracks each relay's `OK` and states what the client knows when no answer
comes.

Part VIII, A client. Result: Publishing with acknowledgement deadlines, the
distinction between published, acknowledged, refused and unknown, and requeue
after AUTH.

## What this chapter teaches

- Pending publishes with a deadline per relay.
- `OK true`, `OK false` with a prefix, and timeout as unknown.
- `auth-required:` requeues once after successful AUTH.
- A duplicate (`duplicate:`) counts as stored.

## What exists today

- `coax/lib/nostr/relay.coax` holds publishes and deadlines.

## Dependencies

- [29. A relay connection](ch029-a-relay-connection.md)
- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)

## Done means

- Each outcome is reached in a scripted session.
- A publish is never reported as stored without an `OK true` or `duplicate:`.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-01](../nips/official/01.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
