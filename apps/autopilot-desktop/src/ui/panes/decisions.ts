// CL-56: "Decisions" pane — cross-session approvals/decisions queue. STUB —
// implemented in CL-56. Owns this file only.
//
// Implementation notes for CL-56:
// - Render the full queue from `ctx.node?.approvals` (self-contained; same data
//   the CL-48 dashboard card uses), each with context + Approve/Deny via
//   `ctx.request.resolveApproval`. Empty state when nothing needs the owner.
// - Cross-client consistency (CL-29): resolving clears here and elsewhere.

import type { PaneContext } from "../context"
import { emptyLine } from "../dom"

export function renderDecisionsPane(container: HTMLElement, ctx: PaneContext): void {
  const h = document.createElement("h1")
  h.className = "pane-title"
  h.textContent = "Decisions"
  container.append(h)
  const count = ctx.node?.approvals?.length ?? 0
  container.append(emptyLine(count > 0 ? `${count} pending — full queue coming in CL-56.` : "Nothing needs you right now."))
}
