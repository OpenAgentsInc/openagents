# Portable contracts and transaction lifecycle

Status: proposed v1 wire slice. [JSON schemas](contracts/README.md) and
[examples](examples/README.md) define the initial portable envelopes. They do
not replace semantic validation or declare the entire engine implemented.

## Representation and identity

Wire documents use UTF-8 JSON with unique property names, finite numbers, explicit
schema names, and bounded collections. Reject duplicate keys, nonfinite values,
unknown required versions, and unknown envelope fields before mutation. A decoder
must impose nesting and size limits before allocating a complete document.

Project, branch, command, entity, and semantic structure IDs use UUIDs in lowercase
canonical text. Names are presentation fields. Artifact/release/world identities
use SHA-256 with `sha256:` followed by 64 lowercase hexadecimal digits. IDs in
examples are synthetic and carry no grants.

World document and release body digests cover their RFC 8785 JSON Canonicalization
Scheme (JCS) bytes. Array order is meaningful unless a domain rule explicitly
sorts a set by stable ID before serialization. Canonicalization does not repair
invalid topology, normalize a Unicode name silently, or permit unsafe numeric
precision. Raw binary artifact digests cover the exact uncompressed logical blob
bytes; transfer encoding has a separate length/digest when needed. A build-cache
key is not an output digest. [JCS](https://www.rfc-editor.org/rfc/rfc8785) defines
the serialization algorithm; semantic normalization belongs to the schema version.

Structural coordinates are integer millimeters in building-local space. Exact
decimal measurements are parsed before conversion. Sub-millimeter input needs an
explicit snapping policy or fails. A world transform uses meters, normalized
quaternion rotation, and allowed scale; building structure permits yaw and
positive uniform scale. Rendering floats are derived data. See [world model](world-model.md).

## Command envelope

Every manual, semantic, and agent mutation carries:

| Field | Meaning |
| --- | --- |
| `schema` | `aethyrforge.command.v1` |
| `commandId` | Stable UUID idempotency identity for this exact request |
| `projectId`, `branchId` | Explicit destination |
| `baseRevision` | Exact accepted branch revision against which this request was resolved |
| `actorId` | Claimed principal, checked against authenticated transport identity |
| `capabilityRef` | Non-secret grant reference; server verifies current scope and revocation |
| `operations` | One to eight registered typed operations in order |
| `preconditions` | Object version checks and operation-specific semantic dependencies |
| `context` | Captured selection IDs/revision and view revision, never an instruction to retarget |

The instant profile permits 64 KiB encoded command data, eight operations, and
32 distinct explicit mutation targets across the transaction, counting newly
allocated building/entity IDs and addressed existing IDs. Structural records
generated deterministically by accepted building/room operations are separately
bounded by the topology profile. Their IDs and remapping are persisted in the
authoring commit; compilation reuses them and cannot mint new source identities. A spawn
operation allocates explicit new IDs. Larger imports, generation, and mass edits produce
reviewable jobs with a declared resource budget and a final validated commit.
They do not bypass the instant profile by recursively nesting batches.

The initial schema includes `building.place`, `entity.setTransform`,
`entity.remove`, `room.add`, `wall.setPolicy`, and `opening.add`. Each has closed
`args` and explicit `targetIds`. A registerable operation family adds its schema,
semantic validator, inverse intent, required capabilities, resource cost, and
conformance fixtures. Unknown operations fail closed. Raw arbitrary JSON Patch,
JavaScript evaluation, filesystem writes, or model-defined opcodes are not world
mutation interfaces.

Relative requests are resolved against captured context into explicit semantic
IDs and dimensions before submission. The service cannot reinterpret “left”
against another camera after a conflict. The client rejects a result after its selection changes; continued walking alone
does not change the captured view. The authority checks document/object versions
and supplied dependencies, not a private live camera. A changed dependency returns
a structured stale-context result. A creator can submit a newly resolved command.

## Commit protocol

1. Decode with strict wire bounds and authenticate the caller.
2. Check current permission to read command results before looking up command
   identity scoped to project/branch/principal. The same canonical request returns
   its recorded result without execution; a different request with that ID conflicts.
3. Verify membership, grant scope, expiration, and current revocation independently
   of fields in the request or output from an AI model.
4. Compare `baseRevision` and all preconditions with the branch head.
5. Apply operations to a staged document. Validate topology, references, source
   dependencies, rights/capability requirements, and resource profile as a whole.
6. Durably commit the new document digest, incremented revision, request result,
   and outbox event together. No partial operation success is exposed.
7. Return `committed` with `revision`, `worldDigest`, changed IDs, and build request
   identity. Separately report compilation and runtime activation progress.

Document validity is independent of every player's live position. Local tools can
preflight a known occupied edit and offer move/pause before submission. A remote
commit or later player movement can still make an otherwise valid document unsafe
to activate in a preview. That condition holds activation; it cannot roll back a
shared branch's accepted revision.

Before remote acknowledgment a preview is explicitly optimistic. **Saved on this
device**, **Synced**, **Preparing**, **Live**, and **Published** are different
states. A dropped response is resolved by a command-status lookup or an identical
retry; it does not authorize a fresh duplicate mutation. Idempotency results must
be retained in full for at least 30 days. Keep compact command-ID/request-digest/
outcome/revision tombstones for the project lifetime. A known compacted result
returns its revision/history pointer and never re-executes. An actually unknown
UUID is a new command; time cannot be inferred from an arbitrary client ID.
Deleting a project ends its grants and ID namespace; project IDs are never reused.

## Compilation and activation

A generation identifies `generationId`, `worldDigest`, `targetProfile`,
`engineAbi`, compiler identity, and all required artifact digests. It shares
unchanged immutable artifacts rather than copying the world per edit. Completeness
includes rendering, collision, navigation, and selection outputs for changed
dependencies and their declared seams.

The runtime tracks desired, candidate, and active identities. Candidate completion
does not establish freshness. At installation it checks the current desired
generation, dependencies, ABI, memory budget, and migration policy. Resource
layouts and script state are validated on copies. The host switches the complete
compatible generation at a coordinated tick/frame boundary. No frame may show a
removed wall whose old collider still blocks the player.

Failures preserve active state. Obsolete candidates release only their own
resources. A failed newer edit prevents an older late completion from becoming
the desired result. Recover by editing again or explicitly selecting the earlier
revision. Device loss ends GPU-object validity and follows [rendering recovery](rendering.md).

Pose migration values are `preserve`, `hold`, and `explicit-relocate`. Preserve
requires support/clearance in the new world. Hold keeps the old preview and an
actionable diagnostic until safe or canceled. Explicit relocation chooses a
validated spawn/nearest-safe point under a user/session policy; it is recorded.
The engine never promises unchanged position for physically incompatible worlds.

## Errors and undo

Structured failures include a stable code, command or build identity, relevant
semantic IDs/source spans, a short user message, and bounded technical detail.
Initial families include `invalid_command`, `unauthorized`, `stale_revision`,
`stale_target`, `invalid_topology`, `resource_limit`, `missing_dependency`,
`compile_failed`, `incompatible_abi`, `migration_required`, and `device_lost`.
Sensitive inputs, credentials, private prompts, and raw native stacks are excluded
from public messages and shared diagnostics.

Undo submits a compensating command against current state, with new command ID
and preconditions. It does not decrement branch revision, resurrect revoked
grants, rewind other collaborators, or reset simulation implicitly. Reverting
source and restoring a gameplay checkpoint are separate operations.

## Releases and signatures

`aethyrforge.release.v1` pins a project revision/world digest, complete artifact
list, entry points, engine/VM/feature requirements, and rights inventory digest.
The manifest pins exact engine/runtime build and behavior-compiler artifact
digests alongside ABI and VM compatibility versions. `compilerDigest` pins the
world-compiler build including its toolchain fingerprint; `worldSchema` and
`simulationProtocol` declare source and session compatibility. Each pinned
compiler/runtime digest must have the matching role in `artifacts`. The complete
dependency closure is listed there. `startupDigests` is a nonempty subset of that
closure sufficient to initialize every declared entry point safely; later content
can stream. `capabilities` lists registered required features, never grants.
Unknown required features fail compatibility checks. `offlinePolicy` is
`cached-trust` or `fresh-authorization`; the latter forbids offline startup.

`releaseId` is the JCS body digest, external to the body to avoid a circular hash.
`aethyrforge.channel.v1` binds a channel to a release with monotonic sequence,
previous reference digest, signer `keyId`, UTC `issuedAt`/`expiresAt`, and
`trustVersion`. Timestamps use calendar-valid RFC 3339 UTC whole seconds with a
literal `Z`; expiry must follow issue time by at most 24 hours. The control
service rejects future issue times beyond its configured clock-skew allowance.
Renewal signs a new higher-sequence reference, even when its release is unchanged.
`trustVersion` identifies the publication-control trust snapshot used for signing;
online admission still checks the current registry and revocations. It cannot
substitute an older trust snapshot supplied by a publisher. A cached offline
release is governed by its offline policy and never claims a channel is current.

`channelRefDigest` is the SHA-256 digest of the JCS channel body, excluding
detached signatures. A signature statement has exactly `schema`
(`aethyrforge.signature.v1`), `purpose` (`release` or `channel`), `projectId`,
`subjectDigest`, and `keyId`. Ed25519 signs the UTF-8 bytes of
`AETHYRFORGE-SIGNATURE-V1` followed by a single LF byte, followed by JCS statement
bytes with no trailing newline. The detached 64-byte signature uses unpadded
base64url encoding. `subjectDigest` equals the release or channel-body digest
according to `purpose`; identities must also match the verified body. The key
ID in a channel body must match its detached signature statement. The key
registry binds `keyId` to its Ed25519 public key, project roles, validity, and
current revocation. A body-supplied key never grants trust.

Signature verification does not replace permission, freshness, completeness, or
compatibility checks. Full admission,
key rotation, anti-rollback, and active-session policy live in [publishing](publishing.md).

## Compatibility and schema scope

Top-level wire versions and registered component versions are explicit. Unknown
required components block compilation; designated optional components can be
preserved without interpretation under bounded extension rules. A migration
produces a new revision and report, never overwrites source bytes silently.

The initial world example is a bounded structural fixture. General materials,
terrain, animation graphs, audio graphs, UI, gameplay, and vertical connections
must receive registered component schemas and cross-language conformance cases
before their delivery gate. The docs specify those capabilities; the initial JSON
slice does not claim to serialize all of them yet.
