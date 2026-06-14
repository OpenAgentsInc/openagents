// CL-48: "Needs you" pending approvals (approve / deny). STUB — implemented in
// CL-48. Owns this file only.
//
// Implementation notes for CL-48:
// - List `ctx.node?.approvals`; each row shows prompt/kind + Approve/Deny.
// - Resolve via `ctx.request.resolveApproval({ approvalRef, decision })`;
//   optimistically remove the row, then `ctx.refresh()`.
// - Projection only — never render raw secrets/paths.

import type { PaneContext } from "../context"
import { card, emptyLine } from "../dom"

export function renderApprovalsCard(container: HTMLElement, ctx: PaneContext): void {
  const approvals = ctx.node?.approvals ?? []
  if (approvals.length === 0) return // nothing needs the owner — render nothing
  const { section, body } = card(`Needs you (${approvals.length})`)
  body.append(emptyLine("Approvals UI — coming in CL-48."))
  container.append(section)
}
