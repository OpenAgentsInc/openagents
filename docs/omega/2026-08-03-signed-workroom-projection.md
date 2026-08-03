# Signed Workroom projection boundary

The generated All Work contract now carries signed Workroom activity, durable
outbox records, a ledger, and read/enqueue requests and receipts. Activity
preserves signer, Nostr event identity and signature, actor, optional actor
grant and generation, Workroom and Work, audience/privacy, causal parents,
generation, and supersession or revocation.

New events use signing profile `openagents.signed-workroom.v2`. It serializes a NIP-01
event with the signer public key, whole-second occurrence time, registered
activity kind, empty content, and deterministic tags for the projection
profile, event, actor, Workroom, optional Work, audience, privacy, revision,
generation, exact occurrence timestamp, payload digest, causal parents,
evidence, optional actor grant and grant generation, supersession, and
revocation. The authority recomputes the SHA-256
event ID and verifies its BIP-340 Schnorr signature.

| Activity kind    | Nostr kind |
| ---------------- | ---------: |
| membership       |      32150 |
| thread           |      32151 |
| mention          |      32152 |
| assignment       |      32153 |
| delegation       |      32154 |
| agent_session    |      32155 |
| agent_activity   |      32156 |
| code_change      |      32157 |
| review           |      32158 |
| decision         |      32159 |
| evidence         |      32160 |
| verification_ref |      32161 |
| receipt_ref      |      32162 |
| revocation       |      32163 |

This contiguous OpenAgents-owned range is versioned by the projection profile.
A semantic change requires a new profile and registry update; it cannot reuse a
kind with different bytes. Custom kinds are projection transport, not command
or product authority.

Direct admission requires the actor to be
`principal:nostr:<signer-pubkey>` and forbids an attached actor grant.
Organizational, device, and agent actors require an authoritative active
`purpose:signed-workroom:project-activity` grant. The grant binds its issuer,
actor, signer key, exact Workroom and Work, permitted event kinds,
audience/privacy classes, evidence refs, validity interval, and generation.
Admission fails closed when the resolver is absent or the grant is missing,
expired, revoked, superseded, stale, or scope-mismatched. Both occurrence and
admission time must be inside the grant interval; publication rechecks the
current grant before opening a socket. The authority also requires the named
Effective Principal and enqueue capability, a closed audience/privacy match,
known causal parents, advancing generations, and unique event/idempotency
identities. It persists the exact signed projection and relay targets in a
pending canonical outbox before publication.

Every decoded durable state is revalidated before read-side use or mutation.
The ledger revision must reconcile with unique canonical event identities,
every stored activity and outbox record must pass event-ID/signature/actor
verification, and each outbox copy must match its canonical activity exactly.
Schema-valid but cryptographically invalid historical state fails closed.
The durable reader adds the legacy v1 profile and null grant binding to older
direct-actor rows without changing their signed bytes. New admission requires
v2; v1 remains read/replay compatibility only.

The receipt fixes `persistedBeforePublish: true`,
`relayAcceptanceIsAuthority: false`, and `admittedEffect: false`. A signature
proves signer and the serialized projection bytes, including the payload
digest. It does not prove the undisclosed payload content. Relay acceptance is
transport evidence. Neither admits a command, external effect, verification,
owner acceptance, or release.

The owner-local durable file adapter uses atomic replacement and optimistic
revision checks, so restart and stale writers cannot silently drop pending
outbox rows. The typed delivery reducer accepts unique attempts only for the
relay targets already persisted in that outbox row. It records accepted,
rejected, and unreachable results, preserves the exact accepted-relay subset,
retains the bounded per-relay attempt history, keeps partial or failed delivery
retryable, and advances the same optimistic ledger revision without changing
the signed activity bytes. The durable reader migrates pre-delivery pending
rows by adding an empty attempt history before strict validation. Delivery
requests are idempotent. Their receipts fix
`relayAcceptanceIsAuthority: false` and `admittedEffect: false` even when all
configured relays accept the event.

The network publisher consumes only already-persisted outbox rows. It opens
bounded WSS connections to the exact targets stored on the row, publishes the
exact verified NIP-01 event, accepts only a matching relay `OK` frame, and feeds
accepted, rejected, or unreachable attempts into the revision-safe reducer.
It retries only targets without a recorded acceptance. Invalid targets and
missing WebSocket support become typed unreachable attempts without starting a
network effect. Relay-provided text is not persisted, so an untrusted relay
cannot inject content or sensitive data into the canonical ledger.

The publisher is an Effect service with an injectable socket constructor. The
live layer uses the runtime WebSocket implementation. A caller must still
provide the exact activity actor and delivery capability before any socket is
opened. Relay acceptance remains transport evidence only, and a revision race
after publication fails closed so a later retry can reconcile the durable row.

The remaining OAW-009 work includes Omega grant provisioning, delivery and
enqueue UI, and installed two-client outage/replay
falsifiers. Test execution is deferred to the final omega#208 build gate.
