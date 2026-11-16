# Shared gateway tool conversations

Implementation work for #13620. The call/result records reuse the already
published `ToolCall` and `ToolResult` in
`proto/openagents/chat/v2alpha1/conversation.proto`. Their fields keep their
identity. The existing durable-text routes continue to accept text only.

`coax/lib/gateway/tool-wire.coax` is the sole standalone codec. Calls contain
`call_id`, `name` and `input_json`; results contain `call_id`, `output` and
`is_error`. Every field is required and the objects are closed. IDs/names are
1–128 UTF-8 bytes without C0 controls. Arguments retain their original JSON
bytes, at most 32,768, and must be a complete object with no duplicate decoded
keys, invalid escapes or incomplete syntax. The shared JSON tree supplies the
16-level depth bound and a 512-node reader budget. Output is at most 65,536
UTF-8 bytes; empty output, embedded zero and a true error flag remain data.
Encoded reads cap call records at 128 KiB and result records at 512 KiB.

Reading a record does not dispatch it. Syntax validation does not establish a
tool schema match or permission. Execution still requires the owning runner's
offered tool, current grant, validated arguments and retained effect identity.
Imported/replayed calls and results remain inert.

The gateway retains native call IDs alongside tool-name and argument deltas.
Chat Completions continuation uses assistant function calls followed by their
correlated tool results; text-only requests keep their existing bytes. Other
provider adapters must explicitly support rich history before receiving it.
They must refuse a shape they would otherwise silently discard.

The history owner admits at most 128 messages and 64 distinct call IDs over
the whole conversation, with at most eight calls in one assistant message.
Each result settles exactly one unresolved prior call. Parallel results may
arrive in either order; another user/assistant message waits for the whole
batch. Duplicate IDs, repeated or foreign results, incomplete batches and
malformed call/result records refuse the request before serialization.
The Chat wire has no separate result error flag; the original owned result
retains `is_error`, and its output is sent as the tool message's content.

`tool-step.coax` owns streamed ID/name/argument fragments. It publishes no
call until the provider's tool finish and a separate successful transport
completion have both been observed. Refusal, a late event, sparse or duplicate
calls, invalid arguments and exhausted bounds poison the entire candidate.
The caller owns the successful transport observation. The native `GwCall`
adapter supplies `complete` only when a valid finish and successful HTTP 200
EOF were both observed; `finished` alone also includes failures and Stop.
Routine cleanup preserves an already observed completion. Calls returned by
the step borrow its storage until its next reset and remain inert.

The pure completion policy copies up to 1,024 combined terminal provider/reason
bytes into its own storage while waiting for EOF. Duplicate or late semantic
events, a trailing incomplete SSE event, a short declared HTTP body, an
oversized line or exhausted body/event buffers refuse the response. No tool
candidate seals from its successful prefix. Native default buffers hold 65,536
bytes per line and 262,144 bytes per whole body; a DTO's individual extent does
not promise that a multi-call response fits those transport bounds. Give the
call adapter at least 16 event slots when receiving up to eight tool calls.

Native process/HTTP effects live in `client.coax`; the public gateway facade
retains its existing entry points and pure codecs. The plan passes tool
fragments as data and does not retry or switch providers once it handed any
tool fragment to its caller. Its native polling effects live in
`plan-client.coax`, alongside the pure decisions in `plan.coax`.

The Chat parser preserves function call IDs and refuses invalid field types,
unsupported tool types and incomplete event/string buffers. A tool delta may
omit fields it does not extend. Request schemas must be JSON objects, and
raw tool declarations must be arrays; these shape checks do not establish
supported JSON Schema semantics or grant execution.

This document describes an implementation in progress. The owned codecs and
gateway transport alone do not establish the durable gateway executor, mobile
adoption, strict schema support, real provider qualification or #13620 completion.
