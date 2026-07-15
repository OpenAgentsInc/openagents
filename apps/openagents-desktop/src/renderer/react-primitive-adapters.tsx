import {
  StrictMode,
  createElement,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type KeyboardEvent,
  type RefObject,
  type ReactElement,
} from "react"
import { createRoot, type Root } from "react-dom/client"
import { ComponentValueBinding, IntentRef, type IntentError, type IntentReporter, type JsonPayload } from "@effect-native/core"
import { Effect, Scope, Stream } from "@effect-native/core/effect"
import { mountDomThemeStyleSheet } from "@effect-native/render-dom"
import {
  ReactSurfaceErrorBoundary,
  makeReactViewStore,
  type ReactViewStore,
} from "@effect-native/render-dom/react"
import type { Theme } from "@effect-native/tokens"
import { Button } from "#components/ui/button"
import { Input } from "#components/ui/input"
import { ScrollArea } from "#components/ui/scroll-area"
import { Separator } from "#components/ui/separator"
import type { DesktopShellState } from "./shell.ts"
import { formatRelativeTimestamp } from "./shell.ts"
import { DecisionSurface, ReactCommandPalette, ReactComposer } from "./react-composer.tsx"
import { ReviewSurface, StatusNotices } from "./react-review.tsx"
import { ConversationTimeline } from "./react-timeline.tsx"
import "./react-workbench.css"

export type ReactSessionRow = Readonly<{
  id: string
  title: string
  updatedAt: string
  meta: string
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
  const selectedHistoryRef = state.history.page?.rootThreadRef ?? null
  const local = state.threads
    .filter(thread => query === "" || normalized(thread.title).includes(query))
    .map((thread): ReactSessionRow => ({
      id: thread.id,
      title: thread.title || "Untitled session",
      updatedAt: thread.updatedAt,
      meta: formatRelativeTimestamp(thread.updatedAt, now),
      source: "local",
      selected: state.activeThreadId === thread.id,
      intent: "DesktopChatSelected",
    }))
  const history: ReadonlyArray<ReactSessionRow> = query === ""
    ? state.history.catalog.roots.slice(0, state.history.visibleRootCount).filter(row => row.source === "codex").map((row): ReactSessionRow => ({
      id: row.threadRef,
      title: row.title || "Untitled session",
      updatedAt: row.updatedAt,
      meta: `${row.status} · ${formatRelativeTimestamp(row.updatedAt, now)}`,
      selected: selectedHistoryRef === row.threadRef,
      source: "history",
      intent: "HistoryConversationSelected",
    }))
    : state.history.searchResults.filter(row => row.source === "codex").map((row): ReactSessionRow => ({
      id: row.threadRef,
      title: row.title || "Untitled session",
      updatedAt: row.updatedAt,
      meta: `${row.matchKind} · ${formatRelativeTimestamp(row.updatedAt, now)}`,
      selected: selectedHistoryRef === row.threadRef,
      source: "search",
      intent: "HistorySearchResultOpened",
    }))
  const seen = new Set<string>()
  return [...local, ...history]
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt) || left.id.localeCompare(right.id))
    .filter(row => seen.has(row.id) ? false : (seen.add(row.id), true))
}

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(report(
    payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()), payload,
  ) as Effect.Effect<void, IntentError>).catch(() => {})
}

const selectedTitle = (state: DesktopShellState): string => {
  const local = state.threads.find(thread => thread.id === state.activeThreadId)
  if (local !== undefined) return local.title || "Untitled session"
  const selected = state.history.page?.rootThreadRef
  return state.history.catalog.roots.find(root => root.threadRef === selected)?.title ?? "New session"
}

const selectedLifecycle = (state: DesktopShellState): string => {
  if (state.pending) return "Running"
  if (state.history.pendingThreadRef !== null && state.history.pendingThreadRef !== undefined) return "Loading"
  const selected = state.history.page?.selectedThreadRef
  const status = state.history.catalog.agents.find(agent => agent.threadRef === selected)?.status
  if (status === "interrupted" || status === "errored") return "Needs attention"
  if (status === "running" || status === "waiting") return status === "running" ? "Running" : "Waiting"
  return "Ready"
}

export const ConversationHeader = ({ state, report, reviewTriggerRef, onReviewOpen }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
  readonly reviewTriggerRef: RefObject<HTMLButtonElement | null>
  readonly onReviewOpen: () => void
}): ReactElement => {
  const selectedCoding = state.codingCatalog.sessions.find(
    session => session.sessionRef === state.codingCatalog.selectedSessionRef,
  )
  return <header className="oa-react-conversation-header">
    <div className="oa-react-conversation-heading">
      <h1>{selectedTitle(state)}</h1>
      <div className="oa-react-conversation-meta" aria-label="Session status">
        <span data-lifecycle={selectedLifecycle(state).toLocaleLowerCase().replaceAll(" ", "-")}>
          {selectedLifecycle(state)}
        </span>
        {selectedCoding === undefined ? null : <span>{selectedCoding.repositoryLabel}</span>}
      </div>
    </div>
    <Button ref={reviewTriggerRef} className="oa-react-review-trigger" type="button" variant="outline" size="sm" onClick={() => {
      dispatch(report, "GitPanelRefreshRequested")
      onReviewOpen()
    }}>Review changes</Button>
  </header>
}

const focusAdjacentSession = (event: KeyboardEvent<HTMLElement>): void => {
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return
  const rows = [...event.currentTarget.querySelectorAll<HTMLButtonElement>("[data-session-row]")]
  const index = rows.indexOf(event.target as HTMLButtonElement)
  if (index < 0 || rows.length === 0) return
  event.preventDefault()
  rows[(index + (event.key === "ArrowDown" ? 1 : -1) + rows.length) % rows.length]?.focus()
}

export const SessionRail = ({ state, report, open, onClose, railRef }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
  readonly open: boolean
  readonly onClose: () => void
  readonly railRef: RefObject<HTMLElement | null>
}): ReactElement => {
  const rows = projectReactSessionRows(state)
  const shown = state.history.visibleRootCount
  return <aside
    ref={railRef}
    className="oa-react-session-rail"
    data-open={open ? "true" : "false"}
    aria-label="Sessions"
    onKeyDown={focusAdjacentSession}
  >
    <div className="oa-react-rail-titlebar">
      <strong>OpenAgents</strong>
      <Button className="oa-react-rail-close" variant="ghost" size="sm" type="button" onClick={onClose} aria-label="Close sessions">Close</Button>
    </div>
    <Button className="oa-react-new-session" type="button" onClick={() => dispatch(report, "DesktopNewChat")}>New session</Button>
    <label className="oa-react-search">
      <span>Search sessions</span>
      <Input
        type="search"
        value={state.history.searchQuery}
        placeholder="Search Codex sessions"
        onInput={event => dispatch(report, "HistorySearchChanged", event.currentTarget.value)}
      />
    </label>
    <ScrollArea className="oa-react-session-scroll">
    <nav className="oa-react-session-list" aria-label="Recent sessions">
      {!state.history.hydrated && rows.length === 0
        ? <p role="status">Scanning sessions…</p>
        : rows.length === 0
          ? <p>{state.history.searchPending ? "Searching…" : "No sessions found"}</p>
          : rows.map(row => <Button
              key={`${row.source}:${row.id}`}
              type="button"
              variant="ghost"
              className="oa-react-session-row justify-start text-left"
              data-session-row
              data-selected={row.selected ? "true" : "false"}
              aria-current={row.selected ? "page" : undefined}
              onClick={() => {
                dispatch(report, row.intent, row.id)
                onClose()
              }}
            >
              <span>{row.title}</span>
              <small>{row.meta}</small>
            </Button>)}
      {state.history.searchQuery.trim() === "" && shown < state.history.catalog.roots.length
        ? <Button type="button" variant="outline" size="sm" className="oa-react-load-more" onClick={() => dispatch(report, "HistoryCatalogMoreRequested")}>Load more sessions</Button>
        : null}
    </nav>
    </ScrollArea>
    {state.codingCatalog.sessions.length === 0 ? null : <section className="oa-react-workspaces" aria-label="Coding workspaces">
      <Separator />
      <h2>Workspaces</h2>
      {state.codingCatalog.sessions.map(session => <div className="oa-react-workspace-row" key={session.sessionRef}>
        <Button variant="ghost" type="button" onClick={() => dispatch(report, "DesktopCodingSessionOpened", session.sessionRef)}>
          <span>{session.repositoryLabel}</span><small>{session.state}</small>
        </Button>
        <div className="oa-react-workspace-actions">
          {session.state === "recovery_required"
            ? <Button variant="outline" size="xs" type="button" onClick={() => dispatch(report, "DesktopCodingSessionRecovered", session.sessionRef)}>Recover</Button>
            : <Button variant="outline" size="xs" type="button" onClick={() => dispatch(report, "DesktopCodingSessionArchived", session.sessionRef)}>Archive</Button>}
          {state.codingSessionDeleteConfirmRef === session.sessionRef
            ? <>
                <Button variant="destructive" size="xs" type="button" onClick={() => dispatch(report, "DesktopCodingSessionDeleteConfirmed", session.sessionRef)}>Confirm delete</Button>
                <Button variant="ghost" size="xs" type="button" onClick={() => dispatch(report, "DesktopCodingSessionDeleteCancelled")}>Cancel</Button>
              </>
            : <Button variant="ghost" size="xs" type="button" onClick={() => dispatch(report, "DesktopCodingSessionDeleteRequested", session.sessionRef)}>Delete</Button>}
        </div>
      </div>)}
      {state.codingCatalog.nextOffset === null ? null
        : <Button type="button" variant="outline" size="sm" className="oa-react-load-more" onClick={() => dispatch(report, "DesktopCodingCatalogMoreRequested")}>Load more workspaces</Button>}
    </section>}
  </aside>
}

export const WorkbenchShell = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement => {
  const [railOpen, setRailOpen] = useState(false)
  const [reviewOpen, setReviewOpen] = useState(false)
  const railRef = useRef<HTMLElement>(null)
  const toggleRef = useRef<HTMLButtonElement>(null)
  const reviewTriggerRef = useRef<HTMLButtonElement>(null)
  useEffect(() => {
    const onKeyDown = (event: globalThis.KeyboardEvent): void => {
      if (event.key !== "Escape" || !railOpen) return
      setRailOpen(false)
      toggleRef.current?.focus()
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [railOpen])
  useEffect(() => {
    if (railOpen) railRef.current?.querySelector<HTMLInputElement>('input[type="search"]')?.focus()
  }, [railOpen])
  return <div className="oa-react-workbench" data-en-react-surface="true" data-review-open={reviewOpen ? "true" : "false"}>
    <ReactCommandPalette state={state} report={report} />
    <DecisionSurface state={state} report={report} />
    <Button
      ref={toggleRef}
      className="oa-react-mobile-session-trigger"
      variant="outline"
      size="sm"
      type="button"
      onClick={() => setRailOpen(true)}
      aria-expanded={railOpen}
    >Sessions</Button>
    <SessionRail state={state} report={report} open={railOpen} onClose={() => setRailOpen(false)} railRef={railRef} />
    {railOpen ? <button className="oa-react-rail-scrim" aria-label="Close sessions" onClick={() => setRailOpen(false)} /> : null}
    <main className="oa-react-conversation">
      <ConversationHeader state={state} report={report} reviewTriggerRef={reviewTriggerRef} onReviewOpen={() => setReviewOpen(true)} />
      <div className="oa-react-conversation-body">
        <StatusNotices state={state} report={report} />
        <ConversationTimeline page={state.history.page} notes={state.notes} loadingEdge={state.history.loadingEdge} working={state.pending} report={report} />
      </div>
      <ReactComposer state={state} report={report} />
    </main>
    <ReviewSurface state={state} report={report} open={reviewOpen} onOpenChange={setReviewOpen} triggerRef={reviewTriggerRef} />
  </div>
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
