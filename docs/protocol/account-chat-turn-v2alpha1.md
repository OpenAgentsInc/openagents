# Durable rich conversations, v2alpha1

Implementation for #13619. This new owned contract preserves the frozen
`openagents.chat.thread.v1` and Project chat-link identities. The schema is
`proto/openagents/chat/v2alpha1/conversation.proto`. The shared codec is
`coax/web/api/account-chat-turn-wire.coax`; decisions are
`account-chat-turn-policy.coax`. The initial consumer is **durable-text**.

**Status:** protocol and checked lifecycle implementation in progress. No route,
installed app integration, live inference or issue completion is claimed by this
document. TestFlight waits for the complete Claude-port and Arcade goal.

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
a saved turn. No phone-held socket owns execution.

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

Mobile and CLI consume the same protocol, persisted turn and event stream.
Reuse shared transcripts, Markdown, composer, auth and context assembly. Switching
accounts clears private state and invalidates callbacks before rendering another
account. Model/agent/effort decisions remain automatic under `docs/router/`.

Integration must account for the installed transport and history bounds:
`NativeAccountCodec` currently caps account responses at 131,072 encoded bytes,
and v1 history IDs have 24-byte storage. A 131,072-byte decoded rich reply can
encode much larger; use a bounded paged snapshot or a separately admitted native
transport operation, never silent clipping or a global unbounded fetch. New
36-byte conversation IDs need an owned history version alongside v1. The
current private chat's **Stop waiting** only stops its client wait; switch to
the new requested/observed cancellation flow when the backend is connected.

Completion requires actual API storage/worker/recovery, CLI and installed native
flows, live staging inference and all acceptance criteria on #13619; schemas and
unit checks alone do not close the issue.
