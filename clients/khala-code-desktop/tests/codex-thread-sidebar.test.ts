import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import type { KhalaCodeDesktopCodexThreadSummary } from "../src/shared/codex-threads"
import type { KhalaCodeDesktopCodexThreadListResult } from "../src/shared/rpc"
import {
  mountCodexThreadSidebar,
  renameThreadInListData,
  upsertPendingThreadInListData,
} from "../src/ui/codex-thread-sidebar"

const thread = (
  id: string,
  title: string,
): KhalaCodeDesktopCodexThreadSummary => ({
  id,
  sessionId: id,
  title,
  preview: `${title} preview`,
  cwd: "/repo/app",
  projectLabel: "app",
  status: "idle",
  statusLabel: "idle",
  modelProvider: "openai",
  source: "appServer",
  forkedFromId: null,
  parentThreadId: null,
  createdAt: 1,
  updatedAt: 2,
  recencyAt: 3,
  badges: [],
})

describe("Khala Code thread sidebar", () => {
  test("renames the visible thread title in list data immediately", () => {
    const data: KhalaCodeDesktopCodexThreadListResult = {
      ok: true,
      data: [],
      groups: [{ key: "/repo/app", label: "app", threadIds: ["thread-a", "thread-b"] }],
      threads: [
        thread("thread-a", "Old name"),
        thread("thread-b", "Other thread"),
      ],
    }

    const renamed = renameThreadInListData(data, "thread-a", "New name")

    expect(renamed).not.toBe(data)
    expect(renamed.data).toBe(data.data)
    expect(renamed.groups).toBe(data.groups)
    expect(renamed.threads?.map(item => item.title)).toEqual([
      "New name",
      "Other thread",
    ])
    expect(data.threads?.[0]?.title).toBe("Old name")
  })

  test("keeps list data stable when the thread title is already current", () => {
    const data: KhalaCodeDesktopCodexThreadListResult = {
      ok: true,
      data: [],
      threads: [thread("thread-a", "Current name")],
    }

    expect(renameThreadInListData(data, "thread-a", "Current name")).toBe(data)
    expect(renameThreadInListData(data, "missing-thread", "New name")).toBe(data)
  })

  test("prepends a pending active thread until persisted thread metadata catches up", () => {
    const data: KhalaCodeDesktopCodexThreadListResult = {
      ok: true,
      data: [],
      groups: [{ key: "/repo/app", label: "app", threadIds: ["thread-a"] }],
      threads: [thread("thread-a", "Existing")],
    }
    const pending = {
      ...thread("thread-new", "hi"),
      cwd: null,
      projectLabel: "Current chat",
      recencyAt: 10,
    }

    const next = upsertPendingThreadInListData(data, pending)

    expect(next).not.toBe(data)
    expect(next.threads?.map(item => item.id)).toEqual(["thread-new", "thread-a"])
    expect(next.groups?.[0]).toEqual({
      key: "cwd:none",
      label: "Current chat",
      threadIds: ["thread-new"],
    })
    expect(upsertPendingThreadInListData(next, pending)).toBe(next)
  })

  test("renders mixed Codex and Claude catalog entries with harness badges", async () => {
    const window = new Window()
    const previousDocument = globalThis.document
    const previousNavigator = globalThis.navigator
    Object.defineProperty(globalThis, "document", {
      configurable: true,
      value: window.document,
    })
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: window.navigator,
    })
    try {
      const container = document.createElement("aside")
      document.body.append(container)
      const data: KhalaCodeDesktopCodexThreadListResult = {
        ok: true,
        data: [],
        groups: [{ key: "all", label: "All sessions", threadIds: ["codex-thread", "claude-session"] }],
        threads: [
          { ...thread("codex-thread", "Codex work"), badges: ["Codex"] },
          {
            ...thread("claude-session", "Claude plan"),
            badges: ["Claude"],
            modelProvider: "claude",
            source: "claude_sdk_list_sessions",
          },
        ],
      }

      const sidebar = mountCodexThreadSidebar(container, {
        activeThreadId: () => null,
        archiveThread: async threadId => ({ action: "archive", ok: true, threadId }),
        deleteThread: async threadId => ({ action: "delete", ok: true, threadId }),
        forkThread: async threadId => ({ action: "fork", ok: true, threadId }),
        listThreads: async () => data,
        renameThread: async threadId => ({ action: "rename", ok: true, threadId }),
        resumeThread: async threadId => ({
          ok: true,
          thread: {},
          threadId,
          messages: [],
        }),
        sessionId: "desktop-session",
        unarchiveThread: async threadId => ({ action: "unarchive", ok: true, threadId }),
        onNewThreadRequested: () => undefined,
        onThreadSelected: () => undefined,
      })

      sidebar.setVisible(true)
      await sidebar.refresh()

      expect([...container.querySelectorAll(".khala-thread-sidebar-harness-badge")]
        .map(node => node.textContent)).toEqual(["Codex", "Claude"])
      expect(container.textContent).toContain("Codex work")
      expect(container.textContent).toContain("Claude plan")
    } finally {
      Object.defineProperty(globalThis, "document", {
        configurable: true,
        value: previousDocument,
      })
      Object.defineProperty(globalThis, "navigator", {
        configurable: true,
        value: previousNavigator,
      })
      window.close()
    }
  })
})
