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

export type CodexThreadSidebarHandle = {
  readonly refresh: () => Promise<void>
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
  readonly startThread: () => Promise<KhalaCodeDesktopCodexThreadResult>
  readonly unarchiveThread: (threadId: string) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
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

const formatTime = (seconds: number | null): string => {
  if (seconds === null) return ""
  try {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: "short",
      timeStyle: "short",
    }).format(new Date(seconds * 1000))
  } catch {
    return String(seconds)
  }
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

export const mountCodexThreadSidebar = (
  container: HTMLElement,
  options: CodexThreadSidebarOptions,
): CodexThreadSidebarHandle => {
  let archived = false
  let searchTerm = ""
  let state: ViewState = { phase: "idle" }
  let visible = false
  let activeThreadId = options.activeThreadId()
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

  const selectThread = async (threadId: string): Promise<void> => {
    threadMenu.close()
    try {
      const result = await options.resumeThread(threadId)
      activeThreadId = result.threadId
      options.onThreadSelected({
        threadId: result.threadId,
        messages: messagesForResult(result),
      })
      render()
    } catch (error) {
      setStatusError(error)
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

  const startThread = async (): Promise<void> => {
    try {
      const result = await options.startThread()
      activeThreadId = result.threadId
      options.onThreadSelected({
        threadId: result.threadId,
        messages: messagesForResult(result),
      })
      void refresh()
    } catch (error) {
      setStatusError(error)
    }
  }

  const threadMenuHeader = (
    thread: KhalaCodeDesktopCodexThreadSummary,
  ): HTMLElement => {
    const header = el("div", "khala-thread-sidebar-menu-summary")
    const title = el("div", "khala-thread-sidebar-menu-title", thread.title)
    const preview = el("div", "khala-thread-sidebar-menu-preview", thread.preview || thread.id)
    const meta = el("div", "khala-thread-sidebar-menu-meta")
    const time = formatTime(thread.recencyAt ?? thread.updatedAt)
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
            onSelect: () => {
              const name = prompt("Thread name", thread.title)
              if (name === null || name.trim().length === 0) return
              void runMutation(() => options.renameThread(thread.id, name.trim()))
            },
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
          archived
            ? {
                id: "unarchive-thread",
                label: "Unarchive thread",
                description: "Return to active threads",
                icon: "Unarchive",
                onSelect: () => void runMutation(() => options.unarchiveThread(thread.id)),
              }
            : {
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
    row.addEventListener("click", () => void selectThread(thread.id))
    row.append(
      el("span", "khala-thread-sidebar-item-title", thread.title),
      el("span", "khala-thread-sidebar-item-time", formatTime(thread.recencyAt ?? thread.updatedAt) || thread.statusLabel),
    )

    const menuButton = el("button", "khala-thread-sidebar-menu-button")
    menuButton.type = "button"
    menuButton.title = "Thread actions"
    menuButton.setAttribute("aria-label", `Thread actions for ${thread.title}`)
    menuButton.setAttribute("aria-haspopup", "menu")
    menuButton.replaceChildren(sidebarIcon("DotsVerticalMoreMenu", "Thread actions"), srOnly("Thread actions"))
    menuButton.addEventListener("click", event => {
      event.preventDefault()
      event.stopPropagation()
      const rect = menuButton.getBoundingClientRect()
      openThreadMenu(thread, { x: rect.right + 4, y: rect.top })
    })

    item.append(row, menuButton)
    return item
  }

  function render(): void {
    const currentState = state
    container.replaceChildren()
    const header = el("div", "khala-thread-sidebar-header")
    header.append(el("h2", "khala-thread-sidebar-title", "Chat"))
    const newThread = el("button", "khala-thread-sidebar-new")
    newThread.type = "button"
    newThread.title = "New thread"
    newThread.setAttribute("aria-label", "New thread")
    newThread.replaceChildren(sidebarIcon("Plus", "New thread"), srOnly("New thread"))
    newThread.addEventListener("click", () => void startThread())
    header.append(newThread)

    const controls = el("div", "khala-thread-sidebar-controls")
    const search = el("input", "khala-thread-sidebar-search")
    search.type = "search"
    search.value = searchTerm
    search.placeholder = "Search threads"
    search.setAttribute("aria-label", "Search Codex threads")
    search.addEventListener("change", () => {
      searchTerm = search.value
      void refresh()
    })
    const archiveLabel = el("label", "khala-thread-sidebar-toggle")
    const archiveInput = el("input")
    archiveInput.type = "checkbox"
    archiveInput.checked = archived
    archiveInput.addEventListener("change", () => {
      archived = archiveInput.checked
      void refresh()
    })
    archiveLabel.append(archiveInput, el("span", undefined, "Archived"))
    const refreshButton = el("button", "khala-thread-sidebar-refresh")
    refreshButton.type = "button"
    refreshButton.title = "Refresh threads"
    refreshButton.setAttribute("aria-label", "Refresh threads")
    refreshButton.replaceChildren(sidebarIcon("Reload", "Refresh threads"), srOnly("Refresh threads"))
    refreshButton.addEventListener("click", () => void refresh())
    controls.append(search, archiveLabel, refreshButton)
    container.append(header, controls)

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
      const data = await options.listThreads({ archived, searchTerm })
      if (requestSequence !== refreshSequence) return
      state = { phase: "ready", data }
      activeThreadId = data.threads?.find(thread => thread.id === activeThreadId)?.id ?? activeThreadId
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
    setActiveThreadId(threadId) {
      activeThreadId = threadId
      render()
    },
    setVisible(nextVisible) {
      visible = nextVisible
      container.hidden = !visible
      if (!visible) threadMenu.close()
      if (visible && state.phase === "idle") void refresh()
    },
  }
}
