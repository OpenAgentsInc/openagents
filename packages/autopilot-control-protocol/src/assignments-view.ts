export type AssignmentState =
  | "open"
  | "accepted"
  | "in_progress"
  | "done"
  | "unknown"

export type AssignmentRow = {
  leaseRef: string
  title: string
  state: AssignmentState
  rewardSats: number | null
  updatedAt: string
}

type RawRecord = Record<string, unknown>

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function readRecord(value: unknown, key: string): RawRecord | undefined {
  if (!isRecord(value)) return undefined
  const child = value[key]
  if (!isRecord(child)) return undefined
  return child
}

function readString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined
}

function readNumber(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined
}

function firstString(...values: unknown[]): string | undefined {
  for (const value of values) {
    const parsed = readString(value)
    if (parsed !== undefined) return parsed
  }

  return undefined
}

function firstNumber(...values: unknown[]): number | undefined {
  for (const value of values) {
    const parsed = readNumber(value)
    if (parsed !== undefined) return parsed
  }

  return undefined
}

function parseState(value: unknown): AssignmentState {
  const state = readString(value)?.toLowerCase()

  switch (state) {
    case "open":
    case "offered":
    case "pending":
    case "queued":
      return "open"
    case "accepted":
      return "accepted"
    case "in_progress":
    case "in-progress":
    case "active":
    case "running":
    case "working":
      return "in_progress"
    case "done":
    case "complete":
    case "completed":
    case "closed":
    case "settled":
    case "succeeded":
      return "done"
    default:
      return "unknown"
  }
}

function parseRewardSats(row: RawRecord): number | null {
  const reward = readRecord(row, "reward")
  const payout = readRecord(row, "payout")
  const payment = readRecord(row, "payment")
  const amount = firstNumber(
    row.rewardSats,
    row.reward_sats,
    row.bountySats,
    row.bounty_sats,
    row.amountSats,
    row.amount_sats,
    row.payoutSats,
    row.payout_sats,
    reward?.sats,
    payout?.sats,
    payment?.sats,
  )

  return amount === undefined ? null : amount
}

function projectAssignment(row: RawRecord): AssignmentRow {
  const assignment = readRecord(row, "assignment")
  const leaseRef = firstString(
    row.leaseRef,
    row.lease_ref,
    row.ref,
    row.id,
    assignment?.leaseRef,
    assignment?.lease_ref,
  ) ?? ""
  const title = firstString(
    row.title,
    row.assignmentTitle,
    row.assignment_title,
    row.name,
    row.summary,
    row.objective,
    assignment?.title,
    assignment?.name,
    assignment?.summary,
    assignment?.objective,
  ) ?? leaseRef
  const updatedAt = firstString(
    row.updatedAt,
    row.updated_at,
    row.modifiedAt,
    row.modified_at,
    row.createdAt,
    row.created_at,
    assignment?.updatedAt,
    assignment?.updated_at,
  ) ?? ""

  return {
    leaseRef,
    title,
    state: parseState(firstString(row.state, row.status, assignment?.state, assignment?.status)),
    rewardSats: parseRewardSats(row),
    updatedAt,
  }
}

export function projectAssignments(rawLeases: unknown): AssignmentRow[] {
  if (!Array.isArray(rawLeases)) return []

  const rows: AssignmentRow[] = []

  for (const rawLease of rawLeases) {
    if (!isRecord(rawLease)) continue
    rows.push(projectAssignment(rawLease))
  }

  return rows
}
