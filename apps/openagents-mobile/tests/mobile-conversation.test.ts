import { describe, expect, test } from "bun:test"
import type { MutationId } from "@openagentsinc/khala-sync"
import type {
  ConfirmedChatMessage,
  ConfirmedChatThread,
  KhalaSyncConversation,
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
  const live = { phase: "live" as const, cursor: 5, pendingMutationCount: 0 }
  const conversation: KhalaSyncConversation = {
    personalStatus: () => live,
    threadStatus: () => live,
    listConfirmedThreads: () => Effect.succeed([...threads.values()]),
    openThread: () => Effect.succeed(undefined),
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
      }
      return 2 as MutationId
    }),
  }
  return { conversation, threads, messages, commands }
}

describe("contract openagents_mobile.chat.authoritative_sync_mode.v1", () => {
  test("selects local once when confirmed Sync does not become live", async () => {
    let sleeps = 0
    const selection = await selectMobileConversation({
      conversation: () => null,
      pollAttempts: 2,
      sleep: async () => { sleeps += 1 },
    })

    expect(selection).toEqual({ mode: "local" })
    expect(sleeps).toBe(2)
  })

  test("selects live Sync and reconstructs the confirmed initial thread", async () => {
    const fixture = makeConversation()
    const selection = await selectMobileConversation({
      conversation: () => fixture.conversation,
      sleep: async () => undefined,
      pollAttempts: 1,
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
    expect(sleeps).toBe(2)
  })
})
