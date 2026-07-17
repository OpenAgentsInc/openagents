import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"

import type {
  MobileConversationHost,
  MobileConversationThread,
} from "../src/conversation/mobile-conversation"
import {
  buildHomeProgram,
  renderContentView,
} from "../src/screens/home-core"
import {
  MOBILE_TRANSCRIPT_PAGE_SIZE,
  mobileTranscriptUnreadBoundaryIndex,
  nextMobileTranscriptVisibleCount,
} from "../src/screens/mobile-transcript-history"

const createdAt = (index: number): string =>
  new Date(Date.UTC(2026, 6, 17, 20, 0, 0, index)).toISOString()

const messages = (count: number, finalBody = `Message ${count}`) =>
  Array.from({ length: count }, (_, index) => ({
    messageRef: `message.mobile.history.${index + 1}`,
    threadRef: "thread.mobile.history",
    body: index === count - 1 ? finalBody : `Message ${index + 1}`,
    createdAt: createdAt(index),
    updatedAt: createdAt(index),
    version: index + 1,
  }))

const thread = (count: number, total = count, finalBody?: string): MobileConversationThread => ({
  threadRef: "thread.mobile.history",
  title: "Long mobile history",
  status: "active",
  messageCount: total,
  lastMessageAt: createdAt(count - 1),
  updatedAt: createdAt(count - 1),
  version: count,
  messages: messages(count, finalBody),
})

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

describe("T3M-A4 mobile transcript history and scrolling", () => {
  test("bounds retained pages and computes a deterministic unread boundary", () => {
    expect(MOBILE_TRANSCRIPT_PAGE_SIZE).toBe(60)
    expect(nextMobileTranscriptVisibleCount(60, 145)).toBe(120)
    expect(nextMobileTranscriptVisibleCount(120, 145)).toBe(145)
    expect(mobileTranscriptUnreadBoundaryIndex(60, 3)).toBe(57)
    expect(mobileTranscriptUnreadBoundaryIndex(0, 3)).toBeNull()
  })

  test("loads earlier retained rows and names unavailable server history", async () => {
    const initialThread = thread(130, 140)
    const host: MobileConversationHost = {
      listThreads: async () => [initialThread],
      newThread: async () => ({ ok: true, thread: initialThread }),
      openThread: async () => initialThread,
      sendMessage: async () => ({ ok: true, thread: initialThread }),
    }
    const program = buildHomeProgram({ conversation: {
      mode: "sync", host, threads: [initialThread], archivedThreads: [], activeThread: initialThread,
    } })
    expect(program.initialState.khala.transcriptVisibleCount).toBe(60)
    const initial = JSON.stringify(renderContentView(program.initialState))
    expect(initial).toContain("Load 60 earlier")
    expect(initial).toContain("10 earlier messages are not retained on this device")
    expect(initial).toContain('"preserveScrollAnchor":true')
    expect(initial).toContain('"virtualize":true')

    program.khala.loadEarlierTranscript()
    await Effect.runPromise(settle)
    let state = await Effect.runPromise(lastState(program))
    expect(state.khala.transcriptVisibleCount).toBe(120)
    expect(JSON.stringify(renderContentView(state))).toContain("Load 10 earlier")

    program.khala.loadEarlierTranscript()
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(state.khala.transcriptVisibleCount).toBe(130)
    expect(JSON.stringify(renderContentView(state))).not.toContain("khala-load-earlier-history")
  })

  test("suspends auto-pin, updates stable rows in place, and recovers unread updates", async () => {
    const initialThread = thread(2)
    let update: ((value: MobileConversationThread) => void) | undefined
    const host: MobileConversationHost = {
      listThreads: async () => [initialThread],
      newThread: async () => ({ ok: true, thread: initialThread }),
      openThread: async () => initialThread,
      watchThread: async (_threadRef, onUpdate) => {
        update = onUpdate
        return { close: async () => undefined }
      },
      sendMessage: async () => ({ ok: true, thread: initialThread }),
    }
    const program = buildHomeProgram({ conversation: {
      mode: "sync", host, threads: [initialThread], archivedThreads: [], activeThread: initialThread,
    } })
    await Effect.runPromise(settle)
    program.khala.setTranscriptPinned(false)
    await Effect.runPromise(settle)

    update?.(thread(2, 2, "Message 2 streamed replacement"))
    await Effect.runPromise(settle)
    let state = await Effect.runPromise(lastState(program))
    expect(state.khala.entries).toHaveLength(2)
    expect(state.khala.entries.at(-1)?.text).toBe("Message 2 streamed replacement")
    expect(state.khala.transcriptUnreadCount).toBe(0)

    update?.(thread(3))
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    const unread = JSON.stringify(renderContentView(state))
    expect(state.khala.transcriptPinned).toBe(false)
    expect(state.khala.transcriptUnreadCount).toBe(1)
    expect(unread).toContain("1 unread update")
    expect(unread).toContain("Jump to latest · 1 unread")
    expect(unread).toContain('"pinToEnd":false')

    program.khala.jumpToLatestTranscript()
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(state.khala.transcriptPinned).toBe(true)
    expect(state.khala.transcriptUnreadCount).toBe(0)
    expect(state.khala.transcriptScrollToKey).toBe("message.mobile.history.3")
  })
})
