import {
  StrictMode,
  createElement,
  useEffect,
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
  CircleAlert,
  Download,
  LoaderCircle,
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
import { capabilityForHarness, desktopConversationShortcutLabel, formatRelativeTimestamp } from "./shell.ts"
import { DecisionSurface, ReactCommandPalette, ReactComposer } from "./react-composer.tsx"
import { StatusNotices } from "./react-review.tsx"
import { ConversationTimeline, SafeReactMarkdown } from "./react-timeline.tsx"
import { RedactedSensitiveText } from "./react-sensitive-text.tsx"
import { DESKTOP_STAGE_LABEL } from "./branding.ts"
import { projectDesktopSidebarDestinations } from "./sidebar-destinations.ts"

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

const selectedTitle = (state: DesktopShellState): string => {
  const local = state.threads.find(thread => thread.id === state.activeThreadId)
  if (local !== undefined) return local.title || "Untitled session"
  const selected = state.history.page?.rootThreadRef
  return state.history.catalog.roots.find(root => root.threadRef === selected)?.title ?? "New session"
}

const selectedLifecycle = (state: DesktopShellState): string => {
  if (state.activeThreadId !== null && state.pending) return "Running"
  if (state.history.pendingThreadRef !== null && state.history.pendingThreadRef !== undefined) return "Loading"
  const selected = state.history.page?.selectedThreadRef
  const status = state.history.catalog.agents.find(agent => agent.threadRef === selected)?.status
  if (status === "interrupted" || status === "errored") return "Needs attention"
  if (status === "running" || status === "waiting") return status === "running" ? "Running" : "Waiting"
  return "Ready"
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

const sharedRailIcon = (icon: "ChatCompose" | "Chats" | "Settings"): DesktopRailIcon => {
  if (icon === "ChatCompose") return "new-session"
  if (icon === "Chats") return "chat"
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
    state.workspace === "settings" ? "settings" : "chat",
    rows.some(row => row.selected),
  )
  const primaryDestinations = destinations.filter(destination => destination.id !== "shell-settings-toggle")
  const settingsDestination = destinations.find(destination => destination.id === "shell-settings-toggle")
  const [selectedSettingsSectionId, setSelectedSettingsSectionId] = useState("settings-general")
  const settingsSections: ReadonlyArray<DesktopRailDestination> = state.workspace !== "settings" ? [] : [
    { id: "settings-general", label: "General", icon: "general", selected: selectedSettingsSectionId === "settings-general", current: selectedSettingsSectionId === "settings-general" ? "page" : undefined },
    { id: "settings-codex", label: "Codex CLI", icon: "settings", selected: selectedSettingsSectionId === "settings-codex", current: selectedSettingsSectionId === "settings-codex" ? "page" : undefined },
    ...(state.settings.localCodexUsageControlAvailable ? [{ id: "settings-privacy", label: "Privacy", icon: "privacy" as const, selected: selectedSettingsSectionId === "settings-privacy", current: selectedSettingsSectionId === "settings-privacy" ? "page" as const : undefined }] : []),
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
    onBack={() => dispatch(report, "DesktopNavigationBackRequested")}
    onCollapse={onCollapse}
    onDestinationSelect={selected => {
      if (selected.id.startsWith("settings-")) {
        setSelectedSettingsSectionId(selected.id)
        const targetId = selected.id === "settings-codex"
          ? "react-runtime-maintenance-title"
          : selected.id === "settings-privacy"
            ? "react-local-usage-title"
            : selected.id === "settings-account"
              ? "react-provider-accounts-title"
              : "react-settings-title"
        document.getElementById(targetId)?.scrollIntoView({
          behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
          block: "start",
        })
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
  const workspaceSurface = state.workspace === "settings"
      ? <main className="oa-react-workspace-surface oa-react-settings-khala" data-react-workspace="settings">
          <StaticKhalaDecoration view={settingsFrame} placement="settings-frame" />
          <header className="oa-react-settings-header" id="react-settings-title">
            <StaticKhalaDecoration view={settingsHeaderAccent} placement="settings-header" />
            <div><p>OpenAgents</p><h1>Settings</h1></div>
            <Button type="button" variant="outline" onClick={() => dispatch(report, "DesktopHarnessMaintenanceRefreshRequested")}>Refresh</Button>
          </header>
          <section className="oa-react-settings-section" aria-labelledby="react-runtime-maintenance-title">
            <h2 id="react-runtime-maintenance-title">Codex CLI</h2>
            <p>Installed Codex version, channel, and update truth from this Mac.</p>
            {state.settings.harnessMaintenance.view.state === "loading" ? <p role="status">Checking harnesses…</p>
              : state.settings.harnessMaintenance.view.state === "unavailable" ? <p role="alert">{state.settings.harnessMaintenance.view.message}</p>
              : state.settings.harnessMaintenance.view.harnesses.filter(harness => harness.harness === "codex").map(harness => <article className="oa-react-settings-status-article" key={harness.harness} data-harness={harness.harness} data-status={harness.advisory}>
                  <div><strong>Codex</strong><span>{harness.installedVersion ?? "Not installed"} · {harness.channel}</span>{harness.recoveryMessage == null ? null : <small>{harness.recoveryMessage}</small>}</div>
                  <span className="oa-react-settings-status-label">{harness.advisory === "current" ? "Up to date" : harness.advisory === "behind_latest" ? "Update available" : "Version unknown"}</span>
                  {harness.updateSupported ? <Button type="button" size="sm" disabled={state.settings.harnessMaintenance.updating !== null} onClick={() => dispatch(report, "DesktopHarnessUpdateRequested", harness.harness)}>Update</Button> : null}
                </article>)}
            {state.settings.harnessMaintenance.lastOutcome === null ? null : <p role="status">{state.settings.harnessMaintenance.lastOutcome}</p>}
          </section>
          {state.settings.localCodexUsageControlAvailable
            ? <section className="oa-react-settings-section" aria-labelledby="react-local-usage-title">
                <h2 id="react-local-usage-title">Share local Codex usage</h2>
                <p>When on, OpenAgents reports how many tokens each turn used — the input, cached-input, output, reasoning, and total token counts — plus the model name and a one-time turn reference. Only those numbers are sent: never your prompts, responses, files, paths, account names, or credentials. This updates the aggregate public tokens-served counter. Turn it off any time; queued reports are deleted.</p>
                <Button
                  type="button"
                  variant={state.settings.shareLocalCodexUsage ? "default" : "outline"}
                  aria-pressed={state.settings.shareLocalCodexUsage}
                  onClick={() => dispatch(
                    report,
                    "DesktopLocalCodexUsageSharingToggled",
                    !state.settings.shareLocalCodexUsage,
                  )}
                >{state.settings.shareLocalCodexUsage ? "Sharing on" : "Sharing off"}</Button>
              </section>
            : null}
          <section className="oa-react-settings-section" aria-labelledby="react-provider-accounts-title">
            <h2 id="react-provider-accounts-title">Codex account</h2>
            <p>Your Codex account identity is blurred until you explicitly reveal it.</p>
            {state.fleet.phase === "loading" || state.fleet.phase === "idle"
              ? <p role="status">Checking provider accounts…</p>
              : state.fleet.accounts.filter(account => account.provider === "codex").length === 0
                ? <p>No Codex account connected.</p>
                : state.fleet.accounts.filter(account => account.provider === "codex").map(account => <article className="oa-react-settings-status-article" key={account.ref} data-provider-account={account.ref} data-status={account.readiness}>
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
      composer={state.history.page === null ? <div className="oa-react-composer-stack">
        <DecisionSurface state={state} report={report} />
        <ReactComposer state={state} report={report} />
      </div> : null}
      header={<ConversationHeader state={state} />}
      notices={<StatusNotices state={state} report={report} />}
      timeline={<ConversationTimeline page={state.history.page} notes={state.notes} loadingEdge={state.history.loadingEdge} working={state.activeThreadId !== null && state.pending && !waitingForAnswer} waitingForAnswer={waitingForAnswer} workingDirectory={state.workingDirectory} agentName={capabilityForHarness(state)?.displayName ?? (state.selectedHarness === "codex" ? "Codex" : "Claude")} report={report} />}
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
