import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type ReactElement } from "react"
import { ArrowLeft, ArrowRight, Camera, ChevronLeft, ChevronRight, CircleStop, Columns2, ExternalLink, File, FileDiff, Globe2, Monitor, MousePointer2, Pin, PinOff, Plus, RefreshCw, RotateCcw, Search, Settings2, Smartphone, Tablet, TerminalSquare, X } from "lucide-react"
import type { IntentError, IntentReporter, JsonPayload } from "@effect-native/core"
import { ComponentValueBinding, IntentRef } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"

import { Button } from "#components/ui/button"
import { Input } from "#components/ui/input"
import type { IdeExplorerCommand } from "./ide-path-index.ts"
import { PierreWorkspaceTree, pierreWorkspacePaths } from "./ide/pierre-tree-adapter.tsx"
import type { DesktopShellState } from "./shell.ts"
import { workspaceEditorTabDirty } from "./workspace-editor.ts"
import { MonacoEditorHost } from "./monaco-editor-host.tsx"
import { resolveIdeMonacoEditorOptions } from "../ide/workbench-contract.ts"
import { PierreReviewAdapter, type PierreDiffAnnotation } from "../ide/pierre-diffs-adapter.tsx"
import type { IdeReviewIntent, IdeReviewSelection } from "../ide/review-contract.ts"
import { activeGitReviewSource } from "./ide/review-source.ts"
import { AgentProposalList, AgentProposalReviewPanel } from "./react-agent-code.tsx"
import { AgentContextTray } from "./react-agent-context.tsx"
import { ReactIdeCursor } from "./ide/react-cursor.tsx"
import { ReactIdeDebugPanel } from "./ide/react-debug.tsx"
import { ReactIdeRunPanel, type IdeRunPanelMode } from "./ide/react-run.tsx"
import { ReactXtermProjection } from "./ide/react-xterm.tsx"
import {
  languageItemsFor,
  languageResultFor,
  monacoProjectLanguageProjection,
} from "../ide/language-workbench-contract.ts"
import "./react-managed-sandbox-surface.css"

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(report(
    payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()), payload,
  ) as Effect.Effect<void, IntentError>).catch(() => undefined)
}

const pathName = (pathRef: string): string => pathRef.split("/").at(-1) ?? pathRef

function tablistKey<Ref extends string>(
  event: KeyboardEvent<HTMLElement>,
  refs: ReadonlyArray<Ref>,
  activeRef: Ref | null,
  select: (ref: Ref) => void,
): void {
  if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key) || refs.length === 0) return
  event.preventDefault()
  // Roving-tab keyboard navigation starts from the focused tab. The selected
  // tab can legitimately differ while a user moves focus through the list;
  // basing the next item on selection would skip tabs and break ARIA's
  // expected Arrow-key behavior.
  const focused = [...event.currentTarget.querySelectorAll<HTMLElement>('[role="tab"]')]
    .findIndex(tab => tab === event.currentTarget.ownerDocument.activeElement)
  const current = focused >= 0
    ? focused
    : activeRef === null ? 0 : Math.max(0, refs.indexOf(activeRef))
  const next = event.key === "Home" ? 0 : event.key === "End" ? refs.length - 1
    : (current + (event.key === "ArrowRight" ? 1 : -1) + refs.length) % refs.length
  select(refs[next]!)
  const tab = event.currentTarget.querySelectorAll<HTMLElement>('[role="tab"]')[next]
  const focusTarget = tab instanceof HTMLButtonElement ? tab : tab?.querySelector<HTMLButtonElement>("button:first-child")
  focusTarget?.focus()
}

export const ReactFilesSidebar = ({ state, report }: { readonly state: DesktopShellState; readonly report: IntentReporter }): ReactElement => {
  const browser = state.workspaceBrowser
  const [createKind, setCreateKind] = useState<"file" | "directory" | null>(null)
  const projection = browser.pathIndexProjection
  const pierrePaths = pierreWorkspacePaths(projection)
  const searchMatches = browser.searchPage?.state === "available" ? browser.searchPage.matches : []
  const openEntry = (pathRef: string, kind: "file" | "directory", source: "explorer" | "workspace_search" = "explorer"): void => {
    dispatch(report, "WorkspaceBrowserEntrySelected", pathRef)
    if (kind === "file" && browser.grantRef !== null) {
      dispatch(report, "WorkspaceEditorOpenRequested", { grantRef: browser.grantRef, pathRef, ...(browser.pathIndexSnapshot === null ? {} : { source, identity: browser.pathIndexSnapshot.identity }) })
    } else if (kind === "directory") {
      dispatch(report, "WorkspaceBrowserTreeToggled", pathRef)
    }
  }
  const onExplorerIntent = (intent: IdeExplorerCommand): void => {
    dispatch(report, "WorkspaceBrowserExplorerCommandRequested", intent)
    if (intent._tag !== "Open") return
    const node = projection?.nodes.find(candidate => candidate.nodeRef === intent.nodeRef)
    if (node?.kind === "file" && browser.grantRef !== null) {
      dispatch(report, "WorkspaceEditorOpenRequested", { grantRef: browser.grantRef, pathRef: node.pathRef, ...(browser.pathIndexSnapshot === null ? {} : { source: "explorer", identity: browser.pathIndexSnapshot.identity }) })
    } else if (node?.kind === "directory") {
      dispatch(report, "WorkspaceBrowserTreeToggled", node.pathRef)
    }
  }
  const indexStatus = projection?.state
  return <section className="oa-react-files-tree" aria-label="Workspace files">
    <header className="oa-react-files-tree-toolbar">
      <strong>Explorer</strong>
      <Button size="sm" variant="ghost" onClick={() => { setCreateKind("file"); dispatch(report, "WorkspaceBrowserCreateStarted", { parentRef: "", kind: "file" }) }}>New file</Button>
      <Button size="sm" variant="ghost" onClick={() => { setCreateKind("directory"); dispatch(report, "WorkspaceBrowserCreateStarted", { parentRef: "", kind: "directory" }) }}>New folder</Button>
      <Button size="sm" variant="ghost" onClick={() => dispatch(report, "WorkspaceBrowserExplorerCommandRequested", { _tag: "Rescan" })}>Rescan</Button>
    </header>
    {createKind === null ? null : <form className="oa-react-files-create" onSubmit={event => { event.preventDefault(); dispatch(report, "WorkspaceBrowserEditorSubmitted"); setCreateKind(null) }}>
      <Input autoFocus aria-label={`New ${createKind} name`} placeholder={createKind === "file" ? "new-file.ts" : "new-folder"} value={browser.editor?.value ?? ""} onChange={event => dispatch(report, "WorkspaceBrowserEditorChanged", event.currentTarget.value)} />
      <Button size="sm" type="submit">Create</Button><Button size="sm" variant="ghost" type="button" onClick={() => { setCreateKind(null); dispatch(report, "WorkspaceBrowserEditorCancelled") }}>Cancel</Button>
    </form>}
    <p id="oa-workspace-index-status" role="status">
      {indexStatus === undefined ? "Path index unavailable."
        : indexStatus._tag === "Scanning" ? `Indexing ${indexStatus.progress.admittedNodes} files; ${indexStatus.progress.pendingDirectories} folders remain.`
        : indexStatus._tag === "Partial" ? `Partial index: ${indexStatus.progress.admittedNodes} paths are ready; ${indexStatus.progress.pendingDirectories} folders remain.`
        : indexStatus._tag === "Truncated" ? `Index limited to ${indexStatus.limit} paths. Narrow the root or search.`
        : indexStatus._tag === "Degraded" ? `Index degraded: ${indexStatus.reason}`
        : indexStatus._tag === "Unavailable" ? `Index unavailable: ${indexStatus.message}`
        : indexStatus._tag === "Error" ? `Index error: ${indexStatus.message}`
        : indexStatus._tag === "Empty" ? "The indexed workspace has no admitted files."
        : indexStatus._tag === "Stopped" ? `Index stopped: ${indexStatus.reason}`
        : `${indexStatus.nodeCount} indexed paths ready.`}
    </p>
    <div className={`oa-react-files-tree-scroll${browser.phase === "ready" && browser.query.trim() === "" && pierrePaths.length > 0 ? " oa-react-files-tree-scroll--pierre" : ""}`}>
      {browser.phase === "idle" ? <p role="status">Preparing workspace files…</p>
        : browser.phase === "loading" ? <p role="status">Loading workspace files…</p>
        : browser.phase === "unavailable" ? <p role="alert">{browser.reason ?? "Workspace files are unavailable."}</p>
        : browser.searchState === "searching" ? <p role="status">Searching…</p>
        : browser.query.trim() !== "" && browser.searchPage !== null
          ? searchMatches.length === 0 ? <p>No matches.</p> : searchMatches.map(match => <button className="oa-react-file-search-result" key={`${match.pathRef}:${match.line ?? ""}`} onClick={() => openEntry(match.pathRef, "file", "workspace_search")} type="button">
              <File aria-hidden="true" /><span><strong>{match.pathRef}</strong>{match.preview === null ? null : <small>{match.line === null ? "" : `Line ${match.line} · `}{match.preview}</small>}</span>
            </button>)
          : pierrePaths.length === 0 ? <p>No indexed workspace files.</p>
            : projection === null ? <p role="alert">The path index projection is unavailable.</p>
              : <PierreWorkspaceTree projection={projection} onIntent={onExplorerIntent} />}
    </div>
  </section>
}

const managedSandboxIsolationLabel = (isolation: "gce_vm" | "firecracker_microvm"): string =>
  isolation === "gce_vm" ? "GCE VM" : "Firecracker microVM"

export const ManagedSandboxPlacement = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement => {
  const snapshot = state.managedSandbox
  const admission = snapshot.admission
  const resource = snapshot.resource
  const target = resource?.target ?? (admission._tag === "Available" ? admission.target : null)
  const imageDigest = resource?.imageDigest ?? (admission._tag === "Available" ? admission.imageDigest : null)
  const profileRef = resource?.profileRef ?? (admission._tag === "Available" ? admission.profileRef : null)
  const lease = resource?.lease ?? (admission._tag === "Available" ? admission.lease : null)
  const budget = resource?.budget ?? (admission._tag === "Available" ? admission.budget : null)
  const capabilities = resource?.capabilities ?? (admission._tag === "Available" ? admission.requestedCapabilities : [])
  const lifecycle = resource?.facts.lifecycle ?? "uncreated"
  const attached = state.agentCode.attachment !== null
  const hasResource = resource !== null && resource.facts.lifecycle !== "deleted"
  const canCreate = admission._tag === "Available" && attached && !hasResource
  const canStop = resource !== null && ["ready", "idle", "running", "failed", "recovery_required"].includes(resource.facts.lifecycle)
  const canResume = resource?.facts.lifecycle === "stopped"
  const canInterrupt = resource?.facts.lifecycle === "running" && snapshot.turn !== null && ["pending", "running"].includes(snapshot.turn.status)
  const canDelete = resource !== null && !["deleting", "deleted"].includes(resource.facts.lifecycle)
  const unavailable = admission._tag === "Unavailable"
  const custody = resource?.target.dataPosture ?? (admission._tag === "Available" ? admission.custody : null)
  const maxCost = budget === null ? null : `$${(budget.maxCostMicros / 1_000_000).toFixed(2)} max`
  return <section className="oa-managed-sandbox-placement" aria-label="OpenAgents-managed placement" data-state={lifecycle}>
    <div className="oa-managed-sandbox-primary">
      <span className="oa-managed-sandbox-mark" aria-hidden="true"><Monitor /></span>
      <div>
        <strong>OpenAgents-managed placement</strong>
        <span>{unavailable ? admission.reason : target === null ? "Admission available; placement not created." : `${managedSandboxIsolationLabel(target.isolation)} · ${target.region}`}</span>
      </div>
      <span className="oa-managed-sandbox-lifecycle">{lifecycle.replaceAll("_", " ")}</span>
    </div>
    <dl className="oa-managed-sandbox-facts">
      <div><dt>Target</dt><dd>{target === null ? "Unavailable" : `${target.provider} / ${managedSandboxIsolationLabel(target.isolation)}`}</dd></div>
      <div><dt>Image</dt><dd title={imageDigest ?? undefined}>{imageDigest === null ? "—" : `${imageDigest.slice(0, 19)}…`}</dd></div>
      <div><dt>Profile</dt><dd>{profileRef ?? "—"}</dd></div>
      <div><dt>Custody</dt><dd>{custody?.replaceAll("_", " ") ?? "—"}</dd></div>
      <div><dt>Signal</dt><dd>{snapshot.freshness} · {snapshot.latencyClass.replaceAll("_", " ")}</dd></div>
      <div><dt>Generation</dt><dd>{resource === null ? "—" : `${resource.resourceGeneration} / v${resource.version}`}</dd></div>
      <div><dt>Lease</dt><dd>{lease === null ? "—" : `${lease.state} · ${new Date(lease.expiresAt).toLocaleString()}`}</dd></div>
      <div><dt>Cost cap</dt><dd>{maxCost ?? "—"}</dd></div>
      <div className="oa-managed-sandbox-capabilities"><dt>Capabilities</dt><dd>{capabilities.length === 0 ? "None admitted" : capabilities.map(capability => `${capability.kind}:${capability.state}`).join(" · ")}</dd></div>
    </dl>
    <div className="oa-managed-sandbox-actions">
      <Button size="sm" variant="ghost" onClick={() => dispatch(report, "DesktopManagedSandboxAdmissionRefreshed")}><RefreshCw aria-hidden="true" />Refresh</Button>
      <Button size="sm" disabled={!canCreate} onClick={() => dispatch(report, "DesktopManagedSandboxCreateRequested")}>Create</Button>
      <Button size="sm" variant="outline" disabled={!hasResource || !attached} onClick={() => dispatch(report, "DesktopManagedSandboxInspectRequested")}>Inspect</Button>
      <Button size="sm" variant="outline" disabled={!canStop || !attached} onClick={() => dispatch(report, "DesktopManagedSandboxStopRequested")}>Stop</Button>
      <Button size="sm" variant="outline" disabled={!canResume || !attached} onClick={() => dispatch(report, "DesktopManagedSandboxResumeRequested")}>Resume</Button>
      <Button size="sm" variant="outline" disabled={!canInterrupt || !attached} onClick={() => dispatch(report, "DesktopManagedSandboxInterruptRequested")}>Interrupt</Button>
      <Button size="sm" variant="ghost" disabled={!canDelete || !attached} onClick={() => dispatch(report, "DesktopManagedSandboxDeleteRequested")}>Delete</Button>
      {state.managedSandboxNotice === null ? null : <span role="status" aria-live="polite">{state.managedSandboxNotice}</span>}
    </div>
  </section>
}

export const ReactWorkspaceEditor = ({ state, report }: { readonly state: DesktopShellState; readonly report: IntentReporter }): ReactElement => {
  const editor = state.workspaceEditor
  const tab = editor.tabs.find(candidate => candidate.pathRef === editor.activePathRef) ?? null
  const [quickQuery, setQuickQuery] = useState("")
  const [symbolQuery, setSymbolQuery] = useState("")
  const [renameQuery, setRenameQuery] = useState("")
  const [settingsOpen, setSettingsOpen] = useState(false)
  const paths = (state.workspaceBrowser.pathIndexProjection?.nodes ?? [])
    .filter(node => node.kind === "file")
    .map(node => node.pathRef)
  const editorOptions = resolveIdeMonacoEditorOptions(editor.workbench.settings)
  const projectLanguage = tab?.documentRef === undefined || tab.generation === undefined || tab.modelVersion === undefined
    ? null
    : monacoProjectLanguageProjection(editor.language, {
        documentRef: tab.documentRef,
        documentGeneration: tab.generation,
        documentVersion: tab.modelVersion,
      })
  const languageBinding = tab?.documentRef === undefined || tab.generation === undefined || tab.modelVersion === undefined
    ? null
    : { documentRef: tab.documentRef, documentGeneration: tab.generation, documentVersion: tab.modelVersion }
  const languageItems = <Capability extends Parameters<typeof languageItemsFor>[1]["capability"]>(capability: Capability) =>
    languageBinding === null ? [] : languageItemsFor(editor.language, { ...languageBinding, capability })
  const diagnosticResult = languageBinding === null ? null : languageResultFor(editor.language, { ...languageBinding, capability: "diagnostics" })
  const diagnostics = languageItems("diagnostics").filter(item => item._tag === "Diagnostic")
    .filter(item => editor.language.problemsFilter === "all" ||
      (editor.language.problemsFilter === "errors" && item.severity === "error") ||
      (editor.language.problemsFilter === "warnings" && item.severity === "warning"))
  const allSymbols = languageItems("document_symbols").filter(item => item._tag === "Symbol")
  const symbols = allSymbols
    .filter(item => symbolQuery.trim() === "" || item.name.toLocaleLowerCase().includes(symbolQuery.trim().toLocaleLowerCase()))
  const references = languageItems("references").filter(item => item._tag === "Location")
  const definitions = languageItems("definition").filter(item => item._tag === "Location")
  const editResult = languageBinding === null ? null : [...editor.language.results].reverse().find(result =>
    result.documentRef === languageBinding.documentRef &&
    result.documentGeneration === languageBinding.documentGeneration &&
    result.documentVersion === languageBinding.documentVersion &&
    result.items.some(item => item._tag === "TextEdit") &&
    result.state._tag !== "Stale" && result.state._tag !== "Cancelled") ?? null
  const activeSymbol = allSymbols.find(symbol => symbol.range !== null && tab !== null &&
    tab.selection.start >= symbol.range.start.offset && tab.selection.start <= symbol.range.end.offset) ?? null
  const projectEligible = tab?.document !== null && tab !== null && ["typescript", "javascript"].includes(tab.document.languageMode)
  const languageStatus = !projectEligible ? "Project language off for this document"
    : editor.language.service._tag === "Ready" ? `Project ${editor.language.service.providerVersion} · generation ${editor.language.service.serviceGeneration}`
    : editor.language.service._tag === "Starting" ? "Project language starting…"
    : editor.language.service._tag === "Degraded" ? `Project language degraded · ${editor.language.service.reason}`
    : editor.language.service._tag === "Failed" ? `Project language failed · ${editor.language.service.reason}`
    : editor.language.service._tag === "Stopped" ? "Project language stopped"
    : "Project language idle"
  const selectLanguageLocation = (location: { readonly itemRef: string; readonly pathRef: string }): void => {
    if (location.pathRef === tab?.pathRef) {
      dispatch(report, "WorkspaceEditorLanguageLocationSelected", location.itemRef)
      return
    }
    const grantRef = state.workspaceBrowser.grantRef
    const identity = state.workspaceBrowser.pathIndexSnapshot?.identity
    if (grantRef !== null && identity !== undefined) dispatch(report, "WorkspaceEditorOpenRequested", {
      grantRef,
      pathRef: location.pathRef,
      source: "workspace_search",
      identity,
    })
  }
  const openQuickOpen = (): void => {
    setQuickQuery("")
    dispatch(report, "WorkspaceEditorQuickOpenChanged", { query: "", paths })
  }
  useEffect(() => {
    const onKeyDown = (event: globalThis.KeyboardEvent): void => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLocaleLowerCase() !== "p") return
      event.preventDefault()
      openQuickOpen()
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [paths.join("\n")])
  return <section className="oa-react-file-editor" aria-label="File editor">
    <div className="oa-react-file-tab-actions">
      <Button size="icon-sm" variant="ghost" aria-label="Editor navigation back" disabled={editor.workbench.navigation.cursor <= 0} onClick={() => dispatch(report, "WorkspaceEditorNavigationStepped", "back")}><ArrowLeft aria-hidden="true" /></Button>
      <Button size="icon-sm" variant="ghost" aria-label="Editor navigation forward" disabled={editor.workbench.navigation.cursor >= editor.workbench.navigation.entries.length - 1} onClick={() => dispatch(report, "WorkspaceEditorNavigationStepped", "forward")}><ArrowRight aria-hidden="true" /></Button>
      <Button size="icon-sm" variant="ghost" aria-label="Quick Open" onClick={openQuickOpen}><Search aria-hidden="true" /></Button>
      <Button size="icon-sm" variant="ghost" aria-label="Reopen closed editor" disabled={editor.closedPathRefs.length === 0 || state.workspaceBrowser.grantRef === null} onClick={() => state.workspaceBrowser.grantRef === null ? undefined : dispatch(report, "WorkspaceEditorClosedTabReopened", state.workspaceBrowser.grantRef)}><RotateCcw aria-hidden="true" /></Button>
      <Button size="sm" variant="ghost" disabled={editor.tabs.length < 2} onClick={() => dispatch(report, "WorkspaceEditorTabsClosed", "others")}>Close others</Button>
      <Button size="sm" variant="ghost" disabled={editor.tabs.length < 2} onClick={() => dispatch(report, "WorkspaceEditorTabsClosed", "right")}>Close right</Button>
    </div>
    <div className="oa-react-file-tabs" role="tablist" aria-label="Open files" onKeyDown={event => tablistKey(event, editor.tabs.map(item => item.pathRef), editor.activePathRef, pathRef => dispatch(report, "WorkspaceEditorTabSelected", pathRef))}>
      {editor.tabs.map(item => <div aria-selected={item.pathRef === editor.activePathRef} key={item.pathRef} role="tab">
        <button data-tab-mode={item.tabMode ?? "pinned"} onDoubleClick={() => dispatch(report, "WorkspaceEditorTabModeChanged", { pathRef: item.pathRef, tabMode: "pinned" })} onClick={() => dispatch(report, "WorkspaceEditorTabSelected", item.pathRef)} tabIndex={item.pathRef === editor.activePathRef ? 0 : -1} type="button"><File aria-hidden="true" />{pathName(item.pathRef)}{workspaceEditorTabDirty(item) ? <span aria-label="Unsaved changes">•</span> : null}</button>
        <button aria-label={`Move ${item.pathRef} left`} disabled={editor.tabs[0]?.pathRef === item.pathRef} onClick={() => dispatch(report, "WorkspaceEditorTabMoved", { pathRef: item.pathRef, delta: -1 })} type="button"><ChevronLeft aria-hidden="true" /></button>
        <button aria-label={`Move ${item.pathRef} right`} disabled={editor.tabs.at(-1)?.pathRef === item.pathRef} onClick={() => dispatch(report, "WorkspaceEditorTabMoved", { pathRef: item.pathRef, delta: 1 })} type="button"><ChevronRight aria-hidden="true" /></button>
        <button aria-label={`${item.tabMode === "preview" ? "Pin" : "Unpin"} ${item.pathRef}`} onClick={() => dispatch(report, "WorkspaceEditorTabModeChanged", { pathRef: item.pathRef, tabMode: item.tabMode === "preview" ? "pinned" : "preview" })} type="button">{item.tabMode === "preview" ? <Pin aria-hidden="true" /> : <PinOff aria-hidden="true" />}</button>
        <button aria-label={`Close ${item.pathRef}`} onClick={() => dispatch(report, "WorkspaceEditorTabCloseRequested", item.pathRef)} type="button"><X aria-hidden="true" /></button>
      </div>)}
    </div>
    <ManagedSandboxPlacement state={state} report={report} />
    {editor.workbench.quickOpen.phase === "closed" ? null : <div className="oa-react-quick-open" role="dialog" aria-label="Quick Open">
      <div><Search aria-hidden="true" /><Input autoFocus aria-label="Search files by path" placeholder="Type a file name" value={quickQuery} onChange={event => { const query = event.currentTarget.value; setQuickQuery(query); dispatch(report, "WorkspaceEditorQuickOpenChanged", { query, paths }) }} /><Button size="icon-sm" variant="ghost" aria-label="Close Quick Open" onClick={() => dispatch(report, "WorkspaceEditorQuickOpenClosed")}><X aria-hidden="true" /></Button></div>
      <ol>{editor.workbench.quickOpen.results.slice(0, 12).map((result, index) => <li key={result.pathRef}><button aria-current={index === editor.workbench.quickOpen.activeIndex} onClick={() => { if (state.workspaceBrowser.grantRef !== null) dispatch(report, "WorkspaceEditorOpenRequested", { grantRef: state.workspaceBrowser.grantRef, pathRef: result.pathRef, preview: true, ...(state.workspaceBrowser.pathIndexSnapshot === null ? {} : { source: "quick_open", identity: state.workspaceBrowser.pathIndexSnapshot.identity }) }); dispatch(report, "WorkspaceEditorQuickOpenClosed") }} type="button"><File aria-hidden="true" /><span>{result.pathRef}</span><small>{result.score}</small></button></li>)}</ol>
      {editor.workbench.quickOpen.phase === "empty" ? <p>No indexed file matches this path query.</p> : null}
    </div>}
    {tab === null ? <div className="oa-react-editor-empty"><File aria-hidden="true" /><h3>No document open</h3><p>Select a text file from the workspace tree.</p></div> : <>
      <nav className="oa-react-editor-breadcrumbs" aria-label="Document breadcrumbs">{editor.workbench.breadcrumbs.map((item, index) => <span key={`${item.kind}:${index}`}>{index === 0 ? null : <ChevronRight aria-hidden="true" />}{item.label}</span>)}{activeSymbol === null ? null : <span><ChevronRight aria-hidden="true" />{activeSymbol.name}</span>}</nav>
      <header className="oa-react-editor-toolbar">
        <span title={tab.pathRef}>{tab.pathRef}</span>
        <div>
          <Button size="sm" variant="ghost" disabled={tab.undo.length === 0} onClick={() => dispatch(report, "WorkspaceEditorUndoRequested")}>Undo</Button>
          <Button size="sm" variant="ghost" disabled={tab.redo.length === 0} onClick={() => dispatch(report, "WorkspaceEditorRedoRequested")}>Redo</Button>
          <Button size="sm" variant="ghost" aria-pressed={editor.wordWrap} onClick={() => dispatch(report, "WorkspaceEditorWordWrapToggled")}>Wrap</Button>
          <Button size="sm" variant="ghost" aria-pressed={editor.minimap} onClick={() => dispatch(report, "WorkspaceEditorMinimapToggled")}>Minimap</Button>
          <span className="oa-react-language-status" data-language-service={editor.language.service._tag.toLocaleLowerCase()} title={languageStatus}>{languageStatus}</span>
          <Button size="sm" variant="ghost" disabled={!projectEligible} onClick={() => dispatch(report, "WorkspaceEditorLanguageRefreshRequested")}>Refresh language</Button>
          <Button size="sm" variant="ghost" disabled={!projectEligible} onClick={() => dispatch(report, "WorkspaceEditorLanguageCapabilityRequested", { capability: "definition", query: null })}>Definition</Button>
          <Button size="sm" variant="ghost" disabled={!projectEligible} onClick={() => dispatch(report, "WorkspaceEditorLanguageCapabilityRequested", { capability: "references", query: null })}>References</Button>
          <Button size="sm" variant="ghost" disabled={!projectEligible} onClick={() => dispatch(report, "WorkspaceEditorLanguageCapabilityRequested", { capability: "format_document", query: null })}>Format</Button>
          <Button size="sm" variant="ghost" disabled={!projectEligible} onClick={() => dispatch(report, "WorkspaceEditorLanguageCapabilityRequested", { capability: "code_actions", query: null })}>Code actions</Button>
          <Button size="sm" variant="ghost" aria-pressed={editor.split} onClick={() => dispatch(report, "WorkspaceEditorSplitToggled")}><Columns2 aria-hidden="true" />Split</Button>
          {editor.workbench.groups.map((group, index) => <Button size="sm" variant={editor.workbench.focusedGroupRef === group.groupRef ? "secondary" : "ghost"} key={group.groupRef} onClick={() => dispatch(report, "WorkspaceEditorGroupFocused", group.groupRef)}>Group {index + 1}</Button>)}
          <Button size="sm" variant={editor.vimEnabled ? "default" : "outline"} aria-pressed={editor.vimEnabled} onClick={() => dispatch(report, "WorkspaceEditorVimToggled")}>{editor.vimEnabled ? "Vim on" : "Vim off"}</Button>
          <Button size="sm" variant="outline" onClick={() => dispatch(report, "DesktopEditorFileAttached")}><FileDiff aria-hidden="true" />Add context</Button>
          {state.agentCodeNotice === null ? null : <span role="status" aria-live="polite">{state.agentCodeNotice}</span>}
          <Button size="icon-sm" variant={settingsOpen ? "secondary" : "ghost"} aria-label="Editor settings" aria-expanded={settingsOpen} onClick={() => setSettingsOpen(value => !value)}><Settings2 aria-hidden="true" /></Button>
          <Button size="sm" variant="ghost" onClick={() => dispatch(report, "WorkspaceEditorSaveAllRequested")}>Save all</Button>
          <Button size="sm" variant="outline" disabled={tab.saveState === "saving"} onClick={() => dispatch(report, "WorkspaceEditorSaveAsStarted")}>Save As</Button>
          <Button size="sm" disabled={!workspaceEditorTabDirty(tab) || tab.saveState === "saving" || tab.phase === "unavailable"} onClick={() => dispatch(report, "WorkspaceEditorSaveRequested")}>{tab.saveState === "saving" ? "Saving…" : tab.saveState === "saved" ? "Saved" : "Save"}</Button>
        </div>
      </header>
      {!settingsOpen ? null : <section className="oa-react-editor-settings" aria-label="Editor settings">
        <strong>Editor settings</strong><span>Workspace overrides take precedence over user and default values.</span>
        <label><input checked={editor.wordWrap} onChange={() => dispatch(report, "WorkspaceEditorWordWrapToggled")} type="checkbox" />Word wrap</label>
        <label><input checked={editor.minimap} onChange={() => dispatch(report, "WorkspaceEditorMinimapToggled")} type="checkbox" />Minimap</label>
        <label><input checked={editor.vimEnabled} onChange={() => dispatch(report, "WorkspaceEditorVimToggled")} type="checkbox" />Vim mode (user)</label>
        <label>Tab size<select value={String((editor.workbench.settings.overrides.findLast(value => value.id === "editor.tabSize")?.value as { value?: number } | undefined)?.value ?? 2)} onChange={event => dispatch(report, "WorkspaceEditorSettingChanged", { id: "editor.tabSize", scope: "workspace", value: { _tag: "Integer", value: Number(event.currentTarget.value) } })}><option value="2">2</option><option value="4">4</option><option value="8">8</option></select></label>
        <Button size="sm" variant="ghost" onClick={() => dispatch(report, "WorkspaceEditorSettingReset", { id: "editor.tabSize", scope: "workspace" })}>Reset workspace tab size</Button>
        {editor.workbench.settings.errors.map(error => <p role="alert" key={error}>{error}</p>)}
      </section>}
      {editor.saveAsPathRef === null ? null : <div className="oa-react-editor-save-as">
        <Input aria-label="New relative document path" placeholder="src/new-file.ts" value={editor.saveAsPathRef} onChange={event => dispatch(report, "WorkspaceEditorSaveAsChanged", event.currentTarget.value)} />
        <Button size="sm" disabled={editor.saveAsPathRef.trim() === ""} onClick={() => dispatch(report, "WorkspaceEditorSaveAsSubmitted")}>Create copy</Button>
        <Button size="sm" variant="ghost" onClick={() => dispatch(report, "WorkspaceEditorSaveAsCancelled")}>Cancel</Button>
      </div>}
      {editor.closeConfirmRef !== tab.pathRef ? null : <div className="oa-react-editor-conflict" role="alert"><span>Discard unsaved changes?</span><Button size="sm" onClick={() => dispatch(report, "WorkspaceEditorTabCloseConfirmed", tab.pathRef)}>Discard</Button><Button size="sm" variant="ghost" onClick={() => dispatch(report, "WorkspaceEditorTabCloseCancelled")}>Keep editing</Button></div>}
      {tab.phase !== "conflict" ? null : <div className="oa-react-editor-conflict" role="alert"><span>{tab.externalDocument === null ? "The file is no longer available. Your draft is retained." : "Changed outside the editor."}</span>{tab.externalDocument === null ? null : <><Button size="sm" variant="outline" onClick={() => dispatch(report, "WorkspaceEditorConflictReload")}>Reload theirs</Button><Button size="sm" onClick={() => dispatch(report, "WorkspaceEditorConflictKeepMine")}>Save mine</Button></>}</div>}
      <div className="oa-react-editor-work-area">
        <aside className="oa-react-editor-outline" aria-label="Outline">
          <header><strong>Outline</strong><Input aria-label="Filter document symbols" placeholder="Filter symbols" value={symbolQuery} onChange={event => setSymbolQuery(event.currentTarget.value)} /></header>
          {!projectEligible ? <p>Project symbols are not started for this document type.</p>
            : symbols.length === 0 ? <p>{editor.language.activeRequestRefs.length > 0 ? "Reading project symbols…" : editor.language.lastRejection?.message ?? "No current symbols."}</p>
            : <ol>{symbols.slice(0, 200).map(symbol => <li key={symbol.symbolRef}><button aria-current={editor.language.selectedSymbolRef === symbol.symbolRef} style={{ paddingInlineStart: `${8 + symbol.depth * 10}px` }} type="button" onClick={() => dispatch(report, "WorkspaceEditorSymbolSelected", symbol.symbolRef)}><span>{symbol.name}</span><small>{symbol.kind}</small></button></li>)}</ol>}
        </aside>
      {tab.phase === "loading" ? <p role="status">Opening document…</p>
        : tab.phase === "unavailable" ? <p role="alert">{tab.reason ?? "This document is unavailable."}</p>
        : <div className="oa-react-monaco-splits" data-split={editor.split ? "true" : "false"}>
            <MonacoEditorHost
              key={`${tab.documentRef}:primary`}
              tab={tab}
              view="primary"
              wordWrap={editor.wordWrap}
              minimap={editor.minimap}
              vimEnabled={editor.vimEnabled}
              editorOptions={editorOptions}
              projectLanguage={projectLanguage}
              onEvent={event => dispatch(report, "WorkspaceEditorMonacoEventReceived", event)}
            />
            {!editor.split ? null : <MonacoEditorHost
              key={`${tab.documentRef}:secondary`}
              tab={tab}
              view="secondary"
              wordWrap={editor.wordWrap}
              minimap={editor.minimap}
              vimEnabled={editor.vimEnabled}
              editorOptions={editorOptions}
              projectLanguage={projectLanguage}
              onEvent={event => dispatch(report, "WorkspaceEditorMonacoEventReceived", event)}
            />}
          </div>}
      </div>
      <AgentContextTray state={state} report={report} />
      <ReactIdeCursor state={state} report={report} />
      <section className="oa-react-language-panel" aria-label="Project language results">
        <header>
          <strong>Problems</strong>
          <button aria-pressed={editor.language.problemsFilter === "all"} type="button" onClick={() => dispatch(report, "WorkspaceEditorProblemsFilterChanged", "all")}>All {languageItems("diagnostics").filter(item => item._tag === "Diagnostic").length}</button>
          <button aria-pressed={editor.language.problemsFilter === "errors"} type="button" onClick={() => dispatch(report, "WorkspaceEditorProblemsFilterChanged", "errors")}>Errors</button>
          <button aria-pressed={editor.language.problemsFilter === "warnings"} type="button" onClick={() => dispatch(report, "WorkspaceEditorProblemsFilterChanged", "warnings")}>Warnings</button>
          <span>{diagnosticResult === null ? "No current diagnostic receipt" : `${diagnosticResult.state._tag} · ${diagnosticResult.freshnessMs} ms old · ${diagnosticResult.evidenceTier}`}</span>
        </header>
        <div className="oa-react-language-panel-grid">
          <div className="oa-react-problems-list">
            {!projectEligible ? <p>Project diagnostics are not started for this document type.</p>
              : diagnostics.length === 0 ? <p>{editor.language.activeRequestRefs.length > 0 ? "Checking the project…" : editor.language.lastRejection?.message ?? "No problems in the current project receipt."}</p>
              : <ol>{diagnostics.slice(0, 500).map(problem => <li key={problem.diagnosticRef}><button aria-current={editor.language.selectedProblemRef === problem.diagnosticRef} type="button" onClick={() => dispatch(report, "WorkspaceEditorProblemSelected", problem.diagnosticRef)}><b aria-label={problem.severity}>{problem.severity === "error" ? "E" : problem.severity === "warning" ? "W" : "I"}</b><span>{problem.message}</span><small>{problem.pathRef}:{problem.range?.start.line ?? "?"}</small></button></li>)}</ol>}
          </div>
          <aside className="oa-react-language-actions" aria-label="Language actions and locations">
            <form onSubmit={event => { event.preventDefault(); if (renameQuery.trim() !== "") dispatch(report, "WorkspaceEditorLanguageCapabilityRequested", { capability: "rename_preview", query: renameQuery.trim() }) }}>
              <Input aria-label="Rename symbol to" placeholder="Rename symbol to…" value={renameQuery} onChange={event => setRenameQuery(event.currentTarget.value)} />
              <Button size="sm" variant="outline" disabled={!projectEligible || renameQuery.trim() === ""} type="submit">Preview rename</Button>
            </form>
            {editResult === null ? null : <div className="oa-react-language-edit-preview"><span>{editResult.capability.replaceAll("_", " ")} · {editResult.items.filter(item => item._tag === "TextEdit").length} canonical edit(s)</span><Button size="sm" onClick={() => dispatch(report, "WorkspaceEditorLanguageEditsApplied", editResult.resultRef)}>Apply exact receipt</Button></div>}
            {definitions.length === 0 ? null : <div><strong>Definitions</strong>{definitions.slice(0, 20).map(location => <button key={location.itemRef} type="button" onClick={() => selectLanguageLocation(location)}>{location.pathRef}:{location.range?.start.line ?? "?"}</button>)}</div>}
            {references.length === 0 ? null : <div><strong>References</strong>{references.slice(0, 50).map(location => <button key={location.itemRef} type="button" onClick={() => selectLanguageLocation(location)}>{location.pathRef}:{location.range?.start.line ?? "?"}</button>)}</div>}
          </aside>
        </div>
      </section>
    </>}
  </section>
}

export const ReactReviewSurface = ({ state, report }: { readonly state: DesktopShellState; readonly report: IntentReporter }): ReactElement => {
  const [layout, setLayout] = useState<"unified" | "split">("unified")
  const [contextLines, setContextLines] = useState(20)
  const [selection, setSelection] = useState<IdeReviewSelection | null>(null)
  const [annotationDraft, setAnnotationDraft] = useState("")
  const [annotations, setAnnotations] = useState<ReadonlyArray<PierreDiffAnnotation>>([])
  const status = state.git.status
  const changed = status === null ? [] : [
    ...status.staged.map(entry => ({ ...entry, source: "staged" as const })),
    ...status.unstaged.map(entry => ({ ...entry, source: "unstaged" as const })),
    ...status.untracked.map(entry => ({ ...entry, source: "unstaged" as const })),
  ]
  const diff = state.git.diff
  const reviewSource = activeGitReviewSource(state)
  const onReviewIntent = (intent: IdeReviewIntent): void => {
    if (intent.action === "select") setSelection(intent.selection)
  }
  const openInEditor = (): void => {
    const identity = state.workspaceBrowser.pathIndexSnapshot?.identity
    if (reviewSource === null || reviewSource.pathRef === null || state.workspaceBrowser.grantRef === null || identity === undefined) return
    dispatch(report, "WorkspaceEditorOpenRequested", {
      grantRef: state.workspaceBrowser.grantRef,
      pathRef: reviewSource.pathRef,
      source: "review",
      identity,
    })
  }
  return <div className="oa-react-review-workbench" aria-label="Review surface">
    <aside className="oa-react-changed-files" aria-label="Changed files">
      <AgentProposalList state={state} report={report} />
      <header><div><GitBranchLabel branch={status?.branch ?? null} /></div><Button size="icon-sm" variant="ghost" aria-label="Refresh changes" onClick={() => dispatch(report, "GitPanelRefreshRequested")}><RefreshCw aria-hidden="true" /></Button></header>
      {state.git.phase === "loading" ? <p role="status">Refreshing changes…</p>
        : state.git.phase === "unavailable" ? <p role="alert">{state.git.reason ?? "Review is unavailable."}</p>
        : changed.length === 0 ? <p>No local changes.</p> : changed.map(entry => <div className="oa-react-changed-file" key={`${entry.source}:${entry.path}`}>
            <button disabled={entry.status === "untracked"} aria-label={`Review ${entry.path}`} onClick={() => dispatch(report, "GitPanelDiffRequested", { path: entry.path, source: entry.source })} type="button">
              <span data-file-status={entry.status}>{entry.status.slice(0, 1).toLocaleUpperCase()}</span><strong>{entry.path}</strong><small>{entry.source}</small>
            </button>
            <Button size="sm" variant="outline" aria-label={`${entry.source === "staged" ? "Unstage" : "Stage"} ${entry.path}`} onClick={() => dispatch(report, "GitPanelStageToggled", entry.path)}>
              {entry.source === "staged" ? "Unstage" : "Stage"}
            </Button>
            {entry.source === "unstaged" && entry.status !== "untracked" && entry.status !== "unmerged" ? <Button size="sm" variant="destructive" aria-label={`Discard unstaged change in ${entry.path}`} onClick={() => dispatch(report, "GitPanelDiscardRequested", entry.path)}>Discard…</Button> : null}
          </div>)}
      {status?.truncated === true ? <p role="status">The changed-file list is truncated. Narrow the repository view before a mutation.</p> : null}
      {state.git.discardConfirmPath === null ? null : <div role="alertdialog" aria-modal="true" aria-labelledby="oa-discard-title">
        <strong id="oa-discard-title">Discard the exact worktree change?</strong>
        <p>The host will keep a recovery receipt for {state.git.discardConfirmPath}. Staged and conflicted changes are refused.</p>
        <Button variant="destructive" onClick={() => dispatch(report, "GitPanelDiscardConfirmed")}>Discard change</Button>
        <Button variant="outline" onClick={() => dispatch(report, "GitPanelDiscardCancelled")}>Cancel</Button>
      </div>}
      <section aria-label="Commit and delivery">
        <label htmlFor="live-git-commit-message">Commit message</label>
        <Input id="live-git-commit-message" value={state.git.commitMessage} maxLength={20_000} onChange={event => dispatch(report, "GitPanelCommitMessageChanged", event.currentTarget.value)} />
        <Button size="sm" disabled={state.git.committing || state.git.commitMessage.trim() === "" || (status?.staged.length ?? 0) === 0} onClick={() => dispatch(report, "GitPanelCommitRequested")}>{state.git.committing ? "Committing…" : "Commit staged"}</Button>
        <Button size="sm" variant="outline" disabled={state.git.pushing || status === null || status.upstream === null} onClick={() => dispatch(report, "GitPanelPushRequested")}>{state.git.pushing ? "Pushing…" : "Push exact HEAD"}</Button>
        <label htmlFor="live-git-new-branch">New branch</label>
        <Input id="live-git-new-branch" value={state.git.newBranchName} onChange={event => dispatch(report, "GitPanelNewBranchNameChanged", event.currentTarget.value)} />
        <Button size="sm" variant="outline" disabled={status === null || state.git.newBranchName.trim() === ""} onClick={() => dispatch(report, "GitPanelBranchCreateRequested")}>Create and switch</Button>
        <dl aria-label="Delivery phase evidence">
          {(status?.delivery ?? []).map(fact => <div key={fact.phase} data-delivery-proven={fact.proven ? "true" : "false"}>
            <dt>{fact.phase.replaceAll("_", " ")}</dt>
            <dd>{fact.proven ? "Proven" : "Not proven"} · {fact.freshness}{fact.evidenceRefs.length === 0 ? "" : ` · ${fact.evidenceRefs.length} evidence reference${fact.evidenceRefs.length === 1 ? "" : "s"}`}</dd>
          </div>)}
        </dl>
      </section>
      {state.git.actionError === null ? null : <p role="alert">{state.git.actionError}</p>}
      {state.git.receipt === null ? null : <p role="status"><strong>{state.git.receipt.headline}</strong> {state.git.receipt.detail}</p>}
      {state.git.recoveryRef === null ? null : <Button size="sm" variant="outline" onClick={() => dispatch(report, "GitPanelRecoveryRequested")}>Recover discarded change</Button>}
    </aside>
    <section className="oa-react-rich-diff" aria-label="Versioned review">
      {state.agentReviewProposalRef !== null ? <AgentProposalReviewPanel state={state} report={report} />
        : diff === null ? <div className="oa-react-editor-empty"><FileDiff aria-hidden="true" /><h3>Select a changed file or agent proposal</h3><p>Review exact host-projected changes before any mutation is admitted.</p></div> : <>
        <header><div><strong>{diff.path}</strong><small>{reviewSource?._tag ?? diff.source} · {diff.hunks.length} {diff.hunks.length === 1 ? "hunk" : "hunks"}</small></div><div><Button size="sm" disabled={reviewSource === null} onClick={() => dispatch(report, "GitPanelContextAttached", selection)}>Add {selection === null ? "diff" : "selection"}</Button><Button size="icon-sm" variant="ghost" aria-label="Close diff" onClick={() => dispatch(report, "GitPanelDiffClosed")}><X aria-hidden="true" /></Button></div></header>
        {reviewSource === null ? <div className="oa-react-editor-empty" role="alert"><h3>Version identity unavailable</h3><p>Refresh Files and Git status before rendering this exact diff.</p></div> : <>
          <div className="oa-react-review-source-label"><strong>{reviewSource.base.label}</strong><span aria-hidden="true"> → </span><strong>{reviewSource.target.label}</strong><small>{reviewSource.lifecycle._tag}</small></div>
          <div className="oa-react-review-toolbar" role="toolbar" aria-label="Diff layout and context">
            <Button size="sm" variant={layout === "unified" ? "secondary" : "ghost"} aria-pressed={layout === "unified"} onClick={() => setLayout("unified")}>Unified</Button>
            <Button size="sm" variant={layout === "split" ? "secondary" : "ghost"} aria-pressed={layout === "split"} onClick={() => setLayout("split")}>Split</Button>
            <Button size="sm" variant="ghost" disabled={contextLines <= 5} onClick={() => setContextLines(value => Math.max(5, value - 5))}>Less context</Button>
            <span aria-live="polite">{contextLines} context lines</span>
            <Button size="sm" variant="ghost" disabled={contextLines >= 100} onClick={() => setContextLines(value => Math.min(100, value + 5))}>More context</Button>
            <Button size="sm" variant="ghost" onClick={openInEditor}>Open in editor</Button>
          </div>
          {selection === null ? <p className="oa-react-review-selection-hint">Select a line or range to annotate or attach bounded context.</p> : <form className="oa-react-diff-annotation-form" onSubmit={event => {
            event.preventDefault()
            const label = annotationDraft.trim()
            if (label === "") return
            setAnnotations(current => [...current, {
              kind: "comment",
              side: selection.startSide === "base" ? "deletions" : "additions",
              lineNumber: selection.startLine,
              label,
            }])
            setAnnotationDraft("")
          }}><Input aria-label={`Comment on selected line ${selection.startLine}`} maxLength={240} value={annotationDraft} onChange={event => setAnnotationDraft(event.currentTarget.value)} /><Button size="sm" type="submit">Add annotation</Button></form>}
          <div className="oa-react-diff-scroll">
            <PierreReviewAdapter source={reviewSource} options={{ mode: layout, contextLines, selection: null, annotations }} onIntent={onReviewIntent} />
          </div>
        </>}
      </>}
    </section>
  </div>
}

const GitBranchLabel = ({ branch }: { readonly branch: string | null }): ReactElement => <><strong>{branch ?? "Repository"}</strong><small>Changes</small></>

export const ReactTerminalSurface = ({ state, report }: { readonly state: DesktopShellState; readonly report: IntentReporter }): ReactElement => {
  const terminal = state.terminal
  const active = terminal.sessions.find(session => session.sessionRef === terminal.activeRef) ?? null
  const [mode, setMode] = useState<"terminal" | "debug" | IdeRunPanelMode>("terminal")
  const xtermInput = useCallback((data: string) => dispatch(report, "TerminalPtyInputReceived", data), [report])
  const xtermResize = useCallback((cols: number, rows: number) => dispatch(report, "TerminalResizeRequested", { cols, rows }), [report])
  const xtermPreview = useCallback((port: number) => dispatch(report, "TerminalPreviewOpenRequested", port), [report])
  return <section className="oa-react-terminal-workbench" aria-label="Terminal surface">
    <nav className="oa-react-run-modes" role="tablist" aria-label="Terminal debug tasks tests and Output" onKeyDown={event => tablistKey(event, ["terminal", "debug", "tasks", "tests", "output"], mode, candidate => setMode(candidate))}>
      {(["terminal", "debug", "tasks", "tests", "output"] as const).map((candidate) => <button aria-selected={mode === candidate} key={candidate} onClick={() => setMode(candidate)} role="tab" tabIndex={mode === candidate ? 0 : -1} type="button">{candidate === "output" ? "Output" : candidate[0]!.toLocaleUpperCase() + candidate.slice(1)}</button>)}
    </nav>
    {mode === "terminal" ? <>
    <header className="oa-react-terminal-tabs">
      <div role="tablist" aria-label="Terminal sessions" onKeyDown={event => tablistKey(event, terminal.sessions.map(session => session.sessionRef), terminal.activeRef, sessionRef => dispatch(report, "TerminalSelected", sessionRef))}>
        {terminal.sessions.map((session, index) => <div aria-selected={session.sessionRef === terminal.activeRef} key={session.sessionRef} role="tab">
          <button onClick={() => dispatch(report, "TerminalSelected", session.sessionRef)} tabIndex={session.sessionRef === terminal.activeRef ? 0 : -1} type="button"><TerminalSquare aria-hidden="true" /><span>{session.shellLabel || `Terminal ${index + 1}`}</span><small data-status={session.status}>{session.status}</small></button>
          <button aria-label={`Close ${session.shellLabel || `terminal ${index + 1}`}`} onClick={() => dispatch(report, "TerminalCloseRequested", session.sessionRef)} type="button"><X aria-hidden="true" /></button>
        </div>)}
      </div>
      <Button size="icon-sm" variant="ghost" aria-label="New terminal" onClick={() => dispatch(report, "TerminalCreateRequested")}><Plus aria-hidden="true" /></Button>
    </header>
    {terminal.notice === null ? null : <p className="oa-react-terminal-notice" role="alert">{terminal.notice}</p>}
    {active === null ? <div className="oa-react-editor-empty"><TerminalSquare aria-hidden="true" /><h3>No terminal open</h3><p>Start a generation-owned terminal in this workspace.</p><Button size="sm" onClick={() => dispatch(report, "TerminalCreateRequested")}>New terminal</Button></div> : <>
      <div className="oa-react-terminal-toolbar">
        <div><strong>{active.cwdLabel}</strong><small>{active.shellLabel}{active.recovered ? " · recovered" : ""}{active.gap ? " · output gap" : ""}</small></div>
        <div><Button size="sm" variant="ghost" disabled={active.output.length === 0} onClick={() => dispatch(report, "TerminalContextAttached")}><File aria-hidden="true" />Add output</Button><Button size="sm" variant="ghost" aria-label="Interrupt terminal" disabled={active.status !== "running"} onClick={() => dispatch(report, "TerminalInterruptRequested")}><CircleStop aria-hidden="true" />Interrupt</Button><Button size="sm" variant="ghost" onClick={() => dispatch(report, "TerminalRestartRequested")}><RotateCcw aria-hidden="true" />Restart</Button><Button size="icon-sm" variant="ghost" aria-label="Refresh terminals" onClick={() => dispatch(report, "TerminalRefreshRequested")}><RefreshCw aria-hidden="true" /></Button></div>
      </div>
      <ReactXtermProjection session={active} onInput={xtermInput} onResize={xtermResize} onOpenPreview={xtermPreview} />
      {active.previews.length === 0 ? null : <div className="oa-react-terminal-previews" aria-label="Detected previews">{active.previews.map(preview => <button disabled={!preview.ready} key={preview.port} onClick={() => dispatch(report, "TerminalPreviewOpenRequested", preview.port)} type="button">{preview.ready ? "Open" : "Waiting for"} localhost:{preview.port}</button>)}</div>}
    </>}
    </> : mode === "debug" ? <ReactIdeDebugPanel /> : <ReactIdeRunPanel mode={mode} />}
  </section>
}

type PreviewViewport = "responsive" | "mobile" | "tablet" | "desktop"
const previewViewports: ReadonlyArray<Readonly<{ id: PreviewViewport; label: string; width: string | null; icon: typeof Monitor }>> = [
  { id: "responsive", label: "Responsive", width: null, icon: Monitor },
  { id: "mobile", label: "Mobile", width: "390px", icon: Smartphone },
  { id: "tablet", label: "Tablet", width: "768px", icon: Tablet },
  { id: "desktop", label: "Desktop", width: "1280px", icon: Monitor },
]

export const ReactBrowserPreviewSurface = ({ state, report }: { readonly state: DesktopShellState; readonly report: IntentReporter }): ReactElement => {
  const available = state.terminal.sessions.flatMap(session => session.previews.map(preview => ({ ...preview, sessionRef: session.sessionRef })))
  const [selectedPort, setSelectedPort] = useState<number | null>(available[0]?.port ?? null)
  const [viewport, setViewport] = useState<PreviewViewport>("responsive")
  const [mode, setMode] = useState<"server" | "file">("server")
  const [annotating, setAnnotating] = useState(false)
  const [comment, setComment] = useState("")
  const selected = available.find(preview => preview.port === selectedPort) ?? available[0] ?? null
  const activeFile = state.workspaceEditor.tabs.find(tab => tab.pathRef === state.workspaceEditor.activePathRef) ?? null
  const selectedViewport = previewViewports.find(candidate => candidate.id === viewport)!
  return <section className="oa-react-browser-preview" aria-label="Browser preview surface">
    <header className="oa-react-browser-chrome">
      <div role="group" aria-label="Preview navigation"><Button aria-label="Back" disabled size="icon-sm" variant="ghost"><ArrowLeft aria-hidden="true" /></Button><Button aria-label="Forward" disabled size="icon-sm" variant="ghost"><ArrowRight aria-hidden="true" /></Button><Button aria-label="Refresh preview discovery" size="icon-sm" variant="ghost" onClick={() => dispatch(report, "TerminalRefreshRequested")}><RefreshCw aria-hidden="true" /></Button></div>
      <div className="oa-react-browser-address"><Globe2 aria-hidden="true" /><span>{mode === "server" ? selected?.url ?? "No local server detected" : activeFile?.pathRef ?? "No file selected"}</span></div>
      <Button aria-label="Open in system browser" disabled={selected === null || !selected.ready || mode !== "server"} size="icon-sm" variant="ghost" onClick={() => selected === null ? undefined : dispatch(report, "TerminalPreviewOpenRequested", selected.port)}><ExternalLink aria-hidden="true" /></Button>
      <Button aria-label="Annotate preview" aria-pressed={annotating} disabled={selected === null || mode !== "server"} size="icon-sm" variant={annotating ? "secondary" : "ghost"} onClick={() => setAnnotating(value => !value)}><MousePointer2 aria-hidden="true" /></Button>
      <Button aria-label="Preview recording unavailable" disabled title="Recording requires an admitted isolated browser host" size="icon-sm" variant="ghost"><Camera aria-hidden="true" /></Button>
    </header>
    <div className="oa-react-browser-toolbar">
      <div role="tablist" aria-label="Preview source"><button aria-selected={mode === "server"} onClick={() => setMode("server")} role="tab" type="button">Local server</button><button aria-selected={mode === "file"} disabled={activeFile === null} onClick={() => setMode("file")} role="tab" type="button">File</button></div>
      <label>Target<select aria-label="Detected local server" disabled={available.length === 0} value={selected?.port ?? ""} onChange={event => setSelectedPort(Number(event.currentTarget.value))}>{available.map(preview => <option key={`${preview.sessionRef}:${preview.port}`} value={preview.port}>localhost:{preview.port}{preview.ready ? "" : " · waiting"}</option>)}</select></label>
      <div role="group" aria-label="Preview viewport">{previewViewports.map(option => { const Icon = option.icon; return <button aria-label={option.label} aria-pressed={viewport === option.id} key={option.id} onClick={() => setViewport(option.id)} title={option.width === null ? option.label : `${option.label} · ${option.width}`} type="button"><Icon aria-hidden="true" /></button> })}</div>
    </div>
    {annotating && selected !== null ? <form className="oa-react-browser-annotation" onSubmit={event => { event.preventDefault(); if (comment.trim() === "") return; dispatch(report, "DesktopPreviewAnnotationAttached", { sessionRef: selected.sessionRef, port: selected.port, comment: comment.trim(), viewport }); setComment(""); setAnnotating(false) }}><Input autoFocus aria-label="Preview annotation" maxLength={2_000} placeholder="Describe what should change in this preview" value={comment} onChange={event => setComment(event.currentTarget.value)} /><Button size="sm" type="submit">Add to composer</Button><Button size="sm" variant="ghost" type="button" onClick={() => setAnnotating(false)}>Cancel</Button></form> : null}
    <div className="oa-react-browser-stage" data-viewport={viewport}>
      <div className="oa-react-browser-frame" style={selectedViewport.width === null ? undefined : { maxWidth: selectedViewport.width }}>
        {mode === "file" ? activeFile?.document === null || activeFile?.document === undefined ? <div className="oa-react-browser-empty"><File aria-hidden="true" /><h3>No previewable file</h3><p>Open a text file in Files, then return here.</p></div> : <pre aria-label={`File preview for ${activeFile.pathRef}`}>{activeFile.draft}</pre>
          : selected === null ? <div className="oa-react-browser-empty"><Globe2 aria-hidden="true" /><h3>No local server detected</h3><p>Start a dev server in Terminal. OpenAgents discovers only ports explicitly announced by its output.</p></div>
          : <div className="oa-react-browser-empty" data-ready={selected.ready ? "true" : "false"}><Globe2 aria-hidden="true" /><h3>{selected.ready ? `localhost:${selected.port} is ready` : `Waiting for localhost:${selected.port}`}</h3><p>For isolation, this verified local target opens in your system browser. Arbitrary in-app navigation, page credentials, and DOM automation remain unavailable.</p><Button disabled={!selected.ready} onClick={() => dispatch(report, "TerminalPreviewOpenRequested", selected.port)}><ExternalLink aria-hidden="true" />Open preview</Button></div>}
      </div>
    </div>
    <footer className="oa-react-browser-status"><span><span data-status={selected?.ready ? "ready" : "waiting"} />{selected?.ready ? "Server ready" : "No ready server"}</span><span><MousePointer2 aria-hidden="true" />Automation cursor unavailable without an admitted isolated browser host</span><span><Camera aria-hidden="true" />Recording unavailable</span></footer>
  </section>
}
