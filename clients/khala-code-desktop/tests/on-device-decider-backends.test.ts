import { describe, expect, test } from "bun:test"

import { createAppleFmDeciderBackend } from "../src/bun/apple-fm-decider-backend"
import { createGptOssDeciderBackend } from "../src/bun/gpt-oss-decider-backend"

const jsonResponse = (payload: unknown, status = 200): Response =>
  new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json" },
  })

// Minimal fetch fake that routes by URL suffix.
const fakeFetch = (
  routes: Record<string, (init?: RequestInit) => Response>,
): typeof fetch =>
  (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input.toString()
    for (const [suffix, handler] of Object.entries(routes)) {
      if (url.endsWith(suffix)) return handler(init)
    }
    return new Response("not found", { status: 404 })
  }) as typeof fetch

describe("apple-fm decider backend", () => {
  test("probe maps ready helper health to available", async () => {
    const backend = createAppleFmDeciderBackend({
      fetchFn: fakeFetch({
        "/health": () =>
          jsonResponse({ ready: true, model: "apple-foundation-model", message: "available" }),
      }),
    })
    const readiness = await backend.probe()
    expect(readiness.backend).toBe("apple_fm")
    expect(readiness.available).toBe(true)
    expect(readiness.model).toBe("apple-foundation-model")
  })

  test("probe maps a non-ready/unreachable helper to unavailable (no throw)", async () => {
    const backend = createAppleFmDeciderBackend({
      fetchFn: fakeFetch({ "/health": () => jsonResponse({ ready: false }, 200) }),
    })
    expect((await backend.probe()).available).toBe(false)
    const unreachable = createAppleFmDeciderBackend({
      fetchFn: (async () => {
        throw new Error("ECONNREFUSED")
      }) as unknown as typeof fetch,
    })
    expect((await unreachable.probe()).available).toBe(false)
  })

  test("complete posts chat/completions and maps content + usage", async () => {
    const backend = createAppleFmDeciderBackend({
      fetchFn: fakeFetch({
        "/v1/chat/completions": () =>
          jsonResponse({
            model: "apple-foundation-model",
            choices: [{ message: { content: "PONG" } }],
            usage: { promptTokens: 7, completionTokens: 1, totalTokens: 8, truth: "estimated" },
          }),
      }),
    })
    const result = await backend.complete([{ role: "user", content: "say PONG" }])
    expect(result.content).toBe("PONG")
    expect(result.usage.totalTokens).toBe(8)
    expect(result.usage.truth).toBe("estimated")
  })

  test("a sidecar is nudged on probe but never fatal", async () => {
    let nudged = false
    const backend = createAppleFmDeciderBackend({
      sidecar: {
        // eslint-disable-next-line @typescript-eslint/require-await
        readiness: async () => {
          nudged = true
          throw new Error("sidecar declined")
        },
      } as never,
      fetchFn: fakeFetch({ "/health": () => jsonResponse({ ready: true }) }),
    })
    expect((await backend.probe()).available).toBe(true)
    expect(nudged).toBe(true)
  })
})

describe("gpt-oss decider backend (non-Mac drop-in)", () => {
  test("unconfigured endpoint is unavailable, not a crash", async () => {
    const backend = createGptOssDeciderBackend({ env: {} })
    const readiness = await backend.probe()
    expect(readiness.available).toBe(false)
    expect(readiness.detail).toContain("no GPT-OSS endpoint configured")
  })

  test("probe reports available when models are served", async () => {
    const backend = createGptOssDeciderBackend({
      baseUrl: "https://gpt-oss.example/v1",
      fetchFn: fakeFetch({ "/models": () => jsonResponse({ data: [{ id: "gpt-oss-20b" }] }) }),
    })
    const readiness = await backend.probe()
    expect(readiness.available).toBe(true)
    expect(readiness.model).toBe("gpt-oss-20b")
  })

  test("complete maps OpenAI usage shape", async () => {
    const backend = createGptOssDeciderBackend({
      baseUrl: "https://gpt-oss.example/v1",
      fetchFn: fakeFetch({
        "/chat/completions": () =>
          jsonResponse({
            model: "gpt-oss-20b",
            choices: [{ message: { content: "ok" } }],
            usage: { prompt_tokens: 5, completion_tokens: 2, total_tokens: 7 },
          }),
      }),
    })
    const result = await backend.complete([{ role: "user", content: "hi" }])
    expect(result.backend).toBe("gpt_oss")
    expect(result.content).toBe("ok")
    expect(result.usage.totalTokens).toBe(7)
    expect(result.usage.truth).toBe("exact")
  })

  test("reads endpoint + key from env", async () => {
    const backend = createGptOssDeciderBackend({
      env: { KHALA_GPT_OSS_BASE_URL: "https://gpt-oss.env/v1", KHALA_GPT_OSS_API_KEY: "secret" },
      fetchFn: fakeFetch({
        "/models": (init) => {
          const auth = new Headers(init?.headers).get("authorization")
          return jsonResponse({ data: auth === "Bearer secret" ? [{ id: "gpt-oss-20b" }] : [] })
        },
      }),
    })
    expect((await backend.probe()).available).toBe(true)
  })
})
