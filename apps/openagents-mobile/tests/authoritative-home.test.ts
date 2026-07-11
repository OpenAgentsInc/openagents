import { describe, expect, test } from "bun:test"
import { Effect, Stream } from "@effect-native/core/effect"

import type {
  MobileConversationHost,
  MobileConversationSelection,
  MobileConversationThread,
} from "../src/conversation/mobile-conversation"
import {
  buildHomeProgram,
  renderContentView,
  renderDrawerView,
} from "../src/screens/home-core"

const now = "2026-07-10T20:15:00.000Z"
const initialThread: MobileConversationThread = {
  threadRef: "thread.synced.1",
  title: "Synced",
  messageCount: 1,
  lastMessageAt: now,
  updatedAt: now,
  version: 3,
  messages: [{
    messageRef: "message.synced.1",
    threadRef: "thread.synced.1",
    body: "Confirmed",
    createdAt: now,
    updatedAt: now,
    version: 5,
  }],
}

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

const selection = (host: MobileConversationHost): Extract<MobileConversationSelection, { mode: "sync" }> => ({
  mode: "sync",
  host,
  threads: [initialThread],
  activeThread: initialThread,
})

describe("contract openagents_mobile.chat.authoritative_sync_mode.v1 Home", () => {
  test("boots from confirmed refs/versions and exposes confirmed thread navigation", () => {
    const host: MobileConversationHost = {
      listThreads: async () => [initialThread],
      newThread: async () => ({ ok: true, thread: initialThread }),
      openThread: async () => initialThread,
      sendMessage: async () => ({ ok: true, thread: initialThread }),
    }
    const program = buildHomeProgram({ conversation: selection(host) })

    expect(program.initialState).toMatchObject({
      conversationAuthority: "sync",
      syncPhase: "live",
      activeThreadRef: "thread.synced.1",
    })
    expect(program.initialState.khala.entries[0]).toMatchObject({
      key: "message.synced.1",
      text: "Confirmed",
      version: 5,
      status: "done",
    })
    const content = JSON.stringify(renderContentView(program.initialState))
    expect(content).toContain("Confirmed conversation, continuous across your devices.")
    expect(content).toContain('"senderLabel":"YOU"')
    const drawer = JSON.stringify(renderDrawerView(program.initialState))
    expect(drawer).toContain("drawer-thread-thread.synced.1")
    expect(drawer).toContain('"label":"Synced"')
  })

  test("marks a submitted draft pending, then replaces it only with exact confirmed state", async () => {
    let resolveSend: ((value: Awaited<ReturnType<MobileConversationHost["sendMessage"]>>) => void) | undefined
    const confirmed: MobileConversationThread = {
      ...initialThread,
      messageCount: 2,
      version: 7,
      messages: [
        ...initialThread.messages,
        {
          messageRef: "message.mobile.confirmed",
          threadRef: initialThread.threadRef,
          body: "Continue this",
          createdAt: now,
          updatedAt: now,
          version: 7,
        },
      ],
    }
    const host: MobileConversationHost = {
      listThreads: async () => [initialThread],
      newThread: async () => ({ ok: true, thread: initialThread }),
      openThread: async () => initialThread,
      sendMessage: () => new Promise(resolve => { resolveSend = resolve }),
    }
    const program = buildHomeProgram({ conversation: selection(host) })

    program.khala.submitTurn("Continue this")
    await Effect.runPromise(settle)
    const pending = await Effect.runPromise(lastState(program))
    expect(pending.khala.entries.at(-1)).toMatchObject({
      key: "pending-mobile-1",
      text: "Continue this",
      status: "pending",
    })
    expect(JSON.stringify(renderContentView(pending))).toContain('"senderLabel":"YOU · PENDING"')

    resolveSend?.({ ok: true, thread: confirmed })
    await Effect.runPromise(settle)
    const completed = await Effect.runPromise(lastState(program))
    expect(completed.khala.pending).toBe(false)
    expect(completed.khala.entries.some(entry => entry.key.startsWith("pending-"))).toBe(false)
    expect(completed.khala.entries.at(-1)).toMatchObject({
      key: "message.mobile.confirmed",
      version: 7,
      status: "done",
    })
  })

  test("removes an unconfirmed draft and clears account-linked state on denial", async () => {
    const host: MobileConversationHost = {
      listThreads: async () => [initialThread],
      newThread: async () => ({ ok: true, thread: initialThread }),
      openThread: async () => initialThread,
      sendMessage: async () => ({ ok: false, error: "Message is still pending reconciliation." }),
    }
    const program = buildHomeProgram({ conversation: selection(host) })

    program.khala.submitTurn("Never confirmed")
    await Effect.runPromise(settle)
    await Effect.runPromise(settle)
    const failed = await Effect.runPromise(lastState(program))
    expect(failed.khala.entries.some(entry => entry.key.startsWith("pending-"))).toBe(false)
    expect(failed.khala.entries.at(-1)).toMatchObject({
      role: "system",
      status: "failed",
      text: "Message is still pending reconciliation.",
    })

    program.sync.setPhase("denied")
    await Effect.runPromise(settle)
    const denied = await Effect.runPromise(lastState(program))
    expect(denied.activeThreadRef).toBeNull()
    expect(denied.conversationThreads).toEqual([])
    expect(denied.khala.entries).toEqual([])
  })
})
