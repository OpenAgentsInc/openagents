import { afterEach, describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { fileURLToPath } from "node:url"
import { Effect } from "effect"
import {
  allowAllKhalaPermissionService,
  makeKhalaToolServices,
  type KhalaPrivacyRedactionResult,
  type KhalaPrivacyRedactionServiceShape,
} from "@openagentsinc/khala-tools"

import {
  assertAllDefaultToolsRegistered,
  createKhalaCodeDesktopToolRegistry,
  expectedKhalaCodeDesktopToolNames,
  khalaCodeDesktopToolCatalog,
  runKhalaCodeDesktopChatTurn as runKhalaCodeDesktopChatTurnWithDefaultRedaction,
  type RunKhalaCodeDesktopChatTurnInput,
} from "../src/bun/khala-chat-runtime"
import { createDuckDuckGoKhalaWebSearchService } from "../src/bun/khala-web-search-service"
import {
  KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  type KhalaCodeDesktopChatTurnEvent,
} from "../src/shared/rpc"

type FetchCall = {
  readonly body: Record<string, unknown>
  readonly headers: Headers
  readonly url: string
}

const tempDirs: string[] = []
const khalaCodeDesktopRoot = fileURLToPath(new URL("..", import.meta.url))
const disabledDefaultChatToolNames = [
  "write_stdin",
  "ask_user",
  "todo_write",
  "view_image",
  "web_fetch",
  "web_search",
  "browser_navigate",
  "browser_click",
  "browser_type",
  "browser_read_text",
  "browser_read_dom",
  "browser_wait_for",
  "browser_screenshot",
] as const

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

function runKhalaCodeDesktopChatTurn(input: RunKhalaCodeDesktopChatTurnInput) {
  return runKhalaCodeDesktopChatTurnWithDefaultRedaction({
    ...input,
    redaction: input.redaction ?? passthroughRedactionService(),
  })
}

async function runBunJson<T>(script: string, cwd: string): Promise<T> {
  const proc = Bun.spawn([process.execPath, "--eval", script], {
    cwd,
    env: process.env,
    stderr: "pipe",
    stdout: "pipe",
  })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (exitCode !== 0) {
    throw new Error(`bun child process failed with exit ${exitCode}\nstdout:\n${stdout}\nstderr:\n${stderr}`)
  }
  const line = stdout.trim().split(/\n/u).filter(Boolean).at(-1)
  if (line === undefined) {
    throw new Error(`bun child process produced no JSON\nstderr:\n${stderr}`)
  }
  return JSON.parse(line) as T
}

describe("Khala Code desktop chat runtime", () => {
  test("enables every Khala tool by default", () => {
    const registry = createKhalaCodeDesktopToolRegistry()
    const definitions = registry.list()
    const catalog = khalaCodeDesktopToolCatalog()

    expect(assertAllDefaultToolsRegistered(definitions)).toBe(true)
    expect(catalog.defaultEnabled).toBe(true)
    expect(catalog.toolCount).toBe(11)
    expect(catalog.tools.map(tool => tool.name)).toEqual([...expectedKhalaCodeDesktopToolNames()])
    expect(catalog.tools.find(tool => tool.name === "codex_spawn")?.authority).toBe("owner_full_access")
    expect(catalog.tools.find(tool => tool.name === "exec_command")?.authority).toBe("shell")
    for (const disabledName of disabledDefaultChatToolNames) {
      expect(catalog.tools.map(tool => tool.name)).not.toContain(disabledName)
    }
  })

  test("does not locally time-limit long chat-turn RPC requests", () => {
    expect(KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS).toBe(Number.POSITIVE_INFINITY)
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
    const providerToolNames = (calls[0]?.body.tools as unknown[]).map(toolName)
    expect(providerToolNames).toContain("read")
    expect(providerToolNames).toContain("exec_command")
    expect(providerToolNames).toContain("codex_spawn")
    for (const disabledName of disabledDefaultChatToolNames) {
      expect(providerToolNames).not.toContain(disabledName)
    }
    const requestMessages = calls[0]?.body.messages as Array<{ content?: string; role?: string }>
    expect(requestMessages[0]).toMatchObject({ role: "system" })
    expect(requestMessages[0]?.content).toContain("first-person plural")
    expect(requestMessages[0]?.content).toContain("we are Khala")
    expect(requestMessages[0]?.content).toContain("We are Khala. How can we help?")
    expect(requestMessages[0]?.content).toContain("usually one or two sentences, then use tools")
    expect(requestMessages[0]?.content).toContain("avoid long front-loaded plans")
    expect(requestMessages[0]?.content).toContain("Pylon/Codex fleet tools")
    expect(requestMessages[0]?.content).toContain("Never end a turn with only tool output")
    expect(requestMessages[0]?.content).toContain("do not infer behavior from filenames alone")
    expect(requestMessages[0]?.content).toContain("If a tool result is truncated")
    expect(requestMessages[1]).toMatchObject({ role: "system" })
    expect(requestMessages[1]?.content).toContain("Available Khala Code Desktop tools:")
    expect(requestMessages[1]?.content).toContain("- read (read):")
    expect(requestMessages[1]?.content).toContain("- codex_spawn (owner_full_access):")
    expect(requestMessages[1]?.content).toContain("without invoking any tool")
    expect(JSON.stringify(result)).not.toContain("agent-token")
  })

  test("redacts user text before provider requests and reveals placeholders locally", async () => {
    const redaction = fakeRedactionService()
    const { calls, fetchFn } = captureFetch([
      { choices: [{ message: { content: "Hello [GIVEN_NAME_1] [SURNAME_1]." } }] },
    ])

    const result = await runKhalaCodeDesktopChatTurn({
      env: { OPENAGENTS_AGENT_TOKEN: "agent-token" },
      fetchFn,
      redaction,
      request: {
        messages: [{ body: "My name is Alex Rivera.", id: "u1", role: "user" }],
        sessionId: "session-redaction",
      },
    })

    expect(result.ok).toBe(true)
    expect(result.messages[0]?.body).toBe("Hello Alex Rivera.")
    const requestMessages = calls[0]?.body.messages as Array<{ content?: string; role?: string }>
    expect(requestMessages.find(message => message.role === "user")?.content)
      .toBe("My name is [GIVEN_NAME_1] [SURNAME_1].")
    expect(JSON.stringify(calls[0]?.body.messages)).not.toContain("Alex Rivera")
  })

  test("uses the default Rampart model redaction before hosted provider requests", async () => {
    const { requestMessages, result } = await runBunJson<{
      readonly requestMessages: Array<{ readonly content?: string; readonly role?: string }>
      readonly result: { readonly messages: Array<{ readonly body?: string }>; readonly ok: boolean }
    }>(
      `
        import { runKhalaCodeDesktopChatTurn } from "./src/bun/khala-chat-runtime.ts";

        const calls = [];
        const fetchFn = async (_input, init) => {
          const body = JSON.parse(String(init?.body ?? "{}"));
          calls.push({ body });
          return new Response(
            JSON.stringify({ choices: [{ message: { content: "Hello [GIVEN_NAME_1] [SURNAME_1]." } }] }),
            { headers: { "content-type": "application/json" } },
          );
        };
        const result = await runKhalaCodeDesktopChatTurn({
          env: { OPENAGENTS_AGENT_TOKEN: "agent-token" },
          fetchFn,
          request: {
            messages: [{
              body: "My name is Alice Johnson. Email alice@example.com. I live at 100 Main Street in Chicago, IL 60601.",
              id: "u1",
              role: "user",
            }],
            sessionId: "session-default-rampart-model-redaction-child",
          },
        });
        console.log(JSON.stringify({ result, requestMessages: calls[0]?.body?.messages ?? [] }));
      `,
      khalaCodeDesktopRoot,
    )

    expect(result.ok).toBe(true)
    expect(result.messages[0]?.body).toBe("Hello Alice Johnson.")
    const userMessage = requestMessages.find(message => message.role === "user")
    expect(userMessage?.content).toContain("[GIVEN_NAME_1] [SURNAME_1]")
    expect(userMessage?.content).toContain("[EMAIL_1]")
    expect(userMessage?.content).toContain("[BUILDING_NUMBER_1] [STREET_NAME_1]")
    expect(userMessage?.content).toContain("Chicago, IL 60601")
    expect(JSON.stringify(requestMessages)).not.toContain("Alice Johnson")
    expect(JSON.stringify(requestMessages)).not.toContain("alice@example.com")
    expect(JSON.stringify(requestMessages)).not.toContain("100 Main Street")
  })

  test("routes request-specific OpenRouter BYOK through hosted Khala", async () => {
    const { calls, fetchFn } = captureFetch([
      { choices: [{ message: { content: "BYOK answer" } }] },
    ])

    const result = await runKhalaCodeDesktopChatTurn({
      env: {
        OPENAGENTS_AGENT_TOKEN: "agent-token",
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
      kind: "hosted_openagents",
      model: "openagents/khala",
      provider: "openrouter",
    })
    expect(calls[0]?.url).toBe("https://openagents.com/api/v1/chat/completions")
    expect(calls[0]?.headers.get("authorization")).toBe("Bearer agent-token")
    expect(calls[0]?.headers.get("x-openagents-provider")).toBe("openrouter")
    expect(calls[0]?.headers.get("x-openagents-provider-key")).toBe("sk-or-secretkey")
    expect(calls[0]?.headers.get("http-referer")).toBeNull()
    expect(calls[0]?.body.model).toBe("openagents/khala")
    expect(calls[0]?.body.stream).toBe(true)
    expect(JSON.stringify(result)).not.toContain("sk-or-secretkey")
    expect(JSON.stringify(result)).not.toContain("agent-token")
  })

  test("does not use OPENROUTER_API_KEY alone as a local backend", async () => {
    let called = false
    const fetchFn = (async () => {
      called = true
      return jsonResponse({})
    }) as unknown as typeof fetch

    const result = await runKhalaCodeDesktopChatTurn({
      env: { OPENROUTER_API_KEY: "sk-or-secretkey" },
      fetchFn,
      request: {
        messages: [{ body: "hello", id: "u1", role: "user" }],
        sessionId: "session-1",
      },
    })

    expect(called).toBe(false)
    expect(result.ok).toBe(false)
    expect(result.backend).toMatchObject({
      credentialSource: "env:OPENROUTER_API_KEY",
      kind: "hosted_openagents",
      model: "openagents/khala",
      provider: "openrouter",
    })
    expect(result.messages[0]?.body).toContain("OPENAGENTS_AGENT_TOKEN")
    expect(result.messages[0]?.body).toContain("cannot run the Khala system locally")
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
    expect(result.messages[0]?.body).toContain("cannot run the Khala system locally")
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
        OPENAGENTS_AGENT_TOKEN: "agent-token",
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
    expect(result.backend.kind).toBe("hosted_openagents")
    expect(result.messages[0]?.role).toBe("system")
    expect(result.messages[0]?.body).toContain("Hosted OpenAgents cloud request failed")
    expect(result.messages[0]?.body).toContain("provider_error")
    expect(result.messages[0]?.body).not.toContain("abcdefghijklmnopqrstuvwxyz")
    expect(result.messages[0]?.body).not.toContain("sk-or-secretsecret")
    expect(result.messages[0]?.body).not.toContain("agent-token")
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

  test("continues local inspection after a truncated ls instead of accepting a final answer", async () => {
    const workspace = await tempWorkspace()
    await writeFile(join(workspace, "README.md"), "Project plan\n", "utf8")
    await writeFile(join(workspace, "alpha.txt"), "alpha\n", "utf8")
    await writeFile(join(workspace, "beta.txt"), "beta\n", "utf8")
    const { calls, fetchFn } = captureFetch([
      {
        choices: [{
          finish_reason: "tool_calls",
          message: {
            content: "",
            tool_calls: [{
              function: {
                arguments: JSON.stringify({ limit: 2, path: "." }),
                name: "ls",
              },
              id: "call_ls",
              type: "function",
            }],
          },
        }],
      },
      { choices: [{ message: { content: "We looked around and this is likely a small repo." } }] },
      {
        choices: [{
          finish_reason: "tool_calls",
          message: {
            content: "",
            tool_calls: [{
              function: {
                arguments: JSON.stringify({ path: "README.md" }),
                name: "read",
              },
              id: "call_read",
              type: "function",
            }],
          },
        }],
      },
      { choices: [{ message: { content: "We read README.md and found the project plan." } }] },
    ])

    const result = await runKhalaCodeDesktopChatTurn({
      env: { OPENAGENTS_AGENT_TOKEN: "agent-token" },
      fetchFn,
      request: {
        messages: [{ body: "look around local files", id: "u1", role: "user" }],
        sessionId: "session-truncated-ls",
      },
      services: makeKhalaToolServices({
        permission: allowAllKhalaPermissionService,
        workingDirectory: workspace,
      }),
    })

    expect(result.ok).toBe(true)
    expect(result.usedTools).toEqual(["ls", "read"])
    expect(result.messages.map(message => message.role)).toEqual(["tool", "tool", "assistant"])
    expect(result.messages[0]?.body).toContain("[ls truncated; refine path or increase limit]")
    expect(result.messages[2]?.body).toBe("We read README.md and found the project plan.")
    expect(JSON.stringify(result.messages)).not.toContain("likely")
    expect(calls).toHaveLength(4)
    expect(calls[2]?.body.tools).toBeArray()
    const retryMessages = calls[2]?.body.messages as Array<{ content?: string; role?: string }>
    expect(retryMessages.at(-1)).toMatchObject({
      content: expect.stringContaining("last directory listing was truncated"),
      role: "user",
    })
  })

  test("does not accept speculative file descriptions from ls-only evidence", async () => {
    const workspace = await tempWorkspace()
    await mkdir(join(workspace, "scripts"))
    await writeFile(
      join(workspace, "scripts", "prepare-apple-fm-bridge.sh"),
      "#!/usr/bin/env bash\nrepo_root=\"$(pwd)\"\n",
      "utf8",
    )
    await writeFile(
      join(workspace, "scripts", "verify-packaged-apple-fm-bridge.ts"),
      "export const bridge = 'verified'\n",
      "utf8",
    )
    const { calls, fetchFn } = captureFetch([
      {
        choices: [{
          finish_reason: "tool_calls",
          message: {
            content: "",
            tool_calls: [{
              function: {
                arguments: JSON.stringify({ path: "scripts" }),
                name: "ls",
              },
              id: "call_ls_scripts",
              type: "function",
            }],
          },
        }],
      },
      { choices: [{ message: { content: "Those likely prepare and verify the Apple bridge." } }] },
      {
        choices: [{
          finish_reason: "tool_calls",
          message: {
            content: "",
            tool_calls: [{
              function: {
                arguments: JSON.stringify({ path: "scripts/prepare-apple-fm-bridge.sh" }),
                name: "read",
              },
              id: "call_read_prepare",
              type: "function",
            }],
          },
        }],
      },
      { choices: [{ message: { content: "We read prepare-apple-fm-bridge.sh and found it sets repo_root from pwd." } }] },
    ])

    const result = await runKhalaCodeDesktopChatTurn({
      env: { OPENAGENTS_AGENT_TOKEN: "agent-token" },
      fetchFn,
      request: {
        messages: [{ body: "look around the scripts folder tell me whats in there", id: "u1", role: "user" }],
        sessionId: "session-ls-only-speculation",
      },
      services: makeKhalaToolServices({
        permission: allowAllKhalaPermissionService,
        workingDirectory: workspace,
      }),
    })

    expect(result.ok).toBe(true)
    expect(result.usedTools).toEqual(["ls", "read"])
    expect(result.messages.map(message => message.role)).toEqual(["tool", "tool", "assistant"])
    expect(result.messages[0]?.body).toContain("prepare-apple-fm-bridge.sh")
    expect(result.messages[0]?.body).toContain("verify-packaged-apple-fm-bridge.ts")
    expect(result.messages[2]?.body).toBe("We read prepare-apple-fm-bridge.sh and found it sets repo_root from pwd.")
    expect(JSON.stringify(result.messages)).not.toContain("likely")
    expect(calls).toHaveLength(4)
    const retryMessages = calls[2]?.body.messages as Array<{ content?: string; role?: string }>
    expect(retryMessages.at(-1)).toMatchObject({
      content: expect.stringContaining("only local-file evidence so far is directory names"),
      role: "user",
    })
  })

  test("redacts local tool result content before feeding it back to the provider", async () => {
    const workspace = await tempWorkspace()
    await writeFile(join(workspace, "fixture.txt"), "Alex Rivera is in the local-only file\n", "utf8")
    const redaction = fakeRedactionService()
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
      { choices: [{ message: { content: "We saw [GIVEN_NAME_1] [SURNAME_1]." } }] },
    ])

    const result = await runKhalaCodeDesktopChatTurn({
      env: { OPENAGENTS_AGENT_TOKEN: "agent-token" },
      fetchFn,
      redaction,
      request: {
        messages: [{ body: "read the fixture", id: "u1", role: "user" }],
        sessionId: "session-redaction-tools",
      },
      services: makeKhalaToolServices({
        permission: allowAllKhalaPermissionService,
        workingDirectory: workspace,
      }),
    })

    expect(result.ok).toBe(true)
    expect(result.messages[0]?.body).toContain("Alex Rivera is in the local-only file")
    expect(result.messages[1]?.body).toBe("We saw Alex Rivera.")
    const secondMessages = calls[1]?.body.messages as Array<{ content?: string; role?: string; tool_call_id?: string }>
    const toolMessage = secondMessages.find(message => message.role === "tool" && message.tool_call_id === "call_read")
    expect(toolMessage?.content).toContain("[GIVEN_NAME_1] [SURNAME_1]")
    expect(toolMessage?.content).not.toContain("Alex Rivera")
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

  test("does not accept a canned Khala greeting as the final answer after tools", async () => {
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
      { choices: [{ message: { content: "We are Khala. How can we help?" } }] },
      { choices: [{ message: { content: "We read fixture.txt and found hello from the local tool." } }] },
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
    expect(result.messages[1]?.body).toBe("We read fixture.txt and found hello from the local tool.")
    expect(result.messages[1]?.body).not.toBe("We are Khala. How can we help?")
    expect(calls).toHaveLength(3)
    expect(calls[2]?.body.tools).toBeUndefined()
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

  test("keeps the DuckDuckGo web search service available outside the default chat tool catalog", async () => {
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

function fakeRedactionService(): KhalaPrivacyRedactionServiceShape {
  const protectText = (text: string) =>
    text.replaceAll("Alex Rivera", "[GIVEN_NAME_1] [SURNAME_1]")
  const revealText = (text: string) => text
    .replaceAll("[GIVEN_NAME_1]", "Alex")
    .replaceAll("[SURNAME_1]", "Rivera")
  const result = (original: string): KhalaPrivacyRedactionResult => {
    const text = protectText(original)
    return {
      engine: "@nationaldesignstudio/rampart",
      mode: "rampart_model",
      placeholders: text.match(/\[[A-Z_]+_\d+\]/gu) ?? [],
      redacted: text !== original,
      redactionRefs: text === original ? [] : ["redaction.khala.rampart.pii"],
      text,
    }
  }
  return {
    protectModelText: text => Effect.succeed(result(text)),
    protectUserText: text => Effect.succeed(result(text)),
    revealForLocalUser: text => Effect.succeed(revealText(text)),
    revealTransform: () => Effect.succeed(new TransformStream<string, string>({
      transform(chunk, controller) {
        controller.enqueue(revealText(chunk))
      },
    })),
  }
}

function passthroughRedactionService(): KhalaPrivacyRedactionServiceShape {
  const result = (text: string): KhalaPrivacyRedactionResult => ({
    engine: "@openagentsinc/khala-tools.regex",
    mode: "regex_only",
    placeholders: [],
    redacted: false,
    redactionRefs: [],
    text,
  })
  return {
    protectModelText: text => Effect.succeed(result(text)),
    protectUserText: text => Effect.succeed(result(text)),
    revealForLocalUser: text => Effect.succeed(text),
    revealTransform: () => Effect.succeed(new TransformStream<string, string>({
      transform(chunk, controller) {
        controller.enqueue(chunk)
      },
    })),
  }
}
