import {
  StrictMode,
  createElement,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type RefObject,
  type ReactElement,
} from "react"
import { createRoot, type Root } from "react-dom/client"
import {
  ComponentValueBinding,
  Frame,
  IntentRef,
  type FrameView,
  type IconName,
  type IntentError,
  type IntentReporter,
  type JsonPayload,
} from "@effect-native/core"
import { Effect, Scope, Stream } from "@effect-native/core/effect"
import { mountDomThemeStyleSheet } from "@effect-native/render-dom"
import {
  Folder,
  CircleAlert,
  Download,
  LoaderCircle,
  X,
  type LucideIcon,
} from "lucide-react"
import {
  DesktopConversation,
  DesktopConversationHeader,
  DesktopRailScrim,
  DesktopSessionRail,
  DesktopSidebarExpand,
  DesktopWorkbench,
  type DesktopRailIcon,
} from "@openagentsinc/ui/desktop-workbench"
import {
  ReactSurfaceErrorBoundary,
  makeReactViewStore,
  renderReactDomView,
  type ReactViewStore,
} from "@effect-native/render-dom/react"
import { khalaTheme, type Theme } from "@effect-native/tokens"
import { Button } from "#components/ui/button"
import { Alert, AlertDescription, AlertTitle } from "#components/ui/alert"
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from "#components/ui/dialog"
import { Separator } from "#components/ui/separator"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "#components/ui/tooltip"
import type { DesktopShellState } from "./shell.ts"
import { desktopConversationShortcutLabel, formatRelativeTimestamp } from "./shell.ts"
import { DecisionSurface, ReactCommandPalette, ReactComposer } from "./react-composer.tsx"
import { StatusNotices } from "./react-review.tsx"
import { ConversationTimeline, SafeReactMarkdown } from "./react-timeline.tsx"
import { RedactedSensitiveText } from "./react-sensitive-text.tsx"
import { DESKTOP_STAGE_LABEL } from "./branding.ts"
import { projectDesktopSidebarDestinations } from "./sidebar-destinations.ts"

type ReactSidebarIconName = Extract<IconName, "Folder">

const sidebarIconAssets: Readonly<Record<ReactSidebarIconName, LucideIcon>> = {
  Folder,
}

/** Closed-catalog React lowering; Lucide remains a renderer-private asset implementation. */
const ReactCatalogIcon = ({ name }: { readonly name: ReactSidebarIconName }): ReactElement => {
  const Asset = sidebarIconAssets[name]
  return <Asset aria-hidden="true" data-icon-name={name} focusable="false" />
}

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
  const selectedHistoryRef = state.history.pendingThreadRef ?? state.history.page?.rootThreadRef ?? null
  const local = state.threads
    .filter(thread => query === "" || normalized(thread.title).includes(query))
    .map((thread): ReactSessionRow => ({
      id: thread.id,
      title: thread.title || "Untitled session",
      updatedAt: thread.updatedAt,
      meta: formatRelativeTimestamp(thread.updatedAt, now),
      source: "local",
      selected: selectedHistoryRef === null && state.activeThreadId === thread.id,
      intent: "DesktopChatSelected",
    }))
  const history: ReadonlyArray<ReactSessionRow> = query === ""
    ? state.history.catalog.roots.slice(0, state.history.visibleRootCount).filter(row => row.source === "codex").map((row): ReactSessionRow => ({
      id: row.threadRef,
      title: row.title || "Untitled session",
      updatedAt: row.updatedAt,
      meta: formatRelativeTimestamp(row.updatedAt, now),
      selected: selectedHistoryRef === row.threadRef,
      source: "history",
      intent: "HistoryConversationSelected",
    }))
    : state.history.searchResults.filter(row => row.source === "codex").map((row): ReactSessionRow => ({
      id: row.threadRef,
      title: row.title || "Untitled session",
      updatedAt: row.updatedAt,
      meta: formatRelativeTimestamp(row.updatedAt, now),
      selected: selectedHistoryRef === row.threadRef,
      source: "search",
      intent: "HistorySearchResultOpened",
    }))
  const seen = new Set<string>()
  const rows = [...local, ...history]
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt) || left.id.localeCompare(right.id))
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

export const ConversationHeader = ({ state }: {
  readonly state: DesktopShellState
}): ReactElement => {
  const selectedCoding = state.codingCatalog.sessions.find(
    session => session.sessionRef === state.codingCatalog.selectedSessionRef,
  )
  return <DesktopConversationHeader
    lifecycle={selectedLifecycle(state)}
    secondary={selectedCoding?.repositoryLabel}
    title={selectedTitle(state)}
  />
}

const sharedRailIcon = (icon: "ChatCompose" | "Chats" | "Home" | "Settings"): DesktopRailIcon => {
  if (icon === "ChatCompose") return "new-session"
  if (icon === "Chats") return "chat"
  if (icon === "Home") return "home"
  return "settings"
}

export const SessionRail = ({ state, report, open, onCollapse, onDismiss, railRef }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
  readonly open: boolean
  readonly onCollapse: () => void
  readonly onDismiss: () => void
  readonly railRef: RefObject<HTMLElement | null>
}): ReactElement => {
  const rows = projectReactSessionRows(state)
  const destinations = projectDesktopSidebarDestinations(
    state.workspace === "home" || state.workspace === "settings" ? state.workspace : "chat",
    rows.some(row => row.selected),
  )
  const shown = state.history.visibleRootCount
  const searchOpen = state.presentation.sessionSearchOpen
  const closeSearch = (): void => {
    if (state.history.searchQuery !== "") dispatch(report, "HistorySearchChanged", "")
    dispatch(report, "DesktopSessionSearchDisclosureChanged", false)
  }
  return <DesktopSessionRail
    backLabel={state.navigation.backTitle === null ? "Back" : `Back to ${state.navigation.backTitle}`}
    canGoBack={state.navigation.canGoBack}
    canGoForward={state.navigation.canGoForward}
    canLoadMore={state.history.searchQuery.trim() === "" && shown < state.history.catalog.roots.length}
    destinations={destinations.map(destination => ({
      accessibilityLabel: destination.accessibilityLabel,
      current: destination.accessibilityCurrent,
      icon: sharedRailIcon(destination.icon),
      id: destination.id,
      indicator: destination.indicator?.kind ?? null,
      label: destination.label,
      selected: destination.selected,
    }))}
    footer={state.codingCatalog.sessions.length === 0 ? null : <section className="oa-react-workspaces" aria-label="Coding workspaces">
      <Separator />
      <h2><ReactCatalogIcon name="Folder" /> <span>Workspaces</span></h2>
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
    forwardLabel={state.navigation.forwardTitle === null ? "Forward" : `Forward to ${state.navigation.forwardTitle}`}
    hydrated={state.history.hydrated}
    onBack={() => dispatch(report, "DesktopNavigationBackRequested")}
    onCollapse={onCollapse}
    onDestinationSelect={selected => {
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
    open={open}
    ref={railRef}
    searchOpen={searchOpen}
    searchPending={state.history.searchPending}
    searchQuery={state.history.searchQuery}
    sessions={rows.map(row => ({ id: row.id, meta: row.meta, selected: row.selected, title: row.title }))}
    stageLabel={DESKTOP_STAGE_LABEL}
  />
}

const staticKhalaReporter: IntentReporter = () => Effect.void
const projectHomeFrameSize = [960, 640] as const
const projectHomeStatusSize = [240, 32] as const
const projectHomeFrame = Frame({
  key: "project-home-khala-frame",
  a11y: { hidden: true },
  khala: {
    id: "desktop-project-home-frame",
    motif: "cut-corner-surface",
    width: projectHomeFrameSize[0],
    height: projectHomeFrameSize[1],
    density: "comfortable",
  },
})
const projectHomeStatusAccent = Frame({
  key: "project-home-khala-status-accent",
  a11y: { hidden: true },
  khala: {
    id: "desktop-project-home-status",
    motif: "header-line",
    width: projectHomeStatusSize[0],
    height: projectHomeStatusSize[1],
    density: "compact",
  },
})

const StaticKhalaDecoration = ({ view, placement }: {
  readonly view: FrameView
  readonly placement: "frame" | "status"
}): ReactElement => <div
  className="oa-react-khala-decoration"
  data-project-home-khala-decoration={placement}
  aria-hidden="true"
>{renderReactDomView(view, { report: staticKhalaReporter, theme: khalaTheme })}</div>

export const WorkbenchShell = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement => {
  const [railOpen, setRailOpen] = useState(false)
  const railCollapsed = state.presentation.sidebarCollapsed
  const [codexUpdateOpen, setCodexUpdateOpen] = useState(false)
  const [dismissedCodexVersion, setDismissedCodexVersion] = useState<string | null>(null)
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
  const openRail = (): void => {
    dispatch(report, "DesktopSidebarCollapsedChanged", false)
    setRailOpen(true)
  }
  const closeRail = (): void => {
    dispatch(report, "DesktopSidebarCollapsedChanged", true)
    setRailOpen(false)
  }
  const workspaceSurface = state.workspace === "home"
    ? <main className="oa-react-workspace-surface oa-react-project-home-khala" data-react-workspace="home">
        <StaticKhalaDecoration view={projectHomeFrame} placement="frame" />
        <header><div><p>Project home</p><h1>Coding sessions</h1></div><div className="oa-react-project-home-status"><StaticKhalaDecoration view={projectHomeStatusAccent} placement="status" /><span>{state.codingCatalog.authorityLabel}</span></div></header>
        <p>Resume the exact project, repository, worktree, and task context from this Mac.</p>
        <Button type="button" onClick={() => dispatch(report, "DesktopCodingCatalogChooseRequested")}>Open project folder</Button>
        <section aria-label="Coding sessions">
          {state.codingCatalog.sessions.length === 0
            ? <p>No coding sessions yet.</p>
            : state.codingCatalog.sessions.map(session => <Button
                key={session.sessionRef}
                type="button"
                variant="outline"
                data-coding-session-ref={session.sessionRef}
                onClick={() => dispatch(report, "DesktopCodingSessionOpened", session.sessionRef)}
              ><span>{session.projectLabel}</span><small>{session.repositoryLabel} · {session.worktreeLabel} · {session.state}</small></Button>)}
        </section>
      </main>
    : state.workspace === "settings"
      ? <main className="oa-react-workspace-surface" data-react-workspace="settings">
          <header><div><p>OpenAgents</p><h1>Settings</h1></div><Button type="button" variant="outline" onClick={() => dispatch(report, "DesktopHarnessMaintenanceRefreshRequested")}>Refresh</Button></header>
          <section aria-labelledby="react-runtime-maintenance-title">
            <h2 id="react-runtime-maintenance-title">Codex CLI</h2>
            <p>Installed Codex version, channel, and update truth from this Mac.</p>
            {state.settings.harnessMaintenance.view.state === "loading" ? <p role="status">Checking harnesses…</p>
              : state.settings.harnessMaintenance.view.state === "unavailable" ? <p role="alert">{state.settings.harnessMaintenance.view.message}</p>
              : state.settings.harnessMaintenance.view.harnesses.filter(harness => harness.harness === "codex").map(harness => <article key={harness.harness} data-harness={harness.harness}>
                  <div><strong>Codex</strong><span>{harness.installedVersion ?? "Not installed"} · {harness.channel}</span>{harness.recoveryMessage == null ? null : <small>{harness.recoveryMessage}</small>}</div>
                  <span>{harness.advisory.replaceAll("_", " ")}</span>
                  {harness.updateSupported ? <Button type="button" size="sm" disabled={state.settings.harnessMaintenance.updating !== null} onClick={() => dispatch(report, "DesktopHarnessUpdateRequested", harness.harness)}>Update</Button> : null}
                </article>)}
            {state.settings.harnessMaintenance.lastOutcome === null ? null : <p>{state.settings.harnessMaintenance.lastOutcome}</p>}
          </section>
          <section aria-labelledby="react-provider-accounts-title">
            <h2 id="react-provider-accounts-title">Codex account</h2>
            <p>Your Codex account identity is blurred until you explicitly reveal it.</p>
            {state.fleet.phase === "loading" || state.fleet.phase === "idle"
              ? <p role="status">Checking provider accounts…</p>
              : state.fleet.accounts.filter(account => account.provider === "codex").length === 0
                ? <p>No Codex account connected.</p>
                : state.fleet.accounts.filter(account => account.provider === "codex").map(account => <article key={account.ref} data-provider-account={account.ref}>
                    <div>
                      <strong>Codex</strong>
                      <span>{account.ref}</span>
                    </div>
                    <span>{account.readiness.replaceAll("-", " ")}</span>
                    {account.email === null ? null : <span className="oa-react-provider-email">
                      <span>Authenticated as</span>
                      <RedactedSensitiveText
                        value={account.email}
                        ariaLabel="Toggle account email visibility"
                        revealTooltip="Click to reveal email"
                        hideTooltip="Click to hide email"
                      />
                    </span>}
                  </article>)}
          </section>
        </main>
      : null
  const maintenance = state.settings.harnessMaintenance
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
    <DecisionSurface state={state} report={report} />
    <DesktopSidebarExpand
      ref={toggleRef}
      onClick={openRail}
      aria-expanded={railOpen}
      aria-label="Expand sidebar"
      title="Expand sidebar"
    />
    <SessionRail state={state} report={report} open={railOpen} onCollapse={closeRail} onDismiss={() => setRailOpen(false)} railRef={railRef} />
    {railOpen ? <DesktopRailScrim aria-label="Close sessions" onClick={() => setRailOpen(false)} /> : null}
    {workspaceSurface ?? <DesktopConversation
      composer={<ReactComposer state={state} report={report} />}
      header={<ConversationHeader state={state} />}
      notices={<StatusNotices state={state} report={report} />}
      timeline={<ConversationTimeline page={state.history.page} notes={state.notes} loadingEdge={state.history.loadingEdge} working={state.pending} workingDirectory={state.workingDirectory} report={report} />}
    />}
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
