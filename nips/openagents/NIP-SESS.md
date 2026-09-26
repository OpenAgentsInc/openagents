# NIP-SESS — Engine sessions and turn control

`draft` `optional` — v1, 2026-09-26. The [shared contracts](contracts.md)
are normative. This profile gives clients a common session contract while
retaining the exact semantics and evidence of each admitted engine adapter.
It adds no event kinds and claims no current Coder implementation.

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
