// #6388/#6391: the Claude own-capacity coding lane's turn reporter. It posts the
// exact, cumulative Claude Agent SDK token usage for a completed assignment turn
// to the registered-agent ingest route `/api/pylon/claude/turns`, which records
// the own-capacity `token_usage_events` row (provider `pylon-claude-own-capacity`,
// model `openagents/pylon-claude`). Mirrors the Codex turn reporter; usage only,
// no raw events on this lane yet. Public-safe + fail-soft by design — the caller
// must never let a reporter failure abort the local coding task.

export const PYLON_CLAUDE_TURN_INGEST_PATH = "/api/pylon/claude/turns"
export const PYLON_CLAUDE_TURN_SCHEMA_VERSION = "openagents.pylon.claude_turn.v1"

export type ClaudeTurnUsage = {
  inputTokens: number
  cachedInputTokens?: number
  outputTokens: number
  reasoningOutputTokens?: number
}

export type ClaudeTurnReport = {
  assignmentRef: string
  leaseRef: string
  pylonRef: string
  runRef?: string
  sessionRef?: string
  workspaceRef?: string
  turnIndex: number
  observedAt?: string
  usage: ClaudeTurnUsage
}

export type ClaudeTurnReporter = (report: ClaudeTurnReport) => Promise<void>

const normalizeBaseUrl = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim()
  if (trimmed === undefined || trimmed === "") return undefined
  return trimmed.replace(/\/$/, "")
}

const nonNegativeInteger = (value: number): number =>
  Number.isFinite(value) ? Math.max(0, Math.trunc(value)) : 0

const positiveInteger = (value: number): number =>
  Math.max(1, nonNegativeInteger(value))

export function createPylonClaudeTurnReporter(input: {
  agentToken?: string
  baseUrl?: string
  fetch?: typeof fetch
}): ClaudeTurnReporter | undefined {
  const baseUrl = normalizeBaseUrl(input.baseUrl)
  const agentToken = input.agentToken?.trim()
  if (baseUrl === undefined || agentToken === undefined || agentToken === "") {
    return undefined
  }

  const fetchImpl = input.fetch ?? fetch
  return async report => {
    const turnIndex = positiveInteger(report.turnIndex)
    const body = {
      schemaVersion: PYLON_CLAUDE_TURN_SCHEMA_VERSION,
      assignmentRef: report.assignmentRef,
      leaseRef: report.leaseRef,
      pylonRef: report.pylonRef,
      ...(report.runRef === undefined ? {} : { runRef: report.runRef }),
      ...(report.sessionRef === undefined ? {} : { sessionRef: report.sessionRef }),
      ...(report.workspaceRef === undefined
        ? {}
        : { workspaceRef: report.workspaceRef }),
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
          : {
              reasoningOutputTokens: nonNegativeInteger(
                report.usage.reasoningOutputTokens,
              ),
            }),
      },
    }
    const response = await fetchImpl(
      new URL(PYLON_CLAUDE_TURN_INGEST_PATH, baseUrl),
      {
        method: "POST",
        headers: {
          authorization: `Bearer ${agentToken}`,
          "content-type": "application/json",
          "Idempotency-Key": [
            "pylon.claude.turn",
            report.pylonRef,
            report.assignmentRef,
            report.sessionRef ?? "session.pending",
            String(turnIndex),
          ].join("."),
        },
        body: JSON.stringify(body),
      },
    )

    if (!response.ok) {
      throw new Error(`Pylon Claude turn ingest failed (${response.status})`)
    }
  }
}
