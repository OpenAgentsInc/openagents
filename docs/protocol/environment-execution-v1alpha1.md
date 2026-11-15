# Routed execution handoff v1alpha1

Status, 2026-11-15: handoff records, authenticated input preparation and a
resident task owner for
[#13620](https://github.com/OpenAgentsInc/openagents/issues/13620).
`PrepareTask`, `PutTaskInput` and `ReadTaskInput` persist input under the
current HOST environment grant. `StartTask`, `ReadTask`, `StopTask` and
`RespondTask` use the existing native Agent engines and their permissions.
An account API bridge and OpenAgents CLI expose observation, permission replies
and Stop using these same records. **API router dispatch, task transcript pages
and mobile task controls remain unimplemented.** The current environment
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
task owner retains its execution receipt and complete input outside this
cache before launching, and reconciles task identity there even after cache
collection. Capacity refusal grants no retry with a new task ID.

The broker/operator client uses existing HOST key handling and correlation:

```sh
coder/tui/coder.sh host execution-rpc --host HOST --browser-key PRIVATE_KEY_FILE \
  --grant GRANT --epoch EPOCH --relay RELAY --request-file REQUEST_JSON
```

The request schema selects preparation or task control. The client validates the
returned host, task, command, input hash and length where supplied. It emits
the shared PreparationView or TaskStatus. Exit 0 means the RPC was accepted;
a task's terminal outcome is its status, not this transport exit code.
Exit 2 means malformed/unsupported,
3 unavailable/transport failure, 4 forbidden/revoked, 5 expired/stale,
6 conflicting intent and 7 capacity/rate limit. It never automatically resends
uncertain work. This internal preparation/dispatch client is separate from the
public OpenAgents account task controls below.

## Resident task control

[`task.proto`](../../proto/openagents/environment/execution/v1alpha1/task.proto)
adds the following closed records through the single `task-wire.coax` codec.
Every field is required, including false, zero, empty and null values.

`TaskRequest.schema` is `openagents.environment.execution.task-request.v1alpha1`.

| Field | Meaning and bound |
| --- | --- |
| `schema` | Exact task-request identity |
| `method` | `StartTask`, `ReadTask`, `StopTask` or `RespondTask`; matches HOST's method |
| `task_id` | 32 lowercase hex; retained task identity |
| `command_id` | 32 lowercase hex for a write; empty on Read |
| `expected_revision` | Exact positive status revision on Respond, at most 2^53−1; otherwise zero |
| `interaction_id` | Exact pending native permission ID, 1–256 UTF-8 bytes on Respond; otherwise empty |
| `allow` | True only for an affirmative Respond; false elsewhere |
| `issued_at_ms` | Original positive logical intent time; zero on Read |
| `expires_at_ms` | Greater than issue and at most 300,000 ms later; zero on Read |
| `route_decision` | Existing router-owned `RouteDecision` object on Start, at most 32,768 bytes; null elsewhere |

The encoded request is at most 40,960 bytes, within existing HOST carriage.
Start's task, command and logical window exactly match its prepared Binding.
The resident normalizes the nested route through `rtr_decision_read/write`;
there is no second router codec. Its host/station, engine, exact model/effort
and original report provenance must match. Unknown hard needs and weights
refuse. Actual current resources, repository identities and router eligibility
are rechecked immediately before sending the native prompt.

`TaskStatus.schema` is `openagents.environment.execution.task-status.v1alpha1`.

| Field | Meaning and bound |
| --- | --- |
| `schema` | Exact task-status identity |
| `binding` | Complete immutable Binding through its existing codec |
| `state` | `queued`, `running`, `waiting`, `completed`, `failed`, `canceled` or `unknown` |
| `revision` | Positive observation revision through 2^53−1 |
| `stop_requested` | Retained Stop intent; independently true or false for any outcome |
| `native_session_id` | Actual SDK session/thread identity, at most 128 UTF-8 bytes; empty until observed |
| `interaction_id` | Exact held native permission ID, 1–256 UTF-8 bytes iff waiting; otherwise empty |
| `error_code` | Nonempty iff failed/unknown; closed codes below |
| `updated_at_ms` | Positive last observation time through 2^53−1 |
| `command_id` | This write's 32-hex ID; empty on Read and in the private record |
| `replay` | True only when observing an already committed exact command; false in the private record |

Error codes are `engine_lost`, `login_required`, `engine_unavailable`,
`model_unavailable`, `record_limit`, `storage_unavailable`, `expired`,
`revoked`, `native_failed`, `resident_restarted`, `route_refused` and
`native_unknown`. Status is at most 8,192 bytes.

Read requires observe, Start/Stop operate, and Respond review on the current
HOST grant. Explicit operator environment enable provides review authority
for that exact enrolled device, as the existing runtime does. Every method
rechecks scope before storage access. Live tasks recheck enrollment and grant
expiry/revocation while pumping; an uncertain interrupted effect stays unknown.

The owner uses the resident's existing four slots, `Agent`, Claude/Codex SDKs,
permission controls and native process cleanup. Claude can select the system
login or an existing managed profile without changing the global selection;
Codex currently supports its system login only. Native `auth status`/`login
status` and actual SDK model discovery precede any prompt. Claude's execution
catalog excludes curated display defaults and preserves exact native model
IDs/effort ladders. Unsupported selections refuse; they never fall back under
the retained Binding.

Before any native process effect, Start privately copies the entire canonical
input and pinned report into
`execution-tasks/<grant>/tasks/<task>/`, reserves
`execution-tasks/<grant>/sessions/<session>`, writes queued status, then
publishes the original Start request at `execution-tasks/<grant>/commands/<command>`
as its commit marker. The directory also retains `binding.json`, `input.json`,
`report.json`, `status.json`, the actual `dispatch-report.json` and a content
prefix `session.jsonl`. Directories/files are 0700/0600. The frozen content log
does not supply task terminal state; TaskStatus owns that fact.

Accepted Start consumes and reclaims the upload stage. Further preparation,
upload or input-cache reads for that accepted task refuse `conflict`; clients
reconcile through ReadTask or the original Start. Relay history cannot recreate
consumed stages. A preparation receipt is not a task execution receipt.

Task/session/command identity cannot be replaced. Start retries only observe;
they never run another agent, even after the original logical window or a
resident restart. Without a surviving active owner, nonterminal status becomes
unknown (`resident_restarted`). Missing or mismatched binding/session/start
markers fail closed. Uncommitted preparations may recover; an accepted Start
cannot be automatically redispatched.

Each grant retains at most 512 tasks and 64 MiB of complete input bytes. Each
task retains at most 512 control intents, reserving its last entry for Stop.
Control intent and global command marker precede an SDK answer/Stop effect.
Respond requires the currently held exact revision and native interaction ID.
Changed command reuse or a stale permission cannot reach the SDK. A repeated
exact command reads current state, including after its logical expiry, under
fresh HOST authorization. A terminal Stop is an idempotent no-op.

The complete input is sent through the existing text-oriented Agent as a
canonical JSON conversation preceded by a fixed context instruction. This
retains every admitted instruction, reference and history byte; only the final
user message is the new task. No historical tool effects are replayed.
Confirmed SDK completion/interruption/failure and native process loss are
distinct observations. A requested Stop alone cannot establish cancellation.
Missing or ambiguous terminal facts remain unknown.

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

The resident consumes these records through authenticated HOST methods and
owns their durable upload and execution receipts. The task transcript contract
still needs structured tool pages suitable for reconnecting clients.
API dispatch, mobile/CLI transcript joins
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

The resident task owner's source qualification is recorded in
[resident tasks](../verification/2026-11-15-resident-tasks.md): 69 signed HOST
checks across Claude/Codex native fixtures, real file effects, held permissions,
Stop, exact replay, restart uncertainty and revoked access; 19 filesystem
observations; and the task/terminal authority gates. These are internal native
fixtures, not installed app/CLI or external model qualification.

## Account task bridge

`GET /api/chat/task/v1alpha1?host=HOST&task=TASK` builds the existing ReadTask
request. `POST /api/chat/task/v1alpha1?host=HOST` consumes the existing complete
TaskRequest for ReadTask, StopTask or RespondTask. Both return the sole
TaskStatus representation above, with `Cache-Control: no-store, private` and
`Vary: Cookie, Authorization`. There is no new response wrapper or wire shape.
StartTask is refused here: a future dispatch service must derive the admitted
input and route from the account's actual turn rather than trust a caller's
Binding or routing decision.

Admission requires a verified person account, its exact enrolled host, and
exactly one account connection for that host/grant pair. A shared ambiguous
enrollment returns 409. Before any control, the broker observes the task under
the current native grant and verifies that the returned Binding's turn and
conversation belong to this account. A connection alone grants no access to a
foreign conversation. Missing connections and foreign bindings return 404.
RespondTask additionally requires current funding and unrestricted write
eligibility; lack of funding does not prohibit observation or Stop. Existing
global account freeze/limited-session policy remains in force. POST requires
the normal same-site check; verified CLI NIP-98 uses its existing transport.

The API checks host/task/command correlation on both native replies and the
entire immutable Binding across preflight and control. It never retries a
native command automatically. Native denial maps to 403, expired/conflicting
commands to 409, capacity to 429, malformed/mismatched replies to 502 and
unobserved transport/process outcomes to 503. A failure after a control may be
uncertain: reconcile with ReadTask or retry the exact retained original intent.
The API's broker key remains a server-side file and never enters a response.

The public `openagents tasks` commands use these same records. A control file
is the primary TaskRequest, mode 0600, atomically replaced and synced before
HTTP, protected against concurrent local creation with the shared file lock.
On reuse, method/task/revision/interaction/allow and any explicit command ID
must still match. The original logical window is retained. The user-selected
file path is an explicit local storage override; keep it private until the
command's result has been reconciled.
