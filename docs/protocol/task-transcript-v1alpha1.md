# Native task transcripts v1alpha1

An inert transcript read above the existing signed HOST carriage. The
[schema](../../proto/openagents/environment/execution/v1alpha1/transcript.proto)
adds no methods or fields to the published TaskRequest or session-log profile.
TaskStatus remains authoritative for lifecycle, cancellation and pending
permission identity. The content is the existing `openagents.session.v1` JSONL
prefix, written and read only by `coax/lib/agent/session/log.coax`.

All JSON fields are required. Objects are closed; unknown, duplicate, aliased,
missing and mistyped fields are refused. Integers are exact JSON numbers.

## Read and capture

`TranscriptRequest.schema` is
`openagents.environment.execution.transcript-request.v1alpha1`.

| Field | Contract |
| --- | --- |
| `schema` | Exact identity above |
| `method` | `ReadTaskTranscript`, current HOST observation right |
| `task_id` | 32 lowercase hexadecimal bytes |
| `reader_id` | 32 lowercase hexadecimal bytes; one viewer's read stream |
| `read_id` | 32 lowercase hexadecimal bytes; one immutable capture |
| `sha256` | Empty only for a first request at offset zero; thereafter the capture's 64-byte lowercase hexadecimal digest |
| `offset` | Next UTF-8 byte offset, zero through 4,194,304 |

A viewer retains `reader_id` while refreshing a task. It makes a fresh
`read_id` for a new capture and resumes interrupted pages using the same
reader/read IDs, digest and offset. A new initial read supersedes only that
reader's previous capture. Other viewers use distinct reader IDs. Native work
continues while pages refer to the original captured status and bytes.

`TranscriptCapture.schema` is
`openagents.environment.execution.transcript-capture.v1alpha1`.

| Field | Contract |
| --- | --- |
| `schema` | Exact identity above |
| `reader_id`, `read_id` | Exact originating request identities |
| `status` | Primary TaskStatus, immutable within the capture; empty command ID and replay false |
| `sha256` | SHA256 of all exact JSONL prefix bytes, including newlines; the empty prefix has its ordinary SHA256 |
| `bytes` | Zero through 4,194,304, never an abbreviated byte count |
| `expires_at_ms` | Positive exact integer through 2^53−1; a 30-second lease renewed by a successful page read |

The native service keeps at most four reader captures per grant, 16 globally
and 64 MiB of content. Expired ephemeral captures are reclaimable. Capacity
refuses explicitly; it never evicts another viewer's unexpired capture. A
superseded, expired, missing or mismatched continuation is stale. It grants no
task authority and never starts, stops or resumes execution. Every page still
requires the current native grant; a retained capture cannot bypass revocation.

## Pages and clients

`TranscriptPage.schema` is
`openagents.environment.execution.transcript-page.v1alpha1`.

| Field | Contract |
| --- | --- |
| `schema` | Exact identity above |
| `capture` | Complete primary capture metadata |
| `offset` | Exact requested byte offset |
| `next_offset` | `offset + UTF8(text).length` |
| `complete` | Exactly `next_offset == capture.bytes`; independent of task terminal state |
| `text` | At most 16,384 complete UTF-8 bytes from the captured JSONL prefix; controls other than newline are escaped by the existing log writer |

A page need not start or end on a JSONL record boundary. A nonfinal page must
advance. Clients assemble contiguous pages with the same capture identities,
binding, status revision, total size and digest, verify the complete SHA256,
then decode with the shared inert session reader before replacing their view.
A stale or invalid continuation never mixes old and new captures. They retain
the last complete view while a fresh capture loads. Imported log content never
executes tools. TaskStatus, rather than an invented log end record, determines
whether the task is running, waiting, complete, failed, canceled or unknown.

Permission responses use the captured TaskStatus's exact revision and native
interaction ID. The task owner independently rechecks both, so a displayed
older capture cannot approve a replacement permission request. Stop intent
suppresses approval controls and refuses fresh permission responses; an exact
prior command receipt remains reconcilable.

## Account API and CLI

`GET /api/chat/task/transcript/v1alpha1?host=HOST&task=TASK&reader=READER&read=READ[&sha256=HASH&after=OFFSET]`
returns the sole primary TranscriptPage with `Cache-Control: no-store, private`
and `Vary: Cookie, Authorization`. `after` is the UTF-8 byte offset, default
zero. POST is refused. The existing account task bridge checks the verified
account, exclusive current connection and owned conversation/turn before asking
the native service for content. It also checks the whole immutable Binding
against its preflight status. Broker credentials stay on the server.

`openagents tasks transcript HOST TASK --out FILE` assembles and verifies a
whole capture through this API, then decodes it through the shared inert Session
reader before atomically replacing the mode-0600 export. Capture metadata is
printed as primary JSON. `tasks transcript-page` exposes the same individual
pages with `--reader-id`, `--read-id`, `--sha256` and `--after`; callers keep the
same identities while reconnecting. Refreshing uses a new read ID. CLI and
future mobile consumers share `transcript-client.coax`, rather than a second
transcript parser.

Status: source implementation for
[#13620](https://github.com/OpenAgentsInc/openagents/issues/13620). Native,
HTTP and CLI qualification is recorded separately. Automatic dispatch,
installed mobile integration and a real staging task remain issue acceptance
work. No deployment or mobile qualification is inferred from this document.
