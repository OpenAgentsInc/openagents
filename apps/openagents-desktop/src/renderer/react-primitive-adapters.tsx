import {
  StrictMode,
  createContext,
  createElement,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type FormEvent,
  type ButtonHTMLAttributes,
  type RefObject,
  type ReactElement,
} from "react"
import { createRoot, type Root } from "react-dom/client"
import {
  ComponentValueBinding,
  Frame,
  IntentRef,
  type FrameView,
  type IntentError,
  type IntentReporter,
  type JsonPayload,
} from "@effect-native/core"
import { Effect, Scope, Stream } from "@effect-native/core/effect"
import { mountDomThemeStyleSheet } from "@effect-native/render-dom"
import {
  ArrowDown,
  ArrowUp,
  Archive,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  Download,
  FileDiff,
  Folder,
  FolderGit2,
  GitBranch,
  Globe2,
  LoaderCircle,
  Maximize2,
  Minimize2,
  Plus,
  RefreshCw,
  RotateCcw,
  Search,
  TerminalSquare,
  Trash2,
  X,
} from "lucide-react"
import {
  DesktopConversation,
  DesktopConversationHeader,
  ContextMeter,
  DesktopRailScrim,
  DesktopSessionRail,
  DesktopSidebarExpand,
  DesktopWorkbench,
  type DesktopConversationHeaderMeter,
  type DesktopRailDestination,
  type DesktopRailIcon,
} from "@openagentsinc/ui/desktop-workbench"
import {
  ReactSurfaceErrorBoundary,
  makeReactViewStore,
  renderReactDomView,
  type ReactViewStore,
} from "@effect-native/render-dom/react"
import { type Theme } from "@effect-native/tokens"
import { openagentsDesktopTheme } from "./theme.ts"
import { Button } from "#components/ui/button"
import { Input } from "#components/ui/input"
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from "#components/ui/context-menu"
import { Alert, AlertDescription, AlertTitle } from "#components/ui/alert"
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from "#components/ui/dialog"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "#components/ui/tooltip"
import type { DesktopShellState } from "./shell.ts"
import { capabilityForActiveLane, desktopConversationShortcutLabel, formatRelativeTimestamp } from "./shell.ts"
import { DecisionSurface, ReactCommandPalette, ReactComposer } from "./react-composer.tsx"
import { StatusNotices } from "./react-review.tsx"
import { ConversationTimeline, SafeReactMarkdown } from "./react-timeline.tsx"
import { projectBootSequenceAgents, projectBootSequenceIdentity } from "./boot-sequence.ts"
import { DESKTOP_STAGE_LABEL } from "./branding.ts"
import { projectDesktopSidebarDestinations } from "./sidebar-destinations.ts"
import { ReactBrowserPreviewSurface, ReactFilesSidebar, ReactReviewSurface, ReactTerminalSurface, ReactWorkspaceEditor } from "./react-workspace-surfaces.tsx"
import { ReactSettingsSurface, type ReactSettingsSectionId } from "./react-settings-surface.tsx"
import { ReactFullAutoSurface } from "./react-full-auto-surface.tsx"
import {
  decodeDesktopSurfaceLayout,
  defaultDesktopSurfaceLayout,
  defaultDesktopSurfacePanelWidth,
  desktopSurfaceLayoutStorageKey,
  reduceDesktopSurfaceLayout,
  type DesktopSurfaceKind,
  type DesktopSurfaceLayoutAction,
} from "./surface-layout.ts"

export type ReactSessionRow = Readonly<{
  id: string
  title: string
  createdAt: string
  updatedAt: string
  meta: string
  working: boolean
  source: "local" | "history" | "search"
  selected: boolean
  intent: "DesktopChatSelected" | "HistoryConversationSelected" | "HistorySearchResultOpened"
}>

const normalized = (value: string): string => value.trim().toLocaleLowerCase()

/** Metadata-only projection: no transcript read is required to paint the rail. */
export const projectReactSessionRows = (
  state: DesktopShellState,
  now: Date = new Date(),
): ReadonlyArray<ReactSessionRow> => {
  const query = normalized(state.history.searchQuery)
  const selectedHistoryRef = state.history.pendingThreadRef ?? state.history.page?.rootThreadRef ?? null
  const local = state.threads
    .filter(thread => query === "" || normalized(thread.title).includes(query))
    .map((thread): ReactSessionRow => ({
      id: thread.id,
      title: thread.title || "Untitled session",
      createdAt: thread.createdAt ?? thread.updatedAt,
      updatedAt: thread.updatedAt,
      meta: formatRelativeTimestamp(thread.updatedAt, now),
      working: state.pendingByThread[thread.id] === true || state.fullAutoLiveByThread[thread.id]?.state === "turn_running",
      source: "local",
      selected: selectedHistoryRef === null && state.activeThreadId === thread.id,
      intent: "DesktopChatSelected",
    }))
  const history: ReadonlyArray<ReactSessionRow> = query === ""
    ? state.history.catalog.roots.slice(0, state.history.visibleRootCount).filter(row => row.source === "codex").map((row): ReactSessionRow => ({
      id: row.threadRef,
      title: row.title || "Untitled session",
      createdAt: row.createdAt,
      updatedAt: row.updatedAt,
      meta: formatRelativeTimestamp(row.updatedAt, now),
      working: false,
      selected: selectedHistoryRef === row.threadRef,
      source: "history",
      intent: "HistoryConversationSelected",
    }))
    : state.history.searchResults.filter(row => row.source === "codex").map((row): ReactSessionRow => ({
      id: row.threadRef,
      title: row.title || "Untitled session",
      createdAt: state.history.catalog.roots.find(root => root.threadRef === row.threadRef)?.createdAt ?? row.updatedAt,
      updatedAt: row.updatedAt,
      meta: formatRelativeTimestamp(row.updatedAt, now),
      working: false,
      selected: selectedHistoryRef === row.threadRef,
      source: "search",
      intent: "HistorySearchResultOpened",
    }))
  const seen = new Set<string>()
  const rows = [...local, ...history]
    .sort((left, right) => right.createdAt.localeCompare(left.createdAt) || left.id.localeCompare(right.id))
    .filter(row => seen.has(row.id) ? false : (seen.add(row.id), true))
  return rows.map((row, index) => ({
    ...row,
    meta: state.historyShortcutHintsVisible && state.history.searchQuery.trim() === ""
      ? desktopConversationShortcutLabel(state, index)
      : row.meta,
  }))
}

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(report(
    payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()), payload,
  ) as Effect.Effect<void, IntentError>).catch((error: unknown) => {
    console.error(
      "[openagents-desktop] React intent failed",
      name,
      error instanceof Error ? error.message : "unknown intent error",
    )
  })
}

/**
 * Projects only the active thread's provider rate-limit windows into the rail
 * footer. Per-turn token and context counts remain runtime data but are not
 * sidebar chrome. No observed windows means no footer rather than invented
 * usage state.
 */
export const projectSidebarMeter = (state: DesktopShellState): DesktopConversationHeaderMeter | undefined => {
  const rateLimits = state.meter?.rateLimits
  return rateLimits === undefined || rateLimits.length === 0 ? undefined : { rateLimits }
}

type CodingSession = DesktopShellState["codingCatalog"]["sessions"][number]
export type ReactCodingProjectGroup = Readonly<{
  projectRef: string
  label: string
  status: "active" | "idle" | "recovery" | "archived"
  sessions: ReadonlyArray<CodingSession>
}>
export type ReactCodingProjectSort = "recent" | "name" | "manual"

const codingSessionStatus = (session: CodingSession): ReactCodingProjectGroup["status"] =>
  session.recoveryReason !== null || session.state === "recovery_required"
    ? "recovery"
    : session.state

const codingStatusRank: Readonly<Record<ReactCodingProjectGroup["status"], number>> = {
  recovery: 4,
  active: 3,
  idle: 2,
  archived: 1,
}

/** Project/worktree presentation over the already-admitted device-local catalog. */
export const projectReactCodingGroups = (
  state: DesktopShellState,
  sort: ReactCodingProjectSort,
  manualOrder: ReadonlyArray<string>,
): ReadonlyArray<ReactCodingProjectGroup> => {
  const visible = state.codingCatalog.sessions.filter(session =>
    state.codingSessionFilter === "active"
      ? session.state === "active" || session.state === "idle"
      : state.codingSessionFilter === "recovery"
        ? codingSessionStatus(session) === "recovery"
        : session.state === "archived")
  const manualIndex = new Map(manualOrder.map((ref, index) => [ref, index]))
  const compare = (left: CodingSession, right: CodingSession): number => sort === "name"
    ? `${left.repositoryLabel}/${left.worktreeLabel}`.localeCompare(`${right.repositoryLabel}/${right.worktreeLabel}`)
    : sort === "manual"
      ? (manualIndex.get(left.sessionRef) ?? Number.MAX_SAFE_INTEGER) - (manualIndex.get(right.sessionRef) ?? Number.MAX_SAFE_INTEGER)
      : right.lastActiveAt.localeCompare(left.lastActiveAt)
  const grouped = new Map<string, CodingSession[]>()
  for (const session of visible) {
    const sessions = grouped.get(session.projectRef) ?? []
    sessions.push(session)
    grouped.set(session.projectRef, sessions)
  }
  return [...grouped.entries()].map(([projectRef, sessions]) => {
    const ordered = [...sessions].sort(compare)
    const status = ordered.map(codingSessionStatus).sort((left, right) => codingStatusRank[right] - codingStatusRank[left])[0] ?? "idle"
    return { projectRef, label: ordered[0]?.projectLabel ?? "Project unavailable", status, sessions: ordered }
  }).sort((left, right) => sort === "name"
    ? left.label.localeCompare(right.label)
    : sort === "manual"
      ? (manualIndex.get(left.sessions[0]?.sessionRef ?? "") ?? Number.MAX_SAFE_INTEGER) - (manualIndex.get(right.sessions[0]?.sessionRef ?? "") ?? Number.MAX_SAFE_INTEGER)
      : (right.sessions[0]?.lastActiveAt ?? "").localeCompare(left.sessions[0]?.lastActiveAt ?? ""))
}

const CodingProjectSection = ({ state, report, onDismiss }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
  readonly onDismiss: () => void
}): ReactElement => {
  const [sort, setSort] = useState<ReactCodingProjectSort>("recent")
  const [manualOrder, setManualOrder] = useState<ReadonlyArray<string>>(() => state.codingCatalog.sessions.map(session => session.sessionRef))
  const [expanded, setExpanded] = useState<ReadonlyArray<string>>(() => state.codingCatalog.sessions.map(session => session.projectRef))
  const [selected, setSelected] = useState<ReadonlyArray<string>>([])
  useEffect(() => {
    setManualOrder(current => [
      ...current.filter(ref => state.codingCatalog.sessions.some(session => session.sessionRef === ref)),
      ...state.codingCatalog.sessions.map(session => session.sessionRef).filter(ref => !current.includes(ref)),
    ])
    setExpanded(current => [
      ...current.filter(ref => state.codingCatalog.sessions.some(session => session.projectRef === ref)),
      ...state.codingCatalog.sessions.map(session => session.projectRef).filter(ref => !current.includes(ref)),
    ])
    setSelected(current => current.filter(ref => state.codingCatalog.sessions.some(session => session.sessionRef === ref)))
  }, [state.codingCatalog.sessions])
  const groups = useMemo(() => projectReactCodingGroups(state, sort, manualOrder), [manualOrder, sort, state])
  const move = (sessionRef: string, direction: -1 | 1): void => setManualOrder(current => {
    const index = current.indexOf(sessionRef)
    const target = index + direction
    if (index < 0 || target < 0 || target >= current.length) return current
    const next = [...current]
    const [entry] = next.splice(index, 1)
    if (entry === undefined) return current
    next.splice(target, 0, entry)
    return next
  })
  const toggleSelected = (sessionRef: string): void => setSelected(current =>
    current.includes(sessionRef) ? current.filter(ref => ref !== sessionRef) : [...current, sessionRef])
  const batch = (intent: "DesktopCodingSessionArchived" | "DesktopCodingSessionRecovered"): void => {
    for (const sessionRef of selected) dispatch(report, intent, sessionRef)
    setSelected([])
  }
  return <section className="oa-react-projects" aria-labelledby="oa-react-projects-heading">
    <header className="oa-react-projects-header">
      <strong id="oa-react-projects-heading">Projects</strong>
      <Button type="button" variant="ghost" size="icon-sm" aria-label="Choose project or worktree"
        title="Choose project or worktree" onClick={() => dispatch(report, "DesktopCodingCatalogChooseRequested")}>
        <FolderGit2 aria-hidden="true" />
      </Button>
    </header>
    <div className="oa-react-project-controls">
      {(["active", "recovery", "archived"] as const).map(filter => <button
        aria-pressed={state.codingSessionFilter === filter}
        key={filter}
        onClick={() => dispatch(report, "DesktopCodingCatalogFilterSelected", filter)}
        type="button"
      >{filter === "active" ? "Open" : filter === "recovery" ? "Recover" : "Archived"}</button>)}
      <label>
        <span className="oa-react-sr-only">Sort projects</span>
        <select aria-label="Sort projects" onChange={event => setSort(event.currentTarget.value as ReactCodingProjectSort)} value={sort}>
          <option value="recent">Recent</option>
          <option value="name">Name</option>
          <option value="manual">Manual</option>
        </select>
      </label>
    </div>
    {selected.length === 0 ? null : <div className="oa-react-project-selection" role="toolbar" aria-label={`${selected.length} selected worktrees`}>
      <span>{selected.length} selected</span>
      {state.codingSessionFilter === "recovery"
        ? <button type="button" onClick={() => batch("DesktopCodingSessionRecovered")}><RotateCcw aria-hidden="true" />Recover</button>
        : state.codingSessionFilter === "archived"
          ? null
        : <button type="button" onClick={() => batch("DesktopCodingSessionArchived")}><Archive aria-hidden="true" />Archive</button>}
      <button type="button" onClick={() => setSelected([])}>Clear</button>
    </div>}
    <div className="oa-react-project-groups">
      {groups.length === 0 ? <p>{state.codingSessionFilter === "recovery" ? "No worktrees need recovery." : state.codingSessionFilter === "archived" ? "No archived worktrees." : "Choose a project to start a coding session."}</p> : groups.map(group => {
        const isExpanded = expanded.includes(group.projectRef)
        return <section className="oa-react-project-group" data-project-status={group.status} key={group.projectRef}>
          <button className="oa-react-project-group-trigger" type="button" aria-expanded={isExpanded}
            onClick={() => setExpanded(current => isExpanded ? current.filter(ref => ref !== group.projectRef) : [...current, group.projectRef])}>
            {isExpanded ? <ChevronDown aria-hidden="true" /> : <ChevronRight aria-hidden="true" />}
            <span className="oa-react-project-status-dot" aria-hidden="true" />
            <strong>{group.label}</strong>
            <small>{group.sessions.length}</small>
          </button>
          {!isExpanded ? null : <div className="oa-react-worktree-list">
            {group.sessions.map(session => {
              const status = codingSessionStatus(session)
              const deleting = state.codingSessionDeleteConfirmRef === session.sessionRef
              return <div className="oa-react-worktree-row" data-selected={state.codingCatalog.selectedSessionRef === session.sessionRef ? "true" : "false"} data-status={status} key={session.sessionRef}>
                <input checked={selected.includes(session.sessionRef)} type="checkbox" aria-label={`Select ${session.repositoryLabel} ${session.worktreeLabel}`}
                  onChange={() => toggleSelected(session.sessionRef)} />
                <button className="oa-react-worktree-open" type="button" onClick={() => { dispatch(report, "DesktopCodingSessionOpened", session.sessionRef); onDismiss() }}>
                  <span><GitBranch aria-hidden="true" />{session.worktreeLabel}</span>
                  <small>{session.repositoryLabel} · {status === "recovery" ? "needs recovery" : status} · {formatRelativeTimestamp(session.lastActiveAt)}</small>
                </button>
                {sort !== "manual" ? null : <span className="oa-react-worktree-order">
                  <button type="button" aria-label={`Move ${session.worktreeLabel} up`} onClick={() => move(session.sessionRef, -1)}><ArrowUp aria-hidden="true" /></button>
                  <button type="button" aria-label={`Move ${session.worktreeLabel} down`} onClick={() => move(session.sessionRef, 1)}><ArrowDown aria-hidden="true" /></button>
                </span>}
                {status === "recovery" ? <button className="oa-react-worktree-action" type="button" aria-label={`Recover ${session.worktreeLabel}`} onClick={() => dispatch(report, "DesktopCodingSessionRecovered", session.sessionRef)}><RotateCcw aria-hidden="true" /></button>
                  : session.state === "archived"
                    ? <button className="oa-react-worktree-action" type="button" aria-label={`Delete ${session.worktreeLabel}`} onClick={() => dispatch(report, "DesktopCodingSessionDeleteRequested", session.sessionRef)}><Trash2 aria-hidden="true" /></button>
                    : <button className="oa-react-worktree-action" type="button" aria-label={`Archive ${session.worktreeLabel}`} onClick={() => dispatch(report, "DesktopCodingSessionArchived", session.sessionRef)}><Archive aria-hidden="true" /></button>}
                {deleting ? <span className="oa-react-worktree-delete-confirm">
                  <button type="button" onClick={() => dispatch(report, "DesktopCodingSessionDeleteConfirmed", session.sessionRef)}>Delete</button>
                  <button type="button" onClick={() => dispatch(report, "DesktopCodingSessionDeleteCancelled")}>Cancel</button>
                </span> : null}
              </div>
            })}
          </div>}
        </section>
      })}
    </div>
    {state.codingCatalog.nextOffset === null ? null : <button className="oa-react-project-load-more" type="button" onClick={() => dispatch(report, "DesktopCodingCatalogMoreRequested")}>Load more worktrees</button>}
  </section>
}

const SurfaceOpenContext = createContext<(surface: DesktopSurfaceKind) => void>(() => undefined)

export const ConversationHeader = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement => {
  const workingDirectory = state.workingDirectory
  return <header className="oa-react-conversation-header oa-react-conversation-header--bare">
    <button
      type="button"
      className="oa-react-conversation-working-directory"
      aria-label={workingDirectory === null
        ? "Working directory unavailable"
        : `Working directory: ${workingDirectory}. Change working directory`}
      title={workingDirectory ?? "Working directory unavailable"}
      onClick={() => dispatch(report, "DesktopWorkspacePickerRequested")}
    >
      <Folder aria-hidden="true" data-icon-name="Folder" />
      <code>{workingDirectory ?? "Working directory unavailable"}</code>
    </button>
  </header>
}

const FilesModeHeader = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement => {
  const browser = state.workspaceBrowser
  const selectedCoding = state.codingCatalog.sessions.find(
    session => session.sessionRef === state.codingCatalog.selectedSessionRef,
  )
  const lifecycle = browser.phase === "loading" ? "Loading" : browser.phase === "unavailable" ? "Unavailable" : "Ready"
  return <DesktopConversationHeader
    lifecycle={lifecycle}
    secondary={selectedCoding === undefined ? undefined : `${selectedCoding.repositoryLabel} / ${selectedCoding.worktreeLabel}`}
    title="Files"
    actions={<form className="oa-react-files-mode-actions" aria-label="Files controls" onSubmit={event => { event.preventDefault(); dispatch(report, "WorkspaceBrowserSearchRequested") }}>
      <label className="oa-react-files-mode-search">
        <Search aria-hidden="true" />
        <Input aria-label="Search workspace files" placeholder="Search files" value={browser.query}
          onChange={event => dispatch(report, "WorkspaceBrowserQueryChanged", event.currentTarget.value)} />
      </label>
      <div className="oa-react-files-mode-search-kinds" aria-label="Search mode">
        {(["path", "content"] as const).map(mode => <button aria-pressed={browser.searchMode === mode} key={mode} onClick={() => dispatch(report, "WorkspaceBrowserSearchModeSelected", mode)} type="button">{mode === "path" ? "Names" : "Contents"}</button>)}
      </div>
      <Button size="icon-sm" variant="ghost" type="button" aria-label="Refresh workspace files" onClick={() => dispatch(report, "WorkspaceBrowserRefreshRequested")}><RefreshCw aria-hidden="true" /></Button>
      <Button size="icon-sm" variant="ghost" type="button" aria-label="Close Files" onClick={() => dispatch(report, "DesktopFilesModeToggled")}><X aria-hidden="true" /></Button>
    </form>}
  />
}

const surfaceLabel = (surface: DesktopSurfaceKind): string => surface === "review" ? "Review" : surface === "terminal" ? "Terminal" : "Preview"
const SurfaceIcon = ({ surface }: { readonly surface: DesktopSurfaceKind }): ReactElement => surface === "review"
  ? <FileDiff aria-hidden="true" />
  : surface === "terminal"
    ? <TerminalSquare aria-hidden="true" />
    : <Globe2 aria-hidden="true" />

const SurfacePanelContent = ({ state, surface, report }: {
  readonly state: DesktopShellState
  readonly surface: DesktopSurfaceKind
  readonly report: IntentReporter
}): ReactElement => surface === "review"
    ? <ReactReviewSurface state={state} report={report} />
    : surface === "terminal"
      ? <ReactTerminalSurface state={state} report={report} />
      : <ReactBrowserPreviewSurface state={state} report={report} />

export const DesktopSurfaceManager = ({ state, report, conversation }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
  readonly conversation: ReactElement
}): ReactElement => {
  const scope = state.codingCatalog.selectedSessionRef ?? "unbound"
  const storageKey = `${desktopSurfaceLayoutStorageKey}:${scope}`
  const readLayout = (): ReturnType<typeof defaultDesktopSurfaceLayout> => {
    try {
      const stored = window.localStorage.getItem(storageKey)
      return stored === null ? defaultDesktopSurfaceLayout() : decodeDesktopSurfaceLayout(JSON.parse(stored))
    } catch {
      return defaultDesktopSurfaceLayout()
    }
  }
  const [layout, setLayout] = useState(readLayout)
  const [addOpen, setAddOpen] = useState(false)
  const update = (action: DesktopSurfaceLayoutAction): void => setLayout(current => reduceDesktopSurfaceLayout(current, action))
  useEffect(() => setLayout(readLayout()), [storageKey])
  useEffect(() => {
    try { window.localStorage.setItem(storageKey, JSON.stringify(layout)) } catch { /* renderer storage may be disabled */ }
  }, [layout, storageKey])
  useEffect(() => {
    if (state.workspace === "review") update({ type: "open", surface: "review" })
  }, [state.workspace])
  const activate = (surface: DesktopSurfaceKind): void => {
    update({ type: "open", surface })
    if (surface === "terminal") {
      if (state.terminal.sessions.length === 0) dispatch(report, "TerminalCreateRequested")
    } else if (surface !== "browser") {
      dispatch(report, "DesktopWorkspaceSelected", surface)
    }
    setAddOpen(false)
  }
  const close = (surface: DesktopSurfaceKind, action: "close" | "close_others" | "close_right"): void => {
    const next = reduceDesktopSurfaceLayout(layout, { type: action, surface })
    setLayout(next)
    if (next.active === null) dispatch(report, "DesktopWorkspaceSelected", "chat")
    else if (next.active !== "terminal" && next.active !== "browser") dispatch(report, "DesktopWorkspaceSelected", next.active)
  }
  const closeAll = (): void => {
    update({ type: "close_all" })
    dispatch(report, "DesktopWorkspaceSelected", "chat")
  }
  const active = layout.active
  return <div className="oa-react-surface-layout" data-maximized={layout.maximized ? "true" : "false"}>
    <div className="oa-react-chat-column"><SurfaceOpenContext.Provider value={activate}>{conversation}</SurfaceOpenContext.Provider></div>
    {active === null ? null : <>
      <button
        aria-label="Resize workbench panel"
        aria-orientation="vertical"
        aria-valuemax={960}
        aria-valuemin={320}
        aria-valuenow={layout.width}
        className="oa-react-surface-resize"
        onDoubleClick={() => update({ type: "resize", width: defaultDesktopSurfacePanelWidth })}
        onKeyDown={event => {
          if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return
          event.preventDefault()
          update({ type: "resize", width: layout.width + (event.key === "ArrowLeft" ? 24 : -24) })
        }}
        onPointerMove={event => {
          if (event.buttons !== 1) return
          update({ type: "resize", width: window.innerWidth - event.clientX })
        }}
        role="separator"
        type="button"
      />
      <aside className="oa-react-surface-panel" style={layout.maximized ? undefined : { width: layout.width }}>
        <header className="oa-react-surface-tabs">
          <div role="tablist" aria-label="Workbench surfaces" onKeyDown={event => {
            if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key) || layout.surfaces.length === 0) return
            event.preventDefault()
            const current = Math.max(0, layout.surfaces.indexOf(active))
            const next = event.key === "Home" ? 0 : event.key === "End" ? layout.surfaces.length - 1
              : (current + (event.key === "ArrowRight" ? 1 : -1) + layout.surfaces.length) % layout.surfaces.length
            activate(layout.surfaces[next]!)
            event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"] > button:first-child')[next]?.focus()
          }}>
            {layout.surfaces.map(surface => <ContextMenu key={surface}>
              <ContextMenuTrigger render={<div aria-selected={active === surface} role="tab">
                <button onClick={() => activate(surface)} tabIndex={active === surface ? 0 : -1} type="button"><SurfaceIcon surface={surface} /><span>{surfaceLabel(surface)}</span></button>
                <button
                aria-label={`Close ${surfaceLabel(surface)}`}
                onClick={event => { event.stopPropagation(); close(surface, "close") }}
                type="button"
              ><X aria-hidden="true" /></button></div>} />
              <ContextMenuContent aria-label={`${surfaceLabel(surface)} tab actions`}>
                <ContextMenuItem onClick={() => close(surface, "close")}>Close</ContextMenuItem>
                <ContextMenuItem onClick={() => close(surface, "close_others")}>Close others</ContextMenuItem>
                <ContextMenuItem disabled={layout.surfaces.at(-1) === surface} onClick={() => close(surface, "close_right")}>Close to the right</ContextMenuItem>
                <ContextMenuItem onClick={closeAll}>Close all</ContextMenuItem>
              </ContextMenuContent>
            </ContextMenu>)}
          </div>
          <div className="oa-react-surface-tab-actions">
            <button aria-expanded={addOpen} aria-label="Add surface" onClick={() => setAddOpen(open => !open)} type="button"><Plus aria-hidden="true" /></button>
            <button aria-label={layout.maximized ? "Restore panel size" : "Maximize panel"} aria-pressed={layout.maximized} onClick={() => update({ type: "toggle_maximized" })} type="button">{layout.maximized ? <Minimize2 aria-hidden="true" /> : <Maximize2 aria-hidden="true" />}</button>
            <button aria-label="Close panel" onClick={closeAll} type="button"><X aria-hidden="true" /></button>
          </div>
          {!addOpen ? null : <div className="oa-react-surface-add" role="menu">
            {(["review", "terminal", "browser"] as const).filter(surface => surface !== "browser" || state.terminal.sessions.some(session => session.previews.length > 0)).map(surface => <button disabled={layout.surfaces.includes(surface)} key={surface} onClick={() => activate(surface)} role="menuitem" type="button"><SurfaceIcon surface={surface} />{surfaceLabel(surface)}</button>)}
          </div>}
        </header>
        <SurfacePanelContent state={state} surface={active} report={report} />
      </aside>
    </>}
  </div>
}

const sharedRailIcon = (icon: "ChatCompose" | "Chats" | "Settings" | "Zap"): DesktopRailIcon => {
  if (icon === "ChatCompose") return "new-session"
  if (icon === "Chats") return "chat"
  if (icon === "Zap") return "zap"
  return "settings"
}

export const SessionRail = ({ state, report, open, onCollapse, onDismiss, railRef, selectedSettingsSectionId, onSettingsSectionSelect }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
  readonly open: boolean
  readonly onCollapse: () => void
  readonly onDismiss: () => void
  readonly railRef: RefObject<HTMLElement | null>
  readonly selectedSettingsSectionId: ReactSettingsSectionId
  readonly onSettingsSectionSelect: (sectionId: ReactSettingsSectionId) => void
}): ReactElement => {
  const rows = projectReactSessionRows(state)
  const destinations = projectDesktopSidebarDestinations(
    state.workspace === "settings" ? "settings" : state.workspace === "full-auto" ? "full-auto" : "chat",
    rows.some(row => row.selected),
  )
  const primaryDestinations = destinations.filter(destination => destination.id !== "shell-settings-toggle")
  const settingsDestination = destinations.find(destination => destination.id === "shell-settings-toggle")
  const settingsSections: ReadonlyArray<DesktopRailDestination> = state.workspace !== "settings" ? [] : [
    { id: "settings-general", label: "General", icon: "general", selected: selectedSettingsSectionId === "settings-general", current: selectedSettingsSectionId === "settings-general" ? "page" : undefined },
    { id: "settings-codex", label: "Codex CLI", icon: "settings", selected: selectedSettingsSectionId === "settings-codex", current: selectedSettingsSectionId === "settings-codex" ? "page" : undefined },
    { id: "settings-extensions", label: "Extensions", icon: "general", selected: selectedSettingsSectionId === "settings-extensions", current: selectedSettingsSectionId === "settings-extensions" ? "page" : undefined },
    { id: "settings-source-control", label: "Source control", icon: "general", selected: selectedSettingsSectionId === "settings-source-control", current: selectedSettingsSectionId === "settings-source-control" ? "page" : undefined },
    { id: "settings-keybindings", label: "Keybindings", icon: "settings", selected: selectedSettingsSectionId === "settings-keybindings", current: selectedSettingsSectionId === "settings-keybindings" ? "page" : undefined },
    { id: "settings-diagnostics", label: "Diagnostics", icon: "privacy", selected: selectedSettingsSectionId === "settings-diagnostics", current: selectedSettingsSectionId === "settings-diagnostics" ? "page" : undefined },
    { id: "settings-connections", label: "Connections", icon: "general", selected: selectedSettingsSectionId === "settings-connections", current: selectedSettingsSectionId === "settings-connections" ? "page" : undefined },
    { id: "settings-account", label: "Account", icon: "account", selected: selectedSettingsSectionId === "settings-account", current: selectedSettingsSectionId === "settings-account" ? "page" : undefined },
  ]
  const meter = projectSidebarMeter(state)
  const searchOpen = state.presentation.sessionSearchOpen
  const [renameTarget, setRenameTarget] = useState<ReactSessionRow | null>(null)
  const [renameTitle, setRenameTitle] = useState("")
  const [renameSubmitted, setRenameSubmitted] = useState(false)
  const [renameValidationError, setRenameValidationError] = useState<string | null>(null)
  const [contextMenuThreadRef, setContextMenuThreadRef] = useState<string | null>(null)
  const renameInputRef = useRef<HTMLInputElement>(null)
  const renameHostState = renameTarget !== null && state.threadRename?.threadRef === renameTarget.id
    ? state.threadRename
    : null
  const renameSaving = renameHostState?.status === "saving"
  const renameError = renameValidationError ?? (renameHostState?.status === "failed" ? renameHostState.error : null)
  const closeRename = (): void => {
    const returnThreadRef = renameTarget?.id ?? null
    setRenameTarget(null)
    setRenameTitle("")
    setRenameSubmitted(false)
    setRenameValidationError(null)
    if (returnThreadRef !== null) queueMicrotask(() =>
      dispatch(report, "DesktopChatRenameDismissed", returnThreadRef))
    if (returnThreadRef !== null) queueMicrotask(() => {
      document.querySelector<HTMLButtonElement>(`[data-en-key="sidebar-thread-${returnThreadRef}"]`)?.focus()
    })
  }
  const openRename = (row: ReactSessionRow): void => {
    setContextMenuThreadRef(null)
    queueMicrotask(() => {
      setRenameTarget(row)
      setRenameTitle(row.title)
      setRenameSubmitted(false)
      setRenameValidationError(null)
      dispatch(report, "DesktopChatRenameDismissed", row.id)
    })
  }
  useEffect(() => {
    if (renameTarget === null) return
    renameInputRef.current?.focus()
    renameInputRef.current?.select()
  }, [renameTarget])
  useEffect(() => {
    if (!renameSubmitted || renameTarget === null || state.threadRename !== null) return
    const renamed = state.threads.find(thread => thread.id === renameTarget.id)
    if (renamed?.title !== renameTitle.trim()) return
    setRenameTarget(null)
    setRenameTitle("")
    setRenameSubmitted(false)
    setRenameValidationError(null)
  }, [renameSubmitted, renameTarget, renameTitle, state.threadRename, state.threads])
  const submitRename = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault()
    if (renameTarget === null || renameSaving) return
    const title = renameTitle.trim()
    if (title === "") {
      setRenameValidationError("Enter a title before saving.")
      renameInputRef.current?.focus()
      return
    }
    setRenameValidationError(null)
    setRenameSubmitted(true)
    dispatch(report, "DesktopChatRenameRequested", { threadRef: renameTarget.id, title })
  }
  const closeSearch = (): void => {
    if (state.history.searchQuery !== "") dispatch(report, "HistorySearchChanged", "")
    dispatch(report, "DesktopSessionSearchDisclosureChanged", false)
  }
  return <><DesktopSessionRail
    backLabel={state.navigation.backTitle === null ? "Back" : `Back to ${state.navigation.backTitle}`}
    canGoBack={state.navigation.canGoBack}
    canGoForward={state.navigation.canGoForward}
    canLoadMore={false}
    destinations={state.workspace === "settings" ? settingsSections : primaryDestinations.map(destination => ({
      accessibilityLabel: destination.accessibilityLabel,
      current: destination.accessibilityCurrent,
      icon: sharedRailIcon(destination.icon),
      id: destination.id,
      indicator: destination.indicator?.kind ?? null,
      label: destination.label,
      selected: destination.selected,
    }))}
    forwardLabel={state.navigation.forwardTitle === null ? "Forward" : `Forward to ${state.navigation.forwardTitle}`}
    footer={meter === undefined ? undefined : <div aria-label="Rate limits" className="oa-react-sidebar-meter">
      <ContextMeter
        {...(meter.usage === undefined ? {} : { usage: meter.usage })}
        {...(meter.rateLimits === undefined ? {} : { rateLimits: meter.rateLimits })}
      />
    </div>}
    hydrated={state.history.hydrated}
    mode={state.workspace === "files" ? { title: "Files", content: <ReactFilesSidebar state={state} report={report} /> } : undefined}
    onBack={() => dispatch(report, "DesktopNavigationBackRequested")}
    onCollapse={onCollapse}
    onDestinationSelect={selected => {
      if (selected.id.startsWith("settings-")) {
        onSettingsSectionSelect(selected.id as ReactSettingsSectionId)
        return
      }
      const destination = destinations.find(candidate => candidate.id === selected.id)
      if (destination === undefined) return
      dispatch(report, destination.intent.name, destination.intent.payload)
      onDismiss()
    }}
    onForward={() => dispatch(report, "DesktopNavigationForwardRequested")}
    onLoadMore={() => dispatch(report, "HistoryCatalogMoreRequested")}
    onSearchOpenChange={nextOpen => nextOpen
      ? dispatch(report, "DesktopSessionSearchDisclosureChanged", true)
      : closeSearch()}
    onSearchQueryChange={query => dispatch(report, "HistorySearchChanged", query)}
    onSessionSelect={selected => {
      const row = rows.find(candidate => candidate.id === selected.id)
      if (row === undefined) return
      dispatch(report, row.intent, row.id)
      onDismiss()
    }}
    renderSession={(session, rowElement) => {
      const row = rows.find(candidate => candidate.id === session.id)
      if (row?.source !== "local") return rowElement
      return <ContextMenu
        onOpenChange={open => setContextMenuThreadRef(open ? row.id : null)}
        open={contextMenuThreadRef === row.id}
      >
        <ContextMenuTrigger render={rowElement as ReactElement<ButtonHTMLAttributes<HTMLButtonElement>>} />
        <ContextMenuContent aria-label={`Actions for ${row.title}`}>
          <ContextMenuItem onClick={() => openRename(row)}>Rename</ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>
    }}
    open={open}
    ref={railRef}
    searchOpen={searchOpen}
    searchPending={state.history.searchPending}
    searchQuery={state.history.searchQuery}
    sessions={rows.map(row => ({
      id: row.id,
      meta: row.meta,
      selected: row.selected,
      title: row.title,
      working: row.working && !state.historyShortcutHintsVisible,
    }))}
    workspaceSection={state.workspace === "settings" ? undefined : <CodingProjectSection state={state} report={report} onDismiss={onDismiss} />}
    settingsDestination={settingsDestination === undefined ? undefined : {
      accessibilityLabel: settingsDestination.accessibilityLabel,
      current: state.workspace === "settings" ? undefined : settingsDestination.accessibilityCurrent,
      icon: state.workspace === "settings" ? "back" : sharedRailIcon(settingsDestination.icon),
      id: settingsDestination.id,
      indicator: settingsDestination.indicator?.kind ?? null,
      label: state.workspace === "settings" ? "Back" : settingsDestination.label,
      selected: state.workspace === "settings" ? false : settingsDestination.selected,
    }}
    stageLabel={DESKTOP_STAGE_LABEL}
  />
  {renameTarget === null ? null :
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/45 p-4" data-slot="dialog-backdrop">
    <div
      aria-describedby="desktop-chat-rename-description"
      aria-labelledby="desktop-chat-rename-heading"
      aria-modal="true"
      className="grid w-full max-w-md gap-5 rounded-lg border bg-background p-6 shadow-lg"
      onKeyDown={event => {
        if (event.key !== "Escape") return
        event.preventDefault()
        closeRename()
      }}
      role="dialog"
    >
      <form className="grid gap-5" onSubmit={submitRename}>
        <div className="grid gap-2">
          <h2 className="text-lg font-semibold" id="desktop-chat-rename-heading">Rename chat</h2>
          <p className="text-sm text-muted-foreground" id="desktop-chat-rename-description">Choose a short title that will appear in the sidebar and conversation header.</p>
        </div>
        <div className="grid gap-2">
          <label className="text-sm font-medium" htmlFor="desktop-chat-rename-title">Chat title</label>
          <Input
            aria-describedby={renameError === null ? undefined : "desktop-chat-rename-error"}
            aria-invalid={renameError === null ? undefined : true}
            autoFocus
            disabled={renameSaving}
            id="desktop-chat-rename-title"
            maxLength={120}
            onInput={event => {
              setRenameTitle(event.currentTarget.value)
              setRenameValidationError(null)
            }}
            ref={renameInputRef}
            value={renameTitle}
          />
          {renameError === null ? null : <p className="text-sm text-destructive" id="desktop-chat-rename-error" role="alert">{renameError}</p>}
        </div>
        <div className="flex justify-end gap-2">
          <Button onClick={closeRename} type="button" variant="outline">Cancel</Button>
          <Button disabled={renameSaving} type="submit">{renameSaving ? "Saving…" : "Save"}</Button>
        </div>
      </form>
    </div>
    </div>}
  </>
}

const staticKhalaReporter: IntentReporter = () => Effect.void
const settingsFrameSize = [960, 720] as const
const settingsHeaderSize = [360, 48] as const
const settingsFrame = Frame({
  key: "settings-khala-frame",
  a11y: { hidden: true },
  khala: {
    id: "desktop-settings-frame",
    motif: "cut-corner-surface",
    width: settingsFrameSize[0],
    height: settingsFrameSize[1],
    density: "compact",
  },
})
const settingsHeaderAccent = Frame({
  key: "settings-khala-header-accent",
  a11y: { hidden: true },
  khala: {
    id: "desktop-settings-header",
    motif: "header-line",
    width: settingsHeaderSize[0],
    height: settingsHeaderSize[1],
    density: "compact",
  },
})

const StaticKhalaDecoration = ({ view, placement }: {
  readonly view: FrameView
  readonly placement: "settings-frame" | "settings-header"
}): ReactElement => <div
  className="oa-react-khala-decoration"
  data-khala-decoration={placement}
  data-settings-khala-decoration={placement}
  aria-hidden="true"
>{renderReactDomView(view, { report: staticKhalaReporter, theme: openagentsDesktopTheme })}</div>

export const WorkbenchShell = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement => {
  const [railOpen, setRailOpen] = useState(false)
  const railCollapsed = state.presentation.sidebarCollapsed
  const [codexUpdateOpen, setCodexUpdateOpen] = useState(false)
  const [dismissedCodexVersion, setDismissedCodexVersion] = useState<string | null>(null)
  const [selectedSettingsSectionId, setSelectedSettingsSectionId] = useState<ReactSettingsSectionId>(
    state.workspace === "settings" && state.connections.phase === "ready" ? "settings-connections" : "settings-general",
  )
  const railRef = useRef<HTMLElement>(null)
  const toggleRef = useRef<HTMLButtonElement>(null)
  useEffect(() => {
    const onKeyDown = (event: globalThis.KeyboardEvent): void => {
      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
        event.preventDefault()
        event.stopPropagation()
        if (railCollapsed && !railOpen) {
          dispatch(report, "DesktopSidebarCollapsedChanged", false)
          setRailOpen(true)
        } else {
          dispatch(report, "DesktopSidebarCollapsedChanged", true)
          setRailOpen(false)
        }
        return
      }
      if (event.key === "Escape" && railOpen) {
        setRailOpen(false)
        toggleRef.current?.focus()
      }
    }
    window.addEventListener("keydown", onKeyDown, true)
    return () => window.removeEventListener("keydown", onKeyDown, true)
  }, [railCollapsed, railOpen, report])
  useEffect(() => {
    if (railOpen) railRef.current?.querySelector<HTMLInputElement>('input[type="search"]')?.focus()
  }, [railOpen])
  useEffect(() => {
    if (state.workspace === "settings" && state.fleet.phase === "idle") {
      dispatch(report, "FleetRefreshRequested")
    }
  }, [report, state.fleet.phase, state.workspace])
  const openRail = (): void => {
    dispatch(report, "DesktopSidebarCollapsedChanged", false)
    setRailOpen(true)
  }
  const closeRail = (): void => {
    dispatch(report, "DesktopSidebarCollapsedChanged", true)
    setRailOpen(false)
  }
  const workspaceSurface = state.workspace === "files"
      ? <main className="oa-react-files-mode" data-react-workspace="files" aria-label="Files workspace">
          <FilesModeHeader state={state} report={report} />
          <ReactWorkspaceEditor state={state} report={report} />
        </main>
      : state.workspace === "settings"
      ? <main className="oa-react-workspace-surface oa-react-settings-khala" data-react-workspace="settings">
          <StaticKhalaDecoration view={settingsFrame} placement="settings-frame" />
          <header className="oa-react-settings-header" id="react-settings-title">
            <StaticKhalaDecoration view={settingsHeaderAccent} placement="settings-header" />
            <div><p>OpenAgents</p><h1>Settings</h1></div>
            <Button type="button" variant="outline" onClick={() => dispatch(report, "DesktopHarnessMaintenanceRefreshRequested")}>Refresh</Button>
          </header>
          <ReactSettingsSurface state={state} report={report} sectionId={selectedSettingsSectionId as ReactSettingsSectionId} />
        </main>
      // FA-UX-01 (#8974): the dedicated Full Auto launcher/run view is a full
      // workspace-surface override, exactly like Settings -- it replaces
      // DesktopSurfaceManager's `conversation` (and therefore the ordinary
      // chat composer) rather than coexisting beside it the way the Files/
      // Review side panels do.
      : state.workspace === "full-auto"
      ? <main className="oa-react-workspace-surface oa-react-full-auto-workspace" data-react-workspace="full-auto">
          <ReactFullAutoSurface state={state} report={report} />
        </main>
      : null
  const maintenance = state.settings.harnessMaintenance
  const waitingForAnswer = state.activeThreadId !== null && state.pending &&
    state.notes.some(note => note.question?.status === "pending")
  const codexReleaseNotes = maintenance.codexReleaseNotes ?? null
  const codex = maintenance.view.state === "loaded"
    ? maintenance.view.harnesses.find(item => item.harness === "codex") ?? null
    : null
  const codexUpdateAvailable = codex?.advisory === "behind_latest" && codex.latestVersion !== null
  const startCodexUpdate = (): void => {
    if (codex === null || !codex.updateSupported || maintenance.updating !== null) return
    setCodexUpdateOpen(true)
    setDismissedCodexVersion(codex.latestVersion)
    dispatch(report, "DesktopHarnessUpdateRequested", "codex")
  }
  return <DesktopWorkbench railCollapsed={railCollapsed}>
    <ReactCommandPalette state={state} report={report} />
    <SessionRail state={state} report={report} open={railOpen} onCollapse={closeRail} onDismiss={() => setRailOpen(false)} railRef={railRef} selectedSettingsSectionId={selectedSettingsSectionId} onSettingsSectionSelect={sectionId => {
      setSelectedSettingsSectionId(sectionId)
      if (sectionId === "settings-connections") dispatch(report, "DesktopConnectionsRefreshRequested")
      if (sectionId === "settings-account") dispatch(report, "FleetRefreshRequested")
    }} />
    {railOpen ? <DesktopRailScrim aria-label="Close sessions" onClick={() => setRailOpen(false)} /> : null}
    {workspaceSurface ?? <DesktopSurfaceManager state={state} report={report} conversation={<DesktopConversation
        composer={state.history.page === null ? <div className="oa-react-composer-stack">
          <DecisionSurface state={state} report={report} />
          <ReactComposer state={state} report={report} />
        </div> : null}
        header={<ConversationHeader state={state} report={report} />}
        notices={<StatusNotices state={state} report={report} />}
        timeline={<ConversationTimeline page={state.history.page} notes={state.notes} loadingEdge={state.history.loadingEdge} working={state.activeThreadId !== null && state.pending && !waitingForAnswer} waitingForAnswer={waitingForAnswer} workingDirectory={state.workingDirectory} agentName={capabilityForActiveLane(state)?.displayName ?? (state.selectedHarness === "codex" ? "Codex" : "Claude")} bootSequenceAgents={projectBootSequenceAgents(state)} bootSequenceIdentity={projectBootSequenceIdentity(state)} report={report} />}
      />} />}
    {/* Render the fixed expand toggle AFTER the workspace surface. Its
        `-webkit-app-region: no-drag` only carves a clickable hole out of the
        bare chat header's `drag` region when its annotated region is collected
        later in tree order than that header; a sibling placed before the header
        loses the region and the OS drag layer swallows every real click. */}
    <DesktopSidebarExpand
      ref={toggleRef}
      onClick={openRail}
      aria-expanded={railOpen}
      aria-label="Expand sidebar"
      title="Expand sidebar"
    />
    {codexUpdateAvailable && dismissedCodexVersion !== codex.latestVersion
      ? <Alert className="oa-react-codex-update-notice" role="status">
          <CircleAlert aria-hidden="true" />
          <AlertTitle>Codex update available</AlertTitle>
          <AlertDescription>
            {codex.installedVersion ?? "Installed version unknown"} → {codex.latestVersion}
          </AlertDescription>
          <button className="oa-react-codex-update-dismiss" type="button" aria-label="Dismiss Codex update" title="Dismiss" onClick={() => setDismissedCodexVersion(codex.latestVersion)}>
            <X aria-hidden="true" />
          </button>
          <div className="oa-react-codex-update-actions">
            <Button variant="outline" size="sm" type="button" onClick={() => dispatch(report, "DesktopSettingsToggled")}>Settings</Button>
            <Button size="sm" type="button" disabled={!codex.updateSupported} onClick={startCodexUpdate}>
              <Download aria-hidden="true" /> Update
            </Button>
          </div>
        </Alert>
      : null}
    <Dialog open={codexUpdateOpen} onOpenChange={setCodexUpdateOpen}>
      <DialogContent className="oa-react-codex-update-dialog">
        <DialogHeader>
          <DialogTitle>Update Codex</DialogTitle>
          <DialogDescription>
            {codex === null
              ? "Checking the installed Codex CLI."
              : `${codex.installedVersion ?? "Unknown version"} → ${codex.latestVersion ?? "latest"} via ${codex.channel}.`}
          </DialogDescription>
        </DialogHeader>
        <section className="oa-react-codex-update-state" aria-live="polite">
          {maintenance.updating === "codex"
            ? <><LoaderCircle className="oa-react-codex-update-spinner" aria-hidden="true" /><div><strong>Updating Codex…</strong><p>The existing install is pinned first; success is reported only after the updated binary answers a fresh version probe.</p></div></>
            : maintenance.lastOutcome === null
              ? <><Download aria-hidden="true" /><div><strong>Ready to update</strong><p>OpenAgents will stay on the detected package channel and will not touch Codex login state.</p></div></>
              : <div><strong>Update result</strong><p>{maintenance.lastOutcome}</p></div>}
        </section>
        <section className="oa-react-codex-release-notes">
          <div className="oa-react-codex-release-heading">
            <h3>{codexReleaseNotes?.title ?? "What’s new"}</h3>
            {codexReleaseNotes?.publishedAt === null || codexReleaseNotes?.publishedAt === undefined
              ? null
              : <time dateTime={codexReleaseNotes.publishedAt}>{new Date(codexReleaseNotes.publishedAt).toLocaleDateString()}</time>}
          </div>
          {codexReleaseNotes === null
            ? <p className="oa-react-codex-release-fallback">Release notes could not be loaded. The update can still be verified against the npm registry and local version probe.</p>
            : <div className="oa-react-codex-release-body"><SafeReactMarkdown value={codexReleaseNotes.body} /></div>}
        </section>
        <DialogFooter>
          <Button variant="outline" type="button" onClick={() => setCodexUpdateOpen(false)}>Close</Button>
          <Button type="button" disabled={codex === null || !codex.updateSupported || maintenance.updating !== null || codex.advisory === "current"} onClick={startCodexUpdate}>
            {maintenance.updating === "codex" ? <><LoaderCircle className="oa-react-codex-update-spinner" aria-hidden="true" /> Updating…</> : "Update Codex"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </DesktopWorkbench>
}

const ReactWorkbenchProjection = ({ store, report }: {
  readonly store: ReactViewStore<DesktopShellState>
  readonly report: IntentReporter
}): ReactElement => {
  const snapshot = useSyncExternalStore(store.subscribe, store.getSnapshot, store.getServerSnapshot)
  if (snapshot.status === "loading") return <div role="status">Loading workbench…</div>
  if (snapshot.status === "failed") return <section role="alert">The workbench stopped updating.</section>
  return <WorkbenchShell state={snapshot.view} report={report} />
}

export type ReactWorkbenchSurface = Readonly<{
  root: Root
  activeSubscribers: () => number
  unmount: Effect.Effect<void>
}>

/** One React root over one Effect-owned state stream; React owns no domain store. */
export const mountReactWorkbench = (
  container: Element,
  stateStream: Stream.Stream<DesktopShellState>,
  report: IntentReporter,
  options: Readonly<{ document?: Document; theme: Theme }>,
): Effect.Effect<ReactWorkbenchSurface, never, Scope.Scope> => Effect.gen(function*() {
  const document = options.document ?? container.ownerDocument
  const stylesheet = mountDomThemeStyleSheet(document, options.theme)
  const store = yield* makeReactViewStore(stateStream)
  const root = createRoot(container)
  let unmounted = false
  const unmount = Effect.sync(() => {
    if (unmounted) return
    unmounted = true
    root.unmount()
    stylesheet.dispose()
  })
  yield* Effect.addFinalizer(() => unmount)
  root.render(createElement(
    StrictMode,
    null,
    createElement(
      ReactSurfaceErrorBoundary,
      { resetKey: 0 },
      createElement(ReactWorkbenchProjection, { store, report }),
    ),
  ))
  yield* store.firstCommit
  yield* Effect.promise(() => new Promise<void>(resolve => setTimeout(resolve, 0)))
  return { root, activeSubscribers: store.activeSubscribers, unmount }
})
