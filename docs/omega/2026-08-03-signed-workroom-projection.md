# Signed Workroom projection boundary

The generated All Work contract now carries signed Workroom activity, durable
outbox records, a ledger, and read/enqueue requests and receipts. Activity
preserves signer, Nostr event identity and signature, actor, Workroom and Work,
audience/privacy, causal parents, generation, and supersession or revocation.

The authority requires the Effective Principal and enqueue capability, a
closed audience/privacy match, known causal parents, advancing generations,
and unique event/idempotency identities. It persists the exact signed activity
and relay targets in a pending canonical outbox before publication.

The receipt fixes `persistedBeforePublish: true`,
`relayAcceptanceIsAuthority: false`, and `admittedEffect: false`. A signature
proves signer and bytes. Relay acceptance is transport evidence. Neither admits
a command, external effect, verification, owner acceptance, or release.

The owner-local durable file adapter uses atomic replacement and optimistic
revision checks, so restart and stale writers cannot silently drop pending
outbox rows. The remaining OAW-009 work includes the publisher adapter,
multi-relay delivery reducer, signature-verification adapter, generated
omega-effectd methods, Omega consumer, and installed two-client falsifiers.
Test execution is deferred to the final omega#208 build gate.
