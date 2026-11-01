# 29. A relay connection

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A client keeps a connection to each relay it uses. This chapter builds the
state machine that decides what to do when a connection opens, drops or
misbehaves. It does no I/O itself: the host moves bytes and passes the time.

Part VIII, A client. Result: The pure client state machine: OPEN, CONNECTING,
CLOSED and BACKOFF, connection generations that make stale reports harmless,
and capped jittered backoff.

## What this chapter teaches

- States and transitions.
- Generations: anything reported for an older connection is ignored.
- Backoff with a cap and host-supplied jitter.
- The NIP-42 hook: an `AUTH` challenge is passed to chapter 38's code.

## What exists today

- `coax/lib/nostr/relay.coax`.
- Record:
  [2026-10-29-nostr-relay-client.md](../../docs/verification/2026-10-29-nostr-relay-client.md).

## Dependencies

- [24. Relay messages](ch024-relay-messages.md)
- [28. Sockets, TLS and relay URLs](ch028-sockets-tls-and-relay-urls.md)

## Done means

- A scripted fixture drops, refuses and stalls the relay, and the machine
  reaches the expected state each time.
- Zero mutation survivors in `relay.coax`.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-01](../nips/official/01.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
