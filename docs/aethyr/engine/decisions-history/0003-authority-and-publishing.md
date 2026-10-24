> Deprecated (owner, 2026-10-24): history only. Architecture decisions live in [`docs/adr/`](../../../adr/README.md).

# 0003: Separate authored revisions, play sessions, and releases

Status: selected, 2026-10-22.

## Decision

Validated structural commands commit against one serialized branch head. CRDTs
support designated source text/comments, with exact frontiers frozen for builds.
Offline authoring becomes a branch with explicit merge. Multiplayer play uses a
separate simulation authority pinned to a release.

Publishing admits immutable complete manifests and moves a signed sequenced
channel reference using expected-previous compare-and-swap. Routine game updates
upload changed artifacts. The stable host/engine does not rebuild or restart
merely because a creator adds a room.

## Rationale and consequences

CRDT convergence alone does not preserve geometric validity or permissions.
Last-writer-wins topology hides interacting edits. A container/image per content
change adds unnecessary build/deployment work. A mutable URL pointing at a mix of
new and old assets prevents reliable reproduction. Replacing a running session's
game version implicitly can invalidate state and connected clients.

The selected design requires persistent idempotency, conflict UI, rights checks,
cache identities, key rotation, freshness protection, and explicit session
migrations. Rollback selects a prior package; it does not undo irreversible
external effects or magically restore an old gameplay checkpoint.

## Revisit

New collaborative data structures or distribution transports are acceptable only
if they preserve transaction validity, release identity, revocation, and session
compatibility. Transport interoperability does not justify duplicate authority.
See [collaboration](../collaboration.md) and [publishing](../publishing.md).
