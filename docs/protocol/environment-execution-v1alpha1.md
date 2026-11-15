# Routed execution handoff v1alpha1

Status, 2026-11-15: checked handoff records and an authenticated resident input
preparation consumer for
[#13620](https://github.com/OpenAgentsInc/openagents/issues/13620).
`PrepareTask`, `PutTaskInput` and `ReadTaskInput` persist input under the
current HOST environment grant. **Task start, router dispatch and mobile or
OpenAgents CLI task commands remain unimplemented.** The current environment
runtime remains its separate Claude-only profile. These are source-level
capabilities, not a claim of a production deployment.

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
complete input hash/length and authenticated provider engine/account pair
(Claude's `system` and Codex's `system` are different accounts). The checked
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
before permitting a start. An upload has no execution effect. The pure range
classifier is consumed by the resident store described below.

## Authenticated resident preparation

[`preparation.proto`](../../proto/openagents/environment/execution/v1alpha1/preparation.proto)
adds three records without changing the published handoff or runtime records.
Their single codec is `coax/lib/agent/environment/preparation-wire.coax`.
All fields below are required; views use an empty command ID for a read.

| Record | Fields |
| --- | --- |
| `PreparationRequest` | `schema: "openagents.environment.execution.read-input.v1alpha1"`, `task_id` (32 lowercase hex) |
| `PreparationView` | `schema: "openagents.environment.execution.preparation.v1alpha1"`, `host_pubkey` (64 hex), `task_id` (32 hex), `command_id` (32 hex or empty on read), `input_sha256` (64 hex), `input_bytes` (1–1,048,576), `retained_bytes` (0 through input length), `complete` (boolean), `replay` (boolean), `expires_at_ms` (original binding deadline) |
| `InputCommandReceipt` | `schema: "openagents.environment.execution.input-receipt.v1alpha1"`, `task_id` (32 hex), `command_id` (32 hex), `request_sha256` (64 hex), `issued_at_ms`, `expires_at_ms` (the unchanged chunk's five-minute window) |

The existing signed/encrypted HOST `app.rpc` carries these methods:

| Method | Params | Required right | Result |
| --- | --- | --- | --- |
| `PrepareTask` | `Binding` | operate | Immutable stage binding, empty on first preparation |
| `PutTaskInput` | `InputChunk` | operate | Exact durable retained byte count |
| `ReadTaskInput` | `PreparationRequest` | observe | Current progress reconstructed from disk |

Every call rechecks the current grant/epoch, configured environment and exact
enrolled device/grant before accessing the store. Project membership does not
admit any method. The current workspace ID is `default`, whose actual path is
locally configured; a caller cannot choose a path. Staging input does not prove
that the selected engine/account/model is ready or admit execution.

The existing resident lifetime lock serializes writes. Input lives under
`($CODER_HOME or $HOME)/.openagents/coder/host/execution-input/<grant>/<task>/`
in private 0700 directories, with
0600 `binding.json`, `input.json` and `commands/<command>` files. Prepare
compares normalized binding bytes; changed reuse conflicts. Before publishing
new input bytes, Put durably records the SHA256 of the **whole normalized
chunk**, including its unchanged logical window. A retained command cannot
be reused for a different range, payload or window. An interrupted write can
recover against that receipt without minting a new intent.

Exact retained-range and binding retries report `replay: true` without
rewriting data. Such observations remain available after expiry; adding bytes
or recording a new command requires both the binding and chunk windows to be
fresh. Full completion requires actual canonical-input, hash and length
verification, including on reads after restart. Corruption cannot become a
complete view merely because the file length matches.

At most four stages per grant and 512 upload receipts per stage are retained.
Preparing a new task collects expired **input-only** stages; unknown entries
conservatively consume capacity. This cache owns no engine or session. A
future start consumer must retain its execution receipt and complete input
outside this cache before launching, and reconcile task identity there even
after cache collection. Capacity refusal grants no retry with a new task ID.

The broker/operator client uses existing HOST key handling and correlation:

```sh
coder/tui/coder.sh host execution-rpc --host HOST --browser-key PRIVATE_KEY_FILE \
  --grant GRANT --epoch EPOCH --relay RELAY --request-file REQUEST_JSON
```

The request schema selects one of the three methods. The client validates the
returned host, task, command, input hash and length where supplied. It emits
the shared PreparationView. Exit 0 means success; 2 malformed/unsupported,
3 unavailable/transport failure, 4 forbidden/revoked, 5 expired/stale,
6 conflicting intent and 7 capacity/rate limit. It never automatically resends
uncertain work. This internal client is not the future OpenAgents task CLI.

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

The resident now consumes these records through authenticated HOST methods and
owns their durable upload receipts. Provider selection and process effects
remain the next execution integration.
The task event/control contract must carry structured tool correlation, held
permissions and truthful Stop/unknown outcomes. API dispatch, mobile/CLI joins
and a real staging patch/test task remain the acceptance gates in #13620.

Initial qualification: the policy's 93 root-site mutants and input preparation's
47 were all killed. The codec's 432 root-site mutants finished with 421 killed,
11 reviewed equivalents and zero unresolved survivors. The annotations explain
fixed capacities, repeated budget checks, an unused append-match argument and
the unreachable encoded-size branch. Escaped-key guards have negative checks.
The native fixture tool verifies actual complete preparation as well as exact
bytes, descriptor keys and SHA256, and the production library compiles to C and
a native object. All three modules name their own `coax test` command so the
fixture tool's imports cannot hide them from suite discovery. These checks prove
the handoff primitives, not resident execution or the issue's end-to-end gates.
The escaped-key fixture generator is itself pinned to independent exact bytes;
all 33 of its changed-procedure mutants were killed in both consuming suites.

Preparation qualification: all 80 policy and 198 codec root-site mutants were
killed. The descriptor/exact-byte oracle checks both progress views and the
whole-request receipt digest. The native filesystem oracle passes 27 checks,
including private modes, interrupted receipt/data publication, replay without
mtime changes, conflicting intent, corruption and both capacity bounds.
The signed HOST smoke exercises explicit environment admission, actual chunk
publication, resident restart, progress reconstruction, exact retry, complete
hash verification and revoked-grant refusal. Its relay is an explicit local
fixture with a bounded 240-event/REQ/key burst budget for three resident starts
and their two-minute history replays; production defaults are unchanged.
No provider login or model call is needed for preparation. The Linux
socket qualifier also verifies actual kernel peer UID checks on both ends.
Two earlier full smoke runs passed 48/49: a final pairing assertion did not
observe the expected reply even though the operation was persisted. The second
failure moved from re-pair to forget. This observation is retained; clients
must reconcile uncertain outcomes rather than assume an unobserved effect failed.
With the fixture's explicit burst budget, the complete native smoke passed all
49 checks without a resident publication error. This qualifies the stated
fixture capacity, not delivery under an exhausted production relay quota.
