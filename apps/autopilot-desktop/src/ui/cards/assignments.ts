// CL-50: "Assignments" — open work leases (read-only). Owns this file only.
//
// Renders a card listing active work-lease assignments on the connected node.
// Read-only: no mutation. No pane wiring needed.

import type { AssignmentRow } from "../../shared/rpc"
import type { PaneContext } from "../context"
import { card, escapeHtml } from "../dom"

// Pure helper: derive display strings for a single assignment row.
// Exported for unit-testing.
export function assignmentMeta(row: AssignmentRow): { goal: string; meta: string } {
  const goal = row.goal.trim() !== "" ? row.goal : row.assignmentRef.slice(-8)

  const datePart = row.expiresAt ? ` · expires ${row.expiresAt.slice(0, 10)}` : ""
  const refSuffix = row.assignmentRef.slice(-6)
  const meta = `${row.paymentMode}${datePart} · ${refSuffix}`

  return { goal, meta }
}

export function renderAssignmentsCard(container: HTMLElement, ctx: PaneContext): void {
  const assignments = ctx.node?.assignments ?? []
  if (assignments.length === 0) return // no open leases — render nothing

  const { section, body } = card(`Assignments (${assignments.length})`)

  // Subtitle line
  const subtitle = document.createElement("p")
  subtitle.className = "card-subtitle"
  subtitle.textContent = "open work leases · read-only"
  body.append(subtitle)

  // One row per assignment
  for (const row of assignments) {
    const { goal, meta } = assignmentMeta(row)

    const item = document.createElement("div")
    item.className = "assignment-row"

    const goalEl = document.createElement("div")
    goalEl.className = "assignment-goal"
    goalEl.innerHTML = escapeHtml(goal)

    const metaEl = document.createElement("div")
    metaEl.className = "assignment-meta"
    metaEl.innerHTML = escapeHtml(meta)

    item.append(goalEl, metaEl)
    body.append(item)
  }

  container.append(section)
}
