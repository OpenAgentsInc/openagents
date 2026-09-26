# NIP-CTRL — Scoped task control across clients

`draft` `optional` — v1, 2026-09-26. The [shared contracts](contracts.md)
are normative. This profile binds additional clients to an existing task
owner, defines authenticated steering and cancellation, and permits bounded
catch-up after disconnection. It defines no new event kinds.

Terminal, mobile, web, and headless clients can control one task without
copying its execution authority or creating another agent. This is a target
contract, not a claim that current Coder clients implement device pairing,
durable resume, or these operations.

## Relationship to the existing contracts

| Existing contract | Reuse and boundary |
| --- | --- |
| [CTX](NIP-CTX.md) | Task identity, objective, corrections, frame revisions, and evidence. CTRL does not define a second task frame. |
| [POL](NIP-POL.md) | Recipient disclosure, exact action approvals, and host policy. Task control does not grant approval, spending, or publication authority. |
| [COORD](NIP-COORD.md) | Dependencies, cancellation, claims, reservations, findings, and integration. A client connection is not a resource claim. |
| [RUN](NIP-RUN.md) | Authoritative journal, recovery, controller generations, and fenced handoff. Reconnecting a client does not transfer ownership. |
| [CAP](NIP-CAP.md) and [CJ](NIP-CJ.md) | Registered typed operations, binding admission, authenticated invocation, idempotency, and results. CTRL is an operation profile over execution jobs. |
| Official [NIP-46](../official/46.md) | Optional remote signing. A signer connection or permission to sign an event is not permission to control a task. |
| Block [NIP-OA](../block/NIP-OA.md) and [NIP-AA](../block/NIP-AA.md) | Agent provenance and relay admission. Neither supplies the task-scoped rights below. |
| Block [NIP-AO](../block/NIP-AO.md) | Optional live telemetry and advisory `cancel_turn`. Its ephemeral owner-only channel cannot replace durable command admission or catch-up. |
| Block [NIP-RS](../block/NIP-RS.md), [NIP-PL](../block/NIP-PL.md), and [NIP-ER](../block/NIP-ER.md) | Read-position sync, mobile wakeups, and reminders. They grant no task control and establish no execution result. |

A host exposes the following CAP operation roles with pinned SchemaRefs:

| Role | Input | Result |
| --- | --- | --- |
| Accept a pairing response | `openagents.control-pairing.v1` | `openagents.control-access-result.v1` |
| Submit a task command | `openagents.task-command.v1` | `openagents.task-command-result.v1` |
| Read task state or retained history | `openagents.task-read.v1` | `openagents.task-view.v1` |
| Revoke client access | `openagents.control-revoke.v1` | `openagents.control-access-result.v1` |

Remote calls use CJ execution v1 and its existing `25920`/`26920`/`27020`
kinds. Separately signed invitations, pairing responses, grants, revocations,
commands, and views use the private `3188` artifact envelope. An envelope
alone dispatches nothing. Local IPC can use the same artifacts with durable
host-authenticated provenance. Relays validate the envelope and its ACLs;
hosts validate the decrypted operation and authority. A CJ `completed` result
means the control operation answered; it does not mean the command was
accepted or that the coding task completed.

The host admits the pairing operation separately for an invited client before
a task grant exists. That narrow bootstrap admission can validate and consume
only its named invitation; it grants no task observation, execution, model
call, or spending. Rate and byte limits apply before artifact resolution.

## Encoding, principals, and limits

Every artifact defined below contains `v`, `requires: []`, and optional inert
`meta`, plus exactly its specified fields. Reject unknown semantic fields,
enum values, versions, duplicate keys, and unsupported required features.
Common IDs are random 64-hex values; pubkeys and references use the shared
contracts. Timestamps are Unix seconds. Host time, not event `created_at`,
enforces expiry. The common byte and nesting limits apply.

The **owner** is the principal whose authority establishes the task. The
**authority** is the currently admitted RUN controller that serializes task
state and control admission. A **client** is a distinct public key admitted
to a stated subset of control rights. A device label is optional display
metadata, never a hardware identity or authorization credential.

The host MUST independently establish the owner/controller relationship.
An invitation, self-signed grant, display name, relay membership, or supplied
owner field cannot establish it. Keys and provider credentials stay in their
own host-owned stores; pairing does not copy an owner's secret key.

This version scopes each grant to one task and one RUN controller generation.
Several grants can serve a dashboard, but listing tasks requires their
individual observation authority. There is no implicit all-task, all-device,
or organization-wide grant. A later profile can define broader scope without
turning an empty list or wildcard into authority in this version.

## Rights and scope

The exact rights are `observe`, `steer`, and `cancel`. A grant contains a
nonempty, duplicate-free subset. Rights do not imply one another.

- `observe` permits the task-read operation and only the evidence allowed by
  the pinned disclosure policy and actual source grants. It does not permit
  steering, execution, or fetching an arbitrary referenced artifact.
- `steer` permits submitting a correction to the admitted task frame. The
  controller resolves it under the owner's instruction policy. It does not
  permit editing grants, increasing budgets, selecting a new recipient, or
  replacing protected acceptance criteria by implication.
- `cancel` permits requesting cancellation of the scoped task and its managed
  descendants. It does not prove that processes stopped or effects reversed.

Task creation, paid work, spending increases, publication, training, package
adoption, administrative changes, and controller transfer require their own
admitted operations. No general `approve` right exists in CTRL. A client
returning a POL approval decision must be the separately recognized approver
for the exact action, even if it already has all three CTRL rights. A device
may instead ask a NIP-46 signer to produce the owner's POL decision; the
receiving host still checks that decision and the actual approver authority.

Every scope is the closed object `{task, controller, generation}`: task ID,
authority pubkey, and RUN generation. It MUST match the host's current task
owner and controller. Handoff invalidates old-generation admission. Existing
grant bytes do not silently follow a task to a different machine or controller.

## Pairing and explicit admission

Pairing requires an independently authenticated introduction, such as a local
owner action binding the displayed client pubkey and the known host pubkey.
A QR code or URL can carry that introduction; scanning an untrusted link
does not establish trust. Client-supplied connection hints must pass host
destination policy before use. This profile defines the signed artifacts,
not a new URI scheme or an unauthenticated invitation-discovery service.

An invitation has `v: "openagents.control-invitation.v1"` and:

| Field | Meaning |
| --- | --- |
| `invitation`, `challenge` | Independently random common IDs. The challenge binds proof of possession to this exchange. |
| `owner`, `authority`, `client` | Exact, distinct client/authority keys and the independently established owner. |
| `scope` | Exact task/controller/generation object. |
| `rights` | Proposed rights, still inert. |
| `policy` | POL disclosure/control policy ArtifactRef supported by the host. |
| `issued_at`, `expires_at` | Expiry later than issue time and within a host-declared maximum pairing window. |

The authority signs and encrypts the invitation to the exact client key.
The client verifies the expected authority and owner before accepting it.
No task objective, transcript, file path, credential, or content locator
belongs in the invitation. Opaque scope identifiers are the minimum control
metadata disclosed by pairing; they are not an observation grant.

A response has `v: "openagents.control-pairing.v1"`, `invitation`
(ArtifactRef), `client` (pubkey), `challenge` (the exact value), `rights`
(a nonempty subset of the invitation), and `accepted` (boolean). The client
signs its private envelope. The host verifies its signer, the original
authority-signed invitation, matching challenge, exact recipient, current
scope, expiry, and unused invitation identity. A relayed or copied response
cannot substitute another client key. `accepted: false` closes the invitation
without granting access.

Proof of the client's key and challenge possession is necessary but does
not authorize it. The host also requires an explicit owner admission recorded
locally or by a supported POL approval for this exact pairing action. It
MUST atomically consume the invitation and persist the resulting grant or
refusal before acknowledging it. Repeated identical responses return the same
access result; conflicting responses cannot create another grant. Expired
invitations cannot be revived by backdating a signed event.

A grant has `v: "openagents.control-grant.v1"` and the fields `grant`
(new common ID), `epoch` (zero), `owner`, `authority`, `client`, `scope`,
`rights`, `policy`, `invitation`, `pairing`, `admission`, `issued_at`, and
`expires_at`. Invitation, pairing, and admission are exact ArtifactRefs;
admission is the retained owner authorization evidence accepted by the host.
Rights are no wider than the response and admission. Expiry is bounded by
host policy and the owner's admission; it does not have to equal the shorter
pairing window. The authority signs the grant and retains the complete
admission closure. Renewal or a scope change requires a new invitation and
grant ID. A copied grant is not a bearer credential: every operation requires
the named client's authenticated signature and current host admission.

An access result has `v: "openagents.control-access-result.v1"`, `request`
(ArtifactRef), `status` (`granted`, `revoked`, `duplicate`, or `refused`),
`access` (grant or revocation ArtifactRef, or null), and `reason` (common
refusal code or null). Granted/revoked and duplicate results require the exact
previously persisted access artifact. Refused results require a reason and
null access. Authenticate the expected authority's result; a reflected result
from the client is not host admission.

## Revocation and current authority

A revocation request has `v: "openagents.control-revoke.v1"`, `request`
(common ID), `grant` (ArtifactRef), and `reason` (inert string of at most
1,024 UTF-8 bytes). Only the owner or an independently admitted access
administrator can revoke; `steer`, `cancel`, and `observe` do not grant that
administrative role. A host may terminate access under its own stricter policy.

The persisted result has `v: "openagents.control-revocation.v1"`, `grant`
(ArtifactRef), `epoch` (one), `authority` (pubkey), `request` (ArtifactRef),
`authorization` (ArtifactRef), and `revoked_at`. The authority atomically
advances that grant's retained authorization epoch from zero to one and
marks it revoked before answering. Epoch one is terminal for this grant ID.
Keep its tombstone until at least grant expiry plus the host's maximum
accepted clock skew. Restart must not reset the epoch or restore an expired
or revoked grant. Re-enrollment creates a distinct grant ID.

Every command, read, and referenced-content fetch checks current admission at
the actual authority, including epoch, expiry, controller generation, and
applicable disclosure. Clients cannot establish nonrevocation from an absent
relay event or cached grant. If current authority cannot be checked, no new
effect or disclosure is admitted. Disconnecting or revoking access does not
erase data already received or prove that an admitted command stopped. The
owner can separately request task cancellation and reconciliation.

## Commands, ordering, and honest receipts

A command has `v: "openagents.task-command.v1"` and:

| Field | Meaning |
| --- | --- |
| `command` | Common ID, stable through retransmission. |
| `grant`, `epoch` | Exact grant ArtifactRef and its active epoch, zero in this version. |
| `scope` | Same exact task/controller/generation as the grant. |
| `expected_revision` | Required CTX frame revision the client intends to affect. |
| `issued_at`, `expires_at` | Fresh command window, within grant expiry and host limits. |
| `action` | `steer` or `cancel`; requires that exact right. |
| `payload` | Closed action-specific object below. |

For `steer`, payload is `{message, replaces}`. Message is an ArtifactRef to
bounded UTF-8 instruction text; replaces is a duplicate-free list of exact
objective/constraint ArtifactRefs in the expected frame, possibly empty.
For `cancel`, payload is `{reason}`, with the same inert-text bound as a
revocation request. No executable code, new grant, or arbitrary host command
can acquire control semantics by appearing in either payload.

The client signs the command. The host validates the original signature even
when another admitted operation relays it. The signer must equal the grant's
client. Original author provenance must survive forwarding; an unsigned JSON
copy is insufficient. Before admission, compare scope, epoch, current frame
revision, rights, deadlines, and host authority. A stale revision produces a
conflict and no task mutation. No last-writer-wins rule applies to concurrent
steering. Cancellation against a stale frame also requires renewed intent;
the host must not silently retarget a later task revision.

The idempotency key is `(authority, grant ID, command ID)`. Its fingerprint
is SHA-256 of JCS of the entire command. Persist that identity, admission,
and the intended transition atomically before acknowledging or dispatching
effects. Identical retransmission returns the retained command result without
another frame update or cancellation request. Different bytes under the same
key are `idempotency_conflict`. Preserve receipts or tombstones through the
command's expiry and admitted recovery horizon; absence after retention is
not permission to redispatch. CJ transport retries preserve this command ID.

An accepted steering command creates the next CTX frame under the controller's
instruction-resolution policy and references the authenticated correction.
Preserve the command's signed declaration as provenance. A refused correction
does not advance the frame. A correction invalidates affected pending contexts
and proposals; it does not erase past effects, reset spending, or widen policy.
Once cancellation is admitted, use COORD/RUN/CJ cancellation semantics:
prevent new queued dispatch, propagate the request, and retain unresolved
effects and accounting. Control receipt is distinct from confirmed stop.

A result has `v: "openagents.task-command-result.v1"`, `command`
(ArtifactRef), `status` (`accepted`, `duplicate`, `conflict`, `refused`, or
`unknown`), `receipt` (ArtifactRef or null), and `reason` (common refusal
code or null). Accepted/duplicate results require a retained receipt;
conflict/refused require a reason. The closed receipt is
`openagents.task-command-receipt.v1` with `command` (ArtifactRef), `authority`
(pubkey), `admitted_at`, and `disposition` (`correction_recorded` or
`cancel_requested`). It records command handling, not successful execution.
RUN records reference this receipt and any resulting frame or pending effects.

Control-only clients receive this narrow receipt, not the RUN record, current
objective, current frame, transcript, artifacts, active tool arguments, or
another client's commands. Conflict responses do not reveal a newer revision
or whether a guessed task exists. A caller that lacks observation can obtain
updated control metadata only through a separately authorized owner flow.
Unknown handling requires reconciliation; it is not permission to send a
new command ID with the same intended effect.

## Observation, catch-up, and optional synchronization

A read has `v: "openagents.task-read.v1"`, `request` (common ID), `grant`
(ArtifactRef), `epoch`, `scope`, `view` (`state` or `history`), `after`
(opaque cursor or null), `max_items` (1–256), and `max_bytes` (1–1,048,576).
It requires `observe` and current source/disclosure admission. A cursor is
opaque bounded text, at most 1,024 UTF-8 bytes. It is a host-bound retrieval
position, never authorization. Bind it to grant, scope, view, disclosure
policy, and snapshot; reject cross-client or stale-policy reuse. A `state`
read has null `after`. The host may impose smaller advertised limits.

A view has `v: "openagents.task-view.v1"`, `request` (ArtifactRef), `authority`
(pubkey), `scope`, `captured_at`, `policy` (ArtifactRef), `items`, `next`
(opaque cursor or null), and `coverage` (`complete`, `partial`, or `unknown`).
Each item is `{kind, artifact, provenance}`: kind is `frame`, `run_record`,
`trace`, `usage`, `finding`, or `projection`; artifact and provenance are
ArtifactRefs. Provenance is a shared evidence descriptor identifying the
source, capture coverage, and any derivation; it must describe the supplied
artifact bytes. Frame, run record, and finding artifacts use CTX, RUN, and
COORD schemas; trace and usage require pinned schemas supported by the client.
Unknown required item schemas refuse instead of being rendered as verified
history. Validate original signatures where claimed. A view is the authority's
bounded observation, not a second authoritative task journal. The request's
grant epoch, the view's scope generation, and the underlying RUN chain must
agree where present. The
authority signs the result or its envelope.

A projection artifact has `v: "openagents.task-projection.v1"`, `content`
(ArtifactRef), `sources` (ordered ArtifactRefs the recipient may see),
`reason` (`redacted`, `unavailable`, `unverifiable`, or `bounded`), and
`coverage` (`partial` or `unknown`). Content uses a pinned, supported display
schema and carries no executable authority. Sources may be empty when even
source identifiers are restricted; this does not establish original provenance.
The accompanying evidence descriptor states the current authority's derivation
and actual coverage. A projection cannot claim to be a complete RUN record.

Use existing RUN replay and retained ATIF artifacts behind this operation.
Apply access checks before selection, pagination, counts, and content fetch.
Respect the requested item and byte bounds, counting serialized framing and
references; fetch of a referenced object requires its own bounds and admission.
A history cursor is tied to a finite snapshot so a moving live tail does not
prevent pagination from finishing. Missing records, expired retention, and
redaction produce partial/unknown coverage with source-linked projection
evidence; they cannot become a complete transcript. An empty page, EOSE, or
missing newer head cannot establish that no later work exists.

Re-encrypt historical records only under their owner's permitted disclosure.
Do not pretend that re-signing another controller's record preserves its
original authorship. When the original signed record cannot be independently
validated by the recipient, return a new `projection` with derivation and
the current authority's attribution. A redacted projection has new bytes
and identity; it must not claim to be the complete original RUN chain.

Revocation ends future authorized disclosure; encryption cannot retract prior
copies. Replication, retention, artifact fetching, and hosted sync need their
own admitted storage and recipient policy. Optional sync does not transport
credentials, authorize training/publication, or resolve a RUN fork by timestamp.
NIP-RS can synchronize read positions only; NIP-PL can wake a client only.
The client resumes by validating retained state, never by treating a push or
read marker as task authority. Offline UI may show cached data as stale, but
must not represent queued controls as applied or increase local authority.

## Handoff and lifecycle

For a richer workbench, [WS](NIP-WS.md) defines reusable finite projection
cuts, pages, deltas, and command visibility. Its views preserve the independent
CTRL observation checks; they do not replace this profile's bounded task-read
schema or grant directory-wide access. [SESS](NIP-SESS.md) defines engine queue,
steering, and interruption semantics. A CTRL frame correction does not prove
an engine consumed it, and a SESS operation still needs the appropriate
independent session authority. Do not silently widen an existing control grant
when composing these profiles.

The useful lifecycle is invitation → client proof → owner admission → active
grant → bounded reads/commands → expiry or revocation. Task closure does not
erase its history, but closes new steering/cancellation admission. History
remains readable only through a still-valid observation grant and retention.

Grant admission and controller handoff serialize at the authority. RUN handoff
must stop and fence old dispatch before the next controller takes over.
Pending controls retain their original IDs and observed dispositions; no
client retries them at the new host as fresh work. The new controller requires
explicit owner admission for new grants. Unknown work stays unknown until
reconciled. If the old controller is lost, use RUN's independently trusted
fencing/recovery authority or refuse automatic takeover.

Capacity discovery, build slots, disk reservations, placement heuristics, and
workspace transfer remain CAP/COORD bindings and host work. CTRL conveys their
admitted observations when permitted; it introduces no fleet inventory, resource
lease, payment rail, marketplace order, or computer-control command language.

## Worked wire flow

The following notation uses deliberately redacted identities such as
`<owner-key>`, `<client-key>`, and `<artifact-ref>`. They are explanatory
placeholders, not valid wire values or fixture keys.

1. At a trusted desktop UI, the owner admits an invitation for `<client-key>`
   with `observe` and `cancel` for one task and generation. The controller
   publishes a signed `3188` envelope, `p`-tagged and encrypted to that client,
   containing the invitation artifact. Its opaque mailbox reveals no task name.
2. The client verifies the pinned authority and returns the invitation digest,
   exact challenge, and accepted rights in its signed pairing artifact. A
   registered CAP pairing operation receives it through CJ execution. The host
   verifies the owner admission and atomically consumes the invitation, records
   the grant, and returns its signed access result. Relay `OK` alone is not it.
3. The client invokes task-read with `view: "history"`, null cursor, and bounded
   page limits. It validates the authority's signed view, expands only admitted
   references, and marks a missing tail as partial. A wake notification merely
   causes this same authenticated read.
4. The client submits a signed `cancel` command with its exact grant, generation,
   and expected frame revision. The host persists `cancel_requested` and returns
   a command receipt. The UI shows cancellation requested until observed RUN
   evidence establishes stop or unresolved work.
5. A disconnected retransmission with the same command ID returns the retained
   receipt. It does not cancel a different run or create a second effect. If
   the owner revokes the grant first, later reads and unadmitted commands refuse.

## Conformance

Fixtures must cover forged owner/controller relationships, arbitrary signer
invitations, wrong challenge or client, expired/backdated pairing, duplicate
and conflicting pairing, and a crash between consumption and response. Cover
control rights independently, including observation-only mutation refusal,
cancel-only transcript refusal, and a steering grant that cannot approve a
POL action or expand spend/disclosure.

Exercise stale frames, wrong generation, duplicate/conflicting command IDs,
revocation racing command admission, expiry and restart, lost acknowledgments,
post-dispatch cancellation, and unknown effects. Exercise pagination bounds,
cursor swapping, unauthorized ID lookup, denied artifact expansion, retention
gaps, redacted projections, original-signature loss, RUN forks, and old-head
replay. Verify handoff with an unreachable unfenced worker cannot produce
two active controllers. Verify push, NIP-OA, NIP-AA, and NIP-46 permission
cannot substitute for a current control grant.

Advertise `nip-ctrl-v1` only for the configured host/client role with these
behaviors tested. A relay serving `3188` and CJ may advertise those underlying
roles; that alone does not establish CTRL admission, privacy projection,
durable command handling, or task control. Use NIP-11 `supported_extensions`,
not a numeric `supported_nips` entry.
