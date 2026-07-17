import { useEffect, useMemo, useRef, useState, type CSSProperties, type FormEvent, type ReactElement } from "react"
import { ChevronDown, ChevronRight, CircleStop, File, FileDiff, Folder, FolderOpen, Plus, RefreshCw, RotateCcw, Search, TerminalSquare, X } from "lucide-react"
import type { IntentError, IntentReporter, JsonPayload } from "@effect-native/core"
import { ComponentValueBinding, IntentRef } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"

import { Button } from "#components/ui/button"
import { Input } from "#components/ui/input"
import type { DesktopShellState } from "./shell.ts"
import { visibleWorkspaceRows } from "./workspace-browser.ts"
import { workspaceEditorTabDirty } from "./workspace-editor.ts"

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(report(
    payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()), payload,
  ) as Effect.Effect<void, IntentError>).catch(() => undefined)
}

const pathName = (pathRef: string): string => pathRef.split("/").at(-1) ?? pathRef

const WorkspaceTree = ({ state, report }: { readonly state: DesktopShellState; readonly report: IntentReporter }): ReactElement => {
  const browser = state.workspaceBrowser
  const projection = useMemo(() => visibleWorkspaceRows(browser), [browser])
  const searchMatches = browser.searchPage?.state === "available" ? browser.searchPage.matches : []
  const openEntry = (pathRef: string, kind: "file" | "directory"): void => {
    dispatch(report, "WorkspaceBrowserEntrySelected", pathRef)
    if (kind === "file" && browser.grantRef !== null) {
      dispatch(report, "WorkspaceEditorOpenRequested", { grantRef: browser.grantRef, pathRef })
    } else if (kind === "directory") {
      dispatch(report, "WorkspaceBrowserTreeToggled", pathRef)
    }
  }
  return <aside className="oa-react-files-tree" aria-label="Workspace files">
    <form className="oa-react-files-search" onSubmit={(event: FormEvent) => { event.preventDefault(); dispatch(report, "WorkspaceBrowserSearchRequested") }}>
      <Search aria-hidden="true" />
      <Input aria-label="Search workspace files" placeholder="Search files" value={browser.query}
        onChange={event => dispatch(report, "WorkspaceBrowserQueryChanged", event.currentTarget.value)} />
      <Button size="icon-sm" variant="ghost" type="button" aria-label="Refresh workspace files" onClick={() => dispatch(report, "WorkspaceBrowserRefreshRequested")}><RefreshCw aria-hidden="true" /></Button>
    </form>
    <div className="oa-react-files-search-modes" aria-label="Search mode">
      {(["path", "content"] as const).map(mode => <button aria-pressed={browser.searchMode === mode} key={mode} onClick={() => dispatch(report, "WorkspaceBrowserSearchModeSelected", mode)} type="button">{mode === "path" ? "Names" : "Contents"}</button>)}
    </div>
    <div className="oa-react-files-tree-scroll" role="tree">
      {browser.phase === "loading" ? <p role="status">Loading workspace files…</p>
        : browser.phase === "unavailable" ? <p role="alert">{browser.reason ?? "Workspace files are unavailable."}</p>
        : browser.searchState === "searching" ? <p role="status">Searching…</p>
        : browser.query.trim() !== "" && browser.searchPage !== null
          ? searchMatches.length === 0 ? <p>No matches.</p> : searchMatches.map(match => <button className="oa-react-file-search-result" key={`${match.pathRef}:${match.line ?? ""}`} onClick={() => openEntry(match.pathRef, "file")} type="button">
              <File aria-hidden="true" /><span><strong>{match.pathRef}</strong>{match.preview === null ? null : <small>{match.line === null ? "" : `Line ${match.line} · `}{match.preview}</small>}</span>
            </button>)
          : projection.rows.length === 0 ? <p>No workspace files.</p> : projection.rows.map(({ entry, depth }) => {
              const expanded = browser.expandedRefs.includes(entry.pathRef)
              return <button aria-level={depth + 1} aria-selected={browser.selectedRef === entry.pathRef} className="oa-react-file-tree-row" key={entry.pathRef}
                onClick={() => openEntry(entry.pathRef, entry.kind)} role="treeitem" style={{ "--oa-tree-depth": depth } as CSSProperties} type="button">
                {entry.kind === "directory" ? expanded ? <ChevronDown aria-hidden="true" /> : <ChevronRight aria-hidden="true" /> : <span aria-hidden="true" />}
                {entry.kind === "directory" ? expanded ? <FolderOpen aria-hidden="true" /> : <Folder aria-hidden="true" /> : <File aria-hidden="true" />}
                <span>{entry.name}</span>{entry.sizeBytes === null ? null : <small>{entry.sizeBytes.toLocaleString()} B</small>}
              </button>
            })}
      {projection.truncated ? <p>Showing the first 500 entries.</p> : null}
    </div>
  </aside>
}

const WorkspaceEditor = ({ state, report }: { readonly state: DesktopShellState; readonly report: IntentReporter }): ReactElement => {
  const editor = state.workspaceEditor
  const tab = editor.tabs.find(candidate => candidate.pathRef === editor.activePathRef) ?? null
  return <section className="oa-react-file-editor" aria-label="File editor">
    <div className="oa-react-file-tabs" role="tablist" aria-label="Open files">
      {editor.tabs.map(item => <div aria-selected={item.pathRef === editor.activePathRef} key={item.pathRef} role="tab">
        <button onClick={() => dispatch(report, "WorkspaceEditorTabSelected", item.pathRef)} type="button"><File aria-hidden="true" />{pathName(item.pathRef)}{workspaceEditorTabDirty(item) ? <span aria-label="Unsaved changes">•</span> : null}</button>
        <button aria-label={`Close ${item.pathRef}`} onClick={() => dispatch(report, "WorkspaceEditorTabCloseRequested", item.pathRef)} type="button"><X aria-hidden="true" /></button>
      </div>)}
    </div>
    {tab === null ? <div className="oa-react-editor-empty"><File aria-hidden="true" /><h3>No document open</h3><p>Select a text file from the workspace tree.</p></div> : <>
      <header className="oa-react-editor-toolbar">
        <span title={tab.pathRef}>{tab.pathRef}</span>
        <div>
          <Button size="sm" variant="ghost" disabled={tab.undo.length === 0} onClick={() => dispatch(report, "WorkspaceEditorUndoRequested")}>Undo</Button>
          <Button size="sm" variant="ghost" disabled={tab.redo.length === 0} onClick={() => dispatch(report, "WorkspaceEditorRedoRequested")}>Redo</Button>
          <Button size="sm" disabled={!workspaceEditorTabDirty(tab) || tab.saveState === "saving" || tab.phase === "unavailable"} onClick={() => dispatch(report, "WorkspaceEditorSaveRequested")}>{tab.saveState === "saving" ? "Saving…" : tab.saveState === "saved" ? "Saved" : "Save"}</Button>
        </div>
      </header>
      <div className="oa-react-editor-find">
        <Input aria-label={`Find in ${tab.pathRef}`} placeholder="Find" value={tab.findQuery} onChange={event => dispatch(report, "WorkspaceEditorFindChanged", event.currentTarget.value)} />
        <small>{tab.findMatches.length === 0 ? "No matches" : `${tab.findIndex + 1} of ${tab.findMatches.length}`}</small>
        <Button size="sm" variant="ghost" disabled={tab.findMatches.length === 0} onClick={() => dispatch(report, "WorkspaceEditorFindPrevious")}>Previous</Button>
        <Button size="sm" variant="ghost" disabled={tab.findMatches.length === 0} onClick={() => dispatch(report, "WorkspaceEditorFindNext")}>Next</Button>
      </div>
      {editor.closeConfirmRef !== tab.pathRef ? null : <div className="oa-react-editor-conflict" role="alert"><span>Discard unsaved changes?</span><Button size="sm" onClick={() => dispatch(report, "WorkspaceEditorTabCloseConfirmed", tab.pathRef)}>Discard</Button><Button size="sm" variant="ghost" onClick={() => dispatch(report, "WorkspaceEditorTabCloseCancelled")}>Keep editing</Button></div>}
      {tab.phase !== "conflict" ? null : <div className="oa-react-editor-conflict" role="alert"><span>{tab.externalDocument === null ? "The file is no longer available. Your draft is retained." : "Changed outside the editor."}</span>{tab.externalDocument === null ? null : <><Button size="sm" variant="outline" onClick={() => dispatch(report, "WorkspaceEditorConflictReload")}>Reload theirs</Button><Button size="sm" onClick={() => dispatch(report, "WorkspaceEditorConflictKeepMine")}>Save mine</Button></>}</div>}
      {tab.phase === "loading" ? <p role="status">Opening document…</p>
        : tab.phase === "unavailable" ? <p role="alert">{tab.reason ?? "This document is unavailable."}</p>
        : <textarea aria-label={`Editor for ${tab.pathRef}`} className="oa-react-code-editor" readOnly={tab.saveState === "saving"} spellCheck={false} value={tab.draft}
            onChange={event => dispatch(report, "WorkspaceEditorEventReceived", { type: "change", value: event.currentTarget.value })}
            onKeyDown={event => { if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === "s") { event.preventDefault(); dispatch(report, "WorkspaceEditorSaveRequested") } }} />}
    </>}
  </section>
}

export const ReactFilesSurface = ({ state, report }: { readonly state: DesktopShellState; readonly report: IntentReporter }): ReactElement =>
  <div className="oa-react-files-workbench" aria-label="Files surface"><WorkspaceTree state={state} report={report} /><WorkspaceEditor state={state} report={report} /></div>

type AnnotationDraft = Readonly<{ key: string; text: string }>

export const ReactReviewSurface = ({ state, report }: { readonly state: DesktopShellState; readonly report: IntentReporter }): ReactElement => {
  const [annotation, setAnnotation] = useState<AnnotationDraft | null>(null)
  const [annotations, setAnnotations] = useState<Readonly<Record<string, string>>>({})
  const status = state.git.status
  const changed = status === null ? [] : [
    ...status.staged.map(entry => ({ ...entry, source: "staged" as const })),
    ...status.unstaged.map(entry => ({ ...entry, source: "unstaged" as const })),
    ...status.untracked.map(entry => ({ ...entry, source: "unstaged" as const })),
  ]
  const diff = state.git.diff
  return <div className="oa-react-review-workbench" aria-label="Review surface">
    <aside className="oa-react-changed-files" aria-label="Changed files">
      <header><div><GitBranchLabel branch={status?.branch ?? null} /></div><Button size="icon-sm" variant="ghost" aria-label="Refresh changes" onClick={() => dispatch(report, "GitPanelRefreshRequested")}><RefreshCw aria-hidden="true" /></Button></header>
      {state.git.phase === "loading" ? <p role="status">Refreshing changes…</p>
        : state.git.phase === "unavailable" ? <p role="alert">{state.git.reason ?? "Review is unavailable."}</p>
        : changed.length === 0 ? <p>No local changes.</p> : changed.map(entry => <button disabled={entry.status === "untracked"} key={`${entry.source}:${entry.path}`} onClick={() => dispatch(report, "GitPanelDiffRequested", { path: entry.path, source: entry.source })} type="button">
            <span data-file-status={entry.status}>{entry.status.slice(0, 1).toLocaleUpperCase()}</span><strong>{entry.path}</strong><small>{entry.source}</small>
          </button>)}
    </aside>
    <section className="oa-react-rich-diff" aria-label="Rich diff">
      {diff === null ? <div className="oa-react-editor-empty"><FileDiff aria-hidden="true" /><h3>Select a changed file</h3><p>Review exact host-projected hunks without Git mutation controls.</p></div> : <>
        <header><div><strong>{diff.path}</strong><small>{diff.source} · {diff.hunks.length} {diff.hunks.length === 1 ? "hunk" : "hunks"}</small></div><div><Button size="sm" onClick={() => dispatch(report, "GitPanelContextAttached")}>Add to composer</Button><Button size="icon-sm" variant="ghost" aria-label="Close diff" onClick={() => dispatch(report, "GitPanelDiffClosed")}><X aria-hidden="true" /></Button></div></header>
        <div className="oa-react-diff-scroll">
          {diff.hunks.map((hunk, hunkIndex) => <section className="oa-react-diff-hunk" key={`${hunk.header}:${hunkIndex}`}>
            <h4>{hunk.header}</h4>
            {hunk.content.split("\n").map((line, lineIndex) => {
              const kind = line.startsWith("+") && !line.startsWith("+++") ? "add" : line.startsWith("-") && !line.startsWith("---") ? "remove" : "context"
              const key = `${hunkIndex}:${lineIndex}`
              return <div className="oa-react-diff-line-group" key={key}>
                <div className="oa-react-diff-line" data-kind={kind}><button aria-label={`Annotate line ${lineIndex + 1}`} onClick={() => setAnnotation({ key, text: annotations[key] ?? "" })} type="button">+</button><span>{lineIndex + 1}</span><code>{line || " "}</code></div>
                {annotations[key] === undefined ? null : <p className="oa-react-diff-annotation">{annotations[key]}</p>}
                {annotation?.key !== key ? null : <form className="oa-react-diff-annotation-form" onSubmit={event => { event.preventDefault(); if (annotation.text.trim() !== "") setAnnotations(current => ({ ...current, [key]: annotation.text.trim() })); setAnnotation(null) }}><Input autoFocus aria-label={`Comment on line ${lineIndex + 1}`} value={annotation.text} onChange={event => setAnnotation({ key, text: event.currentTarget.value })} /><Button size="sm" type="submit">Save note</Button><Button size="sm" variant="ghost" type="button" onClick={() => setAnnotation(null)}>Cancel</Button></form>}
              </div>
            })}
          </section>)}
        </div>
      </>}
    </section>
  </div>
}

const GitBranchLabel = ({ branch }: { readonly branch: string | null }): ReactElement => <><strong>{branch ?? "Repository"}</strong><small>Changes</small></>

export const ReactTerminalSurface = ({ state, report }: { readonly state: DesktopShellState; readonly report: IntentReporter }): ReactElement => {
  const terminal = state.terminal
  const active = terminal.sessions.find(session => session.sessionRef === terminal.activeRef) ?? null
  const outputRef = useRef<HTMLPreElement>(null)
  useEffect(() => {
    const output = outputRef.current
    if (output !== null) output.scrollTop = output.scrollHeight
  }, [active?.output])
  useEffect(() => {
    const output = outputRef.current
    if (output === null || active === null || typeof ResizeObserver === "undefined") return
    let last = ""
    const observer = new ResizeObserver(entries => {
      const box = entries[0]?.contentRect
      if (box === undefined) return
      const cols = Math.max(1, Math.min(1_000, Math.floor(box.width / 8)))
      const rows = Math.max(1, Math.min(1_000, Math.floor(box.height / 18)))
      const next = `${cols}:${rows}`
      if (next === last) return
      last = next
      dispatch(report, "TerminalResizeRequested", { cols, rows })
    })
    observer.observe(output)
    return () => observer.disconnect()
  }, [active?.sessionRef, report])
  return <section className="oa-react-terminal-workbench" aria-label="Terminal surface">
    <header className="oa-react-terminal-tabs">
      <div role="tablist" aria-label="Terminal sessions">
        {terminal.sessions.map((session, index) => <div aria-selected={session.sessionRef === terminal.activeRef} key={session.sessionRef} role="tab">
          <button onClick={() => dispatch(report, "TerminalSelected", session.sessionRef)} type="button"><TerminalSquare aria-hidden="true" /><span>{session.shellLabel || `Terminal ${index + 1}`}</span><small data-status={session.status}>{session.status}</small></button>
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
      <pre className="oa-react-terminal-output" ref={outputRef} aria-label={`Output for ${active.shellLabel}`} tabIndex={0}>{active.output || (active.status === "running" ? "Terminal ready." : `Process exited${active.exitCode === null ? "." : ` with code ${active.exitCode}.`}`)}</pre>
      {active.previews.length === 0 ? null : <div className="oa-react-terminal-previews" aria-label="Detected previews">{active.previews.map(preview => <button disabled={!preview.ready} key={preview.port} onClick={() => dispatch(report, "TerminalPreviewOpenRequested", preview.port)} type="button">{preview.ready ? "Open" : "Waiting for"} localhost:{preview.port}</button>)}</div>}
      <form className="oa-react-terminal-input" onSubmit={event => { event.preventDefault(); dispatch(report, "TerminalInputSubmitted") }}>
        <span aria-hidden="true">›</span><Input aria-label="Terminal input" autoComplete="off" disabled={active.status !== "running"} value={terminal.input} onChange={event => dispatch(report, "TerminalInputChanged", event.currentTarget.value)}
          onKeyDown={event => { if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === "c") { event.preventDefault(); dispatch(report, "TerminalInterruptRequested") } }} />
        <Button size="sm" disabled={active.status !== "running"} type="submit">Run</Button>
      </form>
    </>}
  </section>
}
