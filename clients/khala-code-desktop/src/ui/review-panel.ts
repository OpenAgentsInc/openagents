import { iconElement } from "@openagentsinc/ui/icon-dom"
import type {
  KhalaCodeDiffReviewComment,
  KhalaCodeDiffReviewSubmitDetail,
} from "../shared/diff-review"
import { khalaCodeDiffReviewLineLabel } from "../shared/diff-review"
import {
  KHALA_CODE_REVIEW_LAYOUT_STORAGE_KEY,
  khalaCodeClampReviewLayoutWidth,
  khalaCodeDefaultReviewFocus,
  khalaCodeGroupReviewFiles,
  khalaCodeParseReviewLayout,
  khalaCodeReviewDiffKindLabel,
  khalaCodeReviewRevertState,
  khalaCodeSerializeReviewLayout,
  type KhalaCodeReviewFileEntry,
  type KhalaCodeReviewFocus,
  type KhalaCodeReviewLayoutState,
} from "../shared/review-panel"
import type {
  KhalaCodeDesktopReviewDiffReadRequest,
  KhalaCodeDesktopReviewDiffReadResult,
} from "../shared/rpc"

export type KhalaCodeReviewPanelHandle = Readonly<{
  addComment: (comment: KhalaCodeDiffReviewComment) => void
  destroy: () => void
  refresh: () => Promise<void>
  setVisible: (visible: boolean) => void
}>

export type KhalaCodeReviewComposerContext = Readonly<{
  displayPath: string
  path: string
}>

export type KhalaCodeReviewPanelServices = Readonly<{
  reviewDiffRead: (
    request?: KhalaCodeDesktopReviewDiffReadRequest,
  ) => Promise<KhalaCodeDesktopReviewDiffReadResult>
}>

export type KhalaCodeReviewPanelStorage = Pick<Storage, "getItem" | "setItem">

export type KhalaCodeReviewPanelOptions = KhalaCodeReviewPanelServices & Readonly<{
  onCommentSubmit?: (detail: KhalaCodeDiffReviewSubmitDetail) => void
  onComposerContextSelected?: (context: KhalaCodeReviewComposerContext) => void
  storage?: KhalaCodeReviewPanelStorage
}>

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

const noopStorage: KhalaCodeReviewPanelStorage = {
  getItem: () => null,
  setItem: () => undefined,
}

const defaultStorage = (): KhalaCodeReviewPanelStorage => {
  try {
    return globalThis.localStorage ?? noopStorage
  } catch {
    return noopStorage
  }
}

export const mountKhalaCodeReviewPanel = (
  container: HTMLElement,
  options: KhalaCodeReviewPanelOptions,
): KhalaCodeReviewPanelHandle => {
  container.replaceChildren()

  const storage = options.storage ?? defaultStorage()
  let layout: KhalaCodeReviewLayoutState = khalaCodeParseReviewLayout(
    storage.getItem(KHALA_CODE_REVIEW_LAYOUT_STORAGE_KEY),
  )
  let files: ReadonlyArray<KhalaCodeReviewFileEntry> | null = null
  let comments: KhalaCodeDiffReviewComment[] = []
  let focus: KhalaCodeReviewFocus = khalaCodeDefaultReviewFocus()
  let loading = false
  let loadError: string | null = null
  let truncated = false
  let destroyed = false
  let loadSeq = 0

  const persistLayout = (): void => {
    storage.setItem(KHALA_CODE_REVIEW_LAYOUT_STORAGE_KEY, khalaCodeSerializeReviewLayout(layout))
  }

  const panel = el("div", "khala-code-review-panel")
  panel.dataset.khalaCodeReviewPanel = ""
  panel.style.setProperty("--khala-code-review-width", `${layout.widthPx}px`)

  const header = el("div", "khala-code-review-header")
  const title = el("h2", "khala-code-review-title", "Review")

  const tabs = el("div", "khala-code-review-tabs")
  tabs.setAttribute("role", "tablist")
  tabs.setAttribute("aria-label", "Review panel tabs")
  const filesTabButton = document.createElement("button")
  filesTabButton.type = "button"
  filesTabButton.className = "khala-code-review-tab"
  filesTabButton.dataset.reviewTab = "files"
  filesTabButton.setAttribute("role", "tab")
  filesTabButton.textContent = "Files"
  const commentsTabButton = document.createElement("button")
  commentsTabButton.type = "button"
  commentsTabButton.className = "khala-code-review-tab"
  commentsTabButton.dataset.reviewTab = "comments"
  commentsTabButton.setAttribute("role", "tab")
  commentsTabButton.textContent = "Comments"
  tabs.append(filesTabButton, commentsTabButton)

  const headerActions = el("div", "khala-code-review-header-actions")
  const narrowButton = document.createElement("button")
  narrowButton.type = "button"
  narrowButton.className = "khala-code-review-width-button"
  narrowButton.setAttribute("aria-label", "Narrow review panel")
  narrowButton.title = "Narrow review panel"
  narrowButton.append(iconElement("ArrowLeftSm", {
    className: "khala-code-review-header-icon",
    dataIcon: "review-narrow",
  }))
  const widenButton = document.createElement("button")
  widenButton.type = "button"
  widenButton.className = "khala-code-review-width-button"
  widenButton.setAttribute("aria-label", "Widen review panel")
  widenButton.title = "Widen review panel"
  widenButton.append(iconElement("ArrowRightSm", {
    className: "khala-code-review-header-icon",
    dataIcon: "review-widen",
  }))
  const refreshButton = document.createElement("button")
  refreshButton.type = "button"
  refreshButton.className = "khala-code-review-refresh-button"
  refreshButton.setAttribute("aria-label", "Refresh review diff")
  refreshButton.title = "Refresh review diff"
  refreshButton.append(iconElement("Reload", {
    className: "khala-code-review-header-icon",
    dataIcon: "review-refresh",
  }))
  const collapseButton = document.createElement("button")
  collapseButton.type = "button"
  collapseButton.className = "khala-code-review-collapse-button"
  collapseButton.setAttribute("aria-label", "Collapse review panel")
  collapseButton.title = "Collapse review panel"
  headerActions.append(narrowButton, widenButton, refreshButton, collapseButton)

  header.append(title, tabs, headerActions)

  const body = el("div", "khala-code-review-body")
  const filesRegion = el("div", "khala-code-review-files")
  filesRegion.setAttribute("role", "tabpanel")
  filesRegion.setAttribute("aria-label", "Changed files")
  const commentsRegion = el("div", "khala-code-review-comments")
  commentsRegion.setAttribute("role", "tabpanel")
  commentsRegion.setAttribute("aria-label", "Review comments")
  body.append(filesRegion, commentsRegion)

  panel.append(header, body)
  container.append(panel)

  const renderComments = (): void => {
    commentsRegion.replaceChildren()
    if (comments.length === 0) {
      const empty = el("div", "khala-code-review-empty-state", "No review comments yet")
      commentsRegion.append(empty)
      return
    }
    const list = el("ul", "khala-code-review-comment-list")
    list.setAttribute("aria-label", "Review comments")
    for (const comment of [...comments].reverse()) {
      const item = el("li", "khala-code-review-comment-item")
      item.dataset.commentRef = comment.commentRef
      const labelText = comment.lineNo === 0
        ? `${comment.filePath} (file comment)`
        : khalaCodeDiffReviewLineLabel(comment)
      const label = el("div", "khala-code-review-comment-label", labelText)
      const body = el("div", "khala-code-review-comment-body", comment.body)
      item.append(label, body)
      list.append(item)
    }
    commentsRegion.append(list)
  }

  const submitFileComment = (path: string, body: string): void => {
    const trimmed = body.trim()
    if (trimmed.length === 0) return
    const detail: KhalaCodeDiffReviewSubmitDetail = {
      body: trimmed,
      filePath: path,
      lineKind: "context",
      lineNo: 0,
      lineSide: "new",
      patchRef: `review_panel.${path}`,
    }
    options.onCommentSubmit?.(detail)
  }

  const renderFileDetail = (file: KhalaCodeReviewFileEntry): HTMLElement => {
    const detail = el("div", "khala-code-review-file-detail")
    detail.dataset.diffKind = file.diffKind

    const summary = el(
      "div",
      "khala-code-review-file-detail-summary",
      `${khalaCodeReviewDiffKindLabel(file.diffKind)} - +${file.additions}/-${file.deletions}`,
    )
    if (file.renamedFrom !== undefined) {
      summary.textContent += ` (renamed from ${file.renamedFrom})`
    }

    const actions = el("div", "khala-code-review-file-actions")

    const addContextButton = document.createElement("button")
    addContextButton.type = "button"
    addContextButton.className = "khala-code-review-add-context-button"
    addContextButton.textContent = "Add to composer"
    addContextButton.disabled = options.onComposerContextSelected === undefined
    addContextButton.addEventListener("click", () => {
      options.onComposerContextSelected?.({ displayPath: file.path, path: file.path })
    })

    const revert = khalaCodeReviewRevertState()
    const revertButton = document.createElement("button")
    revertButton.type = "button"
    revertButton.className = "khala-code-review-revert-button"
    revertButton.dataset.reviewRevertState = revert.kind
    revertButton.setAttribute("aria-disabled", "true")
    revertButton.title = revert.message
    revertButton.setAttribute("aria-label", `Revert ${file.path}: ${revert.message}`)
    revertButton.textContent = "Revert"
    const revertStatus = el("div", "khala-code-review-revert-status", revert.message)
    revertStatus.setAttribute("role", "status")
    revertStatus.hidden = true
    revertButton.addEventListener("click", event => {
      event.preventDefault()
      revertStatus.hidden = false
    })

    actions.append(addContextButton, revertButton)

    const commentForm = el("div", "khala-code-review-comment-form")
    const textarea = document.createElement("textarea")
    textarea.className = "khala-code-review-comment-textarea"
    textarea.rows = 2
    textarea.placeholder = `Comment on ${file.path}`
    textarea.setAttribute("aria-label", `Add a review comment for ${file.path}`)
    const submitButton = document.createElement("button")
    submitButton.type = "button"
    submitButton.className = "khala-code-review-comment-submit"
    submitButton.textContent = "Comment"
    submitButton.addEventListener("click", () => {
      submitFileComment(file.path, textarea.value)
      textarea.value = ""
      requestAnimationFrame(() => textarea.focus({ preventScroll: true }))
    })
    textarea.addEventListener("keydown", event => {
      if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault()
        submitButton.click()
      }
    })
    commentForm.append(textarea, submitButton, revertStatus)

    detail.append(summary, actions, commentForm)
    return detail
  }

  const renderFiles = (): void => {
    filesRegion.replaceChildren()
    if (loading) {
      filesRegion.dataset.state = "loading"
      filesRegion.append(el("div", "khala-code-review-state", "Loading review diff"))
      return
    }
    if (loadError !== null) {
      filesRegion.dataset.state = "error"
      const error = el("div", "khala-code-review-state", loadError)
      error.setAttribute("role", "alert")
      filesRegion.append(error)
      return
    }
    if (files === null || files.length === 0) {
      filesRegion.dataset.state = "empty"
      filesRegion.append(el("div", "khala-code-review-state", "No changes to review"))
      return
    }
    filesRegion.dataset.state = "ready"
    if (truncated) {
      filesRegion.append(el(
        "div",
        "khala-code-review-truncated-note",
        "Diff truncated for display; not all changes are shown.",
      ))
    }
    for (const group of khalaCodeGroupReviewFiles(files)) {
      const section = el("section", "khala-code-review-group")
      section.dataset.diffKind = group.diffKind
      const heading = el(
        "h3",
        "khala-code-review-group-heading",
        `${khalaCodeReviewDiffKindLabel(group.diffKind)} (${group.files.length})`,
      )
      section.append(heading)
      const list = el("div", "khala-code-review-group-list")
      list.setAttribute("role", "list")
      for (const file of group.files) {
        const row = document.createElement("button")
        row.type = "button"
        row.className = "khala-code-review-file-row"
        row.setAttribute("role", "listitem")
        row.dataset.diffKind = file.diffKind
        row.dataset.reviewFilePath = file.path
        row.dataset.selected = focus.filePath === file.path ? "true" : "false"
        row.setAttribute("aria-pressed", focus.filePath === file.path ? "true" : "false")
        row.title = file.path

        const badge = el("span", "khala-code-review-file-badge", file.diffKind === "added"
          ? "+"
          : file.diffKind === "deleted"
            ? "-"
            : "~")
        const label = el("span", "khala-code-review-file-path", file.path)
        const stats = el("span", "khala-code-review-file-stats", `+${file.additions}/-${file.deletions}`)
        row.append(badge, label, stats)
        row.addEventListener("click", () => {
          focus = { filePath: focus.filePath === file.path ? null : file.path }
          renderFiles()
        })
        list.append(row)
        if (focus.filePath === file.path) {
          list.append(renderFileDetail(file))
        }
      }
      section.append(list)
      filesRegion.append(section)
    }
  }

  const applyLayout = (): void => {
    panel.dataset.collapsed = layout.collapsed ? "true" : "false"
    panel.dataset.activeTab = layout.activeTab
    panel.style.setProperty("--khala-code-review-width", `${layout.widthPx}px`)
    filesTabButton.dataset.active = layout.activeTab === "files" ? "true" : "false"
    filesTabButton.setAttribute("aria-selected", layout.activeTab === "files" ? "true" : "false")
    commentsTabButton.dataset.active = layout.activeTab === "comments" ? "true" : "false"
    commentsTabButton.setAttribute("aria-selected", layout.activeTab === "comments" ? "true" : "false")
    filesRegion.hidden = layout.collapsed || layout.activeTab !== "files"
    commentsRegion.hidden = layout.collapsed || layout.activeTab !== "comments"
    collapseButton.replaceChildren(iconElement(layout.collapsed ? "ChevronRight" : "ChevronLeft", {
      className: "khala-code-review-header-icon",
      dataIcon: layout.collapsed ? "review-expand" : "review-collapse",
    }))
    collapseButton.setAttribute("aria-label", layout.collapsed ? "Expand review panel" : "Collapse review panel")
    collapseButton.title = collapseButton.getAttribute("aria-label") ?? ""
    narrowButton.disabled = layout.widthPx <= 240
    widenButton.disabled = layout.widthPx >= 640
  }

  filesTabButton.addEventListener("click", () => {
    layout = { ...layout, activeTab: "files" }
    persistLayout()
    applyLayout()
  })
  commentsTabButton.addEventListener("click", () => {
    layout = { ...layout, activeTab: "comments" }
    persistLayout()
    applyLayout()
  })
  collapseButton.addEventListener("click", () => {
    layout = { ...layout, collapsed: !layout.collapsed }
    persistLayout()
    applyLayout()
  })
  narrowButton.addEventListener("click", () => {
    layout = { ...layout, widthPx: khalaCodeClampReviewLayoutWidth(layout.widthPx - 40) }
    persistLayout()
    applyLayout()
  })
  widenButton.addEventListener("click", () => {
    layout = { ...layout, widthPx: khalaCodeClampReviewLayoutWidth(layout.widthPx + 40) }
    persistLayout()
    applyLayout()
  })

  const refresh = async (): Promise<void> => {
    const seq = ++loadSeq
    loading = true
    loadError = null
    renderFiles()
    try {
      const result = await options.reviewDiffRead()
      if (destroyed || seq !== loadSeq) return
      loading = false
      if (!result.ok) {
        loadError = result.error.message
        files = null
        renderFiles()
        return
      }
      files = result.files
      truncated = result.truncated
      if (focus.filePath !== null && !files.some(file => file.path === focus.filePath)) {
        focus = khalaCodeDefaultReviewFocus()
      }
      renderFiles()
    } catch (error) {
      if (destroyed || seq !== loadSeq) return
      loading = false
      loadError = error instanceof Error ? error.message : "Review diff request failed."
      files = null
      renderFiles()
    }
  }

  refreshButton.addEventListener("click", () => {
    void refresh()
  })

  applyLayout()
  renderFiles()
  renderComments()

  let loaded = false

  return {
    addComment(comment) {
      comments = [...comments, comment]
      renderComments()
    },
    destroy() {
      destroyed = true
      container.replaceChildren()
    },
    refresh,
    setVisible(visible) {
      container.hidden = !visible
      panel.dataset.visible = visible ? "true" : "false"
      if (visible && !loaded) {
        loaded = true
        void refresh()
      }
    },
  }
}

export const khalaCodeReviewPanelFileButton = (
  container: HTMLElement,
  path: string,
): HTMLButtonElement | null =>
  Array.from(container.querySelectorAll<HTMLButtonElement>("[data-review-file-path]"))
    .find(button => button.dataset.reviewFilePath === path) ?? null
