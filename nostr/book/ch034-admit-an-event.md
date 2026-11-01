# 34. Admit an event

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A relay decides which events it accepts. This chapter builds the admission
path of the Coax relay, with the limits of the Rust relay it replaces.

Part IX, A relay. Result: The relay's EVENT admission: verify ID and
signature, check size, tag and time limits, answer `OK` with the standard
prefixes, and route each frame from one connection.

## What this chapter teaches

- Order: frame size, JSON shape, ID, signature, policy, store.
- Limits: frames 131,072 bytes, 256 tags, `created_at` at most 900 s ahead.
- Reasons: `invalid:`, `blocked:`, `rate-limited:`, `duplicate:`, `error:`.
- `OK true` only after the store has committed the event.

## What exists today

- `coax/lib/nostr/relay_server.coax`, `store_admit.coax`, `relay_route.coax`
  and the host in `coax/relay/`.
- Records:
  [2026-10-29-coax-relay-r1.md](../../docs/verification/2026-10-29-coax-relay-r1.md),
  [2026-10-30-coax-relay-r1-tls.md](../../docs/verification/2026-10-30-coax-relay-r1-tls.md).

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [24. Relay messages](ch024-relay-messages.md)
- [27. A WebSocket server](ch027-a-websocket-server.md)
- [Coax chapter 24, A complete
  boundary](../../coax/book/ch24-a-complete-boundary.md)

## Done means

- Forged and malformed events refuse with the expected reason.
- The differential sessions against the Rust relay match or have a recorded
  explanation.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-01](../nips/official/01.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
