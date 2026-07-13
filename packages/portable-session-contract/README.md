# Portable session contract

`@openagentsinc/portable-session-contract` is the versioned, public-safe
contract and bounded model boundary for remote-first coding sessions.

It defines host-independent session identity, canonical nested agent topology,
generation-fenced attachments, content-addressed secret-free checkpoints,
provider-neutral targets, capability-lease references, portable commands, and
real-host acceptance journeys. PORT-01 adds the durable Cloud SQL authority.
PORT-02 adds the Effect-based capability broker state machine and injected
owner-local/OpenAgents-managed target-adapter boundary. Placement, target
enrollment, host movement, and UI remain later packets.

The package deliberately has no field for a host path, provider-native session
ID, process ID, socket, credential, auth home, or raw secret. Decoding proves
shape; `auditPortableSessionSnapshot` proves cross-record invariants.

`PortableCapabilityBroker` consumes the frozen lease shape without adding a
credential field. Its records bind owner, session, attachment generation,
target, capability, optional account/tool, least-privilege permissions, and a
short TTL. Issue, redeem, renew, revoke, reissue, release, and wipe operations
are exact-replay idempotent and emit refs-only evidence. Raw material can exist
only inside the injected vault-to-target callback and is absent from snapshots,
outcomes, evidence, prompts, Sync/checkpoint projections, artifacts, and
diagnostics. Reissue requires a fresh destination source-grant ref after the
source grant is revoked and its target installation is wiped.
