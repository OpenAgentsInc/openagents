# 270. Tool calls and Responses for Inkling 2 Mini

> **Status: unimplemented — design draft.** This chapter specifies Inkling 2
> Mini's tool-call parser, thinking/CoT channel separation, and `/v1/responses`
> serving integration in Psionic. No Inkling tool-call parser or Responses
> handler is implemented here yet.

Inkling 2 Mini's core strength is agentic coding, search, and tool use. A raw
token stream is not enough for an agent harness: the server must separate
reasoning/thinking tokens from final assistant text, parse structured tool
calls reliably, and stream typed events over `/v1/responses`.

Building on [chapter 77](ch077-call-a-tool.md),
[chapter 78](ch078-device-generation-calls-a-tool.md), and
[chapter 79](ch079-serve-responses.md), this chapter wires
the Metal runner from [chapter 269](ch269-inkling-2-mini-on-metal.md) into
Psionic's local Responses server.

## What the chapter will build

1. **Inkling output parser.** State-machine parser that splits Inkling 2 Mini's
   stream into reasoning/CoT deltas, assistant message deltas, and structured
   tool-call items (name, call ID, JSON arguments) without buffering the entire
   response.
2. **Tool-result re-injection.** Format tool results back into Inkling 2 Mini's
   chat template while preserving KV-cache prefix reuse across multi-turn
   agentic loops.
3. **`/v1/responses` streaming endpoint.** Serve Inkling 2 Mini locally over
   Psionic's HTTP `/v1/responses` API with SSE events, usage accounting
   (including effective loop count), and cancellation.

## Acceptance checks

- **Worked trace:** parse a multi-step fixture containing a reasoning block,
  a parallel tool-call block, a tool result turn, and a final answer into exact
  `/v1/responses` SSE events.
- **Independent check:** run a scripted coding-agent loop against the local
  `/v1/responses` server and verify round-trip tool execution and prefix KV
  cache hits.
- **Refused inputs:** reject malformed tool JSON, unknown tool schemas, and
  unterminated reasoning/tool tags with explicit error events rather than
  silent corruption.
