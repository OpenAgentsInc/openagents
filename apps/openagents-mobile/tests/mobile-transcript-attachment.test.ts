import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"

import type {
  MobileConversationHost,
  MobileConversationThread,
} from "../src/conversation/mobile-conversation"
import {
  buildHomeProgram,
  normalizeMobileAccessibilityProfile,
  renderContentView,
} from "../src/screens/home-core"
import { mobileAttachmentRef } from "../src/screens/mobile-transcript-attachment"

const now = "2026-07-17T21:00:00.000Z"
const attachmentRef = mobileAttachmentRef("message.mobile.image", 0)

const thread: MobileConversationThread = {
  threadRef: "thread.mobile.image",
  title: "Attachment viewer",
  status: "active",
  messageCount: 1,
  lastMessageAt: now,
  updatedAt: now,
  version: 1,
  messages: [{
    messageRef: "message.mobile.image",
    threadRef: "thread.mobile.image",
    body: "See the confirmed image.",
    attachments: [{
      name: "parity-map.png",
      mediaType: "image/png",
      sizeBytes: 68,
      sha256: "a".repeat(64),
      dataBase64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJ",
    }],
    createdAt: now,
    updatedAt: now,
    version: 1,
  }],
}

const host: MobileConversationHost = {
  listThreads: async () => [thread],
  newThread: async () => ({ ok: true, thread }),
  openThread: async () => thread,
  sendMessage: async () => ({ ok: true, thread }),
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

describe("T3M-A4 mobile transcript attachment viewer", () => {
  test("settles loading, opens a contain-fit viewer, and dismisses it", async () => {
    const program = buildHomeProgram({
      conversation: {
        mode: "sync",
        host,
        threads: [thread],
        archivedThreads: [],
        activeThread: thread,
      },
    })
    const initial = JSON.stringify(renderContentView(program.initialState))
    expect(initial).toContain("Loading image")
    expect(initial).toContain("Image attachment, parity-map.png, 1 KB")
    expect(initial).toContain('"alt":"parity-map.png"')
    expect(initial).toContain("TranscriptAttachmentLoadSettled")
    expect(initial).toContain("TranscriptAttachmentOpened")

    program.khala.openAttachment(attachmentRef)
    await Effect.runPromise(settle)
    expect((await Effect.runPromise(lastState(program))).khala.viewingAttachmentRef).toBeNull()

    program.khala.settleAttachmentLoad(attachmentRef, "ready")
    await Effect.runPromise(settle)
    let state = await Effect.runPromise(lastState(program))
    expect(state.khala.attachmentPreviewStates[attachmentRef]).toBe("ready")
    expect(JSON.stringify(renderContentView(state))).toContain('"label":"Image","tone":"neutral"')

    program.khala.openAttachment(attachmentRef)
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    const viewer = JSON.stringify(renderContentView(state))
    expect(viewer).toContain('"_tag":"Modal"')
    expect(viewer).toContain('"fit":"contain"')
    expect(viewer).toContain("Close viewer")

    program.khala.dismissAttachmentViewer(attachmentRef)
    await Effect.runPromise(settle)
    expect((await Effect.runPromise(lastState(program))).khala.viewingAttachmentRef).toBeNull()
  })

  test("shows honest failure/retry state and ignores foreign callbacks", async () => {
    const program = buildHomeProgram({
      accessibility: normalizeMobileAccessibilityProfile({ fontScale: 2, reduceMotion: true }),
      conversation: {
        mode: "sync",
        host,
        threads: [thread],
        archivedThreads: [],
        activeThread: thread,
      },
    })
    program.khala.settleAttachmentLoad(attachmentRef, "failed")
    program.khala.settleAttachmentLoad("message.foreign:attachment:0", "ready")
    await Effect.runPromise(settle)
    let state = await Effect.runPromise(lastState(program))
    const failed = JSON.stringify(renderContentView(state))
    expect(failed).toContain("Preview unavailable")
    expect(failed).toContain("Retry preview")
    expect(failed).toContain('"minHeight":56')
    expect(state.khala.attachmentPreviewStates["message.foreign:attachment:0"]).toBeUndefined()

    program.khala.retryAttachment(attachmentRef)
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(state.khala.attachmentPreviewStates[attachmentRef]).toBe("loading")
    expect(state.khala.attachmentRetryEpochs[attachmentRef]).toBe(1)
    expect(JSON.stringify(renderContentView(state))).toContain(`${attachmentRef}-image-1`)
  })
})
