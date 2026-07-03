import type {
  KhalaCodeDesktopCodexThreadListResult,
  KhalaCodeDesktopCodexThreadMutationResult,
  KhalaCodeDesktopCodexThreadResult,
  KhalaCodeDesktopMessage,
} from "../shared/rpc"
import {
  friendlyKhalaCodeCodexThreadOpenErrorMessage,
  type KhalaCodeDesktopCodexThreadSummary,
} from "../shared/codex-threads"
import { iconElement } from "@openagentsinc/ui/icon-dom"
import type { IconName } from "@openagentsinc/ui/icon"
import {
  createBasecoatContextMenu,
  type BasecoatMenuDomContent,
  type BasecoatMenuDomPoint,
} from "@openagentsinc/ui/menu-dom"
import { formatCompactThreadTimestamp } from "./thread-time"
import {
  type RecentThreadCycleDirection,
  recentThreadCycleIndex,
  recentThreadHotkeyHintDigits,
  recentThreadsForHotkeys,
} from "./thread-hotkeys"

export type CodexThreadSidebarHandle = {
  readonly recentThreads: () => readonly KhalaCodeDesktopCodexThreadSummary[]
  readonly refresh: () => Promise<void>
  readonly selectAdjacentRecentThread: (direction: RecentThreadCycleDirection) => Promise<boolean>
  readonly selectRecentThread: (index: number) => Promise<boolean>
  readonly setActiveThreadId: (threadId: string | null) => void
  readonly setHotkeyHintsVisible: (visible: boolean) => void
  readonly setVisible: (visible: boolean) => void
  readonly upsertPendingThread: (input: {
    readonly preview: string
    readonly threadId: string
  }) => void
}

export type CodexThreadSelectionSource = "hotkey" | "sidebar"

export type CodexThreadSidebarOptions = {
  readonly activeThreadId: () => string | null
  readonly archiveThread: (threadId: string) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  readonly deleteThread: (threadId: string) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  readonly forkThread: (threadId: string) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  readonly isThreadStreaming?: (threadId: string) => boolean
  readonly listThreads: (input: {
    readonly archived: boolean
    readonly includeHomeSessions: boolean
    readonly searchTerm: string
  }) => Promise<KhalaCodeDesktopCodexThreadListResult>
  readonly renameThread: (threadId: string, name: string) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  readonly resumeThread: (threadId: string) => Promise<KhalaCodeDesktopCodexThreadResult>
  readonly sessionId: string
  readonly unarchiveThread: (threadId: string) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  readonly onNewThreadRequested: () => void
  readonly onThreadSelectionStarted?: (
    input: {
      readonly selectionId: number
      readonly source: CodexThreadSelectionSource
      readonly threadId: string
    },
  ) => void
  readonly onThreadSelected: (
    input: {
      readonly messages: readonly KhalaCodeDesktopMessage[]
      readonly requestedThreadId?: string
      readonly selectionId?: number
      readonly source?: CodexThreadSelectionSource
      readonly threadId: string
    },
  ) => void
  readonly onThreadSelectionFailed?: (
    input: {
      readonly message: string
      readonly selectionId: number
      readonly threadId: string
    },
  ) => void
}

type ViewState =
  | { readonly phase: "idle" }
  | { readonly data?: KhalaCodeDesktopCodexThreadListResult; readonly phase: "loading" }
  | { readonly data?: KhalaCodeDesktopCodexThreadListResult; readonly message: string; readonly phase: "error" }
  | { readonly phase: "ready"; readonly data: KhalaCodeDesktopCodexThreadListResult }

const el = <Tag extends keyof HTMLElementTagNameMap>(
  tag: Tag,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[Tag] => {
  const node = document.createElement(tag)
  if (className !== undefined) node.className = className
  if (text !== undefined) node.textContent = text
  return node
}

const srOnly = (text: string): HTMLSpanElement => {
  const node = el("span", "khala-code-sr-only", text)
  return node
}

const sidebarIcon = (icon: IconName, label: string): HTMLSpanElement =>
  iconElement(icon, {
    className: "khala-thread-sidebar-icon",
    dataIcon: label,
  })

const isThreadStreaming = (
  threadId: string,
  options: Pick<CodexThreadSidebarOptions, "isThreadStreaming">,
): boolean =>
  options.isThreadStreaming?.(threadId) === true

const threadStreamingIndicator = (): HTMLSpanElement => {
  const indicator = el("span", "khala-thread-sidebar-item-spinner")
  indicator.setAttribute("aria-hidden", "true")
  return indicator
}

const threadHotkeyHintContent = (digit: number): HTMLSpanElement => {
  const time = el("span", "khala-thread-sidebar-item-time", `⌘${digit}`)
  time.dataset.hotkeyHint = String(digit)
  time.title = `Jump with ⌘${digit}`
  time.setAttribute("aria-label", `Jump with Command ${digit}`)
  return time
}

// Empty by design: the optimistic active row is indicated by its background only.
const pendingActiveThreadGroupLabel = ""

const threadTimeContent = (
  thread: KhalaCodeDesktopCodexThreadSummary,
  options: Pick<CodexThreadSidebarOptions, "isThreadStreaming">,
): HTMLSpanElement => {
  const time = el("span", "khala-thread-sidebar-item-time")
  if (isThreadStreaming(thread.id, options)) {
    time.dataset.streaming = "true"
    time.title = "Streaming response"
    time.setAttribute("aria-label", "Streaming response")
    time.replaceChildren(threadStreamingIndicator(), srOnly("Streaming response"))
    return time
  }

  time.textContent =
    formatCompactThreadTimestamp(thread.recencyAt ?? thread.updatedAt) || thread.statusLabel
  return time
}

const groupThreads = (
  data: KhalaCodeDesktopCodexThreadListResult,
): readonly {
  readonly label: string
  readonly threads: readonly KhalaCodeDesktopCodexThreadSummary[]
}[] => {
  const threads = data.threads ?? []
  const threadById = new Map(threads.map(thread => [thread.id, thread]))
  const groups = data.groups ?? []
  if (groups.length === 0) return [{ label: "Threads", threads }]
  return groups.map(group => ({
    label: group.label,
    threads: group.threadIds
      .map(id => threadById.get(id))
      .filter((thread): thread is KhalaCodeDesktopCodexThreadSummary => thread !== undefined),
  }))
}

const messagesForResult = (
  result: KhalaCodeDesktopCodexThreadResult | KhalaCodeDesktopCodexThreadMutationResult,
): readonly KhalaCodeDesktopMessage[] =>
  result.messages ?? []

const dataForState = (
  state: ViewState,
): KhalaCodeDesktopCodexThreadListResult | undefined =>
  state.phase === "ready" || state.phase === "loading" || state.phase === "error"
    ? state.data
    : undefined

export const renameThreadInListData = (
  data: KhalaCodeDesktopCodexThreadListResult,
  threadId: string,
  title: string,
): KhalaCodeDesktopCodexThreadListResult => {
  if (data.threads === undefined) return data
  let renamed = false
  const threads = data.threads.map(thread => {
    if (thread.id !== threadId || thread.title === title) return thread
    renamed = true
    return { ...thread, title }
  })
  return renamed ? { ...data, threads } : data
}

export const upsertPendingThreadInListData = (
  data: KhalaCodeDesktopCodexThreadListResult,
  thread: KhalaCodeDesktopCodexThreadSummary,
): KhalaCodeDesktopCodexThreadListResult => {
  const existingIds = new Set(data.threads?.map(candidate => candidate.id) ?? [])
  if (existingIds.has(thread.id)) return data

  const key = thread.cwd ?? "cwd:none"
  const groups = [...(data.groups ?? [])]
  const existingGroup = groups.find(group => group.key === key)
  if (existingGroup === undefined) {
    groups.unshift({
      key,
      label: thread.projectLabel,
      threadIds: [thread.id],
    })
  } else if (!existingGroup.threadIds.includes(thread.id)) {
    const index = groups.indexOf(existingGroup)
    groups[index] = {
      ...existingGroup,
      threadIds: [thread.id, ...existingGroup.threadIds],
    }
  }

  return {
    ...data,
    groups,
    threads: [thread, ...(data.threads ?? [])],
  }
}

export const mountCodexThreadSidebar = (
  container: HTMLElement,
  options: CodexThreadSidebarOptions,
): CodexThreadSidebarHandle => {
  let searchOpen = false
  let searchShouldFocus = false
  let searchTerm = ""
  let includeHomeSessions = false
  let state: ViewState = { phase: "idle" }
  let visible = false
  let hotkeyHintsVisible = false
  let hotkeyHintDigits: ReadonlyMap<string, number> = new Map()
  let activeThreadId = options.activeThreadId()
  let selectingThreadId: string | null = null
  let selectionError: {
    readonly message: string
    readonly threadId: string
  } | null = null
  let renamingThreadId: string | null = null
  let renamingThreadDraft = ""
  let refreshSequence = 0
  let selectionSequence = 0
  const optimisticThreadTitles = new Map<string, string>()
  const optimisticThreads = new Map<string, KhalaCodeDesktopCodexThreadSummary>()
  const threadMenu = createBasecoatContextMenu({
    id: "khala-thread-sidebar-thread-menu",
    ownerDocument: container.ownerDocument,
    className: "khala-thread-sidebar-menu",
  })

  const applyOptimisticThreads = (
    data: KhalaCodeDesktopCodexThreadListResult,
  ): KhalaCodeDesktopCodexThreadListResult => {
    let nextData = data
    for (const [threadId, title] of optimisticThreadTitles) {
      const thread = nextData.threads?.find(candidate => candidate.id === threadId)
      if (thread?.title === title) {
        optimisticThreadTitles.delete(threadId)
        continue
      }
      nextData = renameThreadInListData(nextData, threadId, title)
    }
    if (optimisticThreads.size === 0) return nextData

    const existingIds = new Set(nextData.threads?.map(thread => thread.id) ?? [])
    const missingThreads = [...optimisticThreads.values()].filter(thread => {
      if (!existingIds.has(thread.id)) return true
      optimisticThreads.delete(thread.id)
      return false
    })
    if (missingThreads.length === 0) return nextData

    for (const thread of missingThreads) {
      nextData = upsertPendingThreadInListData(nextData, thread)
    }
    return nextData
  }

  const stateWithThreadTitle = (
    currentState: ViewState,
    threadId: string,
    title: string,
  ): ViewState => {
    const data = dataForState(currentState)
    if (data === undefined) return currentState
    const renamedData = renameThreadInListData(data, threadId, title)
    if (renamedData === data) return currentState
    switch (currentState.phase) {
      case "loading":
        return { phase: "loading", data: renamedData }
      case "error":
        return { phase: "error", message: currentState.message, data: renamedData }
      case "ready":
        return { phase: "ready", data: renamedData }
      case "idle":
        return currentState
    }
  }

  const setStatusError = (error: unknown): void => {
    state = {
      phase: "error",
      message: friendlyKhalaCodeCodexThreadOpenErrorMessage(
        error instanceof Error ? error.message : String(error),
      ),
    }
    render()
  }

  const selectThread = async (
    threadId: string,
    source: CodexThreadSelectionSource,
  ): Promise<boolean> => {
    threadMenu.close()
    if (activeThreadId === threadId) return true
    const selectionId = ++selectionSequence
    activeThreadId = threadId
    selectingThreadId = threadId
    selectionError = null
    options.onThreadSelectionStarted?.({ selectionId, source, threadId })
    render()
    try {
      const result = await options.resumeThread(threadId)
      if (selectionId !== selectionSequence) return false
      activeThreadId = result.threadId
      selectingThreadId = null
      options.onThreadSelected({
        threadId: result.threadId,
        requestedThreadId: threadId,
        selectionId,
        source,
        messages: messagesForResult(result),
      })
      render()
      return true
    } catch (error) {
      if (selectionId !== selectionSequence) return false
      selectingThreadId = null
      selectionError = {
        threadId,
        message: friendlyKhalaCodeCodexThreadOpenErrorMessage(
          error instanceof Error ? error.message : String(error),
        ),
      }
      options.onThreadSelectionFailed?.({
        message: selectionError.message,
        selectionId,
        threadId,
      })
      render()
      return false
    }
  }

  const runMutation = async (
    action: () => Promise<KhalaCodeDesktopCodexThreadMutationResult>,
  ): Promise<void> => {
    try {
      const result = await action()
      if (!result.ok) throw new Error(result.error ?? `${result.action} failed`)
      if (result.newThreadId !== undefined) {
        activeThreadId = result.newThreadId
        options.onThreadSelected({
          threadId: result.newThreadId,
          messages: messagesForResult(result),
        })
      }
      await refresh()
    } catch (error) {
      setStatusError(error)
    }
  }

  const runRenameMutation = async (
    threadId: string,
    name: string,
  ): Promise<void> => {
    try {
      const result = await options.renameThread(threadId, name)
      if (!result.ok) throw new Error(result.error ?? "rename failed")
      await refresh()
    } catch (error) {
      if (optimisticThreadTitles.get(threadId) === name) {
        optimisticThreadTitles.delete(threadId)
      }
      setStatusError(error)
    }
  }

  const startNewChat = (): void => {
    threadMenu.close()
    activeThreadId = null
    options.onNewThreadRequested()
    render()
  }

  const cancelRename = (): void => {
    renamingThreadId = null
    renamingThreadDraft = ""
    render()
  }

  const beginRename = (thread: KhalaCodeDesktopCodexThreadSummary): void => {
    renamingThreadId = thread.id
    renamingThreadDraft = thread.title
    render()
  }

  const submitRename = (
    thread: KhalaCodeDesktopCodexThreadSummary,
    value: string,
  ): void => {
    const name = value.trim()
    if (name.length === 0 || name === thread.title.trim()) {
      cancelRename()
      return
    }
    renamingThreadId = null
    renamingThreadDraft = ""
    optimisticThreadTitles.set(thread.id, name)
    state = stateWithThreadTitle(state, thread.id, name)
    render()
    void runRenameMutation(thread.id, name)
  }

  const loadRecentThreadData = async (): Promise<KhalaCodeDesktopCodexThreadListResult> => {
    const requestSequence = ++refreshSequence
    const data = applyOptimisticThreads(
      await options.listThreads({ archived: false, includeHomeSessions, searchTerm: "" }),
    )
    if (requestSequence !== refreshSequence) return data
    state = { phase: "ready", data }
    activeThreadId = data.threads?.find(thread => thread.id === activeThreadId)?.id ?? activeThreadId
    render()
    return data
  }

  const selectRecentThread = async (index: number): Promise<boolean> => {
    if (!Number.isInteger(index) || index < 0 || index >= 10) return false
    try {
      const data = searchTerm.length === 0
        ? dataForState(state) ?? await loadRecentThreadData()
        : await loadRecentThreadData()
      const thread = recentThreadsForHotkeys(data.threads ?? [])[index]
      if (thread === undefined) return false
      return await selectThread(thread.id, "hotkey")
    } catch (error) {
      setStatusError(error)
      return false
    }
  }

  const selectAdjacentRecentThread = async (
    direction: RecentThreadCycleDirection,
  ): Promise<boolean> => {
    try {
      const data = searchTerm.length === 0
        ? dataForState(state) ?? await loadRecentThreadData()
        : await loadRecentThreadData()
      const index = recentThreadCycleIndex({
        activeThreadId,
        direction,
        threads: data.threads ?? [],
      })
      if (index === null) return false
      const thread = recentThreadsForHotkeys(data.threads ?? [])[index]
      if (thread === undefined) return false
      return await selectThread(thread.id, "hotkey")
    } catch (error) {
      setStatusError(error)
      return false
    }
  }

  const threadMenuContent = (
    thread: KhalaCodeDesktopCodexThreadSummary,
  ): BasecoatMenuDomContent => ({
    label: `Thread actions for ${thread.title}`,
    sections: [
      {
        items: [
          {
            id: "rename-thread",
            label: "Rename thread",
            icon: "Pencil",
            onSelect: () => beginRename(thread),
          },
          {
            id: "fork-thread",
            label: "Fork thread",
            icon: "BranchAlt",
            onSelect: () => void runMutation(() => options.forkThread(thread.id)),
          },
          {
            id: "copy-session-id",
            label: "Copy session ID",
            icon: "Copy",
            onSelect: () => {
              const sessionId = thread.sessionId ?? thread.id
              void navigator.clipboard?.writeText(sessionId).catch(() => undefined)
            },
          },
          {
            id: "archive-thread",
            label: "Archive thread",
            icon: "Archive",
            onSelect: () => void runMutation(() => options.archiveThread(thread.id)),
          },
          {
            id: "delete-thread",
            label: "Delete thread",
            icon: "Trash",
            destructive: true,
            onSelect: () => {
              if (!confirm(`Delete ${thread.title}?`)) return
              void runMutation(() => options.deleteThread(thread.id))
            },
          },
        ],
      },
    ],
  })

  const openThreadMenu = (
    thread: KhalaCodeDesktopCodexThreadSummary,
    point: BasecoatMenuDomPoint,
  ): void => {
    threadMenu.openAt(point, threadMenuContent(thread))
  }

  const threadRenameForm = (
    thread: KhalaCodeDesktopCodexThreadSummary,
  ): HTMLFormElement => {
    const form = el("form", "khala-thread-sidebar-rename-form")
    form.dataset.threadId = thread.id
    form.setAttribute("aria-label", `Rename ${thread.title}`)

    const renameInput = el("input", "khala-thread-sidebar-rename-input") as HTMLInputElement
    renameInput.name = "threadName"
    renameInput.type = "text"
    renameInput.value = renamingThreadDraft
    renameInput.autocomplete = "off"
    renameInput.setAttribute("aria-label", "Thread name")
    renameInput.addEventListener("input", () => {
      renamingThreadDraft = renameInput.value
    })

    const save = el("button", "khala-thread-sidebar-rename-action")
    save.type = "submit"
    save.title = "Save thread name"
    save.setAttribute("aria-label", "Save thread name")
    save.replaceChildren(sidebarIcon("Check", "Save thread name"), srOnly("Save thread name"))

    const cancel = el("button", "khala-thread-sidebar-rename-action")
    cancel.type = "button"
    cancel.title = "Cancel rename"
    cancel.setAttribute("aria-label", "Cancel rename")
    cancel.replaceChildren(sidebarIcon("X", "Cancel rename"), srOnly("Cancel rename"))
    cancel.addEventListener("click", event => {
      event.preventDefault()
      event.stopPropagation()
      cancelRename()
    })

    form.addEventListener("submit", event => {
      event.preventDefault()
      event.stopPropagation()
      submitRename(thread, renameInput.value)
    })
    form.addEventListener("keydown", event => {
      if (event.key !== "Escape") return
      event.preventDefault()
      event.stopPropagation()
      cancelRename()
    })
    form.addEventListener("focusout", event => {
      const nextTarget = event.relatedTarget
      if (nextTarget instanceof Node && form.contains(nextTarget)) return
      requestAnimationFrame(() => {
        const activeElement = form.ownerDocument.activeElement
        if (activeElement instanceof Node && form.contains(activeElement)) return
        if (renamingThreadId === thread.id) cancelRename()
      })
    })

    form.append(renameInput, save, cancel)
    requestAnimationFrame(() => {
      renameInput.focus({ preventScroll: true })
      renameInput.select()
    })
    return form
  }

  const toggleSearch = (): void => {
    threadMenu.close()
    if (searchOpen) {
      searchOpen = false
      searchShouldFocus = false
      if (searchTerm.length === 0) {
        render()
        return
      }
      searchTerm = ""
      void refresh()
      return
    }
    searchOpen = true
    searchShouldFocus = true
    render()
  }

  const toggleHomeSessions = (): void => {
    includeHomeSessions = !includeHomeSessions
    void refresh()
  }

  const threadButton = (
    thread: KhalaCodeDesktopCodexThreadSummary,
  ): HTMLElement => {
    const active = activeThreadId === thread.id
    const selecting = selectingThreadId === thread.id
    const item = el("div", "khala-thread-sidebar-item")
    item.dataset.threadId = thread.id
    item.dataset.status = thread.status
    item.dataset.active = active ? "true" : "false"
    if (selecting) item.dataset.selecting = "true"
    item.addEventListener("contextmenu", event => {
      event.preventDefault()
      event.stopPropagation()
      openThreadMenu(thread, { x: event.clientX, y: event.clientY })
    })

    const row = el("button", "khala-thread-sidebar-item-row")
    row.type = "button"
    row.title = `${thread.title} — ${thread.preview || thread.id}`
    row.setAttribute("aria-haspopup", "menu")
    row.dataset.active = active ? "true" : "false"
    if (selecting) {
      row.dataset.selecting = "true"
      row.setAttribute("aria-busy", "true")
    }
    if (active) row.setAttribute("aria-current", "true")
    row.addEventListener("click", () => void selectThread(thread.id, "sidebar"))
    row.addEventListener("keydown", event => {
      if (event.key !== "ContextMenu" && !(event.shiftKey && event.key === "F10")) return
      event.preventDefault()
      event.stopPropagation()
      const rect = row.getBoundingClientRect()
      openThreadMenu(thread, { x: rect.right, y: rect.top })
    })
    const hintDigit = hotkeyHintsVisible ? hotkeyHintDigits.get(thread.id) : undefined
    row.append(
      el("span", "khala-thread-sidebar-item-title", thread.title),
      hintDigit === undefined
        ? threadTimeContent(thread, options)
        : threadHotkeyHintContent(hintDigit),
    )

    item.append(renamingThreadId === thread.id ? threadRenameForm(thread) : row)
    if (selectionError?.threadId === thread.id) {
      item.append(dismissibleErrorRow(selectionError.message, () => {
        selectionError = null
        render()
      }))
    }
    return item
  }

  const dismissibleErrorRow = (
    message: string,
    onDismiss: () => void,
    className = "khala-thread-sidebar-row-error",
  ): HTMLDivElement => {
    const wrapper = el("div", className)
    wrapper.append(el("span", `${className}-text`, message))
    const dismiss = el("button", "khala-thread-sidebar-row-error-dismiss")
    dismiss.type = "button"
    dismiss.title = "Dismiss"
    dismiss.setAttribute("aria-label", "Dismiss error")
    dismiss.replaceChildren(sidebarIcon("X", "Dismiss"), srOnly("Dismiss"))
    dismiss.addEventListener("click", event => {
      event.stopPropagation()
      onDismiss()
    })
    wrapper.append(dismiss)
    return wrapper
  }

  function render(): void {
    const currentState = state
    hotkeyHintDigits = hotkeyHintsVisible
      ? recentThreadHotkeyHintDigits(dataForState(currentState)?.threads ?? [])
      : new Map()
    container.replaceChildren()
    const header = el("div", "khala-thread-sidebar-header")
    header.append(el("h2", "khala-thread-sidebar-title", "Chat"))

    const headerActions = el("div", "khala-thread-sidebar-header-actions")
    const searchToggle = el("button", "khala-thread-sidebar-search-toggle")
    searchToggle.type = "button"
    searchToggle.title = searchOpen ? "Close thread search" : "Search threads"
    searchToggle.setAttribute("aria-label", searchOpen ? "Close thread search" : "Search threads")
    searchToggle.setAttribute("aria-expanded", searchOpen ? "true" : "false")
    searchToggle.setAttribute("aria-controls", "khala-thread-sidebar-search-flyout")
    if (searchTerm.length > 0) searchToggle.dataset.active = "true"
    searchToggle.replaceChildren(sidebarIcon("Search", "Search threads"), srOnly("Search threads"))
    searchToggle.addEventListener("click", toggleSearch)

    const homeSessionsToggle = el("button", "khala-thread-sidebar-home-toggle")
    homeSessionsToggle.type = "button"
    homeSessionsToggle.title = includeHomeSessions
      ? "Showing all home sessions"
      : "Show all home sessions"
    homeSessionsToggle.setAttribute(
      "aria-label",
      includeHomeSessions ? "Showing all home sessions" : "Show all home sessions",
    )
    homeSessionsToggle.setAttribute("aria-pressed", includeHomeSessions ? "true" : "false")
    if (includeHomeSessions) homeSessionsToggle.dataset.active = "true"
    homeSessionsToggle.replaceChildren(sidebarIcon("History", "Home sessions"), srOnly("Home sessions"))
    homeSessionsToggle.addEventListener("click", toggleHomeSessions)

    const newThread = el("button", "khala-thread-sidebar-new")
    newThread.type = "button"
    newThread.title = "New thread"
    newThread.setAttribute("aria-label", "New thread")
    newThread.replaceChildren(sidebarIcon("Plus", "New thread"), srOnly("New thread"))
    newThread.addEventListener("click", startNewChat)
    headerActions.append(searchToggle, homeSessionsToggle, newThread)
    header.append(headerActions)
    container.append(header)

    if (searchOpen) {
      const searchFlyout = el("div", "khala-thread-sidebar-search-flyout")
      searchFlyout.id = "khala-thread-sidebar-search-flyout"

      const search = el("input", "khala-thread-sidebar-search")
      search.type = "search"
      search.name = "threadSearch"
      search.value = searchTerm
      search.placeholder = "Search threads"
      search.autocomplete = "off"
      search.setAttribute("aria-label", "Search Codex threads")
      search.addEventListener("change", () => {
        searchTerm = search.value.trim()
        void refresh()
      })
      search.addEventListener("search", () => {
        searchTerm = search.value.trim()
        void refresh()
      })
      search.addEventListener("keydown", event => {
        if (event.key !== "Escape") return
        event.preventDefault()
        searchOpen = false
        searchShouldFocus = false
        if (searchTerm.length === 0) {
          render()
          return
        }
        searchTerm = ""
        void refresh()
      })

      if (searchShouldFocus) {
        searchShouldFocus = false
        requestAnimationFrame(() => search.focus({ preventScroll: true }))
      }

      const refreshButton = el("button", "khala-thread-sidebar-refresh")
      refreshButton.type = "button"
      refreshButton.title = "Refresh threads"
      refreshButton.setAttribute("aria-label", "Refresh threads")
      refreshButton.replaceChildren(sidebarIcon("Reload", "Refresh threads"), srOnly("Refresh threads"))
      refreshButton.addEventListener("click", () => void refresh())

      searchFlyout.append(search, refreshButton)
      container.append(searchFlyout)
    }

    const data = dataForState(currentState)
    if (currentState.phase === "idle" || data === undefined) {
      container.append(el("p", "khala-thread-sidebar-empty", "Loading threads"))
      return
    }
    if (currentState.phase === "error") {
      const dismissedMessage = currentState.message
      container.append(dismissibleErrorRow(
        dismissedMessage,
        () => {
          state = data === undefined ? { phase: "idle" } : { phase: "ready", data }
          render()
        },
        "khala-thread-sidebar-error",
      ))
    }
    if ((data.threads ?? []).length === 0) {
      container.append(el("p", "khala-thread-sidebar-empty", "No threads"))
      return
    }

    for (const group of groupThreads(data)) {
      if (group.threads.length === 0) continue
      const section = el("section", "khala-thread-sidebar-group")
      if (group.label.trim().length > 0) {
        section.append(el("h3", "khala-thread-sidebar-group-title", group.label))
      }
      const list = el("div", "khala-thread-sidebar-list")
      list.append(...group.threads.map(threadButton))
      section.append(list)
      container.append(section)
    }
  }

  async function refresh(): Promise<void> {
    threadMenu.close()
    const requestSequence = ++refreshSequence
    const previousData = dataForState(state)
    state = previousData === undefined
      ? { phase: "loading" }
      : { phase: "loading", data: previousData }
    render()
    try {
      const data = applyOptimisticThreads(
        await options.listThreads({ archived: false, includeHomeSessions, searchTerm }),
      )
      if (requestSequence !== refreshSequence) return
      state = { phase: "ready", data }
      activeThreadId = data.threads?.find(thread => thread.id === activeThreadId)?.id ?? activeThreadId
      if (
        renamingThreadId !== null &&
        data.threads?.some(thread => thread.id === renamingThreadId) !== true
      ) {
        renamingThreadId = null
        renamingThreadDraft = ""
      }
    } catch (error) {
      if (requestSequence !== refreshSequence) return
      const message = friendlyKhalaCodeCodexThreadOpenErrorMessage(
        error instanceof Error ? error.message : String(error),
      )
      state = previousData === undefined
        ? { phase: "error", message }
        : { phase: "error", message, data: previousData }
    }
    render()
  }

  render()

  return {
    recentThreads: () => recentThreadsForHotkeys(dataForState(state)?.threads ?? []),
    refresh,
    selectAdjacentRecentThread,
    selectRecentThread,
    setActiveThreadId(threadId) {
      activeThreadId = threadId
      if (threadId !== selectingThreadId) selectingThreadId = null
      render()
    },
    setHotkeyHintsVisible(nextVisible) {
      if (hotkeyHintsVisible === nextVisible) return
      hotkeyHintsVisible = nextVisible
      render()
    },
    setVisible(nextVisible) {
      visible = nextVisible
      container.hidden = !visible
      if (!visible) {
        threadMenu.close()
        renamingThreadId = null
        renamingThreadDraft = ""
      }
      if (visible && state.phase === "idle") void refresh()
    },
    upsertPendingThread(input) {
      const preview = input.preview.trim()
      if (input.threadId.length === 0 || preview.length === 0) return
      const now = Date.now()
      optimisticThreads.set(input.threadId, {
        id: input.threadId,
        sessionId: input.threadId,
        title: preview.split(/\r?\n/u)[0]?.slice(0, 80) ?? input.threadId,
        preview,
        cwd: null,
        projectLabel: pendingActiveThreadGroupLabel,
        status: "active",
        statusLabel: "active",
        modelProvider: null,
        source: "codex",
        forkedFromId: null,
        parentThreadId: null,
        createdAt: now,
        updatedAt: now,
        recencyAt: now,
        badges: [],
      })
      const data = dataForState(state)
      if (data !== undefined) {
        state = state.phase === "loading"
          ? { phase: "loading", data: applyOptimisticThreads(data) }
          : state.phase === "error"
            ? { phase: "error", message: state.message, data: applyOptimisticThreads(data) }
            : state.phase === "ready"
              ? { phase: "ready", data: applyOptimisticThreads(data) }
              : state
      }
      activeThreadId = input.threadId
      render()
    },
  }
}
