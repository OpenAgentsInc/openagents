import type { AssignmentState } from "./assignments-view.js"

type RawRecord = Record<string, unknown>

type AssignmentDetailState =
  | Extract<AssignmentState, "open" | "accepted" | "unknown">
  | "claimed"
  | "submitted"
  | "rejected"

export type AssignmentDetailView = {
  assignmentRef: string
  title: string
  state: AssignmentDetailState
  rewardSats: number | null
  claimedByHash: string | null
}

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function readRecord(value: unknown, key: string): RawRecord | undefined {
  if (!isRecord(value)) return undefined
  if (!isRecord(value[key])) return undefined
  return value[key]
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

function parseState(value: unknown): AssignmentDetailState {
  const state = readString(value)?.toLowerCase()

  switch (state) {
    case "open":
    case "offered":
    case "pending":
    case "queued":
      return "open"
    case "claimed":
    case "claim_accepted":
    case "claim-accepted":
    case "in_progress":
    case "in-progress":
    case "active":
    case "running":
    case "working":
      return "claimed"
    case "submitted":
    case "done":
    case "complete":
    case "completed":
    case "ready_for_review":
    case "ready-for-review":
      return "submitted"
    case "accepted":
    case "closed":
    case "settled":
    case "succeeded":
      return "accepted"
    case "rejected":
    case "failed":
    case "declined":
      return "rejected"
    default:
      return "unknown"
  }
}

function parseRewardSats(row: RawRecord): number | null {
  const assignment = readRecord(row, "assignment")
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
    assignment?.rewardSats,
    assignment?.reward_sats,
    reward?.sats,
    payout?.sats,
    payment?.sats,
  )

  return amount === undefined ? null : amount
}

export function projectAssignmentDetail(raw: unknown): AssignmentDetailView {
  if (!isRecord(raw)) {
    return {
      assignmentRef: "",
      title: "",
      state: "unknown",
      rewardSats: null,
      claimedByHash: null,
    }
  }

  const assignment = readRecord(raw, "assignment")
  const claim = readRecord(raw, "claim")
  const assignee = readRecord(raw, "assignee")
  const claimant = readRecord(raw, "claimant")
  const assignmentRef = firstString(
    raw.assignmentRef,
    raw.assignment_ref,
    raw.leaseRef,
    raw.lease_ref,
    raw.ref,
    raw.id,
    assignment?.assignmentRef,
    assignment?.assignment_ref,
    assignment?.leaseRef,
    assignment?.lease_ref,
    assignment?.ref,
    assignment?.id,
  ) ?? ""
  const title = firstString(
    raw.title,
    raw.assignmentTitle,
    raw.assignment_title,
    raw.name,
    raw.summary,
    raw.objective,
    assignment?.title,
    assignment?.name,
    assignment?.summary,
    assignment?.objective,
  ) ?? assignmentRef
  const claimedByHash = firstString(
    raw.claimedByHash,
    raw.claimed_by_hash,
    raw.agentHash,
    raw.agent_hash,
    raw.ownerHash,
    raw.owner_hash,
    claim?.claimedByHash,
    claim?.claimed_by_hash,
    claim?.agentHash,
    claim?.agent_hash,
    assignee?.hash,
    assignee?.agentHash,
    assignee?.agent_hash,
    claimant?.hash,
    claimant?.agentHash,
    claimant?.agent_hash,
  ) ?? null

  return {
    assignmentRef,
    title,
    state: parseState(firstString(
      raw.state,
      raw.status,
      assignment?.state,
      assignment?.status,
      claim?.state,
      claim?.status,
    )),
    rewardSats: parseRewardSats(raw),
    claimedByHash,
  }
}
