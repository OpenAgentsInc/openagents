import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, readFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"

import { createClaudeAppSdkChatRuntime } from "../src/bun/claude-app-sdk-chat-runtime"
import { CLAUDE_APP_SDK_GAP_MATRIX } from "../src/bun/claude-app-sdk-gap-matrix"
import {
  CLAUDE_AGENT_SDK_PARITY_VERSION,
  CLAUDE_PARITY_CONTRACT,
} from "../src/bun/claude-parity-contract"
import {
  createClaudeApprovalService,
  createClaudeHeadlessAutoDenyApprovalService,
} from "../src/bun/claude-approvals"
import {
  khalaCodeDesktopClaudeTokenUsageEvent,
  readKhalaCodeDesktopClaudeTokenUsageInboxFlags,
} from "../src/bun/claude-token-usage-telemetry"
import { createClaudeSessionStore } from "../src/bun/claude-session-store"
import { createClaudeThreadItemProjector } from "../src/bun/claude-thread-item-projector"
import {
  decodeKhalaCodeDesktopRpcResult,
  type KhalaCodeDesktopChatTurnEvent,
} from "../src/shared/rpc"

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

async function tempPath(name: string): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "khala-code-claude-"))
  tempDirs.push(dir)
  return join(dir, name)
}

async function* messages(items: readonly unknown[]): AsyncGenerator<unknown> {
  for (const item of items) yield item
}

describe("Claude Agent SDK chat runtime", () => {
  test("projects SDK text reasoning tools and usage into neutral turn events", () => {
    const projector = createClaudeThreadItemProjector({
      desktopSessionId: "desktop-session-1",
      turnId: "desktop-turn-1",
    })

    const first = projector.project({
      type: "assistant",
      uuid: "assistant-message-1",
      session_id: "claude-session-1",
      message: {
        content: [
          { type: "thinking", text: "checking constraints" },
          { type: "text", text: "Hello from Claude" },
          { type: "tool_use", id: "tool-use-1", name: "Bash", input: { command: "pwd" } },
        ],
      },
    })
    const second = projector.project({
      type: "user",
      uuid: "tool-result-message-1",
      session_id: "claude-session-1",
      message: {
        content: [{ type: "tool_result", tool_use_id: "tool-use-1", content: "ok" }],
      },
    })
    const done = projector.project({
      type: "result",
      subtype: "success",
      uuid: "result-1",
      session_id: "claude-session-1",
      usage: {
        input_tokens: 7,
        cache_read_input_tokens: 2,
        output_tokens: 11,
        reasoning_output_tokens: 3,
      },
    })

    expect(first.events.map(event => event.type)).toEqual([
      "message_start",
      "message_delta",
      "message_done",
      "message_start",
      "message_delta",
      "message_done",
      "tool_event",
    ])
    expect(second.events[0]).toMatchObject({
      event: { invocationId: "tool-use-1", kind: "tool_completed" },
      type: "tool_event",
    })
    expect(projector.messages().map(message => message.body)).toEqual([
      "checking constraints",
      "Hello from Claude",
    ])
    expect(done.usage).toEqual({
      cachedInput: 2,
      input: 7,
      output: 11,
      reasoningOutput: 3,
    })
    expect(projector.toolNames()).toEqual(["Bash"])
  })

  test("skips unmodeled SDK messages and renders stream_event deltas", () => {
    const projector = createClaudeThreadItemProjector({
      desktopSessionId: "desktop-session-stream",
      turnId: "desktop-turn-stream",
    })

    expect(projector.project({ type: "rate_limit_event", uuid: "rate-limit-1" }).events).toEqual([])
    projector.project({
      event: {
        content_block: { type: "text" },
        index: 0,
        type: "content_block_start",
      },
      session_id: "claude-session-stream",
      type: "stream_event",
      uuid: "stream-start",
    })
    const delta = projector.project({
      event: {
        delta: { text: "partial text", type: "text_delta" },
        index: 0,
        type: "content_block_delta",
      },
      session_id: "claude-session-stream",
      type: "stream_event",
      uuid: "stream-delta",
    })
    const done = projector.project({
      event: {
        index: 0,
        type: "content_block_stop",
      },
      session_id: "claude-session-stream",
      type: "stream_event",
      uuid: "stream-stop",
    })

    expect(delta.events).toContainEqual({
      delta: "partial text",
      messageId: "desktop-turn-stream-claude-stream-0",
      turnId: "desktop-turn-stream",
      type: "message_delta",
    })
    expect(done.events).toContainEqual({
      messageId: "desktop-turn-stream-claude-stream-0",
      turnId: "desktop-turn-stream",
      type: "message_done",
    })
    expect(projector.messages()).toMatchObject([{ body: "partial text" }])
  })

  test("captures SDK modelUsage from interrupted results", () => {
    const projector = createClaudeThreadItemProjector({
      desktopSessionId: "desktop-session-usage",
      turnId: "desktop-turn-usage",
    })

    const result = projector.project({
      modelUsage: {
        "claude-opus-4": {
          cache_read_input_tokens: 5,
          input_tokens: 10,
          output_tokens: 7,
          reasoning_output_tokens: 2,
        },
      },
      session_id: "claude-session-usage",
      subtype: "interrupted",
      type: "result",
      uuid: "result-usage",
    })

    expect(result.status).toBe("interrupted")
    expect(result.usage).toEqual({
      cachedInput: 5,
      input: 10,
      output: 7,
      reasoningOutput: 2,
    })
  })

  test("persists desktop session mapping in the v1 Claude session store", async () => {
    const statePath = await tempPath("claude-sessions.json")
    const store = createClaudeSessionStore({
      now: () => new Date("2026-07-01T12:00:00.000Z"),
      path: statePath,
    })

    await store.put("desktop-session-1", {
      sessionId: "claude-session-1",
      lastTurnId: "turn-1",
    })

    expect(await store.get("desktop-session-1")).toEqual({
      sessionId: "claude-session-1",
      lastTurnId: "turn-1",
      updatedAt: "2026-07-01T12:00:00.000Z",
    })
    expect(JSON.parse(await readFile(statePath, "utf8"))).toMatchObject({
      schema: "khala-code-desktop.claude-sessions.v1",
      sessions: {
        "desktop-session-1": { sessionId: "claude-session-1" },
      },
    })
  })

  test("starts fresh sessions with sessionId and resumes later turns with resume alone", async () => {
    const statePath = await tempPath("claude-runtime-state.json")
    const events: KhalaCodeDesktopChatTurnEvent[] = []
    const queryCalls: unknown[] = []
    const runtime = createClaudeAppSdkChatRuntime({
      onEvent: event => events.push(event),
      query: input => {
        queryCalls.push(input)
        return messages([
          {
            type: "assistant",
            uuid: "assistant-1",
            session_id: "claude-session-1",
            message: { content: [{ type: "text", text: "First turn" }] },
          },
          { type: "result", subtype: "success", session_id: "claude-session-1" },
        ])
      },
      sessionStore: createClaudeSessionStore({ path: statePath }),
      workingDirectory: "/repo",
    })

    const first = await runtime.startTurn({
      messages: [{ body: "hello", id: "user-1", role: "user" }],
      sessionId: "desktop-session-1",
      turnId: "turn-1",
    })
    const second = await runtime.startTurn({
      messages: [{ body: "again", id: "user-2", role: "user" }],
      sessionId: "desktop-session-1",
      turnId: "turn-2",
    })

    expect(first.backend).toMatchObject({
      kind: "claude_app_sdk",
      runtimeMode: "claude_runtime",
      threadId: "claude-session-1",
    })
    expect(second.backend.threadId).toBe("claude-session-1")
    expect((queryCalls[1] as { options: Record<string, unknown> }).options).toMatchObject({
      resume: "claude-session-1",
    })
    expect((queryCalls[1] as { options: Record<string, unknown> }).options).not.toHaveProperty("sessionId")
    expect(events.map(event => event.type)).toContain("message_delta")
  })

  test("startThread then startTurn sends only the fresh sessionId", async () => {
    const statePath = await tempPath("claude-start-thread-state.json")
    const queryCalls: unknown[] = []
    const runtime = createClaudeAppSdkChatRuntime({
      query: input => {
        queryCalls.push(input)
        return messages([
          { type: "result", subtype: "success", session_id: "desktop-session-started" },
        ])
      },
      sessionStore: createClaudeSessionStore({ path: statePath }),
      workingDirectory: "/repo",
    })

    await runtime.startThread({ sessionId: "desktop-session-started" })
    await runtime.startTurn({
      messages: [{ body: "hello", id: "user-started", role: "user" }],
      sessionId: "desktop-session-started",
      turnId: "turn-started",
    })

    expect((queryCalls[0] as { options: Record<string, unknown> }).options).toMatchObject({
      sessionId: "desktop-session-started",
    })
    expect((queryCalls[0] as { options: Record<string, unknown> }).options).not.toHaveProperty("resume")
  })

  test("interrupts the active SDK query handle", async () => {
    const statePath = await tempPath("claude-interrupt-state.json")
    let interrupted = false
    let release!: () => void
    const released = new Promise<void>(resolve => {
      release = resolve
    })
    let queryStarted!: () => void
    const started = new Promise<void>(resolve => {
      queryStarted = resolve
    })
    const runtime = createClaudeAppSdkChatRuntime({
      query: () => {
        queryStarted()
        return {
          async *[Symbol.asyncIterator]() {
            await released
            yield { type: "result", subtype: "success", session_id: "claude-session-interrupt" }
          },
          interrupt: async () => {
            interrupted = true
            release()
          },
          close: async () => undefined,
        }
      },
      sessionStore: createClaudeSessionStore({ path: statePath }),
      workingDirectory: "/repo",
    })

    const turn = runtime.startTurn({
      messages: [{ body: "wait", id: "user-1", role: "user" }],
      sessionId: "desktop-session-interrupt",
      turnId: "turn-interrupt",
    })
    await started
    await Promise.resolve()
    const interruptedResult = await runtime.interruptTurn({
      sessionId: "desktop-session-interrupt",
      turnId: "turn-interrupt",
    })
    await turn

    expect(interrupted).toBe(true)
    expect(interruptedResult).toMatchObject({
      ok: true,
      desktopTurnId: "turn-interrupt",
    })
  })

  test("does not report interrupt success when the SDK handle has no interrupt method", async () => {
    const statePath = await tempPath("claude-no-interrupt-state.json")
    let release!: () => void
    const released = new Promise<void>(resolve => {
      release = resolve
    })
    let queryStarted!: () => void
    const started = new Promise<void>(resolve => {
      queryStarted = resolve
    })
    const runtime = createClaudeAppSdkChatRuntime({
      query: () => {
        queryStarted()
        return {
          async *[Symbol.asyncIterator]() {
            await released
            yield { type: "result", subtype: "success", session_id: "claude-session-no-interrupt" }
          },
          close: async () => {
            release()
          },
        }
      },
      sessionStore: createClaudeSessionStore({ path: statePath }),
      workingDirectory: "/repo",
    })

    const turn = runtime.startTurn({
      messages: [{ body: "wait", id: "user-no-interrupt", role: "user" }],
      sessionId: "desktop-session-no-interrupt",
      turnId: "turn-no-interrupt",
    })
    await started
    await Promise.resolve()
    const interruptedResult = await runtime.interruptTurn({
      sessionId: "desktop-session-no-interrupt",
      turnId: "turn-no-interrupt",
    })
    await turn

    expect(interruptedResult).toMatchObject({
      ok: false,
      desktopTurnId: "turn-no-interrupt",
      error: "Claude Agent SDK query does not expose interrupt().",
    })
  })

  test("imports the Claude SDK gap matrix into the checked runtime surface", () => {
    expect(CLAUDE_APP_SDK_GAP_MATRIX.map(row => row.id)).toEqual([
      "claude.phase1.chat_stream",
      "claude.phase1.interrupt",
      "claude.phase1.session_resume",
      "claude.phase2.approvals",
      "claude.phase2.telemetry_ingest",
      "claude.phase3.sidebar_catalog",
      "claude.phase3.slash_registry",
      "claude.phase3.parity_contract",
    ])
    expect(CLAUDE_APP_SDK_GAP_MATRIX.filter(row => row.status === "covered").map(row => row.phase))
      .toEqual(["phase_1", "phase_1", "phase_1", "phase_2", "phase_2", "phase_3", "phase_3", "phase_3"])
  })

  test("bridges canUseTool through the Claude approval queue with native decision shapes", async () => {
    const approvalService = createClaudeApprovalService()
    const statePath = await tempPath("claude-approval-state.json")
    let decision: unknown
    let approvalRequested!: () => void
    const approvalReady = new Promise<void>(resolve => {
      approvalRequested = resolve
    })
    const runtime = createClaudeAppSdkChatRuntime({
      approvalService,
      query: input => ({
        async *[Symbol.asyncIterator]() {
          const canUseTool = input.options.canUseTool as (
            toolName: string,
            toolInput: Record<string, unknown>,
            options: Record<string, unknown>,
          ) => Promise<unknown>
          const decisionPromise = canUseTool("Bash", { command: "pwd" }, {
            signal: new AbortController().signal,
            suggestions: [{ toolName: "Bash", rule: "allow" }],
            title: "Claude wants to run pwd",
          })
          approvalRequested()
          decision = await decisionPromise
          yield { type: "result", subtype: "success", session_id: "claude-approval-session" }
        },
      }),
      sessionStore: createClaudeSessionStore({ path: statePath }),
      workingDirectory: "/repo",
    })

    const turn = runtime.startTurn({
      messages: [{ body: "run pwd", id: "user-approval", role: "user" }],
      sessionId: "desktop-approval",
      turnId: "turn-approval",
    })
    await approvalReady
    const pending = approvalService.pending()[0]
    expect(pending).toMatchObject({
      toolName: "Bash",
      options: { title: "Claude wants to run pwd" },
    })
    const suggestedPermissions = pending!.options.suggestions ?? []
    await approvalService.respond(pending!.id, {
      behavior: "allow",
      decisionClassification: "always_allow",
      updatedPermissions: suggestedPermissions,
    })
    await turn

    expect(decision).toEqual({
      behavior: "allow",
      decisionClassification: "always_allow",
      updatedPermissions: [{ toolName: "Bash", rule: "allow" }],
    })
  })

  test("injects khala_fleet into Claude mcpServers at startTurn without config mutation", async () => {
    const statePath = await tempPath("claude-mcp-state.json")
    const queryCalls: unknown[] = []
    const runtime = createClaudeAppSdkChatRuntime({
      env: { KHALA_CODE_DESKTOP_BUN_COMMAND: "bun-test" },
      query: input => {
        queryCalls.push(input)
        return messages([{ type: "result", subtype: "success", session_id: "claude-mcp-session" }])
      },
      repoRoot: "/repo/openagents",
      sessionStore: createClaudeSessionStore({ path: statePath }),
      workingDirectory: "/repo/workspace",
    })

    await runtime.startTurn({
      messages: [{ body: "fleet status", id: "user-mcp", role: "user" }],
      sessionId: "desktop-mcp",
      turnId: "turn-mcp",
    })

    expect((queryCalls[0] as { options: Record<string, unknown> }).options).toMatchObject({
      mcpServers: {
        khala_fleet: {
          args: ["/repo/openagents/clients/khala-code-desktop/src/bun/khala-fleet-mcp-server.ts"],
          command: "bun-test",
          cwd: "/repo/openagents",
          type: "stdio",
        },
      },
    })
  })

  test("reports exact Claude result usage through the desktop token path", async () => {
    const statePath = await tempPath("claude-token-state.json")
    const reports: unknown[] = []
    const runtime = createClaudeAppSdkChatRuntime({
      query: () => messages([{
        modelUsage: {
          "claude-sonnet-4": {
            cacheReadInputTokens: 3,
            inputTokens: 10,
            outputTokens: 7,
            reasoningOutputTokens: 2,
          },
        },
        session_id: "claude-token-session",
        subtype: "success",
        total_cost_usd: 0.01,
        type: "result",
      }]),
      sessionStore: createClaudeSessionStore({ path: statePath }),
      tokenUsageReporter: report => {
        reports.push(report)
        return Effect.void as never
      },
      workingDirectory: "/repo",
    })

    await runtime.startTurn({
      messages: [{ body: "count tokens", id: "user-token", role: "user" }],
      sessionId: "desktop-token",
      turnId: "turn-token",
    })

    expect(reports).toMatchObject([{
      claudeSessionId: "claude-token-session",
      desktopSessionId: "desktop-token",
      desktopTurnId: "turn-token",
      model: "claude-sonnet-4",
      roleRef: "architect",
      totalCostUsd: 0.01,
      usage: {
        cachedInput: 3,
        input: 10,
        output: 7,
        reasoningOutput: 2,
      },
    }])
    expect(khalaCodeDesktopClaudeTokenUsageEvent(reports[0] as never)).toMatchObject({
      provider: "pylon-claude-direct-local",
      roleRef: "architect",
      safeMetadata: {
        roleRef: "architect",
      },
      sourceRoute: "pylon_claude_direct_local",
      tokenCounts: {
        totalTokens: 22,
      },
      usageTruth: "exact",
    })
  })

  test("projects Claude settings from SDK init supportedModels and accountInfo", async () => {
    const statePath = await tempPath("claude-settings-state.json")
    const runtime = createClaudeAppSdkChatRuntime({
      env: { KHALA_CODE_DESKTOP_CLAUDE_PERMISSION_MODE: "plan" },
      query: () => ({
        accountInfo: async () => ({
          apiProvider: "firstParty",
          email: "operator@example.com",
          organization: "OpenAgents",
          subscriptionType: "pro",
        }),
        async *[Symbol.asyncIterator]() {
          yield { subtype: "init", type: "system", model: "claude-sonnet-4" }
          yield { type: "result", subtype: "success", session_id: "claude-settings-session" }
        },
        initializationResult: async () => ({ model: "claude-sonnet-4", permissionMode: "plan" }),
        supportedModels: async () => [{
          description: "Fast coding model",
          displayName: "Claude Sonnet 4",
          supportsEffort: true,
          supportedEffortLevels: ["low", "medium", "high"],
          value: "claude-sonnet-4",
        }],
      }),
      sessionStore: createClaudeSessionStore({ path: statePath }),
      workingDirectory: "/repo",
    })

    await runtime.startTurn({
      messages: [{ body: "settings", id: "user-settings", role: "user" }],
      sessionId: "desktop-settings",
      turnId: "turn-settings",
    })

    await expect(runtime.claudeSettingsRead()).resolves.toMatchObject({
      account: {
        email: "operator@example.com",
        subscriptionType: "pro",
      },
      init: {
        model: "claude-sonnet-4",
        permissionMode: "plan",
      },
      models: {
        selected: {
          displayName: "Claude Sonnet 4",
          value: "claude-sonnet-4",
        },
      },
      ok: true,
    })
  })

  test("backs the Claude sidebar with listSessions and getSessionMessages", async () => {
    const statePath = await tempPath("claude-sidebar-state.json")
    const runtime = createClaudeAppSdkChatRuntime({
      importer: async () => ({
        getSessionMessages: async (sessionId: string) => ({
          messages: [{
            message: { content: [{ type: "text", text: "Recovered transcript" }], role: "assistant" },
            session_id: sessionId,
            uuid: "message-1",
          }],
        }),
        listSessions: async () => ({
          sessions: [{
            createdAt: 10,
            cwd: "/repo",
            id: "claude-sidebar-session",
            preview: "Recovered",
            title: "Sidebar session",
            updatedAt: 20,
          }],
        }),
        query: () => messages([]),
      }),
      sessionStore: createClaudeSessionStore({ path: statePath }),
      workingDirectory: "/repo",
    })

    await expect(runtime.listThreads({ searchTerm: "sidebar" })).resolves.toMatchObject({
      ok: true,
      groups: [{ key: "claude", threadIds: ["claude-sidebar-session"] }],
      threads: [{
        id: "claude-sidebar-session",
        preview: "Recovered",
        source: "claude_agent_sdk",
        title: "Sidebar session",
      }],
    })
    await expect(runtime.readThread({ threadId: "claude-sidebar-session" })).resolves.toMatchObject({
      messages: [{ body: "Recovered transcript", role: "assistant" }],
      modelProvider: "claude",
      threadId: "claude-sidebar-session",
    })
    await expect(runtime.resumeThread({
      sessionId: "desktop-sidebar",
      threadId: "claude-sidebar-session",
    })).resolves.toMatchObject({
      desktopSessionId: "desktop-sidebar",
      messages: [{ body: "Recovered transcript", role: "assistant" }],
      modelProvider: "claude",
      threadId: "claude-sidebar-session",
    })
  })

  test("builds Claude slash registry from supportedCommands init slash_commands and dispatches as prompt", async () => {
    const statePath = await tempPath("claude-slash-state.json")
    const prompts: string[] = []
    const runtime = createClaudeAppSdkChatRuntime({
      importer: async () => ({
        query: () => messages([]),
        supportedCommands: async () => [{ name: "compact", description: "Compact session" }],
      }),
      query: input => {
        prompts.push(input.prompt)
        return {
          async *[Symbol.asyncIterator]() {
            yield {
              slash_commands: [{ name: "init", description: "Init-provided command" }],
              subtype: "init",
              type: "system",
            }
            yield { type: "commands_changed" }
            yield { type: "result", subtype: "success", session_id: "claude-slash-session" }
          },
          supportedCommands: async () => [{ name: "review", description: "Review changes" }],
        }
      },
      sessionStore: createClaudeSessionStore({ path: statePath }),
      workingDirectory: "/repo",
    })

    await runtime.startTurn({
      messages: [{ body: "prime commands", id: "user-slash-prime", role: "user" }],
      sessionId: "desktop-slash",
      turnId: "turn-slash-prime",
    })
    await expect(runtime.slashCommandList()).resolves.toMatchObject({
      commands: expect.arrayContaining([
        expect.objectContaining({ command: "init" }),
        expect.objectContaining({ command: "review" }),
      ]),
      ok: true,
    })

    const dispatchResult = await runtime.slashCommandDispatch({
      raw: "/review src",
      sessionId: "desktop-slash",
      threadId: "claude-slash-session",
    })
    expect(decodeKhalaCodeDesktopRpcResult("slashCommandDispatch", dispatchResult)).toEqual({
      action: "claude_prompt_slash",
      command: "review",
      message: "Dispatched Claude /review.",
      ok: true,
      status: "dispatched",
      threadId: "claude-slash-session",
    })
    expect(prompts.at(-1)).toBe("/review src")
  })

  test("pins the Claude parity contract to the installed SDK range", async () => {
    const packageJson = JSON.parse(await Bun.file(new URL("../package.json", import.meta.url)).text()) as {
      readonly dependencies?: Record<string, string>
    }
    const installedRange = packageJson.dependencies?.["@anthropic-ai/claude-agent-sdk"]
    if (installedRange === undefined) throw new Error("package.json is missing @anthropic-ai/claude-agent-sdk.")
    expect(CLAUDE_AGENT_SDK_PARITY_VERSION).toBe(installedRange)
    expect(CLAUDE_PARITY_CONTRACT.map(row => row.id)).toEqual([
      "claude.phase1.query_stream",
      "claude.phase2.approvals",
      "claude.phase2.telemetry",
      "claude.phase3.sidebar",
      "claude.phase3.slash",
    ])
    expect(CLAUDE_APP_SDK_GAP_MATRIX.filter(row => row.phase === "phase_3").map(row => row.status))
      .toEqual(["covered", "covered", "covered"])
  })

  test("auto-denies Claude approvals in headless mode with a typed reason", async () => {
    const service = createClaudeHeadlessAutoDenyApprovalService("headless cannot prompt")
    await expect(service.canUseTool("Bash", { command: "pwd" }, {})).resolves.toEqual({
      behavior: "deny",
      decisionClassification: "headless_auto_deny",
      interrupt: false,
      message: "headless cannot prompt Tool: Bash.",
    })
    expect(service.pending()).toEqual([])
  })

  test("decodes Claude settings through the concrete RPC schema", async () => {
    const projection = {
      account: {
        apiKeySource: null,
        apiProvider: "firstParty",
        email: "operator@example.com",
        organization: null,
        subscriptionType: "pro",
        tokenSource: null,
      },
      errors: [],
      init: {
        model: "claude-sonnet-4",
        permissionMode: "plan",
        system: { model: "claude-sonnet-4" },
      },
      models: {
        options: [{
          description: null,
          displayName: "Claude Sonnet 4",
          selected: true,
          supportsAdaptiveThinking: null,
          supportsEffort: true,
          supportedEffortLevels: ["low"],
          value: "claude-sonnet-4",
        }],
        selected: {
          description: null,
          displayName: "Claude Sonnet 4",
          selected: true,
          supportsAdaptiveThinking: null,
          supportsEffort: true,
          supportedEffortLevels: ["low"],
          value: "claude-sonnet-4",
        },
      },
      observedAt: "2026-07-01T12:00:00.000Z",
      ok: true,
    }
    expect(decodeKhalaCodeDesktopRpcResult("claudeSettingsRead", projection)).toEqual(projection)
  })

  test("records Claude token persistent failures as Inbox flags", async () => {
    const ledgerPath = await tempPath("claude-token-failure-events.jsonl")
    const reporter = (await import("../src/bun/claude-token-usage-telemetry")).createKhalaCodeDesktopClaudeTokenUsageReporter({
      env: {
        KHALA_CODE_TOKEN_USAGE_BEARER_TOKEN: "test-token",
        KHALA_CODE_TOKEN_USAGE_BASE_URL: "https://example.invalid",
      },
      fetch: async () => new Response("nope", { status: 503 }),
      localLedgerPath: ledgerPath,
    })

    const exit = await Effect.runPromiseExit(reporter({
      claudeSessionId: "claude-failure-session",
      desktopSessionId: "desktop-failure",
      desktopTurnId: "turn-failure",
      model: "claude-sonnet-4",
      observedAt: "2026-07-01T12:00:00.000Z",
      sequence: 1,
      usage: { cachedInput: 0, input: 1, output: 2, reasoningOutput: 0 },
    }))
    expect(exit._tag).toBe("Failure")
    await expect(readKhalaCodeDesktopClaudeTokenUsageInboxFlags({ localLedgerPath: ledgerPath }))
      .resolves.toMatchObject([{
        idempotencyKey: "khala-code-desktop:direct-local-claude:claude-failure-session:turn-failure:1",
        reason: "Claude token usage reporting failed (503)",
      }])
  })
})
