export const PYLON_CODEX_TURN_INGEST_PATH = "/api/pylon/codex/turns"
export const PYLON_CODEX_EVENT_CHUNK_INGEST_PATH = "/api/pylon/codex/event-chunks"
export const PYLON_CODEX_TURN_SCHEMA_VERSION = "openagents.pylon.codex_turn.v1"
export const PYLON_CODEX_EVENT_CHUNK_SCHEMA_VERSION = "openagents.pylon.codex_event_chunk.v1"

export type CodexTurnUsage = {
  inputTokens: number
  cachedInputTokens?: number
  outputTokens: number
  reasoningOutputTokens?: number
}

export type CodexTurnReportItem = {
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

export type CodexTurnReport = {
  assignmentRef: string
  leaseRef: string
  pylonRef: string
  runRef?: string
  sessionRef?: string
  workspaceRef?: string
  turnIndex: number
  observedAt?: string
  usage: CodexTurnUsage
  items: ReadonlyArray<CodexTurnReportItem>
  rawEvents?: ReadonlyArray<Record<string, unknown>>
}

export type CodexTurnReporter = (report: CodexTurnReport) => Promise<void>

export type CodexEventChunkReport = {
  assignmentRef: string
  leaseRef: string
  pylonRef: string
  runRef?: string
  sessionRef?: string
  workspaceRef?: string
  turnIndex: number
  chunkIndex: number
  observedAt?: string
  rawEvents: ReadonlyArray<Record<string, unknown>>
  items?: ReadonlyArray<CodexTurnReportItem>
}

export type CodexEventChunkReporter = (report: CodexEventChunkReport) => Promise<void>

const normalizeBaseUrl = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim()
  if (trimmed === undefined || trimmed === "") return undefined
  return trimmed.replace(/\/$/, "")
}

const nonNegativeInteger = (value: number): number => {
  if (!Number.isFinite(value)) return 0
  return Math.max(0, Math.trunc(value))
}

const positiveInteger = (value: number): number => Math.max(1, nonNegativeInteger(value))

const boundedString = (value: string | undefined, max: number): string | undefined => {
  const trimmed = value?.trim()
  if (trimmed === undefined || trimmed === "") return undefined
  return trimmed.length > max ? trimmed.slice(0, max) : trimmed
}

export function createPylonCodexTurnReporter(input: {
  agentToken?: string
  baseUrl?: string
  fetch?: typeof fetch
}): CodexTurnReporter | undefined {
  const baseUrl = normalizeBaseUrl(input.baseUrl)
  const agentToken = input.agentToken?.trim()
  if (baseUrl === undefined || agentToken === undefined || agentToken === "") {
    return undefined
  }

  const fetchImpl = input.fetch ?? fetch
  return async report => {
    const turnIndex = positiveInteger(report.turnIndex)
    const body = {
      schemaVersion: PYLON_CODEX_TURN_SCHEMA_VERSION,
      assignmentRef: report.assignmentRef,
      leaseRef: report.leaseRef,
      pylonRef: report.pylonRef,
      ...(report.runRef === undefined ? {} : { runRef: report.runRef }),
      ...(report.sessionRef === undefined ? {} : { sessionRef: report.sessionRef }),
      ...(report.workspaceRef === undefined ? {} : { workspaceRef: report.workspaceRef }),
      turnIndex,
      ...(report.observedAt === undefined ? {} : { observedAt: report.observedAt }),
      usage: {
        inputTokens: nonNegativeInteger(report.usage.inputTokens),
        ...(report.usage.cachedInputTokens === undefined
          ? {}
          : { cachedInputTokens: nonNegativeInteger(report.usage.cachedInputTokens) }),
        outputTokens: nonNegativeInteger(report.usage.outputTokens),
        ...(report.usage.reasoningOutputTokens === undefined
          ? {}
          : { reasoningOutputTokens: nonNegativeInteger(report.usage.reasoningOutputTokens) }),
      },
      items: report.items.map(item => ({
        ordinal: positiveInteger(item.ordinal),
        itemType: item.itemType,
        ...(boundedString(item.status, 80) === undefined ? {} : { status: boundedString(item.status, 80) }),
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
        ...(typeof item.outputBytes === "number" ? { outputBytes: nonNegativeInteger(item.outputBytes) } : {}),
        ...(typeof item.changeCount === "number" ? { changeCount: nonNegativeInteger(item.changeCount) } : {}),
        ...(boundedString(item.toolName, 120) === undefined ? {} : { toolName: boundedString(item.toolName, 120) }),
      })),
      ...(report.rawEvents === undefined ? {} : { rawEvents: report.rawEvents }),
    }
    const response = await fetchImpl(new URL(PYLON_CODEX_TURN_INGEST_PATH, baseUrl), {
      method: "POST",
      headers: {
        authorization: `Bearer ${agentToken}`,
        "content-type": "application/json",
        "Idempotency-Key": [
          "pylon.codex.turn",
          report.pylonRef,
          report.assignmentRef,
          report.sessionRef ?? "session.pending",
          String(turnIndex),
        ].join("."),
      },
      body: JSON.stringify(body),
    })

    if (!response.ok) {
      throw new Error(`Pylon Codex turn ingest failed (${response.status})`)
    }
  }
}

export function createPylonCodexEventChunkReporter(input: {
  agentToken?: string
  baseUrl?: string
  fetch?: typeof fetch
}): CodexEventChunkReporter | undefined {
  const baseUrl = normalizeBaseUrl(input.baseUrl)
  const agentToken = input.agentToken?.trim()
  if (baseUrl === undefined || agentToken === undefined || agentToken === "") {
    return undefined
  }

  const fetchImpl = input.fetch ?? fetch
  return async report => {
    if (report.rawEvents.length === 0) return
    const turnIndex = positiveInteger(report.turnIndex)
    const chunkIndex = positiveInteger(report.chunkIndex)
    const body = {
      schemaVersion: PYLON_CODEX_EVENT_CHUNK_SCHEMA_VERSION,
      assignmentRef: report.assignmentRef,
      leaseRef: report.leaseRef,
      pylonRef: report.pylonRef,
      ...(report.runRef === undefined ? {} : { runRef: report.runRef }),
      ...(report.sessionRef === undefined ? {} : { sessionRef: report.sessionRef }),
      ...(report.workspaceRef === undefined ? {} : { workspaceRef: report.workspaceRef }),
      turnIndex,
      chunkIndex,
      ...(report.observedAt === undefined ? {} : { observedAt: report.observedAt }),
      rawEvents: report.rawEvents,
      ...(report.items === undefined
        ? {}
        : {
            items: report.items.map(item => ({
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
              ...(typeof item.outputBytes === "number" ? { outputBytes: nonNegativeInteger(item.outputBytes) } : {}),
              ...(typeof item.changeCount === "number" ? { changeCount: nonNegativeInteger(item.changeCount) } : {}),
              ...(boundedString(item.toolName, 120) === undefined
                ? {}
                : { toolName: boundedString(item.toolName, 120) }),
            })),
          }),
    }
    const response = await fetchImpl(new URL(PYLON_CODEX_EVENT_CHUNK_INGEST_PATH, baseUrl), {
      method: "POST",
      headers: {
        authorization: `Bearer ${agentToken}`,
        "content-type": "application/json",
        "Idempotency-Key": [
          "pylon.codex.event-chunk",
          report.pylonRef,
          report.assignmentRef,
          report.sessionRef ?? "session.pending",
          String(turnIndex),
          String(chunkIndex),
        ].join("."),
      },
      body: JSON.stringify(body),
    })

    if (!response.ok) {
      throw new Error(`Pylon Codex event chunk ingest failed (${response.status})`)
    }
  }
}
