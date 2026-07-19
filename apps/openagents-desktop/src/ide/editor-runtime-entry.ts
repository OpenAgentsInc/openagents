import * as monaco from "monaco-editor/esm/vs/editor/editor.api.js"
import "monaco-editor/min/vs/editor/editor.main.css"
import "./language-projection.css"
import "monaco-editor/esm/vs/language/css/monaco.contribution.js"
import "monaco-editor/esm/vs/language/html/monaco.contribution.js"
import * as jsonContributionModule from "monaco-editor/esm/vs/language/json/monaco.contribution.js"
import * as typeScriptContributionModule from "monaco-editor/esm/vs/language/typescript/monaco.contribution.js"
import CssWorkerUrl from "monaco-editor/esm/vs/language/css/css.worker?worker&url"
import EditorWorkerUrl from "monaco-editor/esm/vs/editor/editor.worker?worker&url"
import HtmlWorkerUrl from "monaco-editor/esm/vs/language/html/html.worker?worker&url"
import JsonWorkerUrl from "monaco-editor/esm/vs/language/json/json.worker?worker&url"
import TypeScriptWorkerUrl from "monaco-editor/esm/vs/language/typescript/ts.worker?worker&url"

import {
  IdeDocumentSequence,
  IdeMonacoModelVersion,
  IdeMonacoDocumentEventSchema,
  IdeMonacoRuntimeResourceSnapshotSchema,
  IdeVimProjectionSchema,
  type IdeMonacoDocumentEvent,
  type IdeVimMode,
  type IdeVimProjection,
} from "./monaco-document-contract.ts"
import {
  decodeIdeMonacoAttachInput,
  type IdeMonacoAttachInput,
  type IdeMonacoMountedView,
  type IdeMonacoRuntime,
} from "./monaco-runtime-loader.ts"
import {
  IdeMonacoLocalLanguageStateSchema,
  type IdeMonacoLocalLanguageState,
  type IdeMonacoProjectLanguageProjection,
} from "./language-contract.ts"
import { IdeServiceGenerationSchema } from "./project-contract.ts"
import { tokyoNightMonacoThemeData } from "./tokyo-night-theme.ts"

const workerUrls: Readonly<Record<string, string>> = {
  editor: EditorWorkerUrl,
  json: JsonWorkerUrl,
  css: CssWorkerUrl,
  scss: CssWorkerUrl,
  less: CssWorkerUrl,
  html: HtmlWorkerUrl,
  handlebars: HtmlWorkerUrl,
  razor: HtmlWorkerUrl,
  typescript: TypeScriptWorkerUrl,
  javascript: TypeScriptWorkerUrl,
}

const workers = new Set<Worker>()
let runtimeState: "ready" | "stopped" = "ready"
let listenerCount = 0
let vimHandlerCount = 0
let localWorkerGeneration = 0

const createTrackedWorker = (url: string, label: string): Worker => {
  const worker = new Worker(url, { type: "module", name: `oa-ide-${label || "editor"}` })
  workers.add(worker)
  const terminate = worker.terminate.bind(worker)
  worker.terminate = () => {
    workers.delete(worker)
    terminate()
  }
  return worker
}

globalThis.MonacoEnvironment = {
  getWorker: (_workerId: string, label: string) =>
    createTrackedWorker(workerUrls[label] ?? workerUrls.editor!, label),
}

monaco.editor.defineTheme("openagents-tokyo-night", tokyoNightMonacoThemeData())
monaco.editor.setTheme("openagents-tokyo-night")

type ModelEntry = {
  readonly model: monaco.editor.ITextModel
  readonly views: Set<string>
  readonly vimState: VimSharedState
  generation: number
  sequence: number
  syncing: boolean
  localLanguage: IdeMonacoLocalLanguageState
  localLanguageName: string | null
  localLanguageCallbacks: Set<(state: IdeMonacoLocalLanguageState) => void>
  localManualWorker: Worker | null
  localInput: IdeMonacoAttachInput
}

type ViewEntry = {
  readonly editor: monaco.editor.IStandaloneCodeEditor
  readonly modelEntry: ModelEntry
  readonly subscriptions: ReadonlyArray<monaco.IDisposable>
  readonly vim: VimModeController
  readonly localLanguageCallback: (state: IdeMonacoLocalLanguageState) => void
  readonly projectDecorations: monaco.editor.IEditorDecorationsCollection
  projectFoldingProvider: monaco.IDisposable | null
  input: IdeMonacoAttachInput
  disposed: boolean
}

const models = new Map<string, ModelEntry>()
const views = new Map<string, ViewEntry>()

const rangeText = (editor: monaco.editor.IStandaloneCodeEditor): string => {
  const model = editor.getModel()
  const selection = editor.getSelection()
  return model === null || selection === null ? "" : model.getValueInRange(selection)
}

type VimSharedState = {
  readonly marks: Map<string, monaco.Position>
  readonly registers: Map<string, string>
  lastEdit: string | null
}

class VimModeController {
  readonly #editor: monaco.editor.IStandaloneCodeEditor
  readonly #onProjection: (projection: IdeVimProjection) => void
  readonly #onSave: () => void
  readonly #onClose: (force: boolean) => void
  readonly #shared: VimSharedState
  #enabled = false
  #mode: IdeVimMode = "normal"
  #pending: string | null = null
  #count = ""
  #register = "\""
  #disposed = false
  #subscription: monaco.IDisposable | null = null
  #blurSubscription: monaco.IDisposable | null = null

  constructor(
    editor: monaco.editor.IStandaloneCodeEditor,
    onProjection: (projection: IdeVimProjection) => void,
    onSave: () => void,
    onClose: (force: boolean) => void,
    shared: VimSharedState,
  ) {
    this.#editor = editor
    this.#onProjection = onProjection
    this.#onSave = onSave
    this.#onClose = onClose
    this.#shared = shared
  }

  #project(): void {
    this.#onProjection(IdeVimProjectionSchema.make({
      enabled: this.#enabled,
      mode: this.#mode,
      pending: this.#pending,
      count: this.#count === "" ? null : Number.parseInt(this.#count, 10),
    }))
  }

  #setMode(mode: IdeVimMode, pending: string | null = null): void {
    this.#mode = mode
    this.#pending = pending
    this.#count = ""
    this.#project()
  }

  #trigger(command: string, payload: unknown = null): void {
    const count = Math.max(1, Number.parseInt(this.#count || "1", 10))
    for (let index = 0; index < count; index += 1) this.#editor.trigger("openagents.vim", command, payload)
    this.#count = ""
  }

  #move(command: string): void {
    const selecting = this.#mode === "visual" || this.#mode === "visual_line" || this.#mode === "visual_block" || this.#mode === "operator_pending"
    this.#trigger(selecting ? `${command}Select` : command)
  }

  #finishOperator(): void {
    const operator = this.#pending?.slice(0, 1)
    if (operator === "y") {
      const value = rangeText(this.#editor)
      this.#shared.registers.set(this.#register, value)
      this.#shared.registers.set("\"", value)
      const position = this.#editor.getSelection()?.getStartPosition() ?? this.#editor.getPosition()
      if (position !== null) this.#editor.setPosition(position)
    } else if (operator === "d" || operator === "c") {
      const value = rangeText(this.#editor)
      this.#shared.registers.set(this.#register, value)
      this.#shared.registers.set("\"", value)
      this.#trigger("editor.action.clipboardCutAction")
      this.#shared.lastEdit = operator
    }
    this.#setMode(operator === "c" ? "insert" : "normal")
  }

  #lineOperator(operator: string): void {
    this.#trigger("expandLineSelection")
    this.#pending = operator
    this.#finishOperator()
  }

  #insertRegister(after: boolean): void {
    const text = this.#shared.registers.get(this.#register) ?? this.#shared.registers.get("\"") ?? ""
    const model = this.#editor.getModel()
    const position = this.#editor.getPosition()
    if (model === null || position === null || text === "") return
    const offsetPosition = after ? position.delta(0, 1) : position
    this.#editor.executeEdits("openagents.vim", [{ range: new monaco.Range(offsetPosition.lineNumber, offsetPosition.column, offsetPosition.lineNumber, offsetPosition.column), text }])
    this.#shared.lastEdit = after ? "p" : "P"
  }

  #find(character: string, before: boolean): void {
    const model = this.#editor.getModel()
    const position = this.#editor.getPosition()
    if (model === null || position === null) return
    const line = model.getLineContent(position.lineNumber)
    const found = line.indexOf(character, position.column)
    if (found >= 0) this.#editor.setPosition({ lineNumber: position.lineNumber, column: Math.max(1, found + (before ? 1 : 2)) })
    this.#pending = null
    this.#project()
  }

  #runEx(command: string): void {
    const normalized = command.trim().toLocaleLowerCase()
    if (["w", "write", "w!", "write!"].includes(normalized)) this.#onSave()
    else if (["q", "quit"].includes(normalized)) this.#onClose(false)
    else if (["q!", "quit!"].includes(normalized)) this.#onClose(true)
    else if (["wq", "x", "xit", "writequit"].includes(normalized)) {
      this.#onSave()
      this.#onClose(false)
    }
    this.#pending = null
    this.#project()
  }

  #normalKey(event: monaco.IKeyboardEvent, key: string): boolean {
    if (this.#pending?.startsWith(":")) {
      if (key === "Enter") this.#runEx(this.#pending.slice(1))
      else if (key === "Backspace") this.#pending = this.#pending.length <= 1 ? null : this.#pending.slice(0, -1)
      else if (key.length === 1) this.#pending += key
      this.#project()
      return true
    }
    if (this.#pending === "register") {
      if (key.length === 1) this.#register = key
      this.#pending = null
      this.#project()
      return true
    }
    if (this.#pending === "mark_set") {
      const position = this.#editor.getPosition()
      if (key.length === 1 && position !== null) this.#shared.marks.set(key, position)
      this.#pending = null
      this.#project()
      return true
    }
    if (this.#pending === "mark_go") {
      const mark = this.#shared.marks.get(key)
      if (mark !== undefined) this.#editor.setPosition(mark)
      this.#pending = null
      this.#project()
      return true
    }
    if (this.#pending === "find" || this.#pending === "till") {
      if (key.length === 1) this.#find(key, this.#pending === "till")
      return true
    }
    if (this.#mode === "operator_pending") {
      const operator = this.#pending?.slice(0, 1) ?? ""
      if (key === operator) this.#lineOperator(operator)
      else {
        const movements: Readonly<Record<string, string>> = { h: "cursorLeft", j: "cursorDown", k: "cursorUp", l: "cursorRight", w: "cursorWordStartRight", b: "cursorWordStartLeft", e: "cursorWordEndRight", "0": "cursorHome", "$": "cursorEnd" }
        const movement = movements[key]
        if (movement !== undefined) {
          this.#move(movement)
          this.#finishOperator()
        } else this.#setMode("normal")
      }
      return true
    }
    if (/^[1-9]$/.test(key) || (key === "0" && this.#count !== "")) {
      this.#count = `${this.#count}${key}`.slice(0, 4)
      this.#project()
      return true
    }
    const movements: Readonly<Record<string, string>> = { h: "cursorLeft", j: "cursorDown", k: "cursorUp", l: "cursorRight", w: "cursorWordStartRight", b: "cursorWordStartLeft", e: "cursorWordEndRight", "0": "cursorHome", "$": "cursorEnd", G: "cursorBottom" }
    const movement = movements[key]
    if (movement !== undefined) {
      this.#move(movement)
      return true
    }
    if (key === "g") {
      this.#trigger("cursorTop")
      return true
    }
    if (["d", "c", "y", ">", "<"].includes(key)) {
      if (key === ">" || key === "<") {
        this.#trigger(key === ">" ? "editor.action.indentLines" : "editor.action.outdentLines")
        this.#shared.lastEdit = key
        return true
      }
      this.#setMode("operator_pending", key)
      return true
    }
    if (key === "i") this.#setMode("insert")
    else if (key === "a") { this.#trigger("cursorRight"); this.#setMode("insert") }
    else if (key === "I") { this.#trigger("cursorHome"); this.#setMode("insert") }
    else if (key === "A") { this.#trigger("cursorEnd"); this.#setMode("insert") }
    else if (key === "o" || key === "O") { this.#trigger(key === "o" ? "editor.action.insertLineAfter" : "editor.action.insertLineBefore"); this.#setMode("insert") }
    else if (key === "R") { this.#trigger("editor.action.toggleOvertypeInsertMode"); this.#setMode("replace") }
    else if (event.ctrlKey && key.toLocaleLowerCase() === "v") this.#setMode("visual_block")
    else if (key === "v") this.#setMode("visual")
    else if (key === "V") { this.#trigger("expandLineSelection"); this.#setMode("visual_line") }
    else if (key === "u") this.#trigger("undo")
    else if (event.ctrlKey && key.toLocaleLowerCase() === "r") this.#trigger("redo")
    else if (key === "x") { this.#trigger("deleteRight"); this.#shared.lastEdit = "x" }
    else if (key === "J") { this.#trigger("editor.action.joinLines"); this.#shared.lastEdit = "J" }
    else if (key === "p" || key === "P") this.#insertRegister(key === "p")
    else if (key === "." && this.#shared.lastEdit !== null) this.#normalKey(event, this.#shared.lastEdit)
    else if (key === '"') { this.#pending = "register"; this.#project() }
    else if (key === "m") { this.#pending = "mark_set"; this.#project() }
    else if (key === "'") { this.#pending = "mark_go"; this.#project() }
    else if (key === "f" || key === "t") { this.#pending = key === "f" ? "find" : "till"; this.#project() }
    else if (key === "/") { this.#trigger("actions.find"); this.#pending = "/"; this.#project() }
    else if (key === ":") { this.#pending = ":"; this.#project() }
    else if (key === "~") {
      const model = this.#editor.getModel(); const position = this.#editor.getPosition()
      if (model !== null && position !== null) {
        const character = model.getValueInRange(new monaco.Range(position.lineNumber, position.column, position.lineNumber, position.column + 1))
        this.#editor.executeEdits("openagents.vim", [{ range: new monaco.Range(position.lineNumber, position.column, position.lineNumber, position.column + 1), text: character === character.toLocaleUpperCase() ? character.toLocaleLowerCase() : character.toLocaleUpperCase() }])
        this.#trigger("cursorRight"); this.#shared.lastEdit = "~"
      }
    } else return false
    return true
  }

  #onKey = (event: monaco.IKeyboardEvent): void => {
    if (!this.#enabled || this.#disposed || event.browserEvent.isComposing) return
    const key = event.browserEvent.key
    if (key === "Escape") {
      event.preventDefault(); event.stopPropagation()
      if (this.#mode === "replace") this.#trigger("editor.action.toggleOvertypeInsertMode")
      this.#setMode("normal")
      this.#editor.setSelection(this.#editor.getSelection()?.collapseToStart() ?? new monaco.Selection(1, 1, 1, 1))
      return
    }
    if (this.#mode === "insert" || this.#mode === "replace") return
    if (this.#normalKey(event, key)) {
      event.preventDefault()
      event.stopPropagation()
    }
  }

  setEnabled(enabled: boolean): void {
    if (this.#disposed || enabled === this.#enabled) return
    this.#enabled = enabled
    if (enabled) {
      this.#subscription = this.#editor.onKeyDown(this.#onKey)
      this.#blurSubscription = this.#editor.onDidBlurEditorWidget(() => {
        if (this.#enabled && !this.#disposed) this.#setMode("normal")
      })
      vimHandlerCount += 2
      this.#setMode("normal")
    } else {
      if (this.#subscription !== null) vimHandlerCount = Math.max(0, vimHandlerCount - 2)
      this.#subscription?.dispose()
      this.#blurSubscription?.dispose()
      this.#subscription = null
      this.#blurSubscription = null
      this.#setMode("insert")
    }
  }

  dispose(): void {
    if (this.#disposed) return
    this.#disposed = true
    if (this.#subscription !== null) vimHandlerCount = Math.max(0, vimHandlerCount - 2)
    this.#subscription?.dispose()
    this.#blurSubscription?.dispose()
    this.#subscription = null
    this.#blurSubscription = null
  }
}

const languageFor = (value: string): string => {
  if (["typescript", "javascript", "json", "markdown", "rust", "python", "shell", "toml", "yaml", "css", "html", "plaintext"].includes(value)) return value
  return "plaintext"
}

const modelFor = (input: IdeMonacoAttachInput): ModelEntry => {
  const key = input.documentRef as string
  const existing = models.get(key)
  if (existing !== undefined) return existing
  const model = monaco.editor.createModel(
    input.value,
    languageFor(input.language),
    monaco.Uri.parse(`inmemory://openagents/${encodeURIComponent(key)}`),
  )
  const entry: ModelEntry = {
    model,
    views: new Set(),
    vimState: { marks: new Map(), registers: new Map(), lastEdit: null },
    generation: input.generation as number,
    sequence: input.sequence as number,
    syncing: false,
    localLanguage: IdeMonacoLocalLanguageStateSchema.cases.Unsupported.make({ language: languageFor(input.language) }),
    localLanguageName: null,
    localLanguageCallbacks: new Set(),
    localManualWorker: null,
    localInput: input,
  }
  models.set(key, entry)
  return entry
}

const supportedLocalLanguages = new Set(["typescript", "javascript", "json", "css", "scss", "less", "html", "handlebars", "razor"])

type TypeScriptContribution = {
  readonly getTypeScriptWorker: () => Promise<(uri: monaco.Uri) => Promise<unknown>>
  readonly getJavaScriptWorker: () => Promise<(uri: monaco.Uri) => Promise<unknown>>
}

type JsonContribution = {
  readonly getWorker: () => Promise<(uri: monaco.Uri) => Promise<unknown>>
}

const isTypeScriptContribution = (value: unknown): value is TypeScriptContribution =>
  typeof value === "object" && value !== null &&
  typeof Reflect.get(value, "getTypeScriptWorker") === "function" &&
  typeof Reflect.get(value, "getJavaScriptWorker") === "function"

const isJsonContribution = (value: unknown): value is JsonContribution =>
  typeof value === "object" && value !== null &&
  typeof Reflect.get(value, "getWorker") === "function"

const publishLocalLanguage = (entry: ModelEntry, state: IdeMonacoLocalLanguageState): void => {
  entry.localLanguage = IdeMonacoLocalLanguageStateSchema.make(state)
  for (const callback of entry.localLanguageCallbacks) callback(entry.localLanguage)
}

const activateLocalLanguage = (entry: ModelEntry, input: IdeMonacoAttachInput): void => {
  const language = languageFor(input.language)
  entry.localInput = input
  if (!supportedLocalLanguages.has(language)) {
    entry.localManualWorker?.terminate()
    entry.localManualWorker = null
    entry.localLanguageName = language
    publishLocalLanguage(entry, IdeMonacoLocalLanguageStateSchema.cases.Unsupported.make({ language }))
    return
  }
  if (entry.localLanguageName === language && (entry.localLanguage._tag === "Loading" || entry.localLanguage._tag === "Ready")) {
    publishLocalLanguage(entry, entry.localLanguage)
    return
  }
  entry.localManualWorker?.terminate()
  entry.localManualWorker = null
  entry.localLanguageName = language
  localWorkerGeneration += 1
  const workerGeneration = IdeServiceGenerationSchema.make(localWorkerGeneration)
  publishLocalLanguage(entry, IdeMonacoLocalLanguageStateSchema.cases.Loading.make({ language, workerGeneration }))
  void (async () => {
    if (language === "typescript" || language === "javascript") {
      const contribution: unknown = typeScriptContributionModule
      if (!isTypeScriptContribution(contribution)) throw new Error("The packaged TypeScript contribution is unavailable.")
      const worker = language === "typescript"
        ? await contribution.getTypeScriptWorker()
        : await contribution.getJavaScriptWorker()
      await worker(entry.model.uri)
    } else if (language === "json") {
      const contribution: unknown = jsonContributionModule
      if (!isJsonContribution(contribution)) throw new Error("The packaged JSON contribution is unavailable.")
      await (await contribution.getWorker())(entry.model.uri)
    }
    else {
      entry.localManualWorker = createTrackedWorker(workerUrls[language] ?? workerUrls.editor!, language)
      await Promise.resolve()
    }
  })().then(() => {
    if (entry.localLanguageName !== language || entry.model.isDisposed()) return
    const current = entry.localInput
    publishLocalLanguage(entry, IdeMonacoLocalLanguageStateSchema.cases.Ready.make({
      language,
      workerGeneration,
      documentRef: current.documentRef,
      documentGeneration: current.generation,
      documentVersion: current.documentVersion,
      evidenceTier: "document_local",
      capabilities: language === "typescript" || language === "javascript"
        ? ["syntax", "completion", "hover", "format", "folding"]
        : ["syntax", "completion", "hover", "format", "folding"],
    }))
  }).catch(cause => {
    if (entry.localLanguageName !== language || entry.model.isDisposed()) return
    publishLocalLanguage(entry, IdeMonacoLocalLanguageStateSchema.cases.Failed.make({
      language,
      workerGeneration,
      message: cause instanceof Error ? cause.message.slice(0, 500) : "The document-local worker failed to start.",
      recoverable: true,
    }))
  })
}

const monacoRange = (range: IdeMonacoProjectLanguageProjection["diagnostics"][number]["range"]): monaco.Range =>
  new monaco.Range(range.start.line, range.start.column, range.end.line, range.end.column)

const projectLanguageMatches = (input: IdeMonacoAttachInput): input is IdeMonacoAttachInput & {
  readonly projectLanguage: IdeMonacoProjectLanguageProjection
} => input.projectLanguage !== null &&
  input.projectLanguage.documentRef === input.documentRef &&
  input.projectLanguage.documentGeneration === input.generation &&
  input.projectLanguage.documentVersion === input.documentVersion

const applyProjectLanguage = (view: ViewEntry, input: IdeMonacoAttachInput): void => {
  view.projectFoldingProvider?.dispose()
  view.projectFoldingProvider = null
  const project = projectLanguageMatches(input) ? input.projectLanguage : null
  monaco.editor.setModelMarkers(view.modelEntry.model, "openagents-project-language", project === null ? [] : project.diagnostics.map(diagnostic => ({
    ...monacoRange(diagnostic.range),
    severity: diagnostic.severity === "error" ? monaco.MarkerSeverity.Error
      : diagnostic.severity === "warning" ? monaco.MarkerSeverity.Warning
      : diagnostic.severity === "information" ? monaco.MarkerSeverity.Info
      : monaco.MarkerSeverity.Hint,
    message: diagnostic.message,
    source: diagnostic.source,
    code: diagnostic.diagnosticRef,
  })))
  view.projectDecorations.set(project === null ? [] : [
    ...project.semanticTokens.map(token => ({
      range: monacoRange(token.range),
      options: {
        inlineClassName: `oa-ide-project-semantic-${token.tokenType.replaceAll(/[^A-Za-z0-9_-]/gu, "-")}`,
        description: `Project semantic token ${token.itemRef}`,
      },
    })),
    ...project.inlayHints.map(hint => ({
      range: monacoRange(hint.range),
      options: {
        after: {
          content: ` ${hint.label}`,
          inlineClassName: `oa-ide-project-inlay oa-ide-project-inlay--${hint.hintKind}`,
        },
        description: `Project inlay hint ${hint.itemRef}`,
      },
    })),
  ])
  if (project === null || project.foldingRanges.length === 0) return
  const model = view.modelEntry.model
  view.projectFoldingProvider = monaco.languages.registerFoldingRangeProvider(model.getLanguageId(), {
    provideFoldingRanges(candidate) {
      if (candidate !== model || model.isDisposed()) return []
      return project.foldingRanges.map(fold => ({
        start: fold.range.start.line,
        end: Math.max(fold.range.start.line, fold.range.end.line),
        kind: fold.foldKind === "comment" ? monaco.languages.FoldingRangeKind.Comment
          : fold.foldKind === "imports" ? monaco.languages.FoldingRangeKind.Imports
          : fold.foldKind === "region" ? monaco.languages.FoldingRangeKind.Region
          : undefined,
      }))
    },
  })
}

const disposeView = (viewRef: string): void => {
  const entry = views.get(viewRef)
  if (entry === undefined || entry.disposed) return
  entry.disposed = true
  entry.vim.dispose()
  entry.modelEntry.localLanguageCallbacks.delete(entry.localLanguageCallback)
  entry.projectFoldingProvider?.dispose()
  entry.projectDecorations.clear()
  for (const subscription of entry.subscriptions) subscription.dispose()
  listenerCount = Math.max(0, listenerCount - entry.subscriptions.length)
  entry.editor.dispose()
  entry.modelEntry.views.delete(viewRef)
  views.delete(viewRef)
  if (entry.modelEntry.views.size === 0) {
    const modelKey = entry.input.documentRef as string
    monaco.editor.setModelMarkers(entry.modelEntry.model, "openagents-project-language", [])
    entry.modelEntry.localManualWorker?.terminate()
    entry.modelEntry.model.dispose()
    models.delete(modelKey)
  }
}

const attach: IdeMonacoRuntime["attach"] = (host, rawInput, onEvent, onVim, onLocalLanguage) => {
  if (runtimeState === "stopped") throw new Error("The Monaco runtime has stopped.")
  const input = decodeIdeMonacoAttachInput(rawInput)
  const viewKey = input.viewRef as string
  disposeView(viewKey)
  const modelEntry = modelFor(input)
  modelEntry.views.add(viewKey)
  const editor = monaco.editor.create(host, {
    model: modelEntry.model,
    theme: "openagents-tokyo-night",
    automaticLayout: true,
    ariaLabel: `Code editor for ${input.pathLabel}`,
    accessibilitySupport: input.editorOptions.accessibilitySupport ? "on" : "off",
    minimap: { enabled: input.minimap },
    wordWrap: input.wordWrap ? "on" : "off",
    readOnly: input.readOnly,
    lineNumbers: input.editorOptions.lineNumbers ? "on" : "off",
    folding: true,
    matchBrackets: input.editorOptions.bracketMatching ? "always" : "never",
    bracketPairColorization: { enabled: input.editorOptions.bracketMatching },
    guides: { indentation: input.editorOptions.indentationGuides, bracketPairs: input.editorOptions.bracketMatching },
    multiCursorModifier: "alt",
    multiCursorLimit: input.editorOptions.multiCursor ? 10_000 : 1,
    fontSize: input.editorOptions.fontSize,
    lineHeight: input.editorOptions.lineHeight,
    renderWhitespace: input.editorOptions.renderWhitespace,
    rulers: [...input.editorOptions.rulers],
    stickyScroll: { enabled: input.editorOptions.stickyScroll },
    scrollBeyondLastLine: false,
    renderValidationDecorations: "on",
  })
  modelEntry.model.updateOptions({
    tabSize: input.editorOptions.tabSize,
    insertSpaces: input.editorOptions.insertSpaces,
  })
  const initialStart = modelEntry.model.getPositionAt(input.selection.start)
  const initialEnd = modelEntry.model.getPositionAt(input.selection.end)
  editor.setSelection(new monaco.Selection(
    initialStart.lineNumber,
    initialStart.column,
    initialEnd.lineNumber,
    initialEnd.column,
  ))
  const emit = (event: IdeMonacoDocumentEvent): void => onEvent(IdeMonacoDocumentEventSchema.make(event))
  const content = editor.onDidChangeModelContent(event => {
    if (modelEntry.syncing) return
    modelEntry.sequence += 1
    emit(IdeMonacoDocumentEventSchema.cases.Edit.make({
      documentRef: input.documentRef,
      generation: input.generation,
      sequence: IdeDocumentSequence.make(modelEntry.sequence),
      modelVersion: IdeMonacoModelVersion.make(modelEntry.model.getVersionId()),
      value: modelEntry.model.getValue(),
      changes: event.changes.map(change => ({ offset: change.rangeOffset, length: change.rangeLength, text: change.text })),
    }))
  })
  const selection = editor.onDidChangeCursorSelection(event => {
    const start = modelEntry.model.getOffsetAt(event.selection.getStartPosition())
    const end = modelEntry.model.getOffsetAt(event.selection.getEndPosition())
    emit(IdeMonacoDocumentEventSchema.cases.Selection.make({
      documentRef: input.documentRef,
      generation: input.generation,
      viewRef: input.viewRef,
      selection: { start: Math.min(start, end), end: Math.max(start, end) },
    }))
  })
  const saveCommand = editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
    emit(IdeMonacoDocumentEventSchema.cases.Save.make({ documentRef: input.documentRef, generation: input.generation }))
  })
  const vim = new VimModeController(
    editor,
    onVim,
    () => emit(IdeMonacoDocumentEventSchema.cases.Save.make({ documentRef: input.documentRef, generation: input.generation })),
    force => emit(IdeMonacoDocumentEventSchema.cases.Close.make({ documentRef: input.documentRef, generation: input.generation, force })),
    modelEntry.vimState,
  )
  vim.setEnabled(input.vimEnabled)
  const subscriptions = [content, selection]
  listenerCount += subscriptions.length
  const localLanguageCallback = (state: IdeMonacoLocalLanguageState): void => onLocalLanguage(IdeMonacoLocalLanguageStateSchema.make(state))
  modelEntry.localLanguageCallbacks.add(localLanguageCallback)
  const viewEntry: ViewEntry = {
    editor,
    modelEntry,
    subscriptions,
    vim,
    localLanguageCallback,
    projectDecorations: editor.createDecorationsCollection(),
    projectFoldingProvider: null,
    input,
    disposed: false,
  }
  views.set(viewKey, viewEntry)
  activateLocalLanguage(modelEntry, input)
  applyProjectLanguage(viewEntry, input)

  const update = (rawNext: IdeMonacoAttachInput): void => {
    if (viewEntry.disposed) return
    const next = decodeIdeMonacoAttachInput(rawNext)
    if (next.documentRef !== viewEntry.input.documentRef || next.generation !== viewEntry.input.generation || next.viewRef !== viewEntry.input.viewRef) return
    modelEntry.localInput = next
    if ((next.sequence as number) >= modelEntry.sequence && modelEntry.model.getValue() !== next.value) {
      modelEntry.syncing = true
      modelEntry.model.setValue(next.value)
      modelEntry.syncing = false
      modelEntry.sequence = next.sequence as number
    }
    if (modelEntry.model.getLanguageId() !== languageFor(next.language)) {
      monaco.editor.setModelLanguage(modelEntry.model, languageFor(next.language))
      activateLocalLanguage(modelEntry, next)
    } else if (modelEntry.localLanguage._tag === "Ready" && modelEntry.localLanguage.documentVersion !== next.documentVersion) {
      publishLocalLanguage(modelEntry, IdeMonacoLocalLanguageStateSchema.cases.Ready.make({
        ...modelEntry.localLanguage,
        documentRef: next.documentRef,
        documentGeneration: next.generation,
        documentVersion: next.documentVersion,
      }))
    }
    editor.updateOptions({
      ariaLabel: `Code editor for ${next.pathLabel}`,
      minimap: { enabled: next.minimap },
      wordWrap: next.wordWrap ? "on" : "off",
      readOnly: next.readOnly,
      accessibilitySupport: next.editorOptions.accessibilitySupport ? "on" : "off",
      lineNumbers: next.editorOptions.lineNumbers ? "on" : "off",
      matchBrackets: next.editorOptions.bracketMatching ? "always" : "never",
      bracketPairColorization: { enabled: next.editorOptions.bracketMatching },
      guides: { indentation: next.editorOptions.indentationGuides, bracketPairs: next.editorOptions.bracketMatching },
      multiCursorLimit: next.editorOptions.multiCursor ? 10_000 : 1,
      fontSize: next.editorOptions.fontSize,
      lineHeight: next.editorOptions.lineHeight,
      renderWhitespace: next.editorOptions.renderWhitespace,
      rulers: [...next.editorOptions.rulers],
      stickyScroll: { enabled: next.editorOptions.stickyScroll },
    })
    modelEntry.model.updateOptions({
      tabSize: next.editorOptions.tabSize,
      insertSpaces: next.editorOptions.insertSpaces,
    })
    if (next.selectionVersion !== viewEntry.input.selectionVersion) {
      const start = modelEntry.model.getPositionAt(next.selection.start)
      const end = modelEntry.model.getPositionAt(next.selection.end)
      editor.setSelection(new monaco.Selection(start.lineNumber, start.column, end.lineNumber, end.column))
      editor.revealPositionInCenter(start)
    }
    vim.setEnabled(next.vimEnabled)
    applyProjectLanguage(viewEntry, next)
    viewEntry.input = next
  }
  const dispose = (): void => disposeView(viewKey)
  const mounted: IdeMonacoMountedView = { update, focus: () => editor.focus(), dispose }
  if (saveCommand === null) onVim(IdeVimProjectionSchema.make({ enabled: input.vimEnabled, mode: input.vimEnabled ? "normal" : "insert", pending: null, count: null }))
  return mounted
}

const resources = () => IdeMonacoRuntimeResourceSnapshotSchema.make({
  state: runtimeState,
  modelCount: models.size,
  viewCount: views.size,
  workerCount: workers.size,
  listenerCount,
  vimHandlerCount,
})

const dispose = (): void => {
  if (runtimeState === "stopped") return
  runtimeState = "stopped"
  for (const viewRef of [...views.keys()]) disposeView(viewRef)
  for (const entry of models.values()) entry.model.dispose()
  models.clear()
  for (const worker of [...workers]) worker.terminate()
}

export const runtime: IdeMonacoRuntime = { attach, resources, dispose }

globalThis.addEventListener("pagehide", dispose, { once: true })
