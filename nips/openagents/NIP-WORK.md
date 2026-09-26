# NIP-WORK — Tracked objectives and planning

`draft` `optional` — v1. The [shared contracts](contracts.md) are normative.
This NIP defines tracked work that can outlive a conversation, execution,
commercial order, or client. It is a proposed protocol, not an implemented
issue tracker. It assigns no new event kinds.

A work item names an objective and an accountable principal. An agent can
perform work under a separate delegation without becoming its accountable
owner. Coding issues are one use. Research, operations, document preparation,
and other domains use the same records with their own admitted input and
acceptance schemas.

## Scope and composition

A native Issue view and its Work item MUST share one identity and revision
history. Do not create two writable lifecycles and synchronize them afterward.
A planning project groups work toward an outcome; it is not a repository or
an editor workspace. A board, roadmap, calendar, inbox, or search index is a
projection, not a second authority.

| Contract | Owns |
| --- | --- |
| WORK | Tracked objective, planning relations, accountable owner, assignment intent, evidence links, and work disposition. |
| [CTX](NIP-CTX.md) | Exact task frames, source snapshots, evidence, and context selections consumed by execution. |
| [COORD](NIP-COORD.md) | Execution tasks, dependencies, resource claims, fencing, and proposed integration. |
| [CJ](NIP-CJ.md) and [RUN](NIP-RUN.md) | Admitted attempts and their durable effects and outcomes. |
| [SESS](NIP-SESS.md) | Conversation and attached interaction history, including questions and activities. |
| [WS](NIP-WS.md) | Workspace/resource identity and versioned projection cuts. |
| [AUTO](NIP-AUTO.md) | Durable trigger and continuation admission. |
| [MKT](NIP-MKT.md) and [LAB](NIP-LAB.md) | Commercial agreement, delivery, verification, acceptance, disputes, and settlement. |
| [POL](NIP-POL.md) and [CAP](NIP-CAP.md) | Current authority, disclosure, and admitted operations. |

Assignment, a label, a workflow state, and a workroom mention MUST NOT dispatch
execution by themselves. An admitted AUTO rule or explicit caller may propose
a new bounded operation. Each execution pins a work revision and a CTX frame;
later edits cannot change its frozen inputs or acceptance contract. Corrected
requirements require a new admission, and a new LAB order when commercial
terms change. Retain and reconcile earlier work rather than rewriting it.

## Encoding and authentication

All bodies below are closed JSON objects with their named `v`, required
`requires: []`, all listed fields, and optional inert `meta`. Unknown semantic
fields and enum values refuse. Common ArtifactRef, DefinitionRef, EventRef,
ID, pubkey, integer, and parsing rules apply. Arrays contain distinct entries
and have at most 64 entries unless stated otherwise. Display text is bounded
UTF-8 data; it cannot carry grants or executable instructions.

Private records use the encrypted `3188` artifact envelope. Every body with
`issuer` must be declared by that same Nostr signer. Authenticate the original
signed declaration when a record is forwarded. An artifact digest alone proves
no author. An exact reference to a signed record retains its declaration;
changing recipients can change the envelope ID without changing artifact
bytes. No item titles, private identities, source URLs, plaintext digests, or
project membership may leak through public tags.

`WorkRef` is the closed object `{authority, scope, item}`: the admitted authority
pubkey, common random scope ID, and common random item ID. It identifies an
item without containing its future artifact digest. `WorkRevisionRef` is
`{work, revision, artifact}`: WorkRef, positive integer revision, and exact
ArtifactRef to the revision below. Resolve and compare every component before
use. Display numbers such as `BUG-123` are inert aliases.

A WS `ResourceRef` is `{authority, workspace, resource}` and names the owning
workspace resource, not the work item. References to external issues, commits,
reviews, and documents use the source observation contract below. Matching
titles, URLs, slugs, or integer issue numbers do not establish identity.

## Scope authority and operation admission

A scope descriptor has `v: "openagents.work-scope.v1"` and:

| Field | Contract |
| --- | --- |
| `issuer`, `owner` | Same owner pubkey. Owner admits this descriptor; discovery alone cannot install it. |
| `scope` | Fresh random scope ID. |
| `authority` | Pubkey of the admitted host that serializes native work changes. |
| `policy` | Exact ArtifactRef to the host-supported authority, disclosure, retention, and transition policy. |
| `resources` | Exact ArtifactRef to the admitted WS resource namespace, or null if no workspace is involved. |
| `created_at` | Unix seconds; informational, not a concurrency clock. |

The owner chooses the authority through an independently authenticated local
or POL admission. Room membership, an OA owner attestation, a public agent
profile, and a self-published scope do not acquire control over another scope.
A host must resolve the policy before accepting commands and retain the actual
current policy/admission used by each operation. The owner may revoke the host
or narrow policy through its admitted control path; old signed descriptors do
not override current revocation. Scope migration or replacement authority is
unsupported in v1: create a new scope with explicit provenance and separately
admit any new execution. Never reuse an old scope ID with another authority.

CAP operations expose these exact input/output schemas. Remote calls use CJ
execution v1; the operation signer must equal the command `issuer` or, for reads, the
projection definition's recipient.
Local calls authenticate the same principal through the host's admitted path.
Each mutation also needs a current compatible host grant; executing a CJ
request does not grant every work mutation.

| Operation | Input | Output |
| --- | --- | --- |
| Create or revise work/planning item | `openagents.work-command.v1` | `openagents.work-operation-result.v1` |
| Read a scoped projection | WS `openagents.projection-read.v1` | WS `openagents.projection-result.v1` |

Owners govern policy and accountability. A principal granted create/revise may
propose item changes; assign rights are separately required to change the
accountable principal or delegation. A reviewer can submit evidence without
receiving disposition rights. Only an owner or separately admitted disposer
can accept, reject, cancel, reopen, supersede, or archive work. Read, revise,
assign, dispose, and external integration are distinct rights. A policy may
narrow these roles; it cannot make a body field or model output a grant.

The authority serializes mutation admission against the current native item
revision, current policy, actor, and exact inputs. Rights must cover every
changed field; an agent with revise rights alone cannot change accountable
principal, grant, acceptance policy, or disposition. A change to frame or
acceptance requirements requires owner or separately admitted requirements
authority, not general progress-edit permission. It persists the accepted
revision and its RUN admission record before reporting success or publishing
it. Its durable idempotency ledger is keyed by `(scope, issuer, command_id)`;
the fingerprint is SHA-256 of JCS of the entire command. Identical bytes return
the retained result only to a principal still authorized to read it; replay
never redispatches and does not require granting new mutation authority. Changed bytes with that key refuse as `conflict`. The
ledger and accepted history survive process restart; retention cannot expire
an unresolved effect or make an old request new.

A crashed request whose durable result cannot be established is `unknown`.
Clients read/reconcile the same command ID instead of creating a replacement.
A relay acknowledgement proves delivery only. This profile provides no global
consensus, no independent guarantee that a remote host obeyed its admission,
and no exactly-once promise over external systems.

## Items and planning relationships

An item snapshot has `v: "openagents.work-item.v1"` and these fields:

| Field | Contract |
| --- | --- |
| `kind` | `work`, `initiative`, `project`, `milestone`, `cycle`, or `decision`. |
| `title` | Nonempty text, at most 512 UTF-8 bytes. |
| `body` | ArtifactRef to declared content, or null. Untrusted descriptive data. |
| `accountable` | Responsible principal pubkey. Changing it requires assign authority and does not transfer any financial or execution grant. |
| `state` | `open`, `in_progress`, `blocked`, `resolved`, `cancelled`, `superseded`, or `archived`. |
| `frame` | Exact CTX task-frame ArtifactRef for a `work` item, otherwise null. |
| `acceptance` | Exact domain acceptance-policy ArtifactRef, or null for a non-work planning item. Required for `work`; the frame and policy must agree. |
| `delegation` | Work delegation ArtifactRef, or null. |
| `relations` | Array of the typed relations below. |
| `resources` | Array of exact WS ResourceRefs. Linking a resource grants no access. |
| `evidence` | Array of exact work evidence-link ArtifactRefs. |
| `disposition` | Work disposition ArtifactRef, or null. |
| `labels` | Array of inert slugs. At most 32; no authority or automatic execution. |
| `due_at` | Unix seconds or null; an attention target, not automatic acceptance or payment. |
| `timebox` | `{starts_at, ends_at}` for a cycle, with starts_at < ends_at; otherwise null. |

The item schema contains no self-reference. Its accepted revision supplies its
identity. A snapshot cannot reference the revision that will contain it.
Exact task frames, acceptance policy, and sources can be prepared first;
execution/admission links are attached afterward. This ordering avoids hashes
that recursively depend on themselves.

A relation is `{type, target, evidence}`: type is `parent`, `blocks`, `related`,
`duplicate`, or `supersedes`; target is a WorkRef; evidence is an ArtifactRef
or null. Relations are directed. The inverse is a derived view, never a second
independently writable edge. `parent` allows at most one parent. Parent and
blocks targets must be in the same scope and resolve to retained items; both
graphs must be acyclic. Reject self-links. Graph validation has finite host
node, depth and time limits; exceeding them refuses admission rather than
skipping unresolved edges and declaring the graph safe. Related/duplicate/supersedes may
refer to another admitted scope only when its exact identity and disclosure
are validated. Unavailable targets refuse admission; do not invent them.

A duplicate relation is an admitted classification claim, not evidence that
outputs or obligations are equivalent. It cannot copy execution success,
remove an obligation, or reuse a result. COORD's exact reuse contract still
applies. A supersedes relation does not cancel or forgive earlier external
work or payment. Unlinking evidence or a relation removes it from the new
snapshot only; the prior revision remains retained under policy.

Containers do not infer results from child counts. Resolving a project requires
its own disposition and pinned supporting evidence. Resolving one work item
does not automatically resolve its parent, dependent, or commercial order.
A `decision` item records an accountable choice; it grants execution only
through a separately admitted operation.

## Commands and authoritative revisions

A command has `v: "openagents.work-command.v1"` and:

- `issuer`: authenticated proposing principal pubkey.
- `command_id`: stable random request ID.
- `scope`: exact scope-descriptor ArtifactRef.
- `work`: WorkRef with authority/scope matching that descriptor.
- `action`: `create` or `replace`.
- `expected`: WorkRevisionRef, or null only for create.
- `snapshot`: exact `openagents.work-item.v1` ArtifactRef.
- `source`: work source-observation ArtifactRef, or null.
- `reason`: ArtifactRef describing the requested change, or null.
- `expires_at`: Unix-second admission deadline.

Create must target a never-used item ID and start in `open` with null
disposition. Replace requires the exact current revision. Full replacement is
intentional: the host validates all changed fields and their corresponding
rights together. It cannot silently accept only part of an unauthorized
mutation. Imported source observations, prompt text, and recommendations are
proposals, never an admission path around the same checks.

An authoritative revision has `v: "openagents.work-revision.v1"` and:

- `issuer`: the scope authority pubkey.
- `scope`: exact scope-descriptor ArtifactRef.
- `work`: matching WorkRef.
- `revision`: 1 for create, otherwise previous revision + 1.
- `previous`: prior WorkRevisionRef, or null for revision 1.
- `command`: exact authenticated command ArtifactRef.
- `snapshot`: the exact accepted item ArtifactRef from that command.
- `admission`: retained RUN record ArtifactRef or exact RUN EventRef naming
  the command, policy, grant, and admitted effect.
- `recorded_at`: Unix seconds from the authority's clock.

The RUN admission record must not contain a digest of this future revision;
it binds the command and proposed snapshot instead. The revision can then bind
the existing RUN record. A later RUN publication/result record may reference
the revision. No circular artifact or event-ID dependency is permitted.

A read verifies the chain or a previously retained exact anchor. Missing
history is partial, not complete. Two authority-signed successors for one
previous revision are a fork: retain both, stop automatic mutation against
that item, and request resolution through the owner-admitted authority. Do
not choose by wall-clock timestamp, relay arrival, lexicographic ID, or model
preference. A host may resolve only with explicit retained branch evidence and
an owner-admitted recovery outside v1; readers must not infer that no unseen
fork exists.

The operation result has `v: "openagents.work-operation-result.v1"`, `issuer`
(authority), `command` (ArtifactRef), `status` (`accepted`, `duplicate`,
`conflict`, `refused`, or `unknown`), `revision` (WorkRevisionRef or null),
`record` (ArtifactRef to the prior RUN result record or its exact EventRef,
never a reference to this response itself), and `reason` (common
refusal code or null). Accepted/duplicate require the retained accepted
revision. Conflict/refused require a reason and may name the known current
revision. Unknown must not claim an accepted revision that was not recovered.
CJ `completed` means the work operation answered; its contained result may
still be conflict or refusal.

## Delegation and execution links

A delegation has `v: "openagents.work-delegation.v1"`, `issuer`, `work`,
`basis`, `delegate`, `frame`, `grant`, `until`, `executions`, and `order`.
Issuer is the assign-authorized principal. Work is a WorkRef; basis is an exact
prior WorkRevisionRef for that work; delegate is a pubkey; frame is an exact
CTX task-frame ArtifactRef; grant is the independently admitted POL/host grant
ArtifactRef; until is its bounded Unix-second expiry. The frame must equal the
item's pinned frame when the delegation is admitted. The grant must identify
the delegate and compatible task, scope, recipients, effects, and bounds.
A commercial delegation's `order` is the exact MKT OrderRef; otherwise null.
It must name the LAB worker when LAB applies.

Executions is an array of `{proposal, request, run, session}`. Proposal is an
exact COORD proposal ArtifactRef or null; request is an exact CJ execution
request EventRef or null; run is an ArtifactRef to an authenticated RUN root
record with seq zero or null; session is an exact SESS
`openagents.session-admission.v1` ArtifactRef or null. At least one execution reference must be
present per entry. All supplied records must resolve, agree on the task,
delegate and input identities, and remain under current disclosure rules.
A request, result, or run attributed only by display name is insufficient.
For remote work the CJ request signer and designated worker must validate;
LAB linkage still owns the exact order-to-attempt binding.

The delegation is prepared against an existing work revision, then attached
by a replace command. A newly attached delegation's basis must equal that
command's expected revision; its work, grant, frame and delegate must agree
with the proposed snapshot and current assignment admission. An unchanged
historical delegation can remain linked as historical evidence, but cannot
be used to authorize a different frame or executor. Later execution refs require a new signed delegation
and item revision. A declaration alone does not dispatch work or prove the
agent accepted it. Cancellation/reassignment must revoke future use of the
old grant, stop or reconcile active attempts through their owning controls,
and preserve pending/unknown effects. Removing the delegation pointer does
not prove the previous executor stopped and cannot release its resources or
reservations. Never reassign concurrent effect authority solely because an
issue's delegate changed.

## Evidence, verification, and disposition

An evidence link has `v: "openagents.work-evidence.v1"`, `issuer`, `work`,
`basis`, `type`, `artifact`, and `sources`. Issuer is its authenticated author;
basis is the exact work revision to which it applies; artifact is an exact
ArtifactRef; sources is an array of source-observation ArtifactRefs. Type is
`progress`, `plan`, `execution_result`, `deliverable`, `verification`,
`integration`, `review`, `commercial`, or `decision`. Its schema must match
the type or an explicitly supported domain profile. All referenced records
must name the relevant task, source, output, actor, and version where their
own contract requires them. Prose called a verification is not a verifier
result. Provider output, tests, reviewer judgment, destination integration,
and buyer acceptance remain independently inspectable records.

An evidence author may propose a link. Only normal mutation admission can
attach it to an item. A work evidence link attributes a claim; it does not
endorse the underlying claim or turn an untrusted signer into a trusted
reviewer. Corrections use new evidence and retained source relations rather
than overwriting the old record. Retention or redaction reports missing bytes
honestly; a hash alone cannot replace unavailable evidence.

A disposition has `v: "openagents.work-disposition.v1"`, `issuer`, `work`,
`basis`, `outcome`, `evidence`, `successor`, and `reason`. Issuer must have
current disposition authority for that item; basis is its exact current
revision when the disposition is submitted. Outcome is `accepted`, `rejected`,
`revise`, `cancelled`, `superseded`, `reopened`, or `archived`. Evidence is an
array of evidence-link ArtifactRefs; successor is a WorkRef exactly for
superseded, otherwise null; reason is a nonempty descriptive ArtifactRef.
Acceptance requires nonempty evidence and the pinned acceptance-policy checks.
A rejection or unresolved verification must remain visible rather than being
rendered as a successful result.

| Requested state | Required disposition and boundary |
| --- | --- |
| `open`, `in_progress`, `blocked` | From a nonterminal state, no disposition needed for a workflow change. Clearing an existing disposition requires a new `revise` or `reopened` disposition. This state is a tracking claim, not proof of a running process. |
| `resolved` | `accepted` or `rejected` under the pinned policy. The outcome must be displayed; resolved alone never means success. |
| `cancelled` | `cancelled`; cancel future tracking/execution intent through the owning authorities and retain unresolved prior effects. |
| `superseded` | `superseded` plus the matching successor relation. No transfer of success, debt, or authority. |
| `archived` | `archived`; prior state must already be resolved, cancelled, or superseded. Archiving hides a view, not obligations or evidence. |
| Reopening a terminal item | `reopened`, state `open`; the old result/verification/acceptance/payment records remain unchanged. |

Changing frame or acceptance requirements while retaining prior accepted
evidence must mark that evidence as historical through its exact basis.
A change to frame or acceptance on a terminal item requires reopening it,
with state `open` and a new `reopened` disposition. It cannot retain a
terminal accepted claim under changed requirements. The changed item cannot
claim a new accepted outcome until independently accepted under those
requirements. An active LAB order remains frozen;
tracked edits do not amend it.

WORK acceptance concerns a tracked objective. For commercial labor, only the
exact LAB acceptance under the bilateral terms creates the MKT payment basis.
A WORK disposer who is not the LAB buyer/resolver cannot create commercial
acceptance. Conversely payment or a completed LAB order does not authorize a
merge, publish an artifact, or close a broader project. The WORK snapshot may
link those exact signed records, but it cannot synthesize or replace them.

## External systems and imports

A source observation has `v: "openagents.work-source.v1"`, `issuer`,
`system`, `subject`, `adapter`, `observed_at`, `version`, `content`, and
`assurance`. Issuer is the observer pubkey; system and subject are nonempty
opaque identifiers, at most 512 UTF-8 bytes each; adapter is the exact
DefinitionRef of the admitted observer; observed_at is Unix seconds; version
is the provider's opaque version/precondition string or null; content is the
exact captured ArtifactRef; assurance is an exact `openagents.observation.v1` ArtifactRef. Its resource
scope/id must equal system/subject, adapter must match, captured_at must equal
observed_at, provider_revision must equal version, and content must equal the
retained content reference. Its consistency class records the actual read
guarantee; a byte hash cannot strengthen a live observation.
URLs may be locator hints in that artifact, not mutable identity replacements.

An import binds exact source bytes to a proposed native item through the
command's source. The host preserves a durable `(system, subject)` import map
within the scope and records whether the native item is a one-time copy or a
read-only mirror in its admitted policy. A repeat import cannot create an
unrelated duplicate silently or discard intervening native edits. Conflicting
source and native changes produce a proposal/conflict for explicit resolution.
An unavailable historical source becomes a named gap, not fabricated history.

A native work item can link to an external GitHub, NIP-34, Git, calendar, or
other object without becoming that object's authority. Use official
[NIP-34](../official/34.md) for Git collaboration where appropriate and Block
[NIP-GS](../block/NIP-GS.md) for its exact authorship contract. An external
status event, merge announcement, or webhook does not establish a verified
current ref or acceptance. Verify the original source and signer according to
the owning profile.

External writes require their own CAP operation, current grant, provider
precondition, idempotency, and effect receipt under the shared external-effect
contract. There is no atomic transaction across WORK and an external provider.
Persist an intent before dispatch and reconcile external success if the native
link/update fails. A timeout cannot become a fresh publish/merge/payment.
An imported command or signed webhook is input; an admitted AUTO rule may
consume it, but the import itself cannot execute it.

## Projections and attention

Use WS `openagents.projection-cut.v1` for source frontiers, audience, coverage,
consistency and reconnect behavior. A work projection pins its row schema and
policy; rows carry exact WorkRevisionRefs. It must preserve partial history,
staleness, omitted private rows, denied access, and unresolved forks without
exposing hidden item counts or details. A client can show an unconfirmed
proposal locally, but it must not render it as the accepted revision.

Use the WS read/page/delta/marker schemas directly. The work row schema is
`openagents.work-view-row.v1` with `work` (WorkRef) and `revision`
(WorkRevisionRef for the same work), in addition to common v/requires/meta.
Row keys are the WorkRef's authority, scope and item joined with `:`. The
projection pins this row schema and its declared selection; a transformed
board/attention view must use a different supported pinned row schema. The
cut's source frontiers refer to retained authority history, with ordering
specified by the source schema, not an arbitrary collection of latest events.
Complete means complete only for the admitted recipient and selection. Null
page continuation closes that finite enumeration; relay EOSE or an empty
response does not prove complete source history. A stale cursor cannot
authorize a mutation.

An attention record has `v: "openagents.work-attention.v1"`, `issuer`,
`recipient`, `work`, `basis`, `cause`, `reason`, and `expires_at`. Issuer is the
admitted projection producer; recipient is the reader pubkey; basis is a
WorkRevisionRef; cause is an exact source/evidence/session artifact; reason is
`assigned`, `answer_required`, `review_required`, `blocked`, `failed`,
`verification_ready`, `disposition_required`, or `stale`; expires_at is Unix
seconds or null. It is an audience-filtered derived hint. Deduplicate by the
full source identity, work, recipient and reason; duplicate relay deliveries
must not create more work or notification effects. External notification
sending still requires an admitted operation and any AUTO delivery contract.
Read/dismiss state is principal-owned projection state. Neither dismissal nor
expiry answers a question, accepts work, stops a job, or settles payment.

## Conformance and implementation status

Implementations declare their roles: reader, command client, native authority,
external adapter, attention producer, or execution/market linker. A reader is
not a mutating authority. Supporting these schemas does not imply a user
interface, native Git forge, general workflow engine, labor executor, wallet,
or public relay profile. Do not advertise a role before exercising its
admission, persistence, refusal, and recovery behavior.

Required role-appropriate fixtures include:

- Wrong signer, wrong scope, expired/revoked grant, and private source leaked
  into an unauthorized view all refuse without effects.
- Duplicate identical commands return the original accepted revision; changed
  bytes with the same key, stale expected revisions, and missing targets refuse.
- Crash after admission, persistence, external dispatch, and relay publication
  recovers known results or retains unknowns without duplicate effects.
- Parent/blocks cycles, self-links, unsupported resource kinds, and unresolved
  source identities cannot enter accepted state.
- Reassignment while an old attempt is active cannot grant conflicting work
  until the owning execution/resource authorities establish safety.
- Exact work revision, CTX task, LAB order, worker, submission, verification,
  acceptance and payment records remain distinguishable through close/reopen.
- A delivery, green check, issue close, community vote, signed terminal event,
  or paid invoice alone cannot be promoted into the other outcome classes.
- Forks, missing pages, withheld rows, duplicate notifications, cancellation,
  owner absence, and unavailable external state remain visible and bounded.

The first useful proof is one tracked issue through bounded delegated labor,
independent verification, explicit acceptance and retained payment evidence,
with two clients reading the same admitted history. Neither the archive that
motivated this contract nor this draft establishes that proof.
