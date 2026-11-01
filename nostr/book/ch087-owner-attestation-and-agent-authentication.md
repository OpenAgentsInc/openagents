# 87. Owner attestation and agent authentication

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Block's Buzz NIPs let an owner vouch for an agent. This chapter keeps the
exact signed conditions and checks them against current owner roles.

Part X, Every NIP. Result: Block's NIP-OA `auth` tag by which an owner key
authorizes an agent key, and NIP-AA agent authentication to relays with
virtual member privileges and revocation.

## What this chapter teaches

- OA: the tag, its conditions string kept exactly as signed, relay and client
  behavior.
- AA: protocol flow, the relay verification algorithm, virtual member
  privileges and revocation.
- Key possession is separate from community membership.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [38. NIP-42 authentication](ch038-nip-42-authentication.md)

## Done means

- An attestation with altered conditions refuses.
- A revoked agent loses its virtual membership on the next AUTH.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [Block NIP-OA](../nips/block/NIP-OA.md)
- [Block NIP-AA](../nips/block/NIP-AA.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
