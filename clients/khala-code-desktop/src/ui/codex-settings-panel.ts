import type {
  KhalaCodeDesktopCodexConfigValueWriteRequest,
  KhalaCodeDesktopModelRoleRegistryReadResult,
  KhalaCodeDesktopModelRoleRegistryWriteRequest,
} from "../shared/rpc"
import type {
  KhalaCodeDesktopCodexSettingsProjection,
} from "../shared/codex-settings"
import type {
  KhalaCodeDesktopCodexEcosystemProjection,
  KhalaCodeDesktopCodexEcosystemSection,
} from "../shared/codex-ecosystem"
import {
  KHALA_CODE_MODEL_ROLE_ORDER,
  type KhalaCodeModelRoleEntry,
  type KhalaCodeModelRoleRegistry,
} from "../shared/model-roles"

export type CodexSettingsPanelHandle = {
  readonly refresh: () => Promise<void>
  readonly setVisible: (visible: boolean) => void
}

export type CodexSettingsPanelOptions = {
  readonly applyModelRolePreset?: (
    request: { readonly preset: "architect-coder-judge" },
  ) => Promise<{
    readonly ok: boolean
    readonly settings?: KhalaCodeDesktopCodexSettingsProjection
    readonly error?: string
  }>
  readonly fetch: () => Promise<KhalaCodeDesktopCodexSettingsProjection>
  readonly fetchEcosystem?: () => Promise<KhalaCodeDesktopCodexEcosystemProjection>
  readonly fetchModelRoles?: () => Promise<KhalaCodeDesktopModelRoleRegistryReadResult>
  readonly onRender?: () => void
  readonly writeModelRole?: (
    request: KhalaCodeDesktopModelRoleRegistryWriteRequest,
  ) => Promise<KhalaCodeDesktopModelRoleRegistryReadResult & { readonly saved?: boolean }>
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

/**
 * Read-only settings metrics never say the bare, unexplained word "Unset" —
 * a null/undefined value here means the field reflects a default, so it
 * says "Default" in plain language instead
 * (khala_code.settings.no_bare_unset_labels.v1).
 */
const compactValue = (value: unknown): string => {
  if (value === null || value === undefined) return "Default"
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

const stringOrNull = (value: unknown): string | null =>
  typeof value === "string" ? value : null

const controlNameFromLabel = (label: string): string =>
  label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")

const section = (title: string, children: readonly Node[]): HTMLElement => {
  const node = el("section", "khala-settings-section")
  node.append(el("h3", "khala-settings-section-title", title), ...children)
  return node
}

const renderSelect = (
  input: {
    readonly disabled?: boolean
    readonly label: string
    readonly name?: string
    readonly options: readonly SelectOption[]
    readonly selected: string | null
    readonly title?: string
    readonly onChange: (value: string) => void
  },
): HTMLElement => {
  const row = el("label", "khala-settings-control")
  const label = el("span", "khala-settings-control-label", input.label)
  const select = el("select", "khala-settings-select")
  select.name = input.name ?? controlNameFromLabel(input.label)
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

const renderTextInput = (
  input: {
    readonly disabled?: boolean
    readonly label: string
    readonly name?: string
    readonly selected: string | null
    readonly title?: string
    readonly onCommit: (value: string) => void
  },
): HTMLElement => {
  const row = el("label", "khala-settings-control")
  const label = el("span", "khala-settings-control-label", input.label)
  const text = el("input", "khala-settings-select")
  text.name = input.name ?? controlNameFromLabel(input.label)
  text.type = "text"
  text.disabled = input.disabled === true
  text.value = input.selected ?? ""
  if (input.title !== undefined) text.title = input.title
  text.addEventListener("keydown", event => {
    if (event.key === "Enter") input.onCommit(text.value.trim())
  })
  text.addEventListener("change", () => input.onCommit(text.value.trim()))
  row.append(label, text)
  return row
}

export const mountCodexSettingsPanel = (
  container: HTMLElement,
  options: CodexSettingsPanelOptions,
): CodexSettingsPanelHandle => {
  let settings: KhalaCodeDesktopCodexSettingsProjection | null = null
  let ecosystem: KhalaCodeDesktopCodexEcosystemProjection | null = null
  let modelRoles: KhalaCodeModelRoleRegistry | null = null
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

  const applyModelRolePreset = async (preset: "architect-coder-judge"): Promise<void> => {
    if (options.applyModelRolePreset === undefined) {
      setStatus("Model role preset writes are not configured.")
      return
    }
    setStatus(`Applying ${preset}`)
    const result = await options.applyModelRolePreset({ preset })
    if (result.ok) {
      if (result.settings !== undefined) settings = result.settings
      setStatus(`Applied ${preset}`)
      return
    }
    setStatus(result.error ?? `Failed to apply ${preset}`)
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
    const providerOptions = current.providers.options.map(provider => ({
      label: provider.modelCount > 1
        ? `${provider.displayName} (${provider.modelCount} models)`
        : provider.displayName,
      value: provider.id,
    }))
    const configuredProviderMissing = current.config.modelProvider !== null &&
      providerOptions.every(option => option.value !== current.config.modelProvider)
    const providerSelectOptions = providerOptions.length === 0
      ? [{
          disabled: true,
          label: compactValue(current.config.modelProvider),
          value: current.config.modelProvider ?? "",
        }]
      : [
          { label: "Default", value: "" },
          ...providerOptions,
          ...(configuredProviderMissing
            ? [{
                disabled: true,
                label: `${current.config.modelProvider} (not in model list)`,
                value: current.config.modelProvider ?? "",
              }]
            : []),
        ]
    return section("Model", [
      renderSelect({
        label: "Model",
        name: "model",
        selected: selectedModel?.id ?? current.config.model,
        options: current.models.options.filter(model => !model.hidden).map(model => ({
          label: model.displayName,
          value: model.id,
        })),
        onChange: value => void write("model", value),
      }),
      renderSelect({
        disabled: reasoningOptions.length === 0,
        label: "Reasoning",
        name: "model_reasoning_effort",
        selected: current.config.reasoningEffort ?? selectedModel?.defaultReasoningEffort ?? null,
        options: reasoningOptions,
        onChange: value => void write("model_reasoning_effort", value),
      }),
      renderSelect({
        disabled: serviceTierOptions.length <= 1,
        label: "Service tier",
        name: "service_tier",
        selected: current.config.serviceTier ?? "",
        options: serviceTierOptions,
        onChange: value => void write("service_tier", value === "" ? null : value),
      }),
      renderSelect({
        disabled: current.providers.options.length === 0,
        label: "Provider",
        name: "model_provider",
        selected: current.config.modelProvider ?? "",
        options: providerSelectOptions,
        ...(current.providers.options.length === 0
          ? { title: "Provider options are unavailable from the Codex model list." }
          : {}),
        onChange: value => void write("model_provider", value === "" ? null : value),
      }),
      renderSelect({
        label: "Summary",
        name: "model_reasoning_summary",
        selected: current.config.reasoningSummary ?? "",
        options: [
          { label: "Default", value: "" },
          { label: "Auto", value: "auto" },
          { label: "Concise", value: "concise" },
          { label: "Detailed", value: "detailed" },
        ],
        onChange: value => void write("model_reasoning_summary", value === "" ? null : value),
      }),
      renderSelect({
        label: "Verbosity",
        name: "model_verbosity",
        selected: current.config.verbosity ?? "",
        options: [
          { label: "Default", value: "" },
          { label: "Low", value: "low" },
          { label: "Medium", value: "medium" },
          { label: "High", value: "high" },
        ],
        onChange: value => void write("model_verbosity", value === "" ? null : value),
      }),
    ])
  }

  const updateModelRole = async (
    entry: KhalaCodeModelRoleEntry,
    patch: Partial<Pick<KhalaCodeModelRoleEntry, "effort" | "harness" | "model">>,
  ): Promise<void> => {
    if (options.writeModelRole === undefined) return
    setStatus(`Saving ${entry.role} role`)
    const nextEntry = {
      ...entry,
      ...patch,
      ...(patch.model === "" ? { model: undefined } : {}),
    }
    const result = await options.writeModelRole({ entry: nextEntry })
    modelRoles = result.registry
    setStatus(`Saved ${entry.role} role`)
  }

  const renderModelRoleRegistrySection = (
    registry: KhalaCodeModelRoleRegistry | null,
  ): HTMLElement => {
    if (registry === null) {
      return section("Model Roles", [
        el("p", "khala-settings-empty", "Model role registry has not been loaded yet."),
      ])
    }
    return section("Model Roles", KHALA_CODE_MODEL_ROLE_ORDER.flatMap(role => {
      const entry = registry.roles[role]
      return [
        metric(role, `${entry.harness}${entry.model === undefined ? "" : ` / ${entry.model}`}`),
        renderSelect({
          disabled: options.writeModelRole === undefined,
          label: `${role} harness`,
          selected: entry.harness,
          options: [
            { label: "Codex", value: "codex" },
            { label: "Claude", value: "claude" },
            { label: "Khala", value: "khala" },
          ],
          onChange: value => void updateModelRole(entry, {
            harness: value as KhalaCodeModelRoleEntry["harness"],
          }),
        }),
        renderTextInput({
          disabled: options.writeModelRole === undefined,
          label: `${role} model`,
          selected: entry.model ?? "",
          onCommit: value => void updateModelRole(entry, {
            model: value.length === 0 ? undefined : value,
          }),
        }),
        renderSelect({
          disabled: options.writeModelRole === undefined,
          label: `${role} effort`,
          selected: entry.effort ?? "medium",
          options: [
            { label: "Minimal", value: "minimal" },
            { label: "Low", value: "low" },
            { label: "Medium", value: "medium" },
            { label: "High", value: "high" },
            { label: "Xhigh", value: "xhigh" },
          ],
          onChange: value => void updateModelRole(entry, {
            effort: value as KhalaCodeModelRoleEntry["effort"],
          }),
        }),
      ]
    }))
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
    renderSelect({
      label: "Approval",
      selected: stringOrNull(current.config.approvalPolicy) ?? "",
      options: [
        { label: "Default", value: "" },
        { label: "Untrusted", value: "untrusted" },
        { label: "On failure", value: "on-failure" },
        { label: "On request", value: "on-request" },
        { label: "Never", value: "never" },
      ],
      onChange: value => void write("approval_policy", value === "" ? null : value),
    }),
    metric("Reviewer", current.config.approvalsReviewer),
    renderSelect({
      label: "Sandbox",
      selected: current.config.sandboxMode ?? "",
      options: [
        { label: "Default", value: "" },
        { label: "Read only", value: "read-only" },
        { label: "Workspace write", value: "workspace-write" },
        { label: "Danger full access", value: "danger-full-access" },
      ],
      onChange: value => void write("sandbox_mode", value === "" ? null : value),
    }),
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
    metric("Mode", current.collaboration.currentMode),
    metric("Personality", current.collaboration.personality),
    metric("Presets", current.collaboration.modes.map(mode => mode.name).join(", ") || null),
  ])

  const renderModelRolePresetSection = (
    current: KhalaCodeDesktopCodexSettingsProjection,
  ): HTMLElement => {
    const preset = current.modelRolePresets.presets[0]
    if (preset === undefined) {
      return section("Model Role Presets", [
        el("p", "khala-settings-empty", "No model role presets are available."),
      ])
    }
    const card = el("div", "khala-settings-preset-card")
    const title = el("div", "khala-settings-preset-title-row")
    title.append(
      el("strong", "khala-settings-preset-title", preset.title),
      el("span", "khala-settings-preset-state", preset.selected ? "Active" : "Ready"),
    )
    const body = el("p", "khala-settings-preset-description", preset.description)
    const roles = el("ul", "khala-settings-preset-role-list")
    for (const summary of preset.roleSummary) {
      roles.append(el("li", "khala-settings-preset-role", summary))
    }
    const apply = el("button", "khala-settings-refresh", preset.selected ? "Reapply" : "Apply")
    apply.type = "button"
    apply.disabled = loading || options.applyModelRolePreset === undefined
    apply.addEventListener("click", () => void applyModelRolePreset(preset.id))
    card.append(title, body, roles, apply)
    return section("Model Role Presets", [
      card,
      metric("Config key", preset.configKeyPath),
      metric("Promise", preset.promiseRef),
      metric("Proxy rails", preset.noProxyRails ? "none" : "blocked"),
      metric("Subscription resale", preset.noResale ? "blocked" : "unknown"),
      metric("Copy gate", preset.copyGate),
    ])
  }

  const renderAppearanceSection = (
    current: KhalaCodeDesktopCodexSettingsProjection,
  ): HTMLElement => section("Appearance", [
    renderSelect({
      label: "Vim default",
      selected: current.appearance.vimModeDefault === true ? "true" : "false",
      options: [
        { label: "Off", value: "false" },
        { label: "On", value: "true" },
      ],
      onChange: value => void write(current.appearance.keyPaths.vimModeDefault, value === "true"),
    }),
    renderTextInput({
      label: "Statusline",
      selected: current.appearance.statusLine?.join(", ") ?? "",
      title: "Comma-separated Codex status line item ids",
      onCommit: value => void write(
        current.appearance.keyPaths.statusLine,
        value.length === 0 ? null : value.split(",").map(item => item.trim()).filter(Boolean),
      ),
    }),
    renderSelect({
      label: "Statusline colors",
      selected: current.appearance.statusLineUseColors === false ? "false" : "true",
      options: [
        { label: "On", value: "true" },
        { label: "Off", value: "false" },
      ],
      onChange: value => void write(current.appearance.keyPaths.statusLineUseColors, value === "true"),
    }),
    renderTextInput({
      label: "Theme",
      selected: current.appearance.theme ?? "",
      onCommit: value => void write(current.appearance.keyPaths.theme, value.length === 0 ? null : value),
    }),
    renderTextInput({
      label: "Pet",
      selected: current.appearance.pet ?? "",
      onCommit: value => void write(current.appearance.keyPaths.pet, value.length === 0 ? null : value),
    }),
    renderSelect({
      label: "Pet anchor",
      selected: current.appearance.petAnchor ?? "composer",
      options: [
        { label: "Composer", value: "composer" },
        { label: "Screen bottom", value: "screen-bottom" },
      ],
      onChange: value => void write(current.appearance.keyPaths.petAnchor, value),
    }),
    renderSelect({
      label: "Personality",
      selected: current.appearance.personality ?? "",
      options: [
        { label: "Unset", value: "" },
        { label: "None", value: "none" },
        { label: "Friendly", value: "friendly" },
        { label: "Pragmatic", value: "pragmatic" },
      ],
      onChange: value => void write(current.appearance.keyPaths.personality, value === "" ? null : value),
    }),
    metric("Keymap", current.appearance.keymap),
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
      current.sections.imports,
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
      options.onRender?.()
      return
    }
    if (settings === null) {
      container.append(el("div", "khala-settings-empty", "No Codex settings loaded"))
      options.onRender?.()
      return
    }

    if (status.length > 0 || settings.errors.length > 0) {
      const banner = el("div", settings.ok ? "khala-settings-status" : "khala-settings-status khala-settings-status--error")
      banner.textContent = status.length > 0 ? status : settings.errors.join("\n")
      container.append(banner)
    }

    container.append(
      renderBoundarySection(),
      renderModelRoleRegistrySection(modelRoles),
      renderModelSection(settings),
      renderPermissionsSection(settings),
      renderAppearanceSection(settings),
      renderProviderSection(settings),
      renderCollaborationSection(settings),
      renderModelRolePresetSection(settings),
      renderUsageSection(settings),
      renderRequirementsSection(settings),
      ...(options.fetchEcosystem === undefined ? [] : [renderEcosystemSection(ecosystem)]),
    )
    options.onRender?.()
  }

  // Three independent IPC fetches (settings, ecosystem, model roles). A bare
  // `Promise.all` here means one fetch rejecting hides the OTHER fetches'
  // already-succeeded data behind one generic error banner, leaving the
  // panel showing stale data for groups that actually refreshed fine. Each
  // fetch is isolated via `Promise.allSettled` so a failing group only
  // scopes its own error, and every successfully-fetched group still merges
  // into fresh panel state.
  async function refreshSettings(): Promise<void> {
    loading = true
    render()
    const describeError = (reason: unknown): string =>
      reason instanceof Error ? reason.message : String(reason)
    const failures: string[] = []

    const [settingsResult, ecosystemResult, modelRolesResult] = await Promise.allSettled([
      options.fetch(),
      options.fetchEcosystem?.() ?? Promise.resolve(null),
      options.fetchModelRoles?.() ?? Promise.resolve(null),
    ])

    if (settingsResult.status === "fulfilled") {
      settings = settingsResult.value
      if (!settingsResult.value.ok) failures.push(...settingsResult.value.errors)
    } else {
      failures.push(`Settings refresh failed: ${describeError(settingsResult.reason)}`)
    }

    if (ecosystemResult.status === "fulfilled") {
      if (ecosystemResult.value !== null) {
        ecosystem = ecosystemResult.value
        if (!ecosystemResult.value.ok) failures.push(...ecosystemResult.value.errors)
      }
    } else {
      failures.push(`Ecosystem refresh failed: ${describeError(ecosystemResult.reason)}`)
    }

    if (modelRolesResult.status === "fulfilled") {
      if (modelRolesResult.value !== null) modelRoles = modelRolesResult.value.registry
    } else {
      failures.push(`Model roles refresh failed: ${describeError(modelRolesResult.reason)}`)
    }

    status = failures.join("\n")
    loading = false
    render()
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
