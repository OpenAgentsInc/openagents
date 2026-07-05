import type {
  KhalaCodeSupportEntrypointId,
  KhalaCodeSupportProjection,
} from "../shared/support-entrypoints"

export type KhalaCodeSupportSettingsSectionHandle = Readonly<{
  render: () => HTMLElement
  refresh: () => Promise<void>
}>

export type KhalaCodeSupportSettingsSectionOptions = Readonly<{
  copyIssueMetadata: (metadata: string) => Promise<void>
  exportDiagnostics: () => Promise<{ readonly ok: boolean; readonly message: string }>
  fetch: () => Promise<KhalaCodeSupportProjection>
  open: (id: KhalaCodeSupportEntrypointId, url: string) => Promise<boolean>
}>

const el = <Tag extends keyof HTMLElementTagNameMap>(
  tag: Tag,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[Tag] => {
  const node = document.createElement(tag)
  if (className !== undefined) node.className = className
  if (text !== undefined) node.textContent = text
  return node
}

export const mountKhalaCodeSupportSettingsSection = (
  options: KhalaCodeSupportSettingsSectionOptions,
): KhalaCodeSupportSettingsSectionHandle => {
  let projection: KhalaCodeSupportProjection | null = null
  let status = ""
  let sectionNode: HTMLElement | null = null

  const renderIntoCurrentSection = (): void => {
    if (sectionNode !== null) renderInto(sectionNode)
  }

  const refresh = async (): Promise<void> => {
    try {
      projection = await options.fetch()
      status = ""
    } catch (error) {
      status = error instanceof Error ? error.message : String(error)
    }
    renderIntoCurrentSection()
  }

  const runOpen = async (id: KhalaCodeSupportEntrypointId, url: string): Promise<void> => {
    const opened = await options.open(id, url)
    status = opened ? "Opened support link." : "Support link was blocked by the allowlist."
    renderIntoCurrentSection()
  }

  const exportDiagnostics = async (): Promise<void> => {
    const result = await options.exportDiagnostics()
    status = result.message
    renderIntoCurrentSection()
  }

  const copyMetadata = async (): Promise<void> => {
    if (projection === null) return
    await options.copyIssueMetadata(projection.issueMetadata)
    status = "Copied public-safe issue metadata."
    renderIntoCurrentSection()
  }

  function renderInto(section: HTMLElement): void {
    section.replaceChildren(el("h3", "khala-settings-section-title", "Help And Support"))
    if (status.length > 0) section.append(el("div", "khala-keybindings-status khala-support-status", status))
    if (projection === null) {
      section.append(el("p", "khala-settings-empty", "Support entrypoints have not loaded yet."))
      return
    }

    const links = el("div", "khala-support-links")
    for (const entry of projection.entries) {
      const button = el("button", "khala-support-link", entry.label)
      button.type = "button"
      button.dataset.supportId = entry.id
      button.addEventListener("click", () => {
        void runOpen(entry.id, entry.url)
      })
      links.append(button)
    }

    const actions = el("div", "khala-support-actions")
    const exportButton = el("button", "khala-support-action", "Export Diagnostics")
    exportButton.type = "button"
    exportButton.addEventListener("click", () => {
      void exportDiagnostics()
    })
    const copyButton = el("button", "khala-support-action", "Copy Issue Metadata")
    copyButton.type = "button"
    copyButton.addEventListener("click", () => {
      void copyMetadata()
    })
    actions.append(exportButton, copyButton)

    const metadata = el("pre", "khala-support-metadata", projection.issueMetadata)
    section.append(links, actions, metadata)
  }

  const render = (): HTMLElement => {
    const section = el("section", "khala-settings-section khala-support-section")
    sectionNode = section
    renderInto(section)
    return section
  }

  return { render, refresh }
}
