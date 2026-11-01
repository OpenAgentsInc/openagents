# 37. NIP-11 relay information

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Clients learn a relay's limits from its NIP-11 document. This chapter serves
the document and checks that each advertised limit is the one the relay
enforces.

Part IX, A relay. Result: The relay information document served on `Accept:
application/nostr+json`: name, pubkey, `supported_nips`, software, version and
`limitation` fields that match what the relay enforces.

## What this chapter teaches

- The HTTP request on the WebSocket URL with `Accept: application/nostr+json`.
- Fields: `name`, `description`, `pubkey`, `contact`, `supported_nips`,
  `software`, `version`.
- `limitation`: `max_message_length`, `max_subscriptions`, `max_limit`,
  `max_subid_length`, `auth_required`, `created_at_upper_limit`.
- `supported_nips` lists only NIPs the relay implements.

## What exists today

- The Coax relay serves its NIP-11 document from `relay_server.coax`.

## Dependencies

- [34. Admit an event](ch034-admit-an-event.md)

## Done means

- Every advertised limit has a test that hits it.
- `supported_nips` equals the set of NIP chapters marked done for the relay.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-11](../nips/official/11.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
