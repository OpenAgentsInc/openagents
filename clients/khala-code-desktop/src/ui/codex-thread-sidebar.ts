import type {
  KhalaCodeDesktopCodexThreadListResult,
  KhalaCodeDesktopCodexThreadMutationResult,
  KhalaCodeDesktopCodexThreadResult,
  KhalaCodeDesktopMessage,
} from "../shared/rpc"
import type {
  KhalaCodeDesktopCodexThreadSummary,
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
  recentThreadsForHotkeys,
} from "./thread-hotkeys"

export type CodexThreadSidebarHandle = {
  readonly refresh: () => Promise<void>
  readonly selectAdjacentRecentThread: (direction: RecentThreadCycleDirection) => Promise<boolean>
  readonly selectRecentThread: (index: number) => Promise<boolean>
  readonly setActiveThreadId: (threadId: string | null) => void
  readonly setVisible: (visible: boolean) => void
}

export type CodexThreadSidebarOptions = {
  readonly activeThreadId: () => string | null
  readonly archiveThread: (threadId: string) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  readonly deleteThread: (threadId: string) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  readonly forkThread: (threadId: string) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  readonly listThreads: (input: {
    readonly archived: boolean
    readonly searchTerm: string
  }) => Promise<KhalaCodeDesktopCodexThreadListResult>
  readonly renameThread: (threadId: string, name: string) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  readonly resumeThread: (threadId: string) => Promise<KhalaCodeDesktopCodexThreadResult>
  readonly sessionId: string
  readonly unarchiveThread: (threadId: string) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  readonly onNewThreadRequested: () => void
  readonly onThreadSelected: (
    input: {
      readonly messages: readonly KhalaCodeDesktopMessage[]
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

export const mountCodexThreadSidebar = (
  container: HTMLElement,
  options: CodexThreadSidebarOptions,
): CodexThreadSidebarHandle => {
  let searchOpen = false
  let searchShouldFocus = false
  let searchTerm = ""
  let state: ViewState = { phase: "idle" }
  let visible = false
  let activeThreadId = options.activeThreadId()
  let renamingThreadId: string | null = null
  let renamingThreadDraft = ""
  let refreshSequence = 0
  const threadMenu = createBasecoatContextMenu({
    id: "khala-thread-sidebar-thread-menu",
    ownerDocument: container.ownerDocument,
    className: "khala-thread-sidebar-menu",
  })

  const setStatusError = (error: unknown): void => {
    state = { phase: "error", message: error instanceof Error ? error.message : String(error) }
    render()
  }

  const selectThread = async (threadId: string): Promise<boolean> => {
    threadMenu.close()
    try {
      const result = await options.resumeThread(threadId)
      activeThreadId = result.threadId
      options.onThreadSelected({
        threadId: result.threadId,
        messages: messagesForResult(result),
      })
      render()
      return true
    } catch (error) {
      setStatusError(error)
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
    renamingThreadId = null
    renamingThreadDraft = ""
    render()
    if (name.length === 0 || name === thread.title.trim()) return
    void runMutation(() => options.renameThread(thread.id, name))
  }

  const loadRecentThreadData = async (): Promise<KhalaCodeDesktopCodexThreadListResult> => {
    const requestSequence = ++refreshSequence
    const data = await options.listThreads({ archived: false, searchTerm: "" })
    if (requestSequence !== refreshSequence) return data
    state = { phase: "ready", data }
    activeThreadId = data.threads?.find(thread => thread.id === activeThreadId)?.id ?? activeThreadId
    render()
    return data
  }

  const selectRecentThread = async (index: number): Promise<boolean> => {
    if (!Number.isInteger(index) || index < 0 || index >= 10) return false
    try {
      const data = await loadRecentThreadData()
      const thread = recentThreadsForHotkeys(data.threads ?? [])[index]
      if (thread === undefined) return false
      return await selectThread(thread.id)
    } catch (error) {
      setStatusError(error)
      return false
    }
  }

  const selectAdjacentRecentThread = async (
    direction: RecentThreadCycleDirection,
  ): Promise<boolean> => {
    try {
      const data = await loadRecentThreadData()
      const index = recentThreadCycleIndex({
        activeThreadId,
        direction,
        threads: data.threads ?? [],
      })
      if (index === null) return false
      const thread = recentThreadsForHotkeys(data.threads ?? [])[index]
      if (thread === undefined) return false
      return await selectThread(thread.id)
    } catch (error) {
      setStatusError(error)
      return false
    }
  }

  const threadMenuHeader = (
    thread: KhalaCodeDesktopCodexThreadSummary,
  ): HTMLElement => {
    const header = el("div", "khala-thread-sidebar-menu-summary")
    const title = el("div", "khala-thread-sidebar-menu-title", thread.title)
    const preview = el("div", "khala-thread-sidebar-menu-preview", thread.preview || thread.id)
    const meta = el("div", "khala-thread-sidebar-menu-meta")
    const time = formatCompactThreadTimestamp(thread.recencyAt ?? thread.updatedAt)
    meta.append(el("span", undefined, thread.statusLabel || thread.status))
    if (time.length > 0) meta.append(el("span", undefined, time))
    header.append(title, preview, meta)

    if (thread.badges.length > 0) {
      const badges = el("div", "khala-thread-sidebar-menu-badges")
      for (const badge of thread.badges) badges.append(el("span", "khala-thread-sidebar-menu-badge", badge))
      header.append(badges)
    }

    return header
  }

  const threadMenuContent = (
    thread: KhalaCodeDesktopCodexThreadSummary,
  ): BasecoatMenuDomContent => ({
    label: `Thread actions for ${thread.title}`,
    header: threadMenuHeader(thread),
    sections: [
      {
        label: "Thread",
        items: [
          {
            id: "rename-thread",
            label: "Rename thread",
            description: "Set display name",
            icon: "Pencil",
            onSelect: () => beginRename(thread),
          },
          {
            id: "fork-thread",
            label: "Fork thread",
            description: "Create branch thread",
            icon: "BranchAlt",
            onSelect: () => void runMutation(() => options.forkThread(thread.id)),
          },
          {
            id: "copy-session-id",
            label: "Copy session ID",
            description: thread.sessionId === null
              ? "Copy thread ID fallback"
              : "Copy Codex session ref",
            icon: "Copy",
            onSelect: () => {
              const sessionId = thread.sessionId ?? thread.id
              void navigator.clipboard?.writeText(sessionId).catch(() => undefined)
            },
          },
        ],
      },
      {
        label: "Lifecycle",
        items: [
          {
            id: "archive-thread",
            label: "Archive thread",
            description: "Move out of active threads",
            icon: "Archive",
            onSelect: () => void runMutation(() => options.archiveThread(thread.id)),
          },
          {
            id: "delete-thread",
            label: "Delete thread",
            description: "Remove this thread",
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

  const threadButton = (
    thread: KhalaCodeDesktopCodexThreadSummary,
  ): HTMLElement => {
    const item = el("div", "khala-thread-sidebar-item")
    item.dataset.threadId = thread.id
    item.dataset.status = thread.status
    item.dataset.active = activeThreadId === thread.id ? "true" : "false"
    item.addEventListener("contextmenu", event => {
      event.preventDefault()
      event.stopPropagation()
      openThreadMenu(thread, { x: event.clientX, y: event.clientY })
    })

    const row = el("button", "khala-thread-sidebar-item-row")
    row.type = "button"
    row.title = `${thread.title} — ${thread.preview || thread.id}`
    row.setAttribute("aria-haspopup", "menu")
    row.addEventListener("click", () => void selectThread(thread.id))
    row.addEventListener("keydown", event => {
      if (event.key !== "ContextMenu" && !(event.shiftKey && event.key === "F10")) return
      event.preventDefault()
      event.stopPropagation()
      const rect = row.getBoundingClientRect()
      openThreadMenu(thread, { x: rect.right, y: rect.top })
    })
    row.append(
      el("span", "khala-thread-sidebar-item-title", thread.title),
      el(
        "span",
        "khala-thread-sidebar-item-time",
        formatCompactThreadTimestamp(thread.recencyAt ?? thread.updatedAt) || thread.statusLabel,
      ),
    )

    item.append(renamingThreadId === thread.id ? threadRenameForm(thread) : row)
    return item
  }

  function render(): void {
    const currentState = state
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

    const newThread = el("button", "khala-thread-sidebar-new")
    newThread.type = "button"
    newThread.title = "New thread"
    newThread.setAttribute("aria-label", "New thread")
    newThread.replaceChildren(sidebarIcon("Plus", "New thread"), srOnly("New thread"))
    newThread.addEventListener("click", startNewChat)
    headerActions.append(searchToggle, newThread)
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
      container.append(el("p", "khala-thread-sidebar-error", currentState.message))
    }
    if ((data.threads ?? []).length === 0) {
      container.append(el("p", "khala-thread-sidebar-empty", "No threads"))
      return
    }

    for (const group of groupThreads(data)) {
      if (group.threads.length === 0) continue
      const section = el("section", "khala-thread-sidebar-group")
      section.append(el("h3", "khala-thread-sidebar-group-title", group.label))
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
      const data = await options.listThreads({ archived: false, searchTerm })
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
      const message = error instanceof Error ? error.message : String(error)
      state = previousData === undefined
        ? { phase: "error", message }
        : { phase: "error", message, data: previousData }
    }
    render()
  }

  render()

  return {
    refresh,
    selectAdjacentRecentThread,
    selectRecentThread,
    setActiveThreadId(threadId) {
      activeThreadId = threadId
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
  }
}
