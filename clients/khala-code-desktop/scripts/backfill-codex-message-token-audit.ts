import { appendFile, mkdir, readdir, readFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { homedir } from "node:os"
import { dirname, join } from "node:path"

import {
  khalaCodeDesktopCodexMessageTokenAuditMessage,
  khalaCodeDesktopCodexTokenUsageEventRefs,
  type KhalaCodeDesktopCodexMessageTokenAuditRecord,
  type KhalaCodeDesktopCodexMessageTokenAuditUsageEvent,
  type KhalaCodeDesktopCodexTokenUsageCounts,
} from "../src/bun/codex-token-usage-telemetry"
import type { KhalaCodeDesktopMessage } from "../src/shared/rpc"

type JsonObject = Record<string, unknown>

type HistoricalTurn = {
  assistantMessages: KhalaCodeDesktopMessage[]
  clientUserMessage: KhalaCodeDesktopMessage | null
  completedAt: string
  model: string
  submittedAt: string
  tokenUsageSequence: number
  turnId: string
  usage: KhalaCodeDesktopCodexTokenUsageCounts
  usageEvents: KhalaCodeDesktopCodexMessageTokenAuditUsageEvent[]
}

type HistoricalSession = {
  filePath: string
  sessionId: string
  turns: Map<string, HistoricalTurn>
}

type BackfillConfig = {
  aggregateBackfillEventIds: readonly string[]
  aggregateBackfillIdempotencyKeys: readonly string[]
  dryRun: boolean
  excludeTurnIds: ReadonlySet<string>
  outputPath: string
  sessionsRoot: string
}

const emptyUsage = (): KhalaCodeDesktopCodexTokenUsageCounts => ({
  cachedInputTokens: 0,
  inputTokens: 0,
  outputTokens: 0,
  reasoningOutputTokens: 0,
  totalTokens: 0,
})

const addUsage = (
  left: KhalaCodeDesktopCodexTokenUsageCounts,
  right: KhalaCodeDesktopCodexTokenUsageCounts,
): KhalaCodeDesktopCodexTokenUsageCounts => ({
  cachedInputTokens: left.cachedInputTokens + right.cachedInputTokens,
  inputTokens: left.inputTokens + right.inputTokens,
  outputTokens: left.outputTokens + right.outputTokens,
  reasoningOutputTokens: left.reasoningOutputTokens + right.reasoningOutputTokens,
  totalTokens: left.totalTokens + right.totalTokens,
})

const hasUsage = (usage: KhalaCodeDesktopCodexTokenUsageCounts): boolean =>
  usage.inputTokens > 0 ||
  usage.outputTokens > 0 ||
  usage.reasoningOutputTokens > 0 ||
  usage.totalTokens > 0

const isObject = (value: unknown): value is JsonObject =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const objectField = (value: unknown, field: string): JsonObject | null => {
  if (!isObject(value)) return null
  const candidate = value[field]
  return isObject(candidate) ? candidate : null
}

const stringField = (value: unknown, field: string): string | null => {
  if (!isObject(value)) return null
  const candidate = value[field]
  return typeof candidate === "string" && candidate.length > 0 ? candidate : null
}

const numericUsage = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) && value > 0
    ? Math.trunc(value)
    : 0

const usageFromObject = (value: unknown): KhalaCodeDesktopCodexTokenUsageCounts | null => {
  if (!isObject(value)) return null
  const promptDetails = objectField(value, "prompt_tokens_details") ?? {}
  const completionDetails = objectField(value, "completion_tokens_details") ?? {}
  const inputTokens =
    numericUsage(value.input_tokens) +
    numericUsage(value.inputTokens) +
    numericUsage(value.prompt_tokens) +
    numericUsage(value.promptTokens) +
    numericUsage(value.input)
  const outputTokens =
    numericUsage(value.output_tokens) +
    numericUsage(value.outputTokens) +
    numericUsage(value.completion_tokens) +
    numericUsage(value.completionTokens) +
    numericUsage(value.output)
  const reasoningOutputTokens =
    numericUsage(value.reasoning_output_tokens) +
    numericUsage(value.reasoningOutputTokens) +
    numericUsage(completionDetails.reasoning_tokens) +
    numericUsage(completionDetails.reasoningTokens)
  const cachedInputTokens =
    numericUsage(value.cached_input_tokens) +
    numericUsage(value.cachedInputTokens) +
    numericUsage(promptDetails.cached_tokens) +
    numericUsage(promptDetails.cachedTokens)
  const explicitTotal =
    numericUsage(value.total_tokens) +
    numericUsage(value.totalTokens)
  const usage = {
    cachedInputTokens,
    inputTokens,
    outputTokens,
    reasoningOutputTokens,
    totalTokens: explicitTotal > 0 ? explicitTotal : inputTokens + outputTokens,
  }
  return hasUsage(usage) ? usage : null
}

const textFromResponseMessage = (payload: JsonObject): string => {
  const content = payload.content
  if (!Array.isArray(content)) return ""
  return content
    .map(part => isObject(part) && typeof part.text === "string" ? part.text : "")
    .join("")
}

const defaultSessionsRoot = (): string => join(homedir(), ".codex", "sessions")

const defaultOutputPath = (): string =>
  join(homedir(), ".khala-code", "message-token-audit.jsonl")

const parseArgs = (): BackfillConfig => {
  const args = Bun.argv.slice(2)
  const valueAfter = (flag: string): string | null => {
    const index = args.indexOf(flag)
    if (index < 0) return null
    const value = args[index + 1]
    return value === undefined || value.startsWith("--") ? null : value
  }
  const valuesAfter = (flag: string): string[] => {
    const values: string[] = []
    for (let index = 0; index < args.length; index += 1) {
      if (args[index] !== flag) continue
      const value = args[index + 1]
      if (value === undefined || value.startsWith("--")) continue
      values.push(...value.split(",").map(item => item.trim()).filter(Boolean))
    }
    return values
  }
  const envList = (key: string): string[] =>
    (Bun.env[key]?.trim() ?? "")
      .split(",")
      .map(item => item.trim())
      .filter(Boolean)
  return {
    aggregateBackfillEventIds: [
      ...valuesAfter("--aggregate-backfill-event-id"),
      ...envList("KHALA_CODE_HISTORICAL_AGGREGATE_BACKFILL_EVENT_ID"),
    ],
    aggregateBackfillIdempotencyKeys: [
      ...valuesAfter("--aggregate-backfill-idempotency-key"),
      ...envList("KHALA_CODE_HISTORICAL_AGGREGATE_BACKFILL_IDEMPOTENCY_KEY"),
    ],
    dryRun: args.includes("--dry-run"),
    excludeTurnIds: new Set(valuesAfter("--exclude-turn-id")),
    outputPath: valueAfter("--output") ??
      Bun.env.KHALA_CODE_MESSAGE_TOKEN_AUDIT_LOCAL_LEDGER_PATH?.trim() ??
      defaultOutputPath(),
    sessionsRoot: valueAfter("--sessions-root") ??
      Bun.env.CODEX_SESSIONS_ROOT?.trim() ??
      defaultSessionsRoot(),
  }
}

const jsonlFiles = async (root: string): Promise<string[]> => {
  const entries = await readdir(root, { withFileTypes: true })
  const files: string[] = []
  for (const entry of entries) {
    const path = join(root, entry.name)
    if (entry.isDirectory()) {
      files.push(...await jsonlFiles(path))
    } else if (entry.isFile() && entry.name.endsWith(".jsonl")) {
      files.push(path)
    }
  }
  return files.sort()
}

const turnFor = (
  session: HistoricalSession,
  turnId: string,
  timestamp: string,
): HistoricalTurn => {
  const existing = session.turns.get(turnId)
  if (existing !== undefined) return existing
  const turn: HistoricalTurn = {
    assistantMessages: [],
    clientUserMessage: null,
    completedAt: timestamp,
    model: "openagents/codex-direct-local",
    submittedAt: timestamp,
    tokenUsageSequence: 0,
    turnId,
    usage: emptyUsage(),
    usageEvents: [],
  }
  session.turns.set(turnId, turn)
  return turn
}

const updateCompletedAt = (turn: HistoricalTurn, timestamp: string): void => {
  if (timestamp > turn.completedAt) turn.completedAt = timestamp
}

const parseHistoricalSession = async (
  filePath: string,
): Promise<HistoricalSession | null> => {
  const lines = (await readFile(filePath, "utf8")).split("\n").filter(Boolean)
  let sessionId: string | null = null
  let isKhalaCodeDesktop = false
  let currentTurnId: string | null = null
  const session: HistoricalSession = {
    filePath,
    sessionId: "unknown",
    turns: new Map(),
  }

  for (const line of lines) {
    let row: JsonObject
    try {
      row = JSON.parse(line) as JsonObject
    } catch {
      continue
    }
    const timestamp = stringField(row, "timestamp") ?? new Date().toISOString()
    const type = stringField(row, "type")
    const payload = objectField(row, "payload") ?? {}

    if (type === "session_meta") {
      const originator = stringField(payload, "originator")
      isKhalaCodeDesktop = originator === "khala_code_desktop"
      sessionId = stringField(payload, "session_id") ?? stringField(payload, "id")
      session.sessionId = sessionId ?? session.sessionId
      continue
    }
    if (!isKhalaCodeDesktop) continue

    if (type === "turn_context") {
      const turnId = stringField(payload, "turn_id")
      if (turnId !== null) {
        currentTurnId = turnId
        const turn = turnFor(session, turnId, timestamp)
        turn.model = stringField(payload, "model") ?? turn.model
        updateCompletedAt(turn, timestamp)
      }
      continue
    }

    const payloadType = stringField(payload, "type")
    if (type === "event_msg" && payloadType === "task_started") {
      const turnId = stringField(payload, "turn_id")
      if (turnId !== null) {
        currentTurnId = turnId
        updateCompletedAt(turnFor(session, turnId, timestamp), timestamp)
      }
      continue
    }

    const metadataTurnId = stringField(
      objectField(payload, "internal_chat_message_metadata_passthrough"),
      "turn_id",
    )
    if (metadataTurnId !== null) currentTurnId = metadataTurnId

    if (type === "event_msg" && payloadType === "user_message") {
      if (currentTurnId === null) continue
      const message = stringField(payload, "message")
      const clientId = stringField(payload, "client_id")
      if (message === null || clientId === null) continue
      const turn = turnFor(session, currentTurnId, timestamp)
      turn.clientUserMessage = {
        body: message,
        id: clientId,
        role: "user",
      }
      turn.submittedAt = timestamp
      updateCompletedAt(turn, timestamp)
      continue
    }

    if (type === "response_item" && stringField(payload, "type") === "message") {
      if (metadataTurnId === null) continue
      const body = textFromResponseMessage(payload)
      const id = stringField(payload, "id")
      const role = stringField(payload, "role")
      if (body.length === 0 || id === null || role !== "assistant") continue
      const turn = turnFor(session, metadataTurnId, timestamp)
      if (!turn.assistantMessages.some(message => message.id === id)) {
        turn.assistantMessages.push({ body, id, role: "assistant" })
      }
      updateCompletedAt(turn, timestamp)
      continue
    }

    if (type === "event_msg" && payloadType === "token_count") {
      if (currentTurnId === null) continue
      const lastUsage = usageFromObject(
        objectField(objectField(payload, "info"), "last_token_usage"),
      )
      if (lastUsage === null) continue
      const turn = turnFor(session, currentTurnId, timestamp)
      turn.tokenUsageSequence += 1
      turn.usage = addUsage(turn.usage, lastUsage)
      const report = {
        ...(turn.clientUserMessage === null
          ? {}
          : { clientUserMessageId: turn.clientUserMessage.id }),
        codexThreadId: session.sessionId,
        codexTurnId: currentTurnId,
        desktopSessionId: `khala-code-desktop-historical.${session.sessionId}`,
        desktopTurnId: `codex-history-${currentTurnId}`,
        model: turn.model,
        observedAt: timestamp,
        sequence: turn.tokenUsageSequence,
        turnStatus: "historical",
        usage: lastUsage,
      }
      const refs = khalaCodeDesktopCodexTokenUsageEventRefs(report)
      turn.usageEvents.push({
        eventId: refs.eventId,
        idempotencyKey: refs.idempotencyKey,
        observedAt: timestamp,
        sequence: turn.tokenUsageSequence,
        usage: lastUsage,
      })
      updateCompletedAt(turn, timestamp)
    }
  }

  return isKhalaCodeDesktop && sessionId !== null ? session : null
}

const existingCodexTurnIds = async (outputPath: string): Promise<Set<string>> => {
  if (!existsSync(outputPath)) return new Set()
  const ids = new Set<string>()
  for (const line of (await readFile(outputPath, "utf8")).split("\n")) {
    if (line.trim().length === 0) continue
    try {
      const row = JSON.parse(line) as JsonObject
      const record = objectField(row, "record")
      const codexTurnId = stringField(record, "codexTurnId")
      if (codexTurnId !== null) ids.add(codexTurnId)
    } catch {
      continue
    }
  }
  return ids
}

const auditRecordForTurn = (
  session: HistoricalSession,
  turn: HistoricalTurn,
  config: BackfillConfig,
): KhalaCodeDesktopCodexMessageTokenAuditRecord | null => {
  if (turn.clientUserMessage === null) return null
  const hasGlobalBackfill = config.aggregateBackfillEventIds.length > 0
  return {
    clientUserMessage: khalaCodeDesktopCodexMessageTokenAuditMessage(
      turn.clientUserMessage,
      "khala_code_client",
    ),
    codexMessages: turn.assistantMessages.map(message =>
      khalaCodeDesktopCodexMessageTokenAuditMessage(message, "codex_app_server")
    ),
    codexThreadId: session.sessionId,
    codexTurnId: turn.turnId,
    completedAt: turn.completedAt,
    desktopSessionId: `khala-code-desktop-historical.${session.sessionId}`,
    desktopTurnId: `codex-history-${turn.turnId}`,
    model: turn.model,
    reconciliation: {
      ...(config.aggregateBackfillEventIds.length === 0
        ? {}
        : {
          aggregateBackfillEventId: config.aggregateBackfillEventIds[0],
          aggregateBackfillEventIds: config.aggregateBackfillEventIds,
        }),
      ...(config.aggregateBackfillIdempotencyKeys.length === 0
        ? {}
        : {
          aggregateBackfillIdempotencyKey: config.aggregateBackfillIdempotencyKeys[0],
          aggregateBackfillIdempotencyKeys: config.aggregateBackfillIdempotencyKeys,
        }),
      globalCountedTokens: turn.usage.inputTokens + turn.usage.outputTokens,
      globalCounterRoute: "/api/stats/token-usage/events",
      status: hasGlobalBackfill
        ? "global_count_backfilled_aggregate"
        : turn.usageEvents.length > 0
          ? "missing_token_usage_update"
          : "missing_token_usage_update",
      tokenAccountingRequired: true,
      tokenScope: "codex_turn_provider_reported",
      usageTruth: "exact",
    },
    submittedAt: turn.submittedAt,
    turnStatus: "historical",
    usage: turn.usage,
    usageEvents: turn.usageEvents,
  }
}

const appendJsonLine = async (path: string, value: unknown): Promise<void> => {
  await mkdir(dirname(path), { recursive: true })
  await appendFile(path, `${JSON.stringify(value)}\n`, "utf8")
}

const main = async (): Promise<void> => {
  const config = parseArgs()
  const existing = await existingCodexTurnIds(config.outputPath)
  const files = await jsonlFiles(config.sessionsRoot)
  let scannedSessions = 0
  let candidateRecords = 0
  let writtenRecords = 0
  let skippedExisting = 0
  let totalUsage = emptyUsage()

  for (const file of files) {
    const session = await parseHistoricalSession(file)
    if (session === null) continue
    scannedSessions += 1
    for (const turn of session.turns.values()) {
      if (config.excludeTurnIds.has(turn.turnId)) continue
      const record = auditRecordForTurn(session, turn, config)
      if (record === null) continue
      candidateRecords += 1
      if (existing.has(record.codexTurnId ?? "")) {
        skippedExisting += 1
        continue
      }
      totalUsage = addUsage(totalUsage, record.usage)
      if (!config.dryRun) {
        await appendJsonLine(config.outputPath, {
          schemaVersion: "khala-code-desktop.codex-message-token-audit.local.v1",
          recordedAt: new Date().toISOString(),
          record,
          source: {
            filePath: session.filePath,
            reconstruction: "codex_session_jsonl",
          },
        })
      }
      existing.add(record.codexTurnId ?? "")
      writtenRecords += 1
    }
  }

  console.log(JSON.stringify({
    candidateRecords,
    dryRun: config.dryRun,
    globalCountedTokens: totalUsage.inputTokens + totalUsage.outputTokens,
    outputPath: config.outputPath,
    scannedSessions,
    skippedExisting,
    totalUsage,
    writtenRecords,
  }, null, 2))
}

await main()
