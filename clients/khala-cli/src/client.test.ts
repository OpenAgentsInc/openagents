import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { fetchTokensServed, runChatTurn, submitFeedback } from "./client.js"

describe("Khala client", () => {
  test("submits feedback to the public feedback endpoint", async () => {
    const calls: Array<{ readonly url: string; readonly body: string | null }> = []
    const fakeFetch = (async (url: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
      calls.push({ url: String(url), body: typeof init?.body === "string" ? init.body : null })
      return Response.json({
        schemaVersion: "openagents.khala.feedback.submit.v1",
        createdAt: "2026-06-26T16:30:00.000Z",
        feedbackRef: "khala_feedback:test",
        traceRef: "trace_123",
      }, { status: 201 })
    }) as unknown as typeof fetch

    const response = await Effect.runPromise(submitFeedback({
      baseUrl: "https://example.test",
      clientVersion: "0.1.2",
      feedback: "the transcript disappeared",
      fetch: fakeFetch,
      source: "khala-cli-interactive",
      traceRef: "trace_123",
    }))

    expect(response.feedbackRef).toBe("khala_feedback:test")
    expect(calls[0]?.url).toBe("https://example.test/api/khala/feedback")
    expect(JSON.parse(calls[0]?.body ?? "{}")).toEqual({
      clientVersion: "0.1.2",
      feedback: "the transcript disappeared",
      source: "khala-cli-interactive",
      traceRef: "trace_123",
    })
  })

  test("retries transient inference unavailability before streaming", async () => {
    let calls = 0
    const retryEvents: Array<{ readonly retry: number; readonly maxRetries: number }> = []
    const fakeFetch = (async () => {
      calls += 1
      if (calls === 1) {
        return Response.json({ error: "inference_unavailable" }, { status: 502 })
      }
      return sseResponse([
        'event: delta\ndata: {"text":"ok"}',
        'event: done\ndata: {"done":true}',
        "",
      ].join("\n\n"))
    }) as unknown as typeof fetch

    const result = await Effect.runPromise(runChatTurn({
      baseUrl: "https://example.test",
      fetch: fakeFetch,
      messages: [{ role: "user", content: "hi" }],
      mode: "public",
      onRetry: event => retryEvents.push({
        maxRetries: event.maxRetries,
        retry: event.retry,
      }),
    }))

    expect(result.text).toBe("ok")
    expect(calls).toBe(2)
    expect(retryEvents).toEqual([{ maxRetries: 2, retry: 1 }])
  })

  test("fetches the global Khala tokens-served counter", async () => {
    const calls: Array<string> = []
    const fakeFetch = (async (url: Parameters<typeof fetch>[0]) => {
      calls.push(String(url))
      return Response.json({
        schemaVersion: "openagents.public_khala_tokens_served.v1",
        tokensServed: 1_250_000,
        generatedAt: "2026-06-26T16:45:00.000Z",
        staleness: { composition: "live_at_read", maxStalenessSeconds: 0 },
      })
    }) as unknown as typeof fetch

    const response = await Effect.runPromise(fetchTokensServed({
      baseUrl: "https://example.test",
      fetch: fakeFetch,
    }))

    expect(response.tokensServed).toBe(1_250_000)
    expect(calls).toEqual(["https://example.test/api/khala/tokens"])
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
