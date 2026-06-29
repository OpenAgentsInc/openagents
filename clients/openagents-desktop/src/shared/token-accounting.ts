export const OPENAGENTS_DESKTOP_TOKEN_FAILURE_SPOOL_NAME =
  "codex-turn-report-failures.jsonl"

export const OPENAGENTS_DESKTOP_TOKEN_MISSING_ACCOUNTING_BLOCKER =
  "blocker.khala_proof.token_usage.rows_and_tokens_present"

export type TokenAccountingFailureReport = {
  readonly assignmentRef: string
  readonly error: string
  readonly observedAt: string | null
  readonly totalTokens: number
  readonly turnIndex: number | null
}

export type TokenAccountingSpool = {
  readonly byteLength: number
  readonly exists: boolean
  readonly lineCount: number
  readonly path: string
  readonly reports: readonly TokenAccountingFailureReport[]
}

export type TokenAccountingStatusResult = {
  readonly ok: true
  readonly observedAt: string
  readonly spool: TokenAccountingSpool
}

export type TokenAccountingReplayResult =
  | {
      readonly ok: true
      readonly archivedPath: string | null
      readonly observedAt: string
      readonly replayedReports: number
      readonly spool: TokenAccountingSpool
    }
  | {
      readonly ok: false
      readonly error: string
      readonly observedAt: string
      readonly replayedReports: number
      readonly spool: TokenAccountingSpool
    }

export type AssignmentTokenUsageVerification =
  | {
      readonly ok: true
      readonly assignmentRef: string
      readonly cacheReadTokens: number
      readonly demandKind: "own_capacity"
      readonly demandSource: "khala_coding_delegation"
      readonly inputTokens: number
      readonly model: "openagents/pylon-codex"
      readonly observedAt: string
      readonly outputTokens: number
      readonly provider: "pylon-codex-own-capacity"
      readonly reasoningTokens: number
      readonly rowCount: number
      readonly totalTokens: number
      readonly usageTruth: "exact"
    }
  | {
      readonly ok: false
      readonly assignmentRef: string
      readonly blockerRef:
        | typeof OPENAGENTS_DESKTOP_TOKEN_MISSING_ACCOUNTING_BLOCKER
        | "blocker.desktop_token_accounting.assignment_ref_missing"
        | "blocker.desktop_token_accounting.proof_unavailable"
      readonly error?: string
      readonly observedAt: string
    }

type JsonObject = Record<string, unknown>

const isJsonObject = (value: unknown): value is JsonObject =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const stringValue = (value: unknown): string | null =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : null

const numberValue = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value)
    ? Math.max(0, Math.trunc(value))
    : 0

const truncate = (value: string, maxLength: number): string =>
  value.length <= maxLength ? value : `${value.slice(0, maxLength - 3)}...`

export const isPublicSafeAssignmentRef = (value: string): boolean =>
  /^[A-Za-z0-9][A-Za-z0-9_.:-]{2,180}$/.test(value) &&
  !/[\\/]|(?:^|[._:-])(?:secret|token|auth|wallet|private)(?:[._:-]|$)/i.test(
    value,
  )

export const normalizeAssignmentRef = (value: string | null): string | null => {
  if (value === null) return null
  const trimmed = value.trim()
  return isPublicSafeAssignmentRef(trimmed) ? trimmed : null
}

export const parseTokenFailureSpool = (
  input: {
    readonly byteLength: number
    readonly path: string
    readonly text: string
  },
): TokenAccountingSpool => {
  const reports: TokenAccountingFailureReport[] = []
  let lineCount = 0

  for (const line of input.text.split("\n")) {
    if (line.trim() === "") continue
    lineCount += 1

    let parsed: unknown
    try {
      parsed = JSON.parse(line)
    } catch {
      continue
    }
    if (!isJsonObject(parsed)) continue

    const report = isJsonObject(parsed.report) ? parsed.report : null
    const usage = isJsonObject(report?.usage) ? report.usage : null
    const assignmentRef = normalizeAssignmentRef(
      stringValue(report?.assignmentRef),
    )
    if (assignmentRef === null) continue

    reports.push({
      assignmentRef,
      error: truncate(stringValue(parsed.error) ?? "report ingest failed", 160),
      observedAt: stringValue(parsed.observedAt) ?? stringValue(report?.observedAt),
      totalTokens:
        numberValue(usage?.inputTokens) +
        numberValue(usage?.outputTokens) +
        numberValue(usage?.reasoningOutputTokens),
      turnIndex:
        typeof report?.turnIndex === "number" && Number.isFinite(report.turnIndex)
          ? Math.trunc(report.turnIndex)
          : null,
    })
  }

  return {
    byteLength: Math.max(0, Math.trunc(input.byteLength)),
    exists: input.byteLength > 0 || input.text.trim() !== "",
    lineCount,
    path: input.path,
    reports,
  }
}

export const emptyTokenAccountingSpool = (path: string): TokenAccountingSpool => ({
  byteLength: 0,
  exists: false,
  lineCount: 0,
  path,
  reports: [],
})

export const assignmentRefsFromSpool = (
  spool: TokenAccountingSpool,
): readonly string[] => [
  ...new Set(spool.reports.map(report => report.assignmentRef)),
]

export const verificationFromProofPayload = (
  assignmentRef: string,
  payload: unknown,
  observedAt: string,
): AssignmentTokenUsageVerification => {
  const normalizedRef = normalizeAssignmentRef(assignmentRef)
  if (normalizedRef === null) {
    return {
      ok: false,
      assignmentRef,
      blockerRef: "blocker.desktop_token_accounting.assignment_ref_missing",
      observedAt,
    }
  }
  if (!isJsonObject(payload) || !isJsonObject(payload.tokenUsage)) {
    return {
      ok: false,
      assignmentRef: normalizedRef,
      blockerRef: "blocker.desktop_token_accounting.proof_unavailable",
      observedAt,
    }
  }

  const usage = payload.tokenUsage
  const rowCount = numberValue(usage.rowCount)
  const totalTokens = numberValue(usage.totalTokens)
  const exact =
    stringValue(usage.provider) === "pylon-codex-own-capacity" &&
    stringValue(usage.model) === "openagents/pylon-codex" &&
    stringValue(usage.usageTruth) === "exact" &&
    stringValue(usage.demandKind) === "own_capacity" &&
    stringValue(usage.demandSource) === "khala_coding_delegation"

  if (!exact || rowCount <= 0 || totalTokens <= 0) {
    return {
      ok: false,
      assignmentRef: normalizedRef,
      blockerRef: OPENAGENTS_DESKTOP_TOKEN_MISSING_ACCOUNTING_BLOCKER,
      observedAt,
    }
  }

  return {
    ok: true,
    assignmentRef: normalizedRef,
    cacheReadTokens: numberValue(usage.cacheReadTokens),
    demandKind: "own_capacity",
    demandSource: "khala_coding_delegation",
    inputTokens: numberValue(usage.inputTokens),
    model: "openagents/pylon-codex",
    observedAt,
    outputTokens: numberValue(usage.outputTokens),
    provider: "pylon-codex-own-capacity",
    reasoningTokens: numberValue(usage.reasoningTokens),
    rowCount,
    totalTokens,
    usageTruth: "exact",
  }
}
