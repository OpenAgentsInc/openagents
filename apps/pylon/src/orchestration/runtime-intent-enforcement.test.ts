import { Database } from "bun:sqlite"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import { decodeFleetAccountEntity, decodeRuntimeControlIntentRow } from "@openagentsinc/khala-sync"
import type { FleetAccountEntity, KhalaRuntimeEvent, RuntimeControlIntentRow } from "@openagentsinc/khala-sync"

import {
  candidateAccountsFromRegistry,
  chatMessageIdFromBodyRef,
  codexRawEventToRuntimeEvents,
  enforcePendingRuntimeIntents,
  selectDispatchAccountNaive,
  type ActiveRuntimeTurns,
  type CodexRawEvent,
  type EnforceRuntimeIntentsOptions,
} from "./runtime-intent-enforcement.js"
import type { ChatMessageBody, ReadPendingRuntimeIntentsResult } from "./runtime-intents.js"
import { createPylonOrchestrationStore, type PylonOrchestrationStore } from "./store.js"

const account = (overrides: Partial<{
  accountRefHash: string
  readiness: "ready" | "cooldown" | "unavailable" | "unknown"
  capacityAvailable: number
}> = {}): FleetAccountEntity =>
  decodeFleetAccountEntity({
    accountRefHash: overrides.accountRefHash ?? "account.pylon.codex.aaaaaaaaaaaaaaaaaaaaaaaa",
    readiness: overrides.readiness ?? "ready",
    updatedAt: "2026-07-05T12:00:00.000Z",
    ...(overrides.capacityAvailable === undefined ? {} : { capacityAvailable: overrides.capacityAvailable }),
  })

describe("selectDispatchAccountNaive", () => {
  test("picks the first account with positive capacityAvailable", () => {
    const a = account({ accountRefHash: "account.pylon.codex.a000000000000000000000a1", capacityAvailable: 0 })
    const b = account({ accountRefHash: "account.pylon.codex.b000000000000000000000b1", capacityAvailable: 2 })
    const c = account({ accountRefHash: "account.pylon.codex.c000000000000000000000c1", capacityAvailable: 1 })
    expect(selectDispatchAccountNaive([a, b, c])?.accountRefHash).toBe(b.accountRefHash)
  })

  test("falls back to the first ready account when no account reports positive capacity", () => {
    const a = account({
      accountRefHash: "account.pylon.codex.a000000000000000000000a2",
      capacityAvailable: 0,
      readiness: "cooldown",
    })
    const b = account({ accountRefHash: "account.pylon.codex.b000000000000000000000b2", readiness: "ready" })
    expect(selectDispatchAccountNaive([a, b])?.accountRefHash).toBe(b.accountRefHash)
  })

  test("falls back to the first ready account when capacity is entirely unreported", () => {
    const a = account({ accountRefHash: "account.pylon.codex.a000000000000000000000a3", readiness: "unavailable" })
    const b = account({ accountRefHash: "account.pylon.codex.b000000000000000000000b3", readiness: "ready" })
    expect(selectDispatchAccountNaive([a, b])?.accountRefHash).toBe(b.accountRefHash)
  })

  test("returns undefined when no account is ready or has capacity", () => {
    const a = account({ accountRefHash: "account.pylon.codex.a000000000000000000000a4", readiness: "unavailable" })
    expect(selectDispatchAccountNaive([a])).toBeUndefined()
  })

  test("returns undefined for an empty list", () => {
    expect(selectDispatchAccountNaive([])).toBeUndefined()
  })
})

describe("candidateAccountsFromRegistry", () => {
  test("projects codex registry entries into ready, one-slot FleetAccountEntity rows", () => {
    const candidates = candidateAccountsFromRegistry(
      [
        { home: "/tmp/acct-1", hourlyCap: null, manualResetsRemaining: null, openAgentsProviderAccountRef: null, provider: "codex", ref: "acct-1", weeklyCap: null },
      ],
      new Date("2026-07-05T12:00:00.000Z"),
    )
    expect(candidates).toHaveLength(1)
    expect(candidates[0]!.fleetAccount.readiness).toBe("ready")
    expect(candidates[0]!.fleetAccount.capacityAvailable).toBe(1)
    expect(candidates[0]!.fleetAccount.provider).toBe("codex")
    expect(candidates[0]!.registryEntry.ref).toBe("acct-1")
  })

  test("excludes claude_agent accounts — no Claude thread runner is wired into this consumer", () => {
    const candidates = candidateAccountsFromRegistry([
      { home: "/tmp/acct-2", hourlyCap: null, manualResetsRemaining: null, openAgentsProviderAccountRef: null, provider: "claude_agent", ref: "acct-2", weeklyCap: null },
    ])
    expect(candidates).toHaveLength(0)
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

const iso = "2026-07-05T12:00:00.000Z"

const controlIntentRow = (input: {
  seq: number
  intentId: string
  threadId: string
  turnId?: string
  kind: "turn.start" | "turn.interrupt" | "message.append" | "turn.continue" | "turn.retry" | "turn.close"
  bodyRef?: string
  ownerUserId?: string
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
      target: { adapterKind: "codex", lane: "codex_app_server" },
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
      nextEventSequence: () => {
        sequenceCounter += 1
        return sequenceCounter
      },
      nextMutationId: () => 1,
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

  test("message.append is explicitly rejected, never silently dropped", async () => {
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
        expect(result.outcomes[0]!.detail).toContain("mid-turn steering is not supported")
      }
    } finally {
      await cleanup()
    }
  })

  test("turn.continue / turn.retry / turn.close are honestly skipped_stale, not faked applied", async () => {
    const store = memoryStore()
    const { options, cleanup } = await baseOptions({
      readImpl: pageReader([
        controlIntentRow({ intentId: "intent-8", kind: "turn.continue", seq: 1, threadId: "thread-1", turnId: "turn-8" }),
        controlIntentRow({ intentId: "intent-9", kind: "turn.retry", seq: 2, threadId: "thread-1", turnId: "turn-8" }),
        controlIntentRow({ intentId: "intent-10", kind: "turn.close", seq: 3, threadId: "thread-1", turnId: "turn-8" }),
      ]),
    })
    try {
      const result = await enforcePendingRuntimeIntents(store, options)
      expect(result.ok).toBe(true)
      if (result.ok) {
        expect(result.outcomes.map((o) => o.outcome)).toEqual(["skipped_stale", "skipped_stale", "skipped_stale"])
        expect(result.outcomes.every((o) => o.detail?.includes("not implemented") === true)).toBe(true)
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
