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
  | "network"
  | "process"
  | "secret"
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
  invalidatesOn: ReadonlyArray<string>
}>

export type DesktopServiceSourceEvidence = Readonly<{
  module: string
  compositionModule: string
  constructions: ReadonlyArray<string>
}>

export type DesktopServiceTopologyEntry = Readonly<{
  id: string
  label: string
  owner: DesktopServiceOwner
  scope: DesktopServiceScope
  installedAt: DesktopServiceScope
  modules: ReadonlyArray<string>
  sourceEvidence: ReadonlyArray<DesktopServiceSourceEvidence>
  dependsOn: ReadonlyArray<string>
  authority: ReadonlyArray<DesktopAuthority>
  cacheKey?: DesktopServiceCacheKey
  freshness?: DesktopServiceFreshness
  disposal?: DesktopServiceDisposal
  publicSchemas?: ReadonlyArray<DesktopPublicSchemaIdentity>
  ownedResources?: ReadonlyArray<DesktopOwnedResource>
  perimeter?: true
  runtimeExitPerimeter?: true
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
    | "wrong_installation_scope"
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
    | "missing_source_evidence"
    | "missing_source_module"
    | "missing_construction_symbol"
    | "undeclared_source_authority"
    | "forbidden_renderer_source_authority"
    | "source_ambient_authority"
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
  "DesktopWorkspaceTreeRequest",
  "DesktopWorkspaceTreePage",
  "DesktopWorkspaceSearchRequest",
  "DesktopWorkspaceSearchPage",
  "DesktopWorkspaceChange",
  "DesktopWorkspaceCreateRequest",
  "DesktopWorkspaceRenameRequest",
  "DesktopWorkspaceDeleteRequest",
  "DesktopWorkspaceRevealRequest",
  "DesktopWorkspaceOperationResult",
]

export const desktopServiceTopology = [
  {
    id: "electron-main-composition",
    label: "Electron main composition root and IPC perimeter",
    owner: "electron-main",
    scope: "process",
    installedAt: "process",
    modules: ["apps/openagents-desktop/src/main.ts"],
    sourceEvidence: [{
      module: "apps/openagents-desktop/src/main.ts",
      compositionModule: "apps/openagents-desktop/src/main.ts",
      constructions: ["createDesktopRuntimeGateway", "ipcMain.handle", "new BrowserWindow"],
    }],
    dependsOn: [
      "desktop-runtime-gateway",
      "desktop-session-custody",
      "desktop-sync-host",
      "workspace-root",
      "legacy-thread-store",
      "codex-history-reader",
      "desktop-voice-host",
      "fleet-stage-control",
      "codex-connect-host",
      "desktop-host-lifecycle",
      "desktop-operation-correlation",
      "preload-bridge",
    ],
    authority: ["runtime", "policy", "clock", "filesystem", "process", "secret", "network"],
    cacheKey: {
      scope: "none",
      parts: [],
    },
    freshness: {
      source: "static_manifest",
      maxAge: "process_lifetime",
      invalidatesOn: ["app-restart", "protocol-version-change"],
    },
    disposal: {
      disposesWith: "process",
      invalidatesOn: ["app-shutdown"],
    },
    perimeter: true,
    // The process perimeter may capture the owner's launch directory exactly
    // once. It validates the path and converts it into an explicit
    // WorkContext grant before any narrower service receives it.
    ambientAuthority: ["cwd"],
    ownedResources: [{ kind: "native_handle", disposesWith: "process" }],
    failureTaxonomy: "invariant_defect",
  },
  {
    id: "desktop-runtime-gateway",
    label: "Host-owned Desktop Runtime Gateway",
    owner: "electron-main",
    scope: "process",
    installedAt: "process",
    modules: [
      "apps/openagents-desktop/src/runtime-gateway.ts",
      "apps/openagents-desktop/src/runtime-gateway-contract.ts",
    ],
    sourceEvidence: [{
      module: "apps/openagents-desktop/src/runtime-gateway.ts",
      compositionModule: "apps/openagents-desktop/src/main.ts",
      constructions: ["createDesktopRuntimeGateway"],
    }],
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
      invalidatesOn: ["gateway-dispose", "app-shutdown"],
    },
    publicSchemas: runtimeGatewaySchemas.map(name => ({
      name,
      module: "apps/openagents-desktop/src/runtime-gateway-contract.ts",
    })),
    failureTaxonomy: "recoverable_domain_refusal",
  },
  {
    id: "desktop-session-custody",
    label: "Explicit OpenAgents account session vault, PKCE, and sign-out host",
    owner: "electron-main",
    scope: "process",
    installedAt: "process",
    modules: [
      "apps/openagents-desktop/src/desktop-session-vault.ts",
      "apps/openagents-desktop/src/desktop-session-pkce.ts",
    ],
    sourceEvidence: [
      {
        module: "apps/openagents-desktop/src/desktop-session-vault.ts",
        compositionModule: "apps/openagents-desktop/src/main.ts",
        constructions: ["openDesktopSessionVault"],
      },
      {
        module: "apps/openagents-desktop/src/desktop-session-pkce.ts",
        compositionModule: "apps/openagents-desktop/src/main.ts",
        constructions: ["signInDesktopSession", "signOutDesktopSession"],
      },
      {
        module: "apps/openagents-desktop/src/desktop-session-pkce.ts",
        compositionModule: "apps/openagents-desktop/src/desktop-session-pkce.ts",
        constructions: ["openDesktopAuthLoopbackListener"],
      },
    ],
    dependsOn: [],
    authority: ["identity", "transport", "clock", "filesystem", "network", "secret"],
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
      invalidatesOn: ["app-shutdown"],
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
    installedAt: "process",
    modules: [
      "apps/openagents-desktop/src/desktop-sync-host.ts",
      "apps/openagents-desktop/src/desktop-sync-store.ts",
    ],
    sourceEvidence: [
      {
        module: "apps/openagents-desktop/src/desktop-sync-host.ts",
        compositionModule: "apps/openagents-desktop/src/main.ts",
        constructions: ["openDesktopSyncHost"],
      },
      {
        module: "apps/openagents-desktop/src/desktop-sync-store.ts",
        compositionModule: "apps/openagents-desktop/src/desktop-sync-host.ts",
        constructions: ["openDesktopSyncStore"],
      },
    ],
    dependsOn: ["desktop-session-custody"],
    authority: ["database", "identity", "transport", "filesystem", "network"],
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
      invalidatesOn: ["unlink", "app-shutdown"],
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
    installedAt: "work_context",
    modules: [
      "apps/openagents-desktop/src/workspace-service.ts",
      "apps/openagents-desktop/src/workspace-contract.ts",
      "apps/openagents-desktop/src/workspace-search-host.ts",
      "apps/openagents-desktop/src/workspace-search-worker.ts",
    ],
    sourceEvidence: [
      {
        module: "apps/openagents-desktop/src/workspace-service.ts",
        compositionModule: "apps/openagents-desktop/src/main.ts",
        constructions: ["openWorkspaceService"],
      },
      {
        module: "apps/openagents-desktop/src/workspace-search-host.ts",
        compositionModule: "apps/openagents-desktop/src/workspace-service.ts",
        constructions: ["makeWorkspaceSearchHost"],
      },
      {
        module: "apps/openagents-desktop/src/workspace-service.ts",
        compositionModule: "apps/openagents-desktop/src/workspace-search-worker.ts",
        constructions: ["searchWorkspace"],
      },
    ],
    dependsOn: [],
    authority: ["filesystem", "policy", "process"],
    cacheKey: {
      scope: "work_context",
      parts: ["workspace_root_uri", "workspace_selection_generation"],
    },
    freshness: {
      source: "request_response",
      maxAge: "request_lifetime",
      invalidatesOn: ["workspace-select", "file-save", "watch-change", "explicit-refresh"],
    },
    disposal: {
      disposesWith: "work_context",
      invalidatesOn: ["workspace-switch", "app-shutdown"],
    },
    publicSchemas: workspaceSchemas.map(name => ({
      name,
      module: "apps/openagents-desktop/src/workspace-contract.ts",
    })),
    ownedResources: [{ kind: "native_handle", disposesWith: "work_context" }],
    failureTaxonomy: "recoverable_domain_refusal",
  },
  {
    id: "codex-history-reader",
    label: "Read-only merged Codex + Claude history catalog, page, and search projector",
    owner: "electron-main",
    scope: "process",
    installedAt: "process",
    modules: [
      "apps/openagents-desktop/src/codex-history.ts",
      "apps/openagents-desktop/src/claude-history.ts",
      "apps/openagents-desktop/src/merged-history.ts",
      "apps/openagents-desktop/src/history-search.ts",
      "apps/openagents-desktop/src/codex-history-host.ts",
      "apps/openagents-desktop/src/codex-history-worker.ts",
      "apps/openagents-desktop/src/codex-history-contract.ts",
    ],
    sourceEvidence: [
      {
        module: "apps/openagents-desktop/src/codex-history-host.ts",
        compositionModule: "apps/openagents-desktop/src/main.ts",
        constructions: ["makeCodexHistoryHost"],
      },
      {
        module: "apps/openagents-desktop/src/merged-history.ts",
        compositionModule: "apps/openagents-desktop/src/codex-history-worker.ts",
        constructions: ["readMergedHistoryCatalog", "readMergedHistoryPage", "searchMergedHistory"],
      },
      {
        module: "apps/openagents-desktop/src/codex-history.ts",
        compositionModule: "apps/openagents-desktop/src/merged-history.ts",
        constructions: ["readCodexHistoryCatalog", "readCodexHistoryPage"],
      },
      {
        module: "apps/openagents-desktop/src/claude-history.ts",
        compositionModule: "apps/openagents-desktop/src/merged-history.ts",
        constructions: ["readClaudeHistoryCatalog", "readClaudeHistoryPage"],
      },
    ],
    dependsOn: [],
    authority: ["filesystem", "process"],
    cacheKey: {
      scope: "process",
      parts: ["codex_history_root"],
    },
    freshness: {
      source: "filesystem_snapshot",
      maxAge: "process_lifetime",
      invalidatesOn: ["history-root-change", "history-worker-restart"],
    },
    disposal: {
      disposesWith: "process",
      invalidatesOn: ["history-worker-error", "app-shutdown"],
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
      {
        name: "CodexHistorySearchResponse",
        module: "apps/openagents-desktop/src/codex-history-contract.ts",
      },
    ],
    ownedResources: [{ kind: "native_handle", disposesWith: "process" }],
    failureTaxonomy: "recoverable_domain_refusal",
  },
  {
    id: "legacy-thread-store",
    label: "Local development thread JSON store and fallback chat turn",
    owner: "electron-main",
    scope: "request_or_command",
    installedAt: "request_or_command",
    modules: [
      "apps/openagents-desktop/src/thread-store.ts",
      "apps/openagents-desktop/src/chat-service.ts",
      "apps/openagents-desktop/src/chat-contract.ts",
    ],
    sourceEvidence: [
      {
        module: "apps/openagents-desktop/src/thread-store.ts",
        compositionModule: "apps/openagents-desktop/src/main.ts",
        constructions: ["makeThreadStore"],
      },
      {
        module: "apps/openagents-desktop/src/chat-service.ts",
        compositionModule: "apps/openagents-desktop/src/main.ts",
        constructions: ["completeChatTurn"],
      },
    ],
    dependsOn: [],
    authority: ["filesystem", "transport", "network", "process", "secret"],
    cacheKey: {
      scope: "none",
      parts: [],
    },
    freshness: {
      source: "request_response",
      maxAge: "request_lifetime",
      invalidatesOn: ["thread-read", "message-write", "fallback-turn"],
    },
    disposal: {
      disposesWith: "request_or_command",
      invalidatesOn: ["request-finished"],
    },
    publicSchemas: chatSchemas.map(name => ({
      name,
      module: "apps/openagents-desktop/src/chat-contract.ts",
      legacy: true,
    })),
    failureTaxonomy: "dependency_outage",
  },
  {
    id: "fleet-stage-control",
    label: "Pylon Fleet stage request perimeter",
    owner: "electron-main",
    scope: "request_or_command",
    installedAt: "request_or_command",
    modules: [
      "apps/openagents-desktop/src/fleet-control.ts",
      "apps/openagents-desktop/src/fleet-contract.ts",
    ],
    sourceEvidence: [{
      module: "apps/openagents-desktop/src/fleet-control.ts",
      compositionModule: "apps/openagents-desktop/src/main.ts",
      constructions: ["submitFleetBrief"],
    }],
    dependsOn: [],
    authority: ["provider", "policy", "transport", "filesystem", "network", "process", "secret"],
    cacheKey: {
      scope: "none",
      parts: [],
    },
    freshness: {
      source: "request_response",
      maxAge: "request_lifetime",
      invalidatesOn: ["stage-request-finished", "stage-request-cancelled"],
    },
    disposal: {
      disposesWith: "request_or_command",
      invalidatesOn: ["stage-request-finished", "stage-request-cancelled"],
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
    scope: "process",
    installedAt: "process",
    modules: [
      "apps/openagents-desktop/src/codex-connect.ts",
      "apps/openagents-desktop/src/codex-connect-contract.ts",
    ],
    sourceEvidence: [{
      module: "apps/openagents-desktop/src/codex-connect.ts",
      compositionModule: "apps/openagents-desktop/src/main.ts",
      constructions: ["makeCodexConnectService"],
    }],
    dependsOn: [],
    authority: ["provider", "transport", "identity", "filesystem", "process", "secret"],
    cacheKey: {
      scope: "process",
      parts: ["pylon_home", "account_registry_generation"],
    },
    freshness: {
      source: "request_response",
      maxAge: "event_driven",
      invalidatesOn: ["account-list-finished", "connect-status-change", "service-dispose"],
    },
    disposal: {
      disposesWith: "process",
      invalidatesOn: ["service-dispose", "app-shutdown"],
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
    id: "desktop-host-lifecycle",
    label: "Replaceable process, session, WorkContext, and window lifecycle owner",
    owner: "electron-main",
    scope: "process",
    installedAt: "process",
    modules: ["apps/openagents-desktop/src/desktop-host-lifecycle.ts"],
    sourceEvidence: [{
      module: "apps/openagents-desktop/src/desktop-host-lifecycle.ts",
      compositionModule: "apps/openagents-desktop/src/main.ts",
      constructions: ["makeDesktopHostLifecycle"],
    }],
    dependsOn: [
      "desktop-runtime-gateway",
      "desktop-sync-host",
      "workspace-root",
      "codex-connect-host",
      "codex-history-reader",
    ],
    authority: ["runtime", "policy"],
    cacheKey: {
      scope: "process",
      parts: ["app_lifecycle_generation"],
    },
    freshness: {
      source: "event_stream",
      maxAge: "event_driven",
      invalidatesOn: ["service-replacement", "window-close", "app-dispose"],
    },
    disposal: {
      disposesWith: "process",
      invalidatesOn: ["app-dispose"],
    },
    perimeter: true,
    failureTaxonomy: "invariant_defect",
  },
  {
    id: "desktop-operation-correlation",
    label: "Public-safe Desktop operation correlation journal",
    owner: "electron-main",
    scope: "process",
    installedAt: "process",
    modules: ["apps/openagents-desktop/src/desktop-operation-context.ts"],
    sourceEvidence: [{
      module: "apps/openagents-desktop/src/desktop-operation-context.ts",
      compositionModule: "apps/openagents-desktop/src/main.ts",
      constructions: ["makeDesktopCorrelationJournal", "decodeDesktopOperationContext"],
    }],
    dependsOn: [],
    authority: ["policy"],
    cacheKey: {
      scope: "process",
      parts: ["desktop_session_ref", "correlation_ref"],
    },
    freshness: {
      source: "event_stream",
      maxAge: "event_driven",
      invalidatesOn: ["operation-stage", "app-dispose"],
    },
    disposal: {
      disposesWith: "process",
      invalidatesOn: ["app-dispose"],
    },
    publicSchemas: [{
      name: "DesktopOperationContext",
      module: "apps/openagents-desktop/src/desktop-operation-context.ts",
    }],
    failureTaxonomy: "telemetry_degradation",
  },
  {
    id: "desktop-voice-native-helper",
    label: "Process-opaque signed microphone, playback, and voice transport helper",
    owner: "electron-main",
    scope: "process",
    installedAt: "process",
    modules: ["apps/openagents-desktop/src/voice-native-helper.ts"],
    sourceEvidence: [{ module: "apps/openagents-desktop/src/voice-native-helper.ts", compositionModule: "apps/openagents-desktop/src/voice-native-helper.ts", constructions: ["verifyVoiceHelper", "spawnVoiceHelper"] }],
    dependsOn: [], authority: ["filesystem", "process", "network"],
    cacheKey: { scope: "process", parts: ["helper_version", "helper_digest", "host_architecture"] },
    freshness: { source: "static_manifest", maxAge: "process_lifetime", invalidatesOn: ["app-restart", "helper-upgrade"] },
    disposal: { disposesWith: "process", invalidatesOn: ["parent-exit", "app-shutdown", "helper-crash"] },
    ownedResources: [{ kind: "native_handle", disposesWith: "process" }], failureTaxonomy: "dependency_outage",
  },
  {
    id: "desktop-voice-host",
    label: "Generation-fenced Desktop voice lifecycle and public-safe state",
    owner: "electron-main", scope: "conversation_or_run", installedAt: "conversation_or_run",
    modules: ["apps/openagents-desktop/src/voice-host.ts", "apps/openagents-desktop/src/voice-permission-policy.ts"],
    sourceEvidence: [
      { module: "apps/openagents-desktop/src/voice-host.ts", compositionModule: "apps/openagents-desktop/src/voice-host.ts", constructions: ["createDesktopVoiceHost"] },
      { module: "apps/openagents-desktop/src/voice-permission-policy.ts", compositionModule: "apps/openagents-desktop/src/voice-permission-policy.ts", constructions: ["decideDesktopMediaPermission"] },
    ],
    dependsOn: ["desktop-voice-native-helper"], authority: ["runtime", "policy", "transport"],
    cacheKey: { scope: "conversation_or_run", parts: ["voice_session_ref", "generation"] },
    freshness: { source: "event_stream", maxAge: "event_driven", invalidatesOn: ["device-change", "permission-change", "network-state", "gateway-revocation"] },
    disposal: { disposesWith: "conversation_or_run", invalidatesOn: ["stop", "replacement", "suspend", "sign-out", "revoke", "app-shutdown"] },
    ownedResources: [{ kind: "native_handle", disposesWith: "conversation_or_run" }, { kind: "subscription", disposesWith: "conversation_or_run" }],
    failureTaxonomy: "recoverable_domain_refusal",
  },
  {
    id: "preload-bridge",
    label: "Sandboxed preload bridge with schema-checked IPC calls",
    owner: "preload",
    scope: "foreign_host_or_view",
    installedAt: "foreign_host_or_view",
    modules: ["apps/openagents-desktop/src/preload.cts"],
    sourceEvidence: [{
      module: "apps/openagents-desktop/src/preload.cts",
      compositionModule: "apps/openagents-desktop/src/preload.cts",
      constructions: ["contextBridge.exposeInMainWorld"],
    }],
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
      invalidatesOn: ["ipc-reconnect", "window-close"],
    },
    perimeter: true,
    failureTaxonomy: "recoverable_domain_refusal",
  },
  {
    id: "effect-native-renderer",
    label: "Effect Native renderer state, command registry, and projections",
    owner: "renderer",
    scope: "renderer_view",
    installedAt: "renderer_view",
    modules: [
      "apps/openagents-desktop/src/renderer/boot.ts",
      "apps/openagents-desktop/src/renderer/shell.ts",
      "apps/openagents-desktop/src/renderer/runtime-conversation.ts",
      "apps/openagents-desktop/src/renderer/history-workspace.ts",
      "apps/openagents-desktop/src/renderer/command-registry.ts",
      "apps/openagents-desktop/src/renderer/settings.ts",
      "apps/openagents.com/packages/effect-native-render-dom/src/react.ts",
      "apps/openagents.com/packages/effect-native-render-dom/src/react-store.ts",
      "apps/openagents.com/packages/effect-native-render-dom/src/react-lowering.ts",
    ],
    sourceEvidence: [{
      module: "apps/openagents-desktop/src/renderer/boot.ts",
      compositionModule: "apps/openagents-desktop/src/renderer/boot.ts",
        constructions: ["mountDesktopShell", "makeReactDomRenderer", "Scope.make"],
    }],
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
      invalidatesOn: ["view-remount", "renderer-close"],
    },
    runtimeExitPerimeter: true,
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
    if (entry.installedAt !== entry.scope) {
      violations.push(violation(
        "wrong_installation_scope",
        entry.id,
        `${entry.scope} service is installed at ${entry.installedAt} scope.`,
      ))
    }
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
    if ((entry.ambientAuthority?.length ?? 0) > 0 && (
      entry.perimeter !== true || entry.scope !== "process" ||
      entry.ambientAuthority?.some(authority => authority !== "cwd")
    )) {
      violations.push(violation("ambient_authority", entry.id, `Ambient authority: ${entry.ambientAuthority?.join(", ")}.`))
    }
    if (entry.cacheKey === undefined) {
      violations.push(violation("missing_cache_key", entry.id, "Services must declare a cache key or explicit no-cache state."))
    } else if (entry.cacheKey.scope === "none" && entry.cacheKey.parts.length !== 0) {
      violations.push(violation("invalid_cache_key_scope", entry.id, "No-cache services may not declare cache-key parts."))
    } else if (entry.cacheKey.scope !== "none" && entry.cacheKey.parts.length === 0) {
      violations.push(violation("missing_cache_key", entry.id, "Cached services must declare non-empty cache-key parts."))
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
    if (entry.disposal === undefined || entry.disposal.invalidatesOn.length === 0) {
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

export type DesktopServiceSourceSet = Readonly<Record<string, string>>

const sourceAuthorityPatterns: ReadonlyArray<Readonly<{
  authority: Extract<DesktopAuthority, "filesystem" | "network" | "process" | "secret">
  patterns: ReadonlyArray<RegExp>
}>> = [
  {
    authority: "filesystem",
    patterns: [/\bnode:(?:fs|path)\b/u],
  },
  {
    authority: "network",
    patterns: [/\bnode:(?:http|https|net|tls|dgram)\b/u, /\bfetch\s*\(/u],
  },
  {
    authority: "process",
    patterns: [/\bnode:(?:child_process|worker_threads)\b/u, /\bprocess\.(?:env|cwd)\b/u, /\bBun\.(?:spawn|spawnSync)\b/u],
  },
  {
    authority: "secret",
    patterns: [
      /\bprocess\.env(?:\.|\[)[^\n]*(?:TOKEN|SECRET|PASSWORD|API_KEY|PRIVATE_KEY)/u,
      /\benv\s*:\s*process\.env\b/u,
      /\b(?:safeStorage|accessToken|refreshToken)\b/u,
    ],
  },
]

const sourceAmbientPatterns: ReadonlyArray<Readonly<{ name: string; pattern: RegExp }>> = [
  { name: "cwd", pattern: /\bprocess\.cwd\s*\(/u },
  { name: "async_local_storage", pattern: /\bAsyncLocalStorage\b/u },
]

/**
 * Couples the typed topology to the checked-in implementation. The caller
 * supplies source text so this module stays usable in production without
 * receiving filesystem authority; the verification oracle reads the files.
 */
export const validateDesktopServiceSourceCoupling = (
  entries: ReadonlyArray<DesktopServiceTopologyEntry>,
  sources: DesktopServiceSourceSet,
): ReadonlyArray<DesktopServiceTopologyViolation> => {
  const violations: DesktopServiceTopologyViolation[] = []

  for (const entry of entries) {
    if (entry.sourceEvidence.length === 0) {
      violations.push(violation(
        "missing_source_evidence",
        entry.id,
        "Service must name at least one real construction symbol.",
      ))
    }

    for (const module of entry.modules) {
      if (sources[module] === undefined) {
        violations.push(violation("missing_source_module", entry.id, `Missing source module ${module}.`))
      }
    }

    const evidenceSources: string[] = []
    for (const evidence of entry.sourceEvidence) {
      if (!entry.modules.includes(evidence.module)) {
        violations.push(violation(
          "missing_source_module",
          entry.id,
          `Construction evidence module ${evidence.module} is not owned by the service.`,
        ))
        continue
      }
      const source = sources[evidence.module]
      if (source === undefined) continue
      evidenceSources.push(source)
      const compositionSource = sources[evidence.compositionModule]
      if (compositionSource === undefined) {
        violations.push(violation(
          "missing_source_module",
          entry.id,
          `Missing composition module ${evidence.compositionModule}.`,
        ))
        continue
      }
      if (evidence.constructions.length === 0) {
        violations.push(violation(
          "missing_source_evidence",
          entry.id,
          `Construction evidence ${evidence.module} has no symbols.`,
        ))
      }
      for (const construction of evidence.constructions) {
        if (!source.includes(construction) || !compositionSource.includes(construction)) {
          violations.push(violation(
            "missing_construction_symbol",
            entry.id,
            `Construction ${construction} must exist in ${evidence.module} and be referenced by ${evidence.compositionModule}.`,
          ))
        }
      }
    }

    const implementation = evidenceSources.join("\n")
    for (const observed of sourceAuthorityPatterns) {
      if (!observed.patterns.some(pattern => pattern.test(implementation))) continue
      if (entry.owner === "renderer") {
        violations.push(violation(
          "forbidden_renderer_source_authority",
          entry.id,
          `Renderer construction contains ${observed.authority} authority.`,
        ))
      } else if (!entry.authority.includes(observed.authority)) {
        violations.push(violation(
          "undeclared_source_authority",
          entry.id,
          `Source uses undeclared ${observed.authority} authority.`,
        ))
      }
    }

    for (const ambient of sourceAmbientPatterns) {
      if (ambient.pattern.test(implementation) && !entry.ambientAuthority?.includes(ambient.name as "cwd" | "async_local_storage")) {
        violations.push(violation(
          "source_ambient_authority",
          entry.id,
          `Source selects authority through ambient ${ambient.name}.`,
        ))
      }
    }

    if (/\b(?:Effect\.runPromise|ManagedRuntime(?:\.make)?)\b/u.test(implementation) && entry.runtimeExitPerimeter !== true) {
      violations.push(violation(
        "internal_run_promise_escape",
        entry.id,
        "Source contains an Effect runtime exit outside its named perimeter.",
      ))
    }
  }

  return violations
}

export const assertValidDesktopServiceSourceCoupling = (
  sources: DesktopServiceSourceSet,
  entries: ReadonlyArray<DesktopServiceTopologyEntry> = desktopServiceTopology,
): void => {
  const violations = validateDesktopServiceSourceCoupling(entries, sources)
  if (violations.length > 0) {
    throw new Error(violations.map(item => `${item.code}:${item.serviceId}:${item.detail}`).join("\n"))
  }
}

export const assertValidDesktopServiceTopology = (
  entries: ReadonlyArray<DesktopServiceTopologyEntry> = desktopServiceTopology,
): void => {
  const violations = validateDesktopServiceTopology(entries)
  if (violations.length > 0) {
    throw new Error(violations.map(item => `${item.code}:${item.serviceId}:${item.detail}`).join("\n"))
  }
}
