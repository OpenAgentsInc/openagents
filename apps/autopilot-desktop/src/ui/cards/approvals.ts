// CL-48: "Needs you" pending approvals card (approve / deny).
//
// Renders only when there are pending approvals. Each row shows the approval's
// prompt text (or falls back to the kind when prompt is empty) with Approve and
// Deny buttons. Clicking either button optimistically removes the row from the
// DOM and calls ctx.request.resolveApproval; if the result is neither applied
// nor duplicate (i.e. something unexpected happened), ctx.refresh() resyncs.
//
// No raw secrets or file-system paths are ever rendered — only prompt/kind.

import type { ApprovalRow } from "../../shared/rpc"
import type { PaneContext } from "../context"
import { card, escapeHtml } from "../dom"

// Pure helper: returns the display label for an approval row.
// Uses prompt when non-empty, falls back to kind.
export function approvalLabel(row: Pick<ApprovalRow, "prompt" | "kind">): string {
  return row.prompt.trim() !== "" ? row.prompt : row.kind
}

export function renderApprovalsCard(container: HTMLElement, ctx: PaneContext): void {
  const approvals = ctx.node?.approvals ?? []
  if (approvals.length === 0) return // nothing needs the owner — render nothing

  const { section, body } = card(`Needs you (${approvals.length})`)

  for (const approval of approvals) {
    const row = document.createElement("div")
    row.className = "approval-row"
    row.style.cssText = "display:flex;align-items:flex-start;justify-content:space-between;gap:8px;padding:6px 0;border-bottom:1px solid rgba(255,255,255,0.06);"

    const label = document.createElement("p")
    label.className = "approval-prompt"
    label.style.cssText = "margin:0;flex:1;font-size:0.85rem;line-height:1.4;word-break:break-word;"
    // escapeHtml then assign via innerHTML so special chars render safely.
    label.innerHTML = escapeHtml(approvalLabel(approval))

    const buttons = document.createElement("div")
    buttons.className = "approval-buttons"
    buttons.style.cssText = "display:flex;gap:6px;flex-shrink:0;"

    const approveBtn = document.createElement("button")
    approveBtn.textContent = "Approve"
    approveBtn.style.cssText =
      "padding:3px 10px;border:none;border-radius:4px;font-size:0.78rem;cursor:pointer;background:#5ee08a;color:#111;font-weight:600;"

    const denyBtn = document.createElement("button")
    denyBtn.textContent = "Deny"
    denyBtn.style.cssText =
      "padding:3px 10px;border:none;border-radius:4px;font-size:0.78rem;cursor:pointer;background:#ff6b6b;color:#111;font-weight:600;"

    const resolve = (decision: "approve" | "deny") => {
      // Optimistic removal — hide the row immediately.
      row.remove()
      void ctx.request
        .resolveApproval({ approvalRef: approval.approvalRef, decision })
        .then((result) => {
          // If neither applied nor duplicate, the node returned something
          // unexpected; re-fetch to get an accurate view.
          if (!result.applied && !result.duplicate) {
            ctx.refresh()
          }
        })
        .catch(() => {
          // Network error — resync so the row reappears if still pending.
          ctx.refresh()
        })
    }

    approveBtn.addEventListener("click", () => resolve("approve"))
    denyBtn.addEventListener("click", () => resolve("deny"))

    buttons.append(approveBtn, denyBtn)
    row.append(label, buttons)
    body.append(row)
  }

  container.append(section)
}
