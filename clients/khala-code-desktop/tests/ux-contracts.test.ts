import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { Window } from "happy-dom"

import {
  checkBehaviorContractCoverage,
  renderBehaviorContractMarkdown,
  validateBehaviorContractRegistry,
} from "@openagentsinc/behavior-contracts"
import { khalaCodeQaSeedScenarioOracleSourceLayer } from "@openagentsinc/khala-qa-harness/behavior-contract-oracles"
import {
  KHALA_CODE_UX_CONTRACT_DOC_PATH,
  khalaCodeUxContractRegistry,
} from "../src/contracts/ux-contracts"
import type { KhalaCodeDesktopCodexThreadSummary } from "../src/shared/codex-threads"
import type { KhalaCodeDesktopCodexThreadListResult } from "../src/shared/rpc"
import { mountCodexThreadSidebar } from "../src/ui/codex-thread-sidebar"
import { bindRecentThreadHotkeyHints } from "../src/ui/recent-thread-hotkey-hints"
import {
  recentThreadCycleDirectionForEvent,
  recentThreadHotkeyIndexForEvent,
  type RecentThreadHotkeyEvent,
} from "../src/ui/thread-hotkeys"
import {
  renderThinkingIndicator,
  renderThreadLoadingIndicator,
} from "../src/ui/transcript-status-indicators"

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
          khalaCodeQaSeedScenarioOracleSourceLayer({
            readFile: path => Bun.file(path).text(),
            resolvePath: repoPath,
          }),
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

  test("message-loading indication renders as a transcript status bubble", async () => {
    await withDom(async () => {
      const container = document.createElement("main")
      document.body.append(container)

      expect(renderThreadLoadingIndicator(null)).toBeNull()
      const indicator = renderThreadLoadingIndicator(42)
      expect(indicator).not.toBeNull()
      container.append(indicator as HTMLElement)

      const article = container.querySelector<HTMLElement>(
        '[data-khala-thread-loading="true"][data-message-id="thread-loading-42"]',
      )
      expect(article).not.toBeNull()
      expect(article?.classList.contains("message-bubble--assistant")).toBe(true)
      expect(article?.classList.contains("message-bubble--thinking")).toBe(true)

      const status = article?.querySelector<HTMLElement>('[role="status"]')
      expect(status?.getAttribute("aria-live")).toBe("polite")
      expect(status?.getAttribute("aria-label")).toBe("Loading messages")
      expect(status?.dataset.oaAiShimmer).toBe("")
      expect(status?.textContent).toBe("Loading messages")
    })
  })

  test("thinking indication uses the same mountable transcript status renderer", async () => {
    await withDom(async () => {
      const indicator = renderThinkingIndicator("turn-7")
      expect(indicator?.dataset.messageId).toBe("thinking-turn-7")
      expect(indicator?.dataset.khalaThinking).toBe("true")
      const status = indicator?.querySelector<HTMLElement>('[role="status"]')
      expect(status?.getAttribute("aria-label")).toBe("Thinking")
      expect(status?.textContent).toBe("Thinking")
    })
  })
})

// Oracle for khala_code.chat.sidebar_active_thread_background_only.v2
describe("contract khala_code.chat.sidebar_active_thread_background_only.v2", () => {
  test("the active-row background is a distinct energy-blue tone, not the surface mix", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()
    const activeRule = css.split('.khala-thread-sidebar-item[data-active="true"] {')[1]?.split("}")[0] ?? ""
    expect(activeRule).toContain("--oa-color-khala-energy-blue")
    expect(activeRule).not.toContain("--oa-color-khala-surface")
    const hoverRule = css.split(".khala-thread-sidebar-item:hover {")[1]?.split("}")[0] ?? ""
    expect(hoverRule).toContain("--oa-color-khala-surface")
  })

  test("an optimistic current chat uses the active row background hook without visible current-chat copy", async () => {
    await withDom(async () => {
      const container = document.createElement("aside")
      document.body.append(container)
      const data: KhalaCodeDesktopCodexThreadListResult = {
        ok: true,
        data: [],
        groups: [{ key: "all", label: "All sessions", threadIds: ["thread-a"] }],
        threads: [thread("thread-a", "Existing chat", 20)],
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

      sidebar.upsertPendingThread({
        preview: "New request\nwith details",
        threadId: "thread-current",
      })

      const activeItem = container.querySelector<HTMLElement>(
        '[data-thread-id="thread-current"]',
      )
      const activeRow = activeItem?.querySelector<HTMLButtonElement>(
        ".khala-thread-sidebar-item-row",
      )
      expect(activeItem?.dataset.active).toBe("true")
      expect(activeRow?.dataset.active).toBe("true")
      expect(activeRow?.getAttribute("aria-current")).toBe("true")
      expect(activeRow?.textContent).toContain("New request")
      expect(container.textContent).not.toContain("Current chat")
      expect(
        [...container.querySelectorAll(".khala-thread-sidebar-group-title")]
          .map(heading => heading.textContent),
      ).not.toContain("Current chat")
    })
  })
})

// Oracle for khala_code.chat.recent_thread_cmd_hotkeys.v2
describe("contract khala_code.chat.recent_thread_cmd_hotkeys.v2", () => {
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

  test("hotkey hints replace the timestamps of the nine most recent chats in place", async () => {
    await withDom(async () => {
      const threads = Array.from({ length: 12 }, (_, index) =>
        thread(`thread-${index}`, `Chat ${index}`, 1000 - index),
      )
      const container = document.createElement("aside")
      document.body.append(container)
      const sidebar = mountCodexThreadSidebar(container, {
        activeThreadId: () => "thread-1",
        archiveThread: async (threadId: string) => ({ action: "archive" as const, ok: true, threadId }),
        deleteThread: async (threadId: string) => ({ action: "delete" as const, ok: true, threadId }),
        forkThread: async (threadId: string) => ({ action: "fork" as const, ok: true, threadId }),
        listThreads: async () => ({
          ok: true as const,
          data: [],
          groups: [
            { key: "all", label: "All sessions", threadIds: threads.map(entry => entry.id) },
          ],
          threads,
        }),
        renameThread: async (threadId: string) => ({ action: "rename" as const, ok: true, threadId }),
        resumeThread: async (threadId: string) => ({
          ok: true as const,
          thread: {},
          threadId,
          messages: [] as const,
        }),
        sessionId: "desktop-session",
        unarchiveThread: async (threadId: string) => ({ action: "unarchive" as const, ok: true, threadId }),
        onNewThreadRequested: () => undefined,
        onThreadSelected: () => undefined,
      })
      sidebar.setVisible(true)
      await sidebar.refresh()

      const timesBefore = [...container.querySelectorAll<HTMLElement>(
        ".khala-thread-sidebar-item-time",
      )]
      expect(timesBefore.length).toBe(12)
      expect(timesBefore.every(time => time.dataset.hotkeyHint === undefined)).toBe(true)

      sidebar.setHotkeyHintsVisible(true)

      const hints = [...container.querySelectorAll<HTMLElement>(
        ".khala-thread-sidebar-item-time[data-hotkey-hint]",
      )]
      expect(hints.map(hint => hint.dataset.hotkeyHint)).toEqual([
        "1", "2", "3", "4", "5", "6", "7", "8", "9",
      ])
      expect(hints.map(hint => hint.textContent)).toEqual([
        "\u23181", "\u23182", "\u23183", "\u23184", "\u23185", "\u23186", "\u23187", "\u23188", "\u23189",
      ])
      expect(
        container.querySelector('[data-thread-id="thread-0"] [data-hotkey-hint="1"]'),
      ).not.toBeNull()
      expect(
        container.querySelector('[data-thread-id="thread-9"] [data-hotkey-hint]'),
      ).toBeNull()
      expect(container.querySelectorAll(".khala-thread-sidebar-item-time")).toHaveLength(12)
      expect(document.querySelector(".khala-overlay-menu")).toBeNull()

      sidebar.setHotkeyHintsVisible(false)

      const timesAfter = [...container.querySelectorAll<HTMLElement>(
        ".khala-thread-sidebar-item-time",
      )]
      expect(timesAfter.length).toBe(12)
      expect(timesAfter.every(time => time.dataset.hotkeyHint === undefined)).toBe(true)
    })
  })

  test("Meta hold, release, and window blur drive sidebar hints through DOM events", async () => {
    await withDom(async window => {
      const visibility: boolean[] = []
      const binding = bindRecentThreadHotkeyHints(window, {
        setHotkeyHintsVisible: visible => visibility.push(visible),
      }, { holdDelayMs: 0 })

      window.dispatchEvent(new window.KeyboardEvent("keydown", { key: "Meta" }))
      window.dispatchEvent(new window.KeyboardEvent("keydown", { key: "Meta", repeat: true }))
      expect(visibility).toEqual([true])

      window.dispatchEvent(new window.KeyboardEvent("keyup", { key: "Meta" }))
      expect(visibility).toEqual([true, false])

      window.dispatchEvent(new window.KeyboardEvent("keydown", { key: "Meta" }))
      window.dispatchEvent(new window.Event("blur"))
      expect(visibility).toEqual([true, false, true, false])

      binding.dispose()
      window.dispatchEvent(new window.KeyboardEvent("keydown", { key: "Meta" }))
      expect(visibility).toEqual([true, false, true, false])
      expect(document.querySelector(".khala-overlay-menu")).toBeNull()
    })
  })
})
