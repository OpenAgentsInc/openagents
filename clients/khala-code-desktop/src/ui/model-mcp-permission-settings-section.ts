import type { KhalaCodeDesktopCodexEcosystemProjection } from "../shared/codex-ecosystem"
import type { KhalaCodeDesktopCodexSettingsProjection } from "../shared/codex-settings"
import {
  filterKhalaCodeModelManagerEntries,
  khalaCodeMcpManagerIntent,
  projectKhalaCodeModelMcpPermissionManager,
  type KhalaCodeMcpManagerEntry,
  type KhalaCodePermissionAutoAcceptMode,
} from "../shared/model-mcp-permission-manager"

export type KhalaCodeModelMcpPermissionSettingsSectionHandle = Readonly<{
  render: () => HTMLElement
  refresh: () => Promise<void>
}>

export type KhalaCodeModelMcpPermissionSettingsSectionOptions = Readonly<{
  fetchEcosystem: () => Promise<KhalaCodeDesktopCodexEcosystemProjection>
  fetchSettings: () => Promise<KhalaCodeDesktopCodexSettingsProjection>
  writePermissionProfile: (profileId: string) => Promise<{
    readonly ok: boolean
    readonly settings?: KhalaCodeDesktopCodexSettingsProjection
    readonly error?: string
  }>
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

export const mountKhalaCodeModelMcpPermissionSettingsSection = (
  options: KhalaCodeModelMcpPermissionSettingsSectionOptions,
): KhalaCodeModelMcpPermissionSettingsSectionHandle => {
  let settings: KhalaCodeDesktopCodexSettingsProjection | null = null
  let ecosystem: KhalaCodeDesktopCodexEcosystemProjection | null = null
  let hiddenModelIds = new Set<string>()
  let modelSearch = ""
  let permissionAutoAcceptMode: KhalaCodePermissionAutoAcceptMode = "manual"
  let status = ""
  let sectionNode: HTMLElement | null = null

  const renderIntoCurrentSection = (): void => {
    if (sectionNode !== null) renderInto(sectionNode)
  }

  const setStatus = (message: string): void => {
    status = message
    renderIntoCurrentSection()
  }

  const refresh = async (): Promise<void> => {
    const failures: string[] = []
    const [settingsResult, ecosystemResult] = await Promise.allSettled([
      options.fetchSettings(),
      options.fetchEcosystem(),
    ])
    if (settingsResult.status === "fulfilled") settings = settingsResult.value
    else failures.push(`Settings refresh failed: ${settingsResult.reason instanceof Error ? settingsResult.reason.message : String(settingsResult.reason)}`)
    if (ecosystemResult.status === "fulfilled") ecosystem = ecosystemResult.value
    else failures.push(`MCP refresh failed: ${ecosystemResult.reason instanceof Error ? ecosystemResult.reason.message : String(ecosystemResult.reason)}`)
    status = failures.join("\n")
    renderIntoCurrentSection()
  }

  const renderModels = (
    projection: ReturnType<typeof projectKhalaCodeModelMcpPermissionManager>,
  ): HTMLElement => {
    const wrap = el("div", "khala-model-manager")
    const search = el("input", "khala-settings-select khala-model-manager-search")
    search.type = "search"
    search.name = "model-manager-search"
    search.placeholder = "Search models"
    search.value = modelSearch
    search.addEventListener("input", () => {
      modelSearch = search.value
      renderIntoCurrentSection()
    })
    wrap.append(search)

    const rows = el("div", "khala-model-manager-list")
    let lastProvider = ""
    for (const model of filterKhalaCodeModelManagerEntries(projection.models, modelSearch)) {
      if (model.providerDisplayName !== lastProvider) {
        lastProvider = model.providerDisplayName
        rows.append(el("div", "khala-model-manager-provider", lastProvider))
      }
      const row = el("div", "khala-model-manager-row")
      row.dataset.modelId = model.id
      row.dataset.state = model.state
      row.dataset.visible = model.visible ? "true" : "false"

      const label = el("div", "khala-model-manager-label")
      label.append(
        el("span", "khala-model-manager-name", model.displayName),
        el("span", "khala-model-manager-meta", `${model.id} / ${stateText(model.state)}`),
      )
      const detail = el("span", "khala-model-manager-detail", model.detail)
      const toggle = el("button", "khala-model-manager-action", model.hiddenByUser ? "Show" : "Hide")
      toggle.type = "button"
      toggle.disabled = model.hiddenByRuntime
      toggle.addEventListener("click", () => {
        const next = new Set(hiddenModelIds)
        if (next.has(model.id)) next.delete(model.id)
        else next.add(model.id)
        hiddenModelIds = next
        setStatus(`${model.displayName} visibility ${next.has(model.id) ? "hidden" : "shown"} locally.`)
      })
      row.append(label, detail, toggle)
      rows.append(row)
    }
    wrap.append(rows)
    return wrap
  }

  const renderMcpEntry = (entry: KhalaCodeMcpManagerEntry): HTMLElement => {
    const row = el("div", "khala-mcp-manager-row")
    row.dataset.serverId = entry.id
    row.dataset.state = entry.state
    const label = el("div", "khala-mcp-manager-label")
    label.append(
      el("span", "khala-mcp-manager-name", entry.name),
      el("span", "khala-mcp-manager-meta", `${stateText(entry.state)} / ${entry.enabled ? "enabled" : "disabled"}`),
    )
    const detail = el("span", "khala-mcp-manager-detail", entry.detail)
    const actionLabel = entry.state === "needs_auth" ? "Login" : entry.enabled ? "Disable" : "Enable"
    const action = el("button", "khala-mcp-manager-action", actionLabel)
    action.type = "button"
    action.disabled = entry.state === "disabled"
    action.addEventListener("click", () => {
      const intent = khalaCodeMcpManagerIntent(
        entry,
        entry.state === "needs_auth" ? "authenticate" : entry.enabled ? "disable" : "enable",
      )
      setStatus(intent.message)
    })
    row.append(label, detail, action)
    return row
  }

  const renderMcp = (
    projection: ReturnType<typeof projectKhalaCodeModelMcpPermissionManager>,
  ): HTMLElement => {
    const wrap = el("div", "khala-mcp-manager-list")
    if (projection.mcp.length === 0) {
      wrap.append(el("p", "khala-settings-empty", "No MCP servers are visible from Codex yet."))
      return wrap
    }
    for (const entry of projection.mcp) wrap.append(renderMcpEntry(entry))
    return wrap
  }

  const renderPermissions = (
    projection: ReturnType<typeof projectKhalaCodeModelMcpPermissionManager>,
  ): HTMLElement => {
    const wrap = el("div", "khala-permission-manager")
    const select = el("select", "khala-settings-select")
    select.name = "permission-manager-profile"
    for (const profile of projection.permissions.profiles) {
      const option = el("option")
      option.value = profile.id
      option.textContent = profile.label
      option.disabled = !profile.allowed
      option.selected = profile.selected
      select.append(option)
    }
    select.addEventListener("change", () => {
      void options.writePermissionProfile(select.value).then(result => {
        if (result.ok) {
          if (result.settings !== undefined) settings = result.settings
          setStatus(`Permission profile set to ${select.value}.`)
          return
        }
        setStatus(result.error ?? `Failed to set permission profile ${select.value}.`)
      })
    })
    wrap.append(select)

    const modes = el("div", "khala-permission-auto-row")
    for (const mode of ["manual", "session", "directory"] as const) {
      const button = el("button", "khala-permission-auto-action", mode)
      button.type = "button"
      button.dataset.selected = permissionAutoAcceptMode === mode ? "true" : "false"
      button.disabled = !projection.permissions.autoAccept.allowed && mode !== "manual"
      button.addEventListener("click", () => {
        permissionAutoAcceptMode = mode
        setStatus(projectKhalaCodeModelMcpPermissionManager({
          ecosystem,
          hiddenModelIds,
          permissionAutoAcceptMode,
          settings: settings!,
        }).permissions.autoAccept.detail)
      })
      modes.append(button)
    }
    wrap.append(
      modes,
      el("p", "khala-permission-auto-detail", projection.permissions.autoAccept.detail),
    )
    return wrap
  }

  function renderInto(section: HTMLElement): void {
    section.replaceChildren(el("h3", "khala-settings-section-title", "Models, MCP, Permissions"))
    if (status.length > 0) section.append(el("div", "khala-keybindings-status khala-model-mcp-status", status))
    if (settings === null) {
      section.append(el("p", "khala-settings-empty", "Model, MCP, and permission state has not been loaded yet."))
      return
    }
    const projection = projectKhalaCodeModelMcpPermissionManager({
      ecosystem,
      hiddenModelIds,
      permissionAutoAcceptMode,
      settings,
    })
    section.append(
      renderModels(projection),
      renderMcp(projection),
      renderPermissions(projection),
    )
  }

  const render = (): HTMLElement => {
    const section = el("section", "khala-settings-section khala-model-mcp-section")
    sectionNode = section
    renderInto(section)
    return section
  }

  return { render, refresh }
}
