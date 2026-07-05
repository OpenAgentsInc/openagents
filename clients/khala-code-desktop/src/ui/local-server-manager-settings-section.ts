import type {
  KhalaCodeLocalServerContractProjection,
  KhalaCodeLocalServerManagerActionId,
} from "../shared/local-server-runtime"

export type KhalaCodeLocalServerManagerSettingsSectionHandle = Readonly<{
  render: () => HTMLElement
  refresh: () => Promise<void>
}>

export type KhalaCodeLocalServerManagerSettingsSectionOptions = Readonly<{
  fetch: () => Promise<KhalaCodeLocalServerContractProjection>
  runAction: (id: KhalaCodeLocalServerManagerActionId) => Promise<void> | void
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

const stateText = (value: string): string => value.replace(/_/g, " ")

export const mountKhalaCodeLocalServerManagerSettingsSection = (
  options: KhalaCodeLocalServerManagerSettingsSectionOptions,
): KhalaCodeLocalServerManagerSettingsSectionHandle => {
  let projection: KhalaCodeLocalServerContractProjection | null = null
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

  const runAction = async (id: KhalaCodeLocalServerManagerActionId): Promise<void> => {
    const action = projection?.actions.find(candidate => candidate.commandId === id)
    if (action?.enabled === false) {
      status = action.reason
      renderIntoCurrentSection()
      return
    }
    try {
      await options.runAction(id)
      status = id === "server.refresh" ? "Server health refreshed." : "Server action requested."
    } catch (error) {
      status = error instanceof Error ? error.message : String(error)
    }
    renderIntoCurrentSection()
  }

  function renderInto(section: HTMLElement): void {
    section.replaceChildren(el("h3", "khala-settings-section-title", "Local Server Runtime"))
    if (status.length > 0) section.append(el("div", "khala-keybindings-status khala-local-server-status", status))
    if (projection === null) {
      section.append(el("p", "khala-settings-empty", "Local server runtime state has not loaded yet."))
      return
    }

    section.append(el("p", "khala-local-server-boundary", projection.ownershipBoundary))

    const actions = el("div", "khala-local-server-actions")
    for (const action of projection.actions) {
      const button = el("button", "khala-local-server-action", action.label)
      button.type = "button"
      button.disabled = !action.enabled
      button.title = action.reason
      button.dataset.commandId = action.commandId
      button.addEventListener("click", () => {
        void runAction(action.commandId)
      })
      actions.append(button)
    }

    const rows = el("div", "khala-local-server-list")
    for (const row of projection.rows) {
      const item = el("div", "khala-local-server-row")
      item.dataset.kind = row.kind
      item.dataset.state = row.state
      item.append(
        el("span", "khala-local-server-title", `${row.label}${row.isDefault ? " / default" : ""}`),
        el("span", "khala-local-server-state", stateText(row.state)),
        el("span", "khala-local-server-detail", row.detail),
        el("span", "khala-local-server-reason", row.reason),
      )
      rows.append(item)
    }

    const capabilities = el("div", "khala-local-server-capabilities")
    for (const capability of projection.capabilities) {
      const item = el("span", "khala-local-server-capability", capability.label)
      item.dataset.required = capability.required ? "true" : "false"
      capabilities.append(item)
    }

    section.append(
      actions,
      el("div", "khala-status-usage-group-title", "Runtimes"),
      rows,
      el("div", "khala-status-usage-group-title", "Contract Capabilities"),
      capabilities,
      el("p", "khala-local-server-credential-policy", projection.credentialPolicy),
    )
  }

  const render = (): HTMLElement => {
    const section = el("section", "khala-settings-section khala-local-server-section")
    sectionNode = section
    renderInto(section)
    return section
  }

  return { render, refresh }
}
