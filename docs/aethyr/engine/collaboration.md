# Collaboration and project authority

Status: proposed specification. No collaboration service is implemented.

This specification separates people editing a project from people playing a
released game. Project collaboration changes authored intent. Multiplayer
simulation changes a running session. They use different authority, storage,
traffic, and recovery rules. See [architecture](architecture.md),
[contracts](contracts.md), and [runtime](runtime.md).

## Branch authority

**COL-001.** Every project branch has one serialized sequence of accepted
revisions. A Rust project service owns that sequence in hosted mode; the same
Rust validation and command core owns it in local mode, compiled to Wasm for a
browser-only workspace. PostgreSQL transactions enforce the
hosted branch-head comparison and write the command result, new revision,
document references, and event-outbox record atomically. A process lease alone
never establishes authority. Browser-only mode uses the transactional storage
adapter in [operations](operations.md); an optional native companion uses an
embedded transactional store. Both retain the same acceptance rules.

Revisions are immutable records identified by project, branch, and a monotonically
increasing branch-local integer `revision`. Each records its parent, `worldDigest`,
command identity, actor, and validator version. A merge revision also records its
source branch and revision. Revision numbers imply neither authorization nor
chronological order across branches. Objects retain their stable IDs through
moves and edits; deleting and recreating an object creates a new identity.

**COL-002.** A submitted transaction identifies its `schema`, `projectId`,
`branchId`, UUID `commandId`, integer `baseRevision`, `actorId`, `capabilityRef`,
typed `operations`, preconditions, and captured context references. The instant
command profile permits one to eight operations, at most 32 distinct explicit
targets including newly allocated entity/building IDs,
and a 64 KiB encoded command. Larger operations require the reviewed job path.
Preconditions include referenced object versions and relevant structural constraints. The
service rechecks current membership and capabilities, rejects unknown fields or
oversized payloads, validates the complete resulting document, and commits all
operations or none. A model's interpretation receives the same checks as a
button action.

After authenticating the caller and verifying current permission to read this
branch's command result,
an exact retry returns its recorded result within the full-receipt retention
window, initially 30 days. Compact tombstones containing the scoped command ID,
request digest, outcome, and resulting revision remain for the project's lifetime.
A known expired ID returns `retry_window_expired` with a history reference and
never re-executes. Reusing an ID with different request content fails, including
after its full receipt expires. An unknown ID is not presumed old merely from
its UUID. New commands separately require current write grants and all normal
validation. See [contracts](contracts.md) for the authoritative replay order.

A branch-head mismatch returns the current head
and structured conflicts; the server never silently recomputes a relative action
against a new camera or different wall. A client may explicitly resubmit an
independent change after revalidation with a new command ID. A bounded command
status lookup resolves a lost acknowledgment. See the wire rules in
[contracts](contracts.md).

## Immediate interaction without false success

Clients may show an optimistic candidate before remote acceptance. It carries a
visible pending state and remains distinct from the accepted revision and active
runtime generation. Rejection removes or rebases the candidate without discarding
the creator's input. A reconnect never promotes an optimistic document merely
because it rendered successfully.

**COL-003.** Dragging transmits replaceable presence samples while the pointer
moves and commits a final typed transaction. Presence includes cursors,
selections, camera sharing, and advisory object reservations. Branch read access
also governs presence subscriptions; private collaborator activity cannot leak
through a public player channel. Presence expires, is
rate-limited, and never persists as project authority. An advisory reservation
does not prevent authorized edits or override transaction preconditions.

Collaborative undo submits a compensating transaction against current state. It
does not rewind another creator's accepted work. If intervening edits invalidate
the inverse, show the affected objects and offer a reviewed alternative. The
history view distinguishes the originating transaction, its inverse, and the
current document.

## CRDT scope

**COL-004.** Use Loro behind a replaceable document adapter for concurrent source
text, comments, and explicitly designated metadata. A conflict-free replicated
data type (CRDT) can converge updates without proving that a house has valid
topology or that a shader compiles. Rooms, walls, ownership, references, permissions,
release channels, and ordered structural transactions therefore stay outside
unrestricted CRDT merging.

A text room is branch-scoped. Its updates are authenticated, size-bounded,
attributable at the transport boundary, and persisted before acknowledgment. A
source commit freezes an exact CRDT frontier and materialized source digest in
the project revision. Builds consume those immutable bytes, never whatever text
happens to arrive later. Draft text may contain syntax errors without corrupting
the accepted world or replacing its last working generation.

Loro supports snapshot and incremental-update export. Its shallow snapshots limit
which older updates can subsequently merge; compaction must preserve the declared
offline recovery window or force an explicit branch import. The service tests
this condition rather than assuming arbitrary old peers remain compatible.
[Loro export modes](https://loro.dev/docs/tutorial/encoding).

## Offline branches and merging

**COL-005.** Offline work creates a local branch rooted at the last accepted
remote revision. Commands, source frontiers, required blobs, and local receipts
are stored before the UI reports a local save. The UI distinguishes **Saved on
this device** from **Synced**. A browser quota failure stops new durable commits
and offers export; it does not delete unsynced work to make room for a cache.

On reconnect, upload the branch and missing blobs, check present permissions,
then propose a three-way merge using the common ancestor. Never overwrite the
remote head by replaying a disconnected command queue blindly. Disjoint scalar
edits may merge automatically after full validation. Concurrent edits to the same
field, delete-versus-edit, prefab dependency changes, and interacting topology
edits require a conflict decision. A wall removal and an attached room addition
cannot pass merely because their object IDs differ.

The merge tool previews the resulting geometry, collision, and navigation;
resolutions become one validated transaction. Long-lived offline work remains
exportable when authentication expires or access is revoked, but revocation
prevents uploading or reading further remote content. Previously downloaded
plaintext cannot be made unreadable remotely.

## Multiplayer simulation

**COL-006.** A multiplayer session pins a release and protocol version. Its Rust
simulation authority validates player inputs and advances a fixed 60 Hz tick.
Clients predict permitted motion and interpolate remote actors; the authority
corrects them. Cross-machine floating-point determinism is not assumed. Replays
use recorded authoritative snapshots and inputs with an explicit compatibility
version.

Editor presence never becomes gameplay state. A development session may consume
an accepted candidate generation only through the runtime activation protocol;
public sessions follow the [publishing migration policy](publishing.md). A player
cannot turn a gameplay action into an editor transaction without a separate
creator capability.

## Required evidence

The [verification suite](verification.md) must demonstrate duplicate submissions,
lost acknowledgments, concurrent wall edits, disconnect during a save, revoked
membership, source compaction with an old offline peer, merge conflicts, and
compensating undo after another creator edits the same object. It must verify
that every client eventually observes the accepted branch head and that failed
merges leave both source branches recoverable. Capacity and latency targets live
in [performance](performance.md); the collaboration model never promises that
every geographic location shares a zero-latency authority.
