import type {
  KhalaCodeDesktopCodexThreadListResult,
  KhalaCodeDesktopCodexThreadMutationResult,
  KhalaCodeDesktopCodexThreadResult,
  KhalaCodeDesktopMessage,
} from "../shared/rpc"
import type {
  KhalaCodeDesktopCodexThreadSummary,
} from "../shared/codex-threads"

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
  | { readonly phase: "idle" | "loading" }
  | { readonly phase: "error"; readonly message: string }
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

export const mountCodexThreadSidebar = (
  container: HTMLElement,
  options: CodexThreadSidebarOptions,
): CodexThreadSidebarHandle => {
  let archived = false
  let searchTerm = ""
  let state: ViewState = { phase: "idle" }
  let visible = false
  let activeThreadId = options.activeThreadId()

  const setStatusError = (error: unknown): void => {
    state = { phase: "error", message: error instanceof Error ? error.message : String(error) }
    render()
  }

  const selectThread = async (threadId: string): Promise<void> => {
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
      await refresh()
    } catch (error) {
      setStatusError(error)
    }
  }

  const actionButton = (
    label: string,
    action: () => void,
  ): HTMLButtonElement => {
    const button = el("button", "khala-thread-sidebar-action", label)
    button.type = "button"
    button.title = label
    button.addEventListener("click", event => {
      event.preventDefault()
      event.stopPropagation()
      action()
    })
    return button
  }

  const threadButton = (
    thread: KhalaCodeDesktopCodexThreadSummary,
  ): HTMLElement => {
    const button = el("button", "khala-thread-sidebar-item")
    button.type = "button"
    button.dataset.threadId = thread.id
    button.dataset.status = thread.status
    button.dataset.active = activeThreadId === thread.id ? "true" : "false"
    button.addEventListener("click", () => void selectThread(thread.id))

    const main = el("span", "khala-thread-sidebar-item-main")
    main.append(
      el("span", "khala-thread-sidebar-item-title", thread.title),
      el("span", "khala-thread-sidebar-item-preview", thread.preview || thread.id),
    )
    const meta = el("span", "khala-thread-sidebar-item-meta")
    meta.append(
      el("span", undefined, thread.statusLabel),
      el("span", undefined, formatTime(thread.recencyAt ?? thread.updatedAt)),
    )
    const badges = el("span", "khala-thread-sidebar-badges")
    for (const badge of thread.badges) badges.append(el("span", "khala-thread-sidebar-badge", badge))
    const actions = el("span", "khala-thread-sidebar-actions")
    actions.append(
      actionButton("Name", () => {
        const name = prompt("Thread name", thread.title)
        if (name === null || name.trim().length === 0) return
        void runMutation(() => options.renameThread(thread.id, name.trim()))
      }),
      actionButton("Fork", () => void runMutation(() => options.forkThread(thread.id))),
      archived
        ? actionButton("Unarchive", () => void runMutation(() => options.unarchiveThread(thread.id)))
        : actionButton("Archive", () => void runMutation(() => options.archiveThread(thread.id))),
      actionButton("Delete", () => {
        if (!confirm(`Delete ${thread.title}?`)) return
        void runMutation(() => options.deleteThread(thread.id))
      }),
    )
    button.append(main, meta, badges, actions)
    return button
  }

  function render(): void {
    const currentState = state
    container.replaceChildren()
    const header = el("div", "khala-thread-sidebar-header")
    header.append(el("h2", "khala-thread-sidebar-title", "Chat"))
    const newThread = el("button", "khala-thread-sidebar-new", "New")
    newThread.type = "button"
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
    const refreshButton = el("button", "khala-thread-sidebar-refresh", "Refresh")
    refreshButton.type = "button"
    refreshButton.addEventListener("click", () => void refresh())
    controls.append(search, archiveLabel, refreshButton)
    container.append(header, controls)

    if (currentState.phase === "idle" || currentState.phase === "loading") {
      container.append(el("p", "khala-thread-sidebar-empty", "Loading threads"))
      return
    }
    if (currentState.phase === "error") {
      container.append(el("p", "khala-thread-sidebar-error", currentState.message))
      return
    }
    if (currentState.phase !== "ready") return
    const data = currentState.data
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
    state = { phase: "loading" }
    render()
    try {
      const data = await options.listThreads({ archived, searchTerm })
      state = { phase: "ready", data }
      activeThreadId = data.threads?.find(thread => thread.id === activeThreadId)?.id ?? activeThreadId
    } catch (error) {
      state = { phase: "error", message: error instanceof Error ? error.message : String(error) }
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
      if (visible && state.phase === "idle") void refresh()
    },
  }
}
