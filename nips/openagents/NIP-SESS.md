# NIP-SESS — Engine sessions and turn control

`draft` `optional` — v1, 2026-09-26. The [shared contracts](contracts.md)
are normative. This profile gives clients a common session contract while
retaining the exact semantics and evidence of each admitted engine adapter.
It adds no event kinds. The read-only observer profile below has a bounded
implementation in `crates/coder-connect`; that does not implement managed
session opening, execution, or turn control.

A session is a durable conversation lineage, not a process, connection, task,
account, or RUN controller. One engine process can serve several sessions;
one session can acquire several successive engine attachments. Reaching an
engine through ACP, Codex app-server, or a CLI does not imply equal features.

## Relationships and transport

| Contract | Boundary |
| --- | --- |
| [CAP](NIP-CAP.md), [EXT](NIP-EXT.md) | Pin the operation, adapter, schemas, executable closure, supported effects, and host binding. A method catalog describes support; it grants nothing. |
| [CJ](NIP-CJ.md) | Carries authenticated execution requests and results for session operations. Its attempt identity and durable admission remain required. |
| [RUN](NIP-RUN.md), [COORD](NIP-COORD.md) | Own execution journals, budgets, child claims, cancellation, reconciliation, and fenced controller transfer. Session attachment does not elect a new controller. |
| [CTX](NIP-CTX.md) | Owns task frames, instructions in context, history hierarchy, compaction evidence, and source expansion. A session projection does not replace these. |
| [POL](NIP-POL.md), [CTRL](NIP-CTRL.md) | Own disclosure, exact approvals, and task-scoped client control. An observer or steering client cannot create sessions, submit paid turns, or approve effects without separate admission. |
| [ENV](NIP-ENV.md), [WS](NIP-WS.md) | ENV establishes an execution attachment; WS supplies versioned workspace/resource references and bounded projection synchronization. Neither a ready environment nor a live view admits a turn. |
| Block [AO](../block/NIP-AO.md), [AM](../block/NIP-AM.md), [AE](../block/NIP-AE.md) | Telemetry, turn metrics, and agent memory remain separate records. Their presence is not complete session history, native resume support, or authority to answer a pending request. |

Artifacts use the private `3188` envelope or durable host-authenticated local
storage. CAP operations advertise exact input/output SchemaRefs for adapter
inspection, session opening, session commands, reads, and interaction responses.
Remote invocation uses CJ execution v1. A CJ `completed` result means that
operation answered, not that the requested turn ran or passed verification.

Every body below has `v`, `requires: []`, and optional inert `meta`, plus
exactly its stated fields. IDs use the common random 64-hex form; pubkeys,
timestamps, bounds, references, and refusal codes use the shared contracts.
Opaque engine identifiers are nonempty UTF-8 strings of at most 1,024 bytes.
Arrays are duplicate-free where they name identities and contain at most 256
items. Semantic strings not otherwise bounded are at most 1,024 UTF-8 bytes.
Content too large for an artifact must be paged under a supported schema;
silent truncation is forbidden.

## Adapter support and effective configuration

An adapter description is `openagents.session-adapter.v1` with `adapter`
(DefinitionRef), `engine` (ArtifactRef identifying the exact admitted engine
build), `protocol` (SchemaRef), `features`, and `compatibility` (ArtifactRef).
The compatibility artifact binds supported engine/schema identities and
verification evidence, not an unbounded claim to support future versions.

`features` contains exactly one entry for each of `create`, `resume`, `fork`,
`import`, `close`, `configure`, `submit`, `queue`, `reorder`, `steer`,
`interrupt`, `history`, `questions`, and `approvals`. Each entry is
`{feature, support, operation, semantics, evidence, limitations}`:

- `support` is `native`, `emulated`, or `unsupported`.
- Supported entries require an operation DefinitionRef, semantics SchemaRef,
  and an evidence ArtifactRef; unsupported entries have all three null.
- `limitations` is a list of bounded inert strings. Emulation MUST name its
  actual behavior in the pinned semantics; it cannot borrow the native claim.

The queue feature includes `enqueue` and `cancel_queued`; reorder is separately
advertised. Archive is host-owned metadata under separate authorization and
never claims a native engine feature. All remaining actions map to their named
feature. Reads use the WS projection-read schema and separate observation rights.

Engine-advertised support, adapter handling, host authorization, and current
readiness are separate checks. Every optional upstream method must be gated
by the negotiated peer capability and adapter implementation. A typed schema
containing a method is not proof that the adapter handles it. Unknown required
notifications or reverse requests cause an inspectable protocol fault and
block affected dispatch; do not silently discard them and remain live.

A configuration artifact is `openagents.session-configuration.v1` with
`adapter` (DefinitionRef), `recipient` (POL recipient ArtifactRef), `model`
(ArtifactRef identifying the requested or reported model), `settings_schema`
(SchemaRef), `settings` (ArtifactRef valid under that schema), `policy`
(ArtifactRef), `enforcement` (ArtifactRef or null), and `bounds` (common bounds).
The host validates every field against current configuration and admission.
Requested configuration may have null enforcement; effective configuration
requires the retained enforcement plan. Requested and effective artifacts are
both retained. No unsupported setting is silently dropped, defaulted to a
different mode, or advertised as enforced. Credentials remain in host stores.

## Opening, identity, and attachment

An open request is `openagents.session-open.v1` with `command`, `session`
(new common ID), `owner`, `authority` (pubkeys), `workspace` (WS workspace
ArtifactRef or null), `lease`, `materialization` (existing ENV ArtifactRefs),
`adapter` (adapter-description ArtifactRef), `requested` (configuration ArtifactRef),
`authorization` (ArtifactRef), `source`, `issued_at`, and `expires_at`.

`source` is exactly `{kind, artifact}`. Kind is `new`, `resume`, `fork`, or
`import`; new requires null artifact, and every other kind requires an exact
history-export ArtifactRef defined below. Resume opens a new host session
binding to a retained lineage; a reconnect to an existing binding instead
reads that session. Fork creates an independent lineage. The host checks
source rights, actual engine support, compatibility, and current ENV lease
and materialization. An owner field or exported history cannot establish that
authorization. Construct and sign the exact CJ open request before reserving
its ENV execution attachment. The immutable open request never references its
own future attachment; the ENV binding must be active before dispatch.

The host persists a session admission before acknowledging it. Its schema is
`openagents.session-admission.v1` with `request` (ArtifactRef), `session`,
`lineage` (common ID), `owner`, `authority`, `generation` (zero), `revision`
(zero), `attachment`, `adapter`, `requested`, `effective` (ArtifactRefs),
`source` (the open source object), `authorization`, and `admitted_at`.
New, fork, and import use a new lineage ID. Fork/import retain their exact
source lineage as provenance. Resume preserves the source lineage only with
separately verified source-engine support, exclusive native-session admission,
and any required RUN transfer. It does not impersonate the source controller.
Admission attachment is evidence of the opening operation, not a continuing
grant for later turns. The lease and materialization remain pinned through
the retained request.

Only a host independently authorized by the owner can establish this binding.
Opening a second binding is not authority to drive the same native session
concurrently. The host must fence or refuse overlapping native attachments.
When exclusivity cannot be established, imported content is read-only and the
host must refuse execution through it. ENV leases and RUN dispatch fencing
remain necessary where applicable.

A scope is exactly `{authority, session, generation}`. Generation increments
on replacing the active runtime materialization or native session binding;
session revision increases for each admitted canonical transition. Native
connection and session IDs remain private adapter references, not portable
session IDs. Reconnect does not
change generation when the same runtime and native session binding can be
verified. A replacement requires fresh ENV admission and reconciliation of
active/unknown work before the host can mark the session ready. A changed
controller uses RUN handoff, not a local generation increment to evade fencing.

An ENV attachment binds one exact CJ execution, including its attempt and
fingerprint. New runtime execution and promoted turns require their own
currently active ENV bindings. Drain and reconcile the preceding attachment
before reusing its lease slot. This operation-level lifecycle does not itself
change the persistent session generation. The host cannot treat the opening
attachment or an idle engine as permission for a queued turn.

Separately admitted host reads, queue changes, lifecycle operations, and
steer/interrupt control do not consume a second compute slot. An engine
control signal targets the exact active turn and attachment, under current
authority and the declared adapter semantics. It can narrow or stop existing
work but cannot create another execution, recipient, effect scope, or budget.
A permitted steering supplement remains explicit SESS/POL evidence; it does
not rewrite the original signed CJ request. Applying it requires that the
active execution binding remains valid.

## Commands and queue law

A command is `openagents.session-command.v1` with `command`, `scope`,
`expected_revision`, `authorization` (ArtifactRef), `issued_at`, `expires_at`,
`action`, and `payload`. The authenticated author is the actor; a forwarded
command retains its original signed declaration. The action selects exactly
one closed payload:

| Action | Payload |
| --- | --- |
| `submit` | `{intent: ArtifactRef}` |
| `enqueue` | `{intent: ArtifactRef}` |
| `cancel_queued` | `{intent: common ID}` |
| `reorder` | `{intents: ordered common IDs}` |
| `steer` | `{turn: common ID, expected_turn_revision: integer, correction: ArtifactRef}` |
| `interrupt` | `{turn: common ID, expected_turn_revision: integer, reason: inert string}` |
| `configure` | `{requested: configuration ArtifactRef}` |
| `archive` | `{archived: boolean}` |
| `close` | `{reason: inert string}` |

An intent is `openagents.turn-intent.v1` with `intent` (common ID),
`task_frame`, `input`, `context`, `configuration`, `authorization` (ArtifactRefs), `bounds`, and
`expires_at`. It binds the complete proposed input, attachments, task version,
recipient, settings, and budget. Source content is validated and retained
under disclosure policy before queue admission. Queue admission reserves only
what its receipt says; dispatch must perform all current budget checks.

Command idempotency is `(authority, session, command)` with fingerprint
SHA-256 of JCS of the complete command or open request. Open requests use
their requested session ID in this key. Persist the fingerprint, transition,
queue state, and any effect intent atomically before acknowledging acceptance.
Identical retries return the recorded receipt; different content is
`idempotency_conflict`. A transport timeout requires status reconciliation,
not a new command ID for the same uncertain effect.

The authority checks live rights, generation, revision, expiry, schema and
adapter support before each admission. Concurrent commands with a stale
revision conflict; timestamp order and last-writer-wins cannot resolve intent.
Authorized idempotent replay can retrieve an existing result after command
expiry, but cannot disclose it to a now-revoked reader or dispatch again.
Retain command tombstones for the admitted recovery horizon. Absence after
that horizon is unknown, not evidence that it is safe to resubmit.

`submit` admits only against a verified idle session with no earlier queued
intent; otherwise return conflict. `enqueue` appends to one durable FIFO. Its
states are `queued`, `promoted`, `cancelled`, `expired`, and `refused`.
Promotion creates exactly one recorded turn identity and references its RUN
admission. A single authority promotes only the current head after the prior
turn's quiescence is established. Revalidate authorization, source freshness,
configuration, expiry, and shared reservations at promotion. A stale item
becomes refused, not silently edited. Reorder names the exact remaining queue
permutation at the expected revision; promoted work is never reorderable.

An adapter acknowledgment of interruption, silence, transport closure, or an
idle timeout does not establish quiescence. Quiescence needs authoritative
terminal engine evidence and disposition of managed child work; unresolved
effects remain unknown under RUN. Unknown active work blocks automatic next
promotion unless a separately admitted isolation policy proves it cannot
conflict or spend the same reservation.

Steering names one active turn and its revision. A CTX correction is admitted
under instruction policy and translated only through the adapter's declared
steer semantics. Starting another turn, enqueueing, or killing/restarting a
process is not native steering. The host refuses unsupported steering unless
the caller explicitly chose the pinned emulated operation. Task-level CTRL
steering is a frame correction; it does not prove that an engine consumed it.

Interrupt asks the engine to stop the exact turn. Its receipt records the
request, followed separately by observed stop, failure, or unknown state.
Archive affects discoverability only. Close prevents new turn admissions and
requests cleanup; it cannot report stopped while owned processes or effects
remain unresolved. Configure is admitted only while idle with an empty queue
in this profile; changing the configuration of queued work requires new intent.

## Results, state, and pending interactions

A control result is `openagents.session-control-result.v1` with `command`
(ArtifactRef), `status` (`accepted`, `duplicate`, `conflict`, `refused`, or
`unknown`), `receipt` (ArtifactRef or null), and `reason` (common code or null).
Accepted/duplicate require the retained receipt; conflict/refused require a
reason and null receipt. Unknown may carry the last retained receipt but cannot
claim acceptance or stop beyond its evidence.

A control receipt is `openagents.session-control-receipt.v1` with `command`
(ArtifactRef), `scope`, `before_revision`, `after_revision`, `transition`
(one of the command actions or `open`), `admitted_at`, and `records` (RUN
record ArtifactRefs). It proves host admission, not execution completion.
Open has both revisions zero; other accepted mutations advance revision by
one. A duplicate returns the identical original receipt. Receipts do not
automatically disclose transcript, workspace paths, or other pending inputs.

A turn result is `openagents.session-turn-result.v1` with `scope`, `turn`
(common ID), `intent` (ArtifactRef), `run` (RUN reference ArtifactRef),
`outcome` (common outcome), `dispatched` (boolean), `cause` (`normal`,
`user_interrupt`, `budget`, `deadline`, `provider_refusal`, `provider_failure`,
`hook_stop`, `policy`, `protocol_fault`, or `lost_observation`), `outputs`
(ArtifactRefs), `receipt` (execution-receipt ArtifactRef), `verification`,
`integration` (common enums), and `quiescence` (ArtifactRef or null).
Completed means the admitted turn operation finished; it does not mean the
user's task succeeded. A model finish marker, hook stop, or budget stop is
not verification. The receipt and result must agree on the observed effect
and outcome. Quiescence is a separately supported evidence artifact proving
the prior turn cannot keep dispatching in this attachment; null cannot promote
the next queued intent. Retain partial outputs and unknown usage on failures.

A state artifact is `openagents.session-state.v1` with `admission`
(ArtifactRef), `scope`, `revision`, `effective` (configuration ArtifactRef),
`phase` (`attaching`, `ready`, `running`, `blocked`, `interrupting`, `closing`,
`closed`, `unknown`, or `faulted`), `active_turn` (common ID or null),
`turn_revision` (integer or null), `queue` (ordered intent/state ArtifactRefs),
`interactions` (ArtifactRefs), `history` (history-export ArtifactRef),
`children` (child-session ArtifactRefs), and `archived`
(boolean). Active turn and turn revision are both null or both non-null.
Each queued state artifact is `openagents.queued-intent.v1` with `intent`
(ArtifactRef), `state` (queue enum), `turn` (common ID or null), and `receipt`
(ArtifactRef); only promoted has a non-null turn. State is derived from
retained receipts and engine observations, never inferred from mounted UI.

A child-session artifact is `openagents.session-child.v1` with `parent`
(scope), `turn` (common ID), `child` (session-admission ArtifactRef or null),
`native_source` (native-envelope ArtifactRef or null), `relationship`
(`managed` or `observed`), and `authorization` (ArtifactRef or null).
Managed requires a child admission and current delegated authorization.
Observed requires native source evidence and grants no control, including
when the child cannot be mapped to an admitted session. An unknown parent
or orphaned native child is retained as explicit history loss, not flattened
into a false parent/child relationship.

An interaction is `openagents.session-interaction.v1` with `interaction`
(common ID), `scope`, `turn`, `turn_revision`, `native_request`
(private native-envelope ArtifactRef), `kind` (`question` or `approval`),
`recipient` (pubkey), `body` (ArtifactRef), `expires_at`, and `state`
(`pending`, `answered`, `denied`, `expired`, `cancelled`, or `unknown`).
A question body is `openagents.session-question.v1` with `prompt`
(ArtifactRef) and `answer_schema` (SchemaRef). Approval bodies are exact POL
approval-request artifacts. Both use pinned body schemas. Secret inputs
require a separately admitted secret broker;
ordinary question answers cannot carry credentials.

A response is `openagents.session-interaction-response.v1` with `interaction`
(ArtifactRef), `scope`, `responder` (pubkey), `response` (ArtifactRef), and
`expires_at`. The signer must be the independently admitted recipient and
the exact pending request/generation must still be current. An approval
response is a POL decision from the actual approver; general session control
does not supply approval authority. A question answer is data validated by
its answer schema, never permission for an effect by implication.

One host arbiter durably claims the first authorized response before replying
upstream. Duplicate identical responses retrieve its record; conflicting or
late ones refuse. Persist delivery intent and retain the method-correct native
response. If the engine response was sent but not confirmed, report unknown;
do not deliver again unless its pinned adapter supports idempotent resolution.
Unknown native reverse methods receive a method-correct unsupported/denied
response when possible and an incompatibility record, never a guessed success
shape. Browser prompts and model text cannot appoint an approver.

## Native history, import, and projection loss

A native envelope is `openagents.session-native.v1` with `scope`, `adapter`
(DefinitionRef), `engine` (ArtifactRef), `protocol` (SchemaRef), `connection`
(common ID), `direction` (`to_engine` or `from_engine`), `ordinal` (integer),
`method` (bounded string), `native_id` (opaque string or null), `captured_at`,
`payload` (ArtifactRef or null), and `capture` (`complete`, `redacted`,
`unavailable`, or `unsupported`). Ordinal orders captures within that
connection, not all engine history. Null payload requires non-complete capture.
Private diagnostic capture is bounded and subject to retention/disclosure;
do not put secrets or raw native bodies into public logs or portable summaries.

A history export is `openagents.session-history.v1` with `lineage`, `source`
(the tagged source object below), `adapter` (DefinitionRef), `engine`
(ArtifactRef or null), `cut` (WS projection-cut ArtifactRef or null),
`native` (ordered native envelope ArtifactRefs), `portable` (CTX history or
supported ATIF ArtifactRef),
`losses`, `resume` (ArtifactRef or null), and `coverage` (`complete`, `partial`,
or `unknown`). Each loss is `{source: ArtifactRef or null, category, reason}`;
category is `omitted`, `redacted`, `unsupported`, `unavailable`, or `derived`.
Derived portable views retain provenance. Full native capture must precede
normalization where allowed; otherwise report its loss explicitly.

Source is exactly one of `{kind: "managed", admission: ArtifactRef}` or
`{kind: "foreign", import: ArtifactRef}`. Managed requires the exact original
session admission and a non-null cut. A foreign import uses
`openagents.session-import-source.v1` with `content` (exact retained source
ArtifactRef), `format` (SchemaRef), `importer` (DefinitionRef), `engine`
(ArtifactRef or null), `native_session` (opaque ID or null), `captured_at`,
`authorization` (ArtifactRef), and `coverage` (`complete`, `partial`, or
`unknown`). It can represent retained Claude/Codex files or another approved
foreign source without inventing an OpenAgents admission. Importer output
records parser/schema version, unmapped item types, missing parents, truncation,
and unavailable source portions as losses. Unknown engine identity remains
null. A foreign lineage ID is a new host-assigned archival identity, not a
claim that the source used that ID. A foreign export without a WS view has
null cut; exact source bytes establish its bounded archival cut. Foreign
import does not establish native resume support.

The resume artifact uses a pinned adapter schema and contains opaque resume
state, never exported provider credentials. A supported engine resume still
needs a fresh attachment and current authority. Importing display text cannot
claim native continuation. Forks bind the exact source cut and make independent
future identity; importing history or compaction never recreates old authority,
approvals, live tools, paid reservations, or child control. Provider-owned child
sessions can be observed only under source admission; they are not managed
COORD children merely because the adapter reports a parent edge.

Use WS snapshot/delta synchronization for shell, session, and detail views.
Connection health, restored history, projection catch-up, engine readiness,
and active execution are separate facts. An idle replay heuristic cannot mark
history complete. Unknown item variants remain inspectable private evidence
or explicit loss; they cannot disappear from a claimed complete transcript.

## Read-only observation of retained foreign history

`openagents.history-observer.v1` is an optional, separately admitted profile
for reading local retained engine-history projections. It is not an engine
session admission, a CTRL task scope, or a WS synchronized projection. It
cannot create, resume, submit, steer, approve, interrupt, archive, or delete
engine work. In particular, a CTRL grant for one task MUST NOT authorize
enumerating unrelated retained chats.

This profile is a narrow exception to the CJ invocation requirement above:
finite read requests and their replies travel as original signed private
`3188` artifacts. No operation executes engine work, no new kind is allocated,
and no generic CAP/CJ execution support is advertised. A host that needs
mutation or managed-session semantics uses separately admitted contracts.

### Local pairing and source authority

The client creates its own key in its local protected store. It can supply its
public key to the operator or redeem the short-lived computer invitation below.
Local owner admission creates a distinct host key and a grant binding that
client to explicitly selected source roots. The
host stores canonical roots and their filesystem identity privately. A display
label, relay membership, possession of a connection code, or guessed file
identifier is not source admission. A host MUST NOT discover additional roots
from an incoming read. Changing a root, recipient, relay, or disclosure scope
requires a new grant.

A grant is `openagents.history-observer-grant.v1` with `requires: []`, `grant`
(common ID), `host`, `client` (distinct pubkeys), `relay` (the exact admitted
URL), `sources` (1–2 `{id, label, kind}` objects), `issued_at`, and `expires_at`.
Source IDs use common IDs; labels are inert strings of 1–128 bytes; the initial
kinds are `codex` and `claude`, each occurring at most once. Expiry is later
than issuance and at most 30 days later. The local operator's explicit pairing
action is the source-disclosure authorization, not a claim about the original
engine's owner or controller. The original host-signed grant is encrypted to
the client and retained before the connection code is returned.

The public connection code is `openagents.history-observer-connection.v1`
with `requires: []`, `host`, `client`, `relay`, `grant`, `sources`, `expires_at`,
and `authorization` (the exact encrypted grant event). It contains no key,
credential, source path, or transcript. Its source list describes admitted
collections, not all source-file IDs. The operator transfers this code through
an independently trusted channel. The client pins its host and exact grant,
decrypts the original event with its own key, and requires all duplicated
fields to agree. A valid self-signed code from another host cannot replace a
previously trusted connection without explicit local pairing.

Production uses `wss` with certificate validation and no URL credentials or
fragment. Plain `ws` is permitted only for an explicitly enabled loopback
test profile, never enabled by a field supplied by the remote code. Neither
client nor host follows remote artifact locators, redirects, or source paths.

### Computer invitation bootstrap

A computer can admit source roots before it knows the phone's key. Its explicit
local `connect` action displays the selected canonical roots and a five-minute
single-use invitation, both as a QR code and as an equivalent paste string. This
invitation is a temporary disclosure capability. Anyone who can see it can
attempt to become its one admitted device, so it MUST NOT appear in public
logs, telemetry, or a remote QR-generation service. It contains no identity
secret, filesystem path, or transcript. Local QR files use private permissions
and are removed after redemption, expiry, or command exit.

The string is `coder-pair:` followed by unpadded base64url of these bytes, in
order: version byte `1`; host x-only public key (32 bytes); invitation ID
(32 random bytes); capability (32 independently random bytes); issued time
(8-byte unsigned big-endian seconds); expiry (8-byte unsigned big-endian
seconds); relay URL byte length (2-byte unsigned big-endian); and the exact
UTF-8 relay URL (1–256 bytes). No trailing bytes are allowed. Encoded strings
are at most 640 bytes. Times use common safe integers, expiry is exactly 300
seconds after issuance, and relay policy remains local. A scanned string
cannot enable the loopback-test policy.

Before displaying an invitation, the host durably stores its ID, capability
digest, exact relay, issuance and expiry, original canonical source identities,
and operator-selected grant expiry. It stores no recoverable capability in the
admission book. At most 64 invitations are retained. The host MUST NOT substitute
new roots at redemption or create a grant before committing consumption.

A redemption body is `openagents.history-observer-pair-request.v1` with
`requires: []`, `request`, `invitation` (common IDs), `capability` (lower-case
32-byte hex), `relay`, `issued_at`, and `expires_at`. It is an original signed
private `3188` artifact from the phone to the pinned host. Its mailbox is the
request ID; body/envelope lifetimes agree and remain within the invitation and
the ordinary 60-second request bound. NIP-42 proves the publishing identity to
the relay; the original signature and encrypted capability prove redemption to
the host. Relay membership alone cannot redeem an invitation.

Under one durable local lock, the host checks the original signature, distinct
phone/host keys, capability digest, relay, time window, cancellation, and source
identity. It binds the first valid redemption to that phone and atomically
stores both consumption and the ordinary read-only grant before replying.
Same-phone retries with the same capability retrieve that grant while the
invitation and grant remain current. Different-phone reuse refuses, including
after process restart. Revoking the resulting grant also prevents bootstrap
retries from disclosing it; retries never mint a replacement grant. Interrupted
or uncertain persistence returns no success. Unused invitations can be cancelled
locally and cannot authorize anything after expiry.

A response is `openagents.history-observer-pair-reply.v1` with `requires: []`,
`request`, `request_event`, `invitation`, `issued_at`, `expires_at`, and `result`.
Result is exactly `{status: "ok", connection}` with the ordinary connection
code, or `{status: "refused", code}`. The original signer is the QR-pinned host,
recipient the requesting phone, and mailbox the request ID. The client checks
the exact request-event correlation and validates the encrypted grant and all
connection fields before saving anything. A failure preserves an existing
working connection. The host bounds newly admitted bootstrap replies to 32 per
invitation; exact signed-request retries return retained replies without using
another slot. Bootstrap is read-only source admission, never CTRL or engine
authority, and adds no event kind or public discovery record.

Required bootstrap fixtures include successful encrypted redemption and first
read, wrong proof/host/recipient, malformed or expired code, another-device
reuse, same-device retry after reopening the store, cancellation, revocation,
source replacement, concurrent redemption, and persistence failure. A local
synthetic relay proves protocol behavior, not physical camera scanning.

### Bounded request and reply

A request is `openagents.history-observer-request.v1` with `requires: []`,
`request` (common ID), `grant` (grant ID), `authorization` (exact original grant
event ID), `issued_at`, `expires_at`, and `query`. Its encrypted recipient is
the pinned host and original signer is the granted client. Query is exactly
`{kind: "catalog", request: CatalogRequest}` or
`{kind: "page", request: TranscriptRequest}` under the versioned reader DTO
contract in `crates/coder-history`. Catalog requests have a bounded cursor and
limit; transcript requests name only an opaque reader-issued `source_id`,
cursor, and byte limit. Requests carry no host path or executable content.

The host verifies signature, recipient, schema, current grant, original grant
event identity, selected source roots, and request freshness before reading.
Request expiry is within the grant and at most 60 seconds after issue. The
envelope issue/retention fields agree with request issue/expiry; its opaque
`h` mailbox equals the request ID. The host checks actual root identity at each
read. Replacement or loss requires an explicit refusal, not an automatic bind
to a new directory. Per grant, allow at most 240 newly admitted reads per
60-second window. An exact request retry is not another newly admitted read.

A reply is `openagents.history-observer-reply.v1` with `requires: []`,
`request`, `request_event` (exact original event ID), `grant`, `issued_at`,
`expires_at`, and `result`. Result is exactly one of
`{status: "ok", observation}` or `{status: "refused", code}`. Observation is
`{kind: "catalog", page: CatalogPage}` or
`{kind: "page", page: TranscriptPage}` under the reader DTO contract. The
variant must match the query. The original signer is the pinned host, encrypted
recipient the granted client, and mailbox the request ID. Expiry is no later
than the request expiry. No response can substitute for another request even
when the source, query, or visible text happens to match.

Only bounded inline JSON is supported. Observer bodies are at most 128 KiB;
catalogs have at most 32 entries and transcript pages at most 32 KiB of raw
source bytes within the reader's 112 KiB encoded-page bound. Exact private
envelope and NIP-44 bounds also apply. Oversize, unsupported, partial,
unavailable, and changed-source cases cannot be silently labeled complete.
Stable refusal codes are `revoked`, `expired`, `source_changed`, `unavailable`,
`rate_limited`, `malformed`, `forbidden`, `unsupported`, `bounds`, and
`conflict`. Local transport errors are distinct from signed refusal replies.

The host retains the exact request-event binding and signed reply through its
request lifetime. Equal request IDs with different event bytes conflict;
identical retries return retained bytes only while the grant is current.
Expired requests cannot trigger a new read. A reconnect may renew a finite
subscription and resend the original signed request while it remains fresh.
Polling with a new request ID makes a new bounded observation; it does not
claim a continuous or atomic WS cut. Clients retain each source's reader
cursor and show capture time, explicit losses, and disconnected/cache state.
Host reply issue time is not the engine's record timestamp. A client that only
retains receipt time MUST label it as received or checked, not source capture.
Clients MUST validate raw byte bounds, record IDs and offsets, newline
completion, and next-record cursors before merging transcript chunks into a
cache. A valid host signature does not make an inconsistent reader page valid.

### Expiry, revocation, and disclosure limits

The operator can durably revoke the whole grant or any admitted source. Either
action ends the existing grant: partial source changes require a new grant.
Persist the terminal revocation before acknowledging it. Keep its tombstone
through at least grant expiry plus the maximum request window, and never
reactivate that grant ID. Each subsequent read, including a cached retry,
checks current local authority. Failure to read the grant store refuses.

Signed grant verification offline establishes identity and its declared
lifetime only. It cannot establish that the host has not revoked access.
Expiry or an authenticated revocation refusal stops further disclosure and
marks cached content according to the client's retention policy. Revocation
cannot erase bytes previously disclosed or recall an already admitted reply
in flight. Relay `OK`, EOSE, silence, or successful decryption never proves
that the source history is complete, current, or actively executing.

Required observer fixtures cover wrong host/client/signature, stale responses,
request mismatch, read after expiry/revocation, changed roots/cursors, bounded
paging, rate limits, strict relay destinations, concurrent local administration,
atomic failure/reopen, exact retries, and NIP-42 authenticated encrypted relay
exchange. A transport fixture publishes synthetic content only and is not
evidence that a production relay retains or enforces these artifacts.

## Conformance

Required fixtures cover native versus emulated features, schema/binary drift,
unadvertised reverse requests, changed configuration, concurrent open of one
native session, stale attachment generation, child-session separation, and
requested versus effective settings. Test queue admission and promotion crash
boundaries, reordered/duplicate commands, changed payload under one ID, expired
queued work, stop without quiescence, and a lost acknowledgment after dispatch.

Test competing question/approval responses, revoked recipients, request expiry,
wrong POL subjects, secret-shaped inputs, unknown native requests, and a crash
between response intent and native resolution. Replay tests must preserve
unprojected items, exact source cuts, explicit redaction, historical losses,
cache age, and unavailable resume state. An implementation advertises only the
features and engine identities for which these paths are supported; generated
types and a successful chat turn do not establish complete adapter support.
