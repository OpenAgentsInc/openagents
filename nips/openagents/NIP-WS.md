# NIP-WS — Workspace resources and synchronized views

`draft` `optional` — v1, 2026-09-26. The [shared contracts](contracts.md)
are normative. This profile binds projects, resources, documents, checkpoints,
and client views to the authority that owns their actual state. It introduces
no event kinds and describes target behavior, not a shipped workbench.

## Boundaries and existing protocols

| Existing contract | Reuse and boundary |
| --- | --- |
| Official [NIP-34](../official/34.md), Block [MP](../block/NIP-MP.md) | Repository announcements, Git collaboration, and cross-repository grouping. A repository coordinate or project membership does not identify a local checkout, authorize a file mutation, or establish its current revision. |
| Official [NIP-54](../official/54.md), [NIP-78](../official/78.md) | Published articles and application data. Neither defines conditional edits of a host document or complete operational projection replay. |
| Block [WP](../block/NIP-WP.md), [RS](../block/NIP-RS.md), [PL](../block/NIP-PL.md) | Workspace presentation, read-position synchronization, and notification delivery. Read, snooze, seen, and delivered are not execution, document version, or command acceptance states. |
| [CAP](NIP-CAP.md), [CJ](NIP-CJ.md), [POL](NIP-POL.md) | Typed host operations, authenticated invocation, exact action approvals, disclosure, and enforcement. Resource registration, a view, and collaboration membership confer no grant. |
| [ENV](NIP-ENV.md), [RUN](NIP-RUN.md), [COORD](NIP-COORD.md) | Environment allocation/attachment, durable effects and recovery, and claims/fencing. WS does not provision environments or transfer a RUN controller. |
| [CTX](NIP-CTX.md), [SESS](NIP-SESS.md), [CTRL](NIP-CTRL.md) | Task context, engine-session behavior, and task control. These can use the view contract below without merging their authority or state machines. |

A workspace is a host-admitted resource namespace. A project association is
an organizational link, not an execution root. A WORK planning project and a
Block MP repository grouping can both reference a workspace without becoming
its authority. Equal repository URLs or matching basenames cannot prove that
two checkouts, worktrees, documents, or workspaces are the same resource.

All new records contain `v`, `requires: []`, and optional inert `meta`, plus
exactly their stated fields. Use common IDs, pubkeys, timestamps, bounds,
ArtifactRefs, SchemaRefs, DefinitionRefs, and refusal codes. All unspecified
strings are inert bounded UTF-8, at most 1,024 bytes. Lists contain at most
256 entries per artifact unless a smaller bound is stated. An authenticated
private `3188` envelope or durable host-authenticated record establishes
attribution; a digest or a client-supplied authority field does not.

CAP operations advertise exact schemas for resource registration/read/change,
checkpoint creation/restoration, and projection reads. Remote calls use CJ
execution v1. Native IPC can implement the same contracts. Source and operation
implementations remain Rust host work; LSP, DAP, editor widgets, or a provider
SDK remain adapters rather than new sources of authority.

## Workspace and resource identity

A `ResourceRef` is exactly `{authority, workspace, resource}`: the authority's
pubkey and two common IDs. A workspace binding is
`openagents.workspace-binding.v1` with `workspace`, `owner`, `authority`,
`generation`, `revision`, `environment` (ENV attachment ArtifactRef or null
for a pre-materialization or non-compute resource namespace), `projects` (ArtifactRefs),
`roots` (ResourceRefs), `policy` (ArtifactRef), `authorization` (ArtifactRef),
and `created_at`. A new binding starts at generation/revision zero. Updates
increment revision; replacement of a root mapping or execution attachment
increments generation as well. Preserve prior artifacts.

ENV bootstrap uses an existing binding/snapshot that does not reference its
own future attachment. A pre-materialization binding can have null environment;
a later binding revision can cite an independently admitted active attachment.
That reference records provenance, not a perpetual execution grant. Successive
per-operation ENV attachments need not rewrite this workspace binding unless
the actual admitted root mapping or placement changes. Each operation still
requires its own current ENV admission.

The host independently establishes owner authority and canonical source
bindings. Actual absolute paths, filesystem handles, credentials, service
account identifiers, and connection material stay behind those bindings.
Display names and resource IDs cannot be used as host paths. Root membership
does not grant traversal outside the admitted namespace.

A resource descriptor is `openagents.workspace-resource.v1` with `ref`
(ResourceRef), `binding` (workspace-binding ArtifactRef), `kind`
(`collection`, `document`, `repository`, `worktree`, or `artifact`), `parent`
(ResourceRef or null), `name` (bounded display string), `adapter`
(DefinitionRef), `operations` (DefinitionRefs), `observation` (shared
observation ArtifactRef), and `lifecycle` (`available`, `unavailable`,
`removed`, or `unknown`). The source adapter pins whether rename preserves
identity; deletion followed by recreation must not silently resurrect old
version-dependent authority. Parent and ref share a workspace and authority.
Roots have null parent; other resources have one admitted parent. Cycles
refuse. Collections can be partially observed; listing one is not proof of
complete membership.

The observation's resource scope is the workspace ID and its ID is the
resource ID. Its adapter matches the descriptor. Host canonicalization also
binds the authority; these strings alone are not cross-host identities.
Recheck actual path resolution, symlinks, mounts, external account scope,
source versions, and read/write permissions at use. A resource can be
observable through one operation while another is unsupported or unauthorized.

## Documents, ranges, and conditional changes

A document snapshot is `openagents.document-snapshot.v1` with `resource`
(ResourceRef), `binding` (workspace-binding ArtifactRef), `observation`
(ArtifactRef), `content` (ArtifactRef or null), `encoding` (pinned format ID),
`derivation` (evidence ArtifactRef or null), `coverage` (`complete`, `partial`,
or `unknown`), and `editable` (boolean).
The content must agree with the observation or be accompanied by a derived
evidence artifact in derivation under a supported encoding profile. Exact
untransformed bytes have null derivation. Derived bytes have their own content
identity and are view-only; ranges bind those bytes, not an implied original.
In this v1 core, editable snapshots require complete exact bytes and a conditional provider
revision; binary, oversized, unsupported, or partial captures may be viewed
but cannot silently enter text-save operations. Unsupported encodings refuse.

Document ranges are `{snapshot, unit, start, end}`. Snapshot is an exact
document-snapshot ArtifactRef; unit is `utf8_byte`, and start/end form a
half-open interval within the retained bytes at valid UTF-8 boundaries.
Other indexing schemes need explicit adapter conversion evidence before
producing this range. Diagnostics, excerpts, review comments, and edits bind
that snapshot. A range from an older revision may remain historical evidence;
it cannot be applied to new text by assuming unchanged line numbers.

A change request is `openagents.resource-change.v1` with `command`, `actor`,
`resource` (ResourceRef), `binding` (workspace-binding ArtifactRef),
`expected` (observation ArtifactRef), `operation` (DefinitionRef), `input`
(ArtifactRef valid under that operation), `authorization` (ArtifactRef),
`issued_at`, and `expires_at`.
The signer must equal actor. The operation supplies a closed domain schema
for create, replace, move, delete, Git, or external-service mutation; an
arbitrary string or command line is not an operation definition.

The exact operation defines target scope, absence preconditions for creation,
source and destination versions for a move, downstream idempotency, effects,
confirmation evidence, and reconciliation. For a text replacement, input is
`openagents.document-replacement.v1` with `snapshot` (ArtifactRef), `content`
(ArtifactRef), and `encoding` (the unchanged admitted encoding ID). Snapshot,
resource, observation, and expected binding must agree. A replacement's new
bytes are a proposal until the owning source confirms the mutation.

Change and restoration proposals contain no approval decision. Authorization
identifies independently established existing authority, not a decision that
references this same proposal. A separately authenticated invocation uses
`openagents.workspace-action-admission.v1` with `proposal` (change-request
or restoration-plan ArtifactRef), `actor` (pubkey), and `approval` (POL
decision ArtifactRef or null). The signer equals actor. When approval is
required, its exact POL action pins the proposal as input and agrees with the
operation, binding, effects, bounds, and resource preconditions. The host
checks actual approver authority. The proposal and decision never hash-reference
each other; a proposal's own bytes cannot grant permission to execute it.

Idempotency is `(authority, actor, command)` with fingerprint SHA-256 of JCS
of the complete admission wrapper, which pins the immutable proposal. Persist
admission and effect intent before dispatch.
Identical retransmission returns retained state; conflicting bytes refuse.
After a lost acknowledgment, reconcile by that identity before any new
attempt. Client outboxes persist the complete normalized request and exact
attachments; they must not regenerate a branch name, base version, or payload
behind the same key. Corrupt queued records are quarantined, not discarded
as though the operation had never existed.

The host rechecks the actual current conditional version immediately at the
mutation boundary. A local lock cannot fence independent external writers.
Use supported conditional mutation or a trusted serialization mechanism;
otherwise refuse the conditional-change profile as `cannot_enforce`.
Read-then-write with a race is not compare-and-swap. A changed source or
workspace generation refuses as `stale` or `conflict`; no overwrite fallback
or implicit merge is allowed.

A change result is `openagents.resource-change-result.v1` with `request`
(exact workspace-action-admission ArtifactRef), `outcome` (common outcome),
`dispatched` (boolean), `before`
(observation ArtifactRef), `after` (observation ArtifactRef or null), `receipt`
(execution-receipt ArtifactRef), `verification` and `integration` (common
enums), and `reason` (common refusal code or null). Completed requires source
confirmation and a non-null after observation, including an authenticated
deletion observation for removal under the operation's pinned schema.
Failure after dispatch cannot be relabeled undispatched. No receipt, UI state,
or successful transport response can substitute for a missing source result.

## Worktrees, checkpoints, and restoration

A worktree record is `openagents.worktree-binding.v1` with `resource`
(ResourceRef), `repository` (ResourceRef), `binding` (workspace-binding
ArtifactRef), `creation` (execution-receipt ArtifactRef or null), `base`
(observation ArtifactRef), `owners` (admitted session/run ArtifactRefs),
`lifecycle` (`requested`, `creating`, `ready`, `cleanup_requested`,
`removing`, `removed`, or `unknown`), and `policy` (ArtifactRef).
It records creation and cleanup ownership; it grants no cleanup authority.
Creation can be null only while requested/creating or while unknown; ready
and later known states require confirmed creation evidence. The record also
contains `removal` (execution-receipt ArtifactRef or null): removed requires
confirmed removal evidence, and every other state has null removal. Neither
a pending create nor cleanup request proves that its effect completed.
Setup scripts require their own declared effects and exact implementation.
After restart, compare the actual Git/worktree identity to this record.
Unmatched paths are orphans, not automatically adoptable or removable worktrees.

Automatic cleanup requires current resource identity, creation provenance,
admitted cleanup policy, and proof of no remaining active owners. Confirm
tracked, staged, untracked, ignored, and externally created content under that
policy. A path under a familiar cache directory or an archived session is
insufficient. Missing owner or dirty-state evidence blocks removal; forced
removal needs a new exact destructive action and approval where policy requires.

A checkpoint is `openagents.workspace-checkpoint.v1` with `checkpoint`
(common ID), `binding` (workspace-binding ArtifactRef), `sources`
(ordered observation ArtifactRefs), `manifest` (ArtifactRef), `manifest_schema`
(SchemaRef), `capture` (`atomic`, `version_vector`, or `observational`),
`coverage` (`complete`, `partial`, or `unknown`), `omissions` (ArtifactRefs),
`created_at`, and `retain_until`. The manifest identifies exact retained
objects and reconstruction rules. Capture assurance must be supported by the
source adapters. Similar timestamps do not make a cross-store atomic snapshot.
A hidden Git ref is a locator for retained objects, not a complete checkpoint
of untracked files, provider history, tools, credentials, or processes.

A restoration plan is `openagents.workspace-restore.v1` with `command`,
`checkpoint` (ArtifactRef), `binding` (workspace-binding ArtifactRef),
`expected` (ordered current observation ArtifactRefs), `impact` (ArtifactRef),
`steps` (ordered exact CAP operation/input ArtifactRefs), `safety_checkpoint`
(ArtifactRef), `authorization` (ArtifactRef), `issued_at`, and `expires_at`.
Each step is the closed
object `{operation: DefinitionRef, input: ArtifactRef}`. Impact has a pinned
schema that explicitly enumerates overwritten/deleted resource versions and
the treatment of tracked, staged, untracked, and ignored content where relevant.

Restoration is a separately admitted RUN, not a rewind of reality. Require
live preflight, stopped/fenced conflicting writers, fresh versions, and current
authority; do not automatically replay an offline destructive request. Bind
the exact plan to any POL approval. Persist each step and its result before
advertising progress. Cross-store restoration can stop partially applied:
retain completed effects, unknowns, and recovery choices. Provider history
restoration requires a separately pinned adapter operation and its supported
SESS history semantics; this core defines no universal engine rewind. A
filesystem checkpoint alone cannot establish provider-history restoration.
Compensation is new admitted work, not evidence that
the original operation was atomic or never occurred.

## Reusable view definitions and cuts

The following profile is reusable for workspace trees, session catalogs,
task history, planning graphs, review, and attention. It synchronizes derived
observations; the source contracts retain command and execution authority.

A projection definition is `openagents.projection-definition.v1` with
`projection` (common ID), `authority`, `recipient` (pubkeys), `scope`
(ArtifactRef), `policy` (POL disclosure-policy ArtifactRef), `authorization`
(ArtifactRef), `transform` (DefinitionRef), `parameters` (ArtifactRef),
`row_schema` (SchemaRef), `sources` (ArtifactRefs), and `retention`
(`{retain_until, max_items, max_bytes}`). Scope and parameters use supported
pinned schemas. The definition binds one recipient and exact permitted
selection; it is not a wildcard grant to enumerate other tasks or resources.
The host checks current rights before queries, counts, paging, and every fetch.

A cut is `openagents.projection-cut.v1` with `projection` (the definition's
ArtifactRef), `epoch` (common ID), `sequence` (integer), `frontiers`,
`consistency` (`single_authority` or `federated`), `coverage` (`complete`,
`partial`, or `unknown`), and `captured_at`. Each frontier is
`{source: ArtifactRef, position: bounded opaque string}`. The source's pinned
schema defines position and ordering; every field in the output must be
covered by its required source frontier. Complete coverage means complete
for the declared authorized selection at this cut, never the whole hidden
source or all future work. A federated frontier does not claim an atomic
cross-host snapshot.

Epoch binds one projection definition and a stable interpretation of source
identity/order. A changed source generation, transform, row schema, selection,
recipient, or policy requires a new definition or epoch and resnapshot.
Retain tombstones and remembered higher cuts through the advertised recovery
horizon; stale snapshots cannot move an established client backward. Do not
choose among contradictory cuts by timestamp or arrival order. Unknown or
conflicting source authority makes the view incomplete and blocks actions that
depend on that evidence.

Projection sequence is a counter owned by the view authority, distinct from
RUN sequence and native engine offsets. A snapshot's cut must reflect the
minimum position actually applied by every required projector, not the latest
source event merely observed. Optional projections can lag only when their
fields are excluded or explicitly marked incomplete. Host state used to admit
commands must be current independently of an asynchronous display projection.

## Snapshot, delta, and live handoff

A read request is `openagents.projection-read.v1` with `request`,
`projection` (ArtifactRef), `mode` (`snapshot` or `changes`), `after`
(opaque cursor or null), `max_items` (1–256), and `max_bytes`
(1–1,048,576). The authenticated principal must equal the projection's
recipient. Initial snapshot has null after. All cursors are at most 1,024
UTF-8 bytes and bind definition, recipient, authorization epoch, finite cut,
query, and position. They grant nothing. Wrong scope, policy, epoch, future
position, or position older than retained history returns a typed refusal and
requires fresh admission/resnapshot as appropriate.

Changes mode requires a non-null cursor previously issued with a completed
snapshot or change page. A cursor for a partial snapshot cannot skip directly
to changes. The completed snapshot's cursor remains available for changes even
though its page has null next: the authority returns it in `resume` below.

A page is `openagents.projection-page.v1` with `request` (ArtifactRef),
`cut` (ArtifactRef), `rows`, `next` (opaque cursor or null), `coverage`
(the cut's coverage), and `resume` (opaque cursor or null). Resume is non-null
only on the final snapshot page and names that cut for a later changes read.
A row is `{key, value, sources}`: opaque bounded key,
ArtifactRef valid under row_schema, and ordered provenance ArtifactRefs the
recipient is allowed to see. A complete snapshot enumerates each row exactly
once at its finite cut; null next closes that enumeration, not future activity.
Rows referencing parents, sessions, or other required objects must provide
their declared closure or explicit unavailable/redacted placeholders. Never
silently drop an unresolved parent and declare the graph complete.

A delta is `openagents.projection-delta.v1` with `projection` (ArtifactRef),
`from` and `to` (cut ArtifactRefs), and `changes`. Each change is exactly
`{key, action, value, sources}`. Action is `upsert` or `remove`; upsert
requires a row-schema-valid value ArtifactRef, remove requires null. Sources
retain authorized provenance. From/to share definition and epoch; to.sequence
is from.sequence plus one. Change keys within one delta are unique. Apply the
whole delta atomically, preserving source frontiers and coverage. Identical
duplicate deltas are no-ops; conflicting
bytes for one transition are a protocol fault. A gap pauses application and
requires bounded repair or resnapshot.

A marker is `openagents.projection-marker.v1` with `projection`
(ArtifactRef), `cut` (ArtifactRef), and `state` (`caught_up` or
`resnapshot_required`). Caught-up proves only that this projection supplied
its admitted snapshot and changes through that finite cut. It does not prove
engine readiness, worker completion, or absence of later source records.

A changes response is `openagents.projection-change-page.v1` with `request`
(ArtifactRef), `from`, `to`, `head` (cut ArtifactRefs), `deltas` (ordered
delta ArtifactRefs), `next` and `resume` (opaque cursor or null), and `marker`
(marker ArtifactRef or null). All cuts share one definition and epoch.
From is the client's previously verified cut. To is the last contiguous cut
actually supplied by this page; an empty page has equal from/to. Head is the
finite upper cut captured when this catch-up enumeration began and remains
unchanged across its pages. Every delta joins the preceding cut exactly;
there are no hidden sequence gaps. To cannot exceed head.

Before to reaches head, next is non-null, resume and marker are null, and
the next page begins at to. At head, next is null, resume is non-null for a
subsequent changes read, and marker is caught-up at that exact head. A later
read captures a new finite head. If continuity is unavailable, return the
refusal result below rather than supplying a falsely complete page.
Page limits include framing and referenced delta sizes; each referenced fetch
also requires its own admitted bounds. No page may claim more applied progress
than the fully validated deltas it supplied.

The read operation returns `openagents.projection-result.v1` with `request`
(exact read-request ArtifactRef), `status` (`completed` or `refused`), `page`
(ArtifactRef or null), `marker` (ArtifactRef or null), and `reason` (common
refusal code or null). Completed requires the exact page schema selected by
the request mode, with null marker and reason; the page retains its own marker
where required. Refused requires null page and a reason. A refused result may
include a resnapshot-required marker only when the recipient remains entitled
to that definition and cut; all other refusals have null marker. Inaccessible
policy or wrong scope must not reveal a hidden projection identity. CJ completion
means this bounded read answered, even when its domain result refuses.

The authority must attach a durable change feed or bounded overlap buffer
before taking the snapshot, capture a finite replay head, supply the snapshot
and all deltas through that head, then issue caught-up and continue live
delivery. Equivalent algorithms are permitted only if they establish the same
gap-free cut. Bound items, bytes, and time in every queue. Overflow, missing
source history, or an unknown event schema stops that stream with an explicit
resnapshot-required condition; never drop updates and keep reporting live.
Coalescing must retain complete latest rows and structural dependencies; a
maximum received sequence alone cannot justify dropping intermediate mutations.

All artifact bytes are attributed to the expected view authority and bound by
their exact references. Pages can travel through CJ or admitted retained
artifact operations. This NIP does not introduce an unbounded CJ feedback
payload: a push transport must be separately negotiated with schemas, limits,
authentication, and recovery behavior; paged retrieval remains sufficient.
Socket liveness, relay EOSE, and a push notification cannot serve as caught-up.

## Command visibility, privacy, and client state

A command-visibility artifact is `openagents.projection-command-visibility.v1`
with `command` (an exact retained command receipt ArtifactRef), `projection`
(ArtifactRef), `cut` (ArtifactRef or null), and `state` (`visible`, `pending`,
or `not_in_scope`). Visible requires a cut whose included source frontiers
contain that command's admitted transition; a later snapshot cannot omit it
except through another recorded transition or declared projection rule.
Pending has null cut. Not-in-scope has null cut and does not disclose why a
hidden source was excluded. Only recipients separately entitled to that
receipt and view may query this relationship. Visibility is not external
effect completion or verification.

Clients keep connection health, synchronization health, cached age, operational
state, unread state, and user organization separately. Settled, snoozed,
archived, read, and focused are display/lifecycle concepts under their own
schemas; none grants execution or overrides pending approval, failure, or
unknown status. Geometry, hover, renderer caches, keybindings, virtualized
rows, Markdown, and platform accessibility belong to host/UI implementation.

Apply disclosure before row counts, identifiers, frontiers, and pagination.
When even provenance identities are restricted, return an admitted derived
projection with explicitly partial/unknown evidence; do not leak identifiers
to explain a redaction. Cached observation can remain visible only under its
retention policy and is labeled cached, not current. Revocation stops future
authorized disclosure but cannot retract copies already received.

Room membership and shared cursors are neither edit nor execution rights.
This profile supports conditional single-authority writes; it does not claim
CRDT/OT collaborative editing or define a new group-encryption scheme. Those
need separately pinned operations and consistency profiles. Terminal input,
pointer control, and resize are live-only operations and must not be replayed
from a generic offline outbox. PTY byte framing, terminal emulation, preview
tunnels, and debugger transports require their own admitted domain adapters;
a ResourceRef alone cannot open a port or process handle.

## Conformance

Required identity cases include equal paths on different hosts, equal clone
URLs with different worktrees, symlink/mount changes, namespace reuse, resource
deletion/recreation, and stale document ranges. Required mutation cases include
an independent writer racing save, lost acknowledgments, changed command
payloads, revoked authority, unsupported conditional writes, and partial
checkpoint restoration across stores. Cleanup must fail without creation,
owner, and dirty-state proof.

View tests include subscribe/snapshot races, out-of-order and duplicate deltas,
structural parent gaps, hidden-row count leaks, policy changes during paging,
stale/future cursors, coalescing, buffer overflow, projection lag, unknown
schema versions, authoritative-source forks, and restart after a committed
command before display update. Test read-your-command against included source
frontiers, not wall time. UI and installed-device tests remain necessary for
usability; passing protocol fixtures does not establish those results.
