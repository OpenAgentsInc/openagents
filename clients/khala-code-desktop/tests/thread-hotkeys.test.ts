import { describe, expect, test } from "bun:test"

import type { KhalaCodeDesktopCodexThreadSummary } from "../src/shared/codex-threads"
import {
  recentThreadIndexForDigitKey,
  recentThreadsForHotkeys,
} from "../src/ui/thread-hotkeys"

const thread = (
  id: string,
  recencyAt: number | null,
): KhalaCodeDesktopCodexThreadSummary => ({
  id,
  sessionId: null,
  title: id,
  preview: "",
  cwd: null,
  projectLabel: "No working directory",
  status: "idle",
  statusLabel: "idle",
  modelProvider: null,
  source: "unknown",
  forkedFromId: null,
  parentThreadId: null,
  createdAt: null,
  updatedAt: null,
  recencyAt,
  badges: [],
})

describe("Khala Code recent-thread hotkeys", () => {
  test("maps one through zero onto ten recent chat slots", () => {
    expect(recentThreadIndexForDigitKey("1")).toBe(0)
    expect(recentThreadIndexForDigitKey("9")).toBe(8)
    expect(recentThreadIndexForDigitKey("0")).toBe(9)
    expect(recentThreadIndexForDigitKey("a")).toBeNull()
  })

  test("uses recency order and caps hotkey targets at ten chats", () => {
    const threads = Array.from({ length: 12 }, (_, index) =>
      thread(`thread-${index + 1}`, index + 1)
    )

    expect(recentThreadsForHotkeys(threads).map(item => item.id)).toEqual([
      "thread-12",
      "thread-11",
      "thread-10",
      "thread-9",
      "thread-8",
      "thread-7",
      "thread-6",
      "thread-5",
      "thread-4",
      "thread-3",
    ])
  })
})
