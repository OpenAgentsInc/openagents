import { describe, expect, test } from "bun:test"

import type {
  KhalaCodeDesktopKhalaSyncChatMessage,
  KhalaCodeDesktopKhalaSyncRuntimeMessage,
} from "../src/shared/rpc"
import {
  khalaSyncChatMessageToDesktopMessage,
  khalaSyncRuntimeMessageToDesktopMessage,
  mergeKhalaSyncChatAndRuntimeMessages,
} from "../src/ui/khala-sync-thread-messages-core"

/**
 * #8425 desktop render-gap closeout: this is the exact seam
 * `khalaSyncThreadResult` in `main.ts` (a DOM-mounting entrypoint that isn't
 * itself unit-testable) calls to build the message list
 * `activateCodexThread` renders. Proving this merge is correct is what
 * proves a mobile-dispatched turn's reply becomes visible in the desktop
 * UI, not just present in the Khala Sync data layer.
 */

const chatMessage = (
  patch: Partial<KhalaCodeDesktopKhalaSyncChatMessage> = {},
): KhalaCodeDesktopKhalaSyncChatMessage => ({
  authorUserId: "user-owner",
  body: "please say the test phrase",
  createdAt: "2026-07-04T20:00:00.000Z",
  deletedAt: null,
  messageId: "chat-message.1",
  threadId: "thread.1",
  updatedAt: "2026-07-04T20:00:00.000Z",
  ...patch,
})

const runtimeMessage = (
  patch: Partial<KhalaCodeDesktopKhalaSyncRuntimeMessage> = {},
): KhalaCodeDesktopKhalaSyncRuntimeMessage => ({
  body: "codex mobile-to-desktop-test-ok",
  role: "assistant",
  sortKey: "2026-07-04T20:00:01.000Z",
  turnId: "turn.1",
  ...patch,
})

describe("khalaSyncChatMessageToDesktopMessage", () => {
  test("always renders as role user — chat_message rows are only ever the human prompt", () => {
    expect(khalaSyncChatMessageToDesktopMessage(chatMessage())).toEqual({
      body: "please say the test phrase",
      id: "chat-message.1",
      role: "user",
    })
  })
})

describe("khalaSyncRuntimeMessageToDesktopMessage", () => {
  test("renders a folded runtime turn as an assistant message", () => {
    expect(khalaSyncRuntimeMessageToDesktopMessage(runtimeMessage())).toEqual({
      body: "codex mobile-to-desktop-test-ok",
      id: "runtime-turn.turn.1",
      role: "assistant",
    })
  })
})

describe("mergeKhalaSyncChatAndRuntimeMessages", () => {
  test("closes the #8425 gap: a mobile-dispatched turn's reply appears after the human prompt in the rendered list", () => {
    const merged = mergeKhalaSyncChatAndRuntimeMessages(
      [chatMessage()],
      [runtimeMessage()],
    )
    expect(merged).toEqual([
      { body: "please say the test phrase", id: "chat-message.1", role: "user" },
      { body: "codex mobile-to-desktop-test-ok", id: "runtime-turn.turn.1", role: "assistant" },
    ])
  })

  test("interleaves multiple turns and messages in chronological order, not grouped by kind", () => {
    const merged = mergeKhalaSyncChatAndRuntimeMessages(
      [
        chatMessage({ body: "first prompt", createdAt: "2026-07-04T20:00:00.000Z", messageId: "m1" }),
        chatMessage({ body: "second prompt", createdAt: "2026-07-04T20:00:05.000Z", messageId: "m2" }),
      ],
      [
        runtimeMessage({ body: "first reply", sortKey: "2026-07-04T20:00:01.000Z", turnId: "t1" }),
        runtimeMessage({ body: "second reply", sortKey: "2026-07-04T20:00:06.000Z", turnId: "t2" }),
      ],
    )
    expect(merged.map(message => message.body)).toEqual([
      "first prompt",
      "first reply",
      "second prompt",
      "second reply",
    ])
  })

  test("a desktop-only thread with zero runtime rows behaves exactly as before — pure chat_message rendering", () => {
    const merged = mergeKhalaSyncChatAndRuntimeMessages(
      [chatMessage({ body: "only a chat message" })],
      [],
    )
    expect(merged).toEqual([
      { body: "only a chat message", id: "chat-message.1", role: "user" },
    ])
  })

  test("an empty thread merges to an empty list", () => {
    expect(mergeKhalaSyncChatAndRuntimeMessages([], [])).toEqual([])
  })
})
