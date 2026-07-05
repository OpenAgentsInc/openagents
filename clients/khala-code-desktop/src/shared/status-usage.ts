import type {
  KhalaCodeDesktopMessage,
  KhalaCodeDesktopRuntimeStatus,
  KhalaCodeDesktopThreadTokenSummary,
} from "./rpc"
import { redactKhalaCodeDesktopDiagnosticsText } from "./diagnostics-redaction"

export type KhalaCodeStatusErrorKind =
  | "generic_failure"
  | "local_server_unavailable"
  | "model_unavailable"
  | "provider_auth"
  | "quota_or_rate_limit"

export type KhalaCodeProviderErrorProjection = Readonly<{
  kind: KhalaCodeStatusErrorKind
  title: string
  detail: string
  retryable: boolean
  settingsEntryPoint: "models" | "provider" | "server" | "usage" | null
}>

export type KhalaCodeTimelineMetrics = Readonly<{
  messageCount: number
  toolCallCount: number
  userMessageCount: number
  assistantMessageCount: number
  estimatedVirtualizationUseful: boolean
  anchorIds: readonly string[]
}>

export type KhalaCodeUsageBreakdown = Readonly<{
  totalTokens: number
  leaderboardSyncedTokens: number
  pendingSyncTokens: number
  auditRows: number
  usageEventRows: number
  missingUsageTurns: number
  codexStateTokens: number
  available: boolean
  status: "empty" | "exact" | "needs_attention" | "remote_disabled"
}>

export type KhalaCodeRuntimeStatusSummary = Readonly<{
  readyCount: number
  degradedCount: number
  unavailableCount: number
  rows: readonly {
    readonly id: string
    readonly label: string
    readonly state: "degraded" | "ready" | "unavailable"
    readonly detail: string
    readonly retryable: boolean
  }[]
}>

export type KhalaCodeStatusUsageProjection = Readonly<{
  timeline: KhalaCodeTimelineMetrics
  usage: KhalaCodeUsageBreakdown
  runtime: KhalaCodeRuntimeStatusSummary
  errors: readonly KhalaCodeProviderErrorProjection[]
}>

const limitedDetail = (value: string): string =>
  redactKhalaCodeDesktopDiagnosticsText(
    value.length <= 180 ? value : `${value.slice(0, 177)}...`,
  )

export const classifyKhalaCodeProviderError = (
  input: unknown,
): KhalaCodeProviderErrorProjection => {
  const message = input instanceof Error ? input.message : String(input)
  const normalized = message.toLowerCase()
  if (/auth|unauthori[sz]ed|login|api key|credential|bearer|sk-[a-z0-9_-]+/.test(normalized)) {
    return {
      kind: "provider_auth",
      title: "Provider authentication required",
      detail: limitedDetail(message),
      retryable: true,
      settingsEntryPoint: "provider",
    }
  }
  if (/model.*(not found|missing|unavailable)|unknown model|model_not_found/.test(normalized)) {
    return {
      kind: "model_unavailable",
      title: "Model unavailable",
      detail: limitedDetail(message),
      retryable: true,
      settingsEntryPoint: "models",
    }
  }
  if (/quota|rate.?limit|usage exceeded|too many requests|429/.test(normalized)) {
    return {
      kind: "quota_or_rate_limit",
      title: "Usage or rate limit reached",
      detail: limitedDetail(message),
      retryable: true,
      settingsEntryPoint: "usage",
    }
  }
  if (/server unavailable|connection refused|econnrefused|socket|app-server|pylon unavailable/.test(normalized)) {
    return {
      kind: "local_server_unavailable",
      title: "Local server unavailable",
      detail: limitedDetail(message),
      retryable: true,
      settingsEntryPoint: "server",
    }
  }
  return {
    kind: "generic_failure",
    title: "Turn failed",
    detail: limitedDetail(message),
    retryable: false,
    settingsEntryPoint: null,
  }
}

export const projectKhalaCodeTimelineMetrics = (
  messages: readonly KhalaCodeDesktopMessage[],
): KhalaCodeTimelineMetrics => ({
  messageCount: messages.length,
  toolCallCount: messages.filter(message => message.codexItem !== undefined).length,
  userMessageCount: messages.filter(message => message.role === "user").length,
  assistantMessageCount: messages.filter(message => message.role === "assistant").length,
  estimatedVirtualizationUseful: messages.length >= 250,
  anchorIds: messages.map(message => message.id).slice(-12),
})

export const projectKhalaCodeUsageBreakdown = (
  summary: KhalaCodeDesktopThreadTokenSummary,
): KhalaCodeUsageBreakdown => {
  const needsAttention = summary.missingUsageTurns > 0 || !summary.ok
  return {
    totalTokens: summary.totalTokens,
    leaderboardSyncedTokens: summary.leaderboardSyncedTokens,
    pendingSyncTokens: summary.pendingSyncTokens,
    auditRows: summary.auditRows,
    usageEventRows: summary.usageEventRows,
    missingUsageTurns: summary.missingUsageTurns,
    codexStateTokens: summary.codexStateTokens,
    available: summary.threadId !== null,
    status: summary.remoteDisabled
      ? "remote_disabled"
      : needsAttention
        ? "needs_attention"
        : summary.totalTokens === 0 && summary.codexStateTokens === 0
          ? "empty"
          : "exact",
  }
}

export const projectKhalaCodeRuntimeStatusSummary = (
  input: {
    readonly bootDegradedStates?: readonly {
      readonly detail: string
      readonly method: string
      readonly recoverable: boolean
      readonly state: "degraded"
    }[]
    readonly runtimeStatuses?: readonly KhalaCodeDesktopRuntimeStatus[]
  },
): KhalaCodeRuntimeStatusSummary => {
  const rows: KhalaCodeRuntimeStatusSummary["rows"][number][] = []
  for (const status of input.runtimeStatuses ?? []) {
    rows.push({
      id: `runtime:${status.capability}`,
      label: status.capability.replace(/_/g, " "),
      state: status.status === "ready" ? "ready" : "unavailable",
      detail: status.reason,
      retryable: status.status !== "ready",
    })
  }
  for (const state of input.bootDegradedStates ?? []) {
    rows.push({
      id: `boot:${state.method}`,
      label: state.method,
      state: "degraded",
      detail: state.detail,
      retryable: state.recoverable,
    })
  }
  return {
    readyCount: rows.filter(row => row.state === "ready").length,
    degradedCount: rows.filter(row => row.state === "degraded").length,
    unavailableCount: rows.filter(row => row.state === "unavailable").length,
    rows,
  }
}

export const projectKhalaCodeStatusUsage = (
  input: {
    readonly bootDegradedStates?: Parameters<typeof projectKhalaCodeRuntimeStatusSummary>[0]["bootDegradedStates"]
    readonly messages: readonly KhalaCodeDesktopMessage[]
    readonly runtimeStatuses?: readonly KhalaCodeDesktopRuntimeStatus[]
    readonly threadTokenSummary: KhalaCodeDesktopThreadTokenSummary
    readonly turnErrors?: readonly unknown[]
  },
): KhalaCodeStatusUsageProjection => ({
  timeline: projectKhalaCodeTimelineMetrics(input.messages),
  usage: projectKhalaCodeUsageBreakdown(input.threadTokenSummary),
  runtime: projectKhalaCodeRuntimeStatusSummary({
    ...(input.bootDegradedStates === undefined ? {} : { bootDegradedStates: input.bootDegradedStates }),
    ...(input.runtimeStatuses === undefined ? {} : { runtimeStatuses: input.runtimeStatuses }),
  }),
  errors: (input.turnErrors ?? []).map(classifyKhalaCodeProviderError),
})
