# 270. Tool calls and Responses for Inkling 2 Mini

Inkling 2 Mini's core strength is agentic coding, search, and tool use. A raw
token stream is not enough for an agent harness: the server must separate
native reasoning/thinking segments from final assistant text, parse structured
tool calls reliably, constrain strict JSON-schema outputs so `openagents/core`
(`oa-worker.coax`), Inky (`coax/lib/inky/`), and structured callers never
receive markdown fences, raw tool-call JSON leaks, or prose wrappers, and
stream typed Open Responses events over `/v1/responses` on both Metal 4 and
CUDA.

Building on [chapter 77](ch077-call-a-tool.md),
[chapter 78](ch078-device-generation-calls-a-tool.md),
[chapter 79](ch079-serve-responses.md), and
[chapter 269](ch269-inkling-2-mini-on-metal.md), this chapter implements:

- the `#authority` native reasoning/tool-call parser, strict top-level JSON
  schema validator, and Open Responses SSE formatter in
  [`psionic/lib/inkling-responses.coax`](../lib/inkling-responses.coax),
- the live Metal 4 `/v1/responses` and `/v1/chat/completions` server with
  multi-turn `function_call` / `function_call_output`, KV-prefix reuse, and
  constrained RFC 8259 JSON-object token sampling in
  [`psionic/platform/inkling-serve.c`](../platform/inkling-serve.c), and
- the runnable verification example in
  [`psionic/examples/inkling-responses.coax`](../examples/inkling-responses.coax).

```shell
$ coax run psionic/examples/inkling-responses.coax
parsed 3 native events: reasoning(12 bytes), tool_call(17 bytes), message(10 bytes)
malformed tool JSON refused
strict next_action JSON schema verified (markdown fences refused)
/v1/responses SSE frame formatted: 253 bytes (loop_count=3)
```

## Native token framing and tool-call parsing

Inkling 2 Mini emits native control tokens rather than XML tags:

```text
<|message_model|><|content_thinking|>...<|end_message|>
<|message_model|>[tool_name]<|content_invoke_tool_json|>{...}<|end_message|>
<|message_model|><|content_text|>...<|end_message|><|content_model_end_sampling|>
```

`inkling_parse_native_stream` splits this wire stream into typed `reasoning`,
`tool_call`, and `message` spans while refusing unterminated thinking or tool
blocks, unbalanced or malformed tool JSON objects, trailing garbage after a
tool object, and output-slice overflow. `inkling_parse_stream` keeps
compatibility with `<think>` / `<tool_call>` wire tests.

<!-- source: psionic/lib/inkling-responses.coax types -->
```jai
InklingRespStatus :: enum u8 {ok;unterminated_think;unterminated_tool;bad_tool_json;capacity;}
InklingEventKind :: enum u8 {reasoning;tool_call;message;}
InklingParsedEvent :: struct {
    kind:InklingEventKind;
    start:int;
    len:int;
}
InklingParseResult :: struct {
    status:InklingRespStatus;
    count:int;
}
```

## Strict JSON schema & agent-loop compatibility

Agent callers (`openagents/core/` via `coder/tui/oa-worker.coax`, Inky via
[`coax/lib/inky/loop.coax`](../../coax/lib/inky/loop.coax), and gateway
clients via `coax/lib/gateway/psionic.coax` `gw_psionic_strict`; Microcoder is
now deprecated `#13983`) request strict structured JSON through either:

- Open Responses `text.format` (`type: "json_schema"` or `"json_object"`), or
- Chat Completions `response_format` (`type: "json_schema"` or `"json_object"`).

In [`psionic/platform/inkling-serve.c`](../platform/inkling-serve.c), when
strict JSON mode is active:

1. `format_inkling_prompt` injects the requested JSON schema contract into the
   system instructions and suppresses thinking tokens so generation begins
   directly with `{`.
2. `sample_constrained_json` drives a byte-level incremental RFC 8259 JSON
   object lexer (`JsonLex`) over every candidate token during decode. When the
   unconstrained top sample is already valid JSON grammar continuation, the
   fast path runs in O(1); otherwise it scans the logit vector and picks the
   highest-scoring grammar-valid token, refusing empty first-token replies and
   stopping cleanly as soon as the root JSON object closes (`JL_DONE`).
3. `inkling_json_schema_check` in
   [`psionic/lib/inkling-responses.coax`](../lib/inkling-responses.coax) verifies
   balanced root-object framing and presence of every required top-level key
   while rejecting markdown fences, trailing prose, and nested-only keys.

## `/v1/responses` SSE framing and multi-turn KV reuse

`inkling_format_sse_event` and `inkling_format_sse_usage` emit real Open
Responses SSE event names (`response.reasoning_summary_text.delta`,
`response.function_call_arguments.delta`, `response.output_text.delta`, and
`response.completed`) with dynamic `input_tokens`, `output_tokens`,
`total_tokens`, and `loop_count` accounting for multi-pass looped execution.

In [`psionic/platform/inkling-serve.c`](../platform/inkling-serve.c),
`/v1/responses` and `/v1/chat/completions` parse `input[]` items of type
`function_call` and `function_call_output` (`call_id`, `name`, `arguments`,
`output`) alongside `reasoning.effort` and `tools[]`, stream live thinking
tokens without Nagle or poll stalls (`d388bfb3f4`,
[`coax/lib/inky/run-poll-host.coax`](../../coax/lib/inky/run-poll-host.coax)),
retain post-decode KV cache and native tool-error propagation across turns
(`0fe366840e`), suppress raw tool-call JSON leaks under `tool_choice=none`
(`e7c3a80ae8`), maintain a multi-slot SWA-backed prefix cache with opt-in
prompt-lookup verification on both Metal 4 and CUDA (`71d5d3d92a`), and report
the active engine `loop_count` in `usage`.

<!-- source: psionic/examples/inkling-responses.coax example -->
```jai
main :: () {
    raw := "<|message_model|><|content_thinking|>check Austin<|end_message|><|message_model|>get_weather<|content_invoke_tool_json|>{\"city\":\"Austin\"}<|end_message|><|message_model|><|content_text|>78F sunny.<|end_message|><|content_model_end_sampling|>";
    events: [8]InklingParsedEvent;
    res := inkling_parse_native_stream(raw, events[..]);
    #assert res.status == InklingRespStatus.ok && res.count == 3;
    print("parsed % native events: reasoning(% bytes), tool_call(% bytes), message(% bytes)\n",
        res.count, events[0].len, events[1].len, events[2].len);

    bad := "<|message_model|><|content_invoke_tool_json|>not_json<|end_message|>";
    #assert inkling_parse_native_stream(bad, events[..]).status == InklingRespStatus.bad_tool_json;
    print("malformed tool JSON refused\n");

    req := [3]string.["rationale", "finished", "reply"];
    next_action := "{\"rationale\":\"2+2=4\",\"finished\":true,\"reply\":\"4\"}";
    #assert inkling_json_schema_check(next_action, req[..]);
    #assert !inkling_json_schema_check("```json\n{\"rationale\":\"a\",\"finished\":true,\"reply\":\"4\"}\n```", req[..]);
    print("strict next_action JSON schema verified (markdown fences refused)\n");

    sse: [512]u8;
    n := inkling_format_sse_event(InklingEventKind.reasoning, "check Austin", sse[..], 0);
    n = inkling_format_sse_event(InklingEventKind.tool_call, "{\"city\":\"Austin\"}", sse[..], n);
    n = inkling_format_sse_usage(120, 45, 3, sse[..], n);
    #assert n > 0;
    print("/v1/responses SSE frame formatted: % bytes (loop_count=3)\n", n);
}
```

## Live Metal 4 verification

Verified live against the real 17.63 GB `Inkling-2-Mini-IQ4_NL.gguf` checkpoint
on Apple M5 Max using `psionic-inkling-serve --port 18799 --ctx 4096`:

```text
1. POST /v1/responses (stream=true):
   36 response.reasoning_summary_text.delta events, response.output_text.delta,
   response.completed with usage.loop_count == 3.
2. POST /v1/responses (multi-turn tools):
   Turn 1 emitted function_call get_weather({"city":"Austin"}).
   Turn 2 with function_call_output returned
   "Austin is sunny with a temperature of 78°F right now." (loop_count=3).
3. POST /v1/responses (text.format json_schema = Microcoder next_action):
   {"rationale":"...","finished":true,"reply":"4"} (zero fences or prose).
4. POST /v1/chat/completions (response_format json_schema = Microcoder next_action):
   {"rationale":"...","finished":true,"reply":"42"} (zero fences or prose).
5. #authority mutation sweep:
   coax mutate psionic/lib/inkling-responses.coax --changed origin/main
   -> 354 mutants, 351 killed, 0 survived, 3 marked equivalent.
```
