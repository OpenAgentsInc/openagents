import * as React from "react"
import { flushSync } from "react-dom"
import { createRoot, type Root } from "react-dom/client"

import { cn } from "@openagentsinc/ui/react"
import { iconSvg, type IconName } from "@openagentsinc/ui/icon"

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

type SelectionError = Readonly<{
  message: string
  threadId: string
}>

type ThreadGroup = Readonly<{
  label: string
  threads: readonly KhalaCodeDesktopCodexThreadSummary[]
}>

type SidebarActions = Readonly<{
  beginRename: (thread: KhalaCodeDesktopCodexThreadSummary) => void
  cancelRename: () => void
  dismissGlobalError: () => void
  dismissSelectionError: () => void
  openThreadMenu: (thread: KhalaCodeDesktopCodexThreadSummary, point: BasecoatMenuDomPoint) => void
  refresh: () => void
  selectThread: (threadId: string) => void
  startNewChat: () => void
  submitRename: (thread: KhalaCodeDesktopCodexThreadSummary, value: string) => void
  toggleHomeSessions: () => void
  toggleSearch: () => void
  updateSearch: (value: string) => void
}>

type SidebarReactProps = Readonly<{
  activeThreadId: string | null
  currentState: ViewState
  data: KhalaCodeDesktopCodexThreadListResult | undefined
  groups: readonly ThreadGroup[]
  hotkeyHintDigits: ReadonlyMap<string, number>
  hotkeyHintsVisible: boolean
  includeHomeSessions: boolean
  isThreadStreaming: (threadId: string) => boolean
  renamingThreadDraft: string
  renamingThreadId: string | null
  searchOpen: boolean
  searchShouldFocus: boolean
  searchTerm: string
  selectingThreadId: string | null
  selectionError: SelectionError | null
  actions: SidebarActions
}>

const srOnly = (text: string): React.JSX.Element => (
  <span className="khala-code-sr-only">{text}</span>
)

const SidebarIcon = ({
  icon,
  label,
}: {
  readonly icon: IconName
  readonly label: string
}): React.JSX.Element => (
  <span
    aria-hidden="true"
    className="oa-ui-icon khala-thread-sidebar-icon"
    data-oa-ui-icon={label}
    dangerouslySetInnerHTML={{ __html: iconSvg(icon) }}
  />
)

const isThreadStreaming = (
  threadId: string,
  options: Pick<CodexThreadSidebarOptions, "isThreadStreaming">,
): boolean =>
  options.isThreadStreaming?.(threadId) === true

const ThreadStreamingIndicator = (): React.JSX.Element => (
  <span aria-hidden="true" className="khala-thread-sidebar-item-spinner" />
)

const ThreadHotkeyHint = ({
  digit,
}: {
  readonly digit: number
}): React.JSX.Element => (
  <span
    aria-label={`Jump with Command ${digit}`}
    className="khala-thread-sidebar-item-time"
    data-hotkey-hint={String(digit)}
    title={`Jump with Command ${digit}`}
  >
    {`⌘${digit}`}
  </span>
)

const isThreadResumable = (
  thread: KhalaCodeDesktopCodexThreadSummary,
): boolean =>
  thread.resumable !== false

const ThreadTimeContent = ({
  isStreaming,
  thread,
}: {
  readonly isStreaming: boolean
  readonly thread: KhalaCodeDesktopCodexThreadSummary
}): React.JSX.Element => {
  if (!isThreadResumable(thread)) {
    return (
      <span
        className="khala-thread-sidebar-item-time"
        title={thread.unavailableReason ?? thread.statusLabel}
      >
        {thread.statusLabel}
      </span>
    )
  }

  if (isStreaming) {
    return (
      <span
        aria-label="Streaming response"
        className="khala-thread-sidebar-item-time"
        data-streaming="true"
        title="Streaming response"
      >
        <ThreadStreamingIndicator />
        {srOnly("Streaming response")}
      </span>
    )
  }

  return (
    <span className="khala-thread-sidebar-item-time">
      {formatCompactThreadTimestamp(thread.recencyAt ?? thread.updatedAt) || thread.statusLabel}
    </span>
  )
}

const groupThreads = (
  data: KhalaCodeDesktopCodexThreadListResult,
): readonly ThreadGroup[] => {
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

const DismissibleErrorRow = ({
  className = "khala-thread-sidebar-row-error",
  message,
  onDismiss,
}: {
  readonly className?: string
  readonly message: string
  readonly onDismiss: () => void
}): React.JSX.Element => (
  <div className={cn(className, "flex items-center justify-between gap-1")}>
    <span className={`${className}-text`}>{message}</span>
    <button
      aria-label="Dismiss error"
      className="khala-thread-sidebar-row-error-dismiss"
      onClick={event => {
        event.stopPropagation()
        onDismiss()
      }}
      title="Dismiss"
      type="button"
    >
      <SidebarIcon icon="X" label="Dismiss" />
      {srOnly("Dismiss")}
    </button>
  </div>
)

const SearchFlyout = ({
  actions,
  searchShouldFocus,
  searchTerm,
}: Pick<SidebarReactProps, "actions" | "searchShouldFocus" | "searchTerm">): React.JSX.Element => {
  const searchRef = React.useRef<HTMLInputElement | null>(null)
  const [draft, setDraft] = React.useState(searchTerm)

  React.useLayoutEffect(() => {
    setDraft(searchTerm)
  }, [searchTerm])

  React.useLayoutEffect(() => {
    if (!searchShouldFocus) return
    requestAnimationFrame(() => searchRef.current?.focus({ preventScroll: true }))
  }, [searchShouldFocus])

  const submitDraft = (value: string): void => {
    actions.updateSearch(value)
  }

  return (
    <div
      className="khala-thread-sidebar-search-flyout grid grid-cols-[minmax(0,1fr)_auto] gap-2"
      id="khala-thread-sidebar-search-flyout"
    >
      <input
        ref={searchRef}
        aria-label="Search Codex threads"
        autoComplete="off"
        className="khala-thread-sidebar-search"
        name="threadSearch"
        onChange={event => submitDraft(event.currentTarget.value)}
        onInput={event => setDraft(event.currentTarget.value)}
        onKeyDown={event => {
          if (event.key !== "Escape") return
          event.preventDefault()
          actions.updateSearch("")
          actions.toggleSearch()
        }}
        placeholder="Search threads"
        type="search"
        value={draft}
      />
      <button
        aria-label="Refresh threads"
        className="khala-thread-sidebar-refresh"
        onClick={actions.refresh}
        title="Refresh threads"
        type="button"
      >
        <SidebarIcon icon="Reload" label="Refresh threads" />
        {srOnly("Refresh threads")}
      </button>
    </div>
  )
}

const RenameForm = ({
  actions,
  initialDraft,
  thread,
}: {
  readonly actions: SidebarActions
  readonly initialDraft: string
  readonly thread: KhalaCodeDesktopCodexThreadSummary
}): React.JSX.Element => {
  const inputRef = React.useRef<HTMLInputElement | null>(null)
  const formRef = React.useRef<HTMLFormElement | null>(null)
  const [draft, setDraft] = React.useState(initialDraft)

  React.useLayoutEffect(() => {
    setDraft(initialDraft)
  }, [initialDraft, thread.id])

  React.useLayoutEffect(() => {
    requestAnimationFrame(() => {
      inputRef.current?.focus({ preventScroll: true })
      inputRef.current?.select()
    })
  }, [thread.id])

  return (
    <form
      ref={formRef}
      aria-label={`Rename ${thread.title}`}
      className="khala-thread-sidebar-rename-form grid grid-cols-[minmax(0,1fr)_1.5rem_1.5rem] items-center"
      data-thread-id={thread.id}
      onBlur={event => {
        const nextTarget = event.relatedTarget
        if (nextTarget instanceof Node && event.currentTarget.contains(nextTarget)) return
        requestAnimationFrame(() => {
          const activeElement = formRef.current?.ownerDocument.activeElement
          if (activeElement instanceof Node && formRef.current?.contains(activeElement) === true) return
          actions.cancelRename()
        })
      }}
      onKeyDown={event => {
        if (event.key !== "Escape") return
        event.preventDefault()
        event.stopPropagation()
        actions.cancelRename()
      }}
      onSubmit={event => {
        event.preventDefault()
        event.stopPropagation()
        actions.submitRename(thread, inputRef.current?.value ?? draft)
      }}
    >
      <input
        ref={inputRef}
        aria-label="Thread name"
        autoComplete="off"
        className="khala-thread-sidebar-rename-input"
        name="threadName"
        onChange={event => setDraft(event.currentTarget.value)}
        type="text"
        value={draft}
      />
      <button
        aria-label="Save thread name"
        className="khala-thread-sidebar-rename-action"
        title="Save thread name"
        type="submit"
      >
        <SidebarIcon icon="Check" label="Save thread name" />
        {srOnly("Save thread name")}
      </button>
      <button
        aria-label="Cancel rename"
        className="khala-thread-sidebar-rename-action"
        onClick={event => {
          event.preventDefault()
          event.stopPropagation()
          actions.cancelRename()
        }}
        title="Cancel rename"
        type="button"
      >
        <SidebarIcon icon="X" label="Cancel rename" />
        {srOnly("Cancel rename")}
      </button>
    </form>
  )
}

const ThreadItem = ({
  actions,
  activeThreadId,
  hotkeyHintDigits,
  hotkeyHintsVisible,
  isStreaming,
  renamingThreadDraft,
  renamingThreadId,
  selectingThreadId,
  selectionError,
  thread,
}: Pick<
  SidebarReactProps,
  | "actions"
  | "activeThreadId"
  | "hotkeyHintDigits"
  | "hotkeyHintsVisible"
  | "renamingThreadDraft"
  | "renamingThreadId"
  | "selectingThreadId"
  | "selectionError"
> & Readonly<{
  isStreaming: boolean
  thread: KhalaCodeDesktopCodexThreadSummary
}>): React.JSX.Element => {
  const active = activeThreadId === thread.id
  const selecting = selectingThreadId === thread.id
  const resumable = isThreadResumable(thread)
  const hoverDetail = resumable
    ? thread.preview || thread.id
    : thread.unavailableReason || thread.preview || thread.id
  const hintDigit = hotkeyHintsVisible ? hotkeyHintDigits.get(thread.id) : undefined

  const openMenuFromRow = (event: React.MouseEvent<HTMLElement>): void => {
    event.preventDefault()
    event.stopPropagation()
    actions.openThreadMenu(thread, { x: event.clientX, y: event.clientY })
  }

  return (
    <div
      className="khala-thread-sidebar-item grid min-w-0 grid-cols-[minmax(0,1fr)]"
      data-active={active ? "true" : "false"}
      data-resumable={resumable ? undefined : "false"}
      data-selecting={selecting ? "true" : undefined}
      data-status={thread.status}
      data-thread-id={thread.id}
      onContextMenu={openMenuFromRow}
    >
      {renamingThreadId === thread.id ? (
        <RenameForm actions={actions} initialDraft={renamingThreadDraft} thread={thread} />
      ) : (
        <button
          aria-busy={selecting ? "true" : undefined}
          aria-current={active ? "true" : undefined}
          aria-disabled={resumable ? undefined : "true"}
          aria-haspopup="menu"
          className="khala-thread-sidebar-item-row grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-center"
          data-active={active ? "true" : "false"}
          data-selecting={selecting ? "true" : undefined}
          disabled={!resumable}
          onClick={resumable ? () => actions.selectThread(thread.id) : undefined}
          onKeyDown={event => {
            if (event.key !== "ContextMenu" && !(event.shiftKey && event.key === "F10")) return
            event.preventDefault()
            event.stopPropagation()
            const rect = event.currentTarget.getBoundingClientRect()
            actions.openThreadMenu(thread, { x: rect.right, y: rect.top })
          }}
          title={`${thread.title} — ${hoverDetail}`}
          type="button"
        >
          <span className="khala-thread-sidebar-item-title truncate">{thread.title}</span>
          {hintDigit === undefined ? (
            <ThreadTimeContent isStreaming={isStreaming} thread={thread} />
          ) : (
            <ThreadHotkeyHint digit={hintDigit} />
          )}
        </button>
      )}
      {selectionError?.threadId === thread.id ? (
        <DismissibleErrorRow
          message={selectionError.message}
          onDismiss={actions.dismissSelectionError}
        />
      ) : null}
    </div>
  )
}

const ThreadSidebarReact = ({
  actions,
  activeThreadId,
  currentState,
  data,
  groups,
  hotkeyHintDigits,
  hotkeyHintsVisible,
  includeHomeSessions,
  isThreadStreaming: isThreadStreamingForId,
  renamingThreadDraft,
  renamingThreadId,
  searchOpen,
  searchShouldFocus,
  searchTerm,
  selectingThreadId,
  selectionError,
}: SidebarReactProps): React.JSX.Element => (
  <>
    <div className="khala-thread-sidebar-header flex items-center justify-between gap-3">
      <h2 className="khala-thread-sidebar-title truncate">Chat</h2>
      <div className="khala-thread-sidebar-header-actions inline-flex items-center">
        <button
          aria-controls="khala-thread-sidebar-search-flyout"
          aria-expanded={searchOpen ? "true" : "false"}
          aria-label={searchOpen ? "Close thread search" : "Search threads"}
          className="khala-thread-sidebar-search-toggle"
          data-active={searchTerm.length > 0 ? "true" : undefined}
          onClick={actions.toggleSearch}
          title={searchOpen ? "Close thread search" : "Search threads"}
          type="button"
        >
          <SidebarIcon icon="Search" label="Search threads" />
          {srOnly("Search threads")}
        </button>
        <button
          aria-label={includeHomeSessions ? "Showing all home sessions" : "Show all home sessions"}
          aria-pressed={includeHomeSessions ? "true" : "false"}
          className="khala-thread-sidebar-home-toggle"
          data-active={includeHomeSessions ? "true" : undefined}
          onClick={actions.toggleHomeSessions}
          title={includeHomeSessions ? "Showing all home sessions" : "Show all home sessions"}
          type="button"
        >
          <SidebarIcon icon="History" label="Home sessions" />
          {srOnly("Home sessions")}
        </button>
        <button
          aria-label="New thread"
          className="khala-thread-sidebar-new"
          onClick={actions.startNewChat}
          title="New thread"
          type="button"
        >
          <SidebarIcon icon="Plus" label="New thread" />
          {srOnly("New thread")}
        </button>
      </div>
    </div>

    {searchOpen ? (
      <SearchFlyout
        actions={actions}
        searchShouldFocus={searchShouldFocus}
        searchTerm={searchTerm}
      />
    ) : null}

    {currentState.phase === "idle" || data === undefined ? (
      <p className="khala-thread-sidebar-empty">Loading threads</p>
    ) : (
      <>
        {currentState.phase === "error" ? (
          <DismissibleErrorRow
            className="khala-thread-sidebar-error"
            message={currentState.message}
            onDismiss={actions.dismissGlobalError}
          />
        ) : null}
        {(data.threads ?? []).length === 0 ? (
          <p className="khala-thread-sidebar-empty">No threads</p>
        ) : (
          groups.map(group =>
            group.threads.length === 0 ? null : (
              <section className="khala-thread-sidebar-group grid" key={group.label}>
                {group.label.trim().length > 0 ? (
                  <h3 className="khala-thread-sidebar-group-title truncate">{group.label}</h3>
                ) : null}
                <div className="khala-thread-sidebar-list grid">
                  {group.threads.map(thread => (
                    <ThreadItem
                      key={thread.id}
                      actions={actions}
                      activeThreadId={activeThreadId}
                      hotkeyHintDigits={hotkeyHintDigits}
                      hotkeyHintsVisible={hotkeyHintsVisible}
                      isStreaming={isThreadStreamingForId(thread.id)}
                      renamingThreadDraft={renamingThreadDraft}
                      renamingThreadId={renamingThreadId}
                      selectingThreadId={selectingThreadId}
                      selectionError={selectionError}
                      thread={thread}
                    />
                  ))}
                </div>
              </section>
            ),
          )
        )}
      </>
    )}
  </>
)

const epochNowMs = (): number =>
  Math.trunc(performance.timeOrigin + performance.now())

// Empty by design: the optimistic active row is indicated by its background only.
const pendingActiveThreadGroupLabel = ""

export const mountCodexThreadSidebar = (
  container: HTMLElement,
  options: CodexThreadSidebarOptions,
): CodexThreadSidebarHandle => {
  let searchOpen = false
  let searchShouldFocus = false
  let searchTerm = ""
  let includeHomeSessions = false
  let state: ViewState = { phase: "idle" }
  let isVisible = false
  let hotkeyHintsVisible = false
  let hotkeyHintDigits: ReadonlyMap<string, number> = new Map()
  let activeThreadId = options.activeThreadId()
  let selectingThreadId: string | null = null
  let selectionError: SelectionError | null = null
  let renamingThreadId: string | null = null
  let renamingThreadDraft = ""
  let refreshSequence = 0
  let selectionSequence = 0
  let reactRoot: Root | null = null
  const optimisticThreadTitles = new Map<string, string>()
  const optimisticThreads = new Map<string, KhalaCodeDesktopCodexThreadSummary>()
  const threadMenu = createBasecoatContextMenu({
    id: "khala-thread-sidebar-thread-menu",
    ownerDocument: container.ownerDocument,
    className: "khala-thread-sidebar-menu",
  })

  const root = (): Root => {
    reactRoot ??= createRoot(container)
    return reactRoot
  }

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
    const thread = dataForState(state)?.threads?.find(candidate => candidate.id === threadId)
    if (thread !== undefined && !isThreadResumable(thread)) {
      selectionError = {
        threadId,
        message: thread.unavailableReason ?? "This stored session record is not resumable.",
      }
      render()
      return false
    }
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
  ): BasecoatMenuDomContent => {
    const copySessionIdItem = {
      id: "copy-session-id",
      label: "Copy session ID",
      icon: "Copy" as const,
      onSelect: () => {
        const sessionId = thread.sessionId ?? thread.id
        void navigator.clipboard?.writeText(sessionId).catch(() => undefined)
      },
    }
    return {
      label: `Thread actions for ${thread.title}`,
      sections: [
        {
          items: isThreadResumable(thread)
            ? [
                {
                  id: "rename-thread",
                  label: "Rename thread",
                  icon: "Pencil" as const,
                  onSelect: () => beginRename(thread),
                },
                {
                  id: "fork-thread",
                  label: "Fork thread",
                  icon: "BranchAlt" as const,
                  onSelect: () => void runMutation(() => options.forkThread(thread.id)),
                },
                copySessionIdItem,
                {
                  id: "archive-thread",
                  label: "Archive thread",
                  icon: "Archive" as const,
                  onSelect: () => void runMutation(() => options.archiveThread(thread.id)),
                },
                {
                  id: "delete-thread",
                  label: "Delete thread",
                  icon: "Trash" as const,
                  destructive: true,
                  onSelect: () => {
                    if (!confirm(`Delete ${thread.title}?`)) return
                    void runMutation(() => options.deleteThread(thread.id))
                  },
                },
              ]
            : [copySessionIdItem],
        },
      ],
    }
  }

  const openThreadMenu = (
    thread: KhalaCodeDesktopCodexThreadSummary,
    point: BasecoatMenuDomPoint,
  ): void => {
    threadMenu.openAt(point, threadMenuContent(thread))
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

  const updateSearch = (value: string): void => {
    const nextSearchTerm = value.trim()
    if (searchTerm === nextSearchTerm) return
    searchTerm = nextSearchTerm
    void refresh()
  }

  const toggleHomeSessions = (): void => {
    includeHomeSessions = !includeHomeSessions
    void refresh()
  }

  const dismissGlobalError = (): void => {
    const data = dataForState(state)
    state = data === undefined ? { phase: "idle" } : { phase: "ready", data }
    render()
  }

  const dismissSelectionError = (): void => {
    selectionError = null
    render()
  }

  const actions: SidebarActions = {
    beginRename,
    cancelRename,
    dismissGlobalError,
    dismissSelectionError,
    openThreadMenu,
    refresh: () => void refresh(),
    selectThread: threadId => void selectThread(threadId, "sidebar"),
    startNewChat,
    submitRename,
    toggleHomeSessions,
    toggleSearch,
    updateSearch,
  }

  function render(): void {
    const currentState = state
    const data = dataForState(currentState)
    hotkeyHintDigits = hotkeyHintsVisible
      ? recentThreadHotkeyHintDigits(data?.threads ?? [])
      : new Map()
    const shouldFocusSearch = searchShouldFocus
    flushSync(() => {
      root().render(
        <ThreadSidebarReact
          actions={actions}
          activeThreadId={activeThreadId}
          currentState={currentState}
          data={data}
          groups={data === undefined ? [] : groupThreads(data)}
          hotkeyHintDigits={hotkeyHintDigits}
          hotkeyHintsVisible={hotkeyHintsVisible}
          includeHomeSessions={includeHomeSessions}
          isThreadStreaming={threadId => isThreadStreaming(threadId, options)}
          renamingThreadDraft={renamingThreadDraft}
          renamingThreadId={renamingThreadId}
          searchOpen={searchOpen}
          searchShouldFocus={shouldFocusSearch}
          searchTerm={searchTerm}
          selectingThreadId={selectingThreadId}
          selectionError={selectionError}
        />,
      )
    })
    if (shouldFocusSearch) searchShouldFocus = false
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
      const wasVisible = isVisible
      isVisible = nextVisible
      container.hidden = !isVisible
      if (!isVisible) {
        threadMenu.close()
        renamingThreadId = null
        renamingThreadDraft = ""
      }
      if (isVisible && state.phase === "idle") {
        void refresh()
      } else if (wasVisible !== isVisible) {
        render()
      }
    },
    upsertPendingThread(input) {
      const preview = input.preview.trim()
      if (input.threadId.length === 0 || preview.length === 0) return
      const now = epochNowMs()
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
