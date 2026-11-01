# 100. Pylons and the mesh

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Psionic's Pylons are the largest consumer of this stack. This chapter traces
one decision request through it.

Part XII, Nostr in OpenAgents. Result: Pylons authenticate to the relay with
NIP-42, announce beacons, and answer sealed NIP-44 decision requests; the mesh
opens encrypted channels with this book's primitives.

## What this chapter teaches

- Connect, AUTH and announce a beacon.
- Receive a NIP-44-sealed request, decrypt, answer, seal the result.
- What is verified (signatures, provider binding, freshness) and what is only
  advertised (hardware, capacity).

## What exists today

- `psionic/lib/pylon/run.coax` and `psionic/lib/mesh/opening.coax`; [Psionic
  chapter 24](../../psionic/book/ch024-a-pylon-answers-decisions.md) runs it.

## Dependencies

- [82. Pylons, environments and
  deploys](ch082-pylons-environments-and-deploys.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [38. NIP-42 authentication](ch038-nip-42-authentication.md)
- [Psionic chapter 24, A Pylon answers
  decisions](../../psionic/book/ch024-a-pylon-answers-decisions.md)

## Done means

- A Pylon serves a self-test decision through the Coax relay.
- A request sealed to another provider is not decrypted.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-PYLON](../nips/openagents/NIP-PYLON.md)
- [NIP-44](../nips/official/44.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
