# Signed Workroom projection boundary

The generated All Work contract now carries signed Workroom activity, durable
outbox records, a ledger, and read/enqueue requests and receipts. Activity
preserves signer, Nostr event identity and signature, actor, Workroom and Work,
audience/privacy, causal parents, generation, and supersession or revocation.

The signing profile is `openagents.signed-workroom.v1`. It serializes a NIP-01
event with the signer public key, whole-second occurrence time, registered
activity kind, empty content, and deterministic tags for the projection
profile, event, actor, Workroom, optional Work, audience, privacy, revision,
generation, exact occurrence timestamp, payload digest, causal parents,
evidence, supersession, and revocation. The authority recomputes the SHA-256
event ID and verifies its BIP-340 Schnorr signature.

| Activity kind | Nostr kind |
| --- | ---: |
| membership | 32150 |
| thread | 32151 |
| mention | 32152 |
| assignment | 32153 |
| delegation | 32154 |
| agent_session | 32155 |
| agent_activity | 32156 |
| code_change | 32157 |
| review | 32158 |
| decision | 32159 |
| evidence | 32160 |
| verification_ref | 32161 |
| receipt_ref | 32162 |
| revocation | 32163 |

This contiguous OpenAgents-owned range is versioned by the projection profile.
A semantic change requires a new profile and registry update; it cannot reuse a
kind with different bytes. Custom kinds are projection transport, not command
or product authority.

Direct admission requires the actor to be
`principal:nostr:<signer-pubkey>`. Organizational, device, and agent actors are
not inferred from a signature; they remain refused until a purpose-bound grant
adapter can prove the relationship. The authority also requires the named
Effective Principal and enqueue capability, a closed audience/privacy match,
known causal parents, advancing generations, and unique event/idempotency
identities. It persists the exact signed projection and relay targets in a
pending canonical outbox before publication.

The receipt fixes `persistedBeforePublish: true`,
`relayAcceptanceIsAuthority: false`, and `admittedEffect: false`. A signature
proves signer and the serialized projection bytes, including the payload
digest. It does not prove the undisclosed payload content. Relay acceptance is
transport evidence. Neither admits a command, external effect, verification,
owner acceptance, or release.

The owner-local durable file adapter uses atomic replacement and optimistic
revision checks, so restart and stale writers cannot silently drop pending
outbox rows. The remaining OAW-009 work includes the publisher adapter,
multi-relay delivery reducer, purpose-bound non-human actor grants, Omega
enqueue UI, and installed two-client outage/replay falsifiers. Test execution
is deferred to the final omega#208 build gate.
