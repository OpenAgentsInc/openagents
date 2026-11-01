# 75. Hosts and reachability

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

OpenAgents devices reach the hosts that run their work. NIP-HOST says who may
use a host; NIP-REACH says how to reach it. This chapter builds both on the
shared contracts.

Part X, Every NIP. Result: NIP-HOST invitations, enrollment, grants with
epochs and revocation bound to the original host-signed envelope, and
NIP-REACH host directories, presence, hints and direct channels.

## What this chapter teaches

- Grants: verify the original envelope, issuer and audience, operation scope,
  expiry, epoch and revocation before any effect.
- Enrollment over iroh and reverse enrollment for a headless host.
- REACH: owner host directory, presence, route generations and rollback
  freshness.
- Network membership never grants execution.

## What exists today

- `coax/lib/nostr/host/grant.coax` binds a projected grant to its original
  host-signed, device-encrypted envelope. REACH is not implemented.

## Dependencies

- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [49. HTTP auth](ch049-http-auth.md)

## Done means

- A revoked, expired or replayed grant refuses.
- A stale route generation is not used.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [OpenAgents NIP-HOST](../nips/openagents/NIP-HOST.md)
- [OpenAgents NIP-REACH](../nips/openagents/NIP-REACH.md)
- [OpenAgents shared contracts](../nips/openagents/contracts.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
