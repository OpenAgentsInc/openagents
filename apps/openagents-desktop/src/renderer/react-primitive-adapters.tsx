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
import { ComponentValueBinding, IntentRef, type IconName, type IntentError, type IntentReporter, type JsonPayload } from "@effect-native/core"
import { Effect, Scope, Stream } from "@effect-native/core/effect"
import { mountDomThemeStyleSheet } from "@effect-native/render-dom"
import {
  ChevronLeft,
  ChevronRight,
  Folder,
  House,
  CircleAlert,
  Download,
  LoaderCircle,
  MessageCircle,
  PanelLeft,
  Search,
  Settings,
  SquarePen,
  X,
  type LucideIcon,
} from "lucide-react"
import {
  ReactSurfaceErrorBoundary,
  makeReactViewStore,
  type ReactViewStore,
} from "@effect-native/render-dom/react"
import type { Theme } from "@effect-native/tokens"
import { Button } from "#components/ui/button"
import { Alert, AlertDescription, AlertTitle } from "#components/ui/alert"
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from "#components/ui/dialog"
import { Input } from "#components/ui/input"
import { ScrollArea } from "#components/ui/scroll-area"
import { Separator } from "#components/ui/separator"
import type { DesktopShellState } from "./shell.ts"
import { formatRelativeTimestamp } from "./shell.ts"
import { DecisionSurface, ReactCommandPalette, ReactComposer } from "./react-composer.tsx"
import { ReviewSurface, StatusNotices } from "./react-review.tsx"
import { ConversationTimeline, SafeReactMarkdown } from "./react-timeline.tsx"
import { RedactedSensitiveText } from "./react-sensitive-text.tsx"
import { DESKTOP_STAGE_LABEL } from "./branding.ts"
import { projectDesktopSidebarDestinations } from "./sidebar-destinations.ts"
import "./react-workbench.css"

type ReactSidebarIconName = Extract<
  IconName,
  "ChatCompose" | "Chats" | "ChevronLeft" | "ChevronRight" | "Folder" | "Home" | "Menu" | "Search" | "Settings"
>

const sidebarIconAssets: Readonly<Record<ReactSidebarIconName, LucideIcon>> = {
  ChatCompose: SquarePen,
  Chats: MessageCircle,
  ChevronLeft,
  ChevronRight,
  Folder,
  Home: House,
  Menu: PanelLeft,
  Search,
  Settings,
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
  )
  const shown = state.history.visibleRootCount
  const searchOpen = state.presentation.sessionSearchOpen
  const searchRef = useRef<HTMLInputElement>(null)
  useEffect(() => {
    if (searchOpen) searchRef.current?.focus()
  }, [searchOpen])
  const closeSearch = (): void => {
    if (state.history.searchQuery !== "") dispatch(report, "HistorySearchChanged", "")
    dispatch(report, "DesktopSessionSearchDisclosureChanged", false)
  }
  return <aside
    ref={railRef}
    className="oa-react-session-rail"
    data-open={open ? "true" : "false"}
    aria-label="Sessions"
    onKeyDown={focusAdjacentSession}
  >
    <div className="oa-react-rail-windowbar" aria-label="Sidebar controls">
      <Button className="oa-react-rail-collapse" variant="ghost" size="icon-xs" type="button" onClick={onCollapse} aria-label="Collapse sidebar" title="Collapse sidebar">
        <ReactCatalogIcon name="Menu" />
      </Button>
      <div className="oa-react-history-controls" aria-label="Session navigation">
        <Button
          variant="ghost"
          size="icon-xs"
          type="button"
          disabled={!state.navigation.canGoBack}
          aria-label={state.navigation.backTitle === null ? "Back" : `Back to ${state.navigation.backTitle}`}
          title={state.navigation.backTitle === null ? "Back" : `Back to ${state.navigation.backTitle}`}
          onClick={() => dispatch(report, "DesktopNavigationBackRequested")}
        >
          <ReactCatalogIcon name="ChevronLeft" />
        </Button>
        <Button
          variant="ghost"
          size="icon-xs"
          type="button"
          disabled={!state.navigation.canGoForward}
          aria-label={state.navigation.forwardTitle === null ? "Forward" : `Forward to ${state.navigation.forwardTitle}`}
          title={state.navigation.forwardTitle === null ? "Forward" : `Forward to ${state.navigation.forwardTitle}`}
          onClick={() => dispatch(report, "DesktopNavigationForwardRequested")}
        >
          <ReactCatalogIcon name="ChevronRight" />
        </Button>
      </div>
    </div>
    <div className="oa-react-rail-titlebar">
      <div className="oa-react-rail-brand" aria-label={`OpenAgents ${DESKTOP_STAGE_LABEL}`}>
        <strong>OpenAgents</strong>
        <span className="oa-react-rail-stage" data-app-stage={DESKTOP_STAGE_LABEL.toLowerCase()}>
          {DESKTOP_STAGE_LABEL}
        </span>
      </div>
      <Button
        className="oa-react-search-trigger"
        variant="ghost"
        size="icon-xs"
        type="button"
        onClick={() => searchOpen ? closeSearch() : dispatch(report, "DesktopSessionSearchDisclosureChanged", true)}
        aria-label={searchOpen ? "Close session search" : "Search sessions"}
        aria-expanded={searchOpen}
        title={searchOpen ? "Close search" : "Search sessions"}
      >
        <ReactCatalogIcon name="Search" />
      </Button>
    </div>
    {searchOpen ? <label className="oa-react-search">
        <span className="sr-only">Search sessions</span>
        <Input
          ref={searchRef}
          type="search"
          value={state.history.searchQuery}
          placeholder="Search sessions"
          onKeyDown={event => {
            if (event.key !== "Escape") return
            event.preventDefault()
            closeSearch()
          }}
          onInput={event => dispatch(report, "HistorySearchChanged", event.currentTarget.value)}
        />
      </label> : null}
    <nav className="oa-react-primary-nav" aria-label="Primary">
      {destinations.map(destination => <Button
        key={destination.id}
        className="oa-react-primary-destination justify-start text-left"
        variant="ghost"
        type="button"
        data-sidebar-destination-id={destination.id}
        data-selected={destination.selected ? "true" : "false"}
        aria-current={destination.accessibilityCurrent}
        aria-label={destination.accessibilityLabel}
        onClick={() => {
          dispatch(report, destination.intent.name, destination.intent.payload)
          onDismiss()
        }}
      >
        <ReactCatalogIcon name={destination.icon} />
        <span>{destination.label}</span>
        {destination.indicator === null ? null : <i data-destination-indicator={destination.indicator.kind} aria-hidden="true" />}
      </Button>)}
    </nav>
    <p className="oa-react-section-label">Recent</p>
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
                onDismiss()
              }}
            >
              <span className="oa-react-session-title">{row.title}</span>
              <small className="oa-react-session-meta">{row.meta}</small>
            </Button>)}
      {state.history.searchQuery.trim() === "" && shown < state.history.catalog.roots.length
        ? <Button type="button" variant="outline" size="sm" className="oa-react-load-more" onClick={() => dispatch(report, "HistoryCatalogMoreRequested")}>Load more sessions</Button>
        : null}
    </nav>
    </ScrollArea>
    {state.codingCatalog.sessions.length === 0 ? null : <section className="oa-react-workspaces" aria-label="Coding workspaces">
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
  </aside>
}

export const WorkbenchShell = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement => {
  const [railOpen, setRailOpen] = useState(false)
  const [reviewOpen, setReviewOpen] = useState(false)
  const railCollapsed = state.presentation.sidebarCollapsed
  const [codexUpdateOpen, setCodexUpdateOpen] = useState(false)
  const [dismissedCodexVersion, setDismissedCodexVersion] = useState<string | null>(null)
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
  const openRail = (): void => {
    dispatch(report, "DesktopSidebarCollapsedChanged", false)
    setRailOpen(true)
  }
  const closeRail = (): void => {
    dispatch(report, "DesktopSidebarCollapsedChanged", true)
    setRailOpen(false)
  }
  const workspaceSurface = state.workspace === "home"
    ? <main className="oa-react-workspace-surface" data-react-workspace="home">
        <header><div><p>Project home</p><h1>Coding sessions</h1></div><span>{state.codingCatalog.authorityLabel}</span></header>
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
  return <div className="oa-react-workbench" data-en-react-surface="true" data-review-open={reviewOpen ? "true" : "false"} data-rail-collapsed={railCollapsed ? "true" : "false"}>
    <ReactCommandPalette state={state} report={report} />
    <DecisionSurface state={state} report={report} />
    <Button
      ref={toggleRef}
      className="oa-react-sidebar-expand"
      variant="ghost"
      size="icon-sm"
      type="button"
      onClick={openRail}
      aria-expanded={railOpen}
      aria-label="Expand sidebar"
      title="Expand sidebar"
    ><ReactCatalogIcon name="Menu" /></Button>
    <SessionRail state={state} report={report} open={railOpen} onCollapse={closeRail} onDismiss={() => setRailOpen(false)} railRef={railRef} />
    {railOpen ? <button className="oa-react-rail-scrim" aria-label="Close sessions" onClick={() => setRailOpen(false)} /> : null}
    {workspaceSurface ?? <main className="oa-react-conversation" data-react-workspace="chat">
        <ConversationHeader state={state} report={report} reviewTriggerRef={reviewTriggerRef} onReviewOpen={() => setReviewOpen(true)} />
        <div className="oa-react-conversation-body">
          <StatusNotices state={state} report={report} />
          <ConversationTimeline page={state.history.page} notes={state.notes} loadingEdge={state.history.loadingEdge} working={state.pending} workingDirectory={state.workingDirectory} report={report} />
        </div>
        <ReactComposer state={state} report={report} />
      </main>}
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
