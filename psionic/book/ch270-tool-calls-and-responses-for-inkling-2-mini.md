# 270. Tool calls and Responses for Inkling 2 Mini

Inkling 2 Mini's core strength is agentic coding, search, and tool use. A raw
token stream is not enough for an agent harness: the server must separate
reasoning/thinking tokens from final assistant text, parse structured tool
calls reliably, and stream typed events over `/v1/responses`.

Building on [chapter 77](ch077-call-a-tool.md),
[chapter 78](ch078-device-generation-calls-a-tool.md), and
[chapter 79](ch079-serve-responses.md), this chapter implements Inkling 2 Mini's
reasoning/tool-call parser and `/v1/responses` SSE event formatter in
[`psionic/lib/inkling-responses.coax`](../lib/inkling-responses.coax),
demonstrated by [`psionic/examples/inkling-responses.coax`](../examples/inkling-responses.coax).

```shell
$ coax run psionic/examples/inkling-responses.coax
parsed 3 events: reasoning(10 bytes), tool_call(15 bytes), message(6 bytes)
malformed tool JSON refused
/v1/responses SSE frame formatted: 100 bytes
```

## Reasoning, tool-call, and message separation

`inkling_parse_stream` splits raw Inkling 2 Mini output into typed `reasoning`,
`tool_call`, and `message` segments, rejecting unterminated `<think>` or
`<tool_call>` tags and malformed tool JSON objects.

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

## `/v1/responses` SSE framing

`inkling_format_sse_event` and `inkling_format_sse_usage` format typed SSE
frames including `loop_count` accounting for multi-pass looped execution.

<!-- source: psionic/examples/inkling-responses.coax example -->
```jai
main :: () {
    raw := "<think>check file</think><tool_call>{\"name\":\"read\"}</tool_call>Ready.";
    events: [8]InklingParsedEvent;
    res := inkling_parse_stream(raw, events[..]);
    #assert res.status == InklingRespStatus.ok && res.count == 3;
    print("parsed % events: reasoning(% bytes), tool_call(% bytes), message(% bytes)\n",
        res.count, events[0].len, events[1].len, events[2].len);

    bad := "<tool_call>not_json</tool_call>";
    #assert inkling_parse_stream(bad, events[..]).status == InklingRespStatus.bad_tool_json;
    print("malformed tool JSON refused\n");

    sse: [256]u8;
    n := inkling_format_sse_event(InklingEventKind.reasoning, "check file", sse[..], 0);
    n = inkling_format_sse_usage(120, 45, 3, sse[..], n);
    #assert n > 0;
    print("/v1/responses SSE frame formatted: % bytes\n", n);
}
```
