/**
 * Pure diagnostics report builder (CUT-24 criterion 4, #8704).
 *
 * Maps ALREADY-COLLECTED typed health inputs from each operability surface into
 * the public-safe `DiagnosticsReport`. Kept pure (no Electron, no IO) so it is
 * exhaustively unit-testable, including fault injection (a degraded provider or
 * a closed sync must produce the right level and NEVER leak a path, email,
 * token, or url). The host (`diagnostics-host.ts`) collects the inputs from the
 * live main-process surfaces and calls this.
 *
 * Structural privacy: this builder never reads a filesystem path, account
 * email, url, command, or prompt into a summary. Workspace uses git-state +
 * entry-count only (never `root`); provider uses ref + readiness only (never
 * email); extensions use counts only.
 */
import {
  DIAGNOSTICS_SCHEMA_ID,
  worstLevel,
  type DiagnosticsAction,
  type DiagnosticsLevel,
  type DiagnosticsReport,
  type DiagnosticsRow,
} from "./diagnostics-contract.ts"

// --- Input shapes (mirror the live sources; see the CUT-24 health audit) -----

export type ProviderHealthInput =
  | Readonly<{ state: "ok"; accounts: ReadonlyArray<{ ref: string; readiness: string }> }>
  | Readonly<{ state: "unavailable"; reason?: string }>

export type RuntimeGatewayHealthInput =
  | Readonly<{
      state: "present"
      lifecycle: "starting" | "ready" | "disposed"
      sessionPhase: string
      capabilities: ReadonlyArray<{ id: string; state: "available" | "unavailable" }>
    }>
  | Readonly<{ state: "absent" }>

export type SyncHealthInput =
  | Readonly<{ state: "local_ready" | "closed"; syncPhase: string; pendingMutationCount: number }>
  | Readonly<{ state: "unobserved" }>

export type WorkspaceHealthInput =
  | Readonly<{ state: "selected"; git: "clean" | "changed" | "unavailable"; entryCount: number }>
  | Readonly<{ state: "none" }>

/** Workspace-bounded PTY terminals (CUT-20, #8700). */
export type PtyHealthInput =
  | Readonly<{ state: "available"; sessionCount: number }>
  | Readonly<{ state: "unavailable"; reason: string }>

export type ExtensionsHealthInput =
  | Readonly<{ state: "ok"; enabledCount: number; totalCount: number; dropped: number }>
  | Readonly<{ state: "unavailable"; message?: string }>

export type DiagnosticsInputs = Readonly<{
  appVersion: string
  generatedAt: number
  provider: ProviderHealthInput
  runtimeGateway: RuntimeGatewayHealthInput
  sync: SyncHealthInput
  workspace: WorkspaceHealthInput
  pty: PtyHealthInput
  extensions: ExtensionsHealthInput
}>

const readinessLevel = (readiness: string): DiagnosticsLevel =>
  readiness === "ready" ? "ok" : readiness === "unknown" ? "unknown" : "degraded"

const providerRow = (input: ProviderHealthInput): DiagnosticsRow => {
  if (input.state === "unavailable") {
    return { domain: "provider", level: "unavailable", summary: "Provider accounts unavailable", refs: [], actions: ["reprobe_providers", "refresh"] }
  }
  const total = input.accounts.length
  const ready = input.accounts.filter((account) => account.readiness === "ready").length
  const level: DiagnosticsLevel = total === 0 ? "degraded" : ready === total ? "ok" : ready === 0 ? "unavailable" : "degraded"
  const refs = input.accounts.slice(0, 16).map((account) => account.ref).filter((ref) => /^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(ref))
  const summary = total === 0 ? "No provider accounts connected" : `${ready} of ${total} provider accounts ready`
  const actions: DiagnosticsAction[] = level === "ok" ? ["refresh"] : ["reprobe_providers", "refresh"]
  // account.readiness deliberately not surfaced beyond the aggregate — worst is degraded
  void readinessLevel
  return { domain: "provider", level, summary, refs, actions }
}

const runtimeGatewayRow = (input: RuntimeGatewayHealthInput): DiagnosticsRow => {
  if (input.state === "absent") {
    return { domain: "runtimeGateway", level: "unavailable", summary: "Runtime Gateway not started", refs: [], actions: ["restart_runtime"] }
  }
  const unavailable = input.capabilities.filter((capability) => capability.state === "unavailable")
  const level: DiagnosticsLevel =
    input.lifecycle === "disposed"
      ? "unavailable"
      : input.lifecycle === "starting"
        ? "degraded"
        : unavailable.length === 0
          ? "ok"
          : "degraded"
  const refs = [input.sessionPhase, ...unavailable.slice(0, 15).map((capability) => capability.id)].filter((ref) =>
    /^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(ref),
  )
  const summary =
    input.lifecycle !== "ready"
      ? `Runtime Gateway ${input.lifecycle}`
      : unavailable.length === 0
        ? `Runtime Gateway ready (${input.capabilities.length} capabilities)`
        : `Runtime Gateway ready, ${unavailable.length} capability degraded`
  const actions: DiagnosticsAction[] = level === "ok" ? ["refresh"] : ["restart_runtime", "refresh"]
  return { domain: "runtimeGateway", level, summary, refs, actions }
}

const syncRow = (input: SyncHealthInput): DiagnosticsRow => {
  if (input.state === "unobserved") {
    return { domain: "sync", level: "unknown", summary: "Sync status not yet observed", refs: [], actions: ["reconnect_sync", "refresh"] }
  }
  if (input.state === "closed") {
    return { domain: "sync", level: "unavailable", summary: "Sync session closed", refs: ["closed"], actions: ["reconnect_sync"] }
  }
  const level: DiagnosticsLevel =
    input.syncPhase === "live"
      ? "ok"
      : input.syncPhase === "denied" || input.syncPhase === "must_refetch"
        ? "degraded"
        : "unknown"
  const pending = input.pendingMutationCount
  const summary = `Sync ${input.syncPhase}${pending > 0 ? `, ${pending} pending` : ""}`
  const refs = [input.syncPhase].filter((ref) => /^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(ref))
  const actions: DiagnosticsAction[] = level === "ok" ? ["refresh"] : ["reconnect_sync", "refresh"]
  return { domain: "sync", level, summary, refs, actions }
}

const workspaceRow = (input: WorkspaceHealthInput): DiagnosticsRow => {
  if (input.state === "none") {
    return { domain: "workspace", level: "unknown", summary: "No workspace selected", refs: [], actions: ["refresh_workspace"] }
  }
  const level: DiagnosticsLevel = input.git === "unavailable" ? "degraded" : "ok"
  const summary =
    input.git === "unavailable"
      ? `Workspace active, git unavailable (${input.entryCount} entries)`
      : `Workspace active, git ${input.git} (${input.entryCount} entries)`
  return { domain: "workspace", level, summary, refs: [input.git], actions: level === "ok" ? ["refresh_workspace"] : ["refresh_workspace", "refresh"] }
}

const ptyRow = (input: PtyHealthInput): DiagnosticsRow => {
  if (input.state === "available") {
    return {
      domain: "pty",
      level: "ok",
      summary: input.sessionCount > 0 ? `Terminal ready, ${input.sessionCount} active session${input.sessionCount === 1 ? "" : "s"}` : "Terminal ready, no active sessions",
      refs: [],
      actions: ["refresh"],
    }
  }
  return {
    domain: "pty",
    level: "unavailable",
    summary: input.reason.length > 0 && input.reason.length <= 200 ? input.reason : "Terminal not configured",
    refs: [],
    actions: [],
  }
}

const extensionsRow = (input: ExtensionsHealthInput): DiagnosticsRow => {
  if (input.state === "unavailable") {
    return { domain: "extensions", level: "unavailable", summary: "MCP configuration unavailable", refs: [], actions: ["reload_extensions"] }
  }
  const level: DiagnosticsLevel = input.dropped > 0 ? "degraded" : "ok"
  const summary =
    input.dropped > 0
      ? `${input.enabledCount} of ${input.totalCount} MCP servers enabled, ${input.dropped} invalid dropped`
      : `${input.enabledCount} of ${input.totalCount} MCP servers enabled`
  return { domain: "extensions", level, summary, refs: [], actions: level === "ok" ? ["refresh"] : ["reload_extensions", "refresh"] }
}

/** Build the full public-safe report. Pure. */
export const buildDiagnosticsReport = (inputs: DiagnosticsInputs): DiagnosticsReport => {
  const rows: DiagnosticsRow[] = [
    providerRow(inputs.provider),
    runtimeGatewayRow(inputs.runtimeGateway),
    syncRow(inputs.sync),
    workspaceRow(inputs.workspace),
    ptyRow(inputs.pty),
    extensionsRow(inputs.extensions),
  ]
  return {
    schema: DIAGNOSTICS_SCHEMA_ID,
    generatedAt: inputs.generatedAt,
    appVersion: inputs.appVersion.slice(0, 40),
    overall: worstLevel(rows),
    rows,
  }
}
