export type DesktopServiceScope =
  | "process"
  | "work_context"
  | "conversation_or_run"
  | "request_or_command"
  | "foreign_host_or_view"
  | "renderer_view"

export type DesktopServiceOwner =
  | "electron-main"
  | "preload"
  | "renderer"
  | "shared-contract"
  | "test-fixture"

export type DesktopAuthority =
  | "runtime"
  | "filesystem"
  | "transport"
  | "identity"
  | "policy"
  | "provider"
  | "database"
  | "clock"
  | "foreign_host"
  | "view_projection"

export type DesktopPublicSchemaIdentity = Readonly<{
  name: string
  module: string
  legacy?: true
}>

export type DesktopOwnedResource = Readonly<{
  kind: "fiber" | "subscription" | "native_handle" | "database" | "file" | "http_listener"
  disposesWith: DesktopServiceScope | "external"
}>

export type DesktopServiceCacheKey = Readonly<{
  scope: DesktopServiceScope | "none"
  parts: ReadonlyArray<string>
}>

export type DesktopServiceFreshness = Readonly<{
  source: "static_manifest" | "event_stream" | "request_response" | "filesystem_snapshot" | "database_subscription" | "renderer_state" | "session_state"
  maxAge: "process_lifetime" | "work_context_lifetime" | "conversation_lifetime" | "request_lifetime" | "event_driven" | "animation_frame"
  invalidatesOn: ReadonlyArray<string>
}>

export type DesktopServiceDisposal = Readonly<{
  disposesWith: DesktopServiceScope | "external"
  closes: ReadonlyArray<DesktopOwnedResource["kind"]>
}>

export type DesktopServiceTopologyEntry = Readonly<{
  id: string
  label: string
  owner: DesktopServiceOwner
  scope: DesktopServiceScope
  modules: ReadonlyArray<string>
  dependsOn: ReadonlyArray<string>
  authority: ReadonlyArray<DesktopAuthority>
  cacheKey?: DesktopServiceCacheKey
  freshness?: DesktopServiceFreshness
  disposal?: DesktopServiceDisposal
  publicSchemas?: ReadonlyArray<DesktopPublicSchemaIdentity>
  ownedResources?: ReadonlyArray<DesktopOwnedResource>
  perimeter?: true
  internalRunPromise?: true
  ambientAuthority?: ReadonlyArray<"cwd" | "async_local_storage" | "renderer_path" | "module_singleton">
  failureTaxonomy?: "recoverable_domain_refusal" | "dependency_outage" | "interruption" | "invariant_defect" | "telemetry_degradation"
}>

export type DesktopServiceTopologyViolation = Readonly<{
  code:
    | "duplicate_service_id"
    | "unknown_dependency"
    | "cycle"
    | "wrong_scope_dependency"
    | "renderer_runtime_authority"
    | "duplicate_public_schema_identity"
    | "ambient_authority"
    | "unowned_resource"
    | "missing_cache_key"
    | "invalid_cache_key_scope"
    | "missing_freshness"
    | "missing_disposal"
    | "invalid_disposal_scope"
    | "internal_run_promise_escape"
  serviceId: string
  detail: string
}>

const scopeRank: Record<DesktopServiceScope, number> = {
  process: 0,
  work_context: 1,
  conversation_or_run: 2,
  request_or_command: 3,
  foreign_host_or_view: 3,
  renderer_view: 4,
}

const runtimeGatewaySchemas = [
  "DesktopRuntimeGatewayRequest",
  "DesktopRuntimeGatewayResponse",
  "DesktopRuntimeGatewayEvent",
]

const chatSchemas = [
  "DesktopThread",
  "DesktopTurnRequest",
]

const workspaceSchemas = [
  "DesktopWorkspaceFileRequest",
  "DesktopWorkspaceSaveRequest",
  "DesktopWorkspaceGitDiffRequest",
]

export const desktopServiceTopology = [
  {
    id: "electron-main-composition",
    label: "Electron main composition root and IPC perimeter",
    owner: "electron-main",
    scope: "process",
    modules: ["apps/openagents-desktop/src/main.ts"],
    dependsOn: [
      "desktop-runtime-gateway",
      "desktop-session-custody",
      "desktop-sync-host",
      "workspace-root",
      "legacy-thread-store",
      "codex-history-reader",
      "fleet-stage-control",
      "codex-connect-host",
      "preload-bridge",
    ],
    authority: ["runtime", "policy", "clock"],
    cacheKey: {
      scope: "process",
      parts: ["electron.app.instance", "desktop.protocol_version"],
    },
    freshness: {
      source: "static_manifest",
      maxAge: "process_lifetime",
      invalidatesOn: ["app-restart", "protocol-version-change"],
    },
    disposal: {
      disposesWith: "process",
      closes: ["native_handle"],
    },
    perimeter: true,
    ownedResources: [{ kind: "native_handle", disposesWith: "process" }],
    failureTaxonomy: "invariant_defect",
  },
  {
    id: "desktop-runtime-gateway",
    label: "Host-owned Desktop Runtime Gateway",
    owner: "electron-main",
    scope: "process",
    modules: [
      "apps/openagents-desktop/src/runtime-gateway.ts",
      "apps/openagents-desktop/src/runtime-gateway-contract.ts",
    ],
    dependsOn: [
      "desktop-sync-host",
      "desktop-session-custody",
      "codex-history-reader",
    ],
    authority: ["runtime", "policy"],
    cacheKey: {
      scope: "process",
      parts: ["runtime_gateway.protocol_version", "host_session_generation"],
    },
    freshness: {
      source: "event_stream",
      maxAge: "event_driven",
      invalidatesOn: ["gateway-dispose", "session-change", "sync-host-event"],
    },
    disposal: {
      disposesWith: "process",
      closes: ["subscription", "fiber"],
    },
    publicSchemas: runtimeGatewaySchemas.map(name => ({
      name,
      module: "apps/openagents-desktop/src/runtime-gateway-contract.ts",
    })),
    failureTaxonomy: "recoverable_domain_refusal",
  },
  {
    id: "desktop-session-custody",
    label: "OpenAgents account session vault, PKCE, recovery, and sign-out host",
    owner: "electron-main",
    scope: "process",
    modules: [
      "apps/openagents-desktop/src/desktop-session-vault.ts",
      "apps/openagents-desktop/src/desktop-session-pkce.ts",
      "apps/openagents-desktop/src/desktop-session-recovery.ts",
    ],
    dependsOn: [],
    authority: ["identity", "transport", "clock"],
    cacheKey: {
      scope: "process",
      parts: ["safe_storage_profile", "session_store_generation"],
    },
    freshness: {
      source: "session_state",
      maxAge: "event_driven",
      invalidatesOn: ["sign-in", "sign-out", "token-refresh", "recovery-listener-close"],
    },
    disposal: {
      disposesWith: "process",
      closes: ["file", "http_listener"],
    },
    ownedResources: [
      { kind: "file", disposesWith: "process" },
      { kind: "http_listener", disposesWith: "request_or_command" },
    ],
    perimeter: true,
    failureTaxonomy: "dependency_outage",
  },
  {
    id: "desktop-sync-host",
    label: "Host-owned Khala Sync local store and authenticated session adapter",
    owner: "electron-main",
    scope: "process",
    modules: [
      "apps/openagents-desktop/src/desktop-sync-host.ts",
      "apps/openagents-desktop/src/desktop-sync-store.ts",
    ],
    dependsOn: ["desktop-session-custody"],
    authority: ["database", "identity", "transport"],
    cacheKey: {
      scope: "process",
      parts: ["owner_user_id", "sync_database_path"],
    },
    freshness: {
      source: "database_subscription",
      maxAge: "event_driven",
      invalidatesOn: ["session-owner-change", "local-mutation", "remote-sync-event"],
    },
    disposal: {
      disposesWith: "process",
      closes: ["database", "subscription"],
    },
    publicSchemas: [
      {
        name: "KhalaSyncSchema",
        module: "packages/khala-sync/src/index.ts",
      },
    ],
    ownedResources: [
      { kind: "database", disposesWith: "process" },
      { kind: "subscription", disposesWith: "process" },
    ],
    failureTaxonomy: "dependency_outage",
  },
  {
    id: "workspace-root",
    label: "Selected workspace filesystem and Git review surface",
    owner: "electron-main",
    scope: "work_context",
    modules: [
      "apps/openagents-desktop/src/workspace-service.ts",
      "apps/openagents-desktop/src/workspace-contract.ts",
    ],
    dependsOn: [],
    authority: ["filesystem", "policy"],
    cacheKey: {
      scope: "work_context",
      parts: ["workspace_root_uri", "workspace_selection_generation"],
    },
    freshness: {
      source: "filesystem_snapshot",
      maxAge: "work_context_lifetime",
      invalidatesOn: ["workspace-select", "file-save", "git-refresh"],
    },
    disposal: {
      disposesWith: "work_context",
      closes: ["file"],
    },
    publicSchemas: workspaceSchemas.map(name => ({
      name,
      module: "apps/openagents-desktop/src/workspace-contract.ts",
    })),
    failureTaxonomy: "recoverable_domain_refusal",
  },
  {
    id: "codex-history-reader",
    label: "Read-only Codex history catalog and page projector",
    owner: "electron-main",
    scope: "process",
    modules: [
      "apps/openagents-desktop/src/codex-history.ts",
      "apps/openagents-desktop/src/codex-history-contract.ts",
    ],
    dependsOn: [],
    authority: ["filesystem"],
    cacheKey: {
      scope: "process",
      parts: ["codex_home", "codex_history_root"],
    },
    freshness: {
      source: "filesystem_snapshot",
      maxAge: "event_driven",
      invalidatesOn: ["history-poll", "manual-refresh"],
    },
    disposal: {
      disposesWith: "process",
      closes: ["file"],
    },
    publicSchemas: [
      {
        name: "CodexHistoryCatalog",
        module: "apps/openagents-desktop/src/codex-history-contract.ts",
      },
      {
        name: "CodexHistoryPage",
        module: "apps/openagents-desktop/src/codex-history-contract.ts",
      },
    ],
    failureTaxonomy: "recoverable_domain_refusal",
  },
  {
    id: "legacy-thread-store",
    label: "Local development thread JSON store and fallback chat turn",
    owner: "electron-main",
    scope: "conversation_or_run",
    modules: [
      "apps/openagents-desktop/src/thread-store.ts",
      "apps/openagents-desktop/src/chat-service.ts",
      "apps/openagents-desktop/src/chat-contract.ts",
    ],
    dependsOn: ["workspace-root"],
    authority: ["filesystem", "transport"],
    cacheKey: {
      scope: "conversation_or_run",
      parts: ["workspace_root_uri", "thread_id"],
    },
    freshness: {
      source: "filesystem_snapshot",
      maxAge: "conversation_lifetime",
      invalidatesOn: ["thread-select", "message-write", "fallback-turn"],
    },
    disposal: {
      disposesWith: "conversation_or_run",
      closes: ["file"],
    },
    publicSchemas: chatSchemas.map(name => ({
      name,
      module: "apps/openagents-desktop/src/chat-contract.ts",
      legacy: true,
    })),
    ownedResources: [{ kind: "file", disposesWith: "conversation_or_run" }],
    failureTaxonomy: "dependency_outage",
  },
  {
    id: "fleet-stage-control",
    label: "Pylon Fleet stage request perimeter",
    owner: "electron-main",
    scope: "request_or_command",
    modules: [
      "apps/openagents-desktop/src/fleet-control.ts",
      "apps/openagents-desktop/src/fleet-contract.ts",
    ],
    dependsOn: ["workspace-root"],
    authority: ["provider", "policy", "transport"],
    cacheKey: {
      scope: "request_or_command",
      parts: ["fleet_stage_request_id", "objective_digest"],
    },
    freshness: {
      source: "request_response",
      maxAge: "request_lifetime",
      invalidatesOn: ["stage-request-finished", "stage-request-cancelled"],
    },
    disposal: {
      disposesWith: "request_or_command",
      closes: ["subscription"],
    },
    publicSchemas: [
      {
        name: "FleetStageRequest",
        module: "apps/openagents-desktop/src/fleet-contract.ts",
      },
    ],
    perimeter: true,
    failureTaxonomy: "recoverable_domain_refusal",
  },
  {
    id: "codex-connect-host",
    label: "Local Pylon Codex account reconnect host",
    owner: "electron-main",
    scope: "request_or_command",
    modules: [
      "apps/openagents-desktop/src/codex-connect.ts",
      "apps/openagents-desktop/src/codex-connect-contract.ts",
    ],
    dependsOn: [],
    authority: ["provider", "transport", "identity"],
    cacheKey: {
      scope: "request_or_command",
      parts: ["codex_account_ref", "connect_request_id"],
    },
    freshness: {
      source: "request_response",
      maxAge: "request_lifetime",
      invalidatesOn: ["connect-request-finished", "connect-request-cancelled"],
    },
    disposal: {
      disposesWith: "request_or_command",
      closes: ["native_handle"],
    },
    publicSchemas: [
      {
        name: "CodexAccountsResult",
        module: "apps/openagents-desktop/src/codex-connect-contract.ts",
      },
      {
        name: "CodexConnectStatus",
        module: "apps/openagents-desktop/src/codex-connect-contract.ts",
      },
    ],
    perimeter: true,
    ownedResources: [{ kind: "native_handle", disposesWith: "request_or_command" }],
    failureTaxonomy: "interruption",
  },
  {
    id: "preload-bridge",
    label: "Sandboxed preload bridge with schema-checked IPC calls",
    owner: "preload",
    scope: "foreign_host_or_view",
    modules: ["apps/openagents-desktop/src/preload.cts"],
    dependsOn: [
      "desktop-runtime-gateway",
      "workspace-root",
      "legacy-thread-store",
      "fleet-stage-control",
      "codex-connect-host",
    ],
    authority: ["foreign_host"],
    cacheKey: {
      scope: "foreign_host_or_view",
      parts: ["browser_window_id", "preload_protocol_version"],
    },
    freshness: {
      source: "event_stream",
      maxAge: "event_driven",
      invalidatesOn: ["ipc-reconnect", "window-close"],
    },
    disposal: {
      disposesWith: "foreign_host_or_view",
      closes: ["subscription"],
    },
    perimeter: true,
    failureTaxonomy: "recoverable_domain_refusal",
  },
  {
    id: "effect-native-renderer",
    label: "Effect Native renderer state, command registry, and projections",
    owner: "renderer",
    scope: "renderer_view",
    modules: [
      "apps/openagents-desktop/src/renderer/boot.ts",
      "apps/openagents-desktop/src/renderer/shell.ts",
      "apps/openagents-desktop/src/renderer/runtime-conversation.ts",
      "apps/openagents-desktop/src/renderer/history-workspace.ts",
      "apps/openagents-desktop/src/renderer/command-registry.ts",
      "apps/openagents-desktop/src/renderer/settings.ts",
    ],
    dependsOn: ["preload-bridge"],
    authority: ["view_projection"],
    cacheKey: {
      scope: "renderer_view",
      parts: ["browser_window_id", "renderer_route", "state_generation"],
    },
    freshness: {
      source: "renderer_state",
      maxAge: "animation_frame",
      invalidatesOn: ["renderer-command", "host-event", "view-remount"],
    },
    disposal: {
      disposesWith: "renderer_view",
      closes: ["subscription"],
    },
    failureTaxonomy: "telemetry_degradation",
  },
] as const satisfies ReadonlyArray<DesktopServiceTopologyEntry>

const violation = (
  code: DesktopServiceTopologyViolation["code"],
  serviceId: string,
  detail: string,
): DesktopServiceTopologyViolation => ({ code, serviceId, detail })

const findCycle = (
  entries: ReadonlyArray<DesktopServiceTopologyEntry>,
): ReadonlyArray<string> | null => {
  const byId = new Map(entries.map(entry => [entry.id, entry]))
  const visiting = new Set<string>()
  const visited = new Set<string>()
  const path: string[] = []

  const walk = (serviceId: string): ReadonlyArray<string> | null => {
    if (visiting.has(serviceId)) {
      return [...path.slice(path.indexOf(serviceId)), serviceId]
    }
    if (visited.has(serviceId)) return null
    const entry = byId.get(serviceId)
    if (entry === undefined) return null
    visiting.add(serviceId)
    path.push(serviceId)
    for (const dependencyId of entry.dependsOn) {
      const cycle = walk(dependencyId)
      if (cycle !== null) return cycle
    }
    path.pop()
    visiting.delete(serviceId)
    visited.add(serviceId)
    return null
  }

  for (const entry of entries) {
    const cycle = walk(entry.id)
    if (cycle !== null) return cycle
  }
  return null
}

export const validateDesktopServiceTopology = (
  entries: ReadonlyArray<DesktopServiceTopologyEntry> = desktopServiceTopology,
): ReadonlyArray<DesktopServiceTopologyViolation> => {
  const violations: DesktopServiceTopologyViolation[] = []
  const byId = new Map<string, DesktopServiceTopologyEntry>()

  for (const entry of entries) {
    if (byId.has(entry.id)) {
      violations.push(violation("duplicate_service_id", entry.id, "Service ids must be unique."))
    }
    byId.set(entry.id, entry)
  }

  for (const entry of entries) {
    for (const dependencyId of entry.dependsOn) {
      const dependency = byId.get(dependencyId)
      if (dependency === undefined) {
        violations.push(violation("unknown_dependency", entry.id, `Unknown dependency ${dependencyId}.`))
        continue
      }
      if (entry.perimeter !== true && scopeRank[dependency.scope] > scopeRank[entry.scope]) {
        violations.push(violation(
          "wrong_scope_dependency",
          entry.id,
          `${entry.scope} service may not capture narrower ${dependency.scope} dependency ${dependencyId}.`,
        ))
      }
    }

    if (entry.scope === "renderer_view" && entry.authority.some(authority => authority !== "view_projection")) {
      violations.push(violation("renderer_runtime_authority", entry.id, "Renderer/view state may only own view projections."))
    }
    if ((entry.ambientAuthority?.length ?? 0) > 0) {
      violations.push(violation("ambient_authority", entry.id, `Ambient authority: ${entry.ambientAuthority?.join(", ")}.`))
    }
    if (entry.cacheKey === undefined || entry.cacheKey.parts.length === 0) {
      violations.push(violation("missing_cache_key", entry.id, "Services must declare a non-empty cache key."))
    } else if (entry.cacheKey.scope !== "none" && entry.cacheKey.scope !== entry.scope) {
      violations.push(violation(
        "invalid_cache_key_scope",
        entry.id,
        `Cache key scope ${entry.cacheKey.scope} must match service scope ${entry.scope}.`,
      ))
    }
    if (entry.freshness === undefined || entry.freshness.invalidatesOn.length === 0) {
      violations.push(violation("missing_freshness", entry.id, "Services must declare freshness invalidation."))
    }
    if (entry.disposal === undefined) {
      violations.push(violation("missing_disposal", entry.id, "Services must declare their disposal owner."))
    } else if (entry.disposal.disposesWith !== "external" && scopeRank[entry.disposal.disposesWith] < scopeRank[entry.scope]) {
      violations.push(violation(
        "invalid_disposal_scope",
        entry.id,
        `Service disposal escapes to wider ${entry.disposal.disposesWith} scope instead of owning ${entry.scope} scope.`,
      ))
    }
    for (const resource of entry.ownedResources ?? []) {
      if (resource.disposesWith !== "external" && scopeRank[resource.disposesWith] < scopeRank[entry.scope]) {
        violations.push(violation(
          "unowned_resource",
          entry.id,
          `${resource.kind} disposes with wider ${resource.disposesWith} scope instead of owning ${entry.scope} scope.`,
        ))
      }
    }
    if (entry.internalRunPromise === true && entry.perimeter !== true) {
      violations.push(violation("internal_run_promise_escape", entry.id, "Internal runPromise/runPromise-like exits must stay at named perimeter modules."))
    }
  }

  const cycle = findCycle(entries)
  if (cycle !== null) {
    violations.push(violation("cycle", cycle[0] ?? "unknown", `Cycle: ${cycle.join(" -> ")}.`))
  }

  const schemaModules = new Map<string, Set<string>>()
  for (const entry of entries) {
    for (const schema of entry.publicSchemas ?? []) {
      if (schema.legacy === true) continue
      const modules = schemaModules.get(schema.name) ?? new Set<string>()
      modules.add(schema.module)
      schemaModules.set(schema.name, modules)
    }
  }
  for (const [schemaName, modules] of schemaModules) {
    if (modules.size > 1) {
      violations.push(violation(
        "duplicate_public_schema_identity",
        schemaName,
        `Public schema ${schemaName} is declared by ${[...modules].join(", ")}.`,
      ))
    }
  }

  return violations
}

export const assertValidDesktopServiceTopology = (
  entries: ReadonlyArray<DesktopServiceTopologyEntry> = desktopServiceTopology,
): void => {
  const violations = validateDesktopServiceTopology(entries)
  if (violations.length > 0) {
    throw new Error(violations.map(item => `${item.code}:${item.serviceId}:${item.detail}`).join("\n"))
  }
}
