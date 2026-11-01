# 42. Protected events

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

NIP-70 lets an author say an event belongs only on relays they chose. This
chapter enforces it with NIP-42.

Part IX, A relay. Result: Events tagged `["-"]` accepted only from their
authenticated author, so a third party cannot republish them to this relay.

## What this chapter teaches

- The tag: `["-"]`.
- The relay refuses a protected event unless the connection authenticated as
  its `pubkey`.
- The refusal: `auth-required:` first, then `restricted:`.

## What exists today

- The Coax relay has AUTH plumbing but not this policy.

## Dependencies

- [38. NIP-42 authentication](ch038-nip-42-authentication.md)

## Done means

- A protected event from an unauthenticated connection refuses; after AUTH as
  the author it is accepted.
- A protected event from another authenticated key refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-70](../nips/official/70.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
