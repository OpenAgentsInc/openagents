import { iconElement } from "@openagentsinc/ui/icon-dom"
import type { IconName } from "@openagentsinc/ui/icon"
import {
  KHALA_CODE_EDITOR_DEFAULT_MAX_FILE_BYTES,
  type KhalaCodeEditorError,
  type KhalaCodeEditorTreeNode,
  type KhalaCodeEditorWorkspaceRoot,
} from "../shared/editor"
import type {
  KhalaCodeDesktopEditorDirectoryReadRequest,
  KhalaCodeDesktopEditorDirectoryReadResult,
  KhalaCodeDesktopEditorFileReadRequest,
  KhalaCodeDesktopEditorFileReadResult,
  KhalaCodeDesktopEditorWorkspaceReadResult,
} from "../shared/rpc"

type MonacoApi = typeof import("monaco-editor")
type MonacoEditor =
  import("monaco-editor").editor.IStandaloneCodeEditor
type MonacoModel =
  import("monaco-editor").editor.ITextModel

type MonacoWorkerConstructor = { new (): Worker }
type MonacoWorkerModule = { readonly default: MonacoWorkerConstructor }
type MonacoEnvironmentOwner = typeof globalThis & {
  MonacoEnvironment?: {
    getWorker: (_workerId: string, label: string) => Worker
  }
}

export type KhalaCodeEditorPanelHandle = Readonly<{
  destroy: () => void
  refresh: () => Promise<void>
  setVisible: (visible: boolean) => void
}>

export type KhalaCodeEditorComposerContext = Readonly<{
  displayPath: string
  kind: "file" | "selection"
  lineEnd?: number
  lineStart?: number
  path: string
  providerId: string
  rootPath: string
}>

export type KhalaCodeEditorPanelServices = Readonly<{
  editorDirectoryRead: (
    request?: KhalaCodeDesktopEditorDirectoryReadRequest,
  ) => Promise<KhalaCodeDesktopEditorDirectoryReadResult>
  editorFileRead: (
    request: KhalaCodeDesktopEditorFileReadRequest,
  ) => Promise<KhalaCodeDesktopEditorFileReadResult>
  editorWorkspaceRead: () => Promise<KhalaCodeDesktopEditorWorkspaceReadResult>
}>

export type KhalaCodeEditorMonacoLoader = () => Promise<MonacoApi>

export type KhalaCodeEditorPanelOptions = KhalaCodeEditorPanelServices & Readonly<{
  loadMonaco?: KhalaCodeEditorMonacoLoader
  onComposerContextSelected?: (context: KhalaCodeEditorComposerContext) => void
}>

type EditorTreeItem = {
  children: string[]
  error: string | null
  expanded: boolean
  loading: boolean
  node: KhalaCodeEditorTreeNode
}

type FlatTreeRow = Readonly<{
  item: EditorTreeItem
  key: string
}>

type EditorOpenTab = Readonly<{
  displayPath: string
  key: string
  language: string
  name: string
  path: string
  providerId: string
}>

let monacoRuntimePromise: Promise<MonacoApi> | null = null
let khalaMonacoThemeDefined = false

const nodeKey = (
  input: Pick<KhalaCodeEditorTreeNode, "path" | "providerId">,
): string => `${input.providerId}::${input.path}`

const rootNode = (root: KhalaCodeEditorWorkspaceRoot): KhalaCodeEditorTreeNode => ({
  childrenLoaded: false,
  changeKind: null,
  depth: 0,
  kind: "directory",
  mtime: null,
  name: root.label,
  parentPath: null,
  path: root.path,
  providerId: root.providerId,
  readonly: root.readonly,
  rootPath: root.path,
  sizeBytes: null,
  symlink: false,
})

const compareNodes = (
  left: KhalaCodeEditorTreeNode,
  right: KhalaCodeEditorTreeNode,
): number => {
  if (left.kind === "directory" && right.kind !== "directory") return -1
  if (left.kind !== "directory" && right.kind === "directory") return 1
  return left.name.localeCompare(right.name, undefined, { sensitivity: "base" })
}

const iconForNode = (node: KhalaCodeEditorTreeNode, expanded: boolean): IconName =>
  node.kind === "directory"
    ? expanded
      ? "FolderOpen"
      : "Folder"
    : "FileCode"

const changeLabel = (changeKind: KhalaCodeEditorTreeNode["changeKind"]): string => {
  switch (changeKind) {
    case "added":
      return "A"
    case "deleted":
      return "D"
    case "modified":
      return "M"
    default:
      return ""
  }
}

const changeTitle = (changeKind: KhalaCodeEditorTreeNode["changeKind"]): string => {
  switch (changeKind) {
    case "added":
      return "Added"
    case "deleted":
      return "Deleted"
    case "modified":
      return "Modified"
    default:
      return ""
  }
}

const formatBytes = (bytes: number | null): string => {
  if (bytes === null) return "Folder"
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

const relativeDisplayPath = (node: Pick<KhalaCodeEditorTreeNode, "name" | "path" | "rootPath">): string => {
  const root = node.rootPath.replace(/[\\/]+$/, "")
  if (node.path === root || node.path === node.rootPath) return node.name
  const prefix = `${root}/`
  const windowsPrefix = `${root}\\`
  if (node.path.startsWith(prefix)) return node.path.slice(prefix.length)
  if (node.path.startsWith(windowsPrefix)) return node.path.slice(windowsPrefix.length)
  return node.name
}

const extensionForPath = (path: string): string => {
  const name = path.split(/[\\/]/).pop()?.toLowerCase() ?? ""
  const dot = name.lastIndexOf(".")
  if (dot <= 0) return name
  return name.slice(dot)
}

const languageForPath = (path: string): string => {
  const extension = extensionForPath(path)
  switch (extension) {
    case ".c":
      return "c"
    case ".cpp":
    case ".cc":
    case ".cxx":
    case ".hpp":
    case ".hxx":
      return "cpp"
    case ".css":
      return "css"
    case ".go":
      return "go"
    case ".html":
    case ".htm":
      return "html"
    case ".java":
      return "java"
    case ".js":
    case ".mjs":
    case ".cjs":
      return "javascript"
    case ".json":
    case ".jsonc":
      return "json"
    case ".md":
    case ".markdown":
      return "markdown"
    case ".py":
      return "python"
    case ".rs":
      return "rust"
    case ".sh":
    case ".bash":
    case ".zsh":
      return "shell"
    case ".sql":
      return "sql"
    case ".ts":
    case ".mts":
    case ".cts":
      return "typescript"
    case ".tsx":
      return "typescript"
    case ".jsx":
      return "javascript"
    case ".toml":
      return "toml"
    case ".xml":
      return "xml"
    case ".yaml":
    case ".yml":
      return "yaml"
    case "dockerfile":
      return "dockerfile"
    case "makefile":
      return "makefile"
    default:
      return "plaintext"
  }
}

const languageLabel = (language: string): string =>
  language === "plaintext"
    ? "Text"
    : language
      .replace(/^./, first => first.toUpperCase())
      .replace("Typescript", "TypeScript")
      .replace("Javascript", "JavaScript")

const errorTitle = (error: KhalaCodeEditorError): string => {
  switch (error.code) {
    case "binary_file":
      return "Unsupported file"
    case "file_too_large":
      return "File too large"
    case "outside_workspace":
      return "Outside workspace"
    case "not_found":
      return "File not found"
    default:
      return "Unable to open file"
  }
}

const rgbToHex = (value: string): string | null => {
  const match = /^rgba?\((\d+),\s*(\d+),\s*(\d+)/i.exec(value.trim())
  if (match === null) return null
  return `#${[match[1], match[2], match[3]]
    .map(component => Math.max(0, Math.min(255, Number(component)))
      .toString(16)
      .padStart(2, "0"))
    .join("")}`
}

const cssColorToHex = (
  container: HTMLElement,
  value: string,
  fallback: string,
): string => {
  const trimmed = value.trim()
  if (/^#[0-9a-f]{6}$/i.test(trimmed)) return trimmed
  const probe = document.createElement("span")
  probe.style.color = trimmed
  probe.style.display = "none"
  container.append(probe)
  const resolved = getComputedStyle(probe).color
  probe.remove()
  return rgbToHex(resolved) ?? fallback
}

const tokenColor = (
  container: HTMLElement,
  token: string,
  fallback: string,
): string => cssColorToHex(container, getComputedStyle(container).getPropertyValue(token), fallback)

const defineKhalaMonacoTheme = (monaco: MonacoApi, container: HTMLElement): void => {
  if (khalaMonacoThemeDefined) return
  khalaMonacoThemeDefined = true
  monaco.editor.defineTheme("khala-code", {
    base: "vs-dark",
    colors: {
      "editor.background": tokenColor(container, "--oa-color-khala-void", "#070a12"),
      "editor.foreground": tokenColor(container, "--oa-color-component-text", "#e6edf7"),
      "editor.lineHighlightBackground": tokenColor(container, "--oa-color-khala-surface", "#101728"),
      "editorLineNumber.activeForeground": tokenColor(container, "--oa-color-khala-energy-cyan", "#84f7ff"),
      "editorLineNumber.foreground": tokenColor(container, "--oa-color-khala-energy-soft", "#78a9b5"),
      "editor.selectionBackground": tokenColor(container, "--oa-color-khala-energy-cyan", "#245d68"),
      "editorCursor.foreground": tokenColor(container, "--oa-color-khala-energy-cyan", "#84f7ff"),
    },
    inherit: true,
    rules: [
      { foreground: "8ee9ff", token: "keyword" },
      { foreground: "f6d365", token: "string" },
      { foreground: "c3d4e8", token: "identifier" },
      { foreground: "7ec8ff", token: "number" },
      { foreground: "6f8397", token: "comment" },
    ],
  })
}

const loadMonacoRuntime = async (): Promise<MonacoApi> => {
  const [
    monaco,
    editorWorker,
    jsonWorker,
    cssWorker,
    htmlWorker,
    typeScriptWorker,
  ] = await Promise.all([
    import("monaco-editor"),
    import("monaco-editor/esm/vs/editor/editor.worker?worker") as Promise<MonacoWorkerModule>,
    import("monaco-editor/esm/vs/language/json/json.worker?worker") as Promise<MonacoWorkerModule>,
    import("monaco-editor/esm/vs/language/css/css.worker?worker") as Promise<MonacoWorkerModule>,
    import("monaco-editor/esm/vs/language/html/html.worker?worker") as Promise<MonacoWorkerModule>,
    import("monaco-editor/esm/vs/language/typescript/ts.worker?worker") as Promise<MonacoWorkerModule>,
    import("monaco-editor/min/vs/editor/editor.main.css"),
  ])
  const EditorWorker = editorWorker.default
  const JsonWorker = jsonWorker.default
  const CssWorker = cssWorker.default
  const HtmlWorker = htmlWorker.default
  const TypeScriptWorker = typeScriptWorker.default
  ;(globalThis as MonacoEnvironmentOwner).MonacoEnvironment = {
    getWorker(_workerId, label) {
      if (label === "json") return new JsonWorker()
      if (label === "css" || label === "scss" || label === "less") return new CssWorker()
      if (label === "html" || label === "handlebars" || label === "razor") return new HtmlWorker()
      if (label === "typescript" || label === "javascript") return new TypeScriptWorker()
      return new EditorWorker()
    },
  }
  return monaco
}

const defaultLoadMonaco = (): Promise<MonacoApi> => {
  monacoRuntimePromise ??= loadMonacoRuntime()
  return monacoRuntimePromise
}

const el = <K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] => {
  const node = document.createElement(tag)
  if (className !== undefined) node.className = className
  if (text !== undefined) node.textContent = text
  return node
}

export const mountKhalaCodeEditorPanel = (
  container: HTMLElement,
  options: KhalaCodeEditorPanelOptions,
): KhalaCodeEditorPanelHandle => {
  container.replaceChildren()

  const treeItems = new Map<string, EditorTreeItem>()
  let rootKeys: string[] = []
  let flatRows: FlatTreeRow[] = []
  let renderedRows: FlatTreeRow[] = []
  let openTabs: EditorOpenTab[] = []
  let focusedKey: string | null = null
  let selectedKey: string | null = null
  let searchQuery = ""
  let workspaceLoaded = false
  let workspaceLoading: Promise<void> | null = null
  let fileRequestSeq = 0
  let destroyed = false
  let monacoEditor: MonacoEditor | null = null
  let monacoModel: MonacoModel | null = null
  let activeFileContext: Omit<KhalaCodeEditorComposerContext, "kind" | "lineEnd" | "lineStart"> | null = null
  let selectionChangeDisposable: { dispose: () => void } | null = null

  const panel = el("div", "khala-code-editor-panel")
  panel.dataset.khalaCodeEditorPanel = ""

  const treePane = el("section", "khala-code-editor-tree-pane")
  treePane.setAttribute("aria-label", "Editor workspace files")

  const treeHeader = el("div", "khala-code-editor-pane-header")
  const treeTitle = el("h2", "khala-code-editor-pane-title", "Workspace")
  const treeStatus = el("span", "khala-code-editor-pane-status", "Source")
  const refreshButton = document.createElement("button")
  refreshButton.type = "button"
  refreshButton.className = "khala-code-editor-refresh-button"
  refreshButton.setAttribute("aria-label", "Refresh workspace")
  refreshButton.title = "Refresh workspace"
  refreshButton.append(iconElement("Reload", {
    className: "khala-code-editor-refresh-icon",
    dataIcon: "editor-refresh",
  }))
  const treeHeaderMeta = el("div", "khala-code-editor-header-meta")
  treeHeaderMeta.append(treeStatus, refreshButton)
  treeHeader.append(treeTitle, treeHeaderMeta)

  const searchWrap = el("div", "khala-code-editor-search")
  const searchInput = document.createElement("input")
  searchInput.type = "search"
  searchInput.name = "khala-code-editor-file-search"
  searchInput.className = "khala-code-editor-search-input"
  searchInput.placeholder = "Search files"
  searchInput.setAttribute("aria-label", "Search workspace files")
  searchWrap.append(searchInput)

  const treeBody = el("div", "khala-code-editor-tree")
  treeBody.setAttribute("role", "tree")
  treeBody.setAttribute("aria-label", "Workspace file tree")

  treePane.append(treeHeader, searchWrap, treeBody)

  const sourcePane = el("section", "khala-code-editor-source-pane")
  sourcePane.setAttribute("aria-label", "Editor source view")

  const sourceHeader = el("div", "khala-code-editor-source-header")
  const fileTitle = el("h2", "khala-code-editor-source-title", "No file selected")
  const fileMeta = el("span", "khala-code-editor-source-meta", "Source")
  const sourceContextButton = document.createElement("button")
  sourceContextButton.type = "button"
  sourceContextButton.className = "khala-code-editor-source-context-button"
  sourceContextButton.disabled = true
  sourceContextButton.title = "Add selected source context"
  sourceContextButton.setAttribute("aria-label", "Add selected source context")
  sourceContextButton.append(iconElement("Plus", {
    className: "khala-code-editor-source-context-icon",
    dataIcon: "editor-add-context",
  }))
  const sourceHeaderMeta = el("div", "khala-code-editor-header-meta")
  sourceHeaderMeta.append(fileMeta, sourceContextButton)
  sourceHeader.append(fileTitle, sourceHeaderMeta)

  const tabStrip = el("div", "khala-code-editor-tabs")
  tabStrip.setAttribute("role", "tablist")
  tabStrip.setAttribute("aria-label", "Open editor files")

  const sourceBody = el("div", "khala-code-editor-source-body")
  const sourceState = el("div", "khala-code-editor-source-state", "No source open")
  const monacoHost = el("div", "khala-code-editor-source-monaco")
  monacoHost.hidden = true
  sourceBody.append(sourceState, monacoHost)

  sourcePane.append(sourceHeader, tabStrip, sourceBody)
  panel.append(treePane, sourcePane)
  container.append(panel)

  const storeItem = (node: KhalaCodeEditorTreeNode): EditorTreeItem => {
    const key = nodeKey(node)
    const existing = treeItems.get(key)
    if (existing !== undefined) {
      existing.node = {
        ...node,
        childrenLoaded: existing.node.childrenLoaded || node.childrenLoaded,
      }
      return existing
    }
    const item: EditorTreeItem = {
      children: [],
      error: null,
      expanded: false,
      loading: false,
      node,
    }
    treeItems.set(key, item)
    return item
  }

  const collectRows = (key: string, rows: FlatTreeRow[]): void => {
    const item = treeItems.get(key)
    if (item === undefined) return
    rows.push({ item, key })
    if (!item.expanded) return
    for (const childKey of item.children) collectRows(childKey, rows)
  }

  const rowMatchesSearch = (row: FlatTreeRow, normalizedQuery: string): boolean => {
    if (normalizedQuery.length === 0) return true
    const displayPath = relativeDisplayPath(row.item.node).toLowerCase()
    return row.item.node.name.toLowerCase().includes(normalizedQuery) ||
      displayPath.includes(normalizedQuery)
  }

  const buttonForKey = (key: string): HTMLButtonElement | null =>
    Array.from(treeBody.querySelectorAll<HTMLButtonElement>("[data-khala-editor-node-key]"))
      .find(button => button.dataset.khalaEditorNodeKey === key) ?? null

  const focusRow = (key: string): void => {
    focusedKey = key
    renderTree()
    queueMicrotask(() => buttonForKey(key)?.focus())
  }

  const focusByDelta = (delta: number): void => {
    if (renderedRows.length === 0) return
    const currentIndex = Math.max(0, renderedRows.findIndex(row => row.key === focusedKey))
    const nextIndex = Math.max(0, Math.min(renderedRows.length - 1, currentIndex + delta))
    focusRow(renderedRows[nextIndex].key)
  }

  const showSourceState = (
    title: string,
    meta: string,
    message: string,
    state: "empty" | "error" | "loading" | "unsupported",
  ): void => {
    activeFileContext = null
    sourceContextButton.disabled = true
    selectionChangeDisposable?.dispose()
    selectionChangeDisposable = null
    fileTitle.textContent = title
    fileMeta.textContent = meta
    sourceBody.dataset.state = state
    sourceState.hidden = false
    sourceState.textContent = message
    monacoHost.hidden = true
    monacoEditor?.setModel(null)
    monacoModel?.dispose()
    monacoModel = null
  }

  const renderTabs = (): void => {
    tabStrip.replaceChildren()
    tabStrip.dataset.state = openTabs.length === 0 ? "empty" : "ready"
    if (openTabs.length === 0) {
      const emptyTabs = el("span", "khala-code-editor-tabs-empty", "No open files")
      tabStrip.append(emptyTabs)
      return
    }
    for (const tab of openTabs) {
      const selected = tab.key === selectedKey
      const tabEl = el("div", "khala-code-editor-tab")
      tabEl.dataset.path = tab.path
      tabEl.dataset.selected = selected ? "true" : "false"
      tabEl.setAttribute("role", "tab")
      tabEl.setAttribute("aria-selected", selected ? "true" : "false")
      tabEl.tabIndex = 0
      tabEl.title = tab.displayPath
      tabEl.addEventListener("click", () => {
        void openFile(tab.key)
      })
      tabEl.addEventListener("keydown", event => {
        if (event.key !== "Enter" && event.key !== " ") return
        event.preventDefault()
        void openFile(tab.key)
      })

      const tabName = el("span", "khala-code-editor-tab-name", tab.name)
      const tabMeta = el("span", "khala-code-editor-tab-meta", languageLabel(tab.language))
      const closeButton = document.createElement("button")
      closeButton.type = "button"
      closeButton.className = "khala-code-editor-tab-close"
      closeButton.setAttribute("aria-label", `Close ${tab.name}`)
      closeButton.title = `Close ${tab.name}`
      closeButton.append(iconElement("X", {
        className: "khala-code-editor-tab-close-icon",
        dataIcon: "editor-close-tab",
      }))
      closeButton.addEventListener("click", event => {
        event.stopPropagation()
        closeTab(tab.key)
      })

      tabEl.append(tabName, tabMeta, closeButton)
      tabStrip.append(tabEl)
    }
  }

  const selectedLineRange = (): Pick<KhalaCodeEditorComposerContext, "kind" | "lineEnd" | "lineStart"> => {
    const selection = monacoEditor?.getSelection()
    if (selection === null || selection === undefined || activeFileContext === null) return { kind: "file" }
    const lineStart = Math.max(1, Math.min(selection.startLineNumber, selection.endLineNumber))
    const lineEnd = Math.max(lineStart, Math.max(selection.startLineNumber, selection.endLineNumber))
    return { kind: "selection", lineStart, lineEnd }
  }

  const getSelectedComposerContext = (): KhalaCodeEditorComposerContext | null => {
    if (activeFileContext === null) return null
    return {
      ...activeFileContext,
      ...selectedLineRange(),
    }
  }

  sourceContextButton.addEventListener("click", () => {
    const context = getSelectedComposerContext()
    if (context === null) return
    options.onComposerContextSelected?.(context)
  })

  const renderTreeState = (message: string, state: "empty" | "error" | "loading"): void => {
    treeBody.replaceChildren()
    treeBody.dataset.state = state
    const stateEl = el("div", "khala-code-editor-tree-state", message)
    if (state === "error") stateEl.setAttribute("role", "alert")
    treeBody.append(stateEl)
  }

  const renderTree = (): void => {
    flatRows = []
    for (const rootKey of rootKeys) collectRows(rootKey, flatRows)
    const normalizedQuery = searchQuery.trim().toLowerCase()
    renderedRows = flatRows.filter(row => rowMatchesSearch(row, normalizedQuery))
    treeBody.replaceChildren()
    treeBody.dataset.state = renderedRows.length === 0 ? "empty" : "ready"
    if (renderedRows.length === 0) {
      renderTreeState(normalizedQuery.length === 0 ? "No workspace" : "No matching files", "empty")
      return
    }
    if (focusedKey === null || !renderedRows.some(row => row.key === focusedKey)) {
      focusedKey = renderedRows[0]?.key ?? null
    }
    for (const row of renderedRows) {
      const { item, key } = row
      const { node } = item
      const isDirectory = node.kind === "directory"
      const active = selectedKey === key
      const button = document.createElement("button")
      button.type = "button"
      button.className = "khala-code-editor-tree-row"
      button.dataset.expanded = item.expanded ? "true" : "false"
      button.dataset.kind = node.kind
      button.dataset.khalaEditorNodeKey = key
      button.dataset.path = node.path
      button.dataset.selected = active ? "true" : "false"
      if (node.changeKind !== undefined && node.changeKind !== null) {
        button.dataset.changeKind = node.changeKind
      }
      button.setAttribute("role", "treeitem")
      button.setAttribute("aria-level", String(node.depth + 1))
      button.setAttribute("aria-selected", active ? "true" : "false")
      if (isDirectory) button.setAttribute("aria-expanded", item.expanded ? "true" : "false")
      button.style.setProperty("--khala-code-editor-tree-depth", String(node.depth))
      button.tabIndex = key === focusedKey ? 0 : -1
      button.title = relativeDisplayPath(node)
      button.addEventListener("focus", () => {
        focusedKey = key
      })
      button.addEventListener("click", () => {
        focusedKey = key
        void activateItem(key)
      })

      const chevron = iconElement(
        isDirectory && item.expanded ? "ChevronDown" : "ChevronRight",
        {
          className: "khala-code-editor-row-chevron",
          dataIcon: isDirectory ? "directory-toggle" : "file-spacer",
        },
      )
      chevron.dataset.visible = isDirectory ? "true" : "false"
      const fileIcon = iconElement(iconForNode(node, item.expanded), {
        className: "khala-code-editor-row-icon",
        dataIcon: node.kind,
      })
      const label = el("span", "khala-code-editor-row-label", node.name)
      const change = el(
        "span",
        "khala-code-editor-row-change",
        changeLabel(node.changeKind),
      )
      const changeTitleText = changeTitle(node.changeKind)
      if (changeTitleText.length > 0) change.title = changeTitleText
      const status = el(
        "span",
        "khala-code-editor-row-status",
        item.loading ? "Loading" : item.error === null ? "" : "Error",
      )
      button.append(chevron, fileIcon, label, change, status)
      treeBody.append(button)
      if (item.error !== null && item.expanded) {
        const errorLine = el("div", "khala-code-editor-tree-error", item.error)
        errorLine.setAttribute("role", "alert")
        errorLine.style.setProperty("--khala-code-editor-tree-depth", String(node.depth + 1))
        treeBody.append(errorLine)
      }
    }
  }

  const expandDirectory = async (key: string): Promise<void> => {
    const item = treeItems.get(key)
    if (item === undefined || item.node.kind !== "directory" || item.loading) return
    item.expanded = true
    if (item.node.childrenLoaded) {
      renderTree()
      return
    }
    item.loading = true
    item.error = null
    renderTree()
    const result = await options.editorDirectoryRead({
      path: item.node.path,
      providerId: item.node.providerId,
    })
    if (destroyed) return
    item.loading = false
    if (!result.ok) {
      item.error = result.error.message
      renderTree()
      return
    }
    const sortedEntries = [...result.entries].sort(compareNodes)
    item.node = { ...result.node, childrenLoaded: true }
    item.children = sortedEntries.map(entry => nodeKey(storeItem(entry).node))
    item.error = null
    item.expanded = true
    renderTree()
  }

  const collapseDirectory = (key: string): void => {
    const item = treeItems.get(key)
    if (item === undefined || item.node.kind !== "directory") return
    item.expanded = false
    renderTree()
  }

  const openFile = async (key: string): Promise<void> => {
    const item = treeItems.get(key)
    if (item === undefined || item.node.kind === "directory") return
    const node = item.node
    const requestSeq = ++fileRequestSeq
    selectedKey = key
    renderTree()
    showSourceState(node.name, "Loading", "Loading source", "loading")
    const result = await options.editorFileRead({
      maxBytes: KHALA_CODE_EDITOR_DEFAULT_MAX_FILE_BYTES,
      path: node.path,
      providerId: node.providerId,
    })
    if (destroyed || requestSeq !== fileRequestSeq) return
    if (!result.ok) {
      showSourceState(errorTitle(result.error), formatBytes(node.sizeBytes), result.error.message, "unsupported")
      return
    }
    const language = languageForPath(result.path)
    const meta = `${languageLabel(language)} - ${formatBytes(result.sizeBytes)}`
    const displayPath = relativeDisplayPath({
      name: node.name,
      path: result.path,
      rootPath: result.rootPath,
    })
    const tab: EditorOpenTab = {
      displayPath,
      key,
      language,
      name: node.name,
      path: result.path,
      providerId: result.providerId,
    }
    openTabs = [
      tab,
      ...openTabs.filter(existing => existing.key !== key),
    ].slice(0, 12)
    renderTabs()
    showSourceState(node.name, meta, "Loading editor", "loading")
    activeFileContext = {
      displayPath,
      path: result.path,
      providerId: result.providerId,
      rootPath: result.rootPath,
    }
    sourceContextButton.disabled = options.onComposerContextSelected === undefined
    sourceContextButton.title = "Add selected file context"
    try {
      const monaco = await (options.loadMonaco ?? defaultLoadMonaco)()
      if (destroyed || requestSeq !== fileRequestSeq) return
      defineKhalaMonacoTheme(monaco, container)
      monacoHost.hidden = false
      sourceState.hidden = true
      sourceBody.dataset.state = "ready"
      fileTitle.textContent = node.name
      fileMeta.textContent = meta
      monacoEditor ??= monaco.editor.create(monacoHost, {
        automaticLayout: true,
        fontFamily: getComputedStyle(container).getPropertyValue("--oa-font-code").trim() ||
          "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
        largeFileOptimizations: true,
        minimap: { enabled: false },
        readOnly: true,
        scrollBeyondLastLine: false,
        theme: "khala-code",
      })
      monaco.editor.setTheme("khala-code")
      monacoEditor.setModel(null)
      monacoModel?.dispose()
      monacoModel = monaco.editor.createModel(result.content, language, monaco.Uri.file(result.path))
      monacoEditor.setModel(monacoModel)
      selectionChangeDisposable?.dispose()
      selectionChangeDisposable = monacoEditor.onDidChangeCursorSelection(() => {
        sourceContextButton.title = selectedLineRange().kind === "selection"
          ? "Add selected line context"
          : "Add selected file context"
      })
      sourceContextButton.title = "Add selected line context"
      monacoEditor.layout()
      renderTabs()
    } catch (error) {
      if (destroyed || requestSeq !== fileRequestSeq) return
      showSourceState(
        "Editor unavailable",
        meta,
        error instanceof Error ? error.message : String(error),
        "error",
      )
    }
  }

  const closeTab = (key: string): void => {
    const wasSelected = selectedKey === key
    openTabs = openTabs.filter(tab => tab.key !== key)
    if (wasSelected) {
      selectedKey = null
      showSourceState("No file selected", "Source", "No source open", "empty")
      const next = openTabs[0]
      renderTabs()
      renderTree()
      if (next !== undefined) {
        void openFile(next.key)
      }
      return
    }
    renderTabs()
  }

  const activateItem = async (key: string): Promise<void> => {
    const item = treeItems.get(key)
    if (item === undefined) return
    if (item.node.kind === "directory") {
      if (item.expanded) collapseDirectory(key)
      else await expandDirectory(key)
      return
    }
    await openFile(key)
  }

  const handleTreeKeydown = (event: KeyboardEvent): void => {
    if (renderedRows.length === 0) return
    const current = treeItems.get(focusedKey ?? "")
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault()
        focusByDelta(1)
        return
      case "ArrowUp":
        event.preventDefault()
        focusByDelta(-1)
        return
      case "Home":
        event.preventDefault()
        focusRow(renderedRows[0].key)
        return
      case "End":
        event.preventDefault()
        focusRow(renderedRows[renderedRows.length - 1].key)
        return
      case "ArrowRight":
        event.preventDefault()
        if (current?.node.kind === "directory") {
          if (!current.expanded) {
            void expandDirectory(focusedKey ?? "")
          } else if (current.children.length > 0) {
            focusRow(current.children[0])
          }
        }
        return
      case "ArrowLeft":
        event.preventDefault()
        if (current?.node.kind === "directory" && current.expanded) {
          collapseDirectory(focusedKey ?? "")
          return
        }
        if (current?.node.parentPath !== null && current?.node.parentPath !== undefined) {
          const parentKey = nodeKey({
            path: current.node.parentPath,
            providerId: current.node.providerId,
          })
          if (treeItems.has(parentKey)) focusRow(parentKey)
        }
        return
      case "Enter":
        event.preventDefault()
        void activateItem(focusedKey ?? "")
        return
    }
  }

  const openFocusedSearchResult = (): void => {
    const row = renderedRows.find(candidate => candidate.key === focusedKey) ?? renderedRows[0]
    if (row === undefined) return
    focusedKey = row.key
    void activateItem(row.key)
  }

  const loadWorkspace = async (): Promise<void> => {
    treeStatus.textContent = "Loading"
    treeBody.setAttribute("aria-busy", "true")
    renderTreeState("Loading workspace", "loading")
    const result = await options.editorWorkspaceRead()
    if (destroyed) return
    treeBody.removeAttribute("aria-busy")
    if (!result.ok) {
      treeStatus.textContent = "Error"
      workspaceLoaded = false
      renderTreeState(result.error.message, "error")
      return
    }
    treeItems.clear()
    rootKeys = result.roots.map(root => nodeKey(storeItem(rootNode(root)).node))
    focusedKey = rootKeys[0] ?? null
    selectedKey = null
    openTabs = []
    workspaceLoaded = true
    treeStatus.textContent = result.roots.length === 1 ? "Ready" : `${result.roots.length} roots`
    renderTree()
    if (rootKeys.length > 0) await expandDirectory(rootKeys[0])
  }

  const refresh = async (): Promise<void> => {
    workspaceLoading = null
    workspaceLoaded = false
    monacoEditor?.setModel(null)
    monacoModel?.dispose()
    monacoModel = null
    showSourceState("No file selected", "Source", "No source open", "empty")
    renderTabs()
    const load = loadWorkspace()
    workspaceLoading = load
    try {
      await load
    } finally {
      if (workspaceLoading === load) workspaceLoading = null
    }
  }

  const ensureWorkspaceLoaded = (): void => {
    if (workspaceLoaded || workspaceLoading !== null) return
    const load = loadWorkspace()
    workspaceLoading = load
    void load.finally(() => {
      if (workspaceLoading === load) workspaceLoading = null
    })
  }

  refreshButton.addEventListener("click", () => {
    void refresh()
  })
  searchInput.addEventListener("input", () => {
    searchQuery = searchInput.value
    renderTree()
  })
  searchInput.addEventListener("keydown", event => {
    if (event.key !== "Enter") return
    event.preventDefault()
    openFocusedSearchResult()
  })
  treeBody.addEventListener("keydown", handleTreeKeydown)

  renderTreeState("No workspace", "empty")
  renderTabs()

  return {
    destroy() {
      destroyed = true
      treeBody.removeEventListener("keydown", handleTreeKeydown)
      selectionChangeDisposable?.dispose()
      monacoEditor?.dispose()
      monacoModel?.dispose()
      container.replaceChildren()
    },
    refresh,
    setVisible(visible) {
      container.hidden = !visible
      panel.dataset.visible = visible ? "true" : "false"
      if (visible) {
        ensureWorkspaceLoaded()
        queueMicrotask(() => monacoEditor?.layout())
      }
    },
  }
}
