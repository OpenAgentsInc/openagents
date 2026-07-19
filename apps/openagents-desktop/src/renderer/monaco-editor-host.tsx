import { useEffect, useRef, useState, type ReactElement } from "react"
import { Effect } from "effect"

import {
  IdeDocumentGeneration,
  IdeDocumentSequence,
  IdeEditorViewRef,
  IdeMonacoModelVersion,
  IdeMonacoDocumentEventSchema,
  IdeVimProjectionSchema,
  type IdeMonacoDocumentEvent,
  type IdeVimProjection,
  makeIdeDocumentRef,
} from "../ide/monaco-document-contract.ts"
import {
  IdeMonacoAttachInputSchema,
  loadIdeMonacoRuntime,
  type IdeMonacoAttachInput,
  type IdeMonacoMountedView,
} from "../ide/monaco-runtime-loader.ts"
import type { WorkspaceEditorTab } from "./workspace-editor.ts"
import type { IdeMonacoEditorOptions } from "../ide/workbench-contract.ts"
import {
  IdeMonacoLocalLanguageStateSchema,
  type IdeMonacoLocalLanguageState,
  type IdeMonacoProjectLanguageProjection,
} from "../ide/language-contract.ts"

export interface MonacoEditorHostProps {
  readonly tab: WorkspaceEditorTab
  readonly view: "primary" | "secondary"
  readonly wordWrap: boolean
  readonly minimap: boolean
  readonly vimEnabled: boolean
  readonly editorOptions: IdeMonacoEditorOptions
  readonly projectLanguage: IdeMonacoProjectLanguageProjection | null
  readonly onEvent: (event: IdeMonacoDocumentEvent) => void
}

const documentRefFor = (tab: WorkspaceEditorTab) =>
  tab.documentRef ?? makeIdeDocumentRef(tab.document?.grantRef ?? "fixture-compatibility", 0)

const viewRefFor = (tab: WorkspaceEditorTab, view: MonacoEditorHostProps["view"]) =>
  IdeEditorViewRef.make(`ide.view.${String(documentRefFor(tab)).replaceAll(".", "-")}.${view}`)

const inputFor = (props: MonacoEditorHostProps): IdeMonacoAttachInput =>
  IdeMonacoAttachInputSchema.make({
    documentRef: documentRefFor(props.tab),
    generation: props.tab.generation ?? IdeDocumentGeneration.make(0),
    sequence: props.tab.incrementalSequence ?? IdeDocumentSequence.make(0),
    documentVersion: props.tab.modelVersion ?? IdeMonacoModelVersion.make(1),
    viewRef: viewRefFor(props.tab, props.view),
    pathLabel: props.tab.pathRef,
    language: props.tab.document?.languageMode ?? "plaintext",
    value: props.tab.draft,
    selection: props.tab.selection,
    selectionVersion: props.tab.selectionVersion,
    wordWrap: props.wordWrap,
    minimap: props.minimap,
    vimEnabled: props.vimEnabled,
    editorOptions: props.editorOptions,
    readOnly: props.tab.saveState === "saving" || props.tab.phase === "unavailable",
    projectLanguage: props.projectLanguage,
  })

export const MonacoEditorHost = (props: MonacoEditorHostProps): ReactElement => {
  const hostRef = useRef<HTMLDivElement>(null)
  const mountedRef = useRef<IdeMonacoMountedView | null>(null)
  const eventRef = useRef(props.onEvent)
  const propsRef = useRef(props)
  const [phase, setPhase] = useState<"loading" | "ready" | "unavailable">("loading")
  const [message, setMessage] = useState("Loading the offline editor…")
  const [localLanguage, setLocalLanguage] = useState<IdeMonacoLocalLanguageState | null>(null)
  const [vim, setVim] = useState<IdeVimProjection>(() => IdeVimProjectionSchema.make({
    enabled: props.vimEnabled,
    mode: props.vimEnabled ? "normal" : "insert",
    pending: null,
    count: null,
  }))
  eventRef.current = props.onEvent
  propsRef.current = props

  useEffect(() => {
    const abort = new AbortController()
    let mounted: IdeMonacoMountedView | null = null
    setPhase("loading")
    setMessage("Loading the offline editor…")
    setLocalLanguage(null)
    void Effect.runPromise(loadIdeMonacoRuntime(abort.signal)).then(runtime => {
      const host = hostRef.current
      if (abort.signal.aborted || host === null) return
      mounted = runtime.attach(
        host,
        inputFor(propsRef.current),
        event => eventRef.current(IdeMonacoDocumentEventSchema.make(event)),
        projection => setVim(IdeVimProjectionSchema.make(projection)),
        state => setLocalLanguage(IdeMonacoLocalLanguageStateSchema.make(state)),
      )
      mountedRef.current = mounted
      setPhase("ready")
      setMessage("Editor ready")
    }).catch(error => {
      if (abort.signal.aborted) return
      setPhase("unavailable")
      setMessage(error instanceof Error ? error.message : "The offline editor runtime is unavailable.")
    })
    return () => {
      abort.abort()
      mounted?.dispose()
      if (mountedRef.current === mounted) mountedRef.current = null
    }
  }, [props.tab.documentRef, props.tab.generation, props.tab.document?.grantRef, props.view])

  useEffect(() => {
    mountedRef.current?.update(inputFor(props))
  }, [props])

  const localLabel = localLanguage === null ? "Document local · waiting"
    : localLanguage._tag === "Ready" ? `Document local · ${localLanguage.language} worker ready`
    : localLanguage._tag === "Loading" ? `Document local · loading ${localLanguage.language}`
    : localLanguage._tag === "Unsupported" ? `Document local · syntax only (${localLanguage.language})`
    : `Document local · failed (${localLanguage.language})`
  const projectLabel = props.projectLanguage === null ? "Project language · no current evidence"
    : `Project language · generation ${props.projectLanguage.serviceGeneration}`

  return <section
    className="oa-react-monaco-pane"
    data-monaco-document-ref={documentRefFor(props.tab)}
    data-monaco-generation={props.tab.generation ?? 0}
    data-monaco-view={props.view}
    data-vim-enabled={props.vimEnabled ? "true" : "false"}
    aria-label={props.view === "primary" ? `Editor for ${props.tab.pathRef}` : `Secondary editor for ${props.tab.pathRef}`}
  >
    <div className="oa-react-monaco-host" ref={hostRef} />
    <footer className="oa-react-monaco-status" aria-live="polite">
      <span data-monaco-phase={phase}>{phase === "ready" ? `${props.tab.document?.languageMode ?? "plaintext"} · ${props.tab.document?.encoding ?? "utf-8"}` : message}</span>
      <span data-language-tier="document-local">{localLabel}</span>
      <span data-language-tier="project-local">{projectLabel}</span>
      <span>{(props.tab.gapRecoveries ?? 0) === 0 ? "Synced" : `${props.tab.gapRecoveries ?? 0} sequence gap${props.tab.gapRecoveries === 1 ? "" : "s"} recovered`}</span>
      <strong data-vim-mode={vim.mode}>{vim.enabled ? `Vim ${vim.mode.replaceAll("_", " ")}${vim.pending === null ? "" : ` · ${vim.pending}`}${vim.count === null ? "" : ` · ${vim.count}`}` : "Vim off"}</strong>
    </footer>
    {phase !== "unavailable" ? null : <div className="oa-react-monaco-unavailable" role="alert">
      <strong>Editor unavailable</strong>
      <span>{message}</span>
      <span>Your canonical draft remains in the document service and recovery store.</span>
    </div>}
  </section>
}
