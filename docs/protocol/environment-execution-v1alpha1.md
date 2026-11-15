# Routed execution handoff v1alpha1

Status, 2026-11-15: checked handoff records and admission primitives for
[#13620](https://github.com/OpenAgentsInc/openagents/issues/13620). **No task
upload/start RPC, router dispatch, mobile task view or CLI task command is
installed by this foundation.** Those consumers and the task event/control
contract are the next implementation steps. The current environment runtime
remains its separate Claude-only profile.

The defining schema is
[`execution.proto`](../../proto/openagents/environment/execution/v1alpha1/execution.proto),
profile `environment-execution`. One codec,
[`execution-wire.coax`](../../coax/lib/agent/environment/execution-wire.coax),
reads and writes all three records. JSON objects are closed, every listed field
is required, keys are emitted in protobuf field order, and integers are exact
JSON numbers through 2^53−1. Text must be complete UTF-8 without NUL. Unknown,
duplicate, escaped-alias and missing keys are refused. A failed decode does not
publish a partial record.

## Binding

`schema` is `openagents.environment.execution.binding.v1alpha1`.

| Field | Meaning and bound |
| --- | --- |
| `schema` | Exact record identity above |
| `task_id` | 32 lowercase hexadecimal bytes; one routed task |
| `conversation_id` | Existing `cnv_` plus 32 lowercase hex; ordinary account or Project chat |
| `turn_id` | Existing `trn_` plus 32 lowercase hex; admitted user turn |
| `route_id` | 32 lowercase hex; identity of the retained route decision |
| `host_pubkey` | 64 lowercase hex; the exact enrolled computer |
| `workspace_id` | 1–64 bytes, lowercase letters/digits/dot/hyphen; an operator-configured ID, never a requested filesystem path |
| `session_id` | 32 lowercase hex; stable resident managed session |
| `command_id` | 32 lowercase hex; original start intent, unchanged on retry |
| `engine_id` | `claude` or `codex` for this first execution profile; other router candidates require their own functioning adapter |
| `account_id` | `system` or 32 lowercase hex, scoped to this host and engine; no token, email or credential |
| `model_id` | 1–128 bytes, lowercase letters/digits/dot/hyphen; exact offered model, with no implicit default |
| `effort_id` | 0–16 bytes of the same alphabet; empty only when the offered model has no effort ladder |
| `input_sha256` | 64 lowercase hex; SHA256 of the complete canonical `Input` JSON bytes |
| `input_bytes` | 1–1,048,576; exact length of those encoded bytes |
| `report_sha256` | 64 lowercase hex; exact authenticated computer report retained with the route |
| `report_observed_at_ms` | Positive timestamp, no later than `issued_at_ms` |
| `issued_at_ms` | Positive original logical-intent time |
| `expires_at_ms` | Greater than issue, at most 300,000 ms later and at most 2^53−1; unchanged on retry |

The selection is automatic. A changed model, effort, account, host or input is
a different binding, not a quiet substitution beneath an old command ID.
`ex_choice` matches the actual engine catalog and effort bit exactly. It does
not use the desktop catalog helper's fallback to another effort. A static
catalog alone is not evidence that an account can execute the task.

The binding contains no HOST grant. The consumer must admit the current
signed HOST request, exact enrolled device/grant and configured workspace,
then compare the validated binding with the resident's own host, workspace,
complete input hash/length and authenticated provider account. The checked
`ex_scope` and `ex_pin` primitives enforce those comparisons; their facts must
come from the owner, not fields supplied by a caller. Project membership and
Project context confer no computer control. Reachability, fresh reports,
capacity and task requirements must be rechecked by the dispatch consumer.

## Complete input

`schema` is `openagents.environment.execution.input.v1alpha1`.

| Field | Meaning and bound |
| --- | --- |
| `schema` | Exact record identity above |
| `instructions` | Approved instructions, possibly empty |
| `reference` | Approved reference material as data, possibly empty |
| `messages` | 1–128 ordered `InputMessage` objects; final message is the current user request |

`instructions` plus `reference` must fit the existing **32,768 UTF-8 byte**
Project context budget, enforced through the shared `pcb_context` rule.
Each message has exactly `role` (`user` or `assistant`) and nonempty `text`.
A user message is at most 16,384 bytes; assistant text and the sum of all
message text are at most 131,072 bytes. No selected text is silently clipped.
Tool calls, attachments and imported commands are not accepted by this first
input consumer. Historical text is context, never an instruction to replay
historical effects. Rich tools/results belong to the forthcoming task timeline
and functioning consumer rather than a widened frozen chat/runtime shape.

## Input chunks

`schema` is `openagents.environment.execution.chunk.v1alpha1`.

| Field | Meaning and bound |
| --- | --- |
| `schema` | Exact record identity above |
| `task_id` | 32 lowercase hex; task whose input is being prepared |
| `command_id` | 32 lowercase hex; retained upload intent identity |
| `input_sha256` | Expected complete canonical input SHA256 |
| `input_bytes` | Expected complete encoded length, 1–1,048,576 |
| `offset` | Nonnegative decoded UTF-8 byte offset in the input file |
| `text` | 1–4,096 complete UTF-8 bytes of canonical input JSON |
| `issued_at_ms` | Positive original upload intent time |
| `expires_at_ms` | The same five-minute logical-intent rule as Binding |

The frozen HOST body ceiling is 49,152 bytes. Even a maximally escaped chunk
is below 32,768 encoded bytes, leaving room for carriage metadata. A complete
admitted history therefore need not fit in one envelope. Splits preserve UTF-8
boundaries and offsets count bytes.

`ex_chunk_action` admits an append only at the retained end. A range wholly
retained with exactly matching bytes is a replay; a gap, changed retained range,
overlap into new bytes, invalid length or overflow is refused. The resident
consumer must scope storage to the admitted grant/task/hash/total, durably retain
command identity, and verify the complete canonical input hash and byte length
before permitting a start. An upload has no execution effect. Persisting these
ranges, reconciling after restarts and starting an engine are not implemented
by the pure range classifier.

## Checks and remaining integration

The codec checks include complete context/history capacity, UTF-8 byte offsets,
worst-case JSON expansion, closed fields, exact timestamps, unsupported
engines, model/effort refusal and caller arena/output bounds. The fixture
oracle holds every record to its descriptor and exact JSON bytes and checks
the input's actual length and SHA256 against both its binding and chunk.
`execution-policy.coax` is authority code with scope, pinning, exact choice and
range replay checks.

[`execution-input.coax`](../../coax/lib/agent/environment/execution-input.coax)
prepares append/replay bytes without publishing them. Its authority checks bind
each chunk to the validated task/hash/total. At the complete length it verifies
the actual SHA256, decodes the whole input and requires byte-for-byte canonical
re-encoding before returning `complete`. A matching hash of malformed or
noncanonical JSON is insufficient. Failed preparation leaves the owner's
retained state unchanged; its separate output scratch buffer is unpublished.
The effect owner must supply distinct retained/chunk/output buffers and only
commit a successful preparation durably. These pure helpers perform no file
write, HOST authorization or process start.

Next, the resident consumes these records through a negotiated authenticated
profile and owns their durable receipts, provider selection and process effects.
The task event/control contract must carry structured tool correlation, held
permissions and truthful Stop/unknown outcomes. API dispatch, mobile/CLI joins
and a real staging patch/test task remain the acceptance gates in #13620.
