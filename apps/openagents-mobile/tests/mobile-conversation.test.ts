import { describe, expect, test } from "bun:test"
import { MutationId, SyncVersionWatermark, type KhalaRuntimeControlIntent } from "@openagentsinc/khala-sync"
import type {
  ConfirmedChatMessage,
  ConfirmedChatThread,
  KhalaSyncConversation,
  KhalaSyncAgentTimeline,
  KhalaSyncRuntimeCommands,
  KhalaSyncRuntimeInteractions,
  KhalaSyncConversationChange,
  RuntimeInteractionDecisionCommand,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

import {
  makeMobileConversationHost,
  selectMobileConversation,
} from "../src/conversation/mobile-conversation"

const now = "2026-07-10T20:15:00.000Z"

const makeConversation = (input: Readonly<{
  appendConfirmed?: boolean
}> = {}): Readonly<{
  conversation: KhalaSyncConversation
  threads: Map<string, ConfirmedChatThread>
  messages: Map<string, Array<ConfirmedChatMessage>>
  commands: Array<Record<string, string>>
  confirmMessage: (threadRef: string, messageRef: string, body: string) => void
}> => {
  const threads = new Map<string, ConfirmedChatThread>([[
    "thread.synced.1",
    {
      threadRef: "thread.synced.1",
      title: "Synced",
      messageCount: 1,
      lastMessageAt: now,
      updatedAt: now,
      version: 3,
    },
  ]])
  const messages = new Map<string, Array<ConfirmedChatMessage>>([[
    "thread.synced.1",
    [{
      messageRef: "message.synced.1",
      threadRef: "thread.synced.1",
      body: "Confirmed",
      createdAt: now,
      updatedAt: now,
      version: 5,
    }],
  ]])
  const commands: Array<Record<string, string>> = []
  const listeners = new Set<(change: KhalaSyncConversationChange) => void>()
  const live = { phase: "live" as const, cursor: 5, pendingMutationCount: 0 }
  const notify = (threadRef: string): void => {
    for (const listener of [...listeners]) listener({
      kind: "content",
      status: live,
      threadRef,
    })
  }
  const conversation: KhalaSyncConversation = {
    personalStatus: () => live,
    threadStatus: () => live,
    listConfirmedThreads: () => Effect.succeed([...threads.values()]),
    openThread: () => Effect.succeed(undefined),
    closeThread: () => Effect.succeed(undefined),
    subscribeThread: (_threadRef, listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    listConfirmedMessages: threadRef => Effect.succeed(messages.get(threadRef) ?? []),
    createThread: args => Effect.sync(() => {
      commands.push({ id: "create", threadRef: args.threadId })
      threads.set(args.threadId, {
        threadRef: args.threadId,
        title: args.title,
        messageCount: 0,
        lastMessageAt: null,
        updatedAt: now,
        version: 6,
      })
      messages.set(args.threadId, [])
      notify(args.threadId)
      return 1 as MutationId
    }),
    appendMessage: args => Effect.sync(() => {
      commands.push({
        id: "append",
        threadRef: args.threadId,
        messageRef: args.messageId,
        ...(args.attachments === undefined
          ? {}
          : { attachmentCount: String(args.attachments.length) }),
      })
      if (input.appendConfirmed !== false) {
        const list = messages.get(args.threadId) ?? []
        list.push({
          messageRef: args.messageId,
          threadRef: args.threadId,
          body: args.body,
          createdAt: now,
          updatedAt: now,
          version: 7,
        })
        messages.set(args.threadId, list)
        const thread = threads.get(args.threadId)!
        threads.set(args.threadId, {
          ...thread,
          messageCount: list.length,
          lastMessageAt: now,
          version: 7,
        })
        notify(args.threadId)
      }
      return 2 as MutationId
    }),
  }
  const confirmMessage = (threadRef: string, messageRef: string, body: string): void => {
    const list = messages.get(threadRef) ?? []
    list.push({
      messageRef,
      threadRef,
      body,
      createdAt: now,
      updatedAt: now,
      version: 8,
    })
    messages.set(threadRef, list)
    const thread = threads.get(threadRef)!
    threads.set(threadRef, {
      ...thread,
      messageCount: list.length,
      lastMessageAt: now,
      version: 8,
    })
    notify(threadRef)
  }
  return { conversation, threads, messages, commands, confirmMessage }
}

describe("contract openagents_mobile.chat.authoritative_sync_mode.v1", () => {
  test("production conversation reconciliation contains no interval polling loop", async () => {
    const source = await Bun.file(new URL(
      "../src/conversation/mobile-conversation.ts",
      import.meta.url,
    )).text()
    expect(source).not.toContain("await sleep(100)")
    expect(source).not.toContain("for (let attempt")
    expect(source).toContain("openKhalaConversationLive")
  })

  test("selects local once when confirmed Sync does not become live", async () => {
    const selection = await selectMobileConversation({
      conversation: () => null,
    })

    expect(selection).toEqual({ mode: "local" })
  })

  test("selects live Sync and reconstructs the confirmed initial thread", async () => {
    const fixture = makeConversation()
    const selection = await selectMobileConversation({
      conversation: () => fixture.conversation,
    })

    expect(selection.mode).toBe("sync")
    if (selection.mode !== "sync") throw new Error("expected synced selection")
    expect(selection.threads[0]).toMatchObject({ threadRef: "thread.synced.1", version: 3 })
    expect(selection.activeThread?.messages[0]).toMatchObject({
      messageRef: "message.synced.1",
      body: "Confirmed",
      version: 5,
    })
  })

  test("restores an exact preferred thread instead of inferring the first row", async () => {
    const fixture = makeConversation()
    fixture.threads.set("thread.synced.2", {
      threadRef: "thread.synced.2",
      title: "Restored coding session",
      messageCount: 1,
      lastMessageAt: now,
      updatedAt: now,
      version: 6,
    })
    fixture.messages.set("thread.synced.2", [{
      messageRef: "message.synced.2",
      threadRef: "thread.synced.2",
      body: "Exact restored thread",
      createdAt: now,
      updatedAt: now,
      version: 6,
    }])

    const selection = await selectMobileConversation({
      conversation: () => fixture.conversation,
      preferredThreadRef: "thread.synced.2",
    })

    expect(selection).toMatchObject({
      mode: "sync",
      activeThread: {
        threadRef: "thread.synced.2",
        messages: [{ body: "Exact restored thread" }],
      },
    })
  })

  test("binds one closeable live thread lease and stops updates after close", async () => {
    const fixture = makeConversation({ appendConfirmed: false })
    const host = makeMobileConversationHost({ conversation: fixture.conversation })
    const updates: string[] = []
    const lease = await host.watchThread?.("thread.synced.1", thread => {
      updates.push(thread.messages.at(-1)?.body ?? "empty")
    })
    expect(lease).not.toBeNull()
    fixture.confirmMessage("thread.synced.1", "message.watch.1", "Before close")
    for (let attempt = 0; attempt < 20 && updates.length === 0; attempt += 1) {
      await new Promise(resolve => setTimeout(resolve, 0))
    }
    expect(updates.length).toBeGreaterThan(0)
    expect(updates.every(value => value === "Before close")).toBe(true)
    const updatesBeforeClose = updates.length
    await lease?.close()
    fixture.confirmMessage("thread.synced.1", "message.watch.2", "After close")
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(updates).toHaveLength(updatesBeforeClose)
  })

  test("create and append wait for their exact stable client refs", async () => {
    const fixture = makeConversation()
    const ids = ["new-thread", "new-message"]
    const host = makeMobileConversationHost({
      conversation: fixture.conversation,
      randomId: () => ids.shift()!,
      sleep: async () => undefined,
      pollAttempts: 1,
    })

    const created = await host.newThread()
    expect(created).toMatchObject({ ok: true, thread: { threadRef: "thread.mobile.new-thread" } })
    const result = await host.sendMessage({
      threadRef: "thread.synced.1",
      body: "Follow-up",
      attachments: [{
        name: "pixel.png",
        mediaType: "image/png",
        sizeBytes: 3,
        sha256: "a".repeat(64),
        dataBase64: "AQID",
      }],
    })
    expect(result).toMatchObject({
      ok: true,
      thread: {
        messages: [{ messageRef: "message.synced.1" }, { messageRef: "message.mobile.new-message" }],
      },
    })
    expect(fixture.commands).toEqual([
      { id: "create", threadRef: "thread.mobile.new-thread" },
      { id: "append", threadRef: "thread.synced.1", messageRef: "message.mobile.new-message", attachmentCount: "1" },
    ])
  })

  test("never converts an unconfirmed append into completion", async () => {
    const fixture = makeConversation({ appendConfirmed: false })
    let sleeps = 0
    const host = makeMobileConversationHost({
      conversation: fixture.conversation,
      randomId: () => "pending",
      pollAttempts: 2,
      sleep: async () => { sleeps += 1 },
    })

    const result = await host.sendMessage({ threadRef: "thread.synced.1", body: "Pending" })
    expect(result).toEqual({ ok: false, error: "Message is still pending reconciliation." })
    expect(sleeps).toBe(1)
  })

  test("reconciles an asynchronous confirmation from the live subscription without interval polling", async () => {
    const fixture = makeConversation({ appendConfirmed: false })
    const never = new Promise<void>(() => undefined)
    const host = makeMobileConversationHost({
      conversation: fixture.conversation,
      randomId: () => "live-confirmation",
      pollAttempts: 30,
      sleep: () => never,
    })

    const pending = host.sendMessage({
      threadRef: "thread.synced.1",
      body: "Arrives through the change stream",
    })
    await new Promise(resolve => setTimeout(resolve, 0))
    fixture.confirmMessage(
      "thread.synced.1",
      "message.mobile.live-confirmation",
      "Arrives through the change stream",
    )

    expect(await pending).toMatchObject({
      ok: true,
      thread: { messages: [{}, { messageRef: "message.mobile.live-confirmation" }] },
    })
  })

  test("admits exact confirmed mobile retry/cancel/resume/close controls", async () => {
    const fixture = makeConversation()
    const intents: Array<KhalaRuntimeControlIntent> = []
    let activeRunRef: string | null = null
    let activeStatus: "queued" | "completed" | "canceled" = "completed"
    let activeSequence = 0
    let activeVersion = 8
    const recordControl = (
      intent: KhalaRuntimeControlIntent,
      status: "queued" | "canceled",
      mutationId: number,
    ) => Effect.sync(() => {
      intents.push(intent)
      activeStatus = status
      activeSequence += 1
      activeVersion += 1
      return MutationId.make(mutationId)
    })
    const runtime: KhalaSyncRuntimeCommands = {
      outcome: input => {
        const intent = intents.find(candidate => candidate.intentId === input.intentId)
        if (intent === undefined) return Effect.succeed(null)
        return Effect.succeed({
          commandRef: input.intentId,
          mutationId: null,
          runRef: activeRunRef,
          status: intent.kind === "turn.close"
            ? "settled" as const
            : intent.kind === "turn.interrupt"
              ? "canceled" as const
              : "accepted" as const,
          threadRef: input.threadRef,
          updatedAt: now,
          version: activeVersion,
        })
      },
      startTurn: intent => Effect.sync(() => {
        intents.push(intent)
        activeRunRef = intent.turnId ?? null
        activeStatus = "completed"
        activeSequence = 1
        activeVersion += 1
        return MutationId.make(3)
      }),
      appendUserMessage: intent => Effect.sync(() => {
        intents.push(intent)
        return MutationId.make(4)
      }),
      continueTurn: intent => recordControl(intent, "queued", 6),
      retryTurn: intent => recordControl(intent, "queued", 7),
      closeTurn: intent => recordControl(intent, "canceled", 8),
      interruptTurn: intent => recordControl(intent, "canceled", 5),
    }
    const snapshot = () => ({
      status: { phase: "live" as const, cursor: 8, pendingMutationCount: 0 },
      run: activeRunRef === null ? null : {
        runRef: activeRunRef,
        routeRef: "thread.synced.1",
        runtime: "codex" as const,
        backend: "pylon" as const,
        status: activeStatus,
        createdAt: now,
        updatedAt: now,
        startedAt: now,
        completedAt: activeStatus === "completed" ? now : null,
        failedAt: null,
        canceledAt: activeStatus === "canceled" ? now : null,
        version: activeVersion,
      },
      events: activeRunRef === null ? [] : [{
        eventRef: `event.mobile.${activeSequence}`,
        runRef: activeRunRef,
        sequence: activeSequence,
        eventType: activeStatus === "canceled"
          ? "turn.interrupted"
          : activeStatus === "queued" ? "turn.queued" : "turn.finished",
        summary: activeStatus === "canceled"
          ? "Interrupted"
          : activeStatus === "queued" ? "Queued" : "Completed",
        status: activeStatus,
        artifactRefs: [],
        item: activeStatus === "canceled"
          ? { kind: "interrupted" as const }
          : activeStatus === "queued"
            ? {
                kind: "connected" as const,
                lane: "codex_app_server" as const,
                turnRef: activeRunRef,
              }
            : { kind: "terminal" as const, status: "completed" as const },
        createdAt: now,
        version: 8 + activeSequence,
      }],
    })
    const timeline: KhalaSyncAgentTimeline = {
      status: () => snapshot().status,
      open: () => Effect.succeed(undefined),
      snapshot: () => Effect.succeed(snapshot()),
      snapshotForThread: () => Effect.succeed(snapshot()),
    }
    const ids = [
      "runtime-message",
      "runtime-turn",
      "runtime-retry-1",
      "runtime-cancel-1",
      "runtime-resume",
      "runtime-cancel-2",
      "runtime-retry-2",
      "runtime-cancel-3",
      "runtime-close",
    ]
    const host = makeMobileConversationHost({
      conversation: fixture.conversation,
      runtime,
      timeline,
      randomId: () => ids.shift()!,
      pollAttempts: 1,
      sleep: async () => undefined,
    })

    const sent = await host.sendMessage({
      body: "Continue from mobile",
      threadRef: "thread.synced.1",
    })
    expect(sent.ok).toBe(true)
    expect(intents[0]).toMatchObject({
      bodyRef: "chat_message.message.mobile.runtime-message",
      kind: "turn.start",
      threadId: "thread.synced.1",
      turnId: "turn.mobile.runtime-turn",
    })

    const retried = await host.controlTurn?.({
      action: "retry",
      runRef: "turn.mobile.runtime-turn",
      threadRef: "thread.synced.1",
    })
    expect(retried?.ok).toBe(true)
    expect(intents.at(-1)).toMatchObject({ kind: "turn.retry", target: { lane: "codex_app_server" } })

    expect((await host.interrupt?.({ runRef: "turn.mobile.runtime-turn", threadRef: "thread.synced.1" }))?.ok).toBe(true)
    expect(intents.at(-1)?.kind).toBe("turn.interrupt")
    expect((await host.controlTurn?.({ action: "resume", runRef: "turn.mobile.runtime-turn", threadRef: "thread.synced.1" }))?.ok).toBe(true)
    expect(intents.at(-1)?.kind).toBe("turn.continue")
    expect((await host.controlTurn?.({ action: "cancel", runRef: "turn.mobile.runtime-turn", threadRef: "thread.synced.1" }))?.ok).toBe(true)
    expect((await host.controlTurn?.({ action: "retry", runRef: "turn.mobile.runtime-turn", threadRef: "thread.synced.1" }))?.ok).toBe(true)
    expect((await host.controlTurn?.({ action: "cancel", runRef: "turn.mobile.runtime-turn", threadRef: "thread.synced.1" }))?.ok).toBe(true)
    expect((await host.controlTurn?.({ action: "close", runRef: "turn.mobile.runtime-turn", threadRef: "thread.synced.1" }))?.ok).toBe(true)
    expect(intents.map(intent => intent.kind)).toEqual([
      "turn.start",
      "turn.retry",
      "turn.interrupt",
      "turn.continue",
      "turn.interrupt",
      "turn.retry",
      "turn.interrupt",
      "turn.close",
    ])
  })

  test("binds controls to the confirmed Claude lane and refuses an unknown lane", async () => {
    const fixture = makeConversation()
    const intents: Array<KhalaRuntimeControlIntent> = []
    let runtimeKind: "claude_code" | undefined = "claude_code"
    let status: "canceled" | "queued" = "canceled"
    let version = 30
    const runtime: KhalaSyncRuntimeCommands = {
      appendUserMessage: () => Effect.succeed(MutationId.make(1)),
      closeTurn: () => Effect.succeed(MutationId.make(2)),
      continueTurn: intent => Effect.sync(() => {
        intents.push(intent)
        status = "queued"
        version += 1
        return MutationId.make(3)
      }),
      interruptTurn: () => Effect.succeed(MutationId.make(4)),
      retryTurn: () => Effect.succeed(MutationId.make(5)),
      startTurn: () => Effect.succeed(MutationId.make(6)),
      outcome: input => Effect.succeed(intents.some(intent => intent.intentId === input.intentId)
        ? {
            commandRef: input.intentId,
            mutationId: null,
            runRef: "turn.mobile.claude",
            status: "accepted" as const,
            threadRef: input.threadRef,
            updatedAt: now,
            version,
          }
        : null),
    }
    const snapshot = () => ({
      status: { phase: "live" as const, cursor: version, pendingMutationCount: 0 },
      run: {
        runRef: "turn.mobile.claude",
        routeRef: "thread.synced.1",
        ...(runtimeKind === undefined ? {} : { runtime: runtimeKind }),
        backend: "pylon" as const,
        status,
        createdAt: now,
        updatedAt: now,
        startedAt: now,
        completedAt: null,
        failedAt: null,
        canceledAt: status === "canceled" ? now : null,
        version,
      },
      events: [],
    })
    const timeline: KhalaSyncAgentTimeline = {
      status: () => snapshot().status,
      open: () => Effect.void,
      snapshot: () => Effect.succeed(snapshot()),
      snapshotForThread: () => Effect.succeed(snapshot()),
    }
    const host = makeMobileConversationHost({
      conversation: fixture.conversation,
      runtime,
      timeline,
      randomId: () => "claude-control",
      sleep: async () => undefined,
    })

    expect((await host.controlTurn?.({
      action: "resume",
      runRef: "turn.mobile.claude",
      threadRef: "thread.synced.1",
    }))?.ok).toBe(true)
    expect(intents[0]).toMatchObject({
      kind: "turn.continue",
      target: { lane: "claude_pylon" },
    })

    runtimeKind = undefined
    status = "canceled"
    version += 1
    const refused = await host.controlTurn?.({
      action: "resume",
      runRef: "turn.mobile.claude",
      threadRef: "thread.synced.1",
    })
    expect(refused).toEqual({
      ok: false,
      error: "The confirmed runtime lane is unavailable.",
    })
    expect(intents).toHaveLength(1)
  })

  test("surfaces a durable expired result instead of executing after offline delay", async () => {
    const fixture = makeConversation()
    const intents: Array<KhalaRuntimeControlIntent> = []
    const runtime: KhalaSyncRuntimeCommands = {
      appendUserMessage: () => Effect.succeed(MutationId.make(4)),
      continueTurn: () => Effect.succeed(MutationId.make(6)),
      retryTurn: () => Effect.succeed(MutationId.make(7)),
      closeTurn: () => Effect.succeed(MutationId.make(8)),
      interruptTurn: () => Effect.succeed(MutationId.make(5)),
      startTurn: intent => Effect.sync(() => {
        intents.push(intent)
        return MutationId.make(3)
      }),
      outcome: input => Effect.succeed({
        commandRef: input.intentId,
        mutationId: null,
        runRef: "turn.mobile.expired-turn",
        status: "expired",
        threadRef: input.threadRef,
        updatedAt: "2026-07-10T20:21:00.000Z",
        version: 9,
      }),
    }
    const ids = ["expired-message", "expired-turn"]
    const host = makeMobileConversationHost({
      conversation: fixture.conversation,
      runtime,
      randomId: () => ids.shift()!,
      now: () => new Date("2026-07-10T20:15:00.000Z"),
      commandTtlMs: 60_000,
      pollAttempts: 1,
      sleep: async () => undefined,
    })

    expect(await host.sendMessage({
      body: "Do not execute after reconnect",
      threadRef: "thread.synced.1",
    })).toEqual({
      ok: false,
      error: "Runtime command expired while this device was offline.",
    })
    expect(intents).toHaveLength(1)
    expect(intents[0]).toMatchObject({
      expiresAt: "2026-07-10T20:16:00.000Z",
      intentId: "intent.start.turn.mobile.expired-turn",
    })
  })

  test("queues an exact mobile interaction decision and waits for confirmed resolution", async () => {
    const fixture = makeConversation()
    const decisions: RuntimeInteractionDecisionCommand[] = []
    let interactionStatus: "pending" | "resolved" = "pending"
    const snapshot = () => ({
      status: { phase: "live" as const, cursor: 9, pendingMutationCount: 0 },
      run: {
        runRef: "turn.interaction.1",
        routeRef: "thread.synced.1",
        status: "waiting_for_input" as const,
        createdAt: now,
        updatedAt: now,
        startedAt: now,
        completedAt: null,
        failedAt: null,
        canceledAt: null,
        version: 9,
      },
      events: [{
        eventRef: "interaction.mobile.1",
        runRef: "turn.interaction.1",
        sequence: 2,
        eventType: "runtime.interaction.tool_approval",
        summary: "Approve tests",
        status: interactionStatus,
        artifactRefs: [],
        item: {
          kind: "approval" as const,
          interactionRef: "interaction.mobile.1",
          prompt: "Run the focused tests?",
          status: interactionStatus,
          expiresAt: "2026-07-10T20:20:00.000Z",
          ...(interactionStatus === "resolved"
            ? { decisionRef: "decision.mobile.fixture" }
            : {}),
        },
        createdAt: now,
        version: interactionStatus === "resolved" ? 10 : 9,
      }],
    })
    const timeline: KhalaSyncAgentTimeline = {
      status: () => snapshot().status,
      open: () => Effect.succeed(undefined),
      snapshot: () => Effect.succeed(snapshot()),
      snapshotForThread: () => Effect.succeed(snapshot()),
    }
    const interactions: KhalaSyncRuntimeInteractions = {
      status: () => ({ phase: "live", cursor: SyncVersionWatermark.make(9) }),
      list: () => Effect.succeed([]),
      decide: command => Effect.sync(() => {
        decisions.push(command)
        interactionStatus = "resolved"
        return MutationId.make(11)
      }),
    }
    const host = makeMobileConversationHost({
      conversation: fixture.conversation,
      interactions,
      timeline,
      randomId: () => "fixture",
      now: () => new Date("2026-07-10T20:16:00.000Z"),
      sleep: async () => undefined,
      pollAttempts: 1,
    })

    expect(await host.decideInteraction?.({
      interactionRef: "interaction.mobile.1",
      threadRef: "thread.synced.1",
      turnRef: "turn.interaction.1",
      decision: { kind: "tool_approval", outcome: "approve" },
    })).toMatchObject({
      ok: true,
      thread: {
        timeline: {
          events: [{ item: { status: "resolved", decisionRef: "decision.mobile.fixture" } }],
        },
      },
    })
    expect(decisions).toEqual([{
      interactionRef: "interaction.mobile.1",
      threadId: "thread.synced.1",
      turnId: "turn.interaction.1",
      envelope: {
        decisionRef: "decision.mobile.fixture",
        idempotencyKey: "idem.decision.mobile.fixture",
        decidedAt: "2026-07-10T20:16:00.000Z",
        surface: "mobile",
        decision: { kind: "tool_approval", outcome: "approve" },
      },
    }])
  })
})
