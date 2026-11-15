# Durable rich conversations, v2alpha1

Implementation for #13619. This new owned contract preserves the frozen
`openagents.chat.thread.v1` and Project chat-link identities. The schema is
`proto/openagents/chat/v2alpha1/conversation.proto`; additive service envelopes
are in `service.proto`. Submit/events use `account-chat-turn-wire.coax` and
resources/worker envelopes use `account-chat-turn-record.coax`. Lifecycle and
queue decisions are `account-chat-turn-policy.coax` and
`account-chat-work-policy.coax`. The initial consumer is **durable-text**.

**Status:** transactional routes, the resident-worker adapter and CLI integration
are implemented and undergoing runtime verification. Native adoption and live
staging evidence remain pending; #13619 is open. No deployment or release is
claimed here. TestFlight waits for the complete Claude-port and Arcade goal.

## HTTP consumer

All routes below are account-owned, private/no-store and versioned separately
from the frozen v1 routes. Only the configured, freshly signed runner key may
use the runner door; that key is not an account credential.

| Method and path | Request / response |
| --- | --- |
| `POST /api/chat/submit/v2alpha1` | SubmitRequest → 202 ControlReply |
| `POST /api/chat/stop/v2alpha1` | StopRequest → 200 ControlReply |
| `GET /api/chat/turn/v2alpha1?id=TURN` | Complete materialized Turn and its cursor |
| `GET /api/chat/events/v2alpha1?id=TURN&after=N` | EventPage, at most 32 events and 131,072 encoded bytes |
| `GET /api/chat/conversations/v2alpha1?project_id=PROJECT&cursor=CONVERSATION` | ConversationList, at most 60 summaries, ID order |
| `GET /api/chat/conversation/v2alpha1?id=CONVERSATION&after=N` | Conversation with at most 32 messages; `after` is a message offset |
| `POST /api/chat/runner/v2alpha1` | RunnerRequest → RunnerJob (`next`) or ControlReply (`run`, `lost`) |

Omitted `after` is zero. Omitted Project/index cursor means unfiltered/first
page. Cursors ahead of the current resource conflict; malformed values refuse.
The index's next cursor is its last returned ID; conversation pages use a
decimal message offset. Each request checks ownership again.

Conversation revision counts admitted user turns. Events, Stop and model output
advance the event cursor and updated time, not the conversation revision. A
new turn requires the latest conversation revision and preserves its Project
association. The initial funded team beta admits at most 32 queued/running
turns per account, one per conversation, with complete history limited to 128
messages / 131,072 decoded bytes and selected Project context to 32,768 bytes.
The fixed automatic initial model has a 2,048 output-token request cap. Later
capability/routing work belongs to #13622.

Clients restore a Turn through `ctr_turn_read`, then atomically apply whole
pages through `ctr_page_apply` and the existing reducer. A bad later event,
wrong identity or gap publishes no part of that page. Duplicate pages never
append text again. This first HTTP consumer exposes bounded polling/replay;
the event codec's SSE framing is available for a later streaming subscription.

## Service envelopes and storage

These additive envelopes do not change the earlier conversation message fields.
All listed fields are present; absent optional references/cursors are null.

| Record | Fields |
| --- | --- |
| ControlReply | `schema`, `conversation_id`, `turn_id`, `conversation_revision`, `state`, `stop_requested`, `event_cursor` |
| ConversationSummary | `id`, `title`, `revision`, `project_id?`, `active_turn_id?`, `updated_at_ms` |
| ConversationList | `schema`, `items` (summaries), `next_cursor?` |
| EventPage | `schema`, `conversation_id`, `turn_id`, `after`, `events`, `next_cursor`, `has_more`, `state` |
| RunnerRequest | `schema`, `operation`, `operation_id`, `worker_id`, `turn_id?`, `dispatch_id?` |
| RunnerJob | `schema`, `has_work`, `turn_id?`, `conversation_id?`, `dispatch_id?` |

Control and job readers cap encoded input at 2,048 bytes; runner requests at
4,096. Worker and operation IDs are 1–80 UTF-8 bytes. A `next` request has null
turn/dispatch; `run` and `lost` require both, with dispatch `run_` plus 32
lowercase hex digits. A job has all three references or none. Resource timestamps
are nonnegative safe JSON integers; titles are at most 160 UTF-8 bytes. Turn
metadata records model/provider IDs, effort, route identity/reason, admitted
context snapshot and timestamps alongside the materialized view. Usage stays
nullable. Page composers take fragments emitted by the owned message/summary
codecs; they are not decoders for arbitrary supplied JSON.

Migration 0204 stores conversations, immutable admitted inputs/Project snapshots,
materialized turns, immutable event records, account operation identities and
runner claim receipts. Account-qualified foreign keys prevent associations
with another account's Project/conversation/turn. Unique indexes serialize an
active turn per conversation, turn ordinals, operations and event sequences.
Every private table follows account merge/purge; internal claim receipts are
omitted from account exports. Admission/replay/recovery still require Coax policy
and transactional adapters; database constraints alone do not implement them.

Claims last 60 seconds and leave the public state queued. Dispatch requires the
current live fence, a current grant and a never-started turn. A replaced claim
retains its original receipt, so replaying its operation cannot select another
turn. A running dispatched attempt with no heartbeat for 450 seconds is eligible
for unknown recovery. Time and grant facts come from trusted server effects.

## Identity and writes

Conversation IDs are `cnv_` plus 32 lowercase hexadecimal digits; turn IDs are
`trn_` plus 32 digits. Caller operation IDs are 1–80 UTF-8 bytes, without NUL.
The authenticated account owns every conversation, turn, event and operation.
Private Project membership and current ownership are checked independently;
Project association grants no computer control. Responses are private/no-store.

Submit carries every `SubmitRequest` field, with explicit null for absent
optionals. A new conversation uses empty `conversation_id` and revision zero;
an append uses its exact positive revision. Project ID and expected Project
revision occur together. Legacy ID/revision occur together, only on creation.
Unknown/duplicate fields, noncanonical escaped keys, unsupported block types,
NULs, invalid UTF-8, fractional/negative/unsafe integer counts and overflow are
refused. JSON uses numbers, not protobuf's default decimal strings.

The first consumer accepts 1–16 nonempty text blocks, with at most 16,384
decoded UTF-8 bytes across the user message and 131,072 encoded request bytes.
Ordered block boundaries are preserved. Image/document/tool/result/citation/
opaque variants have schema slots for subsequent real consumers; this consumer
refuses them. Supporting those slots is not inferred from their presence here.

Account-scoped operation identity binds the canonical submitted request and
route. Exact replay returns the original conversation/turn identity and current
state. Different input with the same operation ID conflicts. Replay is resolved
before revision checks, so a committed operation remains reconcilable. One active
turn owns a conversation at a time. Admission snapshots the complete actual
model input and Project instructions/knowledge revisions before dispatch.

## Execution and durable events

States are queued, running, completed, failed, cancelled and unknown. The latter
four are terminal. Starting a queued attempt records dispatch identity before an
external request; a lost dispatched worker becomes unknown, never automatically
starts another inference call. An undispatched queued turn can be canceled
without contacting a provider. Stop on a running turn records a request; only
the worker's observed cancellation ends it as cancelled. A completed reply may
win the race with Stop. Unknown and partial results remain inspectable.

The database serializes each policy transition and immutable event append.
Sequence starts at 1, advances exactly by one and retires at 2,147,483,645. The
initial queued snapshot has cursor zero. An event carries the conversation and
turn IDs, sequence, type, resulting state and cancellation-request flag. Text
events carry their block index and complete UTF-8 delta. Usage events carry
cumulative observations. Terminal events carry the actual stop reason/error.
No events mutate a terminal turn. The stored materialized reply must be the
same reduction of this log that clients observe.

Execution belongs to the existing resident `oa-agent` service, whose deployment
already allocates CPU outside requests (`coax/web/agent/deploy.sh`). The API
deployment scales to zero and currently uses request CPU: replying 202 and then
forking an unowned inference child there cannot establish this lifetime. Reuse
the existing wake/runner signing/turn loop, with a new versioned account-turn
claim/input/event/end interface. The API retains the queue, dispatch identity,
immutable input, events and metering. A replayed wake can claim only the same
undispatched job; the worker's claim is fenced before upstream I/O. A dispatched
worker loss ends as unknown rather than reissuing the model call. Recovery must
scan committed queued work as well as accept wakes, so a lost wake cannot strand
a saved turn. The runner deployment now requests `--min-instances 1`; CPU
allocation alone is insufficient once it scales to zero. The worker scans on
startup and every 30 seconds, in addition to the shared wake transport. Exercise
recovery after a missed wake and service restart before enabling it in a live
environment. No phone-held socket owns execution.

Reuse the existing runner's signed inference door: the resident worker can hold
an authenticated inference HTTP request open while the API persists normalized
events before relaying them. Its live request retains API execution even when
the phone disconnects. Provider credentials remain in the API. Retrying event
delivery or waking the worker is distinct from starting another model request;
a lost dispatched request still reconciles as unknown. No second model client,
queue service or provider credential store is required.

Resume requests select events strictly after a persisted cursor. SSE uses
`id: SEQUENCE`, `event: turn` and the same JSON event codec as replay/polling.
Clients ignore already-applied events, require contiguous next sequences and
refetch/replay a gap; they never append a repeated text delta. A snapshot plus
its cursor is read atomically. Bounded replay pages expose their continuation;
they do not silently skip events. Disconnect/background does not own execution.

## Usage, history and surfaces

Usage has nullable input/output/cache-read/cache-write/reasoning token counts.
Null means unobserved; numeric zero is a real observation. Preserve terminal
stop reasons separately from lifecycle state. A provider error or truncated
transport is not a completed empty answer. Quotas and settlement stay with the
existing inference/money services, correlated to the admitted operation.

A v1 continuation explicitly copies an authorized revision into a new rich
conversation with immutable provenance and its existing private Project
association. Later v1 edits cannot change this cut. Original v1 records and links
remain readable. Paginate history; choose a complete bounded model context or
refuse it explicitly, without silently omitting old messages.

Copied v1 messages use stable IDs `CONVERSATION.legacy.INDEX` and an empty
`turn_id`: they have original text/role and no rich turn receipt. The new
conversation pins the source thread ID/revision. Its copy timestamps record
the copy, because v1 messages have no per-message timestamp to recover.

Mobile and CLI consume the same protocol, persisted turn and event stream.
The native source reads indexes, conversation pages, materialized turns and event
pages through the one `account-chat-turn-record.coax` codec. Its per-conversation
state owns HTTP bytes and copies selection metadata out of scratch arenas.
History continuation is checked against the requested message offset. A bounded
visible transcript window changes presentation only; it never changes the model's
admitted history. New turns and Stop retain exact pending writes until their
receipts resolve. Native `clipboard.write.v2` is a separate installed profile
for complete 128 KiB message copies; v1 remains unchanged.
Reuse shared transcripts, Markdown, composer, auth and context assembly. Switching
accounts clears private state and invalidates callbacks before rendering another
account. Model/agent/effort decisions remain automatic under `docs/router/`.

Integration must account for the installed transport and history bounds:
`NativeAccountCodec` caps its settings responses at 131,072 encoded bytes;
the generic native HTTP service already accepts up to 1 MiB, and v1 history IDs
have 24-byte storage. A 131,072-byte decoded rich reply can encode much larger
and its bridge envelope adds another layer of escaping. Use bounded event and
history pages, never silent clipping or a global unbounded fetch. New
36-byte conversation IDs need an owned history version alongside v1. The
current private chat's **Stop waiting** only stops its client wait; switch to
the new requested/observed cancellation flow when the backend is connected.

Completion requires actual API storage/worker/recovery, CLI and installed native
flows, live staging inference and all acceptance criteria on #13619; schemas and
unit checks alone do not close the issue.
