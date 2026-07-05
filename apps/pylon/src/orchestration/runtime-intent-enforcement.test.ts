import { Database } from "bun:sqlite"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import { decodeFleetAccountEntity, decodeRuntimeControlIntentRow } from "@openagentsinc/khala-sync"
import type { KhalaRuntimeEvent, RuntimeControlIntentRow } from "@openagentsinc/khala-sync"

import { hashPylonAccountRef } from "../account-registry.js"
import {
  candidateAccountsFromRegistry,
  chatMessageIdFromBodyRef,
  claudeRawMessageToRuntimeEvents,
  codexRawEventToRuntimeEvents,
  enforcePendingRuntimeIntents,
  type ActiveRuntimeTurns,
  type CandidateAccount,
  type ClaudeRawMessage,
  type CodexRawEvent,
  type EnforceRuntimeIntentsOptions,
} from "./runtime-intent-enforcement.js"
import type { ChatMessageBody, ReadPendingRuntimeIntentsResult } from "./runtime-intents.js"
import { createPylonOrchestrationStore, type PylonOrchestrationStore } from "./store.js"

describe("candidateAccountsFromRegistry", () => {
  test("projects codex registry entries into ready, one-slot FleetAccountEntity rows (naive mode: no summary given)", async () => {
    const candidates = await candidateAccountsFromRegistry(
      [
        { home: "/tmp/acct-1", hourlyCap: null, manualResetsRemaining: null, openAgentsProviderAccountRef: null, provider: "codex", ref: "acct-1", weeklyCap: null },
      ],
      { now: new Date("2026-07-05T12:00:00.000Z") },
    )
    expect(candidates).toHaveLength(1)
    expect(candidates[0]!.fleetAccount.readiness).toBe("ready")
    expect(candidates[0]!.fleetAccount.capacityAvailable).toBe(1)
    expect(candidates[0]!.fleetAccount.provider).toBe("codex")
    expect(candidates[0]!.registryEntry.ref).toBe("acct-1")
  })

  test("projects claude_agent registry entries into ready, one-slot FleetAccountEntity rows (#8404)", async () => {
    const candidates = await candidateAccountsFromRegistry([
      { home: "/tmp/acct-2", hourlyCap: null, manualResetsRemaining: null, openAgentsProviderAccountRef: null, provider: "claude_agent", ref: "acct-2", weeklyCap: null },
    ])
    expect(candidates).toHaveLength(1)
    expect(candidates[0]!.fleetAccount.readiness).toBe("ready")
    expect(candidates[0]!.fleetAccount.provider).toBe("claude_agent")
    expect(candidates[0]!.registryEntry.ref).toBe("acct-2")
  })

  test("projects both providers together from a mixed registry", async () => {
    const candidates = await candidateAccountsFromRegistry([
      { home: "/tmp/acct-1", hourlyCap: null, manualResetsRemaining: null, openAgentsProviderAccountRef: null, provider: "codex", ref: "acct-1", weeklyCap: null },
      { home: "/tmp/acct-2", hourlyCap: null, manualResetsRemaining: null, openAgentsProviderAccountRef: null, provider: "claude_agent", ref: "acct-2", weeklyCap: null },
    ])
    expect(candidates.map((c) => c.fleetAccount.provider)).toEqual(["codex", "claude_agent"])
  })

  test("with a real summary (#8410 follow-up): a registry account with no local Codex login is excluded (unavailable, not ready)", async () => {
    const workspaceRoot = await mkdtemp(join(tmpdir(), "runtime-intent-enforcement-readiness-"))
    try {
      const summary = { paths: { cache: join(workspaceRoot, "cache"), config: join(workspaceRoot, "config.json"), home: workspaceRoot, releases: join(workspaceRoot, "releases") } }
      const candidates = await candidateAccountsFromRegistry(
        [
          { home: join(workspaceRoot, "accounts", "codex", "no-login"), hourlyCap: null, manualResetsRemaining: null, openAgentsProviderAccountRef: null, provider: "codex", ref: "no-login", weeklyCap: null },
        ],
        { env: {}, summary },
      )
      expect(candidates).toHaveLength(1)
      expect(candidates[0]!.fleetAccount.readiness).not.toBe("ready")
      expect(candidates[0]!.fleetAccount.capacityAvailable).toBe(0)
    } finally {
      await rm(workspaceRoot, { force: true, recursive: true })
    }
  })
})

describe("chatMessageIdFromBodyRef", () => {
  test("extracts the message id from the chat_message.<id> convention", () => {
    expect(chatMessageIdFromBodyRef("chat_message.msg-abc123")).toBe("msg-abc123")
  })

  test("returns null for an unrelated or missing bodyRef", () => {
    expect(chatMessageIdFromBodyRef("prompt.private.something")).toBeNull()
    expect(chatMessageIdFromBodyRef(undefined)).toBeNull()
  })
})

describe("codexRawEventToRuntimeEvents", () => {
  const ctx = () => ({
    allocateSequence: (() => {
      let n = 0
      return () => {
        n += 1
        return n
      }
    })(),
    nowIso: () => "2026-07-05T12:00:00.000Z",
    source: { adapterKind: "codex" as const, lane: "codex_app_server" as const, surface: "server" as const },
    threadId: "thread-1",
    turnId: "turn-1",
    turnStarted: { value: false },
  })

  test("turn.started emits exactly once even if Codex repeats it", () => {
    const c = ctx()
    const first = codexRawEventToRuntimeEvents({ type: "turn.started" }, c)
    const second = codexRawEventToRuntimeEvents({ type: "turn.started" }, c)
    expect(first).toHaveLength(1)
    expect(first[0]!.kind).toBe("turn.started")
    expect(second).toHaveLength(0)
  })

  test("agent_message item completion emits a text delta + completed pair with the same messageId", () => {
    const events = codexRawEventToRuntimeEvents(
      { item: { text: "hello from codex", type: "agent_message" }, type: "item.completed" },
      ctx(),
    )
    expect(events.map((e) => e.kind)).toEqual(["text.delta", "text.completed"])
    const delta = events[0]! as Extract<KhalaRuntimeEvent, { kind: "text.delta" }>
    const completed = events[1]! as Extract<KhalaRuntimeEvent, { kind: "text.completed" }>
    expect(delta.text).toBe("hello from codex")
    expect(delta.messageId).toBe(completed.messageId)
  })

  test("a successful command_execution emits tool.call + tool.result", () => {
    const events = codexRawEventToRuntimeEvents(
      { item: { exit_code: 0, status: "completed", type: "command_execution" }, type: "item.completed" },
      ctx(),
    )
    expect(events.map((e) => e.kind)).toEqual(["tool.call", "tool.result"])
    expect((events[0]! as Extract<KhalaRuntimeEvent, { kind: "tool.call" }>).toolName).toBe("commandExecution")
  })

  test("a failed command_execution emits tool.call + tool.error", () => {
    const events = codexRawEventToRuntimeEvents(
      { item: { exit_code: 1, status: "failed", type: "command_execution" }, type: "item.completed" },
      ctx(),
    )
    expect(events.map((e) => e.kind)).toEqual(["tool.call", "tool.error"])
  })

  test("turn.completed emits usage.recorded + turn.finished(stop) with a required usageRef", () => {
    const events = codexRawEventToRuntimeEvents(
      { type: "turn.completed", usage: { input_tokens: 100, output_tokens: 20, reasoning_output_tokens: 5 } },
      ctx(),
    )
    expect(events.map((e) => e.kind)).toEqual(["usage.recorded", "turn.finished"])
    const usageEvent = events[0]! as Extract<KhalaRuntimeEvent, { kind: "usage.recorded" }>
    expect(usageEvent.usage.usageRef.length).toBeGreaterThan(0)
    expect(usageEvent.usage.totalTokens).toBe(125)
    expect((events[1]! as Extract<KhalaRuntimeEvent, { kind: "turn.finished" }>).finishReason).toBe("stop")
  })

  test("turn.failed emits turn.finished(error)", () => {
    const events = codexRawEventToRuntimeEvents({ type: "turn.failed" }, ctx())
    expect(events).toHaveLength(1)
    expect((events[0]! as Extract<KhalaRuntimeEvent, { kind: "turn.finished" }>).finishReason).toBe("error")
  })

  test("unrecognized event types are ignored", () => {
    expect(codexRawEventToRuntimeEvents({ type: "thread.started" }, ctx())).toEqual([])
  })
})

describe("claudeRawMessageToRuntimeEvents", () => {
  const ctx = () => ({
    allocateSequence: (() => {
      let n = 0
      return () => {
        n += 1
        return n
      }
    })(),
    nowIso: () => "2026-07-05T12:00:00.000Z",
    pendingToolCalls: new Map<string, string>(),
    source: { adapterKind: "claude_code" as const, lane: "claude_pylon" as const, surface: "server" as const },
    threadId: "thread-1",
    turnId: "turn-1",
    turnStarted: { value: false },
  })

  test("system/init emits turn.started exactly once even if Claude repeats it", () => {
    const c = ctx()
    const first = claudeRawMessageToRuntimeEvents({ session_id: "sess-1", subtype: "init", type: "system" }, c)
    const second = claudeRawMessageToRuntimeEvents({ session_id: "sess-1", subtype: "init", type: "system" }, c)
    expect(first).toHaveLength(1)
    expect(first[0]!.kind).toBe("turn.started")
    expect(second).toHaveLength(0)
  })

  test("an assistant text content block emits a text delta + completed pair with the same messageId", () => {
    const events = claudeRawMessageToRuntimeEvents(
      { message: { content: [{ text: "hello from claude", type: "text" }] }, type: "assistant" },
      ctx(),
    )
    expect(events.map((e) => e.kind)).toEqual(["text.delta", "text.completed"])
    const delta = events[0]! as Extract<KhalaRuntimeEvent, { kind: "text.delta" }>
    const completed = events[1]! as Extract<KhalaRuntimeEvent, { kind: "text.completed" }>
    expect(delta.text).toBe("hello from claude")
    expect(delta.messageId).toBe(completed.messageId)
  })

  test("an assistant thinking content block emits a reasoning delta + completed pair", () => {
    const events = claudeRawMessageToRuntimeEvents(
      { message: { content: [{ thinking: "let me think", type: "thinking" }] }, type: "assistant" },
      ctx(),
    )
    expect(events.map((e) => e.kind)).toEqual(["reasoning.delta", "reasoning.completed"])
    expect((events[0]! as Extract<KhalaRuntimeEvent, { kind: "reasoning.delta" }>).text).toBe("let me think")
  })

  test("a tool_use block emits tool.call and records the pending call for later correlation", () => {
    const c = ctx()
    const events = claudeRawMessageToRuntimeEvents(
      { message: { content: [{ id: "toolu_1", input: {}, name: "Bash", type: "tool_use" }] }, type: "assistant" },
      c,
    )
    expect(events.map((e) => e.kind)).toEqual(["tool.call"])
    expect((events[0]! as Extract<KhalaRuntimeEvent, { kind: "tool.call" }>).toolName).toBe("Bash")
    expect(c.pendingToolCalls.get("toolu_1")).toBe("Bash")
  })

  test("a matching tool_result in a later user message emits tool.result with the correlated toolName", () => {
    const c = ctx()
    claudeRawMessageToRuntimeEvents(
      { message: { content: [{ id: "toolu_2", input: {}, name: "Read", type: "tool_use" }] }, type: "assistant" },
      c,
    )
    const events = claudeRawMessageToRuntimeEvents(
      { message: { content: [{ tool_use_id: "toolu_2", type: "tool_result" }] }, type: "user" },
      c,
    )
    expect(events.map((e) => e.kind)).toEqual(["tool.result"])
    expect((events[0]! as Extract<KhalaRuntimeEvent, { kind: "tool.result" }>).toolName).toBe("Read")
  })

  test("a failed tool_result (is_error) emits tool.error", () => {
    const c = ctx()
    claudeRawMessageToRuntimeEvents(
      { message: { content: [{ id: "toolu_3", input: {}, name: "Bash", type: "tool_use" }] }, type: "assistant" },
      c,
    )
    const events = claudeRawMessageToRuntimeEvents(
      { message: { content: [{ is_error: true, tool_use_id: "toolu_3", type: "tool_result" }] }, type: "user" },
      c,
    )
    expect(events.map((e) => e.kind)).toEqual(["tool.error"])
  })

  test("a success result emits usage.recorded + turn.finished(stop) with a required usageRef", () => {
    const events = claudeRawMessageToRuntimeEvents(
      {
        is_error: false,
        subtype: "success",
        type: "result",
        usage: { cache_read_input_tokens: 3, input_tokens: 100, output_tokens: 20 },
      },
      ctx(),
    )
    expect(events.map((e) => e.kind)).toEqual(["usage.recorded", "turn.finished"])
    const usageEvent = events[0]! as Extract<KhalaRuntimeEvent, { kind: "usage.recorded" }>
    expect(usageEvent.usage.usageRef.length).toBeGreaterThan(0)
    expect(usageEvent.usage.totalTokens).toBe(120)
    expect((events[1]! as Extract<KhalaRuntimeEvent, { kind: "turn.finished" }>).finishReason).toBe("stop")
  })

  test("error_max_turns maps to finishReason length", () => {
    const events = claudeRawMessageToRuntimeEvents({ subtype: "error_max_turns", type: "result" }, ctx())
    expect((events[1]! as Extract<KhalaRuntimeEvent, { kind: "turn.finished" }>).finishReason).toBe("length")
  })

  test("error_during_execution maps to finishReason error", () => {
    const events = claudeRawMessageToRuntimeEvents({ subtype: "error_during_execution", type: "result" }, ctx())
    expect((events[1]! as Extract<KhalaRuntimeEvent, { kind: "turn.finished" }>).finishReason).toBe("error")
  })

  test("unrecognized message types are ignored", () => {
    expect(claudeRawMessageToRuntimeEvents({ type: "tool_use_summary" }, ctx())).toEqual([])
  })
})

const iso = "2026-07-05T12:00:00.000Z"

const controlIntentRow = (input: {
  seq: number
  intentId: string
  threadId: string
  turnId?: string
  kind: "turn.start" | "turn.interrupt" | "message.append" | "turn.continue" | "turn.retry" | "turn.close"
  bodyRef?: string
  ownerUserId?: string
  targetLane?: "codex_app_server" | "claude_pylon" | "ai_sdk_core"
}): RuntimeControlIntentRow =>
  decodeRuntimeControlIntentRow({
    createdAt: iso,
    intent: {
      causalityRefs: [],
      createdAt: iso,
      idempotencyKey: `idem.${input.intentId}`,
      intentId: input.intentId,
      kind: input.kind,
      origin: { lane: "khala_sync_mobile_control", surface: "mobile" },
      redactionClass: "private_ref",
      schema: "openagents.khala_runtime_control_intent.v1",
      target: { lane: input.targetLane ?? "codex_app_server" },
      threadId: input.threadId,
      visibility: "private",
      ...(input.turnId === undefined ? {} : { turnId: input.turnId }),
      ...(input.bodyRef === undefined ? {} : { bodyRef: input.bodyRef }),
      ...(input.kind === "message.append" ? { messageId: "runtime-message.fixture.1" } : {}),
    },
    intentId: input.intentId,
    kind: input.kind,
    ownerUserId: input.ownerUserId ?? "user-1",
    seq: input.seq,
    status: "accepted",
    threadId: input.threadId,
    turnId: input.turnId ?? null,
    updatedAt: iso,
  })

const memoryStore = (): PylonOrchestrationStore => createPylonOrchestrationStore(new Database(":memory:"))

const pageReader = (
  intents: ReadonlyArray<RuntimeControlIntentRow>,
): EnforceRuntimeIntentsOptions["readImpl"] =>
  async () =>
    ({ intents, nextAfter: intents[intents.length - 1]?.seq ?? 0, ok: true, upToDate: true }) satisfies ReadPendingRuntimeIntentsResult

const baseAccount = {
  home: "/tmp/pylon-account-1",
  hourlyCap: null,
  manualResetsRemaining: null,
  openAgentsProviderAccountRef: null,
  provider: "codex" as const,
  ref: "acct-1",
  weeklyCap: null,
}

const claudeBaseAccount = {
  home: "/tmp/pylon-account-claude-1",
  hourlyCap: null,
  manualResetsRemaining: null,
  openAgentsProviderAccountRef: null,
  provider: "claude_agent" as const,
  ref: "acct-claude-1",
  weeklyCap: null,
}

const baseOptions = async (overrides: Partial<EnforceRuntimeIntentsOptions> = {}): Promise<{
  options: EnforceRuntimeIntentsOptions
  pushedEvents: Array<KhalaRuntimeEvent>
  cleanup: () => Promise<void>
}> => {
  const workspaceRoot = await mkdtemp(join(tmpdir(), "runtime-intent-enforcement-"))
  const pushedEvents: Array<KhalaRuntimeEvent> = []
  const activeTurns: ActiveRuntimeTurns = new Map()
  const options: EnforceRuntimeIntentsOptions = {
    activeTurns,
    adminToken: "admin-secret",
    agentToken: "agent-secret",
    baseUrl: "https://openagents.com",
    ensureWorkspace: async (threadId) => join(workspaceRoot, threadId),
    fetchChatMessageImpl: async () => ({ message: null, ok: true }),
    listCandidateAccounts: async () => candidateAccountsFromRegistryForTest(),
    log: () => {},
    pushEventImpl: async (input) => {
      pushedEvents.push(input.event)
    },
    pylonRef: "pylon.fixture.1",
    readImpl: pageReader([]),
    resolveAccountSelection: async (entry) => ({
      accountRef: entry.ref,
      accountRefHash: "account.pylon.codex.aaaaaaaaaaaaaaaaaaaaaaaa",
      home: entry.home,
      provider: entry.provider,
      selector: "registry_ref",
    }),
    workspaceRoot,
    ...overrides,
  }
  return {
    cleanup: async () => rm(workspaceRoot, { force: true, recursive: true }),
    options,
    pushedEvents,
  }
}

function candidateAccountsFromRegistryForTest() {
  return candidateAccountsFromRegistry([baseAccount])
}

const fakeCodexRunner = (events: ReadonlyArray<CodexRawEvent>): EnforceRuntimeIntentsOptions["codexThreadRunner"] =>
  async () => ({
    events: (async function* () {
      for (const event of events) yield event
    })(),
  })

const fakeClaudeRunner = (
  messages: ReadonlyArray<ClaudeRawMessage>,
): EnforceRuntimeIntentsOptions["claudeThreadRunner"] =>
  async () => ({
    messages: (async function* () {
      for (const message of messages) yield message
    })(),
  })

describe("enforcePendingRuntimeIntents", () => {
  test("turn.start with an unresolvable bodyRef is recorded failed, no background dispatch launched", async () => {
    const store = memoryStore()
    const { options, cleanup } = await baseOptions({
      readImpl: pageReader([
        controlIntentRow({ intentId: "intent-1", kind: "turn.start", seq: 1, threadId: "thread-1", turnId: "turn-1" }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("failed")
        expect(result.outcomes[0]!.detail).toContain("bodyRef")
      }
      expect(options.activeTurns.size).toBe(0)
    } finally {
      await cleanup()
    }
  })

  test("turn.start with a missing chat_message is recorded failed", async () => {
    const store = memoryStore()
    const { options, cleanup } = await baseOptions({
      fetchChatMessageImpl: async () => ({ message: null, ok: true }),
      readImpl: pageReader([
        controlIntentRow({
          bodyRef: "chat_message.missing-1",
          intentId: "intent-2",
          kind: "turn.start",
          seq: 1,
          threadId: "thread-1",
          turnId: "turn-2",
        }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("failed")
        expect(result.outcomes[0]!.detail).toContain("does not exist")
      }
    } finally {
      await cleanup()
    }
  })

  test("turn.start with no dispatch-ready account is recorded failed", async () => {
    const store = memoryStore()
    const message: ChatMessageBody = {
      authorUserId: "user-1",
      body: "do the thing",
      createdAt: iso,
      deletedAt: null,
      messageId: "msg-1",
      threadId: "thread-1",
      updatedAt: iso,
    }
    const { options, cleanup } = await baseOptions({
      fetchChatMessageImpl: async () => ({ message, ok: true }),
      listCandidateAccounts: async () => [],
      readImpl: pageReader([
        controlIntentRow({
          bodyRef: "chat_message.msg-1",
          intentId: "intent-3",
          kind: "turn.start",
          seq: 1,
          threadId: "thread-1",
          turnId: "turn-3",
        }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("failed")
        expect(result.outcomes[0]!.detail).toContain("no dispatch-ready")
      }
    } finally {
      await cleanup()
    }
  })

  test("a real turn.start dispatch streams translated events end-to-end and finishes", async () => {
    const store = memoryStore()
    const message: ChatMessageBody = {
      authorUserId: "user-1",
      body: "please say hello",
      createdAt: iso,
      deletedAt: null,
      messageId: "msg-1",
      threadId: "thread-1",
      updatedAt: iso,
    }
    let finishSeen: (() => void) | undefined
    const finished = new Promise<void>((resolve) => {
      finishSeen = resolve
    })
    const pushedEvents: Array<KhalaRuntimeEvent> = []
    const { options, cleanup } = await baseOptions({
      codexThreadRunner: fakeCodexRunner([
        { type: "turn.started" },
        { item: { text: "hello!", type: "agent_message" }, type: "item.completed" },
        {
          item: { exit_code: 0, status: "completed", type: "command_execution" },
          type: "item.completed",
        },
        { type: "turn.completed", usage: { input_tokens: 10, output_tokens: 2, reasoning_output_tokens: 0 } },
      ]),
      fetchChatMessageImpl: async () => ({ message, ok: true }),
      pushEventImpl: async (input) => {
        pushedEvents.push(input.event)
        if (input.event.kind === "turn.finished") finishSeen?.()
      },
      readImpl: pageReader([
        controlIntentRow({
          bodyRef: "chat_message.msg-1",
          intentId: "intent-4",
          kind: "turn.start",
          seq: 1,
          threadId: "thread-1",
          turnId: "turn-4",
        }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("applied")
        expect(result.outcomes[0]!.detail).toContain("dispatch started against account")
      }
      await finished
      expect(pushedEvents.map((e) => e.kind)).toEqual([
        "turn.started",
        "text.delta",
        "text.completed",
        "tool.call",
        "tool.result",
        "usage.recorded",
        "turn.finished",
      ])
      const sequences = pushedEvents.map((e) => e.sequence)
      expect(sequences).toEqual([...sequences].sort((a, b) => a - b))
      expect(new Set(sequences).size).toBe(sequences.length)
      expect((pushedEvents[pushedEvents.length - 1]! as Extract<KhalaRuntimeEvent, { kind: "turn.finished" }>).finishReason).toBe("stop")
    } finally {
      await cleanup()
    }
  })

  test("a real turn.start dispatch against target.lane claude_pylon streams translated Claude events end-to-end and finishes (#8404)", async () => {
    const store = memoryStore()
    const message: ChatMessageBody = {
      authorUserId: "user-1",
      body: "please say hello",
      createdAt: iso,
      deletedAt: null,
      messageId: "msg-1",
      threadId: "thread-1",
      updatedAt: iso,
    }
    let finishSeen: (() => void) | undefined
    const finished = new Promise<void>((resolve) => {
      finishSeen = resolve
    })
    const pushedEvents: Array<KhalaRuntimeEvent> = []
    const { options, cleanup } = await baseOptions({
      claudeThreadRunner: fakeClaudeRunner([
        { session_id: "claude-sess-1", subtype: "init", type: "system" },
        { message: { content: [{ text: "hello!", type: "text" }] }, type: "assistant" },
        {
          message: { content: [{ id: "toolu_1", input: {}, name: "Bash", type: "tool_use" }] },
          type: "assistant",
        },
        { message: { content: [{ tool_use_id: "toolu_1", type: "tool_result" }] }, type: "user" },
        {
          is_error: false,
          subtype: "success",
          type: "result",
          usage: { input_tokens: 10, output_tokens: 2 },
        },
      ]),
      fetchChatMessageImpl: async () => ({ message, ok: true }),
      listCandidateAccounts: async () => candidateAccountsFromRegistry([claudeBaseAccount]),
      pushEventImpl: async (input) => {
        pushedEvents.push(input.event)
        if (input.event.kind === "turn.finished") finishSeen?.()
      },
      readImpl: pageReader([
        controlIntentRow({
          bodyRef: "chat_message.msg-1",
          intentId: "intent-4b",
          kind: "turn.start",
          seq: 1,
          targetLane: "claude_pylon",
          threadId: "thread-1",
          turnId: "turn-4b",
        }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("applied")
      }
      await finished
      expect(pushedEvents.map((e) => e.kind)).toEqual([
        "turn.started",
        "text.delta",
        "text.completed",
        "tool.call",
        "tool.result",
        "usage.recorded",
        "turn.finished",
      ])
      expect(pushedEvents.every((e) => e.source.lane === "claude_pylon")).toBe(true)
      expect(pushedEvents.every((e) => e.source.adapterKind === "claude_code")).toBe(true)
      expect((pushedEvents[pushedEvents.length - 1]! as Extract<KhalaRuntimeEvent, { kind: "turn.finished" }>).finishReason).toBe("stop")
      expect(store.getRuntimeClaudeSessionId("thread-1")).toBe("claude-sess-1")
    } finally {
      await cleanup()
    }
  })

  test("turn.start targeting an unsupported target.lane is recorded failed, not silently routed to Codex", async () => {
    const store = memoryStore()
    const message: ChatMessageBody = {
      authorUserId: "user-1",
      body: "irrelevant",
      createdAt: iso,
      deletedAt: null,
      messageId: "msg-1",
      threadId: "thread-1",
      updatedAt: iso,
    }
    const { options, cleanup } = await baseOptions({
      fetchChatMessageImpl: async () => ({ message, ok: true }),
      readImpl: pageReader([
        controlIntentRow({
          bodyRef: "chat_message.msg-1",
          intentId: "intent-4c",
          kind: "turn.start",
          seq: 1,
          targetLane: "ai_sdk_core",
          threadId: "thread-1",
          turnId: "turn-4c",
        }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("failed")
        expect(result.outcomes[0]!.detail).toContain("ai_sdk_core")
      }
      expect(options.activeTurns.size).toBe(0)
    } finally {
      await cleanup()
    }
  })

  test("turn.start against target.lane claude_pylon with no ready Claude account is recorded failed and names Claude", async () => {
    const store = memoryStore()
    const message: ChatMessageBody = {
      authorUserId: "user-1",
      body: "irrelevant",
      createdAt: iso,
      deletedAt: null,
      messageId: "msg-1",
      threadId: "thread-1",
      updatedAt: iso,
    }
    const { options, cleanup } = await baseOptions({
      fetchChatMessageImpl: async () => ({ message, ok: true }),
      listCandidateAccounts: async () => candidateAccountsFromRegistryForTest(), // codex-only registry
      readImpl: pageReader([
        controlIntentRow({
          bodyRef: "chat_message.msg-1",
          intentId: "intent-4d",
          kind: "turn.start",
          seq: 1,
          targetLane: "claude_pylon",
          threadId: "thread-1",
          turnId: "turn-4d",
        }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("failed")
        expect(result.outcomes[0]!.detail).toContain("no dispatch-ready local Claude account")
      }
    } finally {
      await cleanup()
    }
  })

  test("turn.interrupt against an active locally-running turn aborts it and records turn.interrupted", async () => {
    const store = memoryStore()
    const pushedEvents: Array<KhalaRuntimeEvent> = []
    const activeTurns: ActiveRuntimeTurns = new Map()
    let sequenceCounter = 0
    const abortController = new AbortController()
    activeTurns.set("turn-5", {
      abortController,
      clientGroupId: "cg-fixture",
      clientId: "c-fixture",
      interrupted: false,
      lane: "codex_app_server",
      nextEventSequence: () => {
        sequenceCounter += 1
        return sequenceCounter
      },
      nextMutationId: () => 1,
      pendingAppendMessageIds: [],
      threadId: "thread-1",
    })
    const { options, cleanup } = await baseOptions({
      activeTurns,
      pushEventImpl: async (input) => {
        pushedEvents.push(input.event)
      },
      readImpl: pageReader([
        controlIntentRow({
          intentId: "intent-5",
          kind: "turn.interrupt",
          seq: 1,
          threadId: "thread-1",
          turnId: "turn-5",
        }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("applied")
      }
      expect(abortController.signal.aborted).toBe(true)
      expect(pushedEvents.map((e) => e.kind)).toEqual(["turn.interrupted"])
    } finally {
      await cleanup()
    }
  })

  test("turn.interrupt with no locally active turn is skipped_stale", async () => {
    const store = memoryStore()
    const { options, cleanup } = await baseOptions({
      readImpl: pageReader([
        controlIntentRow({
          intentId: "intent-6",
          kind: "turn.interrupt",
          seq: 1,
          threadId: "thread-1",
          turnId: "turn-does-not-exist",
        }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("skipped_stale")
        expect(result.outcomes[0]!.detail).toContain("no locally running turn")
      }
    } finally {
      await cleanup()
    }
  })

  test("message.append with an unresolvable bodyRef is recorded failed", async () => {
    const store = memoryStore()
    const { options, cleanup } = await baseOptions({
      readImpl: pageReader([
        controlIntentRow({ intentId: "intent-7", kind: "message.append", seq: 1, threadId: "thread-1" }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("failed")
        expect(result.outcomes[0]!.detail).toContain("bodyRef")
      }
    } finally {
      await cleanup()
    }
  })

  test("message.append with no turnId (a bare append, not steering) is applied with nothing to attach to", async () => {
    const store = memoryStore()
    const message: ChatMessageBody = {
      authorUserId: "user-1",
      body: "fyi for later",
      createdAt: iso,
      deletedAt: null,
      messageId: "msg-bare",
      threadId: "thread-1",
      updatedAt: iso,
    }
    const { options, cleanup } = await baseOptions({
      fetchChatMessageImpl: async () => ({ message, ok: true }),
      readImpl: pageReader([
        controlIntentRow({
          bodyRef: "chat_message.msg-bare",
          intentId: "intent-7b",
          kind: "message.append",
          seq: 1,
          threadId: "thread-1",
        }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("applied")
        expect(result.outcomes[0]!.detail).toContain("no turn to attach to")
      }
    } finally {
      await cleanup()
    }
  })

  test("message.append targeting a turn that is not currently dispatching locally is skipped_stale, not silently dropped", async () => {
    const store = memoryStore()
    const message: ChatMessageBody = {
      authorUserId: "user-1",
      body: "steer this",
      createdAt: iso,
      deletedAt: null,
      messageId: "msg-steer",
      threadId: "thread-1",
      updatedAt: iso,
    }
    const { options, cleanup } = await baseOptions({
      fetchChatMessageImpl: async () => ({ message, ok: true }),
      readImpl: pageReader([
        controlIntentRow({
          bodyRef: "chat_message.msg-steer",
          intentId: "intent-7c",
          kind: "message.append",
          seq: 1,
          threadId: "thread-1",
          turnId: "turn-not-active",
        }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("skipped_stale")
        expect(result.outcomes[0]!.detail).toContain("not currently dispatching")
      }
    } finally {
      await cleanup()
    }
  })

  test("message.append queued against an actively-dispatching local turn becomes a real follow-up runtime.startTurn once that turn settles", async () => {
    const store = memoryStore()
    const originalMessage: ChatMessageBody = {
      authorUserId: "user-1",
      body: "please say hello",
      createdAt: iso,
      deletedAt: null,
      messageId: "msg-1",
      threadId: "thread-1",
      updatedAt: iso,
    }
    const appendedMessage: ChatMessageBody = {
      authorUserId: "user-1",
      body: "actually also do this",
      createdAt: iso,
      deletedAt: null,
      messageId: "msg-2",
      threadId: "thread-1",
      updatedAt: iso,
    }
    let resolveFollowUp: (() => void) | undefined
    const followUpPushed = new Promise<void>((resolve) => {
      resolveFollowUp = resolve
    })
    const followUpCalls: Array<{ name: string; args: unknown }> = []
    const { options, cleanup } = await baseOptions({
      codexThreadRunner: fakeCodexRunner([
        { type: "turn.started" },
        { type: "turn.completed", usage: { input_tokens: 1, output_tokens: 1, reasoning_output_tokens: 0 } },
      ]),
      fetchChatMessageImpl: async (input) => {
        if (input.messageId === "msg-1") return { message: originalMessage, ok: true }
        if (input.messageId === "msg-2") return { message: appendedMessage, ok: true }
        return { message: null, ok: true }
      },
      pushControlIntentImpl: async (input) => {
        followUpCalls.push({ args: input.args, name: input.name })
        resolveFollowUp?.()
        return { ok: true, result: { mutationId: input.mutationId, status: "applied" } }
      },
      readImpl: pageReader([
        controlIntentRow({
          bodyRef: "chat_message.msg-1",
          intentId: "intent-20",
          kind: "turn.start",
          seq: 1,
          threadId: "thread-1",
          turnId: "turn-20",
        }),
        controlIntentRow({
          bodyRef: "chat_message.msg-2",
          intentId: "intent-21",
          kind: "message.append",
          seq: 2,
          threadId: "thread-1",
          turnId: "turn-20",
        }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("applied")
        expect(result.outcomes[1]!.outcome).toBe("applied")
        expect(result.outcomes[1]!.detail).toContain("queued instead of applied")
      }
      await followUpPushed
      expect(followUpCalls).toHaveLength(1)
      expect(followUpCalls[0]!.name).toBe("runtime.startTurn")
      const args = followUpCalls[0]!.args as { kind: string; threadId: string; bodyRef: string; turnId: string }
      expect(args.kind).toBe("turn.start")
      expect(args.threadId).toBe("thread-1")
      expect(args.bodyRef).toBe("chat_message.msg-2")
      expect(args.turnId).not.toBe("turn-20")
    } finally {
      await cleanup()
    }
  })

  test("turn.continue / turn.retry are honestly skipped_stale, not faked applied", async () => {
    const store = memoryStore()
    const { options, cleanup } = await baseOptions({
      readImpl: pageReader([
        controlIntentRow({ intentId: "intent-8", kind: "turn.continue", seq: 1, threadId: "thread-1", turnId: "turn-8" }),
        controlIntentRow({ intentId: "intent-9", kind: "turn.retry", seq: 2, threadId: "thread-1", turnId: "turn-8" }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes.map((o) => o.outcome)).toEqual(["skipped_stale", "skipped_stale"])
        expect(result.outcomes.every((o) => o.detail?.includes("not implemented") === true)).toBe(true)
      }
    } finally {
      await cleanup()
    }
  })

  test("turn.close with no local dispatch active is applied — the server-side mutator already made it authoritative", async () => {
    const store = memoryStore()
    const { options, cleanup } = await baseOptions({
      readImpl: pageReader([
        controlIntentRow({ intentId: "intent-10", kind: "turn.close", seq: 1, threadId: "thread-1", turnId: "turn-10" }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("applied")
        expect(result.outcomes[0]!.detail).toContain("closed")
      }
    } finally {
      await cleanup()
    }
  })

  test("turn.close against a still-actively-dispatching local turn is skipped_stale — close does not silently abort it", async () => {
    const store = memoryStore()
    const activeTurns: ActiveRuntimeTurns = new Map()
    activeTurns.set("turn-11", {
      abortController: new AbortController(),
      clientGroupId: "cg-fixture",
      clientId: "c-fixture",
      interrupted: false,
      lane: "codex_app_server",
      nextEventSequence: () => 1,
      nextMutationId: () => 1,
      pendingAppendMessageIds: [],
      threadId: "thread-1",
    })
    const { options, cleanup } = await baseOptions({
      activeTurns,
      readImpl: pageReader([
        controlIntentRow({ intentId: "intent-11", kind: "turn.close", seq: 1, threadId: "thread-1", turnId: "turn-11" }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("skipped_stale")
        expect(result.outcomes[0]!.detail).toContain("still actively dispatching")
      }
    } finally {
      await cleanup()
    }
  })

  test("dedup: a redelivered intent id is not re-dispatched and comes back marked deduped", async () => {
    const store = memoryStore()
    const row = controlIntentRow({ intentId: "intent-11", kind: "message.append", seq: 1, threadId: "thread-1" })
    const { options, cleanup } = await baseOptions({ readImpl: pageReader([row]) })
    try {
      const first = await enforcePendingRuntimeIntents(store, options)
      expect(first.ok).toBe(true)
      const second = await enforcePendingRuntimeIntents(store, options)
      expect(second.ok).toBe(true)
      if (second.ok) {
        expect(second.outcomes[0]!.deduped).toBe(true)
        expect(second.outcomes[0]!.outcome).toBe("failed")
      }
    } finally {
      await cleanup()
    }
  })

  test("one throwing intent is isolated: a failed outcome is recorded and the watermark still advances", async () => {
    const store = memoryStore()
    const message: ChatMessageBody = {
      authorUserId: "user-1",
      body: "irrelevant — the account lookup throws first",
      createdAt: iso,
      deletedAt: null,
      messageId: "does-not-matter",
      threadId: "thread-1",
      updatedAt: iso,
    }
    const { options, cleanup } = await baseOptions({
      fetchChatMessageImpl: async () => ({ message, ok: true }),
      listCandidateAccounts: async () => {
        throw new Error("boom: registry read exploded")
      },
      readImpl: pageReader([
        controlIntentRow({
          bodyRef: "chat_message.does-not-matter",
          intentId: "intent-12",
          kind: "turn.start",
          seq: 7,
          threadId: "thread-1",
          turnId: "turn-12",
        }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes[0]!.outcome).toBe("failed")
        expect(result.outcomes[0]!.detail).toContain("boom")
        expect(result.nextAfter).toBe(7)
      }
      expect(store.getRuntimeIntentWatermark()).toBe(7)
    } finally {
      await cleanup()
    }
  })

  test("watermark persists across ticks and is scoped per ownerUserId", async () => {
    const store = memoryStore()
    const { options, cleanup } = await baseOptions({
      ownerUserId: "user-1",
      readImpl: pageReader([
        controlIntentRow({ intentId: "intent-13", kind: "message.append", ownerUserId: "user-1", seq: 3, threadId: "thread-1" }),
      ]),
    })
    try {
      expect(store.getRuntimeIntentWatermark("user-1")).toBe(0)
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      expect(store.getRuntimeIntentWatermark("user-1")).toBe(3)
      expect(store.getRuntimeIntentWatermark("user-2")).toBe(0)
    } finally {
      await cleanup()
    }
  })

  test("transport failure from the reader is surfaced ok:false without touching the watermark", async () => {
    const store = memoryStore()
    const { options, cleanup } = await baseOptions({
      readImpl: async () => ({ error: "network_failed", ok: false, reason: null, status: null }),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(false)
      if (!result.ok) expect(result.watermark).toBe(0)
    } finally {
      await cleanup()
    }
  })
})

// ---------------------------------------------------------------------------
// Thread-resume account affinity (#8410 follow-up)
// ---------------------------------------------------------------------------

/** Two fully-tied ready codex candidates (same capacity/load), optionally
 * with one of them marked unhealthy — real `FleetAccountEntity` rows built
 * the same way `candidateAccountsFromRegistry` builds them, so
 * `selectDispatchAccount`'s real round-robin/eligibility logic runs
 * unmodified against them. */
const twoTiedCodexAccounts = (unhealthyRef?: "acct-pin-a" | "acct-pin-b"): CandidateAccount[] =>
  (["acct-pin-a", "acct-pin-b"] as const).map((ref) => {
    const accountRefHash = hashPylonAccountRef("codex", ref)
    const ready = ref !== unhealthyRef
    return {
      fleetAccount: decodeFleetAccountEntity({
        accountRefHash,
        capacityAvailable: ready ? 1 : 0,
        capacityBusy: 0,
        capacityQueued: 0,
        provider: "codex",
        readiness: ready ? "ready" : "unavailable",
        updatedAt: iso,
      }),
      registryEntry: {
        home: `/tmp/${ref}`,
        hourlyCap: null,
        manualResetsRemaining: null,
        openAgentsProviderAccountRef: null,
        provider: "codex" as const,
        ref,
        weeklyCap: null,
      },
    }
  })

const accountHashFromDispatchDetail = (detail: string | null): string => {
  const match = detail === null ? null : /dispatch started against account (\S+)/.exec(detail)
  if (match === null) throw new Error(`detail did not name a dispatched account: ${detail}`)
  return match[1]!
}

describe("thread-resume account affinity (#8410 follow-up)", () => {
  const messageFor = (threadId: string, messageId: string): ChatMessageBody => ({
    authorUserId: "user-1",
    body: "please say hello",
    createdAt: iso,
    deletedAt: null,
    messageId,
    threadId,
    updatedAt: iso,
  })

  test("pins a thread's SECOND turn.start dispatch to the SAME account as its first, bypassing round-robin", async () => {
    const store = memoryStore()
    const threadId = "thread-pin-affinity-1"
    // Persisted across both dispatches below, exactly like production
    // (`runtime-intent-supervisor.ts` keeps one `Map` alive across ticks) —
    // WITHOUT the pin, this is what would make `selectDispatchAccount` cycle
    // to the other tied account on the second dispatch.
    const lastDispatchedAccountByThread = new Map<string, string>()
    const { options, cleanup } = await baseOptions({
      codexThreadRunner: fakeCodexRunner([]),
      lastDispatchedAccountByThread,
      listCandidateAccounts: async () => twoTiedCodexAccounts(),
    })
    try {
      // First dispatch: no pin exists yet, so it's an ordinary (deterministic,
      // lowest-hash) round-robin pick between the two tied accounts.
      const first = await enforcePendingRuntimeIntents(store, {
        ...options,
        fetchChatMessageImpl: async () => ({ message: messageFor(threadId, "msg-a"), ok: true }),
        readImpl: pageReader([
          controlIntentRow({ bodyRef: "chat_message.msg-a", intentId: "intent-pin-1", kind: "turn.start", seq: 1, threadId, turnId: "turn-pin-1" }),
        ]),
      })
      expect(first.ok).toBe(true)
      const firstHash = first.ok ? accountHashFromDispatchDetail(first.outcomes[0]!.detail) : ""
      expect(store.getRuntimeDispatchAccountRefHash(threadId)).toBe(firstHash)

      // Second dispatch: same thread, different turn. WITHOUT the pin,
      // `selectDispatchAccount`'s round-robin tie-break (`lastDispatchedAccountByThread`,
      // still wired below exactly as production wires it) would cycle to the
      // OTHER tied account here — that loss of Codex/Claude session
      // continuity across turns is exactly what #8410 reported. The pin
      // should override that and force the SAME account again.
      // the SAME account again, not the round-robin's natural cycle.
      const second = await enforcePendingRuntimeIntents(store, {
        ...options,
        fetchChatMessageImpl: async () => ({ message: messageFor(threadId, "msg-b"), ok: true }),
        readImpl: pageReader([
          controlIntentRow({ bodyRef: "chat_message.msg-b", intentId: "intent-pin-2", kind: "turn.start", seq: 2, threadId, turnId: "turn-pin-2" }),
        ]),
      })
      expect(second.ok).toBe(true)
      const secondHash = second.ok ? accountHashFromDispatchDetail(second.outcomes[0]!.detail) : ""
      expect(secondHash).toBe(firstHash)
      expect(store.getRuntimeDispatchAccountRefHash(threadId)).toBe(firstHash)
    } finally {
      await cleanup()
    }
  })

  test("falls back to round-robin and RE-PINS once the pinned account goes unhealthy", async () => {
    const store = memoryStore()
    const threadId = "thread-pin-affinity-2"
    const lastDispatchedAccountByThread = new Map<string, string>()
    const { options, cleanup } = await baseOptions({
      codexThreadRunner: fakeCodexRunner([]),
      lastDispatchedAccountByThread,
      listCandidateAccounts: async () => twoTiedCodexAccounts(),
    })
    try {
      const first = await enforcePendingRuntimeIntents(store, {
        ...options,
        fetchChatMessageImpl: async () => ({ message: messageFor(threadId, "msg-a"), ok: true }),
        readImpl: pageReader([
          controlIntentRow({ bodyRef: "chat_message.msg-a", intentId: "intent-pin-3", kind: "turn.start", seq: 1, threadId, turnId: "turn-pin-3" }),
        ]),
      })
      expect(first.ok).toBe(true)
      const firstHash = first.ok ? accountHashFromDispatchDetail(first.outcomes[0]!.detail) : ""
      const firstRef = firstHash === hashPylonAccountRef("codex", "acct-pin-a") ? "acct-pin-a" : "acct-pin-b"
      expect(store.getRuntimeDispatchAccountRefHash(threadId)).toBe(firstHash)

      // Now the previously-pinned account goes unhealthy (real per-account
      // readiness, #8410 follow-up item 1, would surface this the same way).
      const second = await enforcePendingRuntimeIntents(store, {
        ...options,
        fetchChatMessageImpl: async () => ({ message: messageFor(threadId, "msg-b"), ok: true }),
        listCandidateAccounts: async () => twoTiedCodexAccounts(firstRef),
        readImpl: pageReader([
          controlIntentRow({ bodyRef: "chat_message.msg-b", intentId: "intent-pin-4", kind: "turn.start", seq: 2, threadId, turnId: "turn-pin-4" }),
        ]),
      })
      expect(second.ok).toBe(true)
      const secondHash = second.ok ? accountHashFromDispatchDetail(second.outcomes[0]!.detail) : ""
      expect(secondHash).not.toBe(firstHash)
      // Re-pinned to the newly-selected (healthy) account.
      expect(store.getRuntimeDispatchAccountRefHash(threadId)).toBe(secondHash)
    } finally {
      await cleanup()
    }
  })
})
