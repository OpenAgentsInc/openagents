import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"
import {
  allowAllKhalaPermissionService,
  makeKhalaToolServices,
} from "@openagentsinc/khala-tools"

import {
  assertAllDefaultToolsRegistered,
  createKhalaCodeDesktopToolRegistry,
  expectedKhalaCodeDesktopToolNames,
  khalaCodeDesktopToolCatalog,
  runKhalaCodeDesktopChatTurn,
} from "../src/bun/khala-chat-runtime"
import { createDuckDuckGoKhalaWebSearchService } from "../src/bun/khala-web-search-service"
import type { KhalaCodeDesktopChatTurnEvent } from "../src/shared/rpc"

type FetchCall = {
  readonly body: Record<string, unknown>
  readonly headers: Headers
  readonly url: string
}

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

async function tempWorkspace(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "khala-code-desktop-chat-"))
  tempDirs.push(dir)
  return dir
}

function jsonResponse(body: unknown, init?: ResponseInit): Response {
  return new Response(JSON.stringify(body), {
    ...init,
    headers: {
      "content-type": "application/json",
      ...(init?.headers ?? {}),
    },
  })
}

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

function captureFetch(responses: readonly unknown[]): {
  readonly calls: FetchCall[]
  readonly fetchFn: typeof fetch
} {
  const calls: FetchCall[] = []
  let index = 0
  return {
    calls,
    fetchFn: (async (input: string | URL | Request, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url
      const headers = new Headers(init?.headers)
      const body = JSON.parse(String(init?.body ?? "{}")) as Record<string, unknown>
      calls.push({ body, headers, url })
      const response = responses[index] ?? responses.at(-1) ?? { choices: [{ message: { content: "ok" } }] }
      index += 1
      return jsonResponse(response)
    }) as typeof fetch,
  }
}

describe("Khala Code desktop chat runtime", () => {
  test("enables every Khala tool by default", () => {
    const registry = createKhalaCodeDesktopToolRegistry()
    const definitions = registry.list()
    const catalog = khalaCodeDesktopToolCatalog()

    expect(assertAllDefaultToolsRegistered(definitions)).toBe(true)
    expect(catalog.defaultEnabled).toBe(true)
    expect(catalog.toolCount).toBe(21)
    expect(catalog.tools.map(tool => tool.name)).toEqual([...expectedKhalaCodeDesktopToolNames()])
    expect(catalog.tools.find(tool => tool.name === "browser_screenshot")?.authority).toBe("browser")
    expect(catalog.tools.find(tool => tool.name === "web_search")?.authority).toBe("network")
  })

  test("uses hosted OpenAgents chat completions by default with the full tool catalog", async () => {
    const { calls, fetchFn } = captureFetch([
      { choices: [{ message: { content: "Hosted answer" } }] },
    ])

    const result = await runKhalaCodeDesktopChatTurn({
      env: { OPENAGENTS_AGENT_TOKEN: "agent-token" },
      fetchFn,
      request: {
        messages: [{ body: "hello", id: "u1", role: "user" }],
        sessionId: "session-1",
      },
    })

    expect(result.ok).toBe(true)
    expect(result.backend.kind).toBe("hosted_openagents")
    expect(result.messages[0]?.body).toBe("Hosted answer")
    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe("https://openagents.com/api/v1/chat/completions")
    expect(calls[0]?.headers.get("authorization")).toBe("Bearer agent-token")
    expect(calls[0]?.body.stream).toBe(true)
    expect(calls[0]?.body.stream_options).toEqual({ include_usage: true })
    expect((calls[0]?.body.tools as unknown[]).map(toolName)).toContain("browser_screenshot")
    expect((calls[0]?.body.tools as unknown[]).map(toolName)).toContain("web_search")
    const requestMessages = calls[0]?.body.messages as Array<{ content?: string; role?: string }>
    expect(requestMessages[0]).toMatchObject({ role: "system" })
    expect(requestMessages[0]?.content).toContain("first-person PLURAL")
    expect(requestMessages[0]?.content).toContain("we are Khala")
    expect(requestMessages[0]?.content).toContain("We are Khala. How can we help?")
    expect(requestMessages[0]?.content).toContain("Never end a turn with only tool output")
    expect(JSON.stringify(result)).not.toContain("agent-token")
  })

  test("uses OpenRouter BYOK when OPENROUTER_API_KEY is present", async () => {
    const { calls, fetchFn } = captureFetch([
      { choices: [{ message: { content: "BYOK answer" } }] },
    ])

    const result = await runKhalaCodeDesktopChatTurn({
      env: {
        OPENROUTER_API_KEY: "sk-or-secretkey",
        OPENROUTER_MODEL: "anthropic/claude-haiku",
      },
      fetchFn,
      request: {
        messages: [{ body: "hello", id: "u1", role: "user" }],
        sessionId: "session-1",
      },
    })

    expect(result.ok).toBe(true)
    expect(result.backend).toMatchObject({
      credentialSource: "env:OPENROUTER_API_KEY",
      kind: "openrouter_byok",
      model: "anthropic/claude-haiku",
      provider: "openrouter",
    })
    expect(calls[0]?.url).toBe("https://openrouter.ai/api/v1/chat/completions")
    expect(calls[0]?.headers.get("authorization")).toBe("Bearer sk-or-secretkey")
    expect(calls[0]?.headers.get("http-referer")).toBe("https://openagents.com")
    expect(calls[0]?.headers.get("x-openrouter-title")).toBe("Khala Code")
    expect(calls[0]?.headers.get("x-openrouter-categories")).toBe(
      "cli-agent,cloud-agent,personal-agent,programming-app",
    )
    expect(calls[0]?.body.model).toBe("anthropic/claude-haiku")
    expect(calls[0]?.body.stream).toBe(true)
    expect(JSON.stringify(result)).not.toContain("sk-or-secretkey")
  })

  test("uses Granite as the default OpenRouter BYOK model", async () => {
    const { calls, fetchFn } = captureFetch([
      { choices: [{ message: { content: "BYOK answer" } }] },
    ])

    const result = await runKhalaCodeDesktopChatTurn({
      env: { OPENROUTER_API_KEY: "sk-or-secretkey" },
      fetchFn,
      request: {
        messages: [{ body: "hello", id: "u1", role: "user" }],
        sessionId: "session-1",
      },
    })

    expect(result.ok).toBe(true)
    expect(result.backend).toMatchObject({
      kind: "openrouter_byok",
      model: "ibm-granite/granite-4.1-8b",
    })
    expect(calls[0]?.body.model).toBe("ibm-granite/granite-4.1-8b")
  })

  test("streams OpenAI-compatible assistant deltas over chat turn events", async () => {
    const calls: FetchCall[] = []
    const events: KhalaCodeDesktopChatTurnEvent[] = []
    const fetchFn = (async (input: string | URL | Request, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url
      const headers = new Headers(init?.headers)
      const body = JSON.parse(String(init?.body ?? "{}")) as Record<string, unknown>
      calls.push({ body, headers, url })
      return sseResponse([
        'data: {"id":"chat_1","model":"openagents/khala","choices":[{"delta":{"content":"Hel"}}]}',
        'data: {"id":"chat_1","model":"openagents/khala","choices":[{"delta":{"content":"lo"}}]}',
        'data: {"id":"chat_1","model":"openagents/khala","choices":[{"delta":{},"finish_reason":"stop"}]}',
        "data: [DONE]",
        "",
      ].join("\n\n"))
    }) as typeof fetch

    const result = await runKhalaCodeDesktopChatTurn({
      env: { OPENAGENTS_AGENT_TOKEN: "agent-token" },
      fetchFn,
      onEvent: event => events.push(event),
      request: {
        messages: [{ body: "hello", id: "u1", role: "user" }],
        sessionId: "session-1",
        turnId: "turn-stream",
      },
    })

    expect(result.ok).toBe(true)
    expect(result.messages[0]?.body).toBe("Hello")
    expect(calls[0]?.headers.get("accept")).toContain("text/event-stream")
    expect(calls[0]?.body.stream).toBe(true)
    expect(events.map(event => event.type)).toEqual([
      "message_start",
      "message_delta",
      "message_delta",
      "message_done",
    ])
    expect(events.filter(event => event.type === "message_delta").map(event => event.delta)).toEqual(["Hel", "lo"])
    expect(events[0]?.type === "message_start" ? events[0].message.role : undefined).toBe("assistant")
  })

  test("returns an honest setup message instead of faking a hosted response without a token", async () => {
    let called = false
    const fetchFn = (async () => {
      called = true
      return jsonResponse({})
    }) as unknown as typeof fetch

    const result = await runKhalaCodeDesktopChatTurn({
      env: {},
      fetchFn,
      request: {
        messages: [{ body: "hello", id: "u1", role: "user" }],
        sessionId: "session-1",
      },
    })

    expect(called).toBe(false)
    expect(result.ok).toBe(false)
    expect(result.backend.kind).toBe("hosted_openagents")
    expect(result.messages[0]?.body).toContain("OPENAGENTS_AGENT_TOKEN")
    expect(result.messages[0]?.body).toContain("OPENROUTER_API_KEY")
  })

  test("retries a hosted provider error without tool declarations for plain chat", async () => {
    const calls: FetchCall[] = []
    let index = 0
    const fetchFn = (async (input: string | URL | Request, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url
      const headers = new Headers(init?.headers)
      const body = JSON.parse(String(init?.body ?? "{}")) as Record<string, unknown>
      calls.push({ body, headers, url })
      index += 1
      if (index === 1) {
        return jsonResponse(
          { error: "provider_error", reason: "tool parser rejected the request" },
          { status: 502 },
        )
      }
      return jsonResponse({ choices: [{ message: { content: "We are Khala Code." } }] })
    }) as typeof fetch

    const result = await runKhalaCodeDesktopChatTurn({
      env: { OPENAGENTS_AGENT_TOKEN: "agent-token" },
      fetchFn,
      request: {
        messages: [{ body: "who are you", id: "u1", role: "user" }],
        sessionId: "session-1",
      },
    })

    expect(result.ok).toBe(true)
    expect(result.messages[0]?.body).toBe("We are Khala Code.")
    expect(calls).toHaveLength(2)
    expect(calls[0]?.body.tools).toBeArray()
    expect(calls[1]?.body.tools).toBeUndefined()
    expect(calls[1]?.body.tool_choice).toBeUndefined()
  })

  test("returns a sanitized provider failure message instead of throwing", async () => {
    const fetchFn = (async () =>
      jsonResponse(
        {
          error: "provider_error",
          reason: "upstream said Bearer abcdefghijklmnopqrstuvwxyz and sk-or-secretsecret failed",
        },
        { status: 401 },
      )) as unknown as typeof fetch

    const result = await runKhalaCodeDesktopChatTurn({
      env: {
        OPENROUTER_API_KEY: "sk-or-secretsecret",
        OPENROUTER_MODEL: "anthropic/claude-haiku",
      },
      fetchFn,
      request: {
        messages: [{ body: "hello", id: "u1", role: "user" }],
        sessionId: "session-1",
      },
    })

    expect(result.ok).toBe(false)
    expect(result.backend.kind).toBe("openrouter_byok")
    expect(result.messages[0]?.role).toBe("system")
    expect(result.messages[0]?.body).toContain("OpenRouter request failed")
    expect(result.messages[0]?.body).toContain("provider_error")
    expect(result.messages[0]?.body).not.toContain("abcdefghijklmnopqrstuvwxyz")
    expect(result.messages[0]?.body).not.toContain("sk-or-secretsecret")
  })

  test("executes model tool calls locally and feeds results back to the model", async () => {
    const workspace = await tempWorkspace()
    await writeFile(join(workspace, "fixture.txt"), "hello from the local tool\n", "utf8")
    const events: KhalaCodeDesktopChatTurnEvent[] = []
    const { calls, fetchFn } = captureFetch([
      {
        choices: [{
          finish_reason: "tool_calls",
          message: {
            content: "",
            tool_calls: [{
              function: {
                arguments: JSON.stringify({ path: "fixture.txt" }),
                name: "read",
              },
              id: "call_read",
              type: "function",
            }],
          },
        }],
      },
      { choices: [{ message: { content: "We read fixture.txt." } }] },
    ])

    const result = await runKhalaCodeDesktopChatTurn({
      env: { OPENAGENTS_AGENT_TOKEN: "agent-token" },
      fetchFn,
      onEvent: event => events.push(event),
      request: {
        messages: [{ body: "read the fixture", id: "u1", role: "user" }],
        sessionId: "session-1",
        turnId: "turn-tools",
      },
      services: makeKhalaToolServices({
        permission: allowAllKhalaPermissionService,
        workingDirectory: workspace,
      }),
    })

    expect(result.ok).toBe(true)
    expect(result.usedTools).toEqual(["read"])
    expect(result.messages.map(message => message.role)).toEqual(["tool", "assistant"])
    expect(result.messages[0]?.body).toContain("read: ok")
    expect(result.messages[0]?.body).toContain("\n\n")
    expect(result.messages[0]?.body).toContain("hello from the local tool")
    expect(result.messages[1]?.body).toBe("We read fixture.txt.")
    expect(events.some(event =>
      event.type === "message_start" &&
      event.message.role === "tool" &&
      event.message.body === "read: running"
    )).toBe(true)
    expect(events.some(event =>
      event.type === "message_replace" &&
      event.message.role === "tool" &&
      event.message.body.includes("hello from the local tool")
    )).toBe(true)
    expect(calls).toHaveLength(2)
    const secondMessages = calls[1]?.body.messages as Array<{ content?: string; role?: string; tool_call_id?: string }>
    expect(secondMessages.some(message =>
      message.role === "tool" &&
      message.tool_call_id === "call_read" &&
      message.content?.includes("hello from the local tool") === true
    )).toBe(true)
  })

  test("forces a visible final answer when the model stops after tool output", async () => {
    const workspace = await tempWorkspace()
    await writeFile(join(workspace, "fixture.txt"), "hello from the local tool\n", "utf8")
    const { calls, fetchFn } = captureFetch([
      {
        choices: [{
          finish_reason: "tool_calls",
          message: {
            content: "",
            tool_calls: [{
              function: {
                arguments: JSON.stringify({ path: "fixture.txt" }),
                name: "read",
              },
              id: "call_read",
              type: "function",
            }],
          },
        }],
      },
      { choices: [{ message: { content: "" } }] },
      { choices: [{ message: { content: "We found fixture.txt and it says hello from the local tool." } }] },
    ])

    const result = await runKhalaCodeDesktopChatTurn({
      env: { OPENAGENTS_AGENT_TOKEN: "agent-token" },
      fetchFn,
      request: {
        messages: [{ body: "read the fixture", id: "u1", role: "user" }],
        sessionId: "session-1",
      },
      services: makeKhalaToolServices({
        permission: allowAllKhalaPermissionService,
        workingDirectory: workspace,
      }),
    })

    expect(result.ok).toBe(true)
    expect(result.usedTools).toEqual(["read"])
    expect(result.messages.map(message => message.role)).toEqual(["tool", "assistant"])
    expect(result.messages[1]?.body).toBe("We found fixture.txt and it says hello from the local tool.")
    expect(calls).toHaveLength(3)
    expect(calls[2]?.body.tools).toBeUndefined()
    expect(calls[2]?.body.tool_choice).toBeUndefined()
    const forcedMessages = calls[2]?.body.messages as Array<{ content?: string; role?: string; tool_call_id?: string }>
    expect(forcedMessages.some(message =>
      message.role === "tool" &&
      message.tool_call_id === "call_read" &&
      message.content?.includes("hello from the local tool") === true
    )).toBe(true)
    expect(forcedMessages.at(-1)).toMatchObject({
      content: expect.stringContaining("answer the user's request now"),
      role: "user",
    })
  })

  test("carries previous tool cards into the next model request as context", async () => {
    const { calls, fetchFn } = captureFetch([
      { choices: [{ message: { content: "We learned the prior listing included README.md and src." } }] },
    ])

    const result = await runKhalaCodeDesktopChatTurn({
      env: { OPENAGENTS_AGENT_TOKEN: "agent-token" },
      fetchFn,
      request: {
        messages: [
          { body: "list files", id: "u1", role: "user" },
          { body: "ls: ok\n\nREADME.md\nsrc/", id: "t1", role: "tool" },
          { body: "what did you learn", id: "u2", role: "user" },
        ],
        sessionId: "session-1",
      },
    })

    expect(result.ok).toBe(true)
    expect(result.messages[0]?.body).toBe("We learned the prior listing included README.md and src.")
    const requestMessages = calls[0]?.body.messages as Array<{ content?: string; role?: string }>
    expect(requestMessages.some(message =>
      message.role === "assistant" &&
      message.content?.includes("Previous tool result") === true &&
      message.content?.includes("README.md") === true &&
      message.content?.includes("src/") === true
    )).toBe(true)
  })

  test("backs desktop web_search with bounded DuckDuckGo instant-answer results", async () => {
    const urls: string[] = []
    const fetchFn = (async (input: string | URL | Request) => {
      const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url
      urls.push(url)
      return jsonResponse({
        AbstractText: "Khala is an OpenAgents coding agent.",
        AbstractURL: "https://openagents.com/khala",
        Heading: "Khala",
        RelatedTopics: [{
          FirstURL: "https://openagents.com/docs",
          Text: "OpenAgents docs - Khala setup",
        }],
      })
    }) as unknown as typeof fetch

    const service = createDuckDuckGoKhalaWebSearchService(fetchFn)
    const result = await Effect.runPromise(service.search({
      domains: ["openagents.com"],
      limit: 2,
      query: "khala desktop",
      recencyDays: 7,
    }))

    expect(result.provider).toBe("duckduckgo-instant-answer")
    expect(result.results).toHaveLength(2)
    expect(result.results[0]).toMatchObject({
      title: "Khala",
      url: "https://openagents.com/khala",
    })
    expect(decodeURIComponent(urls[0] ?? "")).toContain("site:openagents.com")
    expect(decodeURIComponent(urls[0] ?? "")).toContain("after:")
  })
})

function toolName(value: unknown): string | undefined {
  if (typeof value !== "object" || value === null) return undefined
  const fn = (value as { function?: { name?: unknown } }).function
  return typeof fn?.name === "string" ? fn.name : undefined
}
