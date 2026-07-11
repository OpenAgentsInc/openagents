import { describe, expect, test } from "bun:test"
import { MutationId, type KhalaRuntimeControlIntent } from "@openagentsinc/khala-sync"
import type {
  ConfirmedChatMessage,
  ConfirmedChatThread,
  KhalaSyncConversation,
  KhalaSyncAgentTimeline,
  KhalaSyncRuntimeCommands,
  KhalaSyncConversationChange,
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
      commands.push({ id: "append", threadRef: args.threadId, messageRef: args.messageId })
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
    })
    expect(result).toMatchObject({
      ok: true,
      thread: {
        messages: [{ messageRef: "message.synced.1" }, { messageRef: "message.mobile.new-message" }],
      },
    })
    expect(fixture.commands).toEqual([
      { id: "create", threadRef: "thread.mobile.new-thread" },
      { id: "append", threadRef: "thread.synced.1", messageRef: "message.mobile.new-message" },
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

  test("admits the same message/thread/run refs through the shared runtime command and can interrupt that confirmed run", async () => {
    const fixture = makeConversation()
    const intents: Array<KhalaRuntimeControlIntent> = []
    let activeRunRef: string | null = null
    let activeStatus: "completed" | "canceled" = "completed"
    let activeSequence = 0
    const runtime: KhalaSyncRuntimeCommands = {
      outcome: () => Effect.succeed(null),
      startTurn: intent => Effect.sync(() => {
        intents.push(intent)
        activeRunRef = intent.turnId ?? null
        activeStatus = "completed"
        activeSequence = 1
        return MutationId.make(3)
      }),
      appendUserMessage: intent => Effect.sync(() => {
        intents.push(intent)
        return MutationId.make(4)
      }),
      interruptTurn: intent => Effect.sync(() => {
        intents.push(intent)
        activeStatus = "canceled"
        activeSequence += 1
        return MutationId.make(5)
      }),
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
        version: 8,
      },
      events: activeRunRef === null ? [] : [{
        eventRef: `event.mobile.${activeSequence}`,
        runRef: activeRunRef,
        sequence: activeSequence,
        eventType: activeStatus === "canceled" ? "turn.interrupted" : "turn.finished",
        summary: activeStatus === "canceled" ? "Interrupted" : "Completed",
        status: activeStatus,
        artifactRefs: [],
        item: activeStatus === "canceled"
          ? { kind: "interrupted" as const }
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
    const ids = ["runtime-message", "runtime-turn", "runtime-interrupt"]
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

    const interrupted = await host.interrupt?.({
      runRef: "turn.mobile.runtime-turn",
      threadRef: "thread.synced.1",
    })
    expect(interrupted?.ok).toBe(true)
    expect(intents[1]).toMatchObject({
      kind: "turn.interrupt",
      threadId: "thread.synced.1",
      turnId: "turn.mobile.runtime-turn",
    })
  })

  test("surfaces a durable expired result instead of executing after offline delay", async () => {
    const fixture = makeConversation()
    const intents: Array<KhalaRuntimeControlIntent> = []
    const runtime: KhalaSyncRuntimeCommands = {
      appendUserMessage: () => Effect.succeed(MutationId.make(4)),
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
})
