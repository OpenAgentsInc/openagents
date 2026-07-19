# Bounded Agent Client Protocol stdio transport

`@openagentsinc/agent-stdio-transport` is the shared Node transport for local
Agent Client Protocol peers such as `grok agent stdio` and Cursor `agent acp`.
It spawns one resolved executable with an argument array and `shell: false`.
stdout is reserved for NDJSON while stderr is separately bounded and redacted.
This package does not implement Agent Communication Protocol or A2A.

## Lifecycle

| State      | Admission | Meaning                                                                      |
| ---------- | --------- | ---------------------------------------------------------------------------- |
| `startup`  | closed    | executable resolution, optional bounded version probe, and spawn             |
| `running`  | open      | requests, notifications, reverse requests, and responses flow concurrently   |
| `draining` | closed    | optional `session/cancel`, pending rejection, stdin close, bounded exit wait |
| `exited`   | closed    | clean process exit recorded exactly once                                     |
| `failed`   | closed    | crash, parse failure, invalid envelope, overflow, or forced termination      |
| `disposed` | closed    | handlers, timers, streams, and process listeners released                    |

Every instance has a monotonically increasing generation. Pending work stores
that generation, so an old process can never settle a new process's request.
Shutdown is `stdin.end()` → grace period → `SIGTERM` → grace period →
`SIGKILL`. JSON-RPC `$/cancel_request` cancels one request ID. ACP
`session/cancel` remains a distinct semantic notification keyed by session ID.

## Trusted policy defaults

| Limit                        |             Default | Closed behavior                                  |
| ---------------------------- | ------------------: | ------------------------------------------------ |
| line bytes                   |               1 MiB | protocol violation                               |
| accumulated stdout bytes     |               2 MiB | protocol violation                               |
| inbound/outbound queue depth |           256 / 256 | overload/protocol violation                      |
| outbound requests            |                  64 | local overload error                             |
| concurrent reverse requests  |                  16 | JSON-RPC overload error                          |
| notifications per second     |                 512 | protocol violation                               |
| stderr capture               |              64 KiB | retain prefix. Count dropped bytes               |
| private evidence             | 128 entries / 1 MiB | retain hash/size after raw cap                   |
| request / reverse deadlines  |         60 s / 30 s | one timeout outcome. Late response observed only |
| shutdown / terminate grace   |           1 s / 1 s | escalate deterministically                       |

Overrides must be positive safe integers. Framing operates on raw bytes, accepts
LF/CRLF and blank lines, handles fragmented/coalesced reads, and fatally rejects
malformed JSON, arrays, invalid UTF-8, and over-limit frames. Writes are
serialized and honor Node stream backpressure.

Reverse handlers execute outside the parser microtask. Missing handlers return
`-32601`. `AgentStdioHandlerError` supports `-32602` invalid params or an
authority refusal. Internal failures, deadlines, overload, and cancellation
produce structured errors. Integer, string, and null IDs are keyed by exact
JSON type, so `1` and `"1"` cannot collide. Unknown or late responses are
counted and never resolve unrelated work.

## Receipts, secrecy, and evidence

Public receipts include resolved executable identity, sanitized args, env key
names (never values), optional version output, PID/generation, timestamps,
exit/signal/outcome, bounded redacted stderr, and counters. Auth values, cached
tokens, Cursor login state, prompts, file content, and provider metadata are
redacted from public strings. Raw native envelopes are held only in a bounded
private plane and require the instance's explicit evidence capability from
`authorizeNativeEvidence()`.

`getReceipt().counters` exposes current and peak inbound/outbound queue,
outbound request, and reverse-request pressure plus total/max request and
reverse latency. `getTraces()` returns a bounded metadata-only lifecycle trace.
it never includes params, results, prompt text, file content, or environment
values. `getResourceDiagnostics()` is the leak-test surface for pending maps,
queues, buffered bytes, native evidence, and process/stream listeners.

Representative receipt outcomes are: clean EOF (`state: exited`,
`terminalOutcome: clean_exit`), nonzero process exit (`failed`, `crash`), a
request deadline (`running`, `requestsTimedOut` incremented and no connection
terminal outcome), malformed stdout (`failed`, `protocol_violation`), and kill
escalation (`failed`, `forced_termination`). Tests assert each form rather than
inferring it from an exception string.

The fixture suite covers simultaneous outbound and reverse RPC, fragmented and
coalesced framing, CRLF/blank lines, malformed/non-UTF-8 output, in-flight and
reverse overload, handler timeout, late responses, request versus session
cancellation, exact queue/rate/stderr/evidence/deadline boundaries,
redaction/evidence bounds, forced termination, and 20 sequential
start/request/dispose leak cycles with listener/map/queue/native-evidence and
bounded-heap assertions.

```bash
pnpm --dir packages/agent-stdio-transport run typecheck
pnpm --dir packages/agent-stdio-transport run test
```
