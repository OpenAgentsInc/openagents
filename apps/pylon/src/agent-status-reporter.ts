export const PYLON_AGENT_STATUS_SCHEMA_VERSION =
  "openagents.pylon.agent_status.v1"

export type AgentStatusRunnerKind = "codex_sdk" | "claude_agent" | "local_command"

export type AgentStatusUsage = {
  inputTokens?: number
  cachedInputTokens?: number
  outputTokens?: number
  reasoningOutputTokens?: number
}

export type AgentStatusItem = {
  ordinal: number
  itemType:
    | "agent_message"
    | "command_execution"
    | "error"
    | "file_change"
    | "mcp_tool_call"
    | "reasoning"
    | "unknown"
    | "web_search"
  status?: string
  message?: string
  reasoningSummary?: string
  commandLabel?: string
  exitCode?: number
  outputBytes?: number
  changeCount?: number
  toolName?: string
}

export type AgentStatusReport = {
  assignmentRef: string
  leaseRef: string
  pylonRef: string
  runnerKind: AgentStatusRunnerKind
  runRef?: string
  sessionRef?: string
  workspaceRef?: string
  turnIndex: number
  observedAt?: string
  usage?: AgentStatusUsage
  items: ReadonlyArray<AgentStatusItem>
  rawEvents?: ReadonlyArray<Record<string, unknown>>
}

export type AgentStatusReporter = (report: AgentStatusReport) => Promise<void>

export const normalizeAgentStatusBaseUrl = (
  value: string | undefined,
): string | undefined => {
  const trimmed = value?.trim()
  if (trimmed === undefined || trimmed === "") return undefined
  return trimmed.replace(/\/$/, "")
}

export const nonNegativeInteger = (value: number | undefined): number => {
  if (typeof value !== "number" || !Number.isFinite(value)) return 0
  return Math.max(0, Math.trunc(value))
}

export const positiveInteger = (value: number): number =>
  Math.max(1, nonNegativeInteger(value))

export const boundedString = (
  value: string | undefined,
  max: number,
): string | undefined => {
  const trimmed = value?.trim()
  if (trimmed === undefined || trimmed === "") return undefined
  return trimmed.length > max ? trimmed.slice(0, max) : trimmed
}

export const encodeAgentStatusItems = (
  items: ReadonlyArray<AgentStatusItem>,
): ReadonlyArray<Record<string, unknown>> =>
  items.map(item => ({
    ordinal: positiveInteger(item.ordinal),
    itemType: item.itemType,
    ...(boundedString(item.status, 80) === undefined
      ? {}
      : { status: boundedString(item.status, 80) }),
    ...(boundedString(item.message, 64 * 1024) === undefined
      ? {}
      : { message: boundedString(item.message, 64 * 1024) }),
    ...(boundedString(item.reasoningSummary, 64 * 1024) === undefined
      ? {}
      : { reasoningSummary: boundedString(item.reasoningSummary, 64 * 1024) }),
    ...(boundedString(item.commandLabel, 120) === undefined
      ? {}
      : { commandLabel: boundedString(item.commandLabel, 120) }),
    ...(typeof item.exitCode === "number" ? { exitCode: item.exitCode } : {}),
    ...(typeof item.outputBytes === "number"
      ? { outputBytes: nonNegativeInteger(item.outputBytes) }
      : {}),
    ...(typeof item.changeCount === "number"
      ? { changeCount: nonNegativeInteger(item.changeCount) }
      : {}),
    ...(boundedString(item.toolName, 120) === undefined
      ? {}
      : { toolName: boundedString(item.toolName, 120) }),
  }))

export const encodeAgentStatusUsage = (
  usage: AgentStatusUsage | undefined,
): Record<string, number> | undefined => {
  if (usage === undefined) return undefined
  return {
    ...(usage.cachedInputTokens === undefined
      ? {}
      : { cachedInputTokens: nonNegativeInteger(usage.cachedInputTokens) }),
    ...(usage.inputTokens === undefined
      ? {}
      : { inputTokens: nonNegativeInteger(usage.inputTokens) }),
    ...(usage.outputTokens === undefined
      ? {}
      : { outputTokens: nonNegativeInteger(usage.outputTokens) }),
    ...(usage.reasoningOutputTokens === undefined
      ? {}
      : { reasoningOutputTokens: nonNegativeInteger(usage.reasoningOutputTokens) }),
  }
}

export const encodeAgentStatusReport = (
  report: AgentStatusReport,
): Record<string, unknown> => ({
  schemaVersion: PYLON_AGENT_STATUS_SCHEMA_VERSION,
  assignmentRef: report.assignmentRef,
  leaseRef: report.leaseRef,
  pylonRef: report.pylonRef,
  runnerKind: report.runnerKind,
  ...(report.runRef === undefined ? {} : { runRef: report.runRef }),
  ...(report.sessionRef === undefined ? {} : { sessionRef: report.sessionRef }),
  ...(report.workspaceRef === undefined ? {} : { workspaceRef: report.workspaceRef }),
  turnIndex: positiveInteger(report.turnIndex),
  ...(report.observedAt === undefined ? {} : { observedAt: report.observedAt }),
  ...(encodeAgentStatusUsage(report.usage) === undefined
    ? {}
    : { usage: encodeAgentStatusUsage(report.usage) }),
  items: encodeAgentStatusItems(report.items),
  ...(report.rawEvents === undefined ? {} : { rawEvents: report.rawEvents }),
})
