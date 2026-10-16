# NIP-SESS — Engine Sessions

`draft` `optional` — v1, 2026-09-26. The [shared contracts](contracts.md)
are normative. This profile gives clients a common session contract while
retaining the exact semantics and evidence of each admitted engine adapter.
It adds no event kinds. The read-only observer profile below has a bounded
implementation in `crates/coder-connect`. Managed sessions (opening, command
and queue rules, turn control, interactions, history export/import, and child
linkage) are implemented separately in
[`session-host`](../../crates/session-host/README.md) for in-process Rust
adapters; see the [Rust managed-session profile](#rust-managed-session-profile).

A session is a durable conversation lineage, not a process, connection, task,
account, or RUN controller. One engine process can serve several sessions;
one session can acquire several successive engine attachments. Reaching an
engine through ACP, Codex app-server, or a CLI does not imply equal features.

Test-time capabilities: the steering rows for delegate engines and the ATIF `portable` history carry part of [test-time delegation](../../docs/essays/2026-09-29-test-time-capabilities.md#6-test-time-delegation) ([mapping](../../docs/essays/2026-09-29-test-time-capabilities.md#how-the-protocol-carries-test-time-capabilities)).

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

### Steering capability

The `steer` feature entry's semantics name three facts, because engines
steer differently and an accepted steer is not a consumed one:

- **Native mode**: `mid_turn` (a running turn takes the message),
  `turn_boundary` (the engine takes new input only when a turn ends), or
  `unsupported`.
- **Emulation**: `cancel_and_continue` (stop the running turn, then start a
  new turn that carries the message), or none. A host runs emulation only
  when the caller chose it; support for it never upgrades a native claim.
- **Acknowledgment**: the evidence that the engine consumed the steer:
  `engine_replay` (the engine repeats the message in its own stream),
  `turn_answer` (the engine answers the steer for the exact expected turn),
  `next_turn_start` (the turn that carries the message starts, and its
  admission records the revision it read), `input_written` (the message was
  written to the engine's input: acceptance only), or `none`.

A host refuses native steering of a running turn when the mode is not
`mid_turn`, and refuses emulation the adapter lacks. When the named turn
ended before the steer reached it, the steer is re-dispatched as a new turn
on an engine that can start one, because nothing consumed it. The host
records consumption as its own ATIF step, separate from the steer's
acceptance, and keeps a ledger of routed steers until one is consumed.

Known engine behavior, as adapter capability rows:

| Engine surface | Native mode | Acknowledgment | Notes |
| --- | --- | --- | --- |
| Claude Code, `claude -p --input-format stream-json` | `mid_turn` | `engine_replay` with `--replay-user-messages`; otherwise `input_written` | A steer is a stdin user line. Send it with `priority: "now"` only when no tool is open, and `"next"` while a tool runs, because `now` aborts in-flight MCP calls. |
| Codex app-server | `mid_turn` | `turn_answer` | `turn/steer` names `expectedTurnId`. After the turn ends, the message becomes the next `turn/start`. |
| Codex, `codex exec` | `turn_boundary` | `next_turn_start` | Reads one prompt and closes its input; a new turn is `codex exec resume`. |
| ACP agents | `mid_turn` only with an advertised steering extension; otherwise `turn_boundary` | Per the extension; otherwise `next_turn_start` | Without the extension, emulation is cancel and re-prompt at a step boundary, only when chosen. |
| Microcoder (`microcoder-repository`) | `turn_boundary` | `next_turn_start` | A run reads its instructions once, at admission. |
| Devin CLI, `devin acp` (a repository run on a `devin` route) | `turn_boundary` | `next_turn_start` | ACP v1 takes one `session/prompt` at a time and Devin 3000.11.3 advertises no steering extension. Emulation is `session/cancel`, then the next turn reattaches the same session with `session/load` and prompts it with the message. |
| OpenCode, `opencode acp` (a repository run on an `opencode` route) | `turn_boundary` | `next_turn_start` | As for Devin: one `session/prompt` at a time, emulation is `session/cancel`, and the next turn reattaches the session with `session/load`. |
| Grok Build, `grok agent stdio` (a repository run on a `grok` route) | `turn_boundary` | `next_turn_start` | As for Devin: one `session/prompt` at a time, emulation is `session/cancel`, and the next turn reattaches the session with `session/load`. |
| OpenCode, `opencode run` (Coder One's delegate executor) | `turn_boundary` | `next_turn_start` | Reads its whole message before the turn; a new turn is `opencode run --session`. |

The adapters in this repository state their rows in code: `coder_one::steering`
for the Claude Code, Codex, and Microluna session adapters,
`coder::task::adapter::STEERING` for Microcoder,
`coder_delegate::steering::DEVIN_ACP` for a Devin route,
`coder_delegate::steering::OPENCODE_ACP` for an OpenCode route, and
`coder_delegate::steering::GROK_ACP` for a Grok Build route, which each
repository admission records among its capabilities.

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

NIP-HOST `task.command` is the host-wide device form of these commands for
the task owner's single-lineage tasks: `send` is `submit` of the next turn
against an ended task, `queue` is `enqueue` with promotion when the turn
ends, `steer` and `interrupt` keep the meanings here, and emulated steering
runs only when the device set `emulate`, and `answer` responds to the
ended turn's question or approval request, where the first answer wins.
HOST `task.queue` is the device form of `cancel_queued` and `reorder`, with
an edit lease that holds promotion while a device edits. Its evaluation
order and replay rules are in [HOST](NIP-HOST.md#operations).

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

A host's chat threads with OpenAgents (the desktop app's and `openagents
chat`'s, kept in the host's own chat store) are not engine history and are
not a source of this profile. A device reads and continues them with
NIP-HOST [`thread.list`, `thread.read`, and `thread.send`](NIP-HOST.md#operations)
under its host grant. A Coder task such a thread started is read here like
any Coder task: the thread names the task, and the device opens its
transcript through this profile's `coder` source.

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
URL), `sources` (1–3 `{id, label, kind}` objects), `issued_at`, and `expires_at`.
Source IDs use common IDs; labels are inert strings of 1–128 bytes; the kinds
are `codex`, `claude`, and `coder`, each occurring at most once. A `coder`
source is Coder's task directory, such as `~/.openagents/tasks`: the host reads
only the `<task>.<attempt>.atif.jsonl` files directly inside it, each one task
attempt's ATIF transcript, and no other file there. A client that predates
`coder` refuses a grant that names it. Expiry is later
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
cursor, and byte limit, or, instead of a cursor, an `end` offset for a
backward read. Requests carry no host path or executable content.

Catalog pages list chats newest first by each source's last write, which is
also the entry's `updated_at`. The page cursor covers membership, not order:
a chat that moves between page requests can repeat or be skipped, so readers
merge pages by chat ID. A chat without a title is named from the first line
of its first user message that is not injected context. A Coder chat is one
task attempt: its native ID is the 64-hex task ID from the file name, and its
title comes from its first `User` step. The reader's readable projection shows
the model's replies, commands, test runs, and ending from the loop events in
its `System` steps, and marks the other host evidence as `adapter` records. A backward read
returns the page of whole records ending at or before `end` (any offset at
or past the source's length, such as 2^53 − 1, for the newest complete
record) and, when earlier records remain, `previous`:
the `end` of the page before it. Its `next` cursor continues forward from
its last record, so a reader can open a chat at its end, page back on
request, and poll for new records.

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

Only bounded inline JSON is supported. Through a relay, observer bodies are
at most 128 KiB; catalogs have at most 32 entries and transcript pages at
most 32 KiB of raw source bytes within the reader's 112 KiB encoded-page
bound. The direct transport below allows larger replies. Exact private
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

### Direct tailnet transport

A host that serves chats with NIP-HOST
[tailnet admission](NIP-HOST.md#tailnet-admission) also carries these same
sealed requests and replies on a direct connection to that listener, which
is bound to the host's own tailnet address, never a wildcard or LAN address.
It is a transport, not an admission: a device uses it only with a grant it
already holds, and a relay stays the fallback.

The device connects and sends one line of JSON,
`{v: "openagents.history-observer-direct.v1", requires: []}`, at most 1,024
bytes. The host answers `{v, refused}` and serves the connection only when
`refused` is null: the caller's address is in Tailscale's ranges and its
local `tailscale whois` names the host machine's own untagged user, exactly
the callers admission answers. Refusal codes are `not_tailnet`,
`unavailable`, `tagged`, `not_owner`, and `not_serving`. After that, each
line is one frame. The device sends `{id, event}`, where `event` is an
original signed `3188` request artifact and `id` a connection-local number.
The host answers `{"reply": {id, event, payload}}` with the original signed
reply artifact, or `{"refused": {id, code}}` when it has no signed reply (an
unauthenticated local failure, never a domain result, which the device
reads through the relay instead). Frames of different requests may be in
flight and answered in any order.

The host runs every check of a relay request: signature, recipient, schema,
client, current grant, original authorization, source roots, freshness,
rate, revocation, and request-ID replay. It skips only the relay binding,
since no relay carried the request. A pairing redemption travels only
through its relay. The device verifies the reply exactly as a relay reply.

A direct reply may be larger: a transcript page of up to 160 KiB of raw
source bytes within a 232 KiB encoded page, a catalog page of up to 256
chats, and an observer body of up to 248 KiB. A sealed envelope's inline
body is bounded by NIP-44's standard 64 KiB plaintext, so a direct reply's
body travels beside its envelope: the host seals the reply's envelope with
`inline: null` and the exact digest and size of the reply's canonical bytes,
and sends those bytes as `payload`, NIP-44 v2 encrypted to the device under
the same conversation key with the client plaintext bound of 256 KiB. The
device opens the envelope, decrypts `payload`, and accepts it only when its
bytes are exactly the ones the host-signed envelope names; every other
check is the same. A relay reply keeps the bounds above and its inline
body, and a host refuses a direct-sized request that arrives through a
relay. A backward page whose
whole records do not fit the encoded bound together holds the newest of
them that do, down to one record.

Replay protection is unchanged across routes: the host binds each request
ID to its exact event before reading for it, and a request ID is read at
most once whichever route carries it. Because a direct reply can be four
times a relay reply, the host keeps its bytes in memory through the request
lifetime only; an exact direct retry gets the kept bytes, and any other
retry a signed `conflict`.

The host may also send a nudge, `{"changed": {source}}` or `"catalog"`, when
a source this connection read grew or changed, or when the chat list of a
catalog this connection read changed: a chat started, ended, or was renamed
or archived in any admitted root, at once, or a listed chat grew, at most
every two seconds and only after the device read the list since the last
such nudge. A nudge carries only a source ID already disclosed on that
connection and no bytes, length, or title; it is a hint to read again
through an ordinary request, and grants nothing. A host watches a source,
and a catalog's roots, for at most ten minutes after its last read on that
connection.

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

## Rust managed-session profile

The [session schemas](schemas/session.v1.json) describe every record above.
Hosts also enforce the semantic and authority checks in this document. The
profile pins these supporting records:

| Record | Fields |
| --- | --- |
| `openagents.session-steering.v1` | `native`, `emulation`, `acknowledgment`: the steer feature's evidence artifact. |
| `openagents.session-steer-correction.v1` | `correction` (CTX correction ArtifactRef) and `mode` (`native` or `emulated`). A `steer` command's `correction` names this record, so the caller signs its choice of emulation explicitly. |
| `openagents.session-steer-receipt.v1` | `scope`, `turn`, `correction`, `mode`, `acknowledgment`, `consumed`, and native `evidence`. `consumed` is true only with `engine_replay`, `turn_answer`, or `next_turn_start` evidence. |
| `openagents.session-quiescence.v1` | `scope`, `turn`, the turn's ENV `attachment`, terminal native `evidence` (at least one), optional `children` (signed child-state ArtifactRefs), and `observed_at`. |
| `openagents.session-approval-rule.v1` | Host-signed `generation`, `owner`, `approver`, exact action `operation`, `starts_at`, and `expires_at`. This bounded profile requires owner and approver to match, with separate explicit appointment. |
| `openagents.session-action-binding.v1` | Exact `scope`, `turn`, `native_request`, `adapter`, `engine`, `method`, and native request `native_id`. |
| `openagents.session-action-preconditions.v1` | Exact `scope`, `turn`, `turn_revision`, `effective` configuration, approval `rule`, and `native_request`. |

A control receipt's `transition` may also be `respond`, for an interaction
response. Session-history cuts are WS `projection-cut` records over a
host-signed `projection-definition` whose epoch is the lineage and whose
sequence is the session revision. Turn input in the in-process adapters is
JSON; an emulated steer's correction travels in the next turn's input as
`corrections`. Mapping Claude Code, Codex, and ACP delegate engines to this
profile is not implemented yet.

The native host checks current membership and scope generation before returning
a cached open, command, or interaction response. Possessing an earlier signed
body or its artifact hash grants no access. Host-key administration can replace
the owner/controller sets; revoked actors cannot retrieve new cached replies
or promote previously queued work.

Before an engine start, the host saves the active turn, complete RUN dispatch
intent, consumed queue identity, and the command's pending result together.
A lost start acknowledgment leaves that original command unknown. Neither an
exact retry nor a restart may start it again. Native protocol artifacts saved
without their authoritative active-turn link do not establish durable admission.

A runtime-side session operation verifies the exact attachment grant held by
the allocator, its requesting controller and executor, the lease and
materialization, and the admitted engine identity. Caller-provided grant bytes
or knowledge of an attachment fingerprint cannot substitute for those checks.
The allocator holds its current attachment fence through the native operation;
the mounted COORD authority checks the same current claim under its own fence.
An adapter without this authority refuses runtime admission.

The managed-session host persists the pending opening, session ID, resume
lineage, complete RUN dispatch history, and original unknown reply before
calling the native engine. A lost acknowledgment or process restart never
repeats that opening. The unknown opening continues to reserve its session ID
and resume lineage. Closing or archiving cannot assert native termination or
release an uncertain resume binding; this profile refuses close while the
opening is unknown. Exact retries still require the owner's current authority.

Required native messages that an adapter cannot handle, including questions
from an adapter without question support, produce an inspectable RUN fault
linked to the native capture and an unknown turn result with cause
`protocol_fault`. The fault remains in status and portable history after
restart or a later terminal observation. It blocks new input, steering,
configuration, interaction answers, and promotion. Observation and stopping
remain available; positive terminal evidence does not silently restore dispatch.

This host's close operation requires known turn quiescence. It refuses active
or uncertain work; the controller must first stop and observe the current turn.
For a managed native binding, the host saves a complete close RUN intent and
original unknown command reply before calling native close. Only an
acknowledged native close marks the session closed. An uncertain close keeps
the lineage reserved and is never repeated, including under a different command
ID or after restart. Closing a read-only import has no native engine effect.

Adapter and feature operation references identify full `openagents.cap.v1`
definitions. Their input and output SchemaRefs validate with the shared
contracts implementation. Feature inputs restrict their source kind or command
action, and operation IDs distinguish adapters. Definitions pin engine and
protocol identities and state the actual host/engine effects. Unsupported
features have no operation pins. Definitions alone neither expose CJ dispatch
nor claim enforcement of unstated bounds or transport idempotency.

Private input reads require bytes supplied by or shared with the requesting
principal. Promotion uses the original queue requester's access. Reply records
do not grant access by following arbitrary input references. Public adapter
definitions are shared explicitly; private declarations are encrypted separately
for each recipient. Native resume requires a host-signed managed history linked
to that requester's own source admission. Knowing or supplying export bytes does
not transfer native control. This profile accepts foreign content only as an
archive with its actual supplied or shared bytes.

The library's CJ receiver runs exact admitted adapter operations under the
allocator's charged COORD intake. Its current profile requires the ENV binding
feature, frozen artifact input, one job, zero spend, 1–300 seconds of wall time,
and 64 KiB–1 MiB of output allowance. The synchronous loop adapters read
artifacts and write no external resources. Internal host bookkeeping does not
claim a workspace resource. Opening and metadata operations return control
results; submit stays pending until the exact native turn produces a turn
result. Both result shapes are in the adapter and submit output schemas.
Original retries reuse the saved encrypted event and explicit artifact set,
without repeating intake or charging again. Polling after expiry requests
stopping that original turn before advancing it; only terminal native evidence
proves quiescence. Oversized known output becomes a bounded failure; uncertain
work stays unknown with its accounting hold. Native RUN identities cannot
alias an existing CJ RUN. Real engine processes remain unfinished. The bounded
resident socket profile is described below.

Private active-turn continuations name their original CJ execution. New
question replies and steering require that execution's exact current native
turn, owner, generation, installed engine instance, and ENV/COORD authorities.
The continuation proof cannot authorize a new opening or turn. It neither
charges again nor frees the original unknown hold. New execution intake still
refuses that already dispatched claim. Polls check the same current authorities
before advancing the loop; a refusal requests stopping only the original turn.
Steer and interrupt commands persist their unknown pending reply before native
delivery. Lost replies cannot repeat on retry or restart; uncertain steering
blocks further input until terminal observation. Cached controls require current
ownership and generation without redelivery. Question replies check their own
expiry as well as the pending interaction's expiry.

Configured opening and idle configuration changes deliver admitted content to
the native adapter and require acknowledgment of the exact values applied.
The loop adapters select only `{"model":"loop"}` and apply their closed
`effort` setting to the native turn's step ceiling (`low`: 32; `high`: 64).
Their model, recipient, and policy pins remain fixed within that binding.
Unsupported configuration bounds refuse rather than appearing in effective
enforcement. A configuration change has its own charged execution, saves its
complete RUN intent and unknown command reply before native delivery, and
changes the effective record only after a matching acknowledgment. Lost or
different acknowledgments stay unknown and block further input or configuration
changes without replay, including after restart. New turn intents name only
the current requested or effective configuration, not obsolete opening settings.

The native `replace_runtime` command has payload `{adapter, lease,
materialization, requested, source, workspace}`. It uses the complete adapter
CAP under a fresh ENV-bound CJ execution; it is not a fifteenth feature-matrix
entry. The new adapter must support the selected `new` or `resume` source.
Replacement requires known quiescence, an empty queue, no pending interaction,
and acknowledged closure of the old native binding. An unknown opening,
configuration, close, active turn, or managed child refuses replacement.
Resume additionally requires the owner's exact latest host-signed history and
compatible native resume support. The portable session ID and lineage stay
fixed; the native binding, ENV pins, and configuration may change.

Before closing the old native binding, the host saves the replacement command,
complete RUN intent, and unknown reply. After closure is acknowledged, it
increments the scope generation and saves that pending generation before
opening the replacement. Only the matching new native acknowledgment makes it
ready. A lost closure or opening reply remains unknown and cannot repeat or
make the session ID or lineage available. Exact retries of this replacement
may read its original outcome in the generation it established; other controls
from the preceding generation are stale. Another replacement makes those
earlier cached controls stale too.

An acknowledged replacement produces `openagents.session-runtime-binding.v1`
with `scope`, `previous` (prior binding or initial admission ArtifactRef),
`request`, `attachment`, `lease`, `materialization`, `adapter`, `requested`,
`effective` (ArtifactRefs), and `admitted_at`. The session state and portable
history name it in `meta.runtime`. This record supplies current runtime pins
for later turns and continuations; the immutable initial admission remains in
history. The control receipt uses transition `replace_runtime` and the new
scope. Its revision advances by one after acknowledged replacement.

Native names are scoped to the engine instance that admitted them. Another
host instance cannot treat a reused name as authority for earlier input,
cleanup, observation, or resume reads. An unverified old binding stays unknown,
including when it was idle. Pending interactions become unknown on lost
observation and cancelled on known terminal completion; their current signed
records carry that state. An explicit acknowledged replacement may clear the
current generation's protocol fault while retaining earlier fault evidence in
native history and turn results.

After charged intake, a known admission refusal produces a bounded retained CJ
failure for that exact request. The host keeps each control outcome, including
native duplicates, under its own invocation identity and signs the returned
body. A denied request neither reserves another session's command identity nor
discloses its state. Its cached refusal still checks current host membership.
Recovery may use a native prior reply only when its full input fingerprint
matches; command ID equality alone cannot substitute a different result.
Uncertain native effects remain unknown and keep their accounting holds.

The native `promote` command uses payload `{intent, input_digest, requester}`:
the exact queue-head intent ID, its frozen intent artifact's SHA-256 digest
string, and original requester key. The expected session revision and
generation also apply. It uses the adapter or queue CAP under a fresh charged
ENV execution; enqueue remains a separate metadata operation. The host verifies
the retained original signed enqueue, matching intent and scope, and the
requester's current control rights. A digest selector cannot grant access to
another requester's cache. Input is read under the original requester's
supplied/shared-byte scope and forwarded only to the session and configuration
that requester selected. The fresh caller supplies execution authority and its
own current reservation; promotion does not reuse an earlier binding.

Promotion considers only the current head. An expired or incompatible head
gets an explicit `expired` or `refused` disposition and is never skipped. Such
a completed control operation returns its disposition record without a turn;
an actual start stays pending until that turn's native terminal evidence.
Starting requires the intent's exact task frame to match the current ENV
execution and refuses unsupported intent bounds. The host durably records
intent consumption with the active turn, consumed queue head, and pending
command reply before native delivery. Consumption is scoped to the lineage:
changing command IDs, bindings, runtime generations, or resume sessions cannot
run the same consumed intent again. A new execution needs a new intent ID;
changed bytes under a consumed ID conflict. Exact original command retries
retain their earlier outcome without another native start.

The native host retains the original CJ event before dispatch so its internal
watchdog can observe that exact execution without a connected client. Deadline
and current-authority checks precede further engine progress. Revoked readers
receive no private reply; reader revocation does not prevent the host from
stopping and observing its own exact native work. A saved stop intent prevents
another signal, while subsequent observations may establish quiescence. A
restart cannot replay an opening or input and cannot adopt another engine
instance's native names. The whole initial admission and disclosure share one
monotonic wall-time allowance; individual phases cannot restart that allowance.

The standalone native host can carry signed operations over an owner-only Unix
socket using one bounded JSON line per connection. The outer transport version
is `openagents.session-host-transport.v1`; `id` correlates a request and reply,
and `operation.method` selects `execute`, `poll`, `continue`, `submit`, or `read`.
Execute carries the original CJ event, explicit artifacts, and ENV binding;
poll names that original event; continue also carries a separately signed
private control bundle. Submit carries a private bundle and cannot grant new
engine execution. Socket access supplies no protocol authority. Clients retain
their own keys and verify signed CJ replies against the original request.
The host keeps its engines alive across connections and drives its watchdog
between requests. Frames are at most 40 MiB, there are at most eight open
connections, and an unfinished connection expires after five seconds.

Read carries a signed private `openagents.session-read.v1` artifact with exact
`scope`, `query`, `issued_at`, `expires_at`, and an explicit `artifacts` list.
Only the session's current owner may read through this profile; current host
membership and scope generation are checked on every read. It does not advance
the engine. The encrypted `openagents.session-read-result.v1` reply pins that
query and the current signed session state, and includes only the explicitly
requested, already shared bytes plus that state. At most 256 requested
artifacts plus the state, totaling 4 MiB of bytes, may be returned. Knowing a hash, following a nested reference,
or controlling a different session does not grant cache access. Clients can
request pending interaction records and their prompts in subsequent reads.
Socket transport does not add relay discovery or a remote deployment service.

The native host retains a turn's exact terminal output and native evidence
while managed children still need disposition. It remains blocked and cannot
accept another turn or signal the already ended native turn. Later observation
uses the retained evidence rather than stepping that native turn again.
Quiescence's `children` list includes each managed child's signed disposition.
Native `evidence` remains native protocol records. An uncertain
child, or a stop deadline with unfinished children, leaves the parent unknown
without quiescence. An observed child grants no control and does not block this
managed-work check. This bounded profile links managed children only while the
parent turn is active, within the same owner, and without cycles; it does not
infer delegated execution authority from provider parent edges.
Production child links additionally need a separately mounted current child
authority check; an opaque authorization reference alone refuses. Local loop
fixtures select their explicit test authority. Child dispatch still requires
its own charged execution and cannot use this metadata check as a grant.

Native turn delivery carries the exact admitted frame and context bytes beside
the delivered input. An adapter that cannot handle this input boundary refuses
rather than silently dropping either selection. The pure loop adapters consume
opaque frame/context bytes by computing and retaining their SHA-256 digests;
their native captures and outputs include those digests, not the private
context bytes. Hashes in output do not share the corresponding cached artifacts.
Each delivered input, frame, and context is at most 1 MiB, including input
derived from explicitly carried steering corrections. Authorization remains a
host-side check and is not passed to an engine as permission-bearing text.
Opening and configuration compare the retained adapter's engine and protocol
bytes with the actual installed implementation. Reusing an old admission after
an engine update refuses; it cannot silently select newer native behavior.

The optional `loop-approval` native profile gates one bounded, in-process
`hash` action. Its action CAP pins the input/output schemas, engine, method,
and pure artifact-read effects. The host's operator separately grants approval
for an exact action DefinitionRef and owner until an expiry; session ownership
or control alone does not appoint an approver. This profile supports the
session owner as approver. Policy replacement changes the grant generation;
old pending requests cannot revive after revocation and regrant.

The host freezes the native request, input, scope, turn revision, adapter,
effective configuration, and current approval rule into a complete POL action
and approval request. A direct signed private POL decision travels through the
original charged execution's continuation endpoint. The host maps its exact
request to the retained interaction without fabricating an approver signature.
Ordinary question answers cannot authorize an action. Before the method-correct
native reply, the host rechecks the decision, current rule, expiry, generation,
native request, and original ENV/COORD execution. It saves single-use delivery
intent before the reply; a lost acknowledgment remains unknown and never
repeats. Changed actions, decisions, requests, or conditions refuse. A required
approval with no supported method or current rule records a protocol fault and
permits no effect. The loop's hash executes once after approval; denial computes
no hash. The signed decision's expiry and the original ENV/COORD authority also
gate subsequent engine advancement; a cached acknowledgment cannot extend them.
Steering waits until this approved turn ends. Cancellation after the hash ran
retains its observed digest. Effectful SDK permissions remain unsupported by
this profile.
