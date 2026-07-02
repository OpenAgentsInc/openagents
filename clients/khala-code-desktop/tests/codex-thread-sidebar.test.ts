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
  test("marks the currently active chat row as active", async () => {
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
        groups: [{ key: "all", label: "All sessions", threadIds: ["thread-a", "thread-b"] }],
        threads: [
          thread("thread-a", "Active work"),
          thread("thread-b", "Other work"),
        ],
      }

      let activeThreadId: string | null = "thread-a"
      const sidebar = mountCodexThreadSidebar(container, {
        activeThreadId: () => activeThreadId,
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

      const activeItem = container.querySelector<HTMLElement>('[data-thread-id="thread-a"]')
      const activeRow = activeItem?.querySelector<HTMLButtonElement>(".khala-thread-sidebar-item-row")
      const inactiveItem = container.querySelector<HTMLElement>('[data-thread-id="thread-b"]')
      const inactiveRow = inactiveItem?.querySelector<HTMLButtonElement>(".khala-thread-sidebar-item-row")

      expect(activeItem?.dataset.active).toBe("true")
      expect(activeRow?.dataset.active).toBe("true")
      expect(activeRow?.getAttribute("aria-current")).toBe("true")
      expect(inactiveItem?.dataset.active).toBe("false")
      expect(inactiveRow?.dataset.active).toBe("false")
      expect(inactiveRow?.hasAttribute("aria-current")).toBe(false)

      activeThreadId = "thread-b"
      sidebar.setActiveThreadId(activeThreadId)

      const nextInactiveItem = container.querySelector<HTMLElement>('[data-thread-id="thread-a"]')
      const nextInactiveRow = nextInactiveItem?.querySelector<HTMLButtonElement>(".khala-thread-sidebar-item-row")
      const nextActiveItem = container.querySelector<HTMLElement>('[data-thread-id="thread-b"]')
      const nextActiveRow = nextActiveItem?.querySelector<HTMLButtonElement>(".khala-thread-sidebar-item-row")

      expect(nextInactiveItem?.dataset.active).toBe("false")
      expect(nextInactiveRow?.dataset.active).toBe("false")
      expect(nextInactiveRow?.hasAttribute("aria-current")).toBe(false)
      expect(nextActiveItem?.dataset.active).toBe("true")
      expect(nextActiveRow?.dataset.active).toBe("true")
      expect(nextActiveRow?.getAttribute("aria-current")).toBe("true")
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

  test("keeps the thread list mounted while a selected thread hydrates or fails", async () => {
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
        groups: [{ key: "all", label: "All sessions", threadIds: ["thread-a", "thread-b"] }],
        threads: [
          thread("thread-a", "Visible work"),
          { ...thread("thread-b", "Slow Claude session"), badges: ["Claude"] },
        ],
      }

      let rejectResume: (error: Error) => void = () => {
        throw new Error("resume rejection was not captured")
      }
      const sidebar = mountCodexThreadSidebar(container, {
        activeThreadId: () => "thread-a",
        archiveThread: async threadId => ({ action: "archive", ok: true, threadId }),
        deleteThread: async threadId => ({ action: "delete", ok: true, threadId }),
        forkThread: async threadId => ({ action: "fork", ok: true, threadId }),
        listThreads: async () => data,
        renameThread: async threadId => ({ action: "rename", ok: true, threadId }),
        resumeThread: threadId =>
          new Promise((_resolve, reject) => {
            expect(threadId).toBe("thread-b")
            rejectResume = reject
          }),
        sessionId: "desktop-session",
        unarchiveThread: async threadId => ({ action: "unarchive", ok: true, threadId }),
        onNewThreadRequested: () => undefined,
        onThreadSelectionStarted: () => undefined,
        onThreadSelected: () => undefined,
      })

      sidebar.setVisible(true)
      await sidebar.refresh()

      container
        .querySelector<HTMLButtonElement>('[data-thread-id="thread-b"] .khala-thread-sidebar-item-row')
        ?.click()
      await Promise.resolve()

      expect(container.textContent).toContain("Visible work")
      expect(container.textContent).toContain("Slow Claude session")
      expect(container.textContent).not.toBe("Loading threads")
      expect(
        container
          .querySelector<HTMLButtonElement>('[data-thread-id="thread-b"] .khala-thread-sidebar-item-row')
          ?.getAttribute("aria-busy"),
      ).toBe("true")

      rejectResume(new Error("Claude transcript unavailable"))
      await Promise.resolve()
      await Promise.resolve()

      expect(container.textContent).toContain("Visible work")
      expect(container.textContent).toContain("Slow Claude session")
      expect(container.textContent).toContain("Claude transcript unavailable")
      expect(container.textContent).not.toBe("Loading threads")
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

  test("selects recent visible threads without refetching the catalog", async () => {
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
        groups: [{ key: "all", label: "All sessions", threadIds: ["thread-a", "thread-b"] }],
        threads: [
          { ...thread("thread-a", "Newest work"), recencyAt: 20 },
          { ...thread("thread-b", "Older work"), recencyAt: 10 },
        ],
      }

      let listCalls = 0
      const selectedThreadIds: string[] = []
      const sidebar = mountCodexThreadSidebar(container, {
        activeThreadId: () => null,
        archiveThread: async threadId => ({ action: "archive", ok: true, threadId }),
        deleteThread: async threadId => ({ action: "delete", ok: true, threadId }),
        forkThread: async threadId => ({ action: "fork", ok: true, threadId }),
        listThreads: async () => {
          listCalls += 1
          return data
        },
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
        onThreadSelected: input => selectedThreadIds.push(input.threadId),
      })

      sidebar.setVisible(true)
      await Promise.resolve()
      expect(listCalls).toBe(1)

      await expect(sidebar.selectRecentThread(0)).resolves.toBe(true)

      expect(listCalls).toBe(1)
      expect(selectedThreadIds).toEqual(["thread-a"])
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
