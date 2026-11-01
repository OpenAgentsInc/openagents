# 92. Private agent state

> **Status: unimplemented — design draft.** This chapter is written ahead of
> its code. It has no source-linked listings or examples.

Agents keep private memory, configuration and metrics. These NIPs encrypt them
and set their lifetimes. This chapter builds them without leaking plaintext
into public stores or logs.

Part X, Every NIP. Result: NIP-AE engrams (30174), NIP-AP personas
(30175–30178), NIP-AO ephemeral observation (24200), NIP-AM turn metrics
(44200) and the NIP-PMA private managed-agent aggregate reservation.

## What this chapter teaches

- AE: record types, slugs, addressing, encrypted bodies and tombstones.
- AP: persona configuration and replacement semantics.
- AO: ephemeral frames that are never persisted as history.
- AM: exact usage and pricing identity.
- PMA: a reservation subject to its privacy and transactional gates; not
  advertised.

## What exists today

- Nothing in Coax yet.

## Dependencies

- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [87. Owner attestation and agent
  authentication](ch087-owner-attestation-and-agent-authentication.md)

## Done means

- No AO frame is written to durable storage.
- Decrypted bodies never appear in public snapshots or telemetry, checked by a
  canary.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [Block NIP-AE](../nips/block/NIP-AE.md)
- [Block NIP-AP](../nips/block/NIP-AP.md)
- [Block NIP-AO](../nips/block/NIP-AO.md)
- [Block NIP-AM](../nips/block/NIP-AM.md)
- [Block NIP-PMA](../nips/block/NIP-PMA.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
