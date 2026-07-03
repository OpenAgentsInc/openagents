import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { Window } from "happy-dom"

import {
  checkBehaviorContractCoverage,
  fileOracleSourceLayer,
  renderBehaviorContractMarkdown,
  validateBehaviorContractRegistry,
} from "@openagentsinc/behavior-contracts"
import {
  KHALA_CODE_UX_CONTRACT_DOC_PATH,
  khalaCodeUxContractRegistry,
} from "../src/contracts/ux-contracts"
import type { KhalaCodeDesktopCodexThreadSummary } from "../src/shared/codex-threads"
import type { KhalaCodeDesktopCodexThreadListResult } from "../src/shared/rpc"
import { mountCodexThreadSidebar } from "../src/ui/codex-thread-sidebar"
import { mountRecentThreadOverlay } from "../src/ui/recent-thread-overlay"
import {
  recentThreadCycleDirectionForEvent,
  recentThreadHotkeyIndexForEvent,
  type RecentThreadHotkeyEvent,
} from "../src/ui/thread-hotkeys"

const repoPath = (ref: string): string =>
  new URL(`../../../${ref}`, import.meta.url).pathname

const thread = (
  id: string,
  title: string,
  recencyAt: number,
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
  updatedAt: recencyAt,
  recencyAt,
  badges: [],
})

const withDom = async (
  run: (window: Window) => Promise<void> | void,
): Promise<void> => {
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
    await run(window)
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
}

const hotkeyEvent = (
  overrides: Partial<RecentThreadHotkeyEvent>,
): RecentThreadHotkeyEvent => ({
  altKey: false,
  ctrlKey: false,
  defaultPrevented: false,
  key: "1",
  metaKey: false,
  shiftKey: false,
  ...overrides,
})

describe("khala code ux contract registry", () => {
  test("registry passes mechanical validation", () => {
    const validation = validateBehaviorContractRegistry(khalaCodeUxContractRegistry)
    expect(validation.issues).toEqual([])
    expect(validation.ok).toBe(true)
  })

  test("every enforced bun-test oracle exists and references its contract", async () => {
    const report = await Effect.runPromise(
      checkBehaviorContractCoverage(khalaCodeUxContractRegistry).pipe(
        Effect.provide(
          fileOracleSourceLayer(path => Bun.file(path).text(), repoPath),
        ),
      ),
    )
    expect(report.results.filter(result => result.status !== "covered")).toEqual([])
    expect(report.ok).toBe(true)
  })

  test("the human contract doc stays in sync with the registry", async () => {
    const doc = await Bun.file(repoPath(KHALA_CODE_UX_CONTRACT_DOC_PATH)).text()
    expect(doc).toContain(`Registry version: \`${khalaCodeUxContractRegistry.version}\``)
    for (const contract of khalaCodeUxContractRegistry.contracts) {
      expect(doc).toContain(contract.contractId)
      expect(doc).toContain(contract.statement)
    }
    expect(doc).toContain(
      renderBehaviorContractMarkdown(khalaCodeUxContractRegistry).split("\n")[0] ?? "",
    )
  })
})

// Oracle for khala_code.chat.sidebar_spinner_streaming_only.v1
describe("contract khala_code.chat.sidebar_spinner_streaming_only.v1", () => {
  const listData = (): KhalaCodeDesktopCodexThreadListResult => ({
    ok: true,
    data: [],
    groups: [
      { key: "all", label: "All sessions", threadIds: ["thread-a", "thread-b"] },
    ],
    threads: [
      thread("thread-a", "First chat", 30),
      thread("thread-b", "Second chat", 20),
    ],
  })

  const mountOptions = (overrides: {
    readonly isThreadStreaming?: (threadId: string) => boolean
    readonly resumeThread?: (threadId: string) => Promise<{
      ok: true
      thread: Record<string, never>
      threadId: string
      messages: []
    }>
  }) => ({
    activeThreadId: () => null as string | null,
    archiveThread: async (threadId: string) => ({ action: "archive" as const, ok: true, threadId }),
    deleteThread: async (threadId: string) => ({ action: "delete" as const, ok: true, threadId }),
    forkThread: async (threadId: string) => ({ action: "fork" as const, ok: true, threadId }),
    listThreads: async () => listData(),
    renameThread: async (threadId: string) => ({ action: "rename" as const, ok: true, threadId }),
    resumeThread:
      overrides.resumeThread ??
      (async (threadId: string) => ({
        ok: true as const,
        thread: {},
        threadId,
        messages: [] as const,
      })),
    sessionId: "desktop-session",
    unarchiveThread: async (threadId: string) => ({ action: "unarchive" as const, ok: true, threadId }),
    onNewThreadRequested: () => undefined,
    onThreadSelected: () => undefined,
    ...(overrides.isThreadStreaming === undefined
      ? {}
      : { isThreadStreaming: overrides.isThreadStreaming }),
  })

  test("selecting a thread with the resume RPC in flight shows no spinner", async () => {
    await withDom(async () => {
      const container = document.createElement("aside")
      document.body.append(container)
      const sidebar = mountCodexThreadSidebar(
        container,
        mountOptions({
          resumeThread: () => new Promise(() => undefined),
        }),
      )
      sidebar.setVisible(true)
      await sidebar.refresh()

      const row = container.querySelector<HTMLButtonElement>(
        '[data-thread-id="thread-b"] .khala-thread-sidebar-item-row',
      )
      expect(row).not.toBeNull()
      row?.click()

      const selectingRow = container.querySelector<HTMLButtonElement>(
        '[data-thread-id="thread-b"] .khala-thread-sidebar-item-row',
      )
      expect(selectingRow?.dataset.selecting).toBe("true")
      expect(selectingRow?.getAttribute("aria-busy")).toBe("true")
      expect(container.querySelectorAll(".khala-thread-sidebar-item-spinner")).toHaveLength(0)
      expect(
        selectingRow?.querySelector(".khala-thread-sidebar-item-time"),
      ).not.toBeNull()
    })
  })

  test("a streaming thread shows the spinner in its time slot, others do not", async () => {
    await withDom(async () => {
      const container = document.createElement("aside")
      document.body.append(container)
      const sidebar = mountCodexThreadSidebar(
        container,
        mountOptions({
          isThreadStreaming: threadId => threadId === "thread-a",
        }),
      )
      sidebar.setVisible(true)
      await sidebar.refresh()

      const streamingTime = container.querySelector<HTMLElement>(
        '[data-thread-id="thread-a"] .khala-thread-sidebar-item-time[data-streaming="true"]',
      )
      expect(streamingTime).not.toBeNull()
      expect(
        streamingTime?.querySelector(".khala-thread-sidebar-item-spinner"),
      ).not.toBeNull()
      expect(
        container.querySelector(
          '[data-thread-id="thread-b"] .khala-thread-sidebar-item-spinner',
        ),
      ).toBeNull()
    })
  })

  test("message-loading indication lives in the transcript for cache-miss switches", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    expect(main).toContain("const renderThreadLoadingIndicator = ()")
    expect(main).toContain('shimmer.textContent = "Loading messages"')
    expect(main).toContain("threadSwitchLoadingSelectionId = input.selectionId")
    expect(main).toContain("const threadLoading = renderThreadLoadingIndicator()")
    expect(main).toContain("...(threadLoading === null ? [] : [threadLoading])")
    expect(main).toContain("onThreadSelectionFailed: input => {")
  })
})

// Oracle for khala_code.chat.recent_thread_cmd_hotkeys.v1
describe("contract khala_code.chat.recent_thread_cmd_hotkeys.v1", () => {
  test("Cmd+1 through Cmd+9 map to the nine most recent chats", () => {
    for (let digit = 1; digit <= 9; digit += 1) {
      expect(
        recentThreadHotkeyIndexForEvent(
          hotkeyEvent({ key: String(digit), metaKey: true }),
        ),
      ).toBe(digit - 1)
    }
  })

  test("digits without Cmd, or with other modifiers, jump nowhere", () => {
    expect(recentThreadHotkeyIndexForEvent(hotkeyEvent({ key: "1" }))).toBeNull()
    expect(
      recentThreadHotkeyIndexForEvent(
        hotkeyEvent({ key: "1", metaKey: true, shiftKey: true }),
      ),
    ).toBeNull()
    expect(
      recentThreadHotkeyIndexForEvent(
        hotkeyEvent({ key: "1", altKey: true, metaKey: true }),
      ),
    ).toBeNull()
    expect(
      recentThreadHotkeyIndexForEvent(
        hotkeyEvent({ ctrlKey: true, key: "1", metaKey: true }),
      ),
    ).toBeNull()
    expect(
      recentThreadHotkeyIndexForEvent(
        hotkeyEvent({ defaultPrevented: true, key: "1", metaKey: true }),
      ),
    ).toBeNull()
  })

  test("Cmd+ArrowUp and Cmd+ArrowDown remain the recency cycle extension", () => {
    expect(
      recentThreadCycleDirectionForEvent(hotkeyEvent({ key: "ArrowUp", metaKey: true })),
    ).toBe("newer")
    expect(
      recentThreadCycleDirectionForEvent(hotkeyEvent({ key: "ArrowDown", metaKey: true })),
    ).toBe("older")
    expect(
      recentThreadCycleDirectionForEvent(hotkeyEvent({ key: "ArrowUp" })),
    ).toBeNull()
  })

  test("holding Meta shows the numbered overlay of at most nine recent chats", async () => {
    await withDom(async () => {
      const threads = Array.from({ length: 12 }, (_, index) =>
        thread(`thread-${index}`, `Chat ${index}`, 1000 - index),
      )
      const selected: number[] = []
      const overlay = mountRecentThreadOverlay({
        activeThreadId: () => "thread-1",
        holdDelayMs: 0,
        recentThreads: () => threads,
        onSelect: index => selected.push(index),
      })

      expect(overlay.isVisible()).toBe(false)
      overlay.notifyMetaKeyDown()
      expect(overlay.isVisible()).toBe(true)

      const root = document.querySelector<HTMLElement>(".khala-recent-thread-overlay")
      expect(root).not.toBeNull()
      expect(root?.hidden).toBe(false)

      const items = [...document.querySelectorAll<HTMLButtonElement>(
        ".khala-recent-thread-overlay-item",
      )]
      expect(items).toHaveLength(9)
      expect(items.map(item => item.dataset.digit)).toEqual([
        "1", "2", "3", "4", "5", "6", "7", "8", "9",
      ])
      expect(items[0]?.dataset.threadId).toBe("thread-0")
      expect(items[0]?.textContent).toContain("Chat 0")
      expect(
        items.find(item => item.dataset.threadId === "thread-1")?.dataset.active,
      ).toBe("true")

      overlay.notifyMetaKeyUp()
      expect(overlay.isVisible()).toBe(false)
      expect(root?.hidden).toBe(true)

      overlay.show()
      const third = document.querySelectorAll<HTMLButtonElement>(
        ".khala-recent-thread-overlay-item",
      )[2]
      third?.click()
      expect(selected).toEqual([2])
      expect(overlay.isVisible()).toBe(false)

      overlay.destroy()
    })
  })

  test("the app shell wires Meta hold, release, and window blur to the overlay", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    expect(main).toContain('if (event.key === "Meta"')
    expect(main).toContain("recentThreadOverlay?.notifyMetaKeyDown()")
    expect(main).toContain('window.addEventListener("keyup", event => {')
    expect(main).toContain("recentThreadOverlay?.notifyMetaKeyUp()")
    expect(main).toContain('window.addEventListener("blur", () => {')
    expect(main).toContain("recentThreadOverlay?.hide()")
  })
})
