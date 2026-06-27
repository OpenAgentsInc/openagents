import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { runChatTurn } from "./client.js"
import { parseSseFramesFromText } from "./sse.js"

describe("SSE parsing", () => {
  test("parses event and data fields across standard separators", () => {
    const frames = parseSseFramesFromText([
      ": comment",
      "event: delta",
      'data: {"text":"Hel"}',
      "",
      "event: delta\r",
      'data: {"text":"lo"}\r',
      "\r",
    ].join("\n"))

    expect(frames).toEqual([
      { event: "delta", data: '{"text":"Hel"}' },
      { event: "delta", data: '{"text":"lo"}' },
    ])
  })

  test("streams public Khala delta frames", async () => {
    const calls: Array<{ readonly url: string; readonly body: string | null }> = []
    const fakeFetch = (async (url: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
      calls.push({ url: String(url), body: typeof init?.body === "string" ? init.body : null })
      return sseResponse([
        'event: delta\ndata: {"text":"Hel"}',
        'event: delta\ndata: {"text":"lo"}',
        'event: meta\ndata: {"traceRef":"trace_public","requestedModel":"khala","servedAdapterId":"hydralisk","servedModel":"glm-4.6","usage":{"promptTokens":3,"completionTokens":2,"totalTokens":5}}',
        'event: done\ndata: {"done":true}',
        "",
      ].join("\n\n"))
    }) as unknown as typeof fetch

    const deltas: Array<string> = []
    const result = await Effect.runPromise(runChatTurn({
      mode: "public",
      baseUrl: "https://example.test",
      fetch: fakeFetch,
      messages: [{ role: "user", content: "Hello" }],
      onDelta: (delta) => deltas.push(delta),
    }))

    expect(result.text).toBe("Hello")
    expect(result.traceRef).toBe("trace_public")
    expect(result.metadata.servedAdapterId).toBe("hydralisk")
    expect(result.metadata.usage).toEqual({
      completionTokens: 2,
      promptTokens: 3,
      totalTokens: 5,
    })
    expect(deltas).toEqual(["Hel", "lo"])
    expect(calls[0]?.url).toBe("https://example.test/api/khala/chat")
    expect(JSON.parse(calls[0]?.body ?? "{}")).toEqual({
      messages: [{ role: "user", content: "Hello" }],
    })
  })

  test("keeps public reasoning frames separate from answer text", async () => {
    const fakeFetch = (async () => sseResponse([
      'event: reasoning\ndata: {"text":"thinking in provider channel"}',
      'event: delta\ndata: {"text":"Answer"}',
      'event: done\ndata: {"done":true}',
      "",
    ].join("\n\n"))) as unknown as typeof fetch

    const reasoning: Array<string> = []
    const deltas: Array<string> = []
    const result = await Effect.runPromise(runChatTurn({
      mode: "public",
      baseUrl: "https://example.test",
      fetch: fakeFetch,
      messages: [{ role: "user", content: "Hello" }],
      onDelta: (delta) => deltas.push(delta),
      onReasoning: (delta) => reasoning.push(delta),
    }))

    expect(result.text).toBe("Answer")
    expect(result.reasoningText).toBe("thinking in provider channel")
    expect(deltas).toEqual(["Answer"])
    expect(reasoning).toEqual(["thinking in provider channel"])
  })

  test("streams OpenAI-compatible delta frames", async () => {
    const bodies: Array<Record<string, unknown>> = []
    const fakeFetch = (async (_url: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
      bodies.push(JSON.parse(String(init?.body ?? "{}")) as Record<string, unknown>)
      return sseResponse([
      'data: {"id":"chat_1","model":"openagents/khala","choices":[{"delta":{"content":"Kh"}}]}',
      'data: {"id":"chat_1","model":"openagents/khala","choices":[{"delta":{"content":"ala"}}]}',
      'data: {"id":"chat_1","model":"openagents/khala","choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":4,"completion_tokens":2,"total_tokens":6}}',
      "data: [DONE]",
      "",
    ].join("\n\n"))
    }) as unknown as typeof fetch

    const result = await Effect.runPromise(runChatTurn({
      mode: "api",
      baseUrl: "https://example.test",
      token: "oa_agent_test",
      fetch: fakeFetch,
      messages: [{ role: "user", content: "Hello" }],
    }))

    expect(result.text).toBe("Khala")
    expect(result.metadata.finishReason).toBe("stop")
    expect(result.metadata.usage.totalTokens).toBe(6)
    expect(result.metadata.servedModel).toBe("openagents/khala")
    expect(bodies[0]?.max_tokens).toBe(8192)
  })

  test("continues OpenAI-compatible chats that stop at finish_reason length", async () => {
    const bodies: Array<{ readonly messages?: ReadonlyArray<{ readonly role?: string; readonly content?: string }> }> = []
    const fakeFetch = (async (_url: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
      bodies.push(JSON.parse(String(init?.body ?? "{}")))
      if (bodies.length === 1) {
        return sseResponse([
          'data: {"id":"chat_1","model":"openagents/khala","choices":[{"delta":{"content":"Which of these paths were"},"finish_reason":"length"}]}',
          'data: {"id":"chat_1","model":"openagents/khala","choices":[],"usage":{"prompt_tokens":4,"completion_tokens":5,"total_tokens":9}}',
          "data: [DONE]",
          "",
        ].join("\n\n"))
      }
      return sseResponse([
        'data: {"id":"chat_2","model":"openagents/khala","choices":[{"delta":{"content":" changed depends on the selected task."},"finish_reason":"stop"}]}',
        'data: {"id":"chat_2","model":"openagents/khala","choices":[],"usage":{"prompt_tokens":8,"completion_tokens":7,"total_tokens":15}}',
        "data: [DONE]",
        "",
      ].join("\n\n"))
    }) as unknown as typeof fetch

    const deltas: Array<string> = []
    const result = await Effect.runPromise(runChatTurn({
      mode: "api",
      baseUrl: "https://example.test",
      token: "oa_agent_test",
      fetch: fakeFetch,
      messages: [{ role: "user", content: "Summarize changed paths" }],
      onDelta: delta => deltas.push(delta),
    }))

    expect(result.text).toBe("Which of these paths were changed depends on the selected task.")
    expect(result.metadata.finishReason).toBe("stop")
    expect(deltas).toEqual([
      "Which of these paths were",
      " changed depends on the selected task.",
    ])
    expect(bodies.length).toBe(2)
    expect(bodies[1]?.messages?.at(-2)).toEqual({
      role: "assistant",
      content: "Which of these paths were",
    })
    expect(bodies[1]?.messages?.at(-1)?.content).toContain("Continue the previous answer")
  })

  test("extracts Khala orchestration metadata from OpenAI-compatible openagents receipt", async () => {
    const fakeFetch = (async () => sseResponse([
      'data: {"id":"chat_1","model":"openagents/khala","choices":[{"delta":{"content":"Hi"}}]}',
      'data: {"id":"chat_1","model":"openagents/khala","choices":[],"openagents":{"served_model":"poolside/laguna-m.1-20260312:free","worker":"openrouter-khala-glm-fallback","routing":{"fallback_reason":"rate_limited"}},"usage":{"prompt_tokens":4,"completion_tokens":1,"total_tokens":5}}',
      "data: [DONE]",
      "",
    ].join("\n\n"))) as unknown as typeof fetch

    const result = await Effect.runPromise(runChatTurn({
      mode: "api",
      baseUrl: "https://example.test",
      token: "oa_agent_test",
      fetch: fakeFetch,
      messages: [{ role: "user", content: "Hello" }],
    }))

    expect(result.metadata.requestedModel).toBe("openagents/khala")
    expect(result.metadata.servedModel).toBe("poolside/laguna-m.1-20260312:free")
    expect(result.metadata.servedAdapterId).toBe("openrouter-khala-glm-fallback")
    expect(result.metadata.fallbackReason).toBe("rate_limited")
  })

  test("keeps OpenAI-compatible reasoning_content separate from answer text", async () => {
    const fakeFetch = (async () => sseResponse([
      'data: {"id":"chat_1","model":"openagents/khala","choices":[{"delta":{"reasoning_content":"internal"}}]}',
      'data: {"id":"chat_1","model":"openagents/khala","choices":[{"delta":{"content":"Visible"}}]}',
      'data: {"id":"chat_1","model":"openagents/khala","choices":[],"usage":{"prompt_tokens":4,"completion_tokens":2,"total_tokens":6}}',
      "data: [DONE]",
      "",
    ].join("\n\n"))) as unknown as typeof fetch

    const reasoning: Array<string> = []
    const result = await Effect.runPromise(runChatTurn({
      mode: "api",
      baseUrl: "https://example.test",
      token: "oa_agent_test",
      fetch: fakeFetch,
      messages: [{ role: "user", content: "Hello" }],
      onReasoning: (delta) => reasoning.push(delta),
    }))

    expect(result.text).toBe("Visible")
    expect(result.reasoningText).toBe("internal")
    expect(reasoning).toEqual(["internal"])
  })
})

function sseResponse(text: string): Response {
  return new Response(new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(new TextEncoder().encode(text))
      controller.close()
    },
  }), {
    status: 200,
    headers: { "content-type": "text/event-stream" },
  })
}
