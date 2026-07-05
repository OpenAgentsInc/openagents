export type KhalaCodeDesktopCodexEcosystemSource =
  | "apps"
  | "hooks"
  | "imports"
  | "khala"
  | "marketplace"
  | "mcp"
  | "plugins"
  | "skills"

export type KhalaCodeDesktopCodexEcosystemState =
  | "auth_required"
  | "desktop_extension"
  | "disabled"
  | "disabled_by_admin"
  | "error"
  | "install_required"
  | "managed"
  | "ready"
  | "unknown"

export type KhalaCodeDesktopCodexEcosystemSeverity =
  | "critical"
  | "info"
  | "warning"

export type KhalaCodeDesktopCodexEcosystemItem = Readonly<{
  id: string
  name: string
  source: KhalaCodeDesktopCodexEcosystemSource
  state: KhalaCodeDesktopCodexEcosystemState
  detail: string
  authRequired: boolean
  enabled: boolean | null
  installed: boolean | null
  managed: boolean
  marketplaceName?: string | undefined
  pluginId?: string | undefined
}>

export type KhalaCodeDesktopCodexEcosystemSection = Readonly<{
  source: KhalaCodeDesktopCodexEcosystemSource
  label: string
  count: number
  readyCount: number
  disabledCount: number
  managedCount: number
  authRequiredCount: number
  installRequiredCount: number
  errorCount: number
  unknownCount: number
  items: readonly KhalaCodeDesktopCodexEcosystemItem[]
}>

export type KhalaCodeDesktopCodexEcosystemDiagnostic = Readonly<{
  ref: string
  source: KhalaCodeDesktopCodexEcosystemSource
  severity: KhalaCodeDesktopCodexEcosystemSeverity
  title: string
  detail: string
  action: "authenticate" | "install" | "open_settings" | "refresh" | "review"
  itemId?: string | undefined
  observedAt: string
}>

export type KhalaCodeDesktopCodexEcosystemNotification = Readonly<{
  method: string
  receivedAt: string
  summary: string
  severity: KhalaCodeDesktopCodexEcosystemSeverity
}>

export type KhalaCodeDesktopCodexEcosystemProjection = Readonly<{
  ok: boolean
  cwd: string | null
  observedAt: string
  errors: readonly string[]
  notifications: readonly KhalaCodeDesktopCodexEcosystemNotification[]
  sections: Readonly<{
    apps: KhalaCodeDesktopCodexEcosystemSection
    hooks: KhalaCodeDesktopCodexEcosystemSection
    imports: KhalaCodeDesktopCodexEcosystemSection
    khala: KhalaCodeDesktopCodexEcosystemSection
    marketplace: KhalaCodeDesktopCodexEcosystemSection
    mcp: KhalaCodeDesktopCodexEcosystemSection
    plugins: KhalaCodeDesktopCodexEcosystemSection
    skills: KhalaCodeDesktopCodexEcosystemSection
  }>
  diagnostics: readonly KhalaCodeDesktopCodexEcosystemDiagnostic[]
}>

export type KhalaCodeDesktopCodexEcosystemRawNotification = Readonly<{
  method: string
  params?: unknown
  receivedAt: string
}>

export type ProjectKhalaCodeDesktopCodexEcosystemInput = Readonly<{
  appsList?: unknown
  cwd?: string
  errors?: readonly string[]
  externalAgentConfigDetect?: unknown
  externalAgentConfigImportHistories?: unknown
  hooksList?: unknown
  mcpServerStatusList?: unknown
  notifications?: readonly KhalaCodeDesktopCodexEcosystemRawNotification[]
  observedAt?: string
  pluginInstalled?: unknown
  pluginList?: unknown
  skillsList?: unknown
}>

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const arrayField = (
  value: unknown,
  key: string,
): readonly unknown[] => {
  if (!isRecord(value)) return []
  const field = value[key]
  return Array.isArray(field) ? field : []
}

const stringField = (
  value: unknown,
  key: string,
): string | null => {
  if (!isRecord(value)) return null
  const field = value[key]
  return typeof field === "string" && field.trim().length > 0 ? field : null
}

const booleanField = (
  value: unknown,
  key: string,
): boolean | null => {
  if (!isRecord(value)) return null
  const field = value[key]
  return typeof field === "boolean" ? field : null
}

const objectField = (
  value: unknown,
  key: string,
): Record<string, unknown> | null => {
  if (!isRecord(value)) return null
  const field = value[key]
  return isRecord(field) ? field : null
}

const safeIdPart = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_.:-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 120) || "unknown"

const safeLabel = (
  value: string | null,
  fallback: string,
): string => value ?? fallback

const limitedDetail = (value: string): string =>
  value.length <= 220 ? value : `${value.slice(0, 217)}...`

const countObjectKeys = (value: unknown): number =>
  isRecord(value) ? Object.keys(value).length : 0

const numberField = (
  value: unknown,
  key: string,
): number | null => {
  if (!isRecord(value)) return null
  const field = value[key]
  return typeof field === "number" && Number.isFinite(field) ? field : null
}

const uniqueById = (
  items: readonly KhalaCodeDesktopCodexEcosystemItem[],
): KhalaCodeDesktopCodexEcosystemItem[] => {
  const seen = new Set<string>()
  const result: KhalaCodeDesktopCodexEcosystemItem[] = []
  for (const item of items) {
    if (seen.has(item.id)) continue
    seen.add(item.id)
    result.push(item)
  }
  return result
}

const uniqueDiagnosticsByRef = (
  diagnostics: readonly KhalaCodeDesktopCodexEcosystemDiagnostic[],
): KhalaCodeDesktopCodexEcosystemDiagnostic[] => {
  const seen = new Set<string>()
  const result: KhalaCodeDesktopCodexEcosystemDiagnostic[] = []
  for (const diagnostic of diagnostics) {
    const key = `${diagnostic.ref}:${diagnostic.title}:${diagnostic.detail}`
    if (seen.has(key)) continue
    seen.add(key)
    result.push(diagnostic)
  }
  return result
}

const section = (
  source: KhalaCodeDesktopCodexEcosystemSource,
  label: string,
  items: readonly KhalaCodeDesktopCodexEcosystemItem[],
): KhalaCodeDesktopCodexEcosystemSection => ({
  source,
  label,
  count: items.length,
  readyCount: items.filter(item => item.state === "ready").length,
  disabledCount: items.filter(item =>
    item.state === "disabled" || item.state === "disabled_by_admin"
  ).length,
  managedCount: items.filter(item => item.managed || item.state === "managed").length,
  authRequiredCount: items.filter(item => item.authRequired || item.state === "auth_required").length,
  installRequiredCount: items.filter(item => item.state === "install_required").length,
  errorCount: items.filter(item => item.state === "error").length,
  unknownCount: items.filter(item => item.state === "unknown").length,
  items,
})

const itemDiagnostic = (
  input: {
    readonly action: KhalaCodeDesktopCodexEcosystemDiagnostic["action"]
    readonly detail: string
    readonly item: KhalaCodeDesktopCodexEcosystemItem
    readonly observedAt: string
    readonly severity: KhalaCodeDesktopCodexEcosystemSeverity
    readonly title: string
  },
): KhalaCodeDesktopCodexEcosystemDiagnostic => ({
  ref: `codex_ecosystem.${input.item.source}.${input.item.state}.${safeIdPart(input.item.id)}`,
  source: input.item.source,
  severity: input.severity,
  title: input.title,
  detail: input.detail,
  action: input.action,
  itemId: input.item.id,
  observedAt: input.observedAt,
})

const parseErrorDiagnostics = (
  source: KhalaCodeDesktopCodexEcosystemSource,
  label: string,
  errors: readonly unknown[],
  observedAt: string,
): KhalaCodeDesktopCodexEcosystemDiagnostic[] =>
  errors.map((error, index) => {
    const message = stringField(error, "message") ?? String(error)
    const path = stringField(error, "path") ?? stringField(error, "marketplacePath")
    return {
      ref: `codex_ecosystem.${source}.error.${index}.${safeIdPart(message)}`,
      source,
      severity: "warning" as const,
      title: `${label} load warning`,
      detail: limitedDetail(path === null ? message : `${path}: ${message}`),
      action: "review" as const,
      observedAt,
    }
  })

const parseSkills = (
  skillsList: unknown,
  observedAt: string,
): {
  readonly diagnostics: readonly KhalaCodeDesktopCodexEcosystemDiagnostic[]
  readonly items: readonly KhalaCodeDesktopCodexEcosystemItem[]
} => {
  const diagnostics: KhalaCodeDesktopCodexEcosystemDiagnostic[] = []
  const items: KhalaCodeDesktopCodexEcosystemItem[] = []
  for (const entry of arrayField(skillsList, "data")) {
    diagnostics.push(
      ...parseErrorDiagnostics("skills", "Skill", arrayField(entry, "errors"), observedAt),
    )
    for (const skill of arrayField(entry, "skills")) {
      const name = safeLabel(stringField(skill, "name"), "Unknown skill")
      const scope = safeLabel(stringField(skill, "scope"), "unknown")
      const path = stringField(skill, "path")
      const enabled = booleanField(skill, "enabled")
      const managed = scope === "admin" || scope === "system"
      const state: KhalaCodeDesktopCodexEcosystemState =
        enabled === false
          ? managed ? "managed" : "disabled"
          : enabled === true
            ? "ready"
            : "unknown"
      const item: KhalaCodeDesktopCodexEcosystemItem = {
        id: `skill:${scope}:${path ?? name}`,
        name,
        source: "skills",
        state,
        detail: limitedDetail(`${scope}${path === null ? "" : ` - ${path}`}`),
        authRequired: false,
        enabled,
        installed: null,
        managed,
      }
      items.push(item)
      if (state === "disabled" || state === "managed") {
        diagnostics.push(itemDiagnostic({
          action: "open_settings",
          detail: `${name} is ${state === "managed" ? "managed by Codex policy" : "disabled"}.`,
          item,
          observedAt,
          severity: state === "managed" ? "info" : "warning",
          title: `${name} skill is ${state === "managed" ? "managed" : "disabled"}`,
        }))
      }
      if (state === "unknown") {
        diagnostics.push(itemDiagnostic({
          action: "review",
          detail: `${name} returned an unknown enabled state from Codex app-server.`,
          item,
          observedAt,
          severity: "warning",
          title: "Unknown skill state",
        }))
      }
    }
  }
  return { diagnostics, items }
}

const parseHooks = (
  hooksList: unknown,
  observedAt: string,
): {
  readonly diagnostics: readonly KhalaCodeDesktopCodexEcosystemDiagnostic[]
  readonly items: readonly KhalaCodeDesktopCodexEcosystemItem[]
} => {
  const diagnostics: KhalaCodeDesktopCodexEcosystemDiagnostic[] = []
  const items: KhalaCodeDesktopCodexEcosystemItem[] = []
  for (const entry of arrayField(hooksList, "data")) {
    diagnostics.push(
      ...parseErrorDiagnostics("hooks", "Hook", arrayField(entry, "errors"), observedAt),
    )
    for (const warning of arrayField(entry, "warnings")) {
      diagnostics.push({
        ref: `codex_ecosystem.hooks.warning.${safeIdPart(String(warning))}`,
        source: "hooks",
        severity: "warning",
        title: "Hook warning",
        detail: limitedDetail(String(warning)),
        action: "review",
        observedAt,
      })
    }
    for (const hook of arrayField(entry, "hooks")) {
      const key = safeLabel(stringField(hook, "key"), "unknown-hook")
      const eventName = safeLabel(stringField(hook, "eventName"), "unknown")
      const source = safeLabel(stringField(hook, "source"), "unknown")
      const trustStatus = stringField(hook, "trustStatus")
      const enabled = booleanField(hook, "enabled")
      const managed = booleanField(hook, "isManaged") === true || trustStatus === "managed"
      const state: KhalaCodeDesktopCodexEcosystemState =
        enabled === false
          ? managed ? "managed" : "disabled"
          : trustStatus === "modified" || trustStatus === "untrusted"
            ? "error"
            : enabled === true
              ? managed ? "managed" : "ready"
              : "unknown"
      const item: KhalaCodeDesktopCodexEcosystemItem = {
        id: `hook:${key}`,
        name: key,
        source: "hooks",
        state,
        detail: limitedDetail(`${eventName} - ${source} - trust ${trustStatus ?? "unknown"}`),
        authRequired: false,
        enabled,
        installed: null,
        managed,
        ...(stringField(hook, "pluginId") === null ? {} : { pluginId: stringField(hook, "pluginId")! }),
      }
      items.push(item)
      if (state === "disabled" || state === "managed" || state === "error" || state === "unknown") {
        diagnostics.push(itemDiagnostic({
          action: "review",
          detail: `${key} hook state is ${state}.`,
          item,
          observedAt,
          severity: state === "error" || state === "unknown" ? "warning" : "info",
          title: `${key} hook needs review`,
        }))
      }
    }
  }
  return { diagnostics, items }
}

const parsePluginMarketplaces = (
  pluginList: unknown,
  pluginInstalled: unknown,
  observedAt: string,
): {
  readonly diagnostics: readonly KhalaCodeDesktopCodexEcosystemDiagnostic[]
  readonly marketplaceItems: readonly KhalaCodeDesktopCodexEcosystemItem[]
  readonly pluginItems: readonly KhalaCodeDesktopCodexEcosystemItem[]
} => {
  const diagnostics: KhalaCodeDesktopCodexEcosystemDiagnostic[] = []
  const marketplaceItems: KhalaCodeDesktopCodexEcosystemItem[] = []
  const pluginItems: KhalaCodeDesktopCodexEcosystemItem[] = []
  const marketplaceResponses = [pluginList, pluginInstalled]

  for (const response of marketplaceResponses) {
    diagnostics.push(
      ...parseErrorDiagnostics(
        "marketplace",
        "Marketplace",
        arrayField(response, "marketplaceLoadErrors"),
        observedAt,
      ),
    )
    for (const marketplace of arrayField(response, "marketplaces")) {
      const marketplaceName = safeLabel(stringField(marketplace, "name"), "unknown-marketplace")
      const marketplacePath = stringField(marketplace, "path")
      const marketplaceItem: KhalaCodeDesktopCodexEcosystemItem = {
        id: `marketplace:${marketplaceName}:${marketplacePath ?? "remote"}`,
        name: marketplaceName,
        source: "marketplace",
        state: "ready",
        detail: marketplacePath ?? "remote marketplace",
        authRequired: false,
        enabled: true,
        installed: true,
        managed: false,
        marketplaceName,
      }
      marketplaceItems.push(marketplaceItem)

      for (const plugin of arrayField(marketplace, "plugins")) {
        const name = safeLabel(stringField(plugin, "name"), "Unknown plugin")
        const pluginId = stringField(plugin, "id") ?? `plugin:${marketplaceName}:${name}`
        const installed = booleanField(plugin, "installed")
        const enabled = booleanField(plugin, "enabled")
        const installPolicy = stringField(plugin, "installPolicy")
        const authPolicy = stringField(plugin, "authPolicy")
        const availability = stringField(plugin, "availability")
        const authRequired =
          availability !== "DISABLED_BY_ADMIN" &&
          (
            authPolicy === "ON_INSTALL" && installed !== true ||
            authPolicy === "ON_USE" && installed === true
          )
        const state: KhalaCodeDesktopCodexEcosystemState =
          availability === "DISABLED_BY_ADMIN"
            ? "disabled_by_admin"
            : enabled === false
              ? "disabled"
              : installed === false && installPolicy === "AVAILABLE"
                  ? "install_required"
                  : authRequired
                    ? "auth_required"
                  : availability !== null && availability !== "AVAILABLE"
                    ? "unknown"
                    : enabled === true || installed === true
                      ? "ready"
                      : "unknown"
        const item: KhalaCodeDesktopCodexEcosystemItem = {
          id: pluginId,
          name,
          source: "plugins",
          state,
          detail: limitedDetail(`marketplace ${marketplaceName}; install ${installPolicy ?? "unknown"}; auth ${authPolicy ?? "unknown"}`),
          authRequired,
          enabled,
          installed,
          managed: availability === "DISABLED_BY_ADMIN",
          marketplaceName,
          pluginId,
        }
        pluginItems.push(item)
        if (state === "disabled_by_admin") {
          diagnostics.push(itemDiagnostic({
            action: "review",
            detail: `${name} is disabled by Codex admin policy.`,
            item,
            observedAt,
            severity: "warning",
            title: `${name} disabled by admin`,
          }))
        } else if (state === "auth_required") {
          diagnostics.push(itemDiagnostic({
            action: "authenticate",
            detail: `${name} requires Codex plugin authentication policy ${authPolicy ?? "unknown"}.`,
            item,
            observedAt,
            severity: "warning",
            title: `${name} plugin needs authentication`,
          }))
        } else if (state === "install_required") {
          diagnostics.push(itemDiagnostic({
            action: "install",
            detail: `${name} is available through ${marketplaceName} but is not installed.`,
            item,
            observedAt,
            severity: "info",
            title: `${name} plugin can be installed`,
          }))
        } else if (state === "unknown") {
          diagnostics.push(itemDiagnostic({
            action: "review",
            detail: `${name} returned an unknown availability state from Codex app-server.`,
            item,
            observedAt,
            severity: "warning",
            title: "Unknown plugin state",
          }))
        }
      }
    }
  }

  return {
    diagnostics,
    marketplaceItems: uniqueById(marketplaceItems),
    pluginItems: uniqueById(pluginItems),
  }
}

const parseApps = (
  appsList: unknown,
  observedAt: string,
): {
  readonly diagnostics: readonly KhalaCodeDesktopCodexEcosystemDiagnostic[]
  readonly items: readonly KhalaCodeDesktopCodexEcosystemItem[]
} => {
  const diagnostics: KhalaCodeDesktopCodexEcosystemDiagnostic[] = []
  const items: KhalaCodeDesktopCodexEcosystemItem[] = []
  for (const app of arrayField(appsList, "data")) {
    const id = safeLabel(stringField(app, "id"), "unknown-app")
    const name = safeLabel(stringField(app, "name"), id)
    const accessible = booleanField(app, "isAccessible")
    const enabled = booleanField(app, "isEnabled")
    const pluginNames = arrayField(app, "pluginDisplayNames")
      .map(value => String(value))
      .filter(value => value.trim().length > 0)
    const state: KhalaCodeDesktopCodexEcosystemState =
      accessible === false
        ? "auth_required"
        : enabled === false
          ? "disabled"
          : accessible === true && enabled === true
            ? "ready"
            : "unknown"
    const item: KhalaCodeDesktopCodexEcosystemItem = {
      id: `app:${id}`,
      name,
      source: "apps",
      state,
      detail: limitedDetail(pluginNames.length === 0 ? "Codex app connector" : `plugins ${pluginNames.join(", ")}`),
      authRequired: accessible === false,
      enabled,
      installed: null,
      managed: false,
    }
    items.push(item)
    if (state === "auth_required" || state === "disabled" || state === "unknown") {
      diagnostics.push(itemDiagnostic({
        action: state === "auth_required" ? "authenticate" : "open_settings",
        detail: `${name} connector state is ${state}.`,
        item,
        observedAt,
        severity: state === "unknown" ? "warning" : "info",
        title: `${name} app connector needs attention`,
      }))
    }
  }
  return { diagnostics, items }
}

const parseMcp = (
  mcpServerStatusList: unknown,
  notifications: readonly KhalaCodeDesktopCodexEcosystemRawNotification[],
  observedAt: string,
): {
  readonly diagnostics: readonly KhalaCodeDesktopCodexEcosystemDiagnostic[]
  readonly items: readonly KhalaCodeDesktopCodexEcosystemItem[]
} => {
  const diagnostics: KhalaCodeDesktopCodexEcosystemDiagnostic[] = []
  const items: KhalaCodeDesktopCodexEcosystemItem[] = []
  for (const server of arrayField(mcpServerStatusList, "data")) {
    const name = safeLabel(stringField(server, "name"), "unknown-mcp-server")
    const authStatus = stringField(server, "authStatus")
    const tools = countObjectKeys(objectField(server, "tools") ?? {})
    const resources = arrayField(server, "resources").length
    const templates = arrayField(server, "resourceTemplates").length
    const state: KhalaCodeDesktopCodexEcosystemState =
      authStatus === "notLoggedIn"
        ? "auth_required"
        : authStatus === "unsupported" || authStatus === "bearerToken" || authStatus === "oAuth" || authStatus === "notRequired"
          ? "ready"
          : "unknown"
    const item: KhalaCodeDesktopCodexEcosystemItem = {
      id: `mcp:${name}`,
      name,
      source: "mcp",
      state,
      detail: `${tools} tools, ${resources} resources, ${templates} templates; auth ${authStatus ?? "unknown"}`,
      authRequired: authStatus === "notLoggedIn",
      enabled: true,
      installed: true,
      managed: false,
    }
    items.push(item)
    if (state === "auth_required" || state === "unknown") {
      diagnostics.push(itemDiagnostic({
        action: state === "auth_required" ? "authenticate" : "review",
        detail: `${name} MCP auth status is ${authStatus ?? "unknown"}.`,
        item,
        observedAt,
        severity: "warning",
        title: `${name} MCP server needs authentication`,
      }))
    }
  }

  for (const notification of notifications) {
    if (notification.method !== "mcpServer/startupStatus/updated" && notification.method !== "mcpServer/oauthLogin/completed") {
      continue
    }
    const params = notification.params
    const name = safeLabel(stringField(params, "name"), "unknown-mcp-server")
    const success = booleanField(params, "success")
    const status = stringField(params, "status")
    const error = stringField(params, "error")
    const failureReason = stringField(params, "failureReason")
    const failed = success === false || status === "failed" || failureReason !== null || error !== null
    if (!failed) continue
    const item: KhalaCodeDesktopCodexEcosystemItem = {
      id: `mcp:${name}`,
      name,
      source: "mcp",
      state: failureReason === "reauthenticationRequired" ? "auth_required" : "error",
      detail: limitedDetail(error ?? failureReason ?? status ?? "MCP server failed"),
      authRequired: failureReason === "reauthenticationRequired",
      enabled: true,
      installed: true,
      managed: false,
    }
    diagnostics.push(itemDiagnostic({
      action: item.authRequired ? "authenticate" : "review",
      detail: `${name}: ${item.detail}`,
      item,
      observedAt: notification.receivedAt,
      severity: "warning",
      title: item.authRequired ? `${name} MCP server needs login` : `${name} MCP server failed`,
    }))
  }

  return { diagnostics, items: uniqueById(items) }
}

const countImportResultItems = (value: unknown, key: string): number =>
  arrayField(value, key).length

const parseExternalImports = (
  detectResponse: unknown,
  historiesResponse: unknown,
  notifications: readonly KhalaCodeDesktopCodexEcosystemRawNotification[],
  observedAt: string,
): {
  readonly diagnostics: readonly KhalaCodeDesktopCodexEcosystemDiagnostic[]
  readonly items: readonly KhalaCodeDesktopCodexEcosystemItem[]
} => {
  const diagnostics: KhalaCodeDesktopCodexEcosystemDiagnostic[] = []
  const items: KhalaCodeDesktopCodexEcosystemItem[] = []
  for (const item of arrayField(detectResponse, "items")) {
    const itemType = safeLabel(stringField(item, "itemType"), "UNKNOWN")
    const description = safeLabel(stringField(item, "description"), itemType)
    const cwd = stringField(item, "cwd")
    const scope = cwd === null ? "home" : "workspace"
    const projected: KhalaCodeDesktopCodexEcosystemItem = {
      id: `import:detect:${itemType}:${scope}:${safeIdPart(description)}`,
      name: itemType,
      source: "imports",
      state: "install_required",
      detail: limitedDetail(`${description} (${scope})`),
      authRequired: false,
      enabled: true,
      installed: false,
      managed: false,
    }
    items.push(projected)
    diagnostics.push(itemDiagnostic({
      action: "review",
      detail: `${description} is importable through Codex externalAgentConfig/import.`,
      item: projected,
      observedAt,
      severity: "info",
      title: `${itemType} config can be imported`,
    }))
  }

  for (const history of arrayField(historiesResponse, "data")) {
    const importId = safeLabel(stringField(history, "importId"), "unknown-import")
    const successes = countImportResultItems(history, "successes")
    const failures = countImportResultItems(history, "failures")
    const completedAtMs = numberField(history, "completedAtMs")
    const projected: KhalaCodeDesktopCodexEcosystemItem = {
      id: `import:history:${importId}`,
      name: importId,
      source: "imports",
      state: failures > 0 ? "error" : "ready",
      detail: `${successes} imported, ${failures} failed${completedAtMs === null ? "" : ` at ${completedAtMs}`}`,
      authRequired: false,
      enabled: true,
      installed: true,
      managed: false,
    }
    items.push(projected)
    if (failures > 0) {
      diagnostics.push(itemDiagnostic({
        action: "review",
        detail: `Codex import ${importId} completed with ${failures} failed item(s).`,
        item: projected,
        observedAt,
        severity: "warning",
        title: "Codex import needs review",
      }))
    }
  }

  for (const notification of notifications) {
    if (
      notification.method !== "externalAgentConfig/import/progress" &&
      notification.method !== "externalAgentConfig/import/completed"
    ) {
      continue
    }
    const importId = safeLabel(stringField(notification.params, "importId"), "unknown-import")
    const resultCount = arrayField(notification.params, "itemTypeResults").length
    const projected: KhalaCodeDesktopCodexEcosystemItem = {
      id: `import:notification:${importId}`,
      name: importId,
      source: "imports",
      state: notification.method.endsWith("/completed") ? "ready" : "managed",
      detail: `${resultCount} import result group(s) reported by Codex.`,
      authRequired: false,
      enabled: true,
      installed: true,
      managed: notification.method.endsWith("/progress"),
    }
    items.push(projected)
  }

  return { diagnostics, items: uniqueById(items) }
}

const khalaExtensionItems = (): readonly KhalaCodeDesktopCodexEcosystemItem[] => [
  {
    id: "khala:swarm",
    name: "Khala swarm",
    source: "khala",
    state: "desktop_extension",
    detail: "Khala-only swarm orchestration, separate from Codex default connectors.",
    authRequired: false,
    enabled: true,
    installed: true,
    managed: false,
  },
  {
    id: "khala:fleet",
    name: "Khala Codex fleet",
    source: "khala",
    state: "desktop_extension",
    detail: "Desktop/Pylon fleet helpers kept separate from Codex app and MCP state.",
    authRequired: false,
    enabled: true,
    installed: true,
    managed: false,
  },
]

const notificationSummary = (
  notification: KhalaCodeDesktopCodexEcosystemRawNotification,
): KhalaCodeDesktopCodexEcosystemNotification | null => {
  if (notification.method === "skills/changed") {
    return {
      method: notification.method,
      receivedAt: notification.receivedAt,
      severity: "info",
      summary: "Codex skill files changed; refresh skills/list for the current workspace.",
    }
  }
  if (notification.method === "app/list/updated") {
    const count = arrayField(notification.params, "data").length
    return {
      method: notification.method,
      receivedAt: notification.receivedAt,
      severity: "info",
      summary: `Codex app connector list changed${count > 0 ? ` (${count} apps)` : ""}.`,
    }
  }
  if (notification.method === "mcpServer/startupStatus/updated") {
    const name = safeLabel(stringField(notification.params, "name"), "unknown MCP server")
    const status = safeLabel(stringField(notification.params, "status"), "unknown")
    return {
      method: notification.method,
      receivedAt: notification.receivedAt,
      severity: status === "failed" ? "warning" : "info",
      summary: `${name} MCP startup status is ${status}.`,
    }
  }
  if (notification.method === "mcpServer/oauthLogin/completed") {
    const name = safeLabel(stringField(notification.params, "name"), "unknown MCP server")
    const success = booleanField(notification.params, "success")
    return {
      method: notification.method,
      receivedAt: notification.receivedAt,
      severity: success === false ? "warning" : "info",
      summary: `${name} MCP OAuth login ${success === false ? "failed" : "completed"}.`,
    }
  }
  if (
    notification.method === "externalAgentConfig/import/progress" ||
    notification.method === "externalAgentConfig/import/completed"
  ) {
    const importId = safeLabel(stringField(notification.params, "importId"), "unknown import")
    const resultCount = arrayField(notification.params, "itemTypeResults").length
    return {
      method: notification.method,
      receivedAt: notification.receivedAt,
      severity: "info",
      summary: `Codex external config import ${importId} ${notification.method.endsWith("/completed") ? "completed" : "progressed"} with ${resultCount} result group(s).`,
    }
  }
  return null
}

const notificationDiagnostics = (
  notifications: readonly KhalaCodeDesktopCodexEcosystemNotification[],
): readonly KhalaCodeDesktopCodexEcosystemDiagnostic[] =>
  notifications
    .filter(notification => notification.method === "skills/changed")
    .map(notification => ({
      ref: `codex_ecosystem.skills.changed.${safeIdPart(notification.receivedAt)}`,
      source: "skills" as const,
      severity: "info" as const,
      title: "Codex skills changed",
      detail: notification.summary,
      action: "refresh" as const,
      observedAt: notification.receivedAt,
    }))

export const projectKhalaCodeDesktopCodexEcosystem = (
  input: ProjectKhalaCodeDesktopCodexEcosystemInput,
): KhalaCodeDesktopCodexEcosystemProjection => {
  const observedAt = input.observedAt ?? new Date().toISOString()
  const sourceNotifications = input.notifications ?? []
  const notifications = sourceNotifications
    .map(notificationSummary)
    .filter((notification): notification is KhalaCodeDesktopCodexEcosystemNotification =>
      notification !== null
    )
  const skills = parseSkills(input.skillsList, observedAt)
  const hooks = parseHooks(input.hooksList, observedAt)
  const imports = parseExternalImports(
    input.externalAgentConfigDetect,
    input.externalAgentConfigImportHistories,
    sourceNotifications,
    observedAt,
  )
  const plugins = parsePluginMarketplaces(input.pluginList, input.pluginInstalled, observedAt)
  const apps = parseApps(input.appsList, observedAt)
  const mcp = parseMcp(input.mcpServerStatusList, sourceNotifications, observedAt)
  const khalaItems = khalaExtensionItems()
  const sections = {
    apps: section("apps", "Apps and connectors", apps.items),
    hooks: section("hooks", "Hooks", hooks.items),
    imports: section("imports", "External config imports", imports.items),
    khala: section("khala", "Khala desktop extensions", khalaItems),
    marketplace: section("marketplace", "Plugin marketplaces", plugins.marketplaceItems),
    mcp: section("mcp", "MCP servers", mcp.items),
    plugins: section("plugins", "Plugins", plugins.pluginItems),
    skills: section("skills", "Skills", skills.items),
  }
  const errors = input.errors ?? []
  const endpointDiagnostics: KhalaCodeDesktopCodexEcosystemDiagnostic[] = errors.map((error, index) => ({
    ref: `codex_ecosystem.endpoint.error.${index}.${safeIdPart(error)}`,
    source: "marketplace",
    severity: "warning",
    title: "Codex ecosystem endpoint failed",
    detail: limitedDetail(error),
    action: "refresh",
    observedAt,
  }))
  const diagnostics = uniqueDiagnosticsByRef([
    ...endpointDiagnostics,
    ...notificationDiagnostics(notifications),
    ...skills.diagnostics,
    ...hooks.diagnostics,
    ...imports.diagnostics,
    ...plugins.diagnostics,
    ...apps.diagnostics,
    ...mcp.diagnostics,
  ])

  return {
    ok: errors.length === 0 && diagnostics.every(diagnostic => diagnostic.severity !== "critical"),
    cwd: input.cwd ?? null,
    observedAt,
    errors,
    notifications,
    sections,
    diagnostics,
  }
}
