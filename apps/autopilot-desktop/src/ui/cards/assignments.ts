// CL-50: "Assignments" — open work leases (read-only). STUB — implemented in
// CL-50. Owns this file only.
//
// Implementation notes for CL-50:
// - List `ctx.node?.assignments`; each row: goal (or trimmed ref), paymentMode,
//   expiry date if present, short ref. Subtitle "open work leases · read-only".

import type { PaneContext } from "../context"
import { card, emptyLine } from "../dom"

export function renderAssignmentsCard(container: HTMLElement, ctx: PaneContext): void {
  const assignments = ctx.node?.assignments ?? []
  if (assignments.length === 0) return // no open leases — render nothing
  const { section, body } = card(`Assignments (${assignments.length})`)
  body.append(emptyLine("Assignments UI — coming in CL-50."))
  container.append(section)
}
