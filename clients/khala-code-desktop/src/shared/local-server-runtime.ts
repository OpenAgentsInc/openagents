import type { KhalaCodeDesktopRuntimeStatus } from "./rpc"
import { redactKhalaCodeDesktopDiagnosticsText } from "./diagnostics-redaction"

export type KhalaCodeLocalServerRuntimeKind =
  | "ai_sdk_core"
  | "codex_app_server"
  | "khala_local_server"
  | "pylon"

export type KhalaCodeLocalServerRuntimeState =
  | "degraded"
  | "planned"
  | "ready"
  | "unavailable"

export type KhalaCodeLocalServerCapabilityId =
  | "auth"
  | "file_api"
  | "health"
  | "identity"
  | "lifecycle"
  | "origin_policy"
  | "permissions"
  | "project_routes"
  | "provider_models"
  | "review_api"
  | "session_routes"
  | "stream_events"
  | "terminal_api"
  | "tool_calls"

export type KhalaCodeLocalServerCapability = Readonly<{
  id: KhalaCodeLocalServerCapabilityId
  label: string
  required: boolean
}>

export type KhalaCodeLocalServerRuntimeRow = Readonly<{
  detail: string
  isDefault: boolean
  kind: KhalaCodeLocalServerRuntimeKind
  label: string
  reason: string
  state: KhalaCodeLocalServerRuntimeState
}>

export type KhalaCodeLocalServerManagerActionId =
  | "server.open_manager"
  | "server.refresh"
  | "server.restart_local"
  | "server.select_default"

export type KhalaCodeLocalServerManagerAction = Readonly<{
  commandId: KhalaCodeLocalServerManagerActionId
  enabled: boolean
  label: string
  reason: string
}>

export type KhalaCodeLocalServerContractProjection = Readonly<{
  capabilities: readonly KhalaCodeLocalServerCapability[]
  defaultRuntime: KhalaCodeLocalServerRuntimeKind
  ownershipBoundary: string
  rows: readonly KhalaCodeLocalServerRuntimeRow[]
  actions: readonly KhalaCodeLocalServerManagerAction[]
  credentialPolicy: string
}>

export const KHALA_CODE_LOCAL_SERVER_CAPABILITIES: readonly KhalaCodeLocalServerCapability[] = [
  { id: "health", label: "Health and readiness", required: true },
  { id: "auth", label: "Authentication and credential refusal", required: true },
  { id: "origin_policy", label: "Renderer origin/CORS policy", required: true },
  { id: "identity", label: "Server identity and version skew", required: true },
  { id: "project_routes", label: "Project routes", required: true },
  { id: "session_routes", label: "Session routes", required: true },
  { id: "provider_models", label: "Provider and model listing", required: true },
  { id: "stream_events", label: "AI SDK/OpenAgents stream events", required: true },
  { id: "tool_calls", label: "Tool calls and results", required: true },
  { id: "permissions", label: "OpenAgents permission authority", required: true },
  { id: "file_api", label: "File APIs", required: true },
  { id: "terminal_api", label: "Terminal APIs", required: true },
  { id: "review_api", label: "Review and diff APIs", required: true },
  { id: "lifecycle", label: "Start, stop, restart, recovery", required: true },
]

const statusByCapability = (
  statuses: readonly KhalaCodeDesktopRuntimeStatus[],
): Map<KhalaCodeDesktopRuntimeStatus["capability"], KhalaCodeDesktopRuntimeStatus> => {
  const map = new Map<KhalaCodeDesktopRuntimeStatus["capability"], KhalaCodeDesktopRuntimeStatus>()
  for (const status of statuses) map.set(status.capability, status)
  return map
}

const stateFromRuntimeStatus = (
  status: KhalaCodeDesktopRuntimeStatus | undefined,
): KhalaCodeLocalServerRuntimeState => {
  if (status === undefined) return "unavailable"
  if (status.status === "ready") return "ready"
  if (status.status === "not_configured") return "planned"
  return status.available ? "degraded" : "unavailable"
}

const safeDetail = (value: string): string =>
  redactKhalaCodeDesktopDiagnosticsText(value.length <= 220 ? value : `${value.slice(0, 217)}...`)

export const projectKhalaCodeLocalServerManager = (
  input: {
    readonly runtimeStatuses?: readonly KhalaCodeDesktopRuntimeStatus[]
  } = {},
): KhalaCodeLocalServerContractProjection => {
  const statuses = statusByCapability(input.runtimeStatuses ?? [])
  const pylon = statuses.get("pylon")
  const codexHarness = statuses.get("codex_harness")
  const coding = statuses.get("coding")
  const pylonState = stateFromRuntimeStatus(pylon)
  const codexState = stateFromRuntimeStatus(codexHarness)
  const codingState = stateFromRuntimeStatus(coding)
  const localState: KhalaCodeLocalServerRuntimeState =
    pylonState === "ready" || codingState === "ready" ? "planned" : "degraded"

  return {
    capabilities: KHALA_CODE_LOCAL_SERVER_CAPABILITIES,
    credentialPolicy:
      "Remote server credentials stay out of renderer logs, support bundles, command palette records, and public traces.",
    defaultRuntime: "khala_local_server",
    ownershipBoundary:
      "Khala owns the local server contract; it can be implemented inside Pylon or beside Pylon, while Codex app-server remains an important bridge for Codex threads and approvals.",
    rows: [
      {
        detail: "Contract defined for health, auth, projects, sessions, streams, tools, files, terminals, review, and lifecycle.",
        isDefault: true,
        kind: "khala_local_server",
        label: "Khala Local Server",
        reason:
          localState === "planned"
            ? "First desktop pass exposes the manager and contract before switching execution."
            : "Needs Pylon/coding runtime health before it can become executable.",
        state: localState,
      },
      {
        detail: safeDetail(pylon?.reason ?? "Pylon status has not reported yet."),
        isDefault: false,
        kind: "pylon",
        label: "Pylon Runtime",
        reason: "Candidate host for the Khala-owned local server boundary.",
        state: pylonState,
      },
      {
        detail: safeDetail(codexHarness?.reason ?? "Codex harness status has not reported yet."),
        isDefault: false,
        kind: "codex_app_server",
        label: "Codex App-Server Bridge",
        reason: "Current carrier for Codex threads and approvals, not the whole Khala server strategy.",
        state: codexState,
      },
      {
        detail:
          codingState === "ready"
            ? safeDetail(coding?.reason ?? "Coding runtime ready.")
            : "AI SDK Core lane is present as the stream/tool compatibility target for the local server.",
        isDefault: false,
        kind: "ai_sdk_core",
        label: "Khala AI SDK Core",
        reason: "Maps AI SDK stream parts into OpenAgents runtime events while OpenAgents keeps tool authority.",
        state: codingState === "ready" ? "ready" : "planned",
      },
    ],
    actions: [
      {
        commandId: "server.open_manager",
        enabled: true,
        label: "Open Manager",
        reason: "Available from Settings and the command palette.",
      },
      {
        commandId: "server.refresh",
        enabled: true,
        label: "Refresh Health",
        reason: "Reload Pylon, Codex harness, and coding runtime status.",
      },
      {
        commandId: "server.restart_local",
        enabled: false,
        label: "Restart Local Server",
        reason: "Enabled when the Khala-owned local server process is wired to a lifecycle controller.",
      },
      {
        commandId: "server.select_default",
        enabled: false,
        label: "Choose Default Server",
        reason: "Remote/default server selection waits for credential-safe storage and refusal tests.",
      },
    ],
  }
}
