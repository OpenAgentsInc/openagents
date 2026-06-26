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
    expect(deltas).toEqual(["Hel", "lo"])
    expect(calls[0]?.url).toBe("https://example.test/api/khala/chat")
    expect(JSON.parse(calls[0]?.body ?? "{}")).toEqual({
      messages: [{ role: "user", content: "Hello" }],
    })
  })

  test("streams OpenAI-compatible delta frames", async () => {
    const fakeFetch = (async () => sseResponse([
      'data: {"choices":[{"delta":{"content":"Kh"}}]}',
      'data: {"choices":[{"delta":{"content":"ala"}}]}',
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

    expect(result.text).toBe("Khala")
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
