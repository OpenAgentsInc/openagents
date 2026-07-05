import { describe, expect, test } from "bun:test"

import type { KhalaCodeDesktopCodexThreadSummary } from "../src/shared/codex-threads"
import type { KhalaCodeDesktopMessage } from "../src/shared/rpc"
import {
  KHALA_CODE_CLOSED_SESSION_TABS_STORAGE_KEY,
  khalaCodeSessionActionIntentFor,
  khalaCodeSessionNavigationTarget,
  projectKhalaCodeSessionActionIntents,
  readKhalaCodeClosedSessionTabs,
  recordKhalaCodeClosedSessionTab,
  removeKhalaCodeClosedSessionTab,
} from "../src/shared/session-actions"

const thread = (
  input: Partial<KhalaCodeDesktopCodexThreadSummary> & Pick<KhalaCodeDesktopCodexThreadSummary, "id">,
): KhalaCodeDesktopCodexThreadSummary => {
  const { id, ...overrides } = input
  return {
    badges: [],
    createdAt: null,
    cwd: "/work/openagents",
    forkedFromId: null,
    id,
    modelProvider: "codex",
    parentThreadId: null,
    preview: "",
    projectLabel: "openagents",
    recencyAt: null,
    resumable: true,
    sessionId: id,
    source: "codex_app_server_thread_projection",
    status: "idle",
    statusLabel: "idle",
    title: id,
    unavailableReason: null,
    updatedAt: null,
    ...overrides,
  }
}

const message = (id: string): KhalaCodeDesktopMessage => ({
  body: id,
  id,
  role: "assistant",
})

const memoryStorage = (): Storage => {
  const values = new Map<string, string>()
  return {
    get length() {
      return values.size
    },
    clear: () => values.clear(),
    getItem: key => values.get(key) ?? null,
    key: index => [...values.keys()][index] ?? null,
    removeItem: key => {
      values.delete(key)
    },
    setItem: (key, value) => {
      values.set(key, value)
    },
  }
}

describe("Khala Code session actions", () => {
  test("projects fork archive navigation intents and keeps share disabled", () => {
    const projection = projectKhalaCodeSessionActionIntents({
      activeThreadId: "thread-2",
      closedTabs: [{ closedAt: 42, threadId: "thread-1" }],
      messages: [message("m1"), message("m2")],
      sessions: [thread({ id: "thread-1" }), thread({ id: "thread-2" }), thread({ id: "thread-3" })],
    })

    expect(khalaCodeSessionActionIntentFor(projection, "fork")).toEqual(expect.objectContaining({
      commandId: "session.fork",
      enabled: true,
      runtimeBoundary: "codex_app_server",
      threadId: "thread-2",
    }))
    expect(khalaCodeSessionActionIntentFor(projection, "archive")).toEqual(expect.objectContaining({
      commandId: "session.archive",
      enabled: true,
    }))
    expect(khalaCodeSessionActionIntentFor(projection, "share")).toEqual(expect.objectContaining({
      commandId: "session.share",
      enabled: false,
      runtimeBoundary: "khala_owned_server",
    }))
    expect(khalaCodeSessionActionIntentFor(projection, "share")?.reason).toContain("private local transcripts")
    expect(khalaCodeSessionNavigationTarget(projection, "previous_session")).toBe("thread-1")
    expect(khalaCodeSessionNavigationTarget(projection, "next_session")).toBe("thread-3")
    expect(khalaCodeSessionNavigationTarget(projection, "restore_closed_tab")).toBe("thread-1")
  })

  test("disables unsupported runtime actions without leaking transcript bodies", () => {
    const projection = projectKhalaCodeSessionActionIntents({
      activeThreadId: "stored-local",
      messages: [message("private transcript text")],
      sessions: [thread({
        id: "stored-local",
        resumable: false,
        unavailableReason: "Stored local metadata does not include a current app-server UUID thread id.",
      })],
    })

    expect(khalaCodeSessionActionIntentFor(projection, "fork")).toEqual(expect.objectContaining({
      enabled: false,
      reason: "Stored local metadata does not include a current app-server UUID thread id.",
    }))
    expect(JSON.stringify(projection.intents)).not.toContain("private transcript text")
    expect(khalaCodeSessionActionIntentFor(projection, "previous_message")).toEqual(expect.objectContaining({
      enabled: false,
      reason: "Need at least two visible messages",
    }))
  })

  test("records, deduplicates, and removes closed session tabs", () => {
    const storage = memoryStorage()

    recordKhalaCodeClosedSessionTab(storage, "thread-1", 10)
    recordKhalaCodeClosedSessionTab(storage, "thread-2", 20)
    recordKhalaCodeClosedSessionTab(storage, "thread-1", 30)

    expect(storage.getItem(KHALA_CODE_CLOSED_SESSION_TABS_STORAGE_KEY)).not.toBeNull()
    expect(readKhalaCodeClosedSessionTabs(storage)).toEqual([
      { closedAt: 30, threadId: "thread-1" },
      { closedAt: 20, threadId: "thread-2" },
    ])
    expect(removeKhalaCodeClosedSessionTab(storage, "thread-1")).toEqual([
      { closedAt: 20, threadId: "thread-2" },
    ])
  })
})
