/**
 * Legacy laneless gateway fallback (#8712): the request must carry the live
 * public model slug `openagents/khala` (the stale "openagents-gateway-default"
 * slug was the on-camera 400), and a non-OK response surfaces the bounded
 * response-body detail instead of a bare status.
 */
import { afterEach, beforeEach, describe, expect, test } from "bun:test"

import { completeChatTurn } from "./chat-service.ts"

const savedEnv = {
  token: process.env.OPENAGENTS_AGENT_TOKEN,
  model: process.env.OPENAGENTS_INFERENCE_GATEWAY_MODEL,
}

beforeEach(() => {
  process.env.OPENAGENTS_AGENT_TOKEN = "test-token"
  delete process.env.OPENAGENTS_INFERENCE_GATEWAY_MODEL
})

afterEach(() => {
  if (savedEnv.token === undefined) delete process.env.OPENAGENTS_AGENT_TOKEN
  else process.env.OPENAGENTS_AGENT_TOKEN = savedEnv.token
  if (savedEnv.model === undefined) delete process.env.OPENAGENTS_INFERENCE_GATEWAY_MODEL
  else process.env.OPENAGENTS_INFERENCE_GATEWAY_MODEL = savedEnv.model
})

describe("completeChatTurn", () => {
  test("posts the live public model slug openagents/khala", async () => {
    const bodies: Array<Record<string, unknown>> = []
    const fetchImpl = (async (_url: unknown, init?: RequestInit) => {
      bodies.push(JSON.parse(String(init?.body)) as Record<string, unknown>)
      return new Response(JSON.stringify({ choices: [{ message: { content: "hi there" } }] }), { status: 200 })
    }) as typeof fetch
    const text = await completeChatTurn(
      [{ key: "u1", role: "user", text: "hello", timestamp: "10:00" }],
      fetchImpl,
    )
    expect(text).toBe("hi there")
    expect(bodies[0]!.model).toBe("openagents/khala")
  })

  test("a non-OK response surfaces the bounded response-body detail", async () => {
    const fetchImpl = (async () =>
      new Response(JSON.stringify({ error: { message: "unknown model openagents-gateway-default" } }), { status: 400 })
    ) as unknown as typeof fetch
    await expect(
      completeChatTurn([{ key: "u1", role: "user", text: "hello", timestamp: "10:00" }], fetchImpl),
    ).rejects.toThrow(/The model gateway returned 400\. .*unknown model/)
  })

  test("long error bodies are bounded to 300 characters of detail", async () => {
    const fetchImpl = (async () => new Response("x".repeat(5_000), { status: 500 })) as unknown as typeof fetch
    const error = await completeChatTurn(
      [{ key: "u1", role: "user", text: "hello", timestamp: "10:00" }],
      fetchImpl,
    ).catch((caught: unknown) => caught as Error)
    expect(error).toBeInstanceOf(Error)
    expect((error as Error).message.length).toBeLessThanOrEqual("The model gateway returned 500. ".length + 300)
  })
})
