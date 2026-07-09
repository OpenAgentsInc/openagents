export const PYLON_AGENT_STATUS_SCHEMA_VERSION =
  "openagents.pylon.agent_status.v1"
export const PYLON_AGENT_RUNNER_STATUS_EVENT_SCHEMA_VERSION =
  "openagents.pylon.agent_runner_status_event.v1"
export const PYLON_AGENT_RUNNER_CONTROL_COMMAND_SCHEMA_VERSION =
  "openagents.pylon.agent_runner_control_command.v1"

export type AgentStatusRunnerKind = "codex_sdk" | "claude_agent" | "local_command"

export type AgentRunnerNeutralState =
  | "idle"
  | "queued"
  | "working"
  | "waiting"
  | "blocked"
  | "done"
  | "failed"
  | "offline"

export type AgentRunnerControlVerb =
  | "status.list"
  | "task.list"
  | "task.update"
  | "task.dispatch"
  | "dispatch.cancel"

export const PYLON_AGENT_RUNNER_CONTROL_VERBS: ReadonlyArray<AgentRunnerControlVerb> = [
  "status.list",
  "task.list",
  "task.update",
  "task.dispatch",
  "dispatch.cancel",
]

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

export type AgentRunnerStatusHistoryEntry = {
  state: AgentRunnerNeutralState
  stateStartedAt: string
}

export type AgentRunnerStatusEvent = {
  eventRef: string
  runnerRef: string
  runnerKind: string
  state: AgentRunnerNeutralState
  stateStartedAt: string
  updatedAt: string
  assignmentRef?: string
  taskId?: string
  dispatchContextId?: string
  assigneeHandle?: string
  pylonRef?: string
  worktreeId?: string
  worktreeRef?: string
  capabilityRefs?: ReadonlyArray<string>
  supportedControlVerbs?: ReadonlyArray<AgentRunnerControlVerb>
  refs?: ReadonlyArray<string>
  blockerRefs?: ReadonlyArray<string>
  stateHistory?: ReadonlyArray<AgentRunnerStatusHistoryEntry>
}

export type AgentRunnerControlCommand = {
  commandRef: string
  verb: AgentRunnerControlVerb
  issuedAt: string
  target: {
    runnerRef?: string
    runnerKind?: string
    taskId?: string
    dispatchContextId?: string
    groupAddress?: string
  }
  payload?: {
    status?: AgentRunnerNeutralState
    title?: string
    prompt?: string
    result?: string
    reason?: string
    refs?: ReadonlyArray<string>
  }
}

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

export const boundedStringArray = (
  values: ReadonlyArray<string> | undefined,
  maxItems: number,
  maxLength: number,
): string[] =>
  [...(values ?? [])]
    .map(value => boundedString(value, maxLength))
    .filter((value): value is string =>
      value !== undefined &&
      !value.startsWith("/") &&
      !/^[A-Za-z]:[\\/]/.test(value),
    )
    .slice(0, maxItems)

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

export const encodeAgentRunnerStatusEvent = (
  event: AgentRunnerStatusEvent,
): Record<string, unknown> => ({
  schemaVersion: PYLON_AGENT_RUNNER_STATUS_EVENT_SCHEMA_VERSION,
  eventRef: event.eventRef,
  runnerRef: event.runnerRef,
  runnerKind: event.runnerKind,
  state: event.state,
  stateStartedAt: event.stateStartedAt,
  updatedAt: event.updatedAt,
  ...(event.assignmentRef === undefined ? {} : { assignmentRef: event.assignmentRef }),
  ...(event.taskId === undefined ? {} : { taskId: event.taskId }),
  ...(event.dispatchContextId === undefined ? {} : { dispatchContextId: event.dispatchContextId }),
  ...(event.assigneeHandle === undefined ? {} : { assigneeHandle: boundedString(event.assigneeHandle, 120) }),
  ...(event.pylonRef === undefined ? {} : { pylonRef: event.pylonRef }),
  ...(event.worktreeId === undefined ? {} : { worktreeId: boundedString(event.worktreeId, 160) }),
  ...(event.worktreeRef === undefined ? {} : { worktreeRef: event.worktreeRef }),
  ...(event.capabilityRefs === undefined ? {} : { capabilityRefs: boundedStringArray(event.capabilityRefs, 32, 160) }),
  ...(event.supportedControlVerbs === undefined
    ? {}
    : {
        supportedControlVerbs: event.supportedControlVerbs.filter((verb): verb is AgentRunnerControlVerb =>
          PYLON_AGENT_RUNNER_CONTROL_VERBS.includes(verb as AgentRunnerControlVerb),
        ),
      }),
  refs: boundedStringArray(event.refs, 64, 240),
  blockerRefs: boundedStringArray(event.blockerRefs, 64, 240),
  ...(event.stateHistory === undefined
    ? {}
    : {
        stateHistory: event.stateHistory.slice(-20).map(entry => ({
          state: entry.state,
          stateStartedAt: entry.stateStartedAt,
        })),
      }),
})

export const encodeAgentRunnerControlCommand = (
  command: AgentRunnerControlCommand,
): Record<string, unknown> => {
  if (!PYLON_AGENT_RUNNER_CONTROL_VERBS.includes(command.verb)) {
    throw new Error(`unsupported agent runner control verb: ${command.verb}`)
  }
  return {
    schemaVersion: PYLON_AGENT_RUNNER_CONTROL_COMMAND_SCHEMA_VERSION,
    commandRef: command.commandRef,
    verb: command.verb,
    issuedAt: command.issuedAt,
    target: {
      ...(command.target.runnerRef === undefined ? {} : { runnerRef: command.target.runnerRef }),
      ...(command.target.runnerKind === undefined ? {} : { runnerKind: command.target.runnerKind }),
      ...(command.target.taskId === undefined ? {} : { taskId: command.target.taskId }),
      ...(command.target.dispatchContextId === undefined ? {} : { dispatchContextId: command.target.dispatchContextId }),
      ...(command.target.groupAddress === undefined ? {} : { groupAddress: boundedString(command.target.groupAddress, 160) }),
    },
    ...(command.payload === undefined
      ? {}
      : {
          payload: {
            ...(command.payload.status === undefined ? {} : { status: command.payload.status }),
            ...(command.payload.title === undefined ? {} : { title: boundedString(command.payload.title, 240) }),
            ...(command.payload.prompt === undefined ? {} : { prompt: boundedString(command.payload.prompt, 4 * 1024) }),
            ...(command.payload.result === undefined ? {} : { result: boundedString(command.payload.result, 4 * 1024) }),
            ...(command.payload.reason === undefined ? {} : { reason: boundedString(command.payload.reason, 240) }),
            ...(command.payload.refs === undefined ? {} : { refs: boundedStringArray(command.payload.refs, 32, 240) }),
          },
        }),
  }
}
