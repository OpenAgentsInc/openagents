# Portable session contract

`@openagentsinc/portable-session-contract` is the versioned, public-safe
contract and bounded model boundary for remote-first coding sessions.

It defines host-independent session identity, canonical nested agent topology,
generation-fenced attachments, content-addressed secret-free checkpoints,
provider-neutral targets, capability-lease references, portable commands, and
real-host acceptance journeys. It does not implement persistence, placement,
broker redemption, target adapters, or UI. Those begin in PORT-01 and PORT-02.

The package deliberately has no field for a host path, provider-native session
ID, process ID, socket, credential, auth home, or raw secret. Decoding proves
shape; `auditPortableSessionSnapshot` proves cross-record invariants.
