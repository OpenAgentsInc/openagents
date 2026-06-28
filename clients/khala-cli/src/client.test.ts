import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { fetchTokensServed, runArtanisTurn, runChatTurn, submitFeedback } from "./client.js"
import { KhalaCliError } from "./types.js"

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
    expect(retryEvents).toEqual([{ maxRetries: 5, retry: 1 }])
  })

  test("reports first-byte, first-token, stream, and total timing metadata", async () => {
    const fakeFetch = (async () =>
      new Response(new ReadableStream<Uint8Array>({
        async start(controller) {
          await Bun.sleep(20)
          controller.enqueue(new TextEncoder().encode('event: delta\ndata: {"text":"ok"}\n\n'))
          await Bun.sleep(10)
          controller.enqueue(new TextEncoder().encode('event: done\ndata: {"done":true}\n\n'))
          controller.close()
        },
      }), {
        status: 200,
        headers: { "content-type": "text/event-stream" },
      })) as unknown as typeof fetch

    const result = await Effect.runPromise(runChatTurn({
      baseUrl: "https://example.test",
      fetch: fakeFetch,
      messages: [{ role: "user", content: "hi" }],
      mode: "public",
    }))

    expect(result.text).toBe("ok")
    expect(result.metadata.timeToFirstByteMs).toBeGreaterThanOrEqual(0)
    expect(result.metadata.timeToFirstTokenMs).toBeGreaterThanOrEqual(1)
    expect(result.metadata.streamDurationMs).toBeGreaterThanOrEqual(1)
    expect(result.metadata.durationMs).toBeGreaterThanOrEqual(result.metadata.timeToFirstTokenMs ?? 0)
  })

  test("sets a high max_tokens budget on authenticated chat completions", async () => {
    const bodies: Array<Record<string, unknown>> = []
    const fakeFetch = (async (_url: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
      bodies.push(JSON.parse(String(init?.body ?? "{}")) as Record<string, unknown>)
      return sseResponse([
        'data: {"id":"chat_test","model":"openagents/khala","choices":[{"delta":{"content":"complete"},"finish_reason":"stop"}]}',
        'data: {"id":"chat_test","model":"openagents/khala","choices":[],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}',
        "data: [DONE]",
        "",
      ].join("\n\n"))
    }) as unknown as typeof fetch

    const result = await Effect.runPromise(runChatTurn({
      baseUrl: "https://example.test",
      fetch: fakeFetch,
      messages: [{ role: "user", content: "hi" }],
      mode: "api",
      token: "oa_agent_test",
    }))

    expect(result.text).toBe("complete")
    expect(bodies[0]?.max_tokens).toBe(8192)
  })

  test("continues a streamed answer when finish_reason reports length", async () => {
    const bodies: Array<{ readonly messages?: ReadonlyArray<{ readonly role: string; readonly content: string }> }> = []
    let calls = 0
    const fakeFetch = (async (_url: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
      calls += 1
      bodies.push(JSON.parse(String(init?.body ?? "{}")) as { readonly messages?: ReadonlyArray<{ readonly role: string; readonly content: string }> })
      if (calls === 1) {
        return sseResponse([
          'data: {"id":"chat_test","model":"openagents/khala","choices":[{"delta":{"content":"Which of these paths were"},"finish_reason":"length"}]}',
          'data: {"id":"chat_test","model":"openagents/khala","choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}',
          "data: [DONE]",
          "",
        ].join("\n\n"))
      }
      return sseResponse([
        'data: {"id":"chat_test","model":"openagents/khala","choices":[{"delta":{"content":" most relevant to your goal?"},"finish_reason":"stop"}]}',
        'data: {"id":"chat_test","model":"openagents/khala","choices":[],"usage":{"prompt_tokens":12,"completion_tokens":6,"total_tokens":18}}',
        "data: [DONE]",
        "",
      ].join("\n\n"))
    }) as unknown as typeof fetch

    const deltas: string[] = []
    const result = await Effect.runPromise(runChatTurn({
      baseUrl: "https://example.test",
      fetch: fakeFetch,
      messages: [{ role: "user", content: "how should I proceed?" }],
      mode: "api",
      onDelta: delta => deltas.push(delta),
      token: "oa_agent_test",
    }))

    expect(calls).toBe(2)
    expect(result.text).toBe("Which of these paths were most relevant to your goal?")
    expect(deltas.join("")).toBe(result.text)
    expect(result.metadata.finishReason).toBe("stop")
    expect(result.metadata.usage.totalTokens).toBe(33)
    expect(bodies[1]?.messages?.at(-2)).toEqual({
      role: "assistant",
      content: "Which of these paths were",
    })
    expect(bodies[1]?.messages?.at(-1)?.content).toContain("Continue the previous answer")
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

  test("posts the Artanis operator channel to the owner-auth endpoint and returns the reply", async () => {
    const calls: Array<{ readonly url: string; readonly auth: string | null; readonly body: string | null }> = []
    const fakeFetch = (async (url: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
      const headers = new Headers(init?.headers)
      calls.push({
        url: String(url),
        auth: headers.get("authorization"),
        body: typeof init?.body === "string" ? init.body : null,
      })
      return Response.json({ reply: "I just pushed the GLM admission fix and opened #6363." })
    }) as unknown as typeof fetch

    const result = await Effect.runPromise(runArtanisTurn({
      baseUrl: "https://example.test",
      fetch: fakeFetch,
      token: "oa_agent_owner",
      messages: [{ role: "user", content: "What are you doing?" }],
    }))

    expect(result.text).toBe("I just pushed the GLM admission fix and opened #6363.")
    expect(calls[0]?.url).toBe("https://example.test/api/operator/artanis/chat")
    expect(calls[0]?.auth).toBe("Bearer oa_agent_owner")
    expect(JSON.parse(calls[0]?.body ?? "{}")).toEqual({
      messages: [{ role: "user", content: "What are you doing?" }],
    })
  })

  test("requires an owner token for the Artanis channel", async () => {
    const error = await Effect.runPromise(
      runArtanisTurn({
        baseUrl: "https://example.test",
        token: "   ",
        messages: [{ role: "user", content: "hi" }],
      }).pipe(Effect.flip),
    )
    expect(error).toBeInstanceOf(KhalaCliError)
    expect(error.code).toBe("missing_token")
  })

  test("surfaces a non-owner 403 from the Artanis channel as a typed error", async () => {
    const fakeFetch = (async () =>
      Response.json({ error: "forbidden", reason: "owner_only" }, { status: 403 })) as unknown as typeof fetch

    const error = await Effect.runPromise(
      runArtanisTurn({
        baseUrl: "https://example.test",
        fetch: fakeFetch,
        token: "oa_agent_not_owner",
        messages: [{ role: "user", content: "hi" }],
      }).pipe(Effect.flip),
    )
    expect(error).toBeInstanceOf(KhalaCliError)
    expect(error.statusCode).toBe(403)
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
