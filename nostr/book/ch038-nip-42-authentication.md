# 38. NIP-42 authentication

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Some relays require a client to prove it holds a key. This chapter builds
NIP-42 on both sides and states what a successful AUTH does and does not
grant.

Part IX, A relay. Result: Challenges bound to one connection and one relay
URL, kind 22242 events with `relay` and `challenge` tags checked within a time
window, and AUTH on both the client and the relay.

## What this chapter teaches

- The relay sends `["AUTH", challenge]`; a new challenge replaces the old one.
- The client signs kind 22242 with `["relay", url]` and `["challenge", c]`.
- Relay checks: kind, challenge, canonical URL, `created_at` within about ten
  minutes, signature.
- Answers to an old challenge, another relay or another connection generation
  refuse.
- AUTH proves possession of a key; it does not grant membership or any other
  right.

## What exists today

- `coax/lib/nostr/nip42.coax` and `nip42-event.coax`.
- Record:
  [2026-10-29-nostr-nip42.md](../../docs/verification/2026-10-29-nostr-nip42.md).

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [29. A relay connection](ch029-a-relay-connection.md)
- [34. Admit an event](ch034-admit-an-event.md)

## Done means

- Each refusal case above has a test.
- The Psionic chapter 24 Pylon authenticates to the Coax relay over the
  tailnet (52/52 sessions).
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-42](../nips/official/42.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
