import { projectAssignmentDetail } from "./assignment-detail-view.js"

type RawRecord = Record<string, unknown>

export type AssignmentListView = {
  open: {
    ref: string
    title: string
    rewardSats: number | null
  }[]
  claimed: {
    ref: string
    title: string
  }[]
  counts: {
    open: number
    claimed: number
    total: number
  }
}

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function readRows(raw: unknown): unknown[] {
  if (Array.isArray(raw)) return raw
  if (!isRecord(raw)) return []

  if (Array.isArray(raw.assignments)) return raw.assignments
  if (Array.isArray(raw.leases)) return raw.leases
  if (Array.isArray(raw.items)) return raw.items
  if (Array.isArray(raw.rows)) return raw.rows

  return [
    ...(Array.isArray(raw.open) ? raw.open : []),
    ...(Array.isArray(raw.claimed) ? raw.claimed : []),
  ]
}

export function projectAssignmentList(raw: unknown): AssignmentListView {
  const open: AssignmentListView["open"] = []
  const claimed: AssignmentListView["claimed"] = []

  for (const row of readRows(raw)) {
    if (!isRecord(row)) continue

    const assignment = projectAssignmentDetail(row)

    if (assignment.state === "open") {
      open.push({
        ref: assignment.assignmentRef,
        title: assignment.title,
        rewardSats: assignment.rewardSats,
      })
      continue
    }

    if (assignment.state === "claimed") {
      claimed.push({
        ref: assignment.assignmentRef,
        title: assignment.title,
      })
    }
  }

  return {
    open,
    claimed,
    counts: {
      open: open.length,
      claimed: claimed.length,
      total: open.length + claimed.length,
    },
  }
}
