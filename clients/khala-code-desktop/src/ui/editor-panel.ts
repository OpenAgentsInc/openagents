export type KhalaCodeEditorPanelHandle = Readonly<{
  setVisible: (visible: boolean) => void
}>

export const mountKhalaCodeEditorPanel = (
  container: HTMLElement,
): KhalaCodeEditorPanelHandle => {
  container.replaceChildren()

  const panel = document.createElement("div")
  panel.className = "khala-code-editor-panel"
  panel.dataset.khalaCodeEditorPanel = ""

  const treePane = document.createElement("section")
  treePane.className = "khala-code-editor-tree-pane"
  treePane.setAttribute("aria-label", "Editor workspace files")

  const treeHeader = document.createElement("div")
  treeHeader.className = "khala-code-editor-pane-header"

  const treeTitle = document.createElement("h2")
  treeTitle.className = "khala-code-editor-pane-title"
  treeTitle.textContent = "Workspace"

  const treeStatus = document.createElement("span")
  treeStatus.className = "khala-code-editor-pane-status"
  treeStatus.textContent = "Read only"

  treeHeader.append(treeTitle, treeStatus)

  const treeBody = document.createElement("div")
  treeBody.className = "khala-code-editor-tree-empty"
  treeBody.setAttribute("role", "tree")
  treeBody.setAttribute("aria-label", "Workspace file tree")

  const treeEmpty = document.createElement("div")
  treeEmpty.className = "khala-code-editor-empty-line"
  treeEmpty.textContent = "Workspace root"
  treeBody.append(treeEmpty)

  treePane.append(treeHeader, treeBody)

  const sourcePane = document.createElement("section")
  sourcePane.className = "khala-code-editor-source-pane"
  sourcePane.setAttribute("aria-label", "Editor source view")

  const sourceHeader = document.createElement("div")
  sourceHeader.className = "khala-code-editor-source-header"

  const fileTitle = document.createElement("h2")
  fileTitle.className = "khala-code-editor-source-title"
  fileTitle.textContent = "No file selected"

  const fileMeta = document.createElement("span")
  fileMeta.className = "khala-code-editor-source-meta"
  fileMeta.textContent = "Source"

  sourceHeader.append(fileTitle, fileMeta)

  const sourceBody = document.createElement("div")
  sourceBody.className = "khala-code-editor-source-empty"
  sourceBody.textContent = "No source open"

  sourcePane.append(sourceHeader, sourceBody)
  panel.append(treePane, sourcePane)
  container.append(panel)

  return {
    setVisible(visible) {
      container.hidden = !visible
      panel.dataset.visible = visible ? "true" : "false"
    },
  }
}
