import type { DecisionRecord, DecisionState, DecisionVerb } from "./decision.js"

export type ApprovalRow = Pick<DecisionRecord, "requestId" | "actionRef" | "state"> & {
  resolvedVerb: string | null
  availableVerbs: string[]
  expired: boolean
}

type RawRecord = Record<string, unknown>

const APPROVAL_VERBS: readonly DecisionVerb[] = ["approve", "deny", "answer"]

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function readString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined
}

function parseState(value: unknown): DecisionState {
  switch (value) {
    case "pending":
    case "resolved":
    case "cancelled":
    case "expired":
      return value
    default:
      return "expired"
  }
}

function parseResolvedVerb(value: unknown): string | null {
  return typeof value === "string" ? value : null
}

function projectApproval(record: RawRecord): ApprovalRow {
  const state = parseState(record.state)
  const expired = state === "expired"
  const availableVerbs = state === "pending" && !expired ? [...APPROVAL_VERBS] : []

  return {
    requestId: readString(record.requestId) ?? "",
    actionRef: readString(record.actionRef) ?? "",
    state,
    resolvedVerb: parseResolvedVerb(record.resolvedVerb),
    availableVerbs,
    expired,
  }
}

export function projectApprovals(records: unknown[]): ApprovalRow[] {
  if (!Array.isArray(records)) return []

  const rows: ApprovalRow[] = []

  for (const record of records) {
    if (!isRecord(record)) continue
    rows.push(projectApproval(record))
  }

  return rows
}
