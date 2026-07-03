import {
  encodeAgentStatusItems,
  encodeAgentStatusReport,
  nonNegativeInteger,
  normalizeAgentStatusBaseUrl,
  positiveInteger,
  type AgentStatusItem,
} from "./agent-status-reporter.js"

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

export type CodexTurnReportItem = AgentStatusItem

export type CodexTurnReport = {
  assignmentRef: string
  leaseRef: string
  pylonRef: string
  roleRef?: string
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

export function createPylonCodexTurnReporter(input: {
  agentToken?: string
  baseUrl?: string
  fetch?: typeof fetch
}): CodexTurnReporter | undefined {
  const baseUrl = normalizeAgentStatusBaseUrl(input.baseUrl)
  const agentToken = input.agentToken?.trim()
  if (baseUrl === undefined || agentToken === undefined || agentToken === "") {
    return undefined
  }

  const fetchImpl = input.fetch ?? fetch
  return async report => {
    const turnIndex = positiveInteger(report.turnIndex)
    const body = {
      ...encodeAgentStatusReport({
        ...report,
        runnerKind: "codex_sdk",
        turnIndex,
      }),
      schemaVersion: PYLON_CODEX_TURN_SCHEMA_VERSION,
      roleRef: report.roleRef?.trim() || "coder",
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
  const baseUrl = normalizeAgentStatusBaseUrl(input.baseUrl)
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
            items: encodeAgentStatusItems(report.items),
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
