# 24. Relay messages

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Clients and relays exchange JSON arrays over a WebSocket. This chapter reads
and writes every NIP-01 frame and the machine-readable reason prefixes.

Part VI, The event model. Result: Client frames `EVENT`, `REQ`, `CLOSE`,
`AUTH` and `COUNT`, relay frames `EVENT`, `OK`, `EOSE`, `CLOSED`, `NOTICE` and
`AUTH`, and the `OK`/`CLOSED` reason prefixes NIP-20 introduced.

## What this chapter teaches

- Frame shapes, with spans into the original frame.
- `OK` with `true`/`false` and a message; prefixes `duplicate:`, `pow:`,
  `blocked:`, `rate-limited:`, `invalid:`, `restricted:`, `mute:`, `error:`,
  `auth-required:`.
- `CLOSED` for a refused or ended subscription.
- A parsed `EVENT` frame carries an unverified event.

## What exists today

- `coax/lib/nostr/message.coax` reads relay frames and writes client frames;
  `message_server.coax` is the relay's side; `message_agree.coax` checks they
  agree.
- Record:
  [2026-10-30-nostr-messages.md](../../docs/verification/2026-10-30-nostr-messages.md).

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [23. Filters](ch023-filters.md)

## Done means

- Every frame written by one side is read back exactly by the other.
- Captured relay sessions parse without loss.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-01](../nips/official/01.md)
- [NIP-20](../nips/official/20.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
