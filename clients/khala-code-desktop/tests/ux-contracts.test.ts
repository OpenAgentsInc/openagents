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
import {
  friendlyKhalaCodeCodexThreadOpenErrorMessage,
  isKhalaCodeCodexThreadOpenInternalError,
  type KhalaCodeDesktopCodexThreadSummary,
} from "../src/shared/codex-threads"
import type {
  KhalaCodeDesktopCodexThreadListResult,
  KhalaCodeDesktopCodexThreadMutationResult,
  KhalaCodeDesktopFleetRunProjection,
  KhalaCodeDesktopFleetStatus,
  KhalaCodeDesktopKhalaSyncFleetStateResult,
} from "../src/shared/rpc"
import { sessionCatalogEntryToThreadSummary } from "../src/shared/session-catalog"
import { mountCodexThreadSidebar } from "../src/ui/codex-thread-sidebar"
import { mountFleetPanel } from "../src/ui/fleet-status"
import { khalaSyncFleetIndicator } from "../src/ui/fleet-sync-projection"
import { bindRecentThreadHotkeyHints } from "../src/ui/recent-thread-hotkey-hints"
import { KHALA_CODE_HOTBAR_SLOTS, mountKhalaCodeSidebar } from "../src/ui/sidebar"
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
  const previousWindow = globalThis.window
  const previousDocument = globalThis.document
  const previousNavigator = globalThis.navigator
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: window,
    writable: true,
  })
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
    Object.defineProperty(globalThis, "window", {
      configurable: true,
      value: previousWindow,
      writable: true,
    })
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

// Oracle for khala_code.chat.codex_stored_session_records_not_resumed.v1
describe("contract khala_code.chat.codex_stored_session_records_not_resumed.v1", () => {
  test("store-only Codex catalog rows become non-resumable local records", () => {
    const summary = sessionCatalogEntryToThreadSummary({
      catalogEntryId: "codex:thread-history",
      harnessKind: "codex",
      sessionRef: "thread-history",
      threadRef: "thread-history",
      desktopSessionRef: "desktop-session",
      lastTurnRef: null,
      title: "Codex session",
      preview: "",
      cwd: null,
      projectLabel: "Codex",
      status: "ready",
      statusLabel: "Codex session",
      source: "codex_session_store",
      createdAt: null,
      updatedAt: Date.parse("2026-07-03T12:00:00.000Z") / 1000,
      recencyAt: Date.parse("2026-07-03T12:00:00.000Z") / 1000,
    })

    expect(summary).toMatchObject({
      id: "codex:thread-history",
      resumable: false,
      statusLabel: "stored local record",
      title: "Stored Codex session",
      unavailableReason:
        "Stored local Codex session metadata does not include a current app-server UUID thread id.",
    })
  })

  test("stored-only Codex rows stay visible but cannot be selected as chats", async () => {
    await withDom(async () => {
      const container = document.createElement("aside")
      document.body.append(container)
      const data: KhalaCodeDesktopCodexThreadListResult = {
        ok: true,
        data: [],
        groups: [{ key: "all", label: "All sessions", threadIds: ["codex:legacy", "live-thread"] }],
        threads: [
          {
            ...thread("codex:legacy", "Stored Codex session", 30),
            resumable: false,
            statusLabel: "stored local record",
            unavailableReason:
              "Stored local Codex session metadata does not include a current app-server UUID thread id.",
          },
          thread("live-thread", "Live Codex thread", 20),
        ],
      }
      const resumedThreadIds: string[] = []
      const sidebar = mountCodexThreadSidebar(container, {
        activeThreadId: () => null,
        archiveThread: async threadId => ({ action: "archive", ok: true, threadId }),
        deleteThread: async threadId => ({ action: "delete", ok: true, threadId }),
        forkThread: async threadId => ({ action: "fork", ok: true, threadId }),
        listThreads: async () => data,
        renameThread: async threadId => ({ action: "rename", ok: true, threadId }),
        resumeThread: async threadId => {
          resumedThreadIds.push(threadId)
          return {
            ok: true as const,
            thread: {},
            threadId,
            messages: [] as const,
          }
        },
        sessionId: "desktop-session",
        unarchiveThread: async threadId => ({ action: "unarchive", ok: true, threadId }),
        onNewThreadRequested: () => undefined,
        onThreadSelected: () => undefined,
      })
      sidebar.setVisible(true)
      await sidebar.refresh()

      const storedRow = container.querySelector<HTMLButtonElement>(
        '[data-thread-id="codex:legacy"] .khala-thread-sidebar-item-row',
      )
      expect(storedRow?.disabled).toBe(true)
      expect(storedRow?.title).toContain("Stored local Codex session metadata")
      expect(container.textContent).toContain("Stored Codex session")
      expect(container.textContent).not.toContain("invalid session id")

      await expect(sidebar.selectRecentThread(0)).resolves.toBe(true)
      expect(resumedThreadIds).toEqual(["live-thread"])
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

// Oracle for khala_code.composer.no_dead_controls.v1
// Oracle for khala_code.composer.structure_not_bloat.v1
describe("contract khala_code.composer.no_dead_controls.v1 / khala_code.composer.structure_not_bloat.v1", () => {
  test("the composer has no dead Plan toggle and no commented-out chrome mounted", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()
    const executableMain = main
      .replace(/\/\*[\s\S]*?\*\//gu, "")
      .replace(/^\s*\/\/.*$/gmu, "")
    const executableCss = css.replace(/\/\*[\s\S]*?\*\//gu, "")

    expect(main).toContain("const renderReasoningModeSelect = ()")
    expect(main).toContain("writeComposerReasoningMode(select.value)")
    expect(executableMain).not.toContain('label.textContent = "Plan"')
    expect(css).not.toContain(".khala-architect-plan-toggle")

    expect(executableMain).not.toContain("renderHarnessPill()")
    expect(executableMain).not.toContain("renderRuntimeBadge()")
    expect(executableMain).not.toContain("renderMicrophoneIndicator()")
    expect(executableCss).not.toContain(".khala-harness-pill")
    expect(executableCss).not.toContain(".khala-runtime-badge")
    expect(executableCss).not.toContain(".khala-microphone-indicator")
  })
})

// Oracle for khala_code.composer.attach_control_icon_only.v1
describe("contract khala_code.composer.attach_control_icon_only.v1", () => {
  test("the attach label stays hidden regardless of viewport width, and follow-ups render compact", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()
    const desktopOverride = css.split(".khala-code-composer .oa-ai-command-composer-button-label {")[1]?.split("}")[0] ?? ""
    expect(desktopOverride).toContain("display: none")

    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    expect(main).toContain("const renderFollowUpDraft = (")
    expect(main).toContain("khala-code-composer-follow-up")
    const followUpRule = css.split(".khala-code-composer-follow-up {")[1]?.split("}")[0] ?? ""
    expect(followUpRule).toContain("min-height: 26px")
    const messageBubbleRule = css.split(".message-bubble {")[1]?.split("}")[0] ?? ""
    expect(followUpRule).not.toBe(messageBubbleRule)
  })
})

// Oracle for khala_code.chat.no_current_chat_text_flash.v1
describe("contract khala_code.chat.no_current_chat_text_flash.v1", () => {
  test("no 'Current chat' text renders anywhere in the sidebar source, even as a conditional string", async () => {
    const sidebar = await Bun.file(new URL("../src/ui/codex-thread-sidebar-react.tsx", import.meta.url)).text()
    expect(sidebar.toLowerCase()).not.toContain("current chat")
  })
})

// Oracle for khala_code.chat.harness_badge_removed.v1
describe("contract khala_code.chat.harness_badge_removed.v1", () => {
  test("sidebar rows never render a Codex/Claude harness badge", async () => {
    await withDom(async () => {
      const container = document.createElement("aside")
      document.body.append(container)
      const data: KhalaCodeDesktopCodexThreadListResult = {
        ok: true,
        data: [],
        groups: [{ key: "all", label: "All sessions", threadIds: ["thread-a", "thread-b"] }],
        threads: [
          { ...thread("thread-a", "First chat", 20), badges: ["Codex"], modelProvider: "openai" },
          { ...thread("thread-b", "Second chat", 10), badges: ["Claude"], modelProvider: "claude" },
        ],
      }
      const sidebar = mountCodexThreadSidebar(container, {
        activeThreadId: () => null,
        archiveThread: async threadId => ({ action: "archive", ok: true, threadId }),
        deleteThread: async threadId => ({ action: "delete", ok: true, threadId }),
        forkThread: async threadId => ({ action: "fork", ok: true, threadId }),
        listThreads: async () => data,
        renameThread: async threadId => ({ action: "rename", ok: true, threadId }),
        resumeThread: async threadId => ({ ok: true, thread: {}, threadId, messages: [] }),
        sessionId: "desktop-session",
        unarchiveThread: async threadId => ({ action: "unarchive", ok: true, threadId }),
        onNewThreadRequested: () => undefined,
        onThreadSelected: () => undefined,
      })
      sidebar.setVisible(true)
      await sidebar.refresh()

      expect(container.querySelectorAll(".khala-thread-sidebar-harness-badge")).toHaveLength(0)
      expect(container.textContent).not.toContain("Codex")
      expect(container.textContent).not.toContain("Claude")
    })
  })
})

// Oracle for khala_code.chat.sidebar_row_density.v1
describe("contract khala_code.chat.sidebar_row_density.v1", () => {
  test("sidebar rows are borderless with tightened vertical padding", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()
    const itemRule = css.split(".khala-thread-sidebar-item {")[1]?.split("}")[0] ?? ""
    expect(itemRule).toContain("border: 0")
    expect(itemRule).toContain("padding: 0.1rem 0.5rem")
    const activeRule = css.split('.khala-thread-sidebar-item[data-active="true"] {')[1]?.split("}")[0] ?? ""
    expect(activeRule).not.toMatch(/border(?!-radius):\s*(?!0\b)/u)
  })
})

// Oracle for khala_code.chat.starcraft_scrollbar_parity.v1
describe("contract khala_code.chat.starcraft_scrollbar_parity.v1", () => {
  test("the StarCraft scrollbar theme is global, not opt-in per container", async () => {
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()
    expect(css).toContain("*::-webkit-scrollbar {")
    expect(css).toContain("--oa-scrollbar-thumb-highlight")
    expect(css).not.toContain(".khala-code-thread-sidebar::-webkit-scrollbar")
    expect(css).not.toContain("#message-list::-webkit-scrollbar")
    expect(css).not.toContain("scrollbar-width: none")
  })
})

// Oracle for khala_code.chat.thread_open_never_raw_error.v1
describe("contract khala_code.chat.thread_open_never_raw_error.v1", () => {
  test("internal Codex RPC error text is detected and mapped to a friendly message", () => {
    expect(isKhalaCodeCodexThreadOpenInternalError("no rollout found for thread id abc")).toBe(true)
    expect(isKhalaCodeCodexThreadOpenInternalError("invalid session id: invalid character: expected ...")).toBe(true)
    expect(isKhalaCodeCodexThreadOpenInternalError("thread not found")).toBe(true)
    expect(isKhalaCodeCodexThreadOpenInternalError("Claude transcript unavailable")).toBe(false)

    const friendly = friendlyKhalaCodeCodexThreadOpenErrorMessage("no rollout found for thread id abc")
    expect(friendly).not.toContain("rollout")
    expect(friendlyKhalaCodeCodexThreadOpenErrorMessage("Claude transcript unavailable"))
      .toBe("Claude transcript unavailable")
  })

  test("a raw internal error thrown by resumeThread renders the friendly message in the sidebar", async () => {
    await withDom(async () => {
      const container = document.createElement("aside")
      document.body.append(container)
      const data: KhalaCodeDesktopCodexThreadListResult = {
        ok: true,
        data: [],
        groups: [{ key: "all", label: "All sessions", threadIds: ["thread-a"] }],
        threads: [thread("thread-a", "Broken chat", 10)],
      }
      const sidebar = mountCodexThreadSidebar(container, {
        activeThreadId: () => null,
        archiveThread: async threadId => ({ action: "archive", ok: true, threadId }),
        deleteThread: async threadId => ({ action: "delete", ok: true, threadId }),
        forkThread: async threadId => ({ action: "fork", ok: true, threadId }),
        listThreads: async () => data,
        renameThread: async threadId => ({ action: "rename", ok: true, threadId }),
        resumeThread: async () => {
          throw new Error("no rollout found for thread id thread-a")
        },
        sessionId: "desktop-session",
        unarchiveThread: async threadId => ({ action: "unarchive", ok: true, threadId }),
        onNewThreadRequested: () => undefined,
        onThreadSelected: () => undefined,
      })
      sidebar.setVisible(true)
      await sidebar.refresh()

      const row = container.querySelector<HTMLButtonElement>(
        '[data-thread-id="thread-a"] .khala-thread-sidebar-item-row',
      )
      row?.click()
      await new Promise(resolve => setTimeout(resolve, 0))

      expect(container.textContent).not.toContain("no rollout found")
      const errorNode = container.querySelector(".khala-thread-sidebar-row-error-text")
      expect(errorNode?.textContent).toBe(
        "This chat couldn't be opened. Its session may be missing or unavailable — try again or start a new chat.",
      )
    })
  })
})

// Oracle for khala_code.chat.streaming_indicator_survives_navigation.v1
// Oracle for khala_code.transcript.streaming_state_cross_surface_consistency.v1
describe(
  "contract khala_code.chat.streaming_indicator_survives_navigation.v1 / " +
    "khala_code.transcript.streaming_state_cross_surface_consistency.v1",
  () => {
    test("streaming state is tracked per-thread and survives navigation instead of a single global flag", async () => {
      const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()

      expect(main).toContain("const streamingThreadIds = new Map<string, string | null>()")
      expect(main).toContain("const isThreadStreaming = (threadId: string | null): boolean => {")
      expect(main).toContain("const recomputePendingTurnForActiveThread = (): void => {")
      expect(main).toContain("shellModel().pendingTurn = isThreadStreaming(shellModel().activeCodexThreadId)")

      // Composer status and the sidebar badge read the same per-thread source of truth.
      expect(main).toContain("if (shellModel().pendingTurn) return \"streaming\"")
      expect(main).toContain("isThreadStreaming,")

      // Populated at submit time, keyed by the submitting thread.
      expect(main).toContain("streamingThreadIds.set(turnId, submittedThreadId)")

      // Only removed when the owning turn genuinely finishes (finally / stop),
      // never as a blanket reset on thread switch.
      expect(main).toContain("streamingThreadIds.delete(turnId)")
      const switchFunctions = ["beginCodexThreadSwitch", "activateCodexThread", "beginNewCodexThread"]
      for (const fnName of switchFunctions) {
        const fnBody = main.split(`const ${fnName} = `)[1]?.split("\n}\n")[0] ?? ""
        expect(fnBody).not.toContain("streamingThreadIds.clear()")
        expect(fnBody).not.toContain("shellModel().pendingTurn = false")
        expect(fnBody).toContain("recomputePendingTurnForActiveThread()")
      }
    })
  },
)

// Oracle for khala_code.chat.new_thread_appears_promptly.v1
describe("contract khala_code.chat.new_thread_appears_promptly.v1", () => {
  test("an optimistic pending thread appears in the sidebar list immediately with its preview visible", async () => {
    await withDom(async () => {
      const container = document.createElement("aside")
      document.body.append(container)
      const data: KhalaCodeDesktopCodexThreadListResult = {
        ok: true,
        data: [],
        groups: [{ key: "all", label: "All sessions", threadIds: ["thread-a"] }],
        threads: [thread("thread-a", "Existing chat", 10)],
      }
      const sidebar = mountCodexThreadSidebar(container, {
        activeThreadId: () => null,
        archiveThread: async threadId => ({ action: "archive", ok: true, threadId }),
        deleteThread: async threadId => ({ action: "delete", ok: true, threadId }),
        forkThread: async threadId => ({ action: "fork", ok: true, threadId }),
        listThreads: async () => data,
        renameThread: async threadId => ({ action: "rename", ok: true, threadId }),
        resumeThread: async threadId => ({ ok: true, thread: {}, threadId, messages: [] }),
        sessionId: "desktop-session",
        unarchiveThread: async threadId => ({ action: "unarchive", ok: true, threadId }),
        onNewThreadRequested: () => undefined,
        onThreadSelected: () => undefined,
      })
      sidebar.setVisible(true)
      await sidebar.refresh()

      sidebar.upsertPendingThread({ preview: "Brand new request", threadId: "thread-new" })

      expect(container.querySelector('[data-thread-id="thread-new"]')).not.toBeNull()
      expect(container.textContent).toContain("Brand new request")
    })
  })
})

// Oracle for khala_code.chat.sync_remote_thread_appears_without_restart.v1
describe("contract khala_code.chat.sync_remote_thread_appears_without_restart.v1", () => {
  test("the renderer prefers connected chat sync rows and queues optimistic creates", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    const listThreadsBody =
      main.split("listThreads: async request => {")[1]?.split("renameThread: async")[0] ?? ""

    expect(listThreadsBody).toContain("controls.khalaSyncChatThreads")
    expect(listThreadsBody).toContain("khalaSyncChatCanDriveSidebar")
    expect(listThreadsBody).toContain("khalaSyncChatThreadIds.add(thread.threadId)")
    expect(listThreadsBody).toContain("chat.threads.map(chatThreadToSidebarSummary)")
    expect(listThreadsBody).toContain("khala-sync-chat")
    expect(listThreadsBody).toContain("cachedSessionCatalog")
    expect(listThreadsBody.indexOf("controls.khalaSyncChatThreads")).toBeLessThan(
      listThreadsBody.indexOf("cachedSessionCatalog"),
    )

    expect(main).toContain("const khalaSyncThreadCreateRequests = new Set<string>()")
    expect(main).toContain("controls.khalaSyncChatCreateThread")
    expect(main).toContain("controls.khalaSyncChatMessages")
    expect(main).toContain("controls.khalaSyncChatAppendMessage")
    expect(main).toContain("enqueueKhalaSyncChatThreadCreate({")
    expect(main).toContain("khalaSyncThreadResult(threadId)")
    expect(main).toContain("submitKhalaSyncChatMessage(submittedThreadId, message)")
    expect(main).toContain("khalaSyncChatRenameThread")
    expect(main).toContain("source: \"khala_sync_chat_thread\"")
  })
})

// Oracle for khala_code.chat.rename_applies_immediately.v1
describe("contract khala_code.chat.rename_applies_immediately.v1", () => {
  test("confirming a rename updates the visible title before the network call resolves", async () => {
    await withDom(async window => {
      // The inline rename form focuses itself via requestAnimationFrame,
      // which the real desktop webview provides but bun test does not.
      const previousRaf = globalThis.requestAnimationFrame
      globalThis.requestAnimationFrame = ((callback: FrameRequestCallback) => {
        callback(0)
        return 0
      }) as typeof requestAnimationFrame
      try {
      const container = document.createElement("aside")
      document.body.append(container)
      const data: KhalaCodeDesktopCodexThreadListResult = {
        ok: true,
        data: [],
        groups: [{ key: "all", label: "All sessions", threadIds: ["thread-a"] }],
        threads: [thread("thread-a", "Old title", 10)],
      }
      const renameResolver: { resolve: ((value: KhalaCodeDesktopCodexThreadMutationResult) => void) | null } = { resolve: null }
      const sidebar = mountCodexThreadSidebar(container, {
        activeThreadId: () => null,
        archiveThread: async threadId => ({ action: "archive", ok: true, threadId }),
        deleteThread: async threadId => ({ action: "delete", ok: true, threadId }),
        forkThread: async threadId => ({ action: "fork", ok: true, threadId }),
        listThreads: async () => data,
        renameThread: () =>
          new Promise<KhalaCodeDesktopCodexThreadMutationResult>(resolve => {
            renameResolver.resolve = resolve
          }),
        resumeThread: async threadId => ({ ok: true, thread: {}, threadId, messages: [] }),
        sessionId: "desktop-session",
        unarchiveThread: async threadId => ({ action: "unarchive", ok: true, threadId }),
        onNewThreadRequested: () => undefined,
        onThreadSelected: () => undefined,
      })
      sidebar.setVisible(true)
      await sidebar.refresh()

      const row = container.querySelector<HTMLElement>('[data-thread-id="thread-a"] .khala-thread-sidebar-item-row')
      row?.dispatchEvent(new window.MouseEvent("contextmenu", { bubbles: true }) as unknown as Event)
      const renameItem = document.querySelector<HTMLButtonElement>('[data-menu-item="rename-thread"]')
      expect(renameItem).not.toBeNull()
      renameItem?.click()

      const input = container.querySelector<HTMLInputElement>(".khala-thread-sidebar-rename-input")
      expect(input).not.toBeNull()
      if (input !== null) input.value = "New title"
      const form = container.querySelector<HTMLFormElement>(".khala-thread-sidebar-rename-form")
      form?.dispatchEvent(new window.Event("submit", { bubbles: true, cancelable: true }) as unknown as Event)

      // Renamed immediately, before the mocked network call resolves.
      expect(container.textContent).toContain("New title")
      expect(container.textContent).not.toContain("Old title")
      renameResolver.resolve?.({ action: "rename", ok: true, threadId: "thread-a" })
      await new Promise(resolve => setTimeout(resolve, 0))
      } finally {
        globalThis.requestAnimationFrame = previousRaf
      }
    })
  })
})

// Oracle for khala_code.chat.rehydrate_shows_tool_calls.v1
// Oracle for khala_code.transcript.tool_call_path_display.v1
describe(
  "contract khala_code.chat.rehydrate_shows_tool_calls.v1 / khala_code.transcript.tool_call_path_display.v1",
  () => {
    test("thread history projection labels tool cards with relative paths and preserves codexItem data", async () => {
      const projectorTest = await Bun.file(
        new URL("../tests/codex-thread-item-projector.test.ts", import.meta.url),
      ).text()
      expect(projectorTest).toContain("labels tool cards with relative paths instead of absolute worktree paths")
      expect(projectorTest).toContain("projects every supported ThreadItem variant into stable transcript cards")

      const projector = await Bun.file(
        new URL("../src/bun/codex-thread-item-projector.ts", import.meta.url),
      ).text()
      expect(projector).toContain('`Read ${displayPath(path, context)}`')

      const chatRuntime = await Bun.file(
        new URL("../src/bun/codex-app-server-chat-runtime.ts", import.meta.url),
      ).text()
      expect(chatRuntime).toContain("messagesFromThread")
    })
  },
)

// Oracle for khala_code.transcript.consecutive_tool_calls_collapsed.v1
describe("contract khala_code.transcript.consecutive_tool_calls_collapsed.v1", () => {
  test("consecutive tool-call messages group into one collapsible summary line", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    expect(main).toContain("const groupConsecutiveToolCallMessages = (")
    expect(main).toContain("const renderToolCallGroupSummary = (")
    expect(main).toContain("const renderTranscriptMessages = (")
    expect(main).toContain("...renderTranscriptMessages(shellModel().messages)")
    expect(main).toContain("summary.addEventListener(\"click\", () => {")
    expect(main).toContain("items.hidden = expanded")

    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()
    expect(css).toContain(".tool-call-group-summary {")
    expect(css).toContain(".tool-call-group-items {")
  })
})

// Oracle for khala_code.nav.hotbar_no_route_text.v1
// Oracle for khala_code.nav.hotbar_hotkey_always_visible.v1
describe(
  "contract khala_code.nav.hotbar_no_route_text.v1 / khala_code.nav.hotbar_hotkey_always_visible.v1",
  () => {
    test("hotbar buttons show a static label and their own visible hotkey badge, never route text", () => {
      const window = new Window()
      const previousWindow = globalThis.window
      const previousDocument = globalThis.document
      const previousNavigator = globalThis.navigator
      Object.defineProperty(globalThis, "window", { configurable: true, value: window, writable: true })
      Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
      Object.defineProperty(globalThis, "navigator", { configurable: true, value: window.navigator })
      try {
        const container = document.createElement("nav")
        document.body.append(container)
        const handle = mountKhalaCodeSidebar(container, { selectedValue: "chat", onActivate: () => undefined })

        const buttons = [...container.querySelectorAll<HTMLElement>(".khala-code-hotbar-slot")]
        expect(buttons).toHaveLength(KHALA_CODE_HOTBAR_SLOTS.length)

        for (const slot of KHALA_CODE_HOTBAR_SLOTS) {
          const button = container.querySelector<HTMLElement>(`.khala-code-hotbar-slot-${slot.slot}`)
          const label = button?.querySelector(".khala-code-hotbar-label")
          const key = button?.querySelector(".khala-code-hotbar-key")
          expect(label?.textContent).toBe(slot.label)
          expect(label?.textContent).not.toMatch(/[./]/u)
          expect(key?.textContent?.length ?? 0).toBeGreaterThan(0)
          expect(key?.textContent).toContain(slot.hotkey)
        }
        handle.destroy()
      } finally {
        Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow, writable: true })
        Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
        Object.defineProperty(globalThis, "navigator", { configurable: true, value: previousNavigator })
        window.close()
      }
    })
  },
)

// Oracle for khala_code.nav.hotbar_no_stray_special_characters.v1
describe("contract khala_code.nav.hotbar_no_stray_special_characters.v1", () => {
  test("references the existing hotbar Option-digit interception regression test", async () => {
    const sidebarTest = await Bun.file(new URL("../tests/sidebar.test.ts", import.meta.url)).text()
    expect(sidebarTest).toContain("code: \"Digit2\"")
    expect(sidebarTest).toContain("defaultPrevented")
  })
})

// Oracle for khala_code.menus.flyout_single_line_no_preamble.v1
describe("contract khala_code.menus.flyout_single_line_no_preamble.v1", () => {
  test("the thread flyout menu has no header and no per-item subheadline", async () => {
    await withDom(async window => {
      const container = document.createElement("aside")
      document.body.append(container)
      const data: KhalaCodeDesktopCodexThreadListResult = {
        ok: true,
        data: [],
        groups: [{ key: "all", label: "All sessions", threadIds: ["thread-a"] }],
        threads: [thread("thread-a", "Menu target", 10)],
      }
      const sidebar = mountCodexThreadSidebar(container, {
        activeThreadId: () => null,
        archiveThread: async threadId => ({ action: "archive", ok: true, threadId }),
        deleteThread: async threadId => ({ action: "delete", ok: true, threadId }),
        forkThread: async threadId => ({ action: "fork", ok: true, threadId }),
        listThreads: async () => data,
        renameThread: async threadId => ({ action: "rename", ok: true, threadId }),
        resumeThread: async threadId => ({ ok: true, thread: {}, threadId, messages: [] }),
        sessionId: "desktop-session",
        unarchiveThread: async threadId => ({ action: "unarchive", ok: true, threadId }),
        onNewThreadRequested: () => undefined,
        onThreadSelected: () => undefined,
      })
      sidebar.setVisible(true)
      await sidebar.refresh()

      const row = container.querySelector<HTMLElement>('[data-thread-id="thread-a"] .khala-thread-sidebar-item-row')
      row?.dispatchEvent(new window.MouseEvent("contextmenu", { bubbles: true }) as unknown as Event)

      expect(document.querySelector(".oa-ui-menu-dom-header")).toBeNull()
      expect(document.querySelectorAll(".oa-ui-menu-dom-item-description")).toHaveLength(0)
      const items = [...document.querySelectorAll(".oa-ui-menu-dom-item")]
      expect(items.length).toBeGreaterThan(0)
    })
  })
})

// Oracle for khala_code.fleet.menu_no_stray_labels.v1
describe("contract khala_code.fleet.menu_no_stray_labels.v1", () => {
  test("the fleet panel source never renders a literal 'ACCT' label", async () => {
    const fleetStatus = await Bun.file(new URL("../src/ui/fleet-status.ts", import.meta.url)).text()
    expect(fleetStatus).not.toContain("ACCT")
  })
})

// Oracle for khala_code.app.resumes_after_restart.v1
describe("contract khala_code.app.resumes_after_restart.v1", () => {
  test("the last active thread is captured at boot and restored after the initial render", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    expect(main).toContain("const bootRestoreThreadId = localStorage.getItem(activeThreadIdStorageKey)")
    expect(main).not.toContain("localStorage.removeItem(activeThreadIdStorageKey)\n\ntype ThreadSwitchPerformanceSample")
    expect(main).toContain("const restoreActiveThreadAfterRestart = async (): Promise<void> => {")
    expect(main).toContain("void restoreActiveThreadAfterRestart()")
    // Fails soft on a missing/corrupt thread instead of retrying forever.
    const restoreBody = main.split("const restoreActiveThreadAfterRestart = async (): Promise<void> => {")[1]?.split("\n}\n")[0] ?? ""
    expect(restoreBody).toContain("localStorage.removeItem(activeThreadIdStorageKey)")
  })
})

// Oracle for khala_code.app.no_unrequested_first_launch_scripts.v1
describe("contract khala_code.app.no_unrequested_first_launch_scripts.v1", () => {
  test("references the existing Apple FM bridge disabled-on-launch regression tests", async () => {
    const appShellTest = await Bun.file(new URL("../tests/app-shell.test.ts", import.meta.url)).text()
    expect(appShellTest).toContain("keeps the Apple FM bridge disabled in launch startup")
    const packagingTest = await Bun.file(new URL("../tests/apple-fm-packaging.test.ts", import.meta.url)).text()
    expect(packagingTest.length).toBeGreaterThan(0)
    const packageJson = await Bun.file(new URL("../package.json", import.meta.url)).text()
    expect(packageJson).not.toContain("prepare:apple-fm-bridge")
  })
})

// Oracle for khala_code.tokens.per_thread_live_counter.v1
describe("contract khala_code.tokens.per_thread_live_counter.v1", () => {
  test("the per-thread token counter is mounted top-right, polls live, and its click opens sync detail", async () => {
    const main = await Bun.file(new URL("../src/ui/main.ts", import.meta.url)).text()
    expect(main).toContain("const renderThreadTokenCounter = (")
    expect(main).toContain("threadTokenCounter.addEventListener(\"click\", ")
    expect(main).toContain("leaderboardSyncedTokens")
    expect(main).toContain("pendingSyncTokens")
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()
    const meterRule = css.split(".khala-thread-token-meter {")[1]?.split("}")[0] ?? ""
    expect(meterRule).toContain("top: 12px")
    expect(meterRule).toContain("right: 18px")
  })
})

// Oracle for khala_code.terminal.tui_mode_available.v1
describe("contract khala_code.terminal.tui_mode_available.v1", () => {
  test("the TUI reuses the exact desktop harness and gates honestly on availability", async () => {
    const tui = await Bun.file(new URL("../scripts/khala-code-tui.ts", import.meta.url)).text()
    expect(tui).toContain("createCodexAppServerChatRuntime")
    expect(tui).toContain("createCodexAppServerHost")
    expect(tui).toContain("inspectCodexHarnessStatus")
    expect(tui).toContain("/new")
    expect(tui).toContain("/status")
    expect(tui).toContain("/exit")
  })
})

// ---------------------------------------------------------------------------
// KS-6.2 (#8303): Khala Sync fleet cockpit contracts.
// ---------------------------------------------------------------------------

const khalaSyncFleetStateFixture = (
  input: Partial<KhalaCodeDesktopKhalaSyncFleetStateResult> = {},
): KhalaCodeDesktopKhalaSyncFleetStateResult => ({
  accounts: [],
  assignments: [],
  authState: "connected",
  cursor: 4,
  enabled: true,
  ok: true,
  pendingMutations: 0,
  phase: "live",
  reason: null,
  rejections: [],
  run: {
    counters: {
      activeAssignments: 2,
      blockedAssignments: 0,
      completedAssignments: 3,
      failedAssignments: 0,
      workUnitsTotal: 9,
    },
    desiredSlots: 4,
    runId: "fleet.run.contract.test",
    startedAt: "2026-07-04T00:00:01.000Z",
    status: "running",
    updatedAt: "2026-07-04T00:05:00.000Z",
    workerKind: "codex",
  },
  workers: [],
  ...input,
})

const khalaSyncFleetLocalRun = (): KhalaCodeDesktopFleetRunProjection => ({
  counters: {
    activeAssignments: 1,
    blockedAssignments: 0,
    completedAssignments: 1,
    failedAssignments: 0,
    workUnitsTotal: 9,
  },
  createdAt: "2026-07-04T00:00:00.000Z",
  dispatchKind: "supervised_dispatch",
  objectiveProjected: false,
  pylonRef: null,
  refillPolicy: {
    cooldownAware: true,
    maxPerAccount: 1,
    stopCondition: "backlog_empty",
  },
  runRef: "fleet.run.contract.test",
  startedAt: "2026-07-04T00:00:01.000Z",
  state: "running",
  targetConcurrency: 2,
  updatedAt: "2026-07-04T00:00:30.000Z",
  workerKind: "codex",
  workSource: { kind: "fixture" },
})

const khalaSyncFleetStatusFixture = (): KhalaCodeDesktopFleetStatus => ({
  ok: true,
  observedAt: "2026-07-04T00:05:00.000Z",
  pylon: { message: "online", pylonRef: "pylon.public.contract", status: "online" },
  availableCodexAssignments: 1,
  maxCodexAssignments: 2,
  tokenRate: {
    activeAdjustedTokensPerMinute: null,
    completedStatus: "not_measured",
    completedTokenRows: null,
    completedTokensPerMinute: null,
    tokensWindow: null,
    inFlightTokens: null,
    inFlightTokensPerMinute: null,
    source: "unavailable",
    unavailableReason: null,
  },
  accounts: [],
  activeAssignments: [],
  processes: [],
})

const withFleetDom = async (
  run: (window: Window) => Promise<void> | void,
): Promise<void> => {
  const window = new Window()
  const previousDocument = globalThis.document
  const previousWindow = globalThis.window
  const previousMatchMedia = globalThis.matchMedia
  Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
  Object.defineProperty(globalThis, "window", { configurable: true, value: window, writable: true })
  Object.defineProperty(globalThis, "matchMedia", {
    configurable: true,
    value: () => ({ matches: false }),
  })
  try {
    await run(window)
  } finally {
    Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
    Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow, writable: true })
    Object.defineProperty(globalThis, "matchMedia", { configurable: true, value: previousMatchMedia })
    window.close()
  }
}

const mountKhalaSyncFleetPanel = (
  state: () => Promise<KhalaCodeDesktopKhalaSyncFleetStateResult>,
) => {
  const container = document.createElement("div")
  document.body.append(container)
  const panel = mountFleetPanel(container, {
    connectAccount: async accountRef => ({
      ok: true,
      accountRef,
      output: "",
      userCode: null,
      verificationUrl: null,
    }),
    delegateRun: async () => {
      throw new Error("delegate runner should not be called")
    },
    fetch: async () => khalaSyncFleetStatusFixture(),
    fleetRunControl: async request => ({
      ok: true,
      previousState: "running",
      run: khalaSyncFleetLocalRun(),
      supervisorActive: true,
      verb: request.verb,
    }),
    fleetRunList: async () => ({ ok: true, runs: [khalaSyncFleetLocalRun()] }),
    fleetRunStart: async () => {
      throw new Error("fleet run start should not be called")
    },
    fleetWorkerControl: async () => {
      throw new Error("fleet worker control should not be called")
    },
    khalaSyncFleetState: async () => state(),
    khalaSyncFleetMutate: async () => ({ ok: true }),
    loadGymDemoProof: () => {
      throw new Error("gym proof should not be called")
    },
    openExternal: async () => false,
    removeAccount: async () => ({ ok: true }),
    setAccountPaused: async () => ({ ok: true }),
    consumeResetCredit: async () => ({ ok: true }),
    startDelegationOptimization: async () => {
      throw new Error("optimization should not be called")
    },
  })
  return { container, panel }
}

// Oracle khala_sync_indicator_truthful.dom for contract
// khala_code.fleet.khala_sync_indicator_truthful.v1
describe("contract khala_code.fleet.khala_sync_indicator_truthful.v1", () => {
  test("the indicator claims Live only while the sync session phase is live", async () => {
    await withFleetDom(async () => {
      const { container, panel } = mountKhalaSyncFleetPanel(async () =>
        khalaSyncFleetStateFixture({ phase: "live" }),
      )
      await panel.refresh()
      const chip = container.querySelector<HTMLElement>(".khala-fleet-sync-indicator")
      expect(chip).not.toBeNull()
      expect(chip!.dataset.khalaSyncLive).toBe("true")
      expect(chip!.textContent).toBe("Khala Sync: Live")
    })
  })

  test("non-live phases render explicit syncing/reconnecting states, never fake freshness", async () => {
    const cases: ReadonlyArray<{
      phase: KhalaCodeDesktopKhalaSyncFleetStateResult["phase"]
      expected: string
    }> = [
      { phase: "bootstrapping", expected: "Khala Sync: Bootstrapping…" },
      { phase: "catching_up", expected: "Khala Sync: Catching up…" },
      { phase: "idle", expected: "Khala Sync: Reconnecting…" },
    ]
    for (const item of cases) {
      await withFleetDom(async () => {
        const { container, panel } = mountKhalaSyncFleetPanel(async () =>
          khalaSyncFleetStateFixture({ phase: item.phase, cursor: null }),
        )
        await panel.refresh()
        const chip = container.querySelector<HTMLElement>(".khala-fleet-sync-indicator")
        expect(chip).not.toBeNull()
        expect(chip!.dataset.khalaSyncLive).toBe("false")
        expect(chip!.textContent).toBe(item.expected)
        expect(container.textContent).not.toContain("Khala Sync: Live")
      })
    }
  })

  test("pure indicator mapping is truthful for every phase", () => {
    expect(khalaSyncFleetIndicator(khalaSyncFleetStateFixture({ phase: "live" })).live).toBe(true)
    for (const phase of ["bootstrapping", "catching_up", "must_refetch", "idle"] as const) {
      const indicator = khalaSyncFleetIndicator(
        khalaSyncFleetStateFixture({ phase, reason: phase === "must_refetch" ? "scope_reset" : null }),
      )
      expect(indicator.live).toBe(false)
      expect(indicator.label).not.toContain("Live")
    }
  })
})

// Oracle khala_sync_must_refetch_visible.dom for contract
// khala_code.fleet.khala_sync_must_refetch_recovers.v1
describe("contract khala_code.fleet.khala_sync_must_refetch_recovers.v1", () => {
  test("must_refetch keeps the Fleet screen populated with a visible resync state", async () => {
    await withFleetDom(async () => {
      const { container, panel } = mountKhalaSyncFleetPanel(async () =>
        khalaSyncFleetStateFixture({
          phase: "must_refetch",
          cursor: null,
          reason: "scope_reset",
        }),
      )
      await panel.refresh()
      const chip = container.querySelector<HTMLElement>(".khala-fleet-sync-indicator")
      expect(chip).not.toBeNull()
      expect(chip!.dataset.khalaSyncPhase).toBe("must_refetch")
      expect(chip!.dataset.khalaSyncLive).toBe("false")
      expect(chip!.textContent).toBe("Khala Sync: Resyncing (scope_reset)…")
      // Not stranded: the active-run header still renders run state (the
      // synced entities + polling fallback stay visible during re-bootstrap).
      expect(container.textContent).toContain("Active FleetRun")
      expect(container.textContent).toContain("fleet.run.contract.test")
    })
  })
})
