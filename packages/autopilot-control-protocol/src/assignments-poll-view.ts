import type { AssignmentRow } from "./assignments-view.js"

type AssignmentsPollSummary = {
  open: number
  accepted: number
  inProgress: number
  done: number
  totalRewardSats: number
}

const emptySummary = (): AssignmentsPollSummary => ({
  open: 0,
  accepted: 0,
  inProgress: 0,
  done: 0,
  totalRewardSats: 0,
})

function isAssignmentRow(value: unknown): value is AssignmentRow {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function readRewardSats(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    return 0
  }

  return value
}

export function summarizeAssignments(rows: any[]): AssignmentsPollSummary {
  const summary = emptySummary()

  if (!Array.isArray(rows)) return summary

  for (const row of rows) {
    if (!isAssignmentRow(row)) continue

    switch (row.state) {
      case "open":
        summary.open += 1
        break
      case "accepted":
        summary.accepted += 1
        break
      case "in_progress":
        summary.inProgress += 1
        break
      case "done":
        summary.done += 1
        break
    }

    summary.totalRewardSats += readRewardSats(row.rewardSats)
  }

  return summary
}
