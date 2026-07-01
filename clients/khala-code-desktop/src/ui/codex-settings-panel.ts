import type {
  KhalaCodeDesktopCodexConfigValueWriteRequest,
} from "../shared/rpc"
import type {
  KhalaCodeDesktopCodexSettingsProjection,
} from "../shared/codex-settings"
import type {
  KhalaCodeDesktopCodexEcosystemProjection,
  KhalaCodeDesktopCodexEcosystemSection,
} from "../shared/codex-ecosystem"

export type CodexSettingsPanelHandle = {
  readonly refresh: () => Promise<void>
  readonly setVisible: (visible: boolean) => void
}

export type CodexSettingsPanelOptions = {
  readonly fetch: () => Promise<KhalaCodeDesktopCodexSettingsProjection>
  readonly fetchEcosystem?: () => Promise<KhalaCodeDesktopCodexEcosystemProjection>
  readonly write: (
    request: KhalaCodeDesktopCodexConfigValueWriteRequest,
  ) => Promise<{
    readonly ok: boolean
    readonly settings?: KhalaCodeDesktopCodexSettingsProjection
    readonly error?: string
  }>
}

type SelectOption = {
  readonly disabled?: boolean
  readonly label: string
  readonly value: string
}

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

const compactValue = (value: unknown): string => {
  if (value === null || value === undefined) return "Unset"
  if (typeof value === "string") return value
  if (typeof value === "number" || typeof value === "boolean" || typeof value === "bigint") {
    return String(value)
  }
  if (Array.isArray(value)) return `${value.length} items`
  if (typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>)
      .filter(([, entryValue]) => entryValue !== null && entryValue !== undefined)
      .slice(0, 3)
      .map(([key, entryValue]) => `${key}: ${compactValue(entryValue)}`)
    return entries.length === 0 ? "{}" : entries.join(", ")
  }
  return String(value)
}

const metric = (label: string, value: unknown): HTMLElement => {
  const item = el("div", "khala-settings-metric")
  item.append(
    el("span", "khala-settings-metric-label", label),
    el("span", "khala-settings-metric-value", compactValue(value)),
  )
  return item
}

const section = (title: string, children: readonly Node[]): HTMLElement => {
  const node = el("section", "khala-settings-section")
  node.append(el("h3", "khala-settings-section-title", title), ...children)
  return node
}

const renderSelect = (
  input: {
    readonly disabled?: boolean
    readonly label: string
    readonly options: readonly SelectOption[]
    readonly selected: string | null
    readonly title?: string
    readonly onChange: (value: string) => void
  },
): HTMLElement => {
  const row = el("label", "khala-settings-control")
  const label = el("span", "khala-settings-control-label", input.label)
  const select = el("select", "khala-settings-select")
  select.disabled = input.disabled === true || input.options.length === 0
  if (input.title !== undefined) select.title = input.title
  for (const option of input.options) {
    const item = el("option")
    item.value = option.value
    item.textContent = option.label
    item.disabled = option.disabled === true
    if (input.selected !== null && option.value === input.selected) item.selected = true
    select.append(item)
  }
  select.addEventListener("change", () => input.onChange(select.value))
  row.append(label, select)
  return row
}

export const mountCodexSettingsPanel = (
  container: HTMLElement,
  options: CodexSettingsPanelOptions,
): CodexSettingsPanelHandle => {
  let settings: KhalaCodeDesktopCodexSettingsProjection | null = null
  let ecosystem: KhalaCodeDesktopCodexEcosystemProjection | null = null
  let loading = false
  let visible = false
  let status = ""

  const setStatus = (message: string): void => {
    status = message
    render()
  }

  const write = async (
    keyPath: string,
    value: KhalaCodeDesktopCodexConfigValueWriteRequest["value"],
  ): Promise<void> => {
    setStatus(`Saving ${keyPath}`)
    const result = await options.write({ keyPath, value })
    if (result.ok) {
      if (result.settings !== undefined) settings = result.settings
      setStatus(`Saved ${keyPath}`)
      return
    }
    setStatus(result.error ?? `Failed to save ${keyPath}`)
  }

  const renderModelSection = (
    current: KhalaCodeDesktopCodexSettingsProjection,
  ): HTMLElement => {
    const selectedModel = current.models.selected
    const reasoningOptions = selectedModel?.supportedReasoningEfforts.map(option => ({
      label: option.description === null ? option.value : `${option.value} - ${option.description}`,
      value: option.value,
    })) ?? []
    const serviceTierOptions = [
      { label: "Default", value: "" },
      ...(selectedModel?.serviceTiers.map(tier => ({
        label: tier.name,
        value: tier.id,
      })) ?? []),
    ]
    return section("Model", [
      renderSelect({
        label: "Model",
        selected: selectedModel?.id ?? current.config.model,
        options: current.models.options.map(model => ({
          label: model.hidden ? `${model.displayName} (hidden)` : model.displayName,
          value: model.id,
        })),
        onChange: value => void write("model", value),
      }),
      renderSelect({
        disabled: reasoningOptions.length === 0,
        label: "Reasoning",
        selected: current.config.reasoningEffort ?? selectedModel?.defaultReasoningEffort ?? null,
        options: reasoningOptions,
        onChange: value => void write("model_reasoning_effort", value),
      }),
      renderSelect({
        disabled: serviceTierOptions.length <= 1,
        label: "Service tier",
        selected: current.config.serviceTier ?? "",
        options: serviceTierOptions,
        onChange: value => void write("service_tier", value === "" ? null : value),
      }),
      metric("Provider", current.config.modelProvider),
      metric("Summary", current.config.reasoningSummary),
      metric("Verbosity", current.config.verbosity),
    ])
  }

  const renderBoundarySection = (): HTMLElement => section("Harness Boundary", [
    metric("Default chat", "Codex app-server"),
    metric("Primary session", "User Codex home"),
    metric("Fleet workers", "Isolated Pylon Codex homes"),
    metric("Legacy mode", "Experimental fallback only"),
  ])

  const renderPermissionsSection = (
    current: KhalaCodeDesktopCodexSettingsProjection,
  ): HTMLElement => section("Permissions", [
    renderSelect({
      label: "Profile",
      selected: current.permissions.selectedProfile,
      options: current.permissions.profiles.map(profile => ({
        label: profile.description === null ? profile.id : `${profile.id} - ${profile.description}`,
        value: profile.id,
        disabled: !profile.allowed,
      })),
      onChange: value => void write("default_permissions", value),
    }),
    metric("Approval", current.config.approvalPolicy),
    metric("Reviewer", current.config.approvalsReviewer),
    metric("Sandbox", current.config.sandboxMode),
  ])

  const renderProviderSection = (
    current: KhalaCodeDesktopCodexSettingsProjection,
  ): HTMLElement => section("Provider", [
    metric("Namespace tools", current.providerCapabilities.namespaceTools),
    metric("Image generation", current.providerCapabilities.imageGeneration),
    metric("Web search", current.providerCapabilities.webSearch),
    metric("Config layers", current.config.layersAvailable),
  ])

  const renderCollaborationSection = (
    current: KhalaCodeDesktopCodexSettingsProjection,
  ): HTMLElement => section("Collaboration", [
    renderSelect({
      label: "Personality",
      selected: current.config.personality ?? "",
      options: [
        { label: "Unset", value: "" },
        { label: "None", value: "none" },
        { label: "Friendly", value: "friendly" },
        { label: "Pragmatic", value: "pragmatic" },
      ],
      onChange: value => void write("personality", value === "" ? null : value),
    }),
    metric("Mode", current.collaboration.currentMode),
    metric("Presets", current.collaboration.modes.map(mode => mode.name).join(", ") || null),
  ])

  const renderUsageSection = (
    current: KhalaCodeDesktopCodexSettingsProjection,
  ): HTMLElement => {
    const summary = current.usage.summary as Record<string, unknown> | null
    return section("Usage", [
      metric("Lifetime", summary?.lifetimeTokens),
      metric("Peak daily", summary?.peakDailyTokens),
      metric("Current streak", summary?.currentStreakDays),
      metric("Daily buckets", current.usage.dailyUsageBuckets?.length ?? null),
    ])
  }

  const renderRequirementsSection = (
    current: KhalaCodeDesktopCodexSettingsProjection,
  ): HTMLElement => {
    const blockerItems = current.requirements.blockers.length === 0
      ? [el("li", "khala-settings-blocker", "None")]
      : current.requirements.blockers.map(blocker => {
          const item = el("li", "khala-settings-blocker")
          item.textContent = blocker.message
          return item
        })
    const list = el("ul", "khala-settings-blocker-list")
    list.append(...blockerItems)
    return section("Requirements", [
      metric("Managed", current.requirements.managed),
      metric("Default permissions", current.requirements.defaultPermissions),
      metric("Allowed sandboxes", current.requirements.allowedSandboxModes?.join(", ") ?? null),
      list,
    ])
  }

  const renderEcosystemMetric = (
    summary: KhalaCodeDesktopCodexEcosystemSection,
  ): HTMLElement => {
    const node = el("div", "khala-settings-ecosystem-card")
    node.dataset.source = summary.source
    node.append(
      el("strong", "khala-settings-ecosystem-label", summary.label),
      el("span", "khala-settings-ecosystem-value", `${summary.readyCount}/${summary.count} ready`),
      el(
        "span",
        "khala-settings-ecosystem-detail",
        [
          summary.authRequiredCount > 0 ? `${summary.authRequiredCount} auth` : null,
          summary.disabledCount > 0 ? `${summary.disabledCount} disabled` : null,
          summary.managedCount > 0 ? `${summary.managedCount} managed` : null,
          summary.installRequiredCount > 0 ? `${summary.installRequiredCount} install` : null,
          summary.errorCount > 0 ? `${summary.errorCount} errors` : null,
          summary.unknownCount > 0 ? `${summary.unknownCount} unknown` : null,
        ].filter(Boolean).join(", ") || "healthy",
      ),
    )
    return node
  }

  const renderEcosystemSection = (
    current: KhalaCodeDesktopCodexEcosystemProjection | null,
  ): HTMLElement => {
    if (current === null) {
      return section("Codex Ecosystem", [
        el("p", "khala-settings-empty", "Ecosystem state has not been loaded yet."),
      ])
    }
    const grid = el("div", "khala-settings-ecosystem-grid")
    for (const summary of [
      current.sections.skills,
      current.sections.hooks,
      current.sections.plugins,
      current.sections.marketplace,
      current.sections.apps,
      current.sections.mcp,
      current.sections.khala,
    ]) {
      grid.append(renderEcosystemMetric(summary))
    }

    const diagnostics = el("ul", "khala-settings-diagnostic-list")
    const diagnosticItems = current.diagnostics.slice(0, 8)
    if (diagnosticItems.length === 0) {
      diagnostics.append(el("li", "khala-settings-diagnostic", "No Codex ecosystem diagnostics."))
    } else {
      for (const diagnostic of diagnosticItems) {
        const item = el("li", "khala-settings-diagnostic")
        item.dataset.severity = diagnostic.severity
        item.textContent = `${diagnostic.title}: ${diagnostic.detail}`
        diagnostics.append(item)
      }
    }

    return section("Codex Ecosystem", [
      metric("Snapshot", current.ok ? "ready" : "needs attention"),
      metric("Recent changes", current.notifications.length),
      grid,
      diagnostics,
    ])
  }

  function render(): void {
    container.replaceChildren()
    const header = el("header", "khala-settings-header")
    const title = el("div", "khala-settings-title-group")
    title.append(
      el("h2", "khala-settings-title", "Settings"),
      el("p", "khala-settings-subtitle", "Primary Codex app-server session"),
    )
    const refresh = el("button", "khala-settings-refresh", "Refresh")
    refresh.type = "button"
    refresh.disabled = loading
    refresh.addEventListener("click", () => void refreshSettings())
    header.append(title, refresh)
    container.append(header)

    if (loading && settings === null) {
      container.append(el("div", "khala-settings-empty", "Loading Codex settings"))
      return
    }
    if (settings === null) {
      container.append(el("div", "khala-settings-empty", "No Codex settings loaded"))
      return
    }

    if (status.length > 0 || settings.errors.length > 0) {
      const banner = el("div", settings.ok ? "khala-settings-status" : "khala-settings-status khala-settings-status--error")
      banner.textContent = status.length > 0 ? status : settings.errors.join("\n")
      container.append(banner)
    }

    container.append(
      renderBoundarySection(),
      renderModelSection(settings),
      renderPermissionsSection(settings),
      renderProviderSection(settings),
      renderCollaborationSection(settings),
      renderUsageSection(settings),
      renderRequirementsSection(settings),
      renderEcosystemSection(ecosystem),
    )
  }

  async function refreshSettings(): Promise<void> {
    loading = true
    render()
    try {
      const [nextSettings, nextEcosystem] = await Promise.all([
        options.fetch(),
        options.fetchEcosystem?.() ?? Promise.resolve(null),
      ])
      settings = nextSettings
      ecosystem = nextEcosystem
      status = [
        ...(settings.ok ? [] : settings.errors),
        ...(ecosystem === null || ecosystem.ok ? [] : ecosystem.errors),
      ].join("\n")
    } catch (error) {
      status = error instanceof Error ? error.message : String(error)
    } finally {
      loading = false
      render()
    }
  }

  render()

  return {
    refresh: refreshSettings,
    setVisible(nextVisible) {
      visible = nextVisible
      if (visible && settings === null && !loading) void refreshSettings()
    },
  }
}
